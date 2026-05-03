# DragonGUI CSS Capabilities Reference

This document inventories the current DragonGUI CSS styling system for audit.
It focuses on what the implementation can do today, where each feature applies,
and which limitations are intentional.

DragonGUI CSS is a native-widget styling subset. It is not browser CSS: there
is no DOM, HTML parser, JavaScript, general browser layout engine, full media
query system, or full CSS compatibility promise. Animation support is limited
to a first slice of visual `@keyframes` properties.

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
- First-slice `@media` blocks for viewport
  width/height/aspect-ratio/resolution/device-pixel-ratio-aliases/device-size/viewport-segments/color/color-index/monochrome/
  color-gamut/video-color-gamut/orientation/scan/grid/environment-blending/pointer/hover/nav-controls/
  overflow/update/scripting/forced-colors/contrast/inverted-colors/
  dynamic-range/video-dynamic-range/display-mode/color-scheme/reduced-motion/
  reduced-transparency/reduced-data queries.
- First-slice `@supports` blocks for DragonGUI declaration and selector
  feature queries.
- First-slice `@keyframes` blocks for visual animation keyframes.
- First-slice `@font-face` blocks for installed local family names, local
  `.ttf`, `.otf`, `.ttc`, and `.woff` path or `file://` files, and base64
  `data:` URLs containing sfnt or WOFF1 font data.
- First-slice generated content through `::before`, `::after`, and
  `content: "..."`.

Ignored or unsupported:

- Remote font URLs, WOFF2 loading, and packaged font asset resolution are
  not part of the current public subset.
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

## Media Queries

First-slice `@media` supports viewport width, height, aspect-ratio,
resolution, `-webkit-device-pixel-ratio`, `-moz-device-pixel-ratio`,
device-width, device-height, device-aspect-ratio,
horizontal-viewport-segments, vertical-viewport-segments, color, color-index,
monochrome, color-gamut, video-color-gamut, orientation,
scan, grid, environment-blending, overflow-block, overflow-inline, update,
scripting, forced-colors, contrast, inverted-colors, dynamic-range,
video-dynamic-range, color-scheme, display-mode, reduced-motion,
reduced-transparency, and reduced-data queries in logical CSS pixels.
Pointer, hover, nav-controls, update, scripting, and forced-colors queries reflect
DragonGUI's current desktop native rendering model. Resolution uses the current
window scale factor as CSS dppx. Color-gamut currently defaults to `srgb`.
Color-scheme uses the platform
window theme when winit can report one, and falls back to the active theme
background when platform detection is unavailable. DragonGUI reapplies
stylesheets against the current window size, theme, scale, and platform theme
during layout, resize, and theme-change events.

Supported forms:

```css
@media (max-width: 760px) { ... }
@media (min-height: 600px) and (max-height: 900px) { ... }
@media screen and (width >= 900px), (height <= 520px) { ... }
@media (orientation: landscape) { ... }
@media (min-aspect-ratio: 4/3) { ... }
@media (min-resolution: 2dppx) { ... }
@media (-webkit-device-pixel-ratio >= 1) { ... }
@media (device-width >= 900px) and (device-aspect-ratio >= 4/3) { ... }
@media (horizontal-viewport-segments: 1) and (vertical-viewport-segments: 1) { ... }
@media (color >= 8) and (monochrome: 0) { ... }
@media (color-index: 0) { ... }
@media (color-gamut: srgb) { ... }
@media (video-color-gamut: srgb) { ... }
@media (scan: progressive) and (environment-blending: opaque) { ... }
@media (grid: 0) { ... }
@media (pointer: fine) and (hover: hover) { ... }
@media (nav-controls: none) { ... }
@media (overflow-block: scroll) and (overflow-inline: scroll) { ... }
@media (update: fast) { ... }
@media (scripting: none) { ... }
@media (forced-colors: none) { ... }
@media (prefers-contrast: no-preference) { ... }
@media (inverted-colors: none) { ... }
@media (dynamic-range: standard) { ... }
@media (video-dynamic-range: high) { ... }
@media (display-mode: standalone) { ... }
@media (prefers-color-scheme: dark) { ... }
@media (prefers-reduced-motion: reduce) { ... }
@media (prefers-reduced-transparency: no-preference) { ... }
@media (prefers-reduced-data: no-preference) { ... }
```

Limitations:

- Supported features are `width`, `height`, `aspect-ratio`, `resolution`,
  `-webkit-device-pixel-ratio`, `-moz-device-pixel-ratio`,
  `device-width`, `device-height`, `device-aspect-ratio`,
  `horizontal-viewport-segments`, `vertical-viewport-segments`,
  `color`, `color-index`, `monochrome`, `color-gamut`, `video-color-gamut`,
  `orientation`, `pointer`, `any-pointer`, `hover`, `any-hover`,
  `nav-controls`, `scan`, `grid`, `environment-blending`, `overflow-block`,
  `overflow-inline`, `update`, `scripting`, `forced-colors`,
  `prefers-contrast`, `inverted-colors`, `dynamic-range`,
  `video-dynamic-range`, `display-mode`, `prefers-color-scheme`,
  `prefers-reduced-motion`, `prefers-reduced-transparency`, and
  `prefers-reduced-data`.
- Width and height values must be absolute lengths convertible to pixels.
- Aspect-ratio values may use ratios such as `4/3` or numbers such as `1.6`.
- Resolution values may use `dppx`, `x`, `dpi`, or `dpcm`.
- The prefixed `-webkit-device-pixel-ratio` and `-moz-device-pixel-ratio`
  compatibility aliases take non-negative numbers and compare against the same
  current window scale factor as `resolution`.
- Device width and height values must be absolute lengths convertible to pixels;
  device aspect-ratio values may use ratios or numbers. DragonGUI currently
  mirrors the logical app viewport for device-size queries.
- Horizontal and vertical viewport segment values must be non-negative
  integers. DragonGUI currently reports one segment on each axis.
- Color, color-index, and monochrome values must be non-negative integers.
  DragonGUI currently reports `color: 8`, `color-index: 0`, and
  `monochrome: 0` for normal native color displays.
- Color-gamut values are `srgb`, `p3`, and `rec2020`; DragonGUI currently
  assumes `srgb`, so wider gamut queries do not match until platform display
  detection is added.
- Video-color-gamut values are `srgb`, `p3`, and `rec2020`; DragonGUI
  currently mirrors the graphics plane and reports `srgb`.
- Orientation values are `portrait` and `landscape`; square viewports match
  `portrait`.
- Scan values are `interlace` and `progressive`; DragonGUI currently reports
  `progressive` for native window rendering.
- Grid values are `0` and `1`; DragonGUI currently reports `0`, meaning a
  bitmap display rather than a character-cell grid device.
- Environment-blending values are `opaque`, `additive`, and `subtractive`;
  DragonGUI currently reports `opaque`.
- Pointer values are `none`, `coarse`, and `fine`; DragonGUI currently reports
  `fine` for `pointer` and `any-pointer`.
- Hover values are `none` and `hover`; DragonGUI currently reports `hover` for
  `hover` and `any-hover`.
- Nav-controls values are `none` and `back`; DragonGUI currently reports
  `none` because native app windows do not expose a browser back control.
- Overflow-block values are `none`, `scroll`, `optional-paged`, and `paged`;
  DragonGUI currently reports `scroll` for native window rendering.
- Overflow-inline values are `none` and `scroll`; DragonGUI currently reports
  `scroll` for native window rendering.
- Update values are `none`, `slow`, and `fast`; DragonGUI currently reports
  `fast` for normal native window rendering.
- Scripting values are `none`, `initial-only`, and `enabled`; DragonGUI
  currently reports `none`.
- Forced-colors values are `none` and `active`; DragonGUI currently reports
  `none` until platform high-contrast/forced-colors integration is added.
- Contrast values are `no-preference`, `more`, `less`, and `custom`;
  DragonGUI currently reports `no-preference` until platform contrast
  preference integration is added.
- Inverted-colors values are `none` and `inverted`; DragonGUI currently
  reports `none` until platform inverted-colors integration is added.
- Dynamic-range and video-dynamic-range values are `standard` and `high`;
  DragonGUI currently reports `standard` for both until platform display
  dynamic-range integration is added.
- Display-mode values are `browser`, `minimal-ui`, `standalone`, `fullscreen`,
  `window-controls-overlay`, and `picture-in-picture`; DragonGUI currently
  reports `standalone` for its native app window.
- Color-scheme values are `dark` and `light`; DragonGUI uses winit's platform
  window theme when available and falls back to active theme background
  luminance when unavailable.
- Reduced-motion values are `reduce` and `no-preference`; DragonGUI currently
  defaults to `no-preference` until OS preference integration is added.
- Reduced-transparency values are `reduce` and `no-preference`; DragonGUI
  currently defaults to `no-preference` until OS preference integration is
  added.
- Reduced-data values are `reduce` and `no-preference`; DragonGUI currently
  defaults to `no-preference` until OS/network preference integration is added.
- Container queries and broader media features remain unsupported.
- Custom properties are collected from top-level `:root`, with a first slice of
  directly nested `:root` variables inside matching `@media` or static true
  `@supports` blocks.

## Font Faces

First-slice `@font-face` support maps declared CSS family names to installed
local font families or loads local/file-embedded font data into the native text
backend.

Supported form:

```css
@font-face {
    font-family: "Report UI";
    src:
        local("Segoe UI"),
        url("file:///C:/Windows/Fonts/segoeui.ttf") format("truetype");
}
```

Limitations:

- Supported sources are `local("Family Name")`, local path `url(...)` files,
  and local `file://` `url(...)` files.
- Supported file extensions are `.ttf`, `.otf`, `.ttc`, and `.woff`.
- Supported `format(...)` descriptors are `truetype`, `opentype`,
  `collection`, and `woff`; unsupported descriptors are skipped.
- `data:` URLs are supported when they are base64-encoded sfnt TrueType,
  OpenType, TTC, or WOFF1 data. WOFF1 sources are decoded to sfnt data before
  loading.
- Relative paths resolve from the current process working directory.
- Percent-escaped `file://` paths are decoded before loading.
- Missing local families and missing or unsupported font files are skipped by
  the renderer and reported once in
  `debug_snapshot()["gpu"]["renderer"]["font_warnings"]`.
- Remote URLs, WOFF2, and packaged app asset resolution are still pending.

## Feature Queries

First-slice `@supports` evaluates statically while the stylesheet is lowered.
Declaration queries use DragonGUI's CSS property parser, selector queries use
DragonGUI's selector subset parser, and `font-format(...)` checks the current
DragonGUI font loader formats. `at-rule(...)` checks the DragonGUI at-rule
parser surface. `font-tech(...)` checks DragonGUI's current text shaping
features.

Supported forms:

```css
@supports (display: grid) { ... }
@supports not (backdrop-filter: blur(8px)) { ... }
@supports (width: calc(100% - 240px)) and (selector(Panel > Button.primary)) { ... }
@supports (backdrop-filter: blur(8px)) { ... }
@supports font-format(woff) { ... }
@supports at-rule(@media) { ... }
@supports font-tech(features-opentype) { ... }
```

Limitations:

- `@supports` conditions are not runtime state; they only test the DragonGUI CSS
  subset supported by the current build.
- Unsupported declaration and selector queries simply evaluate to false.
- `font-format(...)` evaluates true for `truetype`, `opentype`,
  `collection`, and `woff`. DragonGUI also accepts file-extension aliases
  `ttf`, `otf`, and `ttc` because the native font loader accepts those local
  file sources. It currently evaluates false for `woff2`, `embedded-opentype`,
  and `svg`.
- `at-rule(...)` evaluates true for `media`, `supports`, `keyframes`, and
  `font-face`; it currently evaluates false for unsupported at-rules such as
  `container`.
- `font-tech(...)` evaluates true for `features-opentype`; it currently
  evaluates false for color font technologies, palettes, incremental font
  transfer, and variable-font feature claims.

Supported feature queries reflect DragonGUI's native subset, not browser
support. For example, `backdrop-filter: blur(...) brightness(...)` currently
means the first-slice DragonGUI frosted-surface treatment is available.
- `@supports` can be nested with supported `@media` rules.

Specificity model:

| Selector Component | Specificity Bucket |
| --- | --- |
| `#id` | id |
| `.class` | class |
| `[key="main"]`, `[level="info"]` | class |
| `:hover`, `:focus`, etc. | class |
| Type selectors such as `Button` | type |
| Widget parts such as `::stepper` | no extra specificity |
| Descendant and child chains | sum of each compound selector |

## Selectors

Supported selector forms:

| Form | Example |
| --- | --- |
| Universal | `*`, `Panel > *` |
| Type | `Button` |
| Class | `.primary` |
| Multiple classes | `Button.primary.danger` |
| ID | `#run-button` |
| Key attribute | `[key="primary-action"]` |
| Attribute | `[disabled]`, `[level="info"]`, `[class~="pill"]`, `[text="run" i]` |
| Type + ID + class | `Button#run-button.primary` |
| Pseudo-state | `Button:hover` |
| Widget part | `NumberInput::stepper` |
| Descendant | `Panel Button` |
| Direct child | `Panel.controls > Button` |
| Multi-level child chain | `Window > Panel > HLayout > Button` |
| Direct child targeting a part | `Panel.controls > NumberInput::stepper` |
| Structural child | `Button:first-child`, `Button:last-child`, `Label:only-child`, `Panel:empty` |
| Structural nth child | `NavItem:nth-child(3n+1)`, `NavItem:nth-last-child(2n+1)`, `Panel > *:nth-child(2 of Panel > Button.primary)`, `Panel > *:nth-last-child(1 of Button:first-child)` |
| Selector function | `Button:not(:disabled)`, `Button:is(:hover, :focus)`, `:where(.quiet)`, `Panel:has(> Button:first-child)`, `Panel:has(+ Button.primary)`, `Panel:has(Panel:has(Button.primary))`, `Panel:has(Panel:has(> Badge) > Button.primary)` |
| Comma list | `Button, Dropdown, TextInput` |

Selector behavior:

- CSS type names are exact and case-sensitive, such as `Button`, not `button`.
- `class_` is split on CSS whitespace, so `class_="primary danger"` matches
  `.primary`, `.danger`, and `.primary.danger`.
- ID selectors match explicit widget `id`.
- Attribute selectors match DragonGUI widget metadata and parsed scalar props.
  Supported operators are presence (`[disabled]`), exact (`[level="info"]`),
  whitespace word (`[class~="pill"]`), prefix (`[text^="Run"]`), suffix
  (`[path$=".png"]`), substring (`[text*="ready"]`), and dash-match
  (`[level|="info"]`). Value selectors support explicit ASCII case flags:
  `i` for case-insensitive matching and `s` for case-sensitive matching.
  Boolean attributes are present only when true.
  Supported names include `id`, `type`, `class`, `key`, `text`, `badge`,
  `level`, `placeholder`, `value`, `page`, `orientation`, `target`, `tooltip`,
  `path`, `fit`, `width`, `height`, `min`, `max`, `step`, `disabled`,
  `checked`, `expanded`, `open`, `wrap`, `rows`, `page-size`, `table-rows`, and
  `items-count`.
- Descendant selectors match any ancestor in the widget tree.
- Direct child selectors only check the immediate parent.
- Pseudo-states in selector chains apply to the target widget, not ancestors.
- `:first-child`, `:last-child`, `:only-child`, `:nth-child(...)`, and
  `:nth-last-child(...)` use the target widget's sibling position among all
  children of its parent.
- `:empty` matches widgets with no child widgets. Widget text and scalar props
  do not count as children.
- `:nth-child(...)` and `:nth-last-child(...)` support integer indexes, `odd`,
  `even`, and `an+b` formulas such as `3n+1`, `n+2`, and `-n+3`.
  `:nth-last-child(...)` counts from the end of the sibling list. Both forms
  support the `of <selector-list>` filtering form for compound selector lists
  and supported descendant or child selector chains, such as
  `:nth-child(2 of Panel > Button.primary, Badge[level="info"])` and
  `:nth-last-child(1 of Button.metric)`. Filter target selectors may include
  structural pseudos such as `Button:first-child` and `Label:nth-child(2)`,
  plus data-backed state pseudos such as `Checkbox:checked` and
  `Collapsible:collapsed`.
- `:not(...)`, `:is(...)`, and `:where(...)` support comma-separated
  compound selector arguments, including target pseudo-state arguments such as
  `:hover`, `:focus`, and `:disabled`. `:where(...)` contributes zero
  specificity.
- `:has(...)` supports descendant compound selectors and selector chains, such
  as `Panel:has(Button.primary)` and `Panel:has(HLayout > Badge.success)`.
  A leading `>` restricts the argument to direct children, so
  `Panel:has(> Badge[level="warning"])` and
  `Panel:has(> Button:first-child)` only check immediate children. Structural
  target pseudos such as `:first-child`, `:last-child`, `:only-child`,
  `:empty`, and `:nth-last-child(...)` are supported in `:has(...)`
  arguments. Leading `+` and `~`
  arguments check following siblings, such as
  `Panel:has(+ Button.primary)` and `Panel:has(~ Badge[level="success"])`.
  Nested `:has(...)` is supported when it appears on the argument target, such
  as `Panel:has(Panel:has(Button.primary))`, and on ancestor-side compounds in
  descendant or direct-child argument chains, such as
  `Panel:has(Panel:has(> Badge) > Button.primary)`. Target arguments may use
  data-backed state pseudos (`:disabled`, `:checked`, `:open`, `:expanded`,
  and `:collapsed`). Dynamic state pseudos and widget-part arguments are not
  supported.
- Part selectors are only valid on the target widget, not the parent side of a
  selector chain.

Unsupported selector forms:

- Empty target part selectors, such as `::stepper`.
- Ancestor pseudo-states, such as `Panel:hover Button`.
- Ancestor pseudo-states inside selector functions, such as
  `Panel:is(:hover) Button`.
- Broader `:has(...)` arguments, such as dynamic stateful
  `Panel:has(Button:hover)`, widget-part arguments, and sibling-relative
  nested `:has(...)` on an ancestor side of a selector chain.

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
| `display` | `flex`, `grid`, `block`, `none` | Sets display behavior. |
| `flex-direction` | `row`, `column`, `row-reverse`, `column-reverse` | Sets container direction. Snake-case reverse values are also accepted internally. |
| `flex` | number | Maps to `flex-grow`; clamped to at least `0`. |
| `flex-grow` | number | Clamped to at least `0`. |
| `flex-shrink` | number | Clamped to at least `0`. |
| `width` | logical px, percent, `auto`, compatible `calc()` | Fixed/preferred width depending on widget/layout. |
| `height` | logical px, percent, `auto`, compatible `calc()` | Fixed/preferred height depending on widget/layout. |
| `min-width` | logical px, percent, `auto`, compatible `calc()` | Minimum width. |
| `min-height` | logical px, percent, `auto`, compatible `calc()` | Minimum height. |
| `max-width` | logical px, percent, `auto`, compatible `calc()` | Maximum width. |
| `max-height` | logical px, percent, `auto`, compatible `calc()` | Maximum height. |
| `padding` | 1 to 4 logical-px, percent, or compatible `calc()` values | Expands to per-side padding. `auto` warns and is ignored. |
| `padding-left` | logical px, percent, compatible `calc()` | Left padding. |
| `padding-right` | logical px, percent, compatible `calc()` | Right padding. |
| `padding-top` | logical px, percent, compatible `calc()` | Top padding. |
| `padding-bottom` | logical px, percent, compatible `calc()` | Bottom padding. |
| `margin` | 1 to 4 logical-px, percent, `auto`, or compatible `calc()` values | Expands to per-side margin. |
| `margin-left` | logical px, percent, `auto`, compatible `calc()` | Left margin. |
| `margin-right` | logical px, percent, `auto`, compatible `calc()` | Right margin. |
| `margin-top` | logical px, percent, `auto`, compatible `calc()` | Top margin. |
| `margin-bottom` | logical px, percent, `auto`, compatible `calc()` | Bottom margin. |
| `gap` | logical px, percent, compatible `calc()` | Container gap. `auto` warns and is ignored. |
| `row-gap` | logical px, percent, compatible `calc()` | Grid/flex row gap. |
| `column-gap` | logical px, percent, compatible `calc()` | Grid/flex column gap. |
| `grid-template-columns` | track list | First-slice CSS Grid columns. Supports px, percent, `fr`, `auto`, `minmax()`, `fit-content()`, nested finite `repeat(n, ...)`, and non-nested `repeat(auto-fit, ...)` / `repeat(auto-fill, ...)`. |
| `grid-template-rows` | track list | First-slice CSS Grid rows. Supports px, percent, `fr`, `auto`, `minmax()`, `fit-content()`, nested finite `repeat(n, ...)`, and non-nested `repeat(auto-fit, ...)` / `repeat(auto-fill, ...)`. |
| `grid-template-areas` | quoted rows | Defines rectangular named grid regions, such as `"side main" "side footer"`. |
| `grid-auto-flow` | `row`, `column`, optional `dense` | Controls CSS Grid auto-placement direction and dense packing. `dense` alone maps to `row dense`. |
| `grid-area` | named area | Places a child into a named parent `grid-template-areas` region. Explicit `grid-column` / `grid-row` placement wins if both are set. |
| `grid-column` | line placement | Supports `auto`, line numbers, and `span n`, including `start / end`. |
| `grid-row` | line placement | Supports `auto`, line numbers, and `span n`, including `start / end`. |
| `overflow` | `visible`, `hidden`, `scroll`, `auto` | First-slice overflow behavior. `visible` lets children escape clipping; `hidden` clips; `scroll`/`auto` opt into scroll state. |
| `overflow-x` | `visible`, `hidden`, `scroll`, `auto` | Axis-specific overflow. `scroll`/`auto` use horizontal scroll state when content overflows. |
| `overflow-y` | `visible`, `hidden`, `scroll`, `auto` | Axis-specific overflow. `scroll`/`auto` use vertical scroll state. |
| `position` | `static`, `relative`, `absolute`, `fixed` | First-slice positioning. `relative` is paint-only; `absolute` uses parent layout insets; `fixed` uses viewport insets. |
| `top`, `right`, `bottom`, `left` | logical px | Offsets `position: relative` widgets, places `position: absolute` widgets within their parent layout context, and pins `position: fixed` widgets to the window. |
| `z-index` | integer | First-slice local stacking hint for sibling widget surfaces and normal text. Does not affect layout, hit testing, or overlay priority. |

Length handling:

- Unitless numbers are logical pixels.
- `px` values are logical pixels.
- Percent lengths are supported for `width`, `height`, `min-*`, `max-*`,
  padding, margin, and gap properties.
- `auto` is supported for `width`, `height`, `min-*`, `max-*`, and margin.
  Padding and gap reject `auto`.
- `calc()` is supported for `width`, `height`, `min-*`, `max-*`, padding,
  margin, and gap properties. Supported operators are `+`, `-`, simple
  `*` by a scalar, and simple `/` by a scalar. Mixed percent/pixel expressions
  such as `calc(100% - 240px)` resolve when the parent axis has a definite
  size. `var(--name)` and `var(--name, fallback)` may be used as calc terms.
- Percent and `auto` values still warn and are ignored for paint, border,
  typography, and other properties unless explicitly documented for that
  property.
- Negative lengths are not broadly rejected at parse time; renderer/layout
  behavior depends on the target property.

## Visual Properties

Supported visual properties:

| Property | Accepted Values | Effect |
| --- | --- | --- |
| `background` | color, `linear-gradient(...)`, `radial-gradient(...)`, `blob-gradient(...)`, `mesh-gradient(...)`, or comma-separated paint layers | Fill/background paint. |
| `background-color` | color | Solid fill/background color. |
| `background-image` | `linear-gradient(...)`, `radial-gradient(...)`, `blob-gradient(...)`, `mesh-gradient(...)`, repeating gradients, comma-separated gradient layers, or `none` | Gradient/image paint subset. Layers over `background-color`; `none` clears back to the solid color. `url(...)` sources are not supported. |
| `background-noise` | number | Adds subtle deterministic noise to rect-backed gradient backgrounds. Clamped to `0.0..0.25`. |
| `gradient-interpolation` | `srgb`, `linear-srgb`, or `oklab` | Selects the color interpolation space for gradient stops on rect-backed surfaces. Defaults to `srgb`. |
| `foreground` | color | Foreground glyph/control color for renderers that use visual foreground. |
| `border-color` | color | Border color. |
| `border-width` | logical px | Border width. |
| `border-style` | `solid`, `none`, or `hidden` | Uniform border style subset. `none` and `hidden` reset the rendered border; `solid` uses the current border width and color. |
| `border-radius` | logical px | Uniform corner radius. |
| `border-top-left-radius` | logical px | Per-corner radius override. |
| `border-top-right-radius` | logical px | Per-corner radius override. |
| `border-bottom-right-radius` | logical px | Per-corner radius override. |
| `border-bottom-left-radius` | logical px | Per-corner radius override. |
| `border` | `none`, `0`, or `<width> solid <color>` | Shorthand for border reset, width, and color. Only `solid` and `none` styles are supported. |
| `outline` | `none`, `0`, or `<width> solid <color>` | Paint-only outline shorthand for rect-backed widget surfaces. |
| `outline-color` | color | Outline color. |
| `outline-width` | logical px | Outline width. |
| `outline-style` | `solid`, `none`, or `hidden` | Outline style subset. `none` and `hidden` reset the rendered outline. |
| `outline-offset` | logical px | Offset between the widget edge and outline. Negative offsets are clamped to zero in the first slice. |
| `box-shadow` | comma-separated `inset? <offset-x> <offset-y> <blur?> <spread?> <color>` layers | Outset and inset soft shadows for rect-backed widget surfaces. |
| `opacity` | number | Clamped to `0.0..1.0`. |
| `accent` | color | Widget accent color. |
| `track-color` | color | Slider/progress track color. |
| `thumb-color` | color | Slider thumb color. |
| `transform` | `none`, `translate(...)`, `translateX(...)`, `translateY(...)`, `scale(...)`, `scaleX(...)`, `scaleY(...)`, `rotate(...)` | Paint-only transform shorthand for rect-backed widget surfaces. |
| `translate` | `none`, one or two logical px lengths | Paint-only transform longhand. Percent and Z-axis translate are not supported. |
| `scale` | `none`, one or two numbers | Paint-only transform longhand. |
| `rotate` | `none`, angle in `deg`, `rad`, `turn`, or unitless degrees | Paint-only transform longhand. |

Notes:

- Per-corner radius values inherit from `border-radius` when a specific corner
  is not set.
- `box-shadow` supports one or more comma-separated outset or `inset` shadows.
  Outset shadows paint before the widget surface; inset shadows paint above the
  surface and below child widgets. Outset shadows inside scroll/overflow
  containers are clipped to the inherited paint viewport while keeping the full
  shadow shape.
- `linear-gradient(...)` currently supports angles and `to ...` directions.
  `radial-gradient(...)` currently supports centered circle gradients and
  `circle at <x> <y>` percent/keyword centers.
  The renderer interpolates up to six explicit or inferred color stops. Longer
  gradients are sampled down to six GPU stops. Transparent stops interpolate
  with premultiplied alpha to avoid dark halos when colors fade to transparent.
  `gradient-interpolation: linear-srgb` blends stops in linear-light sRGB;
  `gradient-interpolation: oklab` blends stops through Oklab before converting
  back to sRGB for output.
- `repeating-linear-gradient(...)` and `repeating-radial-gradient(...)` are
  supported when the final explicit stop is less than `100%`; the renderer
  repeats that stop range across the shape.
- `blob-gradient(...)` is a DragonGUI-specific paint for organic blob
  backgrounds. It accepts up to four entries in the form
  `at <x> <y> <color> <radius>`, with percent centers and radius values such
  as `at 22% 30% rgba(90, 169, 255, 0.68) 46%`. Blob layers combine soft
  fields in the shader rather than drawing separate geometric circles.
- `mesh-gradient(...)` is a DragonGUI-specific image-like four-corner gradient.
  It accepts four colors in top-left, top-right, bottom-left, bottom-right
  order and bilinearly blends across the rect. This is intended for smooth
  raster-gradient style surfaces rather than circular blob effects.
- Multiple comma-separated color/gradient background layers can be painted
  back-to-front. This is useful for a soft radial glow over a linear base.
- `background-noise` is a small procedural dither/noise pass for rect-backed
  gradient backgrounds. Values around `0.01..0.03` are intended for softening
  gradient banding without looking visibly grainy.
- Browser `background-image: url(...)` and side-specific border properties such
  as `border-left` are not supported. `border-style` / `outline-style` are
  uniform only; dashed, dotted, double, and per-side styles are not supported.
- General clipping/overflow is first-slice only. Specific renderers also
  implement clipping where designed, such as `Image`, `DataFrameTable`,
  dropdown menus, menu rows, and panel accent fills.
- Scroll containers render overlay scrollbar indicators for opted-in axes when
  content overflows. Panel scrollbars are kept inside rounded panel corners and
  centered on the panel surface. When both axes overflow, the indicators leave
  the bottom-right corner clear. Users can drag the thumb or click the track to
  update the scroll offset. PageUp/PageDown/Home/End scroll the nearest
  scrollable ancestor when keyboard focus is inside a scroll container; Shift
  uses the horizontal axis when horizontal overflow exists.
  Scrollable layout/container widgets expose `::scrollbar-track` and
  `::scrollbar-thumb`. The track part supports `width`, uniform `padding`,
  background, border, radius, and opacity; the thumb part supports `width`,
  background, border, radius, and opacity.
- `Scatter3D` clips its 3D viewport and picking region to the computed
  per-corner border radii.
- `transform` and the `translate` / `scale` / `rotate` longhands are
  paint-only. They do not affect layout or hit testing.
- `position: relative` is paint-only in the first slice. It offsets the
  widget's rect-backed surface and text, but does not affect layout, hit
  testing, or child layout.
- `z-index` sorts siblings in the same parent stacking context for normal
  widget paint and text traversal. Overlays, scatter depth, and hit testing keep
  their existing ordering.

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
| `text-area-rows` | `TextArea` | Preferred visible row count. Rounded to an integer and clamped to at least 1. |
| `scatter-point-size` | `Scatter3D` | Uniform screen-space point size in logical pixels. |
| `scatter-point-style` | `Scatter3D` | Point shape: `circle` (default), `square`, or `gaussian`. |
| `table-row-height` | `DataFrameTable` | Row height in logical pixels. |
| `table-header-height` | `DataFrameTable` | Header height in logical pixels. |
| `table-column-width` | `DataFrameTable` | Uniform body/header column width in logical pixels. |
| `table-index-width` | `DataFrameTable` | Row-index gutter width in logical pixels. |

Unsupported widget-specific needs:

- `TextArea` supports DragonGUI's `text-area-rows` property; CSS `height`
  still forces an exact rendered size.
- There is no scatter colormap CSS property; use the `Scatter3D` constructor or
  `set_colormap()`.

## Transitions

Supported transition CSS:

| Property | Accepted Values | Effect |
| --- | --- | --- |
| `transition` | `<property> <duration> <timing?> <delay?>` | Shorthand for one transition item. |
| `transition-property` | comma-separated supported property names or `none` | Selects paint properties for runtime transition eligibility. |
| `transition-duration` | `ms` or `s` time | Duration. `0ms` disables transition behavior. |
| `transition-timing-function` | `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`, `step-start`, `step-end`, `steps(n, start/end)`, `cubic-bezier(x1, y1, x2, y2)` | Easing curve. Cubic-bezier x control points must be between `0` and `1`; step counts must be positive. |
| `transition-delay` | `ms` or `s` time | Start delay. |

Current runtime support:

- Runtime transitions apply to whole-widget hover, `:focus`, and `:active`
  paint changes and whole-widget `:checked`, `:open`, `:selected`,
  `:expanded`, and `:collapsed` paint changes for widgets with tracked state.
- Solid color and numeric visual fields interpolate. Gradient paints and
  shadows currently switch at the midpoint. Outline color, width, and offset
  interpolate as visual fields.
- `transform` transitions interpolate translate, scale, and rotation for
  rect-backed widget surfaces. Text currently follows translate and uniform
  scale, but not rotation.
- `transition-property` is honored for hover, `:focus`, `:active`, `:checked`,
  `:open`, `:selected`, `:expanded`, and `:collapsed` transitions. Unlisted visual
  fields snap to the current state instead of interpolating.
- Layout, text size, and grid/flex transitions are not supported.
- Active transitions are runtime-only and do not appear as persistent computed
  style.
- Stylesheet changes, inline style patches, full layout rebuilds, and widget
  replacement paths cancel active style transition progress and apply the new
  computed style directly.

Supported transition property names:

```text
all, background, background-color, foreground, border-color, border-width,
border-radius, outline, outline-color, outline-width, outline-offset,
outline-style, opacity, color, accent, track-color, thumb-color, box-shadow,
transform, translate, scale, rotate
```

## Animations

Supported animation CSS:

| Property | Accepted Values | Effect |
| --- | --- | --- |
| `@keyframes` | `from`, `to`, and percentage selectors | Defines visual keyframes. |
| `animation` | one shorthand animation | Sets name, duration, timing, delay, iteration count, direction, fill mode, and play state. |
| `animation-name` | keyframe name or `none` | Selects one keyframe animation for the widget. |
| `animation-duration` | `ms` or `s` time | Duration for one iteration. |
| `animation-timing-function` | `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`, `step-start`, `step-end`, `steps(n, start/end)`, `cubic-bezier(x1, y1, x2, y2)` | Eases progress between keyframes. |
| `animation-delay` | positive or negative `ms` / `s` time | Delays animation start. Negative delays start the animation as if it had already been running. |
| `animation-iteration-count` | positive number, fractional number, or `infinite` | Repeats the animation. Finite fractional counts end at the corresponding partial keyframe when a forward fill mode is active. |
| `animation-direction` | `normal`, `reverse`, `alternate`, `alternate-reverse` | Controls playback direction. |
| `animation-fill-mode` | `none`, `forwards`, `backwards`, `both` | Applies first or last keyframe outside the active interval. |
| `animation-play-state` | `running`, `paused` | Runs or freezes the animation at its first directed frame. |

Current runtime support:

- One animation can run per widget.
- Comma-separated animation shorthand or longhand lists are accepted by using
  the first item. Multiple simultaneous animations remain unsupported.
- Keyframes support visual declarations such as solid colors, opacity,
  borders, shadows, background paints, and transforms.
- Runtime interpolation matches transition support: solid colors and numeric
  visual fields interpolate; gradient paints and shadows switch at midpoint;
  transforms interpolate translate, scale, and rotation.
- Layout, text size, grid/flex, multiple animations, composition, timelines,
  and OS reduced-motion preference integration are not supported yet.

## Backdrop Filter

Supported backdrop filter CSS:

| Property | Accepted Values | Effect |
| --- | --- | --- |
| `backdrop-filter` | `none`, `blur(<px>)`, `brightness(number \| percent)`, `saturate(number \| percent)`, or supported whitespace-separated filter lists | Enables the first-slice frosted-surface treatment on supported widget surfaces. |

Current runtime support:

- Supported surfaces are `Panel`, `Modal`, `Tooltip`, and `Toast`.
- The first slice is clipped to the widget's rounded rectangle.
- The renderer applies a subtle frosted tint/noise treatment derived from the
  blur amount and brightness/saturation factors.
- This is not a full browser-compatible sampled framebuffer blur yet. True
  backdrop sampling still requires an offscreen scene texture and blur pass.
- Filter interpolation is not supported.

## Color Values

Supported color syntax for stylesheet declarations, inline style dictionaries,
and Python `Theme` color fields:

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
- `hwb(...)`
- `lab(...)`
- `lch(...)`
- `oklab(...)`
- `oklch(...)`
- `color(srgb ...)`
- `color(srgb-linear ...)`
- `var(--name)` in stylesheets when the variable resolves to a color, token,
  or string token. Python `Theme` fields do not support `var(...)`.

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

- `color(...)` spaces other than `srgb` and `srgb-linear`, including
  `display-p3`, `a98-rgb`, `prophoto-rgb`, `rec2020`, and `xyz`.
- Other CSS Color 4 function variants outside the listed supported subset.
- Full browser named color inventory beyond the common names listed above.
- Wide-gamut color spaces remain unsupported in declarations; supported color
  functions are converted or clipped into DragonGUI's current sRGB renderer
  path.

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
    border: 1px solid var(--brand);
}
```

Capabilities:

- Custom properties are collected from `:root`.
- `:root` variables are collected before normal declarations are lowered, so
  variables can be used before they appear in source order.
- Direct `:root` variables inside a matching `@media` block can be used by
  declarations inside that same media block.
- Direct `:root` variables inside a static true `@supports` block can be used
  by declarations inside that same supports block.
- Custom properties declared inside a normal selector block can be used by
  declarations in that same block.
- Variable values can resolve to numbers, lengths, colors, keywords, or strings.
- `var(--name)` is supported as a whole property value.
- `var(--name, fallback)` is supported as a whole property value.
- `var()` is supported inside larger parseable property values, including
  borders, shadows, gradients, grid tracks, and transition shorthands.

Limitations:

- Selector-local custom properties do not inherit into descendant widget
  rules.
- Media/support-scoped variables are parse-time scoped to declarations inside
  the same block; this is not full inherited browser custom-property cascade.
- `calc()` is only supported as a whole layout value for sizing and supported
  spacing properties. `var()` inside `calc()` is supported for length terms and
  scalar multipliers/divisors.
- Variables are resolved during parsing/lowering of a stylesheet. Do not rely on
  variables defined in one stylesheet origin being available inside another
  stylesheet string.

## Widget Parts

Parts are renderer-owned styling hooks. They are not real widgets and do not
have separate layout nodes, focus targets, hit-test regions, callbacks, or ids.

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
| `Slider` | `track`, `fill`, `thumb` |
| `ProgressBar` | `track`, `fill`, `label` |
| `Tabs` | `header` |
| `Tab` | `tab`, `accent`, `badge` |
| `NavItem` | `item`, `accent`, `badge` |
| `DataFrameTable` | `header`, `row`, `row-selected`, `grid-line` |
| Most rendered widgets | `before`, `after` generated content |

Part property support:

| Property Class | Base Part | Stateful Part |
| --- | --- | --- |
| Visual properties | supported | supported |
| Text properties | supported | supported |
| `width` | supported where renderer uses it | ignored with warning |
| `height` | supported where renderer uses it | ignored with warning |
| `padding` | supported when uniform | ignored with warning |
| `gap` | parsed; reserved for renderer-specific future use | ignored with warning |
| `content` | supported for `before` / `after` generated content | supported for `before` / `after` generated content |
| Widget-specific properties | ignored | ignored |

Examples:

```css
Panel::accent {
    width: 5px;
    background: accent;
}

Panel::scrollbar-track {
    width: 5px;
    padding: 14px;
    background: rgba(255, 255, 255, 0.10);
    border-radius: 999px;
}

Panel::scrollbar-thumb {
    background: accent;
    border-radius: 999px;
}

HLayout::scrollbar-thumb {
    background: success;
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

Panel.summary::after {
    content: "new";
    width: 42px;
    color: success;
    font-size: 11px;
    text-align: right;
}
```

Unsupported part selectors:

- Unsupported parts warn during cascade.
- Parent-side part selectors in child rules are rejected.
- CSS part names should use dashed names such as `stepper-up`. Snake-case part
  names are normalized only for inline style dictionaries.

Generated `::before` / `::after` content is renderer-owned text. It does not
participate in layout, receive input, or create child widgets. The first slice
supports quoted string content, `attr(name)` lookups against widget metadata and
serialized props, plus visual/text styling. Counters and independent
generated-content layout are not supported. Controls align generated content to
the control centerline, while titled containers such as `Panel`, `Sidebar`, and
`Modal` anchor it to the title band.

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
- `box-shadow` emits paint-only soft shadows. Outset shadows paint before the
  widget surface; inset shadows paint above the surface and below child
  widgets. Outset shadows are clipped against inherited scroll/overflow paint
  viewports. It does not affect layout or hit testing.
- `DataFrameTable` with `border-radius` clips header, rows, selection, grid
  lines, and border to the table shape.
- `Image` with `border-radius` clips the texture to the image content box inside
  the border. Image textures also follow the image widget's paint transform.
- Dropdown popup and menu item fills are clipped to rounded popup bounds.
- `:checked`, `:open`, `:expanded`, `:collapsed`, and `:selected` are resolved
  from runtime widget state and can style both whole widgets and widget parts.
- Whole-widget `:hover`, `:focus`, `:active`, `:checked`, `:open`,
  `:selected`, `:expanded`, and `:collapsed` visual changes can transition when the widget
  has a non-zero CSS transition duration. Outline color, width, and offset
  follow this transition path.
- `transform` and the `translate` / `scale` / `rotate` longhands apply to the
  widget render subtree around the widget's own center. Descendant primitive
  rects, scrollbars, and `Image` textures follow the transformed subtree. Text
  follows translate and uniform scale, but not rotate yet.
- `position: relative` reuses the paint offset path for widget surfaces and
  text. It is intended for small visual nudges and badges, not full overlay
  layout.
- `position: absolute` is backed by layout and uses `top`, `right`, `bottom`,
  and `left` insets. It is intended for explicit-size badges, pins, and small
  controls inside a positioned parent. Absolute children are removed from
  normal flow.
- `position: fixed` is backed by layout and then rebased to the window rect.
  It is intended for explicit-size docks, badges, and floating controls pinned
  to the viewport. Fixed widgets are removed from normal flow and clip against
  the window rather than their parent.
- `z-index` is local to sibling widgets. It is useful for overlapping relative
  badges or small decorative elements, but it is not a full browser stacking
  context implementation.
- `linear-gradient(...)`, `radial-gradient(...)`, layered backgrounds,
  gradient `background-image`, and subtle `background-noise` render on normal
  rect-backed widget surfaces and respect rounded rect clipping. The renderer
  supports up to six stop colors per rect instance.
- Slider uses `track-color`, `thumb-color`, and `Slider::track`,
  `Slider::fill`, `Slider::thumb`.
- ProgressBar uses `ProgressBar::track`, `ProgressBar::fill`, and
  `ProgressBar::label`.
- NumberInput stepper geometry can be styled with `NumberInput::stepper`,
  `::stepper-up`, `::stepper-down`, `::divider`, `::stepper-divider`, and
  `::caret`. `::stepper-down` is the left/decrement side and `::stepper-up`
  is the right/increment side.
- Tab and NavItem badges/accent strips are styled through their parts.

Known renderer gaps:

- Scrollbar CSS parts are currently limited to scrollable layout/container
  widgets.
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
- Computed layout, visual, text, widget, transition, pseudo, and part style
  fields.

Warnings are also generated for:

- Unsupported CSS properties.
- Unsupported selector forms.
- Unsupported length forms, including percentages and `auto` on properties
  that do not support them.
- Unsupported widget parts.
- Stateful part layout declarations.
- Unsupported inline parts that reach the native validator.

## Current Limitations Summary

Selector limitations:

- No ancestor pseudo-states in selector chains.
- No browser pseudo-elements.
- `:nth-child(... of <selector>)` and `:nth-last-child(... of <selector>)`
  filters reject widget parts, dynamic target pseudo-states such as `:hover`,
  `:active`, `:focus`, and `:selected`, `:root`, and otherwise unsupported
  selector forms inside the filter.

Property limitations:

- CSS Grid is first-slice only: named grid areas are supported for rectangular
  regions, nested finite `repeat(n, ...)` track lists are supported, and
  `grid-auto-flow` supports row/column dense auto-placement, but subgrid and
  nested auto-repeat remain unsupported.
  `minmax()` supports px, percent, and `auto` minimums plus px, percent, `fr`,
  and `auto` maximums. `fit-content()` supports px or percent arguments.
  `repeat(auto-fit, ...)` and `repeat(auto-fill, ...)` are supported for
  non-nested auto-repeat track lists.
- Mixed-unit `calc()` requires a definite parent axis and may not resolve in
  every flex/auto sizing case.
- Percent and compatible `calc()` spacing are first-slice only for padding,
  margin, and gap properties. `auto` spacing is supported for margin.
- No general transitions beyond first-slice whole-widget hover, `:focus`,
  `:active`, `:checked`, `:open`, `:selected`, `:expanded`, and `:collapsed` paint/transform
  transitions.
- Animations are limited to the first-slice visual `@keyframes` support
  described above.
- Backdrop filters are limited to the first-slice frosted-surface treatment
  described above.
- Generated content is text-only and does not affect layout.
- No image backgrounds.
- Overflow is first-slice only: scrollbar part styling is available on
  scrollable layout/container widgets, and overlay clipping remains
  renderer-managed.
- Positioning is first-slice only: no global stacking contexts or transformed
  hit testing yet. `absolute` and `fixed` currently expect explicit or intrinsic
  widget size.
- Media queries are first-slice only: viewport `width`/`height`,
  `aspect-ratio`, `resolution`, `-webkit-device-pixel-ratio`,
  `-moz-device-pixel-ratio`, `device-width`, `device-height`,
  `device-aspect-ratio`, `horizontal-viewport-segments`,
  `vertical-viewport-segments`, `color`, `color-index`, `monochrome`,
  `color-gamut`, `video-color-gamut`, `orientation`, `scan`, `grid`,
  `environment-blending`, `pointer`,
  `any-pointer`, `hover`, `any-hover`, `overflow-block`, `overflow-inline`,
  `nav-controls`, `update`, `scripting`, `forced-colors`,
  `prefers-contrast`, `inverted-colors`, `dynamic-range`,
  `video-dynamic-range`, `display-mode`, `prefers-color-scheme`,
  `prefers-reduced-motion`, `prefers-reduced-transparency`, and
  `prefers-reduced-data` conditions are supported, but container queries and
  other media features are not.
- No per-side border shorthand.
- No per-column table width controls; `DataFrameTable` supports uniform
  `table-column-width` and `table-index-width`.
- Scatter-specific CSS is limited to `scatter-point-size` and `scatter-point-style`.

Variable limitations:

- No arbitrary widget-scoped variables.
- Media/support-scoped `:root` variables are first-slice parse-time scoped, not
  full inherited browser custom-property cascade.
- No cross-stylesheet variable sharing guarantee.
- No dynamic variable recomputation beyond stylesheet reparse.

Renderer limitations:

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
