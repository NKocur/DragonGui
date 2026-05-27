# Raspberry Pi Fullscreen Render Performance Audit

Last updated: 2026-05-18

## Problem

The Pi build feels smooth at a smaller window size but gets laggy when the
window is expanded to the screen. This happens even before plotting points, so
the first suspect is the whole renderer's per-frame cost at larger surface
sizes, not only Scatter3D point rendering.

Scatter3D still matters, especially at 150k-200k points, but the benchmark plan
must separate these cases:

- Empty or mostly static GUI at small size versus fullscreen.
- Static GUI with no plot data versus GUI with plot chrome only.
- Scatter3D with no points versus 150k-200k points.
- Idle frame cost versus interaction frame cost.
- CPU-side rebuild/upload cost versus GPU-side surface/fill cost.

## Code Findings

### Fullscreen Increases Base Surface Work

`native/src/runtime.rs` renders the window in full-surface passes:

- Pass 1: base primitives and images.
- Pass 2: each visible Scatter3D widget.
- Pass 3: text and overlay primitives.

Even when Scatter3D has no points, a larger window means larger clears, larger
primitive/text coverage, larger depth texture, and larger present/compositor
work. The current `runtime` debug snapshot already exposes:

- `runtime.frame_work_ms`
- `runtime.frame_prepare_ms`
- `runtime.frame_acquire_ms`
- `runtime.frame_encode_ms`
- `runtime.frame_submit_ms`
- `runtime.frame_present_ms`
- `runtime.wall_fps`
- `gpu.performance.last_primitive_rebuild_ms`
- `gpu.performance.last_text_rebuild_ms`
- `gpu.performance.last_primitive_instance_count`
- `gpu.performance.last_text_entry_count`

These are enough to start isolating whether fullscreen lag is CPU rebuild,
encode/submit/present, or too many redraws.

### Scatter3D Pi Render Scale Only Applies During Interaction

`native/src/runtime_profile.rs` sets Pi Scatter3D interactive render scale to
`0.75`, but `native/src/scatter/mod.rs` only returns that lower scale when
`lod_active` is true:

```rust
pub fn active_render_scale(&self) -> f32 {
    if self.lod_active {
        ...
    } else {
        1.0
    }
}
```

`lod_active` is enabled by scatter pan/orbit interaction paths in
`native/src/runtime.rs`. At rest, Scatter3D renders at 1.0 scale on Pi, even
fullscreen. That is good for quality, but it means idle fullscreen can still be
expensive.

### Scatter3D Uses Screen-Space Quads

`native/src/scatter/points.wgsl` renders each point as a 4-vertex billboard quad.
Circle and gaussian styles do fragment work and discard fragments. Fullscreen
can become fill-rate bound because:

- More pixels are shaded.
- Larger point sizes shade more fragments per point.
- The clip mask and rounded-clip checks run in the point shader.
- The widget may be larger, so the point-size auto-scaler can keep points larger
than it would in a smaller dense viewport.

The current fastest WASP path is compact `xyz` payload. That solved Python/Rust
packing overhead, but it does not reduce fullscreen fragment cost.

### Primitives Are Powerful But Heavy

`native/src/primitives/rect.wgsl` is a general rectangle shader. It supports
rounded corners, gradients, transforms, shadows/effects, clipping, and multiple
color stops through one broad instance format. That is flexible, but on Pi a
large fullscreen UI with many panels/buttons/badges can pay for a complex
fragment path even when most widgets are visually simple.

This matters because the user is seeing lag before plotting. A static control
panel plus plot area can still draw many full-size rounded rects and text
entries at fullscreen.

### Layout/Rebuild Cost Is Already Partly Measurable

`WgpuState::apply_layout()` recomputes layout and rebuilds scatter viewports,
text, primitives, images, and overlays. `WgpuState::rebuild_visuals()` rebuilds
text and primitives without layout. The current snapshot counters can tell us
when a frame was expensive because something rebuilt versus because rendering a
large surface was expensive.

## Benchmark Plan

### 1. Add an Empty GUI Fullscreen Probe

Create a small benchmark under `benchmarking/` that opens the same shell of the
WASP viewer but does not create points, stream data, or animate. It should run
for a fixed number of smoke frames and print:

- Window width/height.
- `wall_fps`.
- `frame_work_ms`.
- `frame_encode_ms`.
- `frame_submit_ms`.
- `frame_present_ms`.
- Primitive rebuild ms/count.
- Text rebuild ms/count.
- Command queue depth.

Run this at:

- 800x480
- 1024x600
- 1280x720
- Actual fullscreen Pi resolution

Expected value: proves whether the base GUI alone is enough to overload the Pi
at fullscreen.

### 2. Add a Redraw-Only Probe

Use the same empty GUI, but drive `app.request_redraw()` at fixed rates:

- 15 Hz
- 30 Hz
- 60 Hz

Compare with no manual redraw driver. If the GUI is smooth when idle but laggy
with redraw pressure, the issue is per-frame fill/present cost. If it is laggy
even without redraw pressure, the issue is likely event handling, compositor, or
layout/animation churn.

### 3. Add a Widget Complexity Matrix

Run the same window sizes with progressively more UI:

- Bare window.
- One empty panel.
- WASP left panel only.
- WASP left panel plus empty Scatter3D.
- WASP left panel plus Scatter3D chrome/grid, no points.
- WASP left panel plus Scatter3D with fake points.

Expected value: identifies whether the cost jumps at general widgets, text,
scatter chrome, or point rendering.

### 4. Split Scatter3D Fill-Rate Tests From Upload Tests

For static prebuilt payloads, benchmark:

- 50k, 100k, 150k, 200k points.
- Point size 1.0, 1.5, 2.0, 3.0.
- `point_style=square`, `circle`, `gaussian`.
- Auto point size on/off.
- Grid/colorbar/labels on/off.
- Render scale 1.0, 0.75, 0.5.

The existing `benchmarking/scatter_stream_compare.py` is close, but it should be
extended to record window size, active render scale, LOD state, and point style.

Expected value: shows if fullscreen scatter is fill-rate bound, shader-style
bound, or upload/packing bound.

### 5. Verify LOD and Render Scale Behavior During Interaction

During scatter orbit/pan, capture debug snapshots and confirm:

- `lod_active` becomes true.
- `effective_draw_point_count` drops as expected.
- `active_render_scale` drops below 1.0.
- `last_render_encode_ms` and `frame_work_ms` improve.

If this does not happen consistently, the interaction LOD path is not covering
the user's slow interaction. If it does happen but still lags, the base surface
or primitive/text overlay cost is likely too high.

## Candidate Optimizations To Test

### A. Pi Fullscreen Render Scale Policy

Add a Pi-only or widget-configurable static render scale for large Scatter3D
viewports, not only interactive render scale. For example:

- Render Scatter3D at 1.0 below a pixel budget.
- Render at 0.75 or 0.5 when widget area exceeds a Pi threshold.
- Keep full quality after interaction only if frame time is healthy.

This is likely the safest first Scatter3D optimization because the offscreen
composite path already exists.

### B. Global Redraw Throttling For Pi

Add a Pi profile redraw governor for explicit `request_redraw()` loops. The
benchmark code and streaming examples can request redraw faster than the Pi can
present at fullscreen. A governor could coalesce redraw requests to 30 Hz or to
the measured frame budget while data uploads continue to coalesce.

This should be tested carefully because desktop should stay unrestricted.

### C. Simple Primitive Fast Path

Add a cheaper primitive shader/path for common simple rectangles:

- Solid color.
- No rounded corners.
- No gradient.
- No transform.
- No shadow/effect.

Keep the current general shader for complex widgets. This is worth testing if
the empty/fullscreen GUI shows high frame cost with many controls and no points.

### D. Pi Theme Simplification

For Pi profile, reduce visual cost in framework/default styles:

- Smaller or zero radii for dense controls.
- Fewer gradients.
- Fewer shadows/effects.
- Avoid large translucent overlays.

This can be tested without changing renderer architecture and may improve the
whole GUI before plot-specific work.

### E. Scatter3D Point Shader Modes

Expose and test a fast Pi point style:

- Square points avoid circle distance/discard work.
- Smaller fixed point sizes reduce fragment load.
- Disable rounded widget clipping when the Scatter3D panel has zero radius.

If square 1px/1.5px points are much faster fullscreen, the bottleneck is
fragment shader/fill-rate, not CPU upload.

### F. Compact `xyz + scalar` Payload

The current compact `xyz` path is much faster than RGB instances, but it uses Z
for coloring. Add a compact 16-byte payload format:

- `x: f32`
- `y: f32`
- `z: f32`
- `scalar: f32`

This preserves fast packing while letting color use phase and depth use scaled
phase or another value. It will not fix fullscreen idle lag, but it improves the
camera script's steady-state data path.

### G. More Detailed GPU Pass Timing

Current frame timing is CPU-side. Add optional timestamp/query or coarse pass
timers for:

- Base primitive pass.
- Scatter pass.
- Scatter offscreen pass.
- Scatter composite pass.
- Text/overlay pass.

If GPU timestamp support is unavailable on the Pi backend, keep CPU encode
timers but split them by pass. This will help distinguish "lots of draw encoding"
from "GPU/present is busy."

## Recommended Order

1. Build `benchmarking/rpi_fullscreen_gui_probe.py` for empty GUI/window-size
   and redraw-rate testing.
2. Extend snapshots to include cheap fields currently missing from Scatter3D:
   active render scale, LOD active/enabled, widget pixel area, point style, and
   render target size.
3. Run the empty/fullscreen matrix before touching shaders.
4. If empty fullscreen is slow, test Pi theme simplification and primitive fast
   path.
5. If empty fullscreen is fine but Scatter3D chrome is slow, test static
   Scatter3D render scale and overlay/grid toggles.
6. If only points are slow, test point style/size/render scale and consider the
   compact `xyz + scalar` payload.

## Success Criteria

The audit is successful when we can answer these with numbers:

- Does fullscreen lag exist with no continuous redraw?
- What redraw rate can fullscreen sustain with no points?
- What is the cheapest fullscreen GUI frame cost?
- How much cost is added by text/primitives, Scatter3D chrome, and points?
- Does Pi interaction LOD actually engage during the slow path?
- Which single setting gives the largest gain: render scale, point size, point
  style, theme simplification, or redraw throttling?

## Initial Probe Results

Added `benchmarking/rpi_fullscreen_gui_probe.py` on 2026-05-18. The probe can
run focused scenarios with fixed smoke frames and prints compact debug snapshot
timings.

Example command:

```bash
PYTHONPATH=/home/xymbu/Desktop/Projects/DragonGui-RPi/python \
DRAGONGUI_PROFILE=pi \
DRAGONGUI_WGPU_BACKEND=gl \
DRAGONGUI_WINDOW_BACKEND=x11 \
.venv/bin/python benchmarking/rpi_fullscreen_gui_probe.py \
  --scenario scatter-empty --width 800 --height 480 --frames 18
```

Early 800x480 results on the current Pi:

| Scenario | Points | Primitive Count | Text Count | Wall FPS | Last Frame | Submit | Present | Notes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `bare` | 0 | 0 | 1 | ~59.5 | ~17.3 ms | ~1.4 ms | ~15.1 ms | Near display cadence; CPU/encode tiny. |
| `labels` | 0 | 0 | 32 | ~59.4 | ~15.5 ms | ~1.0 ms | ~14.2 ms | Text alone is not the slowdown. |
| `controls` | 0 | 63 | 32 | ~25.7 | ~41.4 ms | ~35.1 ms | ~5.8 ms | Dozens of widget primitives trigger the first major slowdown. |
| `controls --radius 0` | 0 | 63 | 32 | ~26.3 | ~36.8 ms | ~30.5 ms | ~6.0 ms | Removing radius helps little; not only rounded corners. |
| `scatter-empty` | 0 | 63 | 30 | ~26.4 | ~38.5 ms | ~36.6 ms | ~1.4 ms | Empty Scatter3D adds almost nothing beyond the control UI. |
| `scatter-points` | 150k | 63 | 30 | ~13.5 | ~71.1 ms | ~69.4 ms | ~0.7 ms | Points roughly double the cost over empty UI. |
| `scatter-points --point-size 1 --point-style square` | 150k | 63 | 30 | ~13.9 | ~73.2 ms | ~66.0 ms | ~6.0 ms | Smaller square points did not materially improve the 150k case. |

Interpretation:

- The first big regression appears before plotting, when general widget
  primitives are present.
- Text rendering is not the main bottleneck in this probe.
- Radius is not the main bottleneck either; the general primitive draw path or
  GL driver/GPU work for those rectangles is the stronger suspect.
- Scatter3D points still add a large additional cost, but point style/size alone
  did not fix it at 150k. The next Scatter3D test should focus on reducing draw
  count at rest, static render scale, or a lower-cost point pipeline.

Next measurements:

- Run the same scenarios at the actual fullscreen Pi resolution.
- Add a primitive-shader fast-path experiment for simple solid rectangles.
- Add static Scatter3D LOD/render-scale experiments so idle 150k does not always
  draw all instances at full widget resolution.

Actual display-size follow-up:

- `xrandr --current` reported 2560x1440 active mode on `HDMI-A-2`.
- DragonGUI final snapshots reported a 2560x1378 window, likely excluding window
  manager decoration/panel space.

| Scenario | Wall FPS | Last Frame | Submit | Present | Notes |
| --- | ---: | ---: | ---: | ---: | --- |
| `bare` | ~21.5 | ~24.6 ms | ~2.2 ms | ~21.8 ms | Large surface alone has a significant present cost. |
| `labels` | ~21.1 | ~32.5 ms | ~2.0 ms | ~29.7 ms | 32 labels do not increase submit cost. |
| `controls` | ~3.9 | ~257.6 ms | ~238.3 ms | ~18.8 ms | General rounded primitives dominate. |
| `controls --radius 0` | ~6.7 | ~149.8 ms | ~137.3 ms | ~11.9 ms | Simple fast path helps, but not enough at this size. |
| `scatter-empty` | ~3.8 | ~261.7 ms | ~248.5 ms | ~12.8 ms | Empty Scatter3D adds little beyond controls. |
| `scatter-points`, 150k | ~3.3 | ~310.3 ms | ~295.4 ms | ~13.1 ms | Points add cost, but baseline primitive cost dominates. |

Backend comparison:

- `DRAGONGUI_WGPU_BACKEND=vulkan DRAGONGUI_WINDOW_BACKEND=wayland` failed with
  a wgpu panic about a requested feature not being available on the device.

Size-scaling follow-up:

| Requested Size | Scenario | Wall FPS | Last Frame | Submit | Present |
| --- | --- | ---: | ---: | ---: | ---: |
| 1280x720 | `controls` | ~13.3 | ~68.2 ms | ~66.6 ms | ~1.2 ms |
| 1280x720 | `controls --radius 0` | ~19.8 | ~45.3 ms | ~39.4 ms | ~5.3 ms |
| 1920x1080 | `controls` | ~6.3 | ~153.8 ms | ~138.8 ms | ~14.7 ms |
| 1920x1080 | `controls --radius 0` | ~9.7 | ~92.8 ms | ~84.5 ms | ~7.7 ms |

Mid-resolution Scatter3D follow-up:

| Requested Size | Scenario | Wall FPS | Last Frame | Submit | Notes |
| --- | --- | ---: | ---: | ---: | --- |
| 1280x720 | `scatter-empty` | ~13.0 | ~75.2 ms | ~73.9 ms | Empty plot matches controls baseline. |
| 1280x720 | `scatter-points`, 150k | ~8.3 | ~117.9 ms | ~114.6 ms | Points are a meaningful second bottleneck at 720p. |

1280x720 point-budget approximation:

| Points | Wall FPS | Last Frame | Submit | Upload | Point Scale |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | ~13.0 | ~75.2 ms | ~73.9 ms | 0.0 ms | 1.0 |
| 25k | ~11.0 | ~89.8 ms | ~86.6 ms | ~9.2 ms | 1.0 |
| 50k | ~10.6 | ~84.4 ms | ~80.7 ms | ~10.8 ms | 1.0 |
| 100k | ~9.3 | ~103.8 ms | ~101.1 ms | ~19.3 ms | ~0.90 |
| 150k | ~8.3 | ~117.9 ms | ~114.6 ms | ~28.5 ms | ~0.79 |
| 200k | ~7.3 | ~126.0 ms | ~123.3 ms | ~32.8 ms | ~0.68 |

Updated interpretation:

- The primitive cost scales steeply with pixel area.
- The simple solid-rect path improves every tested size when controls are made
  square/simple.
- Default rounded controls remain slow because they still use the general
  primitive shader.
- At 720p, Scatter3D point count still matters; at the actual display size, the
  primitive/fullscreen baseline dominates so heavily that point cost is
  secondary.
- Reducing point count from 200k to 50k helps, but the empty plot baseline must
  also improve before this can reach smooth interaction.

## Primitive Fast-Path Experiment

Implemented on 2026-05-18:

- Added `native/src/primitives/rect_simple.wgsl`.
- Added a second primitive render pipeline in `native/src/primitives/mod.rs`.
- Preserved instance ordering by drawing contiguous runs with either the simple
  or general pipeline.
- Fast-path eligibility is currently conservative:
  - zero radii
  - solid fill
  - no transform
  - no shadow/effect
  - rectangle shape only

Validation:

- `cargo check --manifest-path native/Cargo.toml --features pyo3/extension-module,pi`
  passed with only existing warnings.
- Rebuilt and installed the native extension with `bash rpi_setup_and_run.sh build`.

Probe results at 800x480:

| Scenario | Before | After | Interpretation |
| --- | ---: | ---: | --- |
| `controls --radius 0` | ~26.3 FPS, ~30.5 ms submit | ~36.9 FPS, ~21.6 ms submit | Simple solid rect path helps materially. |
| `controls` default radius | ~25.7 FPS | ~26.4 FPS | Most current controls are rounded, so they still use the general shader. |

Conclusion:

- The primitive fast path is worth keeping as a building block.
- To improve the real UI, the next step is either:
  - Pi theme/style simplification that makes dense controls square/simple, or
  - a second fast path for common rounded solid rectangles that avoids the full
    gradient/effects shader while preserving the current look.

## Redraw Throttling Probe

Added `DRAGONGUI_SMOKE_FRAME_INTERVAL_MS` on 2026-05-18 as a benchmark-only
native smoke-loop control. This lets the smoke harness wait between redraws so
we can see whether continuous redraw pressure is creating GPU/driver backlog.

1280x720 `controls` results:

| Smoke Interval | Wall FPS | Last Frame | Frame Work | Submit | Present | Notes |
| ---: | ---: | ---: | ---: | ---: | ---: | --- |
| none/max-rate | ~13.3 | ~68.2 ms | n/a | ~66.6 ms | ~1.2 ms | Existing max-rate result. |
| 33 ms | ~13.4 | ~40.2 ms | ~33.0 ms | ~32.6 ms | ~7.2 ms | 30 FPS target is still too high for this workload. |
| 67 ms | ~13.6 | ~3.6 ms | ~2.8 ms | ~2.2 ms | ~0.8 ms | Long idle gaps clear the backlog, but lower the update cadence. |

Interpretation:

- Redraw throttling/coalescing can reduce backlog and smooth overloaded cases.
- It does not replace renderer optimization; if one frame costs more than the
  target frame budget, the app cannot truly run at that target rate.
- A production governor should coalesce requested redraws and data uploads using
  event-loop timing. The current smoke interval is only a benchmark hook.

## Real WASP-Style Viewer Results

Added controlled benchmark flags to `examples/rpi_v4l2_phase_scatter.py`:

- `--debug-autoinject {none,wave,rings,noise,ramp}` injects fake points without
  clicking the debug UI.
- `--benchmark-summary` prints compact timing fields instead of the full debug
  snapshot.

Measured real-viewer results:

| Window | Points | Wall FPS | Last Frame | Submit | Present | Upload | Pack | Notes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 800x480 | 0 | ~19.1 | ~11.5 ms | ~9.4 ms | ~0.8 ms | ~1.2 ms | 0.0 ms | Smaller window is usable before data. |
| 800x480 | 150k | ~8.9 | ~76.4 ms | ~73.9 ms | ~0.9 ms | ~30.9 ms | ~43.8 ms | Python packing and upload are visible costs. |
| 1280x720 | 0 | ~8.2 | ~86.4 ms | ~78.1 ms | ~7.2 ms | ~0.8 ms | 0.0 ms | Real layout is already slow before plotting. |
| 1280x720 | 150k | ~5.3 | ~153.4 ms | ~149.7 ms | ~1.3 ms | ~29.2 ms | ~31.5 ms | Points add cost, but submit/render remains dominant. |

Interpretation:

- The real WASP-style script is heavier than the synthetic `scatter-empty`
  scenario. Future optimization passes need to benchmark both.
- The compact `xyz` path helped a lot versus RGB payloads, but the live path
  still spends about 30-45 ms in Python packing at 150k points.
- The largest user-visible issue remains render/submit cost at larger windows.

## Static Render-Scale Gap

The current probe's `--render-scale` option sets Scatter3D
`interactive_render_scale`. Code inspection confirmed this only applies while
Scatter3D LOD/interaction is active. Therefore a 1280x720 150k-point sweep:

| Requested Interactive Scale | Wall FPS | Notes |
| ---: | ---: | --- |
| 1.0 | ~6.2 | Baseline synthetic scatter-points run in this sweep. |
| 0.75 | ~4.5 | Noisy/worse run; not a true static render-scale test. |
| 0.5 | ~6.1 | Similar to 1.0; not a true static render-scale test. |

Conclusion:

- We still do not have a valid always-on idle render-scale benchmark.
- Implementing a static Pi render-scale policy remains a benchmarkable
  optimization candidate.

Update:

- Implemented `DRAGONGUI_SCATTER_STATIC_RENDER_SCALE` as a benchmarkable static
  idle render-scale prototype.
- Added `static_render_scale` and `active_render_scale` to Scatter3D snapshots.
- Added `--static-render-scale` to `benchmarking/rpi_fullscreen_gui_probe.py`.

1280x720 synthetic `scatter-points`, 150k:

| Static Scale | Wall FPS | Last Frame | Submit | Notes |
| ---: | ---: | ---: | ---: | --- |
| 1.0 | ~10.4 | ~102.8 ms | ~94.7 ms | Baseline after rounded primitive fast path. |
| 0.75 | ~10.6 | ~100.6 ms | ~96.7 ms | Essentially flat. |
| 0.5 | ~11.4 | ~87.2 ms | ~85.2 ms | Small improvement. |

Real WASP-style `1280x720`, 150k fake points at static scale `0.5` stayed at
about ~7.0 FPS, so static render scale is useful as an optional quality knob
but not enough to solve the real viewer by itself.

## Pass-Level Timing Update

Added CPU-side pass timing fields to runtime snapshots:

- `frame_base_pass_encode_ms`
- `frame_scatter_pass_encode_ms`
- `frame_overlay_pass_encode_ms`
- `frame_encoder_finish_ms`

Clean 1280x720 `scatter-empty` after the rounded primitive fast path:

| Field | Value |
| --- | ---: |
| Wall FPS | ~19.7 |
| Last frame | ~56.2 ms |
| Base pass encode | ~0.06 ms |
| Scatter pass encode | ~0.02 ms |
| Overlay pass encode | ~0.01 ms |
| Encoder finish | ~0.96 ms |
| Submit | ~49.9 ms |
| Present | ~5.8 ms |

The useful takeaway is that CPU command encoding is tiny. Remaining cost is
mostly driver/GPU wait in `submit`, with occasional present cost.
