# DragonGUI Optimization Summary

Date: 2026-05-21

## Scope

This note summarizes the recent performance work committed through
`95482db optimizations`, with the pre-optimization reference point around
`78ca5df`. It focuses on the runtime, renderer, scatter streaming, primitive
drawing, and LinePlot streaming work.

The short version: we moved hot paths away from Python object churn and
per-primitive general shaders, kept high-rate streams latest-frame/coalesced,
and added renderer diagnostics so bottlenecks can be identified instead of
guessed.

## Main Outcomes

- Scatter3D streaming now has a direct GPU-shaped payload path for large point
  clouds.
- Native scatter uploads can skip decode and bounds scanning when prepared
  payloads provide GPU-ready instances and bounds.
- The native command queue coalesces stale scatter and line plot updates so
  high-rate producers do not force every obsolete frame through the renderer.
- The primitive renderer can split dense simple rectangles and line capsules
  into cheaper specialized pipelines.
- LinePlot has a dedicated GPU renderer for the line series instead of emitting
  every line segment as a general primitive rectangle.
- LinePlot streaming can send packed XY bytes and append incrementally in native
  state.
- Debug snapshots now expose the timings and counts needed to distinguish
  Python handoff, native command application, primitive rebuild, GPU upload, and
  render encoding costs.

## Scatter3D Streaming

### Prepared payload path

The scatter streaming work separated point preparation from UI update:

- `Scatter3D.prepare_points(...)` can create a reusable `ScatterPayload`.
- Prepared payloads carry optional bounds.
- `ScatterLiveFrame.replace_prepared(...)` updates the live scatter from a
  prepared payload.
- `ScatterLiveFrame.enqueue_prepared(...)` can send a prepared payload directly
  to the native command queue from the producer thread.

This matters for LiDAR-style streams where the producer already has frame data
and the UI should display the latest full point cloud.

### GPU-shaped instance payloads

The `point_instance_v1` path lets Python hand native a GPU-shaped point buffer.
When bounds are also supplied:

- native decode is `0 ms`;
- native bounds scan is `0 ms`;
- native mostly just uploads the point buffer to the GPU.

The compact `xyz` payload path is still useful when minimizing wire size, but
native must expand/colorize those records before upload.

### Command queue coalescing

The runtime now coalesces high-rate update commands:

- `SetScatterPointsPacked` keeps only the latest coalescable update per scatter.
- repeated Python task drain wakeups are coalesced so producer threads do not
  flood the native command queue with duplicate drain commands.
- adjacent compatible `AppendLinePlotPointsPacked` commands are merged.

This preserves latest-frame semantics under load. It also avoids measuring
obsolete frames as useful work.

### Direct handoff benchmark path

`benchmarking/scatter_stream_compare.py` added modes to isolate the real cost:

- workload: `generate`, `prebuilt-frames`, `prebuilt-payloads`;
- payload: `xyz`, `instances`;
- producer: `flood`, `paced`;
- handoff: `callback`, `direct`.

Useful command:

```powershell
python benchmarking\scatter_stream_compare.py --backend dragongui --points 1000000 --target-hz 240 --producer-mode paced --dragongui-handoff direct --dragongui-update-mode live-frame --dragongui-workload prebuilt-payloads --dragongui-payload-format instances
```

Representative 1M point findings from `benchmarking/scatter_stream_handoff.md`:

| Mode | Result |
| --- | --- |
| `point_instance_v1`, prebuilt payloads | native decode and bounds dropped to `0 ms` |
| latest optimized native upload | roughly `2.5-3.2 ms` per 1M instance upload on the tested desktop |
| paced callback handoff | about `55` acknowledged native uploads/sec |
| paced direct handoff | about `65` acknowledged native uploads/sec |
| direct flood handoff | about `69` native 1M-point uploads/sec with coalescing |

The remaining paced limiter is mostly acknowledgement/handoff overhead, not the
native upload itself.

### Hover metadata split

Primary scatter hover data was split so metadata can be sent separately from
the point buffer:

- `SetScatterPrimaryHoverColumns` sends typed hover columns.
- `SetScatterPrimaryHoverMeta` remains available for string metadata.
- startup timing now records hover column extraction/enqueue costs separately.

This keeps point uploads focused on render data and makes hover tooltip cost
visible in diagnostics.

## Primitive Renderer Split

The original primitive renderer sent every rectangle-like draw through one
large, general-purpose instance format and WGSL shader. Dense UI scenes paid
for gradient, shadow, transform, and effect fields even when drawing simple
solid rounded rectangles.

The renderer now classifies primitives into:

- `SimpleRectInstance`: solid axis-aligned rectangles with radii and clip.
- `LineSegmentInstance`: solid transformed capsules used by fallback line
  segment drawing.
- full `RectInstance`: complex primitives such as gradients, shadows, outlines,
  transforms, and other featureful draws.

New shaders:

- `native/src/primitives/simple_rect.wgsl`
- `native/src/primitives/line_segment.wgsl`
- existing `native/src/primitives/rect.wgsl` remains the complex path

The renderer keeps base and overlay ordering by building batches per pipeline.
A pressure guard collapses back to the complex path when splitting would create
too many small batches.

Runtime diagnostics now report:

- split enabled/collapsed state;
- source/simple/line/complex counts;
- batch counts by pipeline;
- source and uploaded byte counts;
- emit, split, and upload timings.

Toggle:

```powershell
$env:DRAGONGUI_PRIMITIVE_SPLIT='0'  # disable split path for comparison
```

Probe:

```powershell
python examples\css_feature_probes\primitive_benchmark_probe.py
```

Useful probe env vars:

- `DRAGONGUI_PRIMITIVE_BENCH_CELLS`
- `DRAGONGUI_PRIMITIVE_BENCH_CELL_SIZE`
- `DRAGONGUI_PRIMITIVE_BENCH_GAP`
- `DRAGONGUI_PRIMITIVE_BENCH_MODE=solid|rounded|outline|complex`

## LinePlot Streaming And Rendering

### Packed line data

LinePlot can now use packed XY data instead of pushing large Python object
trees for every update.

Native commands:

- `SetLinePlotDataPacked`
- `AppendLinePlotPointsPacked`
- `ClearLinePlotSeries`

Python handles:

- `LinePlot.set_data(...)`
- `LinePlot.append_points(...)`
- `LinePlot.clear_series(...)`
- `LinePlot.set_window_size(...)`

The packed wire format is little-endian `f32` XY pairs.

### Retained native series state

Native `LinePlotSeriesProp` now stores extra metadata:

- cached bounds;
- x-sorted flag;
- payload format;
- logical front offset;
- y-range blocks.

The front offset lets streaming trims advance a logical start pointer without
draining the front of the vector every frame. Compaction happens later when the
offset becomes large enough.

Y-range blocks let visible-range and trim-bound calculations reuse cached block
min/max values instead of rescanning all points.

### Windowing and max-points behavior

Streaming line plots can keep a bounded point history:

- `max_points` trims retained history;
- `window_size` applies a moving x-axis window;
- native metrics report last trim/window/count costs.

This is important for long-running telemetry streams where total retained
points would otherwise grow forever.

### Dedicated GPU line renderer

LinePlot series rendering moved from "one primitive capsule per segment" to a
dedicated renderer:

- `native/src/primitives/line_plot.wgsl`
- storage buffer for packed line points;
- per-series vertex instances;
- shader expands each segment into screen-space triangles;
- built-in anti-aliasing width;
- dashed, dotted, and dash-dot style support;
- decimation before upload when the source point count exceeds the useful
  screen-space detail.

Runtime toggles:

```powershell
$env:DRAGONGUI_LINE_PLOT_RENDERER='0'       # disable dedicated renderer
$env:DRAGONGUI_LINE_PLOT_AA_WIDTH='1.0'     # default, clamped 0.5..2.5
$env:DRAGONGUI_LINE_PLOT_MAX_SEGMENTS='8192'
$env:DRAGONGUI_LINE_PLOT_DECIMATION='auto'  # auto, extrema, off
```

The old primitive-based line path remains as fallback for unsupported styles or
when the dedicated renderer is disabled.

### Decimation modes

LinePlot rendering now has two useful fast paths:

- `auto`: stride-based reduction to a screen-sized point budget.
- `extrema`: bucket min/max preservation for plots where peaks matter.

The renderer reports source points, rendered points, segment count, decimated
series count, decimation time, emit time, and upload time in the debug snapshot.

Probe:

```powershell
python examples\css_feature_probes\line_plot_stream_benchmark_probe.py
```

Useful probe env vars:

- `DRAGONGUI_LINE_BENCH_INITIAL_POINTS`
- `DRAGONGUI_LINE_BENCH_BATCH_POINTS`
- `DRAGONGUI_LINE_BENCH_BATCHES`
- `DRAGONGUI_LINE_BENCH_MAX_POINTS`
- `DRAGONGUI_LINE_BENCH_WINDOW_SIZE`
- `DRAGONGUI_LINE_BENCH_MODE=widget|packed`

## Runtime And Diagnostics

The debug snapshot became a practical profiling surface. Relevant additions:

- per-command timing via `command_timings`;
- command drain fetch/coalesce/apply/flush timing;
- primitive renderer split stats;
- primitive base/overlay encode timings;
- dedicated line renderer stats;
- line plot data metrics per widget;
- scatter upload metrics and queue depth;
- startup resource timing for expensive widget resources.

These metrics were used repeatedly to decide the next target. For example:

- primitive bottlenecks were verified through split stats and primitive probe
  timings;
- line plot bottlenecks were separated into native append, primitive rebuild,
  line renderer decimation/upload, and frame timings;
- scatter streaming was separated into producer generation, Python packing,
  UI handoff, native decode, native bounds, native upload, and presentation.

## Benchmark And Probe Files

Primary benchmark files:

- `benchmarking/scatter_stream_compare.py`
- `benchmarking/scatter_stream_handoff.md`
- `benchmarking/last_scatter_compare.json`
- `examples/css_feature_probes/primitive_benchmark_probe.py`
- `examples/css_feature_probes/line_plot_stream_benchmark_probe.py`

The CSS feature probe README now lists the primitive and line plot benchmark
probes.

## Verification Added

The optimization work added or expanded tests around:

- scatter prepared payloads and live-frame direct handoff;
- scatter hover metadata replay and clearing;
- command queue coalescing;
- line plot packed data, appends, clear behavior, and window size;
- line plot cached y-block bounds and front-offset logic;
- primitive split classification and collapse guard;
- line plot decimation behavior.

Common verification commands used during the work:

```powershell
cargo check --manifest-path native\Cargo.toml --target x86_64-pc-windows-gnu
cargo test --manifest-path native\Cargo.toml --target x86_64-pc-windows-gnu scatter --lib
cargo test --manifest-path native\Cargo.toml --target x86_64-pc-windows-gnu line_plot --lib
python -m pytest tests\test_python_api.py -q -k "scatter or line_plot"
```

## Remaining Optimization Targets

### Scatter

- A cheaper acknowledgement mechanism than polling `debug_snapshot()` would make
  paced streaming measurements more representative.
- A lower-copy Python-to-native buffer path could reduce handoff overhead for
  large `point_instance_v1` payloads.
- Real sensor/LiDAR frames should be measured separately from synthetic frame
  generation and packing.

### LinePlot

- The dedicated renderer still rebuilds/uploads renderer buffers after native
  data changes; persistent/incremental GPU buffer updates may help very large
  long-running streams.
- More visual tuning may be worthwhile for peak-heavy signals, especially when
  choosing between `auto` and `extrema` decimation.
- Benchmark snapshots should be saved to JSON the way scatter benchmarks are,
  so before/after comparisons are easier to preserve.

### Primitive Renderer

- The split path is useful for dense simple UI, but very fragmented scenes can
  still collapse back to the complex path. More batch compaction or sort-safe
  grouping could help if ordering constraints allow it.
- Raspberry Pi should be re-benchmarked with the split path because that was
  the original hardware where primitive cost stood out.

## Practical Guidance

For high-rate Scatter3D:

- prefer prepared payloads;
- prefer `point_instance_v1` when the producer can provide GPU-ready records;
- include bounds when possible;
- use direct handoff for producer-thread streaming;
- use latest-frame/coalesced semantics unless every frame must be shown.

For high-rate LinePlot:

- use packed appends;
- set `max_points` and/or `window_size`;
- leave the dedicated renderer enabled;
- start with `DRAGONGUI_LINE_PLOT_DECIMATION=auto`;
- switch to `extrema` when preserving spikes matters more than exact point
  density.

For primitive-heavy UI:

- leave `DRAGONGUI_PRIMITIVE_SPLIT` enabled;
- compare with the primitive benchmark probe before changing shader or layout
  strategy;
- watch split/collapse and batch counts in the debug snapshot.
