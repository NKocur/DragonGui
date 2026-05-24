# V4 Date And Time Inputs

## Objective

Add common temporal inputs for data filtering, scheduling, and reporting tools.

## Proposed API

```python
dg.DateInput(value="2026-05-22", on_change=lambda date: ...)
dg.TimeInput(value="14:30", on_change=lambda time: ...)
dg.DateTimeInput(value="2026-05-22T14:30:00", on_change=lambda value: ...)
```

Optional picker popup:

```python
dg.DateInput(value=date.today(), picker=True)
```

## Behavior

- First version can be validated text input with formatting.
- Calendar/time popup can be phase 2.
- Values should use ISO strings or Python date/time objects consistently.
- Invalid text keeps focus and uses danger styling.

## Native Work

- Text validation and focus behavior can reuse TextInput.
- Popup calendar requires overlay layout and keyboard navigation.

## Python Work

- Add wrappers around TextInput with validation.
- Add parser/formatter helpers.
- Add `notify=False` live setters.

## Acceptance

- Valid ISO date/time values emit callbacks.
- Invalid values do not emit committed changes.
- Docs state exact value type contract.
- Smoke demo covers date, time, and datetime inputs.

