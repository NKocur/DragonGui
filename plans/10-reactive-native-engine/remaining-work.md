# Reactive Native Engine Remaining Work

## Purpose

This document is the audit handoff for the reactive native engine effort. The
main R1-R6 architecture is implemented: live command queue, event-loop wake,
live widget mutation, live scatter updates, structured inline styles, VDOM
patches, keyed components, debug snapshots, retained table resources, and
generic buffer resource commands.

The items below are the remaining known gaps, deferred work, and audit focus
areas.

## Remaining Implementation Work

### 1. Arrow C Data Interface Resources

Status: not implemented.

What exists now:

- `DataFrameTable` sends supported NumPy-backed numeric columns through the
  Python buffer protocol.
- UTF-8/object columns are packed into offset buffers.
- Rust copies incoming buffers into retained native memory.
- Startup JSON remains bounded.

What is still needed:

- A pinned-owner lifetime design for Arrow arrays/tables.
- A Rust-side `ArrowTableHandle` or equivalent resource type.
- Explicit ownership rules for Python/Arrow objects referenced by Rust.
- Release behavior for Arrow resources.
- Snapshot metadata that reports schema, rows, columns, byte sizes, and owner
  state without serializing data.

Audit concern:

Do not add a fake zero-copy path. Arrow support should not enter runtime until
the owner/lifetime model is explicit and tested.

### 2. Image And Texture Widgets

Status: foundation exists, widgets are not implemented.

What exists now:

- `SetBufferResource { id, kind, bytes }`
- `ReleaseResource { id }`
- `App.set_buffer_resource(...)`
- `App.release_resource(...)`
- Debug snapshots report generic retained buffer resources.

What is still needed:

- `dg.Image(...)` Python widget API.
- Native image resource metadata: width, height, format, color space.
- Texture upload path from generic buffer resource to `wgpu::Texture`.
- Rendering primitive or dedicated image renderer.
- Update path for live image replacement.
- Release path for texture/GPU memory.

Audit concern:

The generic buffer registry stores bytes only. It is not yet a GPU texture
registry.

### 3. Live Scatter Benchmark Artifact

Status: telemetry and demo exist; benchmark should be tightened.

What exists now:

- Live scatter updates report pack time, queue latency, decode time, upload
  time, native total time, payload bytes, and point count.
- `examples/streaming_scatter_tool.py` has a `Print Metrics` button.
- `benches/bench_scatter.py` still focuses on startup/steady-frame timing.

What is still needed:

- A dedicated live-update benchmark or an expanded `bench_scatter.py`.
- Separate reporting for:
  - startup upload
  - live update pack time
  - live command latency
  - native decode
  - GPU upload
  - p50/p95/p99 update time
- Comparable point counts for the Dear PyGui benchmark story.

### 4. Debug Snapshot Callback Limitation

Status: known limitation.

Current behavior:

- `app.debug_snapshot()` works from background threads and after `App.run(...)`
  returns.
- Calling it directly inside a UI callback can time out because the event loop
  cannot service the snapshot request until the callback returns.

Possible fixes:

- Return the latest cached snapshot immediately when called from the UI thread.
- Detect UI-callback context and raise a clearer error.
- Add an async/deferred snapshot request API.

Audit concern:

The current limitation is acceptable for now, but the error path should remain
clear and documented.

### 5. Stylesheet Runtime

Status: intentionally deferred.

What exists now:

- Structured inline `style={...}` maps.
- Pseudo-state nested styles: `hover`, `active`, `focus`, `disabled`.
- Theme token resolution.
- `selector_prototype.py`
- `stylesheet-research.md`

What is still needed before runtime stylesheet support:

- Computed-style cache.
- Selector invalidation rules.
- Specificity and source-order resolution.
- Clear precedence between inline styles and stylesheet rules.
- Debug snapshot metadata for computed style and dirty reasons.

Audit concern:

Selector matching should not be merged into the renderer until computed-style
caching and invalidation are designed.

### 6. Packaging And Compatibility Follow-Up

Status: changed by buffer protocol work.

Current state:

- Native backend now targets `abi3-py311`.
- `pyproject.toml` now requires Python `>=3.11`.

What is still needed:

- Confirm wheel build matrix matches Python 3.11+ only.
- Confirm documentation and setup instructions no longer claim Python 3.10.
- Confirm `start.bat` and development docs use the expected Python version.

Audit concern:

The Python 3.10 support drop is intentional because PyO3's safe buffer API is
not exposed under the Python 3.10 limited ABI. This should be called out in
release notes when packaging starts.

## Audit Focus Areas

### Resource Lifetime

Check:

- Table resources are released when no longer present in the retained widget
  tree.
- Explicit `ReleaseResource` removes generic buffers.
- Stale release commands log/drop instead of panicking.
- Debug snapshots never serialize large resource payloads.

### Threading

Check:

- Background commands wake the event loop.
- Commands drain before redraw.
- Python callbacks do not mutate Rust renderer state directly.
- Closed handles raise in synchronous Python paths.
- Stale background work logs/drops safely.

### Dirty Flags

Check:

- `SetProp`, `SetStyle`, `ReplaceChildren`, `ReplaceNode`, scatter updates,
  table updates, and resource commands mark reasonable dirty flags.
- Over-invalidation is acceptable; stale UI is not.
- Dirty history in snapshots matches actual command behavior.

### Component State

Check:

- Duplicate state keys fail clearly.
- Nested keyed component state survives parent rerenders.
- Removed child component state does not leak indefinitely.
- Dynamic callback registration stays correct after component rerenders.

### Data Paths

Check:

- Startup scatter base64 path still works.
- Live scatter raw-byte path works.
- Startup table JSON remains bounded.
- Startup table column resources drain after native sender binding.
- Live table updates use retained resources.
- Numeric NumPy columns avoid extra Python `bytes(...)` conversion.

## Practical Next Steps

Recommended audit order:

1. Audit R1 command queue and event-loop wake behavior.
2. Audit R2 scatter live updates and telemetry.
3. Audit R3 style parsing, token resolution, and dirty flags.
4. Audit R4/R5 patching and component rerender behavior.
5. Audit R6 debug snapshots and redaction.
6. Audit retained resource lifecycle.
7. Decide whether the next implementation track is Arrow resources, image
   widgets, or live benchmarks.

