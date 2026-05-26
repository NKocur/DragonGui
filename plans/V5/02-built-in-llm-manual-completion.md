# V5 Built-In LLM Manual Completion Plan

Status: implemented, with ongoing enrichment as new public APIs are added.

## Objective

Finish the built-in `dg.help` manual so an LLM can use it as an authoritative,
structured reference for generating DragonGUI applications.

The manual should answer three classes of questions without requiring source
inspection:

1. What widgets, helpers, callbacks, dataclasses, CSS parts, and live methods
   exist?
2. Which widget should be used for a given UI or data workflow?
3. What layout, styling, threading, and performance rules should generated code
   follow?

## Current State

Implemented:

- `python/dragongui/manual.py` exposes a callable `dg.help` object.
- `dg.help()` returns the index.
- `dg.help("path.to.section")` resolves string paths.
- Nested attribute access works, for example `dg.help.layout.panels()`.
- `dg.help.search(query)` returns ranked section metadata.
- `dg.help.to_dict()` returns structured content for tool/LLM ingestion.
- Current topics cover:
  - LLM build rules
  - quickstart
  - app/window/loading/threading/diagnostics
  - layout: panels, flex, flow, grids, splitters, scroll, overlays
  - widgets: inputs, navigation, tables, plots, feedback, media
  - styling: selectors, parts, properties, queries, themes
  - callbacks, live updates, components, recipes, performance, extensions,
    debugging
- Python API tests cover basic nested lookup, search ranking, and structured
  dictionary output.

## Original Identified Gaps

These were the gaps identified before implementation. They are now covered by
the reference branch, coverage tests, or curated guide sections below.

Exports currently missing by exact public-name mention in the manual audit:

- Core/runtime: `BackendUnavailableError`, `backend_info`,
  `native_backend_available`, `HelpSection`
- Components/VDOM/resources: `ComponentCtx`, `ComponentInstance`, `StateSlot`,
  `Patch`, `ResourceRef`, `VNode`
- Dialogs/helpers: `FileDialog`, `open_file_dialog`, `open_files_dialog`,
  `pick_folder_dialog`, `save_file_dialog`, `alert`, `confirm`
- Thread diagnostics UI: `ThreadMonitor`
- Navigation/data dataclasses: `BreadcrumbItem`, `PropertyChange`,
  `TableSort`
- Plot/data payload dataclasses: `HistogramBins`, `BarChartData`,
  `DragDropPayload`, `ScatterHit`, `ScatterPayload`, `LinePlotPayload`,
  `ScatterStreamMetrics`, `ScatterFrameStream`, `ScatterLiveFrame`
- Widgets not yet singled out clearly enough: `DragSource`, `DropTarget`,
  `DropZone`, `RadioButton`, `SearchBox`
- Camera helpers: `link_cameras`, `unlink_cameras`

Other gaps:

- Individual widget leaves are grouped by family, but many widgets do not yet
  have their own `dg.help.widgets.<family>.<widget>()` section.
- Constructor signatures are not available programmatically from the manual.
- Live methods are described by family, not exhaustively per widget.
- Callback argument shapes are described generally, not as a complete matrix.
- CSS part support is manually listed and can drift from
  `_SUPPORTED_PARTS_BY_KIND`.
- CSS type selectors, state selectors, attribute selectors, supported
  properties, and known CSS limits should be summarized from
  `docs/widgets-reference.md` and `docs/css-styling.md`.
- Validation/error behavior is mostly absent. Examples: empty dropdown items,
  invalid splitter sizes, invalid number steps, invalid CSS parts.
- There are no direct links from manual sections to probes/examples.
- There is no coverage test ensuring all `dragongui.__all__` exports are
  represented.
- There is no compatibility/version metadata in `dg.help.to_dict()`.

## Proposed Manual Shape

Keep the current guide sections, then add a structured reference branch:

```text
dg.help.reference
dg.help.reference.exports
dg.help.reference.widgets
dg.help.reference.widgets.button
dg.help.reference.widgets.number_input
dg.help.reference.widgets.scatter3d
dg.help.reference.dataclasses
dg.help.reference.dialogs
dg.help.reference.runtime
dg.help.reference.css_parts
dg.help.reference.css_selectors
dg.help.reference.live_methods
dg.help.reference.callbacks
```

Each reference leaf should include:

- public name
- category
- constructor signature or callable signature
- purpose
- common usage snippet
- callback arguments, if applicable
- live methods, if applicable
- CSS type selector
- supported CSS parts
- validation notes
- related widgets
- related probes/examples

## Implementation Plan

### Phase 1: Coverage Infrastructure

- [x] Add a test that every `dragongui.__all__` symbol appears in either
  `dg.help.to_dict()` or a documented exception list.
- [x] Add a test that every `_SUPPORTED_PARTS_BY_KIND` entry appears in
  `dg.help.reference.css_parts`.
- [x] Add a test that key aliases resolve:
  - `dg.help.css`
  - `dg.help.parts`
  - `dg.help.scatter`
  - `dg.help.tables`
  - `dg.help.dialogs`
  - `dg.help.drag_drop`
- [x] Add a small helper in tests to flatten manual bodies/titles/summaries for
  coverage checks.

### Phase 2: Public Export Reference

- [x] Add `reference.exports` with a grouped inventory of `dragongui.__all__`.
- [x] Add `reference.runtime` for:
  - `BackendUnavailableError`
  - `backend_info`
  - `native_backend_available`
  - `ThreadMonitor`
  - `register_thread_role`
  - `thread_role`
- [x] Add `reference.dialogs` for:
  - `FileDialog`
  - `open_file_dialog`
  - `open_files_dialog`
  - `save_file_dialog`
  - `pick_folder_dialog`
  - `alert`
  - `confirm`
- [x] Add `reference.components` for:
  - `component`
  - `ComponentCtx`
  - `ComponentInstance`
  - `StateSlot`
  - `VNode`
  - `Patch`
  - `ResourceRef`
- [x] Add `reference.dataclasses` for callback payloads and serialized payload
  helpers:
  - `PropertyChange`
  - `BreadcrumbItem`
  - `BreadcrumbSelection`
  - `TableSelection`
  - `TableSort`
  - `HeatmapCell`
  - `BarChartBar`
  - `DragDropPayload`
  - `ScatterPick`
  - `ScatterHit`
  - `ScatterPayload`
  - `LinePlotPayload`
  - `HistogramBins`
  - `BarChartData`
  - `ScatterStreamMetrics`
  - `ScatterFrameStream`
  - `ScatterLiveFrame`

### Phase 3: Per-Widget Reference Leaves

- [x] Add per-widget sections for layout and app-shell widgets:
  - `Window`, `HLayout`, `VLayout`, `ScrollArea`, `GridLayout`,
    `FlowLayout`, `Splitter`, `Pane`, `Panel`, `Collapsible`, `Modal`,
    `Separator`, `Spacer`, `Sidebar`, `StatusBar`
- [x] Add per-widget sections for navigation and menus:
  - `MenuBar`, `Menu`, `MenuItem`, `ContextMenu`, `Tooltip`, `Tabs`, `Tab`,
    `Pages`, `Page`, `NavItem`, `Toolbar`, `ToolbarSeparator`, `Breadcrumbs`,
    `SearchBox`, `Command`, `CommandPalette`
- [x] Add per-widget sections for controls:
  - `Label`, `Badge`, `Tag`, `LED`, `Button`, `SmallButton`, `IconButton`,
    `ImageButton`, `ArrowButton`, `Selectable`, `SelectableList`,
    `RadioButton`, `RadioGroup`, `TextInput`, `TextArea`, `CodeEditor`,
    `LogView`, `DateInput`, `TimeInput`, `DateTimeInput`, `Slider`,
    `RangeSlider`, `ProgressBar`, `LoadingSpinner`, `NumberInput`,
    `DragNumber`, `DragVector`, `Property`, `PropertyGrid`, `Dropdown`,
    `Checkbox`, `ToggleSwitch`, `ColorPicker`
- [x] Add per-widget sections for data/media/plots:
  - `Image`, `HtmlReport`, `DataFrameTable`, `Histogram`, `BarChart`,
    `Heatmap`, `LinePlot`, `PieChart`, `Scatter3D`, `ScatterPlot2D`
- [x] Add per-widget sections for drag/drop and extension hooks:
  - `DragSource`, `DropTarget`, `DropZone`, `ExtensionWidget`

### Phase 4: Live Methods And Callback Matrix

- [x] Add `reference.live_methods` grouped by widget family.
- [x] Document common live methods:
  - `set_style`
  - `set_value`
  - `set_checked`
  - `set_frame`
  - `set_data`
  - `append_points`
  - `set_points`
  - `set_prepared_points`
  - `enqueue_prepared_points`
  - `create_live_frame`
  - `fit`
  - `show`
  - `close`
  - `toast_handle.update`
  - `toast_handle.dismiss`
- [x] Add `reference.callbacks` with callback argument forms:
  - `on_click`
  - `on_change`
  - `on_select`
  - `on_sort`
  - `on_hover`
  - `on_pick`
  - `on_drop`
  - `Command.on_run`
  - `CommandPalette.on_run`
- [x] Include guidance on callback cost, thread scheduling, and live update
  coalescing.

### Phase 5: CSS Reference Synchronization

- [x] Add `reference.css_parts` generated or checked from
  `_SUPPORTED_PARTS_BY_KIND`.
- [x] Add `reference.css_type_selectors` from `docs/widgets-reference.md`.
- [x] Add `reference.css_states` covering supported pseudo states:
  - `:hover`
  - `:focus`
  - `:active`
  - `:disabled`
  - `:checked`
  - `:selected`
  - `:open`
  - `:expanded`
  - `:collapsed`
- [x] Add `reference.css_properties` with the supported property groups:
  layout, grid, overflow, visual, text, transforms, transitions, plot-specific.
- [x] Add `reference.css_limits` for unsupported browser CSS features.
- [x] Decide whether CSS reference content should be hand-maintained in
  `manual.py` or generated from docs/tests. Current approach: registered parts
  are read from `_SUPPORTED_PARTS_BY_KIND`; selector/state/property guidance is
  curated in `manual.py` and covered by tests.

### Phase 6: Recipes And Examples

- [x] Add `recipes.app_shell` for menu/body/status layouts.
- [x] Add `recipes.settings_panel` for property grids and manual forms.
- [x] Add `recipes.streaming_line_plot`.
- [x] Add `recipes.streaming_scatter`.
- [x] Add `recipes.data_table_browser`.
- [x] Add `recipes.drag_drop`.
- [x] Add `recipes.command_palette`.
- [x] Add `recipes.custom_composite`.
- [x] Add `recipes.pytorch_dashboard` based on
  `examples/pytorch_training_dashboard.py`.
- [x] Add links or file paths to relevant probes in each recipe.

### Phase 7: Machine-Readable Metadata

- [x] Add `schema_version` to `dg.help.to_dict()`.
- [x] Add `library_version` from `dragongui.__version__` or equivalent.
- [x] Add stable `path` fields in every serialized section.
- [x] Add optional compact mode:
  `dg.help.to_dict(include_body=False)`.
- [x] Add `dg.help.find_symbol("NumberInput")` or improve `search` so symbol
  lookup is deterministic.
- [x] Consider adding `dg.help.reference.widgets.to_dict()` where widget leaves
  expose signatures as fields instead of prose only.

### Phase 8: Documentation Sync

- [x] Update `docs/widgets.md` to mention `dg.help`.
- [x] Update `docs/widgets-reference.md` to mention that `dg.help` is the
  in-library LLM reference.
- [x] Add a short example under `examples` or `examples/css_feature_probes`
  demonstrating `print(dg.help("widgets.plots.scatter"))`.
- [x] Decide whether generated docs should be emitted from `dg.help.to_dict()`
  or kept as source-of-truth markdown. Current approach: keep docs hand-authored
  and use `dg.help.to_dict()` as the structured runtime source for tools.

## Acceptance Criteria

- `dg.help.search("NumberInput")` returns a NumberInput reference as the first
  result.
- `dg.help("reference.widgets.number_input")` includes signature, purpose,
  callbacks, live methods, CSS parts, validation notes, and related widgets.
- Every public `dragongui.__all__` symbol is covered or intentionally ignored
  with a test-backed exception.
- Every supported CSS part in `_SUPPORTED_PARTS_BY_KIND` is represented.
- The full Python API test suite passes.
- A separate manual coverage test fails when a new public widget/export is
  added without updating `dg.help`.
- The manual remains useful as prose while also being parseable through
  `to_dict()`.

## Current Follow-Up Targets

- [x] Sanitize generated signatures so public references do not include quoted
  forward annotations or unstable object memory addresses.
- [x] Add curated parameter notes for high-value widgets where raw signatures
  are not enough for LLM code generation.
- [x] Add `dg.help.decisions` / `dg.help.choose` for use-this-vs-that guidance.
- Keep enriching special-case reference leaves when new public APIs are added.
  Generic signature coverage prevents missing exports, but LLM usability still
  benefits from curated details for complex widgets.
- Continue adding probe/example paths to high-value widget references.
- Add focused tests whenever a manual section describes a new public callback
  shape or payload type.
- Revisit generated docs only after `dg.help.to_dict()` stabilizes as the
  structured runtime source.
