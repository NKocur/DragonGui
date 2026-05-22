# DragonGUI Benchmark Audit

Date: 2026-05-22

This is a first-pass performance audit across DragonGUI's native CPU prep paths
and selected real windowed renderer probes. The goal is to identify where time is
currently spent before making broader optimization changes.

## Native Microbenchmarks

These benchmarks run as ignored Rust tests in release mode:

```powershell
$env:PYO3_PYTHON='C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe'
cargo test --release --manifest-path native\Cargo.toml bench_layout_many_controls --lib -- --ignored --nocapture
cargo test --release --manifest-path native\Cargo.toml bench_css_cascade_many_widgets --lib -- --ignored --nocapture
cargo test --release --manifest-path native\Cargo.toml bench_text_collect_many_labels --lib -- --ignored --nocapture
cargo test --release --manifest-path native\Cargo.toml bench_progress_bar_primitive_emit --lib -- --ignored --nocapture
cargo test --release --manifest-path native\Cargo.toml bench_table_primitive_emit --lib -- --ignored --nocapture
cargo test --release --manifest-path native\Cargo.toml bench_table_text_collect --lib -- --ignored --nocapture
cargo test --release --manifest-path native\Cargo.toml bench_line_plot_render_data_collect --lib -- --ignored --nocapture
cargo test --release --manifest-path native\Cargo.toml bench_scatter_xyz_decode_colormap --lib -- --ignored --nocapture
```

Baseline results on this machine:

| Area | Scenario | Result |
| --- | --- | --- |
| Layout | 2,000 mixed controls in a flow layout | 2,908 ns/widget |
| CSS cascade | 2,000 mixed widgets with type, class, part, and child selectors | 3,803 ns/widget pure cascade; 10,602 ns/widget clone + cascade in the latest run; original clone + cascade baseline was 15,148 ns/widget |
| CSS cascade, simple stylesheet | 2,000 widgets without ancestor selectors | 2,534 ns/widget with ancestor snapshot gating versus 3,347 ns/widget with forced ancestor snapshots |
| CSS cascade, large stylesheet | 2,000 widgets with 400 extra non-matching rules | 3,233 ns/widget with compiled rule buckets and scratch reuse versus 6,641 ns/widget with forced linear scan |
| Text | 2,000 labels | 963 ns/label |
| ProgressBar primitives | 10,000 progress bars | 547 ns/bar |
| DataFrameTable primitives | 270 visible cells | 0.862 us/frame, 3.2 ns/visible cell |
| DataFrameTable text | 270 visible cells | 138.8 us/frame, 514 ns/visible cell |
| LinePlot render data | 4 series x 100,000 points | 0.26 ns/source point, 9,308 emitted points/frame |
| Scatter3D xyz decode | 500,000 compact xyz points | 9.023 ms without bounds hint, 4.938 ms with bounds hint, 1.83x faster |

## Windowed Renderer Probes

These open short-lived native windows and use debug snapshots from the live
renderer.

```powershell
$env:DRAGONGUI_SMOKE_FRAMES='80'
$env:DRAGONGUI_PRIMITIVE_BENCH_CELLS='3600'
py -3.11 examples\css_feature_probes\primitive_benchmark_probe.py

$env:DRAGONGUI_SMOKE_FRAMES='100'
$env:DRAGONGUI_BENCH_POINTS='500000'
py -3.11 benches\bench_scatter.py

$env:DRAGONGUI_SMOKE_FRAMES='180'
$env:DRAGONGUI_BENCH_AUTOSTART='1'
$env:DRAGONGUI_LINE_BENCH_BATCHES='80'
py -3.11 examples\css_feature_probes\line_plot_stream_benchmark_probe.py
```

Baseline and follow-up results:

| Area | Scenario | Result |
| --- | --- | --- |
| Primitive renderer | 3,600 rounded panels, 3,135 visible rects | 2.55 ms emit, 0.04 ms upload, 0.006 ms encode |
| Scatter3D baseline | 500,000 packed xyz points | 1.75 ms Python pack, 12.09 ms native decode, 3.34 ms upload, 16.11 ms/frame |
| Scatter3D compact GPU path | 500,000 packed xyz points with bounds | 2.69 ms Python pack, 0.00 ms native decode, 0.88 ms upload, 0.90 ms native update, 15.14 ms/frame |
| LinePlot stream | 80 appends x 256 points, 4,097 visible points | 0.018 ms append avg, 0.998 ms native update, 0.024 ms decimate, 0.028 ms emit, 16.32 ms/frame fifo-presented |

## Bottlenecks

1. CSS cascade remains the highest per-widget CPU cost in the synthetic native
   microbenchmarks, though the first optimization pass reduced the mixed-selector
   case by about 28%. The main fixes were precomputed target filters and skipping
   attribute/sibling snapshot construction when the active stylesheet does not
   contain selectors that need them.

   Follow-up benchmark split:

   ```text
   pure cascade:     3,803 ns/widget
   clone + cascade: 10,602 ns/widget
   ```

   A first compiled-style layer now builds rule buckets after parsing and uses
   them for larger stylesheets. In a synthetic stylesheet with 400 extra
   non-matching rules, pure cascade dropped from 6,641 ns/widget with forced
   linear scanning to 3,233 ns/widget with bucketed candidates and reused
   cascade scratch buffers.

   For stylesheets without child/descendant ancestor selectors, cascade now
   skips ancestor snapshot and `StyleAncestor` view construction. The simple
   stylesheet benchmark dropped from 3,347 ns/widget with forced ancestor
   snapshots to 2,534 ns/widget with the gated path.

2. Dense primitive scenes are dominated by CPU primitive emission, not upload or
   command encoding. In the 3,600-panel windowed probe, primitive emit was about
   2.55 ms while upload and encode together were under 0.05 ms.

3. DataFrameTable's visible cell chrome is cheap. Text collection is the current
   table hot path: 138.8 us/frame for 270 visible cells versus 0.862 us/frame
   for table primitives.

4. Scatter3D's single-frame update cost was dominated by native decode,
   followed by upload. For the original 500k compact xyz live probe, decode was
   12.09 ms and upload was 3.34 ms. The first follow-up decode microbenchmark
   showed the bounds-hinted `xyz_f32_v0` CPU path reducing decode from 9.023 ms
   to 4.938 ms for the same point count. The second follow-up added a compact
   GPU vertex path for bounds-bearing `xyz_f32_v0` payloads; the windowed
   benchmark then measured 0.00 ms native decode, 0.88 ms upload, and 0.90 ms
   total native update.

5. LinePlot's dedicated renderer is not a bottleneck in the tested streaming
   case. Decimation, emit, and upload were all well below 0.1 ms; the observed
   16 ms frame time was presentation-limited by fifo vsync.

## Added Coverage

- `native/src/layout.rs`: `bench_layout_many_controls`
- `native/src/css_style.rs`: `bench_css_cascade_many_widgets`
- `native/src/text/mod.rs`: `bench_text_collect_many_labels`
- `native/src/primitives/mod.rs`: `bench_line_plot_render_data_collect`

Existing focused benchmarks now cover:

- ProgressBar primitive emission
- DataFrameTable primitive emission
- DataFrameTable text collection
- Dense primitive windowed rendering
- Scatter3D packed payload update and frame timing
- LinePlot streaming update and renderer timing

## Next Optimization Targets

1. CSS cascade: keep the current parser, continue expanding the compiled
   runtime layer, and target per-node scratch allocation plus declaration
   application costs now that large-rule candidate filtering is in place.

2. Primitive emission: profile `emit_rects_inner` on dense simple-panel scenes,
   then target style lookup and per-node traversal overhead before touching GPU
   upload paths.

3. Scatter3D streaming: the compact GPU path removes native decode as the
   primary bottleneck for default z-colormapped xyz frames. Further work should
   focus on Python-side packing, optional direct NumPy buffer submission, and
   keeping the compact path active for benchmark/probe callers that bypass
   `Scatter3D.prepare_points`.

4. Table text: reduce repeated per-cell text option/style lookup and buffer key
   churn in the virtualized visible-cell path.
