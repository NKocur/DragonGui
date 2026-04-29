# DragonGUI CSS Styling

DragonGUI supports a deliberate CSS subset for styling native widgets. CSS is
parsed by the Rust backend, lowered into DragonGUI-owned style rules, and
compiled into the same `NodeStyle` model used by inline `style={...}` maps.

The goal is familiar styling for DragonGUI widgets, not browser compatibility.
There is no DOM, no HTML, no JavaScript, and no general browser layout engine.

## Loading Stylesheets

Stylesheets can be attached before `app.run(...)`:

```python
import dragongui as dg

app = dg.App()
app.stylesheet("""
Button {
    border-radius: 4px;
    background: surface_alt;
}

Button:hover {
    background: accent_mix_20;
}
""")
```

Stylesheets can also be loaded from disk:

```python
app.load_stylesheet("styles/app.dg.css")
```

For a running app, `app.stylesheet(...)`, `app.load_stylesheet(...)`, and
`app.clear_stylesheets()` enqueue native stylesheet commands. The UI is
restyled without rebuilding the Python document.

## Media Queries

DragonGUI supports a first slice of `@media` for responsive app layouts. Width,
height, and orientation are evaluated from the logical viewport and stylesheets
are reapplied on window resize.

```css
@media (max-width: 760px) {
    Window {
        padding: 12px;
    }

    Panel.sidebar {
        display: none;
    }
}

@media (min-height: 600px) and (max-height: 900px) {
    Label.metric {
        font-size: 18px;
    }
}

@media (orientation: landscape) {
    Panel.toolbar {
        width: 320px;
    }
}
```

Supported media features are `width`, `height`, and `orientation`;
comma-separated query lists, `and`, `or`, `not`, and range syntax are accepted
when the resulting conditions only use those features. Container queries,
`@font-face`, and `@keyframes` are still unsupported.

DragonGUI also supports a first slice of static `@supports` feature queries.
Declaration queries use the DragonGUI property parser, selector queries use the
DragonGUI selector subset, and false queries skip their nested rules.

```css
@supports (display: grid) and (selector(Panel > Button.primary)) {
    Panel.dashboard {
        display: grid;
    }
}

@supports not (backdrop-filter: blur(8px)) {
    Panel.floating {
        background: rgba(18, 25, 39, 0.94);
    }
}
```

## Cascade Order

DragonGUI resolves styles in this order, lowest to highest precedence:

1. Built-in framework stylesheet.
2. Theme stylesheet state.
3. User stylesheet loaded through Python.
4. Inline `style={...}` dictionaries.

Within a stylesheet, the cascade key is:

```text
!important, origin, specificity, source order
```

Inline styles remain the strongest normal author style. This preserves existing
apps and lets local widget overrides win over global stylesheet rules.

## Selectors

Supported selectors:

```css
* { ... }                       /* universal */
Button { ... }                  /* type */
.primary { ... }                /* class */
Button.primary { ... }          /* type + class */
#run-button { ... }             /* explicit widget id */
[key="primary-action"] { ... }  /* explicit widget key */
[disabled] { ... }              /* boolean/state metadata presence */
[level="info"] { ... }          /* exact widget metadata/prop */
[class~="pill"] { ... }         /* whitespace-separated metadata word */
[text^="Run"] { ... }           /* string prefix metadata/prop */
[text="run" i] { ... }          /* ASCII case-insensitive metadata match */
Panel Button { ... }            /* descendant */
Panel.controls > Button { ... } /* direct child */
Panel > * { ... }               /* universal child target */
Panel > HLayout > Button { ... }/* child chain */
Button:hover { ... }            /* pseudo-state */
Button:not(.ghost) { ... }      /* selector function */
Panel > *:nth-child(3n+1) { ... } /* structural formula */
Panel > *:nth-child(2 of Panel > Button.primary) { ... } /* filtered structural formula */
```

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

`class_` supports normal CSS whitespace splitting:

```python
dg.Button("Run", class_="primary danger")
```

Then either `.primary`, `.danger`, or `Button.primary.danger` can match.

Attribute selectors can target stable widget metadata and parsed scalar props,
including `key`, `id`, `type`, `class`, `text`, `level`, `value`, `page`,
`disabled`, `checked`, `expanded`, and `open`. Supported operators are
presence, exact (`=`), word (`~=`), prefix (`^=`), suffix (`$=`), substring
(`*=`), and dash-match (`|=`). Value selectors support ASCII case flags:
`i` for case-insensitive matching and `s` for case-sensitive matching.
Boolean attributes are present only when true.

Unsupported selector forms produce warnings and are ignored. Examples:

- Ancestor pseudo-states inside selector functions, such as `Panel:is(:hover) Button`.
- Browser pseudo-elements such as `::before` and `::after`.

## Widget Type Names

Type selectors use DragonGUI widget names:

- `Window`
- `HLayout`
- `VLayout`
- `Panel`
- `Collapsible`
- `Modal`
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
- `Badge`
- `Tag`
- `Button`
- `TextInput`
- `TextArea`
- `NumberInput`
- `Slider`
- `Dropdown`
- `Checkbox`
- `ProgressBar`
- `Separator`
- `Spacer`
- `Scatter3D`
- `DataFrameTable`
- `Image`

`ColorPicker` is a Python composite widget built from `Panel`, `Label`,
`Slider`, and `Button`; style it through those underlying widget types, classes,
or inline styles.

`Toast` and simple string `Tooltip` overlays are not normal layout widgets, but
they do participate in type/class CSS matching. Toast levels are exposed as
classes, so `Toast.error` and `Toast.success` can style runtime notifications.
Simple `tooltip="..."` overlays match `Tooltip` and `Tooltip.static`; rich
`dg.Tooltip(...)` widgets also match `Tooltip` through the normal widget tree.

## Layout Properties

Supported layout properties:

| CSS | Notes |
| --- | --- |
| `display` | `flex`, `grid`, `block`, `none` |
| `flex-direction` | `row`, `column`, `row-reverse`, `column-reverse` |
| `flex` | maps to `flex-grow` |
| `flex-grow` | non-negative number |
| `flex-shrink` | non-negative number |
| `width` | logical pixels, percent, `auto` |
| `height` | logical pixels, percent, `auto` |
| `min-width` | logical pixels, percent, `auto` |
| `min-height` | logical pixels, percent, `auto` |
| `max-width` | logical pixels, percent, `auto` |
| `max-height` | logical pixels, percent, `auto` |
| `padding` | one to four logical-pixel, percent, or compatible `calc()` values |
| `padding-left` | logical pixels, percent, compatible `calc()` |
| `padding-right` | logical pixels, percent, compatible `calc()` |
| `padding-top` | logical pixels, percent, compatible `calc()` |
| `padding-bottom` | logical pixels, percent, compatible `calc()` |
| `margin` | uniform logical pixels, percent, `auto`, or compatible `calc()` |
| `gap` | logical pixels, percent, compatible `calc()` |
| `row-gap` | logical pixels, percent, compatible `calc()` |
| `column-gap` | logical pixels, percent, compatible `calc()` |
| `grid-template-columns` | first-slice grid track list |
| `grid-template-rows` | first-slice grid track list |
| `grid-column` | line or span placement |
| `grid-row` | line or span placement |
| `overflow` | `visible`, `hidden`, `scroll`, `auto` |
| `overflow-x` | `visible`, `hidden`, `scroll`, `auto` |
| `overflow-y` | `visible`, `hidden`, `scroll`, `auto` |
| `position` | `static`, `relative`, `absolute`, `fixed` |
| `top`, `right`, `bottom`, `left` | logical pixels |
| `z-index` | integer sibling stacking hint |

Percent lengths and first-slice `calc()` are supported for sizing, padding,
uniform margin, and gap properties above. `auto` is supported for sizing and
uniform margin; padding and gap reject `auto`. `calc()` currently supports
addition, subtraction, and simple scalar multiply/divide for pixel and percent
terms, such as `calc(220px + 40px)`, `calc(20% + 30%)`, or
`calc(100% - 240px)`. Mixed percent/pixel expressions resolve when the parent
axis has a definite size.

First-slice CSS Grid supports `display: grid`, track lists using px, percent,
`fr`, `auto`, `minmax()`, `fit-content()`, `repeat(n, ...)`, and
`repeat(auto-fit/auto-fill, ...)`, plus `grid-column` and `grid-row`
placements such as `1`, `2 / 4`, and `1 / span 2`.

First-slice overflow supports explicit clipping with `hidden`, child escape
with `visible`, and scroll opt-in with `auto` or `scroll`. `overflow-x`
containers can scroll horizontally with horizontal wheel input or shift-wheel;
`overflow-y` containers use the vertical wheel path. Scrollable panels draw
inset vertical and horizontal overlay scrollbars that stay inside rounded panel
corners and leave the corner clear when both axes overflow.
Users can drag the thumb, click the track, or use PageUp/PageDown/Home/End when
keyboard focus is inside a scroll container. Shift changes those keys to the
horizontal axis when horizontal overflow exists. `Panel::scrollbar-track`
styles the track and supports `width` plus uniform `padding` for the axis
inset; `Panel::scrollbar-thumb` styles the thumb.

First-slice positioning supports paint-only `relative` offsets,
layout-backed `absolute` children inside their parent, and viewport-backed
`fixed` widgets. Fixed widgets are removed from normal flow and clip against the
window rather than the parent container.

## Visual Properties

Supported visual properties:

| CSS | Notes |
| --- | --- |
| `background` / `background-color` | theme token or hex color |
| `foreground` | theme token or hex color |
| `color` | text color and foreground |
| `border-color` | theme token or hex color |
| `border-width` | logical pixels |
| `border-radius` | logical pixels |
| `border-top-left-radius` | logical pixels |
| `border-top-right-radius` | logical pixels |
| `border-bottom-right-radius` | logical pixels |
| `border-bottom-left-radius` | logical pixels |
| `border` | `none`, `0`, or `<width> solid <color>` |
| `box-shadow` | comma-separated outset or `inset` soft shadow layers |
| `opacity` | `0.0` to `1.0` |
| `accent` | widget accent color |
| `track-color` | slider/progress track color |
| `thumb-color` | slider thumb color |
| `transform` | paint-only `translate(...)`, `scale(...)`, `rotate(...)` shorthand |
| `translate` | paint-only one or two logical-pixel offsets |
| `scale` | paint-only one or two numeric scale factors |
| `rotate` | paint-only angle |

Supported color forms:

- Theme tokens such as `surface`, `accent`, `border`, `danger`.
- Derived tokens such as `accent_mix_20` and `accent_dark`.
- `#RGB`
- `#RGBA`
- `#RRGGBB`
- `#RRGGBBAA`

## Text Properties

Supported text properties:

| CSS | Notes |
| --- | --- |
| `font-size` | logical pixels |
| `font-family` | `serif`, `sans-serif`, `monospace`, or named family |
| `font-weight` | `normal`, `bold`, or numeric `100` to `900` |
| `color` | theme token or hex color |
| `text-align` | `left`, `center`, `right` |

Text properties inherit down the widget tree. Layout and visual properties do
not inherit.

## Widget Properties

Supported widget-specific properties:

| CSS | Widget |
| --- | --- |
| `table-row-height` | `DataFrameTable` |
| `table-header-height` | `DataFrameTable` |

## Widget Parts

Some widgets expose named internal renderer parts. Parts are not real widgets:
they do not get their own callbacks, focus targets, or layout nodes. They are
stable styling hooks for pieces the native renderer already draws.

Part selectors use `Widget::part-name`:

```css
NumberInput::stepper {
    width: 36px;
}

NumberInput::stepper-up {
    background: accent;
    border-top-right-radius: 10px;
    border-bottom-right-radius: 10px;
}

NumberInput::field {
    background: surface;
}

Checkbox:checked::indicator {
    background: success;
}
```

Pseudo-states are widget-level in this slice. For example,
`NumberInput:hover::stepper-up` means "when the whole number input is hovered,
style the right-side increment stepper part."

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

Part styles support the same visual and text properties as widgets. `Panel::accent`
is rendered as a left-side fill slice clipped to the panel's inner rounded shape,
so its `width` can change without manually matching the panel corner radius. A
`DataFrameTable` with `border-radius` clips its header, rows, selection, grid
lines, and border to the table's rounded shape, which keeps tables clean inside
rounded panels. `Image` textures are clipped to the image widget's rounded
content box inside the border. Dropdown and menu item fills are clipped to their
rounded popup bounds.

Rounded `Scatter3D` clipping is not part of the current CSS slice. Scatter uses
a rectangular 3D viewport and needs a dedicated stencil/mask implementation for
rounded clipping. Overflow support is first-slice: horizontal scrolling works,
but visible scrollbars are currently limited to `Panel` scroll containers.

Runtime `Toast` overlays and simple string `Tooltip` overlays have no CSS parts.
Style their surface and text through the type selector instead:

```css
Toast.error {
    background: danger;
    border-color: danger;
    color: white;
}

Tooltip.static {
    background: surface_alt;
    border-radius: 8px;
}
```

A small subset of layout properties is accepted where the renderer already has
matching geometry:

| Property | Common Use |
| --- | --- |
| `width` | panel accent fill width from the left edge, checkbox boxes/indicators, number steppers/dividers/carets, dropdown chevrons, slider thumbs, nav accents, grid lines |
| `height` | checkbox boxes/indicators, number stepper dividers/carets, slider tracks/thumbs, progress fills, tab headers, tab accents |
| `padding` | dropdown rows, tab/nav/table text padding |
| `gap` | reserved for future composite parts |

Stateful part layout is intentionally not supported. A rule such as
`NumberInput:hover::stepper { width: 48px; }` warns and ignores the layout
field, because hover-driven geometry would desynchronize rendering and hit
testing.

Unsupported parts warn through the stylesheet warning/debug snapshot path:

```css
Button::stepper {
    background: danger; /* warning: Button has no stepper part */
}
```

### Inline Part Styles

Inline `style={...}` dictionaries can style parts without a stylesheet:

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

Inline part names accept both dashed and snake-case forms. These are equivalent:

- `stepper-up`
- `stepper_up`

Inline part styles override stylesheet part rules, just like normal inline
widget styles override normal stylesheet rules.
Invalid inline part names raise a Python `ValueError` before the document is
sent to the native backend.

## Variables

Global custom properties can be defined in `:root`:

```css
:root {
    --panel-radius: 8px;
    --brand: #ff6b35;
}

Panel {
    border-radius: var(--panel-radius);
    border-color: var(--brand);
    box-shadow: 0 8px 24px var(--brand);
}
```

V2 supports global `:root` variables only. Scoped custom properties are not
implemented. `var()` may be used as a whole property value or inside larger
parseable values such as borders, shadows, gradients, and transition
shorthands.

## Debugging

`app.debug_snapshot()` includes stylesheet and computed-style data:

- framework/theme/user rule counts
- stylesheet warning count
- last stylesheet error
- matched rules per widget
- matched rules and computed fields per styled widget part
- computed layout, visual, text, and widget style fields

Unsupported CSS does not crash the app. The parser records warnings, skips the
unsupported declaration or selector, and continues applying the rest of the
stylesheet.

## Examples

Useful CSS examples:

- `examples/css_showcase.py`
- `examples/css_design_system_demo.py`
- `examples/all_features_css_demo.py`
- `examples/css_widget_parts_demo.py`

These examples show global type rules, class selectors, direct-child selectors,
pseudo-states, text inheritance, table metrics, widget parts, inline part
styles, and live stylesheet switching.

For a quick native smoke check of every CSS demo, run:

```powershell
python tools\smoke_css_demos.py
```

The smoke tool renders each demo for a few frames and prints only status,
renderer timing, stylesheet rule counts, stylesheet warnings, and a lightweight
layout sanity count derived from `debug_snapshot()`.

Useful options:

```powershell
python tools\smoke_css_demos.py --strict-layout
python tools\smoke_css_demos.py --no-layout-audit
```

`--strict-layout` fails the command if the snapshot audit sees likely clipping
or container overflow. `--no-layout-audit` keeps the check to renderer startup
and stylesheet warnings only.
