# Styling

DragonGUI has a CSS-inspired styling layer for native widgets. It is meant to
feel familiar if you know CSS, but it is not browser CSS: there is no DOM, HTML
box model, JavaScript, or browser layout engine. Styles are parsed by the native
backend, lowered into DragonGUI style rules, and merged with inline
`style={...}` dictionaries.

Use CSS for app-wide visual policy. Use live widget methods for data-bearing
state such as plot data, table frames, active selections, and Scatter3D
colormaps.

## Loading Stylesheets

Attach stylesheets through the app:

```python
import dragongui as dg

app = dg.App()

app.stylesheet("""
Window {
    background: background;
}

Panel.sidebar {
    width: 280px;
    background: surface;
    border-color: border;
}

Button.primary {
    background: accent;
    color: white;
    border-radius: 6px;
}

Button.primary:hover {
    background: accent_dark;
}
""")
```

You can also load UTF-8 CSS from disk:

```python
app.load_stylesheet("styles/app.dg.css")
```

While an app is running, `app.stylesheet(...)`, `app.load_stylesheet(...)`, and
`app.clear_stylesheets()` enqueue native stylesheet commands and restyle the
retained widget tree without rebuilding the Python document.

## Cascade

DragonGUI resolves style sources in this order, lowest to highest precedence:

1. Built-in framework stylesheet.
2. Theme stylesheet state.
3. User stylesheets loaded through Python.
4. Inline `style={...}` dictionaries.

Within stylesheet rules, the cascade key is:

```text
!important, origin, specificity, source order
```

Later rules with the same origin and specificity win. Inline styles are merged
after stylesheets, so they remain the strongest local override.

## Selectors

Type selectors use DragonGUI widget names exactly, such as `Button`,
`ProgressBar`, `DataFrameTable`, and `Scatter3D`. They are case-sensitive.
They match the stable public base-type chain rather than private native
render-kind names. Thus `ColorPicker` matches `ColorPicker` and `Panel`,
`SearchBox` matches `SearchBox` and `HLayout`, and `AppShell` matches
`AppShell` and `FlexLayout`.

Common selector forms:

```css
* { opacity: 0.98; }
Button { border-radius: 6px; }
.primary { background: accent; }
Button.primary.danger { background: danger; }
#run-button { border-color: success; }
[key="main-action"] { outline: 1px solid accent; }
[disabled] { opacity: 0.45; }
[text^="Run"] { font-weight: 700; }
Panel Button { font-size: 13px; }
Panel.controls > Button { width: 100%; }
Button:hover { background: accent_mix_20; }
Button:not(:disabled) { color: text; }
Panel:has(> Checkbox:checked) { border-color: success; }
Panel > *:nth-child(odd) { opacity: 0.96; }
```

Supported widget states include:

- `:hover`
- `:active`
- `:focus`
- `:disabled`
- `:checked`
- `:open`
- `:expanded`
- `:collapsed`
- `:selected`

`class_` values are split on whitespace:

```python
dg.Button("Delete", class_="primary danger")
```

That widget can match `.primary`, `.danger`, or
`Button.primary.danger`.

Attribute selectors can match stable widget metadata and scalar props,
including `id`, `key`, `type`, `class`, `text`, `badge`, `level`,
`placeholder`, `value`, `page`, `orientation`, `target`, `tooltip`, `path`,
`fit`, `state`, `width`, `height`, `size`, `min`, `max`, `step`, `disabled`,
`checked`, `expanded`, `open`, `wrap`, `rows`, `page-size`, `table-rows`, and
`items-count`.

## Widget Parts

Parts are renderer-owned styling hooks for pieces drawn inside a widget. They
are not child widgets, do not receive callbacks, and do not create separate
layout nodes.

Part selectors use `Widget::part`:

```css
ProgressBar::track {
    background: surface_alt;
}

ProgressBar::fill {
    background: success;
}

DataFrameTable::header {
    background: surface_alt;
    font-weight: 700;
}

NumberInput::stepper-up {
    background: accent_mix_20;
}

Checkbox:checked::indicator {
    background: success;
}
```

Frequently used parts:

| Widget | Parts |
| --- | --- |
| `Panel` | `accent`, `scrollbar-track`, `scrollbar-thumb` |
| `Collapsible` | `header`, `indicator`, `body`, `scrollbar-track`, `scrollbar-thumb` |
| `Modal` | `scrim`, `scrollbar-track`, `scrollbar-thumb` |
| `Menu`, `ContextMenu` | `menu`, `item`, `item-hover`, `item-disabled` |
| `Button`, `SmallButton` | `badge` |
| `IconButton`, `ArrowButton` | `icon` |
| `ImageButton` | `image` |
| `NumberInput` | `field`, `stepper`, `stepper-up`, `stepper-down`, `stepper-divider`, `divider`, `caret` |
| `DragNumber` | `field`, `value`, `grip` |
| `Dropdown` | `field`, `chevron`, `menu`, `item`, `item-selected`, `item-hover` |
| `Checkbox` | `row`, `box`, `indicator`, `label` |
| `ToggleSwitch` | `row`, `track`, `thumb`, `label` |
| `Slider` | `track`, `fill`, `thumb` |
| `RangeSlider` | `track`, `range`, `thumb-min`, `thumb-max`, `label` |
| `ProgressBar` | `track`, `fill`, `label` |
| `LoadingSpinner` | `track`, `arc`, `label` |
| `LED` | `dot`, `glow`, `highlight` |
| `Tabs` | `header` |
| `Tab` | `tab`, `accent`, `badge` |
| `NavItem` | `item`, `accent`, `badge` |
| `TreeNode` | `row`, `indicator`, `label`, `guide` |
| `DataFrameTable` | `header`, `row`, `row-selected`, `grid-line`, `scrollbar-track`, `scrollbar-thumb` |
| `Heatmap` | `cell`, `grid`, `hover`, `scalar-bar`, `label` |
| `BarChart` | `label`, `value-label` |

Most rendered widgets also support `::before` and `::after` for first-slice
generated text content:

```css
Panel.summary::after {
    content: "live";
    color: success;
    font-size: 11px;
}
```

Generated content is paint-only. It does not affect layout, receive input, or
create child widgets.

## Inline Styles

Inline styles are Python dictionaries using mostly snake-case keys:

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

Inline styles also support part maps:

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

Inline part names accept dashed and snake-case forms, so `stepper-up` and
`stepper_up` are equivalent.

## Properties

DragonGUI supports layout, visual, text, and widget-specific style properties.
The most common user-facing properties are:

| Group | Examples |
| --- | --- |
| Layout | `display`, `flex-direction`, `flex-wrap`, `flex`, `width`, `height`, `min-width`, `max-height`, `padding`, `margin`, `gap`, `overflow`, `position`, `top`, `right`, `bottom`, `left`, `z-index` |
| Grid | `grid-template-columns`, `grid-template-rows`, `grid-template-areas`, `grid-auto-flow`, `grid-area`, `grid-column`, `grid-row` |
| Visual | `background`, `background-color`, `background-image`, `border`, `border-color`, `border-width`, `border-radius`, `outline`, `box-shadow`, `opacity`, `accent`, `track-color`, `thumb-color`, `transform`, `translate`, `scale`, `rotate` |
| Text | `font-size`, `font-family`, `font-weight`, `font-style`, `color`, `text-align` |
| Tables | `table-row-height`, `table-header-height`, `table-column-width`, `table-index-width` |
| Scatter3D | `scatter-point-size`, `scatter-point-style`, `scatter-grid-visible`, `scatter-grid-planes`, `scatter-legend-position`, `scatter-orientation-axes` |

Text properties inherit down the widget tree. Layout and visual properties do
not inherit.

Colors can use theme tokens, derived tokens, hex values, standard CSS named colors,
and practical CSS color functions:

```css
Label.warning {
    color: warning;
}

Panel.card {
    background: linear-gradient(135deg, surface, surface_alt);
    border-color: rgba(255, 255, 255, 0.18);
}
```

Useful theme tokens include `background`, `surface`, `surface_alt`, `text`,
`foreground`, `muted_text`, `muted`, `accent`, `border`, `danger`, `warning`,
`success`, `focus`, and `disabled`.

## Variables

Global custom properties can be declared in `:root`:

```css
:root {
    --card-radius: 10px;
    --brand: #2f6fed;
}

Panel.card {
    border-radius: var(--card-radius);
    border-color: var(--brand);
}
```

Selector-local custom properties can be used inside the same declaration block:

```css
Panel.metric {
    --metric-bg: linear-gradient(135deg, #172235, #0f1724);
    background: var(--metric-bg);
}
```

DragonGUI does not yet implement full browser-style inherited custom properties
across arbitrary widget subtrees.

## Responsive Rules

DragonGUI supports first-slice `@media` rules for viewport, display, input, and
preference conditions:

```css
@media (max-width: 760px) {
    Panel.sidebar {
        display: none;
    }
}

@media (pointer: fine) and (hover: hover) {
    Button:hover {
        box-shadow: 0 8px 18px rgba(90, 169, 255, 0.28);
    }
}
```

It also supports first-slice width container queries:

```css
Panel.card {
    container-name: card;
    container-type: inline-size;
}

@container card (min-width: 360px) {
    Label.metric {
        font-size: 18px;
    }
}
```

Container queries currently compare `width` or `inline-size` against the
nearest eligible ancestor. Height, block-size, style queries, scroll-state
queries, and container query units are not part of the current slice.

## Fonts And Feature Queries

DragonGUI supports first-slice `@font-face` for installed local families, local
font files, `file://` URLs, and base64 `data:` font URLs containing sfnt or
WOFF1 data:

```css
@font-face {
    font-family: "Report UI";
    src:
        local("Segoe UI"),
        url("file:///C:/Windows/Fonts/segoeui.ttf") format("truetype");
}

Label.title {
    font-family: "Report UI";
}
```

Remote font URLs and WOFF2 loading are not supported yet.

Static `@supports` rules can gate declarations or selectors supported by the
DragonGUI CSS subset:

```css
@supports (display: grid) and (selector(Panel > Button.primary)) {
    Panel.dashboard {
        display: grid;
    }
}
```

## Debugging

Unsupported CSS does not crash the app. The parser records warnings, skips the
unsupported selector or declaration, and keeps applying the rest of the
stylesheet.

Use `app.debug_snapshot()` to inspect stylesheet state:

```python
snapshot = app.debug_snapshot()
print(snapshot["stylesheets"])
```

The snapshot includes rule counts, warning counts, the last stylesheet parse
error, public `render_kind`/`css_types`, matched selectors with source and
specificity, winning and overridden declarations, unmatched eligible user
selectors, computed style fields, and computed part styles.

To troubleshoot an unmatched selector, inspect the node's exact, case-sensitive
`css_types`, then check `unmatched_user_selectors` and parser warnings. If
the selector matched, inspect `provenance[property]` to see whether origin,
specificity, source order, `!important`, or an inline style supplied the winner.
Selectors inside inactive responsive conditions are not currently eligible.

For runtime help inside Python:

```python
import dragongui as dg

print(dg.help("styling.selectors"))
print(dg.help("reference.css_parts"))
print(dg.help("reference.css_properties"))
```

## Practical Limits

Keep these boundaries in mind:

- Browser-only features are not automatically supported.
- Remote fonts, WOFF2, and `background-image: url(...)` are not supported.
- CSS-created layout nodes, hit-test regions, and arbitrary pseudo-elements are
  not supported.
- `::before` and `::after` generated content is text-only and paint-only.
- Container queries are width-only.
- Animations and transitions are first-slice visual features.
- `transform` is paint-only and does not affect layout or hit testing.
- Scatter3D data, colormaps, scalar bars, and axis labels remain API-driven.

For exhaustive implementation inventories, see the root project notes
`../css-styling.md` and `../css-capabilities-reference.md`.
