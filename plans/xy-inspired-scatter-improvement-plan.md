# XY-Inspired Scatter Architecture Improvement Plan

**Created:** 2026-08-05  
**Status:** In progress  
**Scope:** `ScatterPlot2D`, `Scatter3D`, shared column storage, GPU resource
management, startup instrumentation, and visualization benchmarks

## Purpose

The DragonGUI 1.0.0 comparison with Reflex XY 0.0.5 exposed two different
performance profiles:

- DragonGUI has strong retained-scene performance after startup, averaging
  approximately 3.3-6.2 ms per frame through four million source points.
- XY reaches a correct, stable first chart much faster and scales beyond
  DragonGUI's current point ceiling by separating exact source data from the
  representation sent to the renderer.

This plan adapts the useful architectural ideas behind XY to DragonGUI's native
desktop and streaming use cases. It does not propose copying XY's web runtime or
replacing DragonGUI's wgpu renderer.

Supporting measurements are in
[`docs/dragongui-vs-xy-benchmark.md`](../docs/dragongui-vs-xy-benchmark.md).

## Implementation Log

### 2026-08-05: bounded allocation growth and compact bounds scan

Implemented the first P0.1 guard and tested it against the original ceiling:

- Scatter and LOD buffer growth now keeps 2x headroom only when it fits within
  `max_buffer_size`; otherwise capacity is clamped to the device limit.
- Requests larger than one device buffer are rejected before calling wgpu, so
  they cannot trigger the previous validation panic.
- Compact `xyz_f32_v0` payloads can now compute bounds in one read-only scan and
  remain in their 12-byte GPU layout. Missing producer telemetry no longer
  forces expansion to the 32-byte `PointInstance` layout.
- Benchmark validation now requires `payload_status == "Ok"` and
  `effective_draw_point_count == source_rows`, preventing a presented but empty
  chart from being reported as successful.

Measured results on the same 256 MiB-buffer adapter:

| Case | Before | After | Finding |
|---|---:|---:|---|
| 5M exact | wgpu allocation panic | 5M points rendered | Clamping growth removes the false 320 MB allocation. |
| 4M exact frame average | 3.47-6.83 ms | 5.48 ms | No measurable regression outside baseline variance. |
| 4M exact primary upload | 28.94-31.88 ms | 31.47 ms | No material upload regression. |
| 10M exact | allocation panic | 10M points rendered | Compact bounds scan avoids the 320 MB expanded representation. |
| 10M compact bounds | not available | 19.22 ms | Linear scan is cheap enough to preserve the compact fast path. |
| 10M peak RSS | blank guarded probe: ~844 MiB | ~587 MiB | Avoiding expanded instances saves roughly 257 MiB. |

The 10M probe also demonstrated why render-aware validation is mandatory: the
first guarded build presented frames and reported source rows but drew zero
points. `effective_draw_point_count` exposed the false positive immediately.

Remaining P0.1 work includes exact-mode chunking above the compact
single-buffer ceiling, automatic adaptive fallback where policy permits, and
equivalent structured handling for secondary actor and LOD allocations.

### 2026-08-05: adapter, representation, and GPU-memory telemetry

Implemented the core P0.2 snapshot contract:

- Renderer telemetry now publishes adapter name, vendor/device identifiers,
  device type, backend, driver, and relevant device buffer limits.
- Every scatter publishes the selected primary representation and reason,
  source and rendered rows, reduction ratio, retained CPU bytes, payload bytes,
  and active primary stride.
- GPU buffer allocation is reported by role: primary, LOD, extra actors,
  chrome, meshes, and uniforms, along with used primary/LOD bytes and a summed
  total.

The 10M exact verification snapshot now explains the decision without source
inspection:

| Field | Observed value |
|---|---:|
| Adapter | NVIDIA GeForce RTX 3080 Ti / DX12 |
| `max_buffer_size` | 256 MiB |
| Representation | `compact_xyz_f32_v0` |
| Selection reason | `position_only_payload_uses_compact_layout` |
| Source / render rows | 10,000,000 / 10,000,000 |
| Primary used / allocated | 114.44 / 228.88 MiB |
| Total scatter GPU buffers | 229.13 MiB |

An automated artifact check verified that the primary allocation stayed below
the reported device limit and that all per-role allocation values sum exactly
to the reported total. The scatter-focused Python suite remained green at 185
tests.

Remaining P0.2 work is cache byte accounting and chunk-count telemetry once
those representations exist. These are deferred to the phases that introduce
the corresponding caches and chunked buffers.

### 2026-08-05: startup phase telemetry and automatic backend selection

Implemented P0.3 phase timing from native application entry through first
application presentation. The snapshot now separates:

- event-loop resume and window creation;
- instance/surface creation, adapter request, device request, and surface
  configuration;
- startup scatter resources;
- primitive, line-plot, image, scatter-compositor, and text renderer creation;
- state assembly and initial layout;
- total GPU initialization and first application presentation.

The measurements disproved the earlier assumption that scatter ingest caused
the roughly five-second cold start. On the 100K case under forced DX12:

| Phase | Time |
|---|---:|
| First application presentation | 5,281 ms |
| Renderer initialization | 4,681 ms |
| Adapter request | 214 ms |
| Device request | 108 ms |
| Initial layout | 113 ms |
| Startup scatter resources | 33 ms |

A deeper cold run attributed the renderer cost primarily to synchronous shader
pipeline creation in primitives and text. Testing DragonGUI's existing `auto`
backend policy selected Vulkan on the benchmark system and reduced the 100K
first application presentation to 1,568 ms. The 10M exact workload completed in
1,853 ms wall time with first application presentation at 1,679 ms, while still
rendering all 10M points.

Windows now uses automatic backend selection by default instead of forcing
DX12. Users can retain the old behavior with
`DRAGONGUI_WGPU_BACKEND=dx12`; explicit `vulkan` and `gl` overrides remain
available. On this machine the default change improved 100K cold presentation
by approximately 3.4x and 10M wall time by approximately 3.1x.

Further startup work should focus on pipeline caching or lazy pipeline
construction. The instrumentation is now detailed enough to judge those changes
independently of adapter, layout, and data-upload costs.

### 2026-08-05: concurrent independent renderer initialization

Primitive, text, line-plot, image, and scatter-compositor constructors create
independent GPU resources. They now initialize concurrently on scoped threads
using wgpu's thread-safe device and queue handles.

Three fresh-process 100K samples produced:

| Run | Renderer initialization | First application presentation |
|---:|---:|---:|
| 1 | 396 ms | 1,406 ms |
| 2 | 383 ms | 1,409 ms |
| 3 | 365 ms | 1,349 ms |

The sequential automatic-backend baseline was 538 ms for renderer
initialization and 1,568 ms to first application presentation. Concurrent
initialization therefore reduced renderer wall time by 26-32% and first
presentation by roughly 10-14%.

The 10M exact workload improved from 1,853 ms to 1,581 ms total wall time and
from 1,679 ms to 1,409 ms for first application presentation. It continued to
render all 10M points with the compact representation. The full Python API and
VDOM suite passed all 485 tests.

Primitive initialization is now the longest renderer subphase at roughly
365-395 ms. The next startup experiment should compile its simple, line, and
complex pipelines concurrently or introduce a persistent wgpu pipeline cache.

### 2026-08-05: concurrent primitive pipeline compilation

The primitive renderer's simple rectangle, line segment, and complex rectangle
pipelines shared nearly identical descriptors but were created serially. Their
descriptor construction is now factored into one helper and the three pipelines
are requested concurrently.

On Vulkan, three fresh-process samples averaged approximately 358 ms for
primitive/renderer initialization versus approximately 381 ms before this
change, a modest 6% improvement. First presentation averaged 1,366 ms versus
1,388 ms. DX12 still serialized or dominated this work at the driver/device
layer, so inner concurrency did not materially improve the explicit DX12 path.

The final 10M verification rendered all points using `compact_xyz_f32_v0` in
1,640 ms wall time with a 6.37 ms sampled frame average. All 485 Python API and
VDOM tests passed. Because the refactor also removes three duplicated pipeline
descriptors, the small Vulkan improvement is retained; persistent pipeline
caching remains the higher-leverage next experiment.

### 2026-08-05: persistent Vulkan pipeline cache

Implemented WGPU's application-managed pipeline cache for supported adapters:

- The `PIPELINE_CACHE` feature is requested only when the selected adapter and
  backend support it.
- Cache filenames use WGPU's adapter-specific key. WGPU validates its cache
  header, adapter, driver, and version before using persisted bytes, with safe
  fallback for stale or invalid data.
- The default cache lives in the platform user cache directory. It can be
  disabled with `DRAGONGUI_PIPELINE_CACHE=off` or redirected with
  `DRAGONGUI_PIPELINE_CACHE_DIR`.
- Snapshot telemetry reports status, path, loaded/saved bytes, and load/save
  durations. Unsupported backends such as DX12 continue without a cache.

Two independent empty cache directories produced 103,109-byte Vulkan caches.
Repeated fresh-process results were:

| Cache state | Primitive init | Renderer init | First presentation |
|---|---:|---:|---:|
| Cold cache A | 322.6 ms | 323.2 ms | approximately 1.33 s |
| Cold cache B | 325.9 ms | 326.3 ms | 1.334 s |
| Warm run 1 | 13.7 ms | 129.9 ms | 1.090 s |
| Warm run 2 | 14.2 ms | 140.9 ms | 1.073 s |
| Warm run 3 | 17.8 ms | 136.8 ms | 1.131 s |

Cache loading cost approximately 9 ms and saving cost approximately 3 ms. The
10M warm-cache workload rendered all 10M points in 1,393 ms total wall time,
with first presentation at 1,226 ms and a 6.32 ms sampled frame average. DX12
correctly reported `unsupported_or_disabled` and remained functional. All 485
Python API and VDOM tests passed.

With primitive compilation mostly removed from warm startup, the largest
remaining renderer component is glyphon text initialization at approximately
125-160 ms. Overall GPU startup is now dominated by instance/surface, adapter,
device, and surface-configuration work rather than DragonGUI shader pipelines.

### 2026-08-05: overlap system-font discovery with GPU initialization

Profiling split glyphon initialization into font discovery, swash cache,
atlas/viewport, and renderer pipeline phases. Fresh-process warm-cache samples
showed that `FontSystem::new()` consumed 120-133 ms of a 138-151 ms text
constructor; atlas/viewport setup was approximately 3 ms and the cached text
pipeline approximately 15 ms.

System-font discovery is CPU-only, so DragonGUI now starts it before requesting
the wgpu adapter and device. The fully populated `FontSystem` is moved into the
text renderer after GPU setup. Font selection and fallback behavior are
unchanged; the work is hidden behind independent GPU initialization rather
than removed. A new `text_font_system_wait_ms` snapshot field distinguishes
total font work from time spent on the critical path. Apps with the optional
startup loading screen retain the synchronous path to avoid duplicate font
scans between its short-lived renderer and the application renderer.

Three 100K exact samples produced:

| Run | Font discovery work | Font wait | Text constructor | Renderer init | First presentation |
|---:|---:|---:|---:|---:|---:|
| 1 (new pipeline cache) | 149.4 ms | 0.012 ms | 2.40 ms | 300.2 ms | 1,265 ms |
| 2 (warm cache) | 113.8 ms | 0.006 ms | 1.42 ms | 7.83 ms | 922 ms |
| 3 (warm cache) | 124.9 ms | 0.007 ms | 1.23 ms | 7.18 ms | 909 ms |

Before this overlap, warm-cache text construction took 138-151 ms, renderer
initialization took 130-141 ms, and first presentation took 1.07-1.13 seconds.
The warm text constructor is now approximately 99% faster on the critical path,
and first presentation improved by roughly 150-200 ms in comparable runs.

The exact 10M verification rendered all 10,000,000 points in 1,109 ms wall
time, with first presentation at 992 ms, a 1.44 ms text constructor, and only
0.006 ms waiting for the 115.8 ms font scan. All 485 Python API and VDOM tests
passed.

### 2026-08-05: reuse loading-screen text resources

The optional startup loading screen previously constructed and dropped a full
glyphon text renderer, after which application startup repeated system-font
discovery, atlas allocation, viewport setup, and text pipeline creation. The
loading frame now returns its populated `TextRendererDg` to application
startup. The normal application rebuild drains the loading labels and replaces
them with application text while retaining the font database, glyph cache,
atlas, viewport, and GPU pipelines.

Snapshot telemetry now reports `text_reused_loading_renderer`. The benchmark
case also accepts `--loading-screen`, allowing this path to remain in the
repeatable startup matrix.

The same responsive-app loading-screen smoke before and after reuse measured:

| Phase | Before | After | Improvement |
|---|---:|---:|---:|
| Second text initialization | 122.67 ms | 0.001 ms | effectively eliminated |
| Renderer initialization | 123.75 ms | 6.20 ms | 95.0% |
| First application presentation | 1,284 ms | 1,059 ms | 17.5% |

Three 100K exact loading-screen runs reported reuse on every run, with second
text initialization at 0.0011-0.0012 ms, renderer initialization at 6.56-7.99
ms, and first application presentation at 1.063-1.091 seconds. The loading
frame itself continued to present in 124-134 ms.

The 10M exact loading-screen verification rendered all 10,000,000 points in
1,276 ms wall time and presented the correct application frame at 1,158 ms.
The reused text handoff took 0.0013 ms and sampled retained frames averaged
1.42 ms. All 485 Python API and VDOM tests passed.

### 2026-08-05: structured exact-scatter capacity errors

Primary compact and raw exact uploads now distinguish an unsupported fast path
from a device-capacity failure. A rejected allocation records the requested
points, required bytes, active stride and representation, device
`max_buffer_size`, suggested remedies, and `payload_status=CapacityExceeded`.
The runtime exits cleanly and `App.run()` raises the public
`dragongui.ScatterCapacityError` instead of presenting an empty chart or
allowing a wgpu validation panic.

Boundary tests cover requests one byte below, exactly at, and one byte above a
synthetic device limit. A permanent end-to-end capacity probe requests
22,369,622 compact points on the benchmark adapter:

- required compact buffer: 268,435,464 bytes;
- device limit: 268,435,456 bytes;
- result: `ScatterCapacityError` with the exact point count, byte count, limit,
  representation, 12-byte stride, and adaptive/chunking remedies;
- no GPU primary buffer was allocated and no native panic occurred.

The 10M exact control remained valid with all 10,000,000 points rendered,
`capacity_error=null`, and first application presentation at 1,039 ms. The
public Python API and VDOM suite now passes 486 tests.

### 2026-08-05: true 8-byte exact 2D scatter representation

Implemented the next M2 compact-layout improvement end to end. Plain
`ScatterPlot2D` data no longer creates or transmits a synthetic zero-Z column:

- Python packs `xy_f32_v0` as interleaved little-endian float32 pairs;
- the native command and document formats validate an 8-byte stride;
- native bounds, fallback decoding, picking materialization, and capacity
  errors understand XY payloads with an implicit `z=0`;
- wgpu uploads the XY bytes directly with a `Float32x2` instance layout and a
  dedicated flat-scatter shader;
- representation and GPU-memory telemetry report `compact_xy_f32_v0` and an
  8-byte primary stride;
- styled 2D points still use `point_instance_v1`, preserving per-point color,
  size, opacity, scalar, and NaN behavior.

Measured exact-mode results on the same Vulkan RTX 3080 Ti adapter with a
268,435,456-byte `max_buffer_size`:

| Case | Result | Payload / primary used | Native bounds | Primary upload | Native total | Peak RSS |
|---|---:|---:|---:|---:|---:|---:|
| 100K | valid, all points | 0.8 MB | 0.20 ms | 0.14 ms | 0.39 ms | 208 MiB |
| 10M | valid, all points | 80 MB | 19.61 ms | 36.83 ms | 57.34 ms | 453 MiB |
| 33,554,432 | valid, all points | 256 MiB | 75.55 ms | 54.33 ms | 137.15 ms | 1,205 MiB |
| 33,554,433 | structured rejection | 256 MiB + 8 bytes required | n/a | no allocation | n/a | n/a |

Compared with the previous 12-byte XYZ compact run at 10M, source payload and
used GPU bytes fell from 120 MB to 80 MB (33.3%). Peak process RSS fell from
approximately 626 MiB in the comparable warm-cache artifact to 453 MiB in the
final XY run. Upload timing increased from approximately 31 ms to 37 ms in
these individual samples, so the memory win is established but upload-speed
claims require a multi-run median before treating the difference as causal.

The exact single-buffer ceiling increased from 22,369,621 XYZ points to
33,554,432 XY points, a 50% increase. The one-point-over probe raised
`ScatterCapacityError` with the exact 268,435,464-byte request and 8-byte
stride. The Python API and VDOM suite passes 487 tests; Rust test executables
compile successfully. Benchmark artifacts are stored under
`artifacts/xy-benchmark/dragongui-xy-f32-*.json`.

### 2026-08-06: chunked exact XY rendering across buffer limits

Implemented the first M3 chunked exact renderer for compact 2D points. An
`xy_f32_v0` payload larger than the adapter's `max_buffer_size` is now split on
an 8-byte point boundary into reusable vertex buffers. Each buffer is drawn
with its own instance count while the runtime retains one logical payload,
global point count, bounds, revision, and picking index space.

The implementation adds:

- deterministic chunk planning aligned to the active point stride;
- reuse and bounded growth of the first and additional GPU buffers;
- one draw call per exact chunk with no duplicated or omitted boundary row;
- `chunked_xy_f32_v0` representation telemetry, selection reason, chunk count,
  aggregate used bytes, and aggregate allocated bytes;
- unit coverage for exact-at-limit, one-point-over, three chunks, odd synthetic
  device limits, and partial-point rejection;
- a separate XYZ capacity probe so unchunked representations continue to prove
  structured failure behavior.

Measured results on the same 256 MiB-buffer Vulkan adapter:

| Case | Representation | Chunks | Used / allocated GPU point bytes | Bounds | Upload | Native total | Result |
|---|---|---:|---:|---:|---:|---:|---|
| 10M control | `compact_xy_f32_v0` | 1 | 80.0 / 160.0 MB | 23.83 ms | 37.68 ms | 62.52 ms | 10M / 10M rendered |
| 33,554,433 | `chunked_xy_f32_v0` | 2 | 268.44 / 272.63 MB | 73.37 ms | 52.52 ms | 127.23 ms | former failure boundary now renders |
| 40M | `chunked_xy_f32_v0` | 2 | 320.0 / 371.56 MB | 82.60 ms | 78.56 ms | 163.07 ms | 40M / 40M rendered |

The one-point-over allocation includes the existing 4 MiB minimum reuse
capacity for its second buffer. The 40M second chunk receives bounded 2x
headroom, explaining why allocated bytes exceed used bytes. Both policies avoid
reallocating on ordinary growth while every individual buffer remains at or
below the reported 256 MiB device limit.

The 10M control's 2.34 ms sampled frame average matches the preceding 2.33 ms
run, while upload and native-total timings remain within the observed
single-run variance. All 487 Python API and VDOM tests pass, Rust test
executables compile, and the updated unchunked XYZ probe still raises the exact
structured error at 268,435,464 bytes.

Current scope is intentionally narrow: primary opaque `ScatterPlot2D` XY data
only. Styled `point_instance_v1`, compact XYZ, secondary actors, and LOD buffers
still use one buffer and retain structured capacity errors. Chunking also
removes the per-buffer limit, not the need for an aggregate GPU-memory budget;
the adaptive representation planner remains responsible for bounding very
large workloads on lower-memory adapters.

### 2026-08-06: reusable Python `PointStore` and shared XY payload cache

Implemented the first P2.1 source/render separation slice as the public
`dragongui.PointStore` API. Existing dataframe and mapping inputs remain fully
compatible; `ScatterPlot2D(store)` now defaults to the store's `x` and `y`
columns.

The initial store provides:

- exact contiguous numeric columns with preserved authored dtypes;
- `borrowed`, `copied`, and `moved` ownership contracts;
- implicit stable positional row IDs or explicit caller row IDs;
- global source/data revisions and per-column revisions;
- selectively invalidated finite-mask, bounds, and packed-XY caches;
- column replacement and explicit `touch(...)` for externally mutated borrowed
  arrays;
- source/cache byte accounting through `stats()`;
- one shared `xy_f32_v0` payload object for compatible plots.

Borrowed columns are exposed through read-only views, but their original caller
arrays can still change. The caller must invoke `store.touch("column")` after
such a mutation. Existing plots intentionally keep their acknowledged payload;
calling `plot.set_points(store)` advances that plot to the new store revision.
Copied ownership isolates subsequent caller mutation. Moved ownership may reuse
and make compatible input arrays read-only.

Three fresh-process 10M-row/two-plot construction samples measured:

| Input path | Median build | Widget RSS growth | Referenced payload | Unique retained payload |
|---|---:|---:|---:|---:|
| Shared legacy frame arrays | 182.06 ms | 160.13 MB | 160 MB | 160 MB |
| Shared `PointStore` | 145.76 ms | 80.15 MB | 160 MB | 80 MB |

The store reduced retained widget-side payload memory by 49.9% and median
construction time by 19.9%. A native 100K/two-plot smoke rendered both
scatters from the same read-only payload, reporting 100,000 points for each and
a 7.40 ms first sampled frame.

The first benchmark also found a pre-existing scaling bug in row-label
detection: an implicit `range(10_000_000)` was materialized into Python lists
twice per plot. Recognizing positional ranges without allocation reduced the
store benchmark from an invalid 1.31-second first attempt to the measured
145.76 ms median and also benefits large pandas-style range indexes.

All 491 Python API and VDOM tests pass. The repeatable artifacts are generated
by `benchmarks/point_store_case.py` and stored under
`artifacts/xy-benchmark/point-store-*.json`.

This slice shares canonical Python columns and packed CPU payloads. Each plot
still owns its own native retained bytes and GPU buffers after submission. The
next P2.1 step is a native store/resource identity and revision protocol so
multiple plots can reference one native exact source while releasing or
rebuilding derived GPU representations independently.

### 2026-08-06: native `PointStore` identity and shared retained payloads

Implemented the next P2.1 slice. A `PointStore` now exposes a stable opaque
native identity plus a monotonically increasing data revision. Compatible
`ScatterPlot2D` startup, live `set_points(...)`, and colormap re-submissions use
an additive store-aware command when the loaded native backend supports it.
Older native backends automatically receive the unchanged direct packed-payload
command, including commands queued before the sender binds.

The native renderer interns immutable compact payloads by
`(store_id, revision)` and each scatter retains an `Arc<[u8]>` to that exact
allocation. The registry holds weak references, so an obsolete revision is
released as soon as the last scatter advances or disappears. Direct dataframe,
mapping, styled-point, and non-XY paths retain their existing behavior.

Debug snapshots now report both the source attached to each scatter and global
unique store accounting under `resources.point_stores`:

- live revision count;
- unique retained payload bytes;
- number of scatter references;
- per-scatter store ID, revision, payload bytes, and reference count.

The final 10M-row/two-plot native probe reported:

| Metric | Result |
|---|---:|
| Rendered points | 10,000,000 per plot |
| Scatter references | 2 |
| Referenced native payload | 160 MB |
| Unique retained native payload | 80 MB |
| Native retained-payload reduction | 50% |
| Sampled frame average | 4.59 ms |

The definitive artifact is
`artifacts/xy-benchmark/point-store-native-shared-10m.json`; the smaller 100K
protocol probe is retained alongside it. The benchmark script now captures
native source and registry telemetry in addition to Python payload identity.
All 494 Python API and VDOM tests pass, `cargo check` passes, and Rust test
executables compile successfully.

This slice deduplicates steady-state native CPU retention, not every transient
copy: each Python/native command still materializes a temporary `Vec<u8>` before
the second submission resolves to the interned allocation. GPU point buffers
also remain plot-owned, because their lifecycle will ultimately depend on each
plot's viewport and chosen representation. The next source/render separation
step should eliminate duplicate boundary transfers through explicit
register-once/reference commands, then evaluate sharing identical exact GPU
representations where viewport policy permits it.

### 2026-08-06: publish-once native boundary transfer

Implemented the boundary-transfer follow-up with a publish-and-reference
protocol. The first plot using a `(store_id, revision)` sends the immutable
packed payload and binds to it. Later plots in the same app send only their
widget ID, store ID, revision, colormap, and camera-fit policy. The native
reference command clones the existing `Arc<[u8]>` and builds the plot-owned GPU
representation without materializing another Python/native payload copy.

Startup required special handling because scatter resources are queued before
the native sender binds. The pending queue now records a deferred
reference-or-payload operation. At bind time it selects a lightweight reference
when both new native methods exist, the previous store-aware payload command for
an intermediate backend, or the original direct payload command for an older
backend. This keeps mixed Python/native version fallback lossless.

The final 10M-row/two-plot probe reported exactly one payload publication and
one lightweight reference:

| Command/metric | Result |
|---|---:|
| Store payload commands | 1 |
| Store reference commands | 1 |
| Python/native packed bytes submitted | 80 MB |
| Bytes avoided versus two submissions | 80 MB (50%) |
| Unique native retained payload | 80 MB |
| Rendered points | 10,000,000 per plot |
| Sampled frame average | 4.49 ms |

The artifact is
`artifacts/xy-benchmark/point-store-reference-10m.json`. The benchmark records
actual direct/flushed native method counts so pre-bind routing is visible.
Unit coverage exercises bound routing, pre-bind routing, revision publication,
and both intermediate and legacy backend fallbacks.

GPU buffers remain plot-owned and account for the remaining duplicate exact
representation. The next experiment should determine whether identical exact
XY plots can safely share a GPU source buffer while retaining independent
cameras, overlays, picking state, and viewport-dependent future
representations.

### 2026-08-06: shared exact XY GPU source buffers

The shared-buffer experiment succeeded for immutable exact `PointStore` XY
revisions and is now implemented. After the first plot uploads a revision, the
renderer promotes its primary compact XY buffer (including any device-limit
chunks) into an `Arc`-owned source. Later reference commands attach that same
source to another plot without allocating or uploading another primary GPU
buffer.

Only payload-owned state is shared. Every plot continues to own its camera and
uniform buffer, render target, grid and overlays, scene revision, hover/picking
caches, interaction state, and any viewport-dependent LOD buffer. Advancing one
plot to a new store revision creates a new shared source; plots that have not
advanced safely retain the previous source until their last reference drops.
Direct frames, styled point instances, XYZ payloads, and secondary actors keep
their existing plot-owned buffers.

Fresh 10M-row/two-plot measurements:

| Path | Primary GPU allocations | Unique allocated | Unique used |
|---|---:|---:|---:|
| Legacy independent XY plots | 2 × 160 MB | 320 MB | 160 MB |
| Shared `PointStore` revision | 1 × 160 MB | 160 MB | 80 MB |

The allocation is larger than used bytes because the existing bounded growth
policy reserves 2× headroom for an 80 MB single-buffer payload. Sharing removes
one complete allocation, reducing unique primary GPU allocation by 50%. Both
shared plots rendered all 10,000,000 points. The shared run retained the
publish-once command result—one 80 MB payload command and one lightweight
reference command—and reported two CPU and two GPU references.

Debug telemetry now exposes `gpu_memory.primary_shared` per scatter plus
`gpu_unique_allocated_bytes`, `gpu_unique_used_bytes`, and
`gpu_scatter_references` in `resources.point_stores`. Per-scatter allocation
figures describe the logical buffer visible to that plot and therefore repeat
for shared plots; the store-level fields are the deduplicated device totals.

Artifacts:

- `artifacts/xy-benchmark/point-store-shared-gpu-10m.json`
- `artifacts/xy-benchmark/legacy-separate-gpu-10m.json`

The next representation-planner work should build on this source ownership:
cache viewport-dependent LOD/density products by source revision and rendering
policy, without coupling camera or presentation state between plots.

### 2026-08-06: first shared spatial query service

Implemented the first P2.2 query primitive on `PointStore`:

```python
rows = store.query_rect(x0, x1, y0, y1)
store.build_spatial_index(x="x")
```

`query_rect(...)` returns read-only exact positional source rows inside an
axis-aligned 2D viewport. Results remain in source-row order and exclude
non-finite coordinates. The lazy index is revision-keyed and invalidates only
when its X column changes; Y-only changes retain the index and are filtered at
query time. Query counts, candidate rows, index bytes, and monotonic probes are
reported by `PointStore.stats()`.

The selected first strategy is sorted-X narrowing followed by exact Y
filtering. A monotonic finite X column uses `searchsorted` and contiguous Y
slices with no owned index memory. Unsorted data uses a stable sorted row index
plus sorted X values. This deliberately remains a shared source service rather
than plot state, so multiple plots can reuse it.

10M-row, 1%-wide X viewport results over 15 queries:

| Source order | Warm indexed median | Full scan median | Speedup | Index build | Index bytes | Break-even |
|---|---:|---:|---:|---:|---:|---:|
| Monotonic X | 0.76 ms | 66.34 ms | 87.3× | 13.15 ms | 0 | < 1 query |
| Random X | 3.73 ms | 55.96 ms | 15.0× | 1,501.37 ms | 120 MB | ~29 queries |

The random-data build cost is too high for an unconditional first-query
optimization. Therefore `strategy="auto"` uses the zero-copy monotonic path,
reuses an index that already exists, and otherwise performs a scan.
`strategy="sorted_x"` or `build_spatial_index()` is the explicit opt-in for a
repeated unsorted query workload. This avoids a surprising 1.5-second/120 MB
first query while preserving the measured repeated-query win.

Artifacts:

- `artifacts/xy-benchmark/point-store-query-monotonic-10m.json`
- `artifacts/xy-benchmark/point-store-query-random-10m.json`

This is an exact CPU query service, not yet automatic camera refinement. The
next slice should connect camera bounds to this service, add query-generation
coalescing, and compare sorted-X against a bounded 2D bin index for viewports
that are narrow in Y but broad in X.

### 2026-08-06: exact 3D box queries on the shared source

Extended the P2.2 service with the dimension-independent part needed by the 3D
port:

```python
rows = store.query_box(x0, x1, y0, y1, z0, z1)
```

`query_box(...)` returns exact positional source rows inside an axis-aligned 3D
box, in source-row order. It shares the same X index and revision lifecycle as
`query_rect(...)`, then applies exact finite Y and Z filtering to the narrowed
candidates. Consequently, a 2D and 3D plot backed by the same store can reuse
one index. Updating Y or Z does not rebuild the X index; updating X invalidates
it. Explicit row IDs remain recoverable through `store.row_ids[position]`.

10M-row, 1%-wide X and 50%-wide Y/Z box results over 15 queries:

| Source order | Warm indexed median | Full scan median | Speedup | Index build | Index bytes | Break-even |
|---|---:|---:|---:|---:|---:|---:|
| Monotonic X | 0.31 ms | 76.89 ms | 249.1× | 11.96 ms | 0 | < 1 query |
| Random X | 5.64 ms | 92.18 ms | 16.3× | 1,715.55 ms | 120 MB | ~20 queries |

The same conservative policy applies: `auto` uses monotonic or already-built
indexes and scans otherwise; random data requires explicit index construction.
This result validates source/revision/query reuse for 3D but does not claim that
a sorted-X index solves arbitrary camera views. It helps when the candidate
volume is narrow in X. Rotated frusta, broad-X/narrow-YZ views, and
multiresolution traversal still require benchmarking Morton bins, tiled 3D
grids, or an octree.

Artifacts:

- `artifacts/xy-benchmark/point-store-query-box-monotonic-10m.json`
- `artifacts/xy-benchmark/point-store-query-box-random-10m.json`

The next 3D-oriented experiment should add per-chunk 3D bounds and frustum
rejection before committing to a heavier spatial hierarchy. That composes with
exact chunked rendering and can skip entire buffers without changing source-row
identity.

### 2026-08-06: stable 3D chunk bounds and frustum rejection

Implemented stable source-order chunk metadata on `PointStore`:

```python
store.build_chunk_bounds(chunk_rows=65_536)
ranges = store.query_box_chunks(x0, x1, y0, y1, z0, z1)
ranges = store.query_frustum_chunks(planes)
```

Each returned `[start, stop)` range preserves global positional row identity.
Chunk AABBs use only rows with finite X/Y/Z coordinates; entirely non-finite
chunks are rejected. Frustum planes use the documented inside convention
`a*x + b*y + c*z + d >= 0` and conservative positive-vertex AABB rejection, so
an intersecting chunk is never discarded. Box and equivalent six-plane
frustum queries produced identical candidate ranges in the benchmark.

The cache key includes all three coordinate column revisions and chunk size.
Changing an unrelated scalar column retains the bounds; changing X, Y, or Z
invalidates them. `PointStore.stats()` now includes chunk-bound entries, bytes,
query count, and total candidate rows.

10M-row results with 65,536-row source chunks and a centered 10%-wide box:

| Source order | Visible chunks | Candidate rows | Candidate ratio | Bounds build | Metadata | Candidate filter | Full scan |
|---|---:|---:|---:|---:|---:|---:|---:|
| Spatially coherent | 16 / 153 | 1,048,576 | 10.5% | 39.08 ms | 9.9 KB | 1.55 ms | 58.79 ms |
| Random | 153 / 153 | 10,000,000 | 100% | 43.01 ms | 9.9 KB | 13.97 ms | 47.61 ms |

The coherent source achieved 38.0× faster exact candidate filtering after the
small bounds build. Random source order correctly demonstrated the limitation:
every chunk spans the query volume, so AABB rejection skips nothing. Its
chunked filtering loop happened to reduce temporary-array overhead, but that is
not a spatial-culling win and should not justify GPU draw skipping.

Artifacts:

- `artifacts/xy-benchmark/point-store-chunk-bounds-coherent-10m.json`
- `artifacts/xy-benchmark/point-store-chunk-bounds-random-10m.json`

The next implementation step is to carry these stable offsets and bounds into
native exact chunks and skip non-intersecting draw calls. For randomly ordered
sources, benchmark a Morton reorder/index or bounded 3D bins before adding an
octree; chunk AABBs alone are intentionally insufficient.

### 2026-08-06: projection-safe shared XYZ source path

Before native 3D chunk culling, the handoff audit found that `PointStore`
identity distinguished object and revision but not selected columns or payload
dimension. Different XY projections—or XY and XYZ projections—could therefore
present the same native key. Source identities are now opaque
projection-specific values containing format and selected coordinate names.
This closes the aliasing hole while preserving one shared identity for truly
compatible plots.

`PointStore` now also caches immutable packed `xyz_f32_v0` payloads by the three
coordinate-column revisions. `Scatter3D` detects that protocol just as
`ScatterPlot2D` detects shared XY. The publish-once/reference command now carries
the payload format, allowing later 3D plots to bind the registered exact XYZ
source without another Python/native transfer. Touching an unrelated scalar
column retains the packed XYZ cache; touching X, Y, or Z invalidates it.

Fresh 10M-row/two-plot 3D measurements:

| Input path | Build | Widget RSS growth | Referenced XYZ | Unique XYZ | Payload commands |
|---|---:|---:|---:|---:|---:|
| Legacy frame | 234.92 ms | 240.12 MB | 240 MB | 240 MB | 2 direct |
| Shared `PointStore` | 166.87 ms | 120.14 MB | 240 MB | 120 MB | 1 publish + 1 reference |

The shared source reduced construction time by 29.0%, widget/native CPU
retention by 50.0%, and packed boundary transfer by 50.0%. Both plots rendered
all 10,000,000 points. Each plot still allocated its own 240 MB compact XYZ GPU
buffer (120 MB used plus bounded growth headroom), for 480 MB total primary GPU
allocation. Unlike exact XY, XYZ GPU ownership has not yet been deduplicated.

Artifacts:

- `artifacts/xy-benchmark/point-store-reference-xyz-10m.json`
- `artifacts/xy-benchmark/legacy-xyz-10m.json`

This provides the correct revisioned source identity needed to associate 3D
chunk bounds with native payloads. The next native slice should generalize the
shared exact GPU-buffer object from XY to XYZ, then introduce chunk bounds and
draw rejection without duplicating device allocations across plots.

### 2026-08-06: shared exact XYZ GPU buffers

Generalized the source-owned exact GPU-buffer object from XY-only storage to
both compact XY and compact XYZ representations. A PointStore publication now
promotes either format into one revision-keyed native buffer set; later plots
bind that set without another allocation or upload. Each consumer still owns
its camera, colormap uniforms, grid, overlays, and presentation state, so
sharing positions does not couple plot appearance.

The 10M-row/two-plot XYZ workload now reports both plots as
`primary_shared=true` and draws all 10M rows in each plot:

| Metric | Before | After | Change |
|---|---:|---:|---:|
| Unique compact XYZ GPU allocation | 480 MB | 240 MB | -50% |
| Unique compact XYZ GPU bytes used | 240 MB | 120 MB | -50% |
| Native GPU uploads | 2 | 1 | -50% |
| Widget construction | 166.87 ms | 163.19 ms | -2.2% |
| Rendered rows | 10M + 10M | 10M + 10M | unchanged |

The 240 MB allocation contains the existing bounded 2x growth headroom around
the 120 MB payload. Per-widget snapshots intentionally report the referenced
allocation size; the PointStore summary reports 240 MB once as
`gpu_unique_allocated_bytes`, preventing shared memory from being double
counted. A two-plot 1M XY control continued to share one 16 MB allocation with
8 MB used.

The benchmark harness now falls back to the Windows process-memory API when
`psutil` is unavailable, keeping the retained-memory measurements runnable in
minimal Python installations. The complete Python API and VDOM suite passes
all 505 tests, and the native Rust test target compiles with the generalized
buffer type. The MinGW test executable still encounters the repository's
existing Windows loader `STATUS_ENTRYPOINT_NOT_FOUND` when launched directly;
the release extension exercised by the 10M render benchmark loads and renders
successfully.

Artifacts:

- `artifacts/xy-benchmark/point-store-shared-gpu-xyz-10m.json`
- `artifacts/xy-benchmark/point-store-shared-gpu-xy-1m-control.json`

The next slice can now add compact XYZ chunking to this shared object. Once the
GPU chunk offsets match PointStore's stable source ranges, native camera
frustum tests can skip whole draw calls without duplicating buffers or changing
global row identity.

### 2026-08-06: chunked exact XYZ rendering

Extended the compact XYZ upload and render path from one device buffer to a
stable source-order list of 12-byte-aligned buffers. The split point is the
largest whole XYZ row that fits within `max_buffer_size`; no row is divided
between buffers. The renderer now issues every compact chunk draw for both
local and PointStore-shared storage, while `effective_draw_point_count` remains
the original global source-row count.

The previous permanent capacity probe was intentionally inverted into a
success regression. On the 256 MiB-buffer adapter it requests 22,369,622 XYZ
rows, whose 268,435,464-byte payload is eight bytes larger than one allowed
buffer:

| Metric | Result |
|---|---:|
| Source rows | 22,369,622 |
| Compact payload | 268,435,464 bytes |
| Device `max_buffer_size` | 268,435,456 bytes |
| Selected representation | `chunked_xyz_f32_v0` |
| Chunks | 2 |
| Render rows | 22,369,622 |
| Reduction ratio | 1.0 |

The two-plot PointStore workload also rendered all 22,369,622 rows in both
plots from one shared two-buffer set. Unique GPU allocation was 272,629,756
bytes: one nearly 256 MiB first chunk plus the renderer's 4 MiB minimum
allocation for the tiny final chunk. Without source-owned sharing, the two
plots would reference separate sets totaling approximately 520 MiB.

Snapshot telemetry distinguishes the new representation, reports a 12-byte
stride and two chunks, and explains the decision as
`3d_exact_payload_split_at_device_buffer_limit`. The 10M/two-plot control stays
on the single-buffer `compact_xyz_f32_v0` path. Boundary unit coverage verifies
that both even and non-12-byte-aligned device limits split only between rows.
The complete Python API and VDOM suite passes all 505 tests, the native test
executable compiles, and the release extension passes both render probes.

Artifacts:

- `artifacts/xy-benchmark/scatter-capacity-chunked-xyz.json`
- `artifacts/xy-benchmark/point-store-chunked-shared-gpu-xyz-boundary.json`
- `artifacts/xy-benchmark/point-store-shared-gpu-xyz-10m.json`

M3 exact chunking now covers compact position-only XY and XYZ. The remaining
3D work is to use source-aware chunk sizes and attach PointStore AABBs to the
native chunks, enabling conservative camera-frustum draw rejection while
preserving each chunk's global source offset.

### 2026-08-06: native source-chunk AABBs and camera draw rejection

PointStore-backed compact XYZ uploads now build finite-only AABBs while the
native upload walks each stable source range. The bounds, exact global source
offsets, draw counts, and GPU buffers live in the same shared revision-owned
object, so each referencing plot can apply its own camera without rebuilding
or duplicating metadata. This deliberately preserves compatibility with the
existing publish/reference protocol; no new boundary payload is required.

Draw submission transforms the eight AABB corners into homogeneous clip space
and rejects a chunk only when every corner lies outside the same clip plane.
Intersecting chunks remain visible, and chunks containing no finite XYZ row are
rejected. The test coverage includes inside, outside, and plane-crossing AABBs
plus finite/non-finite compact bounds.

An initial 65,536-row GPU granularity rejected 147 of 153 coherent chunks, but
it also created 153 full-view draw calls. That experiment was not retained as
the default. The benchmark-selected native default is 1,048,576 source rows per
chunk, configurable with
`DRAGONGUI_SCATTER_POINT_STORE_CHUNK_ROWS`. Python's 65,536-row query metadata
remains independent because its cost/latency tradeoff differs from GPU draw
submission.

The focused-camera 10M results are:

| Source order | Visible chunks | Candidate rows | Candidate ratio | Culled rows |
|---|---:|---:|---:|---:|
| Spatially coherent | 1 / 10 | 1,048,576 | 10.49% | 8,951,424 |
| Random | 10 / 10 | 10,000,000 | 100% | 0 |

Random order correctly receives no false optimization: each source chunk spans
the camera volume, so every draw remains. For coherent order the renderer skips
nine GPU draw calls and 89.5% of source candidates without changing the exact
source or representation revisions.

The full-view two-plot 10M control averaged 11.08 ms versus the prior 10.83 ms
single-buffer baseline, a 2.3% change within the 10% regression gate. Exact
source-sized native chunks also removed unused 2x growth headroom: unique
shared XYZ allocation fell from 240 MB to exactly 120 MB. The former
single-buffer boundary workload now uses 22 shared source chunks for its
22,369,622 rows, allocates exactly 268,435,464 bytes, and renders every row in
both plots.

Snapshot telemetry now reports chunk source offsets, visible and culled chunk
counts, visible candidates, and culled source rows. The focused frame samples
did not yet demonstrate a stable end-to-end frame-time win over randomized
data, whose off-screen vertices are already cheap for the GPU; this slice
therefore claims verified draw/candidate avoidance and memory improvement, not
an interaction-latency improvement. Phase 6's fixed-schedule camera suite must
measure that separately.

Validation remains green at 505 Python API/VDOM tests, the native test target
compiles, and release-extension coherent, random, full-view, and capacity
boundary renders all complete successfully.

Artifacts:

- `artifacts/xy-benchmark/point-store-native-chunk-culling-coherent-10m.json`
- `artifacts/xy-benchmark/point-store-native-chunk-culling-random-10m.json`
- `artifacts/xy-benchmark/point-store-source-chunked-shared-gpu-xyz-10m.json`
- `artifacts/xy-benchmark/point-store-source-chunked-shared-gpu-xyz-boundary.json`

P2.2 now has a native exact-render integration for coherent 3D sources. The
next architectural step is Phase 3's explicit rendering-policy object, which
will let exact, density, decimated, and adaptive choices share these source
revisions, budgets, and observability fields.

### 2026-08-06: explicit rendering-policy contract

Added the first P3.1 public and native policy slice to `Scatter3D` and
`ScatterPlot2D`:

```python
dg.ScatterPlot2D(store, rendering="exact")
dg.ScatterPlot2D(store, rendering="adaptive")
plot.set_rendering("adaptive")
```

`exact` remains the backwards-compatible default. `adaptive` is now a real
requested policy carried through startup and live commands, but its effective
mode is deliberately `exact` until density and representative products exist.
Snapshots expose all three decision fields:

- `policy_requested="adaptive"`;
- `policy_effective="exact"`;
- `policy_reason="adaptive_exact_fallback_no_derived_representation"`.

This avoids relabeling the existing interaction LOD as XY-style adaptive
rendering. The future public names `density` and `decimated` are validated and
reserved, but currently raise `NotImplementedError` with an actionable message
instead of silently drawing exact points. Unknown values raise `ValueError`.
The live setter follows the same rules and does not rebuild or revise source
data.

The Python runtime feature-detects the new native command, preserving
compatibility with an older installed extension. Native validation accepts the
complete policy vocabulary and always records a requested/effective/reason
triple, so a future representation planner can replace only the resolution
step without changing the command or snapshot schema.

A 1M PointStore 2D render verified the full adaptive handoff: all 1M rows were
retained and rendered with `compact_xy_f32_v0`, while telemetry reported the
adaptive request and exact provisional fallback. Public widget and manual docs
now describe the provisional behavior and reserved modes explicitly.

Validation passes all 507 Python API/VDOM tests, the native test target
compiles, and the release extension completes the adaptive-policy render
probe.

Artifact:

- `artifacts/xy-benchmark/scatter-rendering-policy-adaptive-1m.json`

P3.1 is now structurally started but not complete: policy budgets, hysteresis,
and a non-exact adaptive choice still remain. The next slice should implement
the CPU density product with source-row conservation and representative-row
provenance; that will make `rendering="density"` executable and give
`adaptive` its first real alternative to exact points.

### 2026-08-06: CPU density product and first adaptive choice

Implemented the first derived representation for compact 2D scatter sources.
`rendering="density"` now divides the full data bounds into a 256×256 CPU grid,
aggregates each occupied cell into a count-weighted centroid, and maps the
log-scaled source-row count into representative color and point size. The exact
compact source remains retained, so switching policies does not discard authored
rows and source-row picking continues to resolve against exact coordinates.

`rendering="adaptive"` now selects density for compact 2D sources at or above
250,000 rows and exact rendering below that threshold. Density requests for 3D
or styled `point_instance_v1` payloads retain exact rendering with an observable
fallback reason. `decimated` remains reserved rather than silently aliasing an
unrelated implementation.

Snapshot telemetry now includes the grid dimensions, finite source rows,
represented source rows, a conservation check, representative rows, maximum bin
population, build time, representative strategy, and weight meaning. Live data
replacement reapplies the requested policy after the new exact source revision
arrives.

On the structured 1M-row sine-wave PointStore probe:

- density conserved all 1,000,000 finite rows in 3,548 representatives;
- the representation ratio was 0.003548;
- one-time density construction took 14.89 ms in the forced-density run;
- primary GPU allocation fell from 16,000,000 to 4,194,304 bytes;
- primary GPU bytes used fell from 8,000,000 to 113,536 bytes;
- sampled frame time improved from 14.51 ms exact to 10.02 ms density;
- adaptive selected the same product and sampled at 10.12 ms.

These are single short smoke samples, not a general throughput claim. The
dataset is deliberately structured and occupies far fewer than all 65,536 grid
cells. The next benchmark slice should cover uniform random, clustered, and
highly skewed distributions, then use those results to tune grid resolution,
thresholds, caching, and hysteresis.

Validation passes all 507 Python API/VDOM tests, the native test target compiles,
the density conservation unit test compiles, and release-extension exact,
density, and adaptive render probes complete successfully. The Windows GNU unit
test executable still cannot launch in this shell because of its loader
`STATUS_ENTRYPOINT_NOT_FOUND`; conservation was therefore also verified through
the shipped release extension telemetry.

Artifacts:

- `artifacts/xy-benchmark/scatter-exact-density-baseline-1m.json`
- `artifacts/xy-benchmark/scatter-density-1m.json`
- `artifacts/xy-benchmark/scatter-adaptive-density-1m.json`

P3.1 now has a real alternate product and a first adaptive resolver. Remaining
work includes distribution-sensitive benchmarks, cache reuse across plots,
viewport-aware density rebuilding, policy budgets and hysteresis, and a true
decimated representative product with explicit provenance.

### 2026-08-06: distribution matrix and adaptive hysteresis

Expanded `point_store_case.py` with deterministic structured, uniform,
clustered, and skewed inputs. The generated columns are explicitly C-contiguous
so borrowed `PointStore` cases preserve the same zero-copy contract as normal
benchmarks. Added reproducible distribution and threshold matrix runners with
consolidated JSON summaries.

The 30-frame 1M-row matrix found that every distribution conserved all finite
source rows. Density occupied between 3,548 and 65,536 cells and reduced sampled
steady-state frame time relative to exact rendering in every case:

| Distribution | Representatives | Density build | Exact frame | Density frame | Change |
|---|---:|---:|---:|---:|---:|
| structured | 3,548 | 18.14 ms | 5.90 ms | 4.99 ms | -15.5% |
| uniform | 65,536 | 24.51 ms | 4.95 ms | 4.60 ms | -7.1% |
| clustered | 12,899 | 18.13 ms | 4.93 ms | 4.54 ms | -8.0% |
| skewed | 38,436 | 27.51 ms | 4.94 ms | 4.62 ms | -6.4% |

All four cases reduced primary GPU allocation from 16,000,000 to 4,194,304
bytes (-73.8%). The longer samples are deliberately treated as the policy
evidence; the earlier three-frame smoke samples had materially more variance.

A 30-frame uniform threshold sweep showed that density was faster even at 100k
rows, but at 100k and 250k both exact and density use the renderer's 4 MiB
minimum allocation. Density also removes visual point identity, so speed alone
does not justify selecting it at those sizes. At 500k rows, exact allocation
grew to 8,000,000 bytes while density remained at 4,194,304 bytes and reduced
the sampled frame time by 17.8%.

Replaced the single 250k adaptive cutoff with explicit hysteresis:

- enter density at 300,000 rows;
- remain in density while at or above 200,000 rows;
- return to exact below 200,000 rows.

Snapshots expose both thresholds and distinguish entry, retention, and exit
reasons. This prevents live sources oscillating between products when their row
count fluctuates around one boundary. Release probes verify exact remains
effective at 250k initial rows and density becomes effective at 300k. The
compiled native unit checks the inclusive threshold boundaries.

Validation passes all 507 Python API/VDOM tests, benchmark scripts compile, the
native test target compiles, and the release hysteresis boundary probes pass.

Artifacts:

- `artifacts/xy-benchmark/scatter-density-distributions-30f/summary.json`
- `artifacts/xy-benchmark/scatter-density-thresholds/summary.json`
- `artifacts/xy-benchmark/scatter-adaptive-hysteresis-250k.json`
- `artifacts/xy-benchmark/scatter-adaptive-hysteresis-300k.json`

P3.1 now has evidence-backed entry/exit budgets and basic hysteresis. The next
high-value slice is density-product caching by PointStore revision so multiple
plots and policy toggles can reuse the same CPU aggregation instead of paying
the 18-28 ms build cost per widget.

### 2026-08-06: shared density-product cache

Added a native CPU density cache keyed by PointStore identity, source revision,
grid width/height, and colormap. Each cached entry contains the immutable
representative point product and its conservation/build statistics. Widgets
retain shared `Arc` references while active, and the runtime retains a bounded
cache of at most 16 products so a single plot can switch exact -> density
without rebuilding. New revisions/configurations receive distinct keys; bounded
eviction prevents revision churn from growing memory without limit.

Telemetry now reports per-widget cache-hit state and global entries, unique CPU
bytes, hits, misses, evictions, build milliseconds, and entry capacity. The
PointStore benchmark validates these invariants instead of merely reporting
them.

The 1M uniform two-plot release probe produced:

- one density build/miss and one cache hit;
- one 65,536-row product shared by both plots;
- 2,097,152 unique cached product bytes rather than one CPU product per plot;
- 23.62 ms aggregate build time paid once;
- zero evictions with one of 16 cache entries occupied;
- final sampled frame time of 4.57 ms across the two visible plots.

A single-plot exact -> density toggle probe also produced one miss and one hit,
proving the bounded strong cache survives when no widget temporarily uses the
density product. That probe exposed an oversized GPU-buffer retention issue:
restoring exact allocated 16,000,000 bytes and the subsequent density upload
initially reused that capacity. The derived upload path now releases materially
oversized exact buffers before uploading representatives. The verified final
allocation after the toggle is again 4,194,304 bytes with 2,097,152 bytes used.

Validation passes all 507 Python API/VDOM tests, the native test target compiles,
and both release cache probes satisfy benchmark-enforced hit/miss/entry checks.

Artifacts:

- `artifacts/xy-benchmark/scatter-density-cache-two-plots-1m.json`
- `artifacts/xy-benchmark/scatter-density-cache-toggle-1m.json`

Density products are now reusable across plots and presentation-policy changes.
The next slice should make aggregation viewport-aware so zooming can refine the
visible region instead of always summarizing the full authored bounds.

### 2026-08-06: debounced viewport density refinement

Added visible-data bounds for flat orthographic scatter cameras and extended
the density builder to filter source rows before assigning cells. Full-view
density remains visible while a user pans or zooms. Camera activity schedules a
refinement 150 ms after the most recent interaction, so million-row scans do not
run on every pointer or wheel event. Programmatic parallel-scale changes use the
same path.

Viewport products keep the exact compact source and exact-source picking path.
They are widget-local and disposable; the bounded shared cache continues to
hold the reusable full-view product. A new source revision or rendering-policy
change cancels pending viewport work and resolves from the full source again.

Density telemetry now distinguishes `scope="full"` from `scope="viewport"` and
reports viewport bounds, scanned source rows, visible finite rows, represented
rows, conservation, rebuild count, and debounce duration. Representation-level
visible/culling counters now use the viewport row count instead of incorrectly
describing every authored row as visible.

The release probe used 1M deterministic uniform rows and changed the visible
orthographic half-extents to 0.25 within full bounds of -1..1:

- the settled viewport was `[-0.25, -0.25, 0.25, 0.25]`;
- all 1,000,000 source rows were scanned once after debounce;
- 62,217 finite rows fell inside the viewport;
- all 62,217 visible rows were conserved across density counts;
- 40,184 occupied-cell representatives were rendered;
- the viewport aggregation took 11.14 ms versus 26.51 ms for the initial full product;
- GPU bytes used fell from 2,097,152 for full uniform density to 1,285,888;
- final sampled frame time was 4.63 ms.

The native viewport-filter unit test and release benchmark both enforce visible
row conservation. Validation passes all 507 Python API/VDOM tests, the native
test target compiles, and the release viewport artifact passes its policy,
cache, scope, rebuild, and conservation checks.

Artifact:

- `artifacts/xy-benchmark/scatter-density-viewport-quarter-1m.json`

P3.2 now has full-view aggregation, shared caching, and settled viewport
refinement. The next performance step is to use PointStore spatial queries or a
multiresolution index so viewport rebuild cost scales with candidate rows rather
than scanning the complete source on every settled camera change.

### 2026-08-06: indexed viewport density candidates

Added a native 128×128 coarse spatial index to each cached PointStore-backed
density product. Every finite source row is retained as an exact row ID in its
coarse cell. After pan/zoom debounce, the viewport query gathers only
intersecting-cell candidates and the existing density builder still applies an
exact bounds test, so coarse indexing cannot add out-of-viewport counts.

The index shares the density product's revision/configuration lifetime and is
reused across plots, toggles, and repeated camera refinements. Cache retention
is now bounded by both 16 entries and a 64 MiB byte budget. Telemetry exposes
whether the index was used, grid dimensions, retained bytes, one-time build
milliseconds, candidate rows scanned, and global unique index/cache bytes.

Final 1M-row uniform release probes produced:

- quarter-width viewport `[-0.25, -0.25, 0.25, 0.25]`: 66,346 candidates
  scanned instead of 1,000,000 (93.4% avoided), with the same 62,217 visible
  rows conserved into 40,184 representatives; viewport aggregation took
  7.12 ms versus 11.14 ms for the previous full-source scan;
- deep viewport around ±0.05: 3,915 candidates scanned (99.6% avoided), with
  all 2,456 visible rows conserved into 2,414 representatives; viewport
  aggregation took 0.80 ms;
- the index retained 4,065,540 bytes and took 42-48 ms to build once for the
  source revision; the full cached density product plus index retained
  6,162,692 bytes.

This is deliberately recorded as a tradeoff rather than an unconditional first
render win: the eager index adds CPU time and memory to the initial full density
build, then amortizes that cost across settled pan/zoom refinements. The largest
gain appears at deep zoom, where work now scales with nearby candidates. A
future slice should make index construction lazy/background or move to a
multiresolution representation so first useful density presentation does not
wait for the refinement accelerator.

The native unit test compares indexed and brute-force viewport products and
requires identical conservation/representatives while scanning fewer rows.
Validation passes all 507 Python API/VDOM tests, the native test target compiles,
and both release artifacts pass policy, cache, scope, rebuild, index-use, and
conservation checks.

Artifacts:

- `artifacts/xy-benchmark/scatter-density-indexed-viewport-quarter-1m.json`
- `artifacts/xy-benchmark/scatter-density-indexed-viewport-deep-1m.json`

### 2026-08-06: lazy shared density-index construction

Removed spatial-index construction from the initial full-density path. A
PointStore-backed cached product now contains a thread-safe one-time index slot.
The first settled viewport refinement fills that slot; later refinements and
other plots sharing the product reuse the same immutable index. This preserves
the indexed candidate reduction without delaying the first density overview by
an additional two-pass source scan.

Lazy index growth re-applies the 64 MiB cache byte budget and protects the
active source revision while evicting unrelated cached products. Density
telemetry now reports `spatial_index_status` as `deferred`, `ready`, or
`unavailable`. The benchmark can require the deferred state and verifies zero
index bytes/build time before any viewport request.

Final 1M-row uniform release probes produced:

- initial full density: `spatial_index_status="deferred"`, zero index bytes,
  zero index build milliseconds, and a 2,097,152-byte cached density product;
- first deep viewport refinement: the lazy index took 36.97 ms to build once,
  then only 3,915 candidates were scanned and aggregation took 0.83 ms while
  conserving all 2,456 visible rows;
- two plots sharing the PointStore: one cache miss, one cache hit, one unique
  4,065,540-byte index, and one 43.96 ms build served both plots; their viewport
  aggregations took 0.65 ms and 0.80 ms respectively.

The deferred product test is compiled into the native test target, the release
benchmark enforces deferred/ready state and index use, and all 507 Python
API/VDOM tests pass. The Windows GNU native test executable still cannot launch
in this shell because of its existing `STATUS_ENTRYPOINT_NOT_FOUND` runtime
dependency issue; compilation succeeds.

This completes the lazy half of the startup refinement. The remaining caveat is
that the first settled viewport refinement still pays the index build on the
runtime thread. The next slice should move index construction and viewport
aggregation into cancellable background jobs with camera/source revision checks,
which advances P3.4 and removes that first-interaction hitch.

Artifacts:

- `artifacts/xy-benchmark/scatter-density-lazy-index-initial-1m.json`
- `artifacts/xy-benchmark/scatter-density-lazy-index-deep-1m.json`
- `artifacts/xy-benchmark/scatter-density-lazy-index-shared-two-plots-1m.json`

### 2026-08-06: background revision-safe viewport refinement

Moved lazy index construction, spatial candidate collection, and viewport
density aggregation off the runtime/event-loop thread. A settled camera request
now launches a named CPU worker over immutable `Arc` source bytes and returns
its product through a channel. The worker wakes the event loop, which performs
the small GPU upload and state swap on the owning thread.

Each widget permits at most one active viewport job. Camera or source changes
increment a request revision. New camera work remains debounced while an older
job is active; when that result arrives, it is discarded if its revision or
rendering contract is stale, then the latest due request starts. This bounds
worker growth and prevents an obsolete camera result from replacing current
content. Switching policy or replacing the source invalidates pending work by
the same revision mechanism.

The cached one-time index slot now coordinates simultaneous workers as well as
storing the result: two plots sharing a PointStore cannot duplicate the initial
index build. Build failures are cached as errors rather than repeatedly retried.
Telemetry reports pending state, request revision, jobs started/completed/stale,
errors, and total worker milliseconds.

Final 1M-row uniform release probes produced:

- ordinary deep refinement: one background job completed in 39.14 ms, including
  a 38.32 ms index build and 0.79 ms viewport aggregation; only 3,915 candidates
  were scanned, all 2,456 visible rows were conserved, and sampled frame time
  remained 4.32 ms;
- forced camera supersession from half-scale 0.25 to 0.05: two jobs started,
  exactly one stale result was rejected, one latest result completed, zero
  errors remained, and only the final ±0.05 viewport reached the GPU;
- simultaneous deep refinement in two plots: both jobs shared one unique
  4,065,540-byte index and the one 38.71 ms build; each conserved the same 2,456
  visible rows from 3,915 candidates.

The benchmark now treats stale-result rejection as a correctness condition.
The native test target includes revision/policy/payload rejection cases. All 507
Python API/VDOM tests pass, and the native test target compiles; direct execution
of the Windows GNU test binary remains unavailable in this shell because of the
existing `STATUS_ENTRYPOINT_NOT_FOUND` dependency issue.

This completes the core cancellation/coalescing portion of P3.4. The remaining
P3.4 gap is automatic transition back to exact points when the settled visible
set is sufficiently small. After that, the next independent planner feature is
P3.3 representative decimation.

Artifacts:

- `artifacts/xy-benchmark/scatter-density-background-viewport-deep-1m.json`
- `artifacts/xy-benchmark/scatter-density-background-superseded-1m.json`
- `artifacts/xy-benchmark/scatter-density-background-shared-two-plots-1m.json`

### 2026-08-06: adaptive exact-visible viewport transitions

Completed the remaining representation transition in P3.4. A settled adaptive
viewport now uses the same 300,000-row density entry and 200,000-row density
exit thresholds against exact visible-row counts. At or below the exit threshold
the background job emits one exact point per visible source row; at or above the
entry threshold it emits density centroids. Between the thresholds it retains
the current representation to avoid camera-scale oscillation.

This behavior is adaptive-only. An explicit `rendering="density"` request still
produces density at deep zoom. Exact-visible products contain the authored XY
coordinates while the full compact PointStore payload remains authoritative for
picking and source-row resolution. Telemetry distinguishes
`viewport_representation="exact"` from `"density"`, reports the effective
policy/reason, and identifies `exact_viewport_point_instance_v1` separately from
the density grid.

The zoom-out path initially exposed a 115 ms background rebuild when the final
viewport covered all source bounds. It now reuses the existing cached full-view
density product instead. This makes the common overview -> detail -> overview
round trip both semantically stable and effectively free after the first build.

Final 1M-row uniform release probes produced:

- adaptive deep zoom at ±0.05: 3,915 indexed candidates yielded 2,456 exact
  visible points, all rows were conserved, exact-point construction took
  0.12 ms, GPU used bytes were 78,592, and sampled frame time was 4.70 ms;
- adaptive deep -> full round trip: the deep exact result completed first, the
  full view crossed the density entry threshold, and the cached 65,536-cell
  density product was restored in 0.0004 ms with two completed jobs, zero stale
  results, and zero errors;
- explicit density at the same deep viewport remained density, scanning 3,915
  candidates into 2,414 centroids and conserving all 2,456 visible rows.

The native unit coverage now validates exact viewport filtering/conservation and
adaptive request compatibility. Benchmark contracts enforce exact-visible mode,
the density round trip, and explicit-density stability. All 507 Python API/VDOM
tests pass, and the native test target compiles; direct Windows GNU test execution
remains blocked by the existing `STATUS_ENTRYPOINT_NOT_FOUND` dependency issue.

P3.4 now has debounced provisional reuse, background refinement, bounded
coalescing, stale camera/source rejection, exact-visible fallback, and density
re-entry. The next planned representation feature is P3.3 deterministic
representative decimation for cases where a density view is visually unsuitable.

Artifacts:

- `artifacts/xy-benchmark/scatter-adaptive-viewport-exact-deep-1m.json`
- `artifacts/xy-benchmark/scatter-adaptive-viewport-roundtrip-1m.json`
- `artifacts/xy-benchmark/scatter-density-explicit-remains-density-deep-1m.json`

### 2026-08-06: deterministic spatial representative decimation

Implemented P3.3's first point-like intermediate representation and exposed the
previously reserved `rendering="decimated"` policy through constructors and
`set_rendering()`. Compact 2D sources are partitioned into a 256×256 spatial
grid. Each occupied cell selects the authored source coordinate nearest its cell
center. Equal-score ties use total ordering on x then y rather than source-row
order, and any missing global or viewport min/max x/y coordinates are appended.
The output is therefore bounded to 65,540 points.

Unlike density, decimation does not synthesize centroids or encode population
through point size/color. It preserves a point-cloud appearance and exact
authored representative coordinates. The full compact source remains retained
for exact picking and source-row resolution. Switching from any derived product
back to explicit exact now restores the complete compact source; this also fixes
the same restoration edge after adaptive exact-visible refinement.

Settled decimated pan/zoom uses the existing background job, request revisions,
stale-result rejection, and indexed candidate scan. Telemetry identifies
`decimated_grid_source_points_v1`, the nearest-cell-center-plus-extrema rule,
and a 64-bit coordinate fingerprint that makes determinism measurable.

Final 1M-row uniform release probes produced:

- authored order: 65,540 representatives built in 44.88 ms, 2,097,280 GPU used
  bytes, reduction ratio 0.06554, and fingerprint `840579282467684194`;
- reversed source order: the same 65,540 representatives and identical
  fingerprint, with a 42.56 ms build, demonstrating order-independent output;
- deep viewport ±0.05: 3,915 indexed candidates covered all 2,456 visible rows
  with 2,414 representatives; decimation took 0.87 ms after the lazy index and
  completed through the background refinement path.

Native unit coverage compares forward/reversed representative coordinates and
fingerprints and requires all four planted x/y extrema to survive. Python tests
now accept and serialize the public decimated policy. All 507 Python API/VDOM
tests pass, and the native test target compiles; direct Windows GNU test execution
remains blocked by the existing `STATUS_ENTRYPOINT_NOT_FOUND` dependency issue.

This satisfies deterministic repeatability, row-order-resistant spatial
coverage, and documented extrema survival for P3.3. Decimation products and
their lazy indexes are still widget-local; the next high-value slice is P4.1's
general derived-representation cache so multiple plots and recently revisited
viewports can share density, exact-visible, and decimated products.

Artifacts:

- `artifacts/xy-benchmark/scatter-decimated-uniform-1m.json`
- `artifacts/xy-benchmark/scatter-decimated-uniform-reversed-1m.json`
- `artifacts/xy-benchmark/scatter-decimated-viewport-deep-1m.json`

### 2026-08-06: bounded viewport-derived representation cache

Implemented the first P4.1 cache slice for immutable PointStore-backed settled
viewport products. Density, adaptive exact-visible/density, and decimated
background results are keyed by PointStore identity and source revision, exact
canonicalized viewport bounds, requested representation, adaptive hysteresis
state, 256x256 grid configuration, and colormap. An identical revisit is sent
through the normal result channel with `viewport_job_ms=0.0`, bypassing both the
worker thread and CPU representation rebuild while preserving request-revision
validation and the existing GPU upload/apply path.

The cache is bounded to 32 entries and 64 MiB. Eviction occurs before an insert
that would exceed either bound. Telemetry now reports per-scatter
`derived_cache_hit` and global entry, retained-byte, hit, miss, eviction, and
limit fields. Retained bytes are deliberately conservative because products can
share an Arc-backed spatial index; the counter may count that shared allocation
more than once, which makes the memory limit stricter rather than unsafe.

A 1M-row uniform decimated revisit benchmark requested the same settled +/-0.05
viewport twice. It observed exactly one miss and one hit, one retained product
using a conservative 4,142,788 bytes, two completed viewport requests, and a
final worker time of 0.0 ms. The cached product preserved 2,414 deterministic
representatives for all 2,456 visible rows. The benchmark now has an
`--expect-derived-cache-hit` gate that requires both the global counters and the
per-scatter hit flag; the artifact status is `ok`.

Validation remains green: all 507 Python API/VDOM tests pass, the benchmark
script compiles, and the native Rust test target compiles. The existing Windows
GNU test executable dependency issue still prevents direct native test
execution. The next P4.1 extension is sharing full-view decimation and unifying
recent-product eviction/byte accounting; streaming-specific invalidation remains
P4.2.

Artifact:

- `artifacts/xy-benchmark/scatter-decimated-viewport-cache-revisit-1m.json`

### 2026-08-06: shared full-view decimation products

Extended the bounded P4.1 derived cache to full-view decimation. Multiple plots
referencing the same PointStore identity and revision now build the deterministic
256x256 representative product once and share the resulting immutable Arc-backed
CPU product. The key includes full data bounds, source revision, decimation mode,
and grid resolution. It intentionally omits colormap because decimation preserves
authored coordinates and its reusable geometry has no baked colormap; this moves
the implementation closer to P4.1's rule against fragmenting geometry caches on
presentation-only state.

The full-view path uses the same 32-entry/64-MiB cache, counters, conservative
byte bound, and insertion/eviction helper as settled viewport products. Cache
hits are visible per scatter through `derived_cache_hit`. GPU uploads are still
owned per widget, so this slice removes duplicate CPU scans, selection work, and
CPU product storage without claiming shared GPU buffers.

A release benchmark rendered three decimated plots backed by one 1M-row uniform
PointStore. The first plot performed one 43.54 ms full-source build; the next two
reported cache hits. All three rendered the same 65,540 representatives with
fingerprint `840579282467684194`. Global telemetry reported one miss, two hits,
one entry, zero evictions, and 2,097,280 retained cache bytes. The permanent
`--expect-shared-decimation-cache` gate verifies the one-entry/one-miss contract,
at least N-1 hits, and identical fingerprints across every plot.

All 507 Python API/VDOM tests pass, the benchmark script compiles, and the native
GNU test target compiles. The next P4.1 work is a proper recent-use eviction
order and separating density geometry from colormap so presentation changes do
not fragment density cache entries; shared immutable GPU buffers can follow once
their ownership/eviction lifetime is explicit.

Artifact:

- `artifacts/xy-benchmark/scatter-decimated-shared-full-cache-3plots-1m.json`

### 2026-08-06: least-recently-used derived-cache eviction

Replaced arbitrary HashMap-key eviction in the bounded derived-product cache
with explicit least-recently-used ordering. Every successful full-view or
viewport cache hit removes its key from the recency queue and appends it as the
most-recent entry. Inserts do the same, then evict from the oldest end until
both the 32-entry and conservative 64-MiB limits are satisfied. A shared helper
now owns insertion and byte-budget eviction for both full-view decimation and
settled viewport products, preventing the two paths from drifting.

Telemetry reports `derived_cache_eviction_policy="lru"` and
`derived_cache_recency_updates` alongside the existing hit, miss, eviction,
entry, byte, and limit fields. A native regression test fills all 32 slots,
refreshes the oldest key, inserts a 33rd product, and requires the untouched
second-oldest key to be evicted while the refreshed key survives. The test
target compiles; direct execution remains subject to the existing Windows
native-test loader failure.

The release runtime probe used one 1M-row uniform PointStore and 17 distinct
decimated viewport scales. It refreshed the oldest viewport immediately before
two additional inserts crossed the byte limit, then revisited that viewport at
the end. Results were 18 misses, two hits, two recency updates, three LRU
evictions, 15 retained entries, and 63,750,204 conservative retained bytes.
The final revisit reported `derived_cache_hit=true` and `viewport_job_ms=0.0`;
all 19 viewport requests completed with no stale results or errors. The
benchmark's `--expect-derived-cache-lru` gate verifies the final hit, eviction
policy, pressure counters, and both configured bounds.

All 507 Python API/VDOM tests pass, the benchmark script compiles, the native
GNU test target compiles, formatting is clean, and the artifact status is `ok`.
The next P4.1 slice is separating density geometry/count data from colormap so
presentation-only color changes no longer create distinct density products.

Artifact:

- `artifacts/xy-benchmark/scatter-decimated-derived-cache-lru-1m.json`

### 2026-08-06: density geometry separated from presentation colormap

Removed colormap from both full-density and viewport-derived cache identity.
Density aggregation now produces an immutable reusable product containing
count-weighted centroid positions, count-derived point sizes, and normalized
count intensity stored in a neutral channel. The initial implementation sampled
the widget's colormap into a bounded upload copy; the later shader-coloring slice
below supersedes that presentation path. Decimated and adaptive exact-visible
products continue to use authored colors directly.

This matches P4.1's requirement that presentation-only state not fragment the
geometry cache. A colormap change still updates visible colors, but it scans at
most the 256x256 derived grid rather than the complete authored source and does
not allocate another retained density product. Telemetry reports the effective
`presentation_colormap` on the density representation so tests can distinguish
geometry reuse from a stale-color false hit.

The 1M-row uniform release benchmark built one full density product in 29.66 ms
with 65,536 cells and a 2,097,152-byte retained product. Changing the live plot
from viridis to plasma ended with `presentation_colormap="plasma"`, one cache
entry, one miss, one hit, and zero evictions. The permanent
`--expect-density-colormap-cache` gate requires the new color, a per-scatter
density cache hit, and the one-entry/one-miss global contract. Geometry/color
unit coverage requires viridis and plasma presentation copies to preserve every
centroid position and size while changing sampled RGB values.

All 507 Python API/VDOM tests pass, the benchmark script compiles, the native
GNU test target compiles, and the artifact status is `ok`. The next P4.1 slice
is explicit device/viewport-resolution keying where those inputs affect output,
followed by safe sharing of immutable derived GPU buffers across plots.

Artifact:

- `artifacts/xy-benchmark/scatter-density-colormap-cache-1m.json`

### 2026-08-06: viewport-resolution-aware density products

Settled viewport density no longer always allocates and keys a 256x256 grid.
The effective resolution is now one cell per two physical scatter-viewport
pixels on each axis, independently clamped to 32..256. Those effective grid
dimensions are included in the derived cache key, so different output
resolutions cannot alias while viewport sizes that quantize to the same grid can
still share a product. Full-view density and deterministic decimation remain
fixed at 256x256 to preserve their established contracts and full-view reuse.

Device tier is deliberately not part of the key yet. Density and decimation are
CPU products whose geometry is currently identical across adapters, and their
bounded maximum upload is well below supported buffer limits. Telemetry exposes
`derived_cache_device_tier_keyed=false` with reason
`cpu_product_output_is_device_invariant_v1`; device capability will become a key
input only when the planner actually selects different product output by tier.
This follows P4.1 without fragmenting the cache on irrelevant identity.

Density telemetry now reports the physical viewport size and
`grid_resolution_policy`. Unit coverage checks the 32-cell floor, exact
two-pixels-per-cell scaling, odd-size ceiling behavior, and 256-cell cap. The
benchmark accepts window dimensions plus `--expected-density-grid`, and fails
unless the final viewport product reports both the expected grid and policy.

Two 1M-row uniform deep-viewport release probes passed:

- a 320x240 window produced an effective 318x238 scatter viewport, selected a
  159x119 grid, represented all 2,456 visible rows with 2,313 cells, aggregated
  in 0.63 ms, and used 74,016 GPU point bytes;
- a 900x600 window produced an effective 898x598 scatter viewport, reached the
  256x256 cap, represented the same rows with 2,414 cells, aggregated in 0.88
  ms, and used 77,248 GPU point bytes.

The lazy spatial-index build dominated both first refinements at roughly 41.5
ms, as expected; subsequent keyed revisits avoid that work through the existing
derived cache. All 507 Python API/VDOM tests pass, the native GNU test target
compiles, both benchmark artifacts report `ok`, and formatting is clean. The
next P4.1 slice is safe sharing of immutable derived GPU buffers across plots.

Artifacts:

- `artifacts/xy-benchmark/scatter-density-viewport-resolution-small-1m.json`
- `artifacts/xy-benchmark/scatter-density-viewport-resolution-large-1m.json`

### 2026-08-06: shared immutable decimation GPU buffers

Extended PointStore sharing from immutable CPU decimation products to their
byte-identical `point_instance_v1` GPU buffers. The first plot uploads and
promotes its derived buffer into Arc-backed shared storage keyed by the same
source/view/policy/grid key as the CPU product. Later plots attach that buffer
without a queue upload and report `derived_gpu_cache_hit=true`.

Ownership is deliberately asymmetric: widgets hold strong references while the
runtime cache holds only weak references. Evicting a CPU cache entry therefore
cannot release a GPU allocation still used by an active plot, satisfying P4.1's
in-flight lifetime requirement. Once all widgets leave a product, the allocation
drops naturally; stale weak entries are pruned on the next derived apply. Global
telemetry reports live/weak entries, unique allocated and used bytes, live
scatter references, and the current sharing scope. Density was excluded from
this slice because its bounded upload copy baked the widget's presentation
colormap; the following slice removes that restriction.

A three-plot 1M-row uniform release benchmark built one 65,540-point decimation
product with fingerprint `840579282467684194`. All three widgets reported
`primary_shared=true`; the first created the buffer and the next two reported
both CPU- and GPU-cache hits. Telemetry showed one live GPU entry, three scatter
references, 2,097,280 used bytes, and 4,194,560 allocated bytes. The previous
per-widget path allocated that capacity three times, so this slice saves
8,389,120 allocated GPU bytes in the measured case while preserving identical
representatives.

The permanent `--expect-shared-decimation-gpu` gate requires shared primary
storage on every plot, at least N-1 GPU hits, one live derived GPU entry, exactly
N live references, and internally consistent unique used/allocated bytes. All
507 Python API/VDOM tests pass, the native GNU test target compiles, formatting
is clean, and the artifact status is `ok`.

The next P4 slice, recorded below, moves density colormap sampling into the
shader so neutral-intensity GPU buffers can be shared across presentation styles.

Artifact:

- `artifacts/xy-benchmark/scatter-decimated-shared-gpu-3plots-1m.json`

### 2026-08-06: shader-colored shared density GPU buffers

Moved density colormap sampling from the CPU upload copy into the ordinary
`PointInstance` shader. Neutral density vertices retain normalized intensity in
their red channel, and a reused aligned uniform slot selects shader coloring
without increasing the 272-byte uniform allocation. Each widget owns its
colormap uniform, so plots using Viridis and Plasma can render from the same GPU
vertex buffer. Ordinary authored-color points remain in mode zero.

The derived GPU cache now normalizes identity to the actual immutable output:
source revision, exact output bounds, `density` or `decimated` representation,
and effective grid dimensions. Requested `adaptive` versus explicit `density`
and presentation colormap are intentionally absent because neither changes the
neutral vertex bytes. Exact-visible adaptive products remain unshared in this
cache. Shared-buffer metadata now preserves the original opacity classification,
preventing density's alpha 0.9 points from accidentally taking the opaque path.

A three-plot 1M-row structured release benchmark changed only the first plot to
Plasma while the other two remained Viridis. All three reported
`primary_shared=true`; telemetry showed one live derived GPU entry, three live
scatter references, 113,536 used bytes, and 4,194,304 allocated bytes. Sharing
avoids two additional capacity allocations, saving 8,388,608 allocated GPU bytes
for this case. The first plot reported Plasma and the others Viridis while every
plot retained the same 3,548 representatives and fingerprint.

The permanent `--expect-shared-density-gpu` gate requires mixed presentation
colormaps, shared primary storage on all plots, one live derived GPU entry,
exactly N live references, and consistent unique used/allocated bytes. All 507
Python API/VDOM tests pass, the native GNU test target compiles, the release
shader ran successfully through the benchmark, formatting is clean, and the
artifact status is `ok`.

P4.1 immutable CPU/GPU representation sharing is now complete for density and
decimation. The next planned slice is P4.2 streaming invalidation and coalescing:
make source revisions invalidate only affected products, coalesce rapid updates,
and prove that stale derived work cannot overwrite the latest stream revision.

Artifact:

- `artifacts/xy-benchmark/scatter-density-shared-gpu-mixed-colormap-3plots-1m.json`

### 2026-08-06: revision-aware streaming invalidation and coalescing

Started P4.2 by making latest-frame PointStore replacement monotonic across both
queue and runtime boundaries. Queue replacement no longer relies only on arrival
order for commands targeting the same widget and stable store projection: it
retains the highest source revision while preserving sticky `fit` semantics.
Same-store replacement now updates its existing queue node in place so the one
packed registration command stays ahead of later plots' reference commands.

The runtime maintains a latest accepted revision watermark per PointStore
projection for coalesced updates. A delayed lower revision is discarded with the
observable `stale_source_revision` outcome. Advancing the watermark invalidates
only obsolete entries with the same stable projection ID across compact source,
exact GPU, full-density CPU, viewport-derived CPU/LRU, and derived GPU caches.
Widgets retain strong references, so invalidation cannot release a product still
being rendered. `coalesce=false` remains an explicit lossless/historical escape
hatch and is not subject to the monotonic filter.

Telemetry now reports projection-watermark count, revision advances, stale
updates dropped, and invalidated entry counts for every affected cache family.
Unit coverage checks monotonic action selection, highest-revision batch
coalescing, sticky fit propagation, and highest-revision queue replacement.

The release gate streamed eight rapid replacements of a 250k-row PointStore to
three density plots, advancing the Python source from revision 1 to revision 9.
It then injected revision 1 after revision 9 in the same queue segment and once
again in a delayed batch. The queue coalesced 22 redundant scatter commands; all
three plots acknowledged revision 9, the delayed stale update was dropped, and
only one revision-9 compact payload, density product, and shared density GPU
buffer remained. The final representation used 113,408 GPU point bytes from a
2,000,000-byte exact source payload, preserving the bounded-update contract.

The permanent `--expect-stream-latest-revision` gate requires newest-revision
acknowledgement on every plot, at least one stale drop, obsolete source/density/
derived-GPU invalidation, exactly one retained source revision and density cache
entry, sufficient queue replacements, and no density GPU upload above 65,536
`PointInstance` rows. All 507 Python API/VDOM tests pass, the native GNU test
target compiles with the new unit tests, formatting is clean, and the release
artifact status is `ok`.

This completes the first P4.2 slice for retained current-source PointStore
streams. Remaining P4.2 work is to benchmark the existing prepared-frame path
against its pre-change throughput baseline, add explicit adaptive
`source_retention="none"|"current"` behavior, and prove background reduction
jobs cannot publish a stale source revision during continuous viewport changes.

Artifact:

- `artifacts/xy-benchmark/scatter-density-stream-latest-revision-3plots-250k.json`

### 2026-08-06: prepared-frame throughput regression gate

Added `benchmarks/prepared_frame_throughput_case.py` to protect the existing
direct producer-thread `ScatterLiveFrame.enqueue_prepared(...)` path while P4.2
adds adaptive streaming behavior. The probe prebuilds eight compact
`xyz_f32_v0` payload variants, performs a warmup and five measured rounds, and
places a debug-snapshot observation barrier after every round. Each barrier must
acknowledge the final variant's distinct point count, and the final native queue
must be empty with the expected scatter replacements recorded.

The checked workload uses approximately 20k rows per payload, 2,000 submissions
per round, five rounds, and 200 warmup submissions. It measures the producer's
direct submission path rather than GPU presentation rate: latest-frame
coalescing intentionally allows only the final pending payload to be uploaded.
This isolates Python/native payload handoff and command-queue replacement, the
parts most likely to regress when revision-aware coalescing changes.

The published PyPI 1.0.0 runtime established a median baseline of 62,243
submissions/s and 14,940,946,893 payload bytes/s (13.91 GiB/s). The current
release runtime reached 66,419 submissions/s and 15,943,437,266 bytes/s (14.85
GiB/s), which is 6.71% faster than the baseline and therefore comfortably inside
the plan's maximum 5% regression budget. All five rounds on both runtimes
acknowledged the expected 20,007-point final payload, both queues drained, and
each run coalesced more than 10,000 superseded scatter submissions.

The benchmark accepts `--baseline` and `--max-regression-percent`; its permanent
gate fails on a throughput regression over 5%, a stale final payload, producer
failure, insufficient coalescing, or residual native queue depth. This closes
the prepared-frame performance acceptance item in P4.2 without conflating
submission throughput with rendered frame rate.

Remaining P4.2 work is explicit adaptive
`source_retention="none"|"current"` behavior and a continuous source-plus-
viewport test proving background reductions cannot publish stale revisions.

Artifacts:

- `artifacts/xy-benchmark/prepared-frame-throughput-pypi-1.0.0-baseline.json`
- `artifacts/xy-benchmark/prepared-frame-throughput-current-vs-pypi-1.0.0.json`

### 2026-08-06: explicit adaptive source retention

Implemented `source_retention="current"|"none"` on `Scatter3D` and
`ScatterPlot2D`, including constructor validation, serialized props, live
`set_source_retention()`, native command validation, and debug telemetry. The
default `"current"` preserves compatibility and keeps the latest compact source
for viewport refinement, exact picking, and representation changes.

With `"none"`, native releases the exact CPU payload only after a successful
bounded 2D density or decimated product is installed. Exact rendering and
unsupported adaptive fallbacks keep the source. After release, a requested
representation change remains on the current bounded product and reports
`source_retention_none_resubmit_points_to_change_representation`; selecting
`"current"` reports `current_waiting_for_next_source` until the next
`set_points()` restores an exact source.

PointStore registration now holds a strong payload lease for one native command
drain batch. This lets the first plot release its widget source without breaking
later same-batch shared references; the lease is cleared immediately after the
batch. A rendered 1M-row, three-plot density gate verified one packed
registration plus two references, one shared density CPU product, one shared
derived GPU product with three widget references, and zero live exact
PointStore revisions after the batch.

The paired `"current"` and `"none"` artifacts both pass. `"current"` reports
24,000,000 logical per-widget source bytes (one shared 8,000,000-byte payload),
while `"none"` reports zero source bytes and only 340,608 aggregate derived CPU
bytes across the three plots. This is an 8,000,000-byte unique native source
release for this workload; Python's PointStore/render cache remains owned by the
application. Frame-time samples are recorded but are not treated as a speed
claim because this gate targets ownership correctness and bounded memory.

Remaining P4.2 work is the continuous source-plus-viewport stale-result test.

Artifacts:

- `artifacts/xy-benchmark/source-retention-current-1m-3plots.json`
- `artifacts/xy-benchmark/source-retention-none-1m-3plots.json`

### 2026-08-06: continuous source-plus-viewport stale-result gate

Completed the remaining P4.2 concurrency proof by extending
`benchmarks/point_store_case.py` with `--source-viewport-race`. The gate starts
a settled viewport refinement, advances the PointStore source twice while
workers are pending, and changes the viewport with every source update. A
benchmark-only worker delay makes the overlap deterministic rather than relying
on machine load.

Viewport worker results are now checked against both the latest camera request
generation and the current PointStore source revision before cache insertion or
GPU upload. Density telemetry records the source revision that produced the
active derived representation, so the final newest-source acknowledgement is
directly observable instead of inferred from widget state.

The 1M-row run started three viewport jobs. Two superseded jobs were rejected as
stale, exactly one final job completed, no job remained pending, and no worker
error occurred. Both the active viewport density product and PointStore report
source revision 3. The runtime retained one live source revision, one latest
full-density cache entry, and one latest derived viewport cache entry; the final
viewport rendered 1,345 representative rows from the newest 1,000,000-row
source.

This closes P4.2: prepared-frame throughput stays within budget, adaptive source
retention is explicit, and overlapping source/camera updates cannot publish a
stale derived product. The next planned work moves to Phase 5 startup
measurement and first-use optimization, while the broader pixel-correct stable
benchmark remains a later acceptance milestone.

Artifact:

- `artifacts/xy-benchmark/source-viewport-race-1m.json`

### 2026-08-06: Phase 5 source-ready first-present baseline

Started Phase 5 by completing the missing Python side of startup telemetry.
Ordinary `App.run(...)` now records callback collection, widget walking, live
binding/resource queueing, document serialization, and total pre-native time.
The permanent metric adds public source-ready construction, Python pre-native
work, and native `first_application_present_ms`; it does not substitute
multi-frame run-return time for the first useful presentation.

Added `benchmarks/startup_first_present_case.py` and
`benchmarks/run_startup_first_present_matrix.py`. The runner uses a fresh process
for every sample, validates the final workload representation, takes three
samples by default, and fails if any Phase 5 median misses its target.

| Workload | Target | Median | Range | Result |
|---|---:|---:|---:|---|
| Empty window | <1,000 ms | 958.72 ms | 950.73–988.89 ms | Pass |
| 100k exact 2D scatter | <1,500 ms | 985.04 ms | 957.36–985.76 ms | Pass |
| 1M adaptive 2D scatter | <2,000 ms | 1,002.50 ms | 994.76–1,018.13 ms | Pass |

The nine-run phase medians show startup is no longer dominated by scatter
ingestion: instance/surface creation was 245.16 ms, surface configuration
219.08 ms, adapter request 200.13 ms, font-system preparation 132.32 ms,
initial layout 114.12 ms, and device request 83.89 ms. Scatter resource setup
was only 1.43 ms. Python pre-native work was below 0.4 ms in the sampled runs;
public construction added about 1.2 ms for 100k exact and 7.4 ms for 1M
adaptive.

A directional 100k experiment with the loading screen enabled reused the text
renderer, but first application presentation increased from 990.74 ms to
1,052.75 ms because the loading presentation itself cost 120.89 ms. No default
was changed. The next startup experiment should target unconditional renderer
creation for unused widget families or a safely lazy text path; adapter/device/
surface work is driver-facing and offers less reliable library-level leverage.

Artifacts:

- `artifacts/xy-benchmark/startup-first-present-current.json`
- `artifacts/xy-benchmark/startup-probe-100k-exact.json`
- `artifacts/xy-benchmark/startup-probe-100k-exact-loading.json`

### 2026-08-06: demand-created line and image renderers

Changed startup to construct the line-plot and image renderers only when the
initial retained tree contains their widget families. A full layout rebuild now
ensures either renderer on demand, so `replace_children(...)`, component
replacement, and other live structural updates can introduce line plots or
images after an initially unrelated document.

This removes unused initialization work as intended: across the nine-run matrix,
median `line_plots_init_ms` fell from 1.9340 ms to 0.0002 ms and median
`images_init_ms` fell from 1.4848 ms to 0.0002 ms. Debug telemetry now exposes
`image_renderer_ready`; the existing nullable `line_plot_renderer` snapshot
reports the other lifecycle state.

It is not a first-present speed win because these small constructors previously
ran in parallel with larger renderer work. The paired medians were 978.64 ms
empty, 1,029.63 ms for 100k exact, and 1,041.03 ms for 1M adaptive—respectively
2.08%, 4.53%, and 3.84% slower than the preceding matrix and consistent with
fresh-process driver variance. All three remain inside the Phase 5 gates. The
change is retained for avoiding unused GPU state, not credited as latency
improvement.

A permanent live-insertion probe begins with neither optional renderer, replaces
a container with a three-point line plot and an image, and verifies both
renderers are created. It reported one line series and both renderers ready; the
separate empty probe confirmed both were absent at first presentation. The image
uses an intentionally missing path, so the expected file diagnostic also
exercises nonfatal image-load handling.

All cold-process Phase 5 acceptance medians remain below their targets and the
full source-ready metric is now permanent, so Phase 5 is complete. Further
driver-facing startup experiments should require a separate evidence-backed
goal. The XY improvement sequence now moves to Phase 6 framebuffer correctness
and stable-frame validation.

Artifacts:

- `artifacts/xy-benchmark/startup-first-present-demand-renderers.json`
- `artifacts/xy-benchmark/startup-demand-renderers-empty-validation.json`
- `artifacts/xy-benchmark/lazy-optional-renderers-live-insert.json`

### 2026-08-06: Phase 6 exact scatter correct-and-stable gate

Started P6.1 by reusing the existing bounded scatter-local RGBA readback instead
of adding a second framebuffer API. Added
`benchmarks/scatter_correct_stable_case.py`, which runs capture from a producer
thread after `application_frame_presented`, excluding OS chrome and avoiding
event-loop callback deadlock.

The exact workload plants five 20-pixel RGBA sentinels at the four corners and
center of a square data domain. Correctness requires each uniquely colored
marker to contribute at least eight pixels inside a fixed normalized projection
window. Stability requires at least ten consecutive captures with identical
dimensions, byte length, and SHA-256. Runtime validation additionally requires
an `Ok` payload, five current source rows, five exact render rows, an exact
requested/effective policy, at least one acknowledged update, and an application
frame presentation.

The first gate passed with ten byte-identical 718x718 RGBA captures (2,062,096
bytes each), all hashing to
`2679b31e51a41794165f4896b3312d5a922585c4305bc2d06858352ed172e347`.
The four corner centroids landed at normalized coordinates near 0.158 and 0.841;
the center landed at 0.499. Each marker contributed 255–273 qualifying pixels.
The final runtime reported scene revision 40, exact policy, and five render rows.

This closes the static exact portion of P6.1 and removes the original benchmark
limitation that DragonGUI had no pixel readback validation. Remaining P6 work is
to apply the same contract to adaptive/density output, then add fixed-schedule
pan, zoom, resize, restore, and selection workloads with mutation-to-stability
latency.

Artifact:

- `artifacts/xy-benchmark/scatter-correct-stable-exact-sentinels.json`

### 2026-08-06: density and adaptive correct-and-stable parity

Extended P6.1 with `benchmarks/scatter_density_correct_stable_case.py` and the
paired `benchmarks/run_scatter_density_correct_stable.py` gate. Because bounded
density output intentionally replaces authored per-point colors with count
intensity, this workload uses compact PointStore XY data, four isolated extrema,
and a dense central distribution. Correctness is measured as non-background
pixels in fixed corner/center windows plus native count conservation, source
revision, policy, cache-worker error, and render-row checks.

Both explicit `density` and `adaptive` passed ten consecutive 718x718 RGBA
captures at 1M source rows. Every capture in both modes had SHA-256
`277e9a362ad15032c909a5e6c6cbb944e5a3dcd37a6a5ba7e99e57b2a7573803`.
Each isolated corner produced eight qualifying pixels with a maximum channel
delta of 69 from the measured background; the central window produced 1,389
pixels with a maximum delta of 111.

The native representation conserved all 1,000,000 finite source rows, retained
source revision 1, and rendered 1,570 bounded representatives. Explicit density
and adaptive produced the same representative fingerprint
`2057297816029210493`, the same render-row count, and the same final pixels. The
paired runner fails unless both individual stability gates pass and all four
cross-policy parity checks match.

Static exact, density, and adaptive rendering now satisfy P6.1's pixel-correct
ten-stable-frame contract. Remaining Phase 6 work is P6.2's fixed-schedule
interaction suite and mutation-to-final-correct-frame latency.

Artifacts:

- `artifacts/xy-benchmark/scatter-correct-stable-density-1m.json`
- `artifacts/xy-benchmark/scatter-correct-stable-adaptive-1m.json`
- `artifacts/xy-benchmark/scatter-correct-stable-density-adaptive-1m.json`

### 2026-08-06: fixed-schedule interaction recovery gate

Started P6.2 with `benchmarks/scatter_interaction_correct_stable_case.py`.
The 1M-row adaptive workload submits zoom, pan, a resize to 860x620, a
PointStore source-revision update, a resize back to 720x720, and home/fit on
fixed 140 ms deadlines. Input submission never waits for the preceding render
or density job to complete; measured deadline lag makes accidental
completion-paced scheduling visible.

The first run passed with a maximum schedule lag of 1.01 ms and a mean of
0.66 ms. The 1M-row column replacement plus scatter update call took 11.74 ms.
After the last input, DragonGUI reached a pixel-correct full-density revision in
273.63 ms and produced ten byte-identical correct captures by 381.68 ms. All
ten 718x718 RGBA images hashed to
`e00aac90a440ac7838b90658c35a2f3466449b83f3f8f89d698e6d895476bdd0`.

The final snapshot reported source revision 2, all 1,000,000 source rows
conserved, 1,571 bounded render rows, one completed viewport job, zero stale
jobs, zero viewport errors, and command-queue depth zero. The four isolated
extrema remained visible with eight qualifying pixels apiece and the central
window contained 1,384 qualifying pixels. Across the measured native frame
window, total frame time was 5.55 ms p50, 12.29 ms p95, and 18.85 ms maximum;
active work was 0.32 ms p50 and 0.59 ms p95.

This closed deterministic camera, resize, source-mutation, restore, and
mutation-to-stability coverage in the initial P6.2 slice. Deterministic
selection was added in the later exact-row selection slice below.

Artifact:

- `artifacts/xy-benchmark/scatter-interaction-correct-stable-adaptive-1m.json`

### 2026-08-06: rectangle zoom, repeated Home, and density restore fix

Extended the P6.2 gate with scatter's deterministic rectangle-zoom semantic,
`fit(bounds)`, followed by three independently perturbed Home/full-fit restores.
This is API-level rectangle zoom rather than OS pointer injection; DragonGUI
does not yet expose a scatter box-zoom gesture API. Two fixed-time checkpoints
read the restored camera state, and the final recovery proves the pixels and
full-density scope. The worker now requests exit explicitly, so benchmark
lifetime is independent of display refresh rate and the smoke-frame cap is only
a failure fallback.

The initial extended run exposed a correctness bug. After rectangle zoom and
Home, the camera returned to the full fitted view but the retained density
product remained `scope="viewport"` indefinitely. The screen could therefore
show a full camera framing around only the rectangle-filtered representatives.
Programmatic `fit()`, `reset_camera()`, `set_camera()`, and keyboard Home/R now
schedule density viewport refinement. A full-view request recognizes that its
viewport covers the data bounds and reinstates the cached full-density product.

The unchanged regression passed after rebuilding the native extension. All
three Home checkpoints reported the identical camera target, distance, yaw,
pitch, and projection. Recovery reached current full-density source revision 2
on the first poll, after nine completed viewport jobs with zero stale jobs and
zero errors. The final ten captures remained byte-identical with SHA-256
`e00aac90a440ac7838b90658c35a2f3466449b83f3f8f89d698e6d895476bdd0`.

Expanded P6.2 telemetry from this run:

- public object-build time: 7.54 ms;
- first nonblank / initial correct frame: 1,213.86 / 1,432.72 ms;
- maximum fixed-deadline lag: 0.93 ms;
- last input to first correct / ten stable captures: 238.35 / 357.86 ms;
- gesture-window total frame time: 5.67 ms p50, 14.97 ms p95, 27.27 ms max;
- 681 native presentations over 4.199 seconds, or 162.19 Hz on the benchmark
  display, with a zero-percent miss estimate against the declared 60 Hz target;
- process peak RSS: 301,981,696 bytes (288.0 MiB);
- final scatter GPU allocation: 4,194,576 bytes, including 50,272 used primary
  bytes for 1,571 bounded representatives.

Intermediate scatter-local screenshots were intentionally replaced by camera
state checkpoints because readback may temporarily return `None` while a window
resize is in flight. Final recovered readback remains mandatory. A future
surface-generation-aware screenshot API could make intermediate resize-frame
capture reliable without guessing a delay.

### 2026-08-06: deterministic selection and exact source-row verification

Added the public `Scatter3D.select_rectangle((x0, y0, x1, y1))` API. Bounds are
normalized viewport coordinates with a top-left origin, making programmatic
selection independent of window position, DPI, and OS chrome. The call is
asynchronous and uses the same native selection payload and Python change-event
path as a mouse rectangle drag. It therefore updates `selected`,
`selected_indices`, and `selected_index_values` before invoking the registered
`on_select` callback.

Native selection projects the authoritative compact source rows rather than
the density or decimated GPU representatives. The P6.2 1M-row adaptive gate now
issues a fixed-deadline normalized selection around the isolated top-left
sentinel after its third Home restore. The raw actor-0 payload and public
`selected_indices` both returned `[1]`, exactly matching source row 1, while the
visible density representation remained bounded at 1,571 rows. Callback
latency from command submission was 17.98 ms.

The selection-enabled run retained full-density source revision 2, conserved
all 1,000,000 rows, completed nine viewport jobs with zero stale jobs or
errors, and preserved the same ten-frame SHA-256 stability hash. Public build
time was 7.81 ms, initial correct pixels arrived at 1,472.40 ms, maximum input
deadline lag was 1.00 ms, and last selection input to first correct / ten stable
captures was 238.77 / 355.99 ms. This closes P6.2's deterministic selection
followed by exact-row verification requirement without OS-coordinate
automation.

The remaining interaction-methodology gap is literal OS wheel/drag gesture
injection for cross-library comparison. DragonGUI's API-level zoom, pan,
rectangle bounds, resize, Home, source mutation, and selection semantics are now
covered by one deterministic correct-and-stable gate.

### 2026-08-06: three-run fresh-process interaction baseline

Added `benchmarks/run_scatter_interaction_correct_stable.py` so P6.2 no longer
depends on a favorable single GUI run. The runner launches the full 1M-row
interaction case in at least three fresh processes, preserves each raw artifact,
and summarizes median, minimum, and maximum values for build, readiness,
selection, recovery, frame timing, presentation rate, process memory, GPU
allocation, and source/render rows.

The matrix is a correctness gate first. Every process must pass independently,
produce the same final framebuffer hash, select exact source row 1, restore an
identical Home camera, conserve the current full-density source, and remain
inside conservative failure ceilings: 10 ms maximum schedule lag, 1,000 ms to
the first correct recovered frame, 1,500 ms to ten stable captures, and 250 ms
for the selection callback. Frame p95 is reported rather than used as a
universal pass threshold because it is adapter, driver, and refresh-rate
dependent.

All three fresh processes passed. Each produced SHA-256
`e00aac90a440ac7838b90658c35a2f3466449b83f3f8f89d698e6d895476bdd0`,
selected `[1]`, conserved 1,000,000 source rows, rendered 1,571 bounded rows,
and allocated 4,194,576 scatter GPU bytes. Measured medians were:

- public build: 8.82 ms;
- first nonblank / initial correct: 1,149.93 / 1,361.00 ms;
- maximum fixed-deadline lag: 0.98 ms;
- selection callback: 14.69 ms;
- last input to first correct / ten stable: 260.23 / 366.20 ms;
- total frame p50 / p95 / maximum: 5.14 / 11.59 / 23.85 ms;
- presentation rate: 170.58 Hz, with a zero-percent miss estimate against the
  declared 60 Hz target;
- peak RSS: 333,484,032 bytes (318.04 MiB).

Observed ranges were 244.73-268.35 ms to first correct, 347.96-377.09 ms to ten
stable captures, and 12.90-22.69 ms for exact-row selection. These narrow
ranges establish the current repeatable P6.2 baseline and provide a useful
regression gate for later gesture or renderer changes.

Artifact:

- `artifacts/xy-benchmark/scatter-interaction-correct-stable-matrix-3x.json`

### 2026-08-06: real Windows gesture replay and 2D Home fix

Added `benchmarks/scatter_os_gesture_correct_stable_case.py` to cover the last
DragonGUI-side P6 interaction-methodology gap with native Windows input. The
probe locates its own process window, converts the scatter's reported viewport
coordinates through `ClientToScreen`, and sends a real activation click, wheel,
Shift+left pan drag, window resize/restore, extended-scan-code Home key, and
rectangle-selection drag. It remains separate from the deterministic API gate
because focus, DPI, input routing, and display state are platform variables.

The gate proves each gesture had an effect before accepting final recovery:
wheel changed camera distance from 3.5337 to 3.1097, Shift+drag changed the
target from `[0, 0, 0]` to approximately `[-0.2177, 0.1088, 0]`, Home restored
the exact initial camera, and the rectangle drag returned source row `[1]` in
both the raw actor payload and public `selected_indices`.

Development exposed a second real Home regression. Once the key event used the
correct extended scan code, `ScatterPlot2D` reset to the generic 3D perspective
camera (`parallel=false`, yaw/pitch 0.4) instead of its orthographic XY home.
The shared native reset path now detects `Pan2D`: it refits current full data
bounds and restores parallel XY before scheduling density refinement. Generic
3D scatter reset behavior is unchanged. This fixes both public
`reset_camera()` and physical Home/R.

The fixed 1M adaptive run passed with maximum schedule lag 0.86 ms. Physical
selection callback latency was 52.28 ms, last input to correct full-density
pixels was 244.96 ms, and ten stable captures completed at 340.56 ms. All ten
captures retained SHA-256
`e00aac90a440ac7838b90658c35a2f3466449b83f3f8f89d698e6d895476bdd0`.
The final state conserved all 1,000,000 source rows at revision 2, rendered
1,571 bounded rows, drained the command queue, and reported 4.74 ms frame p50,
12.01 ms p95, and 20.21 ms maximum. Peak RSS was 338,501,632 bytes (322.82
MiB), and final scatter GPU allocation was 4,260,112 bytes.

P6.2 now has both deterministic API-level coverage and a real DragonGUI OS-input
gate. A literal cross-library gesture comparison still requires an equivalent
XY browser harness using the same seeded source, normalized targets, fixed
schedule, and final-stability rule; it is comparison infrastructure rather than
a missing DragonGUI interaction capability.

Artifact:

- `artifacts/xy-benchmark/scatter-os-gesture-correct-stable-adaptive-1m.json`

### 2026-08-06: XY browser gesture gate and trusted-input finding

Re-enabled XY's upstream fixed-cadence gesture phase through
`benchmarks/run_xy_load_benchmark.py --gestures`. The wrapper keeps historical
load-only runs unchanged, retains the upstream 42-input / 33 ms schedule and
ten-stable-frame stop rule, and now records startup exceptions and handshake
state when an arm never becomes ready.

The first rerun exposed a Windows portability issue in the upstream live host:
Tornado served XY's ES module as `text/plain`, so Chrome rejected the import and
`window.__arm` was never created. Added `_xy_live_host_compat.py`, a
benchmark-only launcher that sets an explicit JavaScript MIME header. It does
not modify the XY checkout or installed package. With that shim, both the 10k
direct and 1M adaptive arms reached a correct, ten-frame-stable initial view.

Neither arm passed the final zoom correctness rule. At 1M the scheduled phase
ran for 1,360.10 ms over 81 observed frames, with 16.80 / 17.30 / 18.10 ms
frame p50 / p95 / maximum, 59.6 observed FPS, and zero estimated drops. Those
numbers are diagnostic only: the final x-span remained exactly 1.0 of the home
span, so all 42 JavaScript-dispatched `WheelEvent` objects produced no viewport
change and the run correctly reported `settle_incorrect`. The 10k direct arm
failed identically.

This reinforces a benchmark rule DragonGUI should retain: interaction gates
must prove the semantic effect of every gesture, and at least one gate should
use trusted browser-protocol or OS input. Smooth animation during an input
window is not evidence that the requested interaction occurred. No
cross-library latency winner is reported from these runs because XY did not
reach the final state and the current DragonGUI/XY datasets and gesture mixes
are not yet identical.

Artifacts:

- `artifacts/xy-benchmark/xy-browser-gesture-compat-smoke-10k.json`
- `artifacts/xy-benchmark/xy-browser-gesture-adaptive-1m.json`

Remaining strict-comparison work is to replay the same seeded arrays, viewport,
target, and wheel-only schedule through trusted input on both libraries, then
compare only runs that pass the shared correct-and-ten-stable stop rule.

### 2026-08-06: matched XY workload and cursor-anchored 2D wheel zoom

Added `benchmarks/scatter_xy_wheel_comparison_case.py` to close the DragonGUI
half of the strict comparison contract. It uses XY's seed `20260713`, correlated
Gaussian `float32` arrays, five exact far-tail sentinels, 900×420 scatter
surface, sentinel-0 target, 42 wheel inputs, 33 ms fixed cadence, ≤0.5 x-span
proof, target-pixel check, and ten consecutive correct identical frames. Input
is delivered through real Windows wheel messages rather than constructed event
objects.

The workload exposed a genuine interaction defect before the benchmark could
pass. `ScatterPlot2D` changed camera distance around the plot center, so a
far-tail target would leave the viewport during repeated zoom. The native
wheel path now records the data coordinate under the pointer before zoom and
translates the orthographic camera target after scale changes to keep that
coordinate fixed. `Scatter3D` retains center-based zoom. A pure native test
covers the anchor adjustment, and the existing mixed OS gate confirms center
zoom, pan, resize, Home, and selection behavior remain intact.

The final 1M matched run passed. All 42 inputs landed with maximum schedule lag
1.06 ms over a 1,353.82 ms gesture window. The target sentinel remained lit as
the x-span shrank to 0.004659 of home. Correct pixels arrived 121.94 ms after
the last input. A later asynchronous viewport transition reset the stability
streak as intended; ten consecutive identical final frames completed at
1,429.47 ms with SHA-256
`505f5658eb64e4015287c9cba85250586ff05704ce2b90870e5b57be5c1686a1`.
The final adaptive representation contained the one exact visible source row,
used the lazy spatial index, reported no viewport-job errors, and had a drained
command queue. Peak RSS was 333,094,912 bytes.

The broader real-input regression gate also passed after the change: physical
selection returned exact source row `[1]`, Home restored the initial camera,
the final full-density hash remained unchanged, and correct / ten-stable
recovery completed in 144.76 / 260.16 ms.

Artifacts:

- `artifacts/xy-benchmark/scatter-xy-wheel-comparison-adaptive-1m.json`
- `artifacts/xy-benchmark/scatter-os-gesture-correct-stable-adaptive-1m.json`

The cross-library input blocker described here was closed by the trusted-CDP
follow-up below.

### 2026-08-06: trusted XY wheel comparison and post-input tail target

Extended `benchmarks/run_xy_load_benchmark.py` with `--trusted-wheel`, which
keeps XY's upstream page, source, target, 42-input / 33 ms schedule, zoom proof,
and correct-and-ten-stable stop rule but delivers each wheel input through
Chrome DevTools Protocol `Input.dispatchMouseEvent`. The driver recomputes the
fixed data target's canvas pixel before every input, matching cursor anchoring
as the view changes, and records delivery count plus maximum/mean schedule lag.

The investigation also found a headless Windows-specific XY 0.0.5 limitation.
After hardware-path WebGL context recovery, the installed wheel-handler closure
retains the original zero-size canvas for coordinate normalization while the
event reaches the replacement canvas. That produces non-finite anchor
fractions. `--trusted-wheel` therefore requires upstream's `--software` mode on
this machine; SwiftShader keeps the canvas identity and geometry valid. This is
recorded as a comparability limit rather than patched in the XY checkout.

Both validation sizes passed. At 10k, XY direct mode reached its final stable
view in 1,384.70 ms from the first input, with a 5.10 ms gesture-frame p95 and a
21.90 ms settle phase. At 1M, XY density mode received 42/42 trusted and
cancelable events, queued and applied 42/42 view changes, kept the target under
the cursor, and reduced x-span to 0.0005 of home. It reached the final stable
view in 1,383.20 ms from the first input, including a 20.40 ms post-input settle
phase. Gesture-frame p50 / p95 / maximum were 4.10 / 5.30 / 21.20 ms; maximum
and mean input schedule lag were 14.13 / 2.86 ms. Browser peak RSS was
603,955,200 bytes and Python-host peak RSS was 78,606,336 bytes.

Compared with DragonGUI's matched 1M run, the next question is no longer wheel
correctness but where finalization time is spent. The split-timing rerun applied
the exact visible representation at 263.33 ms after the last input and showed
correct pixels at 93.06 ms. Its 12 validation captures totaled 161.20 ms
(13.43 ms average, 21.49 ms p95), while 11 debug-snapshot queries totaled
97.23 ms. Ten-stable completion was 1,409.34 ms; the remaining interval is
validation-loop pacing/orchestration, not 1.4 seconds of scatter rendering.
XY's browser-frame settle is not an isolated measurement of the same work. The
next optimization should batch or asynchronously sample stability rather than
tune the renderer against the combined wall-clock value.

A per-sample timeline confirmed the diagnosis. In the latest run exact
representation was applied at 15.66 ms and correct pixels arrived at 113.94 ms;
capture and snapshot work totaled 115.49 ms and 67.55 ms. The nine stability
samples arrived at 287.0, 444.9, 607.3, 739.7, 858.6, 980.6, 1,105.5, 1,222.4,
and 1,342.0 ms—roughly 117–162 ms apart. This is validation-loop
wakeup/orchestration behavior. The next implementation should provide a native
or benchmark-side batched stability probe that observes frame/hash state without
serializing a full command round trip for every sample.

The benchmark-side `--batched-stability` experiment removed all per-sample
semantic snapshots and kept only one final runtime check. It still completed in
1,484.48 ms, with 136.70 ms total screenshot work and zero per-sample snapshot
time; sample spacing remained roughly 110–120 ms. The dominant pacing is the
synchronous screenshot/readback request path itself. The next native change
should expose asynchronous or batched screenshot readback so multiple frame
hashes can be validated without blocking each render request behind a complete
command round trip.

Implementation sketch for the next slice:

1. Add scatter readback request state containing reusable color/staging
   resources, a request id, and a `map_async` completion flag.
2. Encode the offscreen copy during the normal render tick; do not call
   `device.poll(wait_indefinitely())` from the Python command handler.
3. Drain completed mappings on later ticks and complete the pending response
   with RGBA bytes or a native SHA-256 hash.
4. Add a batch/hash-only request that keeps resources alive across ten frame
   boundaries, so stability validation does not re-render and remap
   synchronously for every sample.
5. Keep the existing synchronous `screenshot()` API as a compatibility path
   until pixel/error behavior is covered by native and Python tests.

The acceptance target is unchanged: preserve the target pixel and identical
ten-frame hashes while removing the observed 110–120 ms per-sample pacing.

This is directional rather than a pure renderer ranking: DragonGUI uses real
Windows input and native wgpu hardware, while XY uses trusted CDP input and
SwiftShader. The same wheel deltas also yield different final spans (DragonGUI
0.004659, XY 0.0005), although both satisfy the shared zoom proof and preserve
the same fixed target.

Artifacts:

- `artifacts/xy-benchmark/xy-browser-trusted-wheel-dynamic-smoke-10k.json`
- `artifacts/xy-benchmark/xy-browser-trusted-wheel-adaptive-1m.json`
- `artifacts/xy-benchmark/scatter-xy-wheel-comparison-adaptive-1m.json`
- `artifacts/xy-benchmark/scatter-xy-wheel-comparison-adaptive-1m-split-timing.json`
- `artifacts/xy-benchmark/scatter-xy-wheel-comparison-adaptive-1m-validation-timeline.json`
- `artifacts/xy-benchmark/scatter-xy-wheel-comparison-adaptive-1m-batched-stability.json`

### 2026-08-06: frame-generation stability probe

Added an XY-inspired `--frame-generation-stability` benchmark mode. After the
first correct pixel confirmation it waits on completed native `frames_rendered`
generations, then performs one final screenshot and semantic snapshot. The 1M
gate passed with first-correct latency of 121.68 ms and ten stable generations
at 340.16 ms after the last input; final capture work was 17.07 ms. This is
roughly a 4× reduction versus the 1.34–1.48 s screenshot-per-sample diagnostic,
without changing the renderer or weakening the final pixel/hash proof.

This is a benchmark/API-contract improvement rather than a new GPU algorithm,
but it gives DragonGUI the same useful separation XY demonstrates: completed
frame stability is cheap, while expensive pixel readback is reserved for final
confirmation.

Artifact:

- `artifacts/xy-benchmark/scatter-xy-wheel-comparison-adaptive-1m-frame-generation.json`

The fair comparison rule is now explicit: lead performance summaries with
first-correct pixels and exact-representation application, report frame timings
separately, and keep ten-stable completion as a correctness/readback diagnostic.
This prevents DragonGUI's synchronous validation transport from being presented
as scatter-render latency against XY's browser-frame settle.

### 2026-08-06: 75 ms viewport-debounce trial rejected

Tested whether the 150 ms settled-viewport debounce explained DragonGUI's long
correct-to-ten-stable interval. A release build with a 75 ms debounce passed the
same 1M real-wheel correctness gate, retained the exact one-row final product,
and started/completed two viewport jobs with zero stale jobs. It did not improve
the measured outcome: first-correct was 123.12 ms versus the 121.94 ms baseline,
and ten-stable completion was 1,685.49 ms versus 1,429.47 ms. The viewport job
itself took only 39.01 ms.

The trial was reverted and the documented/public debounce remains 150 ms. This
negative result prevents tuning a coalescing policy against a metric dominated
by ten serialized offscreen renders, GPU map waits, base64 transfer, and Python
array copies. A rebuilt 150 ms control passed again with the same final pixel
hash, zero stale jobs, 111.53 ms first-correct latency, and 1,404.37 ms
ten-stable latency. Next, add per-capture and representation-application
timestamps or a batched/asynchronous stability probe; only then reconsider the
debounce.

Rejected-trial artifact:

- `artifacts/xy-benchmark/scatter-xy-wheel-comparison-adaptive-1m-debounce75.json`
- `artifacts/xy-benchmark/scatter-xy-wheel-comparison-adaptive-1m-restore-check.json`

## Problems To Solve

### Full-size GPU allocation precedes LOD

DragonGUI currently expands a compact source payload into a large render vertex
buffer before LOD can reduce the drawn point count. Five million points request
a 320 MB `scatter-vb` allocation, exceeding the benchmark adapter's 256 MiB
maximum buffer size and causing a wgpu validation panic.

Consequences:

- LOD does not increase the maximum ingestible source size.
- Memory grows with the complete source even when only a fraction is drawn.
- A normal dataset-size limit escapes as a native panic instead of a recoverable
  Python error or automatic representation change.

### Source data and render data are coupled

The scatter widget currently owns both the authored data and a GPU-oriented
representation. This makes it difficult to:

- keep exact rows while displaying a density overview;
- refine only the current viewport;
- share data and indexes between plots;
- cache multiple levels of detail;
- avoid rebuilding geometry for presentation-only changes.

### Stride sampling is not a complete dense-data strategy

Stride LOD reduces work by a fixed factor, but its result depends on input order
and can miss sparse clusters, extrema, and outliers. Its output also remains
proportional to source size.

### Cold startup dominates time to first useful chart

The benchmark observed approximately 5.2 seconds between queuing a scatter
payload and native processing. Million-point decode and upload work took only
tens of milliseconds, so data throughput is not the main cold-start bottleneck.

### Benchmark validation stops at native state

Current DragonGUI validation proves that data reached the native resource and
that application frames were presented. It does not prove that expected points
were visible or that rendering had finished changing.

## Target Architecture

DragonGUI should separate authoritative data, derived render data, and
presentation:

```text
Python / native source columns
        |
        v
PointStore: exact columns, bounds, indexes, revisions
        |
        v
Representation planner
        |
        +-- exact visible points
        +-- decimated representatives
        +-- density grid
        +-- cached viewport tile or level
        |
        v
Compact GPU buffers
        |
        v
Scatter renderer and presentation-only styling
```

The exact source remains authoritative. GPU buffers are disposable products of
the current viewport, output resolution, device limits, and rendering policy.

## Design Principles

1. Accept every source row before choosing how many marks to draw.
2. Never allocate a GPU resource without checking device limits.
3. Bound dense overview work by output resolution, not source row count.
4. Preserve a path from rendered cells or representatives to exact source rows.
5. Keep data revision separate from camera and presentation revisions.
6. Do not rebuild positions for colormap, opacity, grid, or legend changes.
7. Make automatic representation choices observable and overridable.
8. Convert resource-limit failures into automatic fallback or descriptive Python
   exceptions, never native panics.

## Phase 0: Safety And Observability

### P0.1 Guard every scatter allocation

Before creating or growing a GPU buffer, calculate and validate:

- required byte count;
- `max_buffer_size`;
- applicable storage and vertex binding limits;
- maximum points supported by the selected layout;
- whether chunking is supported for this render path.

Fallback order:

1. choose a compact point layout;
2. enable or increase reduction when policy permits;
3. split the resource into bounded chunks;
4. raise a descriptive `ScatterCapacityError` if no valid path remains.

Acceptance criteria:

- Five million points cannot trigger a Rust or wgpu panic.
- Exact mode either renders successfully or raises a Python exception containing
  requested points, required bytes, the device limit, and suggested remedies.
- Adaptive mode automatically selects a valid representation where memory
  permits retaining the source columns.
- Tests cover limits one point below, at, and one point above each allocation
  boundary.

### P0.2 Publish adapter and scatter memory information

Extend `debug_snapshot()` with:

- adapter name, device type, backend, and driver;
- `max_buffer_size` and relevant binding limits;
- exact source rows and bytes;
- derived render rows or cells and bytes;
- allocated GPU bytes by buffer role;
- selected representation and selection reason;
- reduction ratio;
- cache entries, hits, misses, and bytes;
- chunk count when chunked rendering is active.

Acceptance criteria:

- The five-million-point decision can be explained from one snapshot.
- Reported per-buffer bytes sum to the scatter resource's reported GPU total.

### P0.3 Instrument time to first application presentation

Publish monotonic timestamps or durations for:

- native entry;
- window creation;
- adapter request and selection;
- device creation;
- surface configuration;
- shader and pipeline creation;
- font initialization;
- document parse;
- startup resource dequeue;
- scatter decode, bounds, reduction, and upload;
- first submit;
- first successful application present.

Acceptance criteria:

- The unexplained startup interval is less than 100 ms after accounting for all
  published phases.
- A benchmark can exit immediately after the first validated application frame.

## Phase 1: Compact GPU Point Layouts

### P1.1 Split source columns from point styling

Add compact layouts selected from the fields actually used:

| Layout | Suggested contents | Target stride |
|---|---|---:|
| Position 2D | `x`, `y` | 8 bytes |
| Position 3D | `x`, `y`, `z`, padding | 16 bytes |
| Position + scalar | position plus scalar | 16 or 20 bytes |
| Optional color | packed RGBA or separate buffer | 4 bytes |
| Optional size | scalar size or separate buffer | 4 bytes |

Billboard corners should be generated in the vertex shader from
`vertex_index`; four expanded vertices must not be stored for every source
point.

Acceptance criteria:

- A position-only 2D plot uses no more than 16 GPU bytes per rendered point.
- Optional attributes consume no GPU memory when unused.
- Five million position-only points fit within a 256 MiB single-buffer limit,
  independent of adaptive rendering work.
- Existing color, scalar, size, selection, and picking behavior remains covered
  by tests.

### P1.2 Chunk exact rendering

Support multiple bounded vertex or storage buffers for exact-mode datasets that
exceed a single buffer's limit.

Requirements:

- chunks use stable global source offsets;
- picking reports the original row index;
- frustum or viewport rejection can skip irrelevant chunks;
- chunks can upload and release independently;
- draw ordering and transparency behavior remain documented.

Acceptance criteria:

- Exact rendering is no longer limited by one `max_buffer_size` allocation.
- Chunk boundaries do not change selected row indices or visible colors.

## Phase 2: Shared Exact Column Storage

### P2.1 Introduce `PointStore`

Add a reusable source-data object, provisionally:

```python
store = dg.PointStore(
    x,
    y,
    z=z,
    scalars=values,
    ownership="borrowed",
)

plot = dg.ScatterPlot2D(store, rendering="adaptive")
```

Responsibilities:

- retain exact contiguous columns;
- preserve dtype and ownership information;
- expose source and data revisions;
- cache finite masks and bounds;
- provide stable source-row identifiers;
- support borrowed, copied, and moved ownership modes;
- serve multiple compatible widgets;
- release native and GPU derivatives independently.

The first implementation may use `float32` as DragonGUI's preferred sensor and
GPU format. Higher-precision columns should remain available for selection,
tooltips, and computation even if render coordinates are converted to a local
`float32` domain.

Acceptance criteria:

- Two plots can share one exact source without duplicating canonical columns.
- Presentation changes do not change the source revision.
- Replacing one column invalidates only dependent bounds, indexes, and render
  representations.
- Ownership and lifetime behavior is documented and tested.

### P2.2 Create shared spatial query services

Support visible-range and nearest-row queries without scanning all rows for
every camera event. Candidate strategies should be benchmarked rather than
selected in advance:

- uniform grid or tiled bins;
- sorted x indexes with y filtering;
- Morton-ordered points;
- quadtree for 2D and octree for 3D;
- multiresolution hierarchy built during ingest.

Acceptance criteria:

- Query work for a narrow viewport scales with visible candidates rather than
  total source size.
- Every representative or density cell can resolve to exact source rows.

## Phase 3: Adaptive Representation Planner

### P3.1 Add explicit rendering policies

Proposed public policy values:

```python
dg.ScatterPlot2D(store, rendering="exact")
dg.ScatterPlot2D(store, rendering="decimated")
dg.ScatterPlot2D(store, rendering="density")
dg.ScatterPlot2D(store, rendering="adaptive")
```

`adaptive` should consider:

- visible source-row estimate;
- viewport width, height, and scale factor;
- configured maximum marks per pixel;
- point size and opacity;
- interaction state;
- available GPU buffer capacity;
- cached representations;
- latency and frame-time targets.

The decision should be stable around thresholds. Use hysteresis so small camera
movements do not repeatedly switch modes.

Acceptance criteria:

- The selected policy and reason appear in `debug_snapshot()`.
- Users can force a mode or cap its memory and quality budgets.
- Automatic policy changes do not alter exact selection results.

### P3.2 Implement density aggregation

Build a screen-bounded 2D density representation. Each cell should be able to
retain configurable statistics:

- count;
- maximum and minimum scalar;
- sum or mean scalar;
- representative source row;
- occupancy;
- optional category counts for a bounded number of categories.

The initial CPU implementation should establish correctness. A later compute
shader path may accelerate repeated aggregation if it wins measured workloads.

Acceptance criteria:

- Density output size is bounded by configured grid resolution.
- The sum of all density counts equals the number of finite source rows in the
  represented viewport.
- Outlier sentinel cells remain visible.
- A one-million and a one-hundred-million-row overview use comparable GPU render
  buffer sizes, subject to source retention memory.

### P3.3 Implement representative decimation

Provide a point-like intermediate mode for cases where density is visually
inappropriate. Candidate rules include:

- first and last point per screen bin;
- extrema per scalar and axis;
- deterministic reservoir representatives;
- order-independent spatial sampling;
- line-specific min/max envelope decimation outside the scatter implementation.

Acceptance criteria:

- Results are deterministic for the same data revision and viewport.
- Reordering source rows does not materially change spatial coverage.
- Planted extrema and outliers survive documented policies.

### P3.4 Refine on camera movement

Camera interaction should request a representation for the new visible range:

1. immediately transform or reuse the previous representation;
2. produce a fast provisional representation if necessary;
3. compute the final viewport representation asynchronously;
4. atomically replace the GPU buffers;
5. return to exact points when the visible set is small enough.

Acceptance criteria:

- The UI thread never performs an unbounded full-source scan.
- Stale camera jobs are cancelled or coalesced.
- The final representation corresponds to the latest camera revision.
- A benchmark distinguishes first responsive frame from final stable frame.

## Phase 4: Representation Caching And Streaming

### P4.1 Cache derived representations

Suggested cache key inputs:

- source data revision;
- normalized viewport or tile coordinates;
- viewport pixel dimensions;
- representation mode and resolution;
- geometry-affecting point attributes;
- device capability tier.

Do not include presentation-only state such as colormap, legend position, grid
visibility, or tooltip formatting.

Acceptance criteria:

- Returning to a recent viewport produces a measurable cache hit.
- Cache memory is bounded and reported.
- Eviction never releases an in-flight GPU resource.

### P4.2 Preserve DragonGUI's live-streaming strengths

`ScatterLiveFrame` and prepared-payload APIs should remain first-class. Add
adaptive processing without forcing sensor publishers through a retained
historical store when they only need latest-frame semantics.

Proposed streaming choices:

- `rendering="exact"`: compact or chunked full replacement;
- `rendering="adaptive"`: latest source frame replaces the store and produces a
  bounded representation;
- `source_retention="none"`: discard exact source after producing render data;
- `source_retention="current"`: keep the latest exact frame for refinement and
  picking;
- coalesce superseded reduction jobs exactly as prepared GPU uploads are
  coalesced today.

Acceptance criteria:

- Existing high-rate prepared-frame benchmarks do not regress by more than 5%.
- Adaptive streaming bounds GPU upload bytes per update.
- The final acknowledged frame always matches the newest source revision.

## Phase 5: Startup Work

Only optimize startup after Phase 0 instrumentation identifies the dominant
stages. Candidate changes include:

- lazily create pipelines not used by the first document;
- persist wgpu pipeline caches where supported;
- initialize font and icon systems only when required;
- overlap independent CPU preparation with device creation;
- avoid decoding startup payloads twice;
- submit the first application frame before non-visible resources;
- defer spatial indexes until after a provisional representation is available;
- reuse a process-level device when multiple DragonGUI windows are launched in
  sequence and lifecycle rules permit it.

Acceptance criteria:

- Empty-window first application present is below 1 second on the benchmark
  machine.
- A 100k exact scatter reaches a validated first frame below 1.5 seconds.
- A 1M adaptive scatter reaches a validated first frame below 2 seconds.
- Targets include cold-process runs and are medians of at least three samples.

## Phase 6: Correct-And-Stable Benchmarking

### P6.1 Add benchmark framebuffer readback

Provide a bounded, explicitly diagnostic API that captures the rendered scatter
surface after presentation. It need not become a general screenshot API in the
first iteration.

Use planted source points with known projected coordinates. A result is valid
only when:

- expected sentinel windows contain non-background pixels;
- the reported source and representation revisions are current;
- the framebuffer hash is identical for ten consecutive frames;
- no layout, device, validation, or command-queue error occurred.

### P6.2 Add interaction workloads

Replay fixed-schedule input instead of pacing input to renderer completion:

- wheel zoom;
- pan drag;
- zoom-to-rectangle;
- resize during dense rendering;
- repeated home-view restore;
- selection followed by exact-row verification.

Report:

- public build time;
- first nonblank frame;
- first correct frame;
- final stable frame;
- gesture frame p50, p95, and maximum;
- achieved presentation rate;
- dropped-frame percentage;
- last input to final correct frame;
- process and GPU memory peaks;
- source and rendered row counts.

Acceptance criteria:

- DragonGUI and comparison libraries use the same seeded arrays, sentinels,
  viewport, and stop rule.
- Progressive or asynchronous work remains on the clock until the final stable
  representation is visible.

## Proposed Public API Direction

Names remain provisional and require normal API review.

```python
store = dg.PointStore.from_columns(
    x=x,
    y=y,
    scalar=temperature,
    ownership="borrowed",
)

plot = dg.ScatterPlot2D(
    store,
    x="x",
    y="y",
    scalars="scalar",
    rendering="adaptive",
    density_resolution="viewport",
    max_marks_per_pixel=2.0,
    gpu_memory_budget="256 MiB",
)

print(plot.rendering_status())
# ScatterRenderingStatus(
#   mode="density",
#   source_rows=100_000_000,
#   visible_source_rows=99_812_441,
#   render_cells=786_432,
#   gpu_bytes=12_582_912,
#   source_revision=4,
#   camera_revision=18,
# )
```

Compatibility requirements:

- Existing `ScatterPlot2D(frame, x=..., y=...)` and `Scatter3D(...)` calls keep
  working.
- Existing `lod`, `lod_threshold`, and `lod_factor` options are supported during
  a deprecation or mapping period.
- Explicit `rendering="exact"` never silently aggregates unless required to
  avoid a crash; in that case it must raise a descriptive error rather than
  changing semantics without notice.

## Testing Strategy

### Unit tests

- layout byte calculation and device-limit boundaries;
- representation policy and hysteresis;
- density count conservation;
- deterministic decimation;
- source-to-representative index mapping;
- cache keys and invalidation;
- source ownership and lifetime;
- stale camera revision rejection;
- chunk/global-index mapping.

### Native integration tests

- exact compact layout at multiple attribute combinations;
- chunked drawing across a buffer boundary;
- adaptive fallback under synthetic device limits;
- density and decimated buffer upload;
- picking from exact, decimated, and density representations;
- device loss and allocation failure recovery.

### Performance tests

Required sizes:

- 10k;
- 100k;
- 1M;
- 2.5M;
- 4M;
- 5M;
- 10M;
- 100M for adaptive mode where host memory permits.

Each size should cover exact, decimated, density, and adaptive modes where the
mode is meaningful.

### Regression gates

- No native panic for any public dataset size or device limit.
- Exact rendered row count matches the finite source-row contract.
- Density counts conserve represented rows.
- Adaptive GPU bytes stay within the configured budget.
- Static retained frame p95 does not regress by more than 10%.
- Prepared-frame streaming throughput does not regress by more than 5%.
- First useful frame targets from Phase 5 remain enforced on a pinned benchmark
  machine.

## Delivery Sequence

| Milestone | Deliverable | Dependency |
|---|---|---|
| M1 | Allocation guards, Python errors, adapter limits, memory snapshot | None |
| M2 | Compact point layouts and shader-generated billboards | M1 |
| M3 | Chunked exact rendering | M2 |
| M4 | `PointStore` and shared source revisions | M1 |
| M5 | CPU density aggregation and adaptive planner | M2, M4 |
| M6 | Viewport refinement and asynchronous cancellation | M4, M5 |
| M7 | Representation cache and streaming integration | M5, M6 |
| M8 | Startup optimization against published phase timings | M1 |
| M9 | Pixel-correct stable benchmark and interaction suite | M5, M6 |

M1 and M2 should ship before broader adaptive work because they eliminate the
crash and improve every exact scatter workload. M4 and M5 establish the major
architectural improvement. Later milestones should remain benchmark-driven.

## Definition Of Done

The plan is complete when:

1. Public scatter APIs cannot cause a GPU allocation panic.
2. Five million exact 2D position-only points render on a device with a 256 MiB
   maximum buffer size, using compact or chunked buffers.
3. Adaptive mode can ingest at least 100M points on a sufficiently provisioned
   host while keeping the derived GPU representation within a documented fixed
   budget.
4. Zooming from a density overview refines to exact visible rows without losing
   source identity.
5. Cold-start milestones are measured from source-ready API call to first
   correct application presentation.
6. DragonGUI's automated benchmark validates correct pixels and ten stable
   frames using the same workload contract as the XY comparison.
7. Existing live-frame streaming and ordinary small-scatter performance remain
   within their regression budgets.

## References

- [DragonGUI vs XY benchmark](../docs/dragongui-vs-xy-benchmark.md)
- [XY repository](https://github.com/reflex-dev/xy)
- [XY benchmark runbook](https://github.com/reflex-dev/xy/blob/main/benchmarks/README.md)
- [XY architecture](https://reflex.dev/docs/xy/advanced)
