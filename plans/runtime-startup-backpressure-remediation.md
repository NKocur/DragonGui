# Runtime Startup and Backpressure Remediation Plan

**Project:** DragonGui
**Created:** July 28, 2026
**Status:** Priority startup and keyed-task remediation complete
**Primary reproducer:** `examples/cathode_ops_stress_demo.py`

## Purpose

The CATHODE-7 stress demo exposed a startup weakness in the interaction between:

- Native loading-screen presentation
- Native renderer and widget-tree initialization
- Python `call_soon_threadsafe(...)` tasks
- The Python-to-native command queue
- Native command-drain fairness
- Winit redraw scheduling

The loading frame appears, but live producers can enqueue work before the first
real application frame is presented. The accumulated Python callbacks then
expand into a much larger native command burst. Immediate continuation wakeups
can repeatedly service that burst before Winit presents the requested redraw,
leaving the application visibly stuck on the loading screen.

This plan makes startup readiness explicit, bounds live-update work, coalesces
obsolete state, and guarantees rendering progress under sustained command load.

## Implementation Progress

### July 28, 2026 — Keyed latest-state task scheduling complete

- Added `coalesce_key=` to both `App.call_soon_threadsafe(...)` and
  `AppHandle.call_soon_threadsafe(...)`.
- Replaced the pending Python-task deque with an ordered, sequence-indexed
  queue plus a key index:
  - Unkeyed callbacks remain lossless FIFO work.
  - A newer callback with the same key removes the older pending callback in
    O(1) and takes its true newest queue position.
  - Executing callbacks are never cancelled.
  - Multiple producer threads share the same locked index safely.
- Added a 6 ms Python-task drain budget in addition to the existing 100-task
  bound. A bounded drain posts exactly one continuation when work remains.
- Added scheduler diagnostics for enqueued, executed, coalesced, queued, and
  high-water task counts plus the configured time budget.
- Updated ordinary `App.run(...)` results to include the same Python scheduler
  snapshot already attached by `run_with_loading(...)`.
- Split CATHODE telemetry by semantics:
  - Line-plot appends, log entries, and periodic map events stay unkeyed and
    lossless.
  - Scope, bar, label, LED, and clock snapshots use
    `coalesce_key="cathode.telemetry.snapshot"`.
- Updated `dg.help` threading and live-update guidance with explicit
  snapshot-versus-event rules.
- Verification:
  - Twenty-five same-key callbacks collapse to the newest callback.
  - Multiple keys retain their newest callbacks at correct queue positions.
  - Four concurrent producers scheduling 400 callbacks leave one executable
    callback and report 399 replacements.
  - A forced zero-duration budget executes one callback and requests one
    continuation for the remaining work.
  - In the 90-frame live CATHODE profile, all 19 lossless stream callbacks
    executed and emitted all 38 plot append commands. Nineteen replaceable
    snapshots collapsed to six executions with 13 obsolete snapshots removed.
  - The Python queue high-water was 15, the native queue ended at zero, and the
    run held 60 FPS with no fairness yields or warnings.

### July 28, 2026 — Startup barrier and render fairness

- Added explicit native startup states for initialization, loading presentation,
  application-frame request, and application-frame presentation.
- Deferred only `DrainPythonTasks` until the first successful application
  present; native startup resources remain eligible for the first frame.
- Preserved the loading minimum-duration deadline before requesting the first
  application frame.
- Added render-before-continuation scheduling. A command drain that exhausts
  its budget now records a pending continuation and requests a frame instead
  of immediately posting another wake.
- Latched producer wakeups while a continuation is awaiting presentation, then
  released exactly one wake after the frame so newly arriving work cannot form
  another pre-render wake chain.
- Adjusted the double-drain wake path so it cannot start a second slice after
  the first slice has yielded for a frame.
- Resumes one command continuation after a successful present, including the
  scatter-upload sequencing path.
- Rate-limited the repeated fairness warning to one report per second and
  included aggregate drain-yield and suppressed-warning counts.
- Added debug-snapshot fields for startup readiness, deferred startup work,
  continuation state, drain yields, and suppressed fairness warnings.
- Added focused native tests for startup deferral policy and readiness labels.
- Validation completed:
  - Native: **770 passed, 12 ignored**
  - Python: **517 passed**
  - One-frame live CATHODE-7 control: frame 1 presented with 311 native
    commands still queued, proving the first frame was not blocked on telemetry.
  - Three-frame live CATHODE-7 smoke: completed in **7.2 seconds** after the
    pre-fix version timed out at 124 seconds. The final wake-latch run completed
    in **7.1 seconds**, presented three frames, and left both continuation flags
    clear.
  - Twenty-frame live smoke: completed in **18 seconds** with 19 bounded drain
    yields, demonstrating continued frame progress while backlog remained.
- The twenty-frame run ended with a queue depth of 360 because the uncoalesced
  telemetry producer still outpaces command consumption. This is the expected
  remaining Phase 3/4 backpressure work, not a redraw starvation recurrence.

---

## Reproduced Failure

Command:

```powershell
python3.12 examples/cathode_ops_stress_demo.py
```

Observed diagnostics:

```text
DragonGUI: command drain reached fairness limit; deferring 760 pending commands
DragonGUI: command drain reached fairness limit; deferring 728 pending commands
DragonGUI: command drain reached fairness limit; deferring 696 pending commands
```

### Measured Reproducer Characteristics

- Serialized widget count: **1,628**
- Python document-construction time: approximately **22 ms**
- Normal startup resource commands: **19**
- Live telemetry interval: **280 ms**
- Native commands generated by one telemetry callback: **30**
  - 26 generic `SetProp` commands
  - 2 line-plot append commands
  - 2 extension-node replacement commands
- Native command batch size: **32**
- Native command-drain budget: **6 ms**
- Python tasks allowed per drain: **100**

The reported queue decreases by exactly 32 commands per warning, matching
`MAX_COMMANDS_PER_DRAIN_BATCH`.

Approximately 25 telemetry callbacks accumulated during native initialization:

```text
25 callbacks × 30 native commands ≈ 750 commands
```

That closely matches the first reported backlog of 760 commands after including
startup resource traffic.

### Root Cause

1. `App.run()` marks widgets live and binds an `AppHandle` before the native
   runtime has presented its first application frame.
2. The demo starts its telemetry producer before entering `App.run()`.
3. `call_soon_threadsafe(...)` correctly deduplicates native
   `DrainPythonTasks` wakeups, but it preserves every queued Python callback.
4. Native startup for the 1,628-widget tree takes long enough for many telemetry
   callbacks to accumulate.
5. The first `DrainPythonTasks` command executes up to 100 callbacks in one
   operation.
6. Those callbacks expand into hundreds of native commands.
7. Generic `SetProp` and telemetry `ReplaceNode` commands are not coalesced in
   the native queue.
8. The native fairness limit stops a drain after 32 commands or 6 ms, then
   immediately posts another wake when work remains.
9. Repeated wake events can be processed ahead of `RedrawRequested`, so the
   loading frame remains visible even though redraws have been requested.

### Secondary Environment Finding

The reported command imports DragonGui from the Python 3.12 `site-packages`
directory rather than from this workspace:

```text
...Python312\site-packages\dragongui
```

The installed Python and native files are older than the current workspace
build. This did not create the backpressure defect, but it makes diagnosis and
verification ambiguous.

---

## Goals

1. Always present the first real application frame before queued live telemetry
   can monopolize the runtime.
2. Keep pending work bounded when producers run faster than rendering.
3. Preserve the latest state for coalescible updates.
4. Preserve ordering and every value for commands whose semantics require it.
5. Guarantee at least one render opportunity between bounded drain slices.
6. Expose readiness and backlog state to Python and diagnostics.
7. Make source-tree versus installed-package mismatches obvious.
8. Protect the behavior with a deterministic version of the CATHODE-7 failure.

## Non-Goals

- Making an unlimited producer rate sustainable.
- Silently dropping event callbacks, user input, dialogs, resource releases, or
  other commands with non-idempotent semantics.
- Coalescing every command type.
- Removing the loading screen.
- Hiding a queue flood by only increasing batch sizes or time budgets.
- Requiring all existing applications to adopt a new scheduling API.

---

# Phase 1 — First-Application-Frame Barrier

**Priority:** P0
**Status:** In progress

## Problem

`DrainPythonTasks` is allowed to execute immediately after native startup even
when only the loading screen has been presented. Startup resource commands and
live-update commands are therefore mixed before the first application frame.

## Required Work

- [x] Add an explicit native runtime readiness state with at least:
  - `LoadingPresented`
  - `ApplicationFrameRequested`
  - `ApplicationFramePresented`
- [x] Treat startup resource uploads separately from queued Python live tasks.
- [x] While the loading screen is active and no application frame has been
      presented:
  - Apply startup resource commands required to construct the initial frame.
  - Defer `DrainPythonTasks`.
  - Request the first application redraw.
- [x] After the first successful application-frame present:
  - Mark the runtime ready.
  - Release one deferred Python-task drain.
  - Continue remaining work through normal bounded scheduling.
- [x] Ensure loading-screen `min_duration_ms` still works.
- [ ] Ensure startup errors continue to display or return correctly.
- [ ] Ensure `run_with_loading(...)` can replace its placeholder before the
      application-ready transition without releasing unrelated telemetry.

## Design Constraint

The barrier must not defer resource commands produced by
`_queue_startup_resources()`. Tables, heatmaps, line plots, and scatter widgets
need those resources to render the first real frame. Only general Python task
execution should wait.

## Tests

- [x] A queued `DrainPythonTasks` is deferred while only the loading frame has
      been presented.
- [ ] Startup table, heatmap, line-plot, and scatter resources are applied
      before the first application frame.
- [x] The first application frame presents with a non-empty command queue.
- [x] Deferred Python tasks resume after that present.
- [ ] `min_duration_ms=0` and non-zero values both transition correctly.
- [ ] Loading disabled follows the same readiness contract without drawing a
      loading frame.
- [ ] `run_with_loading(...)` retains its placeholder replacement semantics.

## Acceptance Criteria

- CATHODE-7 cannot remain on the loading screen because of live command volume.
- `frames_rendered >= 1` before the first accumulated telemetry task executes.

---

# Phase 2 — Render-Before-Continuation Fairness

**Priority:** P0
**Status:** In progress

## Problem

When the drain budget is reached, `bridge.wake()` immediately posts another
user event. A requested redraw may remain behind a chain of wake events.

## Required Work

- [x] Add a `command_drain_continuation_pending` runtime flag.
- [x] When the drain budget is exhausted and commands remain:
  - Mark a continuation pending.
  - Request a redraw.
  - Do not immediately post another wake if a redraw is already pending.
- [x] After `RedrawRequested` successfully presents:
  - Post one continuation wake if commands remain.
- [x] If a drain slice produces no visual work, still arrange a bounded
      continuation. The implementation requests a presentation directly so
      non-visual commands cannot bypass the render opportunity or stall.
- [x] Review the current double drain in `RuntimeEvent::Wake`; retain it only if
      it cannot bypass the render opportunity.
- [x] Keep scatter-upload redraw sequencing correct.
- [x] Keep popup-deferred Python task behavior correct.
- [x] Rate-limit fairness warnings so a sustained load does not flood stderr.

## Tests

- [x] A pending redraw occurs before the second drain slice.
- [x] A large live backlog advances both command and frame counters.
- [ ] Non-visual command backlogs still finish.
- [ ] New commands arriving during rendering trigger exactly one continuation.
- [ ] Scatter upload, screenshot, debug snapshot, and exit commands cannot
      deadlock behind the continuation policy.
- [ ] Fairness warning rate is bounded and reports aggregate information.

## Acceptance Criteria

- Sustained command load cannot starve `RedrawRequested`.
- Frames continue to present while queue depth is non-zero.

---

# Phase 3 — Keyed Python Task Coalescing

**Priority:** P0
**Status:** Complete

## Problem

`call_soon_threadsafe(...)` deduplicates drain wakeups but retains every
callback. For state-snapshot producers, old callbacks are obsolete before they
execute.

## Proposed API

```python
app.call_soon_threadsafe(apply_frame, coalesce_key="telemetry")
```

The existing call remains unchanged:

```python
app.call_soon_threadsafe(callback)
```

## Semantics

- No key: preserve every callback in FIFO order.
- Same key: retain only the newest not-yet-started callback.
- Different keys: preserve relative queue order.
- A currently executing callback is never cancelled.
- Exceptions and diagnostic enqueue origins correspond to the callback that
  actually executes.
- Coalescing is explicit; DragonGui must not guess callback equivalence.

## Required Work

- [x] Extend `App.call_soon_threadsafe(...)` and
      `AppHandle.call_soon_threadsafe(...)` with an optional coalescing key.
- [x] Add an indexed pending-task structure without turning normal FIFO
      scheduling into an O(n) scan.
- [x] Record replaced-task counts.
- [x] Add a time budget to `_drain_python_tasks()` in addition to the existing
      100-task bound.
- [x] Ensure a bounded drain requests exactly one follow-up drain when tasks
      remain.
- [x] Preserve thread safety when multiple producers use the same key.
- [x] Update CATHODE-7 telemetry to use a stable coalescing key.
- [x] Update live-update documentation with snapshot-versus-event guidance.

## Tests

- [x] Twenty-five tasks with one key execute only the newest callback.
- [x] Unkeyed tasks still all execute in FIFO order.
- [x] Multiple keys preserve their most recent callbacks.
- [x] Reentrant scheduling with the same key remains bounded.
- [x] Concurrent producer threads cannot corrupt the queue or execute a
      replaced callback.
- [x] Task exceptions and diagnostics remain correct.
- [x] The time budget stops a drain containing slow callbacks.

## Acceptance Criteria

- The 25 pre-ready CATHODE-7 telemetry callbacks collapse to one callback.
- Existing applications using the current API retain their behavior.

---

# Phase 4 — Native Command Coalescing

**Priority:** P0
**Status:** Not started

## Problem

One telemetry callback can still generate many commands. Repeated callbacks
produce obsolete generic state changes that remain in the native queue.

## 4.1 Generic `SetProp`

### Semantics

For programmatic state assignment, the newest pending value for the same
`(widget_id, property)` can replace older pending values when no structural
barrier invalidates the assumption.

### Required Work

- [ ] Coalesce `SetProp` by widget ID and property.
- [ ] Define structural barriers, including:
  - `ReplaceNode`
  - `ReplaceChildren`
  - Widget removal
  - Full document/root replacement
- [ ] Never move the latest command earlier than its original queue position.
- [ ] Preserve commands for distinct properties.
- [ ] Preserve commands targeting different widget generations.
- [ ] Track coalesced-command counts and high-water depth.

## 4.2 Paint/Extension Replacement

- [ ] Add an explicit coalescing option for full-snapshot extension repaint
      commands.
- [ ] Use it from `PaintWidget.repaint()`.
- [ ] Retain ordinary structural replacement behavior by default.
- [ ] Prevent coalescing across child-targeting or generation-changing
      barriers.

## 4.3 Streaming Commands

- [ ] Review line-plot append commands separately.
- [ ] Combine adjacent compatible append payloads where this preserves every
      point and `max_points` semantics.
- [ ] Preserve explicitly non-coalesced scatter and event-stream commands.
- [ ] Do not merge user input, callbacks, resource release, dialogs, exit,
      screenshots, or request/response commands.

## Tests

- [ ] Repeated `SetProp("progress", "value", ...)` leaves the newest value.
- [ ] Different properties on one widget remain distinct.
- [ ] Structural barriers prevent invalid coalescing.
- [ ] A stale widget generation cannot receive a coalesced update.
- [ ] Explicitly coalesced paint replacements retain the newest display list.
- [ ] Non-coalesced replacements remain ordered.
- [ ] Combined line appends preserve point order and bounds.
- [ ] Queue-depth accounting remains exact.

## Acceptance Criteria

- Repeated telemetry frames do not grow the queue in proportion to frame count.
- Commands with event or lifecycle semantics are never silently discarded.

---

# Phase 5 — Readiness API for Producers

**Priority:** P1
**Status:** Not started

## Problem

Applications have no supported way to start a producer after the first real
frame. `is_live` currently means that a handle is bound, not that rendering is
ready.

## Proposed API Direction

Exact naming should be finalized during implementation:

```python
app.on_ready(start_worker)
app.wait_until_ready(timeout=10.0)
app.is_ready
```

## Required Work

- [ ] Expose the native `ApplicationFramePresented` transition through
      `AppHandle`.
- [ ] Define one-shot `on_ready` callback behavior.
- [ ] Define thread-safe readiness waiting without blocking the UI thread.
- [ ] Keep `Widget.is_live` semantics unchanged for compatibility.
- [ ] Ensure closing before readiness releases waiters and reports closure.
- [ ] Ensure callbacks registered after readiness run exactly once.
- [ ] Add a recommended producer lifecycle:
  - Register readiness callback.
  - Start worker.
  - Use keyed `call_soon_threadsafe`.
  - Stop worker when the app closes.
- [ ] Update CATHODE-7 to use the readiness API instead of starting telemetry
      before `app.run()`.

## Tests

- [ ] `is_ready` is false while only the loading frame is visible.
- [ ] `on_ready` fires after the first application present.
- [ ] A late `on_ready` registration still runs once.
- [ ] `wait_until_ready` succeeds, times out, and handles close correctly.
- [ ] Readiness callbacks cannot execute on the producer thread accidentally.

## Acceptance Criteria

- Applications no longer need arbitrary startup sleeps.
- The stress demo starts live telemetry only after the UI is visible.

---

# Phase 6 — Loading and Queue Diagnostics

**Priority:** P1
**Status:** In progress

## Required Runtime Metrics

- [ ] Readiness state and transition timestamps. *(State is exposed; transition
      timestamps remain.)*
- [ ] Loading-frame present time.
- [ ] First application-frame present time.
- [ ] Queued Python task count and high-water mark.
- [ ] Coalesced/replaced Python task count.
- [ ] Native command queue depth and high-water mark.
- [ ] Coalesced native command count by command type.
- [ ] Oldest pending command age.
- [ ] Drain batches, commands, elapsed time, and continuation reason. *(Existing
      drain metrics plus continuation/yield state are exposed; reason remains.)*
- [ ] Frames presented while backlog was non-zero.

## Warning Improvements

Replace the current repeated message with a rate-limited diagnostic such as:

```text
DragonGUI: command backlog deferred
pending=760 drained=32 elapsed_ms=6.1 ready=false
coalesced=0 oldest_ms=7120 continuation=after-redraw
```

## Tests

- [ ] Debug snapshots expose all metrics with stable names.
- [ ] High-water marks never decrease.
- [ ] Coalescing counters match deterministic queue tests.
- [ ] Warning output is rate-limited and includes actionable context.
- [ ] Normal small applications emit no backlog warning.

## Acceptance Criteria

- A loading stall can be diagnosed from one debug snapshot or warning line.
- Diagnostics distinguish producer overload from GPU initialization failure.

---

# Phase 7 — Stress Regression Harness

**Priority:** P0 for deterministic tests; P1 for GPU smoke
**Status:** In progress

## Deterministic Python Reproducer

- [ ] Build the default CATHODE-7 window without opening a GUI.
- [ ] Assert the expected stress scale remains above a meaningful threshold.
- [ ] Simulate 25 pre-ready telemetry frames.
- [ ] Record the native command expansion.
- [ ] Assert keyed task coalescing reduces them to one frame.

## Native Scheduling Reproducer

- [ ] Extract or model the drain-continuation policy as a testable state
      transition.
- [ ] Seed a 750–1,000 command backlog.
- [ ] Assert a redraw/present opportunity occurs between drain slices.
- [ ] Assert the queue eventually reaches zero.

## GPU Smoke

- [x] Run CATHODE-7 with:
  - Default 96 tiles and 480 rows
  - Live telemetry enabled
  - `DRAGONGUI_SMOKE_FRAMES=3`
- [ ] Add a heavier variant for manual/nightly validation. *(A manual 20-frame
      live run is recorded; the automated variant remains.)*
- [ ] Assert:
  - [x] First application frame presents within the agreed startup budget.
  - [x] At least three application frames render.
  - [ ] Queue depth remains bounded.
  - [ ] Loading screen transitions exactly once.
  - [x] No repeated fairness-warning flood occurs.
- [ ] Keep `--no-live` as a baseline comparison.

## Proposed Initial Budgets

These should be calibrated on CI and development machines rather than treated
as final universal performance promises:

- First real frame: under 15 seconds on the current Windows development system.
- Pre-ready keyed telemetry callbacks retained: at most 1 per key.
- Native queue high-water under the default stress demo: below 256 after
  readiness/coalescing fixes.
- Frame progress under backlog: at least one present between drain
  continuations.

---

# Phase 8 — Workspace/Installation Identity

**Priority:** P1
**Status:** Not started

## Problem

Running an example from the repository does not guarantee that Python imports
the repository package. A stale installed native extension can be combined with
new example source.

## Required Work

- [ ] Extend `dg.backend_info()` with:
  - Python package path
  - Native extension path
  - Package version
  - Native build/version identifier
  - Capability/schema version
- [ ] Add a small local-runtime verification command or script.
- [ ] Make development documentation show how to run examples against:
  - The workspace package
  - An editable/development installation
  - An installed wheel
- [ ] Consider a diagnostic warning when an example under a DragonGui checkout
      imports a different DragonGui installation.
- [ ] Add the runtime identity to stress-demo startup diagnostics.

## Tests

- [ ] Backend information reports real resolved paths.
- [ ] Python and native version identifiers agree.
- [ ] The verifier detects a stale or mismatched native extension.
- [ ] Packaged wheels do not emit a false workspace warning.

## Acceptance Criteria

- A developer can immediately tell which Python and native DragonGui build is
  executing.

---

## Implementation Order

1. Phase 1 — First-application-frame barrier
2. Phase 2 — Render-before-continuation fairness
3. Phase 7 — Deterministic regression harness
4. Phase 3 — Keyed Python task coalescing
5. Phase 4 — Native command coalescing
6. Phase 5 — Readiness API
7. Phase 6 — Diagnostics
8. Phase 8 — Workspace/install identity
9. Phase 7 — Final GPU smoke and calibrated budgets

The first two phases fix the visible loading-screen failure even before every
backpressure optimization is complete. Task and command coalescing then bound
memory and latency under sustained producers.

---

## Cross-Cutting Safety Rules

- Default APIs preserve existing FIFO behavior unless coalescing is explicitly
  enabled or the command is a documented state assignment.
- State coalescing keeps the newest value.
- Stream coalescing preserves every data element and order.
- Event and lifecycle commands are never coalesced.
- Structural commands form barriers unless generation-aware logic proves a
  merge safe.
- Rendering progress takes precedence over maximizing commands per event-loop
  turn.
- Closing the app must wake readiness waiters and reject new work.

---

## Documentation Work

- [ ] Document the distinction between `is_live` and `is_ready`.
- [ ] Document snapshot tasks versus event tasks.
- [ ] Document `coalesce_key`.
- [ ] Document producer lifecycle and shutdown.
- [ ] Document native command coalescing guarantees.
- [ ] Document loading-screen transition semantics.
- [ ] Add queue-backlog troubleshooting guidance.
- [ ] Add workspace/install verification instructions.
- [ ] Update `dg.help` runtime scheduling content.

---

## Progress Checklist

### Correctness

- [x] First real frame cannot be starved by Python tasks.
- [x] Drain continuation cannot starve redraw.
- [ ] Structural and event command ordering is preserved.

### Backpressure

- [ ] Keyed Python tasks retain only the newest snapshot.
- [ ] Generic state assignments are coalesced safely.
- [ ] Paint telemetry replacements are optionally coalesced.
- [ ] Streaming commands preserve all data.

### API

- [ ] Applications can observe readiness.
- [ ] Producers can start after readiness without sleeps.
- [ ] Existing unkeyed scheduling remains compatible.

### Validation

- [ ] Deterministic CATHODE-7 reproducer passes.
- [x] Full Python suite passes.
- [x] Full native suite passes.
- [x] Three-frame live GPU smoke passes.
- [ ] Loading transition and queue budgets pass.
- [ ] Documentation and `dg.help` are current.
- [x] Local release extension is rebuilt and verified.

---

## Definition of Done

This remediation is complete when:

1. The default live CATHODE-7 stress demo reliably leaves the loading screen.
2. The first application frame presents before accumulated live telemetry runs.
3. A command backlog cannot prevent frame presentation.
4. Snapshot producers can explicitly coalesce obsolete Python callbacks.
5. Repeated generic state assignments do not grow the native queue without
   bound.
6. Event, lifecycle, and stream semantics remain correct.
7. Runtime diagnostics explain backlog, readiness, and coalescing behavior.
8. Developers can verify which Python and native DragonGui build is running.
9. Deterministic, full-suite, and live GPU regressions all pass.
