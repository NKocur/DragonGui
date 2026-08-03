# DragonGUI Live-Update Performance Handoff

**Date:** July 31, 2026
**Last updated:** August 1, 2026
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
- The diagnostic forced-layout control and the first true optimized-versus-safe
  fixed-label differential are complete. All three release-wheel pairs matched
  geometry and screenshots with zero targeted fallback.
- The state-backed control differential is also complete for text inputs,
  editors, following/non-following logs, dropdowns and numeric controls. It
  found and fixed focused multiline caret scroll after live value replacement.
- The line-plot, histogram, and bar-chart chrome differential is complete. It
  proves internal viewport, bounds, clipped label, toolbar/legend, screenshot,
  and target-local line-plot GPU resource equivalence across three pairs.
- The HTML fallback, embedded WebView2, and semantic-icon differentials are
  complete. They found and fixed a WebView2 navigation-transition race; all 18
  final release-wheel samples matched geometry, retained state, and screenshots.
- The first five-by-60-second fixed-label run exposed an intrinsic status-label
  confound. The corrected pure fixed-geometry gate passed with zero layout,
  zero fallback, and 99.05% lower median rebuild-flush p95.
- The eligibility decision is complete: retain the validated explicit
  target-local routes rather than broadening the general content-sized
  predicate without a measured benefit.
- Phases 3 and 4 are complete. Phase 4's current-machine baseline,
  structure-generation guard, typed deferred targets, locality scaling controls,
  10,080-batch targeted-versus-full gate, and matched full-fallback overhead
  gate all passed.
- The workstation is materially faster than the original handoff machine.
  Continue using matched current-machine controls; do not compare absolute
  timings against the July 31 artifacts.

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
fixed composites, diagnostic forced-safe layout, intrinsic width, intrinsic
height, both axes intrinsic, and unsupported properties. The snapshot also
records whether the runtime was configured for `optimized` or `forced-layout`
invalidation. State-backed routes additionally report `target_local_state`.
Plot-chrome routes report `target_local_plot`, HTML fallback copy reports
`target_local_html_fallback`, and semantic icon changes report
`target_local_icon`.

The differential snapshot now includes opt-in per-owner shaped text geometry
and can apply one verified synthetic focus target. The equivalence hash covers
native text/numeric/dropdown state, cursor and internal scroll maps, focus,
caret positions and shaped entry bounds. Requested owner IDs are configured
once with `DRAGONGUI_DIAGNOSTIC_TEXT_GEOMETRY_IDS`, avoiding large payloads in
ordinary snapshots.

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
- `state-controls`
- `plot-chrome`
- `html-fallback`
- `html-webview`
- `semantic-icons`

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

### Plot-chrome differential

The `plot-chrome` case updates fixed-size line plot, histogram, and bar chart
widgets at 60 Hz. It covers Unicode and long axis labels, grid/axes/ticks and
toolbar visibility, tick count, line-plot legend visibility/position, moving
window size, and explicit x/y bounds overrides. The equivalence hash includes
each fixed outer rectangle, internal drawable viewport, visible clip, resolved
bounds, retained chrome props, and opt-in owned plot-label geometry. Synthetic
hover verifies all three hit targets.

The forced-safe diagnostic now includes target-local plot text decisions. In
optimized mode those mutations remain targeted `Dirty::Text` and report
`target_local_plot`; forced-layout promotes the same decisions to layout.
Opt-in `renderer.plot_geometry` exposes the internal viewport and bounds, while
plot overlay text is now attributed to its widget owner for diagnostic geometry
without changing rendering.

The first diagnostic smoke exposed anonymous ownership for plot overlay labels;
the owner probe now includes ephemeral plot entries. The first longer sample
also caught a benchmark bounds generator that moved beyond its static dataset;
the corrected bounded cycle always retains visible line data while still
testing window/bounds precedence.

The final three-repetition release-wheel differential used batched updates, a
0.5-second warmup, two measured seconds, fresh processes, and settled
screenshots. All three optimized/forced pairs passed 59 or 60 checks at about
60 Hz:

- Each sample recorded 3,300 candidates: 3,150 plot decisions and 150
  intrinsic-width status-label decisions.
- Optimized samples recorded 3,150 `target_local_plot` text-only decisions;
  controls recorded zero text-only decisions and 3,150 `forced_safe_layout`
  decisions.
- Final outer rectangles remained 900 by 210 logical pixels. The line,
  histogram, and bar drawable viewports were respectively 838 by 162, 842 by
  158, and 842 by 158 logical pixels.
- The final line renderer retained two series, 30 visible points, and 28
  segments. Targeted diagnostics recorded two line-plot resource rebuilds and
  one non-line target skip with balanced checks.
- Every pair had equivalence hash
  `5f6227800ae47e075fee3add61bded7e00d5452e077b5958990481eafe3d7057`
  and byte-identical screenshot hash
  `193147f1f4feb0067d67681c79ce962c2492f6c4b9df5e9d473505731a0bb769`.

Authoritative local evidence:

```text
artifacts/gui-update-pipeline-phase3-plot-chrome-final-v1/summary.json
```

### HTML fallback, embedded WebView2, and semantic icons

Three new rendered cases close the remaining target-local family coverage:

- `html-fallback` forces the native fallback renderer, mutates Unicode and long
  fallback copy at 60 Hz, and proves source/security state remains independent.
- `html-webview` forces WebView2, changes a report from a local path to inline
  HTML and back, and validates source-family fingerprints, script policy,
  instance bounds, visibility, and final renderer health.
- `semantic-icons` rotates built-in, aliased, unknown-fallback, and custom
  semantic icons while replacing the live icon theme. It validates final
  computed identity, fixed outer geometry and clips, and all hover targets.

`HtmlReport.text` now uses the explicit `target_local_html_fallback` route;
inline HTML, path, base directory, and security changes remain full sync.
`IconButton.icon` uses the explicit `target_local_icon` route after semantic
theme reconciliation. Both routes participate in the forced-layout control.
Snapshots expose stable inline/source fingerprints and raw plus computed icon
identity without retaining the HTML bodies in diagnostics.

The first combined run exposed a real embedded-browser race: a rapid
inline-to-file transition could return WebView2 `ERROR_INVALID_STATE`, leaving
the prior source visible. Each WebView now registers a navigation-completion
handler that wakes the runtime and re-syncs the still-current desired source.
Recovered syncs clear the current `last_error`. The unchanged benchmark then
completed every transition with zero emergency reload attempts.

The final release-wheel matrix used three repetitions, batched updates, a
0.5-second warmup, two measured seconds, fresh processes, and settled
screenshots. All 18 samples validated at approximately 60 Hz. Every one of the
nine optimized/forced pairs matched retained geometry and screenshots:

- HTML fallback: equivalence
  `582363c0ccc9c4b23138ceeeb8ee1eb62063077cb2f9ff75f2c8129191971ae6`.
- Embedded WebView2: equivalence
  `53d0e81f6b58954da8a74f17805295cad9b46535314da86aa2787bdbe4b00b05`;
  all six samples recorded zero reload attempts and no final renderer error.
- Semantic icons: equivalence
  `7dbf88f456568f28857fd2cef27bb881d64bacfda1b1c0228a4292590e338f4c`
  and screenshot
  `df57426f9ba0912123b458e5440466ae56c71f405bb93bdee8eebdf7e5962d5e`.
- The HTML cases shared settled screenshot hash
  `0acc587ddb1e25eeeaf4330c2ed8b706d583f421225132d4561dcaadcb576a75`.
- Across optimized samples, HTML fallback/WebView each recorded 456
  target-local HTML decisions and semantic icons recorded 1,800 target-local
  icon decisions. Forced controls promoted the same totals to layout.

Authoritative local evidence and the fresh runtime are:

```text
artifacts/gui-update-pipeline-phase3-html-icon-final-v2/summary.json
artifacts/live-update-phase3g-html-icon-wheel/dragongui-0.1.0-cp312-abi3-win_amd64.whl
artifacts/live-update-phase3g-html-icon-runtime-navretry
```

### Fixed-label repeated release gate

The required phase-decision run used `labels-fixed:200`, batched updates, a
five-second warmup, five 60-second measured repetitions per mode, alternating
fresh processes, and settled screenshots. All ten samples validated and every
optimized/forced pair shared equivalence hash
`7397ef3ee727309c3782a7e4543e75bc2bbdf60cff49fc3266241db7d92b4914`
and screenshot hash
`a5b819cda657a87d4446ca68d5ba99e89d4d8b823a83cc40fb3f53074249fd07`.

Optimized rebuild-flush p95 samples were 10.1567, 10.3872, 12.3512, 10.5711,
and 14.6792 ms. Forced-layout samples were 12.1743, 12.3149, 12.0998,
11.7736, and 12.2937 ms. The apparent 13.17% median reduction did not meet the
20% target. Stage diagnostics identified a benchmark confound: the
intrinsic-width `pipeline-status` label was updated every generation, forcing
global layout averaging about 10.34 ms in every optimized frame. This v1
artifact remains diagnostic evidence, not the final performance result.

The fallback and correctness portions pass. All 58,410 targeted batches
completed with zero fallback; 3,894,000 fixed-label mutations used the
text-only route, and the 19,470 layout decisions were the intentionally
intrinsic status label. Median throughput remained approximately 60 Hz.

```text
artifacts/gui-update-pipeline-phase3-fixed-label-release-v1/summary.json
```

The correction gives the status label definite width only in `labels-fixed`;
the separate intrinsic scenario retains conservative layout coverage.
Validation now requires every optimized fixed-label candidate to be text-only
and requires exactly zero layout decisions. The corrected five-by-60-second v2
gate passed all ten fresh samples:

- Optimized p95: 0.0959, 0.0929, 0.1138, 0.1341, and 0.1217 ms; median
  0.1138 ms, worst 0.1341 ms.
- Forced-layout p95: 11.6644, 11.5034, 11.9257, 12.3257, and 12.8901 ms;
  median 11.9257 ms, worst 12.8901 ms.
- Median reduction: 99.05%; worst-p95 reduction: 98.96%.
- Optimized decisions: 3,919,500 text-only, zero layout, 78,000 targeted
  batches, and zero fallback.
- Every pair matched equivalence hash
  `66e69e31c25da886961ee8a4f0a2d7452b663e6a3631e70bf7a9d06e33b9dd12`
  and screenshot hash
  `a5b819cda657a87d4446ca68d5ba99e89d4d8b823a83cc40fb3f53074249fd07`.

This closes the Phase 3 correctness, fallback, zero-layout, and 20%
rebuild-flush targets.

```text
artifacts/gui-update-pipeline-phase3-fixed-label-release-v2/summary.json
```

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

The latest source-level validation after the forced-safe differential work is:

```text
cargo test --manifest-path native/Cargo.toml --target x86_64-pc-windows-gnu --lib
918 passed, 13 ignored, 0 failed

py -3.11 -m pytest tests/test_gui_benchmark_validation.py -q
6 passed

py -3.11 -m pytest -q
567 passed
```

The ignored Rust tests are manual benchmarks/soaks, not unexpected failures.
`git diff --check` was clean for the current implementation and plan files.

The latest rendered evidence uses a fresh ABI3 wheel containing the classifier,
forced-safe diagnostic mode, state controls, plot chrome, HTML/WebView, and
semantic-icon coverage. The HTML/icon matrix passed all 18 samples. The later
corrected five-by-60-second fixed-label gate passed all ten samples and all
equivalence checks, with zero layout and zero fallback across 78,000 optimized
targeted batches. Median rebuild-flush p95 was 0.1138 ms optimized versus
11.9257 ms forced-layout, a 99.05% reduction. See:

```text
artifacts/gui-update-pipeline-phase3-forced-layout-wheel-final-v1/summary.json
artifacts/gui-update-pipeline-phase3-state-controls-final-v1/summary.json
artifacts/gui-update-pipeline-phase3-plot-chrome-final-v1/summary.json
artifacts/gui-update-pipeline-phase3-html-icon-final-v2/summary.json
artifacts/gui-update-pipeline-phase3-fixed-label-release-v2/summary.json
```

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

The current development wheel and runtime are:

```text
artifacts/live-update-phase3g-html-icon-wheel/dragongui-0.1.0-cp312-abi3-win_amd64.whl
artifacts/live-update-phase3g-html-icon-runtime-navretry
```

The GNU-built wheel runtime needs the repository's x64 `WebView2Loader.dll`
beside the extension, matching the normal source-tree runtime arrangement.
These artifacts are ignored and are not committed.

### GUI execution

Rendered benchmark subprocesses open native windows and may need execution
outside a restricted sandbox. Run each scenario in a fresh process; GPU/runtime
state and environment-driven probes should not leak between samples.

## What comes next

Phase 3 is closed and the first two Phase 4 safety increments are complete.
Continue with the remaining typed target classes and randomized locality/scaling
probes. Use the current-machine v2 baseline for matched performance comparisons.
Keep state, plot, HTML fallback, and semantic icon families on their validated
explicit target-local routes; do not broaden the general content-sized predicate
unless a new geometry contract and measurable benefit justify it.

The first baseline attempt exposed intrinsic badges and an intrinsic status
label in the supposedly fixed `mixed-state` scenario; v1 is diagnostic only.
After fixing their geometry and adding fail-closed zero-layout/targeted checks,
five fresh 60-second samples all validated. Median throughput was 59.3331 Hz
(worst 56.6667), command-drain p95 was 16.0391 ms (worst 16.3939), command-apply
p95 was 14.8414 ms (worst 15.3339), and rebuild-flush p95 was 1.1492 ms (worst
1.2523). All 249,301 targeted batches completed with zero fallback and all
7,689,977 text candidates stayed on the fast path with zero layout. The
existing mixed text/visual merge already clears the 95% targeted-completion
target; stale-target generations are the next native gap.

```text
artifacts/gui-update-pipeline-phase4-current-machine-baseline-v1/summary.json  # invalid intrinsic-geometry diagnostic
artifacts/gui-update-pipeline-phase4-current-machine-baseline-v2/summary.json  # authoritative fixed-dashboard control
```

The first Phase 4 native increment is also complete. Deferred target batches
capture a retained-tree structure generation; successful node/children
replacement increments the generation. A mismatch rejects targeted reuse and
forces `Dirty::Full`. Snapshots expose the current generation and a dedicated
stale-generation fallback counter.

The fresh Phase 4a wheel passed an optimized/forced-layout fixed mixed-state
smoke with exact geometry and screenshots. Optimized mode sustained 59.9996 Hz,
completed 8,580 targeted batches, and recorded zero layout, ordinary fallback,
or stale-generation fallback. Forced layout sustained 18.9988 Hz. Both modes
shared equivalence hash
`80652a5c0bb50585dd284931f0f98dbc22182ac69320fb66686342ac8076af9a`
and screenshot hash
`6c5ec803bb2de9a39f9d0a8b625be71a4480c85441cdfe58e775360202612492`.

```text
artifacts/gui-update-pipeline-phase4a-structure-generation-smoke-v2/summary.json
artifacts/live-update-phase4a-structure-generation-runtime
```

The second Phase 4 increment adds an opt-in targeted-versus-full verifier.
`DRAGONGUI_DIAGNOSTIC_TARGETED_REBUILD_MODE=verify-full` signs retained
primitive instances in paint order, permanent shaped-text entries (including
owners, bounds, clips, colors, and custom glyphs), and line-plot GPU inputs
after a successful targeted batch. It then reconstructs the same state through
the full text/primitive path, compares component signatures, records mismatch
counters and last signatures, and leaves the full reconstruction installed as
the safe final output. Normal runs default to `off` and do no signature work.

The fresh release-wheel `mixed-state:200` smoke used batched updates, a
0.5-second warmup, ten measured seconds, and screenshots. All **3,614** targeted
batches were verified with **zero** primitive, text, or line-plot mismatches and
zero fallback. The run validated at 26.4923 Hz; this is expected diagnostic
overhead from deliberately doing both rebuilds and is not a candidate baseline.
The equivalence hash was
`68ea50c491c0add10cf02e460cdf8b2fb99323ebb00d245750449a7b5fe15de4`
and the screenshot hash was
`c1823d113c5bc549ace3263400106388bcca6eb3af984065eda2b8c87052dafe`.
The complete regressions passed: Rust **920 passed / 13 ignored** and Python
**567 passed**.

```text
artifacts/gui-update-pipeline-phase4b-targeted-verifier-smoke-v1/summary.json
artifacts/live-update-phase4b-targeted-verifier-wheel/dragongui-0.1.0-cp312-abi3-win_amd64.whl
artifacts/live-update-phase4b-targeted-verifier-runtime
```

The wheel SHA-256 is
`e0aff8649ab735aa0ff3fc4fc365772cd5d0e1246876366ec37fb19d5c627d8d`;
the extracted `_dragongui.pyd` SHA-256 is
`4e020b32a3b20e9b6c0ca4a188abf645cbf138f7b3ed28500939f8fdd8ffe0e7`.
Scatter/image/WebView resource generations and layout/hit-test signatures are
not yet part of this core retained-output signature and remain follow-up
coverage where those target classes become eligible.

The third Phase 4 increment adds deterministic randomized locality controls to
the fixed-label benchmark. `--locality-roots N` updates `N` distinct
pseudo-random labels per callback, validates a hash of all 1,000 final Python
and native values, and fails unless rebuilt roots and text-entry churn remain
bounded by the mutation set with zero targeted/primitive fallback and zero
primitive upload bytes. The matrix summary now promotes those locality counters.

On the current Ryzen 7 5800X / RTX 3080 Ti workstation, both three-repetition
controls used the same Phase 4b runtime, batched updates, two-second warmups,
15-second measurements, 60 Hz targets, and screenshots:

| Probe | Median throughput | Median drain p95 | Median apply p95 | Median flush p95 |
|---|---:|---:|---:|---:|
| 1 of 1,000 | 59.9999 Hz | 0.5984 ms | 0.2115 ms | 0.3774 ms |
| 20 of 1,000 | 60.0014 Hz | 1.0011 ms | 0.5028 ms | 0.5029 ms |

Root accounting was exact: the 1-root runs rebuilt two roots per completed
callback (random label plus status), while the 20-root runs rebuilt 21. Across
all six samples there were zero ordinary or stale-generation fallbacks and zero
primitive upload bytes. The 20-root mutation set is 20 times larger, while
median rebuild-flush p95 increased only 1.33 times. Screenshot and retained
equivalence hashes were stable within each three-run cohort.

A separate ten-second `verify-full` run compared **660** randomized 20-root
batches against full reconstruction with zero primitive, text, or line-plot
mismatches and maintained 60.0008 Hz. This is useful differential coverage but
does not yet close the Phase 4 target of 10,000 randomized batches. The complete
Python suite now reports **568 passed**.

```text
artifacts/gui-update-pipeline-phase4c-locality-1of1000-v1/summary.json
artifacts/gui-update-pipeline-phase4c-locality-20of1000-v1/summary.json
artifacts/gui-update-pipeline-phase4c-locality-20of1000-verify-v1/summary.json
```

The fourth Phase 4 increment consolidates the deferred rebuild state into a
typed `DeferredRebuildBatch`. Its target container explicitly separates
retained-visual roots, primitive-paint roots, table-text roots, overlay text,
global text/primitive fallback requirements, scatter-style synchronization,
and the captured structure generation. Packet splitting deliberately retains
the previous conservative raw-set count, including cross-class duplicates, so
this representation change does not silently broaden batches.

Snapshots now add per-class request counters under
`command_text_rebuilds.target_classes`. The fresh Phase 4d wheel produced:

- Randomized 20-of-1,000 locality smoke: 13,860 retained-visual requests,
  no other classes, 660/660 targeted/full matches, zero fallback, zero primitive
  uploads, and 60.0001 Hz.
- Mixed-state smoke: 108,671 retained-visual and 162,600 primitive-paint
  requests, 3,523/3,523 matches, zero fallback, and the same authoritative
  Phase 4a equivalence/screenshot hashes. Its 24.4014 Hz is diagnostic
  double-rebuild throughput, not a new baseline.

The sustained randomized gate then ran 8,760 additional 20-of-1,000 batches at
59.9999 Hz. All 8,760 matched with zero primitive, text, or line-plot mismatch,
zero ordinary/stale fallback, and zero primitive upload bytes. Together with
the two earlier 660-batch randomized locality probes, Phase 4 now has **10,080
randomized targeted-versus-full comparisons** and closes the 10,000-batch gate.
The sustained run's equivalence hash was
`7592f7cdd6adf56e14ff6a440ced0d9d9fc26ebead5901c1f0c4e3361aa3da85`
and screenshot hash was
`927cbe66454678be2a80a704d3d365a53ecd02336752d59479cb0538f7343d68`.

Complete regressions passed: Rust **922 passed / 13 ignored** and Python **568
passed**.

```text
artifacts/gui-update-pipeline-phase4d-typed-targets-locality-smoke-v1/summary.json
artifacts/gui-update-pipeline-phase4d-typed-targets-mixed-smoke-v1/summary.json
artifacts/gui-update-pipeline-phase4d-randomized-10000-gate-v1/summary.json
artifacts/live-update-phase4d-typed-targets-wheel/dragongui-0.1.0-cp312-abi3-win_amd64.whl
artifacts/live-update-phase4d-typed-targets-runtime
```

The wheel SHA-256 is
`512286b80e0289e09718acbaad65725bd0534744de51cde8eb94b3a786e5afaf`;
the extracted `_dragongui.pyd` SHA-256 is
`01687d1fc026acd28006a8c15f48b0b124100d6c59feaddc19fec4a9fdeb41f2`.

The fifth and final Phase 4 increment adds a matched runtime A/B harness that
alternates control/candidate order, launches every sample in a fresh process,
validates each report, and requires exact retained-state and screenshot hashes
within every pair. Compatibility support lets pre-typed-target control runtimes
omit the newer per-class diagnostics while candidates must provide them.

The Phase 4b pre-typed runtime and Phase 4d typed-target runtime were compared
on this workstation with the intrinsic-text 200-widget full-fallback workload,
batched optimized updates, two-second warmups, 20-second measurements, 60 Hz,
five repetitions, and screenshots. All ten samples validated. Candidate medians
versus control were:

| Metric | Control | Candidate | Candidate change |
|---|---:|---:|---:|
| Throughput | 16.9501 Hz | 17.5499 Hz | 3.54% improvement |
| Command-drain p95 | 92.9661 ms | 89.9343 ms | 3.26% improvement |
| Rebuild-flush p95 | 91.7768 ms | 88.7737 ms | 3.27% improvement |

The gate therefore passed with no material full-fallback regression. Every
sample had equivalence hash
`847172609b9769bba6bce7ee07166459b0cd3a9525178b424e6a68adb2b20040`
and screenshot hash
`1ab6bfb6a0b3caeaae6e0ab218a8198518c550651338d3893db4729bbfb1bd38`.
The final Python regression suite reports **569 passed**; the three benchmark
scripts also pass bytecode compilation.

```text
artifacts/gui-update-pipeline-phase4e-full-fallback-ab-v1/summary.json
```

This closes Phase 4. The next implementation work is Phase 5 command
coalescing-key, barrier, and merge-rule centralization.

Phase 5 orientation has started. The audit found two independent semantic
implementations:

- `native/src/commands.rs` has a private `command_coalesce_key(...)` plus
  `merge_replaced_command_flags(...)` used by queue insertion.
- `native/src/runtime.rs` has a reverse drain-time pass with separate maps and
  sets for properties, plots, scatter resources, theme/style mutations,
  extension display lists, and icon themes, followed by a distinct adjacent
  line-append merge.

The first bounded Phase 5 change should establish a shared command semantic
descriptor (key, barrier class, and merge policy), lock current behavior down
with table-driven and randomized reference tests, and only then switch the queue
and runtime passes to it. `SetProps`, structural replacement, snapshot/request
commands, non-coalesced data updates, and sticky `fit`/`auto_fit` flags are the
explicit boundary cases for that increment.

Phase 5a now completes the shared-contract portion for the command families
already coalesced by both layers. `Command::coalescing_key()`,
`Command::merge_replaced(...)`, and `Command::coalescing_barrier()` are the
single definitions used by queue insertion and drain-time batching. The shared
merge preserves sticky scatter/line `fit` and histogram `auto_fit` flags, while
`coalesce=false`, `SetProps`, append commands, and other lossless commands have
no replacement key.

Structural replacements and request/response observations are now explicit
segment barriers. In particular, `DebugSnapshot` no longer permits a later
property or scatter update to supersede an earlier update before the snapshot;
the snapshot observes the documented prefix. `ReplaceNode` and
`ReplaceChildren` conservatively end the current coalescing segment in both the
queue and runtime pass, preserving updates on both sides of the structure
change. New contract, queue, and runtime tests cover key classification, sticky
merges, snapshot boundaries, and structural boundaries. Complete native
regressions pass: **926 passed / 13 ignored**.

Phase 5b extends the shared contract to the former runtime-only replacement
families. Extension display lists and icon themes now have shared keys, so the
linked queue can release obsolete payloads before drain and the runtime reverse
pass no longer owns separate tracking sets. Additive queue diagnostics report
`extension_display_list` and `icon_theme` replacement counts.

Adjacent line-plot append compatibility and payload concatenation now live in
`Command::try_merge_adjacent(...)`; the drain pass invokes that shared policy
without cloning payload bytes. `PropUpdate` also exposes the same property key
used by `SetProp`. Drain-time `SetProps` processing walks each packet from its
latest update backward, removes superseded widget/property writes within and
across packets, restores surviving packet order, and preserves the packet as a
single command. Structural and observation barriers clear the shared property
segment, so packet updates on both sides remain observable.

Focused tests cover compatible/incompatible append merges, queue-level display
list and icon-theme replacement plus family counters, property packet
deduplication, and snapshot preservation. Complete native regressions pass:
**930 passed / 13 ignored**.

Remaining Phase 5 work includes callback/event barrier classification, adapting
the Phase 2 randomized reference model to barrier segment identities, adding
queue/packet/drain differential coverage, and completing received/applied/
superseded/merged/barrier diagnostics. A fresh release runtime benchmark should
follow those semantics and diagnostics rather than benchmarking this partial
increment in isolation.

Phase 5c makes the old Phase 2 randomized differential barrier-aware. Snapshot
and generic lossless/callback commands now start new segments in the reference,
stable-slot, linked-slot, and generational models. Stable-slot compaction
reconstructs only the latest segment's key map; the generational candidate uses
`SegmentedCoalesceKey` so payloads before a barrier remain live without being
eligible for replacement afterward. The seeded gate passes **2,000 seeds x 200
commands = 400,000 generated commands**, including bounded partial drains,
across all four models.

`DrainPythonTasks` is now a shared `Callback` barrier. Queue insertion and
drain-time batching preserve property, scatter, and scalar-bar commands on both
sides rather than coalescing across Python callback execution. An outdated
scalar-bar test exposed by the full suite was updated to this documented
contract.

Diagnostics now add:

- queue `barrier_segments.total` and structural/observation/callback counts;
- existing queue replacement counts by command family, including the Phase 5b
  display-list and icon-theme families; and
- cumulative runtime `command_drain.coalescing` counts for commands received,
  retained, superseded, and merged; property updates received, retained, and
  superseded; and barrier segments.

Focused tests assert callback segmentation and exact packet/append accounting.
Complete native regressions pass: **930 passed / 13 ignored**.

Remaining Phase 5 work is per-family runtime superseded/merged attribution and
the fresh current-machine release-wheel correctness/performance gate. Native
events use the separate event-loop path rather than `Command`, so there is no
additional queued event command to classify in this contract.

Phase 5d closes the remaining command-semantics work. Runtime coalescing now
attributes received, retained, superseded, and merged commands to a fixed
12-family enum: property, property packet, theme, stylesheet, scatter points,
line plot, histogram, scatter scalar bar, scatter actor, extension display
list, icon theme, and line-plot append. The hot path stores these counters in a
fixed array and only materializes the family-name object for debug snapshots.
`Command::merge_replaced(...)` reports whether a sticky merge occurred, so
merged attribution uses the same policy that performs the merge.

The first release candidate used a `BTreeMap` on the drain hot path. Its
ordered-barrier A/B was semantically exact but failed the 5% gate with 11.50%
command-drain overhead. That measured intermediate is retained at:

```text
artifacts/gui-update-pipeline-phase5d-ordered-barrier-ab-v1/summary.json
```

Replacing the map with the fixed array removed the regression. The
authoritative current-machine A/B runs compare the Phase 4d release runtime
with the fixed-array Phase 5d runtime. Every sample used a fresh process, two
seconds of warmup, 20 seconds of measurement, a 60 Hz target, five alternating
repetitions, screenshots, and a 5% maximum-overhead gate. All 20 samples across
the two workloads validated and matched retained-state and screenshot hashes.

| Workload / metric | Phase 4d control | Phase 5d candidate | Candidate change |
|---|---:|---:|---:|
| Ordered barrier throughput | 59.9993 Hz | 59.9998 Hz | 0.0009% improvement |
| Ordered barrier drain p95 | 3.6116 ms | 3.4999 ms | 3.09% improvement |
| Ordered barrier rebuild-flush p95 | 3.0532 ms | 2.9291 ms | 4.06% improvement |
| Mixed-state throughput | 59.8982 Hz | 59.8998 Hz | 0.0027% improvement |
| Mixed-state drain p95 | 15.7334 ms | 15.5791 ms | 0.98% improvement |
| Mixed-state rebuild-flush p95 | 1.0385 ms | 1.0152 ms | 2.24% improvement |

Ordered-barrier samples share equivalence hash
`bb6c089af9966f1a3a06e2c40159db7f13369145d1b29a175b65118b43452c2f`
and screenshot hash
`0641bc4e82a0e948194f891c5e59e08aed7af8c14dac309f7d60d8efe733fde4`.
Mixed-state samples share equivalence hash
`411e52f18d7d6f06ced7540e8f5e756e506ca58bdf0cb806b33bed5c79f4ae5d`
and screenshot hash
`917013df2102f401dda28de5fc45f7e3444b8514905fc66a191a14aeddc43ac9`.

A representative ordered-barrier candidate snapshot attributes 2,640 received,
1,320 retained, and 1,320 superseded commands to `property_packet`; its nested
property-update accounting reports 54,120 received, 27,720 retained, and
26,400 superseded updates. Focused sticky-scatter attribution is covered by a
native test. Complete native regressions pass: **931 passed / 13 ignored**;
`cargo fmt --check` and `git diff --check` also pass.

```text
artifacts/gui-update-pipeline-phase5d-ordered-barrier-ab-v2/summary.json
artifacts/gui-update-pipeline-phase5d-mixed-state-ab-v1/summary.json
artifacts/live-update-phase5d-command-semantics-wheel/dragongui-0.1.0-cp312-abi3-win_amd64.whl
artifacts/live-update-phase5d-command-semantics-runtime
```

The release wheel SHA-256 is
`0a03445e6c48f94f19c70de1bb1f600da68495e21f5af62c8b33a11cf4bc93e8`;
the extracted `_dragongui.pyd` SHA-256 is
`527c468b3e4558960641718099610b5ced3f06f1f99f612703db5196d5846e4c`.
The wheel does not currently carry `WebView2Loader.dll`; the benchmark runtime
therefore includes the repository copy beside the extracted extension. Direct
native import passed after that packaging dependency was supplied.

This completes Phase 5: queue-level, packet-level, and drain-level processing
now share command keys, barriers, merge rules, randomized semantic coverage,
per-family diagnostics, and a passed current-machine release A/B gate.

Phase 6a starts developer-safety work without adding another scheduling API.
The orientation audit found that `App.call_soon_threadsafe(...,
coalesce_key=...)` already provides the proposed latest-state abstraction, is
thread-safe, retains only the newest pending callback at its latest ordered
position, has a validated batching example, and is covered in `dg.help`.
Adding `latest_updates(...)` now would be a second name for the same scheduler
rather than a distinct safety mechanism.

The missing guardrail was unkeyed/lossless backlog growth. `AppHandle` now
tracks pending and high-water unkeyed task counts separately from the total
queue. At 256 pending unkeyed callbacks it emits an actionable `RuntimeWarning`
that explains FIFO/lossless semantics and recommends a stable `coalesce_key`
only for replaceable snapshots. Further warnings occur only when the backlog
reaches doubling thresholds (512, 1,024, and so on), bounding warning volume.
Debug snapshots add:

```text
runtime.python.unkeyed_tasks_pending
runtime.python.unkeyed_task_queue_high_water
runtime.python.task_queue_growth_warnings
runtime.python.next_task_queue_warning_at
```

Focused tests lower the threshold deterministically and prove exact warnings at
4 and 8 pending callbacks, counter cleanup after drain, and no warning for
1,000 submissions sharing one latest-state key. That healthy control keeps
total task queue high-water at one and executes only update 999. `dg.help` now
contains an explicit latest-state-versus-lossless table and documents the
warning/counter contract. The batched telemetry example's completion callback
is now deliberately unkeyed, preserving the final keyed telemetry snapshot
before the lossless completion event. The complete Python regression suite
passes: **571 passed**; bytecode compilation and `git diff --check` pass.

Phase 6b adds `examples/live_update_producer_safety_probe.py`, a live-runtime
gate that deliberately overloads the latest-state path while placing a marked
lossless event after every burst. Each of 100 bursts submits 100 snapshots under
one stable key, then one unkeyed event and waits for that event before the next
burst. The probe rejects stale Python or native state, a missing/reordered
event, Python queue high-water above two, unkeyed high-water above one, warning
emissions, Python/native queue residue, worker failure, or failure to coalesce.

The authoritative v2 run passed all 12 checks:

- **10,000** keyed snapshot submissions became 100 applied snapshot callbacks;
  **9,900** obsolete callbacks were coalesced.
- All **100** unkeyed lossless events ran in order.
- Python and native retained state ended at snapshot **9,999** and events
  **100**.
- Python task queue high-water was **2**; unkeyed high-water was **1**.
- Python and native queues ended at depth **0**, with zero warning emissions.
- The native linked queue peaked at three physical slots, ended with zero live
  or stale entries, and recorded 100 callback plus one observation barrier.

```text
artifacts/live-update-phase6b-producer-safety-v2/report.json
artifacts/live-update-phase6b-producer-safety-runtime
```

The report SHA-256 is
`5f7b2f6bbc60f2080dcb6bac896430c6201ef67053ca06149a7efca371996517`.
The runtime reuses the validated Phase 5d native extension, SHA-256
`527c468b3e4558960641718099610b5ced3f06f1f99f612703db5196d5846e4c`,
with the current Phase 6 Python layer; its `runtime.py` SHA-256 is
`3cb4c564f0a6da18051928eabec2da35b1188413a6811be9cb1993b927ca8895`.

The native-warning evaluation found that the command bridge already exposes
depth, logical/physical high-water, replacement-family counts, stale entries,
and barrier segments, while the linked-slot queue bounds replaceable state.
This gate's native high-water was only three. A second stderr warning is
therefore deferred until the Phase 7 scenario matrix demonstrates sustained
lossless native growth; without such evidence it would duplicate the Python
warning and lack actionable producer-origin context.

The complete Python regression suite still passes: **571 passed**; probe/manual
bytecode compilation and `git diff --check` pass. Remaining Phase 6 work is the
broader live-plot/stress-example and threading-guidance audit, including whether
expensive-callback supersession needs a distinct warning backed by evidence.

Phase 6c completes that audit. An AST inventory covered every
`call_soon_threadsafe` call in active examples, legacy examples, and the Python
runtime. Replacement workloads now consistently use stable keys:

- live histogram snapshots and scatter-camera targets;
- streaming scatter frames in the focused and all-features demos;
- scatter performance frames and diagnostic/stat snapshots;
- thread-monitor scatter/counter snapshots; and
- the built-in thread monitor's own refresh callback.

The active example set now contains eight explicitly keyed replacement sites
and 13 intentionally unkeyed sites. Every remaining active unkeyed site is a
plot/log append stream, marked lossless event, ordered benchmark action,
one-shot launch/completion callback, or intentional task-failure probe. Theme
Forge and CATHODE were already correct: their plot/log streams remain unkeyed
while their gauge/label/LED snapshots are keyed. Runtime unkeyed paths are
one-shot app/dialog lifecycle work or the legacy scatter callback handoff whose
documented purpose is scheduling every payload.

Public guidance is now consistent across `README.md`,
`docs/library-overview.md`, `docs/widgets-reference.md`,
`docs/sphinx/live-updates.md`, and `dg.help.live_updates.threads()`. Each
distinguishes keyed replaceable snapshots from unkeyed FIFO/lossless work. The
scatter prepared-frame and progress examples use stable keys; append examples
remain unkeyed. `dg.help` links the batching demo, validated producer-safety
probe, and both split-stream stress workloads.

Regression tests require all four standalone guides to contain `coalesce_key`
and lossless guidance, verify the built-in thread monitor uses
`dragongui.thread-monitor.latest`, validate manual example paths/call shapes,
and compile every changed Python example. The complete Python suite passes:
**573 passed**; `git diff --check` passes.

This completes Phase 6. No separate `latest_updates(...)` wrapper was added
because the existing stable-key API already provides the exact abstraction and
is now validated and consistently taught. An expensive-callback supersession
warning remains deferred unless Phase 7 evidence identifies that distinct
failure mode; the current actionable backlog warning and family diagnostics
cover the demonstrated problems without speculative noise.

Phase 7a starts integrated validation with a scenario/probe inventory and the
required per-change smoke tier. Existing coverage maps as follows:

| Phase 7 scenario | Existing executable coverage |
|---|---|
| Dashboard low/medium/high | `gui_live_dashboard_case.py` and its matrix runner |
| Labels-only and mixed-state | `gui_update_pipeline_case.py` and matrix runner |
| Theme/CSS, resize, responsive viewports, client/native decorations | Theme Forge autopilot and focused CSS/layout probes |
| Scroll, focus, selection, tabs, overlays, tooltips, drag/drop | update-pipeline interaction probes plus focused feature probes |
| Malformed/stale inputs and queued shutdown | native/Python regressions, Theme Forge malformed cases, and queue models |
| Lossless trace/log streams | CATHODE and Theme Forge split-stream workers |

The inventory found one automation gap: Theme Forge already provides bounded
autopilot/report/exit controls, but CATHODE has no bounded autopilot or
machine-readable report. Adding that gate is the next Phase 7 increment.

The current Phase 6 isolated runtime then ran the short release-tier matrix on
this workstation: fresh process per sample, two-second warmup, five-second
measurement, 60 Hz target, batched updates, typed diagnostics required, and
screenshots.

| Scenario | Throughput | Dropped ticks | Drain p95 | Apply p95 | Flush p95 |
|---|---:|---:|---:|---:|---:|
| Labels fixed, 200 | 59.8036 Hz | 1 | 4.4818 ms | 4.3607 ms | 0.1151 ms |
| Mixed state, 200 | 59.9975 Hz | 0 | 17.4457 ms | 15.8522 ms | 1.0147 ms |

Both samples completed tick 419 in Python and native retained state and passed
all 30/36 validations. They had zero layout diagnostic failures, zero targeted
fallbacks, zero Python/native queue residue, zero stale native entries, and
zero Python queue-growth warnings. Python task queue high-water was one in both
runs; native queue high-water was one for labels and two for mixed state.
Equivalence/screenshot hashes were respectively:

- labels:
  `44a47a1c2a21b1bc07022d7df62fd6bd725927faea43d742500b7e969410ca8b` /
  `027b3bb623bc7f1d6f1aee1aea248993da4b9352954532c972054f147c96b79e`;
- mixed state:
  `4fac31afc112d6606cff451a3993069c17af912197d926741431961be10cadaf` /
  `d8ddc42a8c8bcec8f6d1f0cada3f66cc030159a1b1568d02aacb38606190590b`.

```text
artifacts/live-update-phase7a-per-change-matrix-v1/summary.json
```

The summary SHA-256 is
`b3271b78bebe9c7aa0e95d9b3e09bf15d4caf54637875db10634461108d41452`.
These are current-machine smoke results, not comparisons with July 31 absolute
timings.

Phase 7b closes the CATHODE orchestration gap. The stress demo now accepts
`--validate-seconds`, `--validation-timeout`, and `--report`, waits for runtime
readiness, stops its producer after the bounded interval, waits for both queues
and the final keyed snapshot to settle, writes a JSON report, and exits with a
pass/fail status. Its instrumentation separately records produced ticks,
unkeyed plot/trace stream ticks, lossless log ticks, and keyed snapshot ticks.
The validation compares the final lane value in Python and the native retained
tree, checks exact lossless retention, queue drainage, warning count, and worker
shutdown. Normal interactive behavior is unchanged. Two focused Python tests
cover the bounded CLI and native-tree/final-value helpers.

The authoritative current-machine run used the full default workload (96
tiles, 480 table rows), ten live seconds, a 20-second readiness/drain timeout,
and the isolated Phase 6b release runtime. All 11 checks passed. It produced 36
ticks and retained all 36 unkeyed stream updates plus all five expected log
events. Thirty-two keyed snapshots executed and four were safely coalesced;
the final tick was 275 and both Python and native state were `78%`. Python task
queue high-water was six (unkeyed high-water five), native high-water was 30,
queue-growth warnings were zero, and both queues drained to zero. The focused
benchmark-validation suite passes **10 tests**, the complete Python suite
passes **575 tests**, and bytecode/diff checks pass.

```text
artifacts/live-update-phase7b-cathode-validation-v1/report.json
```

The report SHA-256 is
`8073864791ed208c4135d9c111bad782a203cfb40fe41e948925626aab1a03ab`.
The exercised script SHA-256 is
`aeea4a4e06b3abe6c9f33c0929aac2755466839b309652c95a8a5a9667218b37`.
The isolated runtime remains `runtime.py`
`3cb4c564f0a6da18051928eabec2da35b1188413a6811be9cb1993b927ca8895`
plus native extension
`527c468b3e4558960641718099610b5ced3f06f1f99f612703db5196d5846e4c`.
Next run Theme Forge and CATHODE together as the bounded autopilot tier, then
continue through the remaining Phase 7 interaction/decorations matrix and
extended soaks.

Phase 7c completes that paired autopilot tier. Theme Forge now honors
`DRAGONGUI_BENCH_PYTHON_PATH`, creates missing report-parent directories, and
joins its background workers during shutdown. The first two-cycle attempt
passed both integrity cycles but exposed the missing-directory exception and a
non-terminating process; it was rejected as evidence. After the orchestration
fix, the same two-cycle, 0.32-second-settle run completed in 187 seconds, wrote
its report, and exited zero against the same isolated runtime. Both cycles had
zero retained-state regressions, zero layout issues, zero bad viewports, zero
bad pages, intact cascade recovery, and no final stylesheet error. Together
with Phase 7b's 11/11 CATHODE result, both required stress autopilots are now
bounded and green.

```text
artifacts/live-update-phase7c-combined-autopilots-v1/theme-forge-report.json
artifacts/live-update-phase7b-cathode-validation-v1/report.json
```

The Theme Forge report SHA-256 is
`cd08bf074841f21f73f578bd98cfa7d1a67542d67922f3fb424e4462249acbf2`;
the exercised script SHA-256 is
`d4904da7537ed0d52d086ddab371a4316737589178b5817204285cfd758959c8`.
Next complete the remaining interaction/decorations scenario matrix, then run
the extended soaks and final comparative report.

Phase 7d adds current-machine interaction and decoration evidence. The visual
audit runner now honors `DRAGONGUI_BENCH_PYTHON_PATH` in both its parent and
generated wrapper and creates wrappers in a dedicated clean temp directory.
This fixed a real harness failure caused by an unrelated `inspect.py` in the
general Windows temp directory shadowing the standard library. The rejected v1
client-chrome artifact records that block; v2 is the authoritative result.

The v2 client-decoration audit passed three fresh, outcome-checked states with
native screenshots and snapshots: keyboard focus traversal across app/titlebar
controls, Enter/Space maximize-restore with Win32 state assertions, and
Alt+Space system-menu open/close. A one-cycle live Theme Forge run with native
OS decorations also passed: zero retained-state regressions, layout issues, bad
viewports/pages, or final stylesheet error, with intact malformed-CSS recovery
and a clean exit.

The concurrent `state-controls:8` batch gate used a two-second warmup and
five-second measurement at 60 Hz. It passed all **62 checks** at 59.9872 Hz
with zero dropped ticks, completed tick 419 in Python/native state, retained
focus and caret geometry, preserved selected values and follow/static scroll
semantics, captured a screenshot, and drained the native queue to zero. All 99
concurrent interaction samples completed (p95 0.8117 ms); queue high-water was
two and there were zero layout failures. Focused visual-audit, headless Theme
Forge, drag/drop payload, and decoration contracts pass **40 tests**; the
complete Python suite passes **575 tests**.

```text
artifacts/live-update-phase7d-interaction-decoration-matrix-v2/report.json
artifacts/live-update-phase7d-interaction-decoration-matrix-v2/theme-forge-native-report.json
artifacts/live-update-phase7d-state-controls-v1/summary.json
```

Their SHA-256 values are respectively
`66e4a8af523518be7ce06388cb8ecd3de3b2ecaf63c2d0c7867d390ad752b4dd`,
`e0e93711927d6c6adef63f64e3bb14c142fa11237f96d2909d03c805c3e0ebe5`,
and `b680c92e0d844c81de55375313ba169db253806491a49e9aee88b59adbd11f19`.
Physical title dragging/resizing/minimize/close and a pointer-driven drag/drop
gesture remain platform/manual rows; automated retained-state and dispatch
contracts are green. Next address those residual rows where automation is
practical, then begin extended queue/memory soaks.

Phase 7e automates the pointer-driven drag/drop row. The visual-audit action
language now supports `drag:#source->#target` using a real Win32 left-button
press, 12 interpolated `WM_MOUSEMOVE` steps with the button held, and release
over the target. `assert-text:#widget=value` then checks the outcome in the
native retained tree. The drag/drop probe gives its first source and result
label stable IDs, and its manifest state is now automated rather than manual.

The isolated-runtime live audit dragged `Sensor A` into the compatible asset
lane, dispatched the Python `on_drop` callback, and verified native retained
text exactly equal to `Asset lane: Sensor A (asset)`. It passed with a native
window screenshot and debug snapshot. Focused action/drag contracts pass **37
tests**. Disabled-source and incompatible-target rejection remain covered by
API/dispatch contracts. Physical titlebar movement, edge/corner resize,
minimize, and close remain explicit platform-manual rows because they move,
hide, or destroy the audit window; automated maximize/restore, focus traversal,
system-menu, both decoration modes, and application drag/drop are green. The
complete Python regression suite passes **576 tests**.

```text
artifacts/live-update-phase7e-pointer-interactions-v1/report.json
```

The report SHA-256 is
`88c5d3879bcb24260484432807b05499fa30872702c865472643ab2dc6ab83f6`.
The exercised harness, probe, and manifest SHA-256 values are respectively
`56222c310a8df8efb415a801b591dcbc12ea0625831fa56ae0500757de3a2eeb`,
`25aace7dbeb544aebaad19b3c16447ef23c282ad44e41437d9af1c2015dac381`,
and `d455d872a4c809f40303cb4c408f0d998356174719bc1818095332dabc42d095`.
The next automated work is the first extended high-load queue/memory soak.

Phase 7f completes the first required 10-minute high-load soak on this
computer. `run_gui_live_dashboard_matrix.py` now exposes the case runner's
existing `--update-mode` option so soak artifacts explicitly record whether
DragonGUI used individual or batch property transport. The authoritative run
used a fresh process, the Phase 6b isolated runtime, batch mode, five-second
warmup, and 600.0001 measured seconds with the full high workload: six 50k-point
line series, 50k scatter points, a 128x128 heatmap, and 200 live labels.

All **23 checks** passed. The run completed 13,858 measured generations at
23.0950 Hz and dropped/coalesced 22,142 scheduled generations under overload;
the final producer tick was 36,299. Python coalesced 22,324 obsolete tasks,
held total queue high-water to one (unkeyed zero), emitted zero growth warnings,
and drained to zero. Native queue high-water/physical peak were 40, with 1,674
replacements, zero stale entries, and zero final depth. Drain recovery took
610.94 ms. Command-drain p95 was 45.2838 ms, and process CPU averaged 96.09% of
one core, identifying command application as the current high-load limit.

RSS started at 245.97 MiB, peaked at 264.84 MiB, and ended at 263.05 MiB:
17.08 MiB whole-run growth or 1.708 MiB/minute. Only 1.00 MiB accumulated in
the second half of the 606 checkpoints, so this first sample is consistent
with warm allocation followed by a plateau rather than sustained linear
growth. Two more independent 10-minute samples are still required before
accepting that memory conclusion. Absolute throughput is current-machine
evidence only; the unexpectedly low 23.1 Hz will be compared across the next
samples and investigated before the final report.

```text
artifacts/live-update-phase7f-high-soak-1-v1/summary.json
artifacts/live-update-phase7f-high-soak-1-v1/raw/high-dragongui-1.json
```

Summary/raw SHA-256 values are respectively
`6d869e985685c4ad43cfea1a523052fd7ef36d4449f0661dddfc81d700344e68` and
`1802e00899502f40cdf53a8024fc6bed99d6c21f8addcef82cf3a4b2e6111f9f`.
The exercised matrix runner SHA-256 is
`03609437220c529041c4a803f9de97a9764eae1933d4c9bd6109439b350b49e3`.
Next run the second independent 10-minute high-load sample with the same
configuration and compare its second-half RSS slope and drain distribution.

Phase 7g completes high-load soak 2 of 3 with the identical fresh-process,
isolated-runtime, batch-mode, five-second-warmup, 600-second configuration. All
**23 checks** again passed. It completed 18,591 measured generations at
30.9850 Hz, dropped/coalesced 17,409 scheduled generations, and reached the
same final producer tick 36,299. Python queue high-water remained one (unkeyed
zero), warnings remained zero, 17,565 obsolete tasks were coalesced, and the
queue drained to zero. Native high-water/physical peak again stayed exactly 40,
with zero stale entries and zero final depth; drain recovery was 394.13 ms.

This repeat confirms correctness and bounded queues but exposes substantial
performance variance. Versus soak 1, throughput improved 34.16% (23.0950 to
30.9850 Hz) and command-drain p95 improved 24.41% (45.2838 to 34.2311 ms),
while CPU remained saturated at 100.95% of one core. Native replacements rose
from 1,674 to 28,980 as the faster run admitted more same-frame work; this did
not increase queue high-water or leave residue.

RSS started at 246.75 MiB, peaked and ended at 265.74 MiB, and grew 18.99 MiB
(1.899 MiB/minute). Quarter deltas were +15.21, +1.50, +0.05, and +2.23 MiB;
the second-half delta was +2.84 MiB. Both samples therefore show most growth in
the first quarter and identical near-flat third quarters, but soak 2 had a
late allocation step. This remains compatible with bounded/episodic allocation
rather than proven linear growth, and soak 3 is required to resolve it.

```text
artifacts/live-update-phase7g-high-soak-2-v1/summary.json
artifacts/live-update-phase7g-high-soak-2-v1/raw/high-dragongui-1.json
```

Summary/raw SHA-256 values are respectively
`0c8f72f0c03fa70c5a26b670d0556bfd218ff5bf30cb9b0bedea7084eab9e18c` and
`44f1ecd1731891c421ab1e72ec0096e2b2c6ccbb93640e45f8cd2d08eae177ad`.
Next run the third identical 10-minute sample, then aggregate all three before
deciding whether memory is acceptably bounded and investigating the throughput
spread.

Phase 7h completes soak 3 and the three-soak aggregate. Soak 3 again passed all
**23 checks**, completed exact final tick 36,299, and drained both queues with
zero warnings or stale entries. It ran at 29.0316 Hz with 41.0441 ms
command-drain p95 and 438.74 ms recovery. Python/native queue high-water stayed
one/40 for the third time. RSS grew from 245.04 to 267.31 MiB (+22.27 MiB,
2.227 MiB/minute); quarter deltas were +16.86, +0.57, +1.49, and +3.35 MiB, so
second-half growth was +4.84 MiB.

Across all three fresh 10-minute samples, **69/69 checks passed**, warnings,
stale entries, and final queue residue were zero, and maximum Python/native
high-water was consistently one/40. Queue correctness and boundedness therefore
pass this gate. Throughput median/range were 29.0316 Hz and 23.0950–30.9850 Hz
(34.16% max-over-min spread); command-drain p95 median/range were 41.0441 ms and
34.2311–45.2838 ms. RSS-growth median/range were 18.99 MiB and 17.08–22.27 MiB;
second-half median/range were 2.84 MiB and 1.00–4.84 MiB.

Memory/performance stability is **not yet accepted** for the 60-minute gate.
Retained output remained stable in every sample (617 widgets, six series,
300,000 line source points, 1,976 rendered line points, and 29 final text
entries), which rules out obvious retained-tree/resource accumulation. The
leading signal is transient text churn: 5.38–7.41 million layout-text cache
misses, 328–452 cache-capacity clears, and 18,483–24,192 glyph-atlas trims.
Those counters rise with completed generations while retained counts remain
fixed. Investigate text-measurement cache and atlas allocation/reclamation
before committing an hour to the release soak.

```text
artifacts/live-update-phase7h-high-soak-3-v1/summary.json
artifacts/live-update-phase7h-high-soak-3-v1/raw/high-dragongui-1.json
artifacts/live-update-phase7h-high-soak-aggregate-v1/summary.json
```

Soak-3 summary/raw SHA-256 values are
`07ce03901af4499579749a7e9e309f8eb68ae321e56580a18f0bd28f7b898a73` and
`0d7d5f85bb64c814665f577bc12f7e873bbc517e8485b2d40b35748f8007f0e8`.
The aggregate SHA-256 is
`ac31598a873e5c70ea68eb2fc284fda2698c1e68b371da933d90019dc20c1208`.
Next attribute cache/atlas churn and run a short controlled verification before
the 60-minute soak.

## Later phases

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
> with Phase 7 integrated validation and rollout. Phase 7a's fresh-process
> labels/mixed per-change matrix passed, Phase 7b added and passed the bounded
> CATHODE report/exit gate at the full 96-tile/480-row workload, and Phase 7c's
> repaired Theme Forge two-cycle autopilot passed and exited cleanly. Phase 7d
> passed the stateful client/native-decoration and live state-controls matrix;
> Phase 7e added and passed real pointer-driven application drag/drop. Physical
> titlebar move/resize/minimize/close remain platform-manual rows. Next begin
> text-cache/atlas churn before the 60-minute soak. All three 10-minute soaks
> passed 69/69 checks with queue high-water one/40 and clean drains, but
> throughput ranged 23.1–31.0 Hz and second-half RSS growth 1.0–4.84 MiB.
> Retained counts were fixed while cache clears and atlas trims accumulated.
> Run a short controlled attribution gate before the hour-long soak. Phase 6
> is complete: Phase 6a added
> rate-limited unkeyed Python task-backlog warnings, snapshot counters, and an
> explicit latest-state-versus-lossless `dg.help` contract. Phase 6b's live
> 10,000-snapshot/100-event gate passed with queue high-water two, exact final
> native state, every event retained, and zero Python/native residue. Phase 6c
> completed the call-site and public threading-guide audit. Phases 4 and 5 are
> complete: typed target classes, the 10,000-batch randomized differential gate
> (10,080 matches), and the matched full-fallback overhead gate all passed. The current-machine v2
> baseline, structure-generation guard, verifier, and locality controls are
> complete. Phase 5 has shared queue/packet/drain command semantics, passed the
> 400,000-command randomized barrier gate, and passed current-machine ordered-
> barrier and mixed-state release A/B gates with exact hashes and no regression.
> Do not compare absolute
> timings against the slower July 31 machine. Phase 3 is complete:
> the authoritative five-by-60-second gate under
> `artifacts/gui-update-pipeline-phase3-fixed-label-release-v2/summary.json`
> passed exact equivalence, zero layout, zero fallback, and measured a 99.05%
> median rebuild-flush p95 reduction. Preserve the explicit state, plot, HTML,
> and icon routes and use differential verification for each locality change.

## August 1 progress — refreshed three-library live-dashboard report

A fresh current-machine comparison now covers DragonGUI, Dear PyGui, and
PyQtGraph under the same dashboard workloads as the July 31 report. Each of
the nine configurations ran in a fresh process with a five-second warmup and a
60-second measurement. Framework order rotated by load. DragonGUI used keyed
latest-frame producer coalescing and the optimized batch property transport.

The benchmark environment is project-local and reproducible:

- portable CPython 3.12.10 from the official Python NuGet package;
- Dear PyGui 2.3.1;
- PyQtGraph 0.14.0 and PyQt6 6.11.0;
- NumPy 2.5.1;
- a fresh MSVC release wheel built from the current source.

All nine samples passed, with 99/99 correctness checks and zero failures. The
matrix took 598.4 seconds and produced these headline results:

| Load | DragonGUI | Dear PyGui | PyQtGraph |
|---|---:|---:|---:|
| Low | 60.000 Hz | 60.000 Hz | 59.983 Hz |
| Medium | 59.966 Hz | 59.999 Hz | 23.384 Hz |
| High | 28.049 Hz | 23.964 Hz | 3.017 Hz |

At high load DragonGUI led Dear PyGui by 17.0% throughput and used 251.3 MiB
peak RSS versus 787.7 MiB. PyQtGraph remained lowest-memory at 123.6 MiB but
processed only 3.017 Hz. DragonGUI high-load submit p95 was 11.253 ms and
native frame-work p95 was 0.596 ms, which keeps Python submission/command
application—not GPU frame work—as the limiting path.

Relative to the published July 31 report, DragonGUI changed as follows:

| Load | Throughput | Submit p95 | Frame p95 | Peak RSS |
|---|---:|---:|---:|---:|
| Low | 58.7 → 60.000 Hz | 0.68 → 0.687 ms | 1.23 → 0.821 ms | 335 → 197.9 MiB |
| Medium | 58.3 → 59.966 Hz | 2.61 → 2.147 ms | 0.84 → 0.712 ms | 372 → 219.2 MiB |
| High | 38.1 → 28.049 Hz | 16.27 → 11.253 ms | 0.89 → 0.596 ms | 432 → 251.3 MiB |

Those published-report deltas are context only: the July 31 run used the
slower prior computer and individual property transport, while this run used
the current computer and batch transport. A controlled same-machine attempt
against the preserved July 3 wheel was rejected: even three one-second smoke
cases exceeded a two-minute timeout and did not write a validated summary. The
two timed-out Python processes were explicitly stopped, and no partial legacy
measurements appear in the report.

Artifacts and provenance:

```text
artifacts/gui-live-dashboard-comparison-2026-08-01-v1/summary.json
artifacts/gui-live-dashboard-comparison-2026-08-01-v1/raw/*.json
plans/gui-live-dashboard-performance-report-2026-08-01.html
```

- summary SHA-256:
  `555c19ff3ac8b44a03cb1322db04da601f8496e7c3b2e451c247f81625d3062a`
- report SHA-256:
  `debc0da62a18ac5560fce2c81d2ac3ff17c7ea81b761eb1ec5042c1b85d6e7cc`
- current benchmark wheel SHA-256:
  `7df7b3fd09f8f38c331ca5f29daf236f49d0b9cfa54766c2f5b59558e73bcff7`
- case/matrix/renderer SHA-256 values:
  `72ad2d5390cf7aa9d3defac86b68ac036f1609730328b63a631d151820d08bf6`,
  `03609437220c529041c4a803f9de97a9764eae1933d4c9bd6109439b350b49e3`,
  and `6dc5c4621f4aea3b56e8826f235083f01dddc193c7a21d6e8cb90da73a37522e`.

The case runner now honors `DRAGONGUI_BENCHMARK_DEPS_PATH` before falling back
to `artifacts/benchmark-deps`, allowing ABI-specific isolated dependency
bundles without changing default behavior. The report renderer can recover the
exact table from an earlier HTML report through `--baseline-report`, labels
cross-machine/method deltas as contextual, and no longer requires a missing
uncoalesced-failure artifact. `python -m py_compile` passed for both changed
benchmark scripts, `git diff --check` passed for the touched report/code/plan
files, and `tests/test_gui_benchmark_validation.py` passed 10/10 tests in
0.15 seconds.

This closes the requested refreshed comparative report, but it does not close
the release plan. Next remains the cache/atlas churn attribution gate, a short
controlled verification, the 60-minute release soak, and the final release
report/legacy-switch cleanup.
