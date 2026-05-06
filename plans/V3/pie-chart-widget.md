# V3 Pie Chart Widget

DragonGUI needs a first-class pie and donut chart widget for composition,
share-of-total, categorical breakdown, and dashboard summary views. Users should
be able to pass labels and values, or point at a categorical column in a frame,
and get a readable chart with legend, labels, hover details, and selection
without manually building arc geometry.

The widget should follow the same V3 direction as `LinePlot`, `Histogram`,
`Scatter3D`, and `DataFrameTable`: Python exposes a small declarative API,
while native owns compact chart data, layout, rendering, interaction state, and
live updates.

## Goals

- Add a reusable `PieChart` widget for categorical shares.
- Support simple labels and values as direct Python sequences.
- Support frame-backed aggregation from a category column and optional value
  column.
- Support pie and donut modes.
- Support absolute value, percent, and label display options.
- Support legend placement and slice labels.
- Support hover readouts for label, value, percent, and optional metadata.
- Support click selection for slices.
- Render professionally by default in dashboards and control panels.
- Add focused probe coverage and V3 demo coverage.

## Current Status

Phase 1 has been restored after the prior implementation was removed:

- Python `PieChart` widget exists.
- Direct labels/values and frame-backed aggregation are supported.
- Native `WidgetKind::PieChart` parsing exists.
- Flat pie/donut slice rendering exists through the primitive renderer.
- Native legend swatches and overlay text labels exist.
- `examples/css_feature_probes/pie_chart_probe.py` exists.

Still missing: hover/selection callbacks, live setters beyond coarse node
replacement, CSS parts, toolbar controls, and richer label collision handling.

## Non-Goals

- Full chart grammar or arbitrary polar plotting.
- Nested sunburst charts in the first pass.
- Exploded multi-ring dashboards in the first pass.
- 3D pie charts.
- Browser/SVG-level chart customization.

## Proposed Python API

Simple values:

```python
chart = dg.PieChart(
    labels=["Search", "Direct", "Referral", "Ads"],
    values=[42, 28, 18, 12],
)
```

Donut chart:

```python
chart = dg.PieChart(
    labels=("Compute", "Storage", "Network"),
    values=(63, 24, 13),
    donut=True,
    inner_radius=0.58,
)
```

Frame-backed count aggregation:

```python
chart = dg.PieChart(frame, category="region")
```

Frame-backed weighted aggregation:

```python
chart = dg.PieChart(frame, category="region", value="revenue", aggregate="sum")
```

Limit categories and group the rest:

```python
chart = dg.PieChart(
    frame,
    category="source",
    value="sessions",
    top_n=6,
    other_label="Other",
)
```

Selection:

```python
chart = dg.PieChart(
    labels=("Open", "Closed", "Blocked"),
    values=(18, 64, 7),
    on_select=lambda selection: print(selection.label, selection.percent),
)
```

Live updates:

```python
chart.set_data(labels=["A", "B", "C"], values=[10, 20, 30])
chart.set_frame(frame, category="status", value="count")
chart.set_selected("Blocked")
chart.clear_selection()
```

## Public API Shape

Constructor:

```python
dg.PieChart(
    data=None,
    *,
    labels=None,
    values=None,
    category=None,
    value=None,
    aggregate="count",
    top_n=None,
    other_label="Other",
    donut=False,
    inner_radius=0.52,
    start_angle=-90,
    clockwise=True,
    min_slice_percent=0.0,
    label_mode="auto",
    value_mode="percent",
    show_legend=True,
    legend_position="right",
    show_labels=True,
    show_toolbar=False,
    selected=None,
    on_select=None,
    colors=None,
    title=None,
    id=None,
    key=None,
    class_=None,
    style=None,
    tooltip=None,
    parent=...,
)
```

Core arguments:

- `labels`, `values`: direct series data.
- `data`, `category`, `value`: frame-backed aggregation path.
- `aggregate`: `count`, `sum`, `mean`, `min`, `max`.
- `top_n`: keep the largest N categories and group the rest.
- `other_label`: label for grouped remainder.
- `donut`: render as donut instead of full pie.
- `inner_radius`: donut hole ratio, clamped to a useful range.
- `start_angle`: first slice angle in degrees.
- `clockwise`: direction of slice sweep.
- `min_slice_percent`: hide direct labels below this threshold.
- `label_mode`: `auto`, `inside`, `outside`, `legend`, or `none`.
- `value_mode`: `percent`, `value`, `both`, or `none`.
- `legend_position`: `right`, `left`, `bottom`, `top`, or `none`.
- `selected`: selected slice label or index.
- `colors`: optional list or mapping of label to color.

## Data Model

Python should normalize initial data into a compact payload:

```python
{
    "labels": ["Search", "Direct", "Referral"],
    "values": [42.0, 28.0, 18.0],
    "total": 88.0,
    "colors": ["#5aa9ff", "#74ddb0", "#ffcc66"],
    "input_count": 4,
    "finite_count": 4,
}
```

Native should parse and validate:

- non-empty labels and values
- finite non-negative values
- total greater than zero
- label/value length match
- color list length may be shorter than values and falls back to palette

Invalid or empty charts should render an empty-state surface with muted text
instead of crashing or producing invalid geometry.

## Native Rendering Plan

Add `WidgetKind::PieChart`.

Renderer responsibilities:

- Compute chart rect and legend rect from widget bounds.
- Convert values to normalized slice angles.
- Render slices as anti-aliased wedge primitives.
- Render donut hole when enabled.
- Render selected or hovered slice with subtle scale/outline/emphasis.
- Render center label for donut mode when enabled.
- Render legend swatches and labels through overlay text.
- Clamp outside labels and leader lines inside the widget rect.

Implementation options:

- Start with a dedicated arc/wedge primitive in the rect primitive shader.
- Encode each slice as center, radius, inner radius, angle start/end, color,
  border, and selection offset.
- Keep text labels in the existing overlay text system.
- Use palette fallback from theme accent plus stable hue rotations.

Default rendering should avoid dated gradients. Slices should use clean flat
fills with subtle borders, hover emphasis, and optional soft center/backdrop
surface for donut mode.

## Interaction Plan

Phase 1 interactions:

- Hover a slice to show:
  - label
  - value
  - percent
  - optional aggregate metadata
- Click a slice to select it.
- Click empty chart area clears selection when `clear_on_empty=True`.
- `on_select(selection | None)` emits a typed selection object.

Phase 2 interactions:

- Keyboard focus and arrow-key slice navigation.
- Legend hover highlights matching slice.
- Legend click selects matching slice.
- Toolbar controls:
  - toggle legend
  - toggle labels
  - switch pie/donut
  - fit/reset label layout

## CSS And Parts

Expose widget type selector:

```css
PieChart {
    background: #101827;
    border-radius: 10px;
    accent: #5aa9ff;
}
```

Candidate parts:

- `plot`
- `slice`
- `slice-hover`
- `slice-selected`
- `slice-border`
- `donut-hole`
- `center-label`
- `label`
- `leader-line`
- `legend`
- `legend-item`
- `legend-swatch`
- `tooltip`
- `toolbar`
- `toolbar-button`

Inline part styles should support dashed and snake-case names, consistent with
existing widget parts.

## Python Implementation Work

- Add `PieChartSelection` dataclass:
  - `index`
  - `label`
  - `value`
  - `percent`
  - `total`
- Add `PieChartData` normalization helper.
- Add direct sequence path.
- Add frame aggregation path.
- Add validation for values and labels.
- Add `set_data(...)`, `set_frame(...)`, `set_selected(...)`,
  `clear_selection()`, `set_legend_visible(...)`, and `set_labels_visible(...)`.
- Add callback compatibility layer similar to table/scatter callbacks.

## Native Implementation Work

- Add document parsing for `pie_chart` props.
- Add CSS type selector mapping.
- Add layout intrinsic sizing:
  - default min height around 260 px
  - flex grow in plot/dashboard panels
- Add primitive data model for slices.
- Add hit testing by polar angle and radius.
- Add hover and selected state tracking.
- Add overlay text collection for labels and legend.
- Add live prop updates:
  - data replacement
  - selected slice
  - legend/label visibility
  - donut mode
- Add debug snapshot state:
  - slice count
  - selected slice
  - hovered slice
  - total
  - render primitive count

## Probe Plan

Create `examples/css_feature_probes/pie_chart_probe.py` with:

- Basic pie chart from direct labels/values.
- Donut chart with center label.
- Frame-backed count aggregation.
- Top-N chart with `Other`.
- Small-slice stress case.
- Legend placement stress.
- Theme styling stress with at least one dark theme and one light theme.
- Selection callback status label.

Probe questions:

- Are slices anti-aliased cleanly?
- Does text remain readable on narrow panels?
- Do labels avoid overlaps or degrade to legend-only?
- Does hover/select feel consistent with other widgets?
- Do legend swatches align with text?
- Does the widget resize predictably in grid panels?

## V3 Demo Integration

Add a `Pie charts` page or a dashboard section with:

- One donut summary chart.
- One category share pie.
- One top-N chart.
- Controls for legend, labels, donut mode, and theme stress.

Keep it visually and structurally similar to the histogram section so plot
widgets feel like a coherent family.

## Testing

Python tests:

- direct label/value serialization
- frame count aggregation
- frame sum aggregation
- top-N grouping
- zero/negative/non-finite validation
- selected label/index serialization
- callback compatibility

Native tests:

- parse pie chart props
- angle normalization sums to full circle
- hit testing maps points to expected slice
- donut hole rejects hits
- selected slice state persists across rebuild
- legend text collection places labels in overlay pass
- empty chart renders without panic

Visual/manual checks:

- probe renders on dark/light/terminal themes
- labels and legend do not clip in narrow panels
- selected and hover slices are visibly distinct
- menu/modal/toast overlays still render above chart text

## Phases

### Phase 1: Static Pie And Donut Rendering

- [x] Add Python `PieChart` widget.
- [x] Add native `WidgetKind::PieChart`.
- [x] Support direct `labels` and `values`.
- [x] Render flat colored pie slices.
- [x] Render donut mode and center hole.
- [x] Add legend text.
- [x] Add `pie_chart_probe.py`.

### Phase 2: Frame Aggregation And Data Ergonomics

- [x] Add `data`, `category`, `value`, and `aggregate`.
- [x] Add `top_n` and `Other` grouping.
- [x] Add palette/color mapping.
- [ ] Add empty-state rendering.

### Phase 3: Interaction

- [ ] Add hover hit testing and tooltip/readout.
- [ ] Add slice selection and `on_select`.
- [ ] Add legend hover/click linking.
- [ ] Add keyboard navigation.

### Phase 4: Live Updates And Toolbar

- [ ] Add `set_data` and `set_frame`.
- [ ] Add selected/legend/label live setters.
- [ ] Add toolbar controls.
- [ ] Add debug snapshot telemetry.

### Phase 5: Polish And CSS Parts

- [ ] Add CSS part selectors.
- [ ] Improve label collision avoidance.
- [ ] Add outside labels and leader lines.
- [ ] Add V3 demo section.
- [ ] Benchmark high-slice-count behavior.

## Open Questions

- Should default small-slice labels move to legend-only automatically?
- Should `PieChart` support negative values by absolute value, reject them, or
  support diverging radial charts later? Recommendation: reject in V1.
- Should percentages include hidden `Other` categories in the denominator?
  Recommendation: yes, percentages should always use the visible grouped total.
- Should donut center text be user supplied or generated from total?
  Recommendation: support both, with generated total as a later enhancement.
- Should legend be a native part of the widget or composed as child widgets?
  Recommendation: native at first for reliable sizing and overlay text.
