# Raspberry Pi Performance Improvement Tracker

Last updated: 2026-05-18

This tracker summarizes what the first fullscreen GUI benchmarks showed, what
we can improve, and what still needs benchmarking.

## Benchmark Findings So Far

At 800x480 on the Pi profile with GL/X11:

| Scenario | Result | Meaning |
| --- | ---: | --- |
| `bare` | ~59 FPS | The basic window can hit display cadence. |
| `labels` with 32 text labels | ~59 FPS | Text alone is not the main bottleneck. |
| `controls` | ~26 FPS | General widget primitives are the first major slowdown. |
| `controls --radius 0` before fast path | ~26 FPS | Radius alone was not enough before the fast path. |
| `controls --radius 0` after simple fast path | ~37 FPS | Simple solid rectangles are meaningfully faster. |
| `controls` default radius after simple fast path | ~26 FPS | Current rounded controls still use the general shader. |
| `scatter-empty` | ~26 FPS | Empty Scatter3D adds little beyond the controls baseline. |
| `scatter-points` with 150k points | ~13.5 FPS | Points roughly double cost over the empty GUI baseline. |
| `scatter-points`, 150k, square 1px | ~13.9 FPS | Point style/size alone did not materially improve 150k. |

Main interpretation:

- The first bottleneck happens before plotting.
- Text is not the main problem.
- Empty Scatter3D is not the main problem.
- General widget primitive rendering is expensive on Pi.
- Scatter3D point count is still a large second bottleneck.
- The simple solid-rect fast path proved that specialized primitive shaders can
  improve Pi performance, but the default UI does not benefit much yet because
  most controls are rounded.

At the active Pi display size, `xrandr --current` reported a 2560x1440 mode.
The DragonGUI window's final drawable height was reported as 1378 px, likely
because of window decorations/panel space.

| Scenario | Result | Submit | Present | Meaning |
| --- | ---: | ---: | ---: | --- |
| `bare` | ~21.5 FPS | ~2.2 ms | ~21.8 ms | Fullscreen-sized surface alone is much heavier than 800x480. |
| `labels` with 32 text labels | ~21.1 FPS | ~2.0 ms | ~29.7 ms | Text still does not explain the slowdown. |
| `controls` default radius | ~3.9 FPS | ~238 ms | ~18.8 ms | Rounded/general widget primitives collapse fullscreen performance. |
| `controls --radius 0` | ~6.7 FPS | ~137 ms | ~11.9 ms | Simple primitive fast path helps, but fullscreen controls are still expensive. |
| `scatter-empty` | ~3.8 FPS | ~248 ms | ~12.8 ms | Empty Scatter3D adds little beyond the controls baseline. |
| `scatter-points` with 150k points | ~3.3 FPS | ~295 ms | ~13.1 ms | At fullscreen, points add cost, but the primitive/fullscreen baseline dominates. |

Backend check:

- `DRAGONGUI_WGPU_BACKEND=vulkan` with `DRAGONGUI_WINDOW_BACKEND=wayland`
  failed on this Pi with a wgpu panic: requested feature unavailable on this
  device. GL/X11 remains the usable benchmark path for now.

Updated interpretation:

- Actual fullscreen is dramatically worse than 800x480.
- The dominant fullscreen issue is the combination of large surface size and
  general/rounded primitive rendering.
- Scatter3D point count matters less at actual fullscreen than expected because
  the UI primitive baseline is already extremely expensive.
- Optimizing rounded primitives and/or reducing effective render area is now
  higher priority than Scatter3D point shader tweaks.

Size-scaling follow-up:

| Requested Size | Scenario | Wall FPS | Last Frame | Submit | Present | Notes |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| 800x480 | `controls` | ~26.4 | ~36.9 ms | ~35.7 ms | ~0.7 ms | Default rounded controls after simple fast path. |
| 800x480 | `controls --radius 0` | ~36.9 | ~22.9 ms | ~21.6 ms | ~1.0 ms | Simple fast path helps. |
| 1280x720 | `controls` | ~13.3 | ~68.2 ms | ~66.6 ms | ~1.2 ms | Cost roughly doubles versus 800x480. |
| 1280x720 | `controls --radius 0` | ~19.8 | ~45.3 ms | ~39.4 ms | ~5.3 ms | Fast path still helps but remains below smooth. |
| 1920x1080 | `controls` | ~6.3 | ~153.8 ms | ~138.8 ms | ~14.7 ms | 1080p is already very slow. |
| 1920x1080 | `controls --radius 0` | ~9.7 | ~92.8 ms | ~84.5 ms | ~7.7 ms | Fast path recovers some headroom. |
| 2560x1378 final window | `controls` | ~3.9 | ~257.6 ms | ~238.3 ms | ~18.8 ms | Actual display-sized default UI is too slow. |
| 2560x1378 final window | `controls --radius 0` | ~6.7 | ~149.8 ms | ~137.3 ms | ~11.9 ms | Simple fast path helps but does not solve fullscreen. |

Mid-resolution Scatter3D follow-up:

| Requested Size | Scenario | Wall FPS | Last Frame | Submit | Notes |
| --- | --- | ---: | ---: | ---: | --- |
| 1280x720 | `scatter-empty` | ~13.0 | ~75.2 ms | ~73.9 ms | Empty plot is close to controls-only. |
| 1280x720 | `scatter-points`, 150k | ~8.3 | ~117.9 ms | ~114.6 ms | Points add meaningful cost at 720p. |
| 2560x1378 final window | `scatter-empty` | ~3.8 | ~261.7 ms | ~248.5 ms | Full display is dominated by primitive/surface cost. |
| 2560x1378 final window | `scatter-points`, 150k | ~3.3 | ~310.3 ms | ~295.4 ms | Points add cost, but less than the fullscreen primitive baseline. |

Scatter3D point-budget approximation at 1280x720:

True idle LOD draw-budget controls are not implemented yet, so this benchmark
uses actual payload point counts as a proxy for potential draw budgets.

| Points | Wall FPS | Last Frame | Submit | Upload | Point Scale | Notes |
| ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 0 | ~13.0 | ~75.2 ms | ~73.9 ms | 0.0 ms | 1.0 | Empty plot baseline. |
| 25k | ~11.0 | ~89.8 ms | ~86.6 ms | ~9.2 ms | 1.0 | Baseline still dominates. |
| 50k | ~10.6 | ~84.4 ms | ~80.7 ms | ~10.8 ms | 1.0 | Similar to 25k within run variance. |
| 100k | ~9.3 | ~103.8 ms | ~101.1 ms | ~19.3 ms | ~0.90 | Point count starts to show more clearly. |
| 150k | ~8.3 | ~117.9 ms | ~114.6 ms | ~28.5 ms | ~0.79 | Previously measured mid-resolution case. |
| 200k | ~7.3 | ~126.0 ms | ~123.3 ms | ~32.8 ms | ~0.68 | Max current Pi cap. |

Point-budget conclusion:

- At 1280x720, lowering from 200k to 50k helps, but not enough by itself
  because the empty plot baseline is already ~13 FPS.
- A Scatter3D draw budget should still be useful, especially once primitive
  rendering improves.
- The large wins still require reducing the primitive/full-surface baseline.

Size-scaling conclusion:

- The primitive cost grows steeply with pixel area.
- The simple fast path consistently helps, but cannot make large windows smooth
  by itself.
- There is likely no single shader tweak that will make 2560-wide fullscreen
  smooth on this Pi. We probably need some combination of:
  - rounded solid fast path
  - lower effective render resolution
  - Pi profile cap on window/render surface size
  - simpler Pi visual theme
  - redraw throttling for live updates

## Things To Improve

### 1. Primitive Renderer For Normal Controls

Status: Needs follow-up.

Why:

- 63 widget primitives dropped the GUI from ~59 FPS to ~26 FPS before plotting.
- The simple solid-rect fast path improved square controls, proving this area is
  worth optimizing.

Implementation direction:

- Keep the general primitive shader for complex effects.
- Route common/simple controls through cheaper pipelines.
- Preserve draw order by drawing contiguous runs, as the simple fast path does.

### 2. Rounded Solid-Rect Fast Path

Status: Implemented and benchmarked.

Why:

- Default controls are rounded, so they still hit the expensive general shader.
- A specialized rounded solid rectangle shader should preserve the current look
  while avoiding gradient/effects/shape branches.

Implementation direction:

- Add a `rect_rounded_solid.wgsl` or extend `rect_simple.wgsl`.
- Eligible instances:
  - solid fill
  - rectangle shape
  - no transform
  - no shadow/effect
  - no gradient/no noise
  - radii may be non-zero
- Use a simpler fragment shader:
  - clip bounds
  - rounded-rect SDF
  - alpha edge
  - return solid color

Expected benchmark:

- Default `controls` should improve without forcing `--radius 0`.

Implementation result:

- Added `native/src/primitives/rect_rounded_solid.wgsl`.
- Added a rounded-solid primitive pipeline beside the existing general and
  square/simple pipelines.
- Preserved primitive draw order by routing contiguous instance runs through
  `General`, `SimpleSquare`, or `RoundedSolid`.
- Kept the square/simple path separate so `--radius 0` controls do not have to
  pay rounded SDF cost.

Post-change benchmark:

| Scenario | Before | After | Notes |
| --- | ---: | ---: | --- |
| 800x480 `controls` default | ~26.4 FPS | ~29.0 FPS | Modest small-window gain. |
| 1280x720 `controls` default | ~13.3 FPS | ~18.9 FPS | Large gain for normal rounded controls. |
| 1280x720 `controls --radius 0` | ~19.8 FPS | ~20.4 FPS | Square fast path did not regress. |
| 1280x720 `scatter-empty` | ~13.0 FPS | ~17.6 FPS | Empty scatter chrome benefits from rounded path. |
| 1280x720 real WASP, no points | ~8.2 FPS | ~11.8 FPS | Real layout improved, though still not smooth. |
| 1280x720 real WASP, 150k fake points | ~5.3 FPS | ~7.0 FPS | Point/data cost remains a second bottleneck. |

Conclusion:

- The rounded solid fast path is worth keeping.
- The remaining 1280x720 and fullscreen cost is no longer explained by the
  default rounded-control path alone.
- Next step is pass-level timing so the remaining submit/render time can be
  split by base primitives, Scatter3D, text/overlay, and present.

### 3. Pi-Specific Simpler Theme

Status: Candidate after rounded fast path.

Why:

- Pi can benefit from fewer expensive visual effects.
- The simple fast path already helps when controls are square/simple.

Implementation direction:

- For `DRAGONGUI_PROFILE=pi`, consider:
  - smaller radius or zero radius in dense controls
  - fewer gradients
  - fewer shadows/effects
  - fewer translucent surfaces
  - smaller padding/gaps where appropriate

Risk:

- This changes visual appearance, so it should be profile-specific and tested
  against layout/readability.

### 4. Static Scatter3D LOD/Render Scale

Status: Static render-scale prototype implemented and benchmarked; idle draw
budgets still pending.

Why:

- Current Scatter3D render scale reduction only applies while interaction LOD is
  active.
- Idle 150k points still draw all points at full widget size.
- 150k points dropped from ~26 FPS empty scatter to ~13.5 FPS.

Implementation direction:

- Add a Pi/static large-viewport mode that lowers render scale or draw count
  even when idle.
- Possible policies:
  - render at 0.75 or 0.5 when widget area exceeds a threshold
  - draw only a fixed point budget at rest on Pi
  - restore quality only if frame time is healthy

Benchmark status:

- Added `DRAGONGUI_SCATTER_STATIC_RENDER_SCALE`, clamped to `0.25..1.0`.
- Static scale is now included in Scatter3D snapshots as
  `static_render_scale`; `active_render_scale` reflects the actual scale used
  at rest.
- `benchmarking/rpi_fullscreen_gui_probe.py` now accepts
  `--static-render-scale`.
- Approximate point-count curve has been run at 1280x720.
- Static render-scale sweep has been run at 1280x720 with 150k points.

Static render-scale sweep, 1280x720 synthetic `scatter-points`, 150k:

| Static Scale | Wall FPS | Last Frame | Submit | Active Scale | Notes |
| ---: | ---: | ---: | ---: | ---: | --- |
| 1.0 | ~10.4 | ~102.8 ms | ~94.7 ms | 1.0 | Baseline after rounded primitive fast path. |
| 0.75 | ~10.6 | ~100.6 ms | ~96.7 ms | 0.75 | Essentially flat. |
| 0.5 | ~11.4 | ~87.2 ms | ~85.2 ms | 0.5 | Small improvement, not a primary fix. |

Real WASP-style `1280x720`, 150k fake points with static scale `0.5`:

- ~7.0 FPS, roughly matching the post-rounded-path full-res result.
- Submit remained ~105 ms and Python packing/upload stayed visible.

Conclusion:

- Static render scale now works and is safe to keep as an optional Pi tuning
  knob.
- It is not enough by itself for the real viewer. The remaining high submit
  time likely needs draw-count reduction, lower point/upload cost, and/or a
  redraw governor.

### 5. Scatter3D Draw-Count Reduction

Status: Partially benchmarked; needs prototype.

Why:

- Smaller square points did not materially help at 150k.
- That suggests count/instance pressure matters more than the point fragment
  style for this workload.

Implementation direction:

- Benchmark fixed draw budgets:
  - 25k
  - 50k
  - 75k
  - 100k
  - 150k
  - 200k
- Consider always-on Pi LOD for large point clouds.

Benchmark status:

- 25k, 50k, 100k, 150k, and 200k payload sizes were measured at 1280x720.
- Results show useful but limited gains while the empty plot baseline is still
  expensive.

### 6. Redraw Throttling/Coalescing

Status: Needs benchmark.

Why:

- Fullscreen frames are expensive enough that too many redraw requests will make
  the GUI feel laggy.
- Live camera use may not need 60 GUI presents per second.

Implementation direction:

- Add a Pi redraw governor or app-level option.
- Coalesce redraw requests to a target such as 30 FPS while still coalescing
  data uploads.

Benchmark status:

- Not yet runnable with the current smoke-frame harness, because smoke mode
  requests redraws as quickly as frames complete.
- Needs either a probe mode that exits on a timer or a native/app-level
  throttled redraw option.

### 7. Better Native Timing Counters

Status: First pass implemented.

Why:

- Current cost shows up mostly in `submit_ms`, but that is still too broad.
- We need to know whether the driver/GPU is spending time on primitives,
  Scatter3D, text/overlay, composite, or present.

Implementation direction:

- Add optional per-pass CPU encode timers:
  - base primitive pass
  - Scatter3D pass
  - Scatter3D offscreen pass
  - Scatter3D composite pass
  - text/overlay pass
- If supported on Pi, add GPU timestamp queries later.

Implementation result:

- Added runtime snapshot fields:
  - `frame_base_pass_encode_ms`
  - `frame_scatter_pass_encode_ms`
  - `frame_overlay_pass_encode_ms`
  - `frame_encoder_finish_ms`
- Added those fields to the synthetic probe and WASP benchmark summary.

First clean 1280x720 `scatter-empty` timing after rounded fast path:

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

Conclusion:

- CPU-side pass encoding is tiny.
- The broad remaining cost is driver/GPU wait in `submit` and sometimes
  `present`.
- If we need deeper attribution, the next timing step is real GPU timestamps or
  A/B toggles by pass/widget, not more CPU encode timers.

## Benchmarks Still Worth Running

### 1. Actual Fullscreen Resolution Matrix

Repeat these scenarios at the Pi's real screen size:

- `bare`
- `labels`
- `controls`
- `scatter-empty`
- `scatter-points`

Goal:

- Confirm how much worse the real fullscreen surface is than 800x480.

### 2. Rounded Solid Fast-Path Benchmark

After implementing the rounded solid fast path:

- Compare default `controls` before/after.
- Compare default `scatter-empty` before/after.
- Compare actual WASP layout before/after.

Goal:

- Verify that the default UI benefits without changing appearance.

### 3. Pi Theme Simplification Benchmark

Compare:

- default theme
- radius 0
- reduced radius
- no gradients/shadows/effects
- compact padding/gaps

Goal:

- Decide whether profile-specific style simplification is worth it.

### 4. Static Scatter3D Render Scale Benchmark

Compare idle scatter at:

- render scale 1.0
- render scale 0.75
- render scale 0.5

Use:

- 150k points
- 200k points
- actual fullscreen size

Goal:

- Determine whether offscreen downscale is a good idle/fullscreen strategy.

Current status:

- Confirmed not yet runnable as a true idle render-scale benchmark. The current
  probe's `--render-scale` argument maps to `interactive_render_scale`, which
  only applies while Scatter3D LOD/interaction is active.
- A 1280x720 sweep with 150k points produced noisy/non-actionable results:
  - requested scale 1.0: ~6.2 FPS
  - requested scale 0.75: ~4.5 FPS
  - requested scale 0.5: ~6.1 FPS
- Do not use those numbers as evidence against static render scaling. We still
  need a true always-active/offscreen render-scale implementation to benchmark.

### 5. Static Scatter3D LOD Benchmark

Compare draw budgets:

- 25k
- 50k
- 75k
- 100k
- 150k
- 200k

Goal:

- Find the best Pi balance between visual density and frame time.

Current status:

- Approximate point-count curve has been run at 1280x720:
  - 25k: ~11.0 FPS
  - 50k: ~10.6 FPS
  - 100k: ~9.3 FPS
  - 150k: ~8.3 FPS
  - 200k: ~7.3 FPS
- True idle LOD still needs implementation.

### 6. Window Backend Comparison

Compare:

- GL/X11
- Vulkan/Wayland
- Vulkan/auto if stable

Goal:

- Since most cost appears in submit/present, backend choice may matter.

### 7. Real WASP Viewer Benchmark

Run probe-style measurements in the actual WASP layout:

- no points
- fake points
- real camera stream

Goal:

- Confirm that the synthetic probe predicts the real app.

Current status:

- Added `--debug-autoinject` and `--benchmark-summary` to
  `examples/rpi_v4l2_phase_scatter.py`.
- Controlled no-points and fake-points runs are now possible without clicking
  debug buttons manually.

Measured with the real WASP-style layout:

| Window | Points | Wall FPS | Last Frame | Submit | Present | Upload | Pack | Notes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 800x480 | 0 | ~19.1 | ~11.5 ms | ~9.4 ms | ~0.8 ms | ~1.2 ms | 0.0 ms | Small window baseline is usable. |
| 800x480 | 150k | ~8.9 | ~76.4 ms | ~73.9 ms | ~0.9 ms | ~30.9 ms | ~43.8 ms | Points plus Python packing dominate the live-data path. |
| 1280x720 | 0 | ~8.2 | ~86.4 ms | ~78.1 ms | ~7.2 ms | ~0.8 ms | 0.0 ms | Real layout is slower than the synthetic empty scatter baseline. |
| 1280x720 | 150k | ~5.3 | ~153.4 ms | ~149.7 ms | ~1.3 ms | ~29.2 ms | ~31.5 ms | Points add cost, but render submit is still the largest number. |

Interpretation:

- The actual viewer layout is materially heavier than the synthetic probe,
  especially at 1280x720.
- Data packing/upload still matters for live camera use, but it is not the only
  issue; the real plot panel/window baseline is already slow before points.
- Further renderer work should be validated against the actual WASP script, not
  only the synthetic probe.

### 8. Redraw Governor Benchmark

Current status:

- Added benchmark-only `DRAGONGUI_SMOKE_FRAME_INTERVAL_MS` support in the native
  smoke-frame loop.
- This sleeps between smoke frames so we can test whether giving the GPU/driver
  idle time changes frame cost.

Measured at 1280x720 `controls`:

| Smoke Interval | Wall FPS | Last Frame | Frame Work | Submit | Present | Interpretation |
| ---: | ---: | ---: | ---: | ---: | ---: | --- |
| none/max-rate | ~13.3 | ~68.2 ms | n/a | ~66.6 ms | ~1.2 ms | Renderer is saturated. |
| 33 ms | ~13.4 | ~40.2 ms | ~33.0 ms | ~32.6 ms | ~7.2 ms | Cannot reach 30 FPS, but less backlog lowers per-frame work. |
| 67 ms | ~13.6 | ~3.6 ms | ~2.8 ms | ~2.2 ms | ~0.8 ms | Long idle gaps make individual render work cheap, but cadence is capped. |

Interpretation:

- A redraw governor will not make an over-budget 720p frame magically hit
  30-60 FPS.
- It may still make the app feel less backed up by coalescing redraws and data
  uploads when the Pi is overloaded.
- The production version should use an event-loop deadline/coalescing design,
  not a sleep in the render path.

## Current Recommended Order

1. Implement rounded solid-rect fast path. Done.
2. Benchmark default `controls`, `scatter-empty`, and the real WASP layout
   before/after that change. Done.
3. Add pass-level native timing counters if submit time is still too broad to
   explain the remaining cost. Done for CPU encode timing.
4. Implement and benchmark true static Scatter3D idle render scale or idle draw
   budgets for large Pi viewports. Static render scale done; idle draw budgets
   still pending.
5. Add a production redraw governor/coalescer after renderer/frame-cost changes
   are measured.
6. If the real WASP camera path remains packing-bound after render work, add a
   more compact camera payload path such as `xyz + scalar` or direct scalar
   coloring from packed arrays.
