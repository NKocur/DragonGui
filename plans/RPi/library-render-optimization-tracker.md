# Raspberry Pi Library Render Optimization Tracker

Last updated: 2026-05-18

## Goal

Find library-level rendering improvements before relying on point decimation.
The outline-ring fix showed that small renderer details can create large Pi
costs, especially when they shade large areas or force driver/GPU waits.

## Current Known Wins

| Optimization | Status | Result |
| --- | --- | --- |
| Rounded solid-rect fast path | Implemented | `1280x720 controls` improved from ~13.3 FPS to ~18.9 FPS. |
| Split large outline rings on Pi | Implemented | Real WASP no-points improved from ~13 FPS to ~41 FPS. |

## Candidate Areas

### 1. No-Depth UI Primitive Pipelines

Why:

- Base and overlay UI primitives are 2D and currently use depth-stencil
  attachments.
- The Pi GL path may pay extra cost for depth attachment load/store/clear.

Plan:

- Add benchmark-only no-depth primitive pipelines or pass mode.
- Compare base primitives with and without depth attachment/state.
- Start with the real WASP no-points case after split outlines.

Results:

| Scenario | Mode | Wall FPS | Submit | Present | Notes |
| --- | --- | ---: | ---: | ---: | --- |
| WASP phase scatter, no points, 1280x720 | baseline after split outlines | 39.01 | 2.97 ms | 9.13 ms | Normal depth-backed base/overlay passes. |
| WASP phase scatter, no points, 1280x720 | base primitives no-depth | 37.35 | 21.42 ms | 0.44 ms | No improvement; work moved from present to submit and total FPS was lower. Overlay cannot drop depth yet because glyphon text pipeline requires it. |
| WASP phase scatter, 150k fake points, 1280x720 | baseline after split outlines | 11.32 | 50.75 ms | 0.85 ms | `--debug-points 150000 --debug-autoinject wave --payload xyz`. |
| WASP phase scatter, 150k fake points, 1280x720 | base primitives no-depth | 11.28 | 54.81 ms | 0.69 ms | No practical gain. |

Decision:

- Do not ship no-depth primitive pipelines as an optimization. The measured
  path is slightly slower on both empty and 150k-point cases, and the overlay
  pass cannot remove depth independently until the text pipeline is changed.

### 2. Large Fill Simplification

Why:

- After split outlines, the largest remaining base primitives are huge rounded
  solid panel fills.
- The rounded solid fast path is much cheaper than the general shader, but it
  still evaluates an SDF over large panel areas.

Plan:

- Add benchmark-only option to simplify large panel fills on Pi:
  - square/no-radius for large filled surfaces, or
  - split rounded large rect into center strips plus small corner quads, or
  - profile-specific reduced radius for large containers.
- Compare with the real WASP no-points case and synthetic controls.

Results:

| Scenario | Mode | Wall FPS | Submit | Present | Notes |
| --- | --- | ---: | ---: | ---: | --- |
| WASP phase scatter, no points, 1280x720 | baseline after split outlines | 37.19 | 7.76 ms | 24.27 ms | Large base rounded area: ~1.66M px. |
| WASP phase scatter, no points, 1280x720 | square-cut large fills | 42.20 | 3.04 ms | 10.01 ms | Fastest, but removes large panel corner rounding. Benchmark-only via `DRAGONGUI_BENCH_SIMPLE_LARGE_ROUNDED_FILLS=1`. |
| WASP phase scatter, no points, 1280x720 | split large rounded fills | 40.12 | 6.90 ms | 25.22 ms | Preserves rounded corners by splitting large solid fills into simple center/edge rects plus small rounded corner rects. Promoted to Pi default. |
| WASP phase scatter, 150k fake points, 1280x720 | baseline after split outlines | 10.98 | 51.75 ms | 0.65 ms | `--debug-points 150000 --debug-autoinject wave --payload xyz`. |
| WASP phase scatter, 150k fake points, 1280x720 | square-cut large fills | 11.70 | 48.40 ms | 0.74 ms | Good speed, but visual corners are not preserved. |
| WASP phase scatter, 150k fake points, 1280x720 | split large rounded fills | 11.81 | 47.64 ms | 1.14 ms | Best 150k result while preserving rounded corners. |

Decision:

- Ship split large rounded fills as the Pi default. It improves real WASP
  no-points rendering by about 8% and 150k-point rendering by about 7.5% while
  preserving visible rounded corners. Keep the square-cut large-fill mode as a
  benchmark-only comparison because it can visibly change panel geometry.

### 3. Redundant Full-Area Fill Audit

Why:

- Nested panels may emit multiple large opaque/translucent fills over the same
  area.
- On Pi, overdraw from large fills is costly.

Plan:

- Use primitive stats to list largest base fills in real WASP and synthetic
  probes.
- Identify whether any full-area fills are visually redundant.
- Add local style/profile changes only if they preserve layout and readability.

Results:

| Scenario | Largest Fill | Area | Alpha | Mode | Notes |
| --- | --- | ---: | ---: | --- | --- |
| WASP phase scatter, no points, 1280x720 | `dg-23` plot panel / `dg-24` Scatter3D overlap | 773,136 px | 1.0 | simple_square | Outer plot panel fill. Mostly under Scatter3D, but visible as the surrounding frame. |
| WASP phase scatter, no points, 1280x720 | `dg-24` Scatter3D body | 689,000 px | 0.72 | simple_square | Large translucent scatter body/background. This is the main remaining overdraw interaction. |
| WASP phase scatter, no points, 1280x720 | `dg-3` navigation panel | 116,112 px | 1.0 | simple_square | Visible left navigation background, not redundant. |
| WASP phase scatter, no points, 1280x720 | opaque large-fill experiment | 689,000 px | forced 1.0 | simple_square | `DRAGONGUI_BENCH_OPAQUE_LARGE_FILLS=1` changed visuals and was slower: 40.23 FPS vs 41.71 FPS default. Removed after test. |
| WASP phase scatter, 150k fake points, 1280x720 | opaque large-fill experiment | 689,000 px | forced 1.0 | simple_square | 11.77 FPS, effectively tied with split-fill default and not worth the visual change. |

Decision:

- No obvious fully redundant full-area fill was found. The big draws are real
  UI surfaces: outer plot panel, Scatter3D body, and nav panel. The only
  suspicious overdraw is the translucent Scatter3D body over the outer plot
  panel. Forcing large translucent fills opaque did not improve no-points
  performance and changes colors, so do not ship that. Keep the new
  `layout_matches` audit metadata in the primitive performance snapshot for
  future emitter attribution.

### 4. Scatter3D Empty/Chrome Skip

Why:

- Scatter3D was not the main no-points culprit, but skipping truly empty work is
  still a general library cleanup.

Plan:

- Detect when Scatter3D has no meaningful points and no visible chrome requiring
  render.
- Skip pass or reduce work when safe.

Results:

| Scenario | Mode | Wall FPS | Submit | Notes |
| --- | --- | ---: | ---: | --- |
| WASP phase scatter startup, 8 placeholder points, 1280x720 | default | 42.47 | 5.47 ms | Startup uses 8 placeholder points to establish bounds/chrome, so it is not truly empty. |
| WASP phase scatter startup, 8 placeholder points, 1280x720 | benchmark skip `<=8` points | 51.54 | 2.87 ms | Big win, but hides startup Scatter3D grid/axes/placeholder view. Rejected as a default library behavior. |
| WASP phase scatter, 150k fake points, 1280x720 | benchmark skip `<=8` points | 11.52 | 47.13 ms | Confirms threshold experiment does not skip real data. |
| WASP phase scatter startup, 8 placeholder points, 1280x720 | shipped safe no-work guard | 41.57 | 4.76 ms | No visual change; no practical speedup for WASP because the widget has real placeholder/chrome work. |

Decision:

- Ship only the safe no-work guard: the runtime now asks Scatter3D whether it
  has any scene work before creating a scatter render pass. This avoids empty
  passes for truly empty Scatter3D widgets without hiding chrome/backgrounds.
- Do not ship point-count threshold skipping. It proved the upper-bound value
  of skipping startup placeholder work, but it would be app-specific and could
  hide intentional grid/axis/placeholder content.

### 5. Render Pass Count / Surface Load-Store

Why:

- The runtime uses separate base, Scatter3D, and overlay passes.
- Each pass may add surface/depth load-store overhead on Pi.

Plan:

- Benchmark pass disabling already exists.
- Add targeted experiments to merge or avoid passes only where safe.
- Measure before/after on real WASP no-points and 150k points.

Results:

| Scenario | Mode | Wall FPS | Submit | Present | Notes |
| --- | --- | ---: | ---: | ---: | --- |
| WASP phase scatter startup, 8 placeholder points, 1280x720 | baseline | 40.71 | 8.89 ms | 23.47 ms | Depth/stencil stored for base, scatter, and overlay passes. |
| WASP phase scatter startup, 8 placeholder points, 1280x720 | discard depth/stencil stores for UI + scatter | 42.11 | 4.60 ms | 10.57 ms | Preserves visuals, but scatter path showed mixed behavior with 150k points. |
| WASP phase scatter, 150k fake points, 1280x720 | baseline | 11.82 | 37.60 ms | 0.70 ms | `--debug-points 150000 --debug-autoinject wave --payload xyz`. |
| WASP phase scatter, 150k fake points, 1280x720 | discard depth/stencil stores for UI + scatter | 11.66 | 45.30 ms | 0.70 ms | Slightly worse; do not discard scatter depth stores. |
| WASP phase scatter startup, 8 placeholder points, 1280x720 | discard depth/stencil stores for UI only | 44.39 | 2.75 ms | 9.04 ms | Best safe startup result. Scatter depth store remains unchanged. |
| WASP phase scatter, 150k fake points, 1280x720 | discard depth/stencil stores for UI only | 11.89 | 37.91 ms | 1.32 ms | Neutral/slightly better than baseline while preserving visuals. |

Decision:

- Ship UI-only depth/stencil store discard as the Pi default. Base and overlay
  passes use depth pipelines only to satisfy pipeline compatibility; their depth
  contents are not needed by later passes. Scatter passes keep storing depth to
  avoid the 150k-point regression seen when discarding scatter depth stores.

## Recommended Order

1. No-depth primitive pipeline/pass benchmark.
2. Large fill simplification benchmark.
3. Redundant full-area fill audit.
4. Empty Scatter3D/chrome skip.
5. Render pass merge/load-store experiments.
