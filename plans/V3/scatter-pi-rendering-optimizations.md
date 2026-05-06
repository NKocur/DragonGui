# V3 Scatter Pi Rendering Optimizations

DragonGUI's `Scatter3D` needs a low-power rendering path that keeps dense
point clouds responsive on Raspberry Pi-class GPUs. The immediate target is
roughly 10 fps with 125k+ points, with enough headroom to make 300k points
plausible when the scene is dense or the user is actively orbiting.

The core idea is to reduce pixel fill and point work during interaction, then
restore full quality when the camera settles. Most of the work belongs in
`native/src/scatter/mod.rs`; the Python API should not need to change for the
first pass.

## Goals

- Improve dense scatter performance on low-power GPUs without sacrificing the
  rest of the UI.
- Keep text, controls, legends, and overlay chrome at full native resolution.
- Make orbit/pan/zoom feel responsive by using lower quality only while the
  user is interacting.
- Reuse the existing scatter LOD infrastructure where possible.
- Add instrumentation so we can compare point count, render scale, and frame
  timing before and after each phase.
- Keep default desktop behavior visually unchanged unless low-power mode is
  enabled or the renderer detects sustained frame pressure.

## Non-Goals

- A full replacement for the scatter renderer.
- Python-level data model changes in the first implementation.
- Scientific-level visibility guarantees while interaction LOD is active.
- Exact Matplotlib/VTK rendering parity.
- A complete octree implementation in the first phase.

## Current Code Notes

- `native/src/scatter/mod.rs` already has an interaction LOD path:
  `lod_enabled`, `lod_threshold`, `lod_factor`, `lod_active`, and
  `upload_lod_buffer(...)`.
- LOD sampling currently uses hash buckets so the first `N` points are a
  representative sample rather than a clustered prefix.
- Point rendering uses screen-space billboard quads, so dense overdraw can be
  expensive on low-bandwidth GPUs.
- The scatter renderer already tracks interaction state such as orbit/pan and
  can use that as the quality-switch trigger.
- The point pipeline currently has depth/stencil behavior that should be
  audited before changing depth writes or transparency behavior.

## Proposed Quality Modes

Add an internal scatter quality policy with three states:

| Mode | Trigger | Render Scale | Point Count | Point Size | Purpose |
| --- | --- | ---: | ---: | ---: | --- |
| `full` | camera settled | 1.0 | all visible points | normal/adaptive | final crisp frame |
| `interactive` | orbit/pan/zoom active | 0.5-0.75 | LOD sample | smaller | responsive manipulation |
| `recovery` | first 200 ms after interaction | ramp to 1.0 | ramp to full | ramp to normal | avoid abrupt quality pop |

Initial configuration can be native-only:

- Cargo feature or runtime constant: `pi-low-power-scatter`.
- Optional env override for testing: `DRAGONGUI_SCATTER_LOW_POWER=1`.
- Optional debug overlay values in existing snapshots: active mode, render
  scale, effective point count, effective point size, culling count.

Python can later expose this as a widget/app option if it proves useful:

```python
dg.Scatter3D(..., quality="auto")
dg.App(scatter_quality="low_power")
```

That API should wait until the native behavior is stable.

## Public Toggle Design Rules

Not every optimization should become a public option. The public API should
expose only meaningful quality/performance tradeoffs, while correctness-
preserving optimizations stay internal.

Always-on internal optimizations:

- Skip depth writes when points are opaque.
- Avoid per-frame depth sorting when a stable or sorting-free path is possible.
- Frustum/chunk culling.
- Octree or grid spatial index construction.

These are implementation details. A reasonable user does not usually want to
disable them, so exposing them would commit the API to internals and create a
constructor full of confusing flags.

User-facing or debug-lab toggles:

- `auto_point_size`: users may need fixed point sizes when size encodes data.
- `lod_threshold` / `lod_factor`: users may choose the interaction quality
  tradeoff for very dense plots.
- `interactive_render_scale`: users may choose full-resolution interaction or
  faster lower-resolution interaction.

The rule: expose a toggle only when a reasonable user could reasonably want
the opposite of the default. Pi-specific builds should use different defaults,
not different renderer code paths.

## A/B Perf Lab Requirements

Add a dedicated visual lab, preferably `examples/scatter_perf_lab.py`, and
optionally mirror the most useful controls in `examples/all_features_v3_demo.py`.

The lab should include:

- Presets:
  - `Desktop defaults`
  - `Pi defaults`
  - `All off`
  - `Max quality`
- Workloads:
  - `125k static`
  - `125k streaming`
  - `1M static`
  - `5M static`
- Public tradeoff controls:
  - render scale slider
  - adaptive point size checkbox
  - LOD threshold slider
  - LOD factor slider/dropdown
- Readouts:
  - rolling FPS over roughly 60 frames
  - frame ms
  - upload ms
  - render encode ms
  - effective drawn points / total points
  - active LOD state
  - effective point size scale

Internal optimizations such as frustum culling, presort, and spatial indexing
may appear in the lab as diagnostic labels, but should not become general
`Scatter3D` constructor flags.

## Phase 1 - Baseline And Metrics

- Add debug counters for:
  - uploaded point count
  - effective drawn point count
  - active LOD factor
  - effective point size override
  - active render scale
  - scatter render pass time when available
- Add a dense scatter probe for:
  - 125k static points
  - 300k static points
  - interaction orbit stress
  - zoomed-in subset view
- Capture baseline behavior on desktop and Pi before optimization changes.

Exit criteria:

- We can tell whether a change improves fill cost, point count, or CPU cost.
- Existing scatter demos still render unchanged in default mode.

## Phase 2 - Adaptive Point Size

Dense point clouds spend a lot of time overdrawing the same pixels. Shrinking
points while zoomed out should be the first implementation because it is small
and likely high impact.

Status: started. Native now supports an `auto_point_size` flag and a
`point_size_scale` uniform. Debug snapshots report `point_size_scale` and
`effective_draw_point_count`. The first heuristic is screen-density based and
should be tuned with Pi measurements.

Implementation:

- Compute an effective point size per scatter viewport each frame.
- Use camera distance, projected bounds, or screen density to choose:
  - zoomed out: 1-2 px
  - normal: existing point size
  - zoomed in: 4-6 px if requested by data/style
- Respect explicit user point-size overrides as an upper bound or opt-out.
- Apply adaptive size through existing uniforms/override paths rather than
  rewriting point buffers.

Exit criteria:

- Dense scenes look visually equivalent when zoomed out.
- Overdraw is visibly reduced on Pi.
- Picking still expands candidate radius by the effective point size.

## Phase 3 - Interaction LOD Decimation

The existing LOD buffer should become the default interactive path for large
point clouds.

Status: partially implemented before this plan. `Scatter3D.set_lod(...)` and
constructor `lod`, `lod_threshold`, and `lod_factor` knobs now expose the
existing interaction LOD path. Extra actors are included by the native LOD
refresh path. LOD buffer construction is now lazy: ordinary idle uploads keep
only the full-quality vertex buffer, and the representative LOD buffers are
built when orbit or pan interaction starts. This keeps streaming/idle perf
measurements from paying hidden LOD rebuild cost when LOD is not being drawn.

Implementation:

- Enable LOD automatically when point count exceeds a threshold such as 50k.
- During orbit/pan/zoom:
  - draw a representative sample, for example 25%-50% of points
  - keep axes, labels, scalar bars, and UI chrome full quality
- When the camera settles:
  - return to full point count immediately or over a short recovery window
  - avoid reallocating point buffers during the transition
- Extend LOD to extra actors, not only the legacy primary point actor.

Exit criteria:

- Interaction is smoother on large clouds.
- Full quality returns after interaction without visual corruption.
- Multi-actor scatter scenes keep consistent density across actors.

## Phase 4 - Dynamic Render Scale

Render scatter content to a lower-resolution offscreen texture while
interacting, then upscale it into the widget rect with linear sampling. UI
chrome remains full resolution.

Status: started. Native now supports `interactive_render_scale` with an
interaction-only offscreen color/depth target and composite pass. Python exposes
`Scatter3D(..., interactive_render_scale=1.0)` and
`set_interactive_render_scale(scale)`, and the scatter perf lab includes a live
slider/preset coverage.

Implementation:

- Add a scatter offscreen render target sized by `render_scale`.
- Render only scatter scene content into the offscreen target.
- Composite the offscreen texture into the widget rect before full-resolution
  overlays/text.
- Use scale values:
  - 1.0 when static
  - 0.75 as the first low-power mode
  - 0.5 during heavy interaction or Pi stress mode
- Recreate the offscreen target only when viewport size or scale bucket changes.

Exit criteria:

- Scatter fill cost drops during interaction.
- Text, controls, legends, and menus stay sharp.
- Resize and DPI changes do not leave stale render targets.

## Phase 5 - Static Vs Dynamic Frame Budgeting

Use a frame-budget policy to combine point size, LOD, and render scale.

Status: started. Native now supports optional `auto_quality` budgeting with a
target FPS. The first implementation adjusts temporary interaction render
scale levels during orbit/pan based on recent CPU frame time and resets to full
quality when interaction stops. The scatter perf lab exposes an auto-quality
checkbox, target-FPS slider, and quality-level readout.

Implementation:

- Track recent scatter frame times with a small moving average.
- If interaction starts, immediately switch to interactive quality.
- If frame time remains over budget, progressively reduce quality:
  - point size down
  - LOD factor up
  - render scale down
- If the camera is still, restore quality over roughly 200 ms.
- Avoid quality flicker by using hysteresis and a minimum dwell time per mode.

Exit criteria:

- Low-power mode feels stable rather than rapidly toggling quality.
- Still frames become crisp.
- Interaction stays near the target 10 fps on Pi-class hardware.

## Phase 6 - CPU Frustum And Depth Culling

When the camera is zoomed into a subset of the cloud, avoid asking the GPU to
process points outside the view.

Implementation:

- Start with coarse CPU culling using projected bounds or actor-level AABBs.
- Add chunk-level culling:
  - split large point actors into fixed-size chunks
  - store each chunk's world-space bounds
  - skip chunks fully outside the frustum
- Avoid per-point CPU culling every frame unless the chunk path proves
  insufficient.

Exit criteria:

- Zoomed-in views draw fewer points.
- Culling overhead is lower than the GPU work it avoids.
- Streaming actors can update chunk bounds incrementally.

## Phase 7 - Depth, Blending, And Sorting Audit

Depth and transparency choices can dominate memory bandwidth on Pi.

Implementation:

- Audit point depth writes for opaque and translucent point modes.
- For opaque point sprites, test disabling depth writes or using a cheaper depth
  path while preserving the current visual result.
- If alpha blending requires sorting, verify whether sorting happens per frame.
- Prefer one of:
  - no per-frame sort
  - sort only after large camera changes
  - additive or weighted blending modes that do not need sorting

Exit criteria:

- No obvious visual regression in dense scenes.
- Per-frame CPU sorting is avoided unless explicitly required.
- Depth bandwidth is reduced where safe.

## Phase 8 - Spatial Index / Octree

Only implement this after the first three wins are measured. It is the largest
effort but can pay off for sparse or zoomed-in views.

Implementation:

- Start with a uniform grid or chunk tree before a full octree.
- Build once on upload for static actors.
- Update incrementally or opt out for high-rate streaming actors.
- Query visible chunks per camera frame.
- Combine with LOD so far-away chunks draw fewer points.

Exit criteria:

- Sparse views show a clear 2-4x improvement.
- Memory overhead is acceptable.
- Static and streaming point actors both have defined behavior.

## Ranked Work Order

| Optimization | Effort | Expected Pi Payoff | Phase |
| --- | --- | --- | --- |
| Adaptive point size | Small | 1.5-2x | 2 |
| LOD decimation during orbit | Medium | 2-3x | 3 |
| Render scale 0.75x during interaction | Medium | 1.5-2x | 4 |
| Static/dynamic quality budget | Medium | stacked gain | 5 |
| Coarse frustum/chunk culling | Medium | 1.3-2x | 6 |
| Depth/write/sort audit | Medium | workload-dependent | 7 |
| Octree/spatial index | Large | 2-4x sparse views | 8 |

The preferred first stack is adaptive point size, interaction LOD, and
interaction render scale. Together they should create enough headroom to make
300k+ point scenes plausible at the 10 fps Pi target.

## Testing Plan

- Add or extend a scatter performance probe with controllable:
  - point count
  - point size
  - low-power mode
  - LOD factor
  - render scale
  - camera interaction state
- Add debug snapshots for active quality mode and effective draw count.
- Verify:
  - default desktop rendering does not change
  - low-power mode responds during orbit/pan/zoom
  - full quality returns when interaction ends
  - picking remains accurate enough with adaptive point size and LOD
  - screenshots/export paths use full-quality mode unless explicitly requested

## Documentation Tasks

- Document the low-power scatter mode in `docs/widgets.md` once exposed.
- Add a V3 demo section showing full quality vs interaction quality if the mode
  becomes user-facing.
- Add implementation notes to this plan after each phase lands, including real
  before/after timing numbers from Pi hardware.
