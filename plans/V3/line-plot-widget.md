# V3 Line Plot Widget

DragonGUI needs a first-class 2D line plot for dashboards, plots, and live
sensor monitoring. The widget should support simple static plots, high-rate
streaming traces, and compact operational panels without forcing users to build
custom canvas-style renderers.

The main performance requirement is that live data updates must avoid Python
widget-tree rebuilds, large JSON payloads, and unnecessary text/layout work.
Like `Scatter3D`, data-only updates should travel as packed buffers and land in
native as `GpuData` work.

## Goals

- Add a reusable `LinePlot` widget for static and live 2D series.
- Support single-series and multi-series plots.
- Provide efficient packed payload APIs for large data and background workers.
- Support append/ring-buffer streaming for sensor monitoring.
- Keep live data updates coalesced and cheap in the native command queue.
- Add axis/grid/legend defaults that look professional in app dashboards.
- Add probes and V3 demo coverage for static plots and live sensor monitoring.

## Non-Goals

- Full Matplotlib parity.
- A complete scientific plotting grammar in the first version.
- Arbitrary annotations, subplots, transforms, or secondary axes initially.
- CPU-side anti-aliased chart rasterization.
- Replacing `Scatter3D` or table diagnostics.

## Proposed Python API

Static single-series plot:

```python
plot = dg.LinePlot(frame, x="time", y="temperature", label="Temp")
```

Static multi-series plot:

```python
plot = dg.LinePlot(
    frame,
    x="time",
    y=["temperature", "pressure", "voltage"],
    labels=["Temp", "Pressure", "Voltage"],
)
```

Live replacement:

```python
plot.set_data(frame, x="time", y="temperature")
```

Streaming append:

```python
plot.append_points(
    x_values,
    y_values,
    series="temperature",
    max_points=10_000,
)
```

Prepared payload path for background workers:

```python
payload = dg.LinePlot.prepare_series(frame, x="time", y=["temp", "pressure"])
plot.set_prepared_series(payload)
```

Rolling sensor mode:

```python
plot = dg.LinePlot(
    max_points=20_000,
    mode="ring",
    x_window=60.0,
    y_limits=(0.0, 100.0),
)
plot.append_points(t_batch, temp_batch, series="temp")
```

## Widget Properties

Initial public properties:

- `x`: source column name for x values.
- `y`: source column name or sequence of column names.
- `labels`: optional display labels for series.
- `colors`: optional per-series colors.
- `line_width`: line width in logical pixels.
- `x_limits`, `y_limits`: fixed plot limits.
- `auto_fit`: automatically compute visible bounds.
- `x_window`: rolling x-axis window for sensor streams.
- `mode`: `"replace"`, `"append"`, or `"ring"`.
- `max_points`: retention cap per series.
- `show_grid`, `show_axes`, `show_legend`: display toggles.
- `hover`: enable crosshair/readout.

The first implementation can omit some options from native behavior as long as
the API shape leaves room for them.

## Native Data Model

Add a line plot runtime resource keyed by widget id.

Per widget:

- plot id
- series map
- visible series order
- x/y bounds
- viewport rect
- axis/grid settings
- command/update telemetry

Per series:

- stable series id
- label
- color
- line width
- packed points buffer
- point count
- append/ring metadata

Packed data formats:

- `xy_f32_v0`: interleaved `x, y` float32 pairs.
- `y_f32_v0`: y-only float32 values with `x_start` and `x_step`.
- optional later: `xy_f64_v0` only if precision needs justify the cost.

Use immutable bytes on the Python side. Avoid lists of point dictionaries.

## Native Commands

Add commands modeled after scatter packed updates:

- `SetLinePlotDataPacked`
- `AppendLinePlotPointsPacked`
- `ClearLinePlotSeries`
- `SetLinePlotViewport`
- `SetLinePlotStyle`

Expected dirty behavior:

- Data replacement and append: `Dirty::GpuData`.
- Style that affects line color or width: `Dirty::Visual`.
- Axis/legend label text: `Dirty::Text`.
- Layout-affecting widget changes: `Dirty::Layout`.

High-rate append commands should support coalescing by widget id and series id.
For sensor monitoring, latest append batches can be merged before native upload
when queue pressure is high.

## Rendering Plan

Phase 1 should render simple polylines:

- Create one vertex buffer per visible series or one packed combined buffer.
- Convert data-space points to plot-space in shader using uniform bounds.
- Draw line strips or segmented line lists depending on WGPU support and
  portability needs.
- Clip to the plot content rect.
- Use per-series color and width.

If native wide lines are not portable enough, render each segment as a small
quad in the vertex shader or CPU-expanded vertex buffer. Prefer correctness and
predictability over relying on platform line-width behavior.

Axes and grid:

- Render grid lines as primitives in the plot renderer or the existing primitive
  renderer.
- Add compact tick labels through the text renderer.
- Keep labels optional so high-rate telemetry panels can disable them.

Decimation:

- Add min/max bucket decimation for large visible data.
- Decimate per pixel column to preserve spikes.
- Recompute decimation only when data, viewport, or plot bounds change.
- Keep raw data for hover/readout.

## Interaction Plan

Phase 1:

- Auto-fit data bounds.
- Fixed x/y limits.
- Rolling x window.
- Hover crosshair with nearest visible point readout.

Phase 2:

- Mouse wheel zoom.
- Drag pan.
- Double-click fit.
- Linked x-axis across multiple `LinePlot` widgets.
- Optional selection/range callback.

## Styling

CSS hooks:

```css
LinePlot {
    background: surface;
    border-color: border;
    border-radius: 8px;
    line-width: 2px;
    plot-grid-color: rgba(255, 255, 255, 0.10);
    plot-axis-color: muted_text;
    plot-zero-line-color: rgba(255, 255, 255, 0.22);
}

LinePlot::legend {
    color: muted_text;
    font-size: 11px;
}

LinePlot::axis-label {
    color: muted_text;
    font-size: 10px;
}
```

Default look should be compact and operational. This is a dashboard tool, not a
marketing chart.

## Python Implementation Phases

### Phase 1: Static Packed Widget

Deliverables:

- Add `LinePlot` class in `python/dragongui/widgets.py`.
- Add payload packing helpers for frame-like objects and NumPy arrays.
- Add `prepare_series(...)`.
- Emit initial document props with packed data metadata.
- Add widget export in `python/dragongui/__init__.py`.
- Add tests for API validation and payload packing.

### Phase 2: Native Parse And Static Render

Deliverables:

- Add `WidgetKind::LinePlot`.
- Parse line plot props in native document code.
- Add line plot runtime resource.
- Render static one-series and multi-series data.
- Add debug snapshot metrics for point count, series count, upload time, and
  decimation time.

### Phase 3: Live Replacement

Deliverables:

- Add `LinePlot.set_data(...)`.
- Add `LinePlot.set_prepared_series(...)`.
- Add `SetLinePlotDataPacked` command.
- Ensure data-only replacement is `GpuData`.
- Add VDOM tests to avoid resending unchanged packed buffers.

### Phase 4: Streaming Append And Ring Buffer

Deliverables:

- Add `append_points(...)`.
- Add `AppendLinePlotPointsPacked` command.
- Add native ring-buffer retention.
- Add command coalescing for high-rate sensor streams.
- Add stress probe for sustained append throughput.

### Phase 5: Axis, Grid, Hover, And Polish

Deliverables:

- Add axis ticks and compact labels.
- Add grid and zero line.
- Add legend.
- Add hover/crosshair readout.
- Add CSS part styling and theme integration.

## Examples And Probes

Add `examples/css_feature_probes/line_plot_probe.py` with:

- static sine/cosine multi-series plot
- dense historical plot with decimation
- live sensor monitor with rolling window
- append stress controls
- theme/style cases

Add V3 demo coverage:

- New `Plots` or `Sensors` section in `examples/all_features_v3_demo.py`.
- Include one static engineering plot and one live rolling sensor monitor.
- Include controls for start/stop stream, line width, y limits, and grid toggle.
- Include a small performance summary from debug snapshot metrics.

## Tests

Python tests:

- valid single-series and multi-series construction
- invalid column names and mismatched labels/colors
- packed payload format and byte size
- `prepare_series(...)` is immutable and worker-safe
- live replacement enqueues packed native command
- append mode enqueues append command
- unchanged VDOM payloads do not emit large patches

Native tests:

- document parsing for `LinePlot`
- command parsing for set/append packed data
- dirty classification for data/style/layout changes
- decimation preserves min/max spikes in buckets
- debug snapshot includes expected metrics

Smoke tests:

- static probe runs for a few frames
- live append probe runs for a few frames
- V3 demo smoke still starts without live stream enabled

## Open Questions

- Should the initial line implementation use line strips, CPU-expanded quads, or
  shader-expanded segments?
- Should `LinePlot` own its own axis labels or reuse generic text primitives?
- Should append commands merge batches in Python, native, or both?
- What should the default retention be for sensor streams?
- Do we need time/datetime axis helpers in V3, or should users pass numeric
  seconds initially?

## Acceptance Criteria

- A user can create a static line plot with one line of Python.
- A user can stream sensor data without freezing the UI.
- Data-only updates do not rebuild the full widget tree.
- A 10k-point rolling sensor plot can update smoothly in the V3 demo.
- A dense 1M-point static series remains usable through decimation.
- The widget has a compact, professional default appearance.
