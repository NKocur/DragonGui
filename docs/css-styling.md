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
Button { ... }                  /* type */
.primary { ... }                /* class */
Button.primary { ... }          /* type + class */
#run-button { ... }             /* explicit widget id */
Panel.controls > Button { ... } /* direct child */
Button:hover { ... }            /* pseudo-state */
```

Supported pseudo-states:

- `:hover`
- `:active`
- `:focus`
- `:disabled`

`class_` supports normal CSS whitespace splitting:

```python
dg.Button("Run", class_="primary danger")
```

Then either `.primary`, `.danger`, or `Button.primary.danger` can match.

Unsupported selector forms produce warnings and are ignored. Examples:

- Descendant selectors such as `Panel Button`.
- Attribute selectors such as `[key="main"]`.
- Universal selectors such as `*`.
- Multi-level child chains such as `Panel > HLayout > Button`.

## Widget Type Names

Type selectors use DragonGUI widget names:

- `Window`
- `HLayout`
- `VLayout`
- `Panel`
- `Modal`
- `MenuBar`
- `Menu`
- `MenuItem`
- `ContextMenu`
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

## Layout Properties

Supported layout properties:

| CSS | Notes |
| --- | --- |
| `display` | `flex`, `block`, `none` |
| `flex-direction` | `row`, `column`, `row-reverse`, `column-reverse` |
| `flex` | maps to `flex-grow` |
| `flex-grow` | non-negative number |
| `flex-shrink` | non-negative number |
| `width` | logical pixels only |
| `height` | logical pixels only |
| `min-width` | logical pixels only |
| `min-height` | logical pixels only |
| `max-width` | logical pixels only |
| `max-height` | logical pixels only |
| `padding` | one to four logical-pixel values |
| `padding-left` | logical pixels only |
| `padding-right` | logical pixels only |
| `padding-top` | logical pixels only |
| `padding-bottom` | logical pixels only |
| `margin` | uniform margin only |
| `gap` | logical pixels only |

Percent lengths and `auto` currently warn and are ignored for these properties.
DragonGUI uses Taffy internally, but the public CSS subset keeps V2 sizing
explicit until percent behavior is fully designed.

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
| `border` | `<width> solid <color>` only |
| `opacity` | `0.0` to `1.0` |
| `accent` | widget accent color |
| `track-color` | slider/progress track color |
| `thumb-color` | slider thumb color |

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
}

Checkbox:checked::indicator {
    background: success;
}
```

Pseudo-states are widget-level in this slice. For example,
`NumberInput:hover::stepper-up` means "when the whole number input is hovered,
style the upper stepper part."

Supported parts:

| Widget | Parts |
| --- | --- |
| `Panel` | `accent` |
| `NumberInput` | `stepper`, `stepper-up`, `stepper-down`, `stepper-divider` |
| `Dropdown` | `chevron`, `menu`, `item`, `item-selected`, `item-hover` |
| `Checkbox` | `row`, `box`, `indicator`, `label` |
| `Slider` | `track`, `fill`, `thumb` |
| `ProgressBar` | `track`, `fill`, `label` |
| `Tabs` | `header` |
| `Tab` | `tab`, `accent` |
| `NavItem` | `item`, `accent` |
| `DataFrameTable` | `header`, `row`, `row-selected`, `grid-line` |

Part styles support the same visual and text properties as widgets. `Panel::accent`
is rendered as a left-side fill slice clipped to the panel's inner rounded shape,
so its `width` can change without manually matching the panel corner radius. A
`DataFrameTable` with `border-radius` clips its header, rows, selection, grid
lines, and border to the table's rounded shape, which keeps tables clean inside
rounded panels. `Image` textures are clipped to the image widget's rounded
content box inside the border. Dropdown and menu item fills are clipped to their
rounded popup bounds.

General `overflow: hidden`, scroll containers, and rounded `Scatter3D` clipping
are not part of the current CSS slice. Scatter uses a rectangular 3D viewport
and needs a dedicated stencil/mask implementation for rounded clipping.

A small subset of layout properties is accepted where the renderer already has
matching geometry:

| Property | Common Use |
| --- | --- |
| `width` | panel accent fill width from the left edge, checkbox boxes/indicators, number steppers, dropdown chevrons, slider thumbs, nav accents, grid lines |
| `height` | checkbox boxes/indicators, slider tracks/thumbs, progress fills, tab headers, tab accents |
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
}
```

V2 supports global `:root` variables only. Scoped custom properties are not
implemented.

## Debugging

`app.debug_snapshot()` includes stylesheet and computed-style data:

- framework/theme/user rule counts
- stylesheet warning count
- last stylesheet error
- matched rules per widget
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
