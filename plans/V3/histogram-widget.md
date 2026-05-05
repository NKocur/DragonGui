# V3 Histogram Widget

DragonGUI needs a first-class histogram widget for distribution analysis,
quality checks, dashboards, and exploratory data tools. Users should be able to
drop a numeric column into a widget and immediately see counts, density, bins,
selection ranges, and useful hover details without building a custom plot.

The widget should follow the same performance direction as `LinePlot`,
`Scatter3D`, and `DataFrameTable`: Python builds a clear declarative widget,
while native owns compact data buffers, binning, rendering, interaction state,
and live updates.

## Goals

- Add a reusable `Histogram` widget for numeric distributions.
- Support automatic and explicit binning.
- Support counts, density, cumulative, and normalized modes.
- Support one or more overlaid/grouped series.
- Support weighted histograms.
- Support live data replacement and streaming append.
- Provide hover readouts for bin range, count, density, and percent.
- Provide optional range selection for filtering workflows.
- Render professionally by default in dashboards and control panels.
- Add focused probes and V3 demo coverage.

## Non-Goals

- Full statistical plotting grammar in the first version.
- Kernel density estimation in the first pass.
- Arbitrary annotations or fitted distribution curves initially.
- Replacing `LinePlot` or `DataFrameTable`.
- Browser-level SVG/chart customization.

## Proposed Python API

Simple histogram:

```python
hist = dg.Histogram(frame, value="latency_ms")
```

Explicit bin count:

```python
hist = dg.Histogram(frame, value="latency_ms", bins=40)
```

Explicit bin edges:

```python
hist = dg.Histogram(
    frame,
    value="temperature",
    bin_edges=[50, 55, 60, 65, 70, 75, 80, 85],
)
```

Density and cumulative modes:

```python
hist = dg.Histogram(
    frame,
    value="score",
    bins=30,
    mode="density",
    cumulative=True,
)
```

Grouped or overlaid series:

```python
hist = dg.Histogram(
    frame,
    value="latency_ms",
    group="region",
    groups=("us-east", "us-west", "eu"),
    mode="count",
    layout="overlay",
)
```

Weighted histogram:

```python
hist = dg.Histogram(frame, value="price", weight="quantity", bins="fd")
```

Range selection:

```python
hist = dg.Histogram(
    frame,
    value="latency_ms",
    selection=(20.0, 80.0),
    on_select=lambda bounds: print(bounds),
)
```

Live updates:

```python
hist.set_data(frame, value="latency_ms")
hist.set_bins(50)
hist.set_mode("density")
hist.set_selection((25.0, 90.0))
hist.clear_selection()
```

Streaming append:

```python
hist.append_values(values, max_points=100_000)
```

## Widget Properties

Initial public properties:

- `frame`: dataframe-like object or column source.
- `value`: numeric column name.
- `bins`: integer count or named strategy.
- `bin_edges`: explicit numeric edge sequence.
- `range`: optional `(min, max)` data range.
- `mode`: `"count"`, `"density"`, `"probability"`, or `"percent"`.
- `cumulative`: boolean cumulative histogram mode.
- `weight`: optional numeric column for weighted counts.
- `group`: optional categorical column for grouped series.
- `groups`: optional explicit group order/filter.
- `layout`: `"overlay"`, `"stack"`, or `"side_by_side"`.
- `orientation`: `"vertical"` or `"horizontal"`.
- `bar_gap`: logical px or fraction-like spacing.
- `colors`: optional per-series colors.
- `show_axes`, `show_grid`, `show_legend`, `show_toolbar`: display toggles.
- `selection`: optional selected `(min, max)` range.
- `hover`: enable hover readout.
- `on_select`: optional callback for selection changes.

Named bin strategies:

- `"auto"`
- `"sqrt"`
- `"sturges"`
- `"rice"`
- `"fd"` for Freedman-Diaconis
- `"scott"`

The first implementation can support `int`, `"auto"`, and explicit
`bin_edges`, then add more strategies after the rendering path is stable.

## Native Data Model

Add a histogram runtime resource keyed by widget id.

Per widget:

- histogram id
- source metadata
- bin edges
- per-series bin counts
- display mode
- value range
- viewport rect
- hover bin index
- selected range
- axis/grid/legend settings
- update telemetry

Per series:

- stable series id
- label
- color
- raw sample count
- finite sample count
- missing/NaN count
- packed bin counts
- optional weighted total

Packed data formats:

- `values_f32_v0`: contiguous float32 values.
- `values_f64_v0`: optional later for high precision.
- `bins_f32_v0`: precomputed bin edges and counts.
- `grouped_values_f32_v0`: values plus compact group ids.

Prefer native binning from packed values for simple use. Also support a
pre-binned payload later so Python/NumPy can bypass native binning when needed.

## Native Commands

Add commands modeled after `LinePlot` and `Scatter3D`:

- `SetHistogramDataPacked`
- `AppendHistogramValuesPacked`
- `SetHistogramBins`
- `SetHistogramMode`
- `SetHistogramSelection`
- `ClearHistogramSelection`
- `ClearHistogramData`

Expected dirty behavior:

- Data replacement/append/bin changes: `Dirty::GpuData` or histogram resource
  rebuild.
- Mode/color/bar style changes: `Dirty::Visual`.
- Axis/legend labels: `Dirty::Text`.
- Size/layout changes: `Dirty::Layout`.

High-rate append should coalesce by widget id. If the histogram is using fixed
bin edges, appends can update counts directly without retaining every raw value.
If bin edges are automatic, either retain a bounded value buffer or mark bins
dirty and recompute.

## Binning Rules

Phase 1 binning:

- Filter non-finite values.
- If `bin_edges` is provided, use it exactly after validation.
- If `bins` is an integer, compute equal-width bins across `range` or finite
  data bounds.
- If `range` is supplied, include only values inside that range.
- Define edge inclusivity clearly:
  - left-inclusive bins
  - final bin includes the right edge
- Track NaN/missing/out-of-range counts for hover/debug snapshots.

Later bin strategies:

- `sqrt`: `ceil(sqrt(n))`
- `sturges`: `ceil(log2(n) + 1)`
- `rice`: `ceil(2 * cbrt(n))`
- `fd`: width from interquartile range
- `scott`: width from standard deviation

## Rendering Plan

Render bars in native primitives or a dedicated histogram path.

Phase 1:

- Draw plot background.
- Draw grid and axes.
- Draw bars as rounded or square rect primitives.
- Clip bars to plot content rect.
- Draw hover highlight for the active bin.
- Draw selection overlay for selected range.
- Draw compact axis tick labels.

Multi-series:

- Overlay mode: bars share bin x positions with alpha blending.
- Side-by-side mode: divide each bin width by series count.
- Stack mode: cumulative bar heights per bin.

Default visual design:

- Slightly rounded bars for app-dashboard feel.
- Modest bar gap by default.
- Low-contrast grid lines.
- Hovered bin uses a brighter fill and outline.
- Selection range uses translucent accent overlay.

## Interaction Plan

Phase 1 interactions:

- Hover a bin to show:
  - bin range
  - count
  - percent of finite samples
  - density/probability if active
  - group label for grouped histograms
- Click-drag to create a selection range.
- Escape or toolbar clear action clears selection.
- Optional `on_select((low, high) | None)` callback.

Phase 2 interactions:

- Toolbar controls:
  - fit
  - pan/zoom range
  - clear selection
  - toggle density/count
  - toggle cumulative
- Wheel zoom with modifier or active zoom mode.
- Double-click fit.

Parent scroll behavior must stay predictable. Wheel zoom should not steal
normal page scrolling unless the user is in a zoom mode or holding a modifier.

## CSS And Parts

Expose widget type selector:

```css
Histogram {
    background: #101827;
    border-radius: 10px;
    accent: #5aa9ff;
}
```

Candidate parts:

- `plot`
- `bar`
- `bar-hover`
- `bar-selected`
- `axis`
- `grid`
- `tick`
- `label`
- `legend`
- `legend-item`
- `selection`
- `tooltip`
- `toolbar`
- `toolbar-button`

Inline part styles should support dashed and snake-case names, consistent with
existing widget parts.

## Python Implementation Work

Add `Histogram` to `python/dragongui/widgets.py`.

Constructor should validate:

- `value` is supplied when `frame` is supplied.
- `bins` is positive when integer.
- `bin_edges` is strictly increasing.
- `mode` is known.
- `layout` is known.
- `orientation` is known.
- colors length matches series/group count when supplied.

Methods:

```python
set_data(frame, value=None, weight=None, group=None)
set_bins(bins=None, bin_edges=None, range=None)
set_mode(mode, cumulative=None)
set_selection(selection)
clear_selection()
append_values(values, weights=None, groups=None, max_points=None)
```

Payload helpers:

```python
Histogram.prepare_values(frame, value, weight=None, group=None)
Histogram.prepare_bins(bin_edges, counts, labels=None)
```

## Native Implementation Work

Document parsing:

- Add `WidgetKind::Histogram`.
- Parse histogram props into `NodeProps`.
- Parse packed payload metadata.
- Include histogram fields in debug snapshots.

Runtime/resource layer:

- Add histogram resource registry entry.
- Implement packed value upload/decode.
- Implement binning and update paths.
- Add command queue support/coalescing.

Layout:

- Provide sensible intrinsic minimum size.
- Ensure it behaves like `LinePlot` in flex/grid panels.
- Verify titled panel and scroll-area composition with probes.

Rendering:

- Add histogram rect generation or renderer path.
- Add axis/grid/tick text collection.
- Add hover/selection overlays.

Events:

- Hit-test bins.
- Track hover bin.
- Implement drag selection.
- Emit selection callback events only when needed.

## Probe And Demo Coverage

Create `examples/css_feature_probes/histogram_probe.py` with:

- Basic count histogram.
- Explicit bin count.
- Explicit bin edges.
- Density and cumulative toggles.
- Overlaid grouped histogram.
- Weighted histogram.
- Range selection panel.
- Styling stress panel with CSS parts.
- Streaming append panel.

Add a V3 demo section:

- Sidebar nav item: `Histograms`.
- Three panels:
  - latency distribution
  - grouped quality scores
  - live streaming distribution
- Control panel:
  - bin count
  - mode toggle
  - cumulative toggle
  - group layout
  - selection clear
  - append stream

## Tests

Python tests:

- Constructor validation.
- `to_dict()` props.
- live setter command shape.
- packed payload helper shape.
- invalid bin edges reject.

Native tests:

- parse `Histogram` node props.
- integer bins produce expected counts.
- explicit bin edges produce expected counts.
- NaN/infinite values are ignored and counted.
- weighted counts accumulate correctly.
- density/probability normalization is stable.
- grouped histograms keep series order.
- histogram bars stay inside plot rect.
- selection range maps to correct data values.
- hover hit-test returns expected bin.
- layout in titled panel does not overlap controls.

## Phases

### Phase 1: Static Single-Series Histogram

- Add Python `Histogram` widget.
- Add native `WidgetKind::Histogram`.
- Support packed float32 values.
- Support integer bins and explicit bin edges.
- Render bars, axes, grid, and basic hover highlight.
- Add `histogram_probe.py`.

### Phase 2: Live Updates And Selection

- Add `set_data`, `set_bins`, and `append_values`.
- Add fixed-bin streaming append.
- Add selection range and `on_select`.
- Add debug snapshot telemetry.

### Phase 3: Grouped And Weighted Histograms

- Add `weight`.
- Add `group`, `groups`, and series colors.
- Add overlay, side-by-side, and stack layouts.
- Add legend support.

### Phase 4: Polish And CSS Parts

- Add CSS part selectors.
- Add toolbar controls.
- Improve default styling.
- Add V3 demo section.

### Phase 5: Performance Pass

- Benchmark large arrays.
- Avoid unnecessary re-binning on visual-only changes.
- Coalesce append commands.
- Add high-rate streaming probe.

## Open Questions

- Should automatic binning happen in Python, native, or both?
- Should `Histogram` retain raw values after binning when using automatic bins?
- Should grouped histograms accept categorical strings directly, or should
  Python pre-map groups to compact ids?
- Should selection callback return only `(low, high)` or include selected row
  masks/counts?
- Should density mode normalize per group or across all groups by default?

## Success Criteria

- A user can write `dg.Histogram(frame, value="latency")` and get a useful plot.
- Static histograms handle at least hundreds of thousands of values without
  slow widget-tree rebuilds.
- Fixed-bin streaming append updates smoothly.
- Hover and range selection work predictably.
- The widget composes inside panels, grids, and scroll areas without layout
  workarounds.
- The V3 demo has a useful histogram section comparable to the `LinePlot` and
  `Scatter3D` sections.
