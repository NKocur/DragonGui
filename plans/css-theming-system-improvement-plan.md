# CSS Theming System Improvement Plan

**Project:** DragonGui  
**Created:** July 27, 2026  
**Status:** In progress — Phases 1–7 and 9–11 complete; Phase 8 platform validation pending
**Source audits:** Windows 3.11 and classic 1990s Mac styling experiments  
**Primary demo:** `examples/nexus_studio_stress_demo.py`

## Progress Log

### July 30, 2026 — Phase 11 Complete: System Theme Tokens

- Audited the Python/native `Theme` schema, framework stylesheet generation,
  custom-property lowering, live theme invalidation, and cascade order.
- Defined the ownership boundary:
  - Theme owns reusable typography and control-system defaults.
  - Framework CSS consumes those defaults through `--dg-*` properties.
  - Theme/application CSS and inline style continue to own detailed widget
    appearance and retain their higher cascade precedence.
  - Shadow presets, per-widget padding recipes, and animation/easing remain CSS
    concerns; they are not duplicated in Theme.
- Added serialized/live-replaceable Theme fields for font stacks, base line
  height, standard and compact control heights, framework border width, focus
  width/offset, panel padding, and toolbar gap.
- Exposed the fields as `--dg-font-family`,
  `--dg-monospace-font-family`, `--dg-line-height`,
  `--dg-control-height`, `--dg-compact-control-height`,
  `--dg-border-width`, `--dg-focus-width`, `--dg-focus-offset`,
  `--dg-panel-padding`, and `--dg-toolbar-gap`.
- Wired the tokens into inherited text defaults, code/log fonts, compact/icon
  control geometry, framework borders and focus rings, native panel/container
  insets, and toolbar gaps.
- Removed `Toolbar`'s hard-coded default gap from serialized widget defaults
  and moved the legacy 6 px value into `Theme.toolbar_gap`, so live Theme
  replacement can actually control the system default while explicit
  `Toolbar(gap=...)`, application CSS, and inline style still win.
- Added safe font-stack lowering so theme-provided family names cannot break
  the generated framework stylesheet.
- Extended runtime dirty classification: typography, control, border, panel,
  and toolbar geometry changes request layout; focus-only geometry remains a
  visual update.
- Updated `dg.help()`, the CSS guide, library overview, widget reference, and
  Python Theme documentation.
- Preserved the exact legacy default line-box geometry. The initial direct
  `Window` multiplier introduced fractional text-centering drift; the final
  implementation uses the new base token to derive the existing 5 px leading
  at default values and leaves CSS `line-height` available for authored rules.
- Validation:
  - Native: **882 passed, 12 ignored, 0 failed**.
  - Python: **535 passed, 0 failed**.
  - `cargo check`, Rust formatting, release wheel build, copy, and import
    verification: pass.
  - Five representative visual targets captured: toolbar, typography,
    default-polish stress, and border/outline/shadow passed; core widgets
    retained its expected manual-interaction status. No target failed.
- Phase 11 is complete.

### July 30, 2026 — Drag ghost regression restored

- Investigated the missing cursor-following drag chip reported in the CATHODE-7
  stress demo. Payload dispatch, accept filtering, and the demo's `DragSource`
  / `DropZone` configuration were correct.
- Traced the regression to the targeted interaction rebuild optimization:
  `drag_pos` continued updating, but `set_drag_drop_state` requested targeted
  source/target work with primitive-overlay rebuilding disabled. The retained
  `emit_drag_drop_overlay` geometry therefore remained valid but was never
  refreshed while dragging.
- Restored the overlay through the narrow invalidation path:
  - pointer-only drag movement rebuilds primitive overlays only;
  - source, target, or kind changes additionally rebuild the affected widget
    subtrees;
  - overlay text and the complete visual tree are not rebuilt per mouse move.
- Focused native drag/drop validation: **3 passed, 0 failed**. Rust compilation
  and formatting checks pass.

### July 29, 2026 — Phase 7 Complete: Runtime Theme and Stylesheet Management

- Replaced the single native theme/user source slots with ordered source
  collections while retaining one aggregated parsed cascade per origin.
- Fixed an existing lifecycle mismatch: repeated anonymous startup or live
  stylesheets now append in source order instead of the native store silently
  retaining only the last same-origin sheet.
- Added stable stylesheet IDs with atomic add/replace semantics. Replacing an
  existing ID preserves its cascade position and does not disturb other sheets.
- Added removal by origin and ID, origin-wide clearing, and active stylesheet
  metadata containing origin, ID, and source order.
- Startup documents now accept optional stylesheet IDs; unnamed documents
  remain backward-compatible.
- Added Python `App.set_stylesheet(id, css)` and
  `App.remove_stylesheet(id)` APIs plus live native bridge commands.
- Added Python `App.set_theme(theme)` and a native atomic theme command.
  Spacing/font-size changes request layout work; paint-only token changes use a
  visual rebuild without reconstructing the widget tree.
- Theme replacement reinstalls framework variables, reparses dependent
  theme/user CSS, and rebuilds the affected layout/text/primitive paths.
- Added regressions proving widget and part tokens update, spacing variables
  trigger new layout values, and paint-only theme changes avoid layout
  invalidation.
- Added a state-preservation regression covering focus, text selection, active
  pages, and horizontal/vertical scroll offsets across restyle and layout.
- Runtime debug snapshots now report every active stylesheet's origin, stable
  ID, and source order under `stylesheets.active`.
- Named stylesheet and theme calls use the existing locked runtime command
  bridge and are safe to invoke from `call_soon_threadsafe` callbacks.
- Added `examples/runtime_theme_switching_demo.py`, which switches live among
  Nexus, Windows 3.11, and classic Mac-style themes without rebuilding widgets.
- Updated the CSS styling guide, capability reference, widget reference,
  Sphinx styling guide, and `dg.help()` source.
- Complete native validation: **837 passed, 12 ignored, 0 failed**.
- Complete Python validation: **524 passed, 0 failed**.
- Phase 7 is complete. Phase 8 styleable window chrome is next.

### July 29, 2026 — Phase 6 Complete: Font Family Fallback Lists

- Extended the text-style font representation with ordered fallback stacks
  while retaining the existing single-family variants for compatibility.
- CSS now preserves comma-separated quoted names and generic families instead
  of treating the entire declaration as one family name.
- Inline styles accept either a comma-separated string or a JSON array, with
  snake_case and kebab-case property names.
- Preserved stack order through stylesheet cascade, inheritance, inline style,
  cache keys, and computed-style/debug JSON serialization.
- Added shaping-time selection of the first available requested family.
  Standard generic families always terminate fallback successfully.
- Integrated `@font-face` aliases into stack availability checks.
- Added caches for named-family availability and resolved stack selections;
  alias changes invalidate both caches.
- Font synchronization now removes inactive aliases and remembers resolved
  source-to-family mappings, so removing and later restoring an `@font-face`
  rule invalidates and rebuilds stack selection without reloading font data.
- Standard and UI generic keywords resolve through the platform font database;
  `system-ui`, `ui-sans-serif`, and `ui-rounded` use its sans-serif family.
- Missing requested names are deduplicated in runtime font diagnostics without
  producing per-frame console warnings.
- Added focused CSS, inline-style, inheritance, serialization, installed-font,
  missing-font, generic-fallback, alias-resolution, measurement/paint identity,
  and stylesheet-replacement regressions. The focused font-family suite passes
  **7 tests with 0 failures**.
- Updated the CSS capability reference, styling guides, Sphinx documentation,
  and `dg.help()` manual source.
- Complete native validation: **831 passed, 12 ignored, 0 failed**. The updated
  Python manual compiles successfully under Python 3.12.
- Phase 6 is complete. Phase 7 runtime theme and stylesheet management is next.

### July 29, 2026 — Phase 5 Complete: Gradient Stop Units and Pattern Fidelity

- Replaced normalized-only gradient stop positions with a serializable
  length-percentage representation that preserves percentage and logical-pixel
  components through parsing, computed style, and debug snapshots.
- Added percentage, logical-pixel, and mixed `calc()` stop parsing. Unitless
  zero remains valid; unsupported nonzero unitless positions are rejected.
- Deferred stop resolution until paint geometry is known. Linear gradients use
  the painted gradient-line length, while radial gradients use the maximum
  center-to-corner radius.
- Applied display scale during logical-pixel resolution so fixed-width
  repeating patterns remain stable across widget sizes and scale correctly at
  non-integer DPI.
- Added two-position hard-stop expansion and verified that it matches explicit
  adjacent equal-color stops.
- Defined deterministic fixup behavior: omitted runs are distributed between
  surrounding positions, decreasing stops are promoted to the preceding
  resolved position, and out-of-range positions clamp to the painted interval.
- Preserved existing six-stop GPU sampling behavior after resolving authored
  positions.
- Updated the CSS capability reference, styling guides, `dg.help()` source, and
  the gradient feature probe with a fixed-pixel repeating stripe example.
- Added parser, hard-stop equivalence, size, DPI, mixed-unit, percentage, and
  debug-snapshot regressions.
- Complete native validation: **826 passed, 12 ignored, 0 failed**. The updated
  Python manual and gradient probe also compile successfully under Python 3.12.
- Phase 5 is complete. Phase 6 font-family fallback lists is next.

### July 29, 2026 — Phase 4.2 Complete: Patterned Borders and Outlines

- Added `dotted`, `dashed`, and `double` to uniform and per-edge border parsing,
  inline styles, computed style, shorthand application, and painting.
- Added patterned `outline` and `outline-style` rendering. Outlines remain
  paint-only; an explicit layout regression verifies that width and offset do
  not alter widget or child geometry.
- Added constant-cost GPU pattern rendering:
  - Uniform patterns use the rounded outline-ring SDF.
  - Per-edge patterns use one GPU strip for each visible edge rather than
    emitting a primitive for every dash or dot.
  - Double lines divide the authored width into two painted thirds separated by
    a one-third gap.
- Defined rounded behavior: uniform patterns follow and clip to the rounded
  ring while phase uses a stable rectangular-perimeter projection; mixed
  per-edge patterns clip each strip with that edge's outer corner radii.
- Existing `border-width` and `border-color` transitions now interpolate their
  per-edge values. Border and outline style keywords remain discrete.
- Added parser, shorthand, outline geometry, non-integer-DPI, GPU metadata,
  transition, and Naga WGSL parse/validation regressions.
- Updated the CSS capability reference, styling guide, widget reference,
  Sphinx summary, `dg.help()` manual source, and the border/outline visual probe.
- Complete native validation: **819 passed, 12 ignored, 0 failed**.
- Phase 4 is complete. Phase 5 gradient stop units and pattern fidelity is next.

### July 29, 2026 — Phase 4.1 Side-Specific Border Foundation

- Extended `VisualStyle` with per-edge width, color, and style overrides while
  preserving the existing uniform border fields as the fallback contract.
- Chose per-side border styles rather than a uniform-only intermediate model so
  `none` and future patterned styles participate correctly in CSS cascade order.
- Added parsing and application for `border-top`, `border-right`,
  `border-bottom`, `border-left`, and all side `-width`, `-color`, and `-style`
  longhands.
- Uniform `border`, `border-width`, `border-color`, and `border-style`
  declarations reset the corresponding side overrides; later side longhands
  override only their own edge.
- Added snake_case and kebab-case inline-style ingestion for the new fields.
- Taffy now reserves the effective width of each edge independently, including
  `none` edges, and the solid renderer emits independently colored and sized
  strips for authored asymmetric borders.
- Expanded provenance mapping so uniform and side shorthands report the
  longhands they affect.
- Added parser/cascade, asymmetric layout containment, and four-edge paint
  regressions. Focused border tests pass.
- Phase 4.1 validation: **811 passed, 12 ignored, 0 failed** in the complete
  native suite; capability documentation regeneration also passed.
- Remaining Phase 4 work is patterned `dotted`, `dashed`, and `double`
  rendering, patterned outlines, transition decisions, rounded-pattern policy,
  broader DPI/visual coverage, documentation, and final validation.

### July 29, 2026 — Phase 3.2 Complete

- Completed authored `::header` geometry:
  - `height` defines the minimum title-band height.
  - `padding` defines the title inset and the minimum space around the title.
  - Titles center vertically when the authored band is taller than the text.
  - Large fonts expand the band beyond its requested height instead of clipping
    or overlapping body content.
- Kept the legacy Panel title geometry unchanged when no header layout
  declarations are authored.
- Extended the same truthful `::header`, `::title`, and `::body` contract to
  titled Sidebar and Modal widgets. Modal retains its independent `::scrim`,
  close-button, shadow, and default title-band behavior beneath authored part
  overrides.
- Consolidated virtual header/body painting into one renderer helper shared by
  Panel, Sidebar, and Modal.
- Added multi-font geometry coverage at 14 px and 52 px, shared native
  Sidebar/Modal paint coverage, and Python validation/smoke coverage for all
  three titled container types.
- Final Phase 3 validation:
  - Native suite: **805 passed, 12 ignored, 0 failed**.
  - Generated capability documentation drift check: **passed**.
  - Python source compilation and shared titled-parts smoke: **passed**.
  - Scoped whitespace validation: **passed**.
- Phase 3 is now complete. Phase 4 border and outline expansion is next.

### July 29, 2026 — Phase 3.2 Panel Header/Body Surfaces and Body Geometry

- Added `Panel::header` and `Panel::body` to the canonical CSS capability
  registry as renderer-owned paint parts and regenerated every capability
  reference table.
- Connected both virtual parts to `TitledContainerGeometry`:
  - `::header` owns the resolved title band.
  - `::body` owns the resolved body viewport below the title gap.
- Header and body backgrounds support the normal DragonGUI background-paint
  path, including solid and gradient paints. Each surface also supports its own
  border color, width, radius, opacity, and state slots.
- Preserved parent corner ownership by default: the header inherits only the
  panel's inner top corners and the body inherits only its inner bottom
  corners. Explicit part radii remain authoritative.
- Made untitled behavior truthful: an untitled Panel exposes no header/body
  geometry and emits neither virtual surface.
- Made `Panel::body { padding: ... }` structural:
  - Children are inset on both axes.
  - Available body width is reduced by both horizontal padding edges.
  - `body_content_origin_y` includes the leading body padding.
  - Leading and trailing body padding participate in scroll extent.
- Preserved `Panel::accent`, title, scrollbar, clipping, and existing titled
  container behavior; the complete native suite passes.
- Added Python serialization coverage for all three structural parts and native
  regressions for:
  - Header/background and border ownership.
  - Body/background ownership.
  - Rounded top/bottom corner ownership.
  - Untitled no-op behavior.
  - Body padding child placement and scroll extent.
- Validation:
  - Native suite: **803 passed, 12 ignored, 0 failed**.
  - Generated capability documentation drift check: **passed**.
  - Python source compilation: **passed**.
- Remaining Phase 3.2 work: authored header sizing/padding semantics, explicit
  multi-font vertical-alignment coverage, and deciding how much of this contract
  should extend to Sidebar and Modal.

### July 28, 2026 — Phase 3.1 SearchBox Parts Complete

- Migrated both vintage theme layers in
  `examples/nexus_studio_stress_demo.py` to the public
  `SearchBox::icon`, `SearchBox::field`, and `SearchBox::clear` contract.
  The classic Mac overrides no longer select private TextInput or IconButton
  children.
- Expanded native forwarding coverage to verify background, border width,
  border color, radius, padding, icon color, and width across all three parts.
- Added focus coverage proving `SearchBox:focus::field` forwards one focus
  outline to the owned field without adding a duplicate outline to the
  composite container.
- Expanded the layout matrix across 0, 1, 2, and 4 logical-pixel outer borders,
  1.0x, 1.25x, and 2.0x scale factors, compact 180-pixel width, and both
  clearable variants. Every owned child remains inside the SearchBox border
  box.
- Added explicit reentrant coverage for the keyed Python task scheduler before
  returning from the performance work to the CSS plan.
- Focused validation:
  - Native semantic-part forwarding test: **passed**.
  - Native SearchBox containment matrix: **passed**.
  - Python source compilation for the library, API tests, and styled demo:
    **passed**.
  - The local Python 3.12 environment does not currently include pytest, so the
    new Python scheduler test is source-validated but awaits the next complete
    Python suite run.

### July 28, 2026 — Phase 3.2 Panel Parts Started

- Added `Panel::title` to the canonical CSS capability registry as a
  text-rendered part and regenerated every capability reference table.
- Wired the title part into native text shaping for Panel captions.
- Updated titled-panel layout reservation to derive font size and line height
  from `Panel::title`, with the existing Panel text style retained as fallback.
  Larger or custom-line-height captions therefore move the body origin instead
  of painting over the first child.
- Added Python serialization coverage and native layout coverage for
  title-part typography.
- Focused validation:
  - Generated capability documentation drift check: **passed**.
  - Native title-part layout test: **passed**.
  - Native capability registry tests: **4 passed**.
  - Python 3.12 title-part construction/serialization smoke: **passed**.
- Next: implement renderer-owned `Panel::header` and `Panel::body` paint
  surfaces against the existing `TitledContainerGeometry`, then connect body
  padding and clipping semantics.

### July 28, 2026 — Phase 3 SearchBox Semantic Parts In Progress

- Extended the canonical capability schema with semantic-only public widget
  records. This allows a composite such as SearchBox to publish parts without
  incorrectly granting those parts to every widget sharing its native
  `HLayout` kind.
- Added the public SearchBox parts:
  - `SearchBox::icon`
  - `SearchBox::field`
  - `SearchBox::clear`
- Updated Python inline-part validation to combine the concrete widget-kind
  capabilities with the public semantic-type capabilities.
- Updated native stylesheet and inline-part validation to use the complete
  public CSS type chain instead of only `WidgetKind`.
- Added controlled native forwarding from SearchBox semantic parts to its
  owned search icon, TextInput field, and conditional clear button.
- Forwarded base layout, visual, and text declarations as well as hover,
  active, focus, and disabled visual slots. Icon declarations also feed the
  owned IconButton's stable `icon` mark part.
- Preserved existing descendant-selector behavior; a focused regression proves
  a later `SearchBox::field` rule correctly overrides an earlier
  `SearchBox TextInput` declaration.
- SearchBox now serializes its `disabled` and `clearable` state on the composite
  node, while continuing to propagate disabled state to its interactive
  children.
- Added Python coverage proving SearchBox accepts the semantic parts while a
  plain HLayout rejects `::field`.
- Added native coverage proving:
  - Semantic capabilities are scoped to the public SearchBox type.
  - Inherited HLayout scrollbar capabilities remain available.
  - Icon color/width, field background/padding, clear hover paint, and disabled
    field opacity reach the owned child styles.
- Regenerated all capability reference tables and passed their drift check.
- Focused validation:
  - Python SearchBox/registry/documentation tests: **4 passed**.
  - Native semantic capability registry tests: **4 passed**.
  - Native SearchBox forwarding test: **passed**.
- Complete validation after the semantic forwarding change:
  - Python suite: **517 passed, 0 failed**.
  - Native suite: **768 passed, 12 ignored, 0 failed**.
  - Native formatting check: **passed**.
  - Release extension rebuild, local package refresh, and Python 3.12
    SearchBox semantic-part smoke: **passed**.
- Remaining SearchBox work: bordered and compact containment tests,
  focus-within behavior, clearable/non-clearable native forwarding coverage,
  and migration of the vintage theme probes to semantic selectors.

### July 28, 2026 — Phase 2 Complete

- Added the authoritative packaged registry at
  `python/dragongui/widget_css_capabilities.json`.
- The schema now records each part-capable widget's public CSS type, native
  kind, Python kind, inherited CSS type chain, supported state profile,
  supported parts, universal part property categories, and per-part renderer
  support status.
- Replaced Python's hand-maintained `_SUPPORTED_PARTS_BY_KIND` dictionary with
  data loaded from the packaged registry.
- Replaced native `widget_kind_supports_part` and its separate scrollbar-kind
  match with a one-time native index built from the same packaged JSON.
- Added global generated-content metadata for `::before` and `::after` to the
  canonical registry, including their text-renderer ownership and explicit
  Window, Spacer, and unknown-kind exclusions.
- Replaced the separate native and Python generated-content exceptions with
  registry-driven validation.
- Added `ScrollArea::scrollbar-track` and `ScrollArea::scrollbar-thumb` across:
  - Python inline-part validation.
  - Native CSS validation.
  - Native scrollbar paint fallback catalog and colors.
  - Native fallback provenance.
  - Public documentation and `dg.help`.
- Published RadioButton `indicator`, `dot`, and `label` parts across Python,
  native validation, generated documentation, and help.
- Added native RadioButton coverage for checked, hover, focus, and disabled
  part-state cascade slots.
- Updated `dg.help.reference.css_parts()` to generate public type, state, part,
  and renderer-status data directly from the registry.
- Added `tools/generate_widget_css_capabilities.py`, which owns generated
  capability tables in:
  - `docs/widgets.md`
  - `docs/widgets-reference.md`
  - `docs/css-styling.md`
  - `docs/css-capabilities-reference.md`
  - `docs/sphinx/styling.md`
- Added Python CI drift coverage for generated documentation and registry/type
  chain parity.
- Added native parity coverage proving every native paint-fallback part is
  registered with `paint` renderer status.
- Audited every registered `paint`, `text`, and `structural` part against its
  native consumer:
  - Confirmed `Pane::pane` and `Collapsible::body` are consumed by their
    structural primitive paths.
  - Wired `Heatmap::cell` into heatmap cell fill rendering.
  - Wired `CodeEditor::field` and `CodeEditor::caret` into editor surface,
    border, and caret rendering.
  - Wired `Dropdown::field` into the closed dropdown surface and border.
  - Removed the untruthful `ImageButton::image` capability because the image
    renderer has no independently styleable image part.
- Added focused native renderer regressions for the Heatmap, CodeEditor, and
  Dropdown corrections, plus Python and native rejection coverage for the
  unsupported `ImageButton::image` selector.
- Focused validation completed:
  - Python registry/help/inline-part tests: **4 passed**.
  - Native registry, ScrollArea, provenance, and RadioButton tests:
    **3 passed**.
- Complete validation after the renderer audit:
  - Python suite: **516 passed, 0 failed**.
  - Native suite: **766 passed, 12 ignored, 0 failed**.
  - Generated documentation drift check: **passed**.
  - Targeted Python compilation and native formatting checks: **passed**.
  - Release native extension rebuild and Python 3.12 import smoke:
    **passed**; the local package now contains the updated extension.
- Phase 2 acceptance is complete. Phase 3 composite semantic parts are next.

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
**Status:** Complete

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

- [x] Design one canonical capability schema containing:
  - Public widget type
  - Native widget kind
  - Python widget kind
  - Inherited public CSS type chain
  - Supported pseudo states
  - Supported CSS parts
  - Per-part supported property categories
  - Renderer support status
- [x] Choose the canonical source location and serialization format.
- [x] Generate or validate Python part metadata from the canonical schema.
- [x] Generate or validate native part metadata from the canonical schema.
- [x] Generate public documentation tables from the schema.
- [x] Generate `dg.help` CSS type/state/part reference data from the schema.
- [x] Add CI drift detection.
- [x] Remove duplicate hand-maintained registries where practical.

## ScrollArea Work

- [x] Confirm ScrollArea owns native scrollbar geometry.
- [x] Add `scrollbar-track` and `scrollbar-thumb` to native support if the
      renderer already exposes that geometry.
- [x] Add matching Python part validation.
- [x] Add framework fallback and provenance entries.
- [x] Add public CSS probe coverage.

## RadioButton Work

- [x] Publish `indicator`, `dot`, and `label` as stable RadioButton parts.
- [x] Add them to Python inline-part validation.
- [x] Add them to all reference documentation.
- [x] Add checked, hover, focus, and disabled part tests.

## Tests

- [x] Every documented semantic part is accepted by Python and native CSS
      validation.
- [x] Every accepted semantic part appears in documentation and `dg.help`.
- [x] Every accepted paint part is consumed by a renderer.
- [x] Capability snapshots are identical across generated Python, native, and
      documentation outputs.
- [x] Unknown parts still produce precise widget-specific warnings.

## Acceptance Criteria

- There is one authoritative widget capability inventory.
- ScrollArea scrollbar styling works without warnings.
- RadioButton parts work consistently through Python, CSS, renderer, docs, and
  help.
- CI fails whenever the public inventories drift.

---

# Phase 3 — Composite Widget Semantic Parts

**Priority:** P1  
**Status:** Complete

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

- [x] Define stable SearchBox part names and semantics.
- [x] Decide whether parts map to child widgets, virtual render parts, or
      controlled style forwarding.
- [x] Ensure part styles can affect appropriate layout, visual, and text
      properties.
- [x] Preserve descendant-selector compatibility.
- [x] Ensure part styling does not duplicate focus rings.
- [x] Ensure clearable and non-clearable variants expose truthful parts.
- [x] Ensure disabled SearchBox state propagates to all parts.
- [x] Update the vintage themes to use semantic parts.

### Tests

- [x] Border, background, padding, icon color, and radius apply to each part.
- [x] SearchBox remains bounded with 0–4 px outer borders.
- [x] SearchBox remains bounded at compact widths and high DPI scales.
- [x] SearchBox works with and without a clear button.
- [x] SearchBox focus-within styling remains stable.

## 3.2 Panel Structural Parts

**Status:** Complete

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

- [x] Define geometry and ownership for titled and untitled panels.
- [x] Expose the title text style through `Panel::title`.
- [x] Expose title-band paint and layout through `Panel::header`.
- [x] Expose content surface, padding, and clipping through `Panel::body`.
- [x] Preserve existing `Panel::accent`.
- [x] Make part behavior truthful when a panel has no title.
- [x] Extend the same model to Modal and Sidebar where appropriate.

### Tests

- [x] Header backgrounds and borders do not overlap body content.
- [x] Title text remains vertically aligned at multiple font sizes.
- [x] Body padding participates in scroll geometry.
- [x] Rounded clipping works across separately styled header/body surfaces.
- [x] Existing panel title and scrollbar tests remain valid.

## Acceptance Criteria

- Theme authors do not need internal SearchBox child selectors.
- Titled panels can implement visually distinct header and body regions.
- Composite part contracts remain stable if internal widget composition changes.

---

# Phase 4 — Border and Outline Expansion

**Priority:** P1  
**Status:** Complete

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

- [x] Extend `VisualStyle` to store four border widths and four border colors.
- [x] Store border style per side, with a uniform style as the fallback.
- [x] Parse:
  - `border-top`
  - `border-right`
  - `border-bottom`
  - `border-left`
  - `border-*-width`
  - `border-*-color`
  - `border-*-style`
- [x] Preserve existing `border` shorthand cascade behavior.
- [x] Add `dotted`, `dashed`, and `double` border rendering.
- [x] Add dotted and dashed outline rendering.
- [x] Define how radii intersect with dashed, dotted, and double borders.
- [x] Make all four border widths participate in Taffy layout.
- [x] Extend computed-style provenance for shorthand/longhand collisions.
- [x] Update transition support where practical.

## Tests

- [x] Each side independently affects layout and paint.
- [x] Shorthand and longhand cascade precedence matches documented behavior.
- [x] Border widths scale correctly at non-integer DPI.
- [x] Child content remains inside asymmetric borders.
- [x] Rounded dashed/dotted/double borders remain clipped correctly.
- [x] Focus outlines do not change layout.
- [x] Zero and `none` correctly reset prior border declarations for the solid
  border model.

## Acceptance Criteria

- A menu bar can have only a bottom border.
- A selected tab can suppress or alter one edge.
- A control can render a genuine dotted focus rectangle.
- Raised/recessed controls no longer require multiple inset-shadow workarounds.

---

# Phase 5 — Gradient Stop Units and Pattern Fidelity

**Priority:** P1  
**Status:** Complete

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

- [x] Replace normalized-only gradient positions with a length-percentage
      representation.
- [x] Parse:
  - percentages
  - logical pixels
  - mixed `calc()` positions where feasible
- [x] Resolve stop positions against the painted gradient line at render time.
- [x] Preserve repeating-gradient period correctly for pixel-based stops.
- [x] Support hard stops with two positions or repeated adjacent stops.
- [x] Prevent CSS normalization from emitting syntax the lowerer cannot parse.
- [x] Define behavior for decreasing, omitted, and out-of-range stop positions.
- [x] Update gradient serialization and debug snapshots.

## Tests

- [x] One-pixel and two-pixel repeating stripes remain fixed across widget sizes.
- [x] Pixel patterns scale correctly with DPI.
- [x] Percentage gradients retain current behavior.
- [x] Mixed-unit gradients resolve deterministically.
- [x] Hard-stop shorthand and expanded stop syntax render identically.
- [x] Existing linear, radial, layered, and interpolation tests continue to pass.

## Acceptance Criteria

- Pinstripe and checker patterns can use stable logical-pixel dimensions.
- Gradient parsing produces no warning for supported CSS stop syntax.

---

# Phase 6 — Font Family Fallback Lists

**Priority:** P1  
**Status:** Complete

## Problem

DragonGui stores one `FontFamily`. CSS declarations such as:

```css
font-family: "Chicago", "Geneva", "Arial", sans-serif;
```

do not behave as ordered fallback lists. Cross-platform themes must select one
likely-installed family or accept the renderer's generic fallback.

## Required Work

- [x] Change text style representation from one family to an ordered list.
- [x] Parse quoted names and generic family keywords from CSS lists.
- [x] Preserve family order through inline styles, stylesheets, inheritance,
      transitions, serialization, and snapshots.
- [x] Resolve the first available family at text-shaping time.
- [x] Integrate `@font-face` aliases into ordered fallback resolution.
- [x] Cache family availability and resolved font selections.
- [x] Define stable generic fallbacks for each platform.
- [x] Report missing requested families only in diagnostics, without noisy
      per-frame warnings.

## Tests

- [x] The first installed family is selected.
- [x] Missing families fall through in declaration order.
- [x] Generic families always resolve.
- [x] `@font-face` aliases work as list entries.
- [x] Font measurement and painting select the same resolved face.
- [x] Runtime stylesheet replacement invalidates the correct font caches.

## Acceptance Criteria

- Cross-platform CSS font stacks behave predictably.
- Layout measurement and rendered text remain consistent after fallback.

---

# Phase 7 — Runtime Theme and Stylesheet Management

**Priority:** P2  
**Status:** Complete

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

- [x] Add stable stylesheet identifiers.
- [x] Support add, replace, remove, and clear operations by stylesheet origin
      and identifier.
- [x] Preserve deterministic source order when replacing a stylesheet.
- [x] Add a native command for atomic theme replacement.
- [x] Recompute theme variables, framework styles, inherited styles, layout,
      text, and paint only as necessary.
- [x] Invalidate relevant font, layout, primitive, and resource caches.
- [x] Preserve application state, focus, selection, and scroll offsets.
- [x] Make theme changes safe through `call_soon_threadsafe`.
- [x] Provide a minimal theme-switching example.

## Tests

- [x] Replacing one named stylesheet does not affect others.
- [x] Repeated switching does not increase active stylesheet count.
- [x] Theme tokens update across widgets and parts.
- [x] Layout-affecting token changes trigger layout.
- [x] Paint-only token changes avoid unnecessary structural rebuilds.
- [x] Focus, text selection, active page, and scroll offsets survive switching.
- [x] Debug snapshots report active stylesheet names and source order.

## Acceptance Criteria

- Nexus, Windows 3.11, and Mac OS styles can be switched live.
- Theme switching does not require reconstructing the application.
- The cascade remains bounded and inspectable.

---

# Phase 8 — Styleable Window Chrome

**Priority:** P2  
**Status:** In progress — retained chrome, stable CSS parts, and core input parity implemented
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

- [x] Research Winit support and platform restrictions.
- [x] Define opt-in client-side decoration behavior.
- [x] Preserve dragging, resizing, snapping, system menus, DPI, and keyboard
      commands on supported client-decoration platforms.
- [ ] Connect retained chrome metadata to the shared platform accessibility
      bridge when that foundation is available.
- [x] Expose stable titlebar CSS parts.
- [x] Provide a safe native-decoration fallback where macOS client resizing is
      unavailable.
- [x] Add repeatable automated Windows viewport/DPI visual coverage.
- [ ] Complete manual Windows interaction testing and separate macOS, X11, and
      Wayland platform matrices.

## Progress Log

### 2026-07-29 — Phase 8.1 retained client-chrome foundation

- Added `Window(..., decorations="native" | "client")`; invalid values fail
  during Python construction and native document parsing.
- Kept native OS decorations as the unchanged default.
- Client mode now disables the Winit-provided frame and prepends a retained
  34-logical-pixel titlebar to the window tree. The titlebar therefore consumes
  layout space and follows normal clipping, scaling, cascade, hover, pressed,
  and focus behavior rather than painting over application content.
- Added stable interim CSS type surfaces: `WindowTitlebar`, `WindowTitle`,
  `WindowMinimize`, `WindowMaximize`, and `WindowClose`.
- Wired title/titlebar pointer presses to Winit's native `drag_window()`, which
  preserves compositor/OS movement behavior and associated snapping where the
  platform supports it.
- Wired retained buttons to native minimize, maximize/restore, and close
  behavior. Right-clicking the title/titlebar invokes Winit's window system menu;
  Winit 0.30.13 currently implements this only on Windows.
- Added `examples/client_window_chrome_demo.py` and documented the opt-in API in
  the widget reference.
- Validation: all 527 maintained Python tests pass; all 839 active native tests
  pass (12 benchmark tests ignored); `cargo check` and example byte-compilation
  pass.

### 2026-07-29 — Phase 8.2 DPI-aware resize routing

- Added an eight-direction client resize classifier for north, south, east,
  west, and all four corners.
- Resize hit regions are six logical pixels and are converted to physical
  pixels using the active window scale factor.
- Maximized and zero-sized windows suppress resize hit regions.
- Cursor feedback changes to the matching native resize cursor and is cached so
  ordinary pointer motion does not repeatedly issue identical platform calls.
- Left-pressing an active resize edge routes through Winit's
  `drag_resize_window()` instead of entering widget interaction.
- Added focused unit coverage for every direction, 200% DPI scaling, maximized
  suppression, interior positions, and out-of-window coordinates.
- Updated the client chrome demo and widget reference with supported platforms
  and the current macOS limitation.

### 2026-07-29 — Phase 8.3 titlebar double-click parity

- Added title/titlebar double-click detection and maximize/restore toggling.
- The gesture requires the second press within 500 milliseconds and within four
  logical pixels of the first; the distance threshold scales with DPI.
- A single press still enters the compositor/native window drag path.
- Pressing elsewhere clears the pending titlebar click so unrelated controls
  cannot accidentally complete the gesture.
- Added focused tests for the time limit, movement limit, absent prior click,
  and 200% DPI threshold.
- Validation after Phases 8.2–8.3: all 527 maintained Python tests pass and all
  844 active native tests pass, with 12 benchmark tests ignored. The demo
  byte-compiles and focused diffs pass whitespace validation.

### 2026-07-29 — Phase 8.4 stable part aliases and keyboard activation

- Registered `Window` in the authoritative widget CSS capability registry with
  forwarded `titlebar`, `title`, `minimize`, `maximize`, and `close` parts.
- Generalized the existing composite-part forwarding stage so `Window::part`
  declarations cascade onto the retained titlebar descendants after their own
  styles resolve.
- Kept concrete `WindowTitlebar`, `WindowTitle`, `WindowMinimize`,
  `WindowMaximize`, and `WindowClose` selectors for direct child-state styling.
  This avoids misrepresenting owner-state selectors as child hover state.
- Updated the client chrome demo to use the stable part aliases for base styling.
- Regenerated every capability table from
  `python/dragongui/widget_css_capabilities.json`; the generated-document drift
  check passes.
- Fixed keyboard activation for retained minimize/maximize/close controls:
  focused controls now route Enter and Space to the same native actions as
  pointer activation.
- Added descriptive tooltips for all three glyph-only controls.
- Added native cascade, capability registry, and keyboard routing tests plus
  Python serialization coverage for the tooltips.
- Validation: all 527 maintained Python tests pass; all 846 active native tests
  pass with 12 benchmark tests ignored; generated documentation is current; the
  demo byte-compiles; and focused diffs pass whitespace validation.

### 2026-07-29 — Phase 8.5 resize-border contract and system-menu shortcut

- Registered `Window::resize-border` as a structural CSS part in the
  authoritative capability registry. It intentionally does not synthesize or
  paint a phantom widget.
- The part's `width` controls the edge and corner resize hit thickness in
  logical pixels. It defaults to `6px`, is clamped to `0px`–`24px`, scales
  through the active window DPI, and can be set to `0px` to disable client
  resize hits.
- Routed unmodified Alt+Space in client-decoration mode to Winit's native
  window menu. Winit currently implements that menu on Windows; Ctrl+Alt+Space
  and Super+Alt+Space remain available to applications.
- Added native tests for the default, authored, disabled, and upper-bounded
  resize widths; the Alt+Space modifier contract; capability classification;
  and forwarding of the structural part through the `Window` CSS surface.
- Updated the client chrome demo and widget reference, and regenerated the CSS
  capability tables from the registry.
- Validation: all 527 maintained Python tests pass; all 848 active native tests
  pass with 12 benchmark tests ignored; generated documentation is current; the
  demo byte-compiles; and focused diffs pass whitespace validation.

### 2026-07-29 — Phase 8.6 maximized-state synchronization

- Added the runtime-owned `window-state="normal" | "maximized"` attribute to
  client-decorated `Window` nodes, making operating-system window state
  available to selectors such as
  `Window[window-state="maximized"]::maximize`.
- The retained maximize control now shows the maximize glyph and “Maximize
  window” tooltip in the normal state, then switches to a restore glyph and
  “Restore window” tooltip while maximized.
- Synchronization runs immediately for DragonGUI maximize actions and reconciles
  again on native resize events, covering OS shortcuts, snapping, and native
  system-menu commands without repeated rebuilds when the state is unchanged.
- Registered `window-state` in the selector attribute mask so the optimized
  cascade materializes it only when a stylesheet references it.
- Added native state mutation and state-dependent forwarded-part cascade tests,
  updated Python serialization coverage, and expanded the demo and widget
  reference.
- Validation: all 527 maintained Python tests pass; all 849 active native tests
  pass with 12 benchmark tests ignored; generated documentation is current; the
  demo byte-compiles; and focused diffs pass whitespace validation.

### 2026-07-29 — Phase 8.7 chrome accessibility metadata and traversal

- Added explicit `accessible_name` metadata and an `accessibility_role` of
  `button` to the retained minimize, maximize, and close controls instead of
  relying on their glyph text.
- Kept tooltip and accessible-name state synchronized: the maximize control
  exposes “Maximize window” normally and “Restore window” while maximized.
- Verified that all three client controls participate in the native
  `WidgetState` focus order, in document order, before ordinary application
  controls. Existing Enter/Space activation and visible focus styling remain
  shared with normal buttons.
- Added Python serialization coverage plus native metadata, dynamic-name, and
  exact focus-order regressions.
- Documented the boundary honestly: DragonGUI has no platform accessibility
  tree or screen-reader bridge yet. The metadata is retained groundwork for the
  shared accessibility foundation tracked in
  `plans/V5/03-general-app-completeness.md`, not a claim of current assistive
  technology exposure.
- Updated the client chrome demo and widget reference.
- Validation: all 527 maintained Python tests pass; all 850 active native tests
  pass with 12 benchmark tests ignored; generated documentation is current; the
  demo and Python widget source byte-compile; and focused diffs pass whitespace
  validation.

### 2026-07-29 — Phase 8.8 safe macOS decoration fallback

- Added a startup platform policy that automatically changes an authored
  `decorations="client"` request to native decorations on macOS, where Winit
  does not implement client edge resizing.
- The fallback removes only DragonGUI's synthesized `WindowTitlebar` before the
  tree enters GPU/layout state, preserving all application-authored children and
  avoiding duplicate native and retained titlebars.
- The effective tree records `requested_decorations="client"`,
  `decorations="native"`, and
  `decoration_fallback="macos-client-resize-unsupported"` for diagnostics.
  Startup also prints a concise fallback message.
- Windows and Linux retain the full client-decoration path unchanged.
- Added platform-independent policy and tree-transformation coverage for macOS,
  Windows, Linux, native-decoration requests, application-content preservation,
  and diagnostic metadata.
- Updated the client chrome demo and widget reference. Actual macOS execution
  remains part of the manual platform matrix because this development host is
  Windows.
- Validation: all 527 maintained Python tests pass; all 851 active native tests
  pass with 12 benchmark tests ignored; generated documentation is current; the
  demo byte-compiles; and focused diffs pass whitespace validation.

### 2026-07-29 — Phase 8.9 platform-aware titlebar double-click settings

- Replaced the fixed titlebar gesture detector with an explicit threshold
  contract containing the interval plus independent horizontal and vertical
  movement tolerances.
- On Windows, DragonGUI now reads the user's configured `GetDoubleClickTime()`
  value and `SM_CXDOUBLECLK` / `SM_CYDOUBLECLK` system metrics. This aligns
  maximize/restore gestures with the desktop's input settings.
- The native interval is bounded to 100–2000 ms and each half-rectangle movement
  tolerance to 1–64 physical pixels to reject invalid or pathological platform
  results.
- Linux retains the prior 500 ms and 4-logical-pixel DPI-scaled fallback. macOS
  uses native decorations under the Phase 8.8 safety policy.
- Enabled only `Win32_UI_Input_KeyboardAndMouse` on the existing `windows`
  dependency; no new dependency was added.
- Refactored tests to inject deterministic thresholds, added rectangular
  tolerance coverage, retained DPI fallback coverage, and added a live Windows
  settings sanity test.
- Updated the client chrome demo and widget reference.
- Validation: all 527 maintained Python tests pass; all 853 active native tests
  pass with 12 benchmark tests ignored; generated documentation is current; the
  demo byte-compiles; and focused diffs pass whitespace validation.

### 2026-07-29 — Phase 8.10 automated Windows visual/DPI matrix

- Registered `client-window-chrome` as a permanent visual-audit target backed by
  `examples/client_window_chrome_demo.py`.
- The target covers 720×520, 940×620, and 1280×800 logical viewports at 100%,
  150%, and 200% scale factors. It also exercises 720×520 and 940×620 live
  resize checkpoints during every capture.
- Added a manifest contract test so the three-viewports-by-three-scales matrix
  and resize checkpoints cannot be silently weakened.
- Rebuilt the local Python 3.12 ABI native extension before capture. Python 3.11
  cannot load this project's `abi3-py312` extension and is therefore suitable
  for source-level tests but not native-window visual validation.
- Windows capture results in
  `artifacts/visual_audit/phase8-windows-client-chrome/`:
  - **9/9 native screenshots captured**.
  - **45/45 debug snapshots captured** (initial/final snapshots and the resize
    sequence for each matrix cell).
  - **0 snapshot errors** and **0 layout diagnostics**.
  - The normal-window captures intentionally report
    `Window[window-state="maximized"]::maximize` as unmatched because that rule
    is state-specific and should only match after maximize.
- Visual inspection of the constrained 720×520 captures at 100% and 200%
  confirmed correctly scaled titlebar/control geometry without clipping or
  client-content overlap.
- The audit remains `needs_manual_interaction` by design: screenshots cannot
  prove title dragging, edge/corner resizing, double-click maximize/restore,
  Alt+Space/right-click system menus, minimize/close, or keyboard traversal.
- Validation:
  - Focused visual-audit suite: **30 passed, 0 failed**.
  - Complete maintained Python suite: **528 passed, 0 failed**.
  - Complete native suite: **853 passed, 12 ignored, 0 failed**.
  - Generated CSS capability documentation drift check: **passed**.
  - Demo/audit Python compilation and repository whitespace check: **passed**.

### 2026-07-29 — Phase 8.11 automated Windows maximize-state validation

- Registered a focused `client-window-chrome-maximized` visual-audit target
  separately from the viewport/DPI matrix. Maximized windows correctly ignore
  ordinary resize requests, so keeping this target separate prevents invalid
  resize checkpoints from weakening either test.
- The target sends native Windows pointer input to the stable synthesized
  `#client-chrome-window--dg-window-maximize` control.
- Enabled strict CSS diagnostics for the capture. The run fails if the retained
  control does not transition the window or if
  `Window[window-state="maximized"]::maximize` remains inactive.
- Windows results in
  `artifacts/visual_audit/phase8-windows-client-chrome-maximized/`:
  - The native window expanded from its 940×620 logical startup size to the
    monitor's 1920×1140 client area.
  - The maximized-state selector recorded **1 match**.
  - **1/1 native screenshot** and **1/1 debug snapshot** were captured.
  - **0 unmatched selectors**, **0 snapshot errors**, and **0 layout
    diagnostics** were reported.
- Visual inspection confirmed that the retained titlebar remains above the
  application content at the maximized bounds. The demo's two content panels
  stretch vertically because their parent layout intentionally has
  `flex_grow: 1`; this is not chrome overlap or clipping.
- The target remains `needs_manual_interaction` because this slice does not yet
  prove restore, titlebar double-click, drag, edge/corner resize gestures,
  system-menu behavior, minimize/close, or keyboard traversal.
- Validation:
  - Focused visual-audit suite: **31 passed, 0 failed**.
  - Complete maintained Python suite: **529 passed, 0 failed**.
  - Native implementation was unchanged; the immediately preceding complete
    native run remains **853 passed, 12 ignored, 0 failed**.
  - Generated CSS capability documentation drift check, demo/audit Python
    compilation, and focused whitespace validation: **passed**.

### 2026-07-29 — Phase 8.12 restore validation and deferred titlebar drag

- Added `assert-window-state:normal|maximized` to the visual-audit action
  vocabulary. On Windows it reads `IsZoomed(hwnd)` and fails the capture when
  the observed OS state differs from the expected state.
- Hardened visual-audit result handling: an action exception or debug-snapshot
  exception written as `{"error": ...}` is now promoted to a top-level failed
  target, copied into the capture record, and counted as `capture-error`.
  Previously, the existence of that error JSON could be mistaken for a valid
  snapshot while the target remained successful.
- Added regression coverage proving action/snapshot errors cannot be hidden by
  a separately successful screenshot.
- Registered `client-window-chrome-transitions` with an outcome-checked retained
  maximize-button sequence:
  1. Click maximize.
  2. Assert the Win32 state is maximized.
  3. Click the same retained control in restore mode.
  4. Assert the Win32 state is normal.
- Windows results in
  `artifacts/visual_audit/phase8-windows-client-chrome-transitions/`:
  - Both Win32 state assertions passed.
  - The native window returned to its exact 940×620 logical startup size.
  - **1/1 screenshot** and **1/1 snapshot** were captured.
  - **0 capture errors** and **0 layout diagnostics** were reported.
  - The final normal-state snapshot expectedly lists the maximized-only CSS
    selector as unmatched.
- Live attempts to inject a titlebar double-click exposed a library-level
  interaction flaw: the first titlebar press immediately called Winit's native
  `drag_window()`, allowing the modal drag loop to consume the second press
  before DragonGUI's double-click detector observed it.
- Changed retained titlebar dragging to a pending gesture:
  - A stationary first press records the click without starting an OS drag.
  - Native dragging begins only after two logical pixels of DPI-scaled pointer
    movement.
  - Pending drag state clears on release, focus loss, cursor exit, or a
    successful double-click.
  - This preserves normal dragging while leaving a stationary second press
    available for maximize/restore detection.
- Added focused native coverage for movement below/at the drag threshold at
  100% and 200% scaling. All four titlebar threshold/gesture tests pass.
- External Win32 mouse injection still did not provide a trustworthy live
  titlebar double-click result after the native fix. The unreliable synthetic
  action was removed instead of creating a flaky or falsely green regression
  target. Human titlebar double-click remains on the manual Windows matrix.
- Validation:
  - Focused visual-audit suite: **33 passed, 0 failed**.
  - Complete maintained Python suite: **531 passed, 0 failed**.
  - Complete native suite: **854 passed, 12 ignored, 0 failed**.
  - Generated CSS capability documentation drift check, demo/audit Python
    compilation, and focused whitespace validation: **passed**.

### 2026-07-30 — Phase 8.13 Windows keyboard and system-menu validation

- Audited the remaining Phase 8 scope and separated deterministic Windows
  outcomes from gestures that still require a human or off-host compositor.
- Extended the visual-audit action contract with native message-queue clicks,
  keyboard input, focus assertions, minimized/maximized/normal Win32 state
  assertions, and native system-menu state assertions.
- Registered `client-window-chrome-windows-input` with three fail-closed states:
  - Exact focus traversal from an application button through retained minimize,
    maximize, and close controls to the next application control.
  - Enter-driven maximize and Space-driven restore, with focus and
    `IsZoomed(hwnd)` checked after the transitions.
  - Alt+Space system-menu opening and Escape dismissal, with
    `GetGUIThreadInfo` checked before and after dismissal.
- Kept minimize, close, retained-titlebar right-click, double-click, dragging,
  and edge/corner resizing on the human Windows matrix. Same-process synthetic
  pointer injection was not reliable enough for those modal OS gestures, so it
  was not counted as coverage.
- Hardened screenshot capture so an unavailable Win32 crop returns a failed
  capture result instead of crashing the entire audit runner.
- Added the repeatable platform checklist in
  `plans/window-chrome-platform-validation.md`, including Windows DPI,
  mixed-DPI monitor, macOS fallback, X11, and native Wayland pass criteria.
- Windows result in
  `artifacts/visual_audit/phase8-windows-client-chrome-input-final/`:
  - **3/3 action states passed**.
  - **3/3 debug snapshots** were valid.
  - **0 capture errors** and **0 layout diagnostics**.
  - The maximized-only selector is expectedly unmatched in the final normal
    state.
- Validation:
  - Focused visual-audit suite: **34 passed, 0 failed**.
  - Complete maintained Python suite: **536 passed, 0 failed**.
  - Live Windows input target: **passed**.
  - Demo/audit Python compilation, manifest JSON parsing, and focused
    whitespace validation: **passed**.

### 2026-07-30 — Phase 8.14 constrained client-title regression

- THEME FORGE exposed a retained-chrome defect at narrow window widths: a
  long `Window::title` kept its intrinsic text width and pushed the fixed
  minimize/maximize/close controls outside the titlebar.
- Hardened the library-owned title node with a zero flex basis,
  `flex-shrink: 1`, `min-width: 0`, hidden overflow, no wrapping, and ellipsis.
  Retained window controls now explicitly opt out of flex shrinking.
- Added native geometry coverage at 320, 390, and 640 logical pixels under
  100%, 150%, and 200% scaling. It verifies constrained title shrinkage,
  ordered controls, titlebar containment, and nonzero control clips.
- Added a headless THEME FORGE smoke test that builds all twelve workspaces,
  verifies named stylesheet order, and checks the retained title/control
  serialization contract.
- Extended visual-audit targets with explicit script arguments, then
  registered:
  - `theme-forge`, covering all twelve routes with deterministic live updates
    disabled.
  - `theme-forge-long-title`, covering the three narrow viewports at all three
    DPI scales with `--long-title`.
- Removed a redundant debug snapshot from no-resize audit captures and scaled
  the remaining snapshot timeout for high-DPI stress documents.
- Validation:
  - Focused Python/API/audit suite: **37 passed, 0 failed**.
  - Native title geometry regression: **1 passed at nine size/DPI
    combinations**.
  - Live twelve-workspace `theme-forge` baseline: **12/12 captures passed**,
    with no layout diagnostics.
  - Live `theme-forge-long-title` visual matrix: **9/9 captures passed**, with
    no layout diagnostics.

### Winit 0.30.13 platform findings

| Capability | Winit support relevant to Phase 8 |
|---|---|
| Decorations on/off | Desktop support through `with_decorations` / `set_decorations`; no effect on mobile/web |
| Window dragging | Windows, macOS, X11, and Wayland; mobile/web report unsupported |
| Edge resizing | Windows, X11, and Wayland; macOS and mobile/web report unsupported |
| Minimize | Desktop support; Wayland cannot programmatically un-minimize |
| Maximize/restore | Desktop support |
| Native system menu | Windows only |

### Remaining Phase 8 work

- Connect the completed chrome metadata to DragonGUI's future shared
  accessibility tree and platform bridge; this cross-library foundation is
  tracked in `plans/V5/03-general-app-completeness.md`.
- Run the manual interaction matrix on Windows. Automated Windows geometry
  coverage now exists for three viewports and three DPI scales, and retained
  maximize/restore activation and state styling are verified with native
  pointer input plus Win32 outcome assertions. Keyboard traversal,
  Enter/Space maximize/restore, and Alt+Space menu open/dismiss now also have
  outcome-checked automation. Human titlebar double-click/right-click,
  dragging, resize gestures, and minimize/close remain manual.
- Run behavior and DPI matrices on macOS, X11, and Wayland before marking
  Phase 8 complete. Use
  `plans/window-chrome-platform-validation.md` as the recorded checklist.

## Acceptance Criteria

- Native decorations remain the reliable default.
- Opt-in custom chrome is functional, accessible, and correctly scaled.
- Applications can achieve a visually coherent full-window theme.

---

# Phase 9 — Icon Theme Infrastructure

**Priority:** P2  
**Status:** Complete

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

- [x] Define a bounded resource-backed monochrome stroke representation.
- [x] Support built-in aliases.
- [x] Support startup application overrides.
- [x] Support live application override replacement and live icon reconciliation.
- [x] Keep icon names semantic rather than widget-specific.
- [x] Preserve monochrome tintable built-in icons as the first rendering tier.
- [x] Evaluate SVG/path input and adopt a safe compact polyline resource first.
- [x] Define sizing, alignment, stroke, tint, and disabled-state behavior.
- [x] Cache parsed normalized icon geometry across primitive rebuilds.
- [x] Expose resolved icon identity in debug snapshots.

## Progress Log

### 2026-07-29 — Phase 9.1 canonical icon identity audit started

- Audited the Python widget API, native primitive renderer, text fallback, and
  computed-style snapshot path.
- Confirmed that icon aliases and fallback behavior currently live in a private
  Rust paint `match`: callers cannot query supported names, unknown names
  silently render the `more` glyph, and debug snapshots do not report that
  substitution.
- Scoped the first implementation slice to a shared canonical built-in
  identity/alias contract, a public Python resolution API, native paint/text
  use of the same resolution semantics, and resolved identity diagnostics.
- Resource-backed replacement geometry and application-level override
  registration remain subsequent Phase 9 slices; this foundation is designed
  so those features can replace a resolved identity without changing widget
  layout.

### 2026-07-29 — Phase 9.1 canonical icon identity foundation complete

- Added public `BUILTIN_ICONS`, `ICON_ALIASES`, `IconResolution`,
  `normalize_icon_name()`, and `resolve_icon()` Python APIs.
- Centralized native built-in and alias resolution in `native/src/icons.rs`;
  primitive painting and the help/warning text fallback now consume canonical
  identities instead of maintaining separate alias branches.
- Preserved unknown-name compatibility: unregistered semantic names still
  render the `more` glyph, while callers can now detect that fallback before
  launching a GUI.
- Added the previously used-but-unrecognized `folder-open` alias, so existing
  demos now paint folder geometry instead of generic dots.
- Computed-style snapshots for icon-bearing controls now expose `requested`,
  `resolved`, `recognized`, `alias`, `fallback`, and `source` diagnostics.
- Updated `dg.help()`, the concise and detailed Markdown widget references, and
  the Sphinx widget guide. The manual's public-export coverage guard includes
  all five new symbols.
- Validation:
  - Python: **532 passed, 0 failed**.
  - Native: **856 passed, 12 ignored, 0 failed**.
  - Focused help/icon API checks: **7 passed, 0 failed**.
  - Rust formatting, Python compilation, and generated CSS capability drift
    checks pass.
- Next Phase 9 slice: define the resource-backed monochrome geometry format and
  application override registry, then route resolved custom resources through
  the renderer without changing the widget's layout box.

### 2026-07-29 — Phase 9.2 resource contract implementation started

- Chose a bounded, JSON-serializable monochrome stroke resource instead of
  accepting arbitrary SVG/XML in the first replacement tier.
- The initial resource uses a logical `view_box`, one shared logical
  `stroke_width`, and one or more polylines with optional closure. Coordinates
  are finite, resource complexity is capped, and paint color continues to come
  from `IconButton::icon`.
- Application overrides are keyed by normalized semantic icon names through
  `App.set_icon_theme(...)`. A value may be a custom stroke resource or another
  semantic built-in name.
- Phase 9.2 initially resolves resources into retained widget nodes at startup.
  This keeps layout unchanged and avoids per-frame registry lookup/parsing.
  Retaining and reconciling the registry for live theme replacement and later
  `IconButton.set_icon()` calls is explicitly deferred to Phase 9.3.

### 2026-07-29 — Phase 9.2 startup resource overrides complete

- Added public immutable `IconStroke` and `IconResource` types. Resources use a
  logical view box, shared stroke width, and open/closed polyline geometry.
- Enforced the same safety contract in Python and native startup parsing:
  finite coordinates, positive view-box dimensions and stroke width, at least
  two points per stroke, at most 64 strokes, and at most 256 total points.
- Added `App.set_icon_theme(mapping)`. Override keys are normalized semantic
  names; values may be resources or semantic aliases. Native alias resolution
  follows application chains, recognizes built-ins, and rejects cycles or
  unknown terminal aliases.
- Application resources resolve into retained icon nodes once at startup.
  Built-in aliases are considered when matching an override, so an override
  for `search` also replaces an `IconButton("zoom")`.
- Custom geometry uses the existing monochrome icon-part paint color, including
  CSS tint and disabled styling, and is uniformly centered within the same
  fixed icon/button layout box used by built-ins.
- Invalid or absent custom data safely falls back through the existing built-in
  renderer. Unoverridden icon names are unchanged.
- Extended computed-style snapshots: custom resources and application aliases
  report `source: "application"`; stroke resources also report
  `resource_type: "stroke"`.
- Added `examples/icon_theme_demo.py` and a manifest-driven `icon-theme` visual
  audit target. The matrix passes at **1×, 1.5×, and 2×** with three
  screenshots, three snapshots, and zero layout diagnostics. Manual inspection
  confirmed centered geometry, consistent tint, and stable button boxes.
- Updated `dg.help()`, concise/detailed widget documentation, and Sphinx docs.
- Validation:
  - Python: **533 passed, 0 failed**.
  - Native: **860 passed, 12 ignored, 0 failed**.
  - Visual-audit tests: **33 passed, 0 failed**.
  - Icon-focused native tests: **11 passed, 0 failed**.
  - Rust formatting, Python compilation, generated CSS capability drift, and
    patch whitespace checks pass.
- Remaining Phase 9 work is now narrowly defined: retain the icon registry in
  runtime state for live `set_icon_theme()` and `IconButton.set_icon()`
  reconciliation, then add a parsed/tessellated geometry cache and decide
  whether a curve-capable second resource tier is justified.

### 2026-07-29 — Phase 9.3 live registry implementation started

- The validated icon-theme registry will become retained native runtime state
  rather than a startup-only transformation input.
- `App.set_icon_theme()` will atomically replace that registry before or during
  runtime. Live replacement re-resolves existing icon nodes and requests
  targeted text+visual work only; it does not reconstruct the widget tree or
  run layout. Text participation is required because built-in help/warning
  marks contain a shaped glyph.
- `IconButton.set_icon()` will normalize and update the retained semantic name,
  immediately reconcile it against the current application registry, and
  request text+visual work because help/warning icons may add or remove a
  shaped fallback glyph.
- Live `ReplaceNode` and `ReplaceChildren` operations will reconcile newly
  inserted icon-bearing subtrees before retained maps and rendering rebuild.
- Invalid live registries are rejected on the Python/native bridge before they
  enter the command queue. Repeated identical themes are no-ops.

### 2026-07-29 — Phase 9.3 live registry and reconciliation complete

- Promoted the validated `IconThemeRegistry` into retained `WgpuState` and
  carried the startup registry through `AppSpec`.
- Added the native `SetIconTheme` runtime command and Python bridge support.
  `App.set_icon_theme()` now works before startup and live using the same
  serialization and validation contract.
- Live replacement atomically reconciles all retained icon-bearing nodes.
  Identical registries are no-ops, and multiple queued replacements coalesce
  to the newest registry before rendering.
- Implemented live `IconButton.set_icon()` handling in the native retained
  tree. The semantic identity is normalized, resolved against the active
  registry, and rebuilt as targeted text+visual work without layout.
- Reconciled icon resources after live `ReplaceNode` and `ReplaceChildren`
  operations, ensuring newly inserted subtrees inherit the active application
  icon theme before retained maps and rendering rebuild.
- Fixed icon text participation for application overrides: semantic aliases to
  help/warning use their shaped `?`/`!` mark, while a custom stroke resource
  suppresses the built-in symbol text instead of painting both.
- Added runtime diagnostics under `icon_theme` with override count, retained
  status, and live-replacement capability. Per-widget computed-style snapshots
  continue to identify built-in versus application resolution.
- Extended `examples/icon_theme_demo.py` with live theme replacement and live
  semantic-identity controls. The visual-audit manifest now captures default,
  live-theme-swapped, and live-icon-changed states at 1×, 1.5×, and 2×.
- Final visual matrix: **9 screenshots, 9 snapshots, 0 errors, and 0 layout
  diagnostics**. Manual inspection confirmed the geometry swap, help-glyph
  transition, stable tint, and unchanged control boxes.
- Updated `dg.help()`, both Markdown widget guides, and Sphinx documentation to
  describe the live lifecycle.
- Validation:
  - Python: **533 passed, 0 failed**.
  - Native: **865 passed, 12 ignored, 0 failed**.
  - Icon-focused native checks: **16 passed, 0 failed**.
  - Rust formatting, Python compilation, generated CSS capability drift, and
    visual-audit manifest checks pass.
- Rebuilt and copied the Python 3.12 native extension after the final changes.
- Phase 9.4 is the remaining performance slice: cache parsed normalized icon
  geometry (and, where useful, emitted/tessellated templates) across primitive
  rebuilds, expose cache metrics, and benchmark theme replacement versus steady
  state. A curve-capable second format remains optional unless real themes
  demonstrate that bounded polylines are insufficient.

### 2026-07-30 — Phase 9.4 parsed geometry cache implementation started

- Audited all custom-icon paint paths. Full primitive rebuilds, targeted
  base-subtree rebuilds, and modal overlay rebuilds currently parse the retained
  JSON resource and allocate transformed point arrays every time an icon is
  emitted.
- The cache is owned by `PrimitivesRenderer`, so parsed geometry survives
  retained-tree paint rebuilds without introducing process-global or
  thread-local state.
- Cache entries are keyed by the validated JSON value content rather than
  semantic icon name. Live theme replacement therefore needs no explicit
  invalidation: unchanged resources reuse their parsed geometry, changed
  resources receive a different key, and old resources age out naturally.
- Parsed entries retain only normalized view-box, stroke-width, point, and
  closure data behind shared references. Rect-, scale-, color-, and
  clip-dependent line instances remain per-emission work because they are not
  reusable across differently sized or styled widgets.
- The cache is bounded to **128 resources** with deterministic oldest-entry
  eviction. This comfortably exceeds the built-in semantic icon inventory while
  placing a hard limit on retained theme-swap history.
- Renderer diagnostics will expose entries, hits, misses, evictions, and parse
  failures. Regression coverage will use deterministic hit/miss assertions
  instead of timing thresholds; steady-state repeated lookup should produce one
  parse followed by cache hits, while changed theme resources should produce
  one additional miss.

### 2026-07-30 — Phase 9.4 cache implementation and focused tests complete

- Added a renderer-owned `IconGeometryCache` with a 128-resource bound,
  content-keyed lookup, shared parsed geometry, and deterministic oldest-entry
  eviction.
- Split custom icon handling into parse and emit stages. Cached logical points
  are transformed directly into line primitives, removing repeated numeric
  decoding and the previous temporary transformed-point allocation.
- Content-key lookup uses the retained `serde_json::Value` directly. It performs
  no steady-state serialized-key allocation; the resource value is cloned only
  when a miss inserts a new bounded cache entry.
- Routed the same retained cache through full-tree, targeted base-subtree, and
  modal-overlay primitive emission. Theme replacement intentionally leaves the
  content-addressed cache intact.
- Extended primitive renderer diagnostics with an `icon_geometry_cache` object:
  `capacity`, `entries`, `hits`, `misses`, `evictions`, and `parse_failures`.
- Added deterministic regressions for:
  - one parse followed by **999 cache hits** for steady-state reuse;
  - bounded eviction across successive replacement resources;
  - invalid-resource accounting without retaining invalid entries;
  - unchanged tinted segment output from parsed geometry; and
  - the public runtime diagnostic snapshot shape.
- Focused validation: **6 passed, 0 failed** across cache, geometry emission,
  and existing custom-resource paint tests. Native test compilation and Rust
  formatting pass.

### 2026-07-30 — Phase 9.4 and Phase 9 complete

- Full validation:
  - Python: **533 passed, 0 failed** under the available Python 3.11 test
    environment.
  - Native: **870 passed, 12 ignored, 0 failed** with the Python 3.12 runtime
    shim required by the ABI3 test binary.
  - Rust formatting, native compilation, and targeted patch whitespace checks
    pass.
- Rebuilt the optimized Python 3.12 native extension and copied it into the
  source package before visual verification.
- The manifest-driven icon-theme audit passed all **9 screenshots/snapshots**
  across 1×, 1.5×, and 2× for default, live-theme-swapped, and
  live-icon-changed states. It reported **0 capture errors, 0 layout issues,
  and 0 unmatched selectors**.
- A first visual run had one transient startup debug-snapshot timeout at the
  default 1× state; its other eight captures passed. Repeating the complete
  matrix with a longer startup stabilization delay passed all nine, indicating
  an audit startup timing event rather than a renderer or cache failure.
- Runtime snapshots confirmed the intended behavior: default rendering retained
  two parsed resources after two misses and four hits; the live theme swap
  retained three resources after three misses and five hits. Every capture
  reported zero cache evictions and parse failures.
- Visual spot checks confirmed stable icon-button sizing, tint, alignment, and
  built-in fallback across the three live states.
- Phase 9 now satisfies all required work and acceptance criteria. A
  curve-capable resource format remains an optional future extension, not a
  completion dependency.

## Acceptance Criteria

- A theme can replace common icon shapes without replacing widget classes.
- Icon layout remains stable across themes.
- Missing overrides fall back to built-in icons.

---

# Phase 10 — Local Background Resources and Patterns

**Priority:** P3  
**Status:** Complete — procedural patterns and managed application images shipped

## Problem

`background-image: url(...)` is unsupported. This is a sound default for remote
resources, but themes cannot use packaged local textures or tiles. Repeating
gradients can approximate some patterns but are not ideal for dithering,
bitmaps, or branded surfaces.

## Required Work

- [x] Define a safe application-resource URL or identifier syntax.
- [x] Continue rejecting remote HTTP/HTTPS resources by default.
- [x] Resolve resources through DragonGui's managed resource registry.
- [x] Support contain, cover, stretch, and repeat modes as a focused subset.
- [x] Add a built-in pattern paint for:
  - [x] checker
  - [x] pinstripe
  - [x] dot/stipple
  - [x] diagonal hatch
- [x] Ensure procedural-pattern clipping, radii, opacity, and transforms work.
- [x] Ensure managed-image clipping, radii, opacity, and transforms work.
- [x] Add resource lifetime and cache tests.

## Progress Log

### 2026-07-30 — Phase 10 audit and implementation split

- Audited the CSS background parser, resolved `BackgroundPaint` model,
  primitive renderer, image renderer, managed buffer registry, computed-style
  snapshots, and current documentation.
- Confirmed that gradient and layered background paints already share one
  rounded-rectangle primitive pipeline. That pipeline applies local clipping,
  corner radii, node opacity, transforms, and DPI-aware logical sizing.
- Confirmed that the existing `ImageRenderer` owns path-decoded widget textures,
  while `ResourceRegistry` owns table and byte-buffer payloads. Neither is
  currently a safe application-background texture registry; routing CSS
  `url(...)` directly to filesystem paths would bypass application ownership,
  lifetime accounting, and remote-source policy.
- Split Phase 10 into two independently testable slices:
  1. **Phase 10.1 — procedural pattern paint:** add common texture-free patterns
     to the existing background paint pipeline.
  2. **Phase 10.2 — managed application images:** define an application resource
     identifier, register/decode/cache packaged image bytes, and implement
     contain/cover/stretch/repeat sampling without enabling arbitrary remote
     loading.
- Phase 10.1 uses the explicit DragonGUI CSS function:

  ```css
  Panel {
      background-image: dg-pattern(checker, #253142, #1d2633, 8px);
  }
  ```

- The first grammar is deliberately bounded:
  - exactly four arguments: pattern kind, foreground color, background color,
    and logical tile size;
  - kinds: `checker`, `pinstripe`, `dot`/`stipple`, and
    `diagonal-hatch`;
  - tile size: finite pixel length from **2px through 128px**;
  - colors use the normal CSS color/token/variable pipeline;
  - no file path, URI, script, arbitrary shader, or unbounded geometry input.
- Pattern rendering uses one existing rectangle instance per layer and one
  bounded shader branch. It does not tessellate repeated cells on the CPU, so
  cost remains stable as panels grow.
- Phase 10.2 will retain the current rejection of all `url(...)` values until
  the managed application-resource registry and lifetime contract exist.

### 2026-07-30 — Phase 10.1 procedural pattern implementation complete

- Added `BackgroundPattern`/`BackgroundPatternKind` to the resolved style model
  and the corresponding bounded CSS representation.
- Added `dg-pattern(kind, foreground, background, tile-size)` parsing for
  `background` and `background-image`, including normal CSS colors, theme
  tokens, variables, layered paint composition, and the `stipple` alias.
- Invalid kinds, argument counts, non-pixel units, non-finite values, and tile
  sizes outside `2px..128px` produce CSS warnings and no paint declaration.
- Added one GPU rectangle-paint branch for checker, pinstripe, dot/stipple, and
  diagonal hatch. Pattern repetition happens in the fragment shader rather than
  by emitting repeated CPU geometry.
- Pattern instances retain the existing paint pipeline's rounded radii, local
  clip, opacity, transforms, background layers, and logical-to-physical DPI
  scaling.
- Computed-style snapshots now expose pattern kind, foreground/background color
  references, and logical tile size.
- Added `background_patterns_probe.py` and the strict manifest target
  `background-patterns` at 1×, 1.5×, and 2×.
- Updated the CSS capability reference, styling guide, widget reference, Sphinx
  styling guide, and `dg.help()` CSS property guidance.
- Focused validation: **6 passed, 0 failed** covering parsing, bounds, layers,
  DPI/opacity/radii instance encoding, snapshot serialization, and WGSL parsing
  and validation.
- Full native/Python suites, release-extension rebuild, and visual verification
  remain before Phase 10.1 is marked complete.

### 2026-07-30 — Phase 10.1 procedural patterns complete

- Full validation:
  - Python: **533 passed, 0 failed**.
  - Native: **875 passed, 12 ignored, 0 failed**.
  - Rust formatting, Python compilation, manifest JSON parsing, and targeted
    patch whitespace checks pass.
- Rebuilt and copied the optimized Python 3.12 native extension before visual
  validation.
- The strict `background-patterns` visual target passed **3 screenshots and
  snapshots** at 1×, 1.5×, and 2× with **0 capture errors, 0 layout issues,
  0 unmatched selectors, and 0 CSS warnings**.
- Visual inspection confirmed stable logical tile scaling, rounded clipping,
  background-color layering, and transformed diagonal-hatch painting at every
  scale.
- The first probe run revealed two unrelated unsupported `text-shadow`
  declarations in the probe stylesheet. They were removed, and the complete
  matrix was rerun warning-free.
- Phase 10.1 is complete. Phase 10.2 remains intentionally separate: define and
  implement managed application image resources plus
  contain/cover/stretch/repeat sampling while continuing to reject arbitrary
  HTTP/HTTPS and filesystem `url(...)` sources.

### 2026-07-30 — Phase 10.2 managed-image architecture audit

- Audited the native `ImageRenderer`, its PNG/JPEG path decoder and texture
  cache, image-widget fit calculations, render ordering, retained
  `ResourceRegistry`, buffer upload/release commands, Python `AppHandle` queue,
  normal startup, loading-screen startup, and debug resource snapshots.
- Reuse the existing retained byte-resource transport rather than add a second
  queue protocol. Managed images use the reserved native resource kind
  `image_encoded`; the registry already supplies app ownership, versioning,
  live replacement, explicit release, and command coalescing.
- Add public Python APIs:

  ```python
  app.set_image_resource("linen", package_bytes)
  app.set_image_resource("logo", Path("assets/logo.png"))
  app.release_image_resource("linen")
  ```

- Registration accepts bytes-like encoded data or an explicit Python
  filesystem path. Path access occurs at the application call site, not from
  CSS or the native renderer. This makes local I/O visible and controlled by
  application code.
- Resource identifiers are trimmed semantic names limited to 128 characters
  and `[A-Za-z0-9._-]`. Encoded data must be non-empty PNG or JPEG and no larger
  than **16 MiB**. Native decode will additionally cap dimensions to
  **4096×4096** and **16 million pixels** before GPU upload.
- Pre-run registrations are retained by `App` and queued before the native
  document starts. Live calls atomically replace the same retained ID. Explicit
  release removes both the Python startup copy and native retained/GPU state.
- The CSS syntax for the rendering slice is:

  ```css
  Panel {
      background-image: app-resource("linen", cover);
  }
  ```

- The second argument is one of `contain`, `cover`, `stretch`, or `repeat`.
  `app-resource(...)` accepts only a registry identifier; it never accepts a
  path, `http:`, `https:`, `file:`, `data:`, or general URI.
- Background image draws will reuse the image texture pipeline but remain
  distinct from path-backed `Image` widgets. They are inset inside the resolved
  border so the existing primitive border remains visible, and they render
  before text. The image instance contract must gain opacity, explicit paint
  clip, and repeat sampling before the slice is complete.

### 2026-07-30 — Phase 10.2 managed application images complete

- Added `App.set_image_resource(id, encoded_bytes_or_path)` for startup
  registration and atomic live replacement, plus
  `App.release_image_resource(id)` for Python/native/GPU release. Both normal
  and loading-screen startup paths queue retained image resources.
- Added defense in depth at Python and native bridge boundaries:
  - IDs are 1–128 characters from `[A-Za-z0-9._-]`.
  - Only non-empty PNG/JPEG payloads up to 16 MiB are accepted.
  - Native decode caps width and height at 4096, pixels at 16 million, and
    decoder allocation at 64 MiB.
  - CSS accepts only semantic IDs. Paths, `url(...)`, HTTP(S), file/data URIs,
    unquoted IDs, and arbitrary URI sources remain rejected.
- Added `background-image: app-resource("id", fit)` for `contain`, `cover`,
  `stretch`, and `repeat`. Managed image/procedural multi-layer mixtures are
  deliberately rejected until exact cross-pipeline layer ordering exists;
  `background-color` is the supported fallback layer.
- Refactored the retained image renderer:
  - Path-backed widgets and app resources use distinct cache keys.
  - Managed cache entries track resource versions and refresh on replacement.
  - Clamp and repeat sampler bind groups share each uploaded texture.
  - Image instances carry opacity, rounded geometry, transforms, and an
    explicit scroll/paint clip.
  - Backgrounds render before normal widget primitives, while path-backed
    `Image` and extension display-list content keep their later content pass.
    Panel backgrounds therefore cannot cover child controls.
  - The shader composes `background-color` beneath transparent pixels and
    contain margins. Missing, released, or rejected resources use a transparent
    sentinel texture, preserving the fallback and border.
- Extended resource/debug snapshots with managed image counts, encoded bytes,
  resource versions, and computed `{resource_id, fit}` image paint details.
- Added parser, bridge validation, registry lifecycle, fit/UV,
  background-spec, startup/live API, replacement, release, and fallback tests.
- Added `managed_background_images_probe.py` and the strict
  `managed-background-images` visual target. It covers default, live
  replacement, and live release at 1×, 1.5×, and 2×.
- Validation:
  - `cargo check` and Rust formatting: pass.
  - Native: **881 passed, 12 ignored, 0 failed**.
  - Python: **535 passed, 0 failed**.
  - Release wheel/extension rebuilt and import-verified.
  - Visual audit: **9/9 captures passed**, with zero layout issues, unmatched
    selectors, capture errors, or stderr diagnostics. Artifacts are in
    `artifacts/visual_audit_managed_background_images_live/`.
- The initial native harness launch failed before executing tests because
  Windows selected Python 3.9's `python3.dll` for the abi3-py312 executable.
  Placing Python 3.12's stable-ABI DLL beside the generated test executable
  corrected the local environment; the complete suite then passed.
- Phase 10 is complete.

## Acceptance Criteria

- Applications can use packaged local surface assets safely.
- Common patterns do not require large textures or unstable gradient tricks.

---

# Phase 11 — Theme Token Expansion

**Priority:** P3  
**Status:** Complete

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

- [x] Decide which values belong in Theme versus framework CSS.
- [x] Expose new values as `--dg-*` custom properties.
- [x] Preserve CSS override precedence.
- [x] Add serialization and runtime replacement support.
- [x] Avoid making Theme a second independent styling language.

## Ownership Decision

Theme owns high-value system defaults: application and monospace font stacks,
base line height, standard/compact control heights, default border width, focus
width/offset, panel padding, and toolbar gap. Framework CSS consumes these
values and exposes them to later stylesheets.

Detailed component padding, shadow recipes, transitions, animation durations,
and easing remain CSS-owned. This keeps Theme compact and prevents a parallel
widget styling API. A Theme replacement changes the framework-wide default;
theme/user CSS and inline declarations remain the mechanisms for scoped
exceptions.

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

- [x] Phase 1 complete: color pipeline
- [x] Phase 2 complete: capability registry
- [ ] Border-box behavior has broad regression coverage

### Core Theme Expressiveness

- [x] Phase 3 complete: composite parts
- [x] Phase 4 complete: borders and outlines
- [x] Phase 5 complete: gradient units
- [x] Phase 6 complete: font fallback lists

### Runtime Theme System

- [x] Phase 7 complete: named stylesheet and theme replacement
- [x] Phase 11 complete: expanded theme tokens

### Advanced Appearance

- [x] Phase 9 complete: icon themes
- [x] Phase 10 complete: local resources and patterns
- [ ] Phase 8 complete: optional window chrome

### Release Validation

- [x] Full Python suite passes
- [x] Full native suite passes
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
