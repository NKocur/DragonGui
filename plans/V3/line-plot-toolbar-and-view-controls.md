# V3 Line Plot Toolbar And View Controls

DragonGUI's `LinePlot` has the start of a toolbar API, but it is not yet a
real plotting interaction system. Python exposes `show_toolbar`, and native
already has primitive hit handling for `Fit`, `Grid`, and `Axes`, but the
renderer currently disables the toolbar through `line_plot_toolbar_enabled(...)`.

This plan adds the familiar plotting controls users expect: fit/home, pan,
zoom, box zoom, grid/axis toggles, hover readout, and stable viewport state for
live streams.

## Implementation Status

- Phase 1 started.
- `show_toolbar=True` now enables the existing native toolbar when the plot is
  large enough to host it.
- `examples/css_feature_probes/line_plot_probe.py` now turns the main plot
  toolbar on by default and includes a live toolbar toggle.
- Remaining work starts at Phase 2: replace text toolbar buttons with compact
  icon-style controls and then add persistent viewport state.

## Current State

- `dg.LinePlot(..., show_toolbar=True)` exists in Python.
- `LinePlot.set_toolbar_visible(...)` can update `show_toolbar` live.
- Native document props include `line_plot_show_toolbar`.
- Native hit handling already recognizes toolbar button labels:
  - `Fit`
  - `Grid`
  - `Axes`
- Toolbar rendering exists as simple text buttons, but is disabled by:

```rust
fn line_plot_toolbar_enabled(_node: &WidgetNode, _rect: [f32; 4]) -> bool {
    false
}
```

- There is no persistent viewport model yet. The plot can auto-fit data bounds,
  but it cannot preserve manual zoom/pan limits as first-class state.
- There is no toolbar mode state for pan, zoom, or inspect.
- There is no back/forward view history.

## Goals

- Make `show_toolbar=True` actually display and handle a toolbar.
- Add professional icon-style toolbar controls instead of text-only buttons.
- Add persistent plot view state for manual x/y limits.
- Add mouse wheel zoom and drag pan.
- Add box zoom for selected ranges.
- Add hover/crosshair readout for nearest visible point.
- Keep toolbar controls compact enough for dashboard plots.
- Preserve parent scroll behavior: wheel zoom should only win when the plot is
  in an explicit zoom mode or when a modifier key is held.
- Add probe coverage in `examples/css_feature_probes/line_plot_probe.py`.

## Non-Goals

- Full Matplotlib parity.
- Export image/data in the first toolbar pass.
- Arbitrary annotations.
- Secondary axes.
- Multi-plot linked brushing in the first implementation.

## Proposed Python API

Initial constructor additions:

```python
plot = dg.LinePlot(
    frame,
    x="time",
    y="temperature",
    show_toolbar=True,
    toolbar=("fit", "pan", "zoom", "box_zoom", "grid", "axes", "inspect"),
    interaction="inspect",
)
```

Live methods:

```python
plot.fit()
plot.set_view(x=(0.0, 60.0), y=(10.0, 80.0))
plot.set_x_limits(0.0, 60.0)
plot.set_y_limits(10.0, 80.0)
plot.clear_view()
plot.set_interaction("pan")
plot.set_toolbar_visible(True)
```

Optional callbacks:

```python
plot = dg.LinePlot(
    frame,
    x="time",
    y="temperature",
    on_view_change=lambda view: print(view),
    on_point_hover=lambda point: print(point),
)
```

Keep callbacks optional. The first pass can expose viewport control without
callbacks if native-to-Python event plumbing makes callbacks risky.

## Native Data Model

Add plot viewport fields to native `NodeProps`:

- `line_plot_x_min: Option<f32>`
- `line_plot_x_max: Option<f32>`
- `line_plot_y_min: Option<f32>`
- `line_plot_y_max: Option<f32>`
- `line_plot_interaction: LinePlotInteraction`
- `line_plot_toolbar_items: Vec<LinePlotToolbarItem>`

Derived behavior:

- If all relevant limits are `None` and `auto_fit=True`, use data bounds.
- If the user pans or zooms, set manual limits and set `auto_fit=False`.
- `fit` clears manual limits and sets `auto_fit=True`.
- Streaming append should respect manual limits unless the user has chosen a
  rolling/follow mode.

Interaction enum:

```rust
enum LinePlotInteraction {
    Inspect,
    Pan,
    Zoom,
    BoxZoom,
}
```

## Native Commands

Add commands:

- `SetLinePlotViewport { id, x_min, x_max, y_min, y_max }`
- `FitLinePlotViewport { id }`
- `SetLinePlotInteraction { id, mode }`
- optional later: `PushLinePlotViewHistory`, `PopLinePlotViewHistory`

Dirty behavior:

- Viewport changes: `Dirty::GpuData` or `Dirty::Visual`, not layout.
- Toolbar mode changes: `Dirty::Visual`.
- Hover/crosshair movement: lightweight overlay redraw only.

## Toolbar Controls

Phase 1 controls:

| Control | Behavior |
| --- | --- |
| Fit/Home | Clears manual limits and fits all visible data. |
| Pan | Drag plot area to shift x/y limits. |
| Zoom | Wheel or vertical drag zooms around cursor. |
| Box Zoom | Drag rectangle, release to zoom into selected range. |
| Grid | Toggle grid visibility. |
| Axes | Toggle axes and ticks together. |
| Inspect | Hover crosshair and nearest point readout. |

Phase 2 controls:

| Control | Behavior |
| --- | --- |
| Back | Restore previous viewport. |
| Forward | Restore next viewport. |
| Lock X | Allow y zoom/pan while x remains fixed. |
| Lock Y | Allow x zoom/pan while y remains fixed. |
| Export | Later: save visible data or image. |

## Rendering

Toolbar appearance:

- Use compact icon-like primitives, not text labels.
- Button size: about `24x24` logical pixels.
- Use the existing rounded triangle/line primitive approach where possible.
- Use native tooltips or text overlays for labels if available.
- Hide or collapse lower-priority tools when the plot is too small.

Plot overlay rendering:

- Crosshair line at cursor.
- Small readout label with series label, x, and y.
- Box zoom rubber-band rectangle.
- Cursor feedback for active pan/zoom mode if feasible.

## Event Handling

Pointer behavior:

- Click toolbar button: update mode or toggle property.
- Double click plot area: fit/home.
- Wheel over plot:
  - default: parent scroll still works.
  - with Ctrl or active Zoom mode: zoom plot.
- Drag in Pan mode: pan viewport.
- Drag in Box Zoom mode: draw selection rectangle, zoom on release.
- Hover in Inspect mode: show crosshair and nearest point readout.

Important constraint:

The plot must not hijack normal page/panel scrolling by default. Many DragonGUI
examples use scrollable panels, so LinePlot wheel behavior must be explicit and
predictable.

## CSS Hooks

Add parts:

```css
LinePlot::toolbar {}
LinePlot::toolbar-button {}
LinePlot::toolbar-button-active {}
LinePlot::crosshair {}
LinePlot::readout {}
LinePlot::selection {}
```

Useful style properties:

- `plot-toolbar-background`
- `plot-toolbar-border-color`
- `plot-toolbar-icon-color`
- `plot-crosshair-color`
- `plot-selection-fill`
- `plot-selection-border-color`

The first version can map these to existing theme colors internally and expose
CSS parts afterward if part styling would delay interaction work.

## Probe Updates

Update `examples/css_feature_probes/line_plot_probe.py`:

- Set `show_toolbar=True` on the main plot.
- Add a compact note explaining expected interactions.
- Add a side plot with `show_toolbar=True` to test small-size behavior.
- Add test data with clear spikes so zoom and inspect are obvious.
- Add buttons to programmatically call:
  - `fit()`
  - `set_view(...)`
  - `set_interaction("pan")`
  - `set_interaction("inspect")`
- Add a streaming case where manual zoom should not be overwritten by append.

## Implementation Phases

### Phase 1: Enable The Existing Toolbar

- Replace the hard-coded `false` in `line_plot_toolbar_enabled`.
- Respect `node.props.line_plot_show_toolbar`.
- Add reasonable size thresholds so tiny sparkline plots do not show controls.
- Keep existing `Fit`, `Grid`, and `Axes` actions working.
- Update `line_plot_probe.py` to show this first toolbar.

### Phase 2: Make The Toolbar Look Native

- Replace text buttons with compact icon-like controls.
- Keep hit targets stable and larger than the visual icon.
- Add text labels through the overlay label path only if needed.
- Verify the toolbar does not shrink the plot too much.

### Phase 3: Persistent Viewport

- Add x/y limit props in Python and native.
- Teach bounds calculation to prefer manual limits over auto-fit bounds.
- Add `fit()`, `set_view(...)`, `set_x_limits(...)`, `set_y_limits(...)`.
- Ensure append/live data updates preserve manual limits.

### Phase 4: Wheel Zoom And Pan

- Add active interaction mode.
- Add plot-area hit testing separate from toolbar hit testing.
- Implement wheel zoom around cursor.
- Implement drag pan.
- Avoid stealing parent scroll unless interaction mode/modifier requires it.

### Phase 5: Box Zoom And Hover Inspect

- Add drag selection rectangle.
- Convert selection rectangle back into data-space limits.
- Add nearest-point lookup for hover.
- Render crosshair/readout overlay.

### Phase 6: View History And Polish

- Add back/forward viewport stack.
- Add keyboard shortcuts if DragonGUI has a stable key event path.
- Add debug snapshot fields for current view and interaction mode.
- Add tests for viewport persistence across data updates.

## Tests

Python:

- Constructor accepts toolbar configuration.
- Invalid toolbar items and interaction modes raise clear errors.
- `fit()` and `set_view(...)` enqueue native commands.
- `set_data(...)` and `append_points(...)` do not clear manual view unless
  explicitly requested.

Native:

- `show_toolbar=True` creates toolbar hit regions.
- Toolbar hit regions are not active when hidden or too small.
- Fit/Grid/Axes actions mutate the right props.
- Manual viewport changes affect bounds calculation.
- Wheel zoom and pan preserve finite, ordered bounds.

Probe/manual:

- Main plot toolbar is visible.
- Grid/Axes toggles visibly update the plot.
- Fit restores full data after zoom/pan.
- Streaming append does not snap the view unexpectedly.
- Parent scroll panels still scroll normally when not in zoom mode.

## Acceptance Criteria

- A user can enable a useful plot toolbar with `show_toolbar=True`.
- The toolbar has a professional compact default appearance.
- Users can fit, pan, zoom, and inspect data without writing custom callbacks.
- Manual view state survives streaming updates.
- Plot interaction does not break normal DragonGUI scroll behavior.
