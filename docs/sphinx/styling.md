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

For live appearance switching, use a stable name:

```python
app.set_stylesheet("appearance", windows_311_css)
app.set_stylesheet("appearance", mac_os_css)  # replaces in place
app.remove_stylesheet("appearance")
app.set_theme(dg.Theme.light())
```

Named replacement preserves source order and keeps the cascade bounded.
Theme changes preserve widget interaction state and distinguish layout-token
changes from paint-only changes. Active names and source order are visible in
runtime debug snapshots. A complete example is available in
`examples/runtime_theme_switching_demo.py`.

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

<!-- BEGIN GENERATED WIDGET CSS CAPABILITIES -->

_Generated from `python/dragongui/widget_css_capabilities.json`. Do not edit this table manually._

Global generated-content hooks: `::before`, `::after` (text renderer).

| Widget | Supported states | Parts and renderer support |
| --- | --- | --- |
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
| Visual | `background`, `background-color`, `background-image` (gradients, `dg-pattern`, managed `app-resource`), `border`, side-specific border shorthands/longhands, `border-color`, `border-width`, `border-style`, `border-radius`, `outline`, `outline-style`, `box-shadow`, `opacity`, `accent`, `track-color`, `thumb-color`, `transform`, `translate`, `scale`, `rotate` |
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

Gradient stops accept percentages, logical pixels, and mixed
`calc()` length-percentage values. Repeating gradients preserve logical-pixel
stripe widths as widgets resize and scale them with display DPI. Two-position
hard stops such as `red 1px 2px` are also supported.

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

Ordered fallback stacks are supported:

```css
Label {
    font-family: "Chicago", "Geneva", Arial, system-ui, sans-serif;
}
```

DragonGUI selects the first available family, including `@font-face` aliases,
and uses platform font-database mappings for generic families. Missing named
families are available through diagnostics without per-frame console warnings.

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
- Remote fonts, WOFF2, and arbitrary `background-image: url(...)` sources are
  not supported.
- CSS-created layout nodes, hit-test regions, and arbitrary pseudo-elements are
  not supported.
- `::before` and `::after` generated content is text-only and paint-only.
- Container queries are width-only.

For texture-free surfaces, use
`dg-pattern(checker, foreground, background, 8px)` in `background` or
`background-image`. The supported kinds are `checker`, `pinstripe`,
`dot`/`stipple`, and `diagonal-hatch`; tile sizes are bounded to `2px..128px`.
Patterns retain rounded clipping, opacity, transforms, layering, and DPI-aware
logical sizing.

For packaged PNG/JPEG surfaces, register bytes or a path in Python and refer to
the semantic ID from CSS:

```python
app.set_image_resource("linen", Path("assets/linen.png"))
```

```css
Panel { background-image: app-resource("linen", cover); }
```

Fits are `contain`, `cover`, `stretch`, and `repeat`. Registration is bounded
to 16 MiB encoded data; native decode is bounded to 4096×4096 and 16 million
pixels. CSS never receives filesystem or remote-loading authority.
- Animations and transitions are first-slice visual features.
- `transform` is paint-only and does not affect layout or hit testing.
- Scatter3D data, colormaps, scalar bars, and axis labels remain API-driven.

For exhaustive implementation inventories, see the root project notes
`../css-styling.md` and `../css-capabilities-reference.md`.
