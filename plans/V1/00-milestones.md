# Milestones

## M0: Package Scaffold

Status: done.

Deliverables:

- `pyproject.toml` configured for PyPI and `maturin`.
- `native/` Rust extension crate.
- `python/dragongui` importable from source.
- `start.bat` for no-install development.
- Basic Python API smoke tests.

Acceptance checks:

- `python -m pytest`
- `python -m maturin build --release --out dist --target x86_64-pc-windows-gnu`
- `python -m maturin sdist --manifest-path native\Cargo.toml --out dist`

## M1: Native Window And GPU Surface

Goal: `app.run(win)` opens a real native window and clears it with `wgpu`.

Deliverables:

- Add `winit` event loop and window creation.
- Add `wgpu` instance, adapter, device, queue, surface, and swapchain config.
- Parse serialized Python window title, width, and height in Rust.
- Keep `DRAGONGUI_DEV_FALLBACK=1` for machines without a built native module.
- Return useful Rust errors as Python exceptions.
- Add a CI smoke test for Windows and macOS at this stage, not at release time.

Acceptance checks:

- `.\start.bat` opens a window when the native module is built.
- Window title and size match `dg.Window(...)`.
- Closing the window exits cleanly.
- Resizing reconfigures the surface.
- Windows CI builds and runs a headless/import smoke test.
- macOS CI builds and runs a native-window smoke test path that respects
  `winit` main-thread rules.
- `python -m pytest` still passes.
- `cargo check --manifest-path native\Cargo.toml` passes.

## M2: DragonSci Scatter Embedded In A Window

Goal: ship the first differentiated demo early: a native DragonGUI window with
a live DragonSci scatter plot.

Deliverables:

- Inspect the installed Python 3.11 package `dragonsci` and its source/binary
  layout to identify the reusable rendering core.
- Extract DragonSci scatter rendering into a shared Rust crate if source is
  available locally, or define the extraction boundary before reimplementation.
- Reuse existing DragonSci `wgpu` pipeline, point buffers, camera controls, and
  colormap logic instead of building a new scatter renderer from scratch.
- Adapt the renderer from DragonSci's standalone/Tkinter context into an
  embedded DragonGUI render pass.
- Add a minimal `Scatter3D` full-window or fixed-rect render path.
- Support generated 500k-point data first, then Python-provided columns.

Acceptance checks:

- `examples/scatter_tool.py` opens a DragonGUI window with a scatter region.
- 500k generated points render interactively.
- Camera controls work inside the DragonGUI window.
- The implementation shares code with DragonSci or documents the exact blocker.
- A first scatter benchmark records upload time and steady-state frame time.

## M3: Layout And Primitive Drawing, No Text

Goal: add enough layout and primitive rendering to place regions around the
scatter plot without taking on text complexity yet.

Deliverables:

- Add a typed Rust document model for widget trees.
- Use `taffy` for horizontal and vertical layout.
- Draw panels, rectangles, borders, separators, button shells, checkbox shells,
  slider tracks, dropdown shells, and scatter plot bounds.
- Add HiDPI scaling and resize-aware layout recomputation.
- Add the initial theme token model in Python and Rust, even if only colors and
  radii are used.

Acceptance checks:

- Example renders a left control region and a scatter region.
- No text rendering is required for this milestone.
- Resize does not panic and recomputes layout.
- Theme background/accent tokens affect primitive drawing.

## M4: Events, Callbacks, Updates, And Text

Goal: make basic interaction real and add text after the event and update path
exists.

Deliverables:

- Add the handle-based update protocol and command queue before per-widget
  mutation expands.
- Mouse hit testing from layout boxes.
- Button click dispatch.
- Checkbox toggle dispatch.
- Slider drag dispatch.
- Python callback registry keyed by widget id and event name.
- UI-thread command queue for callback and background-thread updates.
- Integrate `cosmic-text` or `glyphon` for labels and basic control text.

Acceptance checks:

- `Button(..., on_click=...)` fires exactly once per click.
- Widget mutations are sent as commands, not by reserializing the full app
  document.
- Background thread can enqueue a scatter data update safely.
- Text is readable on Windows and macOS at common HiDPI scales.

## M5: Full Basic Widget Set And Theming

Goal: finish the basic user interface surface around the scatter demo.

Deliverables:

- Implement `Label`, `Button`, `TextInput`, `Slider`, `Dropdown`, `Checkbox`,
  `Panel`, `HLayout`, and `VLayout` with consistent behavior.
- Add light and dark themes.
- Add a token-based theme API for background, surface, text, muted text,
  accent, border, danger, warning, success, radius, spacing, and font size.
- Add focus, hover, active, disabled, and validation states.
- Add keyboard navigation for basic controls.

Acceptance checks:

- The scatter example can be controlled with real UI widgets.
- Changing a theme token updates rendered widgets consistently.
- Widgets do not require Python per-frame rendering.

## M6: Virtualized DataFrame Table

Goal: direct DataFrame display without Python row rendering.

Deliverables:

- `DataFrameTable` Rust widget with row and column virtualization.
- Column metadata from pandas and Polars.
- Visible-row extraction only.
- Sort indicator and selection model.
- Optional Arrow path for efficient typed column access.

Acceptance checks:

- 1M rows can scroll at 60fps on a reference machine.
- Python does not render per-row widgets.
- Memory use scales with visible rows plus column metadata, not total rows.

## M7: Navigation, Tabs, And Multipage Apps

Goal: support real data-tool application shells with tabs, sidebars, and pages.

Deliverables:

- `Tabs` and `Tab` widgets for local view switching.
- `Pages`, `Page`, `Sidebar`, and `NavItem` widgets for multipage app shells.
- Active-page state owned by Rust after startup.
- Mouse and keyboard navigation.
- Focus traversal confined to the active page or tab.
- Hidden pages are not rendered or hit-tested.
- Navigation callbacks and targeted active-page update commands.

Acceptance checks:

- A multipage example can switch between scatter, table, and settings views.
- State inside inactive pages survives switching.
- Scatter/table input does not leak through inactive pages.
- Navigation changes do not reserialize the full app document.
- Dark and light themes style active, hover, focus, and disabled nav states.

Detailed plan: [Navigation And Multipage](./08-navigation-tabs.md)

## M8: Shipping Widget Set

Goal: add the missing everyday widgets expected by real desktop data tools.

Deliverables:

- `NumberInput`, `ProgressBar`, `Separator`, `Spacer`, static `Tooltip`, and
  modal/dialog support before shipping.
- `MenuBar`, `Menu`, `MenuItem`, `ContextMenu`, `Collapsible`, `FileDialog`,
  `ColorPicker`, `Image`, and `StatusBar` before a serious public release.
- Background-safe update path for progress/status/dialog/file-result updates
  through the command queue.
- Widget gallery and long-task examples.

Acceptance checks:

- A widget gallery demonstrates every Tier 1 widget.
- Numeric inputs support typed values, stepping, clamping, and callbacks.
- Progress can be updated from background work without full document resend.
- Tooltips work on multiple widget classes.
- Modal overlay blocks background input.
- Menus/context menus support common desktop workflows.
- File dialog and image display cover basic data-tool file workflows.

Detailed plan: [Shipping Widget Set](./09-shipping-widget-set.md)

## M9: Packaging, Docs, And Benchmarks

Goal: release a credible pre-alpha package with numbers attached.

Deliverables:

- Wheels for Windows, macOS Intel, macOS Apple Silicon, and Linux.
- README quickstart and examples.
- Benchmarks against Dear PyGui.
- CI for tests, smoke builds, wheel build, and source distribution.
- Clear compatibility matrix for Python and OS versions.

Acceptance checks:

- `pip install dragongui` works from TestPyPI.
- Benchmarks are reproducible from a clean checkout.
- README shows current limitations honestly.
- The 500k scatter benchmark remains part of every benchmark run.
