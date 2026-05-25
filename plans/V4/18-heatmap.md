# V4 Heatmap

Status: implemented in first slice.

## Objective

Add matrix/grid visualization for correlation matrices, sensor arrays, images,
confusion matrices, and dense dashboards.

## Proposed API

```python
dg.Heatmap(matrix, x_labels=cols, y_labels=rows, colormap="viridis")
```

## Behavior

- Accepts 2D numeric arrays.
- Auto-compute value range.
- Optional labels and scalar bar.
- Hover returns row/column/value.

## Native Work

- Upload matrix as packed float32 or normalized texture.
- Render as textured quad or per-cell rectangles depending size.
- Use GPU colormap sampling if practical.
- Add label rendering for small matrices only.

## Acceptance

- Large matrices render without primitive explosion.
- Small labeled matrices are readable.
- Hover callback identifies cell.
