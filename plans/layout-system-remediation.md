# Layout System Remediation Plan

## Purpose

This plan addresses the framework-level layout, spacing, clipping, overflow,
and scrolling problems found during the July 2026 layout audit. The goal is to
make ordinary DragonGUI compositions remain bounded and usable without requiring
application authors to repeatedly add defensive `min_width`, `min_height`,
`flex_shrink`, and `overflow` overrides.

The audit found six related classes of problems:

1. The automated smoke layout auditor reads an obsolete snapshot shape and
   silently skips most widgets.
2. Percentage and `calc()` dimensions can disable flex shrinking and extend
   outside their parent.
3. Intrinsic widget measurement and flow wrapping are predicted by a separate
   approximate sizing system instead of the renderer's text metrics.
4. Scroll ranges are reconstructed from mutable final rectangles and can lose
   overflow or bottom/right padding.
5. Safe shrink and minimum-size behavior is present in newer helpers such as
   `AppShell` and `Body`, but not consistently present in the base window,
   container, panel, and composite-widget defaults.
6. Several custom passes move and resize rectangles after Taffy has solved the
   layout without a general parent reflow.

The native layout test run at the time of the audit reported:

```text
110 passed
11 failed
1 ignored
```

The failing tests cover percentage-width overflow, flow layout, checkbox and
badge intrinsic sizing, font-size-aware sizing, panel and horizontal scrolling,
bottom scroll padding, menu metrics, and titled-panel positioning.

## Progress

Last updated: 2026-07-24.

**Plan status: Complete.**

### Completed

- Added `layout.schema_version = 1` to native debug snapshots.
- Repaired the strict smoke auditor to consume `layout.rects`, `layout.clips`,
  and `layout.diagnostics` instead of silently looking for widget IDs at the
  obsolete top level.
- Expanded strict auditing to the main layout/container types and added
  diagnostics for missing geometry, unowned overflow, hidden unreachable
  content, missing scroll ranges, and normal-flow sibling overlap.
- Added focused audit fixtures covering invalid schema, percentage overflow,
  owned scrolling, missing ranges, hidden overflow, inactive pages, sibling
  overlap, and fixed overlays.
- Changed percentage dimensions to remain shrinkable preferred sizes, fixing
  the reproduced `width: 100%` child that extended from a 360-pixel row to
  x=540.
- Propagated numeric widget `width` and `height` props into native Taffy styles
  for widgets that are actually laid out, while keeping inactive overlays out
  of normal flow.
- Restored horizontal scroll ranges for fixed-width children.
- Made panel scroll regressions use explicit content geometry and verified that
  maximum scroll preserves resolved bottom padding.
- Rebased stale layout metric assertions on the active compact theme contract
  and scale-aware tolerances.
- Made authored dimensions axis-aware preferred sizes: a cross-axis height no
  longer disables horizontal flex behavior, and normal-flow percentage,
  logical-pixel, and `calc()` sizes shrink unless `flex_shrink: 0` or a fixed
  widget prop explicitly requests non-shrinking behavior.
- Preserved preferred main-axis sizes inside explicit visible, hidden, and
  scrolling overflow owners so scroll content is not collapsed merely to fit
  the viewport.
- Normalized zero minimum sizes for panels, sidebars, pages, grids, and
  plot/table viewports; made grids, pages, plots, and tables shrinkable flex
  items.
- Made top-level `HLayout` and `VLayout` window bodies shrink between fixed
  window chrome while retaining intrinsic sizing for nested row/column wrappers
  that serve as scroll content.
- Added regressions for percentage grids, percentage plots, multi-scale
  `calc()` sizing, the explicit `flex_shrink: 0` escape hatch, and a vertically
  scrolling window body between menu and status bars.
- Captured Taffy's resolved border and padding metrics for every laid-out
  widget and introduced shared `ResolvedBox` geometry for border, padding, and
  content boxes.
- Replaced separate horizontal and vertical range reconstruction with one
  `ScrollGeometry` path that owns the effective viewport, descendant content
  bounds, and both maximum offsets.
- Migrated titled-container child clipping and post-layout grid/column
  bottom-padding calculations to resolved Taffy metrics instead of re-reading
  authored padding with fallback constants.
- Added a both-axis regression proving that resolved `calc()` right/bottom
  padding is reachable exactly once and is reflected in both stored scroll
  maxima.
- Added shared `TitledContainerGeometry` for the resolved title box, header
  band, body viewport, and body content origin.
- Migrated scroll clipping, absolute body positioning, scrollbar placement,
  modal header painting, modal close-button sizing, and panel/modal title text
  placement to the shared titled geometry.
- Added resolved `calc()` top-padding/gap coverage proving normal and absolute
  body children use the same origin.
- Fixed margin/padding cascade precedence so a side longhand overrides a
  shorthand even when the shorthand was previously lowered into a typed
  logical-pixel value.

### Current Verification Baseline

```text
Native layout-filtered suite: 140 passed, 0 failed, 1 ignored
Python layout/visual audit suite: 31 passed
Full native suite: 635 passed, 8 unrelated renderer/text fixture failures,
                   12 ignored
Strict CSS layout smoke suite: 4 passed, 0 failed
```

The eight broader native failures use manually fabricated renderer rectangles
or unrelated chart-label fixtures. They are recorded separately from computed
layout work and must not be hidden by weakening layout assertions.

### Completed Current Slice

- Shared resolved scroll-box geometry now drives viewport, content bounds,
  horizontal/vertical maxima, titled child clipping, and reconciled
  bottom-padding calculations.
- Shared titled-container geometry now drives the layout, scroll, renderer,
  interaction, and text consumers that previously recomputed title/body sums.
- Added a crate-internal `measure_text_for_layout` service backed by Cosmic
  Text advanced shaping, with a bounded 2,048-entry thread-local cache keyed by
  text, font family, weight/style, numeric variant, letter spacing, font size,
  line height, transform result, width, and wrap mode.
- Migrated authoritative one-line intrinsic widths for labels, buttons, menus,
  badges/tags, boolean controls, form controls, loading spinners, menu
  overlays, tooltip natural widths, toast overlays, and generated
  `::before`/`::after` content away from character-count multipliers.
- Made inline badge measurement merge badge-part typography with the widget
  text style, and made toast measurement consume its resolved virtual-element
  text style.
- Added proportional-glyph, Unicode combining-character, font size,
  font-weight/style/spacing, and equal-character-count control regressions.
- Added constrained-width/wrapped shaping to the same measurement service and
  cache, replaced `estimate_wrapped_text_lines` in label height decisions, and
  added a regression proving equal-length narrow and wide glyph sequences
  produce different shaped line counts.
- Removed `apply_flow_layout_intrinsic_height` and the duplicate helpers that
  independently resolved FlowLayout width, child width/min/max constraints,
  default child heights, padding, row/column gaps, and row breaks before
  Taffy.
- FlowLayout wrapped flex rows now directly determine the container's auto
  height. Added a two-width regression with asymmetric child heights,
  independent row/column gaps, and four-sided padding proving the final child
  rectangles plus bottom padding exactly bound the container.
- Extended `TextMeasurement` with the first shaped line's baseline, derived
  from Cosmic Text's `line_y - line_top`, with single-line and wrapped
  regressions ensuring the baseline remains inside the measured box.
- Rebased three old badge/menu tests from deliberately inflated approximation
  thresholds to assertions against shaped glyph width plus the resolved
  padding and safety inset.
- Factored stylesheet `@font-face` loading and family-alias resolution into one
  routine shared by painting and layout measurement. The layout font database
  now synchronizes before the first layout pass of each frame, and its metric
  cache is cleared whenever a newly resolved alias or font source changes the
  available faces.
- Added an idempotence regression proving a local stylesheet font alias resolves
  to the actual Cosmic Text family used by the shared measurement key and is
  not repeatedly reloaded.
- Added optional maximum-line constraints to shared wrapped measurement and its
  cache key. Capped measurements now report the width, height, line count, and
  first baseline of only the visible shaped runs, with focused uncapped-versus-
  two-line coverage.
- Replaced remaining character-count sizing in pie label chips, horizontal bar
  category lanes, and line-plot legend boxes with the shared shaped metrics.
- Split intrinsic leaf sizing into a shrinkable preferred width and a semantic
  minimum based on the control hit target or non-text chrome. Flex rows now use
  the shaped width as a basis and can shrink long controls without allowing
  their labels to force parent overflow; grid auto tracks and compact wrapper
  cases retain their required intrinsic contribution.
- Removed the standalone badge/tag preferred-width cap so flow layouts retain
  the full shaped pill width, while bounded flex parents can still shrink to
  the pill chrome minimum.
- Added a default ellipsis policy for capped single-line controls, titled
  containers, menus, tabs, and navigation items. Labels continue to wrap by
  default, explicit `text-overflow` remains authoritative, and a regression
  covers two capped long buttons shrinking inside a bounded row.
- Completed the Phase 6.1 mutation-pass inventory, including the exact
  post-Taffy execution order, classification, mutation scope, ancestor risk,
  and intended remediation for every normal-flow, overlay, translation, and
  clipping pass.
- Replaced one-shot grid/masonry mutation with a bounded four-round
  reconciliation loop. Nested parent-first grids can now absorb a child's
  changed packed height in a later round; iteration count and convergence are
  exposed in debug layout snapshots, with a two-round nested regression.
- Generalized grid reconciliation across row-oriented ancestors for
  cross-axis `start`, `center`, and `end` alignment. When a packed grid's final
  height changes, the grid and its normal-flow row siblings are repositioned
  from their final heights instead of retaining stale Taffy-era vertical
  offsets.
- Preserved row `stretch` as a constraint-level height floor for auto-height
  grid children during both packing and ancestor resizing. Masonry content can
  still pack within the grid, but the outer grid no longer collapses below its
  row content box or oscillates between packing and stretch reconciliation.
- Preserved Taffy's allocated main-axis height for auto-height grids directly
  owned by panes and vertical splitters. A packed grid no longer opens a gap
  before the following split region or violates the splitter's total height.
- Moved tooltip placement after grid/masonry reconciliation. Tooltips now
  anchor to a target's final packed coordinates, with a regression covering a
  card that changes both row and column during masonry packing.
- Moved tooltip placement after scroll translation and fixed-position rebasing
  as well. A tooltip targeting scrolled content now follows the target's final
  visible coordinates instead of its unscrolled layout position.
- Removed `repack_grid_auto_rows`. Taffy now exclusively owns ordinary grid
  auto-row positions, asymmetric row heights, gaps, and distributed free track
  space; only opt-in masonry enters post-layout reconciliation. A regression
  proves unequal rows remain separated and report zero reconciliation rounds.
- Split the provisional pre-scroll visible-geometry traversal from the final
  clip traversal. Scroll-range calculation still sees ancestor-clipped
  viewports, including partially visible scroll owners, but the provisional
  pass now visits only scroll owners and their clipping ancestor paths, skips
  unrelated branches, never creates paint clips, and leaves no stale paint
  state. A focused regression verifies that only the final pass publishes
  paint/hit-test clips.
- Consolidated dropdown and menu popup geometry across painting, text, and
  runtime hit testing. Dropdowns now flip above their anchor when space
  permits, and dropdown/menu rectangles are position- and size-bounded by the
  root viewport. Partially visible final rows share the same clip in primitive,
  text, and input consumers.
- Made both plain and rich tooltip clamps bound overlay dimensions as well as
  their origin, preventing authored fixed sizes from extending beyond small
  root viewports. Added focused oversized dropdown, menu, and tooltip viewport
  regressions.
- Bounded toast width and height by the usable viewport, including windows
  smaller than the normal toast minimum. Toast stack entries that cannot fit
  without overlapping an earlier entry now resolve to empty geometry, which
  both the primitive and text overlay consumers skip. Added small-viewport and
  exhausted-stack regressions.
- Reworked modal minimum sizing for below-minimum viewports. Modal margins now
  adapt independently per axis, and the nominal 80-pixel minimum is capped by
  actual available root space, so oversized authored modal dimensions cannot
  escape a tiny window.
- Added an alternating small/large/small viewport invariant around menu and
  status chrome plus a scrolling body. It verifies exact root dimensions,
  finite nonnegative rectangles and clips, deterministic geometry when
  returning to the original size, and scroll maxima/offsets resetting to zero
  when the larger viewport removes overflow.
- Made constructor-level fixed sidebar widths shrinkable by default in flex
  rows and capped them at half of a definite parent width when the viewport
  falls below the requested size. The main `Pages` region therefore retains
  usable space instead of collapsing to zero or starting beyond the root.
  Authored CSS width and `flex-shrink: 0` remain explicit opt-outs. Ordinary
  fixed panels retain their hard-size contract so fixed-height scroll panels
  do not collapse; normal-width AppShell sidebar contracts remain unchanged.
- Changed pixel `pane_size` values from non-shrinkable dimensions into
  shrinkable preferred dimensions. Splitters now also compute a per-child
  emergency budget after subtracting their gutters; `pane_min_size` is capped
  by that budget only when the available axis is below the combined minima.
  Horizontal and vertical panes with 240-pixel sizes and minima now contract
  inside a 180-pixel splitter without overlap or viewport escape. Fractional
  allocation and normal-size minima remain unchanged, and an explicit authored
  `flex-shrink: 0` still opts into intentional clipping.
- Made constructor-level `MenuBar` and `StatusBar` heights responsive in
  below-minimum windows. Each shell bar is capped at one quarter of a definite
  parent height and may shrink, preserving a nonzero body slot instead of
  displacing it. CSS height and `flex-shrink: 0` remain explicit overrides;
  normal-height window and AppShell contracts are unchanged.
- Added `flex-wrap` to the complete layout style pipeline: Python inline styles,
  JSON style parsing, DragonGUI CSS parsing/cascade, and native Taffy layout.
  Horizontal `Toolbar` now defaults to wrapping with a minimum height instead
  of a fixed height, so narrow command strips grow to contain additional rows
  rather than overlapping or clipping them. Authored `flex-wrap: nowrap` and
  explicit heights remain available when clipping or horizontal scrolling is
  intentional. The generated manual and CSS capability/reference pages now
  document both the new property and the toolbar default.
- Made the root viewport override authoritative over authored root
  `min-width`/`min-height` and `max-width`/`max-height`. The root Taffy node now
  receives identical preferred, minimum, and maximum physical viewport sizes,
  so an application minimum can no longer enlarge the render surface.
- Added native semantic layout issues to each snapshot diagnostic entry.
  Authored root minima larger than the physical viewport emit
  `below-minimum-viewport` with axis/available/required metadata, while content
  escaping the default bounded root without an intervening scroll owner emits
  `unreachable-root-overflow`. Explicit root scrolling suppresses the latter.
  The strict Python auditor now surfaces the native message directly and avoids
  duplicating the older rectangle-derived root issue. `docs/layout.md` now
  documents both issue codes, their metadata, and the expected remediation.
- Added a generated raw-`VLayout` resize matrix with fixed header/footer chrome
  and an explicitly both-axis scrolling body containing oversized content. It
  covers `96 x 80`, narrow, medium, and fitting large viewports before returning
  to `96 x 80`; asserts exact root/column bounds, ordered non-overlapping chrome,
  finite nonnegative rectangles and clips, owned horizontal and vertical
  overflow, zeroed ranges at the fitting size, and deterministic geometry and
  scroll maps after the repeated resize.
- Added a generated flex invariance matrix covering row/column direction, one
  and three children, zero/nonzero gaps and padding, tiny/normal viewports,
  hidden/automatic overflow, and `1.0`/`1.5` scale factors. Across 256 bounded
  combinations it asserts exact root ownership, finite nonnegative geometry,
  non-overlapping normal-flow siblings, clip containment, nonnegative scroll
  ranges, reachable content ends, and deterministic repeated solves.
- Fixed empty clip intersections for fully scrolled-off or ancestor-clipped
  descendants. Zero-area clips and derived titled-scroll viewports are now
  anchored to the nearest point inside the active owner/ancestor clip instead
  of retaining translated offscreen coordinates.
- Expanded the eight Phase 8 layout/overflow visual probes to a shared
  `390 x 720`, `640 x 480`, `1024 x 768`, and `1440 x 900` viewport matrix while
  retaining each probe's authored preferred size. A Python regression now
  verifies that every named torture target keeps all four standard viewports.
- Added explicit HiDPI visual-audit selection. The runner accepts bounded
  `--scales` values or expands per-target manifest scales, includes scale in
  artifact names and reproduction commands, and passes the requested factor to
  the native process. The eight layout torture probes now cover both `1.0` and
  `1.5`. In audit mode only, native startup creates a proportionally larger
  physical backing surface while retaining the requested logical viewport and
  uses the forced factor for layout, media queries, rendering, and subsequent
  scale-change events. Invalid factors and non-audit attempts are ignored.
- Added thread-safe scripted window resizing through the existing winit
  user-event bridge. `AppHandle.request_window_resize(width, height)` validates
  logical dimensions and lets the event-loop thread own `request_inner_size`;
  audit-mode HiDPI translates the logical request to the corresponding physical
  surface size. Each of the eight torture probes now visits `640 x 480` and
  `1024 x 768`, waits until the live snapshot reaches each requested logical
  size, persists each checkpoint snapshot, and returns to its actual startup
  size before the final snapshot and screenshot.
- Added artifact-level relational validation for every Phase 8 torture snapshot.
  It verifies the layout schema, physical root/window agreement, finite
  nonnegative rectangle/clip maps, clip containment within paint clips,
  nonnegative scroll maxima with valid owners, and counts native semantic issue
  codes. Startup geometry, clip, scroll-offset, and scroll-maximum maps are now
  compared with the final maps after the scripted resize round trip; drift
  fails the visual-audit target. Focused fixtures prove valid geometry passes
  and escaped clips, negative ranges, native issues, and round-trip changes fail.
- Added stable class/type-based relationships for all eight torture targets:
  stressed flex/two-up sibling separation, fixed-panel clip containment,
  masonry/standard grid card separation, root-bounded overlays, composite
  scroll clip and both-axis ownership, plot/table split separation, explicit
  vertical/horizontal/both-axis overflow ranges, and responsive percentage/grid
  containment. The overflow relationship fixture proves missing axis ownership
  fails independently of the generic geometry checks.
- Organized the accumulated native layout regressions into clearly named
  contract sections with shared geometry helpers. This preserves the existing
  single-module test access while making flex, grid, flow, scroll, panel,
  navigation, overlay, measurement, and resize coverage directly discoverable;
  physical file extraction is deferred until it can reduce compile coupling
  instead of adding churn.
- Audited layout-heavy examples against the repaired constructor/native
  defaults. Removed redundant `AppShell` fill/clip styles, `Body` flex/minimum
  workarounds, and zero-minimum overrides from flow, row, text-input, and panel
  examples. Explicit fixed sizes and overflow ownership remain where they
  communicate the probe's intent.
- Expanded the primary, Sphinx, widget-reference, widget-summary, and built-in
  manual layout guidance with preferred/minimum/hard sizing, non-wrapping
  `HLayout` versus `FlowLayout`, scroll ownership/end padding, titled body
  geometry, responsive sidebars/panes, snapshot diagnostics, intentional
  visible overflow, and migration behavior.
- The first current-build visual execution exposed and closed five additional
  edge cases: debug snapshots now publish the distinct paint-clip map; inactive
  overlay scroll entries are pruned from final maps; fully clipped zero-area
  nodes are excluded from active containment assertions; auto-sized framed
  sections retain natural height inside scroll owners so narrow standard grids
  do not collapse their tracks; and definite `auto-fit`/`auto-fill` tracks are
  safely expanded before reaching Taffy 0.5's fractional-`minmax()` panic path.
- Corrected the responsive and overflow torture probes themselves: their root
  columns now own vertical overflow without imposing an oversized root minimum,
  and intentionally hidden descendant overflow is recognized as bounded rather
  than reported as unreachable root content.
- Replaced the remaining pre-layout one-line height shortcut for wrapping
  labels with Taffy's width-aware leaf measurement callback. Auto-sized panels
  now grow from the label's final shaped line count rather than clipping wrapped
  text or allowing following content to overlap it. Per-leaf intrinsic and
  constrained-height caches keep repeated Taffy measurement inexpensive.
- Made constructor-level responsive grid minimums yield when even one authored
  minimum-width track is wider than the available content box. A narrow
  one-column grid now fits its container instead of preserving the oversized
  minimum and pushing panels, controls, and badges through the right edge.
- Added focused native coverage for the narrower-than-minimum grid case and
  visually rechecked the `responsive-layout` and `layout-panel-bounds` probes at
  `390 x 720` and `1.5x`. Wrapped text now reserves its full height, panel tracks
  remain bounded, and the probe's narrow field rows retain readable actions.
- Completed the final verification matrix. `cargo fmt --check` is green; all
  layout-filtered native tests and all Python layout/visual-audit tests pass;
  and all four default strict CSS smoke demos report zero layout issues.
- Reviewed the generated multi-size/HiDPI artifacts. The seven fully automated
  layout targets pass their relational checks at every configured
  viewport/scale pair, while the interactive overlay target has the required
  eight captures and remains intentionally classified for manual interaction.
  Definitive reruns are recorded in the
  [grid report](../artifacts/layout-remediation-grid-final/REPORT.md),
  [composite report](../artifacts/layout-remediation-composites-final/REPORT.md),
  and [final narrow wrap/bounds report](../artifacts/layout-remediation-wrap-final/REPORT.md).
- Ran `cargo clippy --all-targets`. It still reports the repository's existing
  non-layout lint backlog (roughly 190 warnings and four default-deny errors);
  none originates in the remediated layout paths, and the layout completion
  criteria do not hide or weaken those unrelated findings.

### Final Status

- Complete. All implementation, regression, documentation, strict-audit, and
  visual-review work required by this plan is finished.

### Remaining Major Work

- None.

## Scope

### In Scope

- Window-sized root constraints.
- `HLayout`, `VLayout`, `FlowLayout`, `GridLayout`, `ScrollArea`, `Splitter`,
  `Pane`, `Panel`, `Sidebar`, `Modal`, `Tabs`, `Pages`, and `Page`.
- Leaf and composite widget preferred, minimum, and shrink sizes.
- Text measurement used for layout.
- Panel title/body geometry.
- Overflow clipping and scroll range calculation.
- Post-layout grid, navigation, modal, tooltip, scrolling, and fixed-position
  passes.
- Debug snapshots, strict layout audits, native regression tests, and visual
  stress probes.
- Documentation and compatibility guidance for changed defaults.

### Out of Scope

- Full browser CSS layout parity.
- Replacing Taffy with another layout engine.
- General renderer redesign unrelated to measurement or clipping.
- Pixel-perfect compatibility with every accidental legacy overflow behavior.
- Automatically making every `HLayout` wrap. Non-wrapping rows remain useful;
  they must instead shrink, clip, scroll, or report a diagnostic predictably.

## Desired Layout Contract

The implementation should converge on one explicit contract:

1. The window is always a definite, bounded viewport.
2. Layout containers receive a bounded allocation from their parent.
3. Flexible children default to an automatic minimum of zero on the axis where
   they are expected to shrink.
4. Authored width and height are preferred sizes unless the API explicitly
   requests a non-shrinking fixed size.
5. Intrinsic minimums protect controls only when that protection does not make
   the parent impossible to satisfy, or when the author explicitly requests it.
6. Overflow has one owner. Content either fits, wraps, clips, or scrolls in a
   named container; it must not silently escape through unrelated ancestors.
7. Scroll extents are derived from resolved content geometry before visual
   scrolling translations are applied.
8. Text layout and text rendering use the same measurement service and font
   inputs.
9. Custom layout passes must either participate in constraint solving or run a
   deterministic reconciliation pass that updates every affected ancestor and
   sibling.
10. Debug snapshots and strict audits must expose any violation of these rules.

## Implementation Principles

- Repair observability before changing layout semantics.
- Add a focused regression test before or with every behavior change.
- Separate preferred size, minimum size, and fixed size in code and naming.
- Keep logical-pixel to physical-pixel conversion at well-defined boundaries.
- Use resolved layout values for geometry; do not re-read authored values when
  resolved values are available.
- Avoid adding another widget-specific exception when a shared sizing rule can
  solve the problem.
- Make implicit behavior visible in debug snapshots.
- Preserve intentional overflow through explicit `overflow: visible`.
- Preserve intentional fixed sizing through explicit `flex_shrink: 0`,
  fixed pane sizes, or a future fixed-size helper.
- Validate at multiple scale factors and viewport sizes.

## Phase 0: Establish A Reproducible Baseline

### Objectives

- Make the current failures reproducible on a supported toolchain.
- Record current behavior before changing semantics.
- Separate stale assertions from genuine layout defects.

### Tasks

1. Document the supported native test environment:

   - Python 3.12 for the current `abi3-py312` PyO3 configuration.
   - The `PYO3_PYTHON` override needed when the machine has a stale global
     PyO3 interpreter path.
   - The Rust toolchain and Taffy version used by the repository.

2. Run the complete native test suite serially and in its normal parallel mode:

   ```powershell
   $env:PYO3_PYTHON="<python-3.12-path>"
   cargo test --lib
   cargo test --lib -- --test-threads=1
   ```

3. Re-run each failing layout test with its fully qualified test name to verify
   that it is independently reproducible.

4. For every failing assertion, record:

   - widget tree
   - authored styles
   - resolved Taffy styles
   - pre-postprocess rectangles
   - final rectangles
   - clips
   - content bounds
   - scroll viewport and range
   - expected semantic behavior

5. Add a checked-in baseline note under `artifacts/` or a focused test fixture
   file containing the failure inventory. Do not bless incorrect rectangles as
   golden behavior.

6. Ensure test commands do not modify tracked wheel-cache artifacts. If build
   tooling updates `.test-cache`, isolate or ignore those outputs separately
   from source changes.

### Deliverables

- A reproducible list of independently failing tests.
- A supported test command documented for Windows.
- A classification of each failure as implementation defect, stale metric
  expectation, or ambiguous contract requiring a decision.

### Exit Criteria

- Every original failure is either reproducible or explicitly explained.
- No subsequent phase begins with a silently broken test environment.

## Phase 1: Repair Layout Observability And Strict Auditing

This phase must land first. Later layout changes need trustworthy diagnostics.

Status: completed for the current snapshot schema and first strict-audit
invariants. Additional diagnostic categories may be added alongside later
geometry work.

### 1.1 Fix The Snapshot Schema Consumer

The runtime emits:

```text
layout.rects
layout.clips
layout.diagnostics
layout.scroll_max_x
layout.scroll_max_y
```

`tools/smoke_css_demos.py` currently asks `_rect(layout, widget_id)`, which
expects widget IDs directly under `layout`.

Change the auditor to:

1. Resolve `rects = layout.get("rects")`.
2. Resolve `clips = layout.get("clips")`.
3. Resolve `diagnostics = layout.get("diagnostics")`.
4. Fail with an explicit schema error if these maps are absent.
5. Treat a tree node missing from `rects` as meaningful unless it is a known
   inactive/overlay-only node.
6. Add a snapshot schema version to the runtime output so future consumers can
   reject unsupported shapes instead of silently skipping checks.

### 1.2 Expand Container And Overlay Coverage

Add at least these container types to the audit:

- `grid_layout`
- `flow_layout`
- `scroll_area`
- `splitter`
- `pane`
- `collapsible`
- `tree_view`
- `drag_source`
- `drop_target`
- active `tab` and `page` content regions

Classify overlays using layout semantics rather than a short hard-coded list:

- modal
- tooltip
- context menu
- menu popup
- toast
- fixed-position node

### 1.3 Make The Auditor Overflow-Aware

A child rectangle outside a parent is not always an error. The auditor should
use the parent's computed overflow and scroll state.

Report distinct issue types:

- `unowned-overflow`: child escapes a non-scroll, non-visible parent
- `clipped-control`: interactive widget has a smaller visible clip than its
  minimum usable area
- `unreachable-content`: clipped content exists but no owning scroll range can
  reveal it
- `unexpected-scroll-range`: a fitting container reports scrollable overflow
- `missing-scroll-range`: content exceeds a scrolling viewport with zero range
- `text-clipping-risk`: measured text exceeds its content box without wrapping
  or ellipsis
- `negative-or-nonfinite-geometry`
- `sibling-overlap`
- `viewport-escape`

Use `layout.diagnostics` rather than recomputing all overflow from rectangles in
Python.

### 1.4 Add Sibling Overlap Detection

For normal-flow siblings:

1. Ignore explicitly absolute/fixed nodes.
2. Ignore intentional negative margins if the style system supports them.
3. Compare intersections along the parent's layout axis.
4. Permit border/outset paint overlap while rejecting content-box overlap.
5. Include parent ID, child IDs, rectangles, and computed layout mode in the
   diagnostic.

### 1.5 Test The Auditor

Add Python unit fixtures for:

- current nested snapshot schema
- old/invalid schema rejected loudly
- overflowing percentage child
- clipped child with no scroll owner
- valid `overflow: visible`
- valid scrollable overflow
- overlapping row siblings
- grid and flow containers included
- inactive pages/tabs excluded
- fixed overlays excluded from parent-bound checks

### Exit Criteria

- `--strict-layout` fails on a known overflowing fixture.
- It passes valid visible-overflow and scrolling fixtures.
- It checks every normal-flow node with a rectangle.
- A snapshot schema change cannot turn the audit into a no-op.

## Phase 2: Correct Preferred, Minimum, And Fixed Size Semantics

Status: substantially complete for base containers and flexible viewport
widgets. Authored sizes are now classified on the parent's main axis, explicit
overflow owners preserve scroll content, and the non-shrinking escape hatch is
covered. Remaining leaf/control minimum decisions should be completed together
with shared text measurement in Phase 3 rather than by adding approximate
widget-specific limits.

### 2.1 Replace `layout_size_locks_flex`

The current rule treats logical-pixel, percentage, and `calc()` sizes alike and
sets both grow and shrink to zero unless explicitly overridden.

Replace it with axis-specific classification:

```text
Auto
PreferredLength
PreferredPercent
PreferredCalc
ExplicitFixed
FlexBasis
```

Rules:

- `width`, `height`, percentage, and `calc()` set the preferred size.
- A preferred size may still shrink when the parent has insufficient space.
- `flex_shrink: 0` is the explicit way to make a flex item non-shrinking.
- Fixed panes and framework chrome may set non-shrinking behavior internally.
- Width must not influence height-axis shrink semantics or vice versa.
- `flex_grow` and `flex_shrink` decisions must be made per layout axis.

Taffy has one `flex_shrink` value, so the implementation must use the parent's
main axis to decide whether an authored dimension affects flex behavior.

### 2.2 Fix Percentage Child Behavior

Add regressions for:

- `width: 100%` after a fixed-width sibling
- `width: calc(100% - 12px)` after a fixed-width sibling
- percentage child with `min_width: 0`
- percentage child with an explicit nonzero minimum
- percentage child with explicit `flex_shrink: 0`
- nested row inside panel padding
- same cases at 1.0, 1.25, 1.5, and 2.0 scale factors

Expected default:

```text
allocated child width <= remaining parent content width
```

Expected explicit fixed behavior:

```text
child may overflow, but the audit identifies the owning overflow policy
```

### 2.3 Normalize Base Container Minimums

Review defaults for:

- `Window`
- `HLayout`
- `VLayout`
- `GridLayout`
- `FlowLayout`
- `ScrollArea`
- `Splitter`
- `Pane`
- `Panel`
- `Sidebar`
- `Tabs`
- `Pages`
- `Page`

Recommended baseline:

- flexible layout containers: `min_width: 0`, `min_height: 0`
- top-level window body: allowed to shrink between fixed chrome
- `VLayout`: shrinkable when it is a flex item in a bounded column
- `GridLayout`: shrinkable as a flex item; its tracks still enforce explicit
  minimums
- `FlowLayout`: shrinkable and wrapping
- `Panel`: shrinkable unless an explicit fixed width/height or
  `flex_shrink: 0` is supplied
- `Sidebar(width=...)`: preferred width by default; decide whether a separate
  fixed-width option is needed for compatibility

### 2.4 Normalize Leaf And Composite Minimums

Audit every `WidgetKind` and classify it:

- fixed chrome
- intrinsic control
- flexible field
- flexible viewport
- data visualization
- overlay

Particular changes to evaluate:

- Text inputs, text areas, code editors, logs, plots, tables, and HTML reports
  should default to `min_width: 0` when placed in flexible rows/grids.
- Plots and tables must be allowed to shrink to a documented usable minimum.
- Labels may shrink/wrap or ellipsize according to text style.
- Buttons and compact controls may retain a minimum hit area without forcing
  their full label width to become an unbreakable parent constraint.
- Badges and tags should wrap via `FlowLayout` or clip/ellipsis by explicit
  policy instead of forcing unrelated siblings out of bounds.

### Compatibility

Changing fixed panel/sidebar widths to preferred widths can change legacy
layouts. Provide one of:

- explicit `style={"flex_shrink": 0}`
- a documented `fixed=True` constructor option
- a `FixedPane`/fixed-size helper if repeated use warrants it

### Exit Criteria

- The percentage-width regression passes.
- A two-child row never exceeds its parent solely because the second child uses
  `width: 100%`.
- Ordinary flexible containers no longer require repeated `min_width: 0`.
- Explicit non-shrinking layouts retain an intentional escape hatch.

## Phase 3: Unify Intrinsic Measurement With Text Rendering

Status: in progress. One-line intrinsic sizing, constrained wrapped-label
height, optional maximum lines, baseline reporting, badge parts, generated
content, menus/tooltips, toasts, and chart label/legend bounds now use a shared
Cosmic Text shaping service and bounded metrics cache. Runtime-loaded font
sources and aliases synchronize into the layout measurer before layout. Flow's
duplicate height/row prepass has been removed so Taffy's wrapped rows own
auto-height.

### 3.1 Introduce A Shared Measurement Interface

Create a renderer-backed measurement service used by layout and text painting.
It must accept:

- font family and fallback resolution
- font size
- font weight and style
- letter/word spacing if supported
- line height
- text transform
- available wrap width
- maximum lines or single-line mode
- widget part padding and border
- scale factor

It should return:

```rust
struct TextMeasure {
    width: f32,
    height: f32,
    line_count: usize,
    baseline: f32,
}
```

Cache results using a bounded key containing the font identity, text, shaping
inputs, wrap width, and scale factor.

Implemented first slice:

- `TextMeasurement` currently returns logical width, height, line count, and
  first-line baseline for unconstrained or constrained/wrapped shaping.
- A 2,048-entry thread-local cache is keyed with the supported shaping inputs.
- Layout and renderer measurement use the same `shaped_text_measurement`
  implementation.
- Scale is applied after logical measurement, avoiding duplicate cache entries
  for scale factors that preserve logical geometry.
- Maximum lines participate in both shaping and cache identity.
- Layout and painting use the same stylesheet font-source loader and alias
  resolution routine; layout synchronization runs before the first frame pass.

### 3.2 Remove Character-Count Width Estimation From Layout Decisions

Retire `estimate_text_width` as a constraint-producing function. A lightweight
estimate may remain only for non-authoritative fallback diagnostics before the
font system is ready.

Replace the hard-coded intrinsic width formulas with:

```text
measured text width
+ resolved content padding
+ resolved border
+ resolved icon/indicator/checkbox/track width
+ resolved badge width and gap
```

Do not mix theme fallback padding with authored CSS padding after styles have
already resolved.

Constraint-producing `estimate_text_width` and
`estimate_wrapped_text_lines` functions have been removed from layout and
overlays. Badge/tag, toast, generated-content, pie-label, bar-axis, and
line-legend widths now add shaped text metrics to their resolved/style-specific
chrome. Flow's duplicate structural prepass has also been removed.

### 3.3 Define Overflow Policy For Long Text

Status: complete for intrinsic controls and labels. Preferred caps no longer
become text-derived minimum constraints; single-line capped controls ellipsize
by default, labels wrap by default, and explicit CSS overflow choices win.

Remove arbitrary hard maximums such as button 280 px or label 320 px from
minimum-size constraints.

Instead:

- preferred width may be capped for convenience
- minimum width is based on hit target and non-text chrome
- label content follows wrap, clip, or ellipsis policy
- buttons may shrink below full label width and ellipsize when constrained
- menus and navigation items receive a deliberate overflow policy
- badges/tags keep full intrinsic width in flow layouts but cannot force a
  bounded non-flow parent to overflow without diagnostics

### 3.4 Make Height Font-Aware

Control and label height must be derived from:

- measured line height
- vertical padding
- borders
- minimum hit target
- wrapped line count

Add tests proving that larger CSS font sizes increase width and height without
clipping, while explicitly fixed smaller heights produce a deliberate clipping
diagnostic.

### 3.5 Remove Flow's Independent Prelayout Model

Status: complete for FlowLayout height and row packing. Taffy now owns wrapped
row placement and intrinsic auto-height; no separate Flow-specific width,
height, gap, padding, or line-break resolver remains.

Prefer Taffy's measured-leaf callbacks or equivalent intrinsic size inputs so
the same measured size drives both wrapping and final placement.

If a prepass remains necessary:

- it must consume the same shared measurement results
- it must use resolved padding, gaps, min/max dimensions, and parent content
  width
- its predicted line breaks must be asserted against Taffy's final lines in
  debug builds/tests

### Regression Coverage

- proportional fonts and monospace fonts
- uppercase, narrow, and wide glyphs
- Unicode and combining characters
- styled checkbox box width
- styled toggle track width
- button with inline badge
- standalone badge/tag
- menu label
- wrapped label
- explicit ellipsis
- font sizes from 10 to 32 logical pixels
- multiple scale factors

### Exit Criteria

- Flow, checkbox, badge/tag, menu, and font-size tests pass without loosening
  assertions to hide clipping.
- Layout and rendering consume the same text metrics.
- No authoritative layout calculation uses `chars × constant`.

## Phase 4: Rebuild Scroll Geometry Around Resolved Boxes

Status: core implementation complete. Fixed-size propagation, horizontal and
vertical ranges, nested ownership, resolved border/padding boxes, shared
horizontal/vertical `ScrollGeometry`, and reachable resolved end padding are
covered and green. Remaining scroll work should now be handled through this
shared geometry rather than by introducing new authored-style rereads.

### 4.1 Define Scroll Box Terms

Introduce explicit internal geometry:

```rust
struct ResolvedBox {
    border_box: Rect,
    padding_box: Rect,
    content_box: Rect,
}

struct ScrollGeometry {
    viewport: Rect,
    content_bounds: Rect,
    max_x: f32,
    max_y: f32,
}
```

Use one implementation for panels, modals, scroll areas, and explicitly
scrollable general containers.

### 4.2 Compute Bounds Before Applying Scroll Translation

The sequence should be:

1. Resolve base and reconciled layout.
2. Resolve title/body and content boxes.
3. Compute unscrolled descendant content bounds.
4. Stop traversal at nested scroll owners.
5. Include resolved end padding exactly once.
6. Compute and store `max_x` and `max_y`.
7. Clamp requested offsets.
8. Translate scroll content.
9. Recompute visible and paint clips.

Never derive content extents from already scrolled rectangles.

### 4.3 Use Resolved Padding And Gutters

Remove helpers that re-read authored padding with hard-coded defaults when
resolved padding is available.

Scrollbar gutters must:

- affect the viewport/content allocation once
- not be counted again as content padding
- respect custom scrollbar part width
- reserve the bottom-right corner when both axes scroll
- work identically for implicit panel scrolling and explicit overflow

### 4.4 Correct Nested Scrolling

Required behavior:

- Parent clipping does not create a child scroll range.
- A nested scroll owner stops ancestor content-bound traversal at its own border
  or viewport box.
- Scrolling an ancestor does not alter the child's intrinsic maximum range.
- Fixed descendants do not contribute to scroll content bounds.
- Inactive pages/tabs do not contribute to scroll extents.
- Absolute descendants contribute only when their overflow policy says they
  belong to scrollable content.

### 4.5 Correct Horizontal Scrolling

Add explicit tests for:

- fixed-width row children
- long code/text lines
- grid wider than viewport
- table-owned horizontal scrolling
- both-axis scrolling
- right padding reachable at maximum scroll

Verify that constructor `fixed_width`/`fixed_height` props actually reach Taffy
for every widget that exposes or internally uses them. A property that only
suppresses intrinsic measurement without setting the final size is invalid.

### Exit Criteria

- Panel vertical range, horizontal range, and bottom-padding tests pass.
- At maximum scroll, the final child plus end padding is reachable and visible.
- Fitting content reports zero range.
- Nested scroll ranges are stable regardless of ancestor offset.

## Phase 5: Consolidate Panel, Sidebar, Modal, And Title Geometry

Status: core implementation complete. `TitledContainerGeometry` now provides
the resolved title box, header band, body viewport, and body content origin to
layout, clipping/scrolling, absolute positioning, scrollbar/header painting,
close-button hit geometry, and text placement. Remaining title work should
extend this object rather than recomputing padding/line-height/gap sums.

### 5.1 One Titled-Container Model

Today titled containers use synthetic body nodes, title reservation helpers,
absolute offset correction, body viewport correction, and paint insets.
Consolidate these into one resolved titled-container geometry:

```text
border box
  title box
  body viewport
    body padding box
      child content
```

The title height, title/body gap, padding, border, and visual inset must be
computed once and shared by:

- Taffy style construction
- child placement
- absolute-position containing block
- clipping
- scroll viewport
- painting
- hit testing

### 5.2 Eliminate Floating-Point Equality Assumptions

The current titled absolute-child test differs by a fractional pixel. Define
rounding policy:

- retain physical floating-point coordinates through layout
- snap only at renderer-specific boundaries where necessary
- compare geometry with a scale-aware tolerance
- do not independently recompute the same logical sum in multiple modules

### 5.3 Test Matrix

- titled and untitled panel
- empty and non-empty panel
- custom top/right/bottom/left padding
- custom title font size and line height
- custom title/body gap
- fixed height with scroll
- auto height
- absolute child with top/bottom inset
- nested scroll area
- modal title/body scrolling
- 1.0 through 2.0 scale factor

### Exit Criteria

- Title and content never overlap.
- Child clips start at the body viewport, not the title.
- Absolute top positioning uses the documented containing block.
- Painting, hit testing, clipping, and scrolling agree on body geometry.

## Phase 6: Replace Unsafe Post-Layout Mutation With Deterministic Reconciliation

### 6.1 Inventory Every Pass

Status: complete. The current pipeline after the initial Taffy solve is:

| Order | Pass | Classification | Mutates | Ancestor/sibling risk | Remediation |
| --- | --- | --- | --- | --- | --- |
| 1 | `apply_titled_container_absolute_offsets` | visual translation for absolute body children | titled descendants | low; normal-flow children already use the synthetic body node | retain, but keep it tied to `TitledContainerGeometry` |
| 2 | `apply_navigation_layout` (`layout_tabs`, `layout_pages`, `layout_page_region`) | isolated normal-flow reflow | active tab/page subtree; removes inactive geometry | medium; the active region is re-solved after its parent | keep isolated, then assert it never changes the navigation owner's outer rect |
| 3 | `apply_modal_layout` / `layout_modal` | overlay placement plus isolated child layout | open modal subtree | low for normal flow; modal children must exist before grid reconciliation | retain before the bounded grid pass while asserting sibling invariance |
| 4 | `apply_grid_auto_row_positions` | opt-in masonry normal-flow reflow | masonry children, grid height, affected ancestors and following siblings | medium; bounded and propagated through column, row, pane, and splitter ownership | retain only for masonry |
| 4a | `pack_grid_masonry_columns` | normal-flow reflow | masonry child positions and grid height | medium | retained inside the bounded reconciliation loop |
| 4b | `repack_column_children_after_grid`, `realign_row_children_after_grid`, stretch/allocation height floors, and `resize_auto_height_container_to_children` | ancestor reconciliation | column siblings, row cross-axis alignment/stretch, pane/splitter allocation, and auto-height ancestors | low for covered masonry height changes | retain with invariance coverage |
| 5 | `compute_pre_scroll_clips` | provisional visible-geometry calculation for scroll ranges | visible clips for scroll owners and their clipping ancestor paths only; explicitly clears and never emits paint clips | no flow mutation, but must see stable normal geometry | retain because partially visible scroll owners require ancestor-clipped viewport geometry |
| 6 | `apply_scroll_offsets` / `translate_subtree` | visual translation | scroll descendants | intentional; must not feed translated bounds back into range calculation | retain after normal reconciliation |
| 7 | `apply_fixed_positions` / `rebase_fixed_node` | fixed overlay placement | fixed subtree | low if excluded from normal content bounds | retain before dependent tooltip placement |
| 8 | `apply_tooltip_layout` / `layout_overlay_children` | overlay placement plus isolated child layout | active tooltip subtree | low; consumes reconciled, scrolled, and fixed target geometry | retain immediately before final clips |
| 9 | final `compute_clips` | clipping-only | final visible and paint clip maps | none | retain as the sole producer of final paint/hit-test clip state |

The inventory confirms that grid packing is the remaining custom normal-flow
mutation and is now bounded with column, row, pane, and splitter propagation.
Navigation regions are isolated re-solves; modal, tooltip, and fixed passes are
overlay placement; scroll offsets are visual translations; and clip passes do
not mutate rectangles.

Document inputs, outputs, and affected ancestors for:

- titled-container offsets
- tabs and pages
- modal layout
- tooltip layout
- grid auto-row repacking
- masonry packing
- column reconciliation
- scroll offsets
- fixed positioning
- clipping

For every pass, classify it as:

- constraint-producing
- normal-flow reflow
- overlay placement
- visual translation
- clipping-only

### 6.2 Move Constraint-Producing Work Before Final Layout

Where possible:

- express title/body structure directly in the Taffy tree
- express responsive grid tracks before layout
- provide measured intrinsic leaf sizes before layout
- make navigation content regions real bounded layout roots
- make modal body constraints part of its isolated layout computation

### 6.3 Define A Controlled Reconciliation Loop

Status: complete for the remaining opt-in masonry mutation. Masonry packing
runs in a bounded four-round loop, stops when a round produces no geometry
changes, and exposes iteration/convergence state in debug snapshots. A nested
parent-first fixture proves a child height change is incorporated by its parent
on the second round. Column siblings, row start/center/end/stretch behavior,
pane and vertical-splitter allocation, fixed-height owners, active page
subtrees, and scroll owners now retain their appropriate geometry contracts.
Ordinary auto-row grids no longer enter reconciliation at all.

For behavior Taffy cannot express directly, use a bounded loop:

1. Solve base constraints.
2. Apply grid/masonry packing and calculate changed intrinsic container sizes.
3. Propagate changed sizes through all normal-flow ancestors.
4. Re-solve or reflow affected sibling groups.
5. Repeat until stable or a small fixed iteration limit is reached.
6. Emit a diagnostic if geometry does not converge.

Do not limit propagation to column containers. Account for:

- row siblings
- splitters and panes
- grids inside rows
- auto-height and fixed-height ancestors
- pages/tabs
- scroll owners

### 6.4 Separate Overlay Placement

Status: complete for the built-in widget overlay producers. Tooltip placement
now runs after normal-flow
reconciliation, scroll translation, and fixed rebasing, and therefore consumes
final target geometry. Modal child layout still runs before reconciliation so
grids inside open modals participate in the same bounded pass. Modal sibling
invariance is covered. Dropdown and menu popup geometry is now shared by
painting, text, and runtime hit testing; popups and tooltips are bounded by the
root viewport. Toast geometry is likewise shared by painting and text, bounded
to small viewports, and suppresses stack entries that cannot fit without
overlap. Drag/drop and plot annotations remain renderer-owned transient
overlays; they do not participate in widget flow or mutate `LayoutResult`.

After normal flow is stable:

1. Lay out open modals against the root viewport.
2. Lay out tooltips, menus, context menus, toasts, and fixed nodes.
3. Clamp overlays to the viewport.
4. Resolve z-order.
5. Compute overlay clips independently from normal parent flow where required.

Overlay placement must not resize or move normal-flow siblings.

### 6.5 Finalize Scrolling And Clipping Last

Status: complete. The first traversal is now a dedicated
`compute_pre_scroll_clips` pass that calculates only the visible geometry
needed for scroll maxima, visiting only scroll owners and the clipping ancestor
paths that determine their visible viewports. It skips unrelated branches,
does not populate paint clips, and clears any stale paint state before scroll
translation. The final `compute_clips` pass is the sole producer of
paint/hit-test clips after scroll translation, fixed rebasing, and tooltip
placement. The provisional visible traversal remains necessary because
partially obscured scroll containers calculate their ranges from the visible
viewport.

After all unscrolled geometry is stable:

1. Compute scroll ranges.
2. Apply scroll translations.
3. Place fixed descendants relative to the viewport.
4. Compute visible clips and paint clips once.

Remove redundant clip passes when tests prove they are no longer necessary.

### Exit Criteria

- A changed masonry/grid height correctly moves every later sibling.
- A changed grid width cannot overlap a row sibling.
- Resize produces the same final layout as a fresh layout at that size.
- The reconciliation loop is deterministic and converges.

## Phase 7: Window, Resize, And Responsive Behavior

Status: complete. Alternating app-chrome and generated raw-`VLayout` matrices
cover exact root bounds, finite/nonnegative geometry, owned oversized content,
responsive fixed chrome, stale scroll-range reset, and deterministic return to
an earlier viewport size. AppShell, Pages, modal, sidebar, splitter, and toolbar
root shapes are covered separately.

### 7.1 Make Window Constraints Explicit

Status: complete. Exact physical viewport sizing is now asserted across an
alternating small/large/small sequence with fixed menu/status chrome and a
scrolling body. A separate tiny-viewport modal regression proves overlay
minimums cannot enlarge or escape the root. A narrow fixed-sidebar plus active
`Pages` root now shares the available width without overlap or viewport escape.
Authored root minimum and maximum constraints are now explicitly prevented from
overriding the physical viewport. A generated raw-`VLayout` matrix covers
content smaller than the window plus content taller and wider than the window,
with explicit both-axis scroll ownership and repeated-size determinism.

The root window must always resolve to the current physical viewport regardless
of authored child content.

Add tests for:

- content smaller than window
- content taller than window
- content wider than window
- menu and status bars around flexible content
- raw `VLayout` root
- `AppShell`/`Body` root
- pages root
- resize smaller and larger repeatedly

### 7.2 Define Root Overflow Policy

Status: complete. The default
`Window` is a definite bounded viewport whose descendants are clipped at the
root unless an intermediate owner exposes visible overflow. Explicit
`Window(style={"overflow_y": "auto"})` remains supported and has coverage for
oversized content, scroll maxima, and revealing the final child. Native
`unreachable-root-overflow` diagnostics now identify clipped content with no
scroll owner, and the strict audit consumes them without duplicate reports.

Choose and document one default:

- bounded hidden root with explicit `Body`/`ScrollArea` ownership, or
- vertically scrollable window by default

The recommended default is a bounded root with explicit scroll ownership,
provided strict diagnostics clearly identify unreachable content. Preserve
`Window(style={"overflow_y": "auto"})` for intentionally scrollable document
windows.

### 7.3 Responsive Fixed Chrome

Status: complete. Modal chrome now adapts below its nominal minimum and
remains bounded in a `72 x 54` root. Menu/status/body allocation is covered
across alternating heights. Fixed sidebars now yield at below-minimum widths
while preserving the main pages region, and pixel-sized panes now shrink
within narrow splitters; hard pane minima receive the same fair-share fallback
after gutters. Menu/status shell chrome now yields in tiny-height windows while
preserving the body. Horizontal application toolbars now wrap by default and
grow to contain their rows. Authored root minima now produce structured
below-minimum viewport diagnostics while the physical root remains bounded.

For sidebars, toolbars, status bars, and fixed panes:

- define minimum usable size
- define collapse/shrink behavior
- prevent fixed chrome totals from making the main body negative
- expose diagnostics when the viewport is below the application's declared
  minimum

### Exit Criteria

- Repeated resize does not accumulate offsets or stale scroll ranges.
- No normal child receives negative or non-finite dimensions.
- The main body remains bounded between app chrome.
- Below-minimum viewports fail gracefully through shrink, clip, scroll, or a
  clear diagnostic.

## Phase 8: Regression And Visual Torture Suite

### 8.1 Native Unit Tests

**Status: Complete.** The existing private test module is now divided into
clearly named contract sections and uses shared geometry fixtures/assertions.
Keeping the tests colocated currently preserves access to private layout
internals; extraction remains an optional future structural cleanup rather than
a remediation blocker.

Organize layout tests by contract rather than appending all tests to one large
module:

- `layout/flex_tests.rs`
- `layout/grid_tests.rs`
- `layout/flow_tests.rs`
- `layout/scroll_tests.rs`
- `layout/panel_tests.rs`
- `layout/navigation_tests.rs`
- `layout/overlay_tests.rs`
- `layout/measurement_tests.rs`
- `layout/resize_tests.rs`

If splitting the module is too disruptive initially, use clearly named test
sections and shared fixture builders, then extract later.

### 8.2 Property/Invariance Tests

**Status: Complete.** The bounded flex matrix covers direction,
child count, gap, padding, viewport size, hidden/automatic overflow, and scale
factor. It also exercises fixed preferred sizes plus minimum sizes and repeated
same-size solves. The live visual-audit sequence now persists a fresh startup
snapshot, visits its configured resize checkpoints, returns to the startup
size, and requires equality of geometry, clip, and scroll maps.

Generate bounded combinations of:

- row/column direction
- child count
- preferred and minimum sizes
- gaps and padding
- viewport sizes
- overflow modes
- scale factors

Assert invariants:

- finite, nonnegative sizes
- normal-flow siblings do not overlap
- non-visible overflow stays within the owning clip
- scroll maximum is nonnegative
- maximum scroll makes the content end reachable
- a fresh solve equals resize-to-same-size
- layout is deterministic

### 8.3 Visual Probes

**Status: Complete.** All eight named
probes are registered at the four standard viewport sizes below, in addition
to their authored preferred sizes, and run at both `1.0` and `1.5` scale
factors. Each capture also performs scripted in-process `640 x 480` and
`1024 x 768` checkpoints and returns to its startup size. The generated
checkpoint/final snapshots run the Phase 8.4 generic and target-specific
relational assertions. The seven automated targets pass all configured cases.
The interactive overlay target has all eight required captures and remains an
explicit manual-interaction review rather than being misreported as automated.

Keep and strengthen the existing layout probes:

- `layout_flex_stress_probe.py`
- `layout_panel_bounds_probe.py`
- `layout_grid_masonry_probe.py`
- `layout_overlay_collision_probe.py`
- `layout_scrollable_composites_probe.py`
- `layout_plot_embedding_probe.py`
- `overflow_scrollbar_probe.py`
- `responsive_layout_probe.py`

Run each at:

- 390×720
- 640×480
- 1024×768
- 1440×900
- at least one HiDPI scale

Add scripted resize checkpoints where supported rather than checking startup
size only.

### 8.4 Snapshot Assertions

**Status: Complete.** Generic issue
counting, root/window agreement,
finite/nonnegative geometry, clip containment, scroll owner/range validation,
and resize round-trip equality now run for the final and checkpoint artifacts
of all eight torture probes. Target-specific key-widget relationships now cover
each probe using stable tree classes/types. The generated artifacts were
reviewed, and the final targeted reruns closed the grid, composite-scroll,
wrapped-label, and narrow-panel issues found during that review.

For each probe, store or derive:

- issue count by diagnostic type
- rect/clip containment invariants
- scroll ownership and maximum range
- selected key widget relationships

Avoid broad pixel-perfect rectangle snapshots when relational assertions are
more stable.

### Exit Criteria

- Native layout tests are green.
- Strict layout smoke audit is green for all intended-valid probes.
- Intentional failure fixtures are detected.
- Visual inspection confirms no overlapping, cut-off, or unreachable controls
  at supported sizes.

**Status: Satisfied.** The layout-filtered native suite is green, the four
default strict smoke demos are green, invalid audit fixtures are covered by the
Python regression suite, and the final visual matrix plus targeted reruns show
bounded, reachable controls at the supported viewport and scale combinations.

## Phase 9: Python API And Documentation Cleanup

### 9.1 Reduce Defensive Styling Requirements

**Status: Complete.** Layout-heavy probes were audited against current defaults.
Redundant full-window `AppShell`, flexible `Body`, zero-minimum row/flow, input,
and panel overrides were removed. Explicit overflow owners, fixed probe
dimensions, and `flex_shrink: 0` cases remain where they are the behavior under
test.

After native defaults are corrected, audit examples for repeated patterns:

```python
style={
    "width": "100%",
    "min_width": 0,
    "min_height": 0,
    "flex_shrink": 1,
    "overflow_x": "hidden",
}
```

Remove overrides only when framework defaults now provide the same semantics.
Keep explicit overflow ownership in application code where it communicates
intent.

### 9.2 Update Documentation

**Status: Complete.** The primary layout guide, Sphinx guide, widget summary,
widget reference, CSS cross-reference, and built-in manual now describe the
repaired sizing, wrapping, scrolling, titled-container, responsive, diagnostic,
and visible-overflow contracts.

Update:

- `docs/layout.md`
- `docs/widgets.md`
- `docs/widgets-reference.md`
- `docs/css-styling.md`
- `docs/sphinx/layout.md`
- built-in manual layout and clipping sections

Document:

- preferred versus fixed sizes
- when controls shrink
- automatic minimum-size behavior
- `HLayout` versus `FlowLayout`
- scroll ownership
- panel title/body geometry
- responsive sidebar/pane behavior
- interpreting snapshot diagnostics
- intentional `overflow: visible`

### 9.3 Migration Notes

**Status: Complete.** `docs/layout.md` records the changed percentage,
constructor-size, text-overflow, scroll-padding, and strict-audit behavior plus
the explicit opt-outs.

Call out behavior changes:

- percentage widths now shrink in constrained rows
- fixed panel/sidebar width may become a preferred size
- long control text may ellipsize rather than force parent overflow
- scroll ranges include resolved end padding consistently
- strict audits may expose previously hidden invalid layouts

### Exit Criteria

- New users can build a bounded sidebar/body application from the primary
  layout documentation without manual min-size folklore.
- Existing examples no longer carry unnecessary layout workarounds.
- Intentional fixed and overflow behavior remains easy to request.

## Detailed Failure-To-Work Mapping

| Current Failure Area | Primary Phase | Required Proof |
| --- | --- | --- |
| Percentage child exceeds row | Phase 2 | Child fits remaining content width |
| Flow auto-width controls | Phase 3 | Measured widths and Taffy wrapping agree |
| Checkbox flow widths | Phase 3 | Box, padding, and text all fit |
| Badge/tag pill size | Phase 3 | Layout and paint use identical part metrics |
| Menu intrinsic width | Phase 3 | Label glyphs remain within content clip |
| CSS font-size sizing | Phase 3 | Larger font increases resolved box correctly |
| Panel scroll range missing | Phase 4 | Overflow produces stable positive range |
| Horizontal scroll range missing | Phase 4 | Fixed/wide content produces positive range |
| Bottom padding lost at max scroll | Phase 4 | Final child ends before resolved bottom padding |
| Titled absolute child offset | Phase 5 | One shared body-origin calculation |
| Auditor misses violations | Phase 1 | Strict audit fails known bad fixture |

## Suggested Change Sequence

Use small, reviewable changes in this order:

1. Snapshot schema version and auditor fix.
2. Auditor regression fixtures and expanded container coverage.
3. Percentage/calc preferred-size flex correction.
4. Base container min-size and shrink normalization.
5. Shared resolved-box helpers.
6. Scroll geometry rewrite.
7. Titled-container geometry consolidation.
8. Renderer-backed text measurement service.
9. Intrinsic leaf and flow migration to shared measurement.
10. Post-layout pass reconciliation redesign.
11. Window/resize tests and responsive defaults.
12. Full visual torture suite.
13. Example cleanup, documentation, and migration notes.

Do not combine all semantic changes into one patch. Each step should leave the
test suite at least as green as the previous step and should include a narrow
before/after reproduction.

## Verification Commands

Use the supported Python 3.12 interpreter for PyO3:

```powershell
$env:PYO3_PYTHON="<python-3.12-path>"
cargo test --manifest-path native/Cargo.toml --lib
```

Run Python tests with an environment containing the project test dependencies:

```powershell
python -m pytest
```

Run strict smoke checks:

```powershell
python tools/smoke_css_demos.py --strict-layout
```

Run the layout visual audit set at narrow and desktop sizes:

```powershell
python tools/visual_audit.py --category layout --sizes mobile,1024x768,1440x900
```

Also run:

```powershell
cargo fmt --manifest-path native/Cargo.toml -- --check
cargo clippy --manifest-path native/Cargo.toml --all-targets
```

## Completion Criteria

This remediation is complete only when all of the following are true:

- The strict auditor consumes the current snapshot schema and cannot silently
  skip all rectangles.
- All native layout tests pass on the supported toolchain.
- Percentage and `calc()` children shrink predictably in constrained flex rows.
- Base containers have consistent minimum-size and shrink defaults.
- Layout and rendering use the same text measurement results.
- Flow wrapping agrees with final child placement.
- Scroll extents include resolved content and end padding exactly once.
- Nested scroll owners have stable independent ranges.
- Titled panels, sidebars, and modals share one title/body geometry model.
- Grid/masonry changes propagate through affected ancestors and siblings.
- Repeated resize gives the same result as a fresh layout.
- No valid visual torture probe reports sibling overlap, unreachable clipped
  content, accidental viewport escape, or non-finite geometry.
- Documentation describes the new sizing and overflow contract.
- Intentional fixed sizing and visible overflow remain explicitly available.

## Definition Of Done For Each Patch

Every implementation patch associated with this plan must include:

1. A focused failing test or probe demonstrating the previous behavior.
2. The smallest coherent framework-level fix.
3. Native and/or Python regression coverage.
4. Snapshot or visual verification when the change affects clipping or paint.
5. A note about compatibility impact.
6. No demo-specific width or padding workaround presented as the framework fix.
