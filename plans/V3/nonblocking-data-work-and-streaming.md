# V3 Nonblocking Data Work And Streaming

DragonGUI has enough low-level pieces to update widgets from background
threads, but it does not yet have a first-class library model for long-running
data generation, packing, cancellation, and latest-frame delivery.

The practical symptom is visible in large scatter demos: generating a data set,
packing 125k+ points, or calling `Scatter3D.set_points(...)` from the wrong
place can stall the window even though the native renderer itself is capable of
high throughput.

## Current State

Implemented primitives:

- Python can start normal `threading.Thread` workers.
- `App.call_soon_threadsafe(fn)` can schedule a callable onto the live DragonGUI
  runtime.
- `AppHandle.enqueue_set_scatter_points_packed(...)` can enqueue immutable
  packed scatter payloads directly from Python to native.
- The native bridge has a thread-safe command queue and exposes queue depth in
  debug snapshots.
- Scatter has optimized native upload paths for packed `xyz_f32_v0` and
  `point_instance_v1` payloads.

Important limitation:

- `call_soon_threadsafe` is a UI/runtime scheduler, not a worker pool. Any heavy
  callable posted through it still runs while Python tasks are drained by the
  app.
- `Scatter3D.set_points(...)` mutates Python widget state and packs data
  synchronously before enqueueing the native command. This is fine for small
  updates, but bad for large generated frames inside UI callbacks.
- The command queue currently accepts every large update. Fast producers can
  outrun the UI thread and create queue backlog instead of dropping stale frames.

## Goals

- Keep the GUI responsive while Python generates or loads large data.
- Provide a safe public API for background jobs, progress, cancellation, and
  result delivery.
- Make large widget updates use immutable prepared payloads instead of mutating
  widget objects from worker threads.
- Add latest-wins delivery for high-rate visual streams.
- Preserve current simple APIs for small apps.
- Document the threading contract clearly enough that users do not mistake
  `call_soon_threadsafe` for background execution.

## Non-Goals

- A general-purpose replacement for `asyncio`, multiprocessing, or Dask.
- Making every Python operation interruptible.
- Allowing arbitrary widget tree mutation from background threads.
- Guaranteeing maximum render FPS for every hardware configuration.

## Phase 1: Define The Threading Contract

Document and enforce these rules:

- UI callbacks should stay short.
- `App.call_soon_threadsafe(fn)` runs `fn` on the DragonGUI Python task drain.
- Long data generation, blocking IO, and large serialization must run outside
  `call_soon_threadsafe`.
- Widget objects should be mutated on the UI thread unless the API is explicitly
  documented as thread-safe.
- Thread-safe update APIs must accept immutable payloads or values and enqueue
  native commands without touching widget Python state.

Deliverables:

- Update `docs/library-overview.md` and `docs/widgets-reference.md`.
- Add a short "Safe Background Work" docs page.
- Mark the current background scatter example as a legacy/simple pattern and
  add a large-data-safe version.

## Phase 2: Add App Task APIs

Add a small DragonGUI-managed worker facility.

Proposed API:

```python
handle = app.submit_task(
    build_dataset,
    args=(config,),
    on_result=lambda result: scatter.set_prepared_points(result),
    on_error=lambda exc: status.set_text(str(exc)),
    on_progress=lambda p: progress.set_value(p),
    latest_key="scatter-frame",
)

handle.cancel()
```

Behavior:

- Runs work on a bounded background thread pool.
- Returns a `TaskHandle` with `cancel()`, `done`, `running`, and `exception`.
- Provides a cancellation token to task functions that opt in.
- Posts result, error, and progress callbacks back through
  `call_soon_threadsafe`.
- Throttles progress callbacks so high-frequency tasks cannot flood the queue.
- Supports `latest_key`, where a newer task result can supersede older stale
  results before UI application.

Implementation notes:

- Use `concurrent.futures.ThreadPoolExecutor` initially.
- Keep the executor owned by `App` so shutdown can cancel or drain tasks.
- Capture exceptions and expose them through callback and debug monitor state.
- Avoid storing large result history in `App`; once a callback consumes a
  result, release it.

## Phase 3: Add Prepared Data Payloads

Separate expensive data preparation from widget mutation.

Proposed scatter API:

```python
payload = dg.Scatter3D.prepare_points(
    frame,
    x="x",
    y="y",
    z="z",
    scalars="intensity",
    colormap="viridis",
)

scatter.set_prepared_points(payload)
```

Properties:

- `prepare_points(...)` is pure data work and can safely run on a worker.
- The returned `ScatterPayload` is immutable, owns or references immutable
  bytes, and includes metadata needed by native.
- `set_prepared_points(...)` enqueues the payload without repacking.
- `set_prepared_points(...)` may update UI-side metadata on the UI thread, but
  should also have a lower-level `enqueue_prepared_points(...)` latest-frame path
  for high-rate streaming.

Initial payload fields:

- `widget_kind`
- `payload_format`
- `bytes`
- `point_count`
- `colormap`
- `axis_labels`
- `hover_meta`
- `scalar_bar_meta`
- `pack_ms`
- optional `bounds`

Follow-up payload types:

- table row batches
- image frames
- line/mesh overlays

## Phase 4: Add Latest-Frame Streaming

Add a first-class streaming API for widgets where old frames become stale.

Proposed API:

```python
stream = scatter.stream_frames(
    source=frame_source,
    prepare=dg.Scatter3D.prepare_points,
    policy="latest",
    max_pending=1,
    target_hz=60,
)

stream.start()
stream.stop()
```

Behavior:

- Source and prepare work happen off the UI thread.
- Only the latest pending frame for a widget is kept when the producer outruns
  the consumer.
- UI callbacks remain responsive while frames are generated.
- Stream exposes produced frames, submitted frames, dropped frames, average
  preparation time, average enqueue latency, and native upload timing where
  available.

Native command queue changes:

- Add coalescing for latest-wins commands, keyed by command kind and widget id.
- Start with `SetScatterPointsPacked`.
- Do not coalesce commands with semantic ordering requirements such as clicks,
  modal close requests, or toast lifecycle commands.

Python changes:

- Add `enqueue_latest_scatter_points_packed(...)` or a general latest-command
  wrapper.
- Expose `App.command_queue_depth()` and stream queue metrics to Python.

## Phase 5: Scatter-Specific Ergonomics

Add direct setters for cheap visual properties that should never repack points.

Needed APIs:

```python
scatter.set_point_size(3.0)
scatter.set_opacity(0.85)
scatter.set_colormap("magma", repack=False)
scatter.set_scalar_range(vmin, vmax)
```

Rules:

- Point size and opacity should patch style/native render state when possible.
- Repacking should happen only when the underlying per-point payload changes.
- The API should make it obvious which calls are cheap and which calls rebuild
  point buffers.

## Phase 6: Cancellation And Progress

Provide cancellation and progress as standard task features, not one-off demo
code.

Worker function patterns:

```python
def build_frames(token, progress):
    for i in range(frame_count):
        token.throw_if_cancelled()
        frame = make_frame(i)
        progress(i / frame_count)
    return frames
```

Requirements:

- Cancellation should prevent stale result callbacks.
- Progress callbacks should be rate-limited by default.
- Task exceptions should be captured, surfaced to the app, and visible in debug
  snapshots or a future `ThreadMonitor`.

## Phase 7: Tests And Probes

Add regression coverage before relying on the API in demos.

Tests:

- A submitted task that sleeps or generates data does not block UI task drains.
- Cancelling a task prevents its result callback.
- A newer `latest_key` result supersedes an older one.
- Scatter prepared payloads match `set_points(...)` payload output.
- Latest scatter frame queue drops stale frames under load.
- Command queue depth remains bounded during a synthetic 125k point stream.

Probes:

- `examples/css_feature_probes/background_work_probe.py`
  - generate a large dataset with progress and cancel controls
  - button hover/click remains responsive during generation
- `examples/css_feature_probes/scatter3d_streaming_pipeline_probe.py`
  - source generates 125k point frames
  - stream uses latest-frame policy
  - shows produced/submitted/dropped/uploaded frame counts

## Phase 8: Documentation And Migration

Document three patterns:

- Small update: call `scatter.set_points(...)` directly from a callback.
- Large one-shot update: `app.submit_task(..., on_result=scatter.set_prepared_points)`.
- High-rate stream: `scatter.stream_frames(..., policy="latest")`.

Update existing examples:

- Replace large-frame `call_soon_threadsafe(lambda: scatter.set_points(...))`
  examples with prepared payload or streaming APIs.
- Keep one simple thread example for education, clearly labeled as small-data
  only.

## Acceptance Criteria

- Generating a 125k point frame from a button click does not freeze the window.
- Cancelling generation returns control immediately and prevents stale plot
  replacement.
- A 125k point stream can run with `max_pending=1` without command drain spam.
- Point-size changes on a running scatter do not repack or re-upload point data.
- Debug snapshots expose enough queue/task state to diagnose producer overload.
- Documentation clearly says `call_soon_threadsafe` schedules UI work and is not
  a background worker.
