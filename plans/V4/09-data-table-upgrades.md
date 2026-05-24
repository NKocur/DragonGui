# V4 DataFrameTable Upgrades

## Objective

Bring `DataFrameTable` closer to the table features users expect in data apps
and Dear ImGui-style tools.

## Target Features

1. Sortable headers.
2. Resizable columns.
3. Reorderable columns.
4. Hide/show columns.
5. Frozen header.
6. Frozen leading columns.
7. Editable cells.
8. Header context menu.

## Proposed API

```python
table = dg.DataFrameTable(
    frame,
    sortable=True,
    resizable_columns=True,
    reorderable_columns=True,
    editable=True,
    frozen_columns=1,
    on_sort=lambda sort: ...,
    on_edit=lambda edit: ...,
)
```

Payloads:

```python
TableSort(column: str, descending: bool)
TableEdit(row_index: int, column: str, old_value: object, new_value: object)
```

## Native Work

- Extend table layout with column width state.
- Add header hit zones for sort and resize.
- Store column order/visibility in widget state.
- Keep row virtualization intact.
- Add cell editor overlay for editable cells.

## Python Work

- Add constructor options.
- Add live setters for sort/order/visibility/width.
- Add callback payload dataclasses.
- Export any new payload types.

## Acceptance

- Sorting header callback fires without corrupting selected cell state.
- Column resize is smooth and persisted.
- Hidden columns do not participate in layout or hit testing.
- Frozen headers/columns remain visible during scroll.
- Editable cells commit and cancel correctly.

