# Plots

DragonGUI plot widgets are designed for live data applications. Dense point
clouds use packed data paths to avoid Python callback overhead where possible.

## Scatter3D

Use `Scatter3D` for 3D point clouds:

```python
scatter = dg.Scatter3D(frame, x="x", y="y", z="z", colormap="turbo")
scatter.show_scalar_bar(True, title="z")
```

For streaming, prebuild payloads and submit prepared frames:

```python
payload = scatter.prepare_points(frame, x="x", y="y", z="z")
scatter.enqueue_prepared_points(payload)
```

## LinePlot

Use `LinePlot` for time-series and append-heavy data:

```python
plot = dg.LinePlot(frame, x="step", y="loss", max_points=2000)
plot.append_points([step], [loss], series="loss", max_points=2000)
```

## Recent Scatter Notes

- Opaque Scatter3D points now use a depth-writing path.
- Compact streaming frames preserve live colormap changes.
- Scalar-bar state is synchronized with native retained chrome.

See `../scatter3d-point-depth-fix.md` and
`../scatter3d-streaming-colormap-fix.md` for implementation notes.
