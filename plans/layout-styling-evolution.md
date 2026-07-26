# DragonGUI Layout and Styling Evolution Plan

## Purpose

This plan turns the findings from the Aurora command-center demo into a
framework-level improvement program. The goal is to make DragonGUI layouts:

- predictable from the public Python API;
- responsive without application-specific workarounds;
- styleable using the widget names users actually instantiate;
- safe around intrinsic sizing, shrinking, clipping, and scrolling;
- diagnosable when geometry is technically valid but practically unusable.

This work builds on the completed
[Layout System Remediation](./layout-system-remediation.md). That plan repaired
the layout engine's core geometry, text measurement, scroll ownership, and
post-layout reconciliation. This plan addresses the higher-level contracts
between Python widgets, CSS, responsive composition, diagnostics, and visual
quality.

## Status

Last updated: 2026-07-25.

**Plan status: Complete — Phases 0-11 completed and release-hardened.**

### Current Implementation Progress

- Completed the Phase 0 reproduction suite:
  - `semantic_css_identity_probe.py`
  - `cascade_origin_probe.py`
  - `sidebar_flex_allocation_probe.py`
  - `responsive_grid_orphan_probe.py`
- Registered all four probes in the visual-audit manifest with compact,
  desktop, scale, and resize coverage appropriate to each case.
- Captured the original baseline in
  `artifacts/layout-styling-phase0-baseline/` and the phone allocation baseline
  in `artifacts/layout-styling-phase0-mobile/`.
- Added a manifest-contract test covering the four new audit targets.
- Added the first structural `starved-subtree` check to the strict smoke
  auditor. A visible child container with content now fails when it receives
  zero space on an `HLayout` or `VLayout` parent's main axis.
- Added stable public `css_types` metadata to Python widget and VDOM
  serialization. Composite nodes now retain chains such as
  `SearchBox`, `HLayout`, `Container`, `Widget`.
- Added declarative Python identity control through `CSS_TYPE` and
  `CSS_TYPE_ALIASES`. Public subclasses can publish stable names and aliases,
  set `CSS_TYPE = None` to suppress their own name, and private helper classes
  remain excluded.
- Extended the native widget schema and debug tree with the serialized
  `css_types` chain while retaining both the native `type` and an explicit
  `render_kind`.
- Generalized native type-selector parsing and matching to use serialized
  semantic metadata instead of a hardcoded composite map. Public types,
  public base types, classes, IDs, pseudo states, and existing native kinds
  now use the same selector path and specificity rules.
- Added user-selector match counts and unmatched-selector lists to the native
  debug snapshot. Media-inactive rules are excluded; container-query rules
  remain deferred until diagnostics receive post-layout container context.
- Integrated unmatched selectors with automation. Smoke runs display them and
  support opt-in failure through `--strict-css`; visual-audit JSON and Markdown
  reports retain the selector list, and the semantic identity probe enables
  strict CSS validation.
- Added structured user-selector diagnostics with origin, source index,
  one-based line, column, source order, specificity, and match count.
- Started Phase 2 by introducing a distinct `default_style` payload in Python,
  VDOM, native nodes, and debug-tree snapshots. The existing `style` payload
  now contains author inline declarations only.
- Inserted widget defaults into native resolution after framework rules and
  before theme/application rules, while retaining author inline style as the
  final override.
- Migrated `AppShell`, `WorkbenchLayout`, `WorkbenchMain`, `ScrollArea`,
  `Body`, `Toolbar`, `ToolbarSeparator`, and `SearchBox` away from merging
  defaults into author style dictionaries.
- Completed the remaining composite-constructor audit. `GridLayout`,
  `FlowLayout`, `DropZone`, `LogView`, `DragVector`, `PropertyGrid`,
  `Property`, `SelectableList`, `Breadcrumbs`, `CommandPalette`,
  `RadioGroup`, and `ColorPicker` now serialize constructor defaults through
  `default_style` while preserving caller-provided `style` as author-inline
  declarations.
- Added a shared default-style merge helper for composite subclasses and a
  regression matrix proving that constructor defaults and author-inline
  overrides remain separate for grid, drop-zone, selectable-list,
  color-picker, and property-row representatives.
- Added the first computed-style provenance slice. Parsed CSS declarations now
  retain their authored property name and source value, `widget-default` is an
  explicit cascade origin, and every direct declaration applied to a computed
  style records its origin and ordering metadata.
- Computed-style debug snapshots now expose per-property `winner` and
  `overridden` entries. Stylesheet candidates include selector, source
  location, source order, specificity, importance, and authored value;
  widget-default and inline candidates include their serialized value.
- Added an end-to-end provenance regression covering the complete direct
  declaration chain: framework, widget-default, theme, application/user, and
  author inline.
- Live inline-style reparsing now rebuilds provenance after `SetStyle` and
  specialized runtime style updates, so snapshots cannot retain stale
  declaration candidates after a patch.
- Normalized shorthand provenance onto the computed fields each declaration
  affects. Diagnostics now correctly resolve collisions such as `padding`
  versus `padding-left`, `flex` versus `flex-grow`, and the corresponding
  margin, gap, overflow, border, outline, radius, transition, animation, and
  transform families while retaining `authored_property`.
- Normalized nested serialized pseudo-state and part styles. Default/inline
  maps such as `hover.background`, `parts.stepper.width`, and
  `parts.stepper.hover.background` now compete on the same provenance keys as
  equivalent stylesheet selectors.
- Added inherited text provenance for all eleven fields DragonGUI currently
  inherits. An inherited winner records the immediate `inherited_from` widget,
  preserves the original declaration metadata, and exposes its
  `source_origin`; declarations computed directly on the child still suppress
  inheritance.
- Limited recursive inheritance context to the eleven inheritable property
  candidate lists rather than cloning every provenance entry for every node.
- Added `native-fallback` as the explicit lowest cascade origin. Every property
  chain that participates in computed-style resolution now begins with a
  fallback candidate before framework, widget-default, theme, application,
  inherited, or inline candidates.
- Expanded the representative cascade regression to verify the complete
  six-stage direct chain:
  `native-fallback < framework < widget-default < theme < user < inline`.
- Untouched renderer/theme defaults are deliberately not synthesized into
  debug output yet; reporting their resolved values requires a widget-kind
  fallback catalog shared with layout, text, and primitive rendering.
- Added per-authored-property `default_style_sources` metadata to Python widget
  and VDOM serialization. Each record identifies the public widget type,
  fully qualified Python class, and `widget-default` construction kind.
- Canonical source keys cover flat properties, pseudo states, widget parts, and
  part states, so Python metadata joins the same property chain as native CSS
  provenance.
- Native widget-default candidates now expose `python_widget_type`,
  `python_class`, and `construction` in computed-style snapshots.
- Migrated practical invariant composite defaults from Python construction
  payloads into semantic framework CSS rules. `AppShell`, `WorkbenchLayout`,
  `ScrollArea`, `Body`, `WorkbenchMain`, `DropZone`, `PropertyGrid`,
  `Property`, `Breadcrumbs`, `SearchBox`, `CommandPalette`, and `ColorPicker`
  now obtain their static sizing, spacing, flex, and overflow contracts from
  framework origin.
- Retained constructor- and runtime-dependent values at widget-default origin,
  including shell/workbench gaps and padding, scroll-axis overflow, toolbar
  orientation and compact sizing, selectable-list bounds, color-picker width,
  and composite alignment declarations not yet supported by the CSS IR.
- Added native regressions proving semantic framework defaults resolve for
  representative shell, scroll, search, property, breadcrumb, and picker
  composites and remain overridable by application CSS.
- Audited stateful rendering and live-style reapplication. Hover, active,
  focus, disabled, checked, open, selected, and expanded visuals are resolved
  through temporary merged values rather than mutations of the computed base
  or authored style layers.
- Added an interactive state-cycle regression that enters and leaves hover,
  active, focus, and disabled states while proving computed base styles,
  parsed inline styles, and authored JSON remain unchanged.
- Added a combined media/pseudo cascade regression. Repeated wide-to-narrow-to-
  wide resolution replaces active media candidates without stale provenance,
  preserves widget-default and inline layers, and retains correct
  framework/theme/user/inline precedence for `:hover`.
- Started the resolved fallback catalog with a bounded six-property core shared
  by diagnostics and the layout/text/paint paths: font size, font weight,
  color, opacity, horizontal overflow, and vertical overflow.
- The stylesheet store now retains the effective framework theme used to
  resolve theme-derived fallback values. Untouched computed-style snapshots
  expose exact font-size and color values with `source_origin: theme`, while
  native font-weight, opacity, and overflow defaults expose their resolved
  values directly.
- Replaced independent layout, text, and primitive font-size/opacity literals
  with shared fallback helpers, preventing diagnostics from drifting away from
  rendered behavior.
- Added fallback regressions for untouched properties and overridden chains.
  A CSS declaration now retains the resolved native/theme fallback as its
  first overridden candidate instead of a value-less placeholder.
- Extended the shared catalog with invariant widget layout defaults for
  `display`, `flex-direction`, `flex-wrap`, `flex-grow`, and `flex-shrink`.
  It now describes the stable contracts for windows, horizontal and vertical
  layouts, scroll areas, pages, grids, flows, bars, panes, panels, collapsibles,
  drag/drop containers, trees, and the expanding chart/table families.
- The Taffy style builder reapplies those catalog values before fixed-size
  props and authored styles. Computed-style diagnostics serialize the same
  catalog, so reported fallback values cannot diverge from the layout engine.
- Kept context- or prop-dependent values out of the invariant catalog:
  horizontal/vertical layout shrink depends on the parent, panel/sidebar grow
  depends on fixed width, splitter direction depends on orientation, pane
  growth and sizing depend on pane props, and spacer/image sizing depends on
  their fixed-size inputs. These require a conditional fallback model rather
  than misleading unconditional provenance.
- Added cross-check regressions for ScrollArea, GridLayout, FlowLayout, and
  StatusBar, plus untouched FlowLayout provenance assertions covering the
  exact `flex / row / wrap / 0 / 1` fallback contract.
- Added a node-resolvable conditional layout fallback layer on top of the
  invariant catalog. Splitter orientation, fixed-width Panel/Sidebar behavior,
  fixed versus flexible Image/HtmlReport/Spacer behavior, and content-bearing
  Tabs growth now resolve from the current widget rather than remaining
  duplicated across renderer branches.
- Folded fixed-size prop post-processing into the shared resolver, including
  the existing exclusions for Sidebar width, MenuBar/StatusBar height, and
  non-flow overlay widgets. This keeps fallback provenance aligned with the
  final native grow/shrink values produced for untouched widgets.
- Removed the corresponding conditional grow/shrink assignments from the
  Splitter, Panel, Sidebar, Image, HtmlReport, Spacer, and Tabs Taffy branches.
  Layout construction now receives these values from the same resolver used by
  computed-style diagnostics.
- Added renderer/provenance cross-checks for vertical fixed Splitters, fixed
  Panels and Images, flexible and fixed Spacers, and content-bearing Tabs.
  Parent-sensitive HLayout/VLayout shrink, Pane allocation overridden by live
  splitter state, and parent-axis interpretation of authored preferred sizes
  remain outside this node-only layer.
- Added a shared parent fallback context carrying the parent widget kind,
  resolved flex direction, and whether overflow preserves preferred main-axis
  sizes. Stylesheet traversal derives it after resolving each parent and passes
  it to every child; Taffy construction builds the same context from its
  existing parent arguments.
- Moved HLayout/VLayout shrink selection into the shared resolver. Direct
  Window children remain shrinkable while the same layouts nested under other
  containers retain their non-shrinking contract, with no parallel conditional
  left in the widget-specific Taffy branches.
- Made inferred flex growth axis-aware: an authored width suppresses implicit
  growth in a row but remains a cross-axis constraint in a column, while an
  authored height behaves conversely. Overflow parents also publish the same
  preferred-size-preserving shrink result used by layout, and direct
  ScrollArea children resolve to non-shrinking content.
- Added parent-context renderer and provenance regressions covering Window
  versus Panel nesting, row width versus cross-axis height, and column height
  versus cross-axis width.
- Moved Pane flex growth into the shared fallback resolver. Default and
  `pane_flex` allocation, fixed logical-pixel sizes, and fractional pane sizes
  now drive the same value consumed by Taffy and diagnostic provenance.
- Made runtime computed-style snapshots refresh the Pane native fallback on a
  cloned style using the current `WidgetState.pane_sizes` entry. Fixed live
  sizes report `flex-grow: 0`, fractional sizes report their active fraction,
  and live candidates identify `source_origin: widget-state`.
- Kept construction-time cascade caches immutable during snapshot refresh.
  Prop-derived Pane allocation is labeled `source_origin: widget-prop`, while
  repeated fixed/fractional snapshot tests prove that live diagnostics change
  without rewriting the retained computed style.
- Removed the duplicated Pane flex-grow calculation from its Taffy branch;
  that branch now owns only axis-specific preferred/minimum/maximum geometry.
- Started the intrinsic-geometry catalog with the stable embedded-content
  subset: Image has a 48-by-48 minimum, HtmlReport has a 360 default height and
  240-by-160 minimum, and Extension has an 80 default height with zero minimum.
- Taffy applies these logical-pixel geometry values from the shared catalog
  before fixed props and authored styles. The former Image/HtmlReport/Extension
  branch literals have been removed, leaving one source for both renderer
  dimensions and computed-style provenance.
- Untouched `height`, `min-width`, and `min-height` provenance now exposes the
  exact stable geometry values. Stronger stylesheet height and minimum-width
  declarations retain those native values as their first overridden
  candidates.
- Added catalog-to-Taffy and provenance/override regressions for Image,
  HtmlReport, fixed-height HtmlReport, and Extension. Theme-, font-, wrapping-,
  and content-dependent control geometry remains deliberately outside this
  stable first slice.
- Extended the geometry resolver with the effective computed style and Theme.
  Standard controls, square icon/image/arrow buttons, Badge/Tag, LED,
  Checkbox/ToggleSwitch, Slider/RangeSlider/ProgressBar, TextInput,
  TextArea/CodeEditor/LogView, LoadingSpinner, non-wrapping Label, and
  header-only Tabs now resolve their preferred dimensions through the shared
  catalog.
- Control heights derive from the winning font size and theme spacing. Text
  areas additionally honor the winning CSS row count, LEDs retain prop-driven
  square sizing, and default-wrapping Labels deliberately remain auto-height
  for the measurement pass.
- Removed the corresponding width/height literals and calculations from the
  widget-specific Taffy branches. The catalog is applied before fixed props and
  authored dimensions, retaining the existing override order.
- Geometry provenance identifies theme-derived dimensions with
  `source_origin: theme` and explicit LED sizing with
  `source_origin: widget-prop`. A regression proves that an authored Button
  font size recalculates the native height candidate before an authored height
  wins.
- Added renderer/provenance cross-checks for Buttons, icon buttons, Badge, LED,
  TextArea rows, and wrapping versus non-wrapping Labels. Existing text-area
  and multiline-label measurement regressions remain green.
- Added an explicit `dynamic_geometry` record to per-widget layout diagnostics.
  This keeps post-layout measurements beside resolved rectangles rather than
  fabricating static declarations in computed-style provenance.
- Wrapped Labels now report `source: layout-measurement`, available width,
  measured content height, and final resolved height. Measurements are
  recomputed with the active theme and layout scale at snapshot time.
- TreeNode diagnostics now report `source: widget-state-and-content`, expanded
  state, child count, row height, child-content height, and total resolved
  height. The calculation uses the same row-height helper and parent-axis
  context as hit testing and layout.
- The runtime debug snapshot supplies the active Theme and WidgetState to this
  diagnostic pass. The existing context-free snapshot helper remains available
  for tests and callers that only need raw geometry.
- Added a separation regression proving dynamic Label/TreeNode geometry appears
  in layout diagnostics while wrapped Label height remains absent from static
  cascade provenance.
- Started the shared paint-fallback catalog with the core box and control
  family: Panel, Modal, Sidebar, StatusBar, MenuBar, Button, SmallButton,
  IconButton, ImageButton, ArrowButton, and Dropdown.
- Resting background, border color, border width, and radius now resolve from
  one catalog consumed by both primitive rendering and computed-style
  provenance. Theme-derived values carry `source_origin: theme`; the invariant
  one-pixel border width remains a native value.
- Interactive control paint uses the same catalog with an explicit resting,
  hovered, focused, pressed, or disabled input. Computed-style provenance
  intentionally publishes only the stable resting fallback, so live widget
  state is not misrepresented as a static cascade declaration.
- Replaced the migrated renderer branches' direct surface, border, and radius
  fallbacks with catalog values while retaining authored solid/gradient paint,
  per-corner radii, and state-style precedence.
- Added paint catalog unit coverage plus a cascade regression for untouched
  Panel/Button paint, non-box Label exclusion, and application-CSS overrides
  of all four representative visual properties.
- Extended the shared catalog to Selectable, RadioButton, TextInput, TextArea,
  CodeEditor, LogView, NumberInput, and DragNumber. Selection rows expose their
  stable transparent surface/border contract; fields expose their resting
  mixed surface, theme border, one-pixel border, and theme radius.
- Field hover, focus, disabled, and DragNumber pressed colors now resolve
  through the catalog's explicit interaction input. The migrated primitive
  branches consume those resolved values after authored and part styles, so
  existing CSS and state selectors remain authoritative.
- Kept Checkbox and ToggleSwitch out of this top-level slice because their
  meaningful defaults belong to checked-state `box`, `indicator`, `track`, and
  `thumb` parts. They remain assigned to the widget-part paint migration rather
  than being reported as misleading whole-widget background values.
- Added a renderer/provenance cross-check across both selection widgets and
  representative text, number, and drag fields, including application-CSS
  override chains for all four cataloged visual properties.
- Added part-level paint fallbacks for Slider `track`/`fill`/`thumb`,
  RangeSlider `track`/`range`/`thumb-min`/`thumb-max`, ProgressBar
  `track`/`fill`, and LoadingSpinner `track`/`arc`.
- Computed-style provenance now seeds stable part keys such as
  `part(track).background`, `part(thumb).border-color`, and
  `part(fill).background`. Top-level Slider/RangeSlider `track-color`,
  `thumb-color`, and `accent` fallbacks are also resolved rather than emitted
  as value-less native candidates.
- The primitive renderer consumes the same part catalog after authored part and
  widget paint. Disabled range/progress/spinner colors use the catalog's
  interaction input; value-dependent positions, fill widths, and spinner phase
  remain runtime state outside static cascade provenance.
- Added part-catalog unit coverage, exact native part-provenance assertions,
  user-CSS part override chains, and a primitive cross-check proving Slider and
  ProgressBar emit the cataloged track/thumb/fill colors.
- Extended part paint resolution with an explicit checked flag and migrated
  Checkbox `row`/`box`/`indicator` plus ToggleSwitch `row`/`track`/`thumb`
  defaults. Checked, pressed, and disabled endpoints now come from the same
  catalog used by the renderer.
- Static provenance seeds only unconditional resting parts: Checkbox row/box
  and ToggleSwitch row/track/thumb. The Checkbox indicator is checked-only, so
  it is resolved at render time and deliberately omitted from unconditional
  `part(indicator).background` provenance.
- ToggleSwitch animation still interpolates its runtime checked progress
  between cataloged off/on track endpoints. Thumb position and Checkbox marker
  geometry remain runtime measurements rather than cascade declarations.
- Added checked-control catalog coverage, resting-part provenance and user-CSS
  override assertions, and a renderer cross-check proving checked Checkbox and
  ToggleSwitch primitives emit the shared part colors.
- Extended the catalog through the stable navigation and tree surfaces:
  TreeView/TreeNode top-level paint, TreeNode `row`/`indicator`/`guide`, Tab
  `tab`/active `accent`, and NavItem `item`/active `accent`.
- Part resolution now carries selection separately from checked state. Tree
  selection, active tabs, and active navigation items therefore resolve their
  state colors from the same catalog without conflating those states with
  Checkbox/ToggleSwitch state.
- Migrated the TreeNode, Tab, and NavItem primitive branches to consume catalog
  values after authored widget/part paint. A widget-authored `accent` still
  controls the derived selected mix and retains precedence over theme defaults.
- Static provenance seeds only stable resting navigation parts. Selected-only
  Tab/NavItem accent paint stays runtime-only, while TreeNode row, indicator,
  and guide defaults publish exact native-fallback candidates.
- Added the stable Badge/Tag pill radius to top-level provenance. At that
  intermediate step their fills remained deliberately node-aware because
  `level=success|warning|danger|neutral` changes the semantic color; this
  avoided publishing a false unconditional background candidate.
- Added navigation catalog unit coverage, an exact provenance regression that
  excludes selected-only paint (and, at that intermediate point, semantic-level
  paint), and a renderer cross-check proving selected TreeNode, active Tab, and
  active NavItem primitives emit the shared colors.
- Completed the node-aware Badge/Tag paint slice with a catalog resolver keyed
  by widget kind, semantic `level`, theme, and interaction. Badge now resolves
  its solid semantic fill, matching border color, and zero default border;
  Tag resolves its 22% semantic surface tint, semantic border, and one-pixel
  default border.
- Both primitive rendering and computed-style provenance now call that same
  node-aware resolver. The old standalone renderer-only semantic color helpers
  were removed, eliminating the duplicate level mapping.
- Badge/Tag paint derived from an explicit `level` records
  `source_origin: widget-prop`; absent/unknown levels retain the theme accent
  default. Application CSS background, border-color, and border-width
  candidates continue to override the native semantic defaults.
- Expanded paint catalog coverage across warning, success, and neutral levels,
  strengthened the pill provenance/override regression with exact semantic
  values, and converted the standalone pill renderer test into a catalog
  cross-check.
- Added stable data-widget container paint for Histogram, BarChart, Heatmap,
  PieChart, LinePlot, Scatter3D, and DataFrameTable. The shared catalog now owns
  their resting surface/transparent background, border color, one-pixel border,
  and radius contracts.
- Migrated all corresponding chart container render entry points to consume
  catalog paint after authored CSS. Internal plot fills continue to derive from
  the resolved container surface, while series palettes, pie slices, heatmap
  cells, hover highlights, and toolbar states remain runtime/data paint.
- Added DataFrameTable named-part fallbacks for `header`, `row-selected`, and
  `grid-line`. The renderer now consumes the same header tint, selected-row
  tint, grid color, and grid width published by computed provenance.
- Deliberately left `DataFrameTable::row` without an unconditional background:
  ordinary rows may be transparent or receive runtime odd-row striping, so a
  static fallback candidate would be misleading.
- Added exact data-widget container/part provenance and application-CSS
  override coverage, plus a renderer cross-check for BarChart chrome and
  DataFrameTable header, selected-row, and grid primitives.
- Cataloged scrollbar track/thumb paint for every supported scroll-container
  kind: HLayout, VLayout, Pages, Page, Sidebar, Panel, Collapsible, Modal, and
  DataFrameTable.
- Preserved the two intentional native palettes. Generic containers use the
  softer 22% track/58% thumb alpha pair; DataFrameTable uses its stronger 20%
  track/68% thumb pair and distinct thumb mix. Panel/DataFrameTable framework
  CSS remains free to override those native candidates with transparent tracks
  and theme-border thumbs.
- Both generic panel-scrollbar emission and DataFrameTable scrollbar emission
  now resolve their fallback colors through the shared part catalog after
  authored part paint. The duplicated renderer-local color formulas remain
  only as defensive fallbacks for unsupported future widget kinds.
- Computed provenance now publishes exact native
  `part(scrollbar-track).background` and
  `part(scrollbar-thumb).background` candidates for all supported kinds, with
  framework and application declarations layered above them normally.
- Kept scrollbar thickness, padding, travel, and default pill radii out of paint
  provenance because they depend on resolved part layout, viewport geometry,
  axis, and scale.
- Added generic/table scrollbar catalog and provenance coverage, application
  override chains, a default HLayout renderer cross-check, and catalog-color
  assertions in the existing DataFrameTable scrollbar geometry regression.
- Cataloged the stable overlay paint boundary: Tooltip surface chrome, Modal
  `scrim`, and Menu/ContextMenu `menu`, `item`, `item-hover`, and
  `item-disabled` parts.
- Modal, tooltip, static-tooltip, and menu-popup primitive paths now consume
  those shared fallbacks after authored widget/part paint. Static tooltips keep
  their existing compositor contract by forcing the final fallback surface
  opaque even when the theme surface token carries alpha.
- Computed provenance now seeds exact native/theme candidates for the stable
  overlay surface and part properties. Open/closed visibility, popup position,
  shadows, animation, drag ghosts, dropdown selection indices, and toast
  severity remain runtime/node state rather than unconditional cascade values.
- Added overlay catalog coverage, exact Tooltip/Modal/Menu/ContextMenu
  provenance and user-override assertions, and a renderer cross-check for the
  shared scrim, tooltip, popup, and row-state colors.
- Extended overlay paint resolution through Dropdown popup chrome and rows.
  The stable `menu` and resting `item` parts now seed provenance; selected,
  hovered, and selected-plus-hovered row colors are resolved from the same
  catalog using live selection/interaction inputs without being presented as
  unconditional cascade values.
- Moved Toast info/success/warning/error surface and border derivation into the
  node-aware paint catalog. Both runtime virtual-toast rendering and normal
  Toast-node provenance use those semantic fallbacks, with explicit node
  `level` values identified as `source_origin: widget-prop`.
- Kept toast placement, stacking, opacity, animation, and content measurement
  in the overlay runtime. The drag ghost remains runtime-owned because it has
  no widget or virtual CSS identity and changes accent from live target
  compatibility; cataloging it now would create unverifiable static
  provenance.
- Added exact Dropdown/Toast catalog and provenance coverage, user-CSS
  override assertions, expanded the overlay renderer/catalog cross-check, and
  retained all six toast geometry/opacity regressions.
- Cataloged Heatmap `grid` and `scalar-bar` chrome and migrated the renderer to
  consume those shared fallbacks after authored part paint. Grid color/width
  and scalar-bar border color/width/radius now seed exact static provenance.
- Added a runtime-only Heatmap `hover` fallback consumed when a hovered cell
  exists, without publishing unconditional `part(hover)` provenance. Authored
  hover surface, border, width, and radius remain authoritative.
- Heatmap cell colors and the default scalar gradient remain colormap/data
  output rather than cascade defaults; an authored scalar-bar background can
  intentionally replace the gradient. The remaining chart series, slices,
  auto-contrast labels, selection highlights, and toolbar state were audited
  and retained as data/content/interaction paint rather than keeping Phase 2
  open for false static candidates.
- Expanded the paint catalog and existing data-widget provenance/renderer
  cross-checks to cover Heatmap defaults and authored `grid`, `scalar-bar`, and
  `hover` part overrides.
- Cataloged Collapsible outer surface/border/radius paint and its
  interaction-aware `header` background. The renderer now consumes the same
  resting/hover/focus/pressed values published by the catalog after authored
  widget and part paint.
- Static provenance seeds the Collapsible outer chrome and resting header
  background. Expanded body visibility, body paint inheritance, header/body
  geometry, and indicator direction remain runtime/layout concerns; no false
  unconditional `part(body).background` candidate is emitted.
- Added exact Collapsible catalog and provenance coverage, application-CSS
  override chains, and a renderer cross-check for resting and hovered header
  output while retaining the existing border-ordering and indicator tests.
- Cataloged structural divider paint for Separator and Splitter `gutter`.
  Separator now publishes and renders its theme-border line from the shared
  top-level catalog; Splitter gutter rendering consumes the shared part color
  while preserving authored opacity.
- Splitter gutter provenance records the native theme-border candidate beneath
  the framework's intentional transparent default and any application
  override. Gutter thickness/radius remain layout/framework values, and Pane
  remains paintless unless authored, so neither receives misleading native
  surface paint.
- Added structural-divider catalog, provenance, framework-precedence, user
  override, and renderer cross-checks.
- Audited the remaining Tabs header and CodeEditor gutter candidates. Tabs
  header paint remains intentionally author-only, while CodeEditor gutter paint
  derives from the resolved field surface; neither now publishes a misleading
  unconditional part fallback. Indicator marks whose colors depend on text,
  focus, validity, checked state, or content remain runtime paint.
- Cataloged NumberInput `stepper`, `stepper-up`, and `stepper-down` surfaces for
  resting, hover/focus, and disabled interactions, plus the stable `divider` and
  `stepper-divider` theme-border colors. The renderer consumes those values
  after authored part paint while retaining an authored widget accent for
  interactive stepper tinting.
- Static provenance now seeds only the five stable NumberInput chrome surfaces.
  The inherited field surface, arrow marks, caret, stepper geometry, and
  validity/focus-dependent colors remain contextual and are not presented as
  unconditional cascade candidates.
- Added exact NumberInput paint-catalog, native/user provenance ordering, and
  resting/hovered renderer regressions.
- Cataloged RadioButton indicator chrome: the stable circle surface, border,
  and border width now resolve through the shared part catalog for resting,
  selected, and disabled states. The renderer retains authored part paint and
  an authored widget accent before consuming those fallbacks.
- The RadioButton `dot` fallback is available only when selection is supplied
  to runtime paint resolution. Static provenance seeds the always-present
  `indicator` properties but intentionally omits `part(dot).background`, so a
  selected-only mark is not misrepresented as unconditional computed style.
- Added exact RadioButton catalog, native/user provenance ordering, and
  unchecked/selected renderer regressions.
- Audited DragNumber and LED internal paint. LED dot, glow, border, and
  highlight colors derive from the widget's semantic color/state and the
  resolved dot paint, so they remain node/runtime output instead of receiving
  false unconditional part defaults.
- Cataloged the DragNumber `grip` mark color for normal and disabled
  interactions. The renderer now consumes the shared fallback after authored
  grip paint while retaining an authored widget accent as the interactive grip
  tint source.
- Static provenance seeds only `part(grip).background`; the inherited field
  surface and value text remain control/text concerns. Added exact catalog,
  native/user provenance ordering, and normal/disabled renderer regressions.
- Cataloged the stable Collapsible `indicator` and Dropdown `chevron` mark
  colors for normal and disabled interactions. Both renderers consume the
  shared part fallback after authored state/base part paint.
- Disclosure width, placement, clipping, and open/expanded direction remain
  runtime/layout inputs. Static provenance records only each mark's background
  color, with no attempt to encode direction as cascade state.
- Added exact disclosure catalog, native/user provenance ordering, and
  normal/disabled renderer regressions for both widget families.
- Cataloged stable IconButton and ArrowButton `icon` mark colors for normal and
  disabled interactions. Both renderers now use the shared fallback beneath
  authored top-level foreground and `::icon` part paint.
- Icon names, generated glyph shape, and ArrowButton direction remain
  content/runtime inputs rather than cascade defaults. ImageButton image
  content remains author/data supplied and receives no synthetic icon paint.
- Added exact action-icon catalog, native/user provenance ordering, and
  normal/disabled renderer regressions for both button families.
- Closed the remaining non-chart paint inventory with Selectable `row` paint
  and its selected-only `indicator`. Resting, hover/focus, and selected row
  colors now share the catalog; the indicator is resolved only when selection
  is supplied at runtime and is omitted from unconditional provenance.
- Classified every remaining uncataloged public part: Panel accent and Pane
  surface are author-activated; Button/Tab/NavItem badges and ImageButton image
  paint are content/data supplied; labels, LogView severities, CodeEditor line
  numbers, and DragNumber values are text paint; transient selection/content
  marks remain runtime-only. The identity-less drag overlay remains explicitly
  deferred until it has a styleable virtual node.
- Added the final representative cascade matrix across dimensions, min/max
  dimensions, flex direction/grow/shrink, padding/gap, typography, visual
  properties, pseudo-state declarations, and responsive media declarations.
  The regression verifies the complete ordered chain:
  `native-fallback < framework < widget-default < theme < user < inline`.
- Added a Phase 2 acceptance regression proving that application CSS changes a
  default SearchBox width, a media query changes AppShell direction across
  wide/narrow/wide reapplication, and explicit inline width remains strongest.
- Updated the VDOM origin-separation regression: SearchBox `min-width` is now
  correctly expected from framework CSS rather than the serialized
  `default_style`; the remaining parameter/default alignment metadata stays
  separate from author inline style.
- Verified the rebuilt cascade-origin probe: application CSS resolves the
  default SearchBox to 360 pixels, explicit inline width resolves to 220
  pixels, and the run has zero warnings and unmatched selectors.
- Verified an isolated rebuilt wheel: all seven user selectors in the semantic
  identity probe matched, `SearchBox.semantic-search` resolved to 360 pixels,
  the search node retained
  `["SearchBox", "HLayout", "Container", "Widget"]`, its render kind remained
  `h_layout`, and the stylesheet reported no warnings or unmatched selectors.

### Current Verification

```text
Native CSS style suite: 198 passed, 0 failed, 5 ignored
Focused computed-style snapshot group: 7 passed
Focused provenance regressions: 5 passed
Targeted Python API/VDOM/audit suite: 35 passed
Focused CSS identity/VDOM suite: 18 passed
Smoke/visual-audit tooling suite: 35 passed
Focused Python/VDOM construction metadata: 4 passed
Focused final composite-origin regression: 2 passed
Focused static-default migration: 10 Python tests and 1 native regression passed
Full Python API file after static-default migration: 408 passed, same 4
  unrelated failures
Focused state-safety regressions: 2 passed
Focused resolved-fallback regressions: 2 passed
Invariant widget layout catalog cross-check: 1 passed
Node-conditional fallback cross-checks: 2 passed
Parent-context fallback cross-checks: 2 passed
Live Pane fallback and snapshot cross-checks: 2 passed
Focused Pane/panel regression group: 43 passed
Stable intrinsic-geometry cross-checks: 2 passed
Contextual control-geometry cross-checks: 2 passed
Shared paint catalog unit tests: 20 passed
Paint provenance/override regressions: 11 passed
Focused field/state primitive regressions: 4 passed
Range/progress renderer/catalog cross-check: 1 passed
Checked-control renderer/catalog cross-check: 1 passed
Navigation/tree renderer/catalog cross-check: 1 passed
Semantic Badge/Tag renderer/catalog cross-check: 1 passed
Data-widget renderer/catalog cross-check: 1 passed
Generic scrollbar renderer/catalog cross-check: 1 passed
DataFrameTable scrollbar geometry/catalog cross-check: 1 passed
Overlay renderer/catalog cross-check: 1 passed
NumberInput chrome renderer/catalog cross-check: 1 passed
Focused text-area regression group: 9 passed
Native primitive suite: 105 passed, 7 known failures, 3 ignored
Complete native suite: 716 passed, 8 known failures, 12 ignored
Phase 2 Python API/VDOM/audit suite: 459 passed, 4 unrelated failures
Representative six-stage cascade matrix: 2 passed
Phase 2 SearchBox/AppShell/inline acceptance regression: 1 passed
Focused wrapped-label regression: 1 passed
Layout snapshot and dynamic-geometry group: 6 passed
Native library compile check: passed with 7 existing dead-code warnings
Phase 0 probe smoke run: 4 passed
Phase 0 visual baseline: 4 targets captured
Native rebuilt-wheel semantic probe: 7/7 user selectors matched, 0 warnings;
  source line/column, specificity, source order, and match count verified
Native rebuilt-wheel cascade probe: stylesheet width 360px; inline width 220px
```

The broader Python API run still contains pre-existing environment and
unrelated failures involving SciPy DLL loading, dataframe integer-width
expectations, and node-graph runtime state. The new focused tests pass and
those unrelated failures were not weakened or modified.

## Motivation and Evidence

The Aurora demo passed the strict layout audit at 390x720, 640x480, 1024x768,
and 1440x900 at 1.0x and 1.5x scale. Visual inspection still found several
problems:

1. Public-looking selectors such as `SearchBox.command-search` did not match
   because the node was serialized as `h_layout`.
2. Composite widget defaults are merged into the same style mapping as
   author-supplied inline styles, making the intended cascade unclear.
3. The search control remained at its 164-pixel minimum despite a stylesheet
   width of 340 pixels.
4. A sidebar child panel expanded to roughly 200x565 despite containing only a
   few compact controls.
5. The adaptive metric grid produced an awkward three-plus-one layout at
   1024x768.
6. The initial phone layout gave the workbench zero usable width, yet the
   strict audit reported no issues.
7. Navigation labels and placeholders were truncated without a structured
   diagnostic.
8. The stylesheet snapshot exposed a warning count but not the individual
   warnings or unmatched selectors.

These are not isolated demo defects. They reveal missing public contracts in
widget identity, cascade precedence, flex sizing, responsive layout, and audit
semantics.

## Desired Outcome

An application author should be able to write:

```python
with dg.AppShell(class_="application-shell"):
    with dg.Sidebar(
        title="Aurora",
        width=232,
        collapsed_width=56,
        collapsible=True,
    ):
        ...
    with dg.WorkbenchLayout():
        with dg.Toolbar():
            dg.SearchBox(
                placeholder="Search routes",
                class_="command-search",
                grow=True,
            )
        with dg.Body():
            ...
```

and style it predictably:

```css
SearchBox.command-search {
    width: 340px;
}

@media (max-width: 760px) {
    AppShell.application-shell {
        flex-direction: column;
    }

    Sidebar {
        state: collapsed;
    }

    SearchBox.command-search {
        width: 100%;
    }
}
```

The debug snapshot and strict audit should explain:

- which selectors matched;
- which declarations won;
- why a widget received its final size;
- when a visible subtree was starved or clipped;
- whether text truncation was intentional;
- which container owns any overflow.

## Scope

This plan includes:

- semantic CSS identities for public widgets;
- explicit cascade origins for widget defaults and authored inline styles;
- strict layout and usability diagnostics;
- standardized flex and intrinsic-size defaults;
- responsive flex, shell, sidebar, navigation, and grid behavior;
- composite-widget sizing APIs, starting with `SearchBox`;
- richer debug snapshots and visual-audit state coverage;
- documentation, migration guidance, and updated examples.

## Non-Goals

This plan does not:

- replace Taffy;
- replace Cosmic Text;
- introduce a browser DOM;
- add a JavaScript runtime;
- guarantee that every desktop application automatically becomes a good phone
  application;
- treat subjective visual-design preferences as hard layout errors;
- weaken the clipping and overflow guarantees completed by the previous plan.

## Design Principles

### Public concepts must remain visible

The native renderer may use `h_layout` to render a `SearchBox`, but CSS,
snapshots, diagnostics, and developer tools must retain the public
`SearchBox` identity.

### Defaults are not inline author styles

Framework and widget defaults must be overridable by user stylesheets. Only
values supplied through the public `style={...}` argument should receive
inline-author precedence.

### Flexible behavior must be explicit

Ordinary panels and controls should size to content. Remaining space should be
consumed only by explicit flexible regions such as `Body`, `Spacer`, or a
widget configured with `grow=True`.

### Responsive changes must preserve reachability

Breakpoint changes must never leave the main work area at zero size, place
interactive content outside its paint clip, or create unreachable scrolling.

### Diagnostics must distinguish structural errors from design advisories

A visible zero-sized workbench is a structural error. A deliberately
ellipsized navigation label is a usability advisory. Both should be
observable, but they should not have the same default severity.

### Compatibility must be measurable

Existing render-kind selectors, class selectors, Python APIs, and layout
snapshots must continue working through a documented transition period.

---

# Target Architecture

## 1. Separate render kind from semantic CSS identity

Every serialized node will carry:

```json
{
  "render_kind": "h_layout",
  "css_types": ["SearchBox", "HLayout", "Container", "Widget"],
  "classes": ["search-box", "command-search"]
}
```

Definitions:

- `render_kind` selects native layout, painting, and interaction behavior.
- `css_types` represents the public widget type and relevant public base types.
- `classes` contains framework and application-authored CSS classes.
- `id` retains its existing selector meaning.

Type selectors will match any entry in `css_types`. Existing internal
render-kind selectors remain supported as compatibility aliases, but new
documentation will use public widget names.

Python subclasses should contribute their public class name when it is stable
and serializable. Private helper classes beginning with `_` should not become
public CSS types.

## 2. Introduce explicit style origins

The resolved cascade will distinguish:

1. native fallback;
2. framework stylesheet;
3. widget defaults;
4. theme stylesheet;
5. application stylesheet;
6. author inline style;
7. transient state adjustments required for interaction.

Python widgets will stop merging defaults into the author's `style` mapping.
The serialized node will carry separate fields:

```json
{
  "default_style": {
    "height": 38,
    "min_width": 164
  },
  "inline_style": {
    "width": 340
  }
}
```

Application stylesheets must be able to override `default_style`.
Author-supplied `style={...}` must remain stronger than application
stylesheets.

## 3. Define a shared sizing contract

| Widget category | Default grow | Default shrink | Minimum behavior |
| --- | ---: | ---: | --- |
| Leaf control | 0 | 1 | Semantic hit target and non-text chrome |
| Label | 0 | 1 | Shaped text or wrapping policy |
| Panel | 0 | 1 | Content-sized, zero flex minimum |
| Toolbar/MenuBar/StatusBar | 0 | 0 | Compact intrinsic height |
| SearchBox | Configurable | 1 | Icon chrome plus usable input |
| Body | 1 | 1 | Zero flex minimum on both axes |
| ScrollArea | Configurable | 1 | Bounded viewport, explicit scroll axes |
| Spacer | 1 | 1 | Zero |
| Plot/Table viewport | 1 when placed in flexible body | 1 | Semantic viewport minimum |
| Sidebar | 0 | 0 | Expanded or collapsed configured width |

The contract must be enforced in both Python defaults and native layout tests.

## 4. Use structured diagnostics

Layout diagnostics will use stable codes:

```json
{
  "code": "starved_subtree",
  "severity": "error",
  "widget_id": "dg-18",
  "css_type": "WorkbenchLayout",
  "rect": {"x": 390, "y": 0, "w": 0, "h": 720},
  "parent_id": "dg-2",
  "message": "Visible WorkbenchLayout received zero usable width.",
  "context": {
    "main_axis": "horizontal",
    "previous_sibling": "dg-3",
    "previous_sibling_size": 390
  }
}
```

Diagnostics will be emitted by the engine, retained in snapshots, summarized
by smoke tests, and rendered in visual-audit reports.

---

# Implementation Phases

## Phase 0: Lock the Baseline and Reproductions

### Objectives

- Preserve the current known-good geometry baseline.
- Turn each Aurora finding into a focused regression.
- Avoid changing selector or cascade behavior without before-and-after
  evidence.

### Tasks

- [x] Add a minimal composite-selector probe containing `SearchBox`,
      `Toolbar`, `AppShell`, and `WorkbenchLayout`.
- [x] Prove that public type selectors currently do not match these composites.
- [x] Add a cascade probe that applies conflicting widget defaults,
      application stylesheet declarations, and author inline declarations.
- [x] Add a sidebar panel probe reproducing unintended vertical expansion.
- [x] Add a starved-workbench probe reproducing a zero-width visible subtree.
- [x] Add a grid probe reproducing the three-plus-one card arrangement.
- [x] Record current debug snapshots for each reproduction.
- [x] Add the probes to the visual-audit manifest.
- [x] Record native, Python, smoke, and screenshot baselines.

### Acceptance Criteria

- Each reported problem has a deterministic automated reproduction.
- Existing passing layout-remediation tests remain unchanged.
- The baseline clearly differentiates current behavior from desired behavior.

### Likely Files

- `examples/css_feature_probes/`
- `examples/css_feature_probes/visual_audit_manifest.json`
- `tests/test_layout_audit.py`
- `tests/test_visual_audit.py`
- `native/src/layout.rs`
- `native/src/css_style.rs`

## Phase 1: Semantic CSS Widget Identity

### Objectives

- Make public widget names valid CSS type selectors.
- Preserve native render behavior and existing selectors.
- Expose selector identity in debug tooling.

### Python Tasks

- [x] Add a stable `css_types()` or equivalent metadata method to `Widget`.
- [x] Generate the public CSS type chain from supported widget classes.
- [x] Allow a widget to explicitly define or suppress semantic CSS aliases.
- [x] Serialize the existing render `type` separately from `css_types` in the
      Python widget and VDOM payloads.
- [x] Preserve current `kind` serialization during the compatibility window.
- [x] Add Python tests for direct widgets, composites, subclasses, and private
      helpers.

### Native Tasks

- [x] Extend the native node schema with semantic type aliases.
- [x] Update selector matching so a type selector checks semantic aliases.
- [x] Preserve matching for existing native kind names.
- [x] Define case-normalization and serialization rules.
- [x] Ensure pseudo-elements and pseudo-states retain the originating widget's
      semantic identity.
- [x] Record matched semantic type in computed-style diagnostics.

### Tooling Tasks

- [x] Add per-selector matched-node counts for active user rules.
- [x] Report active user selectors that matched zero nodes.
- [x] Do not report a media rule whose condition was inactive.
- [x] Include selector source, line, specificity, and match count.
- [x] Surface unmatched-selector details in strict smoke output and reports.

### Compatibility Rules

- `.class` and `#id` behavior must not change.
- Existing `panel`, `h_layout`, and similar internal selectors continue to
  match for at least one release cycle.
- Public type selectors are additive.
- Selector specificity does not change merely because a node has multiple
  semantic aliases.

### Acceptance Criteria

- `SearchBox.command-search` matches `SearchBox`.
- `Toolbar.command-toolbar` matches `Toolbar`.
- `HLayout` may match `SearchBox` through its public base-type chain.
- Native rendering continues to use `h_layout`.
- Debug snapshots display both native and public identities.
- A deliberately unmatched public type selector produces an actionable
  diagnostic and can fail strict CSS automation.

## Phase 2: Style-Origin and Cascade Refactor

### Objectives

- Stop treating framework/widget defaults as author inline styles.
- Make responsive application CSS authoritative over defaults.
- Preserve explicit `style={...}` as the strongest author declaration.

### Tasks

- [x] Introduce separate serialized `default_style` and `inline_style`.
- [x] Assign explicit cascade origin and precedence to every declaration.
  - [x] Direct framework, widget-default, theme, application/user, and inline
        declarations carry ordered origins.
  - [x] Add inherited origin and immediate-parent/source-origin linkage.
  - [x] Add native-fallback origin to participating property chains.
- [x] Add source metadata for Python defaults, framework CSS, theme CSS,
      application CSS, and inline styles.
  - [x] Direct stylesheet declarations retain selector, authored value,
        source location/order, specificity, and importance.
  - [x] Serialized widget-default and inline declarations retain origin and
        authored value.
  - [x] Attach Python widget/class construction metadata to widget defaults.
- [x] Refactor `HLayout`, `VLayout`, `AppShell`, `WorkbenchLayout`, `Toolbar`,
      `Body`, `SearchBox`, and other composites to stop merging defaults into
      user style dictionaries.
  - [x] Shell, workbench, scroll-area, toolbar, separator, and SearchBox
        families migrated.
  - [x] Remaining composite constructors audited and migrated.
- [x] Move static defaults into framework classes where practical.
  - [x] Move supported invariant sizing, spacing, flex, and overflow values for
        shell, scroll, workbench, search, property, breadcrumb, drop-zone,
        command-palette, and color-picker composites.
  - [x] Keep `align_items` and `justify_content` in widget defaults until those
        declarations are supported by the stylesheet IR.
- [x] Retain dynamic defaults, such as toolbar orientation, through the
      lower-priority widget-default origin.
  - [x] Preserve parameter-driven gaps, padding, axis overflow, orientation,
        compact sizing, maximum dimensions, and variant styles in Python.
- [x] Ensure stateful native adjustments do not permanently overwrite authored
      values.
  - [x] Verify interactive visual resolution is copy-based and leaves computed,
        widget-default, inline, and authored JSON layers unchanged.
  - [x] Verify repeated media and pseudo-state reapplication replaces active
        candidates and provenance without retaining stale winners.
- [x] Expose the winning declaration and overridden candidates in computed
      styles.
  - [x] Debug snapshots expose winners and overridden candidates for direct
        authored properties.
  - [x] Normalize shorthand/longhand collisions while retaining the original
        authored property.
  - [x] Normalize nested serialized pseudo-state and part-state properties.
  - [x] Add inherited-value provenance.
  - [x] Prepend native fallback to every participating computed-property chain.
  - [x] Catalog and expose resolved native/theme fallback values for properties
        with no authored declarations.
    - [x] Add a shared bounded catalog for base font size, font weight, color,
          opacity, and both overflow axes.
    - [x] Expose catalog values for untouched properties and as the first
          overridden candidate when a stronger declaration exists.
    - [x] Add invariant widget layout defaults for display, flex direction,
          flex wrapping, growth, and shrinking, and consume the same catalog
          from Taffy style construction and computed-style provenance.
    - [x] Model context- and prop-dependent layout defaults without presenting
          them as unconditional computed values.
      - [x] Resolve node-prop and child-structure conditions shared by layout
            and provenance, including fixed-size post-processing.
      - [x] Pass parent layout context into fallback resolution for
            HLayout/VLayout shrink and main-axis authored-size behavior.
      - [x] Represent live-state-dependent Pane allocation without publishing
            stale construction-time values.
    - [x] Consolidate widget-specific intrinsic geometry and paint defaults
          into shared renderer/provenance catalogs.
      - [x] Catalog stable Image, HtmlReport, and Extension default/minimum
            geometry and consume it from Taffy and provenance.
      - [x] Consolidate theme-, font-, wrapping-, and content-dependent control
            geometry without losing its measurement context.
        - [x] Catalog theme/style-resolvable control dimensions, text-area rows,
              explicit LED sizing, and non-wrapping Label height.
        - [x] Bridge post-measure wrapped text and state/content-dependent row
              geometry into diagnostics without treating it as a static
              cascade value.
      - [x] Consolidate widget-specific paint defaults.
        - [x] Catalog resting box paint and stateful control paint for Panel,
              Modal, Sidebar, StatusBar, MenuBar, the button family, and
              Dropdown; consume the resting values from provenance.
        - [x] Catalog stable Selectable/RadioButton row paint and stateful
              TextInput/TextArea/CodeEditor/LogView/NumberInput/DragNumber
              field paint.
        - [x] Catalog Slider, RangeSlider, ProgressBar, and LoadingSpinner
              top-level and named-part paint without treating value-dependent
              geometry as cascade state.
        - [x] Catalog Checkbox and ToggleSwitch resting and checked part paint
              while keeping checked-only indicator and animated position state
              out of unconditional provenance.
        - [x] Catalog TreeView/TreeNode and Tab/NavItem resting and selected
              paint, plus the stable Badge/Tag pill radius, while keeping
              selected-only accents and semantic badge levels out of
              unconditional provenance.
        - [x] Catalog node-aware Badge/Tag semantic fills and borders with
              widget-prop provenance for explicit levels.
        - [x] Catalog stable chart/data-widget container chrome and
              DataFrameTable header, selected-row, and grid-line paint while
              keeping series/cell/striping state dynamic.
        - [x] Catalog generic and DataFrameTable scrollbar track/thumb paint
              while keeping geometry-derived thickness and radius out of static
              provenance.
        - [x] Catalog stable Tooltip surface, Modal scrim, and
              Menu/ContextMenu popup and row-state paint while keeping
              visibility, placement, shadows, animation, and other transient
              overlay state dynamic.
        - [x] Catalog Dropdown popup/resting-row paint and node-aware Toast
              severity surfaces while keeping selection, hover, placement,
              stacking, opacity, and animation as runtime inputs.
        - [x] Catalog stable Heatmap grid/scalar-bar chrome and runtime hover
              paint while retaining cells and the default scalar gradient as
              colormap/data output.
        - [x] Audit remaining chart series, slice, label, selection, and toolbar
              paint and classify data/content/interaction-derived values
              outside static cascade provenance.
        - [x] Catalog Collapsible outer chrome and interaction-aware header
              paint while keeping body visibility, inherited body paint, and
              indicator direction dynamic.
        - [x] Catalog structural Separator and Splitter gutter paint while
              keeping gutter geometry in framework/layout defaults and Pane
              surfaces author-only.
        - [x] Audit Tabs header, CodeEditor gutter, and indicator-only paint;
              retain author-only, inherited, and state/content-derived values
              outside unconditional provenance.
        - [x] Catalog NumberInput stepper surfaces and divider colors while
              keeping field inheritance, arrow/caret marks, validity state, and
              geometry contextual.
        - [x] Catalog RadioButton indicator surface/border chrome and expose the
              selected dot only to runtime checked-state resolution.
        - [x] Catalog DragNumber grip paint while keeping inherited field/value
              text contextual, and classify LED semantic/state-derived
              internals as runtime paint.
        - [x] Catalog Collapsible indicator and Dropdown chevron colors while
              keeping disclosure direction, clipping, placement, and geometry
              in runtime/layout resolution.
        - [x] Catalog IconButton and ArrowButton icon colors while keeping glyph
              identity/direction runtime-derived and ImageButton content
              author/data supplied.
        - [x] Catalog/classify all remaining truthful non-chart widget-part
              paint fallbacks; defer drag overlay paint until it gains a
              styleable virtual identity.

### Required Cascade Tests

For every representative property:

```text
native fallback
  < framework stylesheet
  < widget default
  < theme stylesheet
  < application stylesheet
  < author inline style
```

Test:

- dimensions;
- min/max dimensions;
- flex direction;
- grow/shrink;
- padding/gap;
- typography;
- visual properties;
- media-query declarations;
- pseudo-state declarations.

### Acceptance Criteria

- [x] A stylesheet can change a default `SearchBox` width.
- [x] A stylesheet media query can change a shell's default direction.
- [x] An explicit Python `style={"width": 420}` remains stronger than stylesheet
  width declarations.
- [x] Computed-style output explains why each winning value won.
- [x] No existing widget loses its default appearance when unstyled.

## Phase 3: Structural and Usability Diagnostics

### Objectives

- Detect layouts that are in bounds but unusable.
- Give tools enough context to explain failures.
- Avoid false positives for intentionally inactive content.

### Structural Error Codes

- [x] `starved_subtree`: visible normal-flow subtree has effectively zero usable
      area.
- [x] `empty_paint_clip`: visible widget has a non-zero rect but an empty final
      paint clip.
- [x] `fully_clipped_interactive`: enabled interactive widget is entirely
      clipped.
- [x] `unreachable_scroll_content`: content exists outside the viewport with no
      valid scroll range.
- [x] `disabled_axis_overflow`: scroll content overflows an axis declared
      disabled.
- [x] `flex_allocation_failure`: a required flexible region receives no
      remaining main-axis space.
- [x] `invalid_min_max_resolution`: resolved dimensions violate min/max
      contracts.

### Usability Advisory Codes

- [x] `text_truncated`: text is clipped by line-height or estimated available
      width, with explicit ellipsis exempted.
- [x] `placeholder_truncated`: form-control placeholder is not substantially
      visible.
- [x] `interactive_content_too_small`: control content box is below its
      semantic minimum.
- [x] `scroll_viewport_too_small`: visible scroll viewport is below a useful
      threshold.
- [x] `responsive_orphan`: optional grid advisory for a severely unbalanced
      final row.
- [x] `excessive_unused_flex_space`: optional advisory for a content-sized
      component unexpectedly receiving large free space.

### Exemptions

Diagnostics must ignore or downgrade:

- `display: none`;
- inactive `Page` children;
- closed modals and menus;
- virtualized table rows outside the active range;
- intentionally hidden overflow used for animation;
- zero-thickness separators;
- `Spacer`;
- explicit ellipsis policies;
- offscreen measurement nodes;
- loading-screen transition nodes.

### Audit Integration

- [x] Add severity thresholds to `smoke_css_demos.py`.
- [x] Make structural errors fail `--strict-layout`.
- [x] Keep usability advisories visible without failing by default.
- [x] Add `--strict-usability` for demos that require advisory-free output.
- [x] Group report entries by code, widget, page, size, and scale.
- [x] Include direct snapshot and screenshot links.

### Acceptance Criteria

- [x] The reproduced zero-width workbench fails strict layout.
- [x] Inactive pages do not produce starvation failures.
- [x] Deliberately ellipsized navigation labels do not become structural
      errors; an explicit ellipsis policy suppresses heuristic truncation
      warnings.
- [x] Native structural diagnostics include widget identity, rect, clip,
      parent, and reason.

## Phase 4: Normalize Flex and Intrinsic-Size Defaults

### Objectives

- Make default growth and shrinking consistent.
- Prevent panels and composites from absorbing space unintentionally.
- Keep designated application bodies flexible.

### Tasks

- [x] Audit every shipping widget's grow, shrink, basis, min-size, and overflow
      defaults.
- [x] Add a machine-readable sizing-contract table to tests.
- [x] Make `Panel` content-sized with `flex_grow: 0` by default.
- [x] Verify `Sidebar` lays out content-sized children without distributing
      unused height into ordinary panels.
- [x] Keep `Body`, `Pages`, active `Page`, and intended viewport widgets
      shrink-safe.
- [x] Ensure fixed chrome does not grow or shrink.
- [x] Define how percentage sizes interact with intrinsic and preferred sizes.
- [x] Verify that explicit `flex_grow`, `flex_shrink`, and fixed widget props
      remain authoritative.
- [x] Remove duplicate or contradictory defaults between Python and framework
      CSS.

### Sizing Contract Decisions

- Percentage and `calc()` main-axis dimensions are preferred sizes. They stop
  implicit growth but remain shrinkable under pressure unless the author sets
  `flex_shrink: 0`.
- Explicit `flex_grow` and `flex_shrink` always override framework and native
  fallbacks. A fixed widget prop remains inflexible unless a widget's public
  responsive contract explicitly bounds that prop, as `Sidebar` does.
- `Panel` is content-sized (`flex_grow: 0`, `flex_shrink: 1`) by default.
  Authors opt into remaining-space allocation with explicit growth.
- `Body`, `ScrollArea`, `Pages`, active `Page`, `WorkbenchLayout`, and
  `WorkbenchMain` remain shrink-safe with zero semantic minimums.
- `MenuBar`, `StatusBar`, and `Toolbar` are fixed chrome with zero implicit
  growth and shrink.

### Representative Regression Matrix

Test each of the following in row and column parents:

- short and long labels;
- panels with one and many children;
- nested panels;
- sidebar status cards;
- toolbars with and without spacers;
- body between fixed menu/status chrome;
- plots and tables in panels;
- scroll areas with fixed and flexible siblings;
- 100% and `calc()` preferred sizes;
- min-content and max-content pressure.

### Acceptance Criteria

- [x] The Aurora sidebar health-panel geometry is reproduced by a deterministic
  native layout tree and remains content-sized.
- [x] Adding a normal `Panel` to a tall `Sidebar` does not make it consume all
  remaining height.
- [x] `Body` still consumes the remaining application area.
- [x] Existing window, panel, percentage-size, scroll, and overflow regressions
  remain green.

## Phase 5: Composite Widget Sizing APIs

### Objectives

- Make composite sizing controllable without knowledge of internal children.
- Establish one reusable composite-sizing pattern.

### SearchBox API

Add:

```python
dg.SearchBox(
    width=340,
    min_width=180,
    max_width=None,
    grow=False,
    shrink=True,
)
```

### Tasks

- [x] Remove the contradictory default combination of `width: 100%`,
      `flex_grow: 0`, and `flex_shrink: 0`.
- [x] Define standalone, toolbar, and explicitly growing behavior.
- [x] Ensure the inner input receives remaining width after icon chrome.
- [x] Allow `clearable=False` to release clear-button space.
- [x] Guarantee a useful minimum text-entry region.
- [x] Emit a placeholder-truncation advisory when appropriate.
- [x] Apply the same composite-sizing pattern to `PropertyGrid`,
      `DragVector`, date/time composites, command palettes, and other compound
      controls after the SearchBox implementation proves stable.

### Composite Migration Progress

- [x] `SearchBox`: preferred/min/max width, grow/shrink, zero-basis input, and
  optional clear chrome.
- [x] `DragVector`: content-sized/shrink-safe outer flow with explicit bounds
  and growth while fixed component groups continue to wrap.
- [x] `PropertyGrid`: full cross-axis width, content-sized main-axis behavior,
  explicit bounds/growth, and zero-basis editor slots.
- [x] `CommandPalette`: embedded SearchBox opts out of standalone width and
  fills the modal through framework-owned class styling.
- [x] `ColorPicker`: explicit outer bounds/growth plus zero-basis sliders
  between fixed channel/value labels.
- [x] `DateInput`, `TimeInput`, and `DateTimeInput` classified as atomic native
  text inputs rather than composites; they have no hidden child sizing contract
  to migrate.
- [x] Audit and normalize remaining public compound containers (`Property`,
  `SelectableList`, `Breadcrumbs`, `RadioGroup`, and `TreeView`).

### Acceptance Criteria

- [x] The Aurora desktop SearchBox contract resolves to a 340-pixel preferred
  width.
- [x] It shrinks safely on compact toolbars.
- [x] `grow=True` fills remaining toolbar space.
- [x] Icons remain fixed while only the input region shrinks.
- [x] No child escapes the composite's bounds in the native geometry
  regression.

## Phase 6: Responsive Flex and Application Shell

### Objectives

- Allow breakpoint-driven direction changes.
- Make application shells usable at compact widths.
- Eliminate zero-width workbench failure modes.

### FlexLayout

Introduce or formalize:

```python
dg.FlexLayout(
    direction="row",
    wrap=False,
    gap=8,
    align_items="stretch",
)
```

`direction` is a widget default, not an inline declaration, and can therefore
be overridden by application CSS.

### HLayout and VLayout Compatibility

- [x] Treat `HLayout` and `VLayout` directions as lower-origin defaults.
- [x] Allow application CSS to override direction.
- [x] Retain the convenience and readability of the existing Python classes.
- [x] Verify row/column axis changes recompute percentage sizes, gaps,
      intrinsic contributions, scrolling, and reconciliation.

### AppShell

- [x] Make `AppShell` responsive-direction capable.
- [x] Add a minimum-main-content safeguard.
- [x] Emit `flex_allocation_failure` before a body reaches zero usable area.
- [x] Define behavior when fixed siblings collectively exceed the viewport.
- [x] Prefer shrinking/collapsing eligible chrome before starving `Body`.

### Acceptance Criteria

- [x] A media rule changes an `AppShell` from row to column.
- [x] The workbench remains reachable throughout live resize.
- [x] Repeated resize across a breakpoint converges without stale clips or scroll
  offsets.
- [x] Scale-factor changes preserve the same logical breakpoint semantics.

## Phase 7: Responsive Sidebar and Navigation

### Objectives

- Provide intentional expanded, collapsed, and drawer modes.
- Eliminate application-specific clipped-label rails.

### Proposed Sidebar API

```python
dg.Sidebar(
    title="Aurora",
    width=232,
    collapsed_width=56,
    state="auto",
    collapsible=True,
    compact_mode="rail",
    mobile_mode="drawer",
)
```

### Proposed NavItem API

```python
dg.NavItem(
    "Automation",
    page="automation",
    icon="workflow",
    compact_label="Flows",
    badge=3,
)
```

### Tasks

- [x] Define expanded, collapsed, hidden, and overlay-drawer states.
- [x] Keep state controllable from Python and CSS.
- [x] Preserve keyboard navigation and focus when state changes.
- [x] Provide tooltips or accessible names for icon-only navigation.
- [x] Define badge behavior in compact mode.
- [x] Add a menu-button integration for opening a drawer.
- [x] Ensure drawer overlays do not participate in normal-flow sizing.
- [x] Restore focus to the opener after closing the drawer.
- [x] Add resize and repeated-toggle regressions.

### Acceptance Criteria

- [x] The phone layout can use an icon rail or drawer without truncated primary
  labels.
- [x] The main body keeps a configured minimum usable width.
- [x] Sidebar state changes do not reset the active page.
- [x] Overlay mode remains within the window and is fully dismissible.

## Phase 8: Responsive and Balanced Grid Layout

### Objectives

- Give authors deterministic breakpoint control.
- Avoid visually awkward adaptive grids when desired.

### Proposed API

```python
dg.GridLayout(
    columns={
        "default": 4,
        1100: 2,
        700: 1,
    },
    gap=12,
)
```

Optional:

```python
dg.GridLayout(
    columns="auto-fit",
    min_column_width=210,
    balance_last_row=True,
)
```

### Tasks

- [x] Define breakpoint map ordering and validation.
- [x] Serialize responsive column rules without duplicating CSS logic.
- [x] Resolve columns from logical viewport width, not physical pixels.
- [x] Preserve current integer `columns` behavior.
- [x] Preserve current `min_column_width` adaptive behavior.
- [x] Add an optional last-row balancing strategy.
- [x] Ensure masonry reconciliation works with breakpoint changes.
- [x] Verify nested grids converge after live resize.
- [x] Expose resolved column count and track widths in snapshots.

### Acceptance Criteria

- [x] Aurora metrics render 4 columns on wide desktop, 2x2 on laptop/compact, and
  1 column on phone.
- [x] No three-plus-one layout appears when explicit breakpoints request 2 columns.
- [x] Resize checkpoints converge without stale item positions.
- [x] Existing grid APIs remain compatible.

## Phase 9: Developer Tooling and Visual-Audit States

### Objectives

- Make CSS and layout decisions inspectable.
- Capture more than the default page and resting state.

### Debug Snapshot Additions

- [x] `render_kind`
- [x] `css_types`
- [x] matched selectors with source and specificity
- [x] unmatched eligible user selectors
- [x] winning declaration per property
- [x] overridden declarations per property
- [x] intrinsic size
- [x] preferred size
- [x] semantic minimum
- [x] allocated flex size
- [x] final resolved rect
- [x] final paint clip
- [x] scroll ownership
- [x] structural diagnostics
- [x] usability advisories

### Visual-Audit Manifest States

Allow targets to declare:

```json
{
  "states": [
    {"name": "overview", "route": "overview"},
    {"name": "analytics", "route": "analytics"},
    {"name": "modal-open", "actions": ["click:#new-review"]},
    {"name": "sidebar-collapsed", "actions": ["click:#sidebar-toggle"]}
  ]
}
```

### Tasks

- [x] Add stable IDs to demo interaction targets.
- [x] Add route initialization support.
- [x] Add deterministic click, type, scroll, resize, and wait actions.
- [x] Capture menus, modals, tooltips, collapsed sidebars, and scrolled states.
- [x] Compare diagnostics across state transitions.
- [x] Add report thumbnails grouped by size, scale, route, and state.
- [x] Preserve a no-interaction mode for simple probes.

### Acceptance Criteria

- [x] Every Aurora page has an automated screenshot.
- [x] Modal-open and collapsed-sidebar states are captured.
- [x] Reports expose warnings and diagnostics without opening raw JSON.
- [x] Failed diagnostics link directly to the relevant screenshot and node data.

## Phase 10: Documentation and Example Migration

### Objectives

- Teach the final contracts clearly.
- Remove workarounds from flagship examples.

### Documentation Tasks

- [x] Document public CSS type selectors and base-type matching.
- [x] Document cascade origins and precedence.
- [x] Document flex sizing defaults in a single reference table.
- [x] Document responsive direction, sidebar states, and grid breakpoints.
- [x] Document structural errors versus usability advisories.
- [x] Add a CSS troubleshooting section for unmatched selectors.
- [x] Add computed-style and layout-diagnostic examples.
- [x] Update both Markdown and Sphinx documentation.

### Example Tasks

- [x] Migrate `aurora_command_center_demo.py`.
- [x] Migrate `all_features_professional_demo.py`.
- [x] Remove redundant defensive styles made unnecessary by new defaults.
- [x] Keep intentional examples of explicit fixed, flexible, and scrolling
      behavior.
- [x] Add a small responsive application template.

### Migration Guidance

Document:

- internal render-kind selectors that should become public type selectors;
- composite defaults that now yield to application CSS;
- panels that no longer grow implicitly;
- SearchBox sizing changes;
- optional responsive APIs;
- how to retain old behavior explicitly.

### Acceptance Criteria

- [x] Flagship demos use public widget selectors.
- [x] No flagship demo depends on an internal render-kind selector.
- [x] Documentation examples pass strict layout and strict usability checks.

## Phase 11: Release Hardening

### Tasks

- [x] Run the complete native suite.
- [x] Run the complete Python API and audit suite.
- [x] Run all CSS smoke demos with strict layout.
- [x] Run layout torture targets across the standard size/scale matrix.
- [x] Run Aurora and the professional demo across all pages and states.
- [x] Compare performance, memory, stylesheet matching time, and snapshot size
      against the Phase 0 baseline.
- [x] Verify packaged-wheel behavior rather than source-tree behavior only.
- [x] Verify Windows scale transitions and resize loops.
- [x] Record known unrelated failures without weakening assertions.
- [x] Update release notes and compatibility guidance.

### Required Final Matrix

Sizes:

- 320x640
- 390x720
- 640x480
- 900x640
- 1024x768
- 1180x760
- 1440x900

Scales:

- 1.0x
- 1.25x where supported
- 1.5x
- 2.0x

Transitions:

- wide to compact to wide;
- expanded sidebar to collapsed to expanded;
- default page to alternate page;
- modal closed to open to closed;
- scroll top to bottom to top;
- theme reload;
- scale-factor change.

### Final Acceptance Criteria

- [x] No structural layout errors in the required matrix.
- [x] No unexplained stylesheet warnings.
- [x] No public selector in flagship demos has zero matches.
- [x] No visible normal-flow subtree is starved.
- [x] No enabled interactive control is fully clipped.
- [x] All intended scrolling remains reachable.
- [x] Responsive grid and sidebar behavior matches documented breakpoints.
- [x] Existing API compatibility surface remains intact; four recorded,
      unrelated environment/runtime failures remain without weakened assertions.
- [x] Performance remains within the regression budget.

---

# Testing Strategy

## Native Unit Tests

Focus on:

- semantic selector alias matching;
- cascade origin precedence;
- media-query precedence;
- flex allocation and starvation;
- panel content sizing;
- responsive direction changes;
- sidebar state geometry;
- grid breakpoint resolution;
- structured diagnostic generation;
- diagnostic exemption rules.

## Python API Tests

Focus on:

- serialized CSS identities;
- separate default and inline styles;
- new SearchBox sizing arguments;
- FlexLayout validation;
- Sidebar and NavItem responsive properties;
- responsive grid column maps;
- backward-compatible constructor behavior.

## Snapshot Contract Tests

Validate:

- schema version;
- stable diagnostic codes;
- semantic identity;
- computed-style provenance;
- selector match counts;
- intrinsic/preferred/allocated/final geometry;
- scroll ownership.

## Visual Tests

Validate:

- toolbar and search sizing;
- content-sized sidebar cards;
- body reachability;
- responsive navigation;
- balanced metrics;
- wrapped and ellipsized text;
- modal and menu bounds;
- fixed status bars;
- nested scrolling;
- high-DPI rendering.

## Property and Stress Tests

Generate combinations of:

- parent direction;
- wrap state;
- fixed and percentage sizes;
- min/max constraints;
- grow and shrink factors;
- long Unicode text;
- nested scroll owners;
- open and inactive overlays;
- rapid resize across breakpoints.

Core invariant:

> A visible required application region must either receive usable geometry or
> produce a structured diagnostic that identifies the allocation conflict.

---

# Delivery Strategy

Implement this plan as reviewable slices:

1. Baseline probes and frozen reproductions.
2. Semantic CSS identity.
3. Cascade-origin separation.
4. Structural diagnostics.
5. Flex-default normalization.
6. SearchBox sizing.
7. Responsive FlexLayout/AppShell.
8. Responsive Sidebar/NavItem.
9. Responsive GridLayout.
10. Visual-audit interactions.
11. Documentation, example migration, and release hardening.

Do not combine semantic selector identity, cascade refactoring, and default
flex changes in one change set. Each alters a different public contract and
needs an independently reviewable regression baseline.

# Compatibility and Rollout

## Compatibility Window

- Preserve existing render-kind selectors for at least one release cycle.
- Emit informational migration notices only when an internal selector has a
  direct public replacement.
- Do not change explicit author inline style behavior.
- Gate strict usability advisories separately from structural failures.
- Increment the debug layout schema version when fields or diagnostic shapes
  change.

## Feature Gating

If necessary, stage major behavior behind temporary flags:

- semantic selector aliases;
- new cascade origins;
- normalized panel growth;
- responsive sidebar states;
- strict usability auditing.

Flags are transitional and must have removal milestones.

# Performance Budgets

The implementation should target:

- no more than 5% median style-resolution regression on representative demos;
- no more than 5% median layout regression;
- bounded selector diagnostic storage;
- no unbounded accumulation of resize or state-transition diagnostics;
- no full-tree recomputation solely to produce debug information when debug
  capture is inactive;
- no material frame-time cost from usability advisories in normal release
  mode.

# Risks and Mitigations

## Selector compatibility

**Risk:** A new semantic alias unexpectedly matches an existing broad rule.

**Mitigation:** Record before-and-after selector match counts, preserve
specificity, and run all visual probes before enabling aliases by default.

## Cascade regressions

**Risk:** Styles previously embedded as inline defaults become overridable and
change existing applications.

**Mitigation:** Migrate one widget family at a time, publish computed-style
diffs, and document how to retain the old value explicitly.

## Diagnostic noise

**Risk:** Truncation and small-view advisories overwhelm reports.

**Mitigation:** Separate structural errors from advisories, define exemptions,
deduplicate by widget/code/state, and keep strict usability opt-in initially.

## Responsive state instability

**Risk:** Repeated breakpoint crossings leave stale clips, focus, or scroll
positions.

**Mitigation:** Add resize-loop tests and treat responsive changes as ordinary
full layout invalidations with bounded reconciliation.

## Grid complexity

**Risk:** Breakpoint maps and balancing interact poorly with masonry.

**Mitigation:** Resolve the column count before masonry packing and retain the
existing bounded convergence contract.

## Snapshot growth

**Risk:** Style provenance and diagnostics make snapshots excessively large.

**Mitigation:** Use compact references to stylesheet sources and allow verbose
computed-style details only in requested debug snapshots.

# Success Metrics

Track:

- unmatched eligible selectors in flagship demos;
- structural errors per visual-audit run;
- usability advisories per size and scale;
- number of application-level defensive min-size overrides;
- number of internal render-kind selectors in examples;
- percentage of shipping widgets covered by the sizing-contract matrix;
- style-resolution and layout timing;
- snapshot size;
- visual-audit route/state coverage.

Target end state:

- zero unmatched public selectors in flagship demos;
- zero structural errors in the required matrix;
- zero unexplained zero-area visible subtrees;
- zero application-specific workarounds for composite CSS identity;
- documented sizing contracts for every shipping widget family;
- automated screenshots for every Aurora page and interactive shell state.

# Progress Log

Use this section as implementation proceeds.

## Completed

- Phase 0 baseline probes, manifest registration, snapshots, and focused tests.
- Initial strict-audit `starved-subtree` detection for populated containers on
  row and column main axes.
- Python and VDOM serialization of stable public `css_types` chains.
- Declarative public CSS type names, aliases, suppression, and validation for
  Python subclasses and private helpers.
- Native semantic selector matching based on the serialized public type chain,
  with existing native render-kind selectors preserved.
- Debug-tree `render_kind` and `css_types` identity plus active user-selector
  match counts and unmatched-selector reporting.
- Strict smoke and visual-report surfacing for unmatched selectors.
- Structured selector provenance with source location, source order,
  specificity, and match counts.
- Phase 1 semantic CSS identity completed and verified through an isolated
  packaged wheel.
- Initial Phase 2 style-origin slice: separate widget-default and author-inline
  payloads, native precedence insertion, representative composite migration,
  and rebuilt-wheel cascade verification.
- Remaining Phase 2 composite constructors audited and migrated to keep
  widget defaults separate from author-inline declarations, with regression
  coverage for representative override collisions.
- Initial property-level computed-style provenance implemented for direct
  declarations, including the full five-origin winner/overridden chain and
  source metadata in debug snapshots.
- Shorthand/longhand and nested serialized pseudo-state/part provenance
  normalized onto shared computed-property keys, with authored-property
  metadata retained.
- Inherited text winners now retain their original declaration source and
  immediate parent linkage without overriding child-authored values.
- Native fallback now anchors participating property chains, and the direct
  precedence regression covers all six cascade stages.
- Python widget-default construction metadata now survives Widget and VDOM
  serialization and appears on the corresponding native provenance candidate.
- Practical invariant composite defaults now live in semantic framework CSS
  selectors, while parameter-driven and currently CSS-unsupported alignment
  values remain at widget-default origin.
- Static-default framework regressions cover AppShell, WorkbenchLayout,
  ScrollArea, WorkbenchMain, SearchBox, PropertyGrid, Breadcrumbs, and
  ColorPicker, including application-CSS override behavior.
- Stateful rendering and stylesheet reapplication are now covered by
  non-mutation regressions for interactive state cycles and responsive
  wide/narrow/wide media cycles with pseudo-state provenance.
- The resolved fallback catalog now covers six cross-engine properties plus
  invariant, node-resolvable, and parent-aware conditional widget display/flex
  behavior, including live Pane allocation refreshed at snapshot time, plus
  stable embedded-content and theme/style-resolvable control geometry. Layout
  construction and computed-style provenance consume the same static values,
  while post-measure content geometry is explicitly reported by layout
  diagnostics. Core box/control paint now shares the same catalog/provenance
  model. Selection rows, text/number/drag fields, range/progress parts, and
  checked-control and tree/navigation parts now share it as well. Badge/Tag
  stable pill geometry and node-aware semantic paint are cataloged. Stable
  chart/data container chrome and DataFrameTable header/selection/grid parts
  are also cataloged, along with Heatmap grid/scalar/hover paint,
  generic/table scrollbar colors, stable Tooltip/Modal/Menu/Dropdown overlay
  chrome, node-aware Toast severity paint, and Collapsible outer/header paint;
  structural Separator/Splitter divider paint, NumberInput stepper/divider
  chrome, RadioButton indicator/selected-dot paint, and DragNumber grip paint
  are cataloged as well, along with stable Collapsible/Dropdown disclosure-mark
  colors, IconButton/ArrowButton icon colors, and Selectable row/selected
  indicator paint. Tabs header, CodeEditor gutter, LED internals, ImageButton
  image content, content badges, text-only parts, and author-activated surfaces
  have been explicitly classified rather than receiving false static defaults.
  The identity-less drag-overlay paint is explicitly deferred.
- The representative property matrix now verifies all six cascade stages for
  layout, min/max, flex, spacing, typography, visual, pseudo-state, and media
  declarations. SearchBox width, responsive AppShell direction, explicit
  inline precedence, and computed winner/overridden provenance satisfy every
  Phase 2 acceptance criterion.
- Phase 2 style-origin and cascade refactor completed.
- End-to-end packaged-wheel verification of the semantic identity probe.
- Phase 3 structural diagnostics completed for paint-clip loss, fully clipped
  controls, unreachable scroll content, disabled ScrollArea axes, failed flex
  allocation, and invalid min/max resolution.
- Phase 3 usability advisory coverage completed for truncation, placeholders,
  undersized controls and scroll viewports, grid orphans, and suspicious unused
  flexible space.
- Strict audit severity handling completed: `--strict-layout` enforces
  structural errors, advisories remain visible by default, and
  `--strict-usability` opts into advisory-free enforcement.
- Visual-audit diagnostic reporting now groups by code, widget, page, size,
  and scale and links directly to the corresponding snapshot and screenshot.
- Phase 3 verification completed: 68 focused native runtime tests passed with
  one ignored benchmark; 40 focused Python layout/visual-audit tests passed;
  the optimized packaged-wheel CSS showcase passed strict layout with zero
  structural errors and zero advisories. The complete native suite remained at
  720 passed, 8 previously known failures, and 12 ignored.
- Phase 4 flex and intrinsic-size normalization completed. Ordinary `Panel`
  instances are content-sized, sidebar cards no longer absorb unused height,
  flexible viewport regions remain shrink-safe, and menu/status/toolbar chrome
  is inflexible by default.
- The core public framework contract is captured in
  `tests/fixtures/layout_sizing_contracts.json`. The exhaustive native table in
  `tests/fixtures/native_widget_sizing_contracts.json` records grow, shrink,
  basis, minimum-size, and overflow behavior for every `WidgetKind`; a count
  guard requires future widget variants to be classified.
- Toolbar's invariant grow/shrink/minimum defaults moved from Python
  construction metadata into framework CSS. The remaining constructor sizing
  declarations were classified as parameter-driven public values, fixed
  geometry, or composite-internal behavior. SearchBox's contradictory public
  sizing contract remains intentionally assigned to Phase 5, and responsive
  direction behavior remains assigned to Phase 6.
- Phase 4 verification completed: 148 focused native layout tests passed with
  one ignored benchmark; 200 native CSS/cascade tests passed with five ignored
  benchmarks. The complete native suite reached 724 passed, with the same 8
  previously known non-layout failures and 12 ignored tests. The broad Python
  API/VDOM/layout-audit/visual-audit run reached 464 passed with 4 unrelated
  existing failures: dataframe dtype inference, two unavailable-SciPy convex
  hull cases, and node-graph runtime bridge replacement.
- Phase 5 composite sizing APIs completed. SearchBox established the reusable
  preferred/minimum/maximum width and grow/shrink contract, zero-basis inner
  slot pattern, fixed-chrome behavior, and optional-chrome space release.
- The pattern now covers DragVector, PropertyGrid, Property, SelectableList,
  Breadcrumbs, RadioGroup, CommandPalette, ColorPicker, and TreeView.
  Content-oriented composites no longer grow unintentionally; TreeView remains
  flexible by default as a viewport. Horizontal RadioGroup instances wrap
  under pressure, and Property/PropertyGrid/ColorPicker inner controls consume
  only the space left after fixed labels or chrome.
- DateInput, TimeInput, and DateTimeInput were audited and classified as atomic
  native text inputs rather than hidden-child composites, so no composite
  sizing migration was required.
- Phase 5 verification completed: 149 native layout tests passed with one
  ignored benchmark; 200 native CSS/cascade tests passed with five ignored
  benchmarks. The complete native suite reached 725 passed with the same 8
  previously known failures and 12 ignored tests. The broad Python
  API/VDOM/layout-audit/visual-audit run reached 467 passed with the same four
  unrelated failures recorded after Phase 4.
- Phase 6 responsive flex and application-shell work completed. `FlexLayout`
  exposes CSS-overridable direction, wrapping, gap, and alignment defaults while
  preserving HLayout/VLayout compatibility. AppShell derives from FlexLayout
  and protects direct Body, WorkbenchLayout, and WorkbenchMain children with
  configurable 160-by-96 logical minimums.
- Eligible Sidebar chrome now yields before a protected main region is starved.
  `flex-allocation-failure` reports a non-empty flexible region as soon as it
  falls below its declared main-axis minimum; explicitly non-shrinking siblings
  retain their intentional overflow contract.
- The integrated Phase 6 acceptance regression drives one styled AppShell
  through wide/compact/wide layout passes. It verifies row/column direction,
  axis-specific gaps, recomputed descendant percentage widths, intrinsic
  minimums, reachable body clips, clamped scroll offsets, and exact restoration
  of rectangles, clips, paint clips, and scroll maps after crossing back over
  the breakpoint.
- The same regression replays logical widths on both sides of the breakpoint at
  1x and 2x scale. Direction and normalized geometry remain equivalent, proving
  that media breakpoints retain logical-pixel semantics.
- Final Phase 6 verification: 150 native layout tests passed with one ignored
  benchmark; 201 native CSS/cascade tests passed with five ignored benchmarks;
  five focused Python API/VDOM tests passed.
- Phase 7 responsive sidebar and navigation completed. Sidebar now exposes
  explicit `auto`, `expanded`, `collapsed`, `hidden`, and `drawer` states,
  configurable rail width, compact/mobile policies, live state methods, and an
  accessible `menu_button()` integration.
- Auto policy resolves to expanded above 700 logical pixels, the configured
  compact rail/hidden behavior through 700 pixels, and the configured mobile
  behavior through 480 pixels. Mobile drawer policy remains closed while in
  `auto`; the menu button changes the explicit state to `drawer`.
- NavItem supports icons, compact labels, accessible full labels, and compact
  badge dots. Expanded rows use full labels, intermediate rows prefer compact
  labels, and rails at 72 logical pixels or narrower center the icon without
  drawing truncated primary text.
- Drawers use fixed, viewport-bounded geometry and do not consume AppShell flow.
  Opening remembers the prior Sidebar state and focused opener, moves focus to
  the first enabled navigation item, and closing restores both. Escape and
  successful NavItem activation dismiss the drawer without resetting the
  active page.
- The Phase 7 resize matrix runs wide/rail/mobile-closed/drawer/rail/wide,
  verifies body minimums and bounded overlay geometry, and proves repeated
  rectangles, clips, and paint clips converge. A three-cycle runtime toggle
  regression verifies opener focus and active-page retention.
- Final Phase 7 verification: 150 native layout tests passed with one ignored
  benchmark; 203 native CSS/cascade tests passed with five ignored benchmarks;
  focused runtime, primitive, text, and Python 3.12 smoke tests passed. The
  available Python 3.12 interpreter does not contain pytest, so the committed
  Python pytest cases remain pending execution in the later full environment
  matrix. The broader primitive suite retains its seven previously recorded
  unrelated failures and three ignored benchmarks.
- Phase 8 responsive and balanced grid layout completed. `GridLayout.columns`
  now accepts deterministic maps such as
  `{"default": 4, 1100: 2, 700: 1}`. Numeric keys are inclusive logical
  viewport `max-width` thresholds, validated by Python and serialized in
  ascending canonical order; the native document model performs the same
  defensive ordering.
- Breakpoint selection divides the physical viewport width by the active scale
  factor before matching. The selected count remains a maximum when
  `min_column_width` is configured, preserving safe collapse for grids nested
  inside narrower content regions. Existing integer and `"auto"` APIs remain
  compatible, and `"auto-fit"` now explicitly lowers to native auto-fit tracks.
- `balance_last_row=True` centers incomplete final rows after native grid
  placement without changing track sizes or parent geometry. Balancing is
  intentionally excluded from masonry, explicit templates, and explicitly
  placed children so those reconciliation contracts remain authoritative.
- Layout snapshots now expose `layout.resolved_grids`, keyed by GridLayout id,
  with the resolved column count and physical-pixel column widths. This makes
  breakpoint and track decisions directly inspectable without reconstructing
  them from child rectangles.
- The Aurora metric grid and the responsive-grid-orphan probe now use the
  responsive 4/2/1 contract. Native regressions cover 1x/2x logical breakpoint
  equivalence, wide/compact/phone transitions, nested grids, masonry repacking,
  balancing, auto-fit lowering, and resolved snapshot metadata.
- Final Phase 8 verification: 154 native layout tests passed with one ignored
  benchmark; 16 native document
  tests passed; 11 focused Python GridLayout tests passed; and the combined
  VDOM, visual-audit, and layout-audit suite passed 57 tests. Python compilation
  and direct 3.12 API smoke tests passed, as did `git diff --check`. The full
  Python 3.11 API suite passed 416 tests and retained four unrelated failures:
  one environment-dependent integer-buffer width expectation, two tests affected
  by a broken local SciPy DLL installation, and one NodeGraph runtime-bridge
  replacement test.
- Phase 9 developer tooling and stateful visual auditing:
  - computed-style snapshots now expose structured matched-selector records with
    stylesheet origin, source location/order, and specificity alongside the
    existing winning and overridden declaration provenance;
  - layout diagnostics now expose intrinsic and preferred sizes, semantic
    minima, flex allocation, final rectangles and paint clips, scroll ownership,
    structural diagnostics, and usability advisories per node;
  - visual-audit targets can declare named routes and deterministic click,
    hover, type, scroll, resize, and wait actions while targets without states
    retain one untouched default capture;
  - reports now include state-aware thumbnail galleries, diagnostic comparisons
    between states, and direct screenshot, snapshot, and focused node-data
    links for each reported issue;
  - Aurora now supplies stable interaction IDs and automated coverage for all
    four pages plus modal-open, sidebar-collapsed, workspace-menu,
    sidebar-tooltip, and analytics-scrolled states.
- Final Phase 9 verification: the focused CSS-style suite passed 203 native
  tests with five ignored; the layout suite passed 154 tests with one ignored;
  the combined VDOM, layout-audit, and visual-audit suite passed 62 Python
  tests; the focused visual-audit suite passed all 28 tests; Python compilation
  and `git diff --check` passed. A live 1024x768 Aurora run captured all nine
  declared states with no reported layout diagnostics. The visual review also
  caught and corrected a click-automation false positive so the final
  sidebar-collapsed artifact reflects the actual collapsed state.
- Phase 10 documentation and example migration completed:
  - the Markdown, Sphinx, and built-in `dg.help(...)` references now document
    exact public CSS type chains, base-type matching, cascade origins,
    constructor-default precedence, responsive Sidebar/Grid/Flex behavior,
    the shared flex-sizing contract, structural errors versus usability
    advisories, and computed-style/layout troubleshooting workflows;
  - stale guidance claiming that composites such as `ColorPicker` lacked a
    public type selector, or that lowercase native render kinds were valid type
    selectors, was removed;
  - Aurora and the professional demo now use public composite sizing arguments,
    stable interaction targets, responsive Sidebar contracts, and public widget
    selectors without redundant `Panel` minima or default `SearchBox` widths;
  - `examples/responsive_app_template.py` provides a small routed `AppShell`
    starter with responsive navigation, adaptive grids, explicit scrolling,
    and four visual-audit states across desktop and phone sizes.
- Phase 10 live verification also found and repaired two tooling defects:
  offscreen descendants of a valid scroll owner no longer emit structural
  `empty-paint-clip`/`fully-clipped-interactive` errors, and the professional
  demo reachability validator now inspects the active nested `page-scroll`
  owner rather than its non-scrolling outer `Body`.
- Final Phase 10 verification: strict structural, usability, and active-selector
  smoke checks passed for Aurora, the professional demo, and the responsive
  template with zero issues in every category. Stateful visual audits captured
  nine Aurora phone states, the professional overview at desktop and phone
  sizes, and eight responsive-template route/sidebar captures, all with zero
  diagnostics and zero unmatched selectors. The native runtime style-patch
  suite passed 70 tests with one ignored benchmark; 29 visual-audit tests and
  12 focused Python API/VDOM/layout tests passed; Python compilation and
  `git diff --check` passed. The native wheel and source-tree extension were
  rebuilt successfully before live validation.
- Phase 11 release hardening completed:
  - the complete native suite reached 738 passed, with the same eight known
    primitive/text failures and 12 ignored tests;
  - the combined Python API, VDOM, layout-audit, and visual-audit suite reached
    479 passed and retained the same four unrelated failures;
  - all four CSS smoke demos passed strict layout, strict usability, and strict
    CSS checks with zero warnings or issues after removing four dead selectors;
  - four layout torture probes passed 112 combinations spanning 320x640 through
    1440x900 at 1x, 1.25x, 1.5x, and 2x;
  - Aurora passed compact/desktop 1x/2x coverage for every page and state, then
    passed all 12 states at 1.5x with explicit sidebar, modal, and scroll round
    trips and embedded resize checkpoints;
  - all eight professional-demo pages passed compact/desktop and 1x/2x checks.
    The audit found one real workflow-page defect: a 320px fixed panel clipped
    its activity log by 50px at 2x. Raising the demonstrated panel contract to
    370px restored full scroll reachability in all four post-fix captures;
  - exact Phase 0 comparisons show average frame time improved by 80.2-80.9%
    and apply-layout time by 61.2-65.0%. Stylesheet reapply remains below 0.6ms;
    richer debug provenance increased serialized snapshots by 250-268%;
  - the final wheel was rebuilt, extracted, and verified independently of the
    source tree. It passed WGPU rendering, strict diagnostics, live stylesheet
    reload, and a 600-frame 25.3MB peak-working-set smoke;
  - release and compatibility notes are published in
    `docs/layout-styling-release-notes.md`, with detailed measurements in
    `artifacts/layout-styling-phase11/METRICS.md`.

## In Progress

- None. The layout and styling evolution plan is complete.

## Blocked

- None.

# Completion Checklist

- [x] Phase 0 baseline and reproductions complete.
- [x] Semantic CSS identity complete.
- [x] Style-origin cascade refactor complete.
- [x] Structural and usability diagnostics complete.
- [x] Flex and intrinsic-size normalization complete.
- [x] Composite sizing APIs complete.
- [x] Responsive flex and application shell complete.
- [x] Responsive sidebar and navigation complete.
- [x] Responsive grid behavior complete.
- [x] Developer tooling and visual-audit states complete.
- [x] Documentation and examples migrated.
- [x] Release-hardening matrix passes.
- [x] Compatibility notes published.
- [x] Final benchmarks recorded.
- [x] Plan status changed to Complete.
