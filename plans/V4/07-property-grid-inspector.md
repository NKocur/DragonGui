# V4 PropertyGrid Inspector

## Objective

Add a structured inspector widget for editing object properties without hand
building repetitive label/control rows.

## Proposed API

```python
dg.PropertyGrid(
    {
        "Name": "Sensor A",
        "Enabled": True,
        "Gain": 0.25,
        "Color": "#66ccff",
    },
    schema={
        "Gain": {"type": "float", "min": 0.0, "max": 1.0, "step": 0.01},
        "Color": {"type": "color"},
    },
    on_change=lambda key, value: ...
)
```

Context form:

```python
with dg.PropertyGrid():
    dg.Property("Gain", dg.DragNumber(0.25, min=0.0, max=1.0))
```

## Behavior

- Label column and editor column align across rows.
- Supports sections/groups.
- Supports reset/revert controls per row in phase 2.
- Chooses default editors by value/schema type.

## Native Work

- May be built mostly as Python compound widgets at first.
- Native work is needed only if row virtualization or custom table-like layout
  becomes necessary.
- Reuse `GridLayout`, `Label`, `Checkbox`, `TextInput`, `NumberInput`,
  `ColorPicker`, `Dropdown`, and future `DragNumber`.

## Python Work

- Add `PropertyGrid` container.
- Add `Property` row helper.
- Add schema-to-widget construction.
- Add callback payload:

```python
PropertyChange(key: str, value: object, old_value: object | None)
```

## Acceptance

- Basic dict/schema can produce a working inspector.
- Manual row form supports custom child widgets.
- Label column is stable and aligned.
- Live property updates can notify or suppress callbacks.

