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
