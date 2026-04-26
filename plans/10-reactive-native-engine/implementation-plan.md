# Reactive Native Engine Implementation Plan

## Purpose

This is the concrete implementation plan for the proposal in
`plans/10-reactive-native-engine/proposal.md`.

The plan is based on the code that exists today:

- Python widgets live in `python/dragongui/widgets.py`.
- `App.run()` serializes one document and calls `_backend.run_document(...)`.
- Rust receives JSON in `native/src/app.rs`.
- Rust owns the blocking `winit` event loop in `native/src/runtime.rs`.
- Rust stores one static `WidgetNode` tree and mutable `WidgetState`.
- Layout is in `native/src/layout.rs`.
- Rendering is split across primitives, text, scatter, and table modules.

The first goal is not the component system. The first goal is a live runtime
bridge that lets Python update the native UI after startup.

## Current Status

- R1.1 through R1.5 are implemented.
- R1.6 is implemented: native commands now wake the `winit` event loop through
  a user-event proxy and drain before redraw.
- R1.7 is implemented for the current basic controls:
  `TextInput.set_value`, `Slider.set_value`, `Dropdown.set_value`, and
  `Checkbox.set_checked`.
- A visible manual test exists at `examples/live_update_tool.py`.
- R2.1 through R2.3 are implemented: live `Scatter3D.set_points(...)` now sends
  raw packed xyz bytes through the command bridge and updates the native scatter
  buffer without restarting.
- A visible manual scatter test exists at `examples/streaming_scatter_tool.py`.
- R2.4 live scatter telemetry is implemented: snapshots report point count,
  payload size, Python pack time, queue latency, native decode time, GPU upload
  time, and native apply time.
- R3.1 through the first R3.4 vertical slice are implemented: Python widgets
  serialize `style`/`class_`, Rust parses `NodeStyle`, resolves theme tokens,
  applies layout overrides, and applies primitive color/border/radius/accent
  overrides.
- R3 text styling now supports `font_size`, `color`/`foreground`, and
  `text_align`, `font_family`, and `font_weight` for rendered widget text.
- A visible manual style test exists at `examples/style_showcase.py`.
- R4.1 and R4.2 are implemented as an internal Python layer:
  `python/dragongui/vdom.py` defines `VNode`, `Patch`, `ResourceRef`, widget
  conversion, shallow prop/style diffing, keyed child identity checks, and
  resource-aware comparisons.
- R4.3 has a first safe slice: `AppHandle.apply_patch/apply_patches` maps
  supported `set_prop` patches onto the existing live native command bridge.
  `set_style` patches now map to native `SetStyle`, merge into the retained raw
  style map, reparse `NodeStyle`, and rebuild layout/text/visuals according to
  the style keys changed.
- R4.3 also has a limited structural slice: `replace_children` patches map to
  native `ReplaceChildren`, parse serialized child nodes, replace the retained
  subtree children, rebuild widget state/kind maps, and relayout.
- R4.3 also supports `replace_node` patches through native `ReplaceNode`,
  including root node identity replacement for component rerenders.
- `Widget.set_style(...)` updates Python state and live native style when the
  widget is running.
- `Container.replace_children(...)` updates Python state and live native static
  child subtrees when the container is running.
- A visible manual live style test exists at `examples/live_style_tool.py`.
- A visible manual live child replacement test exists at
  `examples/live_children_tool.py`.
- R5.1 and the first R5.2 vertical slice are implemented:
  `python/dragongui/components.py` provides `@dg.component`, `ComponentCtx`,
  keyed `ctx.state(...)`, root component rendering through `App.run(...)`, and
  live rerender/diff/apply for state changes that produce supported patches.
- Component rerenders can now apply node-identity replacement patches.
- A visible manual component state test exists at
  `examples/component_counter_tool.py`.
- R5.3 first slice is implemented: nested component calls with explicit `key`
  are retained by the parent component runtime, and child `ctx.state(...)`
  values survive parent rerenders.
- A visible manual nested component state test exists at
  `examples/component_nested_tool.py`.
- Live callback registration is implemented through `AppHandle`: dynamic
  `replace_children(...)` and component rerenders register Python click/change
  callbacks by widget id, and Rust falls back to `AppHandle` when a callback id
  is not present in the startup callback maps.
- Dynamic replacement subtrees may now include callback-bearing widgets.
- R6 snapshots now include a bounded native command history and recent dirty
  reasons so audits can see which live commands triggered layout/text/visual/GPU
  invalidation.

## Implementation Principles

- Keep the current context-manager API working throughout.
- Keep generated `id` behavior working for static examples.
- Add `key` as metadata before relying on it for reconciliation.
- Every visible feature slice must add or update a demo GUI under `examples/`
  before the slice is considered complete.
- Prefer over-invalidating to stale UI.
- Keep command payloads data-oriented.
- Do not introduce CSS selectors during this effort.
- Do not require Python to run per frame.
- Run `python -m pytest`, `cargo check`, release build, and smoke examples at
  each completed slice.

## Current Constraints To Respect

### Blocking App Run

Today `App.run()` blocks until the window closes. That means any live handle must
exist before calling native `run_app`, and Rust must bind a wakeable command
sender to that handle before entering `py.allow_threads(...)`.

### Event Loop Wake

Current Rust code uses:

```rust
let event_loop = EventLoop::new()?;
event_loop.set_control_flow(ControlFlow::Wait);
event_loop.run_app(&mut app)?;
```

R1 needs a user-event event loop so background command enqueueing can wake the
UI thread:

```rust
let event_loop = EventLoop::<RuntimeEvent>::with_user_event().build()?;
let proxy = event_loop.create_proxy();
```

Then `DragonApp` handles `ApplicationHandler::user_event(...)`.

### Python Callback Path

Current callbacks are stored in Rust as `Box<dyn Fn() + Send>` and called from
Rust while reacquiring the GIL. This path can stay. Widget methods called inside
those callbacks should enqueue native commands when the widget is live.

### Scatter Startup Path

Current scatter startup sends base64 packed xyz data in the startup document.
R2 should keep this path for startup compatibility, but repeated live updates
must avoid JSON/base64 and pass packed bytes or buffers through PyO3.

## Target Runtime Bridge

Add two related but separate queues:

1. Native command queue: data-oriented commands consumed by Rust.
2. Python task queue: optional high-level helper behind
   `app.call_soon_threadsafe(callable)`.

The native command queue is the required foundation. The Python task queue is a
convenience layer for examples and background thread ergonomics.

### Native Bridge Shape

Rust owns:

```rust
struct CommandBridge {
    queue: Arc<CommandQueue>,
    proxy: Arc<Mutex<Option<EventLoopProxy<RuntimeEvent>>>>,
    closed: AtomicBool,
}
```

Python owns:

```python
class AppHandle:
    def enqueue(self, command: NativeCommand) -> None: ...
    def call_soon_threadsafe(self, fn: Callable[[], None]) -> None: ...

class LiveWidgetHandle:
    app: AppHandle
    id: str
```

Binding flow:

1. `App.run(window)` creates a Python `AppHandle`.
2. `App.run(window)` walks the widget tree and assigns `LiveWidgetHandle`
   objects to widgets before entering native code.
3. Python passes the `AppHandle` object into native `_dragongui.run_app(...)`.
4. Rust creates a `CommandBridge`.
5. Rust binds a native sender into the Python `AppHandle` while holding the GIL.
6. Rust enters `py.allow_threads(|| run_event_loop(spec))`.
7. `run_event_loop` creates the `EventLoopProxy` and installs it in the bridge.
8. Background enqueues wake the event loop through the proxy.
9. Window close marks the bridge closed and invalidates handles.

## R1: Stable Identity And Command Queue

### R1.1 Python Widget Identity

Files:

- `python/dragongui/widgets.py`
- `tests/test_python_api.py`

Changes:

- Add optional `key: str | None = None` to `Widget.__init__`.
- Store `self.key`.
- Include `"key": self.key` in `Widget.to_dict()` when present.
- Thread `key` through every widget constructor.
- Keep `id` unchanged and still callback-facing.
- Validate `key` is a non-empty string when provided.

Acceptance:

- Existing tests pass unchanged or with explicit additions.
- A widget can serialize both `id` and `key`.
- Existing examples still run without specifying keys.

### R1.2 Rust Document Identity Metadata

Files:

- `native/src/document.rs`
- `native/src/runtime.rs`

Changes:

- Add `key: Option<String>` to `WidgetNode`.
- Parse top-level `"key"` from each serialized widget node.
- Keep all current layout, state, callback, and hit-test maps keyed by `id`.
- Add a helper that can compute future identity paths:
  `parent_path/type/key_or_id`.

Acceptance:

- `cargo check` passes.
- Current `WidgetState::from_tree` behavior does not change.
- Debug logs can include key metadata when present.

### R1.3 Command Types And Dirty Flags

Files:

- `native/src/commands.rs` new file
- `native/src/lib.rs`
- `native/src/runtime.rs`

Initial command enum:

```rust
pub enum Command {
    SetProp { id: String, prop: String, value: CommandValue },
    Invalidate { id: String, dirty: Dirty },
    DrainPythonTasks,
}

pub enum CommandValue {
    Bool(bool),
    Float(f32),
    Text(String),
}

pub enum Dirty {
    Layout,
    Text,
    Visual,
    GpuData,
    Full,
}
```

Do not add scatter bytes here yet. That lands in R2.

Changes:

- Add thread-safe queue type with `push`, `drain`, `close`, and `is_closed`.
- Add stale/closed behavior:
  - synchronous enqueue returns an error
  - background helper logs and drops
- Add queue depth to future debug state, even if `debug_snapshot` is not built.

Acceptance:

- Queue unit tests cover push/drain/close.
- Queue is `Send + Sync`.

### R1.4 PyO3 Native Sender

Files:

- `native/src/commands.rs`
- `native/src/lib.rs`
- `native/src/app.rs`
- `python/dragongui/_backend.py`
- `python/dragongui/runtime.py`

Changes:

- Expose a minimal PyO3 class such as `_NativeCommandSender`.
- Methods:
  - `enqueue_set_prop(id, prop, value)`
  - `enqueue_invalidate(id, dirty)`
  - `enqueue_drain_python_tasks()`
  - `is_closed()`
- Convert Python values into `CommandValue`.
- Return Python exceptions on closed/stale synchronous calls.
- Extend native `_dragongui.run_app(...)` to accept an optional Python
  `AppHandle`/runtime object.
- Extend `_backend.run_document(...)` to pass that runtime object when present.
- Rust should call a private Python method such as
  `app_handle._bind_native_sender(sender)` before entering
  `py.allow_threads(...)`.
- Store a `Py<PyAny>` reference to the runtime object in `AppSpec` or a
  dedicated bridge object so `DrainPythonTasks` can reacquire the GIL and call a
  private drain method.

Acceptance:

- Python unit tests can instantiate/run fallback without native sender.
- Native backend exposes sender only through `AppHandle`, not as public API.
- The old no-runtime call path still works for tests and dev fallback.

### R1.5 Python AppHandle And Widget Live Handles

Files:

- `python/dragongui/app.py`
- `python/dragongui/widgets.py`
- `python/dragongui/runtime.py` new file
- `python/dragongui/__init__.py`
- `tests/test_python_api.py`

Changes:

- Add internal `AppHandle`.
- Add internal `LiveWidgetHandle`.
- Add `App.call_soon_threadsafe(fn)`.
- Add `Widget._bind_live(handle)` and `Widget._unbind_live()`.
- Add `Widget.is_live`.
- Add a tree walk in `App.run()` that binds all widgets to live handles before
  native starts.
- On normal return or exception from native, unbind all live handles.

Important behavior:

- In dev fallback, handles should not be bound.
- If native backend is unavailable, handles should not be bound.
- Calling a live-only method after close raises `RuntimeError` in synchronous
  contexts.

Acceptance:

- Tests prove handles bind/unbind around a mocked run.
- Existing callback wrapper tests still pass.
- `App.call_soon_threadsafe` queues a Python task even before native sender is
  attached, then sender attachment can wake/drain.

### R1.6 User Event Wake Path

Files:

- `native/src/runtime.rs`
- `native/src/app.rs`
- `native/src/commands.rs`

Changes:

- Change event loop to a user-event loop with `RuntimeEvent`.
- Add `RuntimeEvent::Wake`.
- Install the proxy into the shared `CommandBridge`.
- Implement `ApplicationHandler::user_event`.
- Drain native commands before redraw.
- Process command dirty flags:
  - `Visual`: `rebuild_primitives()`
  - `Text`: `rebuild_visuals()`
  - `Layout` or `Full`: `apply_layout()`
  - `GpuData`: no-op until R2
- `DrainPythonTasks` should reacquire the GIL and ask Python `AppHandle` to
  drain queued Python callables. Those callables may enqueue more native
  commands; drain again before redraw.

Acceptance:

- A background thread can wake the event loop with a no-op invalidate command.
- Window still exits cleanly.
- Smoke frame mode still exits after the requested frame count.
- Existing mouse/keyboard behavior still works.

### R1.7 Minimal Live SetProp

Files:

- `native/src/runtime.rs`
- `native/src/events.rs`
- `python/dragongui/widgets.py`

Changes:

- Support live `SetProp` for existing Rust-owned mutable values:
  - Checkbox `checked`
  - Slider `value`
  - Dropdown `value`
  - TextInput `value`
- Add Python methods:
  - `checkbox.set_checked(value)`
  - `slider.set_value(value)`
  - `dropdown.set_value(value)`
  - `text_input.set_value(value)`
- If widget is not live, update Python object only.
- If widget is live, update Python object and enqueue `SetProp`.

Acceptance:

- Existing UI can be updated from a Python callback without document resend.
- Text/primitive/layout dirty paths are selected correctly.
- Unknown widget ids are dropped with debug log.

## R2: Live Scatter Updates

### R2.1 Split Scatter Data Packing

Files:

- `python/dragongui/widgets.py`

Changes:

- Replace `_try_pack_xyz(...)` internals with two helpers:
  - `_pack_xyz_bytes(...) -> bytes | None`
  - `_try_pack_xyz(...) -> str | None` for startup base64 compatibility
- Startup document still uses base64.
- Live updates use raw bytes.

Acceptance:

- Existing serialization tests still pass.
- New tests prove raw bytes length is `rows * 12`.

### R2.2 Add Scatter Command

Files:

- `native/src/commands.rs`
- `native/src/runtime.rs`
- `native/src/scatter/mod.rs` if needed

Command:

```rust
Command::SetScatterPointsPacked {
    id: String,
    xyz: Vec<u8>,
    telemetry: Option<ScatterTelemetry>,
}
```

Changes:

- Decode raw little-endian float32 xyz triples into `PointInstance`.
- Reuse current colormap logic initially.
- Upload into the existing `ScatterWidget`.
- Mark `GpuData` dirty and request redraw.

Acceptance:

- Bad byte length returns/logs a clear error.
- Unknown scatter id is stale-handle behavior, not a panic.

### R2.3 Live `Scatter3D.set_points`

Files:

- `python/dragongui/widgets.py`
- `python/dragongui/runtime.py`
- `tests/test_python_api.py`

Changes:

- If not live, keep current behavior: mutate Python fields.
- If live, pack raw xyz bytes and enqueue `SetScatterPointsPacked`.
- Keep Python fields updated so callbacks see current state.
- Add synchronous stale-handle error.
- Add background drop/log behavior through AppHandle helper.

Acceptance:

- `replot_selected()` in `examples/scatter_tool.py` changes the rendered scatter.
- A background thread can push new point data.
- Upload time for live updates is recorded separately from startup upload.

### R2.4 Example And Benchmark

Files:

- `examples/scatter_tool.py`
- `examples/streaming_scatter_tool.py` new file
- `benches/bench_live_scatter.py` new file or extend existing bench

Changes:

- Update the current Plot button to actually update the native scatter.
- Add a streaming example that changes points every 500ms.
- Record:
  - pack time
  - command queue latency
  - native decode time
  - GPU upload time
  - native total apply time
- Expose latest live scatter metrics in
  `debug_snapshot()["gpu"]["resources"]["scatter"]`.
- Add `Print Metrics` to the streaming scatter demo.

Acceptance:

- User-visible scatter changes without restarting.
- Benchmark reports startup upload and live update upload separately.
- Snapshot metrics redact point buffers and only include counts/timings.

## R3: Structured Inline Styles

### R3.1 Python Style Props

Files:

- `python/dragongui/widgets.py`
- `python/dragongui/theme.py`
- `tests/test_python_api.py`

Changes:

- Add `style: dict[str, object] | None = None` and `class_: str | None = None`
  to `Widget`.
- Serialize top-level `style` and `class`.
- Validate style is mapping-like.
- Define `class_` as semantic/debug only.

Acceptance:

- Style/class serialize for all widgets.
- Existing widgets still work without style/class.

### R3.2 Rust Style Model

Files:

- `native/src/style_model.rs` new file
- `native/src/document.rs`
- `native/src/theme.rs`

Changes:

- Add `NodeStyle`.
- Parse style maps from JSON.
- Support the V1 keys listed in the proposal.
- Add token resolver for current `Theme`.
- Unknown style keys warn and ignore.
- Unknown color tokens warn and fall back.

Acceptance:

- Rust tests cover token resolution and simple layout fields.

### R3.3 Taffy Integration

Files:

- `native/src/layout.rs`

Changes:

- Merge style overrides into `style_for(...)`.
- Keep current widget defaults when style keys are absent.
- Apply width/height/min/max/padding/margin/gap/flex keys.

Acceptance:

- A test panel with `style={"width": 340}` lays out to 340 logical pixels.
- Gap/padding style overrides existing theme defaults.

### R3.4 Primitive/Text Integration

Files:

- `native/src/primitives/mod.rs`
- `native/src/text/mod.rs`

Changes:

- Resolve visual/text style from node + widget state.
- Support pseudo-state nested maps:
  - `hover`
  - `active`
  - `focus`
  - `disabled`
- Keep current theme styling as fallback.

Acceptance:

- Button hover style can be changed from Python style props.
- Panel background/border/radius can be changed without Rust edits.

## R4: Typed VNode And Patch Diff

### R4.1 Internal VNode

Files:

- `python/dragongui/vdom.py` new file
- `python/dragongui/widgets.py`

Changes:

- Define `VNode`.
- Add conversion from current widget objects to `VNode`.
- Keep public API unchanged initially.

Acceptance:

- Existing widget tree can produce the same document through VNode conversion.

### R4.2 Diff Algorithm

Files:

- `python/dragongui/vdom.py`
- `tests/test_python_api.py`

Changes:

- Implement shallow prop/style diff.
- Implement keyed child diff.
- Treat resource-like objects as handles.
- Emit patch command objects, not native calls yet.

Acceptance:

- Tests cover replace child, update prop, update style, keyed reorder.
- Large object props compare by identity/version only.

### R4.3 Patch To Command Mapping

Files:

- `python/dragongui/runtime.py`
- `native/src/commands.rs`
- `native/src/runtime.rs`

Changes:

- Map Python `set_prop` patches into existing native `SetProp` commands.
- Map Python `set_style` patches into native `SetStyle` commands.
- Map Python `replace_children` patches into native `ReplaceChildren` commands
  for serialized static child nodes.
- Map Python `replace_node` patches into native `ReplaceNode` commands for
  serialized replacement nodes.
- Native `SetStyle` mutates the retained raw style map, recomputes `NodeStyle`,
  selects a dirty flag, and rebuilds layout/text/visuals.
- Native `ReplaceChildren` replaces a target node's children and rebuilds full
  widget state for the retained tree.
- Rebuild widget state for replaced subtrees carefully.
- Rebuild `widget_kinds` after structural replacement.
- Preserve existing `click_cbs`/`change_cbs` or add a callback registration
  command before allowing new interactive widgets in replaced subtrees.

Acceptance:

- `set_prop` VDOM patches can be applied to live widgets through `AppHandle`.
- `set_style` VDOM patches can be applied to live widgets through `AppHandle`.
- Unsupported patch kinds fail loudly instead of being silently ignored.
- Native `SetStyle` changes a running widget's visual style without full
  document resend.
- Native `ReplaceChildren` changes a running container's static children without
  full document resend.
- Native `ReplaceNode` changes a running retained node without full document
  resend.
- Replacing a small subtree updates layout without full document resend.

## R5: Keyed Component Runtime

### R5.1 Component API

Files:

- `python/dragongui/components.py` new file
- `python/dragongui/__init__.py`
- `tests/test_python_api.py`

Changes:

- Add `@dg.component`.
- Add `ComponentCtx`.
- Add `ctx.state(key, default)`.
- Detect duplicate state keys in a component instance.
- Store component state by parent path + component type + key.

Acceptance:

- Local state survives rerenders.
- Duplicate state keys raise a clear exception.

Status:

- Implemented for root and nested component instances. Nested component calls
  require explicit keys, and child state preservation is covered by R5.3.

### R5.2 Component Render And Rerender

Files:

- `python/dragongui/components.py`
- `python/dragongui/app.py`
- `python/dragongui/runtime.py`

Changes:

- `App.run(...)` accepts a component root as well as current `Window`.
- State setters rerender owning component subtree.
- Diff the old/new VNode subtree.
- Enqueue generated patches.

Acceptance:

- `api-examples/01_simple_scatter_component.py` can become runnable or is
  revised with documented API changes.

Status:

- Implemented for root components when rerenders produce supported patches:
  `set_prop`, `set_style`, `replace_children`, and `replace_node`.
- New callback registration during rerender is implemented through the live
  `AppHandle` callback registry.
- `api-examples/01_simple_scatter_component.py` is runnable with the current
  context-manager API.

### R5.3 Parent/Child Preservation

Files:

- `python/dragongui/components.py`
- `tests/test_python_api.py`

Changes:

- Preserve child component state across parent rerenders when identity is stable.
- Drop state when a child disappears and is not retained by a live subtree.

Acceptance:

- `api-examples/02_nested_dataframe_component.py` can become runnable or is
  revised with documented API changes.

Status:

- Implemented for nested component calls made during another component render
  with explicit `key=...`.
- Child component state survives parent rerenders while identity is stable.
- Nested components without explicit keys raise a clear `ValueError`.
- `api-examples/02_nested_dataframe_component.py` is runnable with the current
  context-manager API.

### R5.4 Background Example

Files:

- `plans/10-reactive-native-engine/api-examples/03_background_scatter_updates.py`
- `examples/streaming_scatter_tool.py`

Changes:

- Reconcile the design fixture with the final R1/R2 command API.
- Make the example runnable.

Acceptance:

- A background thread updates scatter data every 500ms.
- Stopping the stream stops future updates cleanly.
- Closing the window does not crash the worker.

Status:

- `api-examples/03_background_scatter_updates.py` is runnable with the current
  component, command queue, and live scatter APIs.

## R6: Debug Snapshot

### R6.1 Rust Snapshot

Files:

- `native/src/runtime.rs`
- `native/src/commands.rs`
- `native/src/lib.rs`

Changes:

- Added request/response debug snapshot support through the native command
  bridge.
- Added runtime snapshot JSON for:
  - widget tree
  - keys/classes
  - layout rects
  - raw structured styles and theme tokens
  - focus/hover/pressed/open state
  - recent dirty reasons and command history
  - queue depth
  - frame/upload timing
  - resource counts
- Redact buffers.

Acceptance:

- Snapshot serializes to JSON-compatible Python dict.
- `cargo test --target x86_64-pc-windows-gnu` covers snapshot request/response.

### R6.2 Python API

Files:

- `python/dragongui/app.py`
- `python/dragongui/runtime.py`
- `python/dragongui/_backend.py`
- `examples/debug_snapshot_tool.py`

Changes:

- Added `app.debug_snapshot(timeout_ms=...)` for live native apps.
- Added `debug_snapshot` to `App.run(...)` results.
- Added dev-fallback snapshot data for Python-only tests.

Acceptance:

- Snapshot can be printed from examples.
- `python examples\debug_snapshot_tool.py` prints a live snapshot from a
  background thread and a final run-result snapshot after close.

Current limitation:

- Calling `app.debug_snapshot()` from inside a UI callback can time out because
  the event loop cannot service the snapshot request until the callback returns.
  Use a background thread or the `App.run(...)` result snapshot for now.

## Data Resource Follow-Up

Status:

- `DataFrameTable.set_frame(...)` now supports live updates through a bounded
  table-data command that refreshes retained table metadata and sample cells.
- DataFrameTable now assigns a stable `resource_id` and Rust stores formatted
  table cell payloads in a retained `ResourceRegistry` instead of directly on
  `TableState`.
- Table text rendering now resolves visible cells through the registry, and
  debug snapshots report table resource counts and redacted resource metadata.
- Rebuilding retained maps after structural replacement syncs the registry and
  releases stale table resources that are no longer present in the widget tree.
- Live `DataFrameTable.set_frame(...)` now sends supported NumPy-backed numeric
  columns through `SetTableDataColumns`: JSON carries metadata only, while
  column bytes cross the command bridge and are formatted on demand in Rust.
- Live `DataFrameTable.set_frame(...)` also packs NumPy string/object columns as
  UTF-8 offset buffers, so visible string cells beyond the bounded startup
  sample can be resolved from the native resource.
- `App.run(...)` now queues initial `DataFrameTable` column resources before the
  native sender binds. Startup JSON remains bounded, and the full numeric/UTF-8
  buffers drain into Rust immediately after GPU initialization.
- Table column uploads now pass byte views through the Python buffer protocol
  path. Numeric NumPy columns avoid an extra Python `bytes(...)` copy; Rust still
  copies into retained native memory before crossing into the winit-owned UI
  thread.
- The native package now targets `abi3-py311` because PyO3's safe buffer API is
  not exposed for the Python 3.10 limited ABI.
- `SetBufferResource` and `ReleaseResource` now provide a generic retained
  resource path for future image/texture/Arrow-backed work, and
  `App.set_buffer_resource(...)` / `App.release_resource(...)` expose explicit
  lifecycle control while the app is running.
- This is still not the final zero-copy Arrow C Data Interface path.

Remaining:

- Add Arrow C Data Interface resources with a pinned-owner lifetime design.
- Add image/texture widgets that consume generic buffer resources.

## R7: Stylesheet Research

This stays out of the runtime until R1-R6 are stable.

Deliverables:

- `selector_prototype.py` is a separate prototype for selector parsing and
  pseudo-state matching.
- `stylesheet-research.md` records the recommendation.

Recommendation:

- Keep inline structured styles primary for V1.
- Add stylesheets later only as a convenience layer over computed `NodeStyle`.
- Do not merge selector support into runtime until computed-style caching,
  stylesheet invalidation, and stylesheet-specific debug metadata are designed.

## Test And Verification Matrix

Run after each completed slice:

```powershell
python -m pytest
cargo check
cargo build --target x86_64-pc-windows-gnu --release
Copy-Item -Force native\target\x86_64-pc-windows-gnu\release\_dragongui.dll python\dragongui\_dragongui.pyd
$env:DRAGONGUI_SMOKE_FRAMES='3'; python examples\scatter_tool.py
$env:DRAGONGUI_SMOKE_FRAMES='3'; python examples\multipage_tool.py
```

Additional R1 checks:

- Background no-op command wakes event loop.
- Stale command after close logs/drops.
- `DRAGONGUI_DEV_FALLBACK=1` still returns serialized document.
- `examples/live_update_tool.py` visibly demonstrates live native updates for
  basic controls.

Additional R2 checks:

- Live `scatter.set_points(...)` changes visible data.
- Bad scatter byte payload reports a clear error.
- Startup base64 scatter path still works.
- A scatter-specific demo visibly changes point data without restarting.

Additional R3 checks:

- Style serialization tests.
- Rust token resolution tests.
- Layout tests for style width/gap/padding.
- A style/theming demo visibly exercises padding, gap, colors, borders, radius,
  and pseudo-state styles.

## Recommended First Pull Request Boundary

The first implementation PR should include only:

- Python `key` support.
- Rust `WidgetNode.key`.
- `commands.rs` with queue, dirty flags, and tests.
- Native sender binding scaffolding.
- No visible behavior change except serialized `key`.

That PR is small enough to review and creates the foundation without touching
scatter, layout, or rendering behavior.
