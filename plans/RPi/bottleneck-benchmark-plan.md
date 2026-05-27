# Raspberry Pi Bottleneck Benchmark Plan

Last updated: 2026-05-22

## Goal

Identify which parts of the Pi render path still dominate after the rounded
solid-rect fast path:

- Scatter3D point draw cost.
- Scatter3D chrome/grid/axis/colorbar cost.
- Python packing and native upload cost.
- Window/plot pixel-area cost.
- Redraw cadence/backlog cost.
- Remaining primitive/UI surface cost.

The current pass-level CPU encode timings show command encoding is tiny. Most
remaining time appears in `submit` and sometimes `present`, so these benchmarks
use A/B scenarios to infer which GPU/driver work is expensive.

## Current Baseline

Post rounded solid-rect fast path:

| Scenario | Window | Points | Result |
| --- | --- | ---: | --- |
| Synthetic `controls` | 1280x720 | 0 | ~18.9 FPS |
| Synthetic `scatter-empty` | 1280x720 | 0 | ~17.6-19.7 FPS |
| Synthetic `scatter-points` | 1280x720 | 150k | ~10.4 FPS |
| Real WASP viewer | 1280x720 | 0 | ~11.8 FPS |
| Real WASP viewer | 1280x720 | 150k | ~7.0 FPS |

Post compact primitive split, dedicated LinePlot renderer, CSS target filtering,
and current Scatter3D fast paths, measured on 2026-05-22:

| Scenario | Window | Points | Wall FPS | Last Frame | Submit | Present | Notes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| Synthetic `controls` | 800x480 | 0 | ~58.9 | ~15.3 ms | ~4.9 ms | ~9.9 ms | UI-only path is close to display-rate. |
| Synthetic `controls` | 1280x720 | 0 | ~52.0 | ~17.9 ms | ~4.0 ms | ~13.3 ms | Larger window mostly affects present/vsync behavior. |
| Synthetic `scatter-empty` | 800x480 | 0 | ~58.0 | ~21.3 ms | ~5.4 ms | ~15.4 ms | Empty Scatter3D is not a major problem at 800x480. |
| Synthetic `scatter-empty` | 1280x720 | 0 | ~44.1 | ~16.2 ms | ~7.2 ms | ~8.1 ms | Empty Scatter3D still costs more at 720p. |
| Synthetic `scatter-points` | 800x480 | 150k | ~20.4 | ~53.3 ms | ~49.2 ms | ~3.0 ms | Dense points dominate even at small window size. |
| Synthetic `scatter-points` | 1280x720 | 150k | ~15.6 | ~64.9 ms | ~58.8 ms | ~4.4 ms | Primary bottleneck is GPU/driver submit for points. |

Current conclusion:

- General widgets, text, CSS cascade, and primitive upload are not the dominant
  fullscreen bottleneck anymore.
- Dense Scatter3D draw/submit cost is the largest measured bottleneck.
- Pixel area matters, but point count matters more.
- The highest-value next experiments are live point budgeting/decimation and
  deeper Scatter3D pipeline/draw-path benchmarking.

## Benchmark Matrix

### 1. Real WASP Point Budget Sweep

Purpose:

- Find a practical live point budget for the camera viewer.
- Decide whether idle draw-count reduction should be the next optimization.

Run:

- 25k
- 50k
- 75k
- 100k
- 150k
- 200k

Measure:

- `wall_fps`
- `last_frame_ms`
- `frame_submit_ms`
- `frame_present_ms`
- `scatter_upload_ms`
- `last_pack_ms`
- `scatter_effective_drawn`

Results:

| Points | Wall FPS | Last Frame | Submit | Present | Upload | Pack | Notes |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 25k | ~10.7 | ~49.0 ms | ~44.7 ms | ~1.7 ms | ~3.2 ms | ~3.6 ms | Best tested live point budget; still below 15 FPS. |
| 50k | ~9.9 | ~67.6 ms | ~60.1 ms | ~6.2 ms | ~6.4 ms | ~8.9 ms | Noticeable cost increase over 25k. |
| 75k | ~9.0 | ~79.6 ms | ~75.7 ms | ~2.4 ms | ~6.8 ms | ~23.1 ms | Packing was noisy/high in this run. |
| 100k | ~7.9 | ~95.9 ms | ~88.6 ms | ~6.0 ms | ~7.8 ms | ~17.6 ms | Draw/submit cost now dominates. |
| 150k | ~6.9 | ~117.7 ms | ~110.2 ms | ~6.1 ms | ~38.5 ms | ~29.8 ms | Current high-density target is expensive. |
| 200k | ~6.2 | ~127.3 ms | ~125.2 ms | ~0.8 ms | ~38.1 ms | ~27.7 ms | More points are possible, but slow. |

Decision:

- A Pi live-display draw budget is worth implementing.
- 25k-50k is the only tested range close to tolerable at 1280x720.
- 150k-200k should probably be retained as source data but decimated for live
  display unless the user explicitly requests full density.
- `submit` rises with point count, so the major cost is still draw/GPU work.
- Packing/upload also becomes meaningful above 100k, so the live camera path
  should avoid rebuilding/reuploading more points than it needs to show.

### 2. Upload vs Steady Render Split

Purpose:

- Separate one-time data packing/upload from repeated rendering of an already
  uploaded point cloud.
- Decide whether to optimize Python packing/native upload or focus mainly on
  draw/render cost.

Run:

- Inject fake points once, then render smoke frames without changing data.
- Compare against a path that updates/reuploads point data during the run.

Measure:

- `last_pack_ms`
- `scatter_upload_ms`
- `frame_submit_ms`
- `wall_fps`

Results:

| Mode | Points | Wall FPS | Submit | Upload | Pack | Notes |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Static once | 100k | ~8.1 | ~85.7 ms | ~12.5 ms | ~25.7 ms | One fake payload injected, then rendered for 45 smoke frames. |
| Reupload prepared payload 10x | 100k | ~8.2 | ~85.4 ms | ~16.8 ms | ~19.4 ms | Reapplying an already-packed payload did not materially change FPS. |

Decision:

- Reuploading an already-prepared payload is not the main bottleneck.
- Steady draw/submit cost dominates for 100k points at 1280x720.
- Real camera packing still matters because live frames rebuild payloads from
  new data, but upload alone is not enough to explain the frame rate.
- A future benchmark should test true per-frame repacking from changing arrays
  if the camera path still feels slower than fake prepared data.

### 3. Scatter Visibility / Chrome Matrix

Purpose:

- Determine whether Scatter3D chrome is expensive relative to points.

Run:

- Same layout space, no Scatter3D.
- Scatter3D with no points.
- Scatter3D with grid/planes/axes/colorbar disabled.
- Scatter3D with points and minimal chrome.
- Scatter3D with points and full chrome.

Results:

| Scenario | Points | Chrome | Wall FPS | Submit | Present | Notes |
| --- | ---: | --- | ---: | ---: | ---: | --- |
| Synthetic `scatter-empty` | 0 | Full | ~18.8 | ~44.9 ms | ~5.0 ms | Full empty scatter baseline. |
| Synthetic `scatter-empty` | 0 | Minimal | ~20.8 | ~42.3 ms | ~0.7 ms | Chrome has a small cost when no points are present. |
| Synthetic `scatter-points` | 150k | Full | ~10.6 | ~96.4 ms | ~1.1 ms | Points dominate. |
| Synthetic `scatter-points` | 150k | Minimal | ~10.5 | ~89.5 ms | ~4.7 ms | Removing chrome does not materially improve FPS with 150k points. |

Decision:

- Scatter3D chrome is not the big bottleneck for dense point clouds.
- With no points, minimal chrome helps a little, but not enough to explain the
  real viewer slowdown.
- Optimization should focus on point count/draw cost and redraw cadence before
  spending much effort on grid/axis/colorbar rendering.

### 4. Window / Plot Area Sweep

Purpose:

- Quantify how much pixel area drives the submit/present cost.

Run:

- 800x480
- 1024x600
- 1280x720
- Fullscreen/current display size

Use:

- 0 points.
- 150k points.

Results:

| Window | Points | Wall FPS | Last Frame | Submit | Present | Notes |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| 800x480 | 0, controls | ~58.9 | ~15.3 ms | ~4.9 ms | ~9.9 ms | UI-only baseline. |
| 800x480 | 0, scatter-empty | ~58.0 | ~21.3 ms | ~5.4 ms | ~15.4 ms | Empty scatter still display-rate-ish. |
| 800x480 | 150k | ~20.4 | ~53.3 ms | ~49.2 ms | ~3.0 ms | Dense points dominate. |
| 1280x720 | 0, controls | ~52.0 | ~17.9 ms | ~4.0 ms | ~13.3 ms | UI remains near display-rate. |
| 1280x720 | 0, scatter-empty | ~44.1 | ~16.2 ms | ~7.2 ms | ~8.1 ms | Empty scatter costs more at larger area. |
| 1280x720 | 150k | ~15.6 | ~64.9 ms | ~58.8 ms | ~4.4 ms | Point draw/submit is dominant. |

Decision:

- Pixel area matters, but not enough to explain the dense point slowdown by
  itself.
- General UI cost is no longer the main bottleneck after primitive/CSS work.
- Scatter3D point draw cost remains the primary target.

### 4b. Synthetic Point Budget Sweep

Purpose:

- Get a clean point-count curve independent of real camera packing.
- Choose a live-display point budget for WASP before deeper renderer work.

Run:

- `benchmarking/rpi_fullscreen_gui_probe.py --scenario scatter-points --width
  1280 --height 720 --frames 30`

Results from 2026-05-22:

| Points | Wall FPS | Last Frame | Submit | Upload | Notes |
| ---: | ---: | ---: | ---: | ---: | --- |
| 25k | ~35.1 | ~21.3 ms | ~18.9 ms | ~9.2 ms | First range that feels plausibly smooth. |
| 50k | ~28.9 | ~31.5 ms | ~29.8 ms | ~13.9 ms | Still usable, but clear submit increase. |
| 100k | ~20.4 | ~50.0 ms | ~44.5 ms | ~19.2 ms | Submit dominates; frame budget is over 30 FPS. |
| 150k | ~15.6 | ~65.1 ms | ~59.0 ms | ~29.6 ms | Current high-density test point. |
| 200k | ~13.2 | ~75.4 ms | ~69.5 ms | ~35.7 ms | Possible, but not interactive enough. |

Decision:

- A 50k live draw budget is the practical first target for 1280x720.
- 100k+ should be retained as source data but decimated for live display on Pi.
- Upload rises with point count, but steady frame submit rises more and is the
  dominant repeated cost.

### 4c. Static Scatter Render-Scale Sweep

Purpose:

- Check whether rendering Scatter3D to a smaller internal target meaningfully
  reduces fullscreen cost.

Results from 2026-05-22, 1280x720, 150k points:

| Static Render Scale | Wall FPS | Last Frame | Submit | Notes |
| ---: | ---: | ---: | ---: | --- |
| 1.0 | ~16.2 | ~58.5 ms | ~56.0 ms | Baseline for this sweep. |
| 0.75 | ~16.8 | ~59.6 ms | ~57.5 ms | No meaningful improvement. |
| 0.5 | ~18.2 | ~57.1 ms | ~54.8 ms | Modest improvement only. |

Decision:

- Internal render scale helps, but not enough to be the primary fix.
- The point draw/driver submit cost remains the larger bottleneck.

### 5. Redraw Cadence / Backlog Sweep

Purpose:

- Find a stable target redraw cadence for live camera mode.

Run:

- Max-rate smoke redraw.
- 33 ms interval.
- 67 ms interval.
- 100 ms interval.

Results:

| Cadence | Points | Wall FPS | Last Frame | Submit | Present | Notes |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Max-rate smoke | 100k | ~8.0 | ~131.3 ms | ~121.6 ms | ~8.3 ms | GPU/driver stays backed up. |
| 33 ms interval | 100k | ~8.1 | ~87.8 ms | ~85.7 ms | ~0.7 ms | From upload-vs-render static-once run. |
| 67 ms interval | 100k | ~7.6 | ~66.3 ms | ~54.8 ms | ~8.9 ms | Less submit backlog, similar delivered FPS. |
| 100 ms interval | 100k | ~7.8 | ~36.8 ms | ~23.4 ms | ~11.0 ms | Much lower per-frame work, but lower cadence. |

Decision:

- Throttling/coalescing reduces per-frame backlog substantially.
- It does not increase delivered FPS for an over-budget 100k/1280x720 workload.
- A production redraw governor is still useful because it can keep the app from
  queueing obsolete frames and can make interaction feel less jammed.
- The governor should be paired with point budgeting; throttling alone does not
  solve dense full-window rendering.

### 5b. Present Mode / Frame Latency Sweep

Purpose:

- Test whether no-points lag is caused by wgpu surface present mode, vsync, or
  frame queue depth.

Implementation:

- Added `DRAGONGUI_PRESENT_MODE`.
- Added `DRAGONGUI_MAX_FRAME_LATENCY`.
- Added `--present-mode` and `--max-frame-latency` to
  `benchmarking/rpi_fullscreen_gui_probe.py`.
- Added selected present settings to benchmark summaries.

Synthetic `scatter-empty`, 1280x720:

| Present Mode | Frame Latency | Wall FPS | Last Frame | Submit | Present | Notes |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `auto_vsync` | 2 | ~19.7 | ~50.7 ms | ~45.8 ms | ~4.4 ms | Baseline. |
| `auto_no_vsync` | 2 | ~19.8 | ~50.2 ms | ~45.1 ms | ~4.8 ms | No meaningful change. |
| `auto_vsync` | 1 | ~19.9 | ~49.7 ms | ~48.4 ms | ~0.7 ms | No meaningful FPS change. |
| `auto_vsync` | 3 | ~19.7 | ~55.6 ms | ~50.2 ms | ~5.0 ms | No improvement. |

Real WASP-style no-points, 1280x720:

| Present Mode | Frame Latency | Wall FPS | Last Frame | Submit | Present | Notes |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `fifo` default | 2 | ~13.0 | ~76.3 ms | ~69.3 ms | ~5.9 ms | Real no-points baseline. |
| `auto_no_vsync` | 2 | ~13.0 | ~77.4 ms | ~69.2 ms | ~6.9 ms | No meaningful change. |
| `fifo` default | 1 | ~13.0 | ~77.7 ms | ~69.7 ms | ~6.6 ms | No meaningful change. |

Decision:

- Simple present-mode and frame-latency changes do not explain the no-points
  lag on this GL/X11 path.
- The large `submit` wait is still likely GPU/driver work for what we actually
  render, not just queued-vsync behavior.
- Next no-points investigations should focus on render-path A/B tests:
  no-depth UI pass, disabling Scatter3D pass entirely, removing large panel
  fills/backgrounds, or reducing effective window/plot surface area.

### 5c. Real WASP No-Points Render-Path A/B

Purpose:

- Isolate the source of no-points lag in the real WASP viewer.

Implementation:

- Added benchmark-only env toggles:
  - `DRAGONGUI_BENCH_DISABLE_SCATTER_RENDER`
  - `DRAGONGUI_BENCH_DISABLE_BASE_PRIMITIVES`
  - `DRAGONGUI_BENCH_DISABLE_TEXT`
  - `DRAGONGUI_BENCH_DISABLE_OVERLAY_PRIMITIVES`

Real WASP-style no-points, 1280x720:

| Toggle | Wall FPS | Last Frame | Submit | Present | Interpretation |
| --- | ---: | ---: | ---: | ---: | --- |
| None | ~13.0 | ~76.0 ms | ~68.4 ms | ~6.4 ms | Baseline no-points lag. |
| Disable Scatter3D render | ~13.3 | ~73.9 ms | ~72.0 ms | ~0.9 ms | Scatter pass is not the culprit when no points are shown. |
| Disable base primitives | ~47.5 | ~17.6 ms | ~1.6 ms | ~14.9 ms | Base primitives are the dominant no-points cost. |
| Disable text | ~13.0 | ~77.3 ms | ~2.0 ms | ~74.3 ms | Cost shifts to present; no meaningful FPS gain. |
| Disable overlay primitives | ~12.7 | ~76.9 ms | ~69.1 ms | ~6.4 ms | Overlay primitives are not the culprit. |

Decision:

- The real no-points lag is dominated by the base primitive pass.
- Scatter3D, text, and overlay primitives are not the main issue for no-points
  rendering.
- The next optimization target should be base panel/background primitive
  rendering, especially large filled/rounded surfaces.
- Likely follow-up tests:
  - count fast-path vs general-path base primitives
  - measure primitive pixel coverage/area
  - test no-depth primitive pipelines
  - test clipping/scissoring large panel fills
  - test Pi theme changes that reduce large opaque/translucent rounded fills

### 5d. Primitive Coverage / Outline Ring Test

Purpose:

- Determine why the base primitive pass is expensive with no points.
- Test whether full-rectangle outline rings are the source of the large
  `submit` wait.

Implementation:

- Added primitive stats to GPU performance snapshots:
  - base/overlay count
  - base/overlay area
  - general/simple/rounded counts and area
  - largest base rectangles
- Added a benchmark-only split-outline mode:
  - `DRAGONGUI_BENCH_SPLIT_OUTLINE_RINGS=1`
  - It replaces outline-ring full-rect shading with four thin strip rectangles.

Baseline real WASP-style no-points, 1280x720:

| Metric | Value |
| --- | ---: |
| Wall FPS | ~13.0 |
| Submit | ~69.3 ms |
| Base primitive count | 22 |
| Base primitive area | ~3.33M px |
| Base general count | 11 |
| Base general area | ~1.67M px |
| Base rounded count | 11 |
| Base rounded area | ~1.66M px |

Largest baseline base rectangles:

| Mode | Rect | Area | Meaning |
| --- | --- | ---: | --- |
| general | `[176, 0, 1104, 720]` | ~794.9k px | Plot panel outline ring. |
| rounded_solid | `[177, 1, 1102, 718]` | ~791.2k px | Plot panel fill. |
| general | `[186, 36, 1084, 674]` | ~730.6k px | Inner plot/frame outline ring. |
| rounded_solid | `[187, 37, 1082, 672]` | ~727.1k px | Inner plot/frame fill. |
| general | `[0, 0, 176, 720]` | ~126.7k px | Navigation panel outline ring. |
| rounded_solid | `[1, 1, 174, 718]` | ~124.9k px | Navigation panel fill. |

Real WASP-style no-points with split outline rings:

| Metric | Value |
| --- | ---: |
| Wall FPS | ~39.0 |
| Submit | ~2.2 ms |
| Base primitive count | 52 |
| Base primitive area | ~1.67M px |
| Base general count | 1 |
| Base general area | ~41 px |
| Base rounded count | 11 |
| Base rounded area | ~1.66M px |

Real WASP-style 150k fake points with split outline rings:

| Metric | Before Split | Split Outlines |
| --- | ---: | ---: |
| Wall FPS | ~6.9-7.0 | ~11.0 |
| Last Frame | ~117.7 ms | ~59.2 ms |
| Submit | ~110.2 ms | ~51.7 ms |

Decision:

- Full-rectangle outline rings are the main no-points bottleneck.
- The general outline-ring shader shades the entire panel area even for a thin
  border, then discards the interior.
- Splitting thin outlines into four strip rectangles is a major Pi win.
- Implemented as a real Pi-profile optimization. Thin outline rings over large
  rectangles are split automatically on Pi; small controls keep the original
  ring path.

Default Pi behavior after implementation, no benchmark env var:

| Scenario | Wall FPS | Last Frame | Submit | Base General Area | Notes |
| --- | ---: | ---: | ---: | ---: | --- |
| Real WASP no-points | ~41.4 | ~33.3 ms | ~12.1 ms | ~15.4k px | Large outline rings are now split by default. |
| Real WASP 150k fake points | ~10.9 | ~61.4 ms | ~54.3 ms | ~15.4k px | Dense points are now the larger remaining cost. |

### 6. Point Shader / Style Sweep

Purpose:

- Confirm whether point fragment style/size still matters after the primitive
  fast paths.

Run:

- circle, square, gaussian.
- point sizes 1.0, 1.5, 2.0.
- auto point size on/off.

Results:

| Style | Size | Auto Size | Wall FPS | Submit | Notes |
| --- | ---: | --- | ---: | ---: | --- |
| square | auto | on | ~17.1 | ~55.6 ms | Slightly faster than circle/gaussian in this run. |
| circle | auto | on | ~16.4 | ~57.9 ms | Current visual default. |
| gaussian | auto | on | ~16.3 | ~59.4 ms | Not meaningfully better. |
| circle | auto | on | ~16.1 | ~57.4 ms | Point-size sweep baseline. |
| circle | 1.0 | off | ~14.6 | ~64.7 ms | Fixed size is slower than auto-size. |
| circle | 2.0 | off | ~14.1 | ~65.4 ms | Larger fixed points are slower. |

Decision:

- Point style is not the main bottleneck at 150k/1280x720, but square points are
  slightly cheaper.
- Auto point sizing is worth keeping for dense Pi Scatter3D because fixed 1-2 px
  points increased submit time in this run.
- A future visual-quality option could expose `square` for maximum throughput,
  but live point budgeting should come first.

## Recommended Test Order

1. Real WASP point budget sweep.
2. Upload vs steady render split.
3. Scatter visibility/chrome matrix.
4. Window/plot area sweep.
5. Redraw cadence/backlog sweep.
6. Point shader/style sweep.

## Open Implementation Needs

- Add a benchmark mode that repeatedly updates/reuploads fake points during
  smoke frames.
- Add easy chrome toggles to the WASP script or synthetic probe if existing
  arguments are not enough.
- Add an optional no-Scatter3D real-layout mode if needed for the visibility
  matrix.
