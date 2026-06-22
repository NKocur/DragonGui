# Live Updates

Live updates are changes sent to the native runtime after widgets have been
bound by `app.run(...)`.

## Use Live Methods For Small Changes

Prefer:

```python
label.set_value("Done")
progress.set_value(0.75)
scatter.set_colormap("plasma")
```

over rebuilding the entire document tree.

## Threading

Use thread-safe scheduling for general UI callbacks:

```python
app.call_soon_threadsafe(lambda: progress.set_value(value))
```

For high-frequency plot data, use packed enqueue APIs where available, such as
`Scatter3D.enqueue_prepared_points(...)`.

## Retained State

Live commands that affect persisted widget chrome should keep native retained
props in sync. Otherwise later style or chrome sync passes can reapply stale
startup props.
