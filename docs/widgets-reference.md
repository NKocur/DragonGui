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
| Type | `Button` | Uses DragonGUI widget type names. |
| Class | `.primary` | Matches `class_="primary"` or whitespace-split class strings. |
| Type + class | `Button.primary.danger` | Multiple class selectors are supported. |
| ID | `#run-button` | Matches explicit widget `id`. |
| Key attribute | `[key="primary-action"]` | Matches explicit widget `key`. |
| Descendant | `Panel Button` | Matches any ancestor relationship. |
| Direct child | `Panel.controls > Button` | Checks immediate parent. |
| Child chain | `Window > Panel > HLayout > Button` | Multi-level direct-child chains are supported. |
| Pseudo-state | `Button:hover` | Supported pseudo-states are listed below. |
| Structural child | `Button:first-child` | Matches by sibling position. |
| Widget part | `NumberInput::stepper` | Part hooks style renderer-owned sub-elements. |

Unsupported selector forms warn and are ignored: attribute selectors other than
`[key="..."]`, universal selectors, browser pseudo-elements, and selector
functions such as `:not(...)`, `:is(...)`, and `:where(...)`.
`:nth-child(...)` supports integer indexes plus `odd` and `even`.

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
  `linear-gradient(...)` and `radial-gradient(...)`)
- `foreground`
- `border-color`
- `border-width`
- `border-radius`
- `border-top-left-radius`
- `border-top-right-radius`
- `border-bottom-right-radius`
- `border-bottom-left-radius`
- `border: <width> solid <color>`
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

- `table-row-height`
- `table-header-height`

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
  and `hsla(...)`.
- Global `:root` variables through `var(--name)` and
  `var(--name, fallback)`.

Supported background paint values:

- Solid colors through the color syntax above.
- `linear-gradient(...)` and centered `radial-gradient(...)` on `background`
  for rect-backed widget surfaces. The first implementation uses the first and
  last color stops.

Supported length values are logical pixels. Unitless numbers and `px` values
are accepted. Percent lengths and `auto` warn and are ignored for the current
CSS subset.

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
dg.App(title="DragonGUI", theme=None, metadata={})
```

Options:

| Option | Notes |
| --- | --- |
| `title` | App document title. |
| `theme` | Optional `dg.Theme`. |
| `metadata` | JSON-safe metadata included in the app document. |

Methods:

| Method | Notes |
| --- | --- |
| `document(window)` | Builds the JSON-safe app document. |
| `stylesheet(css)` | Adds or applies a user stylesheet. |
| `load_stylesheet(path)` | Reads UTF-8 CSS from disk and applies it. |
| `clear_stylesheets()` | Clears user stylesheets. |
| `run(window)` | Starts the native event loop. Accepts `Window` or component instance. |
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
- No widget parts.
- Typically styled with `gap`, `padding`, `height`, `flex`, and background.

### `VLayout`

Constructor:

```python
dg.VLayout(id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

CSS:

- Type selector: `VLayout`.
- No widget parts.
- Typically styled with `gap`, `padding`, `width`, `flex`, and background.

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

CSS:

- Type selector: `Panel`.
- Parts: `accent`.
- `Panel::accent` styles the left-side accent fill. `width` controls accent
  thickness and the fill is clipped to the panel shape.

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
- Parts: `header`, `indicator`, `body`.
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
- No widget parts.
- `:open` is supported while the modal is visible.
- The modal surface uses normal visual/text CSS. The full-screen scrim is not
  currently exposed as a CSS part.

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
- No widget parts.

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
- No widget parts.
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
- No widget parts.

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
dg.Label(text, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options:

| Option | Notes |
| --- | --- |
| `text` | Display text. |

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
- There is no CSS `rows` property. Use CSS `height` to control rendered size.

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

### `Scatter3D`

Constructor:

```python
dg.Scatter3D(frame, x=..., y=..., z=..., colormap="viridis", on_pick=None, id=None, key=None, class_=None, style=None, tooltip=None, parent=...)
```

Options and callbacks:

| Option | Notes |
| --- | --- |
| `frame` | Data object with addressable numeric columns. |
| `x` | X column name. |
| `y` | Y column name. |
| `z` | Z column name. |
| `colormap` | `viridis`, `plasma`, `inferno`, `magma`, `coolwarm`, `hot`, `gray`, `grey`, `turbo`, `cividis`, `blues`, `greens`, or `reds`. |
| `on_pick` | Called with `ScatterPick` or positional pick fields after point pick. |

Live methods:

- `set_points(frame, x, y=None, z=None)`
- `set_colormap(colormap)`

Callback data:

| Type | Fields |
| --- | --- |
| `ScatterPick` | `index`, `x`, `y`, `z` |

CSS:

- Type selector: `Scatter3D`.
- No widget parts.
- CSS can style the container/surface, but the 3D viewport is currently
  rectangular. Rounded scatter clipping is not implemented.
- Setting `border-radius` on `Scatter3D` currently has no visual effect on the
  3D viewport.
- `Scatter3D` does not currently accept a `disabled` option; `on_pick` remains
  active whenever picking is enabled.

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
  `table-header-height`.
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
- `Panel`
- `Collapsible`
- `Modal`
- `Badge`
- `Tag`
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

Widgets without parts still support normal type/class/id CSS styling.

## Known Gaps And Planned Additions

These are known missing pieces rather than accidental omissions:

- Modal scrim styling is not exposed as a CSS part yet. A future part should
  allow custom scrim color and opacity.
- `TextArea` sizing is controlled by constructor `rows` or CSS `height`; there
  is no CSS `rows` property.
- `Scatter3D` rounded clipping is not implemented. Its 3D viewport remains
  rectangular even inside rounded panels.
- A dedicated log/console output widget is not currently implemented.
