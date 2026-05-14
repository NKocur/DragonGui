# CSS Styling System Part 3: Web UI Capabilities Plan

> Historical V2 plan. Current CSS follow-up work is tracked in
> `plans/V3/css-and-scatter-followup-plan.md`.

Status: Draft for implementation
Owner: DragonGUI V2
Depends on:

- `plans/V2/css-styling-system.md`
- `plans/V2/css-styling-system-part-2-widget-parts.md`
- `docs/css-capabilities-reference.md`

## Objective

Bring DragonGUI CSS closer to the visual and layout capabilities users expect
from modern web UI systems while preserving the native-widget architecture.

The current CSS system already has a solid foundation:

- Lightning CSS parsing lowered into DragonGUI-owned style rules.
- Cascade by origin, specificity, source order, and `!important`.
- Type, class, id, direct-child, pseudo-state, and widget-part selectors.
- `NodeStyle`, `VisualStyle`, `TextStyle`, `WidgetStyle`, and part styles.
- Live stylesheet updates.
- Debug snapshots with matched and computed styles.
- Framework defaults that user CSS can override.

Part 3 should build on that foundation instead of replacing it.

## Product Goal

Make DragonGUI stylesheets capable of producing interfaces that feel designed,
not only recolored.

The most important visible gaps today are:

- Flat surfaces with no elevation.
- Only first-slice gradients; intermediate stop interpolation, first-slice
  repeating gradients, and comma-separated background layers have landed.
  Gradient rendering now carries up to six GPU stops per rect, uses
  premultiplied-alpha interpolation for transparent fades, and supports
  `gradient-interpolation: srgb | linear-srgb | oklab`. DragonGUI-specific
  `blob-gradient(...)` organic background paint has landed for up to four soft
  color fields, and `mesh-gradient(...)` four-corner image-like paint has
  landed for smooth raster-gradient-style surfaces. Gradient
  `background-image` now lowers into the same paint model, layers over
  `background-color`, and supports `none`; URL image backgrounds remain
  unsupported.
- Transition easing now includes standard keywords, step timing, and custom
  cubic-bezier curves; broader animation support is still pending.
- Limited typography controls.
- Limited color syntax.
- Limited responsive layout primitives.
- First interactive state selector and selector-chain slices landed for
  `:open`, `:expanded`, `:collapsed`, `:selected`, descendant selectors,
  multi-level child chains, `[key="..."]`, structural child selectors, and
  structural filters inside `:nth-child(... of ...)` and
  `:nth-last-child(... of ...)`. Selector functions have landed for compound
  selector arguments, including target pseudo-state arguments. Universal
  selectors and attribute selectors for widget
  metadata/scalar props have also landed, including
  presence, string operators, and case flags. First-slice `:has(...)` support
  has landed for descendant compound selectors, descendant selector chains,
  direct child arguments, direct child structural pseudos, and following
  sibling arguments with leading `+` or `~`. Nested `:has(...)` on the
  argument target and on ancestor-side compounds in descendant or direct-child
  argument chains has also landed. Data-backed state target arguments for
  `:has(...)`, `:nth-child(... of ...)`, and `:nth-last-child(... of ...)`
  have landed for `:disabled`, `:checked`, `:open`, `:expanded`, and
  `:collapsed`.

The first implementation slices should prioritize features that visibly improve
existing demos with low architectural risk.

## Non-Goals

This plan does not attempt to make DragonGUI a browser.

Out of scope for the near-term implementation:

- Full CSS compatibility.
- Full DOM-style generated content.
- Arbitrary CSS painting.
- Browser-level layout features that conflict with DragonGUI widget ownership.
- Web JavaScript, media engine, or browser event model compatibility.

When a browser CSS feature is adopted, DragonGUI should support the subset that
maps cleanly to the current widget tree, renderer, and runtime state model.

## Current Architecture Touchpoints

Implementation should primarily touch these areas:

| Area | Files | Role |
| --- | --- | --- |
| CSS parsing/lowering | `native/src/css_style.rs` | Parse selectors, properties, values, cascade, warnings. |
| Style model | `native/src/style.rs` | `NodeStyle`, `LayoutStyle`, `VisualStyle`, `TextStyle`, inline style parsing. |
| Theme/color resolution | `native/src/theme.rs`, `native/src/style.rs` | Color tokens, hex parsing, resolved RGBA. |
| Widget document | `native/src/document.rs` | Widget tree, props, inline styles, computed style attachment. |
| Layout | `native/src/layout.rs` | Taffy integration, sizing, margins, padding, grid/overflow later. |
| Primitive renderer | `native/src/primitives/mod.rs`, `native/src/primitives/rect.wgsl` | Rects, borders, radius, shadows, gradients, transforms. |
| Text renderer | `native/src/text/mod.rs` | Text entries, wrapping, clipping, alignment, typography controls. |
| Runtime state | `native/src/runtime.rs`, `native/src/events.rs` | Hover/focus/active/open/selected state, transitions, animation clock. |
| Framework defaults | `native/src/framework.dg.css` | Built-in baseline styles. |
| Python API | `python/dragongui/widgets.py`, `python/dragongui/app.py` | Inline style ergonomics, examples, possible helper APIs. |
| Debugging | `native/src/runtime.rs` | Style warnings and computed style snapshots. |

## Guiding Principles

1. Prefer incremental style-model additions over one large CSS rewrite.
2. Keep old stylesheets valid.
3. Preserve the current cascade model.
4. Add warnings for unsupported values instead of silently dropping them.
5. Make renderer behavior explicit in `docs/css-capabilities-reference.md`.
6. Implement low-risk text and color wins before high-risk render passes.
7. Keep pseudo-state layout stable. Stateful rules may animate paint, but should
   not change hit-test geometry unless the runtime explicitly supports it.

## Priority Order

| Priority | Feature | Effort | Visual Impact |
| --- | --- | --- | --- |
| 1 | `text-transform`, `letter-spacing`, `line-height`, `font-style`, `font-variant-numeric` | Low | High |
| 2 | `transparent`, named colors, `rgb()`, `rgba()`, `hsl()`, `hwb()` | Low | High |
| 3 | `text-overflow: ellipsis` | Low-Medium | Medium |
| 4 | `var(--name, fallback)` | Low | Medium |
| 5 | `box-shadow` | Medium | Very High |
| 6 | `linear-gradient()` / `radial-gradient()` backgrounds | Medium | Very High |
| 7 | CSS transitions | High | Very High |
| 8 | `:open`, `:expanded`, `:selected` | Low-Medium | High |
| 9 | Descendant selectors and deeper child chains | Medium | Medium |
| 10 | `:not()`, `:is()`, `:where()` | Medium | Medium |
| 11 | Percent sizing and `auto` | Medium | High |
| 12 | `calc()` | Medium | High |
| 13 | CSS Grid via Taffy | Medium | High |
| 14 | `overflow: hidden`, `scroll`, `auto` | Medium-High | High |
| 15 | `transform` | Medium-High | High |
| 16 | `position` and `z-index` | High | High |
| 17 | `@keyframes` animations | High | Medium-High |
| 18 | `backdrop-filter` | High | Medium |
| 19 | `::before` / `::after` generated content | High | Medium |

The first four items are the fastest path to improving the existing CSS demos.
They are mostly parser, style-model, and text-renderer work.

## CSS3-1: Typography Controls

### Scope

Add:

- `text-transform`
- `letter-spacing`
- `line-height`
- `font-style`
- `font-variant-numeric`
- `text-overflow: ellipsis`

### Pre-Milestone Spike

Before CSS3-1 implementation starts, spend a short spike validating the current
glyphon/text stack:

- Confirm whether glyphon exposes stable letter-spacing/tracking control.
- Confirm whether glyphon exposes OpenType feature control for tabular numbers.
- If either feature is unsupported, explicitly move that feature to a deferred
  renderer task before the milestone starts. Do not leave it as an ambiguous
  mid-milestone decision.

### Style Model

Extend `TextStyle` with:

```rust
pub enum TextTransform {
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

pub enum FontStyle {
    Normal,
    Italic,
}

pub enum FontVariantNumeric {
    Normal,
    TabularNums,
}

pub enum LineHeight {
    Multiplier(f32),
    LogicalPx(f32),
}

pub enum TextOverflow {
    Clip,
    Ellipsis,
}
```

Add `letter_spacing: Option<TextSpacing>` where `TextSpacing` starts with
logical pixels and can optionally support `em` values after the parser has a
safe representation.

### CSS Parsing

Supported values:

```css
Label.kicker {
    text-transform: uppercase;
    letter-spacing: 1.2px;
    line-height: 1.2;
    font-style: italic;
    font-variant-numeric: tabular-nums;
}

NavItem {
    text-overflow: ellipsis;
}
```

Initial value support:

- `text-transform`: `none`, `uppercase`, `lowercase`, `capitalize`.
- `letter-spacing`: unitless logical px, `px`. Consider `em` in the same slice
  if the value model can resolve it from font size without layout churn.
- `line-height`: unitless multiplier, `px`.
- `font-style`: `normal`, `italic`.
- `font-variant-numeric`: `normal`, `tabular-nums`.
- `text-overflow`: `clip`, `ellipsis`.

Unsupported values warn and are ignored.

### Text Renderer

Changes in `native/src/text/mod.rs`:

- Apply `text-transform` to displayed text only. Do not mutate widget values or
  callback payloads.
- Apply `font-style` when building glyphon text attributes.
- Apply `font-variant-numeric: tabular-nums` to dashboard numbers, table cells,
  badges, and labels if glyphon exposes OpenType feature control. If not,
  document it as deferred with the letter-spacing spike result.
- Apply `line-height` when calculating text entry line height and vertical
  placement.
- Apply `letter-spacing` if glyphon exposes a stable way to control tracking.
  If not, document it as parsed but renderer-limited and defer actual glyph
  spacing.
- Apply ellipsis only when text is single-line and exceeds its clip width.
  Multi-line ellipsis can be deferred.

### Layout Interaction

- `line-height` should affect text measurement and default control height where
  text metrics drive widget height.
- `text-transform` can change text width; text buffer cache keys must include
  the transformed display string and transform style.

### Acceptance Criteria

- Uppercase section labels can be styled without changing Python strings.
- Letter-spaced labels do not overlap adjacent text.
- Headings can use tighter line height than body text.
- Italic placeholder/caption styles render when the font backend supports them.
- Long tab/nav/dropdown labels can render with ellipsis instead of hard clipping.
- Numeric dashboard and table text can opt into tabular figures if the text
  backend supports it.
- Existing tests still pass.

### Tests

- CSS parser tests for each property.
- Inline style parser tests for snake-case aliases:
  `text_transform`, `letter_spacing`, `line_height`, `font_style`,
  `text_overflow`.
- Text rendering smoke example with long labels and ellipsis.
- Debug snapshot test proving computed text style includes the new fields.

## CSS3-2: Color Syntax And Transparency

### Scope

Add practical web color syntax:

- `transparent`
- Common named colors, at minimum `white`, `black`, `red`, `green`, `blue`,
  `gray`, `grey`, and `transparent`.
- `rgb(...)`
- `rgba(...)`
- `hsl(...)`
- `hsla(...)`
- `hwb(...)`
- `lab(...)`. Implemented by converting CIE Lab D50 into sRGB.
- `lch(...)`. Implemented by converting CIE LCH D50 into Lab, then sRGB.
- `oklab(...)`. Implemented by converting Oklab into sRGB.
- `oklch(...)`. Implemented by converting Oklch into sRGB.
- `color(srgb ...)`. Implemented by lowering direct sRGB channels to RGBA.
- `color(srgb-linear ...)`. Implemented by applying the sRGB transfer function
  to linear sRGB channels before lowering to RGBA.

Defer:

- `color(...)` spaces other than `srgb` and `srgb-linear`.
- Wide-gamut color spaces such as `display-p3`, `a98-rgb`, `prophoto-rgb`,
  and `rec2020`.

### Style Model

Keep the existing color resolution model where possible:

- Existing theme tokens continue to resolve through `ColorRef`.
- Literal colors lower to RGBA.
- Inline style dictionaries use the same practical web color parser for
  `transparent`, common named colors, `rgb()`/`rgba()`, `hsl()`/`hsla()`,
  `hwb()`, and CSS hex alpha forms.
- Python `Theme` color fields use the same practical web color parser for
  literal colors, without theme token or `var(...)` resolution.
- Unknown identifiers should keep warning behavior but should also appear in
  debug snapshot warnings. Falling back to `theme.danger` is useful visually but
  should not be the only signal.

### CSS Parsing

Supported examples:

```css
Button.ghost {
    background: transparent;
    border-color: rgba(255, 255, 255, 0.18);
}

Panel.hero {
    background: hsl(224, 20%, 12%);
}
```

Parsing rules:

- `rgb(255, 255, 255)` and `rgba(255, 255, 255, 0.5)`.
- Percent channels can be deferred if Lightning CSS does not lower them cleanly.
- `hsl()` converts to RGBA during lowering.
- `hwb()` converts to RGBA during lowering.
- `lab()`, `lch()`, `oklab()`, and `oklch()` convert to sRGB RGBA during
  lowering.
- `color(srgb ...)` and `color(srgb-linear ...)` convert to RGBA during
  lowering.
- Alpha is clamped to `0.0..1.0`.

### Acceptance Criteria

- Ghost buttons can use `background: transparent`.
- Semi-transparent overlays can use `rgba(...)`.
- Existing theme tokens and hex colors keep working.
- Unknown named colors warn clearly.

### Tests

- Parser tests for named colors and functional colors.
- Inline style parser tests for web color strings.
- Theme parser tests for web color strings.
- Shared parser tests for `lab()`, `lch()`, `oklab()`, and `oklch()`.
- Shared parser tests for `color(srgb ...)`, `color(srgb-linear ...)`, and
  rejection of wide-gamut `color(...)` spaces.
- Cascade test mixing tokens, hex, rgba, and variables.
- Example update showing translucent panels and ghost buttons.

## CSS3-3: Variable Fallbacks

### Scope

Support `var(--name, fallback)` when the whole property value is a `var()`,
and support embedded `var()` inside larger parseable property values.

Current system already supports `var(--name)` from `:root`. This slice makes
variables less fragile.

### CSS Parsing

Supported:

```css
Button {
    background: var(--button-bg, accent);
    border-radius: var(--button-radius, 6px);
    border: 1px solid var(--button-border, rgba(255, 255, 255, 0.2));
}
```

Still unsupported:

- Nested fallback expressions beyond one level unless trivial to support.
- Inherited variables across descendant widget subtrees. Selector-local
  variables inside the same declaration block are implemented.
- Cross-stylesheet variable sharing guarantees.

### Implementation Notes

- Extend variable lowering in `native/src/css_style.rs`.
- Preserve existing per-stylesheet variable resolution behavior.
- Fallback value should be parsed through the same property parser as a normal
  value.

### Acceptance Criteria

- Missing variable with fallback uses fallback.
- Embedded `var()` works inside borders, shadows, gradients, and transition
  shorthands.
- Missing variable without fallback warns and drops declaration.
- Existing `var(--name)` behavior remains unchanged.

## CSS3-4: Box Shadow

### Scope

Add `box-shadow` for rectangular widget surfaces.

Initial supported subset:

```css
Panel.card {
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.28);
}
```

Supported:

- One or more comma-separated outset or `inset` shadows.
- Offset X/Y.
- Blur radius.
- Optional spread radius.
- Color.
- Outset shadows are clipped against the inherited scroll/overflow paint
  viewport without shrinking the shadow shape to the visible widget rect.

Deferred:

- Shadow on arbitrary non-rect primitives.

### Style Model

Add to `VisualStyle`:

```rust
pub struct BoxShadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: ColorRef,
    pub inset: bool,
}
```

Store shadows as `Vec<BoxShadow>` from the start, even though the first
implementation should cap parsed CSS to zero or one shadow. An empty vec means
no shadow. Starting with a vec avoids a later debug snapshot and renderer API
migration when multiple comma-separated shadows are added.

### Renderer

Changes in `native/src/primitives/mod.rs` and shaders:

- Emit shadow instances before surface rect instances.
- Shadow rect should expand by spread and blur padding.
- Shadow should respect uniform and per-corner radii.
- Use an SDF-style soft edge or a separate shadow shader path.

### Layout Interaction

Box shadows should not affect layout or hit testing.

### Acceptance Criteria

- Panels, modals, dropdowns, menus, buttons, and toasts can show visible
  elevation.
- Shadow opacity and blur remain stable at different scale factors.
- Shadows do not cover widget text.
- No shadow is emitted for fully transparent shadow color or zero blur/spread.

### Tests

- Parser tests for valid and invalid `box-shadow`.
- Debug snapshot includes resolved shadow fields.
- Smoke demo with cards, modal, dropdown, and toast shadows.

## CSS3-5: Gradient Backgrounds

### Scope

Add CSS gradient backgrounds:

- `linear-gradient(...)`
- `radial-gradient(...)`

Initial support should focus on common two-to-six stop gradients.

### Style Model

Current visual backgrounds are color-oriented. Add a paint model rather than
overloading color refs too far:

```rust
pub enum BackgroundPaint {
    Color(ColorRef),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
}
```

Existing `background` / `background-color` declarations should continue to
store a solid color path.

### CSS Parsing

Supported examples:

```css
Button.primary {
    background: linear-gradient(180deg, #ff7a18, #af002d);
}

Panel.hero {
    background: radial-gradient(circle, rgba(255,255,255,0.2), transparent);
}
```

Initial supported subset:

- Linear angle in `deg`.
- `to bottom`, `to right`, and direct angle forms.
- Color stops with optional percent positions.
- Radial circle centered in the rect or positioned with `circle at <x> <y>`.

Deferred:

- Complex radial sizing keywords.
- URL/image-file background images.
- Advanced background layer options such as positioning, sizing, and blend
  modes.

### Renderer

Options:

1. Add gradient fields to rect instances.
2. Add a separate gradient rect pipeline.

Prefer a separate pipeline if it keeps the existing solid rect instance layout
stable.

### Acceptance Criteria

- Buttons and progress fills can use gradients.
- Panel hero/background gradients render without banding severe enough to be
  obvious.
- Solid backgrounds remain fast and unchanged.
- Gradients respect rounded rect clipping.

## CSS3-6: Interactive State Selectors

### Scope

Add public state selectors that reflect existing runtime state:

- `:open`
- `:expanded`
- `:collapsed`
- `:selected`

### State Mapping

| Pseudo-State | Widgets |
| --- | --- |
| `:open` | `Dropdown`, `Menu`, `ContextMenu`, `Modal`, possibly `Collapsible` when expanded. |
| `:expanded` | `Collapsible`. |
| `:collapsed` | `Collapsible`. |
| `:selected` | `Tab`, `NavItem`, dropdown selected item, table selected row/cell if exposed. |

Implementation should be conservative:

- Add state detection in runtime/style matching where current pseudo-states are
  evaluated.
- Keep the precomputed pseudo slot model.
- If the state cannot be determined from `WidgetState`, do not fake it.

### CSS Examples

```css
Collapsible:collapsed::indicator {
    color: muted_text;
}

Dropdown:open {
    border-color: accent;
}

Tab:selected {
    background: surface_alt;
}
```

### Acceptance Criteria

- Collapsible header can be styled differently when collapsed.
- Dropdown can show a distinct open state.
- Selected Tab and NavItem can be styled through CSS rather than only renderer
  defaults.

## CSS3-7: Descendant Selectors And Selector Functions

### Scope

Improve selector expressiveness:

- Descendant selectors: `Panel Button`.
- Multi-level child selectors: `Panel > HLayout > Button`.
- Attribute selector for key: `[key="main"]`.
- Attribute selectors for widget metadata and scalar props, such as
  `[disabled]`, `[level="info"]`, `[class~="pill"]`, and `[text^="Run"]`.
- `:not(...)`.
- `:is(...)`.
- `:where(...)`.
- First-slice `:has(...)` for descendant compound selectors, descendant
  selector chains, direct child arguments, direct child structural pseudos, and
  following sibling arguments with leading `+` or `~`; nested `:has(...)` is
  supported when the nested function is on the argument target or on an
  ancestor-side compound in descendant or direct-child argument chains.
- Universal selectors: `*`, `Panel > *`.
- Structural child selectors: `:first-child`, `:last-child`, `:only-child`,
  `:empty`, `:nth-child(...)`, and `:nth-last-child(...)` with integer,
  odd/even, `an+b` formulas, and `of <selector-list>` filters for compound
  selector lists and supported selector chains.

Status note: descendant selectors, multi-level child chains, `[key="..."]`,
`:first-child`, `:last-child`, `:only-child`, `:empty`, `:nth-child(...)`
integer/odd/even/an+b formulas, `:nth-last-child(...)` reverse formulas,
filtered `:nth-child(... of <selector-list>)`, and filtered
`:nth-last-child(... of <selector-list>)` for compound selector lists,
supported selector chains, and structural target filters, and selector
functions with compound selector arguments have landed.
Target pseudo-state
arguments inside selector functions have also landed. Universal selectors have
landed for whole compounds such as `*`, `*.class`, `*:hover`, and chain targets
such as `Panel > *`. Attribute selectors have landed for stable widget
metadata and parsed scalar props, including presence, string operators, and
case flags. First-slice `:has(...)` matching has landed for descendant
compound selectors and descendant selector chains. Optional leading `>` syntax
restricts an argument to direct children, and direct child structural pseudos
are supported. Leading `+` and `~` arguments now check immediate and later
following siblings. Nested `:has(...)` on the argument target now works, such
as `Panel:has(Panel:has(Button.primary))`, and nested `:has(...)` on an
ancestor-side compound in descendant or direct-child argument chains now works,
such as `Panel:has(Panel:has(> Badge.success) > Button.primary)`. Data-backed
target state pseudos now work inside `:has(...)` arguments and
`:nth-child(... of ...)` and `:nth-last-child(... of ...)` filters for
`:disabled`, `:checked`, `:open`, `:expanded`, and `:collapsed`.

### Implementation Notes

The current selector matcher already builds a target element and ancestor chain.
Use that instead of rewalking the tree per selector.

Needed model changes:

- Preserve selector combinator chains instead of lowering to only one optional
  parent.
- Add sibling index and sibling count to `StyleElement` or a companion
  `StyleContext`.
- Add key to style matching context for `[key="..."]`.
- Add a bounded attribute surface from widget metadata and parsed scalar props.
- For `:where()`, selector specificity is zero.
- For `:is()` and `:not()`, specificity follows CSS-compatible simplified
  behavior. Exact browser parity is not required, but it must be documented.

### Deferred

- Broader `:has(...)` support for dynamic stateful target selectors such as
  `:hover`, `:active`, `:focus`, and `:selected`, widget-part descendant,
  child, or sibling target selectors, plus sibling-relative nested `:has(...)`
  on ancestor-side selectors inside argument chains.
- Dynamic stateful and widget-part target selectors inside `:nth-child(... of
  <selector>)` and `:nth-last-child(... of <selector>)` filters.

### Acceptance Criteria

- `Panel Button` works.
- `Panel > HLayout > Button` works.
- `[key="primary-action"]` works.
- `[level="info"]`, `[disabled]`, and `[text^="Run"]` work.
- `Panel > *` works.
- `Panel > *:nth-child(2 of Button.primary)` works.
- `Panel > *:nth-child(2 of Panel > Button.primary)` works.
- `Panel > *:nth-child(1 of Button:first-child)` works.
- `Panel > Button:nth-last-child(2)` works.
- `Panel > *:nth-last-child(1 of Button.primary)` works.
- `Panel > Label:only-child` works.
- `Panel > Panel.empty-target:empty` works for no child widgets.
- `Panel:has(> Label:only-child)` works for direct child structural pseudos.
- `Panel:has(> Panel.empty-target:empty)` works for direct child structural
  pseudos.
- `Button:not(:disabled)` works.
- `:is(Button, Label).callout` works.
- `:where(.quiet)` works with zero specificity.
- `Button:is(:hover, :focus)` works.
- `Panel:has(Button.primary)` works for descendant compounds.
- `Panel:has(HLayout > Badge.success)` works for descendant selector chains.
- `Panel:has(> Button.primary)` works for direct child compounds.
- `Panel:has(> Button:first-child)` works for direct child structural pseudos.
- `Panel:has(+ Button.primary)` works for immediate following siblings.
- `Panel:has(~ Badge.success)` works for later following siblings.
- `Panel:has(Panel:has(Button.primary))` works for nested target `:has(...)`.
- `Panel:has(Panel:has(> Badge.success) > Button.primary)` works for nested
  ancestor-side `:has(...)` in direct-child argument chains.
- Unsupported complex selectors warn clearly.

## CSS3-8: Transitions

### Scope

Add CSS transitions for paint-oriented properties.

Initial support:

- `transition`
- `transition-property`
- `transition-duration`
- `transition-timing-function`
- `transition-delay`

Initial animatable properties:

- `background`
- `foreground`
- `border-color`
- `border-width`
- `border-radius`
- Per-corner radii.
- `opacity`
- `color`
- `accent`
- `track-color`
- `thumb-color`
- Shadow color/offset/blur if box-shadow has landed.

Defer:

- Layout transitions.
- Text size transitions.
- Grid/flex transitions.
- Complex gradient interpolation.

### Runtime Model

Add runtime animation state:

```rust
struct StyleTransition {
    widget_id: String,
    property: TransitionProperty,
    from: TransitionValue,
    to: TransitionValue,
    start_time: Instant,
    duration: Duration,
    delay: Duration,
    easing: Easing,
}
```

Runtime should:

- Compare previous computed/rendered style against new computed style after
  state changes.
- Start transitions for animatable changed fields.
- Request redraw while transitions are active.
- Resolve final style when complete.

### Transition Lifecycle Policy

Transitions need explicit cancellation and restart rules:

- Stylesheet, framework stylesheet, theme stylesheet, or inline style changes
  cancel all active transitions and apply the new computed style instantly.
- Widget removal cancels and removes all active transitions for that widget
  immediately.
- If the same widget/property starts a new transition while one is already
  active, sample the current interpolated value and use that as the new `from`
  value.
- If the changed property is not animatable, not listed in
  `transition-property`, or has `transition-duration: 0ms`, apply it instantly.
- Transition state is runtime-only and must not leak into debug snapshots as
  persistent computed style.

### Easing

Initial timing functions:

- `linear`
- `ease`
- `ease-in`
- `ease-out`
- `ease-in-out`
- `step-start`
- `step-end`
- `steps(n, start | end)`
- `cubic-bezier(x1, y1, x2, y2)`

Status note: custom cubic-bezier timing and first-slice step timing have landed
for transition and animation timing longhands and their one-item shorthands.
The cubic-bezier x control points are validated to the CSS `0..1` range; step
counts must be positive and support `start`/`end` plus `step-start`/`step-end`.

### Acceptance Criteria

- Button hover color eases instead of snapping.
- Dropdown open border/fill can transition.
- Toast opacity can animate if runtime toast lifecycle uses style transition
  hooks.
- No layout or hit-test desync occurs.

## CSS3-9: Percent, Auto, And Calc Values

### Scope

Add richer value resolution:

- Percent lengths where Taffy can represent them safely.
- `auto` where Taffy can represent it safely.
- `calc()` for length/number values.

### Pre-Milestone Gate

Before implementation, verify the current pinned Taffy version supports the
target sizing behavior:

- Percentage widths and heights in row and column flex containers.
- Percentage min/max sizes.
- Auto sizing for width, height, min/max, margin, and padding targets.
- The exact Taffy value types needed to represent percent, auto, and mixed
  calc results without forcing early conversion to logical pixels.

If the pinned Taffy version cannot represent a target property safely, either
defer that property or make the dependency upgrade an explicit prerequisite.

### Value Model

Current `length_px()` and `require_logical_px()` intentionally reject percent
and auto. This slice should replace those call sites with a typed value model:

```rust
pub enum CssLengthValue {
    LogicalPx(f32),
    Percent(f32),
    Auto,
    Calc(CssCalcExpr),
}
```

Do not collapse everything to `f32` during CSS lowering. Layout resolution needs
to happen later where parent dimensions and Taffy units are available.

### Initial Property Support

Enable percent/auto/calc first on:

- `width`
- `height`
- `min-width`
- `min-height`
- `max-width`
- `max-height`
- `padding` with logical pixels, percent, and compatible `calc()`.
- `margin` and margin longhands with logical pixels, percent, `auto`, and
  compatible `calc()`.
- `gap`, `row-gap`, and `column-gap` with logical pixels, percent, and
  compatible `calc()`.

Keep visual lengths like `border-radius` as logical px only for now.

### Calc Support

Started with a conservative first slice:

- Addition and subtraction.
- Multiplication/division by scalar for one length term.
- Compatible single-unit expressions that lower to all pixels or all percent.
- Mixed pixel/percent expressions when the parent axis is definite.

Still pending:

- Mixed percent/pixel expressions in fully auto-sized parent axes.

Examples:

```css
Panel.main {
    width: calc(100% - 280px);
}

Panel.sidebar {
    min-width: calc(220px + 40px);
}
```

### Acceptance Criteria

- `width: 50%` works for normal containers.
- `height: 100%` works where parent size is definite.
- `width: auto` maps to Taffy auto where supported.
- `calc(20% + 30%)` and `calc(220px + 40px)` work for sizing properties.
- `calc(100% - 240px)` works when the parent axis is definite.
- Unsupported calc unit combinations warn clearly.

## CSS3-10: CSS Grid

Status: In progress. First Taffy-backed grid slice landed for `display: grid`,
`grid-template-columns`, `grid-template-rows`, `grid-column`, `grid-row`,
`column-gap`, and `row-gap`. Track parsing currently supports px, percent,
`fr`, `auto`, `minmax()`, `fit-content()`, small-count and nested finite
`repeat(n, ...)`, and `repeat(auto-fit/auto-fill, ...)`. `grid-auto-flow`
has landed for `row`, `column`, and dense auto-placement.

### Scope

Expose Taffy grid through DragonGUI CSS.

Initial properties:

- `display: grid`
- `grid-template-columns`
- `grid-template-rows`
- `grid-template-areas`
- `grid-auto-flow`
- `grid-area`
- `grid-column`
- `grid-row`
- `column-gap`
- `row-gap`

Defer:

- Subgrid.
- Other auto-placement edge cases beyond Taffy's `grid-auto-flow` support.
- Nested auto-repeat syntax beyond the safe subset. Non-nested
  `repeat(auto-fit, ...)` and `repeat(auto-fill, ...)` are implemented.

### Implementation Notes

Before implementation, verify the current pinned Taffy version supports the
needed grid APIs. If not, this slice becomes a dependency upgrade task first.

### Supported Examples

```css
Panel.dashboard {
    display: grid;
    grid-template-columns: minmax(240px, 1fr) 1fr 1fr;
    grid-template-rows: auto 1fr;
    gap: 12px;
}

Panel.sidebar {
    grid-column: 1;
    grid-row: 1 / span 2;
}
```

### Acceptance Criteria

- A dashboard demo can place sidebar, header, main content, and inspector using
  CSS grid. First demo slice added to `examples/css_web_capabilities_demo.py`.
- The demo exercises dense `grid-auto-flow` auto-placement.
- Grid layout coexists with existing flex containers.
- Unsupported grid syntax warns instead of silently mislaying widgets.

## CSS3-11: Overflow And Scroll Containers

Status: In progress. First public overflow slice landed for `overflow`,
`overflow-x`, and `overflow-y`. `visible` lets children escape normal container
clipping, `hidden` clips, and `auto` / `scroll` opt containers into scroll
state. Vertical scrolling uses the normal wheel path; horizontal scrolling uses
horizontal wheel input or shift-wheel. Scroll containers now render vertical
and horizontal overlay scrollbar indicators for overflowing opted-in axes;
panels keep those indicators inset inside rounded panel surfaces, centered on
the panel surface, and clear of the bottom-right corner when both axes
overflow. Scrollbar thumbs can be dragged, and clicking the track updates the
scroll offset. Scrollable layout/container widgets expose
`::scrollbar-track` and `::scrollbar-thumb`; styling parts have landed for
track/thumb width, track axis inset, background, border, radius, and opacity.
PageUp/PageDown/Home/End keyboard scrolling has landed for the nearest
scrollable ancestor of the focused widget, with Shift using the horizontal axis
when available.

### Scope

Add general overflow handling:

- `overflow: visible`
- `overflow: hidden`
- `overflow: scroll`
- `overflow: auto`
- Optional `overflow-x` and `overflow-y`.

### Renderer Requirements

This is not only a layout property. It requires:

- Clip/scissor stack for primitives.
- Clip regions for text.
- Image clipping.
- Table clipping compatibility.
- Input hit-testing clipped to visible region.
- Scroll state per widget.

First slice limitations:

- Scrollbar styling is currently limited to scrollable layout/container widget
  track/thumb CSS parts.
- Overlay clipping remains renderer-managed and top-layer content is not clipped
  by normal containers.

### Overlay Pass Policy

DragonGUI overlays are renderer-managed top-layer content, not normal children
of the clipped widget subtree. `Tooltip`, dropdown menus, `Modal`,
`ContextMenu`, menus, and `Toast` should render in separate overlay passes after
normal content and must not be clipped by `overflow: hidden` on ancestor
containers.

If a future widget needs an overlay that is intentionally clipped by its parent,
that should be a separate explicit behavior, not the default overlay policy.

### Runtime Requirements

- Mouse wheel routes to nearest scrollable ancestor under cursor.
- Keyboard page up/down/home/end can be scoped to focused scroll container.
- Scroll offsets participate in child layout/rendering without changing widget
  identity.

### Acceptance Criteria

- Fixed-height panels can scroll overflowing content.
- `overflow: hidden` clips child primitives and text.
- Tooltips and dropdown overlays still escape normal content clipping when they
  are overlay-owned.

## CSS3-12: Transforms

### Scope

Add visual transforms:

- `transform: translate(...)`
- `transform: translateX(...)`
- `transform: translateY(...)`
- `transform: scale(...)`
- `transform: rotate(...)`

Optional longhands:

- `translate`
- `scale`
- `rotate`

### Design Decision

Initial transforms should be paint-only. They should not affect layout.

Hit testing can either:

1. Stay untransformed for the first slice and document that transforms are
   decorative.
2. Invert the transform for hit testing.

Recommendation: start with paint-only transforms for hover micro-interactions,
then add transformed hit testing if users need it.

### Renderer

Apply transforms consistently to:

- Primitive rects.
- Text entries.
- Images.

Scatter3D and complex overlays can be deferred.

### Acceptance Criteria

- `Button:hover { transform: translateY(-1px); }` renders.
- `Panel.card:hover { transform: scale(1.01); }` renders.
- Transformed widgets do not corrupt layout.
- `docs/css-capabilities-reference.md` documents that first-slice transforms
  are paint-only and hit testing follows the untransformed widget position.

## CSS3-13: Positioning And Z-Index

Status: In progress. First `position: relative` slice landed for paint-only
logical-pixel offsets through `top`, `right`, `bottom`, and `left`. First
`z-index` slice landed as local sibling ordering for normal widget surfaces and
text. First `position: absolute` slice landed for explicit/intrinsic-size
children placed by layout insets inside a parent layout context. First
`position: fixed` slice landed for explicit/intrinsic-size widgets pinned to
the logical viewport. `relative` layout and hit testing still use the unoffset
widget rectangle; full browser stacking contexts remain pending.

### Scope

Add explicit positioning:

- `position: static`
- `position: relative`
- `position: absolute`
- `position: fixed`
- `top`, `right`, `bottom`, `left`
- `z-index`

### Risk

This is a major layout model extension. It can conflict with:

- Taffy normal flow.
- Current overlay ordering.
- Modal/dropdown/menu/tooltip stacking.
- Hit testing.

### Recommended Approach

1. Implement `position: relative` as paint offset only.
2. Add `z-index` within a local stacking context for siblings.
3. Add `position: absolute` within positioned parent. Implemented as a first
   slice for explicit/intrinsic-size children.
4. Add `position: fixed` only after root/window coordinate behavior is stable.
   First viewport-inset slice implemented.

### Acceptance Criteria

- Badges can be absolutely positioned inside cards.
- Floating toolbar can be positioned in a panel.
- z-index controls sibling overlap predictably.
- Existing overlays keep priority over normal content unless explicitly
  documented otherwise.

## CSS3-14: Keyframes And Continuous Animation

Status: In progress. First visual keyframes slice landed for `@keyframes`
parsing/lowering, animation longhands, a global runtime animation clock, redraw
requests while animations are active, and visual interpolation/reuse of the
transition paint interpolation model. The first slice supports one animation
per widget and visual declarations only. The `animation` shorthand is now
implemented for the supported longhand fields, including first-slice
`animation-play-state: running | paused`. Animation shorthand and longhand
comma lists now accept the first item for the one-animation slice. Timing
functions include standard keywords, custom `cubic-bezier(...)`, `step-start`,
`step-end`, and `steps(n, start | end)`. Finite fractional
`animation-iteration-count` values are preserved and resolve forward-filled
final state at the corresponding partial iteration. Negative `animation-delay`
values are accepted and start animations partway through their timeline.
Multiple simultaneous animations, layout/text animation, composition,
timelines, and OS reduced-motion preference integration remain pending. CSS
`prefers-reduced-motion` media matching has landed and defaults to
`no-preference` until DragonGUI receives an OS preference hook. Outline color,
width, and offset now participate in the same visual transition and keyframe
interpolation path as other numeric/color paint fields.

### Scope

Add:

- `@keyframes`
- `animation-name`
- `animation-duration`
- `animation-timing-function`
- `animation-delay`
- `animation-iteration-count`
- `animation-direction`
- `animation-fill-mode`
- `animation-play-state`. First slice supports `running` and `paused`.
- `animation` shorthand. Implemented for one animation using supported fields.
- Comma-separated animation shorthand and longhand lists. Implemented by using
  the first item for the one-animation slice.
- Fractional finite `animation-iteration-count` values. Implemented.
- Negative `animation-delay`. Implemented for longhand and one-animation
  shorthand.

Initial animatable properties should match transition support.

### Runtime Requirements

- Global animation clock.
- Per-widget active animation state.
- Redraw requests while animations are active.
- Pause/cleanup when widget leaves tree.
- Respect OS reduced-motion setting if DragonGUI adds one. First CSS media
  query gate implemented with a default `no-preference` environment.

### Acceptance Criteria

- Spinner/progress pulse demo can animate without Python timers.
- Animation stops when widget is removed.
- Infinite animations do not leak state.

## CSS3-15: Backdrop Filter

Status: In progress. First CSS/style-model slice landed for
`backdrop-filter: blur(px)` with debug/computed-style visibility and a
rounded frosted-surface renderer treatment for `Panel`, `Modal`, `Tooltip`,
and `Toast`. `brightness()` / `saturate()` and whitespace-separated filter
lists now parse into the style model and influence the first-slice tint
treatment. `Modal::scrim` is exposed as a renderer-owned CSS part for custom
scrim background and opacity. This is not the full sampled framebuffer blur
yet; true backdrop sampling still needs an offscreen scene texture and blur
pass.

### Scope

Add a limited `backdrop-filter` subset:

- `blur(px)`
- `brightness(number | percent)`
- `saturate(number | percent)`
- Whitespace-separated filter lists using the supported functions.

### Renderer Requirements

Backdrop filtering requires rendering previous content into an offscreen texture
and sampling it behind the filtered widget. This is a separate rendering
architecture task, not a simple style property.

### Recommended Use

Only for:

- Modal scrims.
- Floating panels.
- Toast surfaces.

### Acceptance Criteria

- Modal scrim can be styled through `Modal::scrim`; true blur of content behind
  it remains pending.
- Blur respects rounded rect clipping.
- GPU cost is documented.

## CSS3-16: Generated Content

Status: In progress. First generated-content slice landed by reusing the
renderer part selector path for `::before` and `::after`, adding
`content: "..."` and `content: attr(name)` to part styles, and rendering
generated prefix/suffix text as non-interactive overlay text. Generated content
still does not participate in layout and does not support counters or
independent generated boxes.

### Scope

Add limited generated content:

- `::before`
- `::after`
- `content: "..."`
- `content: attr(...)`. Implemented for widget metadata and serialized scalar
  props.

### Design Constraint

Generated content should be renderer-owned virtual content, not real widgets.
It should not have:

- IDs.
- Focus.
- Callbacks.
- Hit-test targets.
- Children.

### Initial Supported Use Cases

- Decorative bullets.
- Prefix/suffix labels.
- Small generated badges.

### Deferred

- Counter functions.
- Generated content with independent layout.

## CSS3-17: Responsive At-Rules And Fonts

Status: First `@media` implementation slice landed. Stylesheets now lower
viewport width/height/aspect-ratio/resolution/prefixed device-pixel-ratio aliases/device-width/device-height/
device-aspect-ratio/horizontal-viewport-segments/vertical-viewport-segments/
color/color-index/monochrome/color-gamut/video-color-gamut/orientation/scan/grid/environment-blending,
pointer/hover/nav-controls/overflow-block/overflow-inline/update/scripting/forced-colors capability, and
`prefers-contrast` / `inverted-colors` / `dynamic-range` /
`video-dynamic-range` / `display-mode` / `prefers-color-scheme` /
`prefers-reduced-motion` / `prefers-reduced-transparency` /
`prefers-reduced-data` media rules into rule metadata, cascade matching
filters them against the current logical window size, platform window theme
when available, active theme fallback color scheme, assumed desktop
input/display capabilities, and scale factor, and the runtime reapplies CSS on
layout/resize/theme-change events. First static `@supports`
implementation slice landed for DragonGUI declaration, selector, and
`font-format(...)` / `at-rule(...)` / `font-tech(...)` feature queries. Container queries remain pending. First
`@font-face` slice landed for installed local font families referenced through
`local(...)`, local `.ttf`, `.otf`, `.ttc`, and `.woff` files referenced through
path or `file://` `url(...)`, and base64 `data:` URLs containing sfnt or WOFF1
font data. Font `format(...)` descriptors are honored for supported formats and
unsupported descriptors are skipped.

Code-backed remaining work:

- Container queries.
- Broader/custom media-query parity beyond the standard feature ids currently
  parsed by lightningcss and lowered by DragonGUI.
- Platform detection for display gamut, pointer/hover capability, forced-colors
  mode, contrast preference, inverted-colors mode, dynamic-range capability,
  OS reduced-motion preference, reduced-transparency preference, and
  reduced-data preference. The runtime currently assumes `srgb`, fine pointer,
  hover support, `nav-controls: none`, `update: fast`, `scripting: none`, `forced-colors: none`,
  `prefers-contrast: no-preference`, `inverted-colors: none`,
  `prefers-reduced-motion: no-preference`,
  `prefers-reduced-transparency: no-preference`, and
  `prefers-reduced-data: no-preference`; both dynamic-range features currently
  default to `standard`, `display-mode` defaults to `standalone`, and
  overflow-block/inline default to `scroll`. `color` defaults to `8`,
  `color-index` defaults to `0`, `monochrome` defaults to `0`, `scan` defaults
  to `progressive`, `grid` defaults to `0`, and `environment-blending`
  defaults to `opaque`. `device-width` / `device-height` /
  `device-aspect-ratio` mirror the logical app viewport, and both viewport
  segment features default to `1`; `video-color-gamut` mirrors `color-gamut`.
  `-webkit-device-pixel-ratio` and `-moz-device-pixel-ratio` map to the same
  current dppx scale factor as `resolution`.
  Color
  scheme now uses winit's
  platform window theme when available and falls back to active DragonGUI theme
  luminance when unavailable.
- Full inherited custom property cascade semantics. Top-level `:root`
  variables, first-slice parse-time `:root` variables inside matching
  `@media` / static true `@supports` blocks, and selector-local variables
  used by declarations in the same rule block are implemented.
- Remote font URLs, WOFF2 loading, and packaged font asset resolution.

### Scope

Later-stage browser-like features:

- `@media`.
- `@supports`.
- Container queries.
- `@font-face`.

### `@media`

Initial useful subset:

```css
@media (max-width: 900px) {
    Panel.sidebar {
        display: none;
    }
}
```

Needs:

- Runtime viewport width/height in style context. Implemented for logical
  window size.
- Reapply styles on window resize when media query match changes. Implemented.
- Remaining: container queries, broader/custom media-query parity, OS
  reduced-motion preference integration, OS reduced-transparency/data
  preference integration, platform forced-colors/contrast/inverted-colors/
  dynamic-range detection, platform display-mode detection, platform
  input/display capability detection, and full inherited media-scoped custom
  property cascade semantics. First-slice parse-time `:root` variables inside
  matching `@media` and static true `@supports` blocks, plus selector-local
  variables used in the same declaration block, are implemented.

### `@supports`

Initial useful subset:

```css
@supports (display: grid) and (selector(Panel > Button.primary)) {
    Panel.dashboard {
        display: grid;
    }
}
```

Needs:

- Declaration feature queries that reuse DragonGUI property lowering.
  Implemented.
- Selector feature queries that reuse DragonGUI selector subset parsing.
  Implemented.
- `not`, `and`, and `or` conditions. Implemented.
- Nesting with supported `@media` rules. Implemented.
- `font-format(...)` support for DragonGUI's current font loader formats.
  Implemented for `truetype`, `opentype`, `collection`, and `woff`, plus local
  file-extension aliases `ttf`, `otf`, and `ttc`; `woff2`,
  `embedded-opentype`, and `svg` currently evaluate false.
- `at-rule(...)` support for DragonGUI's current at-rule parser surface.
  Implemented for `media`, `supports`, `keyframes`, and `font-face`;
  unsupported at-rules such as `container` currently evaluate false.
- `font-tech(...)` support for DragonGUI's current text shaping features.
  Implemented for `features-opentype`; color font technologies, palettes,
  incremental font transfer, and variable-font feature claims currently
  evaluate false.
- Remaining: browser capability parity and feature functions beyond
  `selector(...)`, `font-format(...)`, `at-rule(...)`, and `font-tech(...)`.

### Container Queries

Defer until layout can expose container dimensions back into style matching
without creating feedback loops.

### `@font-face`

First useful subset:

```css
@font-face {
    font-family: "Report UI";
    src:
        local("Segoe UI"),
        url("file:///C:/Windows/Fonts/segoeui.ttf") format("truetype");
}
```

Implemented:

- Parse `@font-face` `font-family`, `local(...)`, and local `url(...)` `src`
  descriptors into DragonGUI stylesheet IR.
- Merge font-face records across framework/theme/user stylesheet origins.
- Map declared CSS families to installed local font families when a matching
  `local(...)` source is present.
- Load supported local font files into glyphon's font database at text rebuild.
- Accept local path and `file://` font URLs, including percent-escaped
  `file://` paths.
- Honor supported `format(...)` descriptors and skip unsupported descriptors.
- Decode base64 `data:` font URLs containing sfnt TrueType/OpenType/TTC data
  into glyphon's font database at text rebuild.
- Decode local `.woff` files and base64 `data:` WOFF1 font URLs into sfnt data
  before loading them into glyphon's font database at text rebuild.
- Map the declared CSS family to the loaded font's exposed family name when
  available.
- Report one-time renderer diagnostics for missing, unsupported, or unusable
  font sources through the debug snapshot.
- Demo coverage in `examples/css_web_capabilities_demo.py`.

Remaining planning decisions:

- Decide whether DragonGUI ships bundled fonts, only supports user-provided
  font files, or supports both. Current implementation supports installed
  local font families, user-provided local path or `file://` sfnt/WOFF1 font
  files, and base64 sfnt/WOFF1 data URLs.
- Estimate wheel/sdist size impact before adding bundled assets.
- Define how font files are referenced from CSS in packaged applications.
- Update maturin/pyproject packaging rules before the renderer work starts.

Remaining:

- Remote URLs.
- WOFF2 loading.
- Packaging story for wheels/sdists.

## Implementation Milestones

### Milestone A: Fast Typographic And Color Wins

Status: In progress. First implementation slice landed for typography
style-model/rendering, web color syntax, variable fallbacks, and embedded
`var()` substitution inside larger parseable values.

Implement:

- `text-transform`
- `line-height`
- `font-style`
- `font-variant-numeric: tabular-nums` if the text backend supports it
- `letter-spacing` after the glyphon tracking spike confirms support
- `text-overflow: ellipsis`
- `transparent`
- named colors
- `rgb()` / `rgba()`
- `hsl()` / `hsla()`
- `hwb()`
- `var(--name, fallback)`, including embedded `var()` inside larger parseable
  property values.

Why first:

- Low risk.
- High demo impact.
- Mostly isolated to `css_style.rs`, `style.rs`, `theme.rs`, and
  `text/mod.rs`.

### Milestone B: Elevation And Rich Paint

Status: In progress. `box-shadow` landed for comma-separated outset and inset
shadows on rect-backed surfaces, including clipping outset shadows against
inherited scroll/overflow paint viewports. First `linear-gradient()` and
`radial-gradient()` slices landed for rect-backed backgrounds. Gradient
rendering now interpolates up to six stop colors, sampling longer gradients
down to six GPU stops. Transparent fades use premultiplied-alpha interpolation
to avoid dark halos, and gradient stop blending can opt into `linear-srgb` or
`oklab` color spaces. `blob-gradient(...)` has landed as a DragonGUI-specific
organic background paint, and `mesh-gradient(...)` has landed as a
DragonGUI-specific four-corner image-like gradient. First-slice repeating gradients have landed for
explicit final stop ranges. Comma-separated background layers now paint
back-to-front, enabling radial glow overlays on linear bases. `background-noise`
has landed as a low-amplitude procedural dither/noise pass for softening
gradient banding on rect-backed backgrounds. `background-image` now accepts the
same gradient and layered gradient paint subset, layers over
`background-color`, and supports `none`; URL/image-file background sources
remain deferred. Uniform `border-style` longhand support has landed for
`solid`, `none`, and `hidden`, mapping to the current uniform border renderer.
First-slice `outline`, `outline-color`, `outline-width`, `outline-style`, and
`outline-offset` support has landed as a paint-only solid/none ring for
rect-backed widget surfaces.

Implement:

- `box-shadow`
- `linear-gradient()`
- `radial-gradient()`
- `border-style` for the uniform `solid`/`none`/`hidden` subset. Implemented.
- `outline` and outline longhands for the solid/none paint-only subset.
  Implemented.

Why second:

- Biggest visual improvement after typography.
- Requires renderer work but not layout model changes.

### Milestone C: State-Aware Styling

Status: In progress. Public state selectors landed for `:open`, `:expanded`,
`:collapsed`, and `:selected` with whole-widget and part styling. Selector
chains also landed for descendants, deeper direct-child chains, `[key="..."]`,
`:first-child`, `:last-child`, `:nth-child(...)` integer/odd/even/an+b
formulas, `:nth-last-child(...)` reverse formulas, `:only-child`, `:empty`,
and filtered `:nth-child(... of <selector-list>)` and
`:nth-last-child(... of <selector-list>)` for compound selector lists,
supported selector chains, and structural target filters.
Selector functions have landed for compound selector arguments, including
target pseudo-state arguments.
Universal selectors have landed for
`*`, `*.class`, `*:hover`, and chain targets such as `Panel > *`. Attribute
selectors have landed for widget metadata and parsed scalar props, including
presence, string operators, and case flags. First-slice `:has(...)` support
has landed for descendant compound selectors, descendant selector chains,
direct child arguments, direct child structural pseudos, and following sibling
arguments with leading `+` or `~`. Nested `:has(...)` on the argument target
and on ancestor-side compounds in descendant or direct-child argument chains
has also landed. Data-backed state target arguments for `:has(...)` and
`:nth-child(... of ...)` and `:nth-last-child(... of ...)` have landed for
`:disabled`, `:checked`, `:open`, `:expanded`, and `:collapsed`.

Implement:

- `:open`
- `:expanded`
- `:collapsed`
- `:selected`
- Descendant selectors. Implemented.
- Multi-level child chains. Implemented.
- `[key="..."]`. Implemented.
- Attribute selectors. Implemented for widget metadata and parsed scalar props,
  including presence, exact, word, prefix, suffix, substring, dash-match, and
  explicit ASCII case-sensitivity flags.
- Structural child selectors. Implemented for `:first-child`, `:last-child`,
  integer `:nth-child(n)`, `:nth-child(odd)`, `:nth-child(even)`, and `an+b`
  formulas such as `3n+1` and `-n+3`, plus `:nth-last-child(...)` with the
  same pattern syntax. Filtered `:nth-child(... of <selector-list>)` and
  `:nth-last-child(... of <selector-list>)` are implemented for compound
  selector lists, supported descendant or child selector chains, and
  structural target filters. Data-backed state target filters are implemented
  for `:disabled`, `:checked`, `:open`, `:expanded`, and `:collapsed`.
- Selector functions such as `:not()`, `:is()`, and `:where()`. Implemented
  for compound selector arguments, including target pseudo-state arguments.
- First-slice `:has(...)`. Implemented for descendant compound selectors,
  descendant selector chains, direct child arguments, and direct child
  structural pseudos, immediate and later following sibling arguments, and
  nested `:has(...)` on the argument target and on ancestor-side compounds in
  descendant or direct-child argument chains. Data-backed state target
  arguments are implemented for `:disabled`, `:checked`, `:open`, `:expanded`,
  and `:collapsed`.
- Universal selectors. Implemented for whole compounds and selector chains.

Why third:

- Makes CSS easier to write and lets users style interactive widgets without
  Python-side classes.

### Milestone D: Motion

Status: In progress. First CSS transition slice landed for parser/style-model
support, debug snapshots, whole-widget hover/`:focus`/`:active` paint transitions, and
whole-widget `:checked`, `:open`, and `:selected` paint transitions for tracked
widgets. First paint-only transform slice also landed for `translate`, `scale`, and `rotate`
on rect-backed widget surfaces, with CSS transform longhands now lowering into
the same transform model. Runtime support currently interpolates solid color,
numeric visual fields, and transform fields. Stylesheet changes, inline style
patches, full layout rebuilds, and widget replacement paths now cancel active
style transition progress. `transition-property` is honored for hover, `:focus`,
`:active`, `:open`, `:checked`, `:selected`, and expanded/collapsed whole-widget
transitions, with unlisted visual fields snapping to the current state. Timing
functions include the standard easing keywords and custom `cubic-bezier(...)`
curves. Outline color, width, and offset now interpolate through both runtime transitions and
visual keyframes. Paint transforms now apply to widget render subtrees:
descendant primitive rects, scrollbars, and image textures follow ancestor
transforms, while text follows translate and uniform scale. Layout transitions,
text rotation, and layout animation for expanding/collapsing content remain
pending. First visual `@keyframes` animation support has also landed on
top of this interpolation model, with one shorthand/animation per widget and
visual declarations only.

Implement:

- Paint-only CSS transitions. First whole-widget hover, `:focus`, `:active`,
  `:checked`, `:open`, and `:selected` slices implemented. Expanded/collapsed
  whole-widget paint transitions also implemented.
- Timing functions. Implemented for `linear`, `ease`, `ease-in`, `ease-out`,
  `ease-in-out`, `step-start`, `step-end`, `steps(n, start | end)`, and
  `cubic-bezier(...)`.
- Paint-only transforms. First rect-surface slice implemented for
  `transform` plus `translate`, `scale`, and `rotate` longhands. Descendant
  primitive rects, scrollbars, and image textures follow transformed widget
  subtrees; text follows translate and uniform scale.
- Continuous animation. First visual `@keyframes` and animation longhand slice
  implemented. `animation` shorthand implemented for one supported animation.
  `animation-play-state` implemented for `running` and `paused`. Fractional
  finite iteration counts are preserved, and negative animation delays are
  supported.

Why fourth:

- High perceived quality.
- Needs runtime animation infrastructure and should come after richer paint
  values exist.

### Milestone E: Responsive Layout

Status: In progress. First percent/auto sizing slice landed for `width`,
`height`, `min-width`, `min-height`, `max-width`, and `max-height`, carrying
typed layout values through CSS lowering into Taffy. First `calc()` sizing slice
landed for pixel, percent, and mixed pixel/percent sizing expressions when the
parent axis is definite. The follow-up typed spacing slice landed for padding,
margin shorthand/longhands, `gap`, `row-gap`, and `column-gap`; margin also
supports `auto`. `var()` terms inside `calc()` are supported for lengths and
scalar multipliers/divisors. Mixed-unit calc in fully auto-sized parent axes
remains pending. The CSS Grid slice now includes named
`grid-template-areas` and child `grid-area` placement for rectangular named
regions, plus nested finite `repeat(n, ...)` track lists.

Implement:

- Percent sizing. First sizing-property slice implemented.
- `auto` sizing. First sizing-property slice implemented.
- `calc()`. First sizing slice implemented, including mixed pixel/percent
  expressions when the parent axis is definite.
- Typed spacing. First padding, margin shorthand/longhand, and gap slice
  implemented.
- CSS Grid. First Taffy-backed track and placement slice implemented. Named
  `grid-template-areas` and `grid-area` placement implemented for rectangular
  regions. `minmax()`
  track parsing/lowering implemented for px/percent/auto minimums and
  px/percent/fr/auto maximums. `fit-content()` track parsing/lowering
  implemented for px and percent arguments. Auto-repeat parsing/lowering
  implemented for `repeat(auto-fit, ...)` and `repeat(auto-fill, ...)`.
  Nested finite repeat track lists are flattened before layout.
  `grid-auto-flow` parses and lowers row/column dense auto-placement.

Why fifth:

- More layout risk.
- Needs careful Taffy integration and more test coverage.

### Milestone F: Overflow And Scroll Containers

Implement:

- General `overflow`. First visible/hidden/auto/scroll slice implemented.
- Scroll state. Vertical and horizontal scroll offsets now work for opted-in
  containers; scroll containers now render vertical and horizontal overlay
  indicators with mouse click/drag operation. PageUp/PageDown/Home/End keyboard
  scrolling now targets the focused scroll context, with mouse-position
  fallback. Scrollbar track/thumb CSS parts now apply to scrollable
  layout/container widgets.
- Clip stack. First explicit visible/hidden child clip behavior implemented.
- TextArea sizing. DragonGUI-specific `text-area-rows` is implemented as a
  widget property that drives the TextArea preferred height while preserving
  `height` as the exact-size override.
- DataFrameTable sizing. Uniform `table-column-width` and
  `table-index-width` are implemented through the table metrics path used by
  rendering, visible range calculation, and hit testing.
- Scatter3D styling. Uniform `scatter-point-size` is implemented as a
  widget-specific property backed by the scatter shader uniform path, so point
  size can change without repacking point data. Rounded Scatter3D clipping is
  implemented for the 3D viewport and picking region using computed per-corner
  border radii.

Why separate:

- Touches layout, rendering, event routing, and hit testing.

### Milestone G: Advanced Browser Features

Evaluate and implement selectively:

- Positioning and z-index. First paint-only relative positioning, local sibling
  `z-index`, layout-backed `position: absolute`, and viewport-backed
  `position: fixed` slices implemented; full stacking contexts remain pending.
- `@keyframes`. First visual animation slice and one-animation shorthand
  implemented; broader animation compatibility remains pending.
- Backdrop filter. First `blur(px)` frosted-surface treatment implemented;
  `brightness()` / `saturate()` and supported filter lists now influence that
  treatment. True sampled framebuffer blur remains pending.
- Generated content. First text-only `::before` / `::after` and `attr(...)`
  slice implemented; layout-aware generated boxes remain pending.
- `@media`. First viewport
  `width`/`height`/`aspect-ratio`/`resolution`/
  `-webkit-device-pixel-ratio`/`-moz-device-pixel-ratio`/`device-width`/
  `device-height`/`device-aspect-ratio`/`horizontal-viewport-segments`/
  `vertical-viewport-segments`/`color`/`color-index`/
  `monochrome`/`color-gamut`/`video-color-gamut`/`orientation`/`scan`/`grid`/
  `environment-blending`/
  `pointer`/`any-pointer`/`hover`/`any-hover`/`nav-controls`/`overflow-block`/
  `overflow-inline`/`update`/`scripting`/
  `forced-colors`/`prefers-contrast`/`inverted-colors`/`dynamic-range`/
  `video-dynamic-range`/`display-mode`/`prefers-color-scheme`/
  `prefers-reduced-motion`/`prefers-reduced-transparency`/
  `prefers-reduced-data` slice implemented;
  broader/custom media parity and platform-backed detection remain pending.
- `@supports`. First static declaration and selector feature-query slice
  implemented.
- `@font-face`. First installed-family, local sfnt/WOFF1 font-file, and
  base64 sfnt/WOFF1 data-URL loading slice implemented; remote fonts, WOFF2, and packaged asset
  resolution remain pending.

These should not block the earlier visible wins.

## Testing Strategy

### CSS Parser Tests

Add focused tests in `native/src/css_style.rs` for:

- Every new property name.
- Valid value parsing.
- Unsupported value warnings.
- Cascade behavior.
- Debug labels for matched rules.
- Selector specificity changes.

### Style Model Tests

Add tests for:

- Inline style dictionary parsing.
- Computed style serialization in debug snapshots.
- Backward compatibility for old fields.

### Renderer Smoke Tests

Add or update examples:

- `examples/css_web_capabilities_demo.py`
- `examples/css_theme_gallery.py`
- `examples/meridian.py`
- `examples/all_features_css_demo.py`

Each milestone should add a visible demo section proving the feature works.

### Runtime Tests

Needed for:

- Transitions completing and cleaning up.
- Widget removal during active animation.
- Open/selected/expanded pseudo-states updating on interaction.
- Scroll container input routing.

### Performance Tests

Add benchmarks or smoke checks for:

- 100 panels with shadows.
- Tables inside rounded/overflow containers.
- Repeated hover transitions.
- Large stylesheet matching after descendant selectors.

## Documentation Updates

Each milestone must update:

- `docs/css-capabilities-reference.md`
- `docs/widgets-reference.md` when widget-specific behavior changes.
- Relevant V2 plan checklist status.
- Demo comments only where needed.

Do not document a feature as supported until it has parser support, renderer
support, and at least one test or smoke example.

## Risks

### Shadow And Gradient Shader Complexity

Changing the rect shader or instance layout can break every widget surface.
Prefer additive pipelines if that keeps existing solid rect rendering stable.

### Transition State Explosion

Transitions can add per-widget runtime state and redraw pressure. Limit the
first slice to paint properties and clean up finished animations aggressively.

### Selector Performance

Descendant selectors and selector functions can multiply match work. Use the
existing ancestor chain and avoid per-selector tree walks.

### Layout Feedback Loops

Percent, calc, grid, container queries, and auto sizing can create feedback
between style and layout. Keep style resolution and layout resolution separate.

### Overflow Touches Everything

General clipping and scrolling affect primitives, text, images, tables,
hit-testing, and overlays. Treat it as its own milestone.

## Success Criteria

Part 3 is successful when:

- Existing CSS demos visibly improve without Python-side layout hacks.
- A web-style card/dashboard UI can use typography hierarchy, translucent
  colors, shadows, gradients, and transitions.
- Users can write common CSS like `Panel Button`, `Button:not(:disabled)`,
  `width: 50%`, and `background: rgba(...)`.
- The debug snapshot clearly reports new computed fields and warnings.
- Existing V1/V2 widgets remain stable and old stylesheets keep working.
