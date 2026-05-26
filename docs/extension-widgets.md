# DragonGUI Extension Widgets

DragonGUI supports two practical extension paths.

## Composite Widgets

Use `@dg.component` when a widget can be built from existing DragonGUI widgets.
This is the preferred extension point for most application-specific controls.

```python
@dg.component
def StatusTile(ctx: dg.ComponentCtx, title: str, value: str):
    count = ctx.state("count", 0)
    with dg.Panel(title, key="tile", parent=None) as panel:
        dg.Label(value, key="value")
        dg.Button("Update", key="update", on_click=lambda: count.set(int(count.value) + 1))
    return panel
```

Composite widgets participate in normal layout, CSS classes, keys, state, and
live patches because they serialize to ordinary built-in widgets.

## ExtensionWidget

`ExtensionWidget` is the V5 native foundation for future custom painted widgets.
It serializes as `type: "extension"` and preserves an `extension_type` string
plus JSON-compatible props.

```python
dg.ExtensionWidget(
    "sparkline",
    {"series": [2, 5, 3, 8]},
    intrinsic_width=160,
    intrinsic_height=44,
    on_click=lambda: print("sparkline clicked"),
    class_="sparkline",
)
```

`on_click` uses the normal DragonGUI callback path. `disabled=True` suppresses
the event and native hit testing. Lower-level event hooks receive
`PaintPointerEvent` objects:

```python
dg.ExtensionWidget(
    "canvas",
    intrinsic_width=320,
    intrinsic_height=180,
    on_pointer_down=lambda event: print(event.local_x, event.local_y),
    on_pointer_move=lambda event: print(event.dx, event.dy),
    on_wheel=lambda event: print(event.dy),
    on_key_down=lambda event: print(event.key),
)
```

## PaintWidget

Use `PaintWidget` when a third-party widget needs native drawing without adding
new Rust code. Subclasses implement `measure(...)` and `paint(ctx)`.

```python
class Sparkline(dg.PaintWidget):
    def __init__(self, values, **kwargs):
        self.values = list(values)
        super().__init__(extension_type="sparkline", **kwargs)

    def measure(self, constraints: dg.MeasureConstraints) -> dg.Size:
        return constraints.clamp(dg.Size(160, 44))

    def paint(self, ctx: dg.PaintContext) -> None:
        ctx.rounded_rect(0, 0, ctx.width, ctx.height, radius=6, fill="surface")
        ctx.polyline([[0, 30], [40, 12], [80, 28], [160, 8]], stroke="accent", width=2)
        ctx.image("assets/status.png", 120, 8, 32, 32, fit="contain", radius=4)
        ctx.text(8, 6, "loss", fill="text", font_size=11, font_weight=700)
```

The initial display-list commands are `rect`, `rounded_rect`, `line`,
`polyline`, `circle`, `text`, and path-backed `image`. Colors may be theme
tokens such as `accent` or CSS-style colors such as `#7dd3fc`.

Current limits:

- It is a leaf widget.
- It supports layout, inline style, CSS classes, and the `ExtensionWidget`
  type selector.
- `PaintWidget` image commands currently load from filesystem paths, matching
  the built-in `Image` widget path model.
- It supports `on_click`, `on_pointer_down`, `on_pointer_move`,
  `on_pointer_up`, `on_wheel`, and focused `on_key_down`. It does not yet
  provide pointer capture, higher-level drag semantics, or key-up events.
