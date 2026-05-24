# V4 BarChart

## Objective

Add categorical bar charts for operational dashboards and summaries.

## Proposed API

```python
dg.BarChart(frame, category="segment", value="revenue", orientation="vertical")
```

Grouped:

```python
dg.BarChart(frame, category="month", value=["sales", "cost"], grouped=True)
```

## Behavior

- Vertical and horizontal bars.
- Single and grouped series.
- Auto-fit value axis.
- Optional labels and hover.

## Native Work

- Add chart renderer or use primitive renderer for small counts.
- For many bars, use packed instance buffer.
- Reuse axis/grid styling from LinePlot where possible.

## Acceptance

- Small charts look good with labels.
- Hundreds of bars remain performant.
- Hover callback returns category, series, value, and index.

