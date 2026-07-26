# Layout

DragonGUI layouts are native, retained widget trees. Prefer predictable
containers and stable dimensions over rebuilding large subtrees.

## Core Containers

- `Window`: top-level application surface.
- `Panel`: titled or untitled framed content area.
- `FlexLayout`: CSS-overridable direction, wrapping, gap, and alignment.
- `HLayout` and `VLayout`: horizontal and vertical stacking.
- `GridLayout`: responsive dashboard grids and masonry layouts.
- `FlowLayout`: wrapping rows for chips, tags, and compact command groups.
- `ScrollArea`: explicit scroll container for overflow content.
- `Splitter` and `Pane`: resizable split views.
- `Pages`, `Tabs`, and `Sidebar`: navigation structures.
- `Spacer` and `Separator`: lightweight spacing and visual dividers.

## Practical Rules

- Start sidebar/body applications with `AppShell`, `Sidebar`, and `Body`.
- Treat `Panel(width=...)`, percentages, and numeric pane sizes as preferred
  sizes that may shrink. Add CSS `flex-shrink: 0` only for an intentional hard
  size.
- Use `GridLayout(masonry=True)` for cards with different natural heights.
- Use `GridLayout(columns={"default": 4, 1100: 2, 700: 1})` when dashboard
  column counts must change at deterministic logical-viewport breakpoints.
- Use `balance_last_row=True` to center an incomplete non-masonry final row.
- Use `HLayout` for one non-wrapping semantic row and `FlowLayout` for controls
  that should wrap as the viewport narrows.
- Give overflowing content an explicit `ScrollArea` or `overflow-x/y: auto`
  owner with a bounded viewport.
- Put large tables and plots in stable containers so layout changes do not force
  unnecessary work.
- Prefer live setters for changing values, labels, plot data, and visibility.

Titled panels reserve a header band and lay out, clip, and scroll children in
the body below it. Sidebars and splitter panes contract gracefully in
below-minimum windows; ordinary sizes remain unchanged.

`Sidebar(state="auto")` expands on desktop, uses its configured compact
rail policy at 700 logical pixels and below, and uses its mobile drawer policy
at 480 logical pixels and below. `AppShell` direction can be overridden with
normal responsive CSS, while `GridLayout.columns` accepts logical-viewport maps
such as `{"default": 4, 1100: 2, 700: 1}`.

## Sizing Contract

| Family | Grow | Shrink | Default |
| --- | ---: | ---: | --- |
| Intrinsic controls and chrome | 0 | 0 | Semantic/intrinsic size |
| `Panel` and content containers | 0 | 1 | Content-sized, zero flex minimum |
| Flexible layouts, pages, plots, tables | 1 | 1 | Flexible, zero flex minimum |
| `Body`, `ScrollArea`, `Spacer` | 1 | 1 | Explicit flexible/overflow role |
| Configurable composites such as `SearchBox` | opt-in | yes | Preferred/min/max constructor contract |

Snapshot issues are separated into structural errors, which violate geometry
correctness and fail strict layout checks, and usability advisories, which flag
valid but impractical results and fail only strict usability checks.

For snapshot diagnostics, scroll-range semantics, intentional visible overflow,
complete examples, and migration notes, see `../layout.md`.
