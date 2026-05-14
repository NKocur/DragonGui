# DragonGUI Widget Reference

This document inventories the current DragonGUI Python API, widget options, live
update methods, callbacks, and CSS styling hooks.

It is intended as an audit reference. It describes what is currently exposed by
the implementation, not a future roadmap.

## Shared Widget Options

Most widgets accept these common keyword arguments:

| Option | Type | Notes |
| --- | --- | --- |
| `id` | `str | None` | Explicit widget id. If omitted, DragonGUI generates one. CSS `#id` selectors target this value. |
| `key` | `str | None` | Stable app-level key. CSS `[key="..."]` selectors target this value. |
| `class_` | `str | None` | CSS class string. Whitespace-separated classes are supported, such as `"primary danger"`. |
| `style` | `Mapping[str, object] | None` | Inline style dictionary. Overrides stylesheet rules. |
| `tooltip` | `str | None` | Simple hover tooltip text. Styled through the virtual `Tooltip` / `Tooltip.static` CSS selectors. |
| `parent` | `Container | None` | Explicit parent. If omitted, the active build context is used. |

`Window` is the exception: it does not accept `tooltip` or `parent`.
`Tooltip` is also special: it is a rich tooltip container and does not accept a
simple `tooltip` option for itself.

All widgets expose:

| Method | Notes |
| --- | --- |
| `set_style(style)` | Replaces the inline style and sends a live style patch if the widget is running. |
| `to_dict()` | Serializes the widget for the native runtime. |
| `to_vnode()` | Converts the widget into a virtual DOM node. |
| `is_live` | True when the widget is bound to a running native app. |

All containers also expose:

| Method | Notes |
| --- | --- |
| `add(child)` | Reparents and appends a child widget. |
| `replace_children(children)` | Live-safe child replacement. Rebinds callbacks and startup resources. |
| context manager | `with dg.Panel(): ...` pushes the container as the active parent. |

## Shared CSS System

### Loading CSS

CSS is loaded through `App`:

```python
app.stylesheet("Button { border-radius: 4px; }")
app.load_stylesheet("styles/app.dg.css")
app.clear_stylesheets()
```

Styles can be loaded before `app.run(...)` or while the app is live.

### Supported Selectors

| Selector | Example | Notes |
| --- | --- | --- |
| Universal | `*`, `Panel > *` | Matches any DragonGUI widget in the current selector position. |
| Type | `Button` | Uses DragonGUI widget type names. |
| Class | `.primary` | Matches `class_="primary"` or whitespace-split class strings. |
| Type + class | `Button.primary.danger` | Multiple class selectors are supported. |
| ID | `#run-button` | Matches explicit widget `id`. |
| Key attribute | `[key="primary-action"]` | Matches explicit widget `key`. |
| Attribute | `[disabled]`, `[level="info"]`, `[text="run" i]` | Matches stable widget metadata and parsed scalar props. |
| Descendant | `Panel Button` | Matches any ancestor relationship. |
| Direct child | `Panel.controls > Button` | Checks immediate parent. |
| Child chain | `Window > Panel > HLayout > Button` | Multi-level direct-child chains are supported. |
| Pseudo-state | `Button:hover` | Supported pseudo-states are listed below. |
| Structural child | `Button:first-child`, `Button:nth-child(3n+1)`, `*:nth-child(2 of Button.primary)`, `*:nth-child(1 of Button:first-child)`, `*:nth-child(1 of Checkbox:checked)` | Matches by sibling position, optionally filtered by a compound selector list or supported selector chain. |
| Selector function | `Button:not(.ghost)`, `Panel:has(HLayout > Badge)`, `Panel:has(> Button:first-child)`, `Panel:has(+ Button.primary)`, `Panel:has(> Checkbox:checked)`, `Panel:has(Panel:has(Button.primary))`, `Panel:has(Panel:has(> Badge) > Button.primary)` | `:not(...)`, `:is(...)`, and `:where(...)` support compound selector lists; `:has(...)` supports descendant selector chains, direct child arguments with leading `>`, following sibling arguments with leading `+` or `~`, data-backed target state pseudos, nested `:has(...)` on the argument target, and nested `:has(...)` on ancestor-side compounds in descendant or direct-child argument chains. |
| Widget part | `NumberInput::stepper` | Part hooks style renderer-owned sub-elements. |

Unsupported selector forms warn and are ignored: browser pseudo-elements and
ancestor pseudo-states inside selector functions such as
`Panel:is(:hover) Button`, plus broader `:has(...)` dynamic stateful and
widget-part arguments.
`:nth-child(...)` supports integer indexes, `odd`, `even`, `an+b` formulas
such as `3n+1` and `-n+3`, and `of <selector-list>` filters for compound
selector lists, supported selector chains, structural target filters, and
data-backed target state filters.

### Supported Pseudo-States

- `:hover`
- `:active`
- `:focus`
- `:disabled`
- `:checked`
- `:open`
- `:expanded`
- `:collapsed`
- `:selected`

`:checked` is meaningful for `Checkbox` and can also style checkbox parts.
`:open` is meaningful for dropdowns, menus, context menus, and open modals.
`:expanded` / `:collapsed` are meaningful for `Collapsible`.
`:selected` is meaningful for active tabs, nav items, pages, and selected
tables.

### Supported CSS Properties

Layout properties:

- `display`
- `flex-direction`
- `flex`
- `flex-grow`
- `flex-shrink`
- `width`
- `height`
- `min-width`
- `min-height`
- `max-width`
- `max-height`
- `padding`
- `padding-left`
- `padding-right`
- `padding-top`
- `padding-bottom`
- `margin`
- `gap`

Visual properties:

- `background` / `background-color` (`background` also accepts
  `linear-gradient(...)`, `radial-gradient(...)`, and comma-separated paint
  layers)
- `background-noise`
- `foreground`
- `border-color`
- `border-width`
- `border-style: solid`, `border-style: none`, or `border-style: hidden`
- `border-radius`
- `border-top-left-radius`
- `border-top-right-radius`
- `border-bottom-right-radius`
- `border-bottom-left-radius`
- `border: none`, `border: 0`, or `border: <width> solid <color>`
- `box-shadow`
- `opacity`
- `accent`
- `track-color`
- `thumb-color`

Text properties:

- `color`
- `font-size`
- `font-family`
- `font-weight`
- `text-align`
- `text-transform`
- `letter-spacing`
- `line-height`
- `font-style`
- `font-variant-numeric`
- `text-overflow`

Widget-specific properties:

- `text-area-rows`
- `scatter-point-size`
- `table-row-height`
- `table-header-height`
- `table-column-width`
- `table-index-width`

Text properties inherit down the widget tree. Layout and visual properties do
not inherit.

### CSS Values

Supported color values:

- Theme tokens such as `background`, `surface`, `surface_alt`, `text`,
  `muted_text`, `accent`, `border`, `danger`, `warning`, `success`, `focus`,
  and `disabled`.
- Derived tokens such as `accent_mix_20` and `accent_dark`.
- `#RGB`
- `#RGBA`
- `#RRGGBB`
- `#RRGGBBAA`
- `transparent`, common named colors, `rgb(...)`, `rgba(...)`, `hsl(...)`,
  `hsla(...)`, and `hwb(...)`.
- Global `:root` variables through `var(--name)` and
  `var(--name, fallback)`, including inside larger parseable property values.

Supported background paint values:

- Solid colors through the color syntax above.
- `linear-gradient(...)`, `radial-gradient(...)`, and comma-separated paint
  layers on `background` for rect-backed widget surfaces. The renderer supports
  up to six stop colors per rect instance.
- DragonGUI-specific `blob-gradient(...)` organic background paint with up to
  four soft fields.
- DragonGUI-specific `mesh-gradient(...)` four-corner background paint for
  smooth image-like gradients.
- Subtle procedural gradient noise through `background-noise`, usually with
  values around `0.01..0.03`.
- Gradient stop interpolation through `gradient-interpolation`, with `srgb`,
  `linear-srgb`, and `oklab` modes.

Supported length values are logical pixels for most properties. Unitless
numbers and `px` values are accepted. Percent lengths, `auto`, and compatible
`calc()` expressions are currently supported for `width`, `height`,
`min-width`, `min-height`, `max-width`, and `max-height`. Mixed percent/pixel
`calc()` expressions resolve when the parent axis has a definite size.

First-slice CSS Grid is available through CSS with `display: grid`,
`grid-template-columns`, `grid-template-rows`, `grid-column`, `grid-row`,
`row-gap`, and `column-gap`. Track lists support px, percent, `fr`, `auto`, and
simple `repeat(n, ...)`.

First-slice overflow is available through CSS with `overflow`, `overflow-x`,
and `overflow-y`. `visible` lets children escape container clipping, `hidden`
clips children, and `auto`/`scroll` opt containers into scroll state.
Horizontal scroll uses horizontal wheel input or shift-wheel; scroll containers
draw vertical and horizontal overlay scrollbars for overflowing opted-in axes.
Scrollbar thumbs can be dragged, and clicking a track moves the thumb toward
that position. Scrollable layout/container widgets expose
`::scrollbar-track` and `::scrollbar-thumb` CSS part hooks.

### Inline Part Styles

Inline styles can target widget parts through a nested `parts` dictionary:

```python
dg.NumberInput(
    42,
    style={
        "parts": {
            "stepper": {"width": 34},
            "stepper_up": {"background": "surface_alt"},
        }
    },
)
```

Inline part names accept dashed or snake-case names. For example,
`stepper-up` and `stepper_up` are equivalent.

## App And Runtime APIs

### `App`

Constructor:

```python
dg.App(title="DragonGUI", theme=None, metadata={}, loading_screen=None)
```

Options:

| Option | Notes |
| --- | --- |
| `title` | App document title. |
| `theme` | Optional `dg.Theme`. |
| `metadata` | JSON-safe metadata included in the app document. |
| `loading_screen` | `None`/`True` uses the default native startup loading screen, `False` disables it, and `dg.LoadingScreen(...)` customizes copy, colors, spinner/progress, and minimum duration. |

Methods:

| Method | Notes |
| --- | --- |
| `document(window)` | Builds the JSON-safe app document. |
| `stylesheet(css)` | Adds or applies a user stylesheet. |
| `load_stylesheet(path)` | Reads UTF-8 CSS from disk and applies it. |
| `clear_stylesheets()` | Clears user stylesheets. |
| `run(window)` | Starts the native event loop. Accepts `Window` or component instance. |
| `run_with_loading(build_window, title=None, width=1024, height=768)` | Starts the native event loop with a placeholder window, shows the startup loading frame, then calls `build_window()` and swaps in the returned `Window` or component root before the first real redraw. |
| `call_soon_threadsafe(fn)` | Schedules a callable on the live DragonGUI runtime. |
| `toast(...)` | Shows a native toast while the app is running. |
| `debug_snapshot(timeout_ms=1000)` | Returns live runtime diagnostics. |
| `set_buffer_resource(resource_id, data, kind="bytes", owner=None)` | Uploads a retained native buffer. |
| `release_resource(resource_id)` | Releases a retained native resource. |

CSS:

- `App` is not a widget and has no CSS selector.
- The root widget is usually `Window`.

### `Theme`

Constructor fields:

| Field | Default |
| --- | --- |
| `background` | `#12121a` |
| `surface` | `#1e1e2e` |
| `surface_alt` | `#32324a` |
| `text` | `#f0f0f7` |
| `muted_text` | `#a8a8ba` |
| `accent` | `#7b73ff` |
| `border` | `#383850` |
| `danger` | `#ff5c7a` |
| `warning` | `#ffbf47` |
| `success` | `#43d48f` |
| `focus` | `#6bdcff` |
| `disabled` | `#66667a` |
| `radius` | `6.0` |
| `spacing` | `8.0` |
| `font_size` | `14.0` |

Helpers:

| Method | Notes |
| --- | --- |
| `Theme.dark(**overrides)` | Creates a dark theme with optional overrides. |
| `Theme.light(**overrides)` | Creates a light theme with optional overrides. |
| `to_dict()` | Serializes the theme for the native runtime. |

CSS:

- Theme fields are available as CSS tokens.
- Color fields accept the same literal color strings as CSS and inline styles:
  hex colors, `transparent`, common named colors, `rgb()/rgba()`,
  `hsl()/hsla()`, and `hwb()`, plus `lab()`, `lch()`, `oklab()`, `oklch()`,
  `color(srgb ...)`, and `color(srgb-linear ...)`. Theme fields do not resolve
  other theme tokens or `var(...)`.
- `radius` and `font_size` also feed built-in framework stylesheet variables.

### Toast Notifications

Function:

```python
dg.toast(
    message,
    level="info",
    duration=3000,
    opacity=None,
    radius=None,
    padding=None,
    position=None,
    app=None,
)
```

Equivalent live method:

```python
app.toast(...)
```

Options:

| Option | Notes |
| --- | --- |
| `message` | Non-empty display text. |
| `level` | `info`, `success`, `warning`, or `error`. |
| `duration` | Milliseconds or `None` for persistent. |
| `opacity` | Optional `0.0` to `1.0` surface opacity. |
| `radius` | Optional non-negative corner radius override. |
| `padding` | Optional non-negative padding override. |
| `position` | `top-right`, `top-left`, `bottom-right`, or `bottom-left`. |
| `app` | `App`, `AppHandle`, or `None` for the active app. |

Returned handle:

| Method | Notes |
| --- | --- |
| `update(...)` | Replaces message, level, duration, and optional styling. |
| `dismiss()` | Dismisses the toast. |

CSS:

- Type selector: `Toast`.
- Level classes: `Toast.info`, `Toast.success`, `Toast.warning`, `Toast.error`.
- Toast levels intentionally differ from Badge/Tag levels: Toast does not
  support `neutral` or `danger`; use `error` for destructive/error toasts.
- No widget parts.
- Supported styling includes surface/text visual properties such as
  `background`, `border-color`, `color`, `border-radius`, `opacity`, and
  `padding`.

## Native Dialog APIs

### `FileDialog`

Static methods:

| Method | Return | Notes |
| --- | --- | --- |
| `FileDialog.open_file(...)` | `str | None` | Single file picker. |
| `FileDialog.open_files(...)` | `list[str] | None` | Multi-file picker. |
| `FileDialog.save_file(...)` | `str | None` | Save-file picker. |
| `FileDialog.pick_folder(...)` | `str | None` | Folder picker. |

Convenience functions:

- `dg.open_file_dialog(...)`
- `dg.open_files_dialog(...)`
- `dg.save_file_dialog(...)`
- `dg.pick_folder_dialog(...)`

Common options:

| Option | Notes |
| --- | --- |
| `title` | Optional OS dialog title. |
| `filters` | File filters as `(name, extensions)` tuples, where applicable. |
| `on_select` | Optional callback. If omitted, the call is synchronous. |
| `app` | Optional app used to route async callback through `call_soon_threadsafe`. |

CSS:

- OS-native dialogs are not DragonGUI widgets and are not CSS styled.

## Layout And Structural Widgets

### `Window`

Constructor:

```python
dg.Window(title, width=1024, height=768, id=None, key=None, class_=None, style=None)
```

Options:

| Option | Notes |
| --- | --- |
| `title` | Window title. |
| `width` | Initial width in logical pixels. |
| `height` | Initial height in logical pixels. |

CSS:

- Type selector: `Window`.
- No widget parts.
- Usually used for root layout, background, padding, gap, and text inheritance.

### `HLayout`

Constructor:

```python
dg.HLayout(id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

CSS:

- Type selector: `HLayout`.
- Parts: `scrollbar-track`, `scrollbar-thumb`.
- Typically styled with `gap`, `padding`, `height`, `flex`, and background.

### `VLayout`

Constructor:

```python
dg.VLayout(id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

CSS:

- Type selector: `VLayout`.
- Parts: `scrollbar-track`, `scrollbar-thumb`.
- Typically styled with `gap`, `padding`, `width`, `flex`, and background.

### `ScrollArea`

Constructor:

```python
dg.ScrollArea(axis="y", gap=None, width=None, height=None, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `axis` | `"y"` for vertical scrolling, `"x"` for horizontal scrolling, `"both"` for both axes, or `"none"` for clipped viewport behavior. |
| `gap` | Vertical child gap in logical pixels. |
| `width` / `height` | Optional preferred viewport size in logical pixels. |

`ScrollArea` is the explicit viewport container for content that can exceed a
bounded parent, similar to Qt's `QScrollArea`. It lays out children vertically
by default, clips to its own rectangle, and exposes draggable overlay
scrollbars when content overflows. It does not draw a panel frame by default;
wrap it in `Panel` when you want a titled or bordered surface.

CSS:

- Type selector: `ScrollArea`.
- Parts: `scrollbar-track`, `scrollbar-thumb`.
- Typically styled with `min-height`, `flex`, `gap`, `padding`, and
  `overflow-x` / `overflow-y`. Leave `height` unset when the scroll area should
  share a column with fixed controls; the default flex behavior takes the
  remaining space without covering siblings.

### `Panel`

Constructor:

```python
dg.Panel(title=None, width=None, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `title` | Optional panel title. |
| `width` | Preferred panel width. |

Overflowing panel children scroll vertically with the mouse wheel and
horizontally with horizontal wheel input or shift-wheel. The panel frame, title,
and inset overlay scrollbar stay in place while child widgets are clipped to
the panel. Scrollbars stay inside rounded panel corners, remain centered on the
panel surface, and avoid the bottom-right corner when both axes overflow. The
thumb can be dragged, clicking the track updates the scroll offset, and
PageUp/PageDown/Home/End scroll the nearest scrollable ancestor when keyboard
focus is inside the panel. Shift uses the horizontal axis when horizontal
overflow exists.
`Panel::scrollbar-track` styles the track and uses `width` plus uniform
`padding` for thickness and axis inset; `Panel::scrollbar-thumb` styles the
thumb.

CSS:

- Type selector: `Panel`.
- Parts: `accent`, `scrollbar-track`, `scrollbar-thumb`.
- `Panel::accent` styles the left-side accent fill. `width` controls accent
  thickness and the fill is clipped to the panel shape.
- `Panel::scrollbar-track` styles the built-in scroll track. `width` controls
  thickness and uniform `padding` controls the top/bottom inset for vertical
  scrollbars and the left/right inset for horizontal scrollbars.
- `Panel::scrollbar-thumb` styles the built-in scroll thumb. `width`,
  `background`, `border`, `border-radius`, and `opacity` are supported.

### `Collapsible`

Constructor:

```python
dg.Collapsible(
    title,
    expanded=True,
    on_change=None,
    disabled=False,
    id=None,
    key=None,
    class_=None,
    style=None,
    tooltip=None,
    parent=...,
)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `title` | Header text. |
| `expanded` | Initial expanded state. |
| `on_change` | Called with `bool` when user toggles. |
| `disabled` | Disables user interaction. |

Live methods:

- `set_expanded(expanded)`
- `expand()`
- `collapse()`
- `toggle()`

CSS:

- Type selector: `Collapsible`.
- Parts: `header`, `indicator`, `body`, `scrollbar-track`,
  `scrollbar-thumb`.
- `:disabled`, `:expanded`, and `:collapsed` are supported.

### `Modal`

Constructor:

```python
dg.Modal(
    title="",
    open=False,
    width=420,
    height=220,
    id=None,
    key=None,
    class_=None,
    style=None,
    tooltip=None,
    parent=...,
)
```

Options:

| Option | Notes |
| --- | --- |
| `title` | Modal title. |
| `open` | Initial visibility. |
| `width` | Modal width. |
| `height` | Modal height. |

Live methods:

- `set_open(open)`
- `show()`
- `close()`

CSS:

- Type selector: `Modal`.
- Parts: `scrim`, `scrollbar-track`, `scrollbar-thumb`.
- `:open` is supported while the modal is visible.
- The modal surface uses normal visual/text CSS. `Modal::scrim` styles the
  full-screen overlay behind the modal surface.

### `Separator`

Constructor:

```python
dg.Separator(orientation="auto", id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `orientation` | `auto`, `horizontal`, or `vertical`. |

CSS:

- Type selector: `Separator`.
- No widget parts.
- Use `width`, `height`, `background`, and `border-color` for styling.

### `Spacer`

Constructor:

```python
dg.Spacer(width=None, height=None, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `width` | Optional fixed width. |
| `height` | Optional fixed height. |

CSS:

- Type selector: `Spacer`.
- No widget parts.
- Usually controlled through `flex`, `width`, and `height`.

## Navigation And Menus

### `MenuBar`

Constructor:

```python
dg.MenuBar(height=34, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `height` | Menu bar height. |

Children:

- Only `Menu` children are accepted.

CSS:

- Type selector: `MenuBar`.
- No widget parts.

### `Menu`

Constructor:

```python
dg.Menu(label, disabled=False, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `label` | Top-level menu label. |
| `disabled` | Disables opening the menu. |

Children:

- Only `MenuItem` children are accepted.
- Must be created directly inside `MenuBar`.

CSS:

- Type selector: `Menu`.
- No widget parts.
- `:hover`, `:active`, `:focus`, `:disabled`, and `:open` can style the menu
  label.

### `MenuItem`

Constructor:

```python
dg.MenuItem(label, on_click=None, disabled=False, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `label` | Popup item text. |
| `on_click` | Called when selected. |
| `disabled` | Disables selection. |

CSS:

- Type selector: `MenuItem`.
- No widget parts.
- Menu item popup rendering uses normal visual/text CSS and pseudo-states.

### `ContextMenu`

Constructor:

```python
dg.ContextMenu(target=None, width=220, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `target` | Widget or widget id that opens the context menu. `None` means global. |
| `width` | Popup width. |

When `target=None`, the context menu is global: right-clicking any normal
DragonGUI widget without a more specific context menu target can open it.
DragonGUI handles this inside its own window; it does not replace operating
system context menus outside the app.

Children:

- Only `MenuItem` children are accepted.

CSS:

- Type selector: `ContextMenu`.
- No widget parts.
- `:open` is supported while the context menu popup is open.

### `Tooltip`

Rich tooltip constructor:

```python
dg.Tooltip(target=widget_or_id, width=280, height=None, id=None, key=None, class_=None, style=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `target` | Widget or widget id that owns the tooltip. |
| `width` | Tooltip width. |
| `height` | Optional fixed height. |

CSS:

- Type selector: `Tooltip`.
- Simple string tooltips from `tooltip="..."` also match `Tooltip.static`.
- No widget parts.

### `Tabs`

Constructor:

```python
dg.Tabs(value=None, on_change=None, disabled=False, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `value` | Selected tab value. Defaults to first child tab. |
| `on_change` | Called with selected value on user change. |
| `disabled` | Disables tab switching. |

Children:

- Only `Tab` children are accepted.

Live methods:

- `set_value(value, notify=False)`

CSS:

- Type selector: `Tabs`.
- Parts: `header`.

### `Tab`

Constructor:

```python
dg.Tab(label, value=None, badge=None, disabled=False, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `label` | Visible tab label. |
| `value` | Selection value. Defaults to a route-style value from `label`. |
| `badge` | Optional inline badge text/count. |
| `disabled` | Disables tab selection. |

Live methods:

- `set_badge(value)`

CSS:

- Type selector: `Tab`.
- Parts: `tab`, `accent`, `badge`.
- `:hover`, `:active`, `:focus`, `:disabled`, and `:selected` are supported.

### `Pages`

Constructor:

```python
dg.Pages(value=None, on_change=None, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `value` | Active page value. Defaults to first child page. |
| `on_change` | Called with active page value when navigation changes. |

Children:

- Only `Page` children are accepted.

Live methods:

- `set_value(value, notify=False)`

CSS:

- Type selector: `Pages`.
- Parts: `scrollbar-track`, `scrollbar-thumb`.

### `Page`

Constructor:

```python
dg.Page(value, title=None, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `value` | Required page value. |
| `title` | Optional title metadata. |

CSS:

- Type selector: `Page`.
- Parts: `scrollbar-track`, `scrollbar-thumb`.
- `:selected` is supported for the active page.

### `Sidebar`

Constructor:

```python
dg.Sidebar(title=None, width=220, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `title` | Optional sidebar title. |
| `width` | Sidebar width. |

CSS:

- Type selector: `Sidebar`.
- Parts: `scrollbar-track`, `scrollbar-thumb`.

### `NavItem`

Constructor:

```python
dg.NavItem(label, page=..., badge=None, disabled=False, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `label` | Visible navigation label. |
| `page` | Target `Pages` value. |
| `badge` | Optional inline badge text/count. |
| `disabled` | Disables navigation. |

Live methods:

- `set_badge(value)`

CSS:

- Type selector: `NavItem`.
- Parts: `item`, `accent`, `badge`.
- `:selected` is supported when the target page is active.

### `StatusBar`

Constructor:

```python
dg.StatusBar(height=28, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `height` | Status bar height. |

CSS:

- Type selector: `StatusBar`.
- No widget parts.

## Text, Labels, And Indicators

### `Label`

Constructor:

```python
dg.Label(text, id=None, key=None, class_=None, style=None, tooltip=None, wrap=True, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `text` | Display text. |
| `wrap` | When true, labels wrap inside constrained containers and reserve approximate multiline height. Pass false for a single-line label. |

Live methods:

- `set_value(value)`

CSS:

- Type selector: `Label`.
- No widget parts.

### `Badge`

Constructor:

```python
dg.Badge(text, level="info", id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `text` | Display text/count. Cannot be `None`. |
| `level` | `neutral`, `info`, `success`, `warning`, `danger`, or `error`. |

Live methods:

- `set_value(value)`
- `set_level(level)`

CSS:

- Type selector: `Badge`.
- No widget parts.
- The current `level` is exposed as an automatic CSS class, so selectors such
  as `Badge.info`, `Badge.success`, and `Badge.error` work.

### `Tag`

Constructor:

```python
dg.Tag(text, level="neutral", id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `text` | Display text/count. Cannot be `None`. |
| `level` | `neutral`, `info`, `success`, `warning`, `danger`, or `error`. Defaults to `neutral`. |

Live methods:

- Inherits `set_value(value)` and `set_level(level)` from `Badge`.

CSS:

- Type selector: `Tag`.
- No widget parts.
- The current `level` is exposed as an automatic CSS class, so selectors such
  as `Tag.neutral`, `Tag.warning`, and `Tag.danger` work.

### `LED`

Constructor:

```python
dg.LED(state=False, states=None, on_color="success", off_color="disabled", size=14, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `state` | `bool` or string state name. `True` maps to `on`; `False` maps to `off`. |
| `states` | Optional mapping of string state names to colors, for example `{"busy": "warning", "ready": "#2dd36f"}`. |
| `on_color` / `off_color` | Default colors for boolean state use. Accepts theme tokens, CSS color strings, or RGB/RGBA sequences. |
| `size` | Indicator diameter in logical pixels. |

Live methods:

- `set_state(state, color=None)`
- `set_on(on=True)`
- `set_color(color)`
- `set_size(size)`

CSS:

- Type selector: `LED`.
- Parts: `LED::dot`, `LED::glow`, `LED::highlight`.
- The current state is exposed as the `state` attribute and as an automatic CSS
  class, so selectors such as `LED.on`, `LED.busy`, and `LED[state="off"]`
  work.
- `LED::dot` controls the visible light body. `LED::glow` controls the halo
  behind on states; `opacity: 0` hides it, and `box-shadow: none` suppresses
  the built-in soft shadow. `LED::highlight` controls the small shine mark.

## Input And Control Widgets

### `Button`

Constructor:

```python
dg.Button(text, on_click=None, badge=None, disabled=False, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `text` | Button label. |
| `on_click` | Called when activated. |
| `badge` | Optional inline badge text/count. |
| `disabled` | Disables activation. |

Live methods:

- `click()`
- `set_badge(value)`

CSS:

- Type selector: `Button`.
- Parts: `badge`.
- Supports `:hover`, `:active`, `:focus`, and `:disabled`.

### `TextInput`

Constructor:

```python
dg.TextInput(value="", placeholder="", on_change=None, disabled=False, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `value` | Current text value. |
| `placeholder` | Placeholder shown when empty. |
| `on_change` | Called with string value after user edits. |
| `disabled` | Disables editing. |

Live methods:

- `set_value(value)`

CSS:

- Type selector: `TextInput`.
- No widget parts.
- Supports normal input visual/text styling and pseudo-states.

### `TextArea`

Constructor:

```python
dg.TextArea(value="", placeholder="", rows=4, wrap=True, on_change=None, disabled=False, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `value` | Current multi-line text value. |
| `placeholder` | Placeholder shown when empty. |
| `rows` | Minimum row count; must be at least 1. |
| `wrap` | Enables word wrapping. |
| `on_change` | Called with string value after user edits. |
| `disabled` | Disables editing. |

Live methods:

- `set_value(value)`

CSS:

- Type selector: `TextArea`.
- No widget parts.
- Uses TextInput-like surface styling plus multi-line text layout.
- Widget-specific property: `text-area-rows`.
- CSS `height` still forces an exact rendered size.

### `NumberInput`

Constructor:

```python
dg.NumberInput(value=0, min=None, max=None, step=1, on_change=None, disabled=False, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `value` | Current numeric value. |
| `min` | Optional minimum. |
| `max` | Optional maximum. |
| `step` | Stepper increment; must be positive. |
| `on_change` | Called with float value after user edits. |
| `disabled` | Disables editing. |

Live methods:

- `set_value(value)`

CSS:

- Type selector: `NumberInput`.
- Parts: `field`, `stepper`, `stepper-up`, `stepper-down`,
  `stepper-divider`, `divider`, `caret`.
- `stepper-down` styles the left/decrement side. `stepper-up` styles the
  right/increment side.
- Per-corner radius properties are useful for making stepper buttons match the
  input shape.

### `Slider`

Constructor:

```python
dg.Slider(value=0, min=0, max=1, step=0.01, on_change=None, disabled=False, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `value` | Current numeric value. |
| `min` | Minimum value. |
| `max` | Maximum value. |
| `step` | Positive step increment. |
| `on_change` | Called with float value after user input. |
| `disabled` | Disables interaction. |

Live methods:

- `set_value(value)`

CSS:

- Type selector: `Slider`.
- Parts: `track`, `fill`, `thumb`.
- Supports `track-color` and `thumb-color` on the widget or parts.

### `ProgressBar`

Constructor:

```python
dg.ProgressBar(value=0, min=0, max=1, label=None, show_value=False, disabled=False, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `value` | Current progress value. |
| `min` | Minimum value. |
| `max` | Maximum value. |
| `label` | Optional fixed label. |
| `show_value` | Shows computed percentage when `label` is omitted. |
| `disabled` | Uses disabled visual state. |

Live methods:

- `set_value(value)`

CSS:

- Type selector: `ProgressBar`.
- Parts: `track`, `fill`, `label`.

### `Dropdown`

Constructor:

```python
dg.Dropdown(items, value=None, on_change=None, disabled=False, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `items` | Non-empty iterable of item strings. |
| `value` | Selected item. Defaults to first item. |
| `on_change` | Called with selected string after user change. |
| `disabled` | Disables opening and selection. |

Live methods:

- `set_value(value)`

CSS:

- Type selector: `Dropdown`.
- Parts: `field`, `chevron`, `menu`, `item`, `item-selected`,
  `item-hover`.
- `:open` is supported while the popup is open.

### `Checkbox`

Constructor:

```python
dg.Checkbox(label, checked=False, on_change=None, disabled=False, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `label` | Label text. |
| `checked` | Initial checked state. |
| `on_change` | Called with `bool` after user toggles. |
| `disabled` | Disables toggling. |

Live methods:

- `set_checked(checked)`

CSS:

- Type selector: `Checkbox`.
- Parts: `row`, `box`, `indicator`, `label`.
- `Checkbox:checked` and `Checkbox:checked::indicator` are supported.

### `ColorPicker`

Constructor:

```python
dg.ColorPicker(value=(255, 100, 0), alpha=True, on_change=None, title="Color", width=320, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `value` | RGB/RGBA tuple. Integers are `0..255`; floats in `0.0..1.0` are normalized colors. |
| `alpha` | Includes alpha channel slider when true. |
| `on_change` | Called with normalized tuple after user edits. |
| `title` | Panel title. |
| `width` | Preferred maximum width. |

Live methods:

- `set_value(value, notify=False)`

CSS:

- `ColorPicker` is built from standard DragonGUI widgets. Style it through
  `Panel`, `Label`, `Slider`, and `Button` selectors, or through `class_` and
  `id` on the `ColorPicker` itself, which are passed to the outer `Panel`.
- There is no separate `ColorPicker` CSS type selector.

## Media And Data Widgets

### `Image`

Constructor:

```python
dg.Image(path, fit="contain", width=None, height=None, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `path` | Non-empty image path. |
| `fit` | `contain`, `cover`, or `stretch`. |
| `width` | Optional fixed width. |
| `height` | Optional fixed height. |

Live methods:

- `set_path(path)`
- `reload()`
- `set_fit(fit)`

CSS:

- Type selector: `Image`.
- No widget parts.
- `border-radius` clips the texture to the image content box inside the border.
- Paint-only `transform`, `translate`, `scale`, `rotate`, and relative
  positioning move the rendered texture with the image widget. Ancestor
  transforms also apply to the rendered texture as part of the transformed
  subtree.

### `Scatter3D`

Constructor:

```python
dg.Scatter3D(
    frame,
    x=...,
    y=...,
    z=...,
    colormap="viridis",
    color=None,
    colors=None,
    scalars=None,
    point_size=4.0,
    point_sizes=None,
    auto_point_size=True,
    opacity=1.0,
    clim=None,
    log_scale=False,
    on_pick=None,
    grid=False,
    major_planes=False,
    minor_planes=False,
    grid_sticky=True,
    grid_all_edges=False,
    lod=False,
    lod_threshold=200_000,
    lod_factor=8,
    interactive_render_scale=1.0,
    auto_quality=False,
    quality_target_fps=10.0,
    id=None, key=None, class_=None, style=None, tooltip=None, parent=...
)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `frame` | Data object with addressable numeric columns. Supports `frame[col]` subscript and `getattr(frame, col)` attribute access. |
| `x` | X column name. |
| `y` | Y column name. |
| `z` | Z column name. |
| `colormap` | `viridis`, `plasma`, `inferno`, `magma`, `coolwarm`, `hot`, `gray`, `grey`, `turbo`, `cividis`, `blues`, `greens`, or `reds`. Default `"viridis"`. |
| `color` | Column name or `(N, 3)`/`(N, 4)` array of per-point RGB/RGBA colors. Takes priority over `scalars`. |
| `colors` | `(N, 3)` or `(N, 4)` float/uint8 RGB/RGBA array. Takes highest priority for per-point color. |
| `scalars` | Column name or 1-D array of per-point scalar values mapped through `colormap`. Used when `color` and `colors` are both absent. |
| `point_size` | Default uniform point size in logical pixels. Default `4.0`. |
| `point_sizes` | Column name or 1-D array of per-point sizes. Overrides `point_size` when provided. |
| `auto_point_size` | If `True`, native may shrink rendered point sprites in dense views to reduce overdraw. Set `False` when exact fixed sizes are required. |
| `opacity` | Uniform alpha applied to all points. `1.0` = fully opaque. |
| `clim` | `(lo, hi)` scalar data range for colormap normalization. Derived from data when `None`. |
| `log_scale` | If `True`, apply log10 to scalar values before colormap mapping. |
| `nan_color` | `(r, g, b)` color in [0, 1] applied to NaN scalar positions instead of the colormap. |
| `size_range` | `(min_px, max_px)` pixel range for normalizing a `point_sizes` column from data units to logical pixels. |
| `grid` | Show axis grid, ticks, and labels. |
| `major_planes` / `minor_planes` | Draw major grid planes and minor subdivision lines. |
| `grid_sticky` | Keep automatically generated nice bounds and tick steps stable while new data remains inside the current grid range. Default `True`. |
| `grid_all_edges` | Draw an unlabeled boundary box around all grid edges as a stable reference frame. Default `False`. |
| `lod` | Enable representative point sampling while orbiting or panning when point count exceeds `lod_threshold`. Default `False`. |
| `lod_threshold` | Point-count threshold for interaction LOD. Default `200_000`. |
| `lod_factor` | Draw roughly `1 / lod_factor` of points while interaction LOD is active. Default `8`. |
| `interactive_render_scale` | Render scatter scene content to a lower-resolution offscreen target while orbiting/panning, then upscale into the widget. `1.0` keeps full resolution; lower values reduce fill cost during interaction. Default `1.0`. |
| `auto_quality` | Enable native interaction quality budgeting. When active, native may temporarily lower interaction render scale to approach `quality_target_fps`. Default `False`. |
| `quality_target_fps` | Target frame rate for `auto_quality`. Default `10.0`. |
| `on_pick` | Called with `ScatterPick` or `(index, x, y, z)` after a point pick. |

When any of `color`, `colors`, `scalars`, `point_sizes`, `opacity != 1.0`, `nan_color`, `clim`, or `log_scale=True` are used, the widget
automatically emits a `point_instance_v1` packet (per-point RGBA). Otherwise it emits the compact `xyz_f32_v0` format (XYZ only, colored by z-range at render time).

Mouse controls:

- Left drag orbits the camera.
- Middle drag, right drag, or Shift+left drag pans.
- Mouse wheel zooms.
- `R` or `Home` resets the active scatter camera.

Live methods:

- `set_points(frame, x, y=None, z=None, *, color=None, colors=None, scalars=None, point_size=None, point_sizes=None, opacity=None, clim=None, log_scale=None, nan_color=None, size_range=None, fit=False)` — replace point data. Set `fit=True` when replacing the scene with a different coordinate frame so the camera target and orbit center refit to the new bounds.
- `create_live_frame(frame=None, *, capacity=None, x=None, y=None, z=None, color=None, colors=None, scalars=None, point_size=None, point_sizes=None, opacity=None, colormap=None, clim=None, log_scale=None, nan_color=None, size_range=None, mode="primary", fit=False)` - create a retained replacement handle for sensors that publish complete frames. The default `mode="primary"` uses the fast primary packed update path without rebuilding the declarative widget tree; `mode="actor"` keeps an independent point actor layer. Call `live.replace(frame)` for simple use, pack off the UI thread with `Scatter3D.prepare_points(...)` and call `live.replace_prepared(payload)` from a GUI callback, or call `live.enqueue_prepared(payload)` directly from a producer thread for high-rate latest-frame streams.
- `prepare_points(frame, *, x, y, z, ...)` - class method that packs a frame into a reusable `ScatterPayload` without mutating a live widget. This is the preferred packing step for background workers and benchmark streams.
- `set_colormap(colormap)` — change colormap; repacks data if per-point colors are baked.
- `reset_camera()` — reset camera to last fitted position.
- `view_xy()`, `view_xz()`, `view_yz()`, `view_isometric()` — snap to a preset view direction.
- `fit(bounds=None)` — fit camera to data bounds; optional explicit `(x_min, y_min, z_min, x_max, y_max, z_max)`.
- `set_camera(state)` — apply a camera state dict with keys `target`, `distance`, `yaw`, `pitch`, `parallel`.
- `get_camera()` — return current camera state dict via debug snapshot, or `None` if not live.
- `set_point_style(style)` — set point shape: `"circle"`, `"square"`, or `"gaussian"`.
- `set_auto_point_size(enabled=True)` — toggle native adaptive point-size shrinking for dense views.
- `set_lod(enabled=True, threshold=200_000, factor=8)` — configure representative point sampling during orbit/pan interaction.
- `set_interactive_render_scale(scale)` - set interaction-only scatter render scale in the range `0.25..1.0`.
- `set_auto_quality(enabled=True, target_fps=None)` - enable or disable native interaction quality budgeting.
- `show_grid(visible=True)` — show or hide grid/ticks/labels.
- `show_grid_planes(major=True, minor=False)` — show or hide major and minor grid planes.
- `set_grid_options(sticky=True, all_edges=False)` — update sticky auto bounds and all-edges boundary behavior.
- `set_ticks(x=None, y=None, z=None)` — override per-axis tick counts; `None` uses auto ticks.
- `parallel_projection` — read/write property; `True` for orthographic, `False` for perspective (default).
- `colormap_names()` — class method returning sorted list of valid colormap names.

Live-frame streaming paths:

For complete-frame streams, first create a retained primary live frame:

```python
scatter = dg.Scatter3D(initial_frame, x="x", y="y", z="z")
live = scatter.create_live_frame(mode="primary")
```

Use `live.replace(frame)` for simple, lower-rate updates when packing cost and
Python callback scheduling are not the bottleneck.

Use `live.replace_prepared(payload, update_metadata=True)` when the producer
thread prepares data but the update is still part of normal GUI state handling.
This path is called from a GUI callback, updates Python-side scatter metadata,
and then enqueues the native upload:

```python
payload = dg.Scatter3D.prepare_points(frame, x="x", y="y", z="z")
app.call_soon_threadsafe(lambda: live.replace_prepared(payload, fit=False))
```

Use `live.enqueue_prepared(payload, update_metadata=False, coalesce=True)` for
high-rate sensors, simulations, or LiDAR-style feeds where the latest complete
frame matters more than every intermediate frame. This path is thread-safe for
producer threads, bypasses Python UI callback scheduling, and lets the native
queue coalesce stale pending frames for the same scatter:

```python
payload = dg.Scatter3D.prepare_points(frame, x="x", y="y", z="z")
live.enqueue_prepared(payload, update_metadata=False, coalesce=True)
```

In short: `replace_prepared(...)` is the stateful GUI-callback path;
`enqueue_prepared(...)` is the direct producer-thread path.

Callback data:

| Type | Fields |
| --- | --- |
| `ScatterPick` | `index`, `x`, `y`, `z`, `widget_id` |

CSS:

- Type selector: `Scatter3D`.
- No widget parts.
- Widget-specific properties: `scatter-point-size` (logical pixels), `scatter-point-style` (`circle` | `square` | `gaussian`), `scatter-grid-visible` (`true` | `false`), `scatter-grid-planes` (`none` | `major` | `minor` | `all`), `scatter-legend-position` (`top-right` | `top-left` | `bottom-right` | `bottom-left`), and `scatter-orientation-axes` (`true` | `false`).
- CSS `scatter-point-size` overrides the packed per-point sizes.
- CSS scatter chrome is limited to static presentation defaults. Axis label
  text, scalar bars, colormaps, point data, and legend entries are controlled
  through the Python API.
- CSS can style the container surface. `border-radius` and per-corner radii clip the 3D viewport and the picking region.
- `opacity`, `transform`, `translate`, `scale`, and `rotate` affect the border/background primitive only; the actual 3D viewport is layout-rect anchored and is not transformed or faded in this release.
- `Scatter3D` does not currently accept a `disabled` option; `on_pick` remains active whenever picking is enabled.

### `DataFrameTable`

Constructor:

```python
dg.DataFrameTable(frame, page_size=100, sample_rows=DEFAULT_TABLE_SAMPLE_ROWS, on_select=None, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `frame` | Table-like data object. |
| `page_size` | Virtualized page size; must be positive. |
| `sample_rows` | Startup sample row count; must be non-negative. |
| `on_select` | Called with `TableSelection` or positional fields after cell/row selection. |

Live methods:

- `set_frame(frame, sample_rows=None)`

Callback data:

| Type | Fields |
| --- | --- |
| `TableSelection` | `row`, `column`, `column_name`, `value` |

CSS:

- Type selector: `DataFrameTable`.
- Parts: `header`, `row`, `row-selected`, `grid-line`.
- Widget-specific properties: `table-row-height`,
  `table-header-height`, `table-column-width`, `table-index-width`.
- `border-radius` clips header, rows, selection, grid lines, and border to the
  table's rounded shape.
- `DataFrameTable` does not currently accept a `disabled` option; `on_select`
  remains active whenever selection callbacks are configured.
- `:selected` is supported when the table has an active selected cell/row.

## Convenience Dialog Widgets

### `alert`

Function:

```python
dg.alert(title, message, open=True, width=420, height=200, on_close=None, parent=...)
```

Returns:

- A `Modal` containing a `Label`, `Spacer`, and `OK` button.

CSS:

- No separate CSS selector.
- Style through the returned `Modal` and its child widgets.

### `confirm`

Function:

```python
dg.confirm(title, message, open=True, width=460, height=220, on_confirm=None, on_cancel=None, parent=...)
```

Returns:

- A `Modal` containing a `Label`, `Spacer`, `Cancel` button, and `Confirm`
  button.

CSS:

- No separate CSS selector.
- Style through the returned `Modal` and its child widgets.

## CSS Type Selector Inventory

Current CSS type selectors:

- `Window`
- `HLayout`
- `VLayout`
- `ScrollArea`
- `Panel`
- `Collapsible`
- `Modal`
- `Badge`
- `Tag`
- `LED`
- `MenuBar`
- `Menu`
- `MenuItem`
- `ContextMenu`
- `Tooltip`
- `Toast`
- `Sidebar`
- `StatusBar`
- `Tabs`
- `Tab`
- `Pages`
- `Page`
- `NavItem`
- `Label`
- `Button`
- `TextInput`
- `TextArea`
- `NumberInput`
- `Slider`
- `ProgressBar`
- `Dropdown`
- `Checkbox`
- `Separator`
- `Spacer`
- `Scatter3D`
- `DataFrameTable`
- `Image`

Notable non-selectors:

- `ColorPicker` is a composite implemented as a `Panel`.
- `App`, `Theme`, `FileDialog`, `alert`, and `confirm` are Python APIs, not
  native widget types.

## CSS Part Inventory

| Widget | Parts |
| --- | --- |
| `HLayout` | `scrollbar-track`, `scrollbar-thumb` |
| `VLayout` | `scrollbar-track`, `scrollbar-thumb` |
| `ScrollArea` | `scrollbar-track`, `scrollbar-thumb` |
| `Pages` | `scrollbar-track`, `scrollbar-thumb` |
| `Page` | `scrollbar-track`, `scrollbar-thumb` |
| `Sidebar` | `scrollbar-track`, `scrollbar-thumb` |
| `Panel` | `accent`, `scrollbar-track`, `scrollbar-thumb` |
| `Collapsible` | `header`, `indicator`, `body`, `scrollbar-track`, `scrollbar-thumb` |
| `Modal` | `scrim`, `scrollbar-track`, `scrollbar-thumb` |
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

Widgets without parts still support normal type/class/id CSS styling.

## Known Gaps And Planned Additions

These are known missing pieces rather than accidental omissions:

- `TextArea` sizing is controlled by constructor `rows`, CSS
  `text-area-rows`, or CSS `height`.
- A dedicated log/console output widget is not currently implemented.
