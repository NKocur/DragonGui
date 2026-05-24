# V4 Selectable And SelectableList

## Objective

Add a dense selection primitive for lists, browsers, inspectors, logs, and
setting panels. This fills the gap between `Dropdown`, `NavItem`, and
`DataFrameTable`.

## Proposed API

```python
item = dg.Selectable("Layer 01", selected=True, on_select=lambda selected: ...)

selection = dg.SelectableList(
    ["CPU", "GPU", "Memory"],
    value="GPU",
    on_change=lambda value: ...
)
```

Multi-select:

```python
selection = dg.SelectableList(
    files,
    selected={"a.csv", "b.csv"},
    selection_mode="multiple",
    on_change=lambda values: ...
)
```

## Behavior

- Single item supports selected, disabled, and callback state.
- List supports single-select and multi-select.
- Shift-click range selection can be phase 2.
- Ctrl-click toggle can be phase 2 if keyboard modifiers are already exposed.
- Keyboard Up/Down moves focus.
- Enter/Space toggles or selects the focused item.
- Optional `max_height` enables scrolling for long lists.

## Native Work

- Add `WidgetKind::Selectable` and optionally `SelectableList`.
- Store selected/focused state in `WidgetState`.
- Add hit testing and keyboard activation.
- Render selected rows with a stable highlight primitive.
- Add text rendering for labels.
- Add debug snapshot entries for selected values.

## Python Work

- Add `Selectable` leaf widget.
- Add `SelectableList` convenience container/compound widget.
- Export from `dragongui`.
- Add `set_selected(...)`, `select(...)`, and `clear_selection(...)`.

## Styling

```css
Selectable {
    padding: 6px 8px;
    border-radius: 4px;
}

Selectable:selected {
    background: accent;
    color: white;
}
```

## Acceptance

- Single selection works with pointer and keyboard.
- Multi-select can return a stable ordered list or set.
- Long lists scroll without layout instability.
- Selection survives rebuilds when keys are stable.
- Smoke example covers single and multi-select modes.

