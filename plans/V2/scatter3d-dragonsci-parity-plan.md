# Scatter3D DragonSci Parity Plan

> Historical V2 plan. Current CSS and Scatter3D styling follow-up work is
> tracked in `plans/V3/css-and-scatter-followup-plan.md`.

Status: Implementation audit/update
Owner: DragonGUI V2
Source audit date: 2026-05-01
Full source pass: 2026-05-01

2026-05-02 update: the checklist below is preserved as the original
implementation plan, not the current source truth. Current DragonFrame has most
of the seven phases implemented. The latest source audit against
`J:\Projects\DragonSci` identified these remaining parity fixes: orthographic
grid scaling, explicit segment/update line overlays, mesh overlay depth/sorting,
legend/scalar-bar stacking, `point_style` property parity, and hover tooltip
documentation.

## Objective

Finish the missing DragonSci-style `Scatter3D` surface in DragonFrame after the
core native widget work. The current DragonFrame implementation can render
embedded scatter widgets with v0/v1 point packets, per-widget cameras, picking,
CSS point styling, and multiple widgets. It does not yet include DragonSci's
plot chrome, scene actors, overlays, or richer interaction APIs.

This plan covers the missing `Scatter3D` features from the full DragonSci source
tree, especially `J:\Projects\DragonSci\python\dragonsci\widget.py`,
`jupyter_widget.py`, `figure.py`, and the native support in
`J:\Projects\DragonSci\src\renderer.rs` / `src\grid.rs`. It intentionally does
not fold the separate DragonSci `Chart2D`, `Line2D`, `Figure`, Jupyter, or
notebook export stack into the base `Scatter3D` widget. Those belong in a
separate chart/widget package if we want full plotting-library parity.

## Current DragonFrame Baseline

Already implemented in DragonFrame:

- Multiple `Scatter3D` widgets in one window.
- Startup and live `xyz_f32_v0` and `point_instance_v1` payloads.
- Point positions, scalar colors, explicit RGB colors, categorical colors,
  per-point sizes, point opacity, `clim`, `nan_color`, `log_scale`, and
  `size_range`.
- Colormap registry and `Scatter3D.colormap_names()`.
- Per-widget camera commands: reset, fit, preset views, get/set camera, and
  parallel projection.
- Point style support: `circle`, `square`, `gaussian`.
- Point picking callbacks routed to the correct scatter widget.
- Rounded clipping, parent scroll clipping, z-index ordering, and debug
  snapshot entries for scatter resources.

Originally missing from DragonFrame:

- Grid box, grid planes, ticks, tick labels, axis title labels, axis visibility.
- DragonSci-compatible `show_grid`, `show_grid_planes`, `set_ticks`,
  `set_axes`, `set_axis_visibility`, and `set_background`.
- Categorical legend, scalar bar, orientation axes.
- World-space user labels.
- Line and box overlays.
- Multi-actor scene API: `add_points`, `update_actor`, `remove_actor`,
  `set_actor_visibility`, `clear`.
- Streaming actors: `add_stream`, `stream`, `clear_stream`.
- Mesh/statistical overlays: convex hulls, ellipsoids, cluster hulls,
  cluster ellipsoids.
- Rectangle selection, lasso selection, hover/pick metadata beyond the point
  index and coordinates.
- DragonSci-compatible picking controls: `enable_point_picking`,
  `enable_rectangle_picking`, `enable_lasso_picking`, `disable_picking`,
  `hover_tooltip`, `picked_point`, `picked_index`, `picked_actor`, and
  `selected`.
- DragonSci's explicit interaction LOD path.
- Screenshot/GIF/export and camera-link helpers.
- `Scatter2D`'s 2D camera lock and marginal histograms.
- `Line2D` / native `Chart2D` line chart APIs.
- `Figure` subplot layout and subplot scalar-bar targeting.
- Jupyter/offscreen framebuffer wrappers.

## Source Feature Map

| DragonSci feature | DragonSci source | DragonFrame status | Plan phase |
| --- | --- | --- | --- |
| Nice bounds/ticks | `src/grid.rs` | Missing | Phase 1 |
| Grid box and tick marks | `src/grid.rs`, `renderer.rs` | Missing | Phase 1 |
| Grid planes, major/minor | `show_grid_planes` | Missing | Phase 1 |
| Axis labels and visibility | `set_axes`, `set_axis_visibility` | Missing | Phase 1 |
| Plot background API | `set_background` | CSS-only today | Phase 1 |
| Categorical legend | `_refresh_legend`, `set_legend` | Metadata not retained | Phase 2 |
| Scalar bar | `scalar_bar`, `set_scalar_bar` | Missing | Phase 2 |
| Orientation axes | `show_orientation_axes` | Missing | Phase 2 |
| World labels | `add_label`, `update_label`, `remove_label` | Missing | Phase 3 |
| Line overlays/boxes | `add_lines`, `add_box` | Missing | Phase 3 |
| Point actors | `add_points`, `update_actor` | Single scene payload only | Phase 4 |
| Streaming actors | `add_stream`, `stream` | Full re-upload only | Phase 4 |
| Selection/hover/lasso | selection methods and event bindings | Missing | Phase 5 |
| Picking controls | `enable_point_picking`, `disable_picking` | Basic `on_pick` only | Phase 5 |
| Hover tooltips | `hover_tooltip`, hover metadata | Missing | Phase 5 |
| LOD during interaction | `_engage_lod`, renderer LOD pipeline | Missing | Phase 5 |
| Mesh/stat overlays | convex hull / ellipsoid helpers | Missing | Phase 6 |
| Export/camera links | screenshot/GIF/link helpers | Missing | Phase 7 |
| Flat-view helpers | `flatten_view`, `set_parallel_scale`, `get_view_bounds_2d` | Partial camera helpers only | Phase 7 |
| Scatter2D | `Scatter2D`, marginals | Missing | Separate plan |
| Line2D / Chart2D | `Line2D`, `chart2d_*` native methods | Missing | Separate plan |
| Figure | `Figure` subplot grid and shared cameras | Missing | Separate plan |
| Jupyter/offscreen | `JupyterScatter3D`, `RenderSurface::Offscreen` | Missing | Separate plan |

## Full Source Pass Notes

The second source pass covered these DragonSci files:

- `python/dragonsci/widget.py`
  - Public classes: `Scatter3D`, `Scatter2D`, `Line2D`.
  - Top-level helpers: `link_cameras`, `unlink_cameras`.
  - Scatter-specific surface includes actors, streams, labels, lines, boxes,
    meshes, statistical overlays, legends, scalar bar, grid, axes, picking,
    lasso, hover tooltips, LOD, export, camera links, and camera helpers.
  - `Scatter2D` is a constrained `Scatter3D` subclass that locks orthographic
    2D behavior, zeros Z coordinates, hides Z axis, and adds marginal
    histogram canvases.
  - `Line2D` uses the same native renderer but drives the dedicated `chart2d_*`
    path: axis limits, tick intervals/formatters, log scales, title, legend,
    spans/reference lines, cursor, box zoom, toolbar, and line streams.
- `python/dragonsci/figure.py`
  - `Figure` owns a grid of `Scatter3D` widgets, supports row/column access,
    linked cameras, equal-aspect cells, and scalar-bar targeting to selected
    subplot rows/columns/cells.
- `python/dragonsci/jupyter_widget.py`
  - `JupyterScatter3D` wraps an offscreen renderer through `jupyter_rfb`,
    handles pointer/wheel events, exposes points/actors/camera/grid/scalar
    bar/background/screenshot/label/picking APIs, and can save PNGs.
  - `JupyterScatter2D` mirrors the 2D locking behavior.
- `src/lib.rs`
  - `PyScatterRenderer` exposes all native commands: point actors, streams,
    camera, offscreen render, chart2d, scalar bar, legend, picking, selection,
    lasso, overlays, orientation axes, LOD, grid/axes/background, labels,
    mesh actors, and colormap listing.
- `src/renderer.rs`
  - Renderer state includes growable buffers, actor registries, stream info,
    screen pick caches, screenshot/offscreen cache, grid/label buffers, scalar
    bar and legend overlay buffers, lasso/selection overlays, line overlays,
    mesh actors, chart2d state, and LOD pipeline state.
- `src/grid.rs`
  - Provides nice bounds, tick steps, axis ticks, log ticks, formatted ticks,
    camera-face detection, grid geometry, grid labels, flat-axis suppression,
    and 2D high-aspect tests.
- `src/shaders`
  - `points.wgsl`, `points_lod.wgsl`, `lines.wgsl`, `mesh.wgsl`, and
    `chart2d_lines.wgsl`.
- Tests
  - `tests/test_smoke.py` is the broad API contract for Scatter3D, Scatter2D,
    Line2D, overlays, picking, export, camera links, grid/axes, and point
    styling.
  - `tests/test_labels.py`, `test_statistical_overlays.py`, `test_marginals.py`,
    `test_figure.py`, and `test_jupyter.py` define additional behavior that
    should inform follow-up plans.

Scope conclusion:

- This plan remains the correct plan for bringing DragonSci's missing
  `Scatter3D` capabilities into DragonFrame.
- `Scatter2D`, `Line2D`/Chart2D, `Figure`, and Jupyter/offscreen rendering are
  now explicitly audited but should not be silently bundled into this base
  Scatter3D implementation. Each deserves a separate V2 plan after this one or
  belongs in a future `dragonwidgets`-style package.

## Architecture Direction

Do not add each feature as an unrelated special case. Convert the native scatter
path into a small scene renderer inside `native/src/scatter`.

Proposed modules:

- `native/src/scatter/grid.rs`
  - Port DragonSci's `nice_bounds`, `axis_ticks`, `tick_step`,
    `format_tick`, and `build_grid`.
  - Keep DragonGUI-specific types small and independent of DragonSci's Tk
    assumptions.
- `native/src/scatter/lines.wgsl`
  - Line-list pipeline for grid, axis ticks, boxes, orientation axes, scalar
    bar marks, and simple overlays.
- `native/src/scatter/points_lod.wgsl`
  - Optional sampled point rendering path for Phase 5. Keep disabled until LOD
    is explicitly supported and visible in debug snapshots.
- `native/src/scatter/mesh.wgsl`
  - Mesh pipeline for Phase 6 statistical overlays. Keep separate from line
    overlays because alpha/depth ordering is different.
- `native/src/scatter/scene.rs`
  - `ScatterSceneState`, actor registry, bounds merge, chrome state, overlay
    state, and metadata used by runtime snapshots.
- `native/src/scatter/text.rs` or a `TextRendererDg` extension
  - Project world labels and grid tick labels into screen-space text entries
    clipped to the scatter viewport.

Key native state:

```rust
struct ScatterChromeState {
    grid_visible: bool,
    major_grid_planes: bool,
    minor_grid_planes: bool,
    tick_override: [Option<usize>; 3],
    axis_labels: [String; 3],
    axis_visible: [bool; 3],
    background_color: Option<[f32; 3]>,
    legend: LegendState,
    scalar_bar: ScalarBarState,
    orientation_axes_visible: bool,
}

struct ScatterActor {
    handle: u32,
    visible: bool,
    points: Vec<PointInstance>,
    bounds_min: glam::Vec3,
    bounds_max: glam::Vec3,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_cap: u64,
}
```

The existing `ScatterRuntime` can own this state initially. If the file grows
too large, split the current `ScatterWidget` into `ScatterRenderer` plus
`ScatterSceneState`.

## Phase 1: Grid, Ticks, Axes, Background

Goal: make the missing "grid options and everything" visible in the dedicated
probe without adding actors or overlays yet.

Python API:

- Add `Scatter3D.show_grid(visible: bool = True)`.
- Add `Scatter3D.show_grid_planes(major: bool = True, minor: bool = False)`.
- Add `Scatter3D.set_ticks(x: int | None = None, y: int | None = None,
  z: int | None = None)`.
- Add `Scatter3D.set_axes(x: str = "X", y: str = "Y", z: str = "Z")`.
- Add `Scatter3D.set_axis_visibility(x: bool = True, y: bool = True,
  z: bool = True)`.
- Add `Scatter3D.set_background(color)` for DragonSci compatibility. CSS
  remains the preferred static styling path, but the method should work live.

Native/API work:

- Add commands in `native/src/commands.rs` and `python/dragongui/runtime.py`:
  - `SetScatterGridVisible`
  - `SetScatterGridPlanes`
  - `SetScatterTicks`
  - `SetScatterAxes`
  - `SetScatterAxisVisibility`
  - `SetScatterBackground`
- Parse optional startup props in `native/src/document.rs` so grid state works
  before the first live command.
- Port DragonSci `src/grid.rs` into `native/src/scatter/grid.rs`.
- Add a line-list render pipeline and grid vertex buffer to scatter rendering.
- Rebuild grid geometry when data bounds, camera orientation, grid options, or
  layout size change.
- Project grid labels through the scatter camera and clip them to the widget's
  visible rect.
- Ensure grid depth behavior is correct:
  - Grid lines render behind points when they are on the back/floor planes.
  - Tick labels and axis titles render as 2D text overlays on top.
  - Multiple scatter widgets do not share depth state.

Testing:

- Unit tests for `nice_bounds`, `axis_ticks`, flat-axis suppression, and tick
  override behavior.
- Python API tests that each method validates inputs and enqueues the expected
  native command.
- Runtime smoke test with one scatter using grid planes and custom axes.
- Dedicated probe section for:
  - grid on/off
  - major planes
  - major plus minor planes
  - custom tick counts
  - custom axis labels
  - hidden Z axis
  - live background update

Acceptance:

- A rounded/clipped scatter panel shows grid lines and tick labels fully inside
  the plot viewport.
- Tick labels do not pile up when the camera is aligned to an axis.
- Scrolling a parent panel does not reproject or resize the grid unexpectedly.

## Phase 2: Legend, Scalar Bar, Orientation Axes

Goal: restore the screen-space plot overlays that users expect from DragonSci.

Python/data work:

- Preserve categorical legend metadata from the current categorical color path:
  title, labels, and RGB swatches.
- Preserve scalar range metadata from scalar/color-mapped paths:
  `vmin`, `vmax`, `log_scale`, `colormap`, and optional title.
- Add `Scatter3D.show_legend(visible: bool = True)`.
- Add `Scatter3D.legend_position` property with:
  `top-right`, `top-left`, `bottom-right`, `bottom-left`.
- Add `Scatter3D.scalar_bar(...)` matching DragonSci:
  `visible`, `vmin`, `vmax`, `log_scale`, `colormap`, `title`.
- Add `Scatter3D.show_orientation_axes(visible: bool = True)`.

Native/API work:

- Add legend/scalar/orientation commands to `commands.rs` and runtime handles.
- Extend scatter startup props and scatter VDOM patch payload with metadata.
- Add screen-space line/quad geometry helpers for:
  - legend swatches and frame
  - scalar color strip or segmented line strip
  - orientation mini-axis lines in the scatter viewport
- Add screen-space text entries for legend labels, scalar ticks/title, and
  orientation axis labels.
- Clip all overlays to the scatter viewport unless the overlay intentionally
  sits inside its own margin area.

Testing:

- Python tests for categorical legend metadata generation and command queuing.
- Native tests for scalar bar tick generation, including log scale.
- Smoke probe with categorical legend, scalar bar, and orientation axes enabled
  at the same time.

Acceptance:

- Categorical plots can display their legend without user-supplied legend data.
- Scalar plots can display a scalar bar that matches the active colormap and
  scalar range.
- Orientation axes stay fixed in the plot corner while the main camera moves.

## Phase 3: Labels, Lines, Boxes

Goal: add lightweight world-space annotation and overlay primitives before
heavier mesh/statistical overlays.

Python API:

- `add_label(position, text, color=(1,1,1), size=14, anchor="center") -> int`
- `update_label(handle, position=None, text=None, color=None, size=None,
  anchor=None)`
- `remove_label(handle)`
- `set_label_visibility(handle, visible)`
- `clear_labels()`
- `add_lines(points, color=(1,1,1)) -> int`  <!-- width deferred: DragonSci does not expose it; thick-line rendering requires quad geometry -->
- `add_box(bounds, color=(1,1,1)) -> int`    <!-- width deferred: same reason -->
- `remove_overlay(handle)`
- `set_overlay_visibility(handle, visible)`
- `clear_overlays()`

Native/API work:

- Add handle registries for labels and line overlays per scatter widget.
- Reuse the line pipeline from Phase 1 for line overlays and boxes.
- Project label anchors each frame and place text through the shared text
  overlay path.
- Keep overlay handles stable across startup and live commands.

Testing:

- Python handle lifecycle tests.
- Native smoke where labels survive camera movement and clipping.
- Probe section with labels, polyline, and bounding box toggles.

Acceptance:

- Labels follow their world positions and remain readable during orbit/pan.
- Overlay visibility/removal does not affect point actors or grid state.

## Phase 4: Actors and Streaming

Goal: support DragonSci's multi-actor scene model without forcing users to
rebuild the entire point cloud for every layer or stream update.

Python API:

- `add_points(...) -> int`
- `update_actor(handle, positions, ...)`
- `remove_actor(handle)`
- `set_actor_visibility(handle, visible)`
- `clear()`
- `add_stream(positions=None, max_points: int, mode: "ring" | "append", ...) -> int`
- `stream(handle, positions, ...)`
- `clear_stream(handle)`

Native/API work:

- Replace the single point buffer in `ScatterWidget` with an actor registry or
  make the existing full-scene buffer actor `0`.
- Merge bounds across visible actors for camera fitting and grid extents.
- Keep CPU point caches per actor for picking and future selection.
- Add point pick payload fields:
  - `actor`
  - `index`
  - `x`, `y`, `z`
  - optional original row/index metadata if Python provides it later.
- Add stream actors with fixed-capacity GPU buffers:
  - append mode stops at capacity
  - ring mode overwrites oldest points
  - bounds update conservatively, with optional full recompute when needed.

Testing:

- Actor add/update/remove/visibility Python tests.
- Native tests for merged bounds and handle lifecycles.
- Smoke probe with three independently colored actors and a live stream.

Acceptance:

- Hiding/removing an actor updates grid bounds and picking.
- Streaming does not reallocate every frame in steady state.

## Phase 5: Selection, Hover, and LOD

Goal: restore richer interaction while keeping behavior explicit and visually
predictable.

Python API:

- `on_select` callback for rectangle/lasso-selected indices.
- Optional `on_hover` callback or hover tooltip formatter.
- `set_lod(enabled: bool = True, threshold: int = 200_000, factor: int = 8)`.
- `enable_point_picking(on_pick=None)`.
- `enable_rectangle_picking(on_select=None)`.
- `enable_lasso_picking(on_select=None)`.
- `disable_picking()`.
- `hover_tooltip` property for DragonSci-style built-in tooltips.
- Public state mirroring DragonSci where useful: `picked_point`,
  `picked_index`, `picked_actor`, and `selected`.

Native/API work:

- Add rectangle selection and lasso hit testing against per-actor CPU points.
- Add a screen-space pick cache equivalent to DragonSci's `ScreenPickCache`
  when point counts make brute-force hit testing too expensive.
- Add event payloads with actor/index arrays and optional world coordinates.
- Preserve DataFrame row/index metadata where Python can provide it.
- Add transient selection rectangle and lasso path overlays.
- Add an explicit LOD state, disabled unless configured or enabled by default
  above a documented threshold.
- If LOD is enabled, draw sampled points during interaction only and restore
  full density immediately after interaction stops.
- Surface active LOD state in debug snapshots so it is never confused with an
  opacity or shader fade issue.

Testing:

- CPU selection unit tests for viewport transforms and actor routing.
- Probe with a dense point cloud and an on-screen LOD status label.

Acceptance:

- Selection callbacks report stable actor/index data.
- LOD never changes opacity, color, or point size; it only changes sampled draw
  count during interaction.

## Phase 6: Mesh and Statistical Overlays

Goal: port the heavier DragonSci overlays only after the line/actor foundation
is stable.

Python API:

- `add_convex_hull`, `update_convex_hull`
- `add_ellipsoid`, `update_ellipsoid`
- `add_cluster_hulls`
- `add_cluster_ellipsoids`
- `remove_mesh`
- `set_mesh_visibility`
- `clear_meshes`

Native/API work:

- Add a mesh pipeline with depth testing and alpha blending.
- Keep mesh actors separate from point actors and line overlays.
- Prefer Python-side geometry generation for hulls/ellipsoids, matching
  DragonSci's dependency boundary.
- Add command payloads for mesh vertices, indices, color, opacity, wireframe.

Testing:

- Python geometry/validation tests.
- Native render smoke with translucent hulls over points.

Acceptance:

- Mesh visibility/removal works without disrupting point actors or grid.
- Translucent overlays render in a predictable order for common cases.

## Phase 7: Export and Camera Linking

Goal: decide which non-visual DragonSci helpers belong in DragonFrame versus a
future plotting package.

Candidate API:

- `screenshot()`
- `save_png(path)`
- `open_gif(path, fps=20, loop=0)`, `write_frame()`, `close_gif()`
- `orbit_gif(path, n_frames=60, fps=20, loop=0, elevation=None,
  on_progress=None)`
- Camera linking between scatter widgets: `link_cameras`, `unlink_cameras`.
- `flatten_view(plane)`, `get_view_bounds_2d()`, and
  `set_parallel_scale(half_w, half_h)`.

Implementation notes:

- Screenshot/export is more complex in DragonFrame because scatter widgets are
  embedded inside a full GUI surface. Decide whether capture means the widget
  viewport only or the whole window.
- Camera linking can be implemented at the Python layer by listening for camera
  changes only if native emits camera-change events. Otherwise it needs a
  polling or command fan-out mechanism.
- `flatten_view` is a small camera helper and can be pulled earlier if probes
  need 2D inspection before export work.
- `get_view_bounds_2d` and `set_parallel_scale` are mainly needed for
  Scatter2D/Line2D parity, but implementing them with camera helpers avoids
  duplicating camera math later.

## Explicit Separate Plans

These source-library areas were audited and are intentionally not part of the
base Scatter3D parity path:

- `Scatter2D`
  - Thin subclass of `Scatter3D` with 2D input coercion, Z-zeroing, permanent
    parallel projection, locked camera presets, hidden Z axis, and marginal
    histogram canvases.
  - Needs a separate plan if DragonFrame should expose first-class 2D scatter
    rather than treating it as a camera preset.
- `Line2D` / native `Chart2D`
  - Dedicated line chart mode with axis limits, tick intervals, tick formatters,
    log scales, legends, spans/reference lines, cursor, box zoom, toolbar,
    static lines, and line streams.
  - This is a charting widget, not a Scatter3D feature. It should be planned as
    a separate widget/library surface.
- `Figure`
  - Tk subplot container that instantiates multiple `Scatter3D` widgets,
    links cameras, maintains equal-aspect cells, and targets scalar bars by
    row/column/cell.
  - DragonFrame already has layout primitives; Figure should be redesigned as
    a composition helper rather than directly ported.
- Jupyter/offscreen wrappers
  - `JupyterScatter3D` uses `jupyter_rfb`, offscreen render targets, JPEG frame
    transport, and notebook pointer/wheel event handling.
  - DragonFrame's embedded runtime should not absorb this unless notebook use
    becomes a product requirement.

## CSS Integration

Initial implementation should prioritize DragonSci-compatible Python methods.
After that, add CSS properties where they make sense for static styling:

- `scatter-grid-visible: true | false`
- `scatter-grid-planes: none | major | minor`
- `scatter-axis-x-label`, `scatter-axis-y-label`, `scatter-axis-z-label`
- `scatter-axis-x-visible`, `scatter-axis-y-visible`, `scatter-axis-z-visible`
- `scatter-legend-position`
- `scatter-orientation-axes: true | false`

Avoid overloading general CSS layout/grid properties. All scatter-specific
properties should use the `scatter-` prefix.

## Probe Work

Extend `examples/css_feature_probes/scatter3d_probe.py` after each phase:

- Phase 1 adds a "Grid and axes" section with buttons for grid on/off, planes,
  minor planes, tick counts, custom labels, hidden axis, and background.
- Phase 2 adds categorical legend, scalar bar, and orientation axes.
- Phase 3 adds labels, lines, and box overlays.
- Phase 4 adds actors and streaming.
- Phase 5 adds selection/hover/LOD.
- Phase 6 adds mesh/statistical overlays.

The probe defaults should stay opaque and non-LOD so renderer defects are easy
to see. Soft Gaussian points, transparency, and LOD should be opt-in controls.

## Test Checklist

Python/unit:

- API method validation and command enqueue tests in `tests/test_python_api.py`.
- VDOM diff tests for new scatter props/metadata in `tests/test_vdom.py`.
- Payload metadata tests for legend/scalar metadata.
- Use DragonSci's source tests as behavior references:
  - `tests/test_smoke.py` for broad Scatter3D API behavior.
  - `tests/test_labels.py` for label lifecycle and clipping.
  - `tests/test_statistical_overlays.py` for mesh/statistical overlay geometry.
  - `tests/test_marginals.py`, `tests/test_figure.py`, and
    `tests/test_jupyter.py` for separate follow-up plans.

Native/unit:

- `cargo test --manifest-path native/Cargo.toml --target x86_64-pc-windows-gnu scatter`
- Grid tick generation, bounds expansion, and label suppression.
- Actor handle lifecycle and bounds merge.
- Selection hit testing.

Smoke/manual:

- Build wheel and copy `_dragongui.pyd` into `python/dragongui`.
- Run `examples/css_feature_probes/scatter3d_probe.py`.
- Run `examples/css_feature_probes/widget_metrics_probe.py` to ensure scatter
  and scroll clipping still coexist.
- Visually inspect rounded panels, scroll clipping, z-order, overlay text, and
  dense point clouds.

## Implementation Order

Recommended order:

1. Phase 1 grid/ticks/axes/background.
2. Phase 2 legend/scalar/orientation overlays.
3. Phase 3 labels/lines/boxes.
4. Phase 4 actors and streaming.
5. Phase 5 selection/hover/LOD.
6. Phase 6 meshes/statistical overlays.
7. Phase 7 export/camera-link helpers.

This order gets the missing visible plot controls into the probe first, then
adds scene composition, then interaction and heavier plotting features.
