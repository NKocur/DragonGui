# V3 CSS And Scatter Follow-Up Plan

The V2 CSS plans are no longer a clean implementation checklist. They now mix
completed work, current behavior notes, stale gaps, and future browser-parity
ideas. This V3 plan is the current source of truth for CSS and Scatter3D styling
follow-up work.

Historical inputs:

- `plans/V2/css-styling-system.md`
- `plans/V2/css-styling-system-part-2-widget-parts.md`
- `plans/V2/css-styling-system-part-3-web-capabilities.md`
- `plans/V2/scatter3d-completion-plan.md`
- `plans/V2/scatter3d-dragonsci-parity-plan.md`

## Current Truth

### Base CSS System

The base DragonGUI CSS system is implemented enough to treat as production
surface, not unfinished roadmap.

Implemented:

- Lightning CSS parsing lowered into DragonGUI-owned CSS IR.
- `StylesheetOrigin`, specificity, source order, and `!important`.
- Framework, theme, user, and inline precedence.
- Type, class, id, key, attribute, direct-child, descendant, and selector-chain
  matching.
- Pseudo-states for hover, active, focus, disabled, checked, open, expanded,
  collapsed, and selected where runtime state exists.
- `:not(...)`, `:is(...)`, `:where(...)`, structural child selectors, universal
  selectors, and first-slice `:has(...)`.
- `:root` custom properties, variable fallbacks, embedded `var(...)` in many
  parseable values, and selector-local variables inside a rule block.
- Text inheritance.
- `App.stylesheet(...)`, `App.load_stylesheet(...)`, and
  `App.clear_stylesheets(...)`, including live commands.
- Built-in `framework.dg.css`.
- Debug snapshots with stylesheet and computed style information.

Known caveat:

- The V2 license gate is only partially closed. `THIRD_PARTY_NOTICES.md` is
  packaged, but it is currently a policy/checklist file, not generated
  dependency notices.

### Widget Parts

The widget-parts plan is implemented enough to treat as current supported CSS
surface.

Implemented:

- `NodePartStyles`, `PartStyle`, and per-corner radii.
- CSS pseudo-element-style part selectors such as `NumberInput::stepper`.
- Inline `style={"parts": {...}}` with dashed/snake-case part normalization.
- Part validation at cascade time.
- Generated `::before` and `::after` content as renderer-owned virtual parts.
- Renderer support for NumberInput, Dropdown, Checkbox, Slider, ProgressBar,
  Tabs, NavItem, DataFrameTable, Panel accent, Modal scrim, LED parts, badges,
  and scrollbars.
- Focused demo coverage in `examples/css_widget_parts_demo.py`.

Remaining widget-part work is not core CSS infrastructure. It belongs to
individual widgets when a stable user-facing part is needed.

Deferred table parts remain:

- `DataFrameTable::surface`
- `DataFrameTable::header-cell`
- `DataFrameTable::row-alt`
- `DataFrameTable::cell`

### CSS Part 3 Web Capabilities

Most first-slice web UI capabilities have landed.

Implemented:

- Typography: `text-transform`, `letter-spacing`, `line-height`, `font-style`,
  `font-variant-numeric`, and single-line ellipsis.
- Practical color syntax: `transparent`, named colors, RGB/HSL/HWB,
  Lab/LCH/Oklab/Oklch, and supported `color(...)` spaces.
- Box shadows, outlines, uniform border styles, gradient backgrounds, layered
  gradient paint, `background-image` gradient paint, `background-noise`,
  `blob-gradient(...)`, and `mesh-gradient(...)`.
- Paint-only transitions and transforms.
- First-slice visual `@keyframes` animations.
- Percent, auto, and compatible `calc()` for sizing and spacing.
- Taffy-backed CSS grid including named areas and several track functions.
- Overflow, scroll containers, overlay scrollbar indicators, and scrollbar
  parts.
- First-slice `position: relative`, `absolute`, `fixed`, and local sibling
  `z-index`.
- First-slice backdrop-filter frosted-surface treatment.
- First-slice `@media`, `@supports`, and local/data `@font-face`.
- First-slice named `@container` width queries with explicit
  `container-type: inline-size`.

Current Part 3 gaps:

- Broader container query parity: height/block-size/aspect-ratio/orientation,
  style queries, scroll-state queries, nested `@container`, and container query
  units.
- Broader/custom media query parity.
- Platform-backed media detection for pointer/hover, color gamut, forced
  colors, contrast, inverted colors, dynamic range, reduced motion, reduced
  transparency, reduced data, and display mode.
- Full inherited custom property cascade semantics.
- Remote font URLs, WOFF2 loading, and packaged font asset resolution.
- True sampled-framebuffer backdrop blur.
- Full browser stacking contexts.
- Layout-aware generated content.
- Multiple simultaneous animations, animation composition, timelines, and
  layout/text animation.
- `background-image: url(...)`.
- Mixed percent/pixel `calc()` in fully auto-sized parent axes.
- CSS grid subgrid and broader auto-repeat parity.
- Dynamic stateful and widget-part arguments in broader `:has(...)` forms.

### Scatter3D Styling And CSS

Scatter3D has advanced well beyond the original V2 completion checklist, but
its CSS integration did not follow all of the proposed scatter-specific
properties.

Implemented:

- Multiple scatter widgets.
- Packed startup and live point payloads.
- Point colors, scalars, colormaps, per-point sizes, opacity, log scale,
  `clim`, `nan_color`, and size ranges.
- Per-widget camera commands and parallel projection.
- Point style support: `circle`, `square`, `gaussian`.
- CSS `scatter-point-size`.
- CSS `scatter-point-style`.
- CSS `scatter-grid-visible`.
- CSS `scatter-grid-planes`.
- CSS `scatter-legend-position`.
- CSS `scatter-orientation-axes`.
- Rounded scatter viewport clipping and picking-region clipping.
- Grid, ticks, axes, grid planes, axis labels, axis visibility, background
  control, legend, scalar bar, orientation axes, labels, line/box overlays,
  actors, streams, selection/hover/LOD, mesh/statistical overlays, screenshot,
  flat views, and camera linking are substantially implemented through Python
  API and native renderer paths.

Not implemented from the V2 scatter CSS integration section:

- `scatter-axis-x-label`
- `scatter-axis-y-label`
- `scatter-axis-z-label`
- `scatter-axis-x-visible`
- `scatter-axis-y-visible`
- `scatter-axis-z-visible`

The current docs intentionally limit Scatter3D CSS to static presentation
defaults:

- `scatter-point-size`
- `scatter-point-style`
- `scatter-grid-visible`
- `scatter-grid-planes`
- `scatter-legend-position`
- `scatter-orientation-axes`

Open Scatter3D parity risks from the latest V2 audit:

- Orthographic grid scaling.
- Verify line overlay parity against DragonSci edge cases; basic
  add/update line overlays plus generic overlay removal and visibility now
  exist in Python and native code.
- Verify mesh overlay depth/sorting correctness; renderer paths exist for
  wireframe, opaque, and transparent meshes, but this still needs a visual
  parity audit.
- Legend/scalar-bar stacking.
- Hover tooltip documentation.
- Verify `point_style` startup, property setter, live command, and CSS reapply
  stay consistent.

## Product Direction

V3 should avoid treating CSS as a browser-compatibility project. The goal is a
predictable native styling layer for DragonGUI apps and data tools.

Prioritize features that:

- Make existing widgets easier to style without Python-specific props.
- Improve demos and probes visibly.
- Reduce documentation/code drift.
- Have bounded renderer/runtime risk.
- Fit DragonGUI's retained native widget tree.

Deprioritize features that:

- Require full DOM or browser layout semantics.
- Need major render graph changes before there is a product use case.
- Add CSS API surface for every renderer implementation detail.

Keep these V2 exclusions explicit unless a future plan makes a narrower product
case:

- Arbitrary browser pseudo-element compatibility.
- User-defined renderer parts.
- Arbitrary nested widgets generated by CSS parts.
- CSS-created hit-test regions.
- A full browser cascade/layout compatibility promise.

## Workstreams

### V3-CSS-0: Reconcile Docs, Plans, And Packaging

Goal: make the repository tell one consistent story before adding more CSS
surface.

Tasks:

- [x] Mark V2 CSS plans as historical or superseded by this V3 plan.
- [x] Update docs where they still imply a planned feature is currently
  complete.
- [x] Update docs where code supports more than the docs claim.
- [x] Add a short "Current CSS Limits" section to the relevant docs.
- [x] Include V3 plan files in sdist packaging.
- [x] Make third-party notice policy explicit in README and packaging.
- [ ] Replace `THIRD_PARTY_NOTICES.md` policy text with generated dependency
  notices before publishing a release artifact.

Acceptance:

- `docs/css-styling.md` and `docs/css-capabilities-reference.md` agree with
  current implementation.
- Unsupported browser/CSS features are listed intentionally, not accidentally.
- Packaging policy for V3 plans and third-party notices is explicit.

### V3-CSS-1: Scatter3D CSS Property Decision

Goal: decide whether scatter chrome should be styleable through CSS or remain
Python/API driven.

Status: option 3 selected for the first V3 implementation slice.

Decision options:

1. Keep only `scatter-point-size` and `scatter-point-style` as CSS.
2. Add static scatter chrome CSS for grid, axes, legend, scalar, and
   orientation options.
3. Add only a smaller subset that reads naturally as styling, not behavior.

Recommended first pass:

- Keep runtime and data-bearing scatter controls as Python API.
- Add CSS only for static presentation defaults:
  - `scatter-grid-visible`
  - `scatter-grid-planes`
  - `scatter-legend-position`
  - `scatter-orientation-axes`
- Do not add axis label text through CSS unless there is a clear app theming
  use case. Axis labels are semantic plot content and usually belong in Python.

Acceptance:

- The decision is documented in CSS docs and widget docs.
- Unsupported scatter CSS properties warn clearly.
- Existing Python methods keep precedence for live user interaction.

### V3-CSS-2: Scatter3D CSS Implementation

Goal: if V3-CSS-1 chooses to add scatter chrome CSS, implement a bounded
presentation subset.

Status: first slice implemented for grid visibility, grid planes, legend
position, and orientation axes. Axis label text, scalar bars, data, colormaps,
and other semantic plot content remain Python/API driven.

Proposed properties:

```css
Scatter3D {
    scatter-grid-visible: true;
    scatter-grid-planes: major;
    scatter-legend-position: top-right;
    scatter-orientation-axes: true;
}
```

Implementation notes:

- Add fields to `WidgetStyle` only for static presentation defaults.
- Parse values in `native/src/css_style.rs`.
- Apply computed style to scatter runtime setup and live stylesheet reapply.
- Define precedence:
  - Live Python commands win for the current runtime session.
  - Startup Python constructor props win over framework defaults.
  - User CSS can set defaults for widgets without explicit Python props.
- Avoid mutating semantic plot content such as axis label strings through CSS in
  the first pass.

Acceptance:

- CSS can turn grid visibility and orientation axes on/off at startup.
- CSS can choose grid plane mode.
- CSS can choose legend position when a legend is visible.
- Live stylesheet changes update visual chrome without replacing point data.
- `examples/css_feature_probes/scatter3d_css_chrome_probe.py` has a CSS chrome
  case.
- `docs/css-capabilities-reference.md` lists the supported subset.

### V3-CSS-3: Container Queries Spike

Goal: determine whether container queries can fit DragonGUI's style/layout
architecture without feedback loops.

Status: first slice implemented. DragonGUI now supports named width/inline-size
queries against explicitly opted-in ancestor containers using previous layout
rects, plus one bounded startup settle pass.

Questions:

- Which containers expose stable dimensions to style matching?
- Can style resolution depend on previous layout without oscillation?
- Should container queries only apply after an initial layout pass?
- Should DragonGUI support a narrower query model such as named layout scopes?

Recommended spike:

- Prototype read-only query matching against previous-frame layout rects.
- Limit first slice to width thresholds.
- Require explicit opt-in with a container name or container-type field if
  needed to keep matching deterministic.

Acceptance:

- [x] A design note states whether V3 will implement container queries, defer
  them, or replace them with a DragonGUI-specific responsive layout primitive.
- [x] If implemented, the first slice has a probe and deterministic
  invalidation rules.

Implemented first-slice contract:

- Containers must opt in with `container-type: inline-size`.
- `container-name` is optional; named `@container` rules match the nearest
  eligible ancestor with that name.
- Supported conditions are `width` and `inline-size` length comparisons,
  including min/max/range/interval forms.
- Matching uses previous layout widths. Startup gets one extra style/layout pass
  when container rules exist so static documents settle without user resize.
- Nested `@container`, style queries, scroll-state queries, height/block-size,
  aspect-ratio/orientation, and container query units are deferred.
- Visual probe: `examples/css_feature_probes/container_queries_probe.py`.

### V3-CSS-4: Font Asset Story

Goal: make `@font-face` useful in packaged apps without pretending remote web
font behavior exists.

Tasks:

- Decide supported packaged font reference forms.
- Add package-relative or resource-relative font URL resolution.
- Decide whether WOFF2 is worth adding.
- Keep remote URL loading disabled unless DragonGUI has a broader network asset
  policy.

Acceptance:

- A packaged app can include a local font and reference it from CSS.
- Missing or unsupported fonts report diagnostics once.
- Docs clearly distinguish local, packaged, data URL, WOFF1, WOFF2, and remote
  behavior.

### V3-CSS-5: Platform Media Detection

Goal: turn first-slice media query assumptions into real platform values where
practical.

Candidate platform-backed values:

- `prefers-color-scheme`
- `prefers-reduced-motion`
- `prefers-reduced-transparency`
- pointer and hover capability
- display scale/resolution
- color gamut
- forced colors and contrast where available

Acceptance:

- Runtime media environment documents which fields are real and which are
  defaults.
- CSS reapplies when a platform value can change during runtime.
- Probes show at least color scheme and reduced motion behavior.

### V3-CSS-6: Advanced Render Features

Goal: handle larger render architecture features only when the use case is
clear.

Backlog:

- True sampled-framebuffer backdrop blur.
- Full browser stacking contexts.
- Layout-aware generated content.
- Multiple simultaneous animations and animation composition.
- URL image backgrounds.
- Text rotation and non-uniform transform support.

Acceptance:

- Each feature gets its own focused plan before implementation starts.
- No feature in this group blocks normal app styling.

## Testing And Verification

Rust:

```powershell
cargo test --manifest-path native/Cargo.toml css_style --lib
cargo test --manifest-path native/Cargo.toml --lib
```

Python:

```powershell
python -m pytest tests/ -v
```

CSS smoke:

```powershell
python tools\smoke_css_demos.py --strict-layout
```

Environment requirement:

- Use Python 3.11+ for PyO3 abi3-py311 and Python tests.
- On Windows, ensure the Python development library used by PyO3 is available;
  otherwise Rust tests may compile but fail to link on `python3.lib`.

Focused probes to keep current:

- `examples/css_widget_parts_demo.py`
- `examples/css_web_capabilities_demo.py`
- `examples/css_feature_probes/selectors_probe.py`
- `examples/css_feature_probes/supports_probe.py`
- `examples/css_feature_probes/media_probe.py`
- `examples/css_feature_probes/font_face_probe.py`
- `examples/css_feature_probes/responsive_layout_probe.py`
- `examples/css_feature_probes/overflow_scrollbar_probe.py`
- `examples/css_feature_probes/positioning_zindex_probe.py`
- `examples/css_feature_probes/backdrop_filter_probe.py`
- `examples/css_feature_probes/generated_content_probe.py`
- `examples/css_feature_probes/scatter3d_probe.py`
- `examples/css_feature_probes/widget_metrics_probe.py`

## Completion Criteria

This V3 follow-up is complete when:

- V2 CSS plans are clearly historical and no longer treated as active
  checklists.
- Current CSS docs match code and probes.
- Scatter3D CSS support is either intentionally limited or extended with a
  documented static chrome subset.
- Packaging and third-party notice policy is closed.
- The selected V3 follow-up workstream has tests/probes and no stale plan claims.
