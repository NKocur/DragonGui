# DragonGUI Widgets

This document catalogs the public DragonGUI Python widgets and the main
features each one exposes. It complements `docs/css-styling.md`, which covers
the CSS property and selector model in more detail.

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
| `HLayout` | Horizontal flex container. Useful for rows, split panes, and button groups. |
| `VLayout` | Vertical flex container. Useful for forms, panels, and stacked sections. |
| `Panel(title=None, width=None)` | Titled or untitled container with optional preferred width. Vertically scrolls overflowing child content with the mouse wheel. Supports `Panel::accent`. |
| `Collapsible(title, expanded=True, on_change=None, disabled=False)` | Header/body container for optional sections. Header toggles expanded state; children lay out only when expanded. Supports `set_expanded()`, `expand()`, `collapse()`, `toggle()`, and `Collapsible::header`, `indicator`, `body`. |
| `Sidebar(title=None, width=220)` | Fixed-width navigation/rail container. |
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
| `Tabs(value=None, on_change=None, disabled=False)` | Container for `Tab` children. If `value` is omitted, the first tab becomes active. `set_value(value, notify=False)` switches tabs programmatically; pass `notify=True` to invoke `on_change`. Emits `change` for native interaction when `on_change` is supplied and not disabled. Supports `Tabs::header`. |
| `Tab(label, value=None, badge=None, disabled=False)` | Tab page container. `value` defaults to a route-safe version of `label`. Must be created directly inside `Tabs`. Optional badge supports `set_badge(value)`. Supports `Tab::tab`, `accent`, and `badge`. |
| `Pages(value=None, on_change=None)` | Container for route-based `Page` children. If `value` is omitted, the first page becomes active. `set_value(value, notify=False)` switches pages programmatically; pass `notify=True` to invoke `on_change`. Native page-route changes emit `change` when `on_change` is supplied; programmatic construction does not call it. |
| `Page(value, title=None)` | Page container. Must be created directly inside `Pages`. |
| `NavItem(label, page, badge=None, disabled=False)` | Navigation row targeting a `Page` route. Optional badge supports `set_badge(value)`. Supports `NavItem::item`, `accent`, and `badge`. |

## Text And Basic Controls

| Widget | Features |
| --- | --- |
| `Label(text)` | Static text. `set_value(value)` updates text. |
| `Badge(text, level="info")` | Compact status/count pill. `level` may be `neutral`, `info`, `success`, `warning`, `danger`, or `error`. Supports `set_value(value)` and `set_level(level)`. |
| `Tag(text, level="neutral")` | Compact bordered status label with the same API as `Badge`. |
| `Button(text, on_click=None, badge=None, disabled=False)` | Clickable button. Emits `click` when `on_click` is supplied and not disabled. `click()` invokes the Python callback directly. Optional badge supports `set_badge(value)`. |
| `TextInput(value="", placeholder="", on_change=None, disabled=False)` | Editable text field. `set_value(value)` updates text. |
| `TextArea(value="", placeholder="", rows=4, wrap=True, on_change=None, disabled=False)` | Editable multiline text field. Newlines are preserved; `rows` controls preferred height; overflow scrolls inside the field; `wrap` controls long-line wrapping. `set_value(value)` updates text. |
| `Checkbox(label, checked=False, on_change=None, disabled=False)` | Boolean control. `set_checked(checked)` updates state. Supports `Checkbox::row`, `box`, `indicator`, and `label`. |
| `Dropdown(items, value=None, on_change=None, disabled=False)` | Select control. `items` must be non-empty; `value` must be in `items` when supplied. `set_value(value)` updates selection. Supports field, chevron, menu, and item parts. |

## Numeric Controls

| Widget | Features |
| --- | --- |
| `Slider(value=0, min=0, max=1, step=0.01, on_change=None, disabled=False)` | Numeric slider. Clamps initial and live values to `[min, max]`; `step` must be greater than zero. Supports `Slider::track`, `fill`, and `thumb`. |
| `NumberInput(value=0, min=None, max=None, step=1, on_change=None, disabled=False)` | Numeric text field with left decrement and right increment steppers. Values must be finite; `step` must be finite and greater than zero. Supports `NumberInput::field`, `stepper`, `stepper-up`, `stepper-down`, `stepper-divider`, `divider`, and `caret`. |
| `ProgressBar(value=0, min=0, max=1, label=None, show_value=False, disabled=False)` | Read-only progress display. `set_value(value)` updates progress and, when `show_value=True`, updates the generated percentage label. Supports `ProgressBar::track`, `fill`, and `label`. |
| `ColorPicker(value=(255, 100, 0), alpha=True, on_change=None, title="Color", width=320)` | Composite panel made from a swatch, sliders, and labels. Accepts RGB/RGBA values as 0-255 integers or normalized 0.0-1.0 floats. `set_value(value, notify=False)` updates the displayed color; pass `notify=True` to invoke `on_change`. User interaction always invokes `on_change`. |

## Media And Data

| Widget | Features |
| --- | --- |
| `Image(path, fit="contain", width=None, height=None)` | Image widget. `fit` may be `contain`, `cover`, or `stretch`; width/height must be positive when supplied. Supports `set_path()`, `reload()`, and `set_fit()`. |
| `Scatter3D(frame, x, y, z, colormap="viridis", on_pick=None)` | GPU 3D scatter plot. Uses frame metadata and packed float32 xyz data when NumPy/addressable columns are available. Emits `ScatterPick` callbacks for point clicks. Supports `set_points()` and `set_colormap()` for live updates. |
| `DataFrameTable(frame, page_size=100, sample_rows=DEFAULT_TABLE_SAMPLE_ROWS, on_select=None)` | Virtualized table for dataframe-like objects. Extracts metadata, cell samples, and optional column buffers. Emits selection callbacks with `TableSelection` from mouse or keyboard selection. Supports `set_frame()`. Supports `DataFrameTable::header`, `row`, `row-selected`, and `grid-line`. |

Supported `Scatter3D` colormaps:

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

| Widget | Parts |
| --- | --- |
| `Panel` | `accent` |
| `Collapsible` | `header`, `indicator`, `body` |
| `Button` | `badge` |
| `NumberInput` | `field`, `stepper`, `stepper-up`, `stepper-down`, `stepper-divider`, `divider`, `caret` |
| `Dropdown` | `field`, `chevron`, `menu`, `item`, `item-selected`, `item-hover` |
| `Checkbox` | `row`, `box`, `indicator`, `label` |
| `Slider` | `track`, `fill`, `thumb` |
| `ProgressBar` | `track`, `fill`, `label` |
| `Tabs` | `header` |
| `Tab` | `tab`, `accent`, `badge` |
| `NavItem` | `item`, `accent`, `badge` |
| `DataFrameTable` | `header`, `row`, `row-selected`, `grid-line` |

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
| `ColorPicker` | `on_change(value: tuple[int, ...])` | `set_value(value, notify=False)` updates silently by default; `notify=True` invokes the callback. |
| `Scatter3D` | `on_pick(point: ScatterPick)` or compatible index/x/y/z callback | `set_points(...)`, `set_colormap(...)`. |
| `DataFrameTable` | `on_select(selection: TableSelection)` or compatible row/column/value callback | `set_frame(...)`. |
| `Image` | None | `set_path(...)`, `reload()`, `set_fit(...)`. |
| `Modal` | None | `show()`, `close()`; `set_open(...)` is a compatibility alias. |
| `toast` / `App.toast` | None | `ToastHandle.update(...)`, `ToastHandle.dismiss()`. Toast updates can also set `opacity`, `radius`, `padding`, and `position`. |
