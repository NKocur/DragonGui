# CSS Theming System Improvement Plan

**Project:** DragonGui  
**Created:** July 27, 2026  
**Status:** In progress — Phase 1 complete; Phase 2 next  
**Source audits:** Windows 3.11 and classic 1990s Mac styling experiments  
**Primary demo:** `examples/nexus_studio_stress_demo.py`

## Progress Log

### July 27, 2026 — Phase 1 Complete

- Traced the color path from LightningCSS serialization through
  `DgCssColor`, `ColorRef`, semantic token resolution, and renderer-facing
  normalized RGBA.
- Confirmed the failure with regression tests: LightningCSS canonicalized
  `#c0c0c0` and `#000080` to `silver` and `navy`, which DragonGui previously
  reclassified as unknown semantic tokens.
- Replaced the seven-name hand-written parser with the standards-backed
  `cssparser` named-color inventory used by the CSS parser dependency.
- Kept DragonGui semantic colors explicit through the canonical
  `THEME_COLOR_TOKENS` inventory.
- Preserved extensible semantic color identifiers while deduplicating unresolved
  token diagnostics to one actionable warning per token per process.
- Added regressions for:
  - Every standard CSS named color.
  - Case-insensitive names.
  - Hex/name/`rgb(...)` equivalence for silver and navy.
  - Semantic-token preservation.
  - Unknown identifier diagnostics.
  - Custom properties and missing-variable fallbacks.
  - Gradients, shadows, borders, outlines, text, and widget-part colors.
- Restored authentic `#c0c0c0` and `#000080` values in the Windows 3.11 demo
  and removed its color-canonicalization workaround.
- Updated the public CSS, theme, library overview, widget, and Sphinx styling
  documentation to advertise the complete standard named-color inventory.
- Validation completed:
  - Focused Phase 1 native tests: passed.
  - Full native suite: **761 passed, 12 ignored, 0 failed**.
  - Python suite: **513 passed**.
  - Windows 3.11 WGPU smoke: **passed** for three frames with exact colors;
    computed `#c0c0c0` output was `[0.7529412, 0.7529412, 0.7529412, 1.0]`
    and no unknown `silver` or `navy` token warning was emitted.

## Purpose

The Windows 3.11 and classic Mac OS styling experiments demonstrated that
DragonGui's CSS system can substantially alter the appearance of a complex
application. They also exposed limitations and inconsistencies between:

- CSS parsing and lowering
- Native color resolution
- Native layout geometry
- Python widget-part validation
- Native widget-part validation
- Renderer part support
- Public CSS documentation
- Runtime stylesheet and theme management

This plan addresses those findings at the library level. The goal is not to add
vintage themes to DragonGui itself. The goal is to make DragonGui expressive,
predictable, and internally consistent enough that radically different themes
can be implemented without private selectors, parser workarounds, or
layout-specific patches.

---

## Goals

1. Make valid supported CSS resolve consistently from parsing through rendering.
2. Establish one source of truth for public widget types, states, and parts.
3. Give composite widgets stable semantic styling parts.
4. Expand border, outline, gradient, and typography capabilities.
5. Make themes replaceable at runtime without accumulating stale stylesheets.
6. Preserve correct border-box layout behavior across every container type.
7. Keep CSS diagnostics precise and actionable.
8. Ensure documentation is generated from or verified against implementation
   capabilities.

## Non-Goals

- Shipping Windows 3.11 or classic Mac OS as built-in production themes.
- Implementing arbitrary browser CSS.
- Supporting remote network resources in CSS.
- Replacing the native renderer with a web engine.
- Making every operating-system window decoration themeable in the first phase.

---

## Current Findings

### Resolved During the Theme Experiments

- Native layout now reserves painted border width in the Taffy box model.
- Bordered SearchBox children are positioned inside the content box.
- SearchBox icon and clear-button containment has native regression coverage.

These fixes must remain protected by broader tests as the CSS system evolves.

### Remaining Problems

| Priority | Problem | Current consequence |
| --- | --- | --- |
| P0 | Color normalization can turn valid colors into unknown semantic tokens | Valid hex colors can render using the danger fallback |
| P0 | Widget-part inventories disagree | Documentation can promise selectors that the renderer rejects |
| P0 | ScrollArea scrollbar parts are documented but rejected natively | ScrollArea scrollbar theming produces warnings |
| P1 | SearchBox lacks semantic parts | Themes depend on internal TextInput/IconButton composition |
| P1 | Panel lacks header/title/body parts | Titled surfaces cannot be styled structurally |
| P1 | Borders and outlines are uniform and solid only | Bevels, bottom-only borders, and dotted focus rings require hacks |
| P1 | Gradient stops support percentages but not logical pixels | Fixed-size pinstripes and patterns scale incorrectly |
| P1 | `font-family` stores only one family | Cross-platform font fallback lists do not work normally |
| P2 | Stylesheets cannot be replaced by name | Runtime theme switching accumulates cascade layers |
| P2 | Native theme tokens cannot be atomically replaced at runtime | Complete light/dark/retro switching requires restart or partial overrides |
| P2 | Window chrome and icon shapes are not themeable | Full visual transformations stop at the application client area |
| P3 | Local background assets and dedicated patterns are limited | Textures and dithering require gradient approximations |

---

# Phase 1 — Color Pipeline Correctness

**Priority:** P0  
**Status:** Complete

## Problem

The CSS pipeline can normalize exact hex colors into CSS color names. For
example:

- `#c0c0c0` becomes `silver`
- `#000080` becomes `navy`

DragonGui's final native color parser recognizes only:

- `transparent`
- `black`
- `white`
- `red`
- `green`
- `blue`
- `gray` / `grey`

Other identifier-like values are treated as DragonGui semantic theme tokens.
Consequently, `silver` and `navy` become unknown tokens and can fall back to the
danger color.

## Required Work

- [x] Trace color representation through Lightning CSS parsing, DragonGui CSS
      lowering, `ColorRef`, theme-token resolution, and renderer upload.
- [x] Decide whether parsed CSS colors should always lower directly to RGBA.
- [x] Preserve semantic tokens such as `accent`, `surface`, and `border`.
- [x] Prevent recognized CSS named colors from being reclassified as semantic
      tokens.
- [x] Add the complete CSS named-color table if RGBA preservation cannot cover
      every parser path.
- [x] Ensure custom properties containing colors follow the same rules.
- [x] Ensure gradient stops, shadows, borders, outlines, text, and widget-part
      colors use the corrected path.
- [x] Remove the one-step RGB workarounds from the vintage demo after the fix.

## Tests

- [x] Hex colors that have canonical CSS names retain their intended RGBA.
- [x] `silver`, `navy`, `maroon`, `purple`, `teal`, `olive`, and other standard
      names resolve correctly.
- [x] Theme tokens still resolve through the active DragonGui theme.
- [x] Unknown identifiers produce one clear warning instead of silently using an
      unrelated color.
- [x] Color values work inside custom properties and nested fallback values.
- [x] Color values work in gradients, shadows, borders, outlines, text, and
      widget parts.

## Acceptance Criteria

- `#c0c0c0`, `silver`, and `rgb(192, 192, 192)` render identically.
- `#000080`, `navy`, and `rgb(0, 0, 128)` render identically.
- No valid supported CSS color becomes an unknown DragonGui token.
- Existing semantic theme-token tests continue to pass.

---

# Phase 2 — Unified Widget Capability Registry

**Priority:** P0  
**Status:** Not started

## Problem

Widget CSS capabilities are declared separately in multiple locations:

- Python `_SUPPORTED_PARTS_BY_KIND`
- Native `widget_kind_supports_part`
- Native paint fallback part catalogs
- Native renderer implementations
- `docs/widgets.md`
- `docs/widgets-reference.md`
- `docs/css-capabilities-reference.md`
- Manual/help content

These sources currently disagree. Examples include:

- Public documentation lists ScrollArea scrollbar parts.
- Native CSS validation rejects ScrollArea scrollbar parts.
- Native code recognizes RadioButton `indicator`, `dot`, and `label`.
- Python and documentation inventories omit those RadioButton parts.

## Required Work

- [ ] Design one canonical capability schema containing:
  - Public widget type
  - Native widget kind
  - Python widget kind
  - Inherited public CSS type chain
  - Supported pseudo states
  - Supported CSS parts
  - Per-part supported property categories
  - Renderer support status
- [ ] Choose the canonical source location and serialization format.
- [ ] Generate or validate Python part metadata from the canonical schema.
- [ ] Generate or validate native part metadata from the canonical schema.
- [ ] Generate public documentation tables from the schema.
- [ ] Generate `dg.help` CSS type/state/part reference data from the schema.
- [ ] Add CI drift detection.
- [ ] Remove duplicate hand-maintained registries where practical.

## ScrollArea Work

- [ ] Confirm ScrollArea owns native scrollbar geometry.
- [ ] Add `scrollbar-track` and `scrollbar-thumb` to native support if the
      renderer already exposes that geometry.
- [ ] Add matching Python part validation.
- [ ] Add framework fallback and provenance entries.
- [ ] Add public CSS probe coverage.

## RadioButton Work

- [ ] Publish `indicator`, `dot`, and `label` as stable RadioButton parts.
- [ ] Add them to Python inline-part validation.
- [ ] Add them to all reference documentation.
- [ ] Add checked, hover, focus, and disabled part tests.

## Tests

- [ ] Every documented part is accepted by Python and native CSS validation.
- [ ] Every accepted part appears in documentation and `dg.help`.
- [ ] Every accepted paint part is consumed by a renderer.
- [ ] Capability snapshots are identical across generated Python, native, and
      documentation outputs.
- [ ] Unknown parts still produce precise widget-specific warnings.

## Acceptance Criteria

- There is one authoritative widget capability inventory.
- ScrollArea scrollbar styling works without warnings.
- RadioButton parts work consistently through Python, CSS, renderer, docs, and
  help.
- CI fails whenever the public inventories drift.

---

# Phase 3 — Composite Widget Semantic Parts

**Priority:** P1  
**Status:** Not started

## 3.1 SearchBox Parts

### Problem

SearchBox is a public composite but themes currently style its internal
implementation:

```css
SearchBox TextInput { ... }
SearchBox IconButton { ... }
IconButton.search-box-clear { ... }
```

This creates coupling to child widget types and private class structure.

### Required Parts

```css
SearchBox::icon
SearchBox::field
SearchBox::clear
```

### Required Work

- [ ] Define stable SearchBox part names and semantics.
- [ ] Decide whether parts map to child widgets, virtual render parts, or
      controlled style forwarding.
- [ ] Ensure part styles can affect appropriate layout, visual, and text
      properties.
- [ ] Preserve descendant-selector compatibility.
- [ ] Ensure part styling does not duplicate focus rings.
- [ ] Ensure clearable and non-clearable variants expose truthful parts.
- [ ] Ensure disabled SearchBox state propagates to all parts.
- [ ] Update the vintage themes to use semantic parts.

### Tests

- [ ] Border, background, padding, icon color, and radius apply to each part.
- [ ] SearchBox remains bounded with 0–4 px outer borders.
- [ ] SearchBox remains bounded at compact widths and high DPI scales.
- [ ] SearchBox works with and without a clear button.
- [ ] SearchBox focus-within styling remains stable.

## 3.2 Panel Structural Parts

### Problem

Panel exposes `accent` and scrollbar parts but not its title/body structure.
Themes cannot directly create title bands, framed captions, or distinct body
surfaces.

### Proposed Parts

```css
Panel::header
Panel::title
Panel::body
```

### Required Work

- [ ] Define geometry and ownership for titled and untitled panels.
- [ ] Expose the title text style through `Panel::title`.
- [ ] Expose title-band paint and layout through `Panel::header`.
- [ ] Expose content surface, padding, and clipping through `Panel::body`.
- [ ] Preserve existing `Panel::accent`.
- [ ] Make part behavior truthful when a panel has no title.
- [ ] Extend the same model to Modal and Sidebar where appropriate.

### Tests

- [ ] Header backgrounds and borders do not overlap body content.
- [ ] Title text remains vertically aligned at multiple font sizes.
- [ ] Body padding participates in scroll geometry.
- [ ] Rounded clipping works across separately styled header/body surfaces.
- [ ] Existing panel title and scrollbar tests remain valid.

## Acceptance Criteria

- Theme authors do not need internal SearchBox child selectors.
- Titled panels can implement visually distinct header and body regions.
- Composite part contracts remain stable if internal widget composition changes.

---

# Phase 4 — Border and Outline Expansion

**Priority:** P1  
**Status:** Not started

## Problem

Current borders and outlines support uniform widths and the `solid`, `none`, and
`hidden` styles only. DragonGui does not support:

- Side-specific border shorthands
- Side-specific border widths
- Side-specific border colors
- Side-specific border styles
- `dotted`
- `dashed`
- `double`

This affects common application styling, not only vintage themes:

- Bottom-only menu and toolbar dividers
- Selected-tab edges
- Table header separators
- Group-box captions
- Keyboard focus rectangles
- Raised and recessed control bevels

## Required Work

- [ ] Extend `VisualStyle` to store four border widths and four border colors.
- [ ] Decide whether border style is uniform initially or per side.
- [ ] Parse:
  - `border-top`
  - `border-right`
  - `border-bottom`
  - `border-left`
  - `border-*-width`
  - `border-*-color`
  - `border-*-style`
- [ ] Preserve existing `border` shorthand cascade behavior.
- [ ] Add `dotted`, `dashed`, and `double` border rendering.
- [ ] Add dotted and dashed outline rendering.
- [ ] Define how radii intersect with dashed, dotted, and double borders.
- [ ] Make all four border widths participate in Taffy layout.
- [ ] Extend computed-style provenance for shorthand/longhand collisions.
- [ ] Update transition support where practical.

## Tests

- [ ] Each side independently affects layout and paint.
- [ ] Shorthand and longhand cascade precedence matches documented behavior.
- [ ] Border widths scale correctly at non-integer DPI.
- [ ] Child content remains inside asymmetric borders.
- [ ] Rounded dashed/dotted/double borders remain clipped correctly.
- [ ] Focus outlines do not change layout.
- [ ] Zero and `none` correctly reset prior border declarations.

## Acceptance Criteria

- A menu bar can have only a bottom border.
- A selected tab can suppress or alter one edge.
- A control can render a genuine dotted focus rectangle.
- Raised/recessed controls no longer require multiple inset-shadow workarounds.

---

# Phase 5 — Gradient Stop Units and Pattern Fidelity

**Priority:** P1  
**Status:** Not started

## Problem

Gradient stops currently accept percentage positions but reject logical-pixel
positions. Fixed-width repeating patterns therefore scale with widget size.

The following should be valid:

```css
repeating-linear-gradient(
    0deg,
    #eeeeee 0px,
    #eeeeee 1px,
    #c4c4c4 1px,
    #c4c4c4 2px
)
```

## Required Work

- [ ] Replace normalized-only gradient positions with a length-percentage
      representation.
- [ ] Parse:
  - percentages
  - logical pixels
  - mixed `calc()` positions where feasible
- [ ] Resolve stop positions against the painted gradient line at render time.
- [ ] Preserve repeating-gradient period correctly for pixel-based stops.
- [ ] Support hard stops with two positions or repeated adjacent stops.
- [ ] Prevent CSS normalization from emitting syntax the lowerer cannot parse.
- [ ] Define behavior for decreasing, omitted, and out-of-range stop positions.
- [ ] Update gradient serialization and debug snapshots.

## Tests

- [ ] One-pixel and two-pixel repeating stripes remain fixed across widget sizes.
- [ ] Pixel patterns scale correctly with DPI.
- [ ] Percentage gradients retain current behavior.
- [ ] Mixed-unit gradients resolve deterministically.
- [ ] Hard-stop shorthand and expanded stop syntax render identically.
- [ ] Existing linear, radial, layered, and interpolation tests continue to pass.

## Acceptance Criteria

- Pinstripe and checker patterns can use stable logical-pixel dimensions.
- Gradient parsing produces no warning for supported CSS stop syntax.

---

# Phase 6 — Font Family Fallback Lists

**Priority:** P1  
**Status:** Not started

## Problem

DragonGui stores one `FontFamily`. CSS declarations such as:

```css
font-family: "Chicago", "Geneva", "Arial", sans-serif;
```

do not behave as ordered fallback lists. Cross-platform themes must select one
likely-installed family or accept the renderer's generic fallback.

## Required Work

- [ ] Change text style representation from one family to an ordered list.
- [ ] Parse quoted names and generic family keywords from CSS lists.
- [ ] Preserve family order through inline styles, stylesheets, inheritance,
      transitions, serialization, and snapshots.
- [ ] Resolve the first available family at text-shaping time.
- [ ] Integrate `@font-face` aliases into ordered fallback resolution.
- [ ] Cache family availability and resolved font selections.
- [ ] Define stable generic fallbacks for each platform.
- [ ] Report missing requested families only in diagnostics, without noisy
      per-frame warnings.

## Tests

- [ ] The first installed family is selected.
- [ ] Missing families fall through in declaration order.
- [ ] Generic families always resolve.
- [ ] `@font-face` aliases work as list entries.
- [ ] Font measurement and painting select the same resolved face.
- [ ] Runtime stylesheet replacement invalidates the correct font caches.

## Acceptance Criteria

- Cross-platform CSS font stacks behave predictably.
- Layout measurement and rendered text remain consistent after fallback.

---

# Phase 7 — Runtime Theme and Stylesheet Management

**Priority:** P2  
**Status:** Not started

## Problem

`app.stylesheet(css)` appends another user stylesheet. `clear_stylesheets()`
removes every user stylesheet. There is no named replacement API, and changing
`app.theme` does not atomically update the active native theme.

Repeated theme switching can grow the cascade and leave unrelated user styles
mixed with appearance styles.

## Proposed API

```python
app.set_theme(theme)
app.set_stylesheet("appearance", css)
app.set_stylesheet("application", app_css)
app.remove_stylesheet("appearance")
```

Exact naming can change during API review.

## Required Work

- [ ] Add stable stylesheet identifiers.
- [ ] Support add, replace, remove, and clear operations by stylesheet origin
      and identifier.
- [ ] Preserve deterministic source order when replacing a stylesheet.
- [ ] Add a native command for atomic theme replacement.
- [ ] Recompute theme variables, framework styles, inherited styles, layout,
      text, and paint only as necessary.
- [ ] Invalidate relevant font, layout, primitive, and resource caches.
- [ ] Preserve application state, focus, selection, and scroll offsets.
- [ ] Make theme changes safe through `call_soon_threadsafe`.
- [ ] Provide a minimal theme-switching example.

## Tests

- [ ] Replacing one named stylesheet does not affect others.
- [ ] Repeated switching does not increase active stylesheet count.
- [ ] Theme tokens update across widgets and parts.
- [ ] Layout-affecting token changes trigger layout.
- [ ] Paint-only token changes avoid unnecessary structural rebuilds.
- [ ] Focus, text selection, active page, and scroll offsets survive switching.
- [ ] Debug snapshots report active stylesheet names and source order.

## Acceptance Criteria

- Nexus, Windows 3.11, and Mac OS styles can be switched live.
- Theme switching does not require reconstructing the application.
- The cascade remains bounded and inspectable.

---

# Phase 8 — Styleable Window Chrome

**Priority:** P2  
**Status:** Not started  
**Scope:** Large; platform-sensitive

## Problem

DragonGui styles only the application client area. The operating-system title
bar, frame, minimize/maximize/close controls, resizing behavior, and system menu
remain native and cannot match a custom theme.

## Proposed Direction

Provide optional client-side decorations while preserving native decorations as
the default.

Potential public structures:

```css
Window::titlebar
Window::title
Window::minimize
Window::maximize
Window::close
Window::resize-border
```

## Required Work

- [ ] Research Winit support and platform restrictions.
- [ ] Define opt-in client-side decoration behavior.
- [ ] Preserve dragging, resizing, snapping, system menus, DPI, accessibility,
      and keyboard commands.
- [ ] Expose stable titlebar CSS parts.
- [ ] Provide safe platform fallbacks where full custom chrome is unavailable.
- [ ] Test Windows, macOS, and Linux separately.

## Acceptance Criteria

- Native decorations remain the reliable default.
- Opt-in custom chrome is functional, accessible, and correctly scaled.
- Applications can achieve a visually coherent full-window theme.

---

# Phase 9 — Icon Theme Infrastructure

**Priority:** P2  
**Status:** Not started

## Problem

CSS can recolor icon parts but cannot replace icon geometry or alter an icon
family's stroke language. Strong visual themes therefore retain DragonGui's
modern icon shapes.

## Proposed API Direction

```python
app.set_icon_theme({
    "search": custom_search_icon,
    "close": custom_close_icon,
    "menu": custom_menu_icon,
})
```

## Required Work

- [ ] Define a resource-backed icon representation.
- [ ] Support built-in aliases and application overrides.
- [ ] Keep icon names semantic rather than widget-specific.
- [ ] Support monochrome tintable icons first.
- [ ] Consider SVG/path or compact vector command input.
- [ ] Define sizing, alignment, stroke, and disabled-state behavior.
- [ ] Cache tessellated/uploaded icon data.
- [ ] Expose resolved icon identity in debug snapshots.

## Acceptance Criteria

- A theme can replace common icon shapes without replacing widget classes.
- Icon layout remains stable across themes.
- Missing overrides fall back to built-in icons.

---

# Phase 10 — Local Background Resources and Patterns

**Priority:** P3  
**Status:** Not started

## Problem

`background-image: url(...)` is unsupported. This is a sound default for remote
resources, but themes cannot use packaged local textures or tiles. Repeating
gradients can approximate some patterns but are not ideal for dithering,
bitmaps, or branded surfaces.

## Required Work

- [ ] Define a safe application-resource URL or identifier syntax.
- [ ] Continue rejecting remote HTTP/HTTPS resources by default.
- [ ] Resolve resources through DragonGui's managed resource registry.
- [ ] Support contain, cover, stretch, and repeat modes as a focused subset.
- [ ] Consider a built-in pattern paint for:
  - checker
  - pinstripe
  - dot/stipple
  - diagonal hatch
- [ ] Ensure clipping, radii, opacity, and transforms work.
- [ ] Add resource lifetime and cache tests.

## Acceptance Criteria

- Applications can use packaged local surface assets safely.
- Common patterns do not require large textures or unstable gradient tricks.

---

# Phase 11 — Theme Token Expansion

**Priority:** P3  
**Status:** Not started

## Problem

`Theme` currently covers core colors, radius, spacing, and font size. A complete
design system also needs typography and control geometry. Without those tokens,
large themes require extensive CSS repetition.

## Candidate Tokens

- Font family stack
- Monospace font family stack
- Base line height
- Control height
- Compact control height
- Default border width
- Focus width and offset
- Default panel padding
- Default toolbar gap
- Default shadow presets
- Animation duration and easing preferences

## Required Work

- [ ] Decide which values belong in Theme versus framework CSS.
- [ ] Expose new values as `--dg-*` custom properties.
- [ ] Preserve CSS override precedence.
- [ ] Add serialization and runtime replacement support.
- [ ] Avoid making Theme a second independent styling language.

## Acceptance Criteria

- Theme provides high-value system tokens.
- Detailed widget appearance remains CSS-owned.
- Theme and CSS values have clear, documented precedence.

---

## Cross-Cutting Testing Strategy

### Unit Tests

- CSS parsing and lowering
- Color conversion
- Shorthand/longhand cascade
- Capability registry generation
- Font-family parsing and resolution
- Gradient stop resolution
- Runtime stylesheet operations

### Layout Tests

- Uniform and asymmetric borders
- Bordered composite widgets
- High-DPI fractional scaling
- Compact and minimum-size layouts
- Scrollbar and titled-container geometry
- Live theme changes that alter spacing or border widths

### Primitive Tests

- Dashed, dotted, and double borders
- Dotted focus outlines
- Pixel-based repeating gradients
- Composite parts
- Pattern and resource backgrounds

### Integration Probes

- Core bordered composites
- SearchBox semantic parts
- Panel header/title/body
- ScrollArea scrollbar parts
- RadioButton parts
- Gradient unit matrix
- Font fallback stack
- Runtime theme switching
- Optional client-side window chrome

### Visual Regression Set

Maintain representative screenshots for:

- Default dark theme
- Default light theme
- High-radius modern theme
- Square high-contrast theme
- Windows 3.11 experiment
- Classic Mac OS experiment
- Compact and high-DPI variants

Visual comparisons should tolerate expected text rasterization differences while
remaining strict about geometry, clipping, overflow, border placement, and
major color changes.

---

## Documentation Work

- [ ] Generate CSS part tables from the canonical capability registry.
- [ ] Document border-box behavior explicitly.
- [ ] Document supported border and outline styles.
- [ ] Document gradient units and repeating-period behavior.
- [ ] Document font fallback resolution.
- [ ] Document named stylesheet lifecycle and precedence.
- [ ] Document runtime theme replacement.
- [ ] Add composite-part examples.
- [ ] Add troubleshooting entries for unmatched parts, unknown tokens, and
      unavailable fonts.
- [ ] Keep `dg.help` synchronized with generated references.

---

## Recommended Implementation Order

1. Phase 1 — Color pipeline correctness
2. Phase 2 — Unified widget capability registry
3. Phase 3 — Composite widget semantic parts
4. Phase 4 — Border and outline expansion
5. Phase 5 — Gradient stop units
6. Phase 6 — Font fallback lists
7. Phase 7 — Runtime theme and stylesheet management
8. Phase 11 — Theme token expansion
9. Phase 9 — Icon themes
10. Phase 10 — Local resources and patterns
11. Phase 8 — Optional styleable window chrome

Phases 1–2 are correctness work and should be completed first. Phases 3–7
provide the largest improvement to everyday theme authoring. Phases 8–11 are
larger design-system capabilities that can proceed after the core CSS contracts
are stable.

---

## Progress Checklist

### Correctness Foundation

- [ ] Phase 1 complete: color pipeline
- [ ] Phase 2 complete: capability registry
- [ ] Border-box behavior has broad regression coverage

### Core Theme Expressiveness

- [ ] Phase 3 complete: composite parts
- [ ] Phase 4 complete: borders and outlines
- [ ] Phase 5 complete: gradient units
- [ ] Phase 6 complete: font fallback lists

### Runtime Theme System

- [ ] Phase 7 complete: named stylesheet and theme replacement
- [ ] Phase 11 complete: expanded theme tokens

### Advanced Appearance

- [ ] Phase 9 complete: icon themes
- [ ] Phase 10 complete: local resources and patterns
- [ ] Phase 8 complete: optional window chrome

### Release Validation

- [ ] Full Python suite passes
- [ ] Full native suite passes
- [ ] CSS probes pass without warnings
- [ ] Strict layout and usability audits pass
- [ ] Visual regression suite passes
- [ ] Windows, macOS, and Linux smoke tests pass
- [ ] `dg.help` and documentation inventories match implementation

---

## Definition of Done

This plan is complete when:

1. Valid supported CSS colors never become unknown theme tokens.
2. Widget types, states, and parts come from one authoritative registry.
3. SearchBox and titled surfaces expose stable semantic parts.
4. Borders support practical per-side styling and focus-ring patterns.
5. Repeating gradients can use fixed logical-pixel stops.
6. Font stacks resolve in ordered cross-platform fallback order.
7. Themes and named stylesheets can be replaced live without cascade growth.
8. Documentation and `dg.help` are generated from or validated against the
   implementation.
9. Default, modern, square, Windows-style, and Mac-style visual probes pass
   without CSS warnings, clipping, overflow, or layout regressions.
