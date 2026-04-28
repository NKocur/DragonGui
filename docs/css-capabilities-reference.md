# DragonGUI CSS Capabilities Reference

This document inventories the current DragonGUI CSS styling system for audit.
It focuses on what the implementation can do today, where each feature applies,
and which limitations are intentional.

DragonGUI CSS is a native-widget styling subset. It is not browser CSS: there
is no DOM, HTML parser, JavaScript, general browser layout engine, media query
system, animation system, or full CSS compatibility promise.

## Entry Points

Stylesheets are attached to an app:

```python
app = dg.App()
app.stylesheet("Button { background: accent; }")
app.load_stylesheet("styles/app.dg.css")
app.clear_stylesheets()
```

Capabilities:

- `App.stylesheet(css)` accepts a non-empty CSS string.
- `App.load_stylesheet(path)` reads UTF-8 CSS from disk and applies it.
- `App.clear_stylesheets()` clears user stylesheets.
- Stylesheets can be queued before `app.run(...)`.
- Stylesheets can be changed while the app is live; the native runtime reapplies
  CSS without rebuilding the Python widget tree.
- Inline `style={...}` dictionaries continue to work and are merged after
  stylesheets.

Non-capabilities:

- Stylesheet hot-reload file watching is not built in.
- Public Python only exposes the user stylesheet origin. Framework/theme origins
  are internal.

## Parsing Model

DragonGUI uses `lightningcss` to parse CSS and immediately lowers the parsed AST
into DragonGUI-owned rules and declarations.

Supported top-level CSS:

- Normal style rules.
- Comma selector lists.
- `:root` custom property declarations.
- `!important` on declarations.

Ignored or unsupported:

- At-rules such as `@media`, `@supports`, and `@keyframes` are not part of the
  current public subset.
- Invalid CSS produces a stylesheet parse error.
- Unsupported selectors and unsupported declarations produce warnings and are
  skipped while the rest of the stylesheet continues.

## Cascade

Current style origins, lowest to highest:

1. Built-in framework stylesheet.
2. Theme stylesheet state.
3. User stylesheet loaded through Python.
4. Inline `style={...}` dictionaries.

Stylesheet declarations sort by:

```text
!important, origin, specificity, source order
```

Important details:

- `!important` works for stylesheet declarations.
- Inline style dictionaries are merged after stylesheet cascade and therefore
  override stylesheet declarations, including `!important` declarations.
- Inline style dictionaries do not support CSS `!important`.
- Later rules with the same origin and specificity win.
- Comma selector lists are lowered into separate rules sharing the same
  declaration block.

Specificity model:

| Selector Component | Specificity Bucket |
| --- | --- |
| `#id` | id |
| `.class` | class |
| `[key="main"]` | class |
| `:hover`, `:focus`, etc. | class |
| Type selectors such as `Button` | type |
| Widget parts such as `::stepper` | no extra specificity |
| Descendant and child chains | sum of each compound selector |

## Selectors

Supported selector forms:

| Form | Example |
| --- | --- |
| Type | `Button` |
| Class | `.primary` |
| Multiple classes | `Button.primary.danger` |
| ID | `#run-button` |
| Key attribute | `[key="primary-action"]` |
| Type + ID + class | `Button#run-button.primary` |
| Pseudo-state | `Button:hover` |
| Widget part | `NumberInput::stepper` |
| Descendant | `Panel Button` |
| Direct child | `Panel.controls > Button` |
| Multi-level child chain | `Window > Panel > HLayout > Button` |
| Direct child targeting a part | `Panel.controls > NumberInput::stepper` |
| Structural child | `Button:first-child` |
| Structural nth child | `NavItem:nth-child(even)` |
| Comma list | `Button, Dropdown, TextInput` |

Selector behavior:

- CSS type names are exact and case-sensitive, such as `Button`, not `button`.
- `class_` is split on CSS whitespace, so `class_="primary danger"` matches
  `.primary`, `.danger`, and `.primary.danger`.
- ID selectors match explicit widget `id`.
- Key attribute selectors match explicit widget `key`.
- Descendant selectors match any ancestor in the widget tree.
- Direct child selectors only check the immediate parent.
- Pseudo-states in selector chains apply to the target widget, not ancestors.
- `:first-child`, `:last-child`, and `:nth-child(...)` use the target
  widget's sibling position among all children of its parent.
- `:nth-child(...)` supports integer indexes plus `odd` and `even`.
- Part selectors are only valid on the target widget, not the parent side of a
  selector chain.

Unsupported selector forms:

- Attribute selectors other than `[key="..."]`.
- Universal selectors, such as `*`.
- Empty target part selectors, such as `::stepper`.
- Ancestor pseudo-states, such as `Panel:hover Button`.
- Complex `:nth-child(...)` formulas beyond integer, `odd`, and `even`.
- Browser pseudo-elements such as `::before` and `::after`.

## Pseudo-States

Supported pseudo-states:

- `:hover`
- `:active`
- `:focus`
- `:disabled`
- `:checked`
- `:open`
- `:expanded`
- `:collapsed`
- `:selected`

Behavior:

- Pseudo-state rules are precomputed into state slots.
- Runtime widget state decides which slot is active.
- For normal widget pseudo-state rules, visual declarations apply.
- For normal widget pseudo-state rules, `color` is accepted and maps to the
  pseudo foreground color.
- For normal widget pseudo-state rules, layout declarations are ignored.
- For normal widget pseudo-state rules, text declarations other than `color`
  are ignored.
- For part pseudo-state rules, visual and text declarations apply.
- For part pseudo-state rules, layout declarations are intentionally ignored and
  produce warnings because hover/active geometry would desynchronize hit testing.

Examples:

```css
Button:hover {
    background: accent_mix_20;
    color: text;
}

Checkbox:checked::indicator {
    background: success;
}

Dropdown:open {
    border-color: accent;
}

Collapsible:collapsed::indicator {
    color: muted_text;
}

Tab:selected::accent {
    background: accent;
}

NumberInput:hover::stepper-up {
    background: accent;
    color: white;
}
```

State-specific notes:

- `:checked` is meaningful for `Checkbox`.
- `:open` is meaningful for `Dropdown`, `Menu`, `ContextMenu`, and open
  `Modal` widgets.
- `:expanded` and `:collapsed` are meaningful for `Collapsible`.
- `:selected` is meaningful for active `Tab`, `NavItem`, active `Page`, and
  `DataFrameTable` widgets with an active selection.
- Dropdown popup row selection is still styled through `Dropdown::item-selected`
  rather than a real child `:selected` selector.

## CSS Type Selectors

Supported native type selectors:

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

Notable non-type selectors:

- `ColorPicker` is a Python composite built from `Panel`, `Label`, `Slider`,
  and `Button`. It has no `ColorPicker` type selector.
- `App`, `Theme`, `FileDialog`, `alert`, and `confirm` are Python APIs rather
  than native widget types.

## Automatic CSS Classes

DragonGUI adds a few classes from runtime metadata:

| Element | Automatic Classes |
| --- | --- |
| `Badge` | Current `level`: `neutral`, `info`, `success`, `warning`, `danger`, `error` |
| `Tag` | Current `level`: `neutral`, `info`, `success`, `warning`, `danger`, `error` |
| `Toast` | Current level: `info`, `success`, `warning`, `error` |
| Simple string tooltip overlays | `static` |

Examples:

```css
Badge.info {
    background: accent;
}

Tag.neutral {
    border-color: border;
}

Toast.error {
    background: danger;
}

Tooltip.static {
    border-radius: 8px;
}
```

Toast levels intentionally differ from Badge/Tag levels. Toast does not support
`neutral` or `danger`; use `error` for destructive/error notifications.

## Layout Properties

Supported layout properties:

| Property | Accepted Values | Effect |
| --- | --- | --- |
| `display` | `flex`, `block`, `none` | Sets display behavior. |
| `flex-direction` | `row`, `column`, `row-reverse`, `column-reverse` | Sets container direction. Snake-case reverse values are also accepted internally. |
| `flex` | number | Maps to `flex-grow`; clamped to at least `0`. |
| `flex-grow` | number | Clamped to at least `0`. |
| `flex-shrink` | number | Clamped to at least `0`. |
| `width` | logical px | Fixed/preferred width depending on widget/layout. |
| `height` | logical px | Fixed/preferred height depending on widget/layout. |
| `min-width` | logical px | Minimum width. |
| `min-height` | logical px | Minimum height. |
| `max-width` | logical px | Maximum width. |
| `max-height` | logical px | Maximum height. |
| `padding` | 1 to 4 logical-px values | Expands to per-side padding. |
| `padding-left` | logical px | Left padding. |
| `padding-right` | logical px | Right padding. |
| `padding-top` | logical px | Top padding. |
| `padding-bottom` | logical px | Bottom padding. |
| `margin` | uniform logical px only | Uniform margin. Non-uniform values warn and are ignored. |
| `gap` | logical px | Container gap. |

Length handling:

- Unitless numbers are logical pixels.
- `px` values are logical pixels.
- Percent lengths are parsed but warn and are ignored for public CSS
  properties.
- `auto` is parsed but warns and is ignored for public CSS properties.
- Negative lengths are not broadly rejected at parse time; renderer/layout
  behavior depends on the target property.

## Visual Properties

Supported visual properties:

| Property | Accepted Values | Effect |
| --- | --- | --- |
| `background` | color, `linear-gradient(...)`, or `radial-gradient(...)` | Fill/background paint. |
| `background-color` | color | Solid fill/background color. |
| `foreground` | color | Foreground glyph/control color for renderers that use visual foreground. |
| `border-color` | color | Border color. |
| `border-width` | logical px | Border width. |
| `border-radius` | logical px | Uniform corner radius. |
| `border-top-left-radius` | logical px | Per-corner radius override. |
| `border-top-right-radius` | logical px | Per-corner radius override. |
| `border-bottom-right-radius` | logical px | Per-corner radius override. |
| `border-bottom-left-radius` | logical px | Per-corner radius override. |
| `border` | `<width> solid <color>` | Shorthand for border width and color. Only `solid` is supported. |
| `box-shadow` | `<offset-x> <offset-y> <blur?> <spread?> <color>` | Single non-inset soft shadow for rect-backed widget surfaces. |
| `opacity` | number | Clamped to `0.0..1.0`. |
| `accent` | color | Widget accent color. |
| `track-color` | color | Slider/progress track color. |
| `thumb-color` | color | Slider thumb color. |

Notes:

- Per-corner radius values inherit from `border-radius` when a specific corner
  is not set.
- `box-shadow` currently supports one non-inset shadow. Multiple comma-separated
  shadows and `inset` shadows warn and are ignored.
- `linear-gradient(...)` currently supports angles and `to ...` directions.
  `radial-gradient(...)` currently supports centered circle gradients.
  Both first-slice gradient renderers use the first and last color stops;
  detailed intermediate stop interpolation is deferred.
- Repeating gradients, multiple background layers, browser `background-image`,
  `border-style`, `border-left`, `outline`, and `overflow` are not supported.
- `border: none` is not supported.
- General clipping/overflow is not supported. Specific renderers implement
  clipping where designed, such as `Image`, `DataFrameTable`, dropdown menus,
  menu rows, and panel accent fills.
- `Scatter3D` currently ignores rounded clipping; its 3D viewport remains
  rectangular.

## Text Properties

Supported text properties:

| Property | Accepted Values | Effect |
| --- | --- | --- |
| `color` | color | Text color. |
| `font-size` | logical px | Text size. |
| `font-family` | family keyword or name | Text family. |
| `font-weight` | `normal`, `bold`, or numeric `100..900` | Text weight, clamped to `100..900`. |
| `text-align` | `left`, `start`, `center`, `middle`, `right`, `end` | Text alignment. |
| `text-transform` | `none`, `uppercase`, `lowercase`, `capitalize` | Display-only text casing. Widget values and callbacks are not changed. |
| `letter-spacing` | unitless logical px, `px`, `em`, or `normal` | Glyph tracking. `em` maps to glyphon tracking units. |
| `line-height` | unitless multiplier, `px` | Text line height and vertical text placement. |
| `font-style` | `normal`, `italic` | Font style. |
| `font-variant-numeric` | `normal`, `tabular-nums` | Enables tabular number glyphs where the selected font supports them. |
| `text-overflow` | `clip`, `ellipsis` | Single-line overflow handling. First implementation renders the marker as `...`. |

Font family keywords:

- `serif`
- `sans`, `sans-serif`, `sans_serif`, `system`
- `mono`, `monospace`
- `cursive`
- `fantasy`
- Any other value is treated as a named family.

Text inheritance:

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

Only text properties inherit. Layout, visual, and widget-specific properties do
not inherit.

Important distinction:

- CSS `color` is a text property.
- Inline JSON `style={"color": ...}` feeds both visual foreground and text color
  through the inline-style parser.

## Widget-Specific Properties

Supported widget-specific CSS:

| Property | Widget | Effect |
| --- | --- | --- |
| `table-row-height` | `DataFrameTable` | Row height in logical pixels. |
| `table-header-height` | `DataFrameTable` | Header height in logical pixels. |

Unsupported widget-specific needs:

- There is no CSS `rows` property for `TextArea`; use constructor `rows` or CSS
  `height`.
- There is no table column width CSS yet.
- There is no scatter colormap CSS property.

## Color Values

Supported color syntax:

- Theme tokens.
- Derived theme tokens.
- `#RGB`
- `#RGBA`
- `#RRGGBB`
- `#RRGGBBAA`
- `transparent`
- Common named colors: `black`, `white`, `red`, `green`, `blue`, `gray`,
  `grey`.
- `rgb(...)`
- `rgba(...)`
- `hsl(...)`
- `hsla(...)`
- `var(--name)` when the variable resolves to a color, token, or string token.

Theme tokens:

- `background`
- `surface`
- `surface_alt`
- `text`
- `foreground`
- `muted_text`
- `muted`
- `accent`
- `border`
- `danger`
- `warning`
- `success`
- `focus`
- `disabled`

Derived tokens:

- `accent_mix_20`
- `accent_mix_12`
- `accent_dark`

Unsupported color syntax:

- `oklch(...)`
- `lab(...)`
- `color(...)`
- Full browser named color inventory beyond the common names listed above.

Identifier-like color values are treated as DragonGUI theme tokens. An unknown
token logs a warning to stderr at render time and falls back to the theme
`danger` color.

## CSS Variables

Supported variable syntax:

```css
:root {
    --brand: #ff6b35;
    --panel-radius: 8px;
}

Button {
    background: var(--brand);
    color: var(--button-text, white);
    border-radius: var(--panel-radius);
}
```

Capabilities:

- Custom properties are collected from `:root`.
- `:root` variables are collected before normal declarations are lowered, so
  variables can be used before they appear in source order.
- Variable values can resolve to numbers, lengths, colors, keywords, or strings.
- `var(--name)` is supported as a whole property value.
- `var(--name, fallback)` is supported as a whole property value.

Limitations:

- Scoped custom properties are not supported.
- `var()` inside larger expressions is not supported.
- `calc()` is not supported.
- Variables are resolved during parsing/lowering of a stylesheet. Do not rely on
  variables defined in one stylesheet origin being available inside another
  stylesheet string.

## Widget Parts

Parts are renderer-owned styling hooks. They are not real widgets and do not
have separate layout nodes, focus targets, hit-test regions, callbacks, or ids.

Supported parts:

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

Part property support:

| Property Class | Base Part | Stateful Part |
| --- | --- | --- |
| Visual properties | supported | supported |
| Text properties | supported | supported |
| `width` | supported where renderer uses it | ignored with warning |
| `height` | supported where renderer uses it | ignored with warning |
| `padding` | supported when uniform | ignored with warning |
| `gap` | parsed; reserved for renderer-specific future use | ignored with warning |
| Widget-specific table properties | ignored | ignored |

Examples:

```css
Panel::accent {
    width: 5px;
    background: accent;
}

NumberInput::stepper {
    width: 34px;
    background: surface_alt;
}

NumberInput::stepper-up {
    border-top-right-radius: 8px;
}

Dropdown::item-hover {
    background: accent_mix_20;
}

Checkbox:checked::indicator {
    background: success;
}
```

Unsupported part selectors:

- Unsupported parts warn during cascade.
- Parent-side part selectors in child rules are rejected.
- CSS part names should use dashed names such as `stepper-up`. Snake-case part
  names are normalized only for inline style dictionaries.

## Inline Style Dictionaries

Inline styles use Python dictionaries and mostly snake_case keys:

```python
dg.Button(
    "Run",
    style={
        "background": "accent",
        "border_radius": 8,
        "hover": {"background": "accent_dark"},
    },
)
```

Inline capabilities:

- Inline styles override all stylesheet rules.
- Inline styles support nested pseudo visual maps: `hover`, `active`, `focus`,
  `disabled`, `checked`, `open`, `expanded`, `collapsed`, `selected`.
- Inline styles support `parts`.
- Inline part names accept dashed or snake-case forms.
- Invalid inline part names raise a Python `ValueError` before the document is
  sent to native code.

Inline part example:

```python
dg.NumberInput(
    42,
    style={
        "parts": {
            "stepper": {"width": 34},
            "stepper_up": {
                "background": "surface_alt",
                "color": "accent",
                "border_top_right_radius": 10,
            },
        }
    },
)
```

Inline-specific value differences:

- Inline color arrays with 3 or 4 numeric channels are accepted.
- Inline font weight accepts more named values than CSS, including `thin`,
  `light`, `medium`, `semibold`, `extra-bold`, and `black`.
- Inline keys are not CSS strings and do not support CSS shorthands such as
  `border: 1px solid border`.

## Virtual Overlay Styling

### Simple Tooltips

Simple tooltips created by `tooltip="..."` are virtual overlay elements.

Selectors:

- `Tooltip`
- `Tooltip.static`

Capabilities:

- Surface/text styling through normal visual/text properties.
- Uniform padding through `padding`.
- Opacity through `opacity`.
- Border radius through `border-radius`.

No parts are exposed.

### Rich Tooltips

`dg.Tooltip(...)` creates a normal widget-tree tooltip container. It matches the
`Tooltip` type selector and any user-supplied `class_`.

### Toasts

Toasts are virtual overlay elements.

Selectors:

- `Toast`
- `Toast.info`
- `Toast.success`
- `Toast.warning`
- `Toast.error`

Capabilities:

- Surface and border color from CSS can override level-derived fallback colors.
- Text color from CSS can override level-derived fallback text color.
- `border-radius` controls toast radius unless an explicit toast API `radius`
  override is supplied.
- `padding` controls toast padding unless an explicit toast API `padding`
  override is supplied.
- `opacity` multiplies the toast API `opacity`.
- Toast API `position` is not controlled by CSS.
- Toast API `duration` is not controlled by CSS.

No parts are exposed.

## Framework Defaults

DragonGUI installs a built-in framework stylesheet before user CSS.

Current default coverage includes:

- `Window` background.
- `Panel`, `Collapsible`, `Tooltip`, and `Modal` surface, border, radius, and
  accent.
- `Sidebar`, `StatusBar`, `MenuBar`, and `Tabs` surface and border.
- `Separator` and `Scatter3D` basic border/background.
- `Button` and `Dropdown` surface, border, radius, accent, text color, and
  hover/active/disabled states.
- `TextInput`, `TextArea`, and `NumberInput` border, radius, accent, color, and
  focus/hover/disabled states.
- `Checkbox`, `Slider`, `ProgressBar`, `Image`, `Tab`, `NavItem`, and `Menu`
  baseline styling.

User CSS can override these defaults through normal cascade rules.

## Renderer-Specific CSS Effects

Not every property affects every widget equally. Current renderer-specific
effects include:

- `Panel::accent` is drawn as a left-side fill slice clipped inside the panel.
- `box-shadow` emits a paint-only soft shadow before the widget surface. It
  does not affect layout or hit testing.
- `DataFrameTable` with `border-radius` clips header, rows, selection, grid
  lines, and border to the table shape.
- `Image` with `border-radius` clips the texture to the image content box inside
  the border.
- Dropdown popup and menu item fills are clipped to rounded popup bounds.
- `:open`, `:expanded`, `:collapsed`, and `:selected` are resolved from
  runtime widget state and can style both whole widgets and widget parts.
- `linear-gradient(...)` and centered `radial-gradient(...)` backgrounds render
  on normal rect-backed widget surfaces and respect rounded rect clipping. The
  first renderer slice uses the first and last color stops as the gradient
  endpoints.
- Slider uses `track-color`, `thumb-color`, and `Slider::track`,
  `Slider::fill`, `Slider::thumb`.
- ProgressBar uses `ProgressBar::track`, `ProgressBar::fill`, and
  `ProgressBar::label`.
- NumberInput stepper geometry can be styled with `NumberInput::stepper`,
  `::stepper-up`, `::stepper-down`, `::divider`, `::stepper-divider`, and
  `::caret`.
- Tab and NavItem badges/accent strips are styled through their parts.

Known renderer gaps:

- No general `overflow: hidden`.
- No rounded clipping for `Scatter3D`.
- No modal scrim CSS part.
- No CSS-controlled toast position/duration.

## Debugging And Audit Hooks

`app.debug_snapshot()` includes CSS-related data:

```json
{
  "stylesheets": {
    "framework_rules": 0,
    "theme_rules": 0,
    "user_rules": 0,
    "warning_count": 0,
    "last_error": null
  },
  "computed_styles": {
    "widget-id": {
      "matched_rules": [],
      "style": {
        "layout": {},
        "visual": {},
        "text": {},
        "widget": {},
        "parts": {}
      }
    }
  }
}
```

Snapshot capabilities:

- Rule counts by origin.
- Warning count.
- Last stylesheet parse error.
- Matched rules per widget.
- Matched part rules per widget part.
- Computed layout, visual, text, widget, pseudo, and part style fields.

Warnings are also generated for:

- Unsupported CSS properties.
- Unsupported selector forms.
- Unsupported length forms such as percentages and `auto`.
- Non-uniform `margin`.
- Unsupported widget parts.
- Stateful part layout declarations.
- Unsupported inline parts that reach the native validator.

## Current Limitations Summary

Selector limitations:

- No universal selector.
- No attribute selectors other than `[key="..."]`.
- No ancestor pseudo-states in selector chains.
- No browser pseudo-elements.
- No `:not`, `:is`, or `:where` selectors yet.
- No complex `:nth-child(...)` formulas beyond integer, `odd`, and `even`.

Property limitations:

- No CSS Grid.
- No `calc()`.
- No percent sizing in public CSS.
- No `auto` sizing in public CSS.
- No transitions or animations.
- No multiple or inset box shadows.
- No repeating, multi-layer, or image backgrounds.
- No `overflow`.
- No per-side border shorthand.
- No `border: none`.
- No CSS table column width controls.
- No scatter-specific CSS properties.

Variable limitations:

- No scoped variables.
- No cross-stylesheet variable sharing guarantee.
- No dynamic variable recomputation beyond stylesheet reparse.

Renderer limitations:

- `Scatter3D` viewport is rectangular.
- Modal scrim is not CSS-addressable.
- Toast stacking geometry is API/runtime controlled, not CSS controlled.
- Parts cannot change layout by pseudo-state.

## Useful CSS Examples In Repo

- `examples/css_showcase.py`
- `examples/css_design_system_demo.py`
- `examples/all_features_css_demo.py`
- `examples/css_widget_parts_demo.py`
- `examples/css_web_capabilities_demo.py`
- `examples/css_theme_gallery.py`
- `examples/meridian.py`
