# V4 Splitter And ResizablePane

## Objective

Add user-resizable layouts for desktop tools: sidebars, inspectors, consoles,
data tables, plot panes, and editor-like shells.

## Proposed API

```python
with dg.Splitter(orientation="horizontal", sizes=[280, "1fr"], min_sizes=[180, 320]):
    with dg.Pane():
        dg.Label("Controls")
    with dg.Pane():
        dg.LinePlot(...)
```

Simpler two-pane helper:

```python
with dg.ResizablePane(side="left", initial_size=280, min_size=180):
    ...
```

## Behavior

- Drag gutter resizes adjacent panes.
- Min/max pane sizes are respected.
- Sizes persist across rebuilds using widget id/key.
- Double click reset can be phase 2.
- Nested splitters should work.

## Native Work

- Add `WidgetKind::Splitter` and `Pane`.
- Extend layout to allocate tracks from fixed/fr/current drag sizes.
- Add gutter hit testing and cursor state.
- Store live pane sizes in `WidgetState`.
- Recompute layout during active drag.

## Python Work

- Add `Splitter` context manager.
- Add `Pane` context manager or internal child convention.
- Add `set_sizes(...)`.
- Export from `dragongui`.

## Styling

CSS parts:

| Part | Meaning |
| --- | --- |
| `gutter` | Draggable separator. |
| `pane` | Pane region. |

## Acceptance

- Horizontal and vertical splitters work.
- Dragging gutter updates layout live.
- Pane sizes persist through rerenders.
- Splitter does not break scroll areas inside panes.
- Smoke demo includes nested splitter and plot/table panes.

