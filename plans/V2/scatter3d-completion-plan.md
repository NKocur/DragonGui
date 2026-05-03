# Scatter3D Completion Plan

Status: Draft for implementation
Owner: DragonGUI V2
Source audit date: 2026-05-01

## Objective

Finish DragonGUI's embedded `Scatter3D` implementation enough that it behaves
like a real native widget inside DragonGUI layouts, instead of a single special
case renderer bolted onto the runtime.

This plan audits `J:\Projects\DragonSci` against the current DragonFrame code.
The goal is not to port every DragonSci feature into the base GUI library.
DragonSci already contains a broad plotting system; DragonGUI should absorb the
parts required for a reliable embedded scatter widget and leave heavier plotting
surfaces for a future dedicated widget package.

## Scope Decision

Keep in DragonGUI now:

- Multiple `Scatter3D` widgets in the same window.
- Correct startup and live point upload for each widget.
- Per-widget camera, bounds fitting, clipping, resizing, and input routing.
- Point sizing/coloring basics that are already visible in CSS probes.
- Point picking callbacks that work with multiple scatter widgets.
- A small public camera/control API matching the current examples.
- Focused CSS integration for point size/style and widget chrome.

Do not pull into DragonGUI now:

- `Line2D`, chart axes for general 2D plotting, `Figure`, subplots.
- Jupyter support, screenshot/GIF export, high-level statistical overlays.
- Convex hulls, ellipsoids, mesh actors, scalar bars, categorical legends,
  and label overlays unless they become required by DragonGUI examples.

Those heavier features are better candidates for a separate package such as
`dragonwidgets` or for keeping in DragonSci.

## Source Audit

### DragonSci Capabilities

DragonSci has a full Tk/wgpu plotting stack:

- Python public API in `J:\Projects\DragonSci\python\dragonsci\widget.py`.
  - `Scatter3D.set_points(...)` supports numpy and DataFrame inputs, colors,
    scalars, categorical color columns, size columns, per-point sizes, opacity,
    `clim`, `nan_color`, and `log_scale`.
  - `add_points`, `update_actor`, `remove_actor`, `set_actor_visibility`, and
    `clear` provide multi-actor scene management.
  - `add_stream`, `stream`, and `clear_stream` provide append/ring streaming.
  - Camera APIs include reset, presets, fit, get/set camera, flatten view, and
    parallel projection.
  - Visual APIs include point style, ticks, grid, grid planes, legend,
    background, axes, scalar bar, orientation axes, labels, lines, boxes, and
    mesh overlays.
  - Interaction APIs include point picking, rectangle selection, lasso
    selection, hover tooltips, and linked cameras.
- Rust/PyO3 boundary in `J:\Projects\DragonSci\src\lib.rs`.
  - `build_instances(...)` constructs `PointInstance` from positions, colors,
    scalars, sizes, colormaps, alpha, and log/clim options.
  - `PyScatterRenderer` exposes actor, stream, camera, picking, grid, overlay,
    screenshot, and chart2d methods.
- Renderer in `J:\Projects\DragonSci\src\renderer.rs`.
  - `Renderer` owns multiple actors, growable GPU buffers, stream actors,
    pick caches, grid/label buffers, scalar/legend overlays, lasso/selection
    overlays, line/mesh actors, orientation axes, chart2d state, and an
    offscreen path.
  - Rendering is scene-oriented: grid/lines, actors, meshes, overlays, and text
    are part of one plot renderer.

### DragonFrame Current State

DragonFrame has a much smaller embedded path:

- Python API is `Scatter3D(frame, x, y, z, colormap, on_pick, ...)` in
  `python/dragongui/widgets.py`.
- Startup document uses `ScatterSpec { colormap, data_b64 }` in
  `native/src/document.rs`.
- Live updates use only `SetScatterPointsPacked { id, xyz, telemetry,
  colormap }` in `native/src/commands.rs`.
- Native decoding accepts only packed `xyz` float32 triples in
  `native/src/runtime.rs`; color is assigned by point index, not by scalar or
  z value.
- Native renderer is `native/src/scatter/mod.rs`. It has one point pipeline,
  one vertex buffer, one camera, one point-size override, viewport/scissor
  clipping, and point picking.
- Runtime state in `native/src/runtime.rs` is singleton-based:
  `scatter: Option<ScatterWidget>`, `scatter_widget_id: Option<String>`,
  `scatter_decode_scratch: Vec<PointInstance>`, and one `ScatterMetrics`.
- Runtime discovers only the first scatter with `find_first_widget_kind_id` /
  `find_visible_scatter_id`. This is why probes with two `Scatter3D` widgets
  cannot render both plots.

## Gap Matrix

| Area | DragonSci | DragonFrame Today | Required Action |
| --- | --- | --- | --- |
| Multiple widgets | Many Tk widgets, each with a renderer | One singleton renderer for first scatter only | Replace singleton with per-widget scatter map |
| Point data | Positions, colors, scalars, sizes, opacity, clim, log scale | XYZ only, color by point index | Add versioned packet format or full instance upload |
| DataFrame support | pandas/polars column extraction, categorical color, size columns | Attribute-only `getattr(frame, col)` packing | Reuse/port robust column extraction and validation |
| Actors | Multi-actor add/update/remove/visibility | One full replacement buffer | Decide minimal actor API or explicit non-goal |
| Streaming | Append/ring actor buffers | Full re-upload on `set_points` | Stage after multi-widget/data correctness |
| Camera | Per-widget fit, reset, presets, get/set, parallel | One camera, reset key only, no Python API | Add per-widget state and commands |
| Bounds fit | Data-derived bounds | Default center/radius regardless of data | Compute bounds during decode/upload and fit on first data |
| Input routing | Widget-local orbit/pan/zoom/pick | Global scatter state | Track active scatter id and route events per widget |
| Picking | Actor/index/world point, rectangle/lasso, hover | Single-widget click pick only | Keep point picking now; stage selection/hover later |
| Visual plot chrome | Grid, ticks, axis labels, scalar bar, legend, labels | Widget background/border plus points only | Keep out of first fix; revisit after core stability |
| Point style | circle/square/gaussian | Shader has style field but runtime always uses circle | Add CSS/API point style |
| Depth handling | One plot per renderer, own depth clear | One depth clear for whole GUI pass | For multiple scatter, isolate depth per scatter pass |
| Debug snapshot | Plot is outside DragonGUI snapshot model | One scatter metrics object | Snapshot per scatter widget |

## Implementation Readiness Details

These are the details that should be settled before coding starts.

### Startup Data Ownership

Current startup scatter data is extracted outside the retained widget tree:

- `native/src/app.rs` calls `document::find_scatter_in_doc(&raw)`.
- `native/src/runtime.rs::AppSpec` carries `scatter: Option<ScatterSpec>`.
- `WgpuState::new(...)` creates one `ScatterWidget` from that one spec.

That shape is incompatible with multiple scatter widgets. The implementation
should remove the singleton startup path and move scatter data into the parsed
widget node:

- Add scatter fields to `NodeProps`, or a nested `ScatterProps` stored on
  `NodeProps`.
- Parse `colormap`, `data_b64`, and `data_format` from each `Scatter3D`
  node's props in `native/src/document.rs`.
- Remove `AppSpec.scatter` once all startup data comes from the tree.
- Build scatter runtimes by walking `widget_tree`, not by using
  `find_scatter_in_doc`.

Compatibility:

- Keep accepting current Python props where `data_b64` has no explicit format.
  Treat that as the existing packed `xyz_f32_v0` format.
- Do not generate implicit demo points for a data-backed `Scatter3D` that lacks
  packable data. Create an empty runtime and expose the missing payload in the
  debug snapshot instead. Examples that need a visible scatter should pass real
  packable arrays.
- Preserve the top-level `RunResult.upload_ms` result by aggregating initial
  scatter upload time across all startup scatter widgets. Per-widget upload
  timings should live in the new `scatters` debug object.
- Keep startup payloads in `data_b64` for this slice, even for
  `point_instance_v1`. A future resource/buffer startup path can remove the
  base64 JSON cost for very large point clouds, but it is outside this fix.
- Python should keep emitting compact `xyz_f32_v0` for the current constructor
  surface when no v1-only options are used. Emit `point_instance_v1` only when
  colors, scalars, per-point sizes, opacity, or explicit point size require
  baked instances.
- Do not include raw `data_b64` in debug snapshots. Snapshot the format, byte
  length, point count, and status instead.
- Startup decode failures should not crash the entire window unless there is a
  deliberate strict mode. Create the runtime with zero points, store the decode
  error in `payload_status`, and surface it in the debug snapshot. Live command
  decode failures can keep the current "log and ignore update" behavior.

Recommended native startup shape:

```rust
#[derive(Debug, Clone)]
struct ScatterPayload {
    data_b64: Option<String>,
    data_format: ScatterPayloadFormat,
    colormap: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScatterPayloadFormat {
    XyzF32V0,
    PointInstanceV1,
}
```

`NodeProps` can own either this nested struct or equivalent fields. The key is
that startup data travels with each `WidgetNode`, not in `AppSpec`.

### Component VDOM And Payload Lifetime

The component path has a separate failure mode from direct widget construction.
`widget_to_vnode(...)` calls `Scatter3D.props()` on every render, and the
generic VDOM diff currently treats scatter props as normal props. That means a
component re-render can produce `SET_PROP` patches for `frame`, `x`, `y`, `z`,
`colormap`, `data_b64`, or `data_format`.

That is not enough to update the native scatter today:

- `python/dragongui/runtime.py::AppHandle.apply_patch(...)` special-cases only
  table payload patches before falling back to generic `enqueue_set_prop(...)`.
- `native/src/runtime.rs::apply_set_prop(...)` does not handle scatter data
  props, so a same-identity `Scatter3D` whose data changes through a component
  re-render would not reliably update the renderer.
- `Scatter3D.props()` repacks and base64-encodes data every time it is called.
  With larger v1 packets, this can make component diffing expensive even before
  native sees an update.
- `shallow_value_equal(...)` compares scalar strings directly, so raw
  `data_b64` in VDOM props can become a large O(n) comparison on every render.

Add a scatter-specific VDOM/resource path analogous to the table special case:

- Do not rely on generic `SET_PROP(data_b64=...)` for scatter point updates.
- Treat the scatter payload as one resource-like update, for example
  `prop == "scatter"` or `prop == "scatter_points"`, containing payload bytes or
  a handle plus `payload_format`, `colormap`, and telemetry.
- `AppHandle.apply_patch(...)` should translate that scatter patch into
  `enqueue_set_scatter_points_packed(...)` with the proper payload format.
- Keep the old direct live setter path working; component patches and
  `Scatter3D.set_points(...)` should converge on the same native command.
- Avoid storing raw base64 strings in normal VDOM props once the resource path
  exists. Use a compact identity/version/length summary for diffing, or a
  `ResourceRef`-style handle, so re-rendering a component does not repeatedly
  compare huge strings.
- If the scatter VDOM path uses buffer resources instead of carrying bytes in
  the patch itself, queue the resource before the native command that references
  it. The current component runtime calls `_queue_startup_resources_for_patches`
  after `apply_patches(...)`, which is fine for table metadata today but would
  be the wrong ordering for a handle-only scatter payload.
- Cache the packed payload inside `Scatter3D` by source/options identity where
  practical. Invalidate on `set_points(...)`, `set_colormap(...)`, and any new
  color/size/scalar option change. Startup `to_dict()` can still include
  `data_b64` for this slice, but `props()` should not repack large arrays
  needlessly during retained component renders.
- If a scatter resource id is introduced, implement `_sync_after_id_change(...)`
  like `DataFrameTable` does so retained ids and resource ownership stay
  aligned after VDOM id retention.
- `REPLACE_NODE` and `REPLACE_CHILDREN` should keep working by decoding startup
  scatter payloads from the replacement node JSON. A separate
  `_queue_startup_resources()` override is not required until scatter startup
  data moves from inline base64 to a binary resource channel.

Tests should cover component re-rendering the same keyed `Scatter3D` with new
data, new colormap, and unchanged data. The unchanged-data case must not send a
large generic `SET_PROP(data_b64=...)` patch or force a repack.

### Render Order And Depth

Multiple scatter widgets must not depend on `HashMap` iteration order.

- Collect visible scatter ids from the widget tree in paint order during
  `apply_layout`. This should use the same z-index-aware sibling ordering as
  primitive/text traversal, not raw `children` order.
- Store that order in `visible_scatter_order`.
- Render in collected order.
- Scatter-vs-scatter hit-test/input should use the reverse collected order so
  the visually topmost scatter wins.
- The z-index-aware traversal currently exists as private duplicated
  `stacking_children(...)` helpers in `native/src/primitives/mod.rs` and
  `native/src/text/mod.rs`. Add a shared helper or mirror the behavior exactly;
  raw reverse child order is not enough once z-index is in play.

Depth isolation is part of the first multi-scatter slice, not a follow-up:

- Render base primitives/images in one pass.
- Render each visible scatter in its own pass with color `Load` and depth
  `Clear(1.0)`.
- Render text and overlay primitives after all scatters while preserving the
  existing overlay ordering: base text, primitive overlays, then text overlays.
- Keep the existing `text.prepare(device, queue)` step before opening render
  passes; splitting passes must not reintroduce glyph/depth borrow conflicts.
- Reset viewport and scissor to the full window before the final overlay pass.

Without the per-scatter depth clear, overlapping or z-indexed scatter widgets
can incorrectly occlude each other through stale depth values.

### CSS Paint Transforms, Relative Position, And Hit Testing

`Scatter3D` is viewport-rendered, while the CSS surface around it is emitted as
normal rect primitives. Rect primitives already follow paint-only transforms and
relative-position offsets, but the scatter renderer currently uses layout rects
and visible clips directly. If transforms are applied to a `Scatter3D` node or
an ancestor, the border/background can move, scale, or rotate while the actual
3D viewport remains anchored to the untransformed layout rect.

First-slice behavior should be explicit:

- Treat `Scatter3D` as layout-rect anchored. Do not claim support for
  `transform`, `translate`, `scale`, `rotate`, or relative paint offsets on the
  actual 3D viewport in this plan.
- Treat normal visual CSS as surface styling unless explicitly bridged into the
  scatter pipeline. `opacity`, filters, and pseudo-state visual changes on the
  `Scatter3D` node can affect the border/background primitive without fading or
  filtering the point cloud. For this slice, document that only
  border/background/radius/clipping plus scatter-specific point CSS affect the
  full widget coherently.
- Docs and probes should avoid transformed scatter examples unless the example
  is intentionally demonstrating that limitation.
- If a guard is feasible, ignore or suppress the transformed scatter surface so
  the border/background does not visually diverge from the viewport. If that is
  too invasive, document the mismatch clearly.
- A later transform-support slice can add translation-only or uniform-scale
  support by passing the same transformed rect/scissor into layout, rendering,
  picking, and camera aspect logic. Rotation is a separate problem because
  viewport/scissor rectangles and point picking are axis-aligned.

Do not let scatter-specific z-index work imply global z-index hit testing.
Existing `events.rs::hit_test(...)` walks children in reverse document order,
not z-index paint order, and docs already state that `z-index` does not affect
normal hit testing. Scatter hit testing should use the visible scatter order so
overlapping scatters target the visually topmost scatter, but normal UI
controls, popups, tables, text areas, and scrollbars must retain priority. Add
tests where a normal control overlaps a scatter to prove scatter input does not
steal events from the control.

### Live Command Migration

Current live commands use:

- Python: `LiveWidgetHandle.enqueue_set_scatter_points_packed(...)`
- Native sender: `NativeCommandSender.enqueue_set_scatter_points_packed(...)`
- Rust command: `Command::SetScatterPointsPacked { id, xyz, telemetry,
  colormap }`

The method name can remain for compatibility, but the payload needs an explicit
format.

Recommended migration:

- Add `payload_format: String` or a small Rust enum to
  `SetScatterPointsPacked`.
- Extend the PyO3 signature with a trailing optional
  `payload_format=None`, defaulting to `"xyz_f32_v0"`.
- Extend `AppHandle.enqueue_set_scatter_points_packed(...)` and
  `LiveWidgetHandle.enqueue_set_scatter_points_packed(...)` with the same
  optional keyword.
- Keep the old call shape valid for current tests and examples.
- In Python, avoid passing a sixth positional argument for default
  `xyz_f32_v0` updates unless needed. Several existing tests and fake native
  senders implement the old five-argument shape. The v1 path can pass the
  trailing format once the tests are updated.
- Fix telemetry point-count calculation so it is based on the payload format:
  `len / 12` for `xyz_f32_v0`, `len / size_of::<PointInstance>()` for the new
  instance format.

Recommended v1 format:

- `point_instance_v1`: tightly packed native `PointInstance` records:
  `x, y, z, size, r, g, b, alpha` as little-endian `f32`.
- Native validates the payload length, copies into `Vec<PointInstance>`, and
  computes bounds from positions.
- Python does high-level DataFrame/scalar/size/color normalization before
  sending the packet.
- Python must emit the wire format as explicit little-endian floats, for
  example via `<f4` arrays or `struct.pack("<f", ...)`, not merely native-endian
  `np.float32`. DragonSci passes typed arrays over PyO3, but DragonGUI's packet
  boundary is a byte protocol.
- Native should not recolor `point_instance_v1` with the command's `colormap`;
  colors are already baked into the packet. The `colormap` argument remains for
  compatibility, telemetry, and repacking from Python state.
- Decode bytes with explicit little-endian `f32::from_le_bytes` chunks or an
  equivalent unaligned-safe path. Do not directly `bytemuck::cast_slice` a
  Python/base64 byte buffer into `PointInstance`; byte buffers are not
  guaranteed to have Rust struct alignment and the packet format is explicitly
  little-endian.

This keeps Rust packet parsing small and reuses DragonSci's Python-side
ergonomics.

Packet validation tests should lock down:

- `size_of::<PointInstance>() == 32`.
- `point_instance_v1` byte order is little-endian `f32`.
- Invalid packet lengths fail with a diagnostic that names the payload format.
- `xyz_f32_v0` keeps accepting the current 12-byte-per-point payload.

### Runtime Input State

Current input state is global:

- `orbit_active: bool`
- `pan_active: bool`
- `scatter_press_pos: Option<[f32; 2]>`

Multi-scatter needs the active widget id:

- `active_scatter_id: Option<String>`
- `scatter_press: Option<(String, [f32; 2])>`
- `orbit_active`/`pan_active` can remain booleans, but all camera mutations
  must look up `active_scatter_id`.

Point picking on release must use the id captured on press, not whatever scatter
is under the release point after a drag or layout change.

Event priority from the current runtime needs to be preserved deliberately:

- Panel scrollbar drags, table hit testing, number steppers, menus/popups,
  focused text inputs, sliders, and normal controls keep priority over scatter.
- Scatter orbit/pan should run after those explicit widget/control hits but
  before generic background focus clearing.
- Mouse wheel routing should check text areas and tables first, then scatter,
  then generic scroll containers. Today `scroll_container_at(...)` runs before
  scatter zoom, so a scatter inside a scrollable panel can lose wheel input to
  the parent panel. The desired behavior is wheel over the plot zooms the plot;
  wheel elsewhere in the panel scrolls the panel. The scrollbar remains the
  reliable parent-scroll control when the pointer is over the plot.
- Scatter camera input must not depend on `hit_test_ui(...)`; `Scatter3D` is not
  included in `events.rs::is_interactive(...)` and should stay out of keyboard
  focus order.

### Fit State

`ScatterWidget` currently owns private `fit_center` and `fit_radius`, but only
the constructor sets them. Data uploads do not update the reset target.

Add one of:

- `ScatterWidget::fit_to_bounds(min, max, queue)` that updates camera,
  `fit_center`, and `fit_radius`.
- `ScatterWidget::set_fit(center, radius, reset_camera, queue)`.

Store `fitted_once` per scatter runtime so first data load fits automatically,
while later live updates do not surprise-reset a user-adjusted camera unless
the caller explicitly asks for `fit`/`reset`.

Bounds rules:

- Compute bounds from finite XYZ positions only. Non-finite positions should not
  expand bounds or break camera fitting. Decide whether to keep those points in
  the packet or drop them during packing; either choice needs a test. If kept,
  CPU picking must explicitly skip non-finite positions so NaN projections cannot
  become a false nearest-point hit.
- Empty payloads and all-non-finite payloads should set `point_count` to zero or
  mark `payload_status` as unusable, and should not call `Camera::fit` with
  infinite/NaN bounds.
- Clamp the radius used for fitting to a small positive value for flat or
  single-point data.
- Scrolling a clipped parent must change only the scatter scissor/visible clip,
  not the full viewport dimensions used for camera aspect. The existing
  `scatter_layout_rect(...)` behavior fixed this for the singleton path and
  must be preserved for each runtime.

### Debug Snapshot Compatibility

Keep broad compatibility with existing smoke checks while adding multi-scatter
details:

- `has_scatter`: true when at least one scatter runtime exists.
- `scatter_widget_id`: first visible scatter id or first runtime id for
  backwards compatibility.
- `scatters`: object keyed by widget id with per-widget point count, payload
  format, update metrics, visible rect, full rect, camera state summary, and
  payload status.
- Existing `scatter` metrics object can point to the first scatter for one
  release, but new tests should assert `scatters`.
- Keep `gpu.resources.scatter` as a backwards-compatible alias to the first
  scatter metrics object. `examples/streaming_scatter_tool.py` currently reads
  `snapshot["gpu"]["resources"]["scatter"]` directly.
- Add `gpu.resources.scatters` for the per-widget metrics map.
- Keep `gpu.renderer.has_scatter` and `gpu.renderer.scatter_widget_id` for one
  release; prefer `gpu.renderer.scatter_widget_ids` or the `scatters` keys in
  new probes/tests.

## Implementation Plan

### Phase 1: Replace Scatter Singleton With Per-Widget State

Goal: every visible `Scatter3D` node owns independent renderer state.

Native changes:

- Introduce a runtime container, for example:

```rust
struct ScatterRuntime {
    widget: ScatterWidget,
    points: Vec<PointInstance>,
    metrics: ScatterMetrics,
    fitted_once: bool,
    data_min: glam::Vec3,
    data_max: glam::Vec3,
    payload_format: String,
    payload_status: ScatterPayloadStatus,
}
```

- Replace these fields in `WgpuState`:
  - `scatter: Option<ScatterWidget>`
  - `scatter_widget_id: Option<String>`
  - `scatter_decode_scratch: Vec<PointInstance>`
  - `scatter_metrics: ScatterMetrics`
- With:
  - `scatters: HashMap<String, ScatterRuntime>`
  - `visible_scatter_order: Vec<String>` if deterministic render/input order is
    easier than walking the tree repeatedly.
- Replace `find_visible_scatter_id(...)` with a collector:

```rust
fn collect_visible_scatter_ids(tree: &WidgetNode, layout: &LayoutResult, out: &mut Vec<String>)
```

- In retained-map rebuilds, create missing scatter runtimes and drop stale ones.
- When `replace_children` or `replace_node` introduces a new `Scatter3D`, decode
  that node's startup payload exactly as startup would. Do not create an empty
  runtime if the replacement node contains valid `data_b64`.
- When a component re-render updates an existing `Scatter3D` with the same
  retained id, route the data change through the scatter point command path, not
  generic `set_prop`.
- During startup, initialize each runtime from that node's parsed scatter props.
- In `apply_layout`, update layout rect, visible clip, clip radii, and point
  size/style for every visible scatter.
- In `render`, draw all visible scatter widgets, not only one.

Depth/render pass requirement:

- Avoid cross-widget depth contamination:
  1. Render background/base primitives/images with the color attachment cleared.
  2. For each visible scatter, start a render pass with color `Load` and depth
     `Clear(1.0)`, then render only that scatter with its viewport/scissor.
  3. Render text and overlay primitives in a final color `Load` pass.

Tests:

- Unit test: two scatter nodes in one tree produce two scatter runtimes.
- Unit test: replacing/removing a subtree drops stale scatter runtimes and
  metrics without dropping unrelated scatters.
- Component/VDOM test: same-key `Scatter3D` data changes send a scatter payload
  update instead of generic `SET_PROP(data_b64=...)`.
- Component/VDOM test: unchanged scatter data does not repack or deep-compare a
  large base64 payload.
- Unit test: z-index changes `visible_scatter_order` and scatter-vs-scatter
  reverse hit testing.
- Smoke: `examples/css_feature_probes/data_widgets_probe.py` shows both compact
  and large scatter plots.
- Debug snapshot includes a list/map of scatter ids, point counts, and metrics.
- `RunResult.upload_ms` is nonzero/aggregated when startup scatters upload data.

### Phase 2: Fix Point Data Semantics

Goal: DragonGUI uploads actual plot point instances, not only `xyz` colored by
row index.

Minimum fix:

- Change current native `xyz` decoder to color by z value or supplied scalar
  range, not by point index.
- Compute `data_min` and `data_max` during decode.
- Store bounds in `ScatterRuntime`.
- Preserve `xyz_f32_v0` as compatibility only. It should use z-derived color
  when there is no scalar/color metadata, matching DragonSci's fallback more
  closely than row-index coloring.

Full target:

- Add a versioned scatter packet format. Keep the current `xyz` format as v0
  compatibility, then add v1 with enough metadata for:
  - positions
  - optional scalar values
  - optional RGB/RGBA colors
  - optional per-point sizes
  - uniform point size
  - opacity
  - colormap
  - clim
  - log scale
  - nan color
- Either:
  - build full `PointInstance` data in Python and upload `[x, y, z, size, r, g,
    b, alpha]`, or
  - upload typed columns and build instances natively.

Recommended first implementation:

- Build full `PointInstance` bytes in Python. It is simpler to integrate with
  DragonGUI's command queue and avoids a large native packet parser.
- Port the relevant Python-side pieces from DragonSci:
  - supported frame column extraction
  - series-to-numpy conversion for pandas/polars-like columns
  - `_normalize_sizes`
  - `_clamp_sizes`
  - categorical color detection for `color=` columns, including DragonSci's
    low-cardinality integer threshold, missing-value label handling, and stable
    legend order
  - scalar normalization, `clim`, `log_scale`, `nan_color`
- Reuse DragonGUI's existing `python/dragongui/dataframe.py` lookup behavior
  where possible so custom frames that implement `__getitem__` keep working.
  Current scatter packing uses `getattr(frame, col)` only, while tables already
  use `frame[column]` first and then attributes.
- Use explicit packet-building arrays with dtype `<f4` for both v0 and v1.
  Avoid structured dtypes whose padding/alignment could differ from the Rust
  `PointInstance` layout.
- Keep Rust validation strict: instance byte length must be a multiple of
  `size_of::<PointInstance>()`.
- Add startup props:
  - `data_b64`: base64 payload.
  - `data_format`: `"xyz_f32_v0"` or `"point_instance_v1"`.
- Add live-command fields:
  - `payload_format`.
  - telemetry point count calculated by payload format.

Python API target:

```python
dg.Scatter3D(
    frame,
    x="x",
    y="y",
    z="z",
    color=None,
    colors=None,
    scalars=None,
    size=None,
    point_sizes=None,
    point_size=4.0,
    colormap="viridis",
    opacity=1.0,
    clim=None,
    log_scale=False,
)
```

Conservative API boundary for this slice:

- Keep the current constructor shape working: `Scatter3D(frame, x="x", y="y",
  z="z", ...)`.
- Only expose new constructor/setter keywords when their packing path is fully
  implemented and tested. For example, `color=` should not become public until
  numeric and categorical columns both have deterministic behavior.
- Initial color support should match DragonSci's priority and shape rules:
  explicit `(N, 3)` RGB colors first, then scalar values, then z-derived
  colormap. Uniform `opacity` can set the packet alpha. Per-point RGBA can be a
  later extension unless it is implemented and tested end to end.
- Validate or clamp point size inputs consistently: `point_size`, CSS
  `scatter-point-size`, direct `point_sizes=`, and DataFrame `size=` with
  `size_range=` should not send negative, NaN, or infinite sizes to the shader.
- Do not add DragonSci's raw `(N, 3)` positions mode or `Scatter2D` behavior
  unless implementation needs it for probes. Those are reasonable future
  additions, but they are not required to finish the current CSS/widget probes.
- Keep `z` required in the first DragonGUI API slice unless the probes need 2D
  convenience. DragonSci fills missing DataFrame `z` with zero, but adding that
  behavior should be a deliberate compatibility decision because it changes the
  constructor contract.
- Do not add actor handles (`add_points`, `update_actor`, etc.) in the first
  data-correctness pass.

Live setters:

- `set_points(...)` accepts the same data options as construction.
- `set_colormap(...)` updates by repacking/reuploading current point data.
- CSS `scatter-point-size` remains an override and wins over packed sizes only
  when explicitly set.
- Add `scatter-point-style` to style dirty classification. It can be a visual
  invalidation unless computed-style snapshots require a broader update.
- If explicit per-point RGB/RGBA colors are active, `set_colormap(...)` should
  update the stored colormap property but not change point colors unless the
  user switches back to scalar/z coloring. Keep enough Python-side source state
  to make this deterministic.

Tests:

- Python packing tests for xyz-only, scalar colormap, explicit RGB, per-point
  sizes, opacity, pandas/polars-like column access, and invalid inputs.
- Native decode tests for v0/v1 packets and bounds computation.
- Compatibility test proving the existing `enqueue_set_scatter_points_packed`
  call shape still sends `xyz_f32_v0`.
- Tests for zero rows, NaN/Inf coordinates, NaN scalar colors, all-equal scalar
  ranges, invalid `clim`, and `point_sizes` length mismatch.
- Tests for low-cardinality categorical integer colors, missing categorical
  values, `size=`/`point_sizes=` mutual exclusion, `size_range` validation, and
  non-finite direct point sizes.
- Tests that table-style custom frames using `__getitem__` can be packed for
  scatter.

### Phase 3: Add Per-Widget Camera Control

Goal: scatter widgets fit real data and can be controlled by Python/examples.

Native:

- Store fit center/radius per scatter runtime.
- Fit camera from data bounds on first point upload and on explicit reset.
- Add commands for:
  - reset camera
  - view xy/xz/yz/isometric
  - set/get camera state if the command bridge can return state cleanly
  - set/get parallel projection
- Keep `CameraState` in `native/src/scatter/camera.rs`; it was copied from
  DragonSci and already has the right shape.
- Add `ScatterWidget` methods for `fit_to_bounds`, `set_view_direction`,
  `set_camera_state`, and `set_parallel_projection`, adapting DragonSci's
  camera math rather than duplicating it in runtime event code.
- Preserve DragonSci's reset behavior: reset uses the last fitted center/radius,
  preserves the current parallel/perspective mode, and should not cause the
  next normal `set_points(...)` to auto-fit over an explicit user reset.

Python:

- Add methods to `dragongui.widgets.Scatter3D`:
  - `reset_camera()`
  - `view_xy()`, `view_xz()`, `view_yz()`, `view_isometric()`
  - `fit(bounds=None)`
  - `set_camera(state)`
  - `get_camera()` only if backed by the existing synchronous debug snapshot or
    by a new command-reply path. The command bridge is currently one-way except
    for `debug_snapshot`, so do not invent a blocking getter without a clear
    transport.
  - `parallel_projection` property if command support is available
  - `colormap_names()` as a small public parity helper backed by the same
    allowed colormap set used by constructor validation.

Tests:

- Camera reset targets the requested widget id, not the first scatter.
- Data outside the default `[-5, 5]` region is visible after startup.
- Replacing points updates fit bounds but does not unexpectedly reset a user
  adjusted camera unless `fit` or reset is requested.
- `Home`/`R` reset applies to the currently active/hovered scatter when
  possible, and falls back to the first visible scatter only when no scatter is
  active.
- Parallel projection state round-trips through reset and camera get/set when
  those APIs are exposed.

### Phase 4: Route Scatter Input By Widget Id

Goal: mouse interaction affects the scatter under the pointer.

Native/runtime:

- Replace `scatter_contains(pos) -> bool` with `scatter_at(pos) -> Option<String>`.
- Track `active_scatter_id` for orbit/pan gestures.
- On left press over scatter, start orbit for that id.
- On Shift+left or middle drag over scatter, pan that id.
- On wheel over scatter, zoom that id.
- On release with small movement, point-pick that id.
- Preserve normal DragonGUI hit testing: UI controls, popups, modals, tables,
  text areas, and scrollbars keep priority.
- Preserve the documented normal hit-test order. Scatter-specific hit testing
  can use z-index-aware visible scatter order for scatter-vs-scatter cases, but
  it must not override a normal UI control that `hit_test_ui(...)` selected.
- Static `tooltip=` and hover styling can still use existing hover hit-testing;
  scatter camera input should not depend on `hit_test_ui` because `Scatter3D`
  is intentionally not a normal focusable control.

Resolved scroll behavior:

- If a scatter is inside a scrollable panel, wheel-over-scatter zooms the plot
  and wheel outside the scatter scrolls the panel. Scrolling the parent panel
  is still available through the panel scrollbar.

Tests:

- Two side-by-side scatter widgets: drag left plot does not move right plot.
- Wheel over scatter zooms that scatter.
- Wheel over a scatter inside a scrollable panel zooms the scatter; wheel over
  the same panel outside the scatter scrolls the panel.
- Scrollbar drag on a parent panel still scrolls the panel and does not orbit.
- Hidden/clipped scatter areas do not receive input.
- Press on one scatter, release over another, and verify the pick/camera action
  still targets the pressed scatter or is cancelled consistently.
- Right-click context menus and open menu popups keep priority over scatter pan.

### Phase 5: Make Point Picking Multi-Scatter Safe

Goal: `on_pick` works reliably once multiple scatter widgets exist.

Native:

- Store CPU point instances/positions per scatter runtime.
- `scatter_pick_payload(id, pos)` uses that widget's camera, viewport, clip,
  and data vector.
- Include optional future fields in native payload without breaking current
  Python:
  - `index`
  - `x`, `y`, `z`
  - `widget_id`
  - `actor_id` if actor support lands
  - `row_index` / `row_label` if DataFrame metadata lands

Python:

- Keep current callback compatibility:
  - one-arg callback receives `ScatterPick`
  - four-arg callback receives `index, x, y, z`
- Extend `ScatterPick` with optional `widget_id`/`actor_id` only after tests
  prove old code still works.

Tests:

- Click pick on the second scatter dispatches to the second scatter callback.
- Pick threshold respects CSS point size.
- Rounded/visible clip prevents picking outside the visible plot.
- Picking continues to work after a live `set_points(...)` update on only one
  of multiple scatters.
- Picking ignores removed/stale scatter ids after `replace_children` or
  `replace_node`.
- Non-finite point positions are never returned as pick hits.
- If packing drops invalid rows, pick payloads either expose the original row
  position/index through sidecar metadata or explicitly document that `index` is
  the packed point index for this slice.

### Phase 6: Fill Minimal Visual Controls

Goal: expose the renderer styling already implied by the shader and CSS probes.

Implement now:

- CSS/API point style:
  - `scatter-point-style: circle | square | gaussian`
  - Python `point_style` property
- CSS/API default point opacity:
  - only if full instance repacking makes it cheap.
- Keep DragonGUI primitive chrome for background, border, radius, shadow, and
  clipping. Do not add a separate plot background renderer unless required.

Files for CSS point style:

- `native/src/style.rs`: add `scatter_point_style`.
- `native/src/css_style.rs`: parse and cascade `scatter-point-style`.
- `native/src/runtime.rs`: include it in computed-style snapshots and pass it
  into each `ScatterWidget`.
- `native/src/scatter/mod.rs`: add a point-style setter that updates the
  uniform `style` field.
- `docs/css-styling.md`, `docs/css-capabilities-reference.md`, and
  `docs/widgets-reference.md`: document the property.

Validation:

- Accept only `circle`, `square`, and `gaussian`.
- Unknown values should produce the same style warning behavior as other
  unsupported/invalid CSS values.
- Python `point_style` should use the same values if it lands in this slice.

Defer unless examples require them:

- Grid and tick labels.
- Axis title labels.
- Scalar bar and categorical legend.
- Orientation axes overlay.
- World-space labels and line/box overlays.

If grid/axes become required, port DragonSci's `src/grid.rs` and line overlay
pipeline deliberately, but keep it behind a small API surface:

```python
scatter.show_grid(True)
scatter.set_axes(x="X", y="Y", z="Z")
scatter.set_ticks(x=None, y=None, z=None)
```

### Phase 7: Decide Actor And Streaming Boundary

Goal: avoid over-porting, but do not trap DragonGUI examples in full reupload
forever.

Recommended order:

1. Finish single-actor `set_points` per widget.
2. Add streaming only if `examples/streaming_scatter_tool.py` or benchmarks
   require better performance.
3. Defer general multi-actor scene APIs to a future widget package unless a
   DragonGUI example needs them.

If streaming lands in DragonGUI:

- Port DragonSci's growable/fixed-capacity stream model.
- Add commands:
  - `CreateScatterStream`
  - `AppendScatterStream`
  - `ClearScatterStream`
- Keep per-widget stream metrics in debug snapshots.

### Phase 8: Probe And Documentation Updates

Goal: close the visual probe loop before returning to broader CSS work.

Update or add probes:

- `examples/css_feature_probes/data_widgets_probe.py`
  - restore/keep two scatter widgets.
  - verify both render, differ by colormap, differ by CSS point size, and both
    can pick.
- `examples/css_feature_probes/widget_metrics_probe.py`
  - add a clear scatter point-size case after multi-scatter works.
  - include one scrollable panel case to ensure scale does not change while
    the parent scrolls.
- Add `examples/css_feature_probes/scatter3d_probe.py` if the cases become too
  large for the data/widget metrics probes.

Docs:

- Update `docs/css-capabilities-reference.md` for scatter CSS properties.
- Update `docs/css-styling.md` for scatter CSS properties and rounded clipping
  behavior.
- Update `README.md` or an example doc with the supported `Scatter3D` subset.
- Add a note that advanced plotting APIs remain outside DragonGUI for now.

Examples/smokes to keep compatible:

- `examples/streaming_scatter_tool.py` reads
  `snapshot["gpu"]["resources"]["scatter"]`.
- `examples/scatter_tool.py`, `examples/meridian.py`,
  `examples/all_features_demo.py`, `examples/all_features_css_demo.py`,
  `examples/css_web_capabilities_demo.py`, and
  `examples/css_feature_probes/data_widgets_probe.py` all exercise the current
  constructor/live-update surface.
- If `widget_metrics_probe.py` gets a scatter case again, it should verify that
  parent scrolling clips rather than resizes the plot viewport.

## Acceptance Criteria

Core completion:

- Two `Scatter3D` widgets in the same window render simultaneously.
- Each scatter has independent point data, point size/style, colormap, layout
  rect, visible clip, camera, and pick callback.
- Startup and live `set_points` work for each scatter id.
- Same-key component rerenders with changed scatter data update the correct
  native scatter instead of falling through to unsupported generic `set_prop`.
- Component rerenders with unchanged scatter data do not repack or compare large
  raw base64 payloads.
- Scatter packets are emitted and decoded as explicit little-endian data.
- Data bounds are used for initial camera fitting.
- Scrolling a parent panel does not shrink/reproject the scatter; visible clip
  only clips the viewport.
- Mouse drag/wheel/pick targets the scatter under the pointer.
- Paint transforms and relative visual offsets are either explicitly unsupported
  for the 3D viewport in docs/probes or guarded so the scatter surface cannot
  diverge from the viewport.
- Debug snapshots expose per-scatter ids, point counts, and update metrics.
- Existing Python API tests for current scatter construction, `set_points`,
  `set_colormap`, and `on_pick` continue to pass.
- `Scatter3D.colormap_names()` returns the supported colormap names.

Probe completion:

- `data_widgets_probe.py` visually shows both scatter panels.
- `widget_metrics_probe.py` shows the intended large-point plot.
- Rounded panel clipping still looks correct.
- `scatter_tool.py`, `streaming_scatter_tool.py`,
  `all_features_css_demo.py`, and `css_web_capabilities_demo.py` smoke-run
  after rebuilding the wheel.

Non-goal confirmation:

- No attempt is made in this plan to port DragonSci `Line2D`, `Figure`,
  Jupyter, GIF export, mesh overlays, hulls, or full chart2d support.

## Deep Audit Addendum

The following items came from the implementation-level pass and should be
treated as part of the coding checklist, not optional cleanup.

Current DragonFrame singleton coupling:

- `native/src/app.rs` still calls `document::find_scatter_in_doc(&raw)` before
  parsing the retained tree.
- `native/src/document.rs::ScatterSpec` only contains `colormap` and
  `data_b64`, and returns the first `scatter_3d` node.
- `native/src/runtime.rs::WgpuState::new(...)` creates one `ScatterWidget`,
  decodes one startup payload, and uses `gen_demo_points_with_colormap(...)`
  when the first scatter has no `data_b64`.
- `rebuild_retained_maps(...)` recreates only one singleton scatter when the
  first scatter id appears or changes.
- `set_scatter_points_packed(...)` validates the target id is a `Scatter3D`
  but then applies the update to the singleton renderer, so updating a second
  scatter would overwrite the first scatter.

Current render/input coupling:

- `render(...)` uses one render pass and one depth clear for the full GUI, then
  draws the singleton scatter before base text and overlays.
- `scatter_contains(...)` and `scatter_pick_payload(...)` have no widget id.
- `WindowRunner` has one `scatter_press_pos` and boolean orbit/pan state.
- Wheel routing currently checks generic scroll containers before scatter zoom.
- `events.rs::hit_test(...)` excludes `Scatter3D` from `is_interactive(...)`,
  so scatter-specific hit testing must remain separate.

Current Python/API coupling:

- `python/dragongui/widgets.py::_pack_xyz_bytes(...)` uses `getattr(frame,
  column)` only. It does not share the table column extraction path that already
  supports `frame[column]`.
- `Scatter3D.props()` emits `data_b64` but not `data_format`.
- `Scatter3D.props()` repacks and base64-encodes point data on every call, which
  includes component VDOM renders.
- `Scatter3D.set_points(...)` and `set_colormap(...)` can only send
  `xyz_f32_v0`.
- `python/dragongui/vdom.py::_diff_props(...)` has a table special case but no
  scatter special case, so scatter data changes become generic `SET_PROP`
  patches.
- `python/dragongui/runtime.py::AppHandle.apply_patch(...)` only translates
  `prop == "table"` into a resource update; scatter `data_b64` changes fall
  through to generic `enqueue_set_prop(...)`.
- Tests in `tests/test_python_api.py` use fake native senders with the old
  `enqueue_set_scatter_points_packed(widget_id, xyz, pack_ms,
  enqueue_epoch_ms, colormap)` signature.
- `Scatter3D` has no public `colormap_names()` helper even though the native
  colormap table and Python validation set already have the DragonSci names.
- Current packing emits native-endian `np.float32` bytes. The future packet
  format should be specified and emitted as little-endian.

Current CSS/layout coupling:

- `native/src/primitives/mod.rs` draws the `Scatter3D` background/border as a
  normal rect primitive, so it participates in paint transforms and relative
  positioning.
- The actual 3D renderer uses layout and visible rects from `LayoutResult`; it
  does not receive transformed paint rects. Transformed or relatively offset
  scatter surfaces can therefore diverge from the viewport, scissor, and picking
  region.
- Surface-level visual CSS such as opacity can affect the primitive shell
  without affecting the point shader. Treat that as a documented limitation
  unless opacity is explicitly bridged into point alpha/uniform state.
- `native/src/events.rs::hit_test(...)` is reverse document order rather than
  z-index paint order. Scatter-specific hit testing can be z-index-aware for
  scatter-vs-scatter, but global control-vs-scatter priority must remain
  explicit.

DragonSci details worth porting carefully:

- `_extract_frame_column`, `_series_to_numpy`, `_coerce_dataframe`,
  `_normalize_sizes`, `_clamp_sizes`, categorical detection, scalar
  normalization, and point-style validation are useful.
- `build_instances(...)` establishes the desired `PointInstance` layout and
  color priority: explicit colors, then scalars, then z-derived colormap.
- `Renderer::set_points(...)` reuses the existing scene actor buffer when the
  point count shrinks or stays under capacity. DragonFrame's
  `ScatterWidget::set_points(...)` already has similar grow-only buffer
  behavior; preserve it per scatter runtime.
- DragonSci's `reset_camera`, `set_view_direction`, `fit_to_bounds`,
  `get_camera_state`, `set_camera_state`, and `set_parallel_projection` are the
  camera methods to adapt.
- DragonSci `reset_camera` preserves the current parallel/perspective mode.
- DragonSci's DataFrame path supports `z=None` by filling zeros, categorical
  colors with a low-cardinality integer heuristic, `size=` normalization through
  `size_range`, row-position/index metadata, and hover metadata. Only expose the
  subset that DragonGUI can carry through its packet/callback model.

DragonSci details not to port in this slice:

- Multi-actor scene API, stream actors, grid/tick labels, scalar bar, legend,
  orientation axes, lasso/rectangle selection, hover tooltip cache, screenshot,
  GIF/Jupyter, mesh and line overlays, and chart2d.

## Suggested Implementation Order

1. Move startup scatter data from `AppSpec.scatter` into per-node props while
   preserving the current v0 payload.
2. Add the multi-scatter runtime map, visible order collection, and per-scatter
   render passes.
3. Route input and point picking by scatter id.
4. Compute data bounds and add per-widget fit/reset behavior.
5. Fix component/VDOM scatter data updates so same-key rerenders use the same
   scatter payload command path as live setters.
6. Add `point_instance_v1` packets and Python packing for colors/scalars/sizes.
7. Add public camera methods.
8. Add point style CSS/API.
9. Update probes and docs.
10. Optional streaming/actor follow-up only if needed.

## First Implementation Slice Detail

This is the smallest no-gap slice that should make the current probes coherent:

1. Add `ScatterPayloadFormat`, `ScatterPayloadStatus`, and per-node scatter
   payload props. Keep `xyz_f32_v0` as the default when `data_format` is absent.
2. Replace startup `AppSpec.scatter` with a tree walk that creates one
   `ScatterRuntime` per `Scatter3D` node and decodes that node's payload.
3. Replace singleton runtime fields with `HashMap<String, ScatterRuntime>` and
   add stale-runtime cleanup during retained tree rebuilds.
4. Add shared paint-order scatter collection and store `visible_scatter_order`
   during `apply_layout`.
5. Update `render(...)` to use base pass, one pass per visible scatter with a
   depth clear, and final overlay pass.
6. Replace `scatter_contains(...)` with `scatter_at(...)`, then route orbit,
   pan, wheel, reset, and pick by active scatter id.
7. Add bounds computation and first-upload fit for both startup and live
   `xyz_f32_v0`.
8. Add component rerender support for same-key scatter data updates without
   generic `SET_PROP(data_b64=...)`.
9. Add debug snapshot compatibility: existing `resources.scatter` alias plus
   new `resources.scatters`.
10. Run native tests and smoke `data_widgets_probe.py`/`streaming_scatter_tool.py`
   before starting `point_instance_v1`.

Then add data-correctness features:

1. Implement Python point-instance packing and native `point_instance_v1`
   decoding.
2. Add `payload_format` to commands while keeping default v0 call compatibility.
3. Add scalar/color/size/opacity tests.
4. Add public camera commands.
5. Add `scatter-point-style` CSS and Python `point_style`.

## File-Level Checklist

DragonFrame native:

- `native/src/app.rs`
  - Remove singleton `find_scatter_in_doc` wiring once per-node parsing is in
    place.
  - Keep result `upload_ms` as an aggregate of startup scatter uploads.
- `native/src/document.rs`
  - Add scatter fields to `NodeProps`.
  - Parse `colormap`, `data_b64`, and `data_format`.
  - Remove or deprecate `find_scatter_in_doc` after all startup scatter data is
    tree-owned.
- `native/src/runtime.rs`
  - Remove `AppSpec.scatter`.
  - Replace singleton fields with the scatter runtime map.
  - Add visible scatter collection and reverse-order scatter-vs-scatter hit
    testing.
  - Split render passes for per-scatter depth clears.
  - Add per-scatter debug snapshots.
  - Route new scatter commands and camera commands.
  - Reorder wheel dispatch so scatter zoom beats generic parent scrolling when
    the pointer is over a scatter viewport.
  - Preserve full layout rect for camera aspect and use visible rect only for
    scissor/clip.
  - Do not accept scatter data changes only through generic `set_prop`; route
    them through validated scatter payload commands or replacement-node decode.
  - Keep transformed/relative scatter behavior explicit: either suppress the
    transformed primitive mismatch or document that the viewport is layout-rect
    anchored for this slice.
- `native/src/scatter/mod.rs`
  - Add fit/camera setter methods.
  - Add point-style setter.
  - Keep rounded clip/scissor behavior from the current probe fixes.
  - Add packet layout tests for `PointInstance`.
- `native/src/commands.rs`
  - Add `payload_format` to scatter point updates.
  - Add camera commands if public camera methods land.
- `native/src/style.rs` and `native/src/css_style.rs`
  - Add `scatter-point-style`.
- `native/src/events.rs`
  - Leave `Scatter3D` out of normal focusable hit testing unless there is a
    deliberate keyboard-accessibility design; scatter camera input should use
    runtime-specific hit testing.
  - Preserve UI-control priority over scatter even when scatter ordering becomes
    z-index-aware for scatter-vs-scatter.
- `native/src/primitives/mod.rs` and `native/src/text/mod.rs`
  - Extract or mirror the existing z-index sibling traversal for scatter
    collection.
  - Account for the fact that the scatter surface primitive can be transformed
    while the 3D viewport cannot yet be transformed.

DragonFrame Python:

- `python/dragongui/widgets.py`
  - Port/trim DragonSci data prep for colors/scalars/sizes.
  - Emit `data_format` in startup props.
  - Send `payload_format` in live updates.
  - Add camera/point-style methods where backed by commands.
  - Add `Scatter3D.colormap_names()`.
  - Store enough source option state for `set_colormap(...)` and `set_points(...)`
    to repack deterministically.
  - Cache packed scatter payloads where practical so `props()` does not repack
    large arrays on every component render.
  - Emit explicit little-endian packet bytes and keep row-index sidecar metadata
    if rows can be filtered/dropped during packing.
- `python/dragongui/runtime.py`
  - Preserve existing live scatter enqueue compatibility.
  - Add payload format and camera command forwarding.
  - Translate scatter VDOM payload patches into scatter point commands instead
    of generic `enqueue_set_prop(...)`.
- `python/dragongui/vdom.py`
  - Add a scatter-specific diff path analogous to the table path.
  - Keep raw `data_b64` out of normal prop equality for large payloads; compare
    compact resource identity/metadata instead.
- `python/dragongui/components.py`
  - Ensure component patch resource detection includes scatter payload patches.
  - If scatter payload patches use resource handles, queue the resource before
    the command that references it rather than after patch application.
  - Keep replacement-node and replacement-children startup decode behavior
    aligned with the native tree-owned payload path.
- `python/dragongui/dataframe.py`
  - Reuse `_column_data`-style lookup for scatter packing instead of
    `getattr`-only packing.
- `python/dragongui/__init__.py`
  - Export new dataclasses/enums only if the public API adds them.

Tests and docs:

- `tests/test_python_api.py`
  - Add packet-format, packing, and callback compatibility tests.
- `tests/test_vdom.py`
  - Confirm added scatter props do not trigger deep equality on resource-like
    frames and do not break existing diff behavior.
  - Confirm same-key scatter component rerenders emit scatter payload updates,
    not generic `SET_PROP(data_b64=...)` patches.
  - Confirm unchanged scatter data does not repack or compare large raw payload
    strings.
- Native unit tests in `runtime.rs`, `document.rs`, `scatter/mod.rs`,
  `commands.rs`, `css_style.rs`, and `style.rs`.
- `examples/css_feature_probes/data_widgets_probe.py`
  - Restore the two-scatter comparison after multi-scatter is fixed.
- `examples/css_feature_probes/widget_metrics_probe.py`
  - Keep the scroll/point-size case.
- `examples/streaming_scatter_tool.py`
  - Either keep working through the `resources.scatter` alias or update to read
    a specific entry in `resources.scatters`.
- `docs/widgets.md`, `docs/widgets-reference.md`,
  `docs/css-styling.md`, `docs/css-capabilities-reference.md`, and
  `docs/library-overview.md`
  - Update the supported scatter subset.
  - State that `Scatter3D` styling supports border/background/radius/clipping
    and point CSS, but paint transforms, relative visual offsets, opacity, and
    visual filters do not move or fade/filter the actual 3D viewport in this
    slice unless explicitly implemented in the scatter pipeline.

This order fixes the current probe blockers first while keeping the larger
DragonSci feature surface from expanding the base GUI library prematurely.
