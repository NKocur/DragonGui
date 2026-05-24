# V4 ScatterPlot2D

## Objective

Add a first-class 2D scatter plot for dashboards and exploratory data tools.
`Scatter3D` is too heavy and semantically different for this use case.

## Proposed API

```python
dg.ScatterPlot2D(frame, x="latency_ms", y="score", color="group", on_pick=...)
```

## Behavior

- Static packed point upload.
- Optional categorical or scalar color.
- Auto-fit bounds.
- Hover/pick callback.
- Pan/zoom can follow LinePlot interaction patterns.

## Native Work

- Add 2D point renderer or extend LinePlot renderer with point series.
- Use packed buffers and shader transforms.
- Add decimation or point-size adaptation for dense plots.

## Acceptance

- 100k points render smoothly.
- Packed startup and live update path avoids JSON point lists.
- Hover/pick returns original row index.

