# DataFrame Table Plan

## Objective

Add direct DataFrame display after the scatter demo and basic widget stack are
real.

The table should virtualize rows and columns in Rust. Python must not create
per-row widgets or render cells every frame.

## Deliverables

- `DataFrameTable` Rust widget.
- Column metadata: name, dtype, width, formatter, sort state.
- Row virtualization from scroll position.
- Column virtualization for wide tables.
- Selection model.
- Keyboard navigation.
- Sort indicators.
- Optional Arrow path for efficient typed column access.

## Data Interop Strategy

Use a staged path:

1. Metadata-only support for pandas and Polars.
2. Visible-row extraction through Python callback for early correctness.
3. NumPy buffer support for numeric columns.
4. Arrow array support for Polars and pandas Arrow-backed columns.
5. Typed formatting and null handling in Rust.

Do not block basic table rendering on perfect zero-copy interop.

## Runtime Update Protocol

Table updates must use commands:

- `table.set_frame(frame)`
- `table.set_sort(column, direction)`
- `table.scroll_to(row)`
- `table.set_selection(rows, columns)`

The full app document should not be resent for scrolling, sorting, or replacing
the DataFrame.

## First Slice

- Render headers and virtualized rows/columns from DataFrame metadata.
- Carry a bounded startup sample of formatted cell values through the document
  so early visible rows show real data without serializing the full frame.
- Fall back to dtype placeholders when the sampled window does not contain a
  requested cell.
- Add scroll-time visible-row extraction through the command protocol next.

## Acceptance Criteria

- 1M rows do not create 1M Python or Rust widget instances.
- Scrolling only asks for visible rows plus overscan.
- 60fps target on a reference machine with 1M rows and 20 columns.
- Memory use scales with visible rows plus column metadata, not total rows.
