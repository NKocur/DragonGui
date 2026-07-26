# DragonGUI Layout

DragonGUI layouts are CSS-styled widgets backed by native layout. Prefer the
highest-level primitive that matches the composition:

| Primitive | Use For |
| --- | --- |
| `AppShell` | Bounded top-level app layouts with sidebars and a main body. |
| `Body` | Flexible scroll-owning main content inside an app shell. |
| `FlexLayout` | CSS-overridable row/column composition that may change direction at breakpoints. |
| `VLayout` | Vertical application roots, forms, and stacked panel content. |
| `HLayout` | Non-wrapping rows where every child is expected to fit. |
| `ScrollArea` | Explicit bounded viewport for overflowing content. |
| `GridLayout` | Responsive card/dashboard layouts that should collapse at narrow widths. |
| `FlowLayout` | Buttons, tags, badges, and chips that should wrap by intrinsic width. |

## Shared Spacing Rhythm

Set the base logical-pixel unit with `Theme.spacing`. DragonGUI derives a
five-step scale for Python layout arguments and CSS:

| Scale | Python | CSS | Value |
| --- | --- | --- | --- |
| Extra small | `theme.space_xs` | `var(--dg-space-xs)` | `spacing × 0.5` |
| Small | `theme.space_sm` | `var(--dg-space-sm)` | `spacing × 1` |
| Medium | `theme.space_md` | `var(--dg-space-md)` | `spacing × 2` |
| Large | `theme.space_lg` | `var(--dg-space-lg)` | `spacing × 3` |
| Extra large | `theme.space_xl` | `var(--dg-space-xl)` | `spacing × 4` |

Use the same semantic step for sibling sections that should align:

```python
theme = dg.Theme.dark(spacing=6)

with dg.Splitter(gutter_size=theme.space_md):
    ...

with dg.GridLayout(columns=2, gap=theme.space_md):
    ...
```

```css
ScrollArea.page-stack {
    gap: var(--dg-space-md);
}
```

Containers own space between children; widgets own their internal padding.
For splitters, `gutter_size` owns pane separation and the full draggable target.
`Splitter::gutter` controls only the thin divider painted inside that target.

## App Shells

For most tools and dashboards, start with `AppShell` and put the flexible main
region in `Body`. This establishes a bounded window-sized layout, makes the
main region shrink correctly next to fixed sidebars, and gives scrolling to one
clear owner:

```python
win = dg.Window("Tool", width=1200, height=800)

with dg.AppShell():
    with dg.Sidebar(title="Controls", width=320):
        ...

    with dg.Body(scroll="y", gap=12):
        with dg.GridLayout(columns=2, min_column_width=360, gap=12):
            ...
```

Use this pattern instead of a raw top-level `HLayout` plus fixed-width panels
when the content may grow. `Body` applies the usual safe defaults:
`min_width: 0`, `min_height: 0`, flexible fill, and explicit overflow.

`AppShell` is a `FlexLayout`. Its direction is a widget default, so application
CSS can replace it at a breakpoint:

```css
@media (max-width: 700px) {
    AppShell {
        flex-direction: column;
    }
}
```

Direct `Body`, `WorkbenchLayout`, and `WorkbenchMain` children keep configurable
width and height safeguards (160 by 96 logical pixels by default). Eligible
siblings such as a normal `Sidebar` shrink before protected main content.
Explicitly non-shrinking siblings may still exceed the viewport; the shell
clips outer overflow and diagnostics report the failed allocation.

`Sidebar(state="auto")` uses an expanded desktop state, a compact rail at
700 logical pixels and below, and a closed overlay drawer policy at 480 logical
pixels and below. Configure `collapsed_width`, `compact_mode`, and
`mobile_mode`, and use `sidebar.menu_button()` or `open_drawer()` when a mobile
opener is required. Explicit `expanded`, `collapsed`, `hidden`, and `drawer`
states are also available for application-controlled navigation.

## Responsive Card Grids

Use `GridLayout` instead of `HLayout` plus per-card `calc()` widths:

```python
with dg.GridLayout(columns=2, min_column_width=360, gap=14):
    with dg.Panel("A", class_="case"):
        ...
    with dg.Panel("B", class_="case"):
        ...
```

An integer `columns` value is a maximum column count when `min_column_width` is
set. The grid uses the available content width and column gap to choose how many
columns fit, then wraps children into additional rows. `columns="auto"` creates
an unbounded auto-fill grid based on `min_column_width`; `"auto-fit"` collapses
unused adaptive tracks.

For deterministic viewport behavior, pass a responsive map:

```python
with dg.GridLayout(
    columns={"default": 4, 1100: 2, 700: 1},
    min_column_width=210,
    gap=12,
):
    ...
```

`"default"` is required. Numeric keys are inclusive logical-viewport
`max-width` thresholds, independent of display scale, and are evaluated from
smallest to largest. The selected count remains a maximum when
`min_column_width` is present, so a narrow grid nested inside a wider window can
still collapse safely. Set `balance_last_row=True` to center an incomplete final
row. This balancing is intentionally skipped for masonry and explicitly placed
grid children.

## Wrapping Rows

Use `FlowLayout` for intrinsic-width controls:

```python
with dg.FlowLayout(gap=8, row_gap=8, align="start"):
    dg.Button("Small")
    dg.Button("Longer Action")
```

`align` accepts `start`, `center`, or `end`. `cross_align` accepts `start`,
`center`, `end`, or `stretch`.

`HLayout` is deliberately a non-wrapping row. Use it when the children form
one semantic line and can shrink, or when a scroll owner intentionally exposes
the extra width. Use `FlowLayout` when the number or intrinsic width of controls
can exceed the available row. Toolbars wrap by default; set
`flex-wrap: nowrap` only when clipping or horizontal scrolling is intentional.

## Preferred, Minimum, And Fixed Sizes

An authored CSS `width`/`height`, percentage, `calc()` value, panel `width`, or
pane `size` is normally a preferred size. A flex parent may shrink it to keep
the layout bounded. Text controls contribute a semantic minimum for their
non-text chrome and use ellipsis when a narrower parent cannot display the
whole single-line label.

Use `min-width`/`min-height` for a real usability floor. At extremely small
viewports, framework chrome, sidebars, and splitter panes cap ordinary
constructor minima so the application still has a usable body region.

Request a deliberately hard main-axis size with authored CSS plus
`flex-shrink: 0`. This is an opt-out: if the size cannot fit, put it inside an
explicit horizontal or vertical scroll owner rather than relying on accidental
window clipping.

### Flex sizing defaults

This table is the user-facing sizing contract. “Intrinsic” means the widget
starts from its content or semantic control geometry; “zero” means it may
shrink to the parent's allocation unless an explicit minimum is authored.

| Widget family | Grow | Shrink | Default minimum / behavior |
| --- | ---: | ---: | --- |
| Buttons, inputs, labels, badges, tags, menus | 0 | 0 | Intrinsic or semantic control geometry |
| `Panel` | 0 | 1 | Content-sized with zero flex minimum |
| `FlowLayout`, `TreeView`, drag/drop containers | 0 | 1 | Content-sized with zero flex minimum |
| `HLayout`, `VLayout`, `GridLayout`, `Pages`, active `Page`, plots and tables | 1 | 1 | Flexible region with zero flex minimum |
| `Body` | 1 | 1 | Flexible, zero minimum, explicit overflow owner |
| `ScrollArea` | 1 | 1 | Bounded viewport; vertical scrolling by default |
| `Spacer` | 1 | 1 | Zero-content flexible space |
| `MenuBar`, `StatusBar`, `Tabs` | 0 | 0 | Compact intrinsic chrome |
| `Sidebar` | 1 | 1 | Configured expanded/collapsed width plus shell safeguards |
| `Image`, `HtmlReport` | 1 | 1 | Flexible viewport with a small semantic minimum |
| `SearchBox` and configurable composites | `grow` argument, default false | `shrink` argument, default true | Preferred/min/max constructor contract |

The table describes native defaults before parent-context adjustments.
Cross-axis stretching may still fill a parent's width or height. Explicit
`grow`, `shrink`, CSS flex properties, authored minima, and fixed dimensions
remain authoritative.

## Panels And Scrolling

Titled `Panel`, `Sidebar`, and `Modal` containers reserve title space before
laying out children. Scrollable titled panels clip their child content to the
body region below the title so the title remains readable. Title height,
resolved top padding, and title/body gap are counted once; the same body
rectangle drives layout, painting, clipping, hit testing, and scrolling.

Panels with vertical scrolling reserve a default scrollbar gutter so content is
not painted under the scrollbar. Explicit `overflow-y: auto` or `scroll` also
reserves gutter space.

Use `ScrollArea` when the viewport is the important behavior and the frame is
provided by another widget:

```python
with dg.Panel("Controls", style={"height": 420, "min_height": 0}):
    with dg.ScrollArea(axis="y", gap=8):
        ...
```

Inside a vertical layout with fixed controls before or after it, leave the
`ScrollArea` height unset. By default it grows into the remaining space, can
shrink below its content, and owns the overflow, matching Qt/Tk style layout
behavior.

This mirrors mature GUI toolkits such as Qt: layouts distribute bounded
rectangles, while scroll containers explicitly own clipping and scroll ranges.
`GridLayout` does not guess which child should scroll; put long content inside a
`ScrollArea` or set `overflow-y: auto` on the intended container.

Scroll ranges include resolved right and bottom padding. When both axes own
overflow, each maximum is nonnegative and moving to it makes the corresponding
content end reachable. Nested scroll regions should each have a definite
viewport; otherwise prefer one outer owner.

Active `Page` children are laid out through their own page style before their
contents are positioned, so page-level padding, gap, and overflow settings can
be used to define the bounded region for child grids and scroll areas.

## Debugging Clipping

`app.debug_snapshot()["layout"]` includes:

- `rects`: resolved widget rectangles
- `clips`: visible clip rectangles
- `diagnostics`: per-widget resolved size, available visible size, overflow
  amount, scroll range, and an `issues` array of native semantic diagnostics
- `scroll_max_x` / `scroll_max_y`: scrollable extents
- `resolved_grids`: resolved column count and physical-pixel track widths for
  populated `GridLayout` nodes

Each `layout.diagnostics[id].inspection` entry consolidates:

- intrinsic, preferred, and semantic-minimum sizing
- allocated flex-axis size, basis, grow, and shrink values
- the final resolved rectangle and final paint clip
- per-axis overflow policy, scroll ownership, offset, and range
- separate structural-diagnostic and usability-advisory arrays

`computed_styles[id].matched_selectors` lists every active matched selector with
its stylesheet origin, source position, source order, and specificity.
`computed_styles[id].provenance[property]` records the winning declaration and
every overridden candidate.

Use `computed_styles` in the same snapshot to see matched CSS rules and sizing
properties that contributed to a layout.

Root diagnostics currently include:

- `below-minimum-viewport`: an authored window minimum is larger than the
  physical viewport. The root remains exactly viewport-sized; inspect
  `axis`, `available`, and `required` to choose a responsive breakpoint.
- `unreachable-root-overflow`: content escapes the default bounded window with
  no intervening scroll owner. Add a `ScrollArea`, assign `overflow-x/y: auto`
  to the intended owner, or make the overflowing row responsive.

Each issue also includes a stable human-readable `message`. Explicitly
scrollable windows own their overflow and do not emit
`unreachable-root-overflow`.

Diagnostics are partitioned by intent:

- **Structural errors** mean the resolved geometry violates a correctness
  contract, such as non-finite/negative geometry, an unreachable visible
  subtree, a starved flexible region, or root overflow with no owner. Strict
  layout checks fail on these.
- **Usability advisories** mean geometry is valid but may be impractical, such
  as a tiny hit target, truncated placeholder, undersized scroll viewport,
  responsive orphan, or excessive unused flex space. Strict usability checks
  can promote these to a failing audit without conflating them with corruption.

```python
snapshot = app.debug_snapshot()
node = snapshot["layout"]["diagnostics"]["results-scroll"]
print(node["inspection"]["sizing"])
print(node["inspection"]["flex_allocation"])
print(node["inspection"]["scroll_ownership"])
print(node["inspection"]["structural_diagnostics"])
print(node["inspection"]["usability_advisories"])
```

## Stateful Visual Audits

Visual-audit targets may declare named routes and deterministic actions:

```json
{
  "states": [
    {"name": "overview", "route": "overview"},
    {"name": "modal-open", "actions": ["click:#new-review", "wait:200"]},
    {"name": "scrolled", "actions": ["scroll:#body=0,240"]}
  ]
}
```

Supported actions are `click:#id`, `hover:#id`, `type:#id=value`,
`scroll:#id=x,y`, `resize:WIDTHxHEIGHT`, and `wait:MILLISECONDS`. Targets
without `states` retain the original single, no-interaction capture.

Reports include a thumbnail gallery labeled by size, scale, route, and state,
plus diagnostic deltas between captures. Failed diagnostics link to the
relevant screenshot, full snapshot, and a focused node-data JSON artifact.

## Intentional Visible Overflow

`overflow: visible` is useful for decoration or content that is intentionally
allowed to escape a container. It does not create a scroll range and should not
be used to hide a sizing bug. Overlays such as menus, tooltips, modals, and
toasts are positioned separately from normal flow and are bounded to the root
viewport by the framework.

## Migration Notes

- Replace internal/native renderer selectors such as lowercase/snake-case
  render kinds with exact public widget types: `SearchBox`, `Toolbar`,
  `ColorPicker`, `AppShell`, and so on. Public composites also match their
  public base types.
- Widget constructor defaults now participate below user stylesheets. Remove
  application rules that existed only to defeat an old composite default;
  retain explicit `style={...}` only when a local override is intentional.
- Ordinary `Panel` instances no longer grow just because unused space exists.
  Set `flex-grow: 1` or use a flexible `Body`/viewport when expansion is the
  intended behavior.
- `SearchBox` defaults to a 340-pixel preferred width, a 180-pixel minimum,
  `grow=False`, and `shrink=True`. Use constructor sizing arguments rather than
  styling its internal input.
- Prefer `FlexLayout`, `AppShell`, responsive `GridLayout.columns`, and Sidebar
  states over manually rebuilding the widget tree at breakpoints. Existing
  fixed behavior remains available through explicit direction, dimensions,
  state, and `flex-shrink: 0`.
- Percentage widths and `calc()` dimensions are shrinkable preferred sizes in
  constrained flex rows.
- Constructor sidebar widths and numeric splitter pane sizes may contract in
  below-minimum viewports. Authored CSS with `flex-shrink: 0` requests the old
  hard-size behavior.
- Long single-line control text may ellipsize instead of forcing its parent to
  overflow. Labels continue to wrap by default.
- Scroll maxima consistently include resolved end padding.
- The strict layout audit can now report invalid geometry that older snapshot
  consumers silently skipped. Assign overflow to a real owner or make the
  affected row responsive instead of suppressing the diagnostic.
