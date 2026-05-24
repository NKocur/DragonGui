# V4 DragNumber, DragVector, And RangeSlider

## Objective

Add compact realtime numeric controls modeled after ImGui drag widgets. These
are better than sliders when the domain is broad or when many numeric fields
need to fit in an inspector.

## Proposed API

```python
dg.DragNumber(0.5, min=0.0, max=1.0, speed=0.01, on_change=...)
dg.DragInt(4, min=0, max=32, speed=1)
dg.DragVector3((0.0, 1.0, 2.0), labels=("X", "Y", "Z"), speed=0.1)
dg.RangeSlider((10.0, 90.0), min=0.0, max=100.0, on_change=...)
```

## Behavior

- Click-drag horizontally changes value.
- Text entry on double click or Enter can be phase 2.
- Shift/Ctrl modifiers adjust speed if modifier state is available.
- Values clamp to min/max.
- Vector variants update individual components.
- Range slider enforces `low <= high`.

## Native Work

- Extend input handling with pointer drag delta capture.
- Add active-drag state with start value and accumulated delta.
- Render compact number field, optional component labels, and active state.
- Emit change events while dragging with throttling/coalescing if needed.

## Python Work

- Add `DragNumber`, `DragInt`, `DragVector2`, `DragVector3`, `DragVector4`.
- Add `RangeSlider`.
- Export live setters:

```python
control.set_value(value, notify=False)
control.set_range(low, high, notify=False)
```

## Styling

CSS parts:

| Part | Meaning |
| --- | --- |
| `field` | Numeric drag surface. |
| `component` | Vector component field. |
| `label` | X/Y/Z/W or custom label. |
| `track` | Range slider track. |
| `handle` | Range slider handle. |

## Acceptance

- Dragging changes values smoothly.
- Keyboard focus works and does not conflict with sliders.
- Vector fields keep stable layout.
- Callback values are typed consistently as int/float/tuple.
- Smoke demo covers drag number, vector, and range controls.

