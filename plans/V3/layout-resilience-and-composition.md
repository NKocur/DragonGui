# V3 Layout Resilience And Composition

DragonGUI can already express many CSS-like layouts, but the current ergonomics
make common application compositions too easy to break. The most visible symptom
is clipping: panels run off the right edge, children sit under panel titles,
scrollbars overlap content, and absolute-positioned widgets collide with normal
flow content unless the example author manually tunes widths, heights, and
padding.

This plan defines the longer-term work needed to make DragonGUI layout behavior
feel closer to mature GUI layout systems while preserving the CSS styling model.

## Problems To Solve

### Fragile Two-Column Layouts

Most probe and demo screens use rows of two panels. Today those rows rely on
manual CSS such as:

```css
HLayout.grid {
    gap: 14px;
}

Panel.case {
    width: calc(50% - 7px);
    min-width: 390px;
}
```

This fails when:

- another class overrides `width`
- the window is too narrow for both `min-width` values
- panel padding, scrollbar gutters, and row gaps push total width past 100%
- the author expects wrapping but `HLayout` does not wrap

### Weak Responsive Composition Primitives

DragonGUI has `HLayout`, `VLayout`, `Panel`, and CSS grid support, but it does
not yet have a simple, first-class Python layout widget for "cards in a
responsive grid" or "flow these children onto new rows." Users should not need
to hand-author `calc()` sizing for basic dashboard panels.

### Scroll Containers Require Too Much Manual CSS

Scrollable panels often need explicit `height`, `overflow-y`, `padding-right`,
`padding-bottom`, and scrollbar part styling. Without that, content can sit
under the title, scrollbars can overlap children, or the last item can be
partially unreachable.

### Absolute Children Collide With Flow Content

Absolute positioning is necessary for edge-placement probes and overlay anchor
tests, but absolute children do not reserve space. This is correct in CSS terms,
but easy to misuse in GUI examples. We need safer patterns and possibly helper
containers for anchored controls.

### Defaults Do Not Protect Authors Enough

Panel title spacing, scroll gutters, scrollbar track padding, child clipping,
and minimum sizing should behave well in common cases without per-example
patches. A default panel should not require repeated tuning to avoid ugly
overlap.

## Goals

- Make common app layouts hard to clip accidentally.
- Provide first-class layout APIs for responsive grids and wrapping flows.
- Improve default scroll container behavior so scrollbars reserve practical
  space and final children are reachable.
- Keep CSS compatibility and existing examples working.
- Reduce probe/demo boilerplate by introducing shared, documented layout
  patterns.
- Add layout regression tests that capture the clipping bugs found during probe
  work.

## Non-Goals

- Full browser CSS layout parity.
- Replacing Taffy or rewriting the layout engine from scratch.
- Making every possible absolute-positioned layout safe automatically.
- Hiding overflow by default in ways that make debugging harder.

## Phase 1: Audit Current Layout Failure Modes

Create a focused inventory from existing examples and probes.

Files to inspect:

- `examples/css_feature_probes/form_controls_probe.py`
- `examples/css_feature_probes/overlay_stack_probe.py`
- `examples/css_feature_probes/layout_containers_probe.py`
- `examples/css_feature_probes/navigation_widgets_probe.py`
- `examples/css_feature_probes/widget_metrics_probe.py`
- `examples/all_features_demo.py`
- `examples/all_features_css_demo.py`
- `examples/css_web_capabilities_demo.py`

Record each failure mode with:

- the widget tree shape
- the CSS involved
- expected behavior
- actual behavior
- whether the issue is example misuse, missing primitive, or native layout bug

Deliverable:

- `plans/V3/layout-failure-inventory.md`

## Phase 2: Add A Responsive Grid Widget

Add a Python widget for common card/dashboard layouts.

Proposed API:

```python
with dg.GridLayout(columns=2, min_column_width=360, gap=14):
    dg.Panel("A")
    dg.Panel("B")
    dg.Panel("C")
```

Behavior:

- uses available width to compute column count
- wraps children to new rows when needed
- respects `min_column_width`
- supports `columns="auto"` or fixed integer columns
- supports row and column gaps
- gives children stable widths without requiring per-child `calc()`

Possible CSS mapping:

- native widget kind: `grid_layout`
- internal layout may use Taffy grid or a DragonGUI pre-layout helper
- CSS properties can still override display/gap/sizing where appropriate

Required tests:

- two columns fit at wide width
- one column at narrow width
- child panels never exceed root content width
- gaps are included in width calculations
- scrollbar gutter does not push grid past available width

## Phase 3: Add A Flow Layout Widget

Add a simpler wrapping row container for buttons, tags, badges, and chips.

Proposed API:

```python
with dg.FlowLayout(gap=8, row_gap=8):
    dg.Button("Small")
    dg.Button("Longer Action")
    dg.Tag("warning")
```

Behavior:

- children keep intrinsic width
- wrap onto new rows when they exceed available width
- container height grows based on rows
- supports alignment options such as `start`, `center`, `end`

Required tests:

- buttons wrap instead of clipping
- row gap affects layout
- flow container reports correct height
- flow inside scrollable panel contributes to scroll range

## Phase 4: Improve Panel Body And Title Layout

Panel title handling should create a clear title region and body region without
children slipping underneath the title.

Work items:

- formalize a title/body model in layout code
- reserve title height consistently for `Panel`, `Modal`, `Sidebar` if titled
- ensure scrollable panel body starts below the title
- ensure `overflow-y: auto` measures body content height, not only outer panel
- document how title padding, body gap, and content padding interact

Required tests:

- first child never overlaps title text
- scrollable titled panel can scroll to the first and last child
- title stays fixed while body scrolls when panel scrolling is enabled
- custom title padding does not create oversized modal headers unexpectedly

## Phase 5: Default Scrollbar Gutter And Reachability

Make scroll containers reserve sane space by default.

Work items:

- keep default content-to-scrollbar spacing consistent across panels and root
- reserve gutter only when scrollbar is possible or visible
- ensure final child can scroll fully into view
- prevent small unusable scrollbars from appearing in tiny clipped regions
- expose CSS variables or properties for scrollbar gutter tuning

Potential CSS:

```css
Panel {
    scrollbar-gutter: stable;
    scrollbar-gap: 8px;
}
```

Required tests:

- scrollbar does not overlap rightmost child content
- bottom item is reachable in vertical scroll containers
- nested scroll containers keep correct scroll ranges when parent is scrolled
- clipped offscreen panels do not create flickering scrollbars

## Phase 6: Safe Probe And Demo Layout Templates

Add reusable CSS and Python helpers for examples so new probes do not reinvent
layout.

Possible helper file:

- `examples/css_feature_probes/probe_helpers.py`

Possible helpers:

```python
def probe_app(title: str) -> tuple[dg.App, dg.Window]:
    ...

def probe_header(title: str, description: str) -> None:
    ...

def probe_grid(columns: int = 2):
    ...

def probe_card(title: str, *, scroll: bool = False):
    ...
```

Alternative:

- create a documented CSS snippet in `examples/css_feature_probes/README.md`
- use native `GridLayout` once Phase 2 is complete

Required updates:

- migrate at least three probes to the helper/template
- document the pattern for future probes
- add examples for wide panels, two-column cards, scrollable cards, and button
  rows

## Phase 7: Better Layout Diagnostics

Add tools to reveal why something clipped.

Work items:

- extend `debug_snapshot()` with computed layout constraints:
  - available width/height
  - min/max size
  - resolved width/height
  - overflow area
  - scroll range
  - active CSS rules affecting sizing
- optionally add a debug overlay mode that draws:
  - content box
  - padding box
  - title/body regions
  - scrollbar gutter
  - clipped overflow

Required tests:

- debug snapshot includes width/min-width/source rule data
- scroll containers report body viewport and content extent
- diagnostics remain safe to call outside UI callbacks

## Phase 8: Documentation

Update docs so users know which layout primitive to choose.

Docs to update:

- `docs/widgets.md`
- `docs/css-styling.md`
- new `docs/layout.md`

Topics:

- when to use `HLayout`, `VLayout`, `GridLayout`, and `FlowLayout`
- how panel title/body layout works
- how scroll containers reserve gutter space
- how to avoid absolute-position overlap
- responsive dashboard/card layout examples

## Acceptance Criteria

The V3 layout work is complete when:

- a two-column probe layout can be built without per-card `calc()` widths
- cards wrap or collapse to one column instead of clipping at narrow widths
- scrollable titled panels can always reveal first and last content
- scrollbars have default spacing from content and panel edges
- modal titlebars do not become oversized from normal padding choices
- existing demos still render correctly
- probe authors have a documented reusable pattern
- layout debug output can explain clipping without guesswork

## Suggested Implementation Order

1. Layout failure inventory
2. Scrollbar gutter and panel body/title fixes
3. `GridLayout`
4. `FlowLayout`
5. Probe/demo helper template
6. Layout diagnostics
7. Documentation pass

The highest-impact near-term work is the responsive grid/card primitive. It
would remove the fragile `HLayout + width: calc(50% - gap)` pattern that caused
several of the clipping problems in the current probes.
