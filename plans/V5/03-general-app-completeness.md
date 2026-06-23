# V5 General App Completeness Plan

Status: draft.

## Objective

Move DragonGUI from a strong data-app/dashboard toolkit toward a credible
general desktop application toolkit.

The goal is not to become a full PyQt/PySide replacement in V5. The goal is to
close the highest-leverage gaps that prevent DragonGUI from being used for
serious long-lived desktop applications outside narrow visualization workflows.

Target outcome:

- DragonGUI remains GPU-native and data-tool focused.
- Users can build larger app shells with predictable lifecycle behavior.
- Core controls feel complete under mouse and keyboard.
- Text, clipboard, tables, trees, and platform integration are good enough for
  internal production tools.
- Remaining gaps are documented as explicit non-goals or future work.

## Current Position

DragonGUI already has a substantial foundation:

- Single-window native runtime through Rust/winit/wgpu.
- Flat Python API with context-manager widget construction.
- Reactive component model and VDOM diffing.
- Live widget handles and thread-safe command scheduling.
- Layout containers: flex rows/columns, responsive grids, flow layout,
  splitters, scroll areas, panels, pages, tabs, sidebars, and status bars.
- App chrome: menu bars, menus, context menus, toolbars, modals, tooltips,
  toasts, native file dialogs, and loading screens.
- Workflow widgets: `PropertyGrid`, `SearchBox`, `CommandPalette`,
  `Breadcrumbs`, `Tabs`, `Pages`, `Sidebar`, and `NavItem` provide much of the
  app-shell surface already.
- Data widgets: `DataFrameTable`, `Scatter3D`, `ScatterPlot2D`, `LinePlot`,
  `Histogram`, `BarChart`, `Heatmap`, and `PieChart`.
- Application widgets: tree view, property grid, command palette, code editor,
  log view, drag/drop zones, date/time inputs, color picker, images, and HTML
  report embedding.
- Custom extension surfaces: `ExtensionWidget`, `PaintWidget`, display-list
  painting, pointer/wheel/focused-key callbacks, and related probes.
- CSS-like styling with selectors, parts, variables, queries, gradients,
  transitions, and first-slice animation support.
- Thread-safe Python task scheduling, live resource upload/release, thread
  diagnostics, and `ThreadMonitor`.
- Runtime diagnostics, debug snapshots, and a broad probe/example suite.
- Built-in `dg.help` manual with structured `to_dict()` output, symbol coverage
  tests, CSS part coverage tests, recipes, troubleshooting, and probe links.

Important gaps remain compared with mature general-purpose GUI frameworks:

- App/window lifecycle is still shallow.
- Component lifecycle is stateful and reactive, but lacks generic effect,
  cleanup, timer, or service-lifecycle APIs.
- Widget APIs have broad coverage but limited depth in tables, trees, text, and
  editable data workflows.
- There is no generic model/view framework.
- Clipboard, text editing, IME, Unicode, accessibility, and localization scope
  need a deliberate pass.
- Platform integration is only partial.
- Styling/layout behavior is powerful but still first-slice in several areas.
- Packaging, compatibility, and regression testing need a stable release lane.

## Whole-Plan Code Audit Snapshot

This audit covers the current Python API, Rust native backend, docs, tests, and
examples. It is meant to keep the V5 plan from duplicating foundations that
already exist.

Legend:

- Covered: usable current functionality that V5 can build on.
- Partial: present but not complete enough for the V5 product bar.
- Missing: still needs design and implementation.

### Workstream 1 - Application Lifecycle

Covered:

- `App.request_exit()` and `AppHandle.request_exit()` enqueue native exit.
- Native `WindowEvent::CloseRequested` exits the event loop.
- Native resize, scale-factor, theme-change, keyboard, modifier, mouse, wheel,
  and IME commit events are handled internally.
- `run_with_loading(...)` and startup timing capture already exist.
- Startup window size is clamped to monitor bounds.
- `App.call_soon_threadsafe()` and `AppHandle.call_soon_threadsafe()` schedule
  Python work onto the UI/runtime path.
- Python task drain timing, last-task status, queued-task depth, and pending
  native-command counts are included in debug snapshots.
- Components support keyed state, VDOM diffing, attach/detach, child-runtime
  pruning, and live callback rebinding.
- `ThreadMonitor`, diagnostics collectors, thread roles, task failure capture,
  and queue-depth sampling already exist.
- `ThreadMonitor` demonstrates built-in widget-owned worker cleanup on detach
  and has tests for stopping and pausing refresh threads.
- `App.set_buffer_resource(...)` and `App.release_resource(...)` expose retained
  native resource management for large/live data workflows.

Partial:

- `Window` currently serializes startup `title`, `width`, and `height`; live
  title/size/fullscreen/icon mutation commands are not present.
- Window events update native runtime state but are not exposed as Python
  callbacks.
- Shutdown is one-way: close/request-exit can exit, but Python cannot cancel
  close or run a public shutdown policy.
- `debug_snapshot()` exposes runtime details, but not a public lifecycle event
  history.
- Background work can be scheduled and observed, but ad hoc daemon threads are
  still common in examples and file-dialog callbacks; there is no public
  app-service/timer abstraction, task handle, or managed cancellation model.
- Component `detach()` unbinds widgets and children, but `ComponentCtx` exposes
  only `app` and keyed `state(...)`; there is no public component effect,
  cleanup, timer, or service hook for user-owned threads, file watchers, timers,
  or external resources.

Missing:

- Public close, resize, focus, theme-change, startup, and shutdown callbacks.
- Window mutation APIs for title, icon, size, minimum size, fullscreen, and
  persisted geometry.
- Multi-window lifecycle.
- Public component lifecycle hooks such as mount, after-render, cleanup, and
  effect-like dependencies.
- Managed timers, repeating tasks, cancellable background jobs, and structured
  background-task error reporting.

Plan adjustment:

- Treat V5 lifecycle as public event/callback and window-command exposure, not
  as basic native event intake.
- Treat background work as lifecycle polish. The primitive queue exists; V5
  should add managed app-service patterns and cleanup contracts.

### Workstream 2 - Platform Integration

Covered:

- Native file dialogs exist for open, open-multiple, save, and folder pick.
- Dialog helpers support synchronous returns and callback-style selection.
- App-local drag/drop exists through `DragSource`, `DropTarget`, and `DropZone`.
- Toast overlays exist with update/dismiss handles.
- `HtmlReport.open_external()` can open its report path or URL through Python's
  browser launcher.
- `HtmlReport` has native Windows WebView2 embedding plumbing, runtime sync,
  fallback/disabled reasons, and unsupported-platform snapshot data.

Partial:

- `CommandPalette` provides searchable command actions, but it is not connected
  to a global shortcut registry.
- `backend_info()` exists, but native output is currently limited to
  name/native/renderer/status/layout/text and fallback output is limited to
  name/native/renderer/status.
- `HtmlReport` embedding is optional and Windows/WebView2-specific; docs and
  support diagnostics need a polished feature-availability story.
- File-dialog callbacks can be scheduled through `app.call_soon_threadsafe()`
  when an app is passed, but the helper returns no cancellable task handle and
  has no lifecycle/status reporting.
- Clipboard behavior appears only in app-specific example code, not as a
  DragonGUI package API.
- Drag/drop is app-local payload transfer, not OS file or MIME drag/drop.
- Native runtime event handling does not currently cover winit file-hover or
  file-drop window events.

Missing:

- Clipboard APIs and standard edit commands.
- OS file drop and MIME drag/drop.
- Global shortcuts/accelerators with platform display strings.
- Top-level `dg.open_external(...)`.
- System tray, printing, and true native OS notifications.

Plan adjustment:

- Reuse the existing dialogs, toast, command-palette, and app-local drag/drop
  surfaces. The new work is OS integration and standard command plumbing.

### Workstream 3 - Text Editing, IME, And Unicode

Covered:

- `TextInput`, `TextArea`, `CodeEditor`, `LogView`, and `NumberInput` are
  implemented.
- Native text rendering uses glyphon/cosmic-text with advanced shaping and a
  shared font system foundation.
- Text widgets keep native value and cursor state.
- Backspace, Delete, ArrowLeft/Right, Home, End, printable text insertion, and
  multiline Enter are implemented.
- Text insertion, deletion, and cursor movement clamp to UTF-8 character
  boundaries rather than arbitrary byte offsets.
- Multiline text widgets support ArrowUp/Down cursor movement and
  scroll-to-caret behavior.
- IME commit text is routed into focused text widgets, and native IME cursor
  area is updated for focused text fields.
- `NumberInput` has internal invalid-number state.

Partial:

- `CodeEditor` is a source-like multiline editor, not a full editor component.
- Home/End behavior is currently basic start/end movement, not a complete
  platform-specific line/paragraph navigation model for multiline editing.
- IME support covers commit text, but not visible composition/preedit UI.
- Renderer shaping is stronger than the editor contract: bidi/RTL and
  complex-script behavior still need an explicit widget-level audit.
- Validation state exists internally for number parsing, but not as a public
  field validation API.

Missing:

- Text selection.
- Copy, cut, paste, select-all, undo, and redo.
- Password/secret text mode.
- Public validation state such as `set_invalid(...)` and `clear_invalid()`.
- Bidi/RTL and complex-script behavior audit.

Plan adjustment:

- Keep the workstream focused on desktop editing semantics. Basic text storage,
  caret movement, and IME commit are already in place.

### Workstream 4 - Accessibility And Keyboard Operation

Covered:

- Native focus order is tracked in `WidgetState`.
- `debug_snapshot()` includes focus-order, focused-widget, hover, active, and
  selection diagnostics useful for keyboard audits.
- Tab and Shift+Tab traverse visible focusable widgets.
- Focus rings and focused-state styling are rendered.
- Escape closes popups and active modals.
- Enter/Space activate common controls, including buttons, checkboxes,
  toggles, selectables, collapsibles, dropdowns, menus, tabs, nav items, tree
  nodes, and tables.
- Arrow/Home/End keyboard behavior already exists for several controls:
  dropdowns, radio groups, sliders, range sliders, tabs, nav items, tree nodes,
  and tables.
- Scroll containers support PageUp, PageDown, Home, and End keyboard scrolling.
- Tables and trees have native keyboard selection/navigation paths.

Partial:

- CSS media hooks for contrast/reduced-motion style decisions exist, but the
  platform accessibility bridge is not implemented.
- Keyboard behavior is broad but not governed by an accessibility role/state
  model.
- There is no public focus API or focus-order override.
- Top-level menu keyboard opening exists, but full menu-item traversal,
  accelerator display, checked/radio item semantics, and shortcut integration
  are not implemented as a desktop menu contract.

Missing:

- Accessibility labels/descriptions on widgets.
- Roles, names, states, bounds, and focusability in a testable tree.
- Platform screen reader bridge.
- Native accessibility bridge dependencies such as AccessKit are not currently
  present in the Rust backend.
- Accessibility tree/snapshot in `debug_snapshot()` beyond current focus-state
  diagnostics.

Plan adjustment:

- Split keyboard hardening from accessibility metadata. Keyboard operation is
  substantially started; accessibility tree and platform bridge remain new work.

### Workstream 5 - Tables, Trees, And Model/View Foundations

Covered:

- `DataFrameTable` supports `sortable`, `resizable_columns`, `on_select`, and
  `on_sort`.
- Native table state tracks scroll row/column, selected cell, sort state,
  source row order, and per-column resized widths.
- Header clicks toggle sort, and column divider drags resize columns.
- Focused tables support Arrow navigation, PageUp/PageDown, Home/End, and
  Enter/Space selection emission.
- `TreeView` and `TreeNode` support nested data, selection, expansion,
  callbacks, disabled state, CSS parts, and context-manager construction.
- Native tree keyboard handling supports select, expand/collapse, and visible
  neighbor navigation.
- `SelectableList` already covers a multiple-selection list use case.
- API tests cover table selection/sort payloads, tree selection/expand payloads,
  and `SelectableList` single/multiple selection state.

Partial:

- DataFrame extraction and column buffers are optimized, but they are not a
  generic model/view protocol.
- Table resizing exists interactively, but public per-column APIs, visibility,
  ordering, and frozen columns do not.
- Current table selection is centered on selected-cell flow.
- Tree data is static after construction except normal child replacement; lazy
  loading, rename/editing, and tree multi-select are still model/workflow
  features rather than current tree basics.

Missing:

- `DataModel` protocol and adapters.
- Editable table cells with commit/cancel callbacks.
- Row, cell, multi-row, and multi-cell table selection modes.
- Lazy tree loading, tree rename/editing, and tree multi-select.
- Proxy sort/filter helpers.

Plan adjustment:

- Preserve optimized `DataFrameTable` paths. V5 should add model/view depth
  around them instead of replacing them.

### Workstream 6 - Widget Depth And App Controls

Covered:

- `MenuBar`, `Menu`, `MenuItem`, and `ContextMenu` exist.
- `Modal`, modal close buttons, `show()`, `close()`, and Escape close exist.
- `alert(...)` and `confirm(...)` helpers exist.
- `PropertyGrid`, `Property`, and `PropertyChange` provide schema-driven
  settings/property inspectors with typed editors, sections, read-only rows,
  disabled state, programmatic setters, and change events.
- `PropertyGrid` has type coercion and editor-level constraints for built-in
  editors, but not a form validation/error-summary lifecycle.
- `SearchBox`, `CommandPalette`, `Toolbar`, `ToolbarSeparator`,
  `Breadcrumbs`, date/time inputs, `ColorPicker`, `RadioGroup`, and
  `SelectableList` exist and are covered by API tests/probes.
- `Tabs`, `Pages`, `Sidebar`, and `NavItem` provide route-like page state,
  callbacks, badges, disabled state, and probe coverage.
- `PaintWidget` and `ExtensionWidget` provide custom drawing and
  pointer/wheel/focused-key hooks with probe coverage.
- Modal rendering and hit testing block normal background pointer interaction.

Partial:

- Menus are clickable app menus with disabled items, but `Menu` and
  `ContextMenu` currently accept only `MenuItem` children, so checked/radio
  items, menu separators, shortcut labels, and full desktop menu traversal need
  explicit menu APIs.
- Modal behavior blocks background interaction, but formal focus trapping needs
  explicit tests and policy.
- Forms can be assembled from field widgets and `PropertyGrid`, but there is no
  form-level validation, submission, dirty/touched, or cross-field helper layer.
- Navigation widgets have route-like values and callbacks, but no history,
  router/deep-linking, or shortcut integration layer.
- Custom widget hooks are useful for leaf controls, but they are not a full
  graphics scene, plugin ABI, or gesture framework; pointer capture, key-up, and
  higher-level drag semantics remain outside the current foundation.

Missing:

- Checked/radio menu items, menu separators, shortcut labels, and full keyboard
  menu traversal.
- Input and progress dialogs.
- Field validation framework, validation summaries, dirty/touched state, and
  submit/cancel helpers.
- Page history/router helper and deep-link route-state policy.
- Command palette shortcut integration.

Plan adjustment:

- The widget set is broad. V5 should deepen desktop interaction contracts rather
  than add many unrelated controls.
- Treat `PropertyGrid`, `SearchBox`, command palette, and navigation controls as
  foundations. The V5 work is workflow-level behavior on top of them.

### Workstream 7 - Styling And Layout Hardening

Covered:

- CSS-like styling supports selectors, parts, state pseudos, variables, media
  queries, gradients, transforms, transitions, overflow, and scrollbars.
- `debug_snapshot()` includes layout rectangles, clips, scroll extents,
  computed styles, resources, renderer data, and CSS warnings.
- `docs/layout.md` documents layout diagnostics and debug-snapshot usage.
- `tools/smoke_css_demos.py --strict-layout` can audit demos for likely
  clipping/container overflow.
- The smoke tool parses `debug_snapshot()` output and default-runs the top-level
  CSS demos with `DRAGONGUI_SMOKE_FRAMES`.
- The probe suite already covers many app-shell, overflow, typography, overlay,
  selector, and responsive layout cases.
- `examples/css_feature_probes` currently contains 71 Python files, including
  many focused probes and benchmark probes.

Partial:

- Diagnostics expose rich state, but not all V5 failure modes as first-class
  warnings.
- `tools/smoke_css_demos.py` is not part of CI or a release gate, and its
  default set does not exercise the nested `examples/css_feature_probes`
  coverage.
- `examples/css_feature_probes/README.md` lists a suggested order ending at 53
  probes, so it is not a complete inventory of current probe files.
- CSS capability docs document support, but stable/experimental/unsupported
  labels are not consistently productized.
- Debug data is textual/snapshot based; there is no per-widget visual debug
  overlay.

Missing:

- First-class warnings for clipped text, zero-sized interactive widgets, and
  unresolved or suspicious sizing.
- Per-widget layout debug overlay.
- Stable app-shell recipe set tied to the smoke/probe suite.

Plan adjustment:

- Build on the existing snapshot and smoke infrastructure. The V5 work is
  warning quality, support labeling, and recipe coverage.

### Workstream 8 - Packaging, Compatibility, And Release Discipline

Covered:

- `pyproject.toml` is configured for maturin/PyO3 packaging.
- The package includes third-party notices in wheel metadata.
- GitHub Actions run Python tests, cross-platform Rust `cargo check`, Windows
  native build/import smoke, and macOS native build/import/window smoke.
- Python version classifiers cover 3.11 through 3.13.
- `backend_info()` exists with native/fallback status.
- `dg.help.to_dict()` includes `schema_version` and `library_version`.
- Help coverage tests fail when exported public symbols or exported class
  members are missing from the manual.
- CSS part coverage tests compare `dg.help` against the widget CSS part
  registry.

Partial:

- CI proves builds and imports, but does not build/publish a wheel artifact
  matrix.
- CI currently uses Python 3.12 for the core jobs, so it does not prove all
  classified Python versions.
- CI uses cross-platform `cargo check`, not the Rust test suite; native runtime
  smoke is also uneven across platforms, with macOS covering a short window
  smoke and Windows covering build/import only.
- CI installs only `pytest` for Python tests even though tests and probes import
  NumPy in many data-widget paths; dependency availability should not depend on
  the runner image.
- `pyproject.toml` includes top-level, V2, and V3 plans plus top-level examples
  in sdists, but currently misses V4/V5/RPi/nested reactive-engine plan docs
  and nested `examples/css_feature_probes/*` assets.
- `pyproject.toml` has `dataframe` and `dev` extras, but no explicit
  test/probe/examples extra for NumPy/Pillow/SciPy/Plotly/PyTorch-style example
  prerequisites.
- Smoke tooling exists, but it is not wired into CI, release-gate jobs, or
  artifact validation.
- Packaging docs exist in overview form, but there is no release checklist doc
  or release-gate command.
- `backend_info()` needs OS, Python, backend, adapter, and feature details for
  support tickets.
- `dg.help` is strong for current APIs, but V5 APIs will need manual sections
  and symbol coverage as they land.
- Help metadata links curated examples/probes for many widgets, but current
  tests do not prove that the full filesystem probe/example inventory is
  documented or shippable.

Missing:

- Wheel build matrix for supported platforms and Python versions.
- Wheel install smoke from built artifacts.
- Rust test execution in the release gate, with platform-specific skips called
  out explicitly.
- Native runtime smoke policy for Windows, macOS, and Linux.
- Public support matrix.
- Semantic versioning, compatibility, and deprecation policy.
- Declared dependency groups or requirements for core tests, data-widget tests,
  visual probes, and heavyweight optional examples.
- sdist/wheel content audit for docs, current plan subdirectories, nested
  probes, examples, notices, and package metadata.
- Automated release checklist that covers tests, smoke probes, examples, docs,
  and `dg.help`.

Plan adjustment:

- Keep the maturin/CI foundation. V5 release work should focus on artifact
  production, support policy, and install-time validation.

### Documentation Drift Found During Audit

- `docs/widgets.md`, `docs/widgets-reference.md`, and
  `docs/library-overview.md` understate `DataFrameTable`; they should document
  `sortable`, `resizable_columns`, and `on_sort`.
- `docs/widgets.md` and `docs/widgets-reference.md` also omit or understate
  newer workflow composites such as `PropertyGrid`, `SearchBox`,
  `CommandPalette`, `Breadcrumbs`, date/time inputs, and `HtmlReport`, even
  though `dg.help` already covers many of them.
- `docs/library-overview.md` still lists descendant selectors, media queries,
  transitions, and scoped CSS variables as absent, while current CSS docs and
  implementation support first-slice versions of those features.
- `docs/library-overview.md` says there is no webview frontend without
  distinguishing that `HtmlReport` can optionally embed local/inline HTML on
  Windows through WebView2 with fallback/unsupported states.
- README and `docs/library-overview.md` understate optional dependency usage:
  NumPy is used broadly by data-widget tests/probes, while Pillow/SciPy/Plotly
  or PyTorch appear in narrower image/report/benchmark/example paths.
- README says `examples/` contains 28 runnable demos, but the current tree has
  33 top-level example scripts and 104 Python files recursively when feature
  probes are included.
- `docs/library-overview.md` lists an older subset of examples and does not
  reflect the expanded V3/all-features, Web/CSS capability, benchmark, and
  feature-probe coverage.
- Platform docs should distinguish app-local drag/drop from OS file/MIME
  drag/drop.
- `examples/css_feature_probes/README.md` should be regenerated or curated
  against the filesystem inventory; it currently omits newer probe files such
  as data-table upgrades, drag/drop, grid tracks, HtmlReport, splitters, thread
  monitor, tool buttons, and camera/scroll benchmark probes.
- Extension widget docs correctly note current limits: no pointer capture,
  higher-level drag gestures, or key-up events.

## Non-Goals

V5 should not attempt to clone Qt.

Explicit non-goals:

- Full browser CSS compatibility.
- Full rich-text document editing.
- A native dynamic plugin ABI.
- Full Qt-style graphics scene parity.
- Full accessibility parity across all platforms in one pass.
- A full translation/resource-bundle/pluralization framework unless explicitly
  promoted into V5 scope.
- Replacing specialized data-widget APIs with only generic abstractions.

## Product Bar

DragonGUI should be considered generally app-capable when these are true:

- A user can build a multi-page desktop workbench without custom native code.
- The app can manage startup, shutdown, window state, dialogs, menus,
  shortcuts, clipboard, and background work through public APIs.
- Tables and trees support common enterprise workflows: selection, sorting,
  column sizing, editing hooks, keyboard navigation, and context actions.
- Text inputs support expected desktop editing behavior, including copy/paste,
  selection, undo/redo, and international text basics.
- Keyboard-only operation is practical for standard controls.
- Localization limits are explicit: either app-owned strings and formatting are
  documented as the V5 policy, or a small locale API is defined and tested.
- Styling and layout failures are predictable, reported, and covered by probes.
- The docs state what is stable, what is experimental, and what is unsupported.

## Workstream 1: Application Lifecycle

Objective: make app shells predictable and suitable for long-running tools.

### API Targets

- `Window.on_close`, `on_resize`, `on_focus`, and `on_theme_change` callbacks.
- `Window.set_title(...)`.
- `Window.set_size(...)`, `set_min_size(...)`, and `set_fullscreen(...)`.
- Window icon support from PNG/ICO paths.
- Persist/restore window geometry helper.
- App shutdown callbacks:
  - `App.on_startup(...)`
  - `App.on_shutdown(...)`
  - `App.request_quit()`
- App-level keyboard shortcut registry.
- Public lifecycle events in `debug_snapshot()`.
- Component lifecycle hooks:
  - `ctx.on_mount(...)`
  - `ctx.on_cleanup(...)`
  - optional effect-like helper keyed by dependency values
- Managed runtime helpers:
  - one-shot timer
  - repeating timer
  - cancellable background job wrapper
  - structured task-error callback or diagnostics stream

### Implementation Notes

- Keep the first milestone single-window. Multi-window support should be a
  follow-up unless the runtime shape makes it cheap.
- Window callbacks must be queued through the same Python callback path as
  widget events.
- Close handling should support canceling the close when user confirmation is
  needed.
- Persisted geometry should be a helper, not hidden global behavior.
- Managed background helpers should reuse `call_soon_threadsafe()` and the
  existing diagnostics collector instead of introducing a second task path.
- Component cleanup must run on component detach and app shutdown.
- File-dialog callback threads should either become managed tasks or be
  explicitly documented as fire-and-callback helpers.

### Acceptance Criteria

- [ ] Example: `examples/app_lifecycle_tool.py`.
- [ ] Example: component cleanup of timer/background worker.
- [ ] Probe: close confirmation, resize event, title update, fullscreen toggle.
- [ ] Tests for Python serialization and command validation.
- [ ] Native tests for close-event policy and geometry state.
- [ ] Tests for component cleanup, cancelled timers, and task-error reporting.
- [ ] `dg.help` lifecycle section updated.

## Workstream 2: Platform Integration

Objective: cover the boring desktop features that make apps feel native.

### API Targets

- Clipboard:
  - `dg.clipboard.get_text()`
  - `dg.clipboard.set_text(text)`
  - `App.clipboard_text()`
  - `App.set_clipboard_text(text)`
- Standard edit commands:
  - copy, cut, paste, select all
  - per-widget enablement state where possible
- File drag-in from the OS onto `DropTarget`/`DropZone`.
- Global app shortcuts with platform-specific display text.
- Open path/URL helper:
  - `dg.open_external(path_or_url)`
- System tray and notifications should be researched but not required for V5.

### Implementation Notes

- Clipboard should use native APIs through Rust where possible.
- Text widgets, tables, code editor, and log view should integrate with
  standard edit commands.
- OS file drag-in can map to the existing drag/drop payload model.
- Shortcut handling must not bypass focused text editing.
- File-dialog callback behavior should be part of the lifecycle policy: either
  expose cancelable dialog jobs or document the current daemon-thread callback
  helper as intentionally lightweight.

### Acceptance Criteria

- [ ] Clipboard probe covering text input, text area, code editor, table cell,
  and log view.
- [ ] OS file-drop probe.
- [ ] Shortcut conflict tests for focused text widgets.
- [ ] Docs list platform support per feature.

## Workstream 3: Text Editing, IME, And Unicode

Objective: make text controls credible for real applications.

### API Targets

- Text selection in `TextInput`, `TextArea`, and `CodeEditor`.
- Copy/cut/paste/select-all behavior.
- Undo/redo stacks for editable text widgets.
- Home/End/PageUp/PageDown behavior aligned with platform expectations.
- Password/secret input mode for `TextInput`.
- Optional validation state:
  - `set_invalid(message=None)`
  - `clear_invalid()`
- IME composition event support where winit exposes it.
- Basic bidi/RTL and complex-script audit.
- Localization scope decision:
  - document that labels, translations, date/number formatting, and plural rules
    are app-owned in V5, or define a deliberately small locale/formatting API
  - verify that text controls do not block future bidi/RTL and font fallback
    work

### Implementation Notes

- Keep `TextInput` and `TextArea` simple but correct before expanding
  `CodeEditor`.
- IME support should degrade gracefully with explicit platform notes.
- Rich text is out of scope; plain text must be solid.
- Text selection should expose enough state for copy and accessibility work.
- If full localization remains out of scope, docs should say so directly and
  list the supported international-text basics.

### Acceptance Criteria

- [ ] Text editing probe with keyboard-only script.
- [ ] Unit tests for text mutation commands.
- [ ] Native input tests for selection, cursor movement, and undo/redo.
- [ ] Manual section documenting supported keyboard shortcuts.
- [ ] Docs state V5 localization policy and known bidi/RTL limitations.

## Workstream 4: Accessibility And Keyboard Operation

Objective: make standard app usage possible without a mouse and lay groundwork
for screen reader support.

### API Targets

- Accessible labels:
  - `aria_label` or `accessibility_label`
  - `accessibility_description`
- Explicit focus order override.
- Public `focus()` method on focusable widgets.
- Focus traversal through all interactive controls.
- Keyboard activation for all button-like controls.
- Keyboard selection for dropdowns, menus, tabs, pages, trees, tables, and
  command palette.
- High-contrast and reduced-motion theme hooks.
- Accessibility snapshot for testing:
  - role
  - name
  - state
  - bounds
  - focusable

### Implementation Notes

- Start with an internal accessibility tree even before platform bridges are
  complete.
- Use the accessibility snapshot to enforce naming and focus coverage.
- Screen reader bridges can be staged by platform after the internal tree is
  reliable.

### Acceptance Criteria

- [ ] Accessibility tree exported in `debug_snapshot()`.
- [ ] Keyboard traversal probe for all core controls.
- [ ] Tests that every focusable widget has role/state metadata.
- [ ] Docs for keyboard operation and accessibility labels.

## Workstream 5: Tables, Trees, And Model/View Foundations

Objective: deepen data/navigation widgets without forcing every app into custom
widget code.

### API Targets

- `DataModel` protocol for table/tree-like data:
  - row count
  - column count
  - cell value
  - optional role data
  - optional editing
  - optional sorting/filtering hooks
- `DataFrameTable` column controls:
  - per-column width
  - column visibility
  - column order
  - frozen index/columns research
- Editable table cells with commit/cancel callbacks.
- Table selection modes:
  - row
  - cell
  - multi-row
  - multi-cell
- Tree editing hooks:
  - rename
  - lazy load children
  - multi-select
  - keyboard expand/collapse
- Proxy helpers for sort/filter where feasible.

### Implementation Notes

- Do not remove the optimized dataframe paths. A generic model should coexist
  with specialized packed-data APIs.
- Start with a Python protocol and adapters rather than a heavy inheritance
  hierarchy.
- Native virtualization should remain responsible for large views.

### Acceptance Criteria

- [ ] Model protocol documented with table and tree examples.
- [ ] Editable table probe.
- [ ] Lazy tree probe.
- [ ] Tests for column sizing, sorting, selection modes, and edit callbacks.
- [ ] `dg.help` model/view guide added.

## Workstream 6: Widget Depth And App Controls

Objective: raise common controls from demo-ready to app-ready.

### Priorities

- Menus:
  - checked menu items
  - radio menu groups
  - separators or a menu-specific separator item using the existing
    `Separator` rendering path
  - keyboard navigation
  - shortcut labels
- Dialogs:
  - message boxes
  - input dialog
  - progress dialog
  - non-blocking result callbacks
- Forms:
  - form/controller layer over existing field widgets and `PropertyGrid`
  - field validation
  - validation summary
  - dirty/touched tracking
  - form submit/cancel helpers
  - disabled/read-only consistency
- Navigation:
  - breadcrumbs keyboard support
  - page history/router helper
  - route-state/deep-linking policy for `Pages`, `Tabs`, `Sidebar`, and
    `Breadcrumbs`
  - command palette shortcut integration
- Overlays:
  - z-order policy
  - modal focus trap
  - escape handling

### Acceptance Criteria

- [ ] Core control probe includes keyboard navigation and state transitions.
- [ ] Modal focus trap test.
- [ ] Menu keyboard, separator, shortcut-label, checked-item, and radio-item
  tests.
- [ ] Form validation example.
- [ ] PropertyGrid/form workflow probe covering validation, dirty state, and
  submit/cancel behavior.
- [ ] Navigation history probe using Pages, Sidebar/NavItem, Tabs, and
  Breadcrumbs together.

## Workstream 7: Styling And Layout Hardening

Objective: make app-scale layout predictable and easier to debug.

### API/Behavior Targets

- Layout warning diagnostics for:
  - overflow without scroll container
  - unresolved percent/calc sizing
  - clipped text in fixed controls
  - zero-sized interactive widgets
- Better default min-size behavior for app panels and data widgets.
- Per-widget layout debug overlay.
- Stable recipes for:
  - app shell
  - sidebar + routed pages
  - toolbar + content + status
  - split-pane workbench
  - master/detail layout
  - modal forms
- CSS support policy labels:
  - stable
  - experimental
  - unsupported

### Acceptance Criteria

- [ ] Layout diagnostics appear in `debug_snapshot()`.
- [ ] Probe suite covers all app shell recipes.
- [ ] Release-gate smoke command runs representative
  `examples/css_feature_probes` entries with `--strict-layout`.
- [ ] Probe inventory is generated or checked against
  `examples/css_feature_probes/*.py`, with helper/benchmark probes explicitly
  included or excluded.
- [ ] Docs include known layout failure modes and fixes.
- [ ] CSS capability docs mark stable vs experimental properties.

## Workstream 8: Packaging, Compatibility, And Release Discipline

Objective: make the library installable and dependable for users who are not
building from source.

### Targets

- Wheels for supported Python versions and platforms.
- Clear support matrix:
  - Python versions
  - OS versions
  - GPU/backend assumptions
  - optional dependency groups for data widgets, tests, probes, and heavyweight
    examples
  - WebView2 availability for `HtmlReport`
  - `HtmlReport` fallback behavior, non-Windows unsupported state, and relevant
    environment flags
- Semantic versioning policy.
- Public API compatibility policy.
- Deprecation mechanism:
  - warnings
  - migration notes
  - removal schedule
- Release checklist:
  - Python tests
  - Rust tests
  - declared test/probe dependency install
  - probe smoke suite
  - representative `examples/css_feature_probes` smoke runs
  - probe inventory freshness check
  - example syntax checks
  - sdist/wheel manifest content audit
  - wheel install smoke
  - docs/help coverage

### Acceptance Criteria

- [ ] `docs/release.md` created.
- [ ] CI/check script for the V5 release gate.
- [ ] CI matrix or release-gate job proves Python 3.11, 3.12, and 3.13 for
  supported platforms.
- [ ] Python test and probe jobs install declared dependencies instead of
  relying on incidental runner packages.
- [ ] Wheel install smoke test.
- [ ] sdist and wheel content checks prove docs, current plan subdirectories,
  `examples/css_feature_probes`, notices, and package metadata are present.
- [ ] CI or release gate runs `tools/smoke_css_demos.py --strict-layout`
  against representative demos and probes.
- [ ] `backend_info()` reports enough OS, Python, renderer, backend adapter,
  and optional feature data for support tickets.
- [ ] `HtmlReport` probe/release smoke records embedded, disabled, fallback, or
  unsupported WebView2 state.

## Milestones

### Milestone A: App Shell Baseline

- Lifecycle callbacks.
- Component cleanup/effect hooks for timers, workers, and external resources.
- Clipboard text support.
- Global shortcuts.
- Managed timer/background-job helpers built on the existing task queue.
- Keyboard traversal audit for core controls.
- App shell recipe docs and example.

### Milestone B: Text And Editing Baseline

- Selection/copy/paste/undo/redo for text controls.
- Standard edit commands.
- Validation state.
- Text editing probe and docs.

### Milestone C: Data App Depth

- Data model protocol draft.
- Table column controls.
- Editable table cells.
- Lazy tree loading.
- Selection-mode coverage.

### Milestone D: Accessibility Foundation

- Internal accessibility tree.
- Accessibility snapshot.
- Roles/states/names for core widgets.
- Keyboard-only probe suite.

### Milestone E: Release Gate

- Release checklist automated enough to run before tags.
- Wheel/install smoke path.
- `HtmlReport` optional-feature support matrix documented.
- Stable/experimental docs updated.
- `dg.help` covers new lifecycle, clipboard, accessibility, and model/view APIs.

## Suggested File/Module Ownership

Likely Python modules:

- `python/dragongui/app.py`
- `python/dragongui/runtime.py`
- `python/dragongui/components.py`
- `python/dragongui/diagnostics.py`
- `python/dragongui/thread_monitor.py`
- `python/dragongui/widgets.py`
- `python/dragongui/dialogs.py`
- `python/dragongui/manual.py`
- new `python/dragongui/clipboard.py`
- new `python/dragongui/accessibility.py`
- new `python/dragongui/models.py`

Likely native modules:

- `native/src/app.rs`
- `native/src/events.rs`
- `native/src/commands.rs`
- `native/src/document.rs`
- `native/src/layout.rs`
- `native/src/primitives/mod.rs`
- `native/src/resources.rs`
- `native/src/table.rs`
- `native/src/text/mod.rs`
- `native/src/html_report_webview.rs`
- `native/src/toast.rs`
- new `native/src/clipboard.rs`
- new `native/src/accessibility.rs`

Likely docs/examples:

- `.github/workflows/ci.yml`
- `pyproject.toml`
- `tools/smoke_css_demos.py`
- `docs/widgets.md`
- `docs/widgets-reference.md`
- `docs/css-capabilities-reference.md`
- `docs/extension-widgets.md`
- `docs/library-overview.md`
- `examples/css_feature_probes/README.md`
- `examples/css_feature_probes/property_grid_probe.py`
- `examples/css_feature_probes/command_palette_probe.py`
- `examples/css_feature_probes/breadcrumbs_probe.py`
- `examples/css_feature_probes/navigation_widgets_probe.py`
- `examples/css_feature_probes/html_report_probe.py`
- new `docs/app-lifecycle.md`
- new `docs/accessibility.md`
- new `docs/model-view.md`
- new `docs/release.md`
- new probes under `examples/css_feature_probes/`

## Open Questions

- Should V5 remain single-window, or should window lifecycle work include a
  minimal multi-window runtime?
- Which platforms are officially targeted first: Windows only, or
  Windows/Linux/macOS?
- Should accessibility use `accesskit` as the platform bridge?
- Should the generic model API be synchronous only at first, or support async
  row loading?
- How much text editing should live in native Rust versus Python command
  helpers?
- Should clipboard APIs live at `dg.clipboard.*`, `App.*`, or both?
- Is V5 localization limited to documenting app-owned strings/formatting plus
  bidi/IME basics, or should it include a small public locale/formatting API?
- Should embedded `HtmlReport` support remain Windows/WebView2-only in V5, or
  should non-Windows embedding be researched before the release gate?
- Should feature probes ship in sdists only, or should selected probes/examples
  also ship in wheels?
- Which probe subset is mandatory in the release gate versus manual visual QA?

## Completion Definition

This plan is complete when DragonGUI can honestly claim:

> DragonGUI is a GPU-native Python desktop toolkit for data-heavy applications
> and internal tools. It supports production-grade single-window app shells,
> standard desktop editing workflows, keyboard operation, core accessibility
> metadata, table/tree application workflows, and a documented release support
> matrix.

At that point, DragonGUI would still not be a full PyQt replacement, but its
general app completeness rating should move from roughly 4/10 to roughly 6/10
against mature desktop frameworks.
