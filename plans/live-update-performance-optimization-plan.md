# Live-Update Performance Optimization and Regression-Control Plan

**Project:** DragonGUI
**Created:** July 31, 2026
**Status:** In progress; Phases 0–6 and the August 3 CPU/memory follow-up are complete, Phase 1 release soak remains a separate release task
**Primary evidence:** `plans/gui-live-dashboard-performance-report.html`
**Primary benchmark:** `benchmarks/gui_live_dashboard_case.py`

## Purpose

The validated live-dashboard benchmark established that DragonGUI's native
renderer is not the main limit in a rapidly updating application. At high load,
native frame work remained below 1 ms at p95, while command drain reached about
17.6 ms at p95. The expensive path is currently:

```text
Python widget updates
  -> Python/native method calls
  -> native command queue
  -> retained-tree property application
  -> dirty-target collection
  -> text/style/layout/primitive rebuild work
```

This plan defines how to test and implement improvements to that path without
trading away correctness, ordering, responsiveness, or API clarity.

The work is deliberately experiment-driven. Every optimization must first
identify the exact cost it intends to remove, preserve a comparable control
path, and pass correctness gates before its timing results are accepted.

## Implementation Progress

### July 31, 2026 — Phase 0 boundary and native queue instrumentation

Completed the first behavior-neutral measurement slice:

- Added Python-side live native-send diagnostics to `AppHandle`:
  - Requested sends.
  - Direct sends.
  - Commands queued before native binding.
  - Commands flushed after binding.
  - Errors.
  - Aggregate and per-method count/average/maximum/total timing.
  - The method map is bounded by DragonGUI's internal sender method names; user
    values cannot create diagnostic keys.
- Added native `CommandQueue` diagnostics:
  - Successful pushes, logical depth, and high-water.
  - Total replacements and fixed replacement families.
  - Push average/maximum/total and p50/p95/p99 over a bounded 128-sample window.
- Exposed the native queue snapshot as `runtime.command_queue` alongside the
  existing command-drain metrics.
- Extended the validated live-dashboard raw report and matrix aggregation with
  Python/native send and native queue measurements.
- Added fail-closed benchmark checks for:
  - Python method counts matching total requested sends.
  - Native queue timing count matching successful pushes.
  - Required native queue diagnostics being present.
  - The native queue being drained at exit.
- Added an isolated benchmark import override so a built wheel can be tested
  without accidentally importing the previously installed native binary.

Verification completed:

- Direct Python 3.12 native-send accounting contract: passed.
- Native queue instrumentation unit test: passed.
- Native queue regression group: **12 passed**.
- Native command-batch coalescing group: **4 passed**.
- Exact-wheel low-load live smoke: **20/20 validations passed**, correct final
  state, zero queue residue, and zero layout diagnostics.
- Exact-wheel 10-second high-load smoke: **20/20 validations passed**, 40.0 Hz,
  correct final state, bounded Python task queue, and zero queue residue.

Initial Phase 0 findings:

- The high-load smoke requested **100,599** live Python/native sends;
  **93,933 (93.4%)** were `SetProp` calls.
- The Python/native send boundary accounted for about **188.5 ms total** across
  the complete high-load run, averaging **0.00187 ms** per send.
- The native queue processed **101,044 pushes**, reached a logical high-water of
  **233**, and spent about **35.1 ms total** in queue insertion. Push p95 was
  **0.0006 ms**.
- Native queue replacement was **zero** in this workload. Keyed Python snapshot
  coalescing prevented stale callbacks before they expanded, while each applied
  snapshot changed many distinct widget/property pairs. The current O(queue)
  replacement scan is therefore not the active dashboard bottleneck, though it
  still needs the planned burst benchmark.
- Command-drain application reached approximately **13.9 ms p95** and deferred
  rebuild flushing reached **11.9 ms p95**. Total command drain was about
  **16.8 ms p95**.
- The native runtime successfully targeted every text batch with zero fallback,
  but **93,933 text dirty requests** became **3,422 targeted rebuild batches**
  and **90,545 rebuilt roots**.
- A short command-level capture showed individual native `SetProp` application
  was inexpensive: 22,578 commands used about 41.1 ms total. The larger cost is
  repeatedly crossing the 32-command drain boundary and flushing targeted
  rebuilds several times for one logical dashboard generation.

This changes the rationale for Phase 1. A typed property packet remains the
first implementation candidate, but its primary expected benefit is one native
command and one deferred rebuild flush for a logical update batch—not merely
fewer Python/Rust calls. The next Phase 0 work is to add labels-only,
intrinsic-text, mixed-state, and queue-burst cases so that benefit can be
isolated before the packet is implemented.

### July 31, 2026 — Phase 0 isolated scaling matrix

Added two validated, DragonGUI-only benchmark programs:

- `benchmarks/gui_update_pipeline_case.py` isolates fixed labels, intrinsic
  text, mixed widget state, same-property bursts, and distinct-property bursts.
- `benchmarks/run_gui_update_pipeline_matrix.py` runs every case in a fresh
  process and rejects a sample unless final Python/native state, queue drain,
  counter accounting, and layout diagnostics are correct.

The 13-case, three-second release-wheel baseline passed every validation. Its
most useful saturation points were:

| Scenario | Scale | Throughput | Callback p95 | Command drain p95 |
|---|---:|---:|---:|---:|
| Fixed labels | 200 | 58.3 Hz | 2.63 ms | 7.06 ms |
| Fixed labels | 1,000 | 22.7 Hz | 9.81 ms | 9.88 ms |
| Mixed state | 200 rows | 26.7 Hz | 4.10 ms | 10.08 ms |
| Same-property burst | 10,000 writes | 33.3 Hz | 32.64 ms | 32.81 ms |
| Distinct-property burst | 1,000 widgets, two writes | 14.6 Hz | 51.91 ms | 48.58 ms |

The distinct 1,000-widget burst also raised native queue push p95 to 0.0393 ms,
reached a queue high-water of 1,002, and caused about 53,000 replacements. That
confirms the current `VecDeque::retain` path becomes material under a large
distinct-key burst, but the much broader cost at ordinary dashboard scale is
still the number of commands and repeated rebuild boundaries. The measured
first target for implementation is therefore the Phase 1 property packet;
Phase 2 queue replacement remains justified as the next independent target.

Raw development artifacts are under the ignored directories
`artifacts/gui-update-pipeline-phase0-v1/` and
`artifacts/gui-update-pipeline-smoke-v1/`.

### July 31, 2026 — Phase 1 typed property packet prototype

Implemented the first additive batch prototype:

- `App.update_batch()` and `AppHandle.update_batch()` collect ordinary live
  widget setter calls without introducing a second state-update API.
- Batches are thread-local per app handle, nest into the outer context, keep
  the last value for duplicate `(widget, property)` keys, and move that final
  write to its correct last-write position.
- Exceptions use the planned flush-and-re-raise contract.
- Any non-property native command is an ordering barrier: pending properties
  flush first, the command is sent, and property collection resumes.
- Older already-bound native senders fall back to individual `SetProp` calls.
- Python diagnostics now report batch contexts, packets, barrier flushes,
  collected/submitted updates, duplicates removed, maximum packet size, and
  compatibility fallbacks.
- The native extension accepts a typed `SetProps { updates }` command, converts
  every value before enqueue, applies siblings independently, aggregates
  applied/stale/no-op outcomes, and preserves one queue command per packet.
- Large packets flush bounded internal deferred-rebuild groups at the existing
  targeted-root safety threshold. This preserves one transport packet without
  forcing an oversized target set into a full renderer rebuild.

Focused verification completed:

- Python batch nesting, duplicate removal, ordering barriers,
  flush-and-re-raise, fallback, and send-accounting contracts: **4 passed**.
- Native typed-packet compilation and queue ordering/accounting test: passed.
- Exact release-wheel rendered smoke matrix: **5/5 scenarios passed** with
  correct final Python/native state, zero layout diagnostics, and drained
  queues.
- A dedicated rendered ordered-barrier case passed in both individual and
  batch modes. It verifies every emitted live style command is preceded by a
  batch flush and that the final post-barrier text reaches Python and native
  retained state.
- Exact-wheel three-second control and candidate matrices: **13/13 scenarios
  passed in both modes**.

The first A/B pass against the same release wheel showed the intended transport
effect. At 200 fixed labels, queue pushes fell from 42,425 to 425 and command
drain p95 fell from 2.59 ms to 2.27 ms. At the saturation points:

| Scenario | Individual throughput | Batched throughput | Individual callback p95 | Batched callback p95 |
|---|---:|---:|---:|---:|
| Fixed labels, 1,000 | 25.3 Hz | 60.0 Hz | 5.82 ms | 3.61 ms |
| Mixed state, 200 rows | 26.7 Hz | 60.0 Hz | 5.25 ms | 3.21 ms |
| Same property, 10,000 writes | 28.0 Hz | 59.7–60.0 Hz | 36.82 ms | 13.8–13.9 ms |
| Distinct properties, 1,000 x 2 | 27.0 Hz | 60.0 Hz | 25.92 ms | 3.73 ms |

Two prototype variants were deliberately rejected or refined during the pass:

1. One unbounded rebuild group per packet reached 60 Hz but exceeded the
   targeted-root safety limit, producing a full-rebuild fallback on every
   200/1,000-widget tick and regressing intrinsic/mixed drain p95.
2. Tracking a second exact unique-target `HashSet` inside each packet removed
   conservative splits but reproducibly reduced heavy-case throughput versus
   reading the renderer's existing bounded target sets. That extra tracking is
   not retained.

The selected prototype has zero targeted-text fallback in the measured cases
and retains the large throughput improvement, but Phase 1 is not yet accepted.
The mixed-state drain p95 remains roughly 19% above the single short control
sample even though backlog and throughput improve dramatically, and longer
runs showed substantial variance. The next work is a repeated, alternated A/B
set plus retained-geometry/screenshot equivalence and direct packet edge-case
tests. Public documentation should wait until those gates pass.

### July 31, 2026 — Correctness fix found by the intrinsic-text benchmark

Manual observation found that the benchmark's deliberately long wrapped label
could paint beneath the following row. The new geometry assertion reproduced
the defect against the previous release wheel:

- Measured wrapped content height: **45 px**.
- Stale resolved label height: **31 px**.
- Benchmark validation: failed as intended.

The cause was an over-broad live-update optimization: all label text mutations
returned `Dirty::Text`, even when text determined intrinsic width or wrapped
height. The retained text and glyphs changed, but sibling placement did not.

The live label classifier now uses `Dirty::Layout` when either affected
dimension is content-dependent:

- Wrapped labels stay on the text-only path only when both width and height are
  explicitly fixed.
- Single-line labels stay on the text-only path when width is explicitly fixed.
- Intrinsic-width or intrinsic-height labels conservatively recompute layout.

Regression coverage now includes three native classifier tests plus rendered
benchmark checks that measured wrapped content fits the resolved rectangle and
the next row begins at or below the first row's bottom. Against the corrected
exact wheel, measured and resolved height are both **45 px**. Both 20-label and
200-label rendered cases passed all validations at the 60 Hz producer target.

This correctness fix intentionally makes the current 200-label intrinsic case
perform a layout pass per applied generation; layout p95 was about 18.6 ms in
the short verification run. It is preferable to stale/overlapping geometry and
provides a concrete Phase 3 target for safe incremental intrinsic-text layout.

Installation verification:

- Reinstalled the corrected release wheel into the primary Python 3.12 user
  environment.
- Found and replaced an ignored in-tree `_dragongui.pyd` from an older build;
  its hash now matches the installed wheel binary.
- Verified both native binaries expose the typed `SetProps` sender.
- Reran the intrinsic-text rendered regression through the normal source-tree
  benchmark import path: all validations passed at 60 Hz.

### July 31, 2026 — Phase 1 equivalence and alternating acceptance harness

Extended the update-pipeline benchmarks with acceptance-grade comparison
evidence:

- Optional settled whole-window screenshot capture after Python/native queues
  drain, stored as dimensions, RGBA byte count, and SHA-256 rather than a large
  embedded pixel payload.
- A canonical retained-state/geometry probe covering status plus representative
  first/last widgets, resolved rectangles, and dynamic geometry.
- `--update-mode both` alternates individual/batch execution order by case and
  repetition in separate fresh processes.
- The matrix fails closed if either sample is invalid, a pair is missing, its
  retained/geometry hashes differ, or its settled screenshot hashes differ.
- Added a saturation-focused `--acceptance` matrix distinct from the quick
  smoke and complete scaling matrices.

The first screenshot-enabled smoke produced **6/6 equivalent pairs**. The
two-repetition saturation pass produced **16/16 equivalent pairs**: every
individual/batch pair had an identical retained-state/geometry hash and a
byte-identical settled screenshot.

Average results across the two short, alternated repetitions were:

| Scenario | Individual | Batch | Change |
|---|---:|---:|---:|
| Fixed labels, 1,000 | 20.0 Hz | 37.5 Hz | +87.5% |
| Intrinsic text, 200 | 20.0 Hz | 60.0 Hz | +200% |
| Mixed state, 200 rows | 29.0 Hz | 55.8 Hz | +92.2% |
| Same property, 10,000 writes | 27.1 Hz | 54.8 Hz | +102.7% |
| Distinct properties, 1,000 x 2 | 18.7 Hz | 34.5 Hz | +84.5% |

At fixed-label scale 200, both modes sustained 60 Hz while average native queue
pushes fell from about 30,306 to 306. At 1,000 distinct properties, native sends
fell from about 99,050 to 88 and callback p95 fell from 24.91 ms to 3.89 ms.

This pass also clarified the remaining acceptance issue. Large logical batches
do more useful work before yielding, so command-drain p95 can exceed the slower
control even while throughput and backlog improve: fixed-label-1,000 averaged
29.90 ms batched versus 23.28 ms individual, and intrinsic-text-200 averaged
17.74 ms versus 10.03 ms. The intrinsic case is an intentional correctness
tradeoff because it now performs required layout. The 1,000-label result still
needs responsiveness/fairness evaluation rather than being waived solely on
throughput. Phase 1 therefore remains provisional pending the five 60-second
repetitions and an interaction-latency probe.

### July 31, 2026 — Lightweight native interaction-latency probe

The first attempt used `debug_snapshot()` as a lossless request/response
barrier. It was rejected because snapshot construction serializes the retained
tree, layout diagnostics, computed styles, and metrics. At 1,000 widgets it
took roughly 450–700 ms, collapsed producer throughput, and measured diagnostic
serialization rather than queue responsiveness. Those results are retained as
negative evidence under `artifacts/gui-update-pipeline-phase1-interaction-v1/`
but are not used for an acceptance decision.

Added a private lightweight native `LatencyProbe` command instead:

- Uses the existing ordered request/response bridge.
- Performs no tree, style, layout, or renderer serialization.
- Completes when the UI thread reaches its exact queue position.
- Is exposed only through private diagnostic helpers and the benchmark, not as
  a public application API.
- Has native bridge and Python forwarding unit coverage.

Three-second 1,000-label and 200-row mixed-state comparisons with a concurrent
100 ms probe interval passed all correctness validations:

| Scenario | Mode | Throughput | Probe p50 | Probe p95 | Probe max |
|---|---|---:|---:|---:|---:|
| Fixed labels, 1,000 | Individual | 20.0 Hz | 49.12 ms | 51.06 ms | 52.63 ms |
| Fixed labels, 1,000 | Batch | 38.0 Hz | 16.01 ms | 25.47 ms | 25.52 ms |
| Mixed state, 200 | Individual | 27.6 Hz | 29.50 ms | 44.85 ms | 47.80 ms |
| Mixed state, 200 | Batch | 55.3 Hz | 7.50 ms | 15.52 ms | 20.14 ms |

This resolves the apparent contradiction in aggregate command-drain p95. A
batch may do more work in one successful drain sample, but it removes the much
larger command backlog that delays a newly arriving lossless request. In these
short high-load probes, batching cut request/response p95 by about 50% for
1,000 labels and 65% for mixed state while roughly doubling throughput. The
long repeated gate remains necessary, but the responsiveness signal now favors
the candidate rather than indicating a fairness regression.

### July 31, 2026 — Five-repetition sustained development gate

Added repeatable exact case selection to the matrix runner so long gates can be
split into bounded slices without changing scenario definitions. Selector
behavior has focused unit coverage.

Ran three five-repetition slices with 1 second warmup and 10 seconds measured
per fresh process. Every run used alternating individual/batch order, a 100 ms
lightweight latency probe, final screenshot capture, retained geometry hashing,
and fail-closed correctness validation:

- Core saturation: 1,000 fixed labels and 200 mixed rows.
- Burst saturation: 10,000 writes to one property and 1,000 distinct keys
  written twice.
- Correctness saturation: 200 intrinsic wrapped labels and ordered live-style
  barriers.

All **30/30 individual/batch equivalence pairs passed**. Every pair produced
the same retained-state/geometry hash and a byte-identical settled screenshot.
No queue residue, final-state mismatch, intrinsic-height failure, timeout, or
ordering-barrier failure occurred.

Conservative results across five repetitions:

| Scenario | Mode | Median throughput | Minimum throughput | Worst interaction p95 |
|---|---|---:|---:|---:|
| Fixed labels, 1,000 | Individual | 20.0 Hz | 16.3 Hz | 68.28 ms |
| Fixed labels, 1,000 | Batch | 37.4 Hz | 35.0 Hz | 26.38 ms |
| Mixed state, 200 | Individual | 27.3 Hz | 16.6 Hz | 67.82 ms |
| Mixed state, 200 | Batch | 54.5 Hz | 33.7 Hz | 39.69 ms |
| Same property, 10,000 writes | Individual | 27.7 Hz | 12.8 Hz | 108.45 ms |
| Same property, 10,000 writes | Batch | 59.3 Hz | 39.2 Hz | 35.23 ms |
| Distinct properties, 1,000 x 2 | Individual | 18.8 Hz | 12.2 Hz | 110.10 ms |
| Distinct properties, 1,000 x 2 | Batch | 34.4 Hz | 33.0 Hz | 29.82 ms |
| Intrinsic text, 200 | Individual | 20.0 Hz | 20.0 Hz | 62.34 ms |
| Intrinsic text, 200 | Batch | 60.0 Hz | 56.9 Hz | 19.51 ms |
| Ordered barrier, 20 | Individual | 60.0 Hz | 59.8 Hz | 17.03 ms |
| Ordered barrier, 20 | Batch | 60.0 Hz | 59.9 Hz | 15.76 ms |

One system slowdown affected both modes in the fourth core repetition. The
candidate retained roughly twice the throughput and lower interaction latency,
which strengthens the conclusion rather than relying only on ideal runs.

The sustained development gate therefore accepts the Phase 1 design. The
remaining five-by-60-second sampling is classified as a release/soak gate, not
a blocker to documenting and integrating the additive API. Before marking the
phase fully complete, update `dg.help`, add a focused public example, and run
the existing high-load live-dashboard benchmark with batching enabled.

### July 31, 2026 — Phase 1 public integration and live-dashboard gate

Integrated the accepted API into the public surface:

- `dg.help.app_model.app()` now lists `update_batch()` among the running-app
  methods.
- Added `dg.help.live_updates.batching()` with a complete example and the
  thread-local, nesting, duplicate-write, barrier, exception, and non-atomic
  semantics.
- Expanded the `App.update_batch()` docstring so interactive API inspection
  describes the same contract.
- Added `examples/live_update_batch_demo.py`, a focused latest-state telemetry
  example that combines task coalescing with one property packet per applied
  frame and safely waits for runtime binding.

The existing realistic live-dashboard case now accepts
`--update-mode individual|batch`. Its batch path groups the status and 200
changing sensor labels after the packed plot-resource updates. New fail-closed
checks require exactly one typed property packet per applied dashboard frame,
matching sender accounting, and the exact expected number of submitted
properties.

Paired high-load development run, each with 1 second warmup and 10 seconds
measured:

| Metric | Individual | Batch | Change |
|---|---:|---:|---:|
| Validated throughput | 13.40 Hz | 46.00 Hz | 3.43x |
| Completed measurement ticks | 134 | 461 | +327 |
| Dropped/coalesced measurement ticks | 466 | 139 | -327 |
| Python/native sends | 33,745 | 13,936 | -58.7% |
| Native queue pushes | 33,897 | 14,447 | -57.4% |
| Native queue high-water | 233 | 33 | -85.8% |
| Submit p95 | 10.57 ms | 10.29 ms | essentially unchanged |
| Command-drain p95 | 18.64 ms | 21.48 ms | +2.84 ms per larger drain sample |

Both samples passed all benchmark validation, including final Python/native
text, retained plot resource sizes, layout diagnostics, generation counters,
queue timing accounting, and zero queue residue. The batched sample also passed
all three new packet-accounting checks. Raw evidence is in
`artifacts/gui-live-dashboard-phase1/`.

This clears the Phase 1 live-dashboard target of at least 44 Hz and confirms
that the synthetic throughput gain transfers to a realistic mixed packed-data
and property-update workload. The individual sample ran below the older 38.1 Hz
historical baseline, so the paired result is treated as development evidence;
the longer rotated soak remains the release-quality comparison.

### July 31, 2026 — Phase 2 queue-contract audit started

Audited `CommandQueue` before building model candidates. The current structure
is one mutex-protected `VecDeque<Command>`. Every replaceable family scans the
physical deque on insertion:

- `SetProp` and theme/stylesheet mutations use `VecDeque::retain`.
- Scatter points, line-plot data, histogram data, scalar bars, and scatter
  actors scan backward and remove matching entries individually.
- Replacement moves the newest command to the tail. Scatter/line `fit` and
  histogram `auto_fit` flags are sticky and OR together across replacements.
- `SetProps` remains one ordered, non-coalesced command; Phase 1 performs its
  duplicate removal before it reaches this queue.
- Full and limited drains remove commands from the front, so partial-drain
  boundaries are part of the contract.

This confirms the expected O(n) insertion cost and adds a second risk:
backward `VecDeque::remove` can also shift elements for each duplicate found.
The cost can therefore grow badly for both large distinct-key queues and queues
containing repeated large-resource payloads.

The audit also found that the queue's current tested semantics are not segmented
by request/response or structural barriers. Replaceable plot commands are
explicitly tested to coalesce across `DebugSnapshot`, and ordinary property
coalescing likewise scans the entire pending deque. This differs from the
original Phase 2 model language. To avoid hiding a behavior change inside a
performance rewrite:

1. The first reference interpreter and candidate comparisons will reproduce
   the current cross-command coalescing contract exactly.
2. Partial drains will remain hard boundaries because already-drained work can
   no longer be replaced.
3. A segmented/barrier policy will be evaluated separately as a correctness
   proposal with explicit snapshot/order tests; it will not be assumed by the
   optimized data structure.

The model must additionally represent stylesheet interactions and sticky flag
merging, not just `(family, key) -> latest payload`, because those are observable
parts of the existing queue behavior.

Implemented the first model-only experiment in
`native/src/command_queue_model.rs`:

- `ReferenceQueue` directly expresses the audited scan-and-remove semantics.
- `StableSlotQueue` appends stable order tokens, tracks the newest live slot per
  coalescing key, invalidates replaced slots in O(1), releases superseded
  payloads immediately, skips stale tokens during drain, and compacts at a
  deterministic 50% stale ratio after 64 physical tokens.
- The abstract command stream covers properties, themes, stylesheet
  set/remove/clear interactions, scatter, line, histogram, scalar-bar and actor
  replacement, sticky flags, lossless commands, snapshots, and arbitrary
  partial drains.
- A deterministic local generator ran 2,000 seeds x 200 operations (400,000
  generated commands), comparing every partial and final drain against the
  reference interpreter.
- A 100,000-write same-key test confirms only one logical command remains,
  compaction occurs, superseded payloads are released, and physical ordering
  tokens remain below twice the 64-token compaction threshold.

Both model tests pass. The first run also caught an unbounded `Vec` preallocation
for the candidate's drain-all sentinel; limiting allocation to live logical
entries fixed it before any production integration. This is exactly the kind of
counterexample the model stage is intended to expose.

Added and tested a second structurally different candidate:

- `LinkedSlotQueue` uses reusable indexed nodes with intrusive previous/next
  links. Replacement unlinks the prior node and moves the new command to the
  tail in constant time.
- It holds no tombstones, needs no compaction, immediately drops superseded
  payloads, and reused exactly one physical slot during 100,000 same-key writes.
- Both candidates now pass the same 2,000-seed mixed-stream equivalence test.

Ran a debug-build model microbenchmark across the legacy reference and both
candidates. These timings are directional rather than release-quality, but the
scaling difference is unambiguous:

| Workload | Legacy deque | Tombstone slots | Linked slots |
|---|---:|---:|---:|
| 32 distinct inserts | 0.081 ms | 0.167 ms | 0.048 ms |
| 1,000 distinct inserts | 8.09 ms | 0.84 ms | 0.97 ms |
| 10,000 distinct inserts | 799.98 ms | 7.45 ms | 7.66 ms |
| 10,000 hot-key writes / 1,000 pending | 164.08 ms | 9.67 ms | 5.21 ms |
| 1,000 hot-key writes / 100,000 pending | 1,600.36 ms | 1.44 ms | 1.29 ms |
| 100,000 same-key writes / otherwise empty | 14.83 ms | 56.68 ms | 55.72 ms |

The last row corrects an assumption in the original target: the legacy queue is
already O(1) for a pure same-key stream because every insertion leaves only one
pending entry. The real failure modes are (a) building a large distinct-key
queue and (b) replacing a hot key while many unrelated commands are pending.
The Phase 2 performance gate is therefore revised to measure those cases.

The linked-slot design is the provisional production favorite: it has bounded
physical memory without periodic compaction and was faster in the backlogged
hot-key cases. It is not selected yet; a segmented/barrier variant and optimized
release-build measurements still need comparison.

Implemented the third candidate, `GenerationalQueue`. Its ordering deque stores
only `(key, generation)` tokens for replaceable commands while a hash map owns
the latest payload. Lossless commands remain inline. Superseded payloads are
released immediately, stale generations are skipped during drain, and the same
deterministic stale-ratio rule bounds physical tokens. It passes the full seeded
partial-drain equivalence suite and a 100,000-write memory/compaction test.

The normal Cargo release test executable could not start because the protected
Microsoft Store Python 3.12 DLL returned `STATUS_ENTRYPOINT_NOT_FOUND` without
an explicit path and `STATUS_ACCESS_DENIED` with it. Compilation itself
succeeded. Because the model uses only Rust's standard library, compiled the
same test module directly with `rustc --test -O`, avoiding PyO3 and system DLL
loading without changing benchmark code.

Five optimized repetitions produced these medians:

| Workload | Legacy deque | Tombstone slots | Linked slots | Generational |
|---|---:|---:|---:|---:|
| 32 distinct inserts | 0.0115 ms | 0.0173 ms | 0.0058 ms | 0.0080 ms |
| 1,000 distinct inserts | 0.6888 ms | 0.1123 ms | 0.1189 ms | 0.1137 ms |
| 10,000 distinct inserts | 61.4449 ms | 1.1985 ms | 1.3617 ms | 0.9662 ms |
| 10,000 hot-key writes / 32 pending | 0.9459 ms | 0.6840 ms | 0.3399 ms | 0.6030 ms |
| 10,000 hot-key writes / 1,000 pending | 13.5935 ms | 0.8061 ms | 0.3408 ms | 0.6488 ms |
| 1,000 hot-key writes / 100,000 pending | 134.9138 ms | 0.0489 ms | 0.0381 ms | 0.0395 ms |
| 100,000 same-key writes / otherwise empty | 5.1602 ms | 5.0021 ms | 3.2954 ms | 4.6796 ms |

All alternatives remove the backlog-dependent scan. Generational tokens are
fastest for bulk distinct insertion, but linked slots are substantially faster
for repeated replacement, also improve the already-cheap pure same-key case,
and hold exactly live logical nodes without compaction latency. Selected the
linked-slot design for the guarded production prototype. Segmented barrier
semantics remain a separate correctness experiment because they intentionally
differ from the current queue contract, not a competing drop-in structure.

### July 31, 2026 — Phase 2 linked-slot production prototype

Replaced the production queue's scan-and-remove `VecDeque` with reusable indexed
nodes linked in logical order. A hash map points each coalescing key to its live
node. Replacement unlinks the prior node, merges sticky flags, immediately drops
its payload, and appends the reused slot at the tail. Full and limited drains
still pop logical commands from the front.

Production key coverage matches all prior replacement families:

- widget properties;
- themes and stylesheet set/remove/clear interactions;
- coalesced scatter points, line-plot data, and histogram data;
- scatter scalar bars and packed actor updates.

The implementation preserves cross-command coalescing and origin-scoped
stylesheet clears. `SetProps` packets remain lossless ordered commands. Added
diagnostics for live entries, physical/free slots, stale entries, compactions,
and peak physical length. Linked slots always report zero stale entries and zero
compactions.

Regression coverage now includes:

- all 21 focused production queue tests;
- all 4 active model/reference tests over 400,000 generated operations;
- cross-snapshot replacement ordering;
- a 10,000-key backlog with 10,000 hot-key replacements, proving no physical
  slot growth;
- partial drain followed by exact free-slot reuse;
- the complete native library: 910 passed, 13 ignored, zero failures.

Built an exact release wheel under `artifacts/live-update-phase2a-wheel/` and an
isolated runtime under `artifacts/live-update-phase2a-runtime/`.

The first 24-sample production burst gate initially exceeded the orchestration
timeout after writing 21 valid raw samples. Added `--resume` to the matrix runner
with fail-closed validation/screenshot requirements, plus exact
`scenario:widgets:burst-repeats` selection. The resumed run executed only the
three missing samples and produced a complete summary.

Production burst results:

- All 24 samples passed validation.
- All 12 individual/batch pairs produced identical retained geometry and
  byte-identical screenshots.
- For the 1,000-key distinct individual case, median native queue push p95 fell
  from 0.0152 ms in the Phase 1 five-run baseline to 0.0010 ms here: 15.2x lower.
- Distinct individual throughput reached 20.0 Hz in all three runs, versus a
  Phase 1 median of 18.81 Hz and minimum of 12.2 Hz.
- Worst distinct individual interaction p95 fell from 110.10 ms to 50.76 ms.
- The pure 10,000-write same-key workload remained variable because its queue is
  already shallow, but introduced no correctness or settled-state regression.

The ordered-barrier follow-up passed validation and screenshot/geometry
equivalence at 59.8 Hz individual and 60.0 Hz batch.

The realistic batched high-load dashboard also passed every validation at
45.2 Hz, compared with 46.0 Hz before the queue rewrite (-1.7%, within the
general gate). It completed 452 of 600 measured frames, drained cleanly, and
peaked at exactly 33 live/physical queue slots with zero stale entries.

Artifacts:

- `artifacts/gui-update-pipeline-phase2a-bursts/summary.json`
- `artifacts/gui-update-pipeline-phase2a-barrier/summary.json`
- `artifacts/gui-live-dashboard-phase2a/high-batch.json`

The production design is accepted at the development-gate level. Remaining
Phase 2 work is the longer repeated release soak, explicit large-payload memory
measurement, and deciding whether snapshot/structural commands should become
hard semantic barriers in a separate change.

### July 31, 2026 — Phase 2 release soak, payload memory, and final acceptance

Added private benchmark-only native sender diagnostics:

- `_queue_debug_snapshot()` returns the synchronous queue metrics without a UI
  runtime or request/response command.
- `_drain_for_test()` drains the already-private sender and returns the command
  count for fail-closed benchmark validation.

These methods are exposed only on `_NativeCommandSender`; they are not public
`dragongui` APIs. Built an exact updated release wheel under
`artifacts/live-update-phase2b-wheel/` and isolated runtime under
`artifacts/live-update-phase2b-runtime/`.

Added `benchmarks/gui_queue_payload_memory_case.py`. The release-wheel probe
submitted 200 replacements of one 8 MiB packed scatter payload without draining:

- 1.56 GiB cumulative native payload bytes copied.
- Exactly one live command and one physical queue slot remained.
- Exactly 199 scatter replacements were recorded.
- Final test drain returned only the newest command and left depth zero.
- Peak RSS grew by 8.4 MiB; cumulative submitted bytes were 190.5x that growth.
- RSS returned close to baseline after drain (59.2 MiB versus 58.7 MiB before).
- All seven payload/memory validation checks passed.

This directly proves superseded large buffers are released promptly rather than
remaining attached to stale ordering entries. Raw evidence:
`artifacts/gui-queue-payload-memory-phase2b/payload-8mib-200.json`.

Ran the five-repetition, 60-second distinct-backlog release soak with 5-second
warmups, fresh processes, screenshots, latency probes, and the isolated release
wheel:

| Run | Throughput | Push p95 | Interaction p95 | Physical high-water |
|---:|---:|---:|---:|---:|
| 1 | 14.58 Hz | 0.0018 ms | 77.00 ms | 1,003 |
| 2 | 14.17 Hz | 0.0018 ms | 76.06 ms | 1,003 |
| 3 | 14.33 Hz | 0.0018 ms | 72.00 ms | 1,003 |
| 4 | 14.02 Hz | 0.0025 ms | 75.25 ms | 1,003 |
| 5 | 14.33 Hz | 0.0017 ms | 71.02 ms | 1,003 |

All five samples passed validation, drained to depth zero, and reported zero
stale entries. Each processed roughly 1.83–1.90 million queue pushes. Median
push p95 was 0.0018 ms and worst was 0.0025 ms. Sustained throughput was lower
than the short development run but tightly grouped (14.02–14.58 Hz); callback
and command-application work, not queue insertion, is now the limiting stage.
Raw evidence: `artifacts/gui-update-pipeline-phase2-release-soak/summary.json`.

Added `benchmarks/gui_queue_scaling_case.py` to exercise the actual production
queue at 32, 1,000, and 100,000 pending property keys. With 10,000 subsequent
hot-key replacements:

| Pending keys | Distinct build | Hot replacements | Hot push p95 | Physical slots |
|---:|---:|---:|---:|---:|
| 32 | 0.080 ms | 6.990 ms | 0.0005 ms | 32 |
| 1,000 | 0.828 ms | 7.485 ms | 0.0005 ms | 1,000 |
| 100,000 | 98.123 ms | 6.898 ms | 0.0005 ms | 100,000 |

Hot-key p95 was exactly flat across the tested range. Every logical depth equaled
physical storage, every scale recorded all 10,000 replacements, all drains
returned exactly one command per key, and all 17 validations passed. Raw
evidence: `artifacts/gui-queue-scaling-phase2b/scaling.json`.

Final semantic decision: preserve the queue's existing tested cross-command
coalescing behavior. Making snapshots or structural commands hard barriers can
change what intermediate state a request observes, so it is not bundled into
this performance rewrite. If desired, it should begin as a separate correctness
proposal with explicit public semantics and before/after snapshot tests.

Phase 2 is complete. The linked queue meets the flat-through-100,000 target,
large-payload memory is bounded, burst/barrier/dashboard equivalence passes,
the full native suite passes, and the long queue-focused soak is stable.

---

### July 31, 2026 — Phase 3 text-invalidation audit started

The first Phase 3 audit found that the earlier intrinsic-text correctness fix
covered `Label`, but most other retained-tree text mutations still returned
`Dirty::Text` unconditionally. That was unsafe for content-sized buttons,
badges, panel titles, navigation labels, and loading-spinner labels: longer
content could change the widget or parent geometry without scheduling layout.
The numeric/nullable badge path also bypassed the shared text classifier.

The first conservative correction is now implemented:

- `Label` retains its previously verified rule: a single-line label needs a
  definite width, while a wrapping label needs definite width and height.
- Other supported retained-tree text and badge properties use text-only
  invalidation only when both width and height are definite. If either axis is
  intrinsic, the mutation schedules layout.
- Loading-spinner labels and the separate button/tab/navigation badge path now
  use the same classifier.
- Missing retained geometry fails closed to `Dirty::Layout`.
- Five representative composite/property pairs are covered in both intrinsic
  and fixed-geometry tests: button text, button badge, panel title, badge text,
  and loading-spinner label.

Initial mutation-path matrix:

| Family | Live textual properties | Geometry dependency | Current Phase 3 policy |
|---|---|---|---|
| Label | `text`, `label` | Intrinsic width; wrapped intrinsic height | Proven label-specific fixed-geometry rule |
| Button/small button | `text`, `label`, `badge` | Content and internal badge chrome can affect both axes | Text-only only with definite width and height |
| Badge/tag | `text`, `label`, `value`, `badge` | Pill/content dimensions are text-dependent | Text-only only with definite width and height |
| Panel/sidebar/modal/page/collapsible | `title`, `text` where supported | Header/content sizing can affect descendants and parent distribution | Text-only only with definite width and height |
| Tab/navigation/menu | `label`, `badge` | Item width and badge chrome are content-dependent | Text-only only with definite width and height |
| Loading spinner | `label`, `text` | Label changes composite width/height | Text-only only with definite width and height |
| Stateful controls | displayed values/labels | Internal chrome, editing bounds, and state may be text-dependent | Not yet eligible; audit separately |
| Plot widgets | axis labels, tick/legend options | Plot chrome changes the drawable viewport even when outer size is fixed | Dedicated target-local plot path validated by the three-pair forced-layout differential; keep it separate from ordinary retained text |
| HTML report | fallback `text` | Document layout and backend behavior differ from ordinary retained text | Dedicated target-local fallback path validated against forced layout; source/security mutations remain full sync |
| Icon button | `icon` | Theme reconciliation and icon metrics may affect internal geometry | Dedicated target-local semantic-icon path validated against forced layout after theme reconciliation |

Focused Rust verification passes all five existing label assertions and all ten
new composite cases. This is only the safe first subset of the matrix. The next
work is rendered differential testing for geometry, clipping, scrolling, hit
testing, and screenshots before expanding the fast path to any stateful control
or plot chrome.

Reason-coded observability is also in place under
`framework.live_text_invalidation` in `debug_snapshot()`. It reports total
candidates, text-only decisions, layout decisions, and fixed counters for:
fixed single-line label, fixed wrapped label, fixed composite, intrinsic width,
intrinsic height, both axes intrinsic, and unsupported property. The benchmark
case and matrix summary preserve these counters in their raw JSON. A native
counter contract test passes, and the complete native suite after the
classifier/counter work passes **913 tests** with **13 intentional ignores**.
The focused Python benchmark-validation suite also passes **6 tests**.

The first exact-wheel rendered A/B used 200 widgets, a 0.5-second warmup, a
2-second measurement window, individual and batched updates, and settled
screenshots:

| Scenario/mode | Throughput | Text candidates | Text-only | Layout |
|---|---:|---:|---:|---:|
| Fixed labels / individual | 59.99 Hz | 30,150 | 30,000 | 150 |
| Fixed labels / batch | 60.01 Hz | 30,150 | 30,000 | 150 |
| Intrinsic text / individual | 20.00 Hz | 10,653 | 0 | 10,653 |
| Intrinsic text / batch | 60.00 Hz | 30,150 | 0 | 30,150 |

The 150 fixed-case layout decisions are the deliberately intrinsic-width status
label, not failed fixed-label eligibility. Both individual/batch pairs had
identical retained-tree/geometry hashes and byte-identical settled screenshots;
all validations passed, queues drained, layout diagnostics stayed clean, and
targeted fallback remained zero. The batch result also confirms that safely
grouping intrinsic relayouts can retain 60 Hz without misclassifying them as
paint-only work. Raw evidence is under
`artifacts/gui-update-pipeline-phase3-text-ab/summary.json`.

The benchmark now fails closed if decision totals or reason totals do not match
candidate count. Fixed-label cases must predominantly exercise text-only work;
intrinsic-label cases must exercise layout and report zero text-only decisions.
The assertion-enabled exact-wheel rerun passed all **24 fixed-label checks** and
all **27 intrinsic-label checks**, sustained 60.00 Hz in both batched cases,
captured valid screenshots, and is stored under
`artifacts/gui-update-pipeline-phase3-text-validation/summary.json`.

#### Relative-size safety correction and composite matrix

The next classifier audit found that `width_value.is_some()` and
`height_value.is_some()` were too broad to prove stable geometry. That test
included `auto`, percentage, and percentage-based `calc()` values even though
they can remain content- or ancestor-dependent. The predicate now treats only
logical-pixel dimensions and validated fixed widget properties as statically
definite. Relative, calculated, and automatic axes conservatively request
layout until a later runtime predicate can prove their resolved containing
block is stable. Four native regressions cover percentage, calculated,
single-axis auto, and two-axis auto sizing.

Added two rendered pipeline scenarios covering ten rows and five composite
families per row:

- Button text.
- Standalone badge text.
- Loading-spinner label.
- Navigation-item label.
- Panel title.

The benchmark alternates short and deliberately long strings, validates all 50
final Python values and normalized native retained values, checks every adjacent
row for overlap, requires clean layout diagnostics and drained queues, and
compares settled retained geometry and screenshots between individual and batch
modes. Spinner animation is disabled in this equivalence case so time-dependent
pixels cannot masquerade as a rendering mismatch.

| Scenario/mode | Throughput | Text candidates | Text-only | Layout | Checks |
|---|---:|---:|---:|---:|---:|
| Fixed composites / individual | 60.00 Hz | 5,508 | 5,400 | 108 | 126 |
| Fixed composites / batch | 60.01 Hz | 5,508 | 5,400 | 108 | 129 |
| Intrinsic-height composites / individual | 54.00 Hz | 5,100 | 0 | 5,100 | 128 |
| Intrinsic-height composites / batch | 60.00 Hz | 5,508 | 0 | 5,508 | 131 |

The fixed case's 108 layout decisions are the intrinsic status label; all 5,400
composite decisions used the fixed-geometry path. In the intrinsic case, all
composite decisions report `intrinsic_height` and none use text-only work. Both
individual/batch pairs produced identical retained-geometry hashes and
byte-identical settled screenshots, with zero layout diagnostics and zero
targeted fallback. Raw evidence:
`artifacts/gui-update-pipeline-phase3-composites-v3/summary.json`.

The composite case now also owns a genuinely bounded 180-logical-pixel scroll
viewport (`flex-grow: 0; flex-shrink: 0`), scrolls to an intentionally excessive
offset after queues settle, and verifies native clamping at the exact scroll
maximum. Scroll offsets and maxima are included in the equivalence hash rather
than being checked only as side assertions. At 125% scale the viewport resolved
to 225 physical pixels with a 281-pixel vertical scroll range; both modes ended
at `scroll_y == scroll_max_y == 281`. Fixed and intrinsic pairs again had
identical retained geometry/scroll hashes and byte-identical bottom-scrolled
screenshots. All four runs passed 128–133 validations at approximately 60 Hz.
Raw evidence:
`artifacts/gui-update-pipeline-phase3-composites-scroll-v3/summary.json`.

The latest complete regression pass after the relative-size correction is
**914 native tests passed**, **13 intentionally ignored**, with the **6 focused
Python benchmark tests** also passing. The remaining differential gap is direct
hit-test/event-target equivalence; geometry, clipping through a bounded scroll
viewport, scroll extent/position, retained state, and screenshots now have
rendered fixed-versus-intrinsic coverage.

Direct hit-test equivalence now uses the existing native synthetic-hover
profiler. Benchmark validation fails unless every requested interactive target
is dispatched and resolved with zero missing or mismatched IDs. Buttons and
navigation items are exercised repeatedly; standalone badges, inactive
spinners, and empty panels remain geometry/screenshot targets but are correctly
excluded from the interactive expectation because they do not own hover input.
Across all four fixed/intrinsic and individual/batch runs, all ten requested
targets resolved exactly, final scroll/geometry hashes matched, screenshots
were byte-identical, and every sample sustained approximately 60 Hz. Each run
passed 132–137 checks. Raw evidence:
`artifacts/gui-update-pipeline-phase3-composites-hit-test-v2/summary.json`.

This completes the Phase 3 differential-test infrastructure for retained state,
geometry, clipping, scroll extent/position, direct interactive hit targets, and
screenshots. Coverage expansion across the remaining stateful-control, plot,
HTML-report, and icon mutation families remains part of the unfinished
widget/property matrix.

Manual review then flagged two presentation details in the composite run:

- Spinner animation is intentionally disabled only in exact screenshot A/B
  cases. A running spinner has time-dependent pixels and can fail a byte-level
  comparison even when both implementations are correct. Its label still
  changes every generation; animated behavior remains covered by the normal
  live stress workloads without an exact screenshot gate.
- The right-hand panel retained its authored horizontal width and correctly
  selected single-line ellipsis, so its width was increased from 220 to 300
  logical pixels to make the mutation easier to inspect. A later exact-frame
  review nevertheless found a separate library defect: on empty compact panels,
  almost the entire title line was vertically clipped at the bottom edge.

The follow-up strengthens the evidence rather than relying on visual judgment:
`available`, overflow, and final paint-clip rectangles now participate in the
equivalence hash; every at-least-partially-visible composite must report no
left/right container clipping; fully offscreen scrolled rows are excluded from
that horizontal assertion because their clip is correctly empty. Native tests
now explicitly cover default ellipsis for panels, navigation items, and loading
spinner labels.

The final widened matrix passed **157–161 checks per sample** at approximately
60 Hz. All interactive hover targets resolved, visible right-hand panels had
zero horizontal overflow, and both individual/batch pairs produced identical
retained geometry, clip, scroll, and state hashes plus byte-identical settled
screenshots. Raw evidence:
`artifacts/gui-update-pipeline-phase3-composites-final-v2/summary.json`.

#### Compact panel-title clipping correction

The compact-panel defect came from using the panel content-box top as the title
baseline after layout had already added title-reservation padding. With a short
empty panel, that could place the nominal title below the available header band;
the paint clip then reduced it to a few glyph fragments along the lower border.

`titled_container_geometry` now retains the resolved/authored top-padding
position whenever it fits, but clamps the title baseline to the last position
that preserves a complete title line inside the header band. This keeps existing
custom-padding behavior and corrects only the undersized case. The new
`compact_empty_titled_panel_keeps_title_inside_header_band` regression test
asserts that the title rectangle remains inside the panel with its complete line
height. The pre-existing resolved-padding test also remains green.

The final rebuilt-runtime capture shows normally aligned, ellipsized titles in
all visible right-hand panels:
`artifacts/gui-update-pipeline-phase3-panel-visual-final/long-title.png`. Matching
individual and batch runs both validated at approximately 60 Hz and produced the
same settled screenshot SHA-256
`28c1461863f7b410bf1eed5124647a88fcd791825af918907004dc40f7bc13d7`.
Validation after the correction:

- Native library: **915 passed, 13 ignored**.
- Titled-panel focus group: **15 passed**.
- Python benchmark validation: **6 passed**.

The spinner remains deliberately static only in exact screenshot-equivalence
runs; normal live stress workloads still exercise its animation.

#### Completed property-by-widget dependency matrix

The follow-up code audit traced every public live mutation that can change
displayed text, text-derived chrome, or semantic icons through
`apply_set_prop`. The matrix deliberately separates content-sized retained
widgets from controls whose outer layout contract does not depend on their
current value. `Dirty::Text` here means rebuild the target's text and dependent
primitives using its existing layout rectangle; it does not mean "text can
never affect geometry" in general.

| Widget/property family | Width/height or parent distribution | Wrap/scroll/hit-test effects | Selector/overlay/plot effects | Required invalidation policy |
|---|---|---|---|---|
| `Label.text`/`label`, no wrap | Width can be intrinsic; height is line-box based | Clip changes inside a fixed width; hit rectangle changes when intrinsic | Property values are not CSS selector inputs | `Text` only with definite width; otherwise `Layout` |
| Wrapped `Label.text`/`label` | Width and shaped line count can change height | Can change ancestor distribution and scroll maximum | No property selector dependency | `Text` only with definite width and height; otherwise `Layout` |
| `Button`/`SmallButton.text`, `label`, `badge` | Caption and badge chrome can affect both axes | Can move sibling hit rectangles and scroll limits | Hover/focus selectors depend on state, not the string | `Text` only with definite width and height; otherwise `Layout` |
| `Badge`/`Tag.text`/`label` | Pill metrics are content-sized on both axes | Can move siblings; normal clipping applies when fixed | Level changes are a separate full/style-bearing path | `Text` only with definite width and height; otherwise `Layout` |
| `Panel`/`Sidebar`/`Modal`/`Page.title`; `Collapsible.title`/`text` | Header metrics can change the container and descendants' available space | Can alter descendant clip/scroll and header hit regions | Open/sidebar state use separate layout/full paths | `Text` only with definite width and height; otherwise `Layout` |
| `Tab`/`NavItem`/`Menu`/`MenuItem.label`; tab/nav `badge` | Label and badge affect route-item chrome | Can move later targets and change bar overflow | Active/hover state is independent of label content | `Text` only with definite width and height; otherwise `Layout` |
| `LoadingSpinner.label`/`text` | Spinner-plus-label composite is content-sized | Label can change sibling placement; animation is paint-time state | `spinning`, speed, and size use the spinner-specific path | `Text` only with definite width and height; otherwise `Layout` |
| `Selectable`/`RadioButton`/`TreeNode`/`Checkbox`/`ToggleSwitch.text`/`label` | Caption is part of content-sized control chrome | Can change sibling/hit geometry | Checked/expanded state is a separate full/layout path | Same conservative definite-width-and-height classifier |
| `NumberInput`/`DragNumber.text`/`label` | Authored captions can affect composite size | Can alter field/chrome allocation | Numeric `value` is separate state-backed text | Captions use the conservative classifier; live numeric value uses targeted `Text` inside the established field rectangle |
| `ProgressBar.text`/`label` | Authored overlay copy can participate in intrinsic sizing | Fixed bars clip overlay text; value changes only fill geometry | `value` changes primitives, not text layout | Caption uses the conservative classifier; value remains targeted `Visual` |
| `TextInput`/`TextArea`/`CodeEditor`/`LogView.value` | Outer control sizing comes from style/native control geometry, not current buffer contents | Rebuilds internal wrapping, caret and text scroll; `LogView(follow)` also changes internal scroll state | Focus/placeholder state is handled by the text renderer | Targeted `Text`; no parent layout. Multiline/caret bounds remain in the target subtree |
| `Dropdown.value` | Selection changes text inside established dropdown chrome | Does not resize the control or move its hit rectangle | Open overlay contents/options are unchanged | Targeted `Text` |
| `LinePlot` axis labels, axes/ticks/toolbar/legend options, tick count, window size | Outer plot rectangle is layout-owned and independent of plot chrome | Plot viewport/chrome is recomputed inside that rectangle | Can change ticks, legend, toolbar and plot resource viewport | Plot-specific targeted `Text`, which rebuilds target primitives, plot resources and text; never ordinary retained-text classification |
| `Histogram`/`BarChart` axis labels, axes/ticks/toolbar options, tick count | Same fixed outer-rectangle contract as line plot | Internal drawable viewport and ticks change | Plot chrome changes are target-local | Plot-specific targeted `Text` |
| `HtmlReport.text` | Native geometry supplies a stable height/minimums; fallback copy does not size the webview | Wrapped fallback copy is clipped within the report rectangle | `path`, `html`, permissions and external fallback are separate `Full` webview-source mutations | Targeted `Text` for fallback copy only; source/security changes remain `Full` |
| `IconButton.icon` | Semantic icon is painted within established icon-button chrome | Hit rectangle remains the button rectangle | Live icon-theme reconciliation may change icon paint/metrics, not outer layout | Icon-specific targeted `Text` after theme reconciliation |

Audit conclusions:

- CSS selectors do not inspect live text/value strings, so these mutations do
  not silently change selector matching. State-bearing mutations such as
  `checked`, `expanded`, route selection, sidebar state, and badge `level` keep
  their existing stronger invalidations.
- Percentage, `calc()`, and `auto` dimensions are not accepted as proof of
  stable geometry for content-sized retained widgets. A resolved rectangle is
  insufficient by itself because a later intrinsic measurement can still
  redistribute its parent.
- The plot `Dirty::Text` routes are intentional special cases. Targeted
  primitive rebuild detection includes `LinePlot`, and rebuilds its GPU plot
  resources when the selected target subtree contains one. Histogram and bar
  chart chrome is rebuilt with the target's primitives/text.
- `HtmlReport.text` is fallback presentation only. Webview source selection is
  derived from `path` or inline `html`; those mutations, base-directory and
  security flags continue to request `Full` synchronization.
- Spinner animation remains enabled in normal workloads. It is disabled only
  in exact-pixel differential cases, where clock-dependent pixels would make
  otherwise equivalent samples non-deterministic.

This closes the static dependency matrix. It does **not** expand the
content-sized fast path beyond the families that passed rendered fixed versus
intrinsic equivalence. Stateful editor/dropdown, plot-chrome, HTML-fallback and
icon cases remain explicit target-local routes; each now has dedicated rendered
optimized-versus-forced-layout evidence.

The matrix audit also corrected the classifier's advertised property set.
`Badge.value`, `Badge.badge`, `Tag.value`, menu/menu-item badges, and panel
`text` had no corresponding retained mutation route but were previously
reported as potentially eligible. They now fail closed as unsupported instead
of polluting eligibility counters. A table-driven native contract covers 32
real widget/property pairs in both fixed and intrinsic-height forms and six
unsupported pairs. The post-audit regression run passes **917 native tests**
with **13 intentional ignores**; the focused benchmark-validation suite passes
**6 tests**. `git diff --check` is clean for the implementation and plan.

#### Forced-safe text-invalidation differential

Added the independent safe control that the initial Phase 3 comparisons were
missing. The runtime now reads
`DRAGONGUI_DIAGNOSTIC_TEXT_INVALIDATION_MODE` once while constructing GPU state.
The default `optimized` mode preserves the classifier. Diagnostic
`forced-layout` mode promotes only otherwise eligible `Dirty::Text` decisions
to `Dirty::Layout`; already intrinsic and unsupported decisions retain their
original safe reason.

The configured mode is captured under `framework.live_text_invalidation.mode`,
and promoted decisions are counted separately as `forced_safe_layout`. A native
regression proves the diagnostic mode promotes the fixed fast path without
rewriting an intrinsic layout decision.

The update-pipeline case and matrix now accept a separate text-invalidation
dimension. `--text-invalidation-mode both` runs optimized and forced-layout
samples in fresh processes while holding the scenario, widget tree, CSS,
viewport, scale, update mode, update sequence, and settled generation fixed.
Raw samples fail unless their requested and native-reported modes agree. The
matrix independently compares retained state, resolved/available geometry,
clips, scroll state and settled screenshots across invalidation modes; its
existing individual/batch comparison remains available as a separate axis.

Built a fresh ABI3 release wheel and isolated runtime:

```text
artifacts/live-update-phase3d-forced-layout-wheel/dragongui-0.1.0-cp312-abi3-win_amd64.whl
artifacts/live-update-phase3d-wheel-runtime
```

The three-repetition, 200-fixed-label differential used batched setters, a
0.5-second warmup, two measured seconds, fresh processes, and settled
screenshots. All three optimized/forced pairs passed:

- Both modes sustained approximately 60 Hz.
- Each sample recorded 30,150 candidates: 30,000 fixed labels plus 150
  intrinsic-width status-label decisions.
- Optimized samples recorded 30,000 `fixed_single_line_label` text-only
  decisions; controls recorded zero text-only decisions and 30,000
  `forced_safe_layout` decisions.
- Targeted fallback remained zero in every sample.
- Every pair had the same retained geometry/clip/scroll/state hash
  `c669df93a758366f32570c18eb91e0773e7f769a5a8125caef5e34bee868f4aa` and
  byte-identical screenshot hash
  `b66f5b90570785bb5312f8656566c30c164dc2bf80d06fd6119bc30f2d177a80`.

Median rebuild-flush p95 was **9.9311 ms optimized** versus **11.3819 ms
forced-layout**, a **12.75% reduction**. This is directional short-run evidence,
not closure of the 20% Phase 3 performance target; the longer repeated release
gate remains open. Authoritative local evidence:

```text
artifacts/gui-update-pipeline-phase3-forced-layout-wheel-final-v1/summary.json
```

Post-change validation:

```text
cargo test --manifest-path native/Cargo.toml --target x86_64-pc-windows-gnu --lib
918 passed, 13 ignored, 0 failed

python -m pytest tests/test_gui_benchmark_validation.py -q
6 passed

python -m pytest -q
567 passed
```

#### State-backed control differential and focused-caret correction

Extended the forced-safe control across the existing target-local state-backed
text routes. Optimized mutations now report `target_local_state`; the diagnostic
control promotes those same decisions to `forced_safe_layout`. This covers:

- `TextInput.value`.
- `TextArea.value`.
- `CodeEditor.value`.
- `LogView.value` with both `follow=True` and `follow=False`.
- `Dropdown.value`.
- `NumberInput.value`.
- `DragNumber.value`.

The rendered `state-controls` scenario uses empty strings, precomposed and
combining Unicode, emoji, Japanese, RTL Arabic, long values and multiline
values. It validates Python and native buffer values, numeric/dropdown state,
outer geometry, available and paint clips, native hit targets, focus retention,
relative caret position, internal text scroll, wrapped visual lines, per-owner
shaped text bounds and screenshots.

New diagnostic infrastructure supports that proof:

- `DRAGONGUI_SYNTHETIC_FOCUS_ID` applies one verified diagnostic focus target
  after the first application frame and records requested, resolved and applied
  state in the runtime snapshot.
- Opt-in `renderer.text_owner_geometry` reports requested retained owners'
  shaped entry origin, scale, visual line count, measured text extent and final
  clip. `DRAGONGUI_DIAGNOSTIC_TEXT_GEOMETRY_IDS` is read once per runtime so
  ordinary large-tree snapshots do not acquire this payload.
- The equivalence hash now includes native text/numeric/dropdown buffers,
  cursors, internal scroll, focused id, caret positions and owner text geometry.

The first state-control smoke found a real correctness defect. Replacing the
value of a focused multiline editor moved its cursor to the end but reset
internal scroll to zero; the shaped caret ended below the visible control. The
live mutation path now computes focused multiline cursor visibility before the
rebuild. In the corrected case, the focused text area retains focus, records a
16-pixel internal scroll and places its caret at relative y=74 inside its
84-pixel outer rectangle in both optimized and forced-layout modes.

Built a fresh ABI3 wheel and isolated runtime:

```text
artifacts/live-update-phase3e-state-controls-wheel/dragongui-0.1.0-cp312-abi3-win_amd64.whl
artifacts/live-update-phase3e-state-controls-runtime
```

The final three-repetition differential used batched updates, a 0.5-second
warmup, two measured seconds, fresh processes and settled screenshots. All
three pairs passed 57 optimized or 58 control checks per sample:

- Both modes sustained approximately 60 Hz.
- Each sample recorded 1,350 candidates: 1,200 state-control decisions plus 150
  intrinsic-width status-label decisions.
- Optimized samples recorded 1,200 `target_local_state` text-only decisions;
  controls recorded zero text-only decisions and 1,200 `forced_safe_layout`
  decisions.
- Targeted fallback remained zero in every sample.
- Every pair retained the same focus, caret `[47.0615234375, 74.0]`, 16-pixel
  text-area scroll, following/non-following log scroll behavior, geometry,
  clips, buffers and text-owner bounds.
- Every pair had equivalence hash
  `91c9ffa8904064dd48a27dc98c171bf2640e1f740f37fb6f146755aa50cee901` and
  byte-identical screenshot hash
  `37c006aef5267a0d0e18327f0872e3e506fdcbf0c16dae88143fa58fa46beb3a`.

Authoritative local evidence:

```text
artifacts/gui-update-pipeline-phase3-state-controls-final-v1/summary.json
```

#### Plot-chrome differential and resource-rebuild proof

Added the rendered `plot-chrome` case for fixed-size `LinePlot`, `Histogram`,
and `BarChart` widgets. Each generation mutates Unicode and long axis labels,
grid/axes/ticks/toolbar visibility, and tick count. The line plot additionally
cycles legend visibility/position, moving-window size, and explicit x/y bounds
overrides so the test proves bounds precedence as well as chrome rendering.

Plot text mutations now participate in the existing forced-safe diagnostic:
optimized decisions report `target_local_plot` and retain targeted
`Dirty::Text`; forced-layout promotes the same candidates to layout. This does
not merge plots into the general label classifier. The plot-specific path
remains explicit because its chrome changes the internal drawable viewport and
line-plot GPU resource mapping while the authored outer rectangle stays fixed.

New opt-in proof data includes:

- `renderer.plot_geometry`, keyed by requested plot id, with fixed outer
  rectangle, visible clip, internal drawable viewport, and resolved bounds.
- Widget ownership on ephemeral plot overlay text, allowing
  `renderer.text_owner_geometry` to validate tick, axis, toolbar, and legend
  clips without adding payload to ordinary snapshots.
- Line-plot retained rebuild counters and final GPU renderer stats in benchmark
  reports and matrix summaries.
- Plot geometry and owned label geometry in the cross-mode equivalence hash.

The first smoke correctly failed because plot overlay labels were anonymous to
the owner diagnostic even though they rendered. The probe now scans requested
ephemeral entries as well as permanent retained entries. The first longer run
then rejected a benchmark-generated x range beyond the static dataset; the
bounded replacement still tests window/bounds precedence while guaranteeing
visible source data for the GPU resource assertion.

Built a fresh ABI3 release wheel and isolated runtime:

```text
artifacts/live-update-phase3f-plot-chrome-wheel/dragongui-0.1.0-cp312-abi3-win_amd64.whl
artifacts/live-update-phase3f-plot-chrome-runtime-current
```

The final three-repetition differential used batched updates, a 0.5-second
warmup, two measured seconds, fresh processes, and settled screenshots. All
three optimized/forced pairs passed 59 or 60 checks per sample:

- Both modes sustained approximately 60 Hz.
- Every sample recorded 3,300 invalidation candidates: 3,150 plot decisions
  and 150 intrinsic-width status-label decisions.
- Optimized samples recorded 3,150 `target_local_plot` text-only decisions;
  controls recorded zero text-only decisions and 3,150 `forced_safe_layout`
  decisions.
- All three authored outer rectangles remained exactly 900 by 210 logical
  pixels. Final internal viewports were 838 by 162 for the line plot and 842 by
  158 for both histogram and bar chart.
- Owned final text geometry contained 15 line-plot, 13 histogram, and 20
  bar-chart entries with clips bounded by their outer plots.
- The line renderer retained two series, 30 visible points, 28 segments, and
  672 bytes of GPU source buffers. Targeted retained diagnostics recorded two
  line-plot resource rebuilds and one non-line target skip with balanced checks.
- Every pair had equivalence hash
  `5f6227800ae47e075fee3add61bded7e00d5452e077b5958990481eafe3d7057`
  and byte-identical screenshot hash
  `193147f1f4feb0067d67681c79ce962c2492f6c4b9df5e9d473505731a0bb769`.

Authoritative local evidence:

```text
artifacts/gui-update-pipeline-phase3-plot-chrome-final-v1/summary.json
```

Full validation remains green: **918 native tests passed**, **13 intentionally
ignored**, **567 Python tests passed**, and the **6 focused benchmark-validation
tests passed**.

#### HTML fallback, embedded WebView2, and semantic-icon differential

Added `html-fallback`, `html-webview`, and `semantic-icons` to the rendered
pipeline case and matrix. The HTML cases distinguish frequently changing
fallback copy from full-sync source and permission changes. Stable FNV-1a
fingerprints prove final inline and embedded source identity without copying
HTML bodies into diagnostics. The icon case rotates built-in aliases, a custom
stroke resource, and an unknown-name fallback while replacing the live icon
theme; the equivalence hash includes the computed semantic identity.

The optimized runtime now reports explicit `target_local_html_fallback` and
`target_local_icon` reasons. Forced-layout promotes those same decisions, while
HTML `path`, `html`, base-directory, and security mutations continue to use
full synchronization. Fixed report/button rectangles, clips, semantic state,
WebView instance bounds and policy, synthetic hover targets, final renderer
health, and screenshots are all validation gates.

The first combined matrix found a WebView2 transition race rather than an
invalidation mismatch. After a file-to-inline navigation, an immediate
inline-to-file request could fail with `ERROR_INVALID_STATE` and retain the old
source. WebView instances now register `NavigationCompleted`; completion wakes
the runtime, which re-syncs the current document and retries any desired source
that was not accepted. A later successful sync clears the current renderer
error. The benchmark's bounded reload fallback was left unchanged as a failure
detector; the corrected runtime needed zero reload attempts.

A fresh release-wheel matrix ran three repetitions with batched updates, a
0.5-second warmup, two measured seconds, fresh processes, and settled
screenshots. All 18 samples passed at approximately 60 Hz, and all nine
optimized/forced pairs matched retained geometry and screenshots. Cross-mode
equivalence hashes were stable across repetitions:

- HTML fallback:
  `582363c0ccc9c4b23138ceeeb8ee1eb62063077cb2f9ff75f2c8129191971ae6`.
- Embedded WebView2:
  `53d0e81f6b58954da8a74f17805295cad9b46535314da86aa2787bdbe4b00b05`.
- Semantic icons:
  `7dbf88f456568f28857fd2cef27bb881d64bacfda1b1c0228a4292590e338f4c`.

All six WebView samples ended with a local-file source, matching source
fingerprints, zero reload attempts, and no final renderer error. Optimized HTML
fallback/WebView samples each accumulated 456 target-local HTML decisions over
three repetitions; optimized icon samples accumulated 1,800 target-local icon
decisions. Forced controls promoted the corresponding totals to layout. The
semantic-icon screenshot hash was
`df57426f9ba0912123b458e5440466ae56c71f405bb93bdee8eebdf7e5962d5e`;
the HTML cases shared settled screenshot hash
`0acc587ddb1e25eeeaf4330c2ed8b706d583f421225132d4561dcaadcb576a75`.

Authoritative local evidence and runtime:

```text
artifacts/gui-update-pipeline-phase3-html-icon-final-v2/summary.json
artifacts/live-update-phase3g-html-icon-wheel/dragongui-0.1.0-cp312-abi3-win_amd64.whl
artifacts/live-update-phase3g-html-icon-runtime-navretry
```

#### Five-by-60-second fixed-label release gate

Ran the required phase-decision sample for `labels-fixed:200`: five 60-second
measured repetitions per mode, five-second warmup, batched updates, alternating
fresh processes, settled screenshots, and the Phase 3g release wheel. A command
wrapper timeout interrupted the first invocation after three already-complete
samples; `--resume` audited and reused only those passing raw samples, then ran
the remaining seven in fresh processes.

All ten samples validated. All five optimized/forced pairs had identical
retained geometry and byte-identical screenshots, with equivalence hash
`7397ef3ee727309c3782a7e4543e75bc2bbdf60cff49fc3266241db7d92b4914`
and screenshot hash
`a5b819cda657a87d4446ca68d5ba99e89d4d8b823a83cc40fb3f53074249fd07`.
Median throughput was 59.9335 Hz optimized and 59.9838 Hz forced-layout.

The correctness and fallback gates pass:

- Optimized samples recorded 3,894,000 fixed-label text-only decisions. Their
  19,470 layout decisions were the deliberately intrinsic status label, not
  fixed-label fallback.
- All 58,410 targeted batches completed with zero targeted fallback, a 0%
  fallback rate against the below-5% target.
- Queues drained, layout diagnostics stayed clean, and all cross-mode state,
  geometry, and screenshot comparisons passed.

This first measurement did **not** pass the performance gate. Rebuild-flush p95
values were:

| Mode | Five p95 samples (ms) | Median (ms) | Worst (ms) |
|---|---|---:|---:|
| Optimized | 10.1567, 10.3872, 12.3512, 10.5711, 14.6792 | 10.5711 | 14.6792 |
| Forced layout | 12.1743, 12.3149, 12.0998, 11.7736, 12.2937 | 12.1743 | 12.3149 |

The median reduction was **13.17%**, below the required 20%. Stage diagnostics
then showed that this was not a pure fixed-label measurement: the benchmark's
intrinsic-width status label was mutated every generation and forced a global
layout pass averaging about 10.34 ms in every optimized frame. The targeted
partial-text p95 itself was only 2.07 ms. This contradicted the Phase 3 target
that the fixed-geometry workload perform zero layout passes.

The eligibility decision is complete: do not broaden the general
content-sized predicate merely to unify code. Stateful controls, plot chrome,
HTML fallback, and semantic icons remain on their explicit target-local routes,
which now have zero-mismatch differential evidence. Any future expansion must
name a new geometry contract and show a measurable benefit.

Authoritative local evidence:

```text
artifacts/gui-update-pipeline-phase3-fixed-label-release-v1/summary.json
```

The benchmark correction gives `pipeline-status` a definite width only in
`labels-fixed`; `intrinsic-text` retains the intrinsic sentinel and continues
to prove conservative fallback. Validation is now stricter: optimized
`labels-fixed` must report exactly zero layout decisions and every candidate
must use text-only invalidation. Forced-layout must still reject all text-only
work. A 10-second smoke immediately passed with 0.1212 ms optimized versus
12.0714 ms forced-layout rebuild-flush p95 and exact geometry/screenshots.

The corrected five-by-60-second gate then ran all ten samples from scratch:

| Mode | Five p95 samples (ms) | Median (ms) | Worst (ms) |
|---|---|---:|---:|
| Optimized | 0.0959, 0.0929, 0.1138, 0.1341, 0.1217 | 0.1138 | 0.1341 |
| Forced layout | 11.6644, 11.5034, 11.9257, 12.3257, 12.8901 | 11.9257 | 12.8901 |

The authoritative median reduction is **99.05%**, and the worst-p95 reduction
is **98.96%**. Optimized mode recorded 3,919,500 candidates, all 3,919,500 on
the text-only path, zero layout decisions, 78,000 completed targeted batches,
and zero fallback. Median throughput was 59.9999 Hz optimized and 59.9668 Hz
forced-layout. All five cross-mode pairs matched equivalence hash
`66e69e31c25da886961ee8a4f0a2d7452b663e6a3631e70bf7a9d06e33b9dd12`
and screenshot hash
`a5b819cda657a87d4446ca68d5ba99e89d4d8b823a83cc40fb3f53074249fd07`.

This closes all Phase 3 targets: zero layout for eligible fixed geometry, zero
differential mismatches, 0% targeted fallback, and more than 20% lower
rebuild-flush p95. Authoritative evidence:

```text
artifacts/gui-update-pipeline-phase3-fixed-label-release-v2/summary.json
```

## Baseline policy after the workstation change

The August 1 continuation is running on a substantially faster computer than
the machine that produced the July 31 baseline below. Those historical absolute
throughput, latency, CPU, and memory values are context only; they must not be
used to claim a regression or improvement on the current workstation.

Same-run optimized/control percentages from Phase 3 remain valid because both
modes ran alternately on this computer with the same wheel, workload, and
settings. Before Phase 4 behavior changes, capture a new current-machine control
baseline for every affected workload. Record CPU, GPU, memory, OS, display
scale, Python/Rust versions, wheel identity, viewport, and present mode in the
artifact. All Phase 4 performance decisions must compare fresh candidate runs
against that current-machine baseline under matched conditions.

The first attempted baseline exposed a benchmark contract defect rather than a
native locality failure. `mixed-state` still gave its badges intrinsic geometry
and updated an intrinsic-width status label. A representative v1 sample
therefore recorded 189,543 layout decisions and zero targeted batches. That
artifact is retained as diagnostic evidence, but is not the fixed-dashboard
control required by Phase 4.

The corrected scenario gives badges and the status label definite geometry.
Validation now requires optimized mixed-state runs to report zero layout,
nonzero targeted completion, and zero fallback. The authoritative pre-change
baseline on the Ryzen 7 5800X / RTX 3080 Ti / approximately 64 GiB workstation
used Python 3.12.12, the Phase 3g wheel, monitor-default scale, runtime-default
present mode, a five-second warmup, and five fresh 60-second measurements:

| Metric | Five samples | Median | Worst |
|---|---|---:|---:|
| Throughput (Hz) | 59.8008, 59.3331, 59.7834, 56.6667, 59.2998 | 59.3331 | 56.6667 |
| Command drain p95 (ms) | 16.0391, 16.1458, 15.8612, 15.8834, 16.3939 | 16.0391 | 16.3939 |
| Command apply p95 (ms) | 15.1372, 14.5967, 14.8355, 14.8414, 15.3339 | 14.8414 | 15.3339 |
| Rebuild flush p95 (ms) | 1.1492, 1.1903, 1.1010, 1.0609, 1.2523 | 1.1492 | 1.2523 |

Across all five runs, 249,301 targeted batches completed with zero fallback,
7,689,977 text candidates used the fast path, and layout remained zero. The
existing mixed text/visual target merge therefore already clears the Phase 4
95% fixed-dashboard completion target; the first new native increment should
address stale-target structure generations and differential verification.

```text
artifacts/gui-update-pipeline-phase4-current-machine-baseline-v1/summary.json  # invalid intrinsic-geometry diagnostic
artifacts/gui-update-pipeline-phase4-current-machine-baseline-v2/summary.json  # authoritative fixed-dashboard control
```

#### August 1, 2026 — Phase 4a structure-generation guard

The deferred rebuild pipeline already maintains separate text, visual,
table-text, and overlay targets, normalizes ancestor/descendant roots, and
merges safe visual targets into targeted text execution. The corrected fixed
dashboard proved this existing path completes 100% of targeted batches. The
first missing safety primitive was an explicit retained-tree generation.

`WgpuState` now increments `structure_generation` after each successful node or
children replacement and captures the current generation when a deferred batch
begins. A targeted candidate whose captured generation differs from the current
tree is rejected before root normalization and forced through `Dirty::Full`.
Diagnostics expose both `framework.structure_generation` and
`command_text_rebuilds.stale_generation_fallback_batches`. Structural commands
already merge as `Full`; the generation check is a second, explicit safety
barrier against future target-lifetime changes.

The counter contract and generation-mismatch eligibility regression pass, as
do the complete native and Python suites. A fresh Phase 4a release-wheel smoke
used the corrected `mixed-state:200` workload for ten measured seconds per
mode:

- Optimized: 59.9996 Hz, 1.3540 ms rebuild-flush p95, 8,580 targeted batches,
  zero layout, zero fallback, zero stale-generation fallback, generation zero.
- Forced layout: 18.9988 Hz and 6.4872 ms rebuild-flush p95.
- Both modes had equivalence hash
  `80652a5c0bb50585dd284931f0f98dbc22182ac69320fb66686342ac8076af9a`
  and screenshot hash
  `6c5ec803bb2de9a39f9d0a8b625be71a4480c85441cdfe58e775360202612492`.

Evidence and exact runtime:

```text
artifacts/gui-update-pipeline-phase4a-structure-generation-smoke-v2/summary.json
artifacts/live-update-phase4a-structure-generation-wheel/dragongui-0.1.0-cp312-abi3-win_amd64.whl
artifacts/live-update-phase4a-structure-generation-runtime
```

#### August 1, 2026 — Phase 4b targeted-versus-full retained-output verifier

Added the startup-selected diagnostic mode
`DRAGONGUI_DIAGNOSTIC_TARGETED_REBUILD_MODE=verify-full` and corresponding
benchmark option `--targeted-rebuild-verification verify-full`. After every
successful deferred targeted rebuild, the runtime now captures process-local,
deterministic signatures for:

- retained primitive instances in paint order, including the base/overlay
  boundary;
- permanent shaped-text entries in paint order, including owner identity,
  authored text/style key, submitted geometry and clips, colors, custom glyphs,
  and shaped glyph positions;
- retained line-plot point and series GPU inputs.

The diagnostic then runs full text and primitive reconstruction without syncing
state transitions a second time, compares each component, records attempts,
matches, component mismatch counters, elapsed time, and last signatures, and
leaves the full reconstruction installed as a safe control result. The default
mode is `off`, so production and ordinary benchmark runs do no signature or
double-rebuild work.

A fresh release wheel passed the corrected `mixed-state:200` rendered smoke
with a 0.5-second warmup and ten measured seconds. It verified all **3,614**
completed targeted batches with zero fallback and **zero primitive, text, or
line-plot mismatches**. Throughput was 26.4923 Hz because verification
deliberately performs targeted and full work; it is diagnostic overhead, not a
new performance baseline. The final equivalence hash was
`68ea50c491c0add10cf02e460cdf8b2fb99323ebb00d245750449a7b5fe15de4`
and screenshot hash was
`c1823d113c5bc549ace3263400106388bcca6eb3af984065eda2b8c87052dafe`.

Complete regressions passed: Rust **920 passed / 13 ignored**, Python **567
passed**. Focused mode/counter tests also classify primitive, text, and line-plot
mismatches independently.

```text
artifacts/gui-update-pipeline-phase4b-targeted-verifier-smoke-v1/summary.json
artifacts/live-update-phase4b-targeted-verifier-wheel/dragongui-0.1.0-cp312-abi3-win_amd64.whl
artifacts/live-update-phase4b-targeted-verifier-runtime
```

Wheel SHA-256:
`e0aff8649ab735aa0ff3fc4fc365772cd5d0e1246876366ec37fb19d5c627d8d`.
Extracted `_dragongui.pyd` SHA-256:
`4e020b32a3b20e9b6c0ca4a188abf645cbf138f7b3ed28500939f8fdd8ffe0e7`.

This closes the core retained-renderer verifier increment. Scatter/image/WebView
resource generations and layout/hit-test signatures remain follow-up coverage
when those target classes are made eligible. The next implementation work is
typed target classes plus randomized 1-of-1,000 / 20-of-1,000 locality probes.

#### August 1, 2026 — Phase 4c locality scaling controls

Added `--locality-roots N` to the fixed-label case and matrix. Each callback
selects `N` distinct labels with a deterministic per-tick pseudo-random schedule.
Validation hashes all 1,000 final Python and native label values and requires:

- rebuilt roots greater than zero and no more than `(N + 1)` per completed
  callback, where the extra root is the fixed status label;
- removed and inserted text entries no greater than rebuilt roots;
- nonzero targeted completion with zero ordinary/stale-generation fallback;
- zero partial-primitive fallback and zero primitive upload bytes.

No new native counter was required: existing rebuilt-root, partial-text-entry,
partial-primitive, and upload-byte diagnostics cover this fixed-label locality
contract. Because only benchmark/test code changed after Phase 4b, the controls
reuse the exact Phase 4b wheel/runtime rather than producing a misleading new
binary identity.

The current-machine controls used batched optimized updates, 1,000 retained
labels, two-second warmups, 15-second measurements, 60 Hz targets, screenshots,
and three fresh processes per mutation scale:

| Probe | Throughput samples (Hz) | Drain p95 median | Apply p95 median | Flush p95 median |
|---|---|---:|---:|---:|
| 1 of 1,000 | 60.0006, 59.8654, 59.9999 | 0.5984 ms | 0.2115 ms | 0.3774 ms |
| 20 of 1,000 | 59.9999, 60.0014, 60.0016 | 1.0011 ms | 0.5028 ms | 0.5029 ms |

The 1-root samples rebuilt 2,040, 2,036, and 2,040 roots, exactly two per
completed callback. Every 20-root sample rebuilt 21,420 roots, exactly 21 per
callback. Text entries inserted were 1,046 / 1,044 / 1,046 and 1,600 / 1,600 /
1,600 respectively; offscreen fixed labels correctly produced no retained text
entry churn. Every sample recorded zero targeted fallback, zero stale-generation
fallback, and zero primitive upload bytes. Increasing the label mutation set by
20 times increased median rebuild-flush p95 by only **1.33 times**, while both
cohorts sustained the 60 Hz target.

Within-cohort equivalence and screenshot hashes were stable:

- 1 of 1,000: equivalence
  `b1f864aa92beadaba6d9cd86fc51898fcabf639fa5a2063fc72b3a487282204c`,
  screenshot
  `5cb9de7513f45bceb84fc755bb9ccb08759e1383fd23bb5eb03721d0667f5e03`.
- 20 of 1,000: equivalence
  `9e01ea3c23d4f2d3337c6a1b92ae7f22dac58bf7c6c313921adeeb8330e22bd5`,
  screenshot
  `e983052eba97ce8109728b5fc1d898f6d207e9cda7a846e68e182461989c928d`.

A separate ten-second 20-of-1,000 `verify-full` run compared **660** randomized
targeted batches with full reconstruction. It reported zero primitive, text,
or line-plot mismatches, zero fallback, and 60.0008 Hz. This does not yet close
the 10,000-randomized-batch Phase 4 gate. Focused locality tests pass and the
complete Python suite reports **568 passed**. Ruff was not available in the
active MSYS2 Python environment; syntax compilation and pytest passed.

```text
artifacts/gui-update-pipeline-phase4c-locality-1of1000-v1/summary.json
artifacts/gui-update-pipeline-phase4c-locality-20of1000-v1/summary.json
artifacts/gui-update-pipeline-phase4c-locality-20of1000-verify-v1/summary.json
```

The next work is the typed-target audit/implementation and then the remaining
9,340 randomized differential comparisons required to reach 10,000.

#### August 1, 2026 — Phase 4d typed deferred targets and randomized gate

The audit found seven parallel `WgpuState` fields representing one deferred
batch. They are now consolidated into `DeferredRebuildBatch`, which owns the
captured structure generation, merged dirty class, and a typed target container:

- `RetainedVisual` widget roots for targeted text plus primitive reconstruction;
- `PrimitivePaint` roots that can safely piggyback on a retained-visual batch;
- table-owned text roots and overlay text as distinct classes;
- explicit retained-visual and primitive-paint global-fallback requirements;
- scatter-style synchronization state.

The 64-root packet boundary continues to use the sum of raw class-set sizes,
including a widget present in more than one class. This preserves the previous
conservative split behavior. Visual roots are unioned only at flush, then use
the existing ancestor normalization, generation guard, targeted renderer, and
full fallback. No renderer or fallback policy was broadened.

Added additive request diagnostics under
`framework.command_text_rebuilds.target_classes` for retained visual, primitive
paint, table text, and overlay text. Existing aggregate request/completion and
fallback counters retain their meanings. Focused tests cover class insertion,
cross-class unioning, table/overlay preservation, raw batch limits, dirty merge
order, generation capture, mismatch classification, and the existing safe
fallback predicates.

Fresh Phase 4d release-wheel rendered evidence:

- `labels-fixed:1000`, randomized 20-root `verify-full`: 60.0001 Hz, 13,860
  retained-visual requests, zero primitive/table/overlay requests, 660 matches,
  zero mismatch/fallback/stale fallback/primitive upload. Equivalence and
  screenshot hashes exactly match the Phase 4c verifier smoke.
- `mixed-state:200`, `verify-full`: 108,671 retained-visual requests, 162,600
  primitive-paint requests, 3,523 matches, and zero mismatch/fallback/stale
  fallback. Equivalence
  `80652a5c0bb50585dd284931f0f98dbc22182ac69320fb66686342ac8076af9a`
  and screenshot
  `6c5ec803bb2de9a39f9d0a8b625be71a4480c85441cdfe58e775360202612492`
  match the authoritative Phase 4a rendered result. Its 24.4014 Hz is expected
  double-rebuild diagnostic throughput.

The sustained randomized gate used the same 20-of-1,000 schedule for a
one-second warmup plus 145 measured seconds. It sustained 59.9999 Hz and
completed **8,760/8,760** targeted/full comparisons with zero primitive, text,
or line-plot mismatches, zero fallback, zero stale-generation fallback, and zero
primitive upload bytes. It rebuilt 183,960 roots, exactly 21 per callback, and
routed all 183,960 requests through `RetainedVisual`.

Combined randomized locality differential coverage is now:

- Phase 4c verifier probe: 660 matches.
- Phase 4d typed-target locality smoke: 660 matches.
- Phase 4d sustained gate: 8,760 matches.
- Total: **10,080 matches, zero mismatches**.

This closes the Phase 4 requirement of zero differential mismatches over 10,000
randomized mutation batches. Sustained-gate equivalence hash:
`7592f7cdd6adf56e14ff6a440ced0d9d9fc26ebead5901c1f0c4e3361aa3da85`;
screenshot hash:
`927cbe66454678be2a80a704d3d365a53ecd02336752d59479cb0538f7343d68`.

Complete regressions pass: Rust **922 passed / 13 ignored**, Python **568
passed**.

```text
artifacts/gui-update-pipeline-phase4d-typed-targets-locality-smoke-v1/summary.json
artifacts/gui-update-pipeline-phase4d-typed-targets-mixed-smoke-v1/summary.json
artifacts/gui-update-pipeline-phase4d-randomized-10000-gate-v1/summary.json
artifacts/live-update-phase4d-typed-targets-wheel/dragongui-0.1.0-cp312-abi3-win_amd64.whl
artifacts/live-update-phase4d-typed-targets-runtime
```

Wheel SHA-256:
`512286b80e0289e09718acbaad65725bd0534744de51cde8eb94b3a786e5afaf`.
Extracted `_dragongui.pyd` SHA-256:
`01687d1fc026acd28006a8c15f48b0b124100d6c59feaddc19fec4a9fdeb41f2`.

#### August 1, 2026 — Phase 4e matched full-fallback overhead gate

Added `run_gui_update_pipeline_runtime_ab.py`, a runtime-to-runtime A/B harness
that alternates control/candidate order by repetition, starts a fresh process per
sample, validates every report, captures machine metadata, and gates throughput,
command-drain p95, and rebuild-flush p95. Each pair must also have exact
retained-state equivalence and screenshot hashes. The case and matrix runners
now accept `--typed-target-diagnostics required|optional`, allowing the Phase 4b
control to omit diagnostics that did not exist yet while requiring them from the
Phase 4d candidate. Focused tests cover the overhead/regression calculations.

The matched gate compared the Phase 4b pre-typed runtime with the Phase 4d
typed-target runtime on the current workstation. It used `intrinsic-text:200`,
batched optimized updates, two-second warmups, 20-second measurements, 60 Hz,
five repetitions, alternated order, and screenshots. All ten runs validated:

| Metric | Phase 4b control median | Phase 4d candidate median | Candidate change |
|---|---:|---:|---:|
| Throughput | 16.9501 Hz | 17.5499 Hz | 3.54% improvement |
| Command-drain p95 | 92.9661 ms | 89.9343 ms | 3.26% improvement |
| Rebuild-flush p95 | 91.7768 ms | 88.7737 ms | 3.27% improvement |

The candidate passed all configured overhead gates. Equivalence was exact in
all pairs at
`847172609b9769bba6bce7ee07166459b0cd3a9525178b424e6a68adb2b20040`;
screenshots were exact at
`1ab6bfb6a0b3caeaae6e0ab218a8198518c550651338d3893db4729bbfb1bd38`.

Final Python regressions pass: **569 passed**. The case runner, matrix runner,
and new runtime A/B runner also pass bytecode compilation.

```text
artifacts/gui-update-pipeline-phase4e-full-fallback-ab-v1/summary.json
```

This closes Phase 4. Continue with centralized Phase 5 command coalescing keys,
barriers, and merge rules.

#### August 1, 2026 — Phase 5 command-semantics audit

The initial audit confirms that coalescing semantics currently live in two
independent implementations. Queue insertion in `native/src/commands.rs` uses
the private `command_coalesce_key(...)` and
`merge_replaced_command_flags(...)`. Drain-time batching in
`native/src/runtime.rs` separately tracks properties, scatter/plot resources,
themes, stylesheets, extension display lists, and icon themes, then performs an
additional adjacent line-append merge. This is the divergence Phase 5 is meant
to eliminate.

The first bounded refactor will define one shared semantic descriptor covering
coalescing key, barrier class, and merge policy. Before either consumer changes,
table-driven and randomized reference tests must preserve the current safe
behavior and explicitly cover:

- `SetProps` expansion/property ordering;
- structural replacement boundaries;
- snapshot and request/response observation boundaries;
- lossless append/event/callback commands;
- `coalesce=false` data updates; and
- sticky scatter/line `fit` and histogram `auto_fit` flags.

After those tests exist, queue insertion and drain-time coalescing can migrate
one command family at a time, with diagnostics distinguishing received,
applied, superseded, merged, and barrier-segment counts.

#### August 1, 2026 — Phase 5a shared key, merge, and strong barriers

The command families already coalesced at both queue insertion and drain time
now use one native contract:

- `Command::coalescing_key()` defines replaceable widget/property, theme,
  stylesheet, scatter, line-plot, histogram, scalar-bar, and actor identities;
- `Command::merge_replaced(...)` owns sticky scatter/line `fit` and histogram
  `auto_fit` merging; and
- `Command::coalescing_barrier()` identifies structural and observation
  boundaries.

The runtime reverse pass now keys its overlapping family map with the shared
`CommandCoalescingKey` rather than independently reconstructing IDs and
properties. Queue insertion and drain-time batching both clear their active
replacement segment at `ReplaceNode`/`ReplaceChildren` and at scatter/window
screenshot, debug-snapshot, and latency-probe requests.

This corrects an earlier semantic mismatch in which a later update could
supersede an earlier update across `DebugSnapshot`. Tests now require the
pre-snapshot update, snapshot request, and post-snapshot update to remain in
that order at both queue and drain levels. Structural tests likewise preserve
property and extension-display-list updates on both sides of replacement. The
barrier is intentionally global and conservative for this first increment;
target-scoped structural segmentation can be considered only with reference
coverage and a measured reason.

New contract tests also prove that `coalesce=false`, `SetProps`, append,
structural, and observation commands do not acquire replacement keys, and that
sticky flags survive replacement only within matching command families.
Complete native regressions pass: **926 passed / 13 ignored**.

Still outstanding for Phase 5: define `SetProps` packet-level handling,
migrate runtime-only display-list/icon-theme and adjacent-append policies,
classify callback/event boundaries, add randomized queue/packet/drain reference
tests, and add received/applied/superseded/merged/barrier diagnostics.

#### August 1, 2026 — Phase 5b packets and runtime-only families

Extension display lists and icon themes now participate in
`Command::coalescing_key()`. This removes their independent runtime tracking
set/boolean and lets queue insertion discard obsolete payloads before drain.
The queue snapshot adds replacement-family counters for
`extension_display_list` and `icon_theme`; existing aggregate replacement
counts retain their meaning.

Adjacent line-plot append semantics are now centralized in
`Command::try_merge_adjacent(...)`. It merges only consecutive appends with the
same widget, series, `max_points`, and payload format, consumes the later byte
buffer without cloning, and returns incompatible commands unchanged. The
runtime pass retains its distinct adjacent merge stage but no longer owns the
compatibility definition.

`PropUpdate::coalescing_key()` now shares the exact widget/property identity
used by individual `SetProp`. Drain-time `SetProps` processing uses it to:

- retain the latest duplicate inside a packet;
- remove an earlier packet update superseded by a later packet or `SetProp`;
- preserve the relative order of surviving updates;
- retain the surviving updates in one `SetProps` command; and
- stop replacement at structural or observation segment barriers.

The queue continues to preserve each property packet atomically; packet-aware
deduplication belongs at drain time because one queue node can contain multiple
independent keys. Tests cover cross-packet replacement and prove that a debug
snapshot preserves the packet before it and the property update after it.

Focused coverage also verifies compatible and incompatible adjacent append
merges and queue-level display-list/icon-theme replacement diagnostics.
Complete native regressions pass: **930 passed / 13 ignored**.

The Phase 2 randomized model still treats snapshots as ordinary lossless
commands and its generational candidate has no barrier segment identity. The
next increment must update the reference, stable-slot, linked-slot, and
generational models together before claiming randomized barrier equivalence.
Callback/event classification and received/applied/superseded/merged/barrier
diagnostics also remain open. Build and benchmark a fresh release runtime after
those items land so the performance gate measures the complete semantics.

#### August 1, 2026 — Phase 5c barrier-aware models and diagnostics

The Phase 2 randomized reference gate now models strong segments. `Snapshot`
and generic lossless/callback commands are barriers in the abstract stream:

- the reference queue limits replacement/removal to the suffix after the most
  recent barrier;
- stable-slot and linked-slot candidates clear their active latest-key maps at
  barriers;
- stable-slot compaction rebuilds the latest map while respecting barriers; and
- the generational candidate stores `(segment, key)` identities, preserving
  earlier payloads while preventing cross-segment invalidation.

The existing seeded differential passes **2,000 seeds × 200 commands = 400,000
generated commands** with random partial drains. Reference, stable-slot,
linked-slot, and generational outputs remain exact for every drain.

Production `DrainPythonTasks` is now `CommandCoalescingBarrier::Callback`.
Queue insertion and runtime reverse coalescing therefore preserve commands on
both sides of Python callback execution. Focused property/scatter tests and the
full scalar-bar regression now enforce that boundary. Native input/window
events travel through the event-loop path rather than the queued `Command`
stream, so no additional event command requires classification here.

Queue diagnostics add:

```text
barrier_segments.total
barrier_segments.by_class.structural
barrier_segments.by_class.observation
barrier_segments.by_class.callback
```

Runtime debug snapshots add cumulative `command_drain.coalescing` data:

- command counts: received, retained, superseded, and merged;
- property-update counts: received, retained, and superseded; and
- barrier-segment count.

Focused tests assert exact callback, packet-deduplication, and adjacent-append
accounting. The first full run found one stale scalar-bar expectation that
assumed callback crossing; updating it to preserve both segments closed the
suite. Complete native regressions pass: **930 passed / 13 ignored**.

The remaining Phase 5 instrumentation item is runtime superseded/merged
attribution by command family. After that, build a fresh release wheel/runtime
and run the current-machine ordered-barrier and representative throughput gates
with exact retained-state and screenshot validation.

#### August 1, 2026 - Phase 5d per-family attribution and release gate

Runtime command coalescing now attributes received, retained, superseded, and
merged commands to a fixed 12-family enum. The families cover property,
property packet, theme, stylesheet, scatter points, line plot, histogram,
scatter scalar bar, scatter actor, extension display list, icon theme, and
line-plot append. `Command::merge_replaced(...)` reports whether the shared
sticky-flag policy performed a merge, so attribution cannot drift from command
semantics. A fixed counter array is used in the drain hot path; family names are
materialized only when serializing a debug snapshot.

This representation was selected from measurement. The first release candidate
used a `BTreeMap`; its exact ordered-barrier A/B measured 11.50% command-drain
overhead and failed the 5% gate. That intermediate remains available at
`artifacts/gui-update-pipeline-phase5d-ordered-barrier-ab-v1/summary.json`.
The fixed-array candidate was rebuilt and remeasured rather than accepting the
instrumentation cost.

The authoritative release A/B compares the Phase 4d control runtime with the
fixed-array Phase 5d candidate on this workstation. Runs used fresh processes,
two-second warmups, 20-second measurements, a 60 Hz target, five alternating
repetitions, screenshots, and a 5% overhead limit.

| Workload / metric | Phase 4d control | Phase 5d candidate | Candidate change |
|---|---:|---:|---:|
| Ordered barrier throughput | 59.9993 Hz | 59.9998 Hz | 0.0009% improvement |
| Ordered barrier command-drain p95 | 3.6116 ms | 3.4999 ms | 3.09% improvement |
| Ordered barrier rebuild-flush p95 | 3.0532 ms | 2.9291 ms | 4.06% improvement |
| Mixed-state throughput | 59.8982 Hz | 59.8998 Hz | 0.0027% improvement |
| Mixed-state command-drain p95 | 15.7334 ms | 15.5791 ms | 0.98% improvement |
| Mixed-state rebuild-flush p95 | 1.0385 ms | 1.0152 ms | 2.24% improvement |

All ten samples per workload validated. Ordered-barrier samples matched
equivalence hash
`bb6c089af9966f1a3a06e2c40159db7f13369145d1b29a175b65118b43452c2f`
and screenshot hash
`0641bc4e82a0e948194f891c5e59e08aed7af8c14dac309f7d60d8efe733fde4`.
Mixed-state samples matched equivalence hash
`411e52f18d7d6f06ced7540e8f5e756e506ca58bdf0cb806b33bed5c79f4ae5d`
and screenshot hash
`917013df2102f401dda28de5fc45f7e3444b8514905fc66a191a14aeddc43ac9`.

The emitted diagnostics contain useful family attribution. One ordered-barrier
candidate sample reports `property_packet` commands as 2,640 received, 1,320
retained, and 1,320 superseded; nested property updates report 54,120 received,
27,720 retained, and 26,400 superseded. A focused native test covers sticky
scatter replacement attribution. Complete native regressions pass: **931
passed / 13 ignored**. Formatting and diff validation pass.

```text
artifacts/gui-update-pipeline-phase5d-ordered-barrier-ab-v2/summary.json
artifacts/gui-update-pipeline-phase5d-mixed-state-ab-v1/summary.json
artifacts/live-update-phase5d-command-semantics-wheel/dragongui-0.1.0-cp312-abi3-win_amd64.whl
artifacts/live-update-phase5d-command-semantics-runtime
```

The release wheel SHA-256 is
`0a03445e6c48f94f19c70de1bb1f600da68495e21f5af62c8b33a11cf4bc93e8`;
the extracted extension SHA-256 is
`527c468b3e4558960641718099610b5ced3f06f1f99f612703db5196d5846e4c`.
The wheel currently omits `WebView2Loader.dll`, so the benchmark runtime carries
the repository copy beside the extension; direct native import then passes.

Phase 5 is complete. Queue-level, packet-level, and drain-level coalescing use
the same keys, barrier classes, and merge policies, with barrier-aware
randomized equivalence, per-family diagnostics, and passed release A/B gates.

#### August 1, 2026 - Phase 6a unkeyed backlog safety

The Phase 6 orientation audit found that the existing keyed task path already
implements the proposed latest-state abstraction. `App.call_soon_threadsafe`
with a stable `coalesce_key` is thread-safe, retains only the newest pending
callback at its latest ordered position, composes with `update_batch()`, and is
already exercised by the public batched telemetry example. A separate
`latest_updates(...)` helper is therefore deferred unless usability evidence
shows that a second name materially improves safety.

The first safety increment instead detects the demonstrated failure mode:
unkeyed/lossless Python task backlog growth. `AppHandle` now counts pending and
high-water unkeyed tasks independently. It emits an actionable `RuntimeWarning`
at 256 pending callbacks, then only when the backlog doubles. The message
explains that unkeyed work is FIFO/lossless and directs replaceable snapshot
producers to a stable `coalesce_key`; genuinely lossless producers are told to
reduce rate or batch their events. This geometric policy bounds warning volume
without hiding continued growth.

Runtime Python snapshots add `unkeyed_tasks_pending`,
`unkeyed_task_queue_high_water`, `task_queue_growth_warnings`, and
`next_task_queue_warning_at`. Deterministic tests use a threshold of four and
verify warnings exactly at four and eight, correct cleanup after drain, and a
healthy 1,000-submission keyed control with no warnings, queue high-water one,
and only the final callback executed.

`dg.help.live_updates.threads()` now includes a table that distinguishes
latest-state keyed snapshots from unkeyed lossless events, explicitly says
intermediate snapshots may be skipped, and documents the warning and snapshot
fields. The batched telemetry example now schedules its completion callback as
an unkeyed lossless event so it cannot replace the final keyed state snapshot.
The complete Python regression suite passes: **571 passed**. Bytecode
compilation and diff validation pass.

#### August 1, 2026 - Phase 6b validated producer-safety gate

`examples/live_update_producer_safety_probe.py` now exercises the public live
runtime rather than only the internal task queue. Every burst submits many
replaceable snapshots under one stable key, follows them with one separately
marked unkeyed event, and waits for that event before continuing. Its
machine-readable report rejects stale Python/native final state, any lost or
reordered event, Python queue high-water above two, unkeyed high-water above
one, warning emissions, Python or native queue residue, worker errors, or a run
that performed no replacement.

The authoritative v2 run used 100 bursts of 100 snapshots:

| Result | Observed |
|---|---:|
| Keyed snapshot submissions | 10,000 |
| Applied snapshot callbacks | 100 |
| Coalesced snapshot callbacks | 9,900 |
| Lossless events submitted/retained | 100 / 100 |
| Final Python/native snapshot | 9,999 / 9,999 |
| Final Python/native event count | 100 / 100 |
| Python total/unkeyed queue high-water | 2 / 1 |
| Python/native final queue depth | 0 / 0 |
| Queue-growth warnings | 0 |
| Native physical-slot high-water | 3 |

The native queue ended with zero live or stale entries and recorded 100
callback barriers plus the final observation barrier. All 12 probe checks
passed.

```text
artifacts/live-update-phase6b-producer-safety-v2/report.json
artifacts/live-update-phase6b-producer-safety-runtime
```

The report SHA-256 is
`5f7b2f6bbc60f2080dcb6bac896430c6201ef67053ca06149a7efca371996517`.
The isolated runtime combines the current Python layer (`runtime.py` SHA-256
`3cb4c564f0a6da18051928eabec2da35b1188413a6811be9cb1993b927ca8895`)
with the validated Phase 5d native extension SHA-256
`527c468b3e4558960641718099610b5ced3f06f1f99f612703db5196d5846e4c`.

Native queue warnings were evaluated but not added. The bridge already reports
depth, logical and physical high-water, replacements by family, stale entries,
and barrier segments; replaceable state uses the bounded linked-slot queue. The
validated producer workload peaked at only three native slots. Emitting a
second warning without a demonstrated sustained lossless-native backlog would
duplicate the Python warning and provide less actionable origin information.
Revisit this only if Phase 7 scenario/soak evidence shows native-only growth.

The full Python regression remains **571 passed**; bytecode and diff checks
pass. Phase 6's target behavior is now demonstrated. The remaining work is the
broader live-plot/stress-example and threading-guidance audit and an
evidence-based decision on expensive-callback supersession warnings.

#### August 1, 2026 - Phase 6c producer call-site and guidance audit

Every `call_soon_threadsafe` site in active examples, legacy examples, and the
Python runtime was inventoried with AST-based classification. Stable keys were
added to replaceable histogram, scatter-camera, streaming-scatter,
scatter-performance, diagnostic/stat, and thread-monitor snapshot paths. The
built-in thread monitor now coalesces refreshes under
`dragongui.thread-monitor.latest`, preventing its own UI refresh from becoming
a backlog during a slow frame.

After the changes, active examples contain eight keyed replacement sites and
13 intentional unkeyed sites. The unkeyed sites are all lossless plot/log
appends, marked events, ordered benchmark actions, one-shot launch/completion
callbacks, or failure-monitor probes. Theme Forge and CATHODE already modeled
the desired split and remain unchanged: append/log streams are unkeyed while
replaceable gauge/label/LED snapshots use stable keys. Runtime unkeyed sites are
one-shot app/dialog lifecycle callbacks or the explicitly per-payload legacy
scatter callback handoff.

Threading guidance is now consistent in:

```text
README.md
docs/library-overview.md
docs/widgets-reference.md
docs/sphinx/live-updates.md
dg.help.live_updates.threads()
```

All five distinguish stable-key latest-state work from unkeyed FIFO/lossless
work. Public progress and prepared-scatter examples are keyed; plot appends are
unkeyed. `dg.help` points to the batching example, producer-safety gate, Theme
Forge, and CATHODE as executable reference patterns.

New regressions require all standalone guides to mention both `coalesce_key`
and lossless semantics and verify the built-in monitor's stable key. Existing
manual tests validate linked paths and Python call shapes. All changed examples
compile, the complete Python suite passes **573 tests**, and diff validation
passes.

Phase 6 is complete. The existing keyed API is the validated convenience
pattern, so a redundant wrapper is not justified. Expensive-callback
supersession warnings remain evidence-gated for Phase 7 rather than adding
noise without a demonstrated distinct backlog.

#### August 1, 2026 - Phase 7a coverage inventory and per-change matrix

The integrated scenario inventory maps the required release coverage to
existing executable tools:

| Requirement | Coverage |
|---|---|
| Dashboard low/medium/high | live-dashboard case and matrix runner |
| Labels-only/mixed state | update-pipeline case and matrix runner |
| Theme/CSS replacement, resize, viewports, decorations | Theme Forge autopilot plus CSS/layout probes |
| Scroll/focus/selection/tabs/overlays/tooltips/drag-drop | interaction and focused feature probes |
| Malformed/stale updates and queued shutdown | native/Python regressions, Theme Forge malformed cases, queue models |
| Lossless trace/log streams | CATHODE and Theme Forge split-stream workers |

Theme Forge already has bounded autopilot cycles, report output, and automatic
exit. CATHODE is the one orchestration gap: its lossless trace/log workload is
present, but it cannot yet run for a bounded interval, capture diagnostics, or
write a machine-readable result. Phase 7b should add that without changing the
workload semantics.

The required per-change labels-only and mixed-state tier ran against the
current Phase 6 isolated runtime on this workstation. Each sample used a fresh
process, two-second warmup, five-second measurement, a 60 Hz target, batched
updates, required typed-target diagnostics, and a screenshot.

| Scenario | Throughput | Dropped | Drain p95 | Apply p95 | Flush p95 |
|---|---:|---:|---:|---:|---:|
| Labels fixed, 200 | 59.8036 Hz | 1 | 4.4818 ms | 4.3607 ms | 0.1151 ms |
| Mixed state, 200 | 59.9975 Hz | 0 | 17.4457 ms | 15.8522 ms | 1.0147 ms |

Both completed tick 419 in Python and native retained state. All 30 labels and
36 mixed-state checks passed, including screenshots, exact final state, typed
targets, text-invalidation accounting, zero layout failures, zero targeted
fallback, zero Python/native residue, and zero stale entries. Python task queue
high-water remained one with zero warnings; native high-water was one/two.

Labels equivalence/screenshot SHA-256 values are
`44a47a1c2a21b1bc07022d7df62fd6bd725927faea43d742500b7e969410ca8b` and
`027b3bb623bc7f1d6f1aee1aea248993da4b9352954532c972054f147c96b79e`.
Mixed-state values are
`4fac31afc112d6606cff451a3993069c17af912197d926741431961be10cadaf` and
`d8ddc42a8c8bcec8f6d1f0cada3f66cc030159a1b1568d02aacb38606190590b`.

```text
artifacts/live-update-phase7a-per-change-matrix-v1/summary.json
```

The summary SHA-256 is
`b3271b78bebe9c7aa0e95d9b3e09bf15d4caf54637875db10634461108d41452`.
Absolute timings are current-machine evidence only.

#### August 1, 2026 - Phase 7b bounded CATHODE validation

The CATHODE stress demo now provides the missing bounded automation controls:
`--validate-seconds`, `--validation-timeout`, and `--report`. It waits for
runtime readiness, runs its existing split producer workload for the requested
interval, stops the producer, drains both queues through the final keyed
snapshot, emits a machine-readable JSON result, requests application exit, and
returns a pass/fail process status. Normal interactive behavior is unchanged.

Validation telemetry distinguishes produced ticks, unkeyed plot/trace stream
ticks, lossless log ticks, and keyed latest-state snapshot ticks. The 11 checks
cover exact stream and log retention, final keyed snapshot application,
identical final Python/native retained lane text, Python/native queue drainage,
zero queue-growth warnings, worker shutdown, and validation errors. Focused CI
contracts also cover the bounded CLI and retained-tree/final-value helpers.

The authoritative run used this workstation, the full 96-tile/480-row default
workload, ten live seconds, a 20-second readiness/drain timeout, and the Phase
6b isolated release runtime. All 11 checks passed:

- 36 produced ticks and all 36 lossless stream ticks retained;
- all five expected log ticks retained;
- 32 keyed snapshots executed and four safely coalesced;
- final tick 275 reached identical Python/native lane text `78%`;
- Python queue high-water six, unkeyed high-water five, native high-water 30;
- zero queue-growth warnings and zero final Python/native queue depth.

```text
artifacts/live-update-phase7b-cathode-validation-v1/report.json
```

The report SHA-256 is
`8073864791ed208c4135d9c111bad782a203cfb40fe41e948925626aab1a03ab`;
the exercised demo SHA-256 is
`aeea4a4e06b3abe6c9f33c0929aac2755466839b309652c95a8a5a9667218b37`.
The isolated runtime hashes remain
`3cb4c564f0a6da18051928eabec2da35b1188413a6811be9cb1993b927ca8895`
for `runtime.py` and
`527c468b3e4558960641718099610b5ced3f06f1f99f612703db5196d5846e4c`
for `_dragongui.pyd`. The focused benchmark-validation suite passes **10
tests**, the complete Python suite passes **575 tests**, and bytecode/diff
checks pass. The next Phase 7 increment is the combined bounded Theme
Forge/CATHODE autopilot tier, followed by the remaining interaction/decorations
matrix and extended soaks.

#### August 1, 2026 - Phase 7c paired stress autopilots

The paired Theme Forge/CATHODE tier is now bounded and green against the same
Phase 6b isolated runtime. Theme Forge was aligned with the other benchmark
entry points by honoring `DRAGONGUI_BENCH_PYTHON_PATH`. Its runner now also
creates missing report-parent directories and joins background workers during
shutdown. The initial two-cycle execution passed both integrity cycles but
exposed the missing-directory exception and failed to terminate, so it was
rejected. The repaired rerun used two cycles and the standard 0.32-second
settle, completed in 187 seconds, wrote its report, and exited zero.

Each Theme Forge cycle reported zero retained-state regressions, zero layout
issues, zero bad viewports/pages, intact cascade recovery after malformed CSS,
and no final stylesheet error. Phase 7b's paired CATHODE workload remains 11/11
green with exact lossless stream/log retention and zero final queue depth.

```text
artifacts/live-update-phase7c-combined-autopilots-v1/theme-forge-report.json
artifacts/live-update-phase7b-cathode-validation-v1/report.json
```

Theme Forge's report and exercised-script SHA-256 values are respectively
`cd08bf074841f21f73f578bd98cfa7d1a67542d67922f3fb424e4462249acbf2` and
`d4904da7537ed0d52d086ddab371a4316737589178b5817204285cfd758959c8`.
The CATHODE report remains
`8073864791ed208c4135d9c111bad782a203cfb40fe41e948925626aab1a03ab`.
The next increment is the remaining interaction/decorations scenario matrix,
followed by extended soaks and the final comparative report.

#### August 1, 2026 - Phase 7d interaction and decoration matrix

The visual-audit harness now honors `DRAGONGUI_BENCH_PYTHON_PATH` in both the
parent process and generated wrapper and creates wrappers in a dedicated clean
temporary directory. This repaired a rejected first attempt where an unrelated
`inspect.py` in the general Windows temp directory shadowed the standard
library. The v1 blocked artifact is diagnostic only; v2 is authoritative.

The v2 client-decoration target passed three fresh outcome-checked states with
native screenshots/snapshots: keyboard focus traversal, Enter/Space
maximize-restore with Win32 state assertions, and Alt+Space system-menu
open/close. Theme Forge then passed one complete live cycle with native OS
decorations, reporting zero retained-state regressions, layout issues, bad
viewports/pages, or final stylesheet error and clean malformed-CSS recovery.

The current-machine `state-controls:8` batch sample used a two-second warmup,
five-second measurement, 60 Hz target, typed diagnostics, screenshot, and a
concurrent 50 ms interaction probe. It passed all **62 checks** at 59.9872 Hz
with zero dropped ticks, exact tick-419 Python/native state, retained focus and
caret geometry, bounded text clips, correct follow/static scroll behavior,
zero layout failures, queue high-water two, and zero final queue depth. All 99
interaction samples completed with 0.8117 ms p95 latency. Focused audit,
headless Theme Forge, drag/drop payload, and decoration contracts pass **40
tests**; the complete Python suite passes **575 tests**.

```text
artifacts/live-update-phase7d-interaction-decoration-matrix-v2/report.json
artifacts/live-update-phase7d-interaction-decoration-matrix-v2/theme-forge-native-report.json
artifacts/live-update-phase7d-state-controls-v1/summary.json
```

The artifact SHA-256 values are respectively
`66e4a8af523518be7ce06388cb8ecd3de3b2ecaf63c2d0c7867d390ad752b4dd`,
`e0e93711927d6c6adef63f64e3bb14c142fa11237f96d2909d03c805c3e0ebe5`,
and `b680c92e0d844c81de55375313ba169db253806491a49e9aee88b59adbd11f19`.
Physical title dragging/resizing/minimize/close and pointer-driven drag/drop
remain platform/manual rows; their retained-state and dispatch contracts are
green. Automate practical residual gestures before marking the complete
scenario matrix done, then proceed to extended queue/memory soaks.

#### August 1, 2026 - Phase 7e native pointer drag/drop

The last practical application-interaction gap is now automated. The visual
audit action language adds `drag:#source->#target`, implemented as a real
Win32 left-button press, 12 interpolated mouse moves with the button held, and
release over the retained target. A paired `assert-text:#widget=value` action
checks callback output directly in the native retained tree. Stable source and
result IDs plus an outcome-checked manifest state convert the drag/drop probe
from manual to automated coverage.

Against the Phase 6b isolated runtime, the live audit dragged `Sensor A` into
the compatible asset lane, dispatched `on_drop`, and verified exact retained
text `Asset lane: Sensor A (asset)`. The target passed with native screenshot
and debug snapshot artifacts. Focused syntax, audit, and drag/drop contracts
pass **37 tests**. Disabled-source and incompatible-target rejection remain
API/dispatch contract-covered.

```text
artifacts/live-update-phase7e-pointer-interactions-v1/report.json
```

The report SHA-256 is
`88c5d3879bcb24260484432807b05499fa30872702c865472643ab2dc6ab83f6`.
Harness, probe, and manifest SHA-256 values are respectively
`56222c310a8df8efb415a801b591dcbc12ea0625831fa56ae0500757de3a2eeb`,
`25aace7dbeb544aebaad19b3c16447ef23c282ad44e41437d9af1c2015dac381`,
and `d455d872a4c809f40303cb4c408f0d998356174719bc1818095332dabc42d095`.
Physical titlebar move, edge/corner resize, minimize, and close remain explicit
platform-manual rows because they move, hide, or destroy the audit window.
Automated maximize/restore, focus traversal, system-menu, client/native
decoration, live retained state, and application drag/drop are green. The next
automated phase is the first extended high-load queue/memory soak. The complete
Python regression suite passes **576 tests**.

#### August 1, 2026 - Phase 7f high-load soak 1 of 3

The live-dashboard matrix runner now exposes the case runner's existing
`--update-mode` selection so extended artifacts explicitly record DragonGUI's
transport. The first authoritative soak used a fresh process, the Phase 6b
isolated runtime, batch mode, five-second warmup, and 600.0001 measured seconds
at the full high load (six 50k-point lines, 50k scatter points, 128x128 heatmap,
and 200 labels) on this workstation.

All **23 validation checks passed**. It completed 13,858 measured generations
at 23.0950 Hz, with 22,142 scheduled generations dropped/coalesced under
overload and exact final producer tick 36,299. Python coalesced 22,324 obsolete
tasks, held queue high-water at one (unkeyed zero), emitted zero warnings, and
drained to zero. Native high-water/physical peak were 40, with 1,674
replacements, zero stale entries, and zero final depth. Drain recovery was
610.94 ms. Command-drain p95 was 45.2838 ms and CPU averaged 96.09% of one
core, making command application the measured high-load limit in this sample.

RSS was 245.97 MiB at start, 264.84 MiB peak, and 263.05 MiB at end. Whole-run
growth was 17.08 MiB (1.708 MiB/minute), but the second half of 606 checkpoints
added only 1.00 MiB. This is consistent with a warm-allocation plateau, not yet
proof of leak freedom; two independent 10-minute repeats remain required.
Absolute throughput is current-machine evidence only. The 23.1 Hz result is
lower than desired and must be repeated and investigated before the final
comparative report.

```text
artifacts/live-update-phase7f-high-soak-1-v1/summary.json
artifacts/live-update-phase7f-high-soak-1-v1/raw/high-dragongui-1.json
```

Summary/raw SHA-256 values are
`6d869e985685c4ad43cfea1a523052fd7ef36d4449f0661dddfc81d700344e68` and
`1802e00899502f40cdf53a8024fc6bed99d6c21f8addcef82cf3a4b2e6111f9f`.
The runner SHA-256 is
`03609437220c529041c4a803f9de97a9764eae1933d4c9bd6109439b350b49e3`.
Next run soak 2 with the same configuration and compare second-half RSS slope,
throughput, command-drain distribution, and recovery time.

#### August 1, 2026 - Phase 7g high-load soak 2 of 3

The second independent soak used the identical fresh-process, isolated-runtime,
batch-mode, five-second-warmup, 600-second high-load configuration. All **23
checks passed**. It completed 18,591 measured generations at 30.9850 Hz,
dropped/coalesced 17,409 scheduled generations, and reached exact final tick
36,299. Python high-water remained one (unkeyed zero), warnings were zero,
17,565 obsolete tasks were coalesced, and final depth was zero. Native
high-water/physical peak again stayed 40, stale/final depth stayed zero, and
drain recovery was 394.13 ms.

The pair now demonstrates material runtime variance despite identical inputs.
Soak 2 throughput was 34.16% above soak 1 (30.9850 versus 23.0950 Hz), while
command-drain p95 was 24.41% lower (34.2311 versus 45.2838 ms). CPU remained
saturated at 100.95% of one core. Native replacements increased from 1,674 to
28,980 as the faster run admitted more overlapping work, without increasing
the 40-slot bound or leaving residue.

RSS started at 246.75 MiB and peaked/ended at 265.74 MiB: +18.99 MiB or 1.899
MiB/minute. Quarter growth was +15.21, +1.50, +0.05, and +2.23 MiB; second-half
growth was +2.84 MiB versus soak 1's +1.00 MiB. Both runs allocate primarily
in the first quarter and are nearly flat in the third, but the late soak-2 step
means bounded allocation is not yet accepted. Soak 3 must resolve both the RSS
shape and 23.1–31.0 Hz throughput spread.

```text
artifacts/live-update-phase7g-high-soak-2-v1/summary.json
artifacts/live-update-phase7g-high-soak-2-v1/raw/high-dragongui-1.json
```

Summary/raw SHA-256 values are
`0c8f72f0c03fa70c5a26b670d0556bfd218ff5bf30cb9b0bedea7084eab9e18c` and
`44f1ecd1731891c421ab1e72ec0096e2b2c6ccbb93640e45f8cd2d08eae177ad`.
Next run soak 3 identically and aggregate all three before the 60-minute gate.

#### August 1, 2026 - Phase 7h high-load soak 3 and aggregate

Soak 3 used the identical configuration and passed all **23 checks**, exact
final tick 36,299, and clean Python/native drains with zero warnings/stale
entries. It ran at 29.0316 Hz, command-drain p95 41.0441 ms, and recovery
438.74 ms. Queue high-water was again one/40. RSS rose from 245.04 to 267.31
MiB (+22.27 MiB, 2.227 MiB/minute); quarter deltas were +16.86, +0.57, +1.49,
and +3.35 MiB, leaving +4.84 MiB second-half growth.

The three fresh 10-minute samples pass **69/69 checks** with zero warnings,
stale entries, or final residue and identical maximum Python/native high-water
of one/40. Queue correctness and boundedness therefore pass. Aggregate results:

- throughput median 29.0316 Hz, range 23.0950–30.9850 Hz (34.16% spread);
- command-drain p95 median 41.0441 ms, range 34.2311–45.2838 ms;
- RSS-growth median 18.99 MiB, range 17.08–22.27 MiB;
- second-half RSS-growth median 2.84 MiB, range 1.00–4.84 MiB.

Memory/performance stability remains unresolved, so the 60-minute gate is
deferred. Retained counts stayed fixed at 617 widgets, six line series, 300,000
source points, 1,976 rendered points, and 29 text entries. In contrast, each
sample accumulated 5.38–7.41 million layout-text cache misses, 328–452 capacity
clears, and 18,483–24,192 glyph-atlas trims, scaling with completed generations.
This points to transient text-measurement/atlas allocation pressure rather than
obvious retained resource growth. Attribute and control that churn before the
hour-long release soak.

```text
artifacts/live-update-phase7h-high-soak-3-v1/summary.json
artifacts/live-update-phase7h-high-soak-3-v1/raw/high-dragongui-1.json
artifacts/live-update-phase7h-high-soak-aggregate-v1/summary.json
```

Soak-3 summary/raw hashes are
`07ce03901af4499579749a7e9e309f8eb68ae321e56580a18f0bd28f7b898a73` and
`0d7d5f85bb64c814665f577bc12f7e873bbc517e8485b2d40b35748f8007f0e8`.
The aggregate SHA-256 is
`ac31598a873e5c70ea68eb2fc284fda2698c1e68b371da933d90019dc20c1208`.
Next run a short controlled cache/atlas attribution gate, then decide whether
to fix or proceed to the 60-minute soak with the limitation documented.

## Historical July 31 baseline — original machine, reference only

The July 31 validated matrix is the initial reference point:

| Load | DragonGUI throughput | Dropped/coalesced measurement ticks | CPU, one core | Peak RSS | Command drain p95 |
|---|---:|---:|---:|---:|---:|
| Low | 58.7 Hz | 77 | 20.9% | 335 MiB | 1.66 ms |
| Medium | 58.3 Hz | 103 | 36.3% | 372 MiB | 6.24 ms |
| High | 38.1 Hz | 1,312 | 91.1% | 432 MiB | 17.56 ms |

The recommended high-load run:

- Retained six 50,000-point line series, a 50,000-point scatter, a 128 x 128
  heatmap, and 200 changing labels.
- Passed all 16 DragonGUI correctness checks.
- Reached the correct final Python and native tick.
- Held Python task queue high-water to one by using a keyed latest-frame task.
- Drained both Python and native queues before exit.
- Reported zero layout diagnostics.

The uncoalesced high-load run is a required negative control. It ended with 318
Python tasks and 226 native commands pending, stopped displaying at tick 3,581
instead of 3,899, and failed seven validations. A change that merely makes that
failure harder to observe is not an improvement.

---

## What Already Exists

The implementation should extend the current system rather than create a
parallel update engine:

- `App.call_soon_threadsafe(..., coalesce_key=...)` already provides O(1)
  latest-task replacement on the Python side.
- `CommandQueue::push` already replaces pending `SetProp` commands with the
  same `(widget_id, property)` key, but currently finds them with
  `VecDeque::retain`, which is O(queue length) per property enqueue.
- `coalesce_runtime_command_batch` performs a second latest-property collapse
  inside each fetched native batch.
- A command batch calls `begin_deferred_rebuilds()` and performs one
  `flush_deferred_rebuilds()` after applying its commands.
- Dirty requests are merged and ranked as GPU data, visual, text, layout, or
  full work.
- Text changes can already collect widget targets and use
  `rebuild_targeted_visuals(...)`.
- Partial-text, dirty-rebuild, command-drain, task-queue, resource, layout, and
  renderer metrics already appear in `debug_snapshot()`.

This means the most likely gains are:

1. Fewer Python-to-native calls and fewer command objects.
2. Constant-time native queue replacement without changing ordering.
3. More accurate dirty classification and fewer conservative fallbacks.
4. Work proportional to affected widgets rather than the whole retained tree.
5. Safer, more discoverable latest-state producer patterns.

---

## Non-Negotiable Correctness Rules

Every phase must preserve these rules:

1. **Lossless work stays lossless.** Clicks, log appends, line appends, commands,
   audit events, and other event streams may not be coalesced unless the API
   explicitly says so.
2. **Latest-state work reaches the newest state.** Replaceable telemetry may
   skip intermediate generations, but the final scheduled generation must be
   visible in both Python and native state.
3. **Ordering boundaries remain meaningful.** Structural replacement,
   stylesheet changes, event dispatch, screenshots, debug snapshots, and
   request/response commands may divide batches and must observe all earlier
   applicable updates.
4. **Python state and native state agree.** A faster native path cannot leave
   widget objects reporting different values from the retained tree.
5. **Geometry is unchanged unless the mutation requires geometry to change.**
   Text optimization must not create stale wrapping, intrinsic width, clipping,
   hitboxes, scroll extents, or accessibility bounds.
6. **No hidden backlog.** Successful runs end with zero Python tasks, zero
   native commands, no continuation latch, and the correct final tick.
7. **No silent fallback.** Fast-path attempts, completions, fallbacks, affected
   roots, and discarded obsolete work must be observable in diagnostics.
8. **Old public code remains valid.** Existing individual widget setters keep
   their behavior; batching is an additive optimization.

---

## Measurement Strategy

### Test layers

Each candidate change is tested at five levels:

| Layer | Purpose | Required evidence |
|---|---|---|
| Native unit/model | Ordering, coalescing, dirty merging, queue structure | Deterministic Rust tests, including randomized operation sequences |
| Python contract | Public API, widget state, exceptions, nesting, thread rules | Focused `tests/test_python_api.py` tests |
| Headless retained-state | Native tree, resource counts, geometry, layout diagnostics | `debug_snapshot()` assertions |
| Rendered integration | Actual pixels, clipping, focus, hover, scroll, hit testing | Window screenshot and visual-audit probes |
| Performance/soak | Throughput, p95/p99, CPU, RSS, queues, final state | Fresh-process JSON benchmark runs with validation required |

No performance result is considered valid if any correctness layer fails.

### A/B experiment mechanism

During development, each native optimization should retain an internal control
mode selected before startup, for example through a private environment value
or test-only constructor option:

```text
legacy       current behavior
candidate    new fast path with conservative fallback
verify       candidate result plus debug-only equivalence assertions where practical
```

These switches are temporary engineering controls, not public configuration.
They should be removed after the phase passes its gates, while its counters and
regression tests remain.

Run control and candidate in separate fresh processes. Alternate execution
order to reduce thermal and background-load bias. Record:

- Commit, dirty-worktree status, Python/Rust versions, OS, CPU, GPU, display
  scale, viewport, and present mode.
- Median, p95, p99, maximum, sample count, and raw sample paths.
- Scheduled, executed, coalesced, and dropped work.
- Native queue depth, queue high-water, drain yields, and recovery time.
- Dirty requests versus executions and fast-path fallback counts.
- RSS start, peak, end, and growth per minute.

### Repetition policy

- Development smoke: one 10-second measured run.
- Phase decision: at least five 60-second runs per affected load and mode.
- Final release candidate: three 10-minute high-load soaks plus one 60-minute
  DragonGUI-only soak.
- Report medians and the worst p95 across repetitions. Do not select the best
  run.

### General acceptance gates

A candidate advances only if:

- All unit and public API tests pass.
- All benchmark validations pass.
- Final Python and native values match the last scheduled generation.
- Layout diagnostics remain zero at all tested viewports.
- Python and native queues drain to zero within five seconds after production.
- No tested workload regresses throughput by more than 5% or p95 latency by
  more than 10%, unless the change fixes a correctness defect and the tradeoff
  is documented.
- Peak RSS does not increase by more than 10%.
- Repeated results show the improvement outside ordinary run-to-run noise.

---

## Phase 0 — Freeze the Baseline and Improve Instrumentation

### Questions to answer

- How much time is spent in Python setter logic, native boundary crossings,
  queue insertion/replacement, command application, and rebuild flushing?
- How many `SetProp` calls and distinct `(widget, property)` values are created
  per dashboard generation?
- Which dirty classes and fallback reasons are generated by the 200 labels?
- Does queue replacement itself become expensive as backlog grows?

### Instrumentation to add

Add cumulative and sampled metrics for:

- Python live setter calls and native sender calls.
- Batch count, properties submitted, unique properties, and duplicate
  properties removed.
- Native queue push count, replacement count, tombstone/compaction count if
  applicable, queue insertion p50/p95/p99, and queue high-water.
- `SetProp` application grouped by property and resulting dirty class.
- Dirty merge escalation, including the pair that caused escalation.
- Targeted rebuild requested roots, normalized roots, completed roots, fallback
  reason, and entries touched.
- Layout/style/text/primitive work skipped by an eligible fast path.

Keep detailed per-property maps bounded so diagnostics cannot become their own
memory leak. Use fixed-size top-N or aggregate known property classes.

### Benchmark additions

Extend the live dashboard with independently selectable workloads:

1. `labels-only`: 20, 200, 1,000 fixed-size labels.
2. `intrinsic-text`: labels whose text changes from short to long and wraps.
3. `mixed-state`: labels, badges, LEDs, progress bars, sliders, and plots.
4. `same-property-burst`: repeatedly replace one property before a drain.
5. `distinct-property-burst`: update many independent widget/property keys.
6. `ordered-barriers`: property changes separated by snapshot, structure, or
   stylesheet commands.

### Phase 0 deliverables

- Versioned raw baseline artifacts.
- A machine-readable comparison script that rejects invalid samples.
- A short baseline report identifying the top two measured costs.
- Tests proving new counters are correct and bounded.

### Exit gate

Do not implement a fast path until its target cost accounts for a meaningful
share of the affected scenario. As a working threshold, it should represent at
least 15% of command-drain time or remove at least 25% of native crossings.

---

## Phase 1 — Batch Widget Property Updates

### Hypothesis

The 200-label workload pays for hundreds of Python/native calls and command
allocations even though the native runtime already merges their rebuild work.
Transporting many small property changes in one native call should reduce
boundary, allocation, queue-lock, and dispatch overhead.

### API experiment

Prototype an additive context manager that preserves ordinary setter code:

```python
with app.update_batch():
    status.set_value("tick 42")
    for label, value in zip(labels, values):
        label.set_value(value)
```

Desired semantics:

- Python widget objects update immediately as they do today.
- Native sends are collected only for the current app handle and current
  thread/context.
- Exiting the outermost context submits one ordered property packet.
- Nested contexts merge into the outer context.
- An exception discards unsent native changes only if Python state can also be
  rolled back safely; otherwise it flushes the collected changes and re-raises.
  The simpler initial contract is **flush-and-re-raise**, clearly documented.
- Unsupported or structural operations flush the current property packet,
  execute in order, and then allow collection to resume.
- The public contract is batching, not database-style atomic rollback.
- Calling it before a widget is live remains harmless and consistent with
  existing setters.

Also expose a lower-level `AppHandle.update_props(...)` only if measurements
show a material advantage over the context manager. Avoid requiring users to
maintain a second source of Python widget state.

### Native representation

Add a packet command such as `SetProps { updates: Vec<PropUpdate> }` and one
Python binding call accepting a compact sequence. Validate and convert values
once at the boundary. On application:

- Preserve first-to-last ordering across distinct keys.
- Keep only the last value for repeated `(id, prop)` keys inside the packet.
- Apply all values while deferred rebuild collection is active.
- Flush one merged rebuild after the packet.
- Report per-update stale/no-op/error outcomes in aggregate diagnostics.

Do not serialize the batch through JSON unless measurement proves conversion
cost negligible. A typed PyO3 sequence or packed small-value representation is
preferred.

### Regression tests

- Empty, one-item, and thousands-of-items batches.
- Duplicate keys: last value wins.
- Multiple properties on one widget retain order.
- Nested batches and exception behavior.
- Mixed widget types and all `CommandValue` variants.
- Stale widget IDs do not abort valid siblings.
- A malformed value fails predictably without partially corrupting Python and
  native state.
- Structural and request/response commands form ordering barriers.
- Callbacks triggered after the batch observe final state, not an intermediate
  state.
- Focus, selection, scroll, hover, and active tabs survive unrelated batches.
- Batched and unbatched runs produce equivalent retained-tree values,
  geometry, resources, and screenshots.

### Performance experiment

Compare 20, 200, and 1,000 label updates per tick in three modes:

1. Existing individual setters.
2. Context-managed batch using the same setters.
3. Internal direct packet control to reveal remaining Python collection cost.

### Phase 1 target

- At least 90% fewer Python/native calls for a 200-widget batch.
- At least 25% lower command-application p95 in `labels-only-200`.
- High-load live-dashboard throughput improves from 38.1 Hz to at least 44 Hz,
  or profiling proves another stage has become dominant.
- No low/medium-load throughput regression beyond the general gate.

---

## Phase 2 — Constant-Time Native Queue Coalescing

### Hypothesis

`CommandQueue::push` currently uses `VecDeque::retain` for every `SetProp`, so
producer cost grows with queue length. The behavior is correct but can become
quadratic during a backlog or a large distinct-key burst.

### Candidate structures to benchmark

Implement model-only prototypes before changing the runtime:

1. **Generational ordered queue:** append sequence IDs, keep the latest ID per
   coalescing key, skip stale entries on drain, and compact when stale density
   or total length crosses a bound.
2. **Stable-slot queue:** order IDs in a deque, store commands in stable slots,
   and replace a key's slot while moving its ordering token to the tail.
3. **Segmented queue:** coalesce replaceable state within segments separated by
   strict-order barriers.

Select based on measured enqueue cost, memory, implementation risk, and exact
ordering—not intuition alone.

### Required queue model

Create a small reference interpreter with the current semantics. Generate
random streams containing:

- `SetProp` for repeated and distinct keys.
- Coalesced and non-coalesced plot data.
- Append/event commands that must remain lossless.
- Structural replacements.
- Stylesheet/theme mutations.
- Debug snapshots and request/response commands. First preserve the current
  tested cross-command coalescing behavior; evaluate hard-barrier semantics as
  a separate correctness change.
- Partial drains at arbitrary boundaries.

Run thousands of seeded sequences through both reference and candidate. Assert
identical observable drained command order and final state. Preserve failing
seeds as permanent tests.

### Memory controls

If the selected design permits stale entries:

- Compact at a deterministic ratio and absolute threshold.
- Bound stale payload memory; large packed plot buffers must be released when
  superseded, not held until a distant drain.
- Expose live entries, stale entries, compactions, bytes superseded, and peak
  physical queue length.

### Phase 2 target

- Queue insertion p95 remains approximately flat from 32 to 100,000 pending
  logical updates.
- Building a 100,000-key distinct backlog is at least 10x faster than the legacy
  queue.
- Replacing a hot key while 100,000 unrelated commands are pending is at least
  10x faster than the legacy queue.
- A pure same-key stream does not regress by more than the general performance
  gate; it is not expected to improve because the legacy queue stays one item
  deep in that case.
- Physical queue memory remains bounded to an explicitly tested multiple of
  live logical work.
- Randomized equivalence and all existing command queue tests pass.

---

## Phase 3 — Safe Text-Only Invalidation

### Hypothesis

Some text changes can update glyph content without recalculating style or
layout, while others change intrinsic size, wrapping, clipping, scroll ranges,
or control chrome. Treating both classes identically either wastes work or
creates stale geometry.

### First establish the current truth

Before changing dirty classes, build a property-by-widget mutation matrix. For
each supported textual property (`text`, `value`, `label`, `badge`, titles,
axis labels, dropdown values, and related fields), record whether changing it
can affect:

- Intrinsic width or height.
- Line wrapping and number of shaped lines.
- Parent flex/grid distribution.
- Scroll extent.
- Hit-test geometry.
- Overlay size or placement.
- Style selectors that reference the property or state.
- Plot chrome or resource layout.

### Conservative eligibility predicate

Introduce a fast path only when the runtime can prove geometry stability. A
first version may require all of:

- The widget has definite used width and height.
- The relevant overflow/wrapping behavior cannot resize ancestors.
- The property is not referenced by selectors.
- The widget type has no text-dependent internal geometry.
- No font, scale, viewport, or style generation changed.

Everything else uses the existing safe path. Track why each candidate was
eligible or rejected.

If necessary, split dirty meaning into clearer internal categories such as:

- `TextPaint`: reshape/rebuild targeted text and dependent primitives only.
- `TextMetrics`: text may affect layout.
- Existing `Visual`, `Layout`, and `Full` categories.

Names are implementation details; the important requirement is that a
paint-only path can never suppress required geometry work.

### Geometry equivalence tests

For every widget/property pair, compare optimized and forced-full runs across:

- Short -> long -> short strings.
- Empty strings, Unicode, emoji, combining marks, RTL, and multiple lines.
- Fixed, minimum, maximum, percentage, flex, and intrinsic sizes.
- Wrapped and non-wrapped text.
- Nested panels, scroll areas, tabs, overlays, and clipped containers.
- 100%, 125%, 150%, and 200% scale where available.
- Narrow, normal, and wide viewports.

Compare retained tree values, every affected layout rectangle, clip rectangle,
scroll maximum, text entry bounds, hit-test results, and screenshots. Exact
geometry should match within a documented floating-point tolerance; pixels may
use a small anti-aliasing tolerance but cannot hide shifted or clipped text.

### Phase 3 target

- Eligible fixed-geometry label updates perform zero style-cascade passes and
  zero layout passes.
- No geometry or visual equivalence failures in the mutation matrix.
- Targeted text fallback rate is below 5% for `labels-only-200` and is correctly
  high for intentionally intrinsic/wrapping cases.
- At least 20% lower rebuild-flush p95 for the fixed-label workload.

---

## Phase 4 — More Precise Retained-Tree Dirty Regions

### Hypothesis

Targeted text rebuilding exists, but visual changes and mixed dirty batches can
still escalate to broad primitive or visual rebuilds. Tracking exactly which
retained roots and resource classes changed should make work proportional to
the mutation set.

### Implementation direction

- Replace a single merged target set with typed dirty targets where needed:
  text, primitive paint, layout subtree, plot chrome, overlay, and GPU data.
- Normalize targets by dropping descendants when an ancestor is already dirty.
- Keep structure generation IDs so stale targets reliably force a safe
  fallback.
- Preserve separate overlay and table-text handling.
- Prevent an unrelated safe visual target from unnecessarily promoting an
  otherwise targeted text batch to a global rebuild.
- Extend existing `rebuild_targeted_primitives(...)` and
  `rebuild_targeted_visuals(...)` instead of creating an independent renderer.
- Add explicit fallbacks for selectors, transitions, ancestor-dependent
  backgrounds, stacking/clip changes, and any primitive whose output depends
  on global traversal order.

### Differential validation

Add a debug-only mode that performs the targeted rebuild, captures its retained
output signature, then forces a full rebuild and compares:

- Primitive counts, ordering, batches, and bounds.
- Text owner IDs, entries, and bounds.
- Plot/scatter/table resource revisions.
- Overlay ownership and clip rectangles.
- Layout and hit-test state.

Use this only in focused tests/probes, not normal releases.

### Locality tests

- Mutating one of 1,000 sibling labels does not revise unrelated siblings.
- Mutating 200 disjoint labels touches 200 roots or fewer after normalization.
- Mutating an ancestor plus descendants touches only the ancestor root.
- Removed/replaced nodes trigger safe fallback and never access stale targets.
- Mixed text plus progress-bar visual updates remain targeted when eligible.
- Style/class/structure changes still force the required broader work.
- Hover, focus, selection, tooltips, and scrollbars remain visually correct.

### Phase 4 target

- Entries/primitives touched scale with dirty roots rather than total widget
  count in the 1-of-1,000 and 20-of-1,000 probes.
- At least 95% targeted completion in the fixed mixed-state dashboard.
- Zero differential mismatches over 10,000 randomized mutation batches.
- No more than 5% overhead for workloads that correctly require full rebuilds.

---

## Phase 5 — Native Command Coalescing by Widget and Property

This priority overlaps Phase 2 but also applies after commands are fetched or
arrive inside a property packet.

### Required semantics

- Latest replaceable `(widget, property)` state wins within a safe segment.
- A `fit=True` request remains sticky when plot replacements are collapsed.
- Appends, events, callbacks, and request/response commands remain ordered and
  lossless.
- Structural replacement is a barrier for child/descendant property updates.
- A snapshot observes all earlier updates and none from later segments.
- Coalescing never crosses a command whose behavior can depend on the
  intermediate value.

### Implementation work

- Define a single `Command::coalescing_key()` and barrier classification rather
  than maintaining separate ad hoc matching logic in queue push and runtime
  batch coalescing.
- Define merge behavior for commands with sticky flags or partial payloads.
- Reuse the reference-model and randomized tests from Phase 2.
- Add diagnostics by command family: received, applied, superseded, merged,
  and barrier segments.

### Exit gate

Queue-level, packet-level, and drain-level coalescing must use the same tested
semantic definitions. Redundant passes may remain only if profiling proves
they catch distinct work at acceptable cost.

---

## Phase 6 — Make High-Frequency Producer Patterns Safe by Default

### Goal

Developers should not need to discover the stale-backlog failure through a
production dashboard. The API and documentation should make state snapshots
and lossless events visibly different.

### Proposed work

1. Add a small public helper or channel abstraction only after the lower-level
   batching results are known. Candidate shape:

   ```python
   telemetry = app.latest_updates("telemetry")
   telemetry.submit(apply_snapshot)
   ```

   It would be convenience over keyed `call_soon_threadsafe`, not a second
   scheduler.

2. Keep the lossless path explicit:

   ```python
   app.call_soon_threadsafe(record_event)  # FIFO, never replaced
   ```

3. Add bounded diagnostics warnings when:

   - An unkeyed producer repeatedly grows the task queue.
   - Native command queue high-water grows over several frames.
   - The same replaceable property is repeatedly superseded after expensive
     Python callbacks rather than before them.

4. Update `dg.help`, threading guidance, live plotting examples, and stress
   demos with a table explaining snapshot versus event semantics.

5. Provide a validated telemetry example using batching inside a keyed task.

### Documentation regression tests

- Every named API exists and has the documented signature.
- Every code example imports and builds headlessly.
- `dg.help` explains that coalescing can skip intermediate state and must not be
  used for lossless events.
- The example's debug snapshot proves bounded queues and correct final state.

### Phase 6 target

- The documented high-frequency example holds Python queue high-water at two
  or less under forced overload.
- It ends at the correct final state and retains every separately marked
  lossless event.
- Queue-growth warnings are rate-limited, actionable, and absent in a healthy
  run.

---

## Phase 7 — Integrated Validation and Rollout

### Required scenario matrix

Run at minimum:

- Live dashboard: low, medium, high, and labels-only variants.
- Theme Forge live workload and autopilot.
- CATHODE stress workload, including lossless trace/log streams.
- CSS/theme replacement during live updates.
- Window resize and scale changes during live updates.
- Scroll, focus, text selection, drag/drop, tabs, overlays, and tooltips while
  batches are active.
- Headless/native-decoration and client-decoration modes.
- Malformed/stale update inputs and app shutdown with queued work.

### CI tiers

**Per change:**

- Native queue, dirty-classification, and targeted-rebuild unit tests.
- Python API contract tests.
- Short validated labels-only and mixed-state smoke tests.
- Existing layout and visual-audit tests.

**Nightly or manual performance job:**

- Five-repetition 60-second A/B matrix.
- Theme Forge and CATHODE autopilots.
- 10-minute high-load memory/queue soak.
- Persist raw JSON and trend it against the last accepted baseline.

**Release gate:**

- Full test suites on supported Python/platform targets.
- Three 10-minute high-load soaks and one 60-minute soak.
- Fresh HTML performance report.
- No unresolved validation failure, queue residue, layout diagnostic, or
  unexplained regression beyond the general thresholds.

### Rollout order

1. Land instrumentation and tests without behavior changes.
2. Land internal batch transport while existing setters remain the default.
3. Expose the batch context manager after equivalence and performance gates.
4. Replace the native queue structure after model testing.
5. Enable text/targeted fast paths conservatively with fallback counters.
6. Expand eligibility only from evidence gathered by differential tests.
7. Add producer convenience APIs and documentation last, once their optimal
   implementation path is stable.

Each phase gets its own benchmark artifact and plan progress entry. Avoid one
large patch combining API, queue, dirty classification, and renderer changes;
that would make both regressions and performance attribution difficult.

---

## Implementation Checklist

### Phase 0 — Evidence and instrumentation

- [x] Add Python/native crossing counters.
- [x] Add batch counters with the Phase 1 packet prototype.
- [x] Add native queue insertion/replacement timing and bounded diagnostics.
- [x] Add dirty-class and fallback-reason metrics.
- [x] Add labels-only, intrinsic-text, mixed-state, and burst cases.
- [x] Add the ordered-barrier benchmark case.
- [x] Capture fresh five-run baseline artifacts on the current computer before
  Phase 4 performance changes; historical-machine absolutes are reference only.
- [x] Identify the first measured implementation target.

### Phase 1 — Batch transport/API

- [x] Prototype nested `app.update_batch()` collection.
- [x] Define exception and ordering-barrier behavior.
- [x] Add typed native `SetProps` packet.
- [x] Add native, Python, retained-state, and rendered equivalence tests.
- [x] Run the first validated development A/B matrix.
- [x] Pass the five-repetition sustained development/equivalence gate.
- [x] Pass a validated high-load live-dashboard development A/B run.
- [x] Document the additive public API and add a focused example.
- [ ] Run the five-by-60-second repeated/alternated release soak and apply final
  Phase 1 release gates.

### Phase 2 — Queue data structure

- [x] Build reference queue model and seeded generator.
- [x] Prototype and equivalence-test the stable-slot candidate.
- [x] Prototype and equivalence-test the linked-slot candidate.
- [x] Prototype and equivalence-test the generational candidate.
- [x] Run the first reference/tombstone/linked model microbenchmark.
- [x] Benchmark at least three candidate structures in optimized code.
- [x] Implement the bounded-memory linked-slot design.
- [x] Preserve discovered model counterexamples as permanent regression tests.
- [x] Run development burst, barrier, live-dashboard, and logical-memory gates.
- [x] Run the repeated release soak and large-payload process-memory gate.

### Phase 3 — Text invalidation

- [x] Build widget/property geometry-dependency matrix.
- [x] Add conservative fast-path eligibility and reasons.
- [x] Add geometry, clip, scroll, hit-test, and screenshot equivalence tests.
- [x] Run fixed versus intrinsic text A/B cases.
- [x] Add a forced-safe layout control and fixed-label differential case.
- [x] Add the state-control differential and focused-caret/scroll validation.
- [x] Add the plot-chrome differential and line-plot GPU rebuild proof.
- [x] Add HTML-fallback, embedded-WebView, and icon differentials.
- [x] Close the repeated-release targeted-fallback gate (0% fallback).
- [x] Close the repeated-release rebuild-flush performance gate (99.05% median
  reduction measured; 20% required).
- [x] Make the post-differential eligibility decision; retain explicit
  target-local routes rather than broadening the general predicate.

### Phase 4/5 — Dirty locality and command semantics

- [x] Add retained-tree structure generations and stale-target fallback diagnostics.
- [x] Consolidate deferred work into typed retained-visual, primitive-paint,
  table-text, and overlay target classes with preserved fallback semantics.
- [x] Add core retained primitive/text/line-plot targeted-versus-full
  differential verification mode.
- [x] Establish the shared command key, sticky-merge, and strong
  structural/observation barrier contract for overlapping queue/drain families.
- [x] Centralize `SetProps`, display-list, icon-theme, and adjacent-append
  semantics across packet-level and runtime-only processing.
- [x] Add callback barrier classification and pass the barrier-aware randomized
  reference gate over 400,000 generated commands with partial drains.
- [x] Add queue barrier-class and aggregate runtime command/property coalescing
  diagnostics.
- [x] Add runtime superseded/merged attribution by command family and run the
  fresh current-machine release correctness/performance gate.
- [x] Add and pass deterministic randomized 1-of-1,000 and 20-of-1,000 locality
  scaling controls.
- [x] Reach 10,000 randomized targeted-versus-full differential batches (10,080
  matches, zero mismatches).
- [x] Confirm full-fallback workloads do not materially regress (matched
  five-repetition runtime A/B improved throughput 3.54%, command-drain p95
  3.26%, and rebuild-flush p95 3.27%).

### Phase 6/7 — Developer safety and release

- [x] Validate the existing keyed latest-state convenience pattern; do not add
  a redundant wrapper without usability evidence.
- [x] Add bounded Python task queue-growth diagnostics with rate-limited,
  actionable warnings and snapshot counters.
- [x] Update `dg.help`, examples, and threading guidance with tested
  latest-state versus FIFO/lossless semantics.
- [x] Generate the refreshed current-machine three-library engineering report
  with 99/99 validated checks and explicit July 31 context deltas.
- [ ] Run full scenario matrix and extended soaks.
- [ ] Generate final comparative report.
- [ ] Remove temporary legacy/candidate switches.

---

## Definition of Complete

This plan is complete when:

- High-load throughput materially exceeds the 38.1 Hz baseline, with an
  initial goal of at least 50 Hz and an aspirational goal of sustaining 60 Hz
  on the baseline machine.
- Low and medium loads remain effectively at 60 Hz.
- Python and native queues remain bounded and drain to zero.
- Latest-state producers reach the correct final state and lossless producers
  retain every event.
- Command application and rebuild flush no longer dominate a 16.67 ms frame
  budget at the accepted high load, or the next dominant cost is clearly
  measured and documented.
- Geometry, screenshots, retained state, interactions, CSS behavior, and
  resource contents pass their equivalence and regression suites.
- The batch and producer APIs are documented in `dg.help` with tested examples.
- A refreshed HTML report contains raw artifact links, correctness results,
  comparison plots, and remaining limitations.

The goal is not merely a higher frame-rate number. The completed system must be
faster because it performs less obsolete and unrelated work, while remaining
observable, bounded, and semantically correct.

## August 1 comparative-report checkpoint

The requested refreshed live-dashboard comparison is complete at
`plans/gui-live-dashboard-performance-report-2026-08-01.html`. It preserves
the original July 31 HTML and adds a dated current-machine report.

Method:

- nine fresh processes: low/medium/high × DragonGUI/Dear PyGui/PyQtGraph;
- one measured repetition per configuration;
- five-second warmup and 60-second measurement;
- rotating framework order;
- DragonGUI keyed latest-frame coalescing plus batch property transport;
- portable project-local CPython 3.12.10 and isolated comparison packages;
- fresh current-source MSVC release wheel, SHA-256
  `7df7b3fd09f8f38c331ca5f29daf236f49d0b9cfa54766c2f5b59558e73bcff7`.

The matrix passed 99/99 validation checks with zero failures. Exact throughput
was:

| Load | DragonGUI | Dear PyGui | PyQtGraph |
|---|---:|---:|---:|
| Low | 60.000 Hz | 60.000 Hz | 59.983 Hz |
| Medium | 59.966 Hz | 59.999 Hz | 23.384 Hz |
| High | 28.049 Hz | 23.964 Hz | 3.017 Hz |

DragonGUI therefore leads the fresh high-load current-machine comparison by
17.0% over Dear PyGui and 829.8% over PyQtGraph. High-load peak RSS was 251.3
MiB for DragonGUI, 787.7 MiB for Dear PyGui, and 123.6 MiB for PyQtGraph.
DragonGUI high-load submit p95/frame-work p95 were 11.253/0.596 ms.

The dated report also shows the published July 31 table beside the fresh
results. DragonGUI low and medium throughput rose from 58.7/58.3 Hz to
60.000/59.966 Hz. High throughput changed from 38.1 to 28.049 Hz, while high
submit p95 improved from 16.27 to 11.253 ms, frame p95 from 0.89 to 0.596 ms,
and peak RSS from 432 to 251.3 MiB. These are not a controlled code-only A/B:
the old report came from the prior slower computer and individual property
transport. The report labels the deltas accordingly.

A same-machine legacy-wheel control was attempted with the preserved July 3
wheel. It did not meet the current harness contract in a usable time: three
one-second smoke cases exceeded a two-minute timeout and produced no validated
summary. Its two remaining Python processes were stopped and the partial run
was excluded.

Artifacts:

```text
artifacts/gui-live-dashboard-comparison-2026-08-01-v1/summary.json
artifacts/gui-live-dashboard-comparison-2026-08-01-v1/raw/*.json
plans/gui-live-dashboard-performance-report-2026-08-01.html
```

Summary/report SHA-256 values are
`555c19ff3ac8b44a03cb1322db04da601f8496e7c3b2e451c247f81625d3062a`
and
`debc0da62a18ac5560fce2c81d2ac3ff17c7ea81b761eb1ec5042c1b85d6e7cc`.
Both changed benchmark scripts passed `python -m py_compile`; the focused
benchmark-validation suite passed 10/10 tests in 0.15 seconds; and the touched
files passed `git diff --check` (line-ending notices only).

This is a validated engineering comparison, not the final Phase 7 release
report. It does not satisfy the definition-of-complete high-load target:
28.049 Hz does not exceed the historical 38.1 Hz target, even though that
target came from another machine. The authoritative next sequence remains:

1. Attribute text-measurement-cache clears and glyph-atlas trims.
2. Run a short controlled verification of the selected churn change.
3. Run the 60-minute high-load release soak.
4. Generate the final release report and remove temporary legacy/candidate
   switches.

## August 3 memory-metric correction and snapshot-free soak

The August 3 framework rerun initially reported 393–394 MiB high-load
DragonGUI peak RSS. Investigation found two benchmark effects rather than a
live retained-resource leak:

1. `gui_live_dashboard_case.py` prepends the repository Python package unless
   `DRAGONGUI_BENCH_PYTHON_PATH` is set. The first matrix therefore combined
   the current Python layer with the older checked-out native extension. A
   corrected run explicitly used the fresh release runtime; the extension
   SHA-256 was
   `8f200d628b44529e116d29e700f28b8cbac8a210d05a9c9bd9a89b844630736e`.
2. The legacy RSS series included full `debug_snapshot()` calls used for
   readiness and final validation. A controlled high-load probe measured a
   68.5 MiB RSS increase around the final full snapshot. The equivalent
   lightweight ordering probe added effectively zero memory. Full-snapshot
   steady RSS was 324.3 MiB; the otherwise equivalent lightweight-probe run
   held 298.9 MiB.

The benchmark now records measurement-window memory separately from warmup
and post-measurement validation. It reports current working-set RSS and Windows
private commit, preserves the legacy all-sample fields, records checkpoint
phase, and exposes validation-snapshot RSS delta independently. A deterministic
unit test proves warmup and post-validation samples cannot enter the new
measurement fields.

Five fresh high-load processes then ran with a five-second warmup, 60-second
measurement, the verified release wheel, batch property transport, keyed
latest-frame coalescing, and lightweight live probes. All 115 correctness
checks passed; Python queue high-water was one and both queues ended at zero.

| Run | Throughput | RSS peak | RSS delta | Private delta |
|---|---:|---:|---:|---:|
| 1 | 45.03 Hz | 297.9 MiB | +1.2 MiB | +0.68 MiB |
| 2 | 44.90 Hz | 298.4 MiB | +2.0 MiB | +1.20 MiB |
| 3 | 44.83 Hz | 298.2 MiB | +2.3 MiB | +1.50 MiB |
| 4 | 44.77 Hz | 298.2 MiB | +1.4 MiB | +0.72 MiB |
| 5 | 45.15 Hz | 298.0 MiB | +1.6 MiB | +0.72 MiB |

Median throughput was 44.90 Hz, median measurement RSS peak was 298.2 MiB,
median RSS growth was 1.57 MiB, and median private-commit growth was 0.72 MiB.
The five one-minute RSS regression slopes ranged from +0.52 to +2.08 MiB/min,
which required a longer control rather than an immediate leak conclusion.

One fresh five-minute high-load process completed at 44.693 Hz with 23/23
checks passing and bounded/drained queues. Across 300 measurement samples, RSS
changed from 296.18 to 300.42 MiB (+4.25 MiB) and private commit changed from
995.81 to 999.90 MiB (+4.09 MiB). Whole-run regression slopes were +0.46
MiB/min RSS and +0.43 MiB/min private commit; second-half slopes were +0.45 and
+0.58 MiB/min respectively. Final retained resource counts remained fixed.

The final snapshot showed the bounded layout-text cache at 8,450 entries with
a 16,384-entry limit after 334 capacity clears. The glyph atlas had trimmed
17,234 times and retained only 29 text entries. Combined with zero queue growth
and fixed renderer resource counts, the remaining low drift is consistent with
bounded text-cache/glyph-atlas allocator churn, not accumulating GUI objects.
If the measured +0.46 MiB/min slope persisted it would still amount to roughly
28 MiB/hour, so the existing 60-minute Phase 7 release soak remains necessary
before release closure.

Artifacts:

```text
artifacts/gui-live-dashboard-comparison-2026-08-03-v1/full-corrected/summary.json
artifacts/gui-live-dashboard-memory-soak-2026-08-03-v1/summary.json
artifacts/gui-live-dashboard-memory-soak-2026-08-03-5min-v1/summary.json
plans/gui-live-dashboard-performance-report-2026-08-03.html
```

## August 3 telemetry-indicator optimization checkpoint

The 16-case telemetry-indicator decomposition identified two concrete
library-level costs and both have now been corrected:

- A fixed-width wrapped status label with an intrinsic height forced a full
  window layout on every short `tick N` replacement. Live label replacement
  now compares the old and new shaped heights at the resolved width. Equal
  heights use retained text invalidation; a changed line count still takes the
  safe layout path.
- Large retained-text batches previously performed many small subtree patches,
  each scanning the retained entry list. Batches of 64 or more roots now cross
  over to one linear full-text rebuild, while primitive work remains targeted.
- `LED.set_state()`, `set_on()`, and `set_color()` now suppress unchanged
  effective state and color properties. CSS-visible state changes are still
  submitted even when their resolved color matches.

The exact same 24/72/160/320 × labels/progress/LEDs/combined matrix was rerun
with fresh processes, a two-second warmup, eight-second measurement, 30 Hz
target, batched transport, final retained-state checks, clean-layout checks,
and queue-drain checks. All 16 candidate cases passed. At 320 channels:

| Mode | CPU before → after | Apply p95 before → after | Drain p95 before → after | Text misses before → after |
|---|---:|---:|---:|---:|
| Labels | 64.8% → 17.0% | 20.15 → 1.78 ms | 20.93 → 3.61 ms | 96,350 → 623 |
| Progress | 33.8% → 17.6% | 12.18 → 1.29 ms | 12.39 → 3.49 ms | 348 → 303 |
| LEDs | 37.9% → 12.9% | 15.23 → 0.66 ms | 15.86 → 1.23 ms | 348 → 303 |
| Combined | 96.7% → 32.0% | 35.55 → 7.98 ms | 36.04 → 9.80 ms | 84,394 → 623 |

The previously overloaded 320 combined case improved from 26.63 Hz with 27
missed/coalesced measured generations to 30.00 Hz with zero drops. The 320 LED
case now sends approximately 36.9 native properties per completed tick from
641 offered API properties; the combined case sends approximately 676.9 from
1,281 offered properties.

Measurement-window RSS was higher in this candidate matrix (roughly 294–311
MiB at 320 channels versus 250–274 MiB in the earlier matrix). None of these
changes intentionally retains per-tick data, and the short runs do not
establish a growth leak, but the absolute regression must be kept visible and
rechecked with the existing snapshot-free memory protocol before release.

Validation:

- native library: **934 passed, 13 intentionally ignored**;
- focused wrapped-label safety test covers equal-height fast path and
  line-count-changing fallback;
- both 320-widget probes and all 16 full-matrix samples passed final state,
  packet, layout, and queue checks;
- benchmark scripts pass CPython 3.12 byte compilation.

Artifacts:

```text
artifacts/gui-telemetry-indicator-decomposition-2026-08-03/summary.json
artifacts/gui-telemetry-indicator-decomposition-2026-08-03-after/summary.json
plans/gui-telemetry-indicator-optimization-comparison-2026-08-03.html
```

The next priority is no longer indicator command application. It is to rerun
the cross-framework telemetry workload with this candidate, then isolate the
candidate's higher absolute RSS using repeated snapshot-free samples before
considering the memory portion closed.

## August 3 telemetry-indicator memory checkpoint

The apparent post-optimization RSS regression was investigated with repeated
fresh-process, snapshot-free measurement windows. The original 320-combined
baseline began measurement at 273.62 MiB RSS and 524.80 MiB private commit;
the first optimized sample began at 310.75 MiB RSS but only 506.26 MiB private
commit. Both shrank or remained flat during measurement. Higher RSS alongside
lower private commit means Windows retained more already-committed pages in
the working set; it does not indicate a new allocation leak.

Three 20-second samples each of 320 labels, LEDs, and combined rows then
tracked RSS, Windows private commit, text entries/owners, layout-text cache,
glyph-atlas trims, primitive buffer bytes, and final correctness. Valid
combined samples varied by less than 0.5 MiB RSS and approximately 0.03 MiB
private commit during their windows. Text entries stayed at 36, primitive
geometry stayed fixed, queues drained, and all final states remained correct.
One label sample missed its real-time deadline during an external scheduling
hiccup and is retained in the artifact but excluded from memory aggregates by
the corrected runner.

Footprint scaling then exposed the actionable library cost. On 64-bit builds:

- `NodeStyle`: 6,712 bytes;
- old `WidgetNode`: 22,776 bytes because it embedded computed, widget-default,
  and inline-authored `NodeStyle` values;
- new `WidgetNode`: 9,368 bytes after representing the two optional authored
  layers as `Option<Box<NodeStyle>>` and allocating them only when declarations
  exist;
- inline retained-node size reduction: **58.9%**.

The hot computed style remains inline, so layout/render access does not gain
an extra pointer chase. CSS cascade, snapshots, and live style patches unwrap
the optional layers only when present. A permanent storage-size regression
test prevents the optional layers from being inlined again.

The exact same three-by-20-second 320-combined workload before and after this
change produced:

| Metric | Before median | After median | Change |
|---|---:|---:|---:|
| Measurement-start RSS | 313.34 MiB | 301.41 MiB | **-11.93 MiB** |
| Measurement-start private commit | 491.12 MiB | 474.48 MiB | **-16.64 MiB** |
| Throughput | 30.000 Hz | 30.000 Hz | no regression |

Shorter three-process controls also stayed at 30 Hz. Labels saved 6.68 MiB
RSS / 2.83 MiB private commit; LEDs saved 9.81 MiB RSS / 4.62 MiB private
commit. The different reductions are expected because the modes contain
different proportions of styled and unstyled nodes.

Validation:

- native library: **935 passed, 13 intentionally ignored**;
- all three post-change combined samples passed every benchmark invariant;
- all six post-change label/LED controls passed;
- retained text and primitive resource counts remained fixed;
- benchmark scripts pass CPython 3.12 byte compilation;
- Rust formatting and changed-file diff checks pass.

Artifacts:

```text
artifacts/gui-telemetry-indicator-memory-2026-08-03-v1/summary.json
artifacts/gui-telemetry-indicator-memory-2026-08-03-compact-styles/summary.json
artifacts/gui-telemetry-indicator-memory-2026-08-03-compact-styles-controls/summary.json
plans/gui-telemetry-indicator-optimization-comparison-2026-08-03.html
```

This closes the suspected indicator memory regression and delivers a measured
retained-tree footprint reduction. The large fixed process baseline is mostly
outside per-widget retained storage and should be decomposed separately before
attempting riskier renderer/backend changes.

## August 3 fixed-baseline GPU memory checkpoint

A new fresh-process probe now separates five initialization boundaries:
standard-library process, compiled extension, public package, serialized
document, and live WGPU window. Five rotated repetitions established these
medians on the current Windows workstation:

| Stage | RSS | Private commit | Increment from prior stage (RSS / private) |
|---|---:|---:|---:|
| Standard-library process | 30.07 MiB | 15.16 MiB | baseline |
| Native extension | 31.28 MiB | 15.67 MiB | +1.21 / +0.51 MiB |
| Public package | 45.05 MiB | 27.75 MiB | +13.77 / +12.08 MiB |
| Document construction | 45.12 MiB | 27.82 MiB | +0.07 / +0.06 MiB |
| Live WGPU window | 259.10 MiB | 431.34 MiB | **+213.98 / +403.52 MiB** |

This localizes the fixed cost to graphics-device/window initialization rather
than Python import, native-module loading, document serialization, or retained
widgets. Source inspection then found that WGPU's default performance memory
hint reserves 128–256 MiB device blocks and 64–128 MiB host blocks. Its
memory-oriented hint starts at 8 MiB device and 4 MiB host blocks.

Three-process minimal-window A/B measurements were decisive:

| WGPU allocator hint | Median RSS | Median private commit |
|---|---:|---:|
| Performance | 259.04 MiB | 431.66 MiB |
| Memory usage | 259.39 MiB | **250.95 MiB** |

The memory-oriented allocator reduced fixed private commit by **180.71 MiB**.
Working-set RSS remained flat because Windows and the display driver still
mapped/touched a similar page set; private commit is the metric affected by the
allocator reservation policy.

Two validated workload A/Bs checked the performance tradeoff:

- 320 combined telemetry channels, three 25-second processes per hint: both
  sustained 30.00 Hz with zero drops and all 78 checks passing. Memory mode
  reduced median peak private commit from 475.52 to 294.15 MiB and CPU from
  34.69% to 33.98%. Submit p95 changed from 2.81 to 3.06 ms, still far inside
  the 33.3 ms frame budget.
- High live-visualization dashboard, two 25-second processes per hint: all
  validation passed. Memory mode reduced average peak private commit from
  1009.78 to 880.15 MiB, kept CPU effectively identical (88.94% vs 88.98%),
  improved submit p95 from 11.94 to 11.78 ms, and changed already-overloaded
  throughput from 28.95 to 28.77 Hz (0.6%).

DragonGUI now defaults to `wgpu::MemoryHints::MemoryUsage`. Advanced users can
restore the former allocator with `DRAGONGUI_WGPU_MEMORY_HINT=performance`.
The effective mode is exposed in the native renderer diagnostic snapshot, and
the baseline probe asserts live-window readiness while recording RSS and
private commit independently.

Final validation after changing the default:

- native library: **936 passed, 13 intentionally ignored**;
- Rust formatting and changed-file whitespace checks pass;
- both benchmark scripts pass CPython 3.12 byte compilation;
- a fresh default-mode live-window smoke reported `memory-usage`, reached the
  event loop, exited cleanly, and measured 262.80 MiB RSS / 250.84 MiB private
  commit on that sample.

Artifacts:

```text
artifacts/fixed-baseline-memory-2026-08-03-v2/summary.json
artifacts/fixed-baseline-memory-performance-2026-08-03/summary.json
artifacts/fixed-baseline-memory-compact-2026-08-03/summary.json
artifacts/wgpu-memory-hint-ab-2026-08-03/
artifacts/wgpu-memory-hint-dashboard-ab-2026-08-03/
artifacts/fixed-baseline-default-final-2026-08-03.json
```

## August 3 post-memory-optimization framework rerun

The validated four-stage telemetry-viewer comparison was rerun with the exact
prior protocol: one fresh process per framework/stage, three-second warmup,
15-second measurement, 30 Hz target, 1,024 retained samples per trace, and all
plots/indicators updated every tick. The matrix now aggregates Windows private
commit separately from RSS, and the HTML report includes a direct DragonGUI
before/after table.

All **124 validation checks passed**. DragonGUI sustained 30.00 Hz with zero
dropped generations at all four stages. At stage 4 (16 plots and 320 indicator
rows), its prior and current results were:

| Metric | Previous | Current | Change |
|---|---:|---:|---:|
| Throughput | 30.00 Hz | 30.00 Hz | maintained |
| Dropped generations | 0 | 0 | maintained |
| CPU | 90.9% | 45.4% | **-45.5 points / ~50%** |
| Update p95 | 4.23 ms | 4.91 ms | +0.68 ms |
| Native frame-work p95 | 0.72 ms | 0.82 ms | +0.10 ms |
| Peak RSS | 296.6 MiB | 322.1 MiB | +25.5 MiB |
| Peak private commit | 1009.6 MiB | 845.3 MiB | **-164.3 MiB** |

The small p95 increases are well inside the 33.3 ms cadence budget and come
from single samples, so they should not be overinterpreted. The CPU reduction
reflects the accumulated update-pipeline work since the prior report, while
the private-commit reduction directly agrees with the WGPU allocator A/B. The
higher RSS again shows that Windows working-set residency is not a substitute
for committed-allocation accounting.

Current stage-4 framework results:

| Framework | Throughput | Drops | CPU | Update p95 | RSS | Private commit |
|---|---:|---:|---:|---:|---:|---:|
| DragonGUI | 30.00 Hz | 0 | 45.4% | 4.91 ms | 322.1 MiB | 845.3 MiB |
| Dear PyGui | 30.00 Hz | 0 | 21.2% | 7.52 ms | 150.4 MiB | 600.6 MiB |
| PyQtGraph | 13.96 Hz | 240 | 103.2% | 10.72 ms | 119.3 MiB | 556.8 MiB |

DragonGUI now beats both comparison adapters on stage-4 application update
p95 and substantially out-delivers PyQtGraph under overload. Dear PyGui still
has the strongest CPU and memory footprint, which remains the next comparative
optimization target. This rerun installed Dear PyGui 2.1.1 because it was
missing from the workstation's Python 3.12 environment; framework-to-framework
numbers are current standings, while only the same-version DragonGUI row is
used for the before/after claim.

Artifacts:

```text
artifacts/gui-telemetry-viewer-comparison-2026-08-03-memory-optimized/summary.json
plans/gui-telemetry-viewer-memory-optimized-report-2026-08-03.html
```

## August 3 repeated-baseline, memory-attribution, and dense-update checkpoint

The next investigation repeated the lightest and heaviest DragonGUI and Dear
PyGui telemetry stages three times each. This removed single-run noise from the
comparison and confirmed that the remaining gap was stable rather than a
transient startup or allocator effect.

| Stage | Framework | Throughput | CPU | Update p95 | RSS | Private commit |
|---|---|---:|---:|---:|---:|---:|
| 1 | DragonGUI | 30.00 Hz | 14.2% | 1.29 ms | 298.1 MiB | 815.1 MiB |
| 1 | Dear PyGui | 30.00 Hz | 4.5% | 1.03 ms | 129.9 MiB | 579.9 MiB |
| 4 | DragonGUI | 30.00 Hz | 46.5% | 4.58 ms | 322.2 MiB | 845.4 MiB |
| 4 | Dear PyGui | 29.99 Hz | 18.4% | 7.40 ms | 150.5 MiB | 600.8 MiB |

All samples passed validation with zero dropped telemetry generations. The
DragonGUI private-commit ranges were tight: 813.6–815.1 MiB at stage 1 and
844.7–845.6 MiB at stage 4.

### Fixed-memory attribution

A new fresh-process profile runner separated a minimal window, a large window,
NumPy import, indicator scaling, line plots, and the complete stage-1 tree.
Three-process medians were:

| Profile | RSS | Private commit |
|---|---:|---:|
| Minimal window | 265.4 MiB | 251.6 MiB |
| Large window | 274.0 MiB | 329.2 MiB |
| NumPy + large window | 286.1 MiB | 821.2 MiB |
| 24 indicators | 282.7 MiB | 332.1 MiB |
| 320 indicators | 304.2 MiB | 359.6 MiB |
| Four line plots | 293.9 MiB | 822.5 MiB |
| Telemetry stage 1 | 298.7 MiB | 823.8 MiB |

The approximately 492 MiB jump comes from importing this workstation's NumPy
2.5 build on Windows, not from DragonGUI's retained widget or plot storage.
Every framework adapter imports NumPy, so that cost inflates the absolute
private-commit values but does not explain the cross-framework difference.
The large GPU surface adds approximately 78 MiB private commit; 24 indicators
add only about 3 MiB and 320 add about 30 MiB; four line plots add roughly
1–3 MiB beyond the NumPy baseline.

Two renderer-memory candidates were measured and deliberately rejected:

- Lazily constructing the line, image, and scatter compositors changed the
  fixed baseline from 259.39 MiB RSS / 250.95 MiB private commit to 260.81 /
  253.79 MiB. It provided no saving and was reverted.
- Experimental 16 MiB and 32 MiB manual WGPU allocation blocks saved at most
  approximately 4.2 MiB versus `MemoryUsage`. That was too small to justify
  replacing the supported, safer allocator hint and was reverted.

### Line-series replacement optimization

`LinePlot.set_data()` previously cleared every native series and resent both
axis labels before replacing data under the same keyed series. The method now:

- clears only series that were actually removed;
- sends axis-label properties only when their values change;
- retains the existing keyed packed-data replacement path.

At stage 4 this removed 8,640 clear commands and reduced direct property sends
from 17,280 to 16. Three repeated samples improved CPU from 46.5% to 40.2%,
update p95 from 4.58 to 4.01 ms, and native drain p95 from 14.41 to 12.77 ms,
while retaining 30 Hz, zero drops, and passing validation.

### Bounded dense-batch rebuild optimization

Profiling then showed approximately 1,081 visual rebuilds for 540 telemetry
ticks. A dense property packet crossed the 512-target safety limit, flushed a
full rebuild in the middle of the packet, and performed another rebuild at its
end. The deferred target collector now promotes itself to one bounded full
visual/text rebuild when it reaches the limit, clears the no-longer-useful
target sets, and ignores later target insertions until that logical batch is
flushed. The `SetProps` handler no longer performs a mid-packet flush.

The expected one-rebuild-per-tick behavior was observed: stage 4 recorded 541
visual rebuilds rather than approximately 1,081. Compared with the repeated
pre-change baseline:

| Metric | Baseline | Final | Change |
|---|---:|---:|---:|
| Stage-4 CPU | 46.5% | **31.4%** | -15.1 points / about 33% |
| Stage-4 native drain p95 | 14.41 ms | **8.77 ms** | -39% |
| Stage-4 update p95 | 4.58 ms | 4.42 ms | -3% |
| Stage-4 throughput | 30.00 Hz | 30.00 Hz | maintained |
| Stage-4 dropped generations | 0 | 0 | maintained |

The light stage remained targeted and also improved: CPU fell from 14.2% to
12.7% and update p95 from 1.29 to 1.04 ms. All six final samples sustained
30 Hz with zero drops, and all 90 benchmark checks passed.

Validation:

- native library: **937 passed, 13 intentionally ignored**;
- dedicated native test verifies target storage is cleared and bounded when
  the dense-batch limit is reached;
- Python 3.12 direct live-update checks verify removed-series cleanup,
  unchanged-axis suppression, packed replacement, and unchanged-LED no-ops;
- touched Python and benchmark modules pass CPython 3.12 byte compilation;
- Rust formatting passes.

Artifacts:

```text
artifacts/gui-telemetry-viewer-focused-repeat-2026-08-03/summary.json
artifacts/fixed-baseline-profiles-2026-08-03/summary.json
artifacts/fixed-baseline-memory-lazy-renderers-2026-08-03/
artifacts/fixed-baseline-block-memory-usage-2026-08-03/
artifacts/fixed-baseline-block-compact-16-2026-08-03/
artifacts/fixed-baseline-block-compact-32-2026-08-03/
artifacts/gui-telemetry-viewer-stage4-line-replace-2026-08-03/summary.json
artifacts/gui-telemetry-viewer-stage4-bounded-full-2026-08-03/summary.json
artifacts/gui-telemetry-viewer-stage1-bounded-full-2026-08-03/summary.json
```

### Post-optimization full framework rerun

The complete one-process-per-cell framework matrix was rerun after both CPU
changes, using the same three-second warmup, 15-second measurement, 30 Hz
target, and four workload stages as the prior HTML report. All **124 validation
checks passed**.

At stage 4, DragonGUI sustained 30.00 Hz with zero drops at 33.2% CPU and a
4.64 ms update p95. Dear PyGui sustained 30.01 Hz at 22.9% CPU and 7.39 ms;
PyQtGraph delivered 14.02 Hz with 239 dropped generations at 102.0% CPU and
10.06 ms. Relative to the prior same-machine DragonGUI report, stage-4 CPU
fell from 45.4% to 33.2% while update latency and correctness remained stable.

The repeated DragonGUI-only median (31.4%) remains the stronger estimate of
the final CPU level; the full matrix uses one sample per framework/stage and is
intended for current comparative placement. Dear PyGui still leads CPU and
memory footprint, while DragonGUI now has the lowest stage-4 application
update p95 and remains the only comparison adapter besides Dear PyGui to hold
the requested cadence at every stage.

Artifacts:

```text
artifacts/gui-telemetry-viewer-comparison-2026-08-03-dense-update/summary.json
plans/gui-telemetry-viewer-dense-update-report-2026-08-03.html
```

## August 3 indexed live-property lookup checkpoint

After the dense-batch rebuild fix, timing attribution showed that native
property application had become the largest remaining update-pipeline bucket:
approximately 2.84 seconds across one stage-4 run, versus approximately 1.38
seconds spent flushing visual rebuilds. The retained tree was the cause. A
high-frequency text property found its widget once to classify invalidation
and then walked the complete tree again to mutate the node.

DragonGUI now builds a safe retained `widget_id -> child-index path` map beside
the existing widget-kind map. Lookup follows the short child path from the
owned root rather than performing a depth-first scan. The index contains no
raw pointers or borrowed references, is reconstructed by every existing
structural replacement path, and has a debug invariant requiring its widget
count to match the kind index. High-frequency text and LED mutations now use
the indexed route; unrelated property behavior is unchanged.

An earlier candidate in this checkpoint was rejected. It bounded text and
primitive target sets independently in an attempt to avoid double-counting
overlapping widgets. Runtime evidence showed that the normalized sets were
largely disjoint and still exceeded the 512-root safety cap. All 540 stage-4
batches therefore paid normalization cost and then fell back to a full
rebuild. CPU increased from 31.4% to 32.6% and drain p95 from 8.77 to 9.31 ms,
so the change was reverted.

The retained path index produced the intended timing signature. Stage-4
rebuild time remained essentially unchanged while command-application total
fell from approximately 2.84 seconds to 1.84–1.96 seconds per run. Three-run
medians were:

| Metric | Previous dense-batch result | Indexed lookup | Change |
|---|---:|---:|---:|
| CPU | 31.4% | **25.6%** | -5.8 points / about 18% |
| Native drain p95 | 8.77 ms | **6.45 ms** | -26% |
| Application update p95 | 4.42 ms | **3.84 ms** | -13% |
| Throughput | 30.00 Hz | 30.00 Hz | maintained |
| Dropped generations | 0 | 0 | maintained |

The light stage also remained healthy at 12.3% CPU, 1.05 ms update p95, and
2.15 ms native drain p95. It sustained 30 Hz with zero drops. Peak RSS and
private commit were effectively unchanged at 318.1 MiB and 815.0 MiB,
respectively.

Validation:

- native library: **938 passed, 13 intentionally ignored**;
- a dedicated nested-tree test verifies root, child, and grandchild paths and
  mutation through the indexed target;
- all six repeated telemetry samples passed every validation check at 30 Hz
  with zero dropped generations;
- the final release extension was rebuilt and installed into the source
  package;
- Rust formatting passes.

Artifacts:

```text
artifacts/gui-telemetry-viewer-stage4-overlap-targets-2026-08-03/summary.json
artifacts/gui-telemetry-viewer-stage4-indexed-props-2026-08-03/summary.json
artifacts/gui-telemetry-viewer-stage1-indexed-props-2026-08-03/summary.json
```

## August 3 GPU-backend memory attribution and Windows policy checkpoint

The retained-tree reductions were real, but repeated framework results showed
that they could not materially change DragonGUI's large fixed graphics cost.
The next pass therefore measured surface size, presentation buffering,
retained renderer construction, screenshot texture usage, and WGPU backend
selection in separate fresh processes before changing production behavior.

### What the attribution found

A new surface sweep added explicit requested window dimensions and the complete
renderer diagnostic snapshot to the fixed-baseline probe. Under the previous
automatic Windows backend, median memory scaled from 259.6 MiB RSS / 250.7 MiB
private commit at 320x240 to 274.1 / 329.5 MiB at 1500x960. The 1920x1080
request plateaued because the runtime correctly clamped the window to the
monitor work area. This showed both a large fixed device/driver component and
a separate size-dependent surface allocation.

The following candidates did not produce a worthwhile reduction and were
reverted:

- reducing desired maximum frame latency from two to one;
- disabling the retained text and primitive renderers, individually and
  together (only about 7 MiB RSS at best, with an unfavorable allocator
  pattern in private commit);
- removing `COPY_SRC` from screenshot-capable surfaces.

The last experiment did expose a correctness issue: OpenGL surfaces on this
machine do not advertise `COPY_SRC`. Surface configuration now requests that
usage only when capabilities support it, and unsupported window screenshots
return a clear runtime error instead of triggering a WGPU validation failure.

Backend selection was the material lever. Three fresh 320x240 processes per
backend measured:

| Backend | Median RSS | Median private commit |
|---|---:|---:|
| Vulkan | 254.3 MiB | 243.8 MiB |
| DirectX 12 | **193.5 MiB** | **226.2 MiB** |
| OpenGL | 117.1 MiB | 71.9 MiB |

OpenGL's small baseline did not survive the performance gate. At telemetry
stage 4 it reached only 21.80 Hz and dropped 122 generations, so it remains an
explicit low-memory/compatibility option rather than the default. DirectX 12
held 30 Hz with zero drops and matched or improved the update timings.

### Production change

DragonGUI now prefers DirectX 12 on Windows and preserves WGPU automatic
backend selection on other platforms. Advanced users can override the policy
with `DRAGONGUI_WGPU_BACKEND=auto|dx12|vulkan|gl`. The renderer snapshot
reports both `backend_policy` and the actual `adapter_backend`, and `dg.help`
documents the policy, override, memory-hint interaction, and OpenGL tradeoffs.

The final three-process stage-4 run used the real default with the override
environment variable removed:

| Metric | Previous automatic Windows backend | DX12 default | Change |
|---|---:|---:|---:|
| CPU | 25.6% | **24.8%** | -0.8 points |
| Update p95 | 3.84 ms | **3.76 ms** | -2% |
| Throughput | 30.00 Hz | 30.00 Hz | maintained |
| Dropped generations | 0 | 0 | maintained |
| Peak RSS | 322.3 MiB | **274.9 MiB** | **-47.4 MiB** |
| Peak private commit | 845.4 MiB | **805.5 MiB** | **-39.8 MiB** |

All 45 workload checks passed. A final minimal live-window smoke confirmed the
effective `dx12` policy/adapter and measured 198.7 MiB RSS / 225.7 MiB private
commit. The complete native suite passes with **939 passed and 13 intentionally
ignored**; Rust formatting and Python 3.12 byte compilation also pass.

Artifacts:

```text
artifacts/gui-surface-memory-sweep-2026-08-03/summary.json
artifacts/gui-memory-backend-vulkan-repeat-2026-08-03/summary.json
artifacts/gui-memory-backend-dx12-repeat-2026-08-03/summary.json
artifacts/gui-memory-backend-gl-repeat-2026-08-03/summary.json
artifacts/gui-telemetry-viewer-stage4-gl-backend-2026-08-03/summary.json
artifacts/gui-telemetry-viewer-stage4-dx12-backend-2026-08-03/summary.json
artifacts/gui-memory-dx12-default-final-smoke-2026-08-03.json
artifacts/gui-telemetry-viewer-stage4-dx12-default-final-2026-08-03/summary.json
```
