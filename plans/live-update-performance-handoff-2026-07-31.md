# DragonGUI Live-Update Performance Handoff

**Date:** July 31, 2026
**Branch:** `master`
**Remote:** `origin` (`https://github.com/NKocur/DragonGui.git`)
**Starting revision for this work:** `d08488b`
**Handoff revision:** the commit containing this document; run `git log -1 --oneline` after pulling
**Primary plan:** [`plans/live-update-performance-optimization-plan.md`](live-update-performance-optimization-plan.md)
**Related CSS/stress plan:** [`plans/theme-forge-performance-remediation.md`](theme-forge-performance-remediation.md)

## Purpose of this document

This is a restartable handoff for continuing the live-update performance work
on another machine or with another agent. It records what is implemented, what
has been proved, what remains uncertain, the exact test and benchmark entry
points, and the local-only evidence that is intentionally not stored in Git.

The short version is:

- Phases 0, 1, and 2 of the live-update performance plan are implemented and
  development-validated.
- Phase 3 has a conservative geometry-aware text invalidation classifier,
  reason-coded diagnostics, a completed static property matrix, and strong
  rendered equivalence infrastructure.
- Phase 3 is **not complete**. Dedicated differential cases are still needed
  for state-backed controls, plot chrome, HTML fallback text, and semantic icon
  changes before expanding any fast-path eligibility.
- The next agent should continue Phase 3 rather than beginning Phase 4.

## Current implementation state

### Python batching and producer-side work

The Python runtime now has a typed `app.update_batch()` path that collects live
property updates, removes duplicate writes where ordering semantics permit it,
and sends one native packet per completed update callback. Ordering barriers are
preserved when a structural or styling command must remain between property
writes.

Important files:

- `python/dragongui/app.py`
- `python/dragongui/runtime.py`
- `examples/live_update_batch_demo.py`
- `tests/test_python_api.py`

The Python/native bridge exposes request, packet, duplicate-removal, barrier,
and method-level counters through diagnostics. The benchmarks validate those
counters rather than accepting timing output without proof that the intended
updates occurred.

### Native command queue

The previous queue behavior was replaced with a bounded-memory linked-slot
design. Replaceable commands retain stable lookup semantics without retaining
unbounded stale payloads. Coalescing covers live widget properties and the
supported packed plot/scatter resource updates while respecting lossless
barriers.

Important files:

- `native/src/commands.rs`
- `native/src/command_queue_model.rs`
- `native/src/lib.rs`

Phase 2 evidence established:

- Flat logical behavior through a 100,000-update backlog.
- Bounded physical slot and payload retention.
- Correct barrier ordering.
- Correct final state for ordinary properties and packed plot resources.
- Stable repeated release soaks and process-memory checks.

The exact historical results and artifact names are in the primary plan.

### Targeted native rebuild pipeline

The runtime can retain target IDs while deferred commands are drained, normalize
them to non-overlapping subtree roots, and rebuild only the affected text and
dependent primitives. Mixed text/visual batches merge their target sets. Plot
targets trigger the required plot-resource rebuild. Stale IDs and oversized
target sets fail closed to a full visual rebuild.

Important file:

- `native/src/runtime.rs`

Key diagnostics include:

- Dirty rebuild request and execution counts.
- Deferred merge counts.
- Targeted/global/overlay text requests.
- Attempted, completed, and fallback targeted batches.
- Rebuilt root counts and the per-batch safety limit.
- Stage timing for full and partial text rebuilds.

### Safe text-only invalidation

The earlier runtime treated many text mutations as paint-only even when longer
content could alter intrinsic layout. Phase 3 now classifies retained text
mutations conservatively.

Current rules:

- A non-wrapping `Label` can use targeted text rebuilding only when width is
  definite.
- A wrapping `Label` needs definite width and height.
- Other content-sized retained composites need definite width and height.
- Logical-pixel sizes and validated fixed widget properties count as definite.
- `auto`, percentages, and `calc()` do **not** count as proof of stability.
- Missing nodes, unsupported properties, and unproved geometry request layout.

The classifier is used by buttons, badges, panel titles, navigation labels and
badges, loading-spinner labels, and the remaining retained composite families.
The static audit covers 32 supported widget/property pairs. Six names that had
no actual mutation route are now rejected instead of being counted as eligible:

- `Badge.badge`
- `Badge.value`
- `Tag.value`
- `Menu.badge`
- `MenuItem.badge`
- `Panel.text`

Reason-coded counts are available under:

```text
framework.live_text_invalidation
```

Reasons currently distinguish fixed single-line labels, fixed wrapped labels,
fixed composites, intrinsic width, intrinsic height, both axes intrinsic, and
unsupported properties.

### Text-overlap correction found by benchmarking

The long-message benchmark exposed wrapped text whose measured line height and
allocated row geometry could disagree, allowing one line to be cut off by the
next. The text/layout fixes and regression coverage are in:

- `native/src/layout.rs`
- `native/src/text/mod.rs`
- `native/src/css_style.rs`

Do not remove the deliberately long and multiline strings from the intrinsic
benchmark; they are correctness probes, not decorative sample copy.

## Completed Phase 3 evidence

The live-update pipeline benchmark now validates final Python values, retained
native values, queue drain/accounting, layout diagnostics, geometry, clips,
scroll positions and maxima, hit-test targets, screenshots, and invalidation
reason totals.

Entry points:

- `benchmarks/gui_update_pipeline_case.py`
- `benchmarks/run_gui_update_pipeline_matrix.py`
- `benchmarks/gui_benchmark_validation.py`
- `tests/test_gui_benchmark_validation.py`

Implemented Phase 3 scenarios:

- `labels-fixed`
- `intrinsic-text`
- `composite-text-fixed`
- `composite-text-intrinsic`

The composite cases cover ten rows of:

- Button text.
- Standalone badge text.
- Loading-spinner label.
- Navigation-item label.
- Panel title.

They use a bounded scroll viewport and validate exact bottom clamping. Their
equivalence hash includes retained nodes, resolved and available rectangles,
overflow, final paint clips, dynamic geometry, scroll offsets, and scroll
maxima. Synthetic hover probes verify direct target resolution for interactive
buttons and navigation items.

The latest authoritative local run was:

```text
artifacts/gui-update-pipeline-phase3-composites-final-v2/summary.json
```

It passed 157–161 checks per sample at approximately 60 Hz. Individual and
batched pairs had identical geometry/clip/scroll/state hashes and byte-identical
settled screenshots. Visible right-side panels had zero horizontal clipping,
and all requested hover targets resolved exactly.

These `artifacts/` files are intentionally ignored by Git. They will not be
present after a fresh clone and should be regenerated when new native behavior
is tested.

## Intentional benchmark behavior that can look like a bug

### Spinner animation

The loading spinners in the exact screenshot-equivalence composite cases use
`spinning=False` intentionally. A running spinner changes pixels with wall-clock
time, so two correct implementations can produce different screenshot hashes.
The spinner **label still updates every generation**. Animated spinner behavior
is covered by ordinary live stress workloads and should remain enabled there.

If animation itself needs differential coverage, compare semantic animation
state and multiple bounded-time frames rather than requiring byte-identical
single screenshots.

### Long panel titles

The composite benchmark deliberately alternates short and long panel titles.
Panel, navigation, and spinner labels use bounded single-line ellipsis by
default. The right-hand test panels were widened from 220 to 300 logical pixels
so the case remains visibly readable while retaining long-string coverage.

### Offscreen clips

Rows fully outside the bounded scroll viewport correctly have empty final paint
clips. Horizontal clipping assertions apply only to at-least-partially-visible
widgets; treating empty clips for offscreen rows as a failure would invalidate
correct scroll clipping.

## Validation status at handoff

The latest source-level validation completed immediately before this handoff:

```text
cargo test --manifest-path native/Cargo.toml --lib
917 passed, 13 ignored, 0 failed

py -3.11 -m pytest tests/test_gui_benchmark_validation.py -q
6 passed

py -3.11 -m pytest -q
567 passed
```

The ignored Rust tests are manual benchmarks/soaks, not unexpected failures.
`git diff --check` was clean for the current implementation and plan files.

The most recent rendered evidence used a wheel/runtime built before the final
classifier property-name cleanup. That cleanup only narrows unreachable
diagnostic eligibility and passes the complete native suite, but a new wheel
should be built before the next rendered matrix.

## Local build and test environment

### Python versions

The bare `python` command on the development machine resolves to Python 3.9 and
is too old for this codebase. Use `py -3.11` for pure-Python tests and tooling.

For PyO3/Cargo on the original Windows machine:

```powershell
$env:PYO3_PYTHON='C:\Users\nkocur\AppData\Local\Microsoft\WindowsApps\PythonSoftwareFoundation.Python.3.12_qbz5n2kfra8p0\python.exe'
cargo test --manifest-path native/Cargo.toml --lib
```

At home, replace that path with the local Python 3.12 executable.

### Rebuilding a benchmark wheel

Use the repository's normal wheel build process, install/extract it into a new
artifact runtime directory, and point the benchmark subprocess at that runtime
with `DRAGONGUI_BENCH_PYTHON_PATH`. Do not reuse the old
`artifacts/live-update-phase3c-runtime` after changing Rust code.

The previous development wheel and runtime were:

```text
artifacts/live-update-phase3c-wheel/dragongui-0.1.0-cp312-abi3-win_amd64.whl
artifacts/live-update-phase3c-runtime
```

They are useful only as historical references and are not committed.

### GUI execution

Rendered benchmark subprocesses open native windows and may need execution
outside a restricted sandbox. Run each scenario in a fresh process; GPU/runtime
state and environment-driven probes should not leak between samples.

## What remains in Phase 3

Continue these tasks in order.

### 1. Add a true forced-safe differential mode

The current individual-versus-batch comparisons prove command-path and settled
rendering equivalence, but both sides use the same classifier. Add a diagnostic
mode that forces otherwise eligible retained text mutations through layout
without changing authored styles or widget geometry. The mode should be
captured in `debug_snapshot()` and benchmark output so results cannot be mixed
up.

Recommended constraints:

- Configure it once per runtime/process; do not read an environment variable on
  every property mutation.
- Keep it diagnostic-only and off by default.
- Preserve the same widget tree, CSS, viewport, scale, update sequence, and
  settled generation between optimized and forced-layout samples.
- Compare geometry, clips, scroll state, hit targets, and screenshots.
- Require the optimized sample to record the expected text-only reason and the
  control sample to record the forced-safe reason.

### 2. Add state-backed control differential cases

Cover at least:

- `TextInput.value`
- `TextArea.value`
- `CodeEditor.value`
- `LogView.value`, with `follow=True` and `follow=False`
- `Dropdown.value`
- `NumberInput.value`
- `DragNumber.value`

Validate outer geometry, internal text/caret bounds, wrapping, internal scroll,
focus retention, final values, clips, hit targets, and screenshots. Use Unicode,
combining marks, RTL, empty text, multiline text, and long values. Do not infer
that a stable outer rectangle guarantees correct caret or text-scroll state.

### 3. Add plot-chrome differential cases

Cover line plot, histogram, and bar chart mutations for:

- Axis labels.
- Axes/ticks/toolbar visibility.
- Tick count.
- Legend visibility and position where supported.
- Line-plot window size and bounds interactions.

The outer plot rectangle should remain stable, but the internal drawable
viewport, ticks, labels, toolbar, legend, and plot GPU resources must agree.
Verify that a targeted line-plot mutation actually increments target-local plot
rebuild diagnostics rather than silently showing stale resources.

### 4. Add HTML fallback and icon differential cases

For `HtmlReport.text`, distinguish fallback presentation from webview source.
Confirm that `path`, inline `html`, base directory, and security flags still use
full synchronization. Test both a platform with the embedded webview and the
fallback renderer when practical.

For `IconButton.icon`, verify semantic icon-theme reconciliation, unchanged
outer/hit geometry, correct clip, and exact final icon identity across default
and live-overridden icon themes.

### 5. Decide whether eligibility should expand

Only expand the general fast-path predicate after the dedicated cases report
zero geometry, clipping, scroll, hit-test, state, and visual mismatches. Special
families may remain on explicit target-local routes; unifying code paths is not
a goal if it weakens the safety proof.

### 6. Close the Phase 3 performance target

The plan still requires a measured reduction in rebuild-flush p95 for the fixed
label workload and a targeted fallback rate below 5% there. Run enough repeated
release samples to calculate a stable comparison, then update the plan with the
fresh artifact paths and exact pass/fail result.

## Later phases — do not start until Phase 3 is closed

Phase 4/5 introduces more precise retained-tree dirty regions, typed target
sets, structure generations, centralized coalescing semantics, randomized
locality verification, and full-fallback regression gates.

Later phases cover:

- Buffer/resource reuse and upload locality.
- Scheduling and animation efficiency.
- Repeated release-grade soak and report refresh.

See the unchecked checklist at the end of the primary plan for the authoritative
sequence.

## Repository and artifact hygiene

- `artifacts/`, `.serena/`, `.codex/`, and `.codex-workbench-sessions/` are
  ignored and must stay out of commits.
- `.test-cache/` contains old tracked material from earlier work. The changing
  pytest node list and missing local wheel/runtime copies are intentionally not
  part of this product checkpoint. A separate cleanup commit should remove all
  remaining tracked cache artifacts and add `.test-cache/` to `.gitignore` if
  desired.
- Benchmark scripts, validation helpers, plans, and human-readable HTML/Markdown
  reports are source material and should remain committed.
- Before committing future work, inspect `git status --short` and stage explicit
  paths rather than using an indiscriminate `git add -A`.

## Suggested first prompt for the next agent

> Read `plans/live-update-performance-handoff-2026-07-31.md` and
> `plans/live-update-performance-optimization-plan.md` completely. Continue
> Phase 3 by implementing a diagnostic forced-layout comparison mode and add a
> rendered fixed-label optimized-versus-forced-layout differential case. Update
> both documents with evidence, rebuild a fresh wheel, and do not begin Phase 4
> until the Phase 3 acceptance targets are satisfied.
