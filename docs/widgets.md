# DragonGUI Widgets

This document catalogs the public DragonGUI Python widgets and the main
features each one exposes. It complements `docs/css-styling.md`, which covers
the CSS property and selector model in more detail.

The same information is also available at runtime through `dg.help`. Use
`dg.help.reference.widgets.number_input()`, `dg.help.reference.css_parts()`, or
`dg.help.find_symbol("NumberInput")` when an LLM/tool needs structured in-library
guidance without reading the docs files directly.

## Common Widget Features

Most widgets accept these keyword arguments:

| Argument | Purpose |
| --- | --- |
| `id` | Explicit runtime id. If omitted, DragonGUI generates one. |
| `key` | Stable identity for retained/component workflows. |
| `class_` | CSS class string. Multiple classes may be separated by whitespace. |
| `style` | Inline style dictionary. Supports layout, visual, text, and supported `parts`. |
| `tooltip` | Optional hover tooltip text. Not accepted by `Window`. |
| `parent` | Explicit parent container. Omit it inside a container context. |

All widgets serialize into the native document tree through `to_dict()`.
Widgets with live update methods call into the running native app when attached
to an `AppHandle`.

Common live methods:

| Method | Applies To | Purpose |
| --- | --- | --- |
| `set_style(style)` | All widgets | Replace inline style and enqueue a style patch when live. |
| `set_value(...)` | Value widgets | Update the displayed value and enqueue a native prop update when live. |
| `set_checked(...)` | `Checkbox` | Update checked state. |
| `show()` / `close()` | `Modal` | Open or close a modal. `set_open(value)` remains as a compatibility alias for boolean state bridges. |

## Layout And Containers

| Widget | Features |
| --- | --- |
| `Window(title, width=1024, height=768)` | Root container and native window metadata. Must be created outside any active layout context. |
| `AppShell(gap=0, direction="row", wrap=False, min_content_width=160, min_content_height=96)` | Responsive full-window app root for sidebar plus main-body layouts. Application CSS can change direction at breakpoints; eligible chrome yields before protected main content falls below its safeguards. |
| `Body(scroll="y", gap=None)` | Flexible scroll-owning main content pane for `AppShell`. Defaults to `min_width=0`, `min_height=0`, and fill behavior. |
| `FlexLayout(direction="row", wrap=False, gap=8, align_items="stretch")` | General flex container whose construction values remain overridable by application CSS and media rules. |
| `HLayout` | Non-wrapping horizontal flex container. Useful for one-line rows, split panes, and button groups whose children can fit or shrink. |
| `VLayout` | Vertical flex container. Useful for forms, panels, and stacked sections. |
| `ScrollArea(axis="y", gap=None, width=None, height=None)` | Bounded scroll viewport for content that may exceed its parent. Defaults to vertical scrolling and uses a vertical layout for children. |
| `GridLayout(columns=2, min_column_width=320, masonry=False, balance_last_row=False, gap=None, row_gap=None)` | Responsive grid container for cards and dashboards. `columns` accepts an integer, `"auto"`, `"auto-fit"`, or a logical-viewport map such as `{"default": 4, 1100: 2, 700: 1}`. Integer/responsive counts remain maxima when `min_column_width` is set. |
| `FlowLayout(gap=None, row_gap=None, align="start", cross_align="start")` | Wrapping row container for intrinsic-width buttons, tags, and badges; prefer it over `HLayout` when row width is uncertain. |
| `Panel(title=None, width=None)` | Titled or untitled container with optional preferred width. Vertically scrolls overflowing child content with the mouse wheel. Supports `Panel::accent`. |
| `Collapsible(title, expanded=True, on_change=None, disabled=False)` | Header/body container for optional sections. Header toggles expanded state; children lay out only when expanded. Supports `set_expanded()`, `expand()`, `collapse()`, `toggle()`, and `Collapsible::header`, `indicator`, `body`. |
| `Sidebar(title=None, width=220, collapsed_width=56, state="auto", collapsible=True, compact_mode="rail", mobile_mode="drawer")` | Responsive navigation container with expanded, collapsed rail, hidden, and overlay-drawer states. Auto mode uses compact policy through 700 logical pixels and closed mobile-drawer policy through 480. `menu_button()` creates an accessible opener; closing restores its focus and the previous Sidebar state. |
| `StatusBar(height=28)` | Bottom/status container with fixed height. |
| `Spacer(width=None, height=None)` | Empty flexible or fixed space. Width/height must be non-negative. |
| `Separator(orientation="auto")` | Visual divider. Orientation may be `auto`, `horizontal`, or `vertical`. |

Container widgets can be used as context managers:

```python
with dg.HLayout():
    dg.Button("Run")
    dg.Button("Stop")
```

## Menus And Overlays

| Widget | Features |
| --- | --- |
| `MenuBar(height=34)` | Container for top-level `Menu` widgets only. |
| `Menu(label, disabled=False)` | Top-level menu inside a `MenuBar`. Contains `MenuItem` children. |
| `MenuItem(label, on_click=None, disabled=False)` | Clickable menu entry. Emits `click` when `on_click` is supplied and not disabled. |
| `ContextMenu(target=None, width=220)` | Popup menu attached to a widget id or widget instance. Contains `MenuItem` children. |
| `Tooltip(target, width=280, height=None)` | Rich hover tooltip attached to a widget id or widget instance. Contains arbitrary children and does not take focus. |
| `Modal(title="", open=False, width=420, height=220)` | Floating modal container. Prefer `show()` and `close()`; `set_open(value)` is kept as a compatibility alias. |
| `toast(message, level="info", duration=3000, opacity=None, radius=None, padding=None, position=None, app=None)` / `App.toast(...)` | Non-blocking native notification overlay. Levels are `info`, `success`, `warning`, and `error`; pass `duration=None` for a persistent toast. `position` accepts `top-right`, `top-left`, `bottom-right`, or `bottom-left`. Returns `ToastHandle` with `update(...)` and `dismiss()`. |

Helper builders:

| Helper | Features |
| --- | --- |
| `open_file_dialog(title=None, filters=None, on_select=None, app=None)` | Native single-file picker. Returns a path synchronously or calls `on_select(path)` asynchronously. |
| `open_files_dialog(title=None, filters=None, on_select=None, app=None)` | Native multi-file picker. Returns a list of paths or `None` on cancel. |
| `save_file_dialog(title=None, filters=None, on_select=None, app=None)` | Native save-file picker. |
| `pick_folder_dialog(title=None, on_select=None, app=None)` | Native folder picker. |
| `alert(title, message, open=True, width=420, height=200, on_close=None)` | Builds a modal with message text and an OK button. |
| `confirm(title, message, open=True, width=460, height=220, on_confirm=None, on_cancel=None)` | Builds a modal with Cancel and Confirm buttons. |

## Navigation

| Widget | Features |
| --- | --- |
| `Tabs(value=None, on_change=None, disabled=False)` | Container for `Tab` children. If `value` is omitted, the first tab becomes active. `set_value(value, notify=False)` switches tabs programmatically; pass `notify=True` to invoke `on_change`. Emits `change` for native interaction when `on_change` is supplied and not disabled. Default tabs paint only the tab buttons; `Tabs::header` is opt-in for a strip/divider/background. |
| `Tab(label, value=None, badge=None, disabled=False)` | Tab page container. `value` defaults to a route-safe version of `label`. Must be created directly inside `Tabs`. Optional badge supports `set_badge(value)`. Supports `Tab::tab`, `accent`, and `badge`. |
| `Pages(value=None, on_change=None)` | Container for route-based `Page` children. If `value` is omitted, the first page becomes active. `set_value(value, notify=False)` switches pages programmatically; pass `notify=True` to invoke `on_change`. Native page-route changes emit `change` when `on_change` is supplied; programmatic construction does not call it. |
| `Page(value, title=None)` | Page container. Must be created directly inside `Pages`. |
| `NavItem(label, page, icon=None, compact_label=None, badge=None, disabled=False)` | Navigation row targeting a `Page` route. Icon-bearing rows use compact labels at intermediate widths and centered icon-only presentation at rail widths; compact badges become indicator dots. The full label remains the accessible name and default tooltip. Optional badge supports `set_badge(value)`. Supports `NavItem::item`, `accent`, and `badge`. |
| `Breadcrumbs(items, current=None, width=None, min_width=0, max_width=None, grow=False, shrink=True)` | Full-width, shrink-safe path navigation. Long paths may be collapsed with `max_items`; explicit bounds and growth control the outer row without styling generated buttons and separators. |
| `TreeView(items=None, selected=None, width=None, min_width=0, max_width=None, grow=True, shrink=True)` | Hierarchical navigation viewport. It remains flexible by default, unlike content-sized form composites, but its bounds and growth can be controlled directly. |

## Text And Basic Controls

| Widget | Features |
| --- | --- |
| `Label(text, wrap=True)` | Static text. Wraps by default inside constrained containers; pass `wrap=False` for single-line labels. `set_value(value)` updates text. |
| `Badge(text, level="info")` | Compact status/count pill. `level` may be `neutral`, `info`, `success`, `warning`, `danger`, or `error`. Supports `set_value(value)` and `set_level(level)`. |
| `Tag(text, level="neutral")` | Compact bordered status label with the same API as `Badge`. |
| `LED(state=False, states=None, on_color="success", off_color="disabled", size=14)` | Compact status light. Boolean state maps to `on`/`off`; string state names resolve through `states`. Supports `set_state()`, `set_on()`, `set_color()`, and `set_size()`. |
| `Button(text, on_click=None, badge=None, disabled=False)` | Clickable button. Emits `click` when `on_click` is supplied and not disabled. `click()` invokes the Python callback directly. Optional badge supports `set_badge(value)`. |
| `IconButton(icon, on_click=None, disabled=False, size=None, width=None, height=None)` | Compact button using a semantic native vector icon. Use `resolve_icon(name)` to inspect canonical aliases and fallback behavior; unknown names render the built-in `more` glyph. Supports live `set_icon(name)` and `IconButton::icon`. |
| `TextInput(value="", placeholder="", on_change=None, disabled=False)` | Editable text field. `set_value(value)` updates text. |

Application icon geometry can be replaced before startup or live with
`App.set_icon_theme({"search": IconResource([...])})`. The bounded monochrome
resource uses `IconStroke` polylines; CSS continues to own tint, state, size,
and spacing. Live `IconButton.set_icon()` changes are reconciled against the
same retained registry. See `examples/icon_theme_demo.py`.
| `SearchBox(value="", placeholder="Search...", width=340, min_width=180, max_width=None, grow=False, shrink=True, clearable=True)` | Composite search field with fixed search/clear chrome and a flexible inner input. It uses a 340-pixel standalone preferred width, shrinks safely to 180 pixels, and fills remaining toolbar or row space with `grow=True`. `clearable=False` releases the clear-button slot. Explicit `style` sizing remains authoritative. |
| `TextArea(value="", placeholder="", rows=4, wrap=True, on_change=None, disabled=False)` | Editable multiline text field. Newlines are preserved; `rows` controls preferred height; overflow scrolls inside the field; `wrap` controls long-line wrapping. `set_value(value)` updates text. |
| `Checkbox(label, checked=False, on_change=None, disabled=False)` | Boolean control. `set_checked(checked)` updates state. Supports `Checkbox::row`, `box`, `indicator`, and `label`. |
| `Dropdown(items, value=None, on_change=None, disabled=False)` | Select control. `items` must be non-empty; `value` must be in `items` when supplied. `set_value(value)` updates selection. Supports field, chevron, menu, and item parts. |
| `SelectableList(items, selection_mode="single", max_height=None, width=None, min_width=0, max_width=None, grow=False, shrink=True)` | Content-sized selection list built from generated rows. `max_height` enables bounded vertical scrolling; outer bounds and growth no longer require styling child rows. |
| `RadioGroup(items, orientation="vertical", width=None, min_width=0, max_width=None, grow=False, shrink=True)` | Content-sized group of radio buttons. Horizontal groups wrap under width pressure instead of allowing fixed radio rows to escape the composite. |

## Numeric Controls

| Widget | Features |
| --- | --- |
| `Slider(value=0, min=0, max=1, step=0.01, on_change=None, disabled=False)` | Numeric slider. Clamps initial and live values to `[min, max]`; `step` must be greater than zero. Supports `Slider::track`, `fill`, and `thumb`. |
| `NumberInput(value=0, min=None, max=None, step=1, on_change=None, disabled=False)` | Numeric text field with left decrement and right increment steppers. Values must be finite; `step` must be finite and greater than zero. Supports `NumberInput::field`, `stepper`, `stepper-up`, `stepper-down`, `stepper-divider`, `divider`, and `caret`. |
| `DragVector(value, component_width=88, width=None, min_width=0, max_width=None, grow=False, shrink=True)` | Wrapping vector editor built from fixed component groups and `DragNumber` fields. Its outer composite is content-sized and shrink-safe by default; set `grow=True` to consume remaining parent space. |
| `ProgressBar(value=0, min=0, max=1, label=None, show_value=False, disabled=False)` | Read-only progress display. `set_value(value)` updates progress and, when `show_value=True`, updates the generated percentage label. Supports `ProgressBar::track`, `fill`, and `label`. |
| `LimitsBar(value=0, min=0, max=100, red_low=None, yellow_low=None, yellow_high=None, red_high=None, disabled=False)` | Read-only telemetry limit display with red/yellow zones on both ends and green in the center. Omitted thresholds default to 10%, 25%, 75%, and 90% of the domain. `set_value(value)` preserves out-of-range telemetry while pegging the marker visually; `set_limits(...)` retains omitted arguments and validates the resulting ordered thresholds. Supports `track`, five zone parts, and `indicator`; see `examples/css_feature_probes/limits_bar_probe.py` for eight CSS treatments. |
| `ColorPicker(value=(255, 100, 0), alpha=True, on_change=None, title="Color", width=320, min_width=180, max_width=None, grow=False, shrink=True)` | Composite panel made from a swatch, zero-basis flexible sliders, and fixed-width labels. Accepts RGB/RGBA values as 0-255 integers or normalized 0.0-1.0 floats. `set_value(value, notify=False)` updates the displayed color; pass `notify=True` to invoke `on_change`. |
| `PropertyGrid(values=None, schema=None, label_width=140, width=None, min_width=0, max_width=None, grow=False, shrink=True)` | Schema-driven property editor. It fills available cross-axis width, remains content-sized on its parent's main axis, and keeps editor slots shrink-safe. Explicit bounds or `grow=True` control outer allocation without styling internal rows. |
| `Property(label, editor=None, label_width=None, width=None, min_width=0, max_width=None, grow=False, shrink=True)` | Hand-authored property row with a fixed label and zero-basis flexible editor slot. It fills available cross-axis width while remaining content-sized on the parent main axis. |

## Media And Data

| Widget | Features |
| --- | --- |
| `Image(path, fit="contain", width=None, height=None)` | Image widget. `fit` may be `contain`, `cover`, or `stretch`; width/height must be positive when supplied. Supports `set_path()`, `reload()`, and `set_fit()`. |
| `Histogram(data, value=None, bins=30, bin_edges=None, range=None, mode="count", cumulative=False)` | Static single-series histogram. Filters non-finite values, supports integer bins or explicit bin edges, and serializes pre-binned edges/counts for native rendering. Modes are `count`, `density`, `probability`, and `percent`. Supports axis/tick labels and optional toolbar buttons for fit, pan, wheel zoom, box zoom, grid, and axes. |
| `PieChart(data=None, labels=None, values=None, category=None, value=None, aggregate="count", donut=False)` | Categorical pie/donut chart. Accepts direct labels/values or frame-backed category aggregation. Supports top-N grouping, custom colors, legend placement, slice labels, donut center text, and optional chart toolbar chrome. Use `set_data()`, `set_frame_data()`, and presentation setters such as `set_donut()`, `set_center_text()`, `set_toolbar_visible()`, and `set_legend_position()` for live updates. Supports `PieChart::label`. |
| `Scatter3D(frame, x, y, z, colormap="viridis", rendering="exact", on_pick=None)` | GPU scatter plot. Compact 2D sources support `rendering="decimated"`, which deterministically keeps one authored point near each 256×256 cell center plus missing extrema, and `rendering="density"`, a cached count-weighted centroid grid. A 128×128 exact-row candidate index is deferred until settled viewport refinement. After a 150 ms debounce, visible-window work runs in the background; only the latest camera/source revision is uploaded and superseded requests are coalesced. `adaptive` uses density from 300k visible rows and exact visible points at or below 200k, with hysteresis between; returning to full bounds reuses cached density immediately. Explicit density and decimated modes remain fixed. The density cache is bounded to 16 entries and 64 MiB. Unsupported 3D derived requests fall back observably to exact rendering. Emits `ScatterPick` callbacks for point clicks. Supports `set_points(..., fit=True)` when replacing the scene, `create_live_frame(mode="primary")` for retained full-frame sensor replacement, `live.replace_prepared(...)` for GUI-callback prepared frames, `live.enqueue_prepared(...)` for producer-thread latest-frame streams, plus `set_rendering()`, `set_colormap()`, `set_auto_point_size()`, `set_lod()`, `set_interactive_render_scale()`, `set_auto_quality()`, `show_grid()`, and `set_grid_options(sticky=True, all_edges=False)` for live updates. |
| `DataFrameTable(frame, page_size=100, sample_rows=DEFAULT_TABLE_SAMPLE_ROWS, on_select=None)` | Virtualized table for dataframe-like objects. Extracts metadata, cell samples, and optional column buffers. Emits selection callbacks with `TableSelection` from mouse or keyboard selection. Supports `set_frame()`. Supports `DataFrameTable::header`, `row`, `row-selected`, and `grid-line`. |

`Scatter3D` and `ScatterPlot2D` also accept `source_retention="current"`
(default) or `"none"`. The latter releases native exact-source bytes after a
bounded 2D density/decimated product is ready; use `set_source_retention()` to
change the policy live. Exact and fallback representations keep their source.

Supported `Scatter3D` colormaps:

PointStore-backed full-view decimation and settled viewport products are reused
when the source revision, exact view bounds, requested policy, adaptive state,
and grid match. Density colormap changes reuse cached geometry/count intensity
and apply the active colormap in the point shader. This derived cache uses
viewport-aware density grids at one cell per two physical pixels, clamped to
32..256; equivalent viewport sizes share the same effective-grid key. CPU
products do not currently vary by device tier, so device identity is not part
of that key. The cache uses
least-recently-used eviction, is bounded
to 32 entries and 64 MiB, and is
observable through debug-snapshot hit, miss, recency-update, eviction, entry,
policy, and byte counters.
CPU geometry is shared. Byte-identical decimation and presentation-neutral
density products also share one immutable GPU buffer across plots. Each density
widget keeps its own shader colormap uniform, so different presentation styles
do not duplicate geometry. Weak cache ownership lets unused buffers release
without invalidating active widgets.
For live PointStore replacement, coalesced updates are revision-aware rather
than arrival-order-only. The command queue retains the highest pending revision
for each widget/store projection, preserves packed-registration order ahead of
dependent references, and the runtime drops delayed revisions older than its
accepted watermark. Advancing the watermark invalidates only obsolete products
for that projection; active widgets retain safe strong references.

```text
viridis, plasma, inferno, magma, coolwarm, hot, gray, grey,
turbo, cividis, blues, greens, reds
```

`DataFrameTable.on_select` receives a `TableSelection` object by default:

```python
def selected(selection: dg.TableSelection) -> None:
    print(selection.row_index, selection.column, selection.value)
```

For convenience, callbacks that declare three positional parameters receive
`row_index`, `column`, and `value`. Four-argument callbacks receive
`row_index`, `column_index`, `column`, and `value`.

When focused, table keyboard navigation uses Arrow keys for cell movement,
PageUp/PageDown for visible-page movement, Home/End for row edges, and
Enter/Space to re-emit the current selection.

`Scatter3D.on_pick` receives a `ScatterPick` object by default:

```python
def picked(point: dg.ScatterPick) -> None:
    print(point.index, point.x, point.y, point.z)
```

Callbacks that declare four positional parameters receive `index`, `x`, `y`,
and `z`.

## CSS Part Catalog

These widgets expose named renderer parts for CSS selectors and inline
`style={"parts": ...}` dictionaries.

<!-- BEGIN GENERATED WIDGET CSS CAPABILITIES -->

_Generated from `python/dragongui/widget_css_capabilities.json`. Do not edit this table manually._

Global generated-content hooks: `::before`, `::after` (text renderer).

| Widget | Supported states | Parts and renderer support |
| --- | --- | --- |
| `AccelerationBars` | `:hover`, `:active`, `:focus`, `:disabled` |  |
| `ArrowButton` | `:hover`, `:active`, `:focus`, `:disabled` | `icon` (paint) |
| `BarChart` | `:hover`, `:active`, `:focus`, `:disabled` | `label` (text), `value-label` (text) |
| `Button` | `:hover`, `:active`, `:focus`, `:disabled` | `badge` (paint) |
| `Checkbox` | `:hover`, `:active`, `:focus`, `:disabled`, `:checked` | `box` (paint), `indicator` (paint), `row` (paint), `label` (text) |
| `CodeEditor` | `:hover`, `:active`, `:focus`, `:disabled` | `caret` (paint), `field` (paint), `gutter` (paint), `line-number` (text) |
| `Collapsible` | `:hover`, `:active`, `:focus`, `:disabled`, `:open`, `:expanded`, `:collapsed` | `header` (paint), `indicator` (paint), `scrollbar-thumb` (paint), `scrollbar-track` (paint), `body` (structural) |
| `ContextMenu` | `:hover`, `:active`, `:focus`, `:disabled`, `:open`, `:expanded`, `:collapsed` | `item` (paint), `item-disabled` (paint), `item-hover` (paint), `menu` (paint) |
| `DataFrameTable` | `:hover`, `:active`, `:focus`, `:disabled`, `:selected` | `grid-line` (paint), `header` (paint), `row` (paint), `row-selected` (paint), `scrollbar-thumb` (paint), `scrollbar-track` (paint) |
| `DragNumber` | `:hover`, `:active`, `:focus`, `:disabled` | `field` (paint), `grip` (paint), `value` (text) |
| `Dropdown` | `:hover`, `:active`, `:focus`, `:disabled`, `:open`, `:expanded`, `:collapsed` | `chevron` (paint), `field` (paint), `item` (paint), `item-hover` (paint), `item-selected` (paint), `menu` (paint) |
| `HLayout` | `:hover`, `:active`, `:focus`, `:disabled` | `scrollbar-thumb` (paint), `scrollbar-track` (paint) |
| `Heatmap` | `:hover`, `:active`, `:focus`, `:disabled` | `cell` (paint), `grid` (paint), `hover` (paint), `scalar-bar` (paint), `label` (text) |
| `IconButton` | `:hover`, `:active`, `:focus`, `:disabled` | `icon` (paint) |
| `LED` | `:hover`, `:active`, `:focus`, `:disabled` | `dot` (paint), `glow` (paint), `highlight` (paint) |
| `LimitsBar` | `:hover`, `:active`, `:focus`, `:disabled` | `green` (paint), `indicator` (paint), `red-high` (paint), `red-low` (paint), `track` (paint), `yellow-high` (paint), `yellow-low` (paint) |
| `LoadingSpinner` | `:hover`, `:active`, `:focus`, `:disabled` | `arc` (paint), `track` (paint), `label` (text) |
| `LogView` | `:hover`, `:active`, `:focus`, `:disabled` | `debug` (text), `error` (text), `info` (text), `line` (text), `warning` (text) |
| `Menu` | `:hover`, `:active`, `:focus`, `:disabled`, `:open`, `:expanded`, `:collapsed` | `item` (paint), `item-disabled` (paint), `item-hover` (paint), `menu` (paint) |
| `Modal` | `:hover`, `:active`, `:focus`, `:disabled`, `:open`, `:expanded`, `:collapsed` | `body` (paint), `header` (paint), `scrim` (paint), `scrollbar-thumb` (paint), `scrollbar-track` (paint), `title` (text) |
| `NavItem` | `:hover`, `:active`, `:focus`, `:disabled`, `:selected` | `accent` (paint), `badge` (paint), `item` (paint) |
| `NumberInput` | `:hover`, `:active`, `:focus`, `:disabled` | `caret` (paint), `divider` (paint), `field` (paint), `stepper` (paint), `stepper-divider` (paint), `stepper-down` (paint), `stepper-up` (paint) |
| `Page` | `:hover`, `:active`, `:focus`, `:disabled` | `scrollbar-thumb` (paint), `scrollbar-track` (paint) |
| `Pages` | `:hover`, `:active`, `:focus`, `:disabled` | `scrollbar-thumb` (paint), `scrollbar-track` (paint) |
| `Pane` | `:hover`, `:active`, `:focus`, `:disabled` | `pane` (structural) |
| `Panel` | `:hover`, `:active`, `:focus`, `:disabled` | `accent` (paint), `body` (paint), `header` (paint), `scrollbar-thumb` (paint), `scrollbar-track` (paint), `title` (text) |
| `PieChart` | `:hover`, `:active`, `:focus`, `:disabled` | `label` (text) |
| `ProgressBar` | `:hover`, `:active`, `:focus`, `:disabled` | `fill` (paint), `track` (paint), `label` (text) |
| `RadioButton` | `:hover`, `:active`, `:focus`, `:disabled`, `:checked` | `dot` (paint), `indicator` (paint), `label` (text) |
| `RangeHistogram` | `:hover`, `:active`, `:focus`, `:disabled` |  |
| `RangeSlider` | `:hover`, `:active`, `:focus`, `:disabled` | `range` (paint), `thumb-max` (paint), `thumb-min` (paint), `track` (paint), `label` (text) |
| `ScrollArea` | `:hover`, `:active`, `:focus`, `:disabled` | `scrollbar-thumb` (paint), `scrollbar-track` (paint) |
| `SearchBox` | `:hover`, `:active`, `:focus`, `:disabled` | `clear` (forwarded), `field` (forwarded), `icon` (forwarded) |
| `Selectable` | `:hover`, `:active`, `:focus`, `:disabled`, `:selected` | `indicator` (paint), `row` (paint), `label` (text) |
| `Sidebar` | `:hover`, `:active`, `:focus`, `:disabled` | `body` (paint), `header` (paint), `scrollbar-thumb` (paint), `scrollbar-track` (paint), `title` (text) |
| `Slider` | `:hover`, `:active`, `:focus`, `:disabled` | `fill` (paint), `thumb` (paint), `track` (paint) |
| `SmallButton` | `:hover`, `:active`, `:focus`, `:disabled` | `badge` (paint) |
| `Splitter` | `:hover`, `:active`, `:focus`, `:disabled` | `gutter` (paint) |
| `Tab` | `:hover`, `:active`, `:focus`, `:disabled`, `:selected` | `accent` (paint), `badge` (paint), `tab` (paint) |
| `Tabs` | `:hover`, `:active`, `:focus`, `:disabled` | `header` (paint) |
| `ToggleSwitch` | `:hover`, `:active`, `:focus`, `:disabled`, `:checked` | `row` (paint), `thumb` (paint), `track` (paint), `label` (text) |
| `TreeNode` | `:hover`, `:active`, `:focus`, `:disabled`, `:open`, `:expanded`, `:collapsed` | `guide` (paint), `indicator` (paint), `row` (paint), `label` (text) |
| `VLayout` | `:hover`, `:active`, `:focus`, `:disabled` | `scrollbar-thumb` (paint), `scrollbar-track` (paint) |
| `Window` | `:hover`, `:active`, `:focus`, `:disabled` | `close` (forwarded), `maximize` (forwarded), `minimize` (forwarded), `title` (forwarded), `titlebar` (forwarded), `resize-border` (structural) |

<!-- END GENERATED WIDGET CSS CAPABILITIES -->

Runtime `Toast` overlays and simple string `tooltip="..."` overlays do not have
parts, but they can be styled with `Toast`, `Toast.error`, `Tooltip`, and
`Tooltip.static` type/class selectors.

Part names in inline styles accept dashed or snake-case spellings:

```python
dg.NumberInput(
    42,
    style={
        "parts": {
            "stepper_up": {"background": "surface_alt"},
            "stepper-down": {"background": "danger"},
        }
    },
)
```

Unsupported inline part names raise `ValueError` before the document is sent to
native code. Unsupported CSS part selectors warn through the stylesheet warning
and debug snapshot path.

## Events And Live Updates

| Widget | Callback | Live Setter |
| --- | --- | --- |
| `Button` | `on_click()` | `click()` invokes the Python callback directly. |
| `MenuItem` | `on_click()` | Activated by native menu interaction. |
| `Tabs` | `on_change(value: str)` | Native tab selection updates the route value. `set_value(value, notify=False)` is silent unless `notify=True`. |
| `Pages` | `on_change(value: str)` | Fires when native interaction changes the active page route. `set_value(value, notify=False)` is silent unless `notify=True`. |
| `TextInput` | `on_change(value: str)` | `set_value(value)` updates the live native value. |
| `TextArea` | `on_change(value: str)` | `set_value(value)` updates the live native value. |
| `Slider` | `on_change(value: float)` | `set_value(value)` clamps and updates the native value. |
| `NumberInput` | `on_change(value: float)` | `set_value(value)` validates, clamps, and updates the native value. |
| `Dropdown` | `on_change(value: str)` | `set_value(value)` validates and updates selection. |
| `Checkbox` | `on_change(value: bool)` | `set_checked(value)` updates checked state. |
| `Collapsible` | `on_change(value: bool)` | `set_expanded(value)`, `expand()`, `collapse()`, `toggle()`. |
| `Button`, `Tab`, `NavItem` badges | None | `set_badge(value)` updates or hides the live badge. |
| `Badge`, `Tag` | None | `set_value(value)` updates text; `set_level(level)` updates semantic color. |
| `LED` | None | `set_state(value)`, `set_on(value)`, `set_color(color)`, and `set_size(size)` update the live indicator. |
| `ColorPicker` | `on_change(value: tuple[int, ...])` | `set_value(value, notify=False)` updates silently by default; `notify=True` invokes the callback. |
| `Histogram` | None | Static first slice; live data updates are planned. |
| `Scatter3D` | `on_pick(point: ScatterPick)` or compatible index/x/y/z callback | `set_points(...)`, `set_colormap(...)`. |
| `DataFrameTable` | `on_select(selection: TableSelection)` or compatible row/column/value callback | `set_frame(...)`. |
| `Image` | None | `set_path(...)`, `reload()`, `set_fit(...)`. |
| `Modal` | None | `show()`, `close()`; `set_open(...)` is a compatibility alias. |
| `toast` / `App.toast` | None | `ToastHandle.update(...)`, `ToastHandle.dismiss()`. Toast updates can also set `opacity`, `radius`, `padding`, and `position`. |
