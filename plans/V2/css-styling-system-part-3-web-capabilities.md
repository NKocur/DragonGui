# CSS Styling System Part 3: Web UI Capabilities Plan

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
- Transition easing now includes standard keywords and custom cubic-bezier
  curves; broader animation support is still pending.
- Limited typography controls.
- Limited color syntax.
- Limited responsive layout primitives.
- First interactive state selector and selector-chain slices landed for
  `:open`, `:expanded`, `:collapsed`, `:selected`, descendant selectors,
  multi-level child chains, `[key="..."]`, and simple structural child
  selectors. Selector functions have landed for compound selector arguments,
  including target pseudo-state arguments. Universal selectors and attribute
  selectors for widget metadata/scalar props have also landed, including
  presence, string operators, and case flags.

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
| 2 | `transparent`, named colors, `rgb()`, `rgba()`, `hsl()` | Low | High |
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

Defer:

- `oklch(...)`
- `lab(...)`
- `color(...)`
- Wide-gamut color spaces.

### Style Model

Keep the existing color resolution model where possible:

- Existing theme tokens continue to resolve through `ColorRef`.
- Literal colors lower to RGBA.
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
- Alpha is clamped to `0.0..1.0`.

### Acceptance Criteria

- Ghost buttons can use `background: transparent`.
- Semi-transparent overlays can use `rgba(...)`.
- Existing theme tokens and hex colors keep working.
- Unknown named colors warn clearly.

### Tests

- Parser tests for named colors and functional colors.
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
- Scoped variables.
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

Deferred:

- Shadow clipping against scroll containers.
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

Initial support should focus on common two-to-four stop gradients.

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
- Background images.
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
- Universal selectors: `*`, `Panel > *`.
- Structural child selectors: `:first-child`, `:last-child`, and
  `:nth-child(...)` with integer, odd/even, `an+b` formulas, and
  `of <selector-list>` filters for compound selector lists and supported
  selector chains.

Status note: descendant selectors, multi-level child chains, `[key="..."]`,
`:first-child`, `:last-child`, `:nth-child(...)` integer/odd/even/an+b
formulas, filtered `:nth-child(... of <selector-list>)` for compound selector
lists and supported selector chains, and selector functions with compound
selector arguments have landed.
Target pseudo-state
arguments inside selector functions have also landed. Universal selectors have
landed for whole compounds such as `*`, `*.class`, `*:hover`, and chain targets
such as `Panel > *`. Attribute selectors have landed for stable widget
metadata and parsed scalar props, including presence, string operators, and
case flags.

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

- `:has(...)`.
- Stateful, structural, and widget-part target selectors inside
  `:nth-child(... of <selector>)` filters.

### Acceptance Criteria

- `Panel Button` works.
- `Panel > HLayout > Button` works.
- `[key="primary-action"]` works.
- `[level="info"]`, `[disabled]`, and `[text^="Run"]` work.
- `Panel > *` works.
- `Panel > *:nth-child(2 of Button.primary)` works.
- `Panel > *:nth-child(2 of Panel > Button.primary)` works.
- `Button:not(:disabled)` works.
- `:is(Button, Label).callout` works.
- `:where(.quiet)` works with zero specificity.
- `Button:is(:hover, :focus)` works.
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
- `cubic-bezier(x1, y1, x2, y2)`

Status note: custom cubic-bezier timing has landed for transition longhands and
the one-item transition shorthand. The x control points are validated to the CSS
`0..1` range.

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
- uniform `margin` with logical pixels, percent, `auto`, and compatible
  `calc()`.
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
`fr`, `auto`, `minmax()`, `fit-content()`, small-count `repeat(n, ...)`, and
`repeat(auto-fit/auto-fill, ...)`.

### Scope

Expose Taffy grid through DragonGUI CSS.

Initial properties:

- `display: grid`
- `grid-template-columns`
- `grid-template-rows`
- `grid-column`
- `grid-row`
- `column-gap`
- `row-gap`

Defer:

- Named grid areas.
- Subgrid.
- Auto-placement edge cases.
- Complex or nested repeat syntax beyond the safe subset.

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
- Grid layout coexists with existing flex containers.
- Unsupported grid syntax warns instead of silently mislaying widgets.

## CSS3-11: Overflow And Scroll Containers

Status: In progress. First public overflow slice landed for `overflow`,
`overflow-x`, and `overflow-y`. `visible` lets children escape normal container
clipping, `hidden` clips, and `auto` / `scroll` opt containers into scroll
state. Vertical scrolling uses the normal wheel path; horizontal scrolling uses
horizontal wheel input or shift-wheel. Scrollable panels now render inset
vertical and horizontal overlay scrollbars that stay inside rounded panel
surfaces, remain centered on the panel surface, and leave the bottom-right
corner clear when both axes overflow. Panel scrollbar thumbs can be dragged,
and clicking the track updates the scroll offset. `Panel::scrollbar-track` and
`Panel::scrollbar-thumb` styling parts have landed for track/thumb width, track
axis inset, background, border, radius, and opacity. PageUp/PageDown/Home/End
keyboard scrolling has landed for the nearest scrollable ancestor of the
focused widget, with Shift using the horizontal axis when available.

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

- Visible scrollbar indicators and scrollbar styling are currently limited to
  Panel track/thumb CSS parts.
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

Initial animatable properties should match transition support.

### Runtime Requirements

- Global animation clock.
- Per-widget active animation state.
- Redraw requests while animations are active.
- Pause/cleanup when widget leaves tree.
- Respect reduced-motion setting if DragonGUI adds one.

### Acceptance Criteria

- Spinner/progress pulse demo can animate without Python timers.
- Animation stops when widget is removed.
- Infinite animations do not leak state.

## CSS3-15: Backdrop Filter

### Scope

Add a limited `backdrop-filter` subset:

- `blur(px)`
- Optional brightness/saturation later.

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

- Modal scrim can blur content behind it.
- Blur respects rounded rect clipping.
- GPU cost is documented.

## CSS3-16: Generated Content

### Scope

Add limited generated content:

- `::before`
- `::after`
- `content: "..."`

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

- `content: attr(...)`.
- Counter functions.
- Generated content with independent layout.

## CSS3-17: Responsive At-Rules And Fonts

Status: First `@media` implementation slice landed. Stylesheets now lower
viewport width/height/orientation media rules into rule metadata, cascade
matching filters them against the current logical window size, and the runtime
reapplies CSS on layout/resize. First static `@supports` implementation slice
landed for DragonGUI declaration and selector feature queries. Container
queries, `@font-face`, and other media features remain pending.

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
- Remaining: media features beyond `width`/`height`/`orientation`,
  container queries, and media-scoped custom property cascade semantics.

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
- Remaining: browser capability parity and feature functions beyond
  `selector(...)`.

### Container Queries

Defer until layout can expose container dimensions back into style matching
without creating feedback loops.

### `@font-face`

Planning decision required before implementation:

- Decide whether DragonGUI ships bundled fonts, only supports user-provided
  font files, or supports both.
- Estimate wheel/sdist size impact before adding bundled assets.
- Define how font files are referenced from CSS in packaged applications.
- Update maturin/pyproject packaging rules before the renderer work starts.

Needs:

- Font asset loading.
- Packaging story for wheels/sdists.
- Integration with glyphon/font system.
- Error handling when font files are missing.

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
- `var(--name, fallback)`, including embedded `var()` inside larger parseable
  property values.

Why first:

- Low risk.
- High demo impact.
- Mostly isolated to `css_style.rs`, `style.rs`, `theme.rs`, and
  `text/mod.rs`.

### Milestone B: Elevation And Rich Paint

Status: In progress. `box-shadow` landed for comma-separated outset and inset
shadows on rect-backed surfaces. First `linear-gradient()` and
`radial-gradient()` slices landed for rect-backed backgrounds. Gradient
rendering now interpolates up to four stop colors, sampling longer gradients
down to four GPU stops. First-slice repeating gradients have landed for
explicit final stop ranges. Comma-separated background layers now paint
back-to-front, enabling radial glow overlays on linear bases. `background-noise`
has landed as a low-amplitude procedural dither/noise pass for softening
gradient banding on rect-backed backgrounds.

Implement:

- `box-shadow`
- `linear-gradient()`
- `radial-gradient()`

Why second:

- Biggest visual improvement after typography.
- Requires renderer work but not layout model changes.

### Milestone C: State-Aware Styling

Status: In progress. Public state selectors landed for `:open`, `:expanded`,
`:collapsed`, and `:selected` with whole-widget and part styling. Selector
chains also landed for descendants, deeper direct-child chains, `[key="..."]`,
`:first-child`, `:last-child`, `:nth-child(...)` integer/odd/even/an+b
formulas, and filtered `:nth-child(... of <selector-list>)` for compound
selector lists and supported selector chains. Selector functions have landed
for compound selector arguments, including target pseudo-state arguments.
Universal selectors have landed for
`*`, `*.class`, `*:hover`, and chain targets such as `Panel > *`. Attribute
selectors have landed for widget metadata and parsed scalar props, including
presence, string operators, and case flags.

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
  formulas such as `3n+1` and `-n+3`. Filtered
  `:nth-child(... of <selector-list>)` is implemented for compound selector
  lists and supported descendant or child selector chains.
- Selector functions such as `:not()`, `:is()`, and `:where()`. Implemented
  for compound selector arguments, including target pseudo-state arguments.
- Universal selectors. Implemented for whole compounds and selector chains.

Why third:

- Makes CSS easier to write and lets users style interactive widgets without
  Python-side classes.

### Milestone D: Motion

Status: In progress. First CSS transition slice landed for parser/style-model
support, debug snapshots, whole-widget hover paint transitions, and
whole-widget `:open` / `:selected` paint transitions for tracked widgets. First
paint-only transform slice also landed for `translate`, `scale`, and `rotate`
on rect-backed widget surfaces, with CSS transform longhands now lowering into
the same transform model. Runtime support currently interpolates solid color,
numeric visual fields, and transform fields. Stylesheet changes, inline style
patches, full layout rebuilds, and widget replacement paths now cancel active
style transition progress. `transition-property` is honored for hover, `:open`,
`:selected`, and expanded/collapsed whole-widget transitions, with unlisted
visual fields snapping to the current state. Timing functions include the
standard easing keywords and custom `cubic-bezier(...)` curves. Layout, text
scale/rotate, child subtree transforms, image texture transforms, and layout
animation for expanding/collapsing content remain pending.

Implement:

- Paint-only CSS transitions. First whole-widget hover, `:open`, and
  `:selected` slices implemented. Expanded/collapsed whole-widget paint
  transitions also implemented.
- Timing functions. Implemented for `linear`, `ease`, `ease-in`, `ease-out`,
  `ease-in-out`, and `cubic-bezier(...)`.
- Paint-only transforms. First rect-surface slice implemented for
  `transform` plus `translate`, `scale`, and `rotate` longhands; text follows
  translate only.

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
uniform margin, `gap`, `row-gap`, and `column-gap`; uniform margin also
supports `auto`. `var()` terms inside `calc()` are supported for lengths and
scalar multipliers/divisors. Mixed-unit calc in fully auto-sized parent axes
remains pending.

Implement:

- Percent sizing. First sizing-property slice implemented.
- `auto` sizing. First sizing-property slice implemented.
- `calc()`. First sizing slice implemented, including mixed pixel/percent
  expressions when the parent axis is definite.
- Typed spacing. First padding, uniform margin, and gap slice implemented.
- CSS Grid. First Taffy-backed track and placement slice implemented. `minmax()`
  track parsing/lowering implemented for px/percent/auto minimums and
  px/percent/fr/auto maximums. `fit-content()` track parsing/lowering
  implemented for px and percent arguments. Auto-repeat parsing/lowering
  implemented for `repeat(auto-fit, ...)` and `repeat(auto-fill, ...)`.

Why fifth:

- More layout risk.
- Needs careful Taffy integration and more test coverage.

### Milestone F: Overflow And Scroll Containers

Implement:

- General `overflow`. First visible/hidden/auto/scroll slice implemented.
- Scroll state. Vertical and horizontal scroll offsets now work for opted-in
  containers; `Panel` scroll containers now render vertical and horizontal
  overlay indicators with mouse click/drag operation. PageUp/PageDown/Home/End
  keyboard scrolling now targets the focused scroll context, with mouse-position
  fallback.
- Clip stack. First explicit visible/hidden child clip behavior implemented.

Why separate:

- Touches layout, rendering, event routing, and hit testing.

### Milestone G: Advanced Browser Features

Evaluate and implement selectively:

- Positioning and z-index. First paint-only relative positioning, local sibling
  `z-index`, layout-backed `position: absolute`, and viewport-backed
  `position: fixed` slices implemented; full stacking contexts remain pending.
- `@keyframes`.
- Backdrop filter.
- Generated content.
- `@media`. First viewport `width`/`height`/`orientation` slice implemented;
  broader media features remain pending.
- `@supports`. First static declaration and selector feature-query slice
  implemented.
- `@font-face`.

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
