# V4 RadioButton And RadioGroup

## Objective

Add a compact mutually exclusive choice control for settings panels and forms.
This is a simpler, always-visible alternative to `Dropdown`.

## Proposed API

```python
mode = dg.RadioGroup(
    ["Fast", "Balanced", "Quality"],
    value="Balanced",
    on_change=lambda value: ...
)
```

Individual radio button:

```python
dg.RadioButton("Quality", value="quality", checked=True)
```

## Behavior

- Exactly one option is selected in normal `RadioGroup` mode.
- Disabled groups prevent changes.
- Horizontal and vertical orientations.
- Keyboard arrows move selection within the group.
- Enter/Space selects the focused option.

## Native Work

- Add `WidgetKind::RadioButton` or model it as a styled selectable with
  exclusive group state.
- Add circular indicator primitive rendering.
- Add keyboard handling for grouped options.
- Preserve selected state across rebuilds by group id and option value.

## Python Work

- Add `RadioButton`.
- Add `RadioGroup` convenience widget that creates child radio buttons.
- Export both widgets.
- Add `set_value(value, notify=False)`.

## Styling

CSS parts:

| Part | Meaning |
| --- | --- |
| `indicator` | Outer radio circle. |
| `dot` | Selected inner dot. |
| `label` | Option text. |

## Acceptance

- Pointer selection updates the group value.
- Keyboard selection works.
- `on_change` fires with the new value.
- Disabled options render and behave correctly.
- `set_value(...)` updates live native state.

