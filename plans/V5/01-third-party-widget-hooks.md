# V5 Third-Party Widget Hooks

Status: in progress.

## Objective

Add a first-class extension path for developers to build reusable DragonGUI
widgets outside the core library.

The goal is to support two levels of third-party widgets:

1. Composite widgets built from existing DragonGUI widgets.
2. Custom painted widgets that can draw their own content, receive input, and
   participate in layout and CSS without requiring a native Rust plugin.

Native dynamic plugin loading is intentionally not the first target. A safe
Python-facing display-list API should cover most user needs with much less
packaging, ABI, and safety risk.

## Current State

DragonGUI already has partial foundations:

- `@component` supports reusable Python widgets composed from existing widgets.
- `Widget` and `Container` can be subclassed and serialized through `to_dict()`.
- Native document parsing preserves unknown widget kinds as `WidgetKind::Unknown`.
- Unknown widgets can still contribute a generic leaf to layout.
- Raw props are preserved on nodes, which is useful for future extension APIs.
- Escape hatches such as `HtmlReport` and `Image` can host externally rendered
  content, but they are not first-class native widgets.

Important gaps:

- Unknown widgets do not render custom content.
- Unknown widgets cannot define custom measure or intrinsic sizing.
- Unknown widgets do not have a custom event contract.
- Unknown widget type names cannot be targeted by type selectors.
- There is no lifecycle contract for widget-owned resources.
- There is no stable third-party API for custom drawing.

## Proposed API

### CompositeWidget

Composite widgets are pure Python wrappers around existing widgets and should be
documented as the simplest extension point.

```python
@dg.component
def StatusTile(ctx, title: str, value: str, level: str = "normal"):
    with dg.Panel(class_="status-tile"):
        dg.Text(title, class_="status-title")
        dg.Text(value, class_=f"status-value {level}")
```

This mostly needs documentation, examples, and stability guarantees around
component state, keys, class names, and styling.

### PaintWidget

Add a custom painted widget base that emits a constrained display list.

```python
class Sparkline(dg.PaintWidget):
    def __init__(self, values, **kwargs):
        super().__init__(**kwargs)
        self.values = values

    def measure(self, constraints: dg.MeasureConstraints) -> dg.Size:
        return dg.Size(width=160, height=44)

    def paint(self, ctx: dg.PaintContext) -> None:
        ctx.rect(0, 0, ctx.width, ctx.height, radius=6, fill="surface")
        ctx.polyline(self.values, stroke="accent", width=1.5)

    def on_pointer_down(self, event: dg.PointerEvent) -> None:
        ...
```

Initial drawing commands:

- `rect`, `rounded_rect`
- `line`, `polyline`
- `text`
- `circle`
- `path` or a small subset of path commands
- `image`
- `clip_rect`

Initial input events:

- pointer enter/leave
- pointer move
- pointer down/up
- click/double click
- wheel
- key down/up when focused

### ExtensionWidget

Internally, native should stop treating extension nodes as generic
`Unknown` for first-class widgets. Add a distinct extension kind:

```json
{
  "type": "extension",
  "props": {
    "extension_type": "sparkline",
    "display_list": [...],
    "intrinsic_width": 160,
    "intrinsic_height": 44
  }
}
```

The public Python API can expose `PaintWidget`; the serialized document can use
an internal `extension` widget type to keep native behavior predictable.

## CSS And Styling

Custom widgets should be styleable with existing class and inline style support.

V5 should add:

- A stable `ExtensionWidget` type selector or `paint-widget` selector.
- Custom extension type matching, for example:

```css
PaintWidget.sparkline {
  color: var(--text);
  background: var(--surface);
  border-color: var(--border);
}
```

- Style token lookup from paint commands:

```python
ctx.rect(..., fill="surface")
ctx.polyline(..., stroke="accent")
ctx.text(..., fill="text-muted")
```

- Optional named parts once the paint API needs them:
  - `canvas`
  - `label`
  - `handle`
  - `marker`

CSS should not require every custom widget to invent hard-coded colors.

## Layout Contract

Paint widgets need predictable sizing without hand-tuned demo styles.

Minimum contract:

- `measure(constraints) -> Size`
- default intrinsic size if no measure function is supplied
- `min_width: 0` compatible behavior inside flex/grid/panels
- respect explicit `width`, `height`, `min_width`, `max_width`,
  `min_height`, and `max_height`
- opt-in scroll clipping through existing panel/scroll containers

Native layout should use the measured or intrinsic size just like built-in leaf
widgets.

## Event Contract

Events should be explicit and Python-friendly.

Initial event data:

- widget id
- local x/y
- window x/y
- button/modifiers
- wheel delta
- click count
- key text/code for focused widgets

Callbacks should be queued through the existing live command/event path rather
than opening a second event channel.

Focus behavior:

- widgets can opt into focus with `focusable=True`
- focus ring uses the same framework styling as buttons/inputs
- keyboard events are delivered only to the focused custom widget

## Resource Lifecycle

Third-party widgets will need resources for images, cached data, and possibly
large display lists.

Initial resource rules:

- Paint commands may reference `Image`/`ResourceRef` values.
- Large display lists should be diffable or resource-backed.
- Resource cleanup should happen when the owning widget id leaves the document.
- Live updates should allow a custom widget to update props/display-list data
  without rebuilding the whole app.

## Implementation Plan

### Phase 1: Document And Stabilize Composite Widgets

- [x] Add documentation for `@component` and `Widget` subclassing as supported
  extension points.
- [x] Add a composite-widget example under `examples/css_feature_probes`.
- [x] Add tests that component state and keys survive normal updates.
- [x] Document limits: composed widgets can only use existing renderer/event
  behavior.

### Phase 2: Add Internal Extension Widget Kind

- [x] Add native `WidgetKind::Extension`.
- [x] Parse `type: "extension"` and preserve `extension_type`.
- [x] Give extension widgets stable leaf layout and CSS class support.
- [x] Keep `WidgetKind::Unknown` for malformed or unsupported types.
- [x] Add smoke tests proving extension nodes do not disappear, overlap, or break
  layout.

### Phase 3: Display-List Paint API

- [x] Add Python `PaintWidget`, `PaintContext`, `MeasureConstraints`, and `Size`.
- [x] Serialize a small display-list format.
- [x] Render initial primitive display-list commands (`rect`, `rounded_rect`,
  `line`, `polyline`, `circle`) through the native primitive renderer.
- [x] Support CSS/theme token resolution in paint commands.
- [x] Add a `sparkline` or `mini_gauge` probe as the first real custom widget.
- [x] Add display-list text commands through the native text renderer.
- [x] Add display-list image commands through the native image/resource path.

### Phase 4: Events And Focus

- [x] Add click-event hit testing for extension widgets.
- [x] Route extension-widget clicks to Python callbacks through the existing
  `on_click` path.
- [x] Add pointer move and wheel event payloads for extension widgets.
- [x] Add optional keyboard focus and key-down events.
- [ ] Add key-up events if a real custom-widget use case needs them.
- [ ] Add drag support only after simple pointer events are stable.

### Phase 5: Performance And Resource Hardening

- [ ] Benchmark display-list serialization and native rendering.
- [ ] Add resource-backed display lists for high-frequency widgets.
- [ ] Cache resolved paint commands when style and props are unchanged.
- [ ] Add frame metrics for extension widget render cost.

## Probe Suite

Add probes as the API grows:

- [x] `custom_composite_widget_probe.py`
- [x] `paint_widget_sparkline_probe.py`
- [x] `paint_widget_events_probe.py`
- `paint_widget_css_probe.py`
- `paint_widget_perf_probe.py`

The probes should test:

- narrow and wide layouts
- panel clipping
- CSS theme switching
- pointer hover/click/drag behavior
- text rendering and high-DPI scaling
- multiple instances with independent state

## Acceptance

This V5 track is ready when:

- Third-party developers can publish a pure Python widget package.
- A custom widget can draw native-looking content without `HtmlReport`.
- Custom widgets participate in normal layout, panels, scrolling, and CSS.
- Custom widget events use the normal app event loop.
- The API has examples and tests stable enough for external use.
- Extension widgets do not require native Rust compilation by the third-party
  developer.

## Non-Goals For Initial V5

- Dynamic Rust plugin loading.
- Arbitrary GPU shader plugins.
- Full HTML/CSS embedding as the main custom-widget story.
- A complete SVG renderer.
- Replacing existing built-in widgets with custom widgets.

## Risks

- Display-list serialization could become expensive for high-frequency widgets.
- A too-large drawing API could become hard to stabilize.
- Event routing needs careful clipping and z-order handling around overlays.
- CSS token resolution has to be predictable or custom widgets will ignore
  theme changes like some early V4 widgets did.

The safest path is to ship a small, well-tested paint/event API first and grow
only when real widget probes expose missing primitives.
