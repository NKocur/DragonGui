# V4 Breadcrumbs

## Objective

Add a compact navigation/path widget for nested workflows, file paths, and
object hierarchy views.

## Proposed API

```python
dg.Breadcrumbs(
    ["Project", "Runs", "Run 42"],
    on_select=lambda index, label: ...
)
```

Path item form:

```python
dg.Breadcrumbs([
    {"label": "Project", "value": "project"},
    {"label": "Runs", "value": "runs"},
])
```

## Behavior

- Items are clickable except optionally the final item.
- Separators render between items.
- Long paths can collapse middle items into an overflow menu.

## Native Work

- Can be built as a Python compound widget first.
- Native support only needed for compact overflow measurement.

## Acceptance

- Click callbacks identify selected segment.
- Long labels do not overflow parent.
- Styling supports item, separator, hover, and current segment.

