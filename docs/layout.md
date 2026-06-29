# DragonGUI Layout

DragonGUI layouts are CSS-styled widgets backed by native layout. Prefer the
highest-level primitive that matches the composition:

| Primitive | Use For |
| --- | --- |
| `AppShell` | Bounded top-level app layouts with sidebars and a main body. |
| `Body` | Flexible scroll-owning main content inside an app shell. |
| `VLayout` | Vertical application roots, forms, and stacked panel content. |
| `HLayout` | Non-wrapping rows where every child is expected to fit. |
| `ScrollArea` | Explicit bounded viewport for overflowing content. |
| `GridLayout` | Responsive card/dashboard layouts that should collapse at narrow widths. |
| `FlowLayout` | Buttons, tags, badges, and chips that should wrap by intrinsic width. |

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
an unbounded auto-fill grid based on `min_column_width`.

## Wrapping Rows

Use `FlowLayout` for intrinsic-width controls:

```python
with dg.FlowLayout(gap=8, row_gap=8, align="start"):
    dg.Button("Small")
    dg.Button("Longer Action")
```

`align` accepts `start`, `center`, or `end`. `cross_align` accepts `start`,
`center`, `end`, or `stretch`.

## Panels And Scrolling

Titled `Panel`, `Sidebar`, and `Modal` containers reserve title space before
laying out children. Scrollable titled panels clip their child content to the
body region below the title so the title remains readable.

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

Active `Page` children are laid out through their own page style before their
contents are positioned, so page-level padding, gap, and overflow settings can
be used to define the bounded region for child grids and scroll areas.

## Debugging Clipping

`app.debug_snapshot()["layout"]` includes:

- `rects`: resolved widget rectangles
- `clips`: visible clip rectangles
- `diagnostics`: per-widget resolved size, available visible size, overflow
  amount, and scroll range
- `scroll_max_x` / `scroll_max_y`: scrollable extents

Use `computed_styles` in the same snapshot to see matched CSS rules and sizing
properties that contributed to a layout.
