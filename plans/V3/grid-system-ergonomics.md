# V3 Grid System Ergonomics

DragonGUI's current `GridLayout` is useful for broad card grids, but it is too
limited and too easy to misuse for compact application layouts. The
`ThreadMonitor` probe exposed this clearly: what looked like panel padding was
actually grid row/column behavior spreading compact diagnostics across a wide
two-column grid.

This plan defines the work needed to make DragonGUI's grid system predictable
for both dashboard/card grids and compact key/value or table-like layouts.

## Current Problems

### Equal-Width Bias

`GridLayout(columns=2)` behaves like a broad two-column layout. That is good
for panels/cards, but poor for compact key/value content:

```text
depth / max                   0
avg                           0.0
total                         10
rate                          4/s
```

The desired layout is usually:

```text
depth / max    0
avg            0.0
total          10
rate           4/s
```

This needs a fixed or intrinsic first column and a flexible second column.

### Python API Cannot Express Track Lists Ergonomically

The obvious API should be possible:

```python
dg.GridLayout(columns=("auto", "1fr"))
dg.GridLayout(columns=(72, "1fr"))
dg.GridLayout(columns=("min-content", "1fr"))
```

Today the helper is oriented around integer column counts and minimum card
widths. That makes dashboard grids convenient, but key/value grids awkward.

### Row Sizing And Row Gaps Are Not Obvious

Compact grids need control over:

- row height
- row gap
- column gap
- whether rows use intrinsic height or a fixed compact height
- whether labels wrap

When defaults are too loose, users often blame panel padding because the visual
symptom is empty space around content.

### Grid Is Used For Multiple Layout Jobs

Current examples use grid-like behavior for at least three different jobs:

- responsive card/dashboard layouts
- dense key/value diagnostics
- small repeated tiles/badges

Those should not all require the same API shape.

## Goals

- Make compact key/value grids easy to express.
- Keep the current simple card-grid API.
- Support explicit CSS-like track lists from Python.
- Make row height and row gap behavior predictable.
- Improve docs so users choose `GridLayout`, `FlowLayout`, or `HLayout` rows correctly.
- Add tests that prevent broad card-grid behavior from leaking into compact grids.

## Non-Goals

- Full browser CSS grid parity.
- Replacing every `HLayout` row with grid.
- Making `GridLayout(columns=2)` automatically infer key/value intent.

## Phase 1: Audit Existing Grid Usage

Inspect all current `GridLayout` usage and classify it.

Files to inspect:

- `python/dragongui/thread_monitor.py`
- `examples/css_feature_probes/responsive_layout_probe.py`
- `examples/css_feature_probes/layout_containers_probe.py`
- `examples/css_feature_probes/navigation_widgets_probe.py`
- `examples/css_feature_probes/form_controls_probe.py`
- `examples/all_features_v3_demo.py`
- any other `rg "GridLayout\\("` matches

For each usage, record:

- intended layout type: card grid, key/value grid, tile grid, form grid
- current API used
- current visual risks
- whether `GridLayout`, `FlowLayout`, or simple rows are the best fit

Deliverable:

- `plans/V3/grid-usage-inventory.md`

## Phase 2: Add Track List Support To Python API

Extend `GridLayout(columns=...)` to accept explicit track lists.

Proposed API:

```python
dg.GridLayout(columns=2)
dg.GridLayout(columns="auto")
dg.GridLayout(columns=("auto", "1fr"))
dg.GridLayout(columns=(72, "1fr"))
dg.GridLayout(columns=("min-content", "1fr"))
dg.GridLayout(columns=("max-content", "1fr"))
dg.GridLayout(columns=("repeat", 3, "1fr"))  # optional later
```

Accepted track values:

- `int | float`: logical pixels
- `"auto"`
- `"1fr"`, `"2fr"`, etc.
- `"min-content"`
- `"max-content"`
- `"fit-content(<length>)"` if parser support already exists or is added

Serialization option:

- keep `props.columns` for integer/auto card mode
- add `style.grid_template_columns` for explicit track list mode

Open question:

- If explicit `columns` track list is given, should `min_column_width` be rejected
  or ignored? Prefer rejecting incompatible combinations to avoid ambiguous
  behavior.

## Phase 3: Add Native Track Support Where Missing

Ensure native layout accepts and correctly maps the track list.

Required native behavior:

- fixed logical px tracks
- `fr` tracks
- `auto` tracks
- min/max content if Taffy supports them in the current version
- gaps included in total width calculations
- scroll gutter does not cause grid overflow

Required tests:

- `columns=(72, "1fr")` keeps first column at 72px
- `columns=("auto", "1fr")` keeps first column intrinsic
- `columns=("1fr", "2fr")` divides remaining width proportionally
- fixed tracks plus gap do not exceed parent unexpectedly
- grid inside scroll area contributes correct content height

## Phase 4: Row Sizing Controls

Add explicit row sizing support to the Python helper.

Proposed API:

```python
dg.GridLayout(
    columns=(72, "1fr"),
    rows="auto",
    row_height=17,
    gap=6,
    row_gap=2,
)
```

Possible options:

- `rows`: explicit row track list, similar to columns
- `row_height`: shorthand for repeated fixed row height
- `auto_rows`: CSS-like grid-auto-rows value

Required tests:

- fixed `row_height` applies to generated rows
- `row_gap=0` actually removes vertical spacing
- row height does not clip text by default
- row height can intentionally clip if user opts into too-small values

## Phase 5: Add KeyValueList Helper

Even with a better grid, diagnostics and property panels are common enough to
deserve a helper.

Proposed API:

```python
with dg.KeyValueList(label_width=72, row_height=17, gap=6, row_gap=2):
    dg.KeyValue("depth / max", "0")
    dg.KeyValue("avg", "0.0")
```

Alternative lightweight API:

```python
with dg.KeyValueList(label_width=72):
    dg.Label("depth / max")
    dg.Label("0")
```

Preferred behavior:

- stable two-column compact layout
- label column fixed or intrinsic
- value column flexes
- sensible row height
- optional value color/status styling

This helper could be implemented in Python on top of `HLayout` rows first, then
later backed by improved grid tracks.

## Phase 6: Update ThreadMonitor

Once grid/key-value APIs are improved, revisit `ThreadMonitor`.

Preferred final shape:

- `ThreadMonitor(title=None, compact=True)` for tight diagnostics
- default title remains available for sidebar/debug panels
- queue metrics use `KeyValueList`
- thread rows use either compact `HLayout` rows or a fixed-track grid:

```python
dg.GridLayout(columns=(10, "1fr", 72), row_height=18, row_gap=2)
```

Acceptance:

- changing panel padding visibly changes outer spacing
- row spacing can be tightened independently
- no broad empty columns unless explicitly requested
- scroll position is preserved across refreshes

## Phase 7: Documentation And Examples

Update:

- `docs/widgets.md`
- new or existing `docs/layout.md`
- `examples/css_feature_probes/README.md`

Document:

- card grid pattern
- key/value grid pattern
- tile/badge flow pattern
- when to use `GridLayout` versus `FlowLayout` versus `HLayout`
- how `gap`, `row_gap`, row height, and track widths interact

Add examples:

- `examples/css_feature_probes/grid_tracks_probe.py`
- compact key/value grid example
- responsive card grid example
- mixed fixed/flexible track example

## Acceptance Criteria

The grid system work is complete when:

- `GridLayout(columns=(72, "1fr"))` works from Python.
- `GridLayout(columns=("auto", "1fr"))` works for intrinsic labels.
- row gap and row height can be controlled predictably.
- compact diagnostics no longer look like they have fake padding.
- dashboard/card grids still work with the existing simple API.
- docs clearly explain the intended layout primitive for common cases.
- tests cover fixed, intrinsic, flexible, and responsive grid tracks.

## Suggested Implementation Order

1. Audit existing grid usage.
2. Add Python track-list API.
3. Add or verify native fixed/fr/auto track support.
4. Add row height / auto row controls.
5. Add `KeyValueList` helper.
6. Migrate `ThreadMonitor`.
7. Add docs and a dedicated grid track probe.
