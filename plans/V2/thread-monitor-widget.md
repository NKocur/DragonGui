# Thread Monitor Widget Plan

## Objective

Add a first-class DragonGUI debugging widget for monitoring Python threading,
native command flow, stale widget commands, and background update health inside
the running application.

Python GUI threading bugs are usually invisible: a background thread dies, a
queue backs up, a stale widget handle drops commands, or callbacks create an
accidental re-entrant update loop. DragonGUI is well-positioned to expose these
problems because it already has a clear boundary between Python threads, the
command queue, and the Rust UI thread.

The goal is not to build a general Python profiler. The goal is to make
DragonGUI-specific threading failures obvious while the app is running.

## Proposed API

```python
with dg.Sidebar(title="Debug"):
    dg.ThreadMonitor(
        show_threads=True,
        show_queue=True,
        show_dropped=True,
        show_widgets=True,
        history_seconds=30,
        refresh_hz=4,
    )
```

Optional development overlay:

```python
app = dg.App(debug_overlay=True)
```

Or:

```python
app.show_thread_overlay(True)
```

The overlay mode should be collapsible and intended for development. The widget
mode should be embeddable in normal layouts like any other DragonGUI widget.

## User-Facing Modes

### Overlay Mode

Overlay mode is a small floating diagnostic panel in a corner of the app.

It should show:

- Current command queue depth.
- Queue depth sparkline.
- Active Python thread count.
- Dead/error thread indicator.
- Dropped command count.
- UI frame time summary.

This mode is meant to be always available during development with minimal setup.
It should not require the user to add a widget to the layout.

### Full Widget Mode

`dg.ThreadMonitor()` is a full diagnostic panel.

It should show:

- Thread inventory.
- Queue depth history.
- Per-thread command rate.
- Dropped command log.
- Last command per live widget.
- Recent Python background task failures.
- Optional enqueue latency / GIL contention approximation.

The full widget should support vertical scrolling once the content exceeds its
layout rect.

## What The Monitor Should Expose

### 1. Thread Inventory

Show every relevant Python thread:

- Thread name.
- Python thread identifier.
- Native thread identifier when available.
- Alive/dead state.
- DragonGUI role, if registered.
- Last command enqueue time.
- Commands enqueued total.
- Commands per second.
- Last exception, if captured through a DragonGUI helper.

Initial implementation can use:

- `threading.enumerate()`
- `threading.get_ident()`
- `threading.get_native_id()` where available
- DragonGUI-owned metadata for threads that interact with the app

### 2. Command Queue Depth

Show command queue depth over time.

This is the most important diagnostic. A rising queue depth means background
work is producing updates faster than the UI thread is consuming them.

Track:

- Current queue depth.
- Max depth over the visible history window.
- Average depth.
- Queue depth sparkline.
- Drain rate.
- Enqueue rate.

### 3. Per-Thread Command Rate

Track command production by originating thread:

- Commands per second.
- Total commands.
- Last command type.
- Last target widget id.
- Last enqueue time.

This answers: "which thread is hammering the UI?"

### 4. Last Command Per Widget

For each live widget that receives commands:

- Widget id.
- Widget type.
- Optional `key`.
- Optional `class_`.
- Last command type.
- Origin thread.
- Time since last command.
- Last dirty flag produced.

This answers: "why is my widget not changing?"

### 5. Dropped Command Log

Every dropped native command should be visible.

Log fields:

- Timestamp.
- Command type.
- Target widget id.
- Target widget type, if known.
- Origin thread id/name.
- Reason.
- Whether the command came from a UI callback or background thread.

Common reasons:

- Stale widget id.
- Closed app handle.
- Closed native command bridge.
- Released resource handle.
- Invalid command payload.

The existing stale-command `eprintln!` behavior should feed this log instead of
being terminal-only.

### 6. Background Task Failures

`App.call_soon_threadsafe()` and future DragonGUI thread helpers should record
Python exceptions that occur in scheduled tasks.

For each failure:

- Timestamp.
- Callable repr/name where available.
- Origin thread.
- Exception type.
- Exception message.
- Short traceback string.

This makes silent thread/task death visible in-app.

### 7. Enqueue Latency / GIL Contention Approximation

Exact GIL contention is difficult to measure from the outside, but DragonGUI can
track practical enqueue latency:

- Time before Python requests enqueue.
- Time after enqueue call returns.
- Time until Rust drains the command.

This gives:

- Python-side enqueue duration.
- Queue wait duration.
- Total command latency.

If the Python enqueue duration spikes, it may indicate Python-side contention or
callback pressure. If queue wait spikes, the UI thread is not draining fast
enough.

## Data Model

Add a native diagnostics registry owned by the Rust runtime.

Proposed Rust structures:

```rust
struct DiagnosticsState {
    queue_samples: RingBuffer<QueueSample>,
    thread_stats: HashMap<ThreadKey, ThreadStats>,
    widget_command_stats: HashMap<WidgetId, WidgetCommandStats>,
    dropped_commands: RingBuffer<DroppedCommand>,
    task_failures: RingBuffer<TaskFailure>,
    command_latency: RingBuffer<CommandLatencySample>,
}
```

```rust
struct QueueSample {
    timestamp_ms: u64,
    depth: usize,
    enqueued_total: u64,
    drained_total: u64,
}
```

```rust
struct ThreadStats {
    name: Option<String>,
    python_ident: u64,
    native_ident: Option<u64>,
    role: Option<String>,
    alive: bool,
    enqueued_total: u64,
    enqueued_per_second: f32,
    last_command: Option<String>,
    last_widget_id: Option<String>,
    last_enqueue_ms: Option<u64>,
}
```

```rust
struct DroppedCommand {
    timestamp_ms: u64,
    command_type: String,
    widget_id: Option<String>,
    widget_kind: Option<String>,
    thread_ident: Option<u64>,
    thread_name: Option<String>,
    reason: String,
}
```

Ring buffers should be bounded by `history_seconds` and sample rate so the
monitor cannot become a memory leak.

## Python Instrumentation

Add a lightweight diagnostics path in the Python runtime layer.

The Python side can attach metadata to each enqueued command:

- Thread name.
- Python thread ident.
- Native thread id when available.
- Optional role.
- Enqueue timestamp.

Possible API:

```python
dg.register_thread_role("streaming-loader")
```

or:

```python
with dg.thread_role("streaming-loader"):
    run_loader()
```

This should be optional. Unregistered threads still appear by name/id.

## Command Metadata

Every command should carry diagnostics metadata in debug builds or when
diagnostics are enabled:

```rust
struct CommandEnvelope {
    command: Command,
    origin: CommandOrigin,
    enqueue_timestamp_ms: u64,
}
```

```rust
struct CommandOrigin {
    python_thread_ident: Option<u64>,
    native_thread_ident: Option<u64>,
    thread_name: Option<String>,
    role: Option<String>,
    source: CommandSource,
}
```

```rust
enum CommandSource {
    UiCallback,
    BackgroundThread,
    ComponentRerender,
    StartupResource,
    Unknown,
}
```

If diagnostics are disabled, this metadata can be omitted or reduced.

## Native Widget Rendering

`ThreadMonitor` should render from Rust-side diagnostics snapshots, not by
calling Python every frame.

Widget sections:

- Header summary.
- Queue sparkline.
- Thread table.
- Widget command table.
- Dropped command log.
- Task failure log.

The first version can render with existing primitives and text infrastructure.
It does not need charts beyond a simple sparkline made of small rects or line
segments.

## Refresh Policy

The monitor should update at a fixed low rate:

- Default: 4 Hz.
- Configurable: `refresh_hz`.
- Minimum: 1 Hz.
- Maximum: 15 Hz.

It should not redraw every time a command is enqueued. Instead:

- The diagnostics registry records every event cheaply.
- The widget snapshots registry state at the refresh interval.
- The widget marks itself visual/text dirty after each snapshot.

This prevents the debugger from becoming the performance problem.

## Privacy And Production Behavior

Default policy:

- Diagnostics are enabled in development examples.
- Overlay is opt-in.
- Full widget is opt-in.
- Command counters and queue depth are safe to keep in normal builds.
- Tracebacks and callable reprs should be considered debug data.

If DragonGUI later adds build modes, traceback capture can be debug-only by
default.

## Milestone Plan

### T0: Planning Document

Status: this document.

Deliverables:

- Define widget goal.
- Define diagnostics data.
- Define API shape.
- Define milestone sequence.

### T1: Diagnostics Event Model

Goal: record command queue behavior without rendering a widget.

Deliverables:

- Add `CommandEnvelope` or equivalent metadata path.
- Record command enqueue count.
- Record command drain count.
- Record queue depth samples.
- Record origin thread id/name for enqueued commands.
- Add bounded ring buffers.
- Include diagnostics summary in `App.debug_snapshot()`.

Acceptance:

- `debug_snapshot()` shows queue depth, enqueued/drained totals, and per-thread
  command counts.
- Metadata collection has bounded memory.
- Existing command behavior remains unchanged.

### T2: Dropped Command Registry

Goal: make stale/closed/invalid command drops observable.

Deliverables:

- Replace terminal-only stale command logging with diagnostics events.
- Keep terminal logging in debug mode if useful.
- Record command type, widget id, origin thread, and reason.
- Add dropped command counts to debug snapshot.

Acceptance:

- Sending a command to a stale widget records a dropped-command entry.
- Background-thread drops do not panic.
- Snapshot includes recent dropped commands.

### T3: Python Thread Metadata

Goal: enrich native diagnostics with Python thread information.

Deliverables:

- Capture `threading.current_thread().name`.
- Capture `threading.get_ident()`.
- Capture `threading.get_native_id()` where available.
- Add optional `dg.register_thread_role(...)`.
- Add optional context manager `dg.thread_role(...)`.

Acceptance:

- Commands from named background threads show the correct thread name.
- Unnamed threads still show stable ids.
- Thread roles appear in debug snapshot.

### T4: ThreadMonitor Widget Shell

Goal: add the visible widget with summary stats.

Deliverables:

- Add Python `dg.ThreadMonitor`.
- Add Rust `WidgetKind::ThreadMonitor`.
- Add layout behavior.
- Add primitive/text rendering for header summary.
- Add fixed-rate refresh.

Acceptance:

- A demo can embed `dg.ThreadMonitor()` in a sidebar.
- The widget shows queue depth, active thread count, and dropped command count.
- The widget updates without Python per-frame work.

### T5: Full Thread And Queue Views

Goal: make the widget useful for real debugging.

Deliverables:

- Thread inventory table.
- Per-thread command rate.
- Queue depth sparkline.
- Recent dropped command log.
- Last command per widget list.

Acceptance:

- A streaming scatter demo shows the producer thread and command rate.
- Artificial queue backup is visible in the sparkline.
- Stale widget command drops are visible in the log.

### T6: Background Task Failure Capture

Goal: expose silent Python task failures.

Deliverables:

- Wrap `call_soon_threadsafe` task execution with exception capture.
- Store task failure entries.
- Render failure count and recent failure log.
- Add traceback truncation.

Acceptance:

- A scheduled task that raises appears in `ThreadMonitor`.
- The app does not crash from a captured background task failure unless the
  user explicitly opts into fail-fast behavior.

### T7: Debug Overlay Mode

Goal: provide always-available development HUD.

Deliverables:

- Add `App(debug_overlay=True)` or equivalent.
- Render collapsible overlay above app content.
- Show queue sparkline, thread count, dropped count, frame time.
- Add keyboard toggle for debug overlay.

Acceptance:

- Overlay can be toggled without changing app layout.
- Overlay does not intercept normal app input when collapsed.
- Overlay cost is low and bounded.

## Demo Plan

Add `examples/thread_monitor_demo.py`.

The demo should include:

- A `Scatter3D` widget.
- A background producer thread pushing scatter data every 100-500 ms.
- A button that intentionally enqueues a stale command after replacing a widget.
- A button that schedules a task that raises an exception.
- An embedded `ThreadMonitor`.
- Optional debug overlay enabled.

The demo should make it easy to verify:

- Thread inventory updates.
- Queue depth changes.
- Command rates are visible.
- Dropped commands appear.
- Task failures appear.

## Risks

### The Debugger Becomes Expensive

If every command records too much data or triggers a redraw, the monitor can
create the performance problem it is supposed to diagnose.

Mitigation:

- Bounded ring buffers.
- Fixed-rate widget refresh.
- Cheap counters on hot paths.
- Full traceback capture only for failures.

### Python Thread State Is Incomplete

Python cannot reliably report "blocked on GIL" for every thread using only
standard APIs.

Mitigation:

- Present enqueue latency and queue wait time as practical signals.
- Avoid claiming exact GIL state unless a reliable implementation exists.

### Thread Exceptions Are Hard To Capture Globally

DragonGUI can capture exceptions in its own scheduled tasks, but arbitrary user
threads can still die outside DragonGUI.

Mitigation:

- Document that DragonGUI captures exceptions from `call_soon_threadsafe`.
- Provide optional thread helper wrappers later.
- Still show thread liveness changes from `threading.enumerate()`.

### Diagnostics Need Stable Widget Metadata

Last-command-per-widget requires stable widget ids and retained metadata.

Mitigation:

- Build this after stable identity and command queue work.
- Store widget kind/key/class metadata in the retained tree.

## Dependencies

This feature should not start before:

- Stable live widget identity exists.
- Command queue exists.
- Dropped command behavior is centralized.
- Debug snapshot exists or is close to complete.

Natural timing:

- Best as a V2 feature.
- Could begin after reactive engine R6.
- The diagnostics registry can start earlier if it improves `debug_snapshot()`.

## Success Criteria

This feature succeeds when:

- A user can see whether background threads are still alive.
- A user can see whether the UI command queue is backing up.
- A user can identify which thread is producing too many commands.
- Stale widget commands are visible in-app.
- Background task failures are visible in-app.
- The widget renders from Rust-side diagnostics data.
- The monitor itself does not require Python work every frame.
- The monitor has bounded memory and predictable refresh cost.

## Long-Term Extensions

Possible later additions:

- Export diagnostics as JSON.
- Save a short diagnostics trace to disk.
- Add warning thresholds for queue depth or command rate.
- Add per-widget command latency histograms.
- Add frame-time and GPU upload timing graphs.
- Add a visual command timeline.
- Add a helper API for managed background workers.
- Add integration with Python logging.
- Add opt-in call stack capture for command enqueue sites.
