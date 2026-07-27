# DragonGUI CSS Styling

DragonGUI supports a deliberate CSS subset for styling native widgets. CSS is
parsed by the Rust backend, lowered into DragonGUI-owned style rules, and
compiled into the same `NodeStyle` model used by inline `style={...}` maps.

The goal is familiar styling for DragonGUI widgets, not browser compatibility.
There is no DOM, no HTML, no JavaScript, and no general browser layout engine.

## Current CSS Limits

DragonGUI CSS intentionally keeps a native-widget scope. These browser features
are not part of the current public subset:

- Full browser container queries. DragonGUI supports only the first-slice
  width model described below.
- Remote font URLs, WOFF2 loading, and packaged font asset resolution.
- `background-image: url(...)`.
- True sampled-framebuffer backdrop blur.
- Full browser stacking contexts.
- Multiple simultaneous animations, animation composition, timelines, and
  layout/text animation.
- CSS grid `subgrid` and nested auto-repeat.
- Arbitrary browser pseudo-elements, user-defined renderer parts, CSS-generated
  nested widgets, and CSS-created hit-test regions.
- Scatter3D data-bearing plot controls such as axis label text, colormaps,
  scalar bars, and point data remain Python/API driven.

For the full inventory, see `docs/css-capabilities-reference.md`.

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
height, aspect ratio, resolution, device size, viewport segments,
color-depth capability, orientation, pointer/hover capability,
navigation-control capability, scan mode, grid-device capability,
environment blending mode, overflow capability,
display update capability, scripting availability, forced-colors mode,
contrast preference, inverted-colors mode, dynamic range capability,
display mode, color-scheme preference, reduced-motion preference, reduced-transparency
preference, and reduced-data preference are evaluated from the logical
viewport, active theme, current desktop input/display assumptions, and window
scale factor, and stylesheets are reapplied on window resize.

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

@media (min-aspect-ratio: 4/3) {
    Panel.dashboard {
        grid-template-columns: 280px 1fr;
    }
}

@media (min-resolution: 2dppx) {
    Button.icon {
        border-width: 1px;
    }
}

@media (-webkit-device-pixel-ratio >= 1) {
    Button.icon {
        border-color: accent;
    }
}

@media (device-width >= 900px) and (device-aspect-ratio >= 4/3) {
    Panel.dashboard {
        border-radius: 14px;
    }
}

@media (horizontal-viewport-segments: 1) and (vertical-viewport-segments: 1) {
    Badge.native {
        outline: 1px solid rgba(90, 169, 255, 0.18);
    }
}

@media (color-gamut: srgb) {
    Badge.live {
        border-color: rgba(255, 255, 255, 0.24);
    }
}

@media (video-color-gamut: srgb) {
    Image.preview {
        border-color: rgba(90, 169, 255, 0.2);
    }
}

@media (color >= 8) and (monochrome: 0) {
    Panel.dashboard {
        border-color: rgba(90, 169, 255, 0.24);
    }
}

@media (color-index: 0) {
    Badge.live {
        opacity: 0.96;
    }
}

@media (scan: progressive) and (environment-blending: opaque) {
    Panel.hero {
        outline: 1px solid rgba(255, 255, 255, 0.16);
    }
}

@media (grid: 0) {
    Badge.live {
        outline: 1px solid rgba(116, 221, 176, 0.18);
    }
}

@media (pointer: fine) and (hover: hover) {
    Button:hover {
        box-shadow: 0 8px 18px rgba(90, 169, 255, 0.28);
    }
}

@media (nav-controls: none) {
    Button.back {
        display: flex;
    }
}

@media (overflow-block: scroll) and (overflow-inline: scroll) {
    Panel.dashboard {
        outline: 1px solid rgba(116, 221, 176, 0.2);
    }
}

@media (update: fast) {
    Badge.live {
        box-shadow: 0 0 0 1px rgba(116, 221, 176, 0.24);
    }
}

@media (scripting: none) {
    Badge.native {
        border-color: accent;
    }
}

@media (forced-colors: none) {
    Button.ghost {
        color: text;
    }
}

@media (prefers-contrast: no-preference) {
    Panel.hero {
        box-shadow: 0 18px 44px rgba(0, 0, 0, 0.34);
    }
}

@media (inverted-colors: none) {
    Badge.status {
        border-color: success;
    }
}

@media (dynamic-range: standard) {
    Panel.preview {
        border-color: rgba(255, 255, 255, 0.18);
    }
}

@media (video-dynamic-range: high) {
    Image.preview {
        box-shadow: 0 0 0 1px accent;
    }
}

@media (display-mode: standalone) {
    Button.launch {
        outline: 1px solid accent;
    }
}

@media (prefers-reduced-motion: reduce) {
    Badge.live {
        animation-play-state: paused;
    }
}

@media (prefers-reduced-transparency: no-preference) {
    Panel.glass {
        backdrop-filter: blur(12px);
    }
}

@media (prefers-reduced-data: no-preference) {
    Badge.live {
        animation-play-state: running;
    }
}

@media (prefers-color-scheme: dark) {
    Panel.hero {
        border-color: rgba(255, 255, 255, 0.22);
    }
}
```

## Container Queries

DragonGUI supports a first slice of named width container queries. A widget must
opt in as a query container with `container-type: inline-size`; optional
`container-name` identifiers let `@container` target a named ancestor. Queries
match against the nearest eligible ancestor using the previous layout pass, with
one bounded extra layout pass on startup so static documents settle
deterministically.

Supported forms:

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

@container (inline-size >= 520px) {
    Panel.summary {
        grid-template-columns: 1fr 1fr;
    }
}
```

Limitations:

- Only `width` and `inline-size` length comparisons are supported.
- Query values must be absolute lengths convertible to pixels.
- Height, block-size, aspect-ratio, orientation, style queries, scroll-state
  queries, nested `@container`, and container query units such as `cqw` are not
  supported yet.

Animation shorthand and longhand comma lists use the first item in DragonGUI's
one-animation slice. Finite fractional `animation-iteration-count` values are
preserved. Negative `animation-delay` values are accepted and start animations
partway through; multiple simultaneous animations are still unsupported.

Supported media features are `width`, `height`, `aspect-ratio`, `resolution`,
`-webkit-device-pixel-ratio`, `-moz-device-pixel-ratio`,
`device-width`, `device-height`, `device-aspect-ratio`,
`horizontal-viewport-segments`, `vertical-viewport-segments`,
`color`, `color-index`, `monochrome`, `color-gamut`, `video-color-gamut`,
`orientation`, `pointer`, `any-pointer`, `hover`, `any-hover`, `nav-controls`,
`scan`, `grid`, `environment-blending`, `overflow-block`, `overflow-inline`,
`update`, `scripting`, `forced-colors`, `prefers-contrast`,
`inverted-colors`, `dynamic-range`, `video-dynamic-range`,
`display-mode`, `prefers-color-scheme`, `prefers-reduced-motion`,
`prefers-reduced-transparency`, and `prefers-reduced-data`;
comma-separated query lists, `and`, `or`, `not`, and range syntax are accepted
when the resulting conditions only use those features. `prefers-color-scheme`
uses the platform window theme when winit can report one, and falls back to the
active theme background when platform detection is unavailable. `color-gamut`
currently defaults to `srgb` until platform display-gamut detection is added.
The prefixed device-pixel-ratio aliases map to DragonGUI's dppx resolution.
`video-color-gamut` currently mirrors `color-gamut` and defaults to `srgb`.
`device-width`, `device-height`, and `device-aspect-ratio` currently mirror the
logical app viewport; both viewport segment features default to `1`.
`color` defaults to `8`, while `color-index` and `monochrome` default to `0`
for normal native color displays. `scan` defaults to `progressive`, `grid`
defaults to `0`, and `environment-blending` defaults to `opaque`. `pointer` /
`any-pointer` currently default to `fine`, `hover` / `any-hover` default to
`hover`, `nav-controls` defaults to `none`, `overflow-block` and
`overflow-inline` default to `scroll`, `update`
defaults to `fast`, `scripting` defaults to `none`, and
`forced-colors` defaults to `none`, matching DragonGUI's current desktop native
rendering model. DragonGUI currently defaults `prefers-contrast`,
`prefers-reduced-motion`, `prefers-reduced-transparency`, and
`prefers-reduced-data` to `no-preference`, `inverted-colors` to `none`, and
both dynamic-range features to `standard`; `display-mode` defaults to
`standalone` for DragonGUI's native app window, until platform mode hooks are added.
Broader media features remain unsupported. Container queries are limited to the
first-slice width model above.

## Font Faces

DragonGUI supports a first slice of `@font-face` for installed local font
families, local font files, and base64 `data:` font URLs.

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

Supported sources are `local("Family Name")` entries, local path or `file://`
`url(...)` files with `.ttf`, `.otf`, `.ttc`, or `.woff` extensions, and base64
`data:` URLs containing sfnt TrueType/OpenType/TTC or WOFF1 data. WOFF1 sources
are decoded to sfnt data before loading. The optional `format(...)` descriptor
is honored; unsupported formats such as `woff2`, `embedded-opentype`, and `svg`
are skipped. Remote URLs, bundled package asset resolution, and WOFF2 loading
are not supported yet. Relative paths are resolved from the current process
working directory. Percent-escaped `file://` paths are decoded before loading.
Missing families, missing files, and unsupported font sources emit one-time
renderer diagnostics and appear in
`debug_snapshot()["gpu"]["renderer"]["font_warnings"]`.

DragonGUI also supports a first slice of static `@supports` feature queries.
Declaration queries use the DragonGUI property parser, selector queries use the
DragonGUI selector subset, `font-format(...)` reflects DragonGUI's current
font loader formats, including the `ttf`, `otf`, and `ttc` aliases for local
file sources. `at-rule(...)` reflects DragonGUI's current at-rule parser
surface, `font-tech(...)` reflects DragonGUI's current text shaping features,
and false queries skip their nested rules.

```css
@supports (display: grid) and (selector(Panel > Button.primary)) {
    Panel.dashboard {
        display: grid;
    }
}

@supports (backdrop-filter: blur(8px) brightness(110%) saturate(1.1)) {
    Panel.floating {
        backdrop-filter: blur(8px) brightness(110%) saturate(1.1);
    }
}

@supports font-format(woff) {
    Label.caption {
        letter-spacing: 0.015em;
    }
}

@supports at-rule(@media) {
    Badge.info {
        border-radius: 9px;
    }
}

@supports font-tech(features-opentype) {
    Label.value {
        font-variant-numeric: tabular-nums;
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
Constructor-generated widget defaults participate at framework/default
precedence rather than inline precedence. Consequently, application CSS can
replace defaults such as a composite's direction or preferred width. Only
values supplied through the public `style={...}` argument receive inline
precedence.

Inline color values accept theme tokens, hex colors, `transparent`, all
standard CSS named colors, and practical `rgb()`, `rgba()`, `hsl()`, `hsla()`, `hwb()`,
`lab()`, `lch()`, `oklab()`, `oklch()`, `color(srgb ...)`, and
`color(srgb-linear ...)` strings.
Python `Theme` color fields accept the same literal color strings, without
theme tokens or `var(...)`.

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
Panel:has(Button.primary) { ... } /* descendant presence */
Panel:has(> Button.primary) { ... } /* direct child presence */
Panel:has(> Button:first-child) { ... } /* direct child structural presence */
Panel:has(> Label:only-child) { ... } /* direct only-child structural presence */
Panel:has(> Panel:empty) { ... } /* direct empty child presence */
Panel:has(HLayout > Badge) { ... } /* descendant child-chain presence */
Panel:has(+ Button.primary) { ... } /* next sibling presence */
Panel:has(~ Badge.success) { ... } /* later sibling presence */
Panel:has(> Checkbox:checked) { ... } /* data-backed state presence */
Panel:has(Panel:has(Button.primary)) { ... } /* nested target presence */
Panel:has(Panel:has(> Badge.success) > Button.primary) { ... } /* nested ancestor-chain presence */
Button:hover { ... }            /* pseudo-state */
Button:not(.ghost) { ... }      /* selector function */
Panel > *:nth-child(3n+1) { ... } /* structural formula */
Panel > *:nth-last-child(2) { ... } /* reverse structural formula */
Panel > Panel:empty { ... }       /* no child widgets */
Panel > Label:only-child { ... } /* structural only child */
Panel > *:nth-child(2 of Panel > Button.primary) { ... } /* filtered structural formula */
Panel > *:nth-last-child(1 of Button.primary) { ... } /* reverse filtered structural formula */
Panel > *:nth-child(1 of Button:first-child) { ... } /* structural filtered formula */
Panel > *:nth-child(1 of Checkbox:checked) { ... } /* data-backed state filter */
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
`state`, `size`, `disabled`, `checked`, `expanded`, and `open`. Supported operators are
presence, exact (`=`), word (`~=`), prefix (`^=`), suffix (`$=`), substring
(`*=`), and dash-match (`|=`). Value selectors support ASCII case flags:
`i` for case-insensitive matching and `s` for case-sensitive matching.
Boolean attributes are present only when true.
`:empty` matches widgets with no child widgets; display text and scalar props
do not count as children.
`:nth-last-child(...)` accepts the same integer, odd/even, `an+b`, and
`of <selector-list>` forms as `:nth-child(...)`, but counts from the end of
the sibling list.

Unsupported selector forms produce warnings and are ignored. Examples:

- Broader `:has(...)` stateful and widget-part arguments. The supported slice
  checks descendant compound and selector-chain arguments, data-backed target
  state pseudos (`:disabled`, `:checked`, `:open`, `:expanded`, and
  `:collapsed`), nested `:has(...)` on the argument target and ancestor-side
  compounds in descendant or direct-child argument chains, structural target
  pseudos such as `:first-child`, `:last-child`, `:only-child`, `:empty`,
  and `:nth-last-child(...)`,
  and leading `>`, `+`, or `~` for direct children or following siblings.
  Dynamic target state pseudos such as `:hover`, `:active`, `:focus`, and
  `:selected`, widget-part
  arguments, and sibling-relative nested `:has(...)` on an ancestor side of an
  argument selector chain remain unsupported.
- Ancestor pseudo-states inside selector functions, such as `Panel:is(:hover) Button`.
- Browser pseudo-elements other than DragonGUI's first-slice generated
  `::before` and `::after`.

## Widget Type Names

Type selectors use the stable public Python widget names, not native
`render_kind` implementation names. Matching follows the public base-type
chain, so a composite or subclass matches both its most-specific public type
and its public bases:

```python
picker = dg.ColorPicker(parent=None)
print(picker.css_types())
# ("ColorPicker", "Panel", "Container", "Widget")
```

```css
ColorPicker { width: 360px; } /* only ColorPicker composites */
Panel { padding: 12px; }      /* also matches ColorPicker through Panel */
```

The same rule applies to framework composites: `SearchBox` also matches
`HLayout`, `Toolbar` matches `HLayout`, and `AppShell` matches `FlexLayout`.
Prefer the most-specific public type when styling one widget family. Base-type
rules are useful for intentional shared behavior.

Representative public type selectors include:

- `Window`
- `AppShell`
- `Body`
- `FlexLayout`
- `HLayout`
- `VLayout`
- `FlowLayout`
- `GridLayout`
- `ScrollArea`
- `WorkbenchLayout`
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
- `LED`
- `Button`
- `TextInput`
- `SearchBox`
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
- `ColorPicker`
- `PropertyGrid`
- `Property`

Type names are exact and case-sensitive. Private helper classes whose Python
names start with `_` are suppressed. Public subclasses may declare a stable
`CSS_TYPE`, add `CSS_TYPE_ALIASES`, or set `CSS_TYPE = None` to inherit only
their public base types.

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
| `flex-wrap` | `nowrap`, `wrap`, `wrap-reverse` |
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
| `margin` | one to four logical-pixel, percent, `auto`, or compatible `calc()` values |
| `margin-left` | logical pixels, percent, `auto`, compatible `calc()` |
| `margin-right` | logical pixels, percent, `auto`, compatible `calc()` |
| `margin-top` | logical pixels, percent, `auto`, compatible `calc()` |
| `margin-bottom` | logical pixels, percent, `auto`, compatible `calc()` |
| `gap` | logical pixels, percent, compatible `calc()` |
| `row-gap` | logical pixels, percent, compatible `calc()` |
| `column-gap` | logical pixels, percent, compatible `calc()` |
| `grid-template-columns` | first-slice grid track list |
| `grid-template-rows` | first-slice grid track list |
| `grid-template-areas` | quoted named grid-area rows |
| `grid-auto-flow` | `row`, `column`, optional `dense` |
| `grid-area` | named grid area for a child |
| `grid-column` | line or span placement |
| `grid-row` | line or span placement |
| `overflow` | `visible`, `hidden`, `scroll`, `auto` |
| `overflow-x` | `visible`, `hidden`, `scroll`, `auto` |
| `overflow-y` | `visible`, `hidden`, `scroll`, `auto` |
| `position` | `static`, `relative`, `absolute`, `fixed` |
| `top`, `right`, `bottom`, `left` | logical pixels |
| `z-index` | integer sibling stacking hint |

Percent lengths and first-slice `calc()` are supported for sizing, padding,
margin, and gap properties above. `auto` is supported for sizing and margin;
padding and gap reject `auto`. `calc()` currently supports
addition, subtraction, and simple scalar multiply/divide for pixel and percent
terms, such as `calc(220px + 40px)`, `calc(20% + 30%)`, or
`calc(100% - 240px)`. Mixed percent/pixel expressions resolve when the parent
axis has a definite size.

First-slice CSS Grid supports `display: grid`, track lists using px, percent,
`fr`, `auto`, `minmax()`, `fit-content()`, nested finite `repeat(n, ...)`, and
non-nested `repeat(auto-fit, ...)` / `repeat(auto-fill, ...)`, plus
`grid-column` and `grid-row` placements such as `1`, `2 / 4`, and
`1 / span 2`. Named `grid-template-areas` and child `grid-area` placement are
also supported for rectangular named regions. `grid-auto-flow` supports
`row`, `column`, `row dense`, `column dense`, and `dense` shorthand.
`subgrid` and nested auto-repeat are not supported yet.

First-slice overflow supports explicit clipping with `hidden`, child escape
with `visible`, and scroll opt-in with `auto` or `scroll`. `overflow-x`
containers can scroll horizontally with horizontal wheel input or shift-wheel;
`overflow-y` containers use the vertical wheel path. Scroll containers draw
vertical and horizontal overlay scrollbar indicators when their opted-in axes
overflow; panels additionally keep the indicators inside rounded corners and
leave the corner clear when both axes overflow.
Users can drag the thumb, click the track, or use PageUp/PageDown/Home/End when
keyboard focus is inside a scroll container. Shift changes those keys to the
horizontal axis when horizontal overflow exists. Scrollable layout/container
widgets expose `::scrollbar-track` and `::scrollbar-thumb`; the track part
supports `width` plus uniform `padding` for the axis inset.

First-slice positioning supports paint-only `relative` offsets,
layout-backed `absolute` children inside their parent, and viewport-backed
`fixed` widgets. Fixed widgets are removed from normal flow and clip against the
window rather than the parent container.

## Visual Properties

Supported visual properties:

| CSS | Notes |
| --- | --- |
| `background` / `background-color` | theme token, color, gradient, `blob-gradient(...)`, `mesh-gradient(...)`, or layered paint |
| `background-image` | `linear-gradient(...)`, `radial-gradient(...)`, `blob-gradient(...)`, `mesh-gradient(...)`, repeating gradients, layered gradients, or `none`; layers over `background-color`; `url(...)` is not supported |
| `background-noise` | subtle procedural noise for rect-backed gradient backgrounds |
| `gradient-interpolation` | `srgb`, `linear-srgb`, or `oklab`; defaults to `srgb` |
| `foreground` | theme token or color |
| `color` | text color and foreground |
| `border-color` | theme token or color |
| `border-width` | logical pixels |
| `border-style` | `solid`, `none`, or `hidden`; uniform only |
| `border-radius` | logical pixels |
| `border-top-left-radius` | logical pixels |
| `border-top-right-radius` | logical pixels |
| `border-bottom-right-radius` | logical pixels |
| `border-bottom-left-radius` | logical pixels |
| `border` | `none`, `0`, or `<width> solid <color>` |
| `outline` | `none`, `0`, or `<width> solid <color>` |
| `outline-color` | theme token or color |
| `outline-width` | logical pixels |
| `outline-style` | `solid`, `none`, or `hidden`; uniform only |
| `outline-offset` | logical pixels; negative values clamp to zero |
| `box-shadow` | comma-separated outset or `inset` soft shadow layers |
| `opacity` | `0.0` to `1.0` |
| `accent` | widget accent color |
| `track-color` | slider/progress track color |
| `thumb-color` | slider thumb color |
| `transform` | paint-only `translate(...)`, `scale(...)`, `rotate(...)` shorthand |
| `translate` | paint-only one or two logical-pixel offsets |
| `scale` | paint-only one or two numeric scale factors |
| `rotate` | paint-only angle |

Transforms do not affect layout or hit testing. Descendant rect surfaces,
scrollbars, and image textures follow transformed widget subtrees. Text follows
translate and uniform scale; text rotation is still unsupported.

Supported color forms:

- Theme tokens such as `surface`, `accent`, `border`, `danger`.
- Derived tokens such as `accent_mix_20` and `accent_dark`.
- `#RGB`
- `#RGBA`
- `#RRGGBB`
- `#RRGGBBAA`
- `transparent`
- All standard CSS named colors.
- `rgb(...)` / `rgba(...)`
- `hsl(...)` / `hsla(...)`
- `hwb(...)`
- `lab(...)` / `lch(...)` / `oklab(...)` / `oklch(...)`
- `color(srgb ...)` / `color(srgb-linear ...)`

## Text Properties

Supported text properties:

| CSS | Notes |
| --- | --- |
| `font-size` | logical pixels |
| `font-family` | `serif`, `sans-serif`, `monospace`, or named family |
| `font-weight` | `normal`, `bold`, or numeric `100` to `900` |
| `color` | theme token or color |
| `text-align` | `left`, `center`, `right` |

Text properties inherit down the widget tree. Layout and visual properties do
not inherit.

## Widget Properties

Supported widget-specific properties:

| CSS | Widget |
| --- | --- |
| `text-area-rows` | `TextArea` |
| `scatter-point-size` | `Scatter3D` |
| `scatter-point-style` | `Scatter3D` |
| `scatter-grid-visible` | `Scatter3D` |
| `scatter-grid-planes` | `Scatter3D` |
| `scatter-legend-position` | `Scatter3D` |
| `scatter-orientation-axes` | `Scatter3D` |
| `table-row-height` | `DataFrameTable` |
| `table-header-height` | `DataFrameTable` |
| `table-column-width` | `DataFrameTable` |
| `table-index-width` | `DataFrameTable` |

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
| `HLayout` | `scrollbar-track`, `scrollbar-thumb` |
| `VLayout` | `scrollbar-track`, `scrollbar-thumb` |
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
| `LED` | `dot`, `glow`, `highlight` |
| `Slider` | `track`, `fill`, `thumb` |
| `ProgressBar` | `track`, `fill`, `label` |
| `Tabs` | `header` |
| `Tab` | `tab`, `accent`, `badge` |
| `NavItem` | `item`, `accent`, `badge` |
| `DataFrameTable` | `header`, `row`, `row-selected`, `grid-line` |
| Most rendered widgets | `before`, `after` generated content |

Part styles support the same visual and text properties as widgets.
`Panel::accent` is rendered as a left-side fill slice clipped to the panel's
inner rounded shape, so its `width` can change without manually matching the
panel corner radius. `Modal::scrim` styles the full-screen overlay behind the
modal surface. `LED::dot` styles the visible light body, `LED::glow` styles the
halo behind on states, and `LED::highlight` styles the small specular mark; set
`LED::glow { opacity: 0; }` to hide the halo, or `box-shadow: none` to suppress
the built-in soft shadow. A `DataFrameTable` with `border-radius` clips its header, rows,
selection, grid lines, and border to the table's rounded shape, which keeps
tables clean inside rounded panels. `Image` textures are clipped to the image
widget's rounded content box inside the border. Dropdown and menu item fills
are clipped to their rounded popup bounds.

```css
LED.busy::dot {
    width: 15px;
    height: 15px;
    background: warning;
    border: 1px solid rgba(94, 58, 0, 0.82);
    border-radius: 5px;
}

LED.busy::glow {
    width: 27px;
    height: 27px;
    background: warning;
    opacity: 0.18;
    box-shadow: none;
}

LED.busy::highlight {
    width: 5px;
    height: 3px;
    background: rgba(255, 255, 255, 0.72);
}
```

`::before` and `::after` are generated-content hooks, not real widget parts.
They support `content: "..."`, `content: attr(name)`, plus visual/text styling
for non-interactive prefix or suffix text. `attr(name)` reads widget metadata
such as `id`, `key`, `class`, and serialized widget props such as `title`,
`level`, or `value`. Generated content does not participate in layout, create
hit targets, or support counters. Controls align generated content through the
control centerline; titled containers such as `Panel`, `Sidebar`, and `Modal`
anchor generated content to the title band so stamps do not float over child
controls.

Rounded `Scatter3D` surfaces clip the 3D viewport and picking region to the
computed per-corner border radii. Overflow support is first-slice: horizontal
scrolling works for opted-in scroll containers, and overlay scrollbar
indicators render for overflowing scroll axes. Outset shadows inside
scroll/overflow containers are clipped to the inherited paint viewport without
shrinking the shadow to the visible portion of the widget.

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

Framework and theme `:root` variables are available to later stylesheet
origins. DragonGUI exposes the active theme spacing scale as:

- `--dg-spacing`
- `--dg-space-xs` (`spacing × 0.5`)
- `--dg-space-sm` (`spacing × 1`)
- `--dg-space-md` (`spacing × 2`)
- `--dg-space-lg` (`spacing × 3`)
- `--dg-space-xl` (`spacing × 4`)

These values are regenerated when the framework theme is installed, including
when application CSS was parsed first.

V2 supports global `:root` variables and a first slice of media/support-scoped
`:root` variables. A variable declared directly inside a matching `@media` or
static true `@supports` block can be used by declarations inside that same
block:

```css
@media (min-width: 900px) {
    :root {
        --card-min: 180px;
    }

    Panel.dashboard {
        grid-template-columns: repeat(2, minmax(var(--card-min), 1fr));
    }
}
```

Selector-local custom properties are also supported inside the same declaration
block:

```css
Panel.card {
    --card-bg: linear-gradient(135deg, #172235, #0f1724);
    --card-radius: 14px;
    background: var(--card-bg);
    border-radius: var(--card-radius);
}
```

Inherited custom properties across arbitrary widget subtrees are not
implemented. `var()` may be used as a whole property value or inside larger
parseable values such as borders, shadows, gradients, grid tracks, and
transition shorthands.

## Debugging

`app.debug_snapshot()` includes stylesheet and computed-style data:

- framework/theme/user rule counts
- stylesheet warning count
- last stylesheet error
- public `render_kind` and `css_types` identity per widget
- matched selectors with origin, source location/order, and specificity
- winning and overridden declarations per normalized property
- unmatched eligible user selectors
- matched rules and computed fields per styled widget part
- computed layout, visual, text, and widget style fields

For example:

```python
snapshot = app.debug_snapshot()
button = snapshot["computed_styles"]["run-button"]

print(button["matched_selectors"])
print(button["provenance"]["background"]["winner"])
print(button["provenance"]["background"]["overridden"])
print(snapshot["stylesheets"]["unmatched_user_selectors"])
```

### Troubleshooting an unmatched selector

1. Give the target a stable `id` and inspect its `css_types` in the snapshot.
2. Use the exact, case-sensitive public name (`SearchBox`, not `search_box`,
   `text_input`, or a native render-kind name).
3. Check `unmatched_user_selectors`. Selectors hidden behind an inactive
   media/container query are not eligible until that condition is active.
4. Check stylesheet warnings for unsupported selector syntax or widget parts.
5. If the selector matched but the value did not win, inspect the property's
   `provenance`: `!important`, origin, specificity, and source order determine
   the winner, with explicit inline styles applied last.
6. For a composite, target its public outer type for outer geometry and a
   documented `::part` or descendant class only when styling an internal piece.

Unsupported CSS does not crash the app. The parser records warnings, skips the
unsupported declaration or selector, and continues applying the rest of the
stylesheet.

For layout primitive selection, responsive card grids, scrollable titled panels,
and clipping diagnostics, see `docs/layout.md`.

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
