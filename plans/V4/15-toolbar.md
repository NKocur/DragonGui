# V4 Toolbar

## Objective

Add a compact command surface for editor and dashboard actions. A toolbar can be
composed manually today, but a first-class widget gives stable spacing,
alignment, grouping, and overflow behavior.

## Proposed API

```python
with dg.Toolbar():
    dg.IconButton("play", tooltip="Run", on_click=run)
    dg.ToolbarSeparator()
    dg.IconButton("save", tooltip="Save", on_click=save)
```

## Behavior

- Horizontal by default, vertical optional.
- Groups and separators align consistently.
- Optional overflow menu can be phase 2.
- Works well inside panels and status bars.

## Native Work

- Toolbar itself can be a styled container.
- Add `ToolbarSeparator` or reuse `Separator` with toolbar styling.
- Overflow requires measurement and popup menu later.

## Acceptance

- Toolbar examples look dense and professional.
- Icon buttons have stable dimensions.
- Separators do not create layout jumps.
- CSS can style toolbar background, border, and separators.

