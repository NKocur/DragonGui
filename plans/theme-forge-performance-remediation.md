# THEME FORGE Performance Remediation

**Started:** 2026-07-30  
**Status:** In progress — live animation and rapid theme replacement optimized

## Scope

Use `examples/theme_forge_stress_demo.py` as a library-level stress workload.
The demo should remain intentionally dense; optimizations must improve
DragonGUI's retained runtime rather than remove test coverage or reduce the
normal live workload.

This investigation also covers client-titlebar interaction starvation. The
retained drag gesture itself is still present, but pointer events cannot start
the native window drag promptly when the UI thread spends most of its time
processing live commands.

## Repeatable profiler

`tools/profile_theme_forge_stress.py` records:

- Python construction and serialization time
- Retained widget and visible layout counts
- Frame and command-drain timings
- Dirty request/execution counts
- Per-command timing
- CSS cascade statistics
- Framework layout, text, primitive, image, and plot timings
- Queue depth and Python scheduler pressure

Representative commands:

```powershell
py -3.12 tools/profile_theme_forge_stress.py --frames 30 --no-live `
  --output artifacts/performance/theme-forge-idle-baseline.json

py -3.12 tools/profile_theme_forge_stress.py --frames 90 `
  --output artifacts/performance/theme-forge-live-baseline.json

py -3.12 tools/profile_theme_forge_stress.py --frames 90 --no-live `
  --theme-cycles 3 `
  --output artifacts/performance/theme-forge-theme-cycle-final.json
```

## Baseline findings

The default workload contains:

- 2,016 retained widgets
- 189 currently laid-out rects
- 373 user CSS rules plus 161 framework rules
- 51,029 selector candidates in a complete cascade
- Approximately 428 KB of compact startup document JSON

Idle rendering is not the problem. The idle profile reaches 58.2 wall FPS
under FIFO presentation, with median steady frame work below 0.8 ms.

The live baseline exposed the dominant bottleneck:

- Four animated panels issue `SetStyle({"width": ...})` updates.
- 308 `SetStyle` commands averaged 33.63 ms each.
- The commands spent 10,359 ms reapplying the complete 2,016-node cascade.
- Command application consumed 10,587 ms of a 16,754 ms run.
- Effective wall FPS fell to 6.94.
- The Python task queue ended with 31 tasks still waiting.
- Command draining yielded for fairness 89 times.

Each command already deferred its layout rebuild, but
`apply_set_style_patch()` still reapplied the entire stylesheet cascade
immediately. Four width changes in one batch therefore paid for four global
cascades before the runtime performed the single merged layout.

This UI-thread monopolization also explains the apparently broken client
titlebar drag: the retained press/move gesture and DPI-scaled threshold tests
remain intact, but native cursor events have little opportunity to run during
125 ms median command drains.

## Fix 1 — batch inline-style cascade work

Completed:

- Inline style commands now mutate and parse their target immediately but mark
  the global cascade dirty instead of reapplying it per command.
- The merged dirty rebuild at the command-batch boundary performs at most one
  required cascade.
- Non-layout visual/text patches still force that deferred cascade before
  rebuilding, preserving computed-style correctness.

Result:

- `SetStyle` command average: 33.63 ms → 0.019 ms.
- Command application total: 10,587 ms → 69 ms.
- Wall FPS: 6.94 → 61.20.
- End-of-run queued tasks: 31 → 1.
- Layout diagnostics remained at zero.

## Fix 2 — direct dimension patch fast path

Completed:

- Additive `width`, `height`, and min/max dimension patches can be merged
  directly into the target's already-computed style.
- Null/removal patches and mixed cosmetic/inherited patches continue through
  the deferred global cascade so underlying stylesheet values and inheritance
  remain correct.
- Added native classification coverage for direct dimension patches, removals,
  and mixed width/color patches.

Result:

- Median `style_reapply` during live layout batches: 32.09 ms → 0.042 ms.
- Command-drain total average: 13.28 ms → 3.89 ms.
- Command-drain p95: 45.16 ms → 11.14 ms.
- Command fairness yields: 7 → 4 in the equal-frame follow-up profile.
- Final queue depth remained zero and wall FPS remained at the FIFO limit
  (60.15).
- Layout diagnostics remained at zero.

The remaining full cascade samples are startup and structural replacement
work, not the animated dimension patches.

## Fix 3 — batch theme and stylesheet replacement cascades

The new `--theme-cycles` profile runs three complete passes through all seven
themes after startup. Its baseline exposed a second command-boundary problem:

- 44 `SetStylesheet` commands each reparsed a sheet and immediately cascaded.
- 22 `SetTheme` commands each rebuilt framework defaults and immediately
  cascaded.
- 202 property updates repeatedly wrote the same theme-button classes and
  status labels.
- Command application consumed 3,057.15 ms.
- Total command-drain time was 3,364.96 ms.
- Ten full rebuilds were executed during the burst.

Completed:

- Stylesheet installation/removal and theme changes now mark styles dirty and
  participate in the existing deferred rebuild boundary.
- A command batch therefore performs at most one final cascade instead of one
  cascade per mutation.
- Full cascades remain synchronous for isolated commands, so a normal single
  theme change still has immediate, deterministic rendering behavior.

Intermediate result:

- Command application: 3,057.15 ms → 512.47 ms.
- `SetStylesheet` average: 46.52 ms → 7.14 ms.
- `SetTheme` average: 45.62 ms → 8.68 ms.

The remaining command time was parsing every superseded intermediate sheet and
rebuilding framework defaults for every superseded theme.

## Fix 4 — queue-level last-write-wins coalescing

Completed:

- Pending `SetTheme` commands retain only the latest theme.
- Pending named stylesheet set/remove/clear mutations retain only the final
  state for each origin and sheet ID.
- Pending `SetProp` commands retain only the latest value for each widget and
  property.
- The runtime batch coalescer applies the same rules as a second safety layer.
- Scatter/line append commands and structural operations are unaffected, so
  time-series samples and tree mutations are not lost.
- Added tests for replacement order, clear semantics, unrelated property
  preservation, and final-value selection.

Three-pass rapid-cycle result:

- Command application: 3,057.15 ms → 30.67 ms (**99.0% lower**).
- Total command-drain time: 3,364.96 ms → 109.68 ms (**96.7% lower**).
- Applied `SetTheme` commands: 22 → 1.
- Applied `SetStylesheet` commands: 44 → 2.
- Applied `SetProp` commands: 202 → 13.
- Executed full rebuilds: 10 → 2.
- Fairness yields: 9 → 1.
- Frame-work p95: 28.35 ms → 1.06 ms.
- Layout diagnostics remained at zero and FIFO wall FPS remained approximately
  60.

The ordinary live workload also remains at 60.02 wall FPS with zero layout
diagnostics after queue coalescing.

## Full-cascade phase breakdown

The final theme still requires one complete 2,016-node cascade. A representative
replacement visits 56,028 candidate rules and matches 23,113 declarations.
After the traversal optimization below, the measured components are:

- Declaration application envelope: approximately 8.2 ms
- Inheritance: approximately 1.2 ms
- Style merges: approximately 3.5–3.6 ms
- Snapshot construction: approximately 3.0 ms
- Native fallback handling: approximately 2.8 ms
- Selector matching: approximately 2.8 ms
- Provenance recording: approximately 1.1–1.2 ms
- Property application itself: approximately 2.0 ms

## Fix 5 — remove unused selector contexts and share inherited provenance

The cascade previously created an `AncestorSnapshot` for every node whenever
the active rule set contained an ancestor or container selector. Leaf snapshots
were immediately popped and could never be observed by a descendant.

It also deep-cloned the same inherited provenance candidate independently for
each sibling. A first shared implementation used a small hash map per parent;
although it reduced the measured inheritance phase, allocation overhead erased
the end-to-end gain. That intermediate design was replaced with a fixed
11-property array and reprofiled.

Completed:

- Leaf widgets no longer create unused ancestor snapshots.
- Ancestor snapshots per cascade: 2,016 → 605.
- Parent text styles remain borrowed while descendants are cascaded.
- Each parent builds inherited provenance candidates once in a fixed array;
  siblings share those immutable records through `Arc`.
- No hash-map allocation or property-name hashing is required for inherited
  lookups.

Result on a repeated three-pass theme-cycle profile:

- Final full cascade: 32.70 ms → 28.11 ms (**14.0% lower**).
- Median full cascade: 29.77 ms → 25.25 ms (**15.2% lower**).
- Inheritance phase: 4.20 ms → 1.16 ms (**72.4% lower**).
- Total rapid-cycle command drain: 109.68 ms → 94.79 ms.
- Live workload: 60.01 wall FPS with zero layout diagnostics.
- Live command-drain p95: 9.54 ms.

## Hidden-page lazy cascade assessment

Inactive `Page` descendants are already removed from layout, which explains the
large difference between 2,016 retained widgets and 189 active layout rects.
They are not yet safe to omit from the CSS cascade:

- Switching a `Pages` value expects the newly active subtree to have current
  computed styles immediately.
- Descendant selectors need the current ancestor chain.
- Container queries need current layout-derived container context.
- Sibling selectors and `:has()` can make a hidden descendant affect styles
  outside its own subtree.
- Debug computed-style and provenance queries currently cover the retained
  tree, not only visible layout nodes.

A correct lazy implementation therefore needs:

1. A stylesheet generation number on computed subtrees.
2. Per-subtree stale/clean tracking.
3. Synchronous recascade when a stale page becomes active.
4. Reconstruction of ancestor, media, and container-query context at the lazy
   subtree root.
5. Selector dependency analysis that disables subtree skipping when sibling,
   `:has()`, or other cross-subtree selectors are active.
6. Debug tooling that either materializes stale styles on demand or explicitly
   reports their generation.

This assessment rules out a simple visibility check in the cascade traversal;
that shortcut would make theme switching appear faster while introducing stale
styles and selector correctness bugs. The next hidden-page prototype should be
generation-based and guarded by selector dependency features.

## Incidental Tabs/scroll visual defect

A scrolled visual-audit capture exposed both a library defect and weak demo
styling in the main twelve-workspace tab strip.

Before the fix:

- `Tabs#forge-tabs` owned a 27 px layout box.
- Its CSS-authored `::header` and the generated Tab child rects were 34 px.
- The following `Body`/`Pages` region began at the 27 px Tabs boundary.
- Tab children therefore extended seven pixels into the scrolling content
  region, allowing the accent/border and scrolled panels to paint through the
  same band.
- The custom Tabs layout ignored CSS horizontal padding and column gap.

Library changes:

- A standalone empty-content Tabs strip now derives its intrinsic owner height
  from the computed `::header` height.
- Tab child rects are bounded by the resolved Tabs content box.
- Tabs now respect resolved horizontal padding and CSS `gap`/`column-gap`.
- Added a regression asserting owner/header containment, non-overlap with the
  following Body, padding, and inter-tab gap.

Demo changes:

- Increased the strip to 38 px and gave it a distinct secondary surface.
- Added 8 px horizontal insets and a 3 px inter-tab gap.
- Replaced the flat transparent treatment with rounded-top tabs, visible hover
  and selected surfaces, clearer text weight, and selected-badge contrast.

Fixed capture geometry:

- Tabs: `y=116`, `h=38`, bottom `154`
- First Tab: `y=117`, `h=36`, bottom `153`
- Pages/scroll viewport: `y=154`

The tab chrome and scrolled content no longer overlap. The targeted scrolled
visual audit passes with zero layout diagnostics.

## Gallery and retained-window follow-up

A focused audit of four issues reported from the Gallery separated demo
authoring defects from reusable library defects.

### Retained client titlebar dragging

Classification: **library runtime defect**.

The first audit found that DragonGUI delayed `Window::drag_window()` until a
later `CursorMoved` event so it could apply its own two-pixel threshold.
Winit's platform contract requires the call immediately after the left-button
press, so the runtime was corrected to let the platform own that threshold.

A manual retest showed dragging was still unreachable and exposed the deeper
blocker: the press path asked the generic interactive-widget hit test for the
titlebar. That hit test intentionally returns buttons, inputs, and other
interactive widgets only. `WindowTitlebar` is an `HLayout` and `WindowTitle` is
a `Label`, so neither could ever be returned and `drag_window()` was never
called.

The runtime now uses a dedicated retained-chrome geometry hit test. It accepts
the visible title label and unused titlebar background, explicitly excludes
the minimize/maximize/close rectangles, and is shared by left-drag and
right-click system-menu handling. The OS drag begins immediately from the
recognized press. A regression proves title and background hits, control
exclusion, outside-titlebar exclusion, and documents why the generic hit test
is not suitable for chrome.

### Window-control glyphs

Classification: **demo defect in the mini preview, plus a library-quality
opportunity in retained chrome**.

The Gallery's mini window was manually authored with `SmallButton("_")`,
`SmallButton("[]")`, and `SmallButton("X")`; it was not rendering a special
mini-chrome library component. Those controls now use native vector
`IconButton` marks.

The actual retained client titlebar was upgraded at the same time from
font-dependent text glyphs to semantic vector IconButtons:

- minimize: `minus`;
- maximize: `stop`;
- restore: `copy`;
- close: `close`.

The controls preserve their stable semantic CSS types, part forwarding,
accessible names, tooltips, focus order, and fixed-width geometry.

The retained restore mark now has its own titlebar-specific geometry instead
of reusing the general-purpose Copy icon unchanged. Its two overlapping window
outlines are approximately 18% smaller, use rounded corners, and remain
centered in the unchanged 46 by 34 logical-pixel hit target. The public Copy
icon is intentionally unchanged. A primitive regression checks specialization,
rounded radii, reduced bounds, and centering.

### Plain tooltip wrapping and clipping

Classification: **library overlay sizing defect**.

Plain tooltip text was already sent through the wrapped-text renderer, but its
overlay rectangle was measured with the default theme font, default line
height, and fallback padding. Rendering then used the CSS-authored
`Tooltip.static` font and padding. A larger themed font or padding could
therefore require more lines than the surface reserved and be clipped.

Tooltip geometry now resolves the same virtual `Tooltip.static` computed style
used by paint and text rendering. Width and height calculations use the
authored font metrics, line height, and horizontal/vertical padding, then clamp
the wrapped overlay to the window. A native regression uses a large-font,
large-padding tooltip at the upper-right window edge and verifies both the
increased wrapped height and viewport containment.

A subsequent manual retest exposed a second sizing defect in the same path.
Height was still estimated as `ceil(single-line width / content width)` and
hard-capped at four lines. Real word wrapping does not distribute words evenly,
so the shaped renderer could produce more lines than the estimate while the
surface stopped growing at line four.

The surface now measures the text with the exact shaped, word-wrapped layout
routine used by rendering. There is no four-line cap. The text clip reserves
the authored bottom padding, and a two-physical-pixel rounding allowance keeps
fractional glyph bounds and final-line descenders away from integer scissor
edges. A renderer-level regression shapes more than four lines with a custom
font size, line height, and asymmetric padding, then proves the final line is
inside both the text clip and tooltip surface. THEME FORGE's rail toggle now
uses a deliberately long plain tooltip for ongoing manual stress testing.

### Gallery SearchBox overlap

Classification: **library composite sizing defect exposed by the demo**.

`SearchBox` deliberately defaults to `flex-shrink: 1` so it can narrow inside
a horizontal toolbar. CSS flex shrink acts on the parent's main axis, however,
so the same default also allowed SearchBox height to collapse inside a
constrained vertical Panel. Its icon, input, and clear-button children retained
their control heights and painted across the collapsed host and the following
SearchBox.

Before the fix:

- clearable SearchBox host: `y=1174`, `h=2`;
- preset SearchBox host: `y=1186`, `h=2`;
- their 26–27 px children overlapped.

`SearchBox` now has a 38 px widget-default `min-height`. Author CSS can still
override that default for compact themes, while horizontal width shrinking is
unchanged. The demo gives both Gallery SearchBoxes stable IDs for regression
inspection.

After the rebuilt-library visual audit:

- `gallery-search-clearable`: `y=1174`, `h=38`, bottom `1212`;
- `gallery-search-preset`: `y=1222`, `h=38`, bottom `1260`;
- inter-control gap: `10 px`.

The focused 1024x768 Gallery visual audit passes.

### Tooltip ancestor clipping

Classification: **library overlay clip-ownership defect**.

Rich `Tooltip` widgets remain retained beneath the Panel containing their
target, while tooltip layout promotes their surface and child geometry into
window coordinates. The final clip pass previously kept using the retained
Panel ancestry. When a tooltip crossed a Panel boundary—especially one with
`overflow: hidden`—the surface could appear outside the Panel while its text
and other children were clipped at the Panel edge.

The layout clip pass now recognizes `Tooltip` as a viewport-promoted overlay.
Its paint clip starts at the window viewport, and its descendants inherit the
tooltip surface clip instead of the target Panel clip. A regression places a
rich tooltip inside an overflow-hidden Panel, deliberately positions it across
the Panel boundary, and verifies that both the surface and child text retain
non-empty clips beyond that boundary.

### Grid card auto-height and bottom padding

Classification: **library grid reconciliation defect exposed by the demo**.

The Gallery's Text entry Panel did have authored bottom padding. The apparent
missing padding occurred because its Grid row allocated the Panel only 455 px
while its SearchBoxes and CodeEditor continued normal-flow layout to 645 px.
The final control therefore extended beyond the Panel instead of contributing
to its auto height.

Ordinary auto-row grids now detect auto-height framed children whose normal
content exceeds the allocated item height. Those children grow to their exact
content bottom plus resolved bottom padding, subsequent grid rows are moved
down by the expanded row height, and the Grid and ancestor column geometry are
reconciled. Explicit row templates, explicit row placement, fixed-height
containers, and column-flow grids remain unchanged.

The focused THEME FORGE snapshot now reports:

- Text entry Panel bottom: `939 px`;
- CodeEditor bottom: `925 px`;
- retained authored bottom inset: `14 px`.

### Extreme-grid replacement targets and bordered auto-height

- [x] Separated the rapid-replacement targets from the intentionally
      destructive `.xt-conflict` shorthand fixture. The targets now retain a
      stable centered box model while the hostile sheet changes their paint
      and asymmetric borders.
- [x] Gave the extreme grid, all six replacement targets, and the malformed
      sheets panel stable IDs for layout regression inspection.
- [x] Corrected native post-layout auto-height reconciliation to include the
      resolved bottom border as well as bottom padding. This applies to normal
      auto-height containers, ordinary grids, and masonry grids; omitting the
      border made thick hostile boxes a few pixels too short and allowed the
      following panel to overlap their content.
- [x] Added a native regression that forces the reconciliation path and checks
      that a 15 px bottom padding plus 9 px bottom border are both retained.
- [x] Registered an extreme-page visual audit state that enables the hostile
      sheet and scrolls to the replacement/malformed-sheet boundary.
- [x] Rebuilt the release extension and completed the hostile extreme-page
      snapshot/geometry verification. All six labels are contained and centered
      within 0.5 px, the malformed-sheets panel begins 10 px below the grid,
      and the snapshot reports zero layout diagnostics.
- [x] Increased the visual-audit debug-snapshot allowance for very large stress
      documents so serializing thousands of computed-style records is not
      mistaken for a deadlocked UI.

### Active-theme semantic swatch row

Classification: **demo-authored flex sizing defect**.

- [x] Removed `width: 100%` from the five sibling semantic-color labels. That
      declaration asked every item in one horizontal row to consume the whole
      row and produced the large accent block followed by clipped label text.
- [x] Made every chip an equal `flex-grow: 1`, `flex-shrink: 1`, zero-basis
      item with `min-width: 0` and automatic width.
- [x] Changed the semantic color surfaces from Labels to lightweight Panels
      with centered child labels. The previous Label backgrounds were present
      in computed style but were not painted, making the palette preview look
      like unexplained text floating in an empty subpanel.
- [x] Let each nested label use the full swatch content width, avoiding the
      small-bold-text intrinsic-width edge case that clipped final letters.
- [x] Added stable IDs for the swatch panel, row, and five labels.
- [x] Registered multi-viewport, multi-scale visual checks that require equal
      chip widths, row containment, and no adjacent overlap.

### Panel-parts fixture containment

Classification: **demo-authored host spacing and paint-clash defect**.

- [x] Removed the outer `card flush` opt-out so the nested Panel-parts probe
      retains the same card inset as every neighboring fixture.
- [x] Removed the unrelated bright host border beneath `Panel::accent`; the
      six-pixel pink accent now owns and follows the rounded left edge without
      green corner slivers.
- [x] Reduced the inner radius and shortened the probe title while preserving
      the deliberately clashing purple `::header`, green `::title`, navy
      `::body`, and pink `::accent` coverage.
- [x] Completed multi-viewport visual and geometry validation at 1x and 1.5x.
      The nested probe retains 15 px left/right card insets, its ScrollArea is
      fully contained, and both snapshots report zero layout diagnostics.

### High-DPI final grid-row containment

Classification: **library grid reconciliation defect**.

- [x] Reproduced the Parts-page overlap at 1.5x: the final splitter card ended
      at physical y=3324 while the following unsupported-parts card began at
      y=3094, a 230 px overlap. At 1x the authored 10 px gap was correct.
- [x] Confirmed the row children were correctly scaled and positioned, but the
      grid rectangle ended 245 px before its final row. No fixed or negative
      demo offsets contributed to the defect.
- [x] Extended ordinary auto-row reconciliation to run when intact rows exceed
      stale grid bounds, not only when content first overflows an individual
      auto-height child.
- [x] Added a native 1.5x regression that deliberately gives a grid stale
      bounds and verifies reconciliation repacks and contains the final row.
- [x] Added stable splitter/unsupported-card IDs and a permanent visual-audit
      assertion requiring at least 8 px separation.
- [x] Rebuilt the release extension and verified the real Parts page at 1x and
      1.5x. The splitter/unsupported gap is 10 physical px at 1x and 15
      physical px at 1.5x, the grid ends exactly with its last card, and both
      snapshots report zero layout diagnostics.

### DropZone label alignment and flex CSS

Classification: **library flex-property plumbing defect**.

`DropZone` has always declared `align_items: center` and
`justify_content: center`, but the native inline-style parser discarded
`justify_content`. The label was centered horizontally but remained near the
top of the 96 px drop surface. The same properties were documented as
stylesheet features even though `align-items`, `align-self`, and
`justify-content` were incomplete in the CSS lowering path.

DragonGUI now carries all three properties through inline/default styles,
stylesheet parsing, cascade merging, computed-style snapshots, layout
invalidation, and Taffy layout. `justify-content` supports start, center, end,
space-between, space-around, and space-evenly. The focused snapshot places the
DropZone center at `2491 px` and its label center at `2491.5 px`.

The follow-up visual check exposed a second, independent edge case: the
centered label's intrinsic width was an exact fit for the shaped text plus its
two text insets. Integer scissor bounds and fractional glyph rasterization
could therefore trim the final character even though the label and DropZone
rectangles were correct. Plain labels now reserve a narrow 2 logical-pixel
rasterization allowance beyond the measured text advance. This is scoped to
`Label`; it does not change `Tab` or `NavItem` sizing. The DropZone regression
now verifies both center alignment and usable text width.

### Built-in help synchronization

Classification: **documentation drift found after the library changes**.

`dg.help` was checked against the live public export list, exported class
members, widget CSS capability registry, example/probe paths, and the current
layout/style implementation. All 176 public exports and all registered CSS
parts remain represented. The audit corrected stale links for demos moved into
`examples/older/`, replaced an obsolete drag/drop recipe with constructors that
serialize successfully, and expanded the runtime help for:

- client-decorated window dragging, vector controls, and shrinking titles;
- static and rich tooltip sizing, wrapping, viewport promotion, and clipping;
- standalone/styled Tabs header ownership;
- Label wrapping and intrinsic text sizing;
- DropZone centering and drag-kind acceptance;
- flex direction/basis/wrapping, `align-items`, `align-self`,
  `justify-content`, grid placement, positioning, and CSS warning behavior;
- ordinary auto-row grid growth and scrolled plot/text clipping.

Help-specific regression assertions now protect the recently added guidance.

A corrective reverse audit then derived the finite CSS property set directly
from `DgStylePropertyName::from_css_name` and compared it with the rendered
manual. This found 63 accepted longhands that were described only by family and
could not be found by exact name. `dg.help.reference.css_properties()` now
publishes a machine-readable and rendered inventory of all 130 native
properties, grouped into layout, visual, text/generated-content, motion, and
widget-specific declarations. A source-derived regression requires this set to
remain exactly synchronized with the native parser.

The same pass corrected two behavioral prose remnants: Terminal assets are
packaged locally rather than CDN-hosted, and the PyTorch dashboard recipe now
uses the current `LogView.append_line(...)` method instead of the removed
`LogView.append(...)` name.

A third, enum-level audit derived pseudo-states, structural selectors, selector
functions, attribute operators, common keyword-value enums, framework CSS
variables, icon identities, and icon aliases from their implementation
sources. It corrected `font-style: oblique` (not accepted), added the accepted
`font-variant-numeric: tabular_nums` compatibility spelling, published the
complete selector forms and 18 `--dg-*` variables, and made icon/alias catalogs
machine-readable.

The same audit removed duplicate machine-readable symbol leaves under
`reference.drag_drop`. That focused section now aliases the canonical widget
and dataclass nodes, so all 176 public symbols have exactly one canonical path
while attribute navigation through `reference.drag_drop.*` remains available.
Source-derived regressions now protect these finite sets and reject stale
`Class.method(...)` or unknown `dg.*` names in hand-authored prose.

A fourth, query-and-constructor audit compared the manual with the remaining
finite capability sets exposed by the native parser and public widget classes.
It added the exact 39 media-feature names, supported media/container query
features, `@supports` functions and operators, supported at-rules, local font
formats/technologies, gradient interpolation choices, and transition/animation
keyword values. These sets are rendered for people and retained as structured
metadata for tooling.

Public constructor choice parameters are now derived from the live widget
class constants and shown under each applicable reference page. The catalog
covers 13 parameters on 10 widgets, including window decorations, toolbar and
radio orientation, scroll axes, sidebar modes, selection modes, legend
positions, toggle label positions, arrow direction, and flex direction and
alignment. This pass also found a real Python/native mismatch:
`FlexLayout(align_items="baseline")` was accepted by Python but discarded by
the native layout parser. The unsupported choice is now rejected immediately
with `ValueError`, keeping the constructor and stylesheet behavior aligned.

The final audit expanded validation from symbol/member existence to executable
documentation call shapes. All 36 fenced Python examples compile, and all
`dg.*(...)` calls in fenced examples plus concrete inline constructor recipes
are bound against the current public signatures. This found stale examples
using `CommandPalette(close_button=...)`, `Panel(children=...)`,
`DataFrameTable(selectable=...)`, a positional `Sidebar` title, positional
keyword-only scatter columns, and ambiguous tooltip/drop/grid shorthand. The
examples now use current constructor forms, including detached panels built
with a context manager inside components. All 286 help nodes render with their
expected heading, and all 176 canonical public symbols round-trip through
`dg.help.find_symbol(...)`.

A continued semantic audit then exercised claims that remain invisible to
signature binding. It corrected the callback overview: selectable lists, tabs,
and pages use `on_change`, while `on_select` belongs to tables, trees, and
breadcrumbs; `NavItem` routes through `Pages` and has no callback parameter.
It also corrected the PropertyGrid recipe from unsupported schema key
`choices` to `options`, a failure found only by constructing the documented
composite headlessly.

The same pass expanded finite public-value discovery from class-local widget
constants to module validators and live methods. Structured help now covers 43
constructor/function parameters across 27 public symbols (214 rendered values)
and 19 method parameters across 8 symbols (113 rendered values). Added areas
include badge levels, log variants, toast levels/positions, queue states,
node-graph runtime policies, scatter streaming modes, flow/splitter/image
options, chart aggregation and interaction modes, all colormaps, PaintContext
alignment/fit options, and Scatter3D legend, label-anchor, point-style, plane,
stream, and scalar-bar choices. Literal catalogs are behavior-tested by
constructing the API or invoking the live/drawing method with every documented
value. Help example style keys and finite keyword values are also checked
against the native-derived CSS inventory.

The NodeGraph follow-up audit now labels the entire subsystem **not ready for
usage**. The root manual index, LLM build rules, a dedicated
`dg.help.node_graph()` page, all 21 canonical `NodeGraph*`/template reference
leaves, search summaries, and structured metadata state that the APIs are
experimental, incomplete, unsupported for application/production use, and not
recommended for generated code. The warning covers graph models, templates,
runtime sessions, bindings, persistence, schemas, and compatibility callbacks;
public export status is explicitly not a readiness guarantee.

The dedicated page records the current version-1 event contract only for
internal development. Its mutation-event metadata is derived from
`_GRAPH_MUTATION_EVENTS`, and regressions verify that `on_graph_event` receives
all dispatched mappings while `on_node_select` and `on_node_move` remain the
two compatibility callbacks accepted through `**callbacks`. The page warns
that event names/payloads and runtime behavior may change without migration
support and that durable graph data must not be shipped yet.

The dialog/live-update audit documented behavior that signatures alone do not
make obvious. File dialogs now state that omitting `on_select` is synchronous
and returns path data or `None`, while callback mode returns `None` immediately
and runs on a worker; passing `app` is required to marshal the callback through
`call_soon_threadsafe` before touching live widgets. `alert` and `confirm` are
documented as immediate `Modal` constructors, and `confirm` explicitly does not
return a boolean.

The same audit found stale LinePlot streaming guidance. The documented
`append_points("series", xs, ys)` form passed three positional arguments even
though `series` is keyword-only, and performance guidance recommended
nonexistent `set_series` and prepared-payload replacement methods. Examples now
use `append_points(xs, ys, series="name")`; replacement uses `set_data(...)`.
The common live-method reference is generated from an implementation-checked
inventory of public owners/methods. Python help examples now infer common
receiver types (`plot`, `scatter`, `table`, and similar handles) and bind their
method calls against live signatures, extending the prior constructor-only
call-shape audit.

The component/state lifecycle audit now documents the rules enforced by the
runtime rather than describing keyed state only in general terms. Top-level
component calls return `ComponentInstance`; nested calls render immediately
and require an explicit non-empty key; state keys must be unique per render;
defaults initialize only missing slots; setters cannot run during render; and
failed mounted updates roll state back. Help also distinguishes ordinary
component roots from `App.run(...)` roots, which must render a `Window`, and
records when keyed child state is retained or pruned. Behavioral regressions
exercise the missing-key, duplicate-state, render-time-set, and root-type
failures.

The validation and application-lifecycle follow-up corrected several semantic
drifts. Finite-number guidance now names only controls that actually enforce
it instead of incorrectly including Slider and ProgressBar. File dialogs are
correctly described as usable without a running App, with synchronous and
callback/thread behavior separated. Detached/removed-widget behavior and
drag/drop JSON/kind validation are explicit. The App overview now covers
component roots, blocking `run(...)` return behavior, `run_with_loading`, icon
themes, retained buffers, which methods require a live app, and the fact that
`document(...)` accepts a Window rather than a ComponentInstance. The Window
signature now includes `decorations`, and generated widget reference pages say
that live methods are used while the event loop is active—not after the
blocking `run(...)` call has returned and detached native handles.

## Validation status

- [x] Deterministic idle profile captured.
- [x] Deterministic live baseline captured.
- [x] Batched style-cascade profile captured.
- [x] Direct dimension fast-path profile captured.
- [x] Three-pass rapid-theme-cycle baseline and final profiles captured.
- [x] Leaf-ancestor and fixed-array inheritance profiles captured separately.
- [x] Focused native style-patch tests pass.
- [x] Theme/stylesheet/property queue-coalescing tests pass.
- [x] Live profiles finish with zero layout diagnostics.
- [x] Complete native suite passes: 889 passed, 12 ignored.
- [x] Targeted Python demo, visual-audit, and client-titlebar tests pass:
      37 passed.
- [x] Targeted scrolled THEME FORGE tab capture passes with non-overlapping
      Tabs, Tab, Pages, and ScrollArea geometry.
- [x] Retained client controls serialize and render as vector IconButtons.
- [x] Styled static-tooltip geometry regression passes.
- [x] Gallery SearchBoxes retain 38 px host height and a 10 px gap.
- [x] Release native extension rebuilt and copied into the source package.
- [x] Complete native suite passes after exact wrapped-tooltip sizing:
      891 passed, 12 ignored.
- [x] Rich-tooltip surface and child text escape an overflow-hidden ancestor
      without escaping the window viewport.
- [x] Complete native suite passes after tooltip clip promotion:
      892 passed, 12 ignored.
- [x] Auto-height grid card retains a 14 px bottom inset after its multiline
      CodeEditor and repacks subsequent rows without overlap.
- [x] DropZone main-axis centering and inline/CSS flex-alignment regressions
      pass.
- [x] Focused THEME FORGE Text entry and Drag and drop geometry capture passes.
- [x] Complete native suite passes after grid and flex-alignment fixes:
      896 passed, 12 ignored.
- [x] DropZone label retains shaped-text padding plus a 2 logical-pixel
      rasterization allowance; complete native suite remains at 896 passed,
      12 ignored.
- [x] `dg.help` covers 176/176 public exports, every exported class member, and
      every registered widget CSS part; all real example/probe metadata paths
      resolve.
- [x] `dg.help` exact CSS inventory matches all 130 finite properties accepted
      by the native parser, with no missing or extra names.
- [x] Corrective prose audit finds no unknown `dg.*` symbols, stale moved-demo
      paths, missing Theme fields, or invalid current method references.
- [x] Native pseudo-state, structural-selector, selector-function, attribute
      operator, common keyword-value, and 18-variable theme sets match `dg.help`
      metadata exactly.
- [x] Built-in icon identities and compatibility aliases are rendered and
      machine-readable directly from the live public catalogs.
- [x] All 176 public symbols have one canonical machine-readable help node;
      drag/drop convenience paths resolve as aliases.
- [x] Updated `dg.help` drag/drop recipe constructs and serializes successfully.
- [x] All 39 native media features and finite media/container/supports
      capabilities match structured `dg.help` metadata.
- [x] All documented transition, animation, gradient-interpolation, and common
      CSS keyword choices match their native parser functions.
- [x] Choice metadata for 13 public constructor parameters matches the live
      constants and rendered signatures; unsupported flex baseline alignment
      is rejected at construction.
- [x] Complete Python API suite passes after the fourth help audit:
      443 passed.
- [x] All 36 fenced Python help examples compile and all concrete public call
      shapes bind against current signatures.
- [x] All 286 help nodes render successfully; all 176 canonical public symbols
      round-trip through `dg.help.find_symbol(...)`.
- [x] Complete Python API suite passes after the final help audit:
      444 passed.
- [x] Callback-family claims match current `on_change`/`on_select` signatures;
      the PropertyGrid schema example constructs headlessly with `options`.
- [x] Structured help covers 43 finite public constructor/function parameters
      and 19 finite live/drawing method parameters; literal values are accepted
      by their implementations.
- [x] Help example style keys and finite CSS keyword values match the
      implementation-derived property/value catalogs.
- [x] Complete Python API suite passes after the continued semantic audit:
      450 passed.
- [x] Root, LLM, topical, search, reference, and structured help surfaces mark
      all 21 canonical NodeGraph-related APIs not ready for usage.
- [x] NodeGraph mutation-event metadata matches the implementation and current
      graph/compatibility callback dispatch claims are regression-tested.
- [x] Complete Python API suite passes after the NodeGraph readiness audit:
      452 passed.
- [x] File-dialog synchronous/asynchronous return and callback-thread rules,
      plus alert/confirm Modal semantics, match implementation-backed tests.
- [x] Live-method inventory matches public owners; all LinePlot streaming
      examples use the keyword-only `series` argument and no stale
      `set_series`/prepared setter is recommended.
- [x] Complete Python API suite passes after the dialog/live-method audit:
      454 passed.
- [x] Component identity, state initialization/rerender constraints, keyed
      child lifetime, and component-root Window requirements are documented
      and implementation-tested.
- [x] Validation guidance matches enforced finite/range, detached-widget,
      drag/drop, and dialog lifecycle behavior.
- [x] App/Window help covers component run roots, blocking event-loop return,
      loading builders, icon/image/buffer resources, live-only methods, and
      client/native decorations.
- [x] Complete Python API suite passes after the component, validation, and App
      lifecycle audit: 456 passed.
- [x] Rebuilt release extension copied into the source package; release DLL and
      installed PYD SHA-256 hashes match.
- [x] Focused Python/API/demo suite passes; broader selected suite reaches
      455 passed with one unrelated `dg.help` path failure for demos moved to
      `examples/older/`.
- [x] Performance profiler passes Python 3.12 compilation.
- [x] User manually confirms titlebar dragging under the rebuilt library.
- [ ] Full THEME FORGE visual route and long-title matrices pass after the
      runtime change.

## Next bottlenecks

1. **Single full-cascade latency**

   Superseded theme operations no longer cascade, but the final deliberate
   theme or stylesheet replacement still costs approximately 25–28 ms because
   all 2,016 nodes are recascaded. The next step is to reduce the cost of that
   required final cascade rather than further optimize burst handling.

2. **Unnecessary hidden-page cascade work**

   Only 189 rects are active, while all 2,016 retained nodes participate in a
   complete cascade. Determine whether page subtrees can retain cached computed
   styles until their stylesheet dependencies or route visibility changes.

3. **Structural slow-tick updates**

   Table replacement and bar-chart subtree replacement remain full/structural
   operations. They are infrequent, but should be profiled independently after
   the CSS path is stable.

4. **Client-titlebar manual outcome**

   Recheck click-hold-move on the title and empty titlebar area. If dragging
   still fails with the event loop below the new command-drain budget, add
   platform outcome logging around Winit's `drag_window()` result and isolate
   the Windows gesture separately from performance starvation.
