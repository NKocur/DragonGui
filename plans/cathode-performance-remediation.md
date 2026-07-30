# CATHODE-7 Performance Profiling and Remediation

**Project:** DragonGui
**Created:** July 28, 2026
**Status:** Priority performance campaign complete; extended profiling deferred
**Primary workload:** `examples/cathode_ops_stress_demo.py`

## Purpose

Use the intentionally oversized CATHODE-7 demo to identify scaling limits in
DragonGUI rather than optimizing the demo until it stops being stressful.
Performance work must be driven by repeatable profiles that distinguish:

- Python widget construction and serialization
- Native startup and first-frame work
- Python task scheduling
- Native command application
- CSS cascade and selector matching
- Layout computation
- Text and primitive rebuilding
- GPU encoding, submission, and presentation
- Queue growth and render progress under sustained producers

The goal is not to make every 1,628-widget application animate at an arbitrary
rate. The goal is to make inexpensive state changes remain inexpensive,
coalesce obsolete work, and expose enough evidence to explain the remaining
cost.

---

## Reproducible Profiling Harness

Added:

```text
tools/profile_cathode_stress.py
```

Examples:

```powershell
py -3.12 tools/profile_cathode_stress.py --frames 10 --no-live
py -3.12 tools/profile_cathode_stress.py --frames 10
py -3.12 tools/profile_cathode_stress.py --frames 20 --output artifacts/performance/cathode.json
py -3.12 tools/profile_cathode_stress.py --frames 10 --disable-live-group scope
```

Each case runs in a fresh process because the native event loop is
process-scoped. The harness records a compact JSON report containing:

- Workload dimensions and native widget count
- Python build and serialization times
- Document size
- Run wall time
- Frame timing stages
- Command queue depth and drain stages
- Highest-cost command types
- Highest-cost framework stages
- Primitive and line-plot renderer statistics
- Loading-screen timings
- Explicitly disabled live-update groups for workload ablation

The full debug tree and layout snapshots are intentionally omitted from the
profile report because they are large and obscure the timing evidence.

The native runtime currently still constructs its full debug snapshot before
the harness compacts it. Consequently, `run_wall_ms` includes end-of-run
diagnostic construction and serialization. Command-stage timings and wall FPS
remain useful, but a lightweight native performance snapshot is still needed
for a clean wall-time benchmark.

---

## Baseline Matrix

All measurements below used the local release extension on the same Windows
development machine. Values are diagnostic baselines, not portable performance
guarantees.

| Case | Widgets | Live | Run wall | Wall FPS | Frame CPU work | Drain total | End queue |
|---|---:|---:|---:|---:|---:|---:|---:|
| Default static | 1,628 | No | 4.94 s | 51.4 | 3.21 ms | 0.18 s | 0 |
| Default live, before rebuild batching | 1,628 | Yes | 9.05 s | 3.10 | 1.65 ms | 5.24 s | 419 |
| Reduced static | 1,052 | No | 3.69 s | 32.1 | 4.50 ms | 0.14 s | 0 |
| Reduced live, before rebuild batching | 1,052 | Yes | 6.62 s | 3.20 | 1.65 ms | 3.42 s | 251 |
| Default live, after rebuild batching | 1,628 | Yes | 5.54 s | 10.6 | 1.61 ms | 0.90 s | 24 |
| Default live, kind-aware LED invalidation | 1,628 | Yes | — | 10.3 | 1.68 ms | 0.92 s | 0 |
| Default live, paint-only scope updates | 1,628 | Yes | — | **83.8** | 3.13 ms | **0.13 s** | **0** |
| Default live, paint-only scope, 60 frames | 1,628 | Yes | — | **63.5** | — | **0.14 s** | **0** |

### Live-Group Ablation

The profiler can independently disable `plot`, `scope`, `bars`, `labels`,
`leds`, `log`, `core-map`, and `clock`. Ten-frame ablations after rebuild
batching isolated the update costs:

| Enabled workload | Wall FPS | Drain total | Full style/layout passes | End queue |
|---|---:|---:|---:|---:|
| All live groups | 10.6 | 0.90 s | 13 | 24 |
| All except scope | 15.3 | 0.63 s | 10 | 0 |
| All except LEDs | 15.3 | — | 10 | 0 |
| All except scope and LEDs | 39.4 | 0.20 s | 5 | 0 |
| Bars only | 53.0 | 0.09 s | 4 startup passes | 0 |
| Labels and clock only | 42.6 | 0.10 s | 4 startup passes | 0 |

After adding kind-aware selector dependency checks, the all-except-scope case
holds **39.2 FPS**, drains in **0.21 s**, performs only the five startup
style/layout passes, and ends with an empty queue. The full workload remains at
**10.3 FPS** because the scope still replaces its complete extension node once
per telemetry update.

### Immediate Conclusions

1. **The GPU draw path is not the primary source of lag.**
   Live frame CPU work is approximately 1.6 ms. Primitive emission is about
   0.2 ms and line-plot encoding is below 0.01 ms in the sampled frames.
2. **Command-side rebuild work caused the visible stalls.**
   Before batching, 10 live frames spent 5.24 seconds in command drains.
3. **Generic `SetProp` was expensive because individual LED state changes
   immediately reapplied styles.**
   It accumulated 3.26 seconds before batching.
4. **Paint-widget `ReplaceNode` was expensive because every repaint forced an
   immediate full style/layout pass.**
   It accumulated 1.20 seconds before batching.
5. **The first batching fix was effective.**
   - Command drain total: 5.24 s → 0.90 s (**83% reduction**)
   - Wall FPS: 3.10 → 10.6 (**3.4× improvement**)
   - Ending queue: 419 → 24 (**94% reduction**)
   - `SetProp` application: 3.26 s → less than 1 ms total
   - `ReplaceNode` application: 1.20 s → approximately 16 ms total
6. **The remaining full-pass bottleneck is CSS cascade, not layout math.**
   After batching:
   - Full `apply_layout`: 94.1 ms average
   - Stylesheet reapplication: 82.0 ms average
   - Actual layout computation: 1.83 ms average
   - Text/primitive rebuilds: a few milliseconds combined
7. **Style matching scales with retained tree size.**
   The reduced 1,052-widget workload spent about 57 ms per style reapply,
   compared with roughly 82–87 ms for 1,628 widgets.
8. **Scope repaint is now the dominant live bottleneck.**
   Removing scope updates while retaining LED updates raises wall FPS from
   10.3 to 39.2. The scope's structural `ReplaceNode` still requests a complete
   cascade and layout on every telemetry update.
9. **Common LED state updates can safely use visual invalidation.**
   The stylesheet store now detects direct and nested attribute dependencies
   by widget kind. An unrelated framework rule such as `Sidebar[state=...]`
   no longer forces LED updates through a whole-tree cascade; an
   `LED[state=...]` or relational selector still conservatively falls back to
   full invalidation.
10. **A display-list update command removes the dominant scope cost.**
    The ten-frame profile rises from 10.3 to 83.8 wall FPS. Seventeen scope
    updates take 1.87 ms total to apply (0.11 ms average), the queue ends empty,
    and no live update triggers a full style/layout pass.
11. **The improvement is sustained beyond the short profile.**
    A 60-frame run holds 63.5 wall FPS with an empty ending queue and only
    0.14 seconds of total command-drain time. All four full style/layout passes
    remain startup-only.
12. **Several stream handlers still bypassed deferred rebuilding.**
    Routing line-plot set/append/clear, histogram, toast, and resource-release
    handlers through `rebuild_for_dirty()` reduces ten-frame primitive rebuilds
    from 79 to 35 and command-drain time from 134.6 ms to 115.0 ms. The final
    profile holds 83.1 wall FPS with an empty queue.
13. **Container queries disabled the whole-tree style cache.**
    Any installed container rule caused every `apply_layout()` to recascade the
    complete tree, even when viewport and container widths were unchanged.
    DragonGUI now caches the last container-query context and reevaluates
    changed contexts after computing the new layout. In the static 1,628-widget
    profile:
    - Style reapplication: 405.5 ms → 160.5 ms
    - Full layout pipeline: 531.6 ms → 275.9 ms
    - Wall FPS during the short startup profile: 51.4 → roughly 90–96
14. **The text atlas was never trimmed.**
    Glyphon requires `TextAtlas::trim()` after rendering so allocations unused
    by the latest prepare cycle can be reclaimed. Without it, dynamic text
    eventually emitted `glyph texture atlas is full` and stopped preparing
    text correctly. A 120-frame run now reports:
    - 120 atlas trims
    - 0 base prepare errors
    - 0 overlay prepare errors
    - 2 stable custom glyphs/images
15. **Selector indexing is not the dominant remaining cascade cost.**
    New cascade counters show one CATHODE cascade visits 1,628 nodes and
    considers 52,891 candidate rules versus approximately 670,736 for a naïve
    412-rule full scan. It matches 15,815 declarations but materializes 72,461
    provenance entries. The next style-engine investigation should therefore
    prioritize provenance and snapshot allocation, selective attribute
    snapshots, and inherited-style copying before adding broader selector
    indexes.
16. **Repeated selector-label formatting was avoidable hot-loop work.**
    Parsed rules now cache their immutable selector label. Three follow-up
    profiles put combined base/container cascade time at a 237.5 ms median
    versus 241.2 ms before the change. This is a small (~1.5%) but repeatable,
    low-risk allocation reduction.
17. **Substage timing rules out selector matching and property application.**
    On the 1,628-widget workload, the instrumented cascade measured:
    - Selector matching: 1.86 ms
    - Property application: 1.52 ms
    - Provenance recording: 26.40 ms
    - Native-fallback provenance: 10.33 ms
    - Inheritance: 2.31 ms
    - Selector/input snapshot construction: 3.24 ms

    The dominant CSS expense was diagnostic bookkeeping, not resolving or
    applying the winning style values.
18. **Immutable provenance sharing materially reduces cascade allocation.**
    Provenance candidates are now reference-counted immutable records.
    Shorthand-expanded properties and separate nodes matched by the same
    declaration share one record, while each property retains the same ordered
    winner/overridden view. A cascade-local rule/declaration cache also avoids
    rebuilding identical records for every matching node. Across three
    fresh-process profiles:
    - Provenance recording median: 26.40 ms → 16.78 ms (**36% reduction**)
    - Declaration application median: 35.59 ms → 23.61 ms (**34% reduction**)
    - Provenance entry count remains 72,461, preserving diagnostic coverage

    Native-fallback provenance is now the largest separately measured cascade
    substage at roughly 11.6–12.0 ms and is the next provenance target.
19. **The sustained live path is responsive, but rebuild granularity is the
    next runtime scaling issue.**
    The 60-frame full live workload holds 62.4 wall FPS, ends with an empty
    queue, and averages only 1.59 ms of CPU frame work. Its 9.97 ms average
    surface-acquire wait is primarily presentation pacing rather than CPU
    rendering. However, the run still performs 76 full-tree text rebuilds
    (131.4 ms total) and 141 primitive rebuilds (57.5 ms total). These costs
    are not yet large enough to make this workload lag, but they scale with the
    entire retained tree and are now the most useful library-level target for
    larger or faster live applications.
20. **A fresh-process performance matrix now separates scaling from update
    frequency.**
    `tools/run_cathode_performance_matrix.py` provides quick, full, and nightly
    presets, runs every case in its own process, and writes comparable JSON and
    Markdown summaries. Timing records now include bounded p50/p95/p99, minimum,
    and maximum values. Dirty diagnostics separately report:
    - Requested layout/text/visual/GPU/full invalidations
    - Rebuilds actually executed after batching
    - Requests merged into a more expensive deferred rebuild
    - Dirty levels attributed to each runtime command type
21. **The first quick matrix confirms near-linear retained-tree startup
    scaling.**
    The static cases measured:
    - 988 widgets: 81.3 ms total style work
    - 1,628 widgets: 123.6 ms total style work
    - 7,004 widgets: 500.4 ms total style work

    The 7,004-widget case remains interactive after startup, but a full text
    rebuild reaches 7.2 ms p95 and total startup command draining reaches
    282.4 ms. Whole-tree operations, rather than GPU encoding, are therefore
    the limiting factor as retained trees grow.
22. **Dirty attribution proves batching is effective but text rebuilding is
    still global.**
    In the 30-frame default live case, commands request 110 text and 205 visual
    invalidations. Batching reduces those to 10 executed text rebuilds and no
    separately executed visual rebuilds. `SetProp` accounts for 101 text and
    176 visual requests. This is a large reduction in rebuild frequency, but
    each executed text rebuild still walks the entire retained tree.
23. **CSS animation frames are the dominant unexplained text-rebuild source.**
    Disabling scope, labels, log, core-map, and clock eliminates all live text
    command invalidations, yet a 30-frame run still performs 31 full text
    rebuilds (51.9 ms total, 3.0 ms p95). Dirty execution reports 10 visual
    rebuilds and zero text rebuilds, proving the additional text work comes
    directly from frame animation rather than command invalidation. CATHODE
    intentionally contains infinite keyframe animations, including a blinking
    text cursor. DragonGUI currently stores text as one global entry list
    without widget ownership, so one animated label forces a complete text
    collection pass.
24. **Animation and transition scheduling now avoid two unnecessary paths.**
    - Background/border-only style transitions rebuild primitives without
      automatically rebuilding text.
    - Completed CSS animations with retained fill-mode visuals no longer keep
      the event loop scheduled forever; activity is tracked separately from
      the retained final visual.

    CATHODE's infinite animations correctly remain active, so the next material
    optimization requires retained widget-to-text-entry ownership rather than
    another coarse dirty classification.
25. **Retained widget ownership now removes global text work from CSS
    animation frames.**
    Base-tree text entries retain their producing widget id and the renderer
    maintains an owner-to-entry index. Text-affecting CSS animation changes
    recollect only the affected widget subtree; paint-only keyframes rebuild
    primitives without touching text. Modal, tooltip, and dataframe-table
    ordering remains on the full-rebuild fallback until those overlay passes
    gain independently patchable ranges.

    In the 30-frame no-dynamic-text workload:
    - Global text collections fell from 31 animation-time passes to 4
      startup/layout passes.
    - 31 partial passes replaced 590 entries with zero fallbacks.
    - Partial text work totaled 11.5 ms at 0.51 ms p95, compared with 51.9 ms
      and 3.0 ms p95 for the previous global animation path.
    - This is approximately a 77% reduction in animation-driven text update
      cost.

    In the default live workload, total measured text work fell from about
    63.0 ms to 41.5 ms despite a higher number of sampled live text commands.
    The remaining global rebuilds are command-driven rather than animation
    driven.
26. **The optimized line-segment pipeline now preserves ancestor paint clips.**
    Scrolling the CATHODE Beam Scope exposed its custom display-list grid and
    trace above the scroll viewport and behind the tab strip. Layout and the
    general primitive representation contained the correct final paint clip,
    but primitive compaction discarded that clip when rotated capsules entered
    the fast GPU line format.

    Compact line instances now carry an absolute screen-space clip into the
    line shader, where fragments outside the final ancestor boundary are
    discarded. This keeps the optimized path enabled and fixes every custom
    paint/display-list line inside clipped or scrolling containers. Focused
    tests cover both clip preservation during compaction and an extension
    display list under a scroll-derived paint clip.
27. **Deferred command batches now retain widget-aware text invalidation.**
    Text-affecting `SetProp`, `SetStyle`, `Invalidate`,
    `UpdateExtensionDisplayList`, and histogram commands previously lost their
    widget ids when their dirty levels entered the deferred accumulator. The
    flush consequently rebuilt text for the entire application even when a
    batch changed only a handful of labels.

    The accumulator now retains text target ids independently of dirty
    severity, removes descendant targets when an ancestor is already dirty,
    and replaces only the affected retained text roots. Paint-only commands can
    still merge into the same batch. Untargeted text changes, stale ids,
    unsupported retained ranges, and batches exceeding 64 normalized roots
    retain the global rebuild fallback. Table and toast commands deliberately
    remain global until their overlay/table ranges are independently patchable.

    A 30-frame default live profile
    (`cathode-default-live-targeted-command-text.json`) recorded:

    - 167 targeted command requests normalized to 150 roots across 16 batches.
    - 16 completed targeted batches and zero fallbacks.
    - Zero command-driven global text collections; the only full text
      collections were the four startup/layout passes.
    - Total `rebuild_visuals` time fell from 27.33 ms in the previous 30-frame
      retained-text baseline to 15.50 ms, approximately a 43% reduction,
      despite the new sample processing 170 rather than 149 text requests.
    - All partial text work, including the existing animation path, totaled
      14.44 ms with a 0.52 ms p95.

    Telemetry now reports targeted/global requests, attempted/completed/fallback
    batches, rebuilt root count, and the safety limit under
    `framework.command_text_rebuilds`.
28. **Dataframe tables and virtual overlays now have independent retained text
    ranges.**
    The text renderer now records explicit boundaries for base-tree text,
    dataframe-table text, virtual overlay text, and ephemeral scatter labels.
    This prevents table cell/header updates from touching generated base text
    owned by the same widget and lets overlays be recollected without
    rebuilding the base or discarding scatter labels.

    `SetTableData` and `SetTableDataColumns` carry table-specific targets
    through deferred batches. Toast show, update, dismissal, and expiration
    rebuild the overlay range and primitives only. Mixed base/table/toast
    batches can replace all required text ranges before issuing one primitive
    rebuild. Generic style changes on a table remain on the conservative
    whole-subtree/global path rather than being misclassified as data-only.

    The repeated 30-frame default CATHODE profile
    (`cathode-default-live-table-overlay-text.json`) recorded:

    - All three dataframe updates as targeted requests.
    - Zero global text requests, compared with three in the preceding profile.
    - 13 completed targeted batches and zero fallbacks.
    - 122 normalized roots rebuilt from 139 text requests.
    - 10.08 ms total `rebuild_visuals` work at 0.78 ms per batch on average.

    A separate live toast smoke test recorded one overlay request, one
    completed overlay batch, one inserted overlay entry, and zero global
    rebuilds or fallbacks. The full native suite passed with 785 tests, 12
    ignored, and zero failures.
29. **Direct interaction paths now use retained text replacement instead of
    whole-window text collection.**
    Pointer hover, plot selection, drag/drop, focus traversal, dropdown and
    context-menu state, text editing and selection, number input, tree
    selection, table interaction, and pointer activation previously called
    the global `rebuild_visuals()` path directly. Even a caret move or hover
    transition could therefore recollect text for the full widget tree.

    These paths now identify the affected widget roots, table-only roots, and
    overlay range before issuing a single targeted visual rebuild. Focus
    transitions include both the old and new focus owners. Multiline editing
    performs the minimum preliminary text pass needed to update caret geometry
    and horizontal scrolling, followed by targeted replacement. Layout-changing
    activation remains on the layout path.

    Interaction batches normalize overlapping widget roots, reject stale ids,
    and share the 64-root command safety limit. Any unsupported range or failed
    partial replacement falls back to the global path. Runtime snapshots expose
    attempts, completed batches, fallbacks, widget roots, table roots, and
    overlay passes under `framework.interaction_text_rebuilds`.

    The post-change 30-frame default CATHODE profile
    (`cathode-default-live-targeted-interactions.json`) recorded:

    - 0 global command-text requests and 0 command-targeted fallbacks.
    - 13 completed command-targeted batches replacing 122 normalized roots.
    - 2.17 ms average CPU frame work, compared with 2.10 ms in the immediately
      preceding sample; this small difference is normal run-to-run variance.
    - 0 interaction attempts, as expected: the CATHODE profiler drives live
      commands but does not synthesize mouse or keyboard input.

    The direct-call audit now leaves global text rebuilding only in explicit
    fallback, global dirty, CSS-transition, and animation-fallback paths. The
    full native suite passed with 786 tests, 12 ignored, and zero failures.
30. **Virtual overlay primitives now rebuild without traversing the base
    widget tree.**
    Primitive rendering already stored base and overlay instances on opposite
    sides of a stable `overlay_start` boundary, but every toast-only update
    discarded both ranges and recursively re-emitted the full tree. On the
    1,628-widget CATHODE document this made an overlay notification pay a base
    traversal cost even though no base surface changed.

    The renderer now retains the base instance prefix, truncates only the old
    overlay tail, and re-emits dropdown, menu, modal, tooltip, toast, and
    drag/drop overlays in their original order. The combined instance list is
    then split and uploaded normally, preserving batching and render order.
    Overlay-only targeted visual batches use this path; mixed base/table and
    overlay changes remain on the conservative full primitive path.

    Primitive telemetry now reports cumulative full and overlay rebuilds plus
    the last rebuild scope under `renderer.primitives.retained_rebuilds`. A
    painted live smoke test showed four retained base batches, two new overlay
    batches, one overlay-only rebuild, one overlay text batch, and zero text
    fallbacks.
    Overlay emission took 0.007 ms in that sample. The regular 30-frame
    CATHODE profile (`cathode-default-live-overlay-primitives.json`) remained
    healthy at 2.14 ms average CPU frame work, with zero overlay rebuilds as
    expected because that workload does not display a toast.
31. **Safe leaf-widget primitives now use retained range replacement.**
    Targeted text and interaction batches still rebuilt every base primitive
    after replacing only the affected text. Full primitive emission now records
    stable ranges for leaf widgets, including explicit empty ranges for
    text-only leaves. A targeted batch regenerates those leaves, splices their
    ranges in original stacking order, adjusts all following leaf and overlay
    offsets, and performs one normal split/upload pass.

    Eligibility is deliberately conservative. Containers, scroll owners,
    tab-attached surfaces, modal subtrees, stale or missing ids, and leaves
    beneath transformed ancestors fall back before renderer state is mutated.
    An empty retained range is accepted only when regeneration also remains
    empty; a new painted surface forces a full rebuild. Mixed overlays can be
    re-emitted after the base splice without a second upload.

    Telemetry reports partial-base attempts, completions, and fallbacks under
    `renderer.primitives.retained_rebuilds`. The 30-frame CATHODE profile
    (`cathode-default-live-retained-leaf-primitives.json`) recorded 15 attempts,
    15 completions, and zero fallbacks. Average targeted `rebuild_visuals`
    time fell from 0.76 ms in the preceding profile to 0.43 ms, approximately
    a 43% reduction. A steadier 60-frame run
    (`cathode-default-live-retained-leaf-primitives-60f.json`) retained the same
    15/15 success rate, averaged 0.48 ms per targeted visual batch, and averaged
    2.06 ms of CPU frame work.

    The reusable `profile_retained_primitive_smoke.py` verified both branches:
    a normal label update completed one partial-base rebuild, while the same
    update below a translated panel recorded one attempted fallback and one
    additional full rebuild. Targeted text replacement remained successful in
    both cases. Pure native tests cover following-range/overlay offset shifts
    and transformed-ancestor exclusion. The complete native suite passed with
    788 tests, 12 ignored, and zero failures.
32. **Retained primitive batches now skip unrelated line-plot regeneration.**
    DragonGUI renders line series through a dedicated renderer, separate from
    the general primitive list. Successful retained leaf batches still rebuilt
    that renderer for every label, badge, table, or extension update even when
    no line-plot state or styling had changed.

    Before rebuilding, the runtime now checks every targeted subtree for a
    `LinePlot`. Direct line-plot targets and targeted ancestors containing a
    line plot rebuild conservatively. Unrelated targets skip the renderer.
    Missing targets also resolve conservatively, although the retained-range
    validator normally rejects them first.

    The retained renderer telemetry now reports targeted line-plot checks,
    rebuilds, and skips. The 60-frame CATHODE profile
    (`cathode-default-live-targeted-line-plot-gating-60f.json`) recorded 17
    checks, 17 skips, zero targeted line-plot rebuilds, and zero primitive
    fallbacks. Average targeted `rebuild_visuals` time fell from 0.48 ms to
    0.33 ms, approximately a further 31% reduction, while overall CPU frame
    work remained stable at 2.07 ms.

    The retained primitive smoke tool now has a `--line-plot` case. Updating a
    live plot axis label recorded one targeted check, one required line-plot
    rebuild, one retained base completion, and zero skips or fallbacks. Native
    tests cover direct, ancestor, unrelated, and stale target classification.
    The full native suite passed with 789 tests, 12 ignored, and zero failures.
33. **Retained primitive ownership now covers safe subtrees and CSS animation
    frames.**
    The first animation-target experiment retained every changed animation id
    but all 61 frame attempts fell back because CATHODE animates panels and
    other containers, not only leaves. That diagnostic run is preserved as
    `cathode-default-live-targeted-animation-primitives-60f.json`.

    Full primitive emission now records nested subtree ranges. Target sets are
    normalized to their shallowest roots, a subtree is re-emitted with its
    descendants and trailing scroll chrome, and replacement adjusts enclosing,
    following, and overlay ranges. Nested ownership is rebuilt from the
    replacement. Transformed ancestors, tab-body attachment, stale ids, and
    unsupported empty-to-painted transitions retain conservative fallbacks.

    CSS animation ticks now retain every visually changed owner separately
    from the subset requiring text replacement. Animation frames patch the
    visual subtrees and only reshape text for text-affecting changes. Toast
    expiration can update overlay ranges and the same animation targets in one
    retained primitive upload.

    In the successful 60-frame profile
    (`cathode-default-live-retained-animation-subtrees-60f.json`):

    - All 76 partial-base attempts completed with zero fallbacks: 61 animation
      frames plus 15 command batches.
    - Full primitive rebuilds fell from 126 to 65.
    - Total `rebuild_primitives` time fell from 140.94 ms to 88.77 ms,
      approximately a 37% reduction.
    - All 76 targeted line-plot checks skipped unrelated line regeneration.
    - CPU frame work averaged 2.23 ms; presentation scheduling variance still
      dominates the total-frame average.

    The full native suite remained green with 789 tests, 12 ignored, and zero
    failures.
34. **Same-shape retained primitive changes now patch GPU buffer slices.**
    The retained subtree path previously avoided whole-tree emission but still
    classified every source primitive, rebuilt every pipeline batch, and
    uploaded all simple, line, and complex instance buffers after each local
    replacement.

    Full splitting now retains a source-index route to each pipeline instance.
    When every replacement preserves its primitive count and pipeline-kind
    sequence, the renderer updates only those routed CPU instances and writes
    the smallest covering slice of each affected GPU buffer. Existing draw
    batches and source routes remain valid. Count changes, pipeline-kind
    changes, invalid routes, and overlay regeneration conservatively use the
    existing full split/upload path. Split-collapsed rendering is also handled
    explicitly by retaining every route in the complex pipeline.

    Telemetry now reports successful partial buffer patches, patch fallbacks,
    cumulative partial-upload bytes, and the last partial-upload size. In the
    60-frame CATHODE profile
    (`cathode-default-live-partial-primitive-uploads-60f.json`):

    - 72 of 78 retained subtree rebuilds used partial GPU writes.
    - Six structural or overlay cases used the full split/upload fallback.
    - Partial writes uploaded 553,712 bytes total. At the final 46,032-byte
      buffer footprint, 72 whole-buffer uploads would have transferred about
      3.31 MB, so the new path avoided approximately 83% of that traffic.
    - Total `rebuild_primitives` time was 86.67 ms versus 88.77 ms in the
      retained-subtree baseline. Primitive emission now dominates these local
      updates, so the CPU timing gain is modest while transfer work drops
      substantially.

    Native tests cover split-enabled, split-disabled, split-collapsed, simple,
    line, and complex route classification plus sparse upload-span selection.
    The complete native suite passed with 791 tests, 12 ignored, and zero
    failures. The release extension was rebuilt and installed with matching
    artifact hashes.
35. **Native fallback provenance is now materialized only for diagnostics.**
    A normal cascade previously resolved layout, geometry, paint, semantic
    part, and theme fallback values for every node, allocated complete
    provenance candidates, and retained them in every computed `NodeStyle`.
    Renderers do not consume this metadata; it exists for computed-style
    inspection.

    The live style cache now retains native fallback candidates only for
    `font-size`, `font-weight`, and `color`, because those candidates are
    required to preserve truthful inherited-text provenance. Computed-style
    snapshots clone each style and materialize the complete layout, geometry,
    paint, part, and widget-state fallback chain on demand. Live pane sizing is
    then refreshed on that same diagnostic clone, so requesting diagnostics
    cannot mutate the render cache.

    Tests continue to exercise the complete fallback catalog through explicit
    diagnostic materialization. A separate production-path regression proves
    that non-inherited fallback candidates are absent from the live cascade
    and restored in the diagnostic copy. Existing inheritance, cascade-order,
    semantic-part, media, and live-pane snapshot expectations remain intact.

    Compared with the preceding 60-frame report, the new CATHODE profile
    (`cathode-default-live-lazy-fallback-provenance-60f.json`) recorded:

    - Native fallback cascade work fell from 12.98 ms to 1.83 ms, an 85.9%
      reduction.
    - Stored provenance entries fell from 72,461 to 40,189, a 44.5% reduction.
    - Four full style reapplications fell from 156.66 ms to 91.94 ms, a 41.3%
      reduction.
    - The four enclosing `apply_layout` passes fell from 272.68 ms to
      177.43 ms, a 34.9% reduction.
    - Average CPU frame work remained healthy at 2.25 ms.

    A separate 10-frame static report
    (`cathode-default-static-lazy-fallback-provenance.json`) reproduced the
    lean 40,189-entry cascade and 1.88 ms native-fallback stage. The complete
    native suite passed with 792 tests, 12 ignored, and zero failures. The
    release extension was rebuilt and installed with matching artifact hashes.
36. **A fresh-process isolated subsystem matrix now complements CATHODE.**
    The integrated demo is useful for realistic interaction between systems,
    but it can hide a large cold-path cost behind unrelated work. The new
    `tools/profile_subsystem_stress.py` harness builds deterministic CSS-,
    text-, primitive-, dataframe-table-, and Scatter3D-heavy documents and
    emits one common compact schema. Matrix mode launches every case in its own
    Python process, then writes machine-readable JSON and a Markdown comparison
    table.

    Default workloads are 1,600 CSS nodes with 250 generated rules, 1,600
    wrapped multilingual labels, 3,600 mixed simple/outline/complex panels, a
    20,000-row virtualized table, and a 125,000-point Scatter3D. Sizes and frame
    counts remain configurable. Reports include Python construction and
    serialization, cascade substages, framework timings, primitive/text
    renderer state, resource counts, and frame-stage distributions.

    The initial matrix (`subsystem-matrix-initial`) isolated two facts that the
    combined workload did not make obvious:

    - The 1,600-label case spent 323.65 ms across three layout passes while
      text painting itself took only 12.47 ms.
    - The 3,600-panel case spent 149.92 ms in style reapplication, with
      authored provenance recording accounting for 73.90 ms of its first
      cascade.

    Table virtualization remained effective at 20,000 rows, and the
    125,000-point scatter case averaged 2.38 ms of CPU frame work. These cases
    are now stable independent regression workloads rather than conclusions
    inferred from CATHODE alone.
37. **The layout text-measurement cache no longer thrashes on large wrapped
    label trees.**
    Layout measures each wrapped label at least twice: once for intrinsic width
    and again for constrained height. The old 2,048-entry cache therefore
    overflowed during the 1,600-label case. Its clear-all capacity policy
    discarded the entire working set, causing all three startup/layout passes
    to reshape nearly every label.

    The bounded cache now holds 16,384 measurements, enough for realistic
    multi-thousand-widget documents while retaining a fixed upper bound.
    Runtime diagnostics expose cache entries, limit, hits, misses, capacity
    clears, and font-synchronization clears. Both subsystem and CATHODE reports
    retain these counters. A native regression constructs 1,100 unique wrapped
    labels, crosses the old capacity boundary, and verifies complete second-pass
    reuse without a clear.

    In the repeated isolated matrix (`subsystem-matrix-after-text-cache`):

    - The label case recorded 3,200 first-pass misses, 6,400 subsequent hits,
      3,200 retained entries, and zero capacity clears.
    - Total layout time fell from 323.65 ms to 124.79 ms, a 61.4% reduction.
    - The cached layout passes fell from approximately 110 ms each to 4–5 ms;
      the remaining 115 ms maximum is the unavoidable first shape of 1,600
      distinct strings.
    - Total enclosing `apply_layout` time fell from 378.22 ms to 184.21 ms, a
      51.3% reduction.

    The 60-frame CATHODE validation
    (`cathode-default-live-layout-text-cache-60f.json`) recorded 99 misses,
    1,520 hits, and zero capacity clears. Its smaller measurement working set
    already fit the old cache, so steady-state timings remained within normal
    run variance. The complete native suite passed with 793 tests, 12 ignored,
    and zero failures. The release extension was rebuilt and installed with
    matching artifact hashes.
38. **Authored-provenance property expansion is now cached per cascade.**
    The cascade already shared each immutable declaration candidate, but every
    matched node still normalized the same authored property, expanded the same
    shorthand, rebuilt the same pseudo/part property keys, and discarded that
    temporary list. A cascade-local cache now stores both the shared candidate
    and its expanded property keys by rule, declaration, and pseudo-state slot.
    Nodes still own their candidate vectors and retain the complete diagnostic
    chain; only the repeated key construction has been removed.

    In the 3,600-node primitive workload
    (`subsystem-provenance-key-cache/primitives.json`):

    - Provenance entries remained exactly 202,511.
    - Provenance recording fell from 73.90 ms to 25.16 ms, a 66.0% reduction.
    - Total declaration application fell from 93.85 ms to 45.06 ms, a 52.0%
      reduction.
    - Selector matching and property application remained effectively
      unchanged, confirming that the improvement is isolated to diagnostic
      bookkeeping.

    The 60-frame integrated CATHODE validation
    (`cathode-default-live-provenance-key-cache-60f.json`) retained all 40,189
    live provenance entries. Provenance recording fell from 16.85 ms to 7.97
    ms, declaration application from 23.55 ms to 14.87 ms, and total style
    reapplication from 93.44 ms to 81.28 ms. Average CPU frame work remained
    healthy at 2.50 ms and the run sustained 62.98 wall FPS. The complete
    native suite passed with 793 tests, 12 ignored, and zero failures. The
    release extension was rebuilt and installed with matching artifact hashes.
39. **Per-node provenance storage now shares keys and keeps short candidate
    chains inline.**
    The property-key cache removed repeated formatting but each node still
    cloned a `String`, inserted it into a `BTreeMap`, and allocated a heap
    `Vec` for every property. Provenance maps now use shared `Arc<str>` keys in
    a pre-sized hash map. Candidate chains retain their cascade order in a
    four-entry inline `SmallVec` and spill to the heap for longer chains. The
    public computed-style snapshot still emits owned string keys through
    `serde_json::Map`, so its deterministic JSON shape is unchanged.

    The implementation was measured in stages against the same 3,600-node
    primitive workload:

    - Shared hash-map keys reduced provenance recording from 25.16 ms to 22.43
      ms.
    - Pre-sizing maps from the known matched-declaration count reduced it to
      18.88 ms.
    - Inline candidate chains reduced it to 12.94 ms.
    - Against the pre-compaction checkpoint, provenance recording is 48.6%
      lower, declaration application is down from 45.06 ms to 33.86 ms, and
      first style reapplication is down from 82.85 ms to 58.57 ms.
    - Against the original isolated finding, provenance recording is 82.5%
      lower and first style reapplication is 56.0% lower, while all 202,511
      provenance entries remain present.

    The integrated 60-frame CATHODE validation
    (`cathode-default-live-inline-provenance-60f.json`) retained all 40,189
    provenance entries. Relative to the property-key-cache run, provenance
    recording fell from 7.97 ms to 5.92 ms and total style reapplication from
    81.28 ms to 76.23 ms. Average CPU frame work was 2.12 ms and the run
    sustained 62.71 wall FPS. The complete native suite passed with 793 tests,
    12 ignored, and zero failures. The release extension was rebuilt and
    installed with matching artifact hashes.
40. **Cascade substage timing is now bulk-scoped instead of
    declaration-scoped.**
    The provenance and property timers previously surrounded every matched
    declaration. At 85,501 declarations, starting and reading those clocks
    hundreds of thousands of times both distorted the supposedly unaccounted
    declaration time and slowed the real cascade. The cascade now runs
    separate provenance and property passes within each precedence layer and
    times each pass once per node. Framework declarations still precede widget
    defaults, and user declarations still precede inline styles, so cascade
    semantics and candidate ordering are unchanged.

    Diagnostics now also report `style_merge_ms` and `part_filter_ms`. In the
    3,600-node primitive report (`subsystem-bulk-cascade-timing/primitives.json`):

    - Declaration application fell from 33.86 ms to 25.09 ms.
    - First style reapplication fell from 58.57 ms to 50.25 ms, a 14.2%
      reduction.
    - The newly reliable breakdown is 11.25 ms provenance recording, 7.70 ms
      default/inline merging, 3.65 ms property application, and 0.65 ms part
      filtering.
    - Those measured substages account for 23.25 ms of the 25.09 ms
      declaration stage; the former unexplained 15 ms gap was primarily
      per-declaration timing overhead.

    The 60-frame integrated validation
    (`cathode-default-live-bulk-cascade-timing-60f.json`) recorded 10.33 ms
    declaration application, including 4.44 ms provenance, 3.73 ms merging,
    1.09 ms property application, and 0.24 ms part filtering. Total style
    reapplication fell from 76.23 ms to 61.43 ms, with the maximum pass falling
    from 41.56 ms to 31.54 ms. The run sustained 63.44 wall FPS. The complete
    native suite passed with 793 tests, 12 ignored, and zero failures, and the
    rebuilt release extension has matching installed and build hashes.
41. **Style overlays now merge sparsely without cloning untouched base
    values.**
    The merge helpers previously expressed optional overlays as
    `overlay.clone().or_else(|| base.clone())`. When an overlay omitted a
    field, this cloned the existing base value and assigned it back unchanged.
    Every complex panel therefore cloned its gradient stops and shadow vector
    for both the empty widget-default layer and the empty inline layer.

    Layout grid definitions, visual paints and colors, text font/color data,
    widget strings, transition property lists, and generated part content now
    assign only when the overlay actually contains a value. Copy-only fields
    retain the same precedence logic. The 3,600-node primitive report
    (`subsystem-sparse-style-merge/primitives.json`) recorded:

    - Style merging fell from 7.70 ms to 4.42 ms, a 42.6% reduction.
    - Declaration application fell from 25.09 ms to 22.04 ms, a 12.2%
      reduction.
    - First style reapplication fell from 50.25 ms to 47.34 ms.
    - All 202,511 provenance entries and existing cascade results were
      retained.

    A follow-up experiment compared each complete `NodeStyle` with a default
    instance before merging. It skipped 7,203 of 7,204 merges but was rejected:
    full-structure comparison raised merge time to 7.44 ms and first style
    reapplication to 62.28 ms. The slower evidence is retained in
    `subsystem-empty-style-fastpath/primitives.json`; none of that fast-path
    code remains.

    The integrated 60-frame validation
    (`cathode-default-live-sparse-style-merge-60f.json`) reduced merge time
    from 3.73 ms to 2.37 ms and declaration application from 10.33 ms to 8.69
    ms. Total style reapplication fell from 61.43 ms to 58.53 ms, with a 30.14
    ms maximum pass. The run sustained 62.88 wall FPS. The complete native
    suite passed with 793 tests, 12 ignored, and zero failures, and the final
    installed extension matches the release build hash.
42. **Non-inherited authored provenance is now deferred to diagnostic
    snapshots.**
    The live cascade previously retained every authored candidate chain even
    though normal layout, paint, and input processing only need the winning
    computed values. Production cascades now retain authored provenance only
    for the eleven inherited text properties needed to explain inheritance.
    Debug computed-style snapshots clone the document and stylesheet store and
    perform an on-demand diagnostic cascade with full provenance, using the
    current media state and the most recently resolved container contexts.
    Tests continue to run cascades in full-provenance mode by default, and an
    explicit lean-cascade regression verifies that an authored `color`
    candidate is retained while a non-inherited `width` candidate is deferred.

    The 3,600-node primitive report
    (`subsystem-deferred-authored-provenance/primitives.json`) recorded:

    - Stored provenance entries fell from 202,511 to 10,810, a 94.7%
      reduction.
    - Provenance recording fell from 11.49 ms to 0.51 ms, a 95.5% reduction.
    - Declaration application fell from 22.04 ms to 11.96 ms, a 45.8%
      reduction.
    - First style reapplication fell from 47.34 ms to 37.58 ms, a 20.6%
      reduction; total sampled style reapplication fell from 49.77 ms to
      39.88 ms.

    The integrated 60-frame validation
    (`cathode-default-live-deferred-authored-provenance-60f.json`) reduced
    stored provenance from 40,189 to 9,788 entries, provenance recording from
    3.90 ms to 1.13 ms, and declaration application from 8.69 ms to 6.00 ms.
    Total sampled style reapplication moved from 58.53 ms to 57.92 ms, while
    average CPU frame work remained statistically flat at 2.50 ms versus 2.48
    ms, as expected for a startup/cascade optimization. A release-level
    diagnostic check confirmed that deferred `width` and `background`
    candidates, their winning source, and overridden framework/native
    candidates are reconstructed correctly on demand. The complete native
    suite passed with 794 tests, 12 ignored, and zero failures. The rebuilt
    release extension and installed module have matching SHA-256 hashes.
43. **Selector snapshots now materialize only referenced widget attributes.**
    A single attribute selector previously caused every node to clone every
    exposed identity and property attribute, even when the active stylesheets
    referenced only a few names. Stylesheet feature analysis now traverses
    compound, ancestor, child, functional, and `:has(...)` selectors once to
    build a compact attribute dependency mask. Current-node, ancestor, and
    sibling snapshots consult that mask before allocating attribute names and
    values. Semantic public widget selectors such as `SearchBox` explicitly
    retain their required `css-type` attribute.

    Cascade telemetry now reports `attribute_snapshot_entries` in addition to
    the number of node snapshots. The 3,600-node primitive report
    (`subsystem-selective-attribute-snapshots/primitives.json`) recorded 10,806
    materialized entries across 3,602 node snapshots and:

    - Snapshot construction fell from 4.18 ms to 2.44 ms, a 41.6% reduction.
    - Selector matching fell from 2.16 ms to 1.87 ms, a 13.3% reduction.
    - First style reapplication fell from 37.58 ms to 32.14 ms, a 14.5%
      reduction.
    - Total sampled style reapplication fell from 39.88 ms to 35.28 ms, an
      11.5% reduction.

    The integrated 60-frame CATHODE validation
    (`cathode-default-live-selective-attribute-snapshots-60f.json`) materialized
    4,625 entries across 1,628 node snapshots. Snapshot construction fell from
    3.12 ms to 1.90 ms, selector matching from 1.63 ms to 1.23 ms, and total
    sampled style reapplication from 57.92 ms to 42.23 ms. The maximum style
    pass fell from 30.18 ms to 21.42 ms. Average CPU frame work improved from
    2.50 ms to 2.36 ms, while the run sustained 63.29 wall FPS. Regression
    coverage verifies direct, nested functional, `:has(...)`, and semantic
    type dependencies. The complete native suite passed with 795 tests, 12
    ignored, and zero failures. The release and installed extension hashes
    match.
44. **CATHODE interaction profiling is now deterministic and
    presentation-aware.**
    The runtime now supports an opt-in synthetic hover sequence through
    `DRAGONGUI_SYNTHETIC_HOVER_IDS`. After the first application frame, each
    requested widget id is resolved to the center of its visible layout rect
    and passed through the same generic hit-test, hover-state, targeted text,
    and retained primitive path used by real pointer movement. One target is
    injected per presented frame. The runtime records dispatch and
    injection-to-next-presentation distributions plus requested, resolved,
    missing, mismatched, and pending counts. Normal applications do not create
    or schedule this profiler unless the environment variable is present.

    `tools/profile_cathode_stress.py` exposes the driver through repeatable
    `--synthetic-hover WIDGET_ID` arguments, automatically reserves enough
    smoke frames, and includes the interaction telemetry in its compact report.
    Two six-step fresh-process runs over the rail-toggle, attach button, and
    search composite (`cathode-synthetic-hover-1.json` and
    `cathode-synthetic-hover-2.json`) produced:

    - Six dispatches, six resolved hits, zero missing targets, and six
      presentation samples in both runs.
    - Average dispatch cost of 8.24 ms and 8.13 ms.
    - Average presentation latency of 11.18 ms and 11.15 ms.
    - The composite search target correctly exposed two center-hit mismatches
      because its owned field child receives the physical hit.

    A final alternating-button baseline
    (`cathode-synthetic-hover-final.json`) eliminated that expected composite
    ambiguity: six dispatches resolved with zero misses or mismatches. Dispatch
    averaged 7.95 ms with an 8.39 ms maximum; presentation latency averaged
    11.00 ms with a 12.41 ms maximum. The run also recorded six successful
    targeted interaction-text rebuilds, eleven widget roots, six overlay
    passes, and zero fallbacks. This makes generic hover dispatch—not hit
    resolution—the next measured interaction bottleneck. The complete native
    suite passed with 796 tests, 12 ignored, and zero failures; the Python
    profiler also passes bytecode compilation. The rebuilt release and
    installed extension hashes match.
45. **Tooltip-free hover changes now retain overlay text and primitives.**
    Generic pointer hover previously passed `rebuild_overlays=true` for every
    target change. That was required when entering or leaving simple tooltips,
    rich tooltip targets, dropdown options, or active popups, but it also
    rebuilt the complete overlay text range for ordinary button-to-button
    movement.

    The first dependency gate correctly distinguished those cases but searched
    the complete widget tree for both hover endpoints. On the 1,628-widget
    CATHODE tree that scan erased the saved overlay work and was rejected.
    Tooltip-bearing target ids are now collected once beside retained widget
    metadata and refreshed with structural tree replacements. Hover and
    cursor-leave paths perform constant-time membership checks; popup and
    dropdown state remains conservatively overlay-dirty.

    A controlled pair of 30-step alternating tooltip-free button profiles used
    identical release sources except that the baseline temporarily forced the
    old unconditional overlay behavior:

    - Overlay passes fell from 30 to zero.
    - Partial-text rebuild samples fell from 93 to 63 and their total cost fell
      from 21.58 ms to 14.55 ms, a 32.6% reduction.
    - Targeted interaction visual work fell from 44.15 ms to 35.37 ms, a 19.9%
      reduction.
    - Average hover dispatch fell from 9.47 ms to 8.87 ms, a 6.4% reduction;
      p95 fell from 11.87 ms to 11.05 ms.
    - Average injection-to-presentation latency moved from 15.15 ms to 15.02
      ms because FIFO presentation timing dominates that measure.

    A tooltip-bearing rail-toggle safety profile retained all six overlay
    passes with six completed interaction rebuilds and zero fallbacks. Native
    tests cover blank, absent, simple, and rich tooltip metadata. The complete
    suite passed with 797 tests, 12 ignored, and zero failures. The final
    release and installed extension hashes match.
46. **Mixed text/visual command batches no longer skip live visual updates.**
    The CATHODE channel trace exposed a correctness regression in the targeted
    text rebuild optimization. Each 280 ms producer tick still reached the
    runtime and appended one point to both line-plot series, but a batch that
    also changed labels merged its dirty level to `Text`. The targeted rebuild
    then returned after rebuilding only text-owned roots, silently omitting the
    pending line plot and other visual widget roots. The retained plot model
    advanced while its GPU geometry remained stale until a later broad rebuild,
    making the trace appear to skip ticks.

    Deferred rebuild bookkeeping now retains visual widget ids separately and
    unions them with text roots before a mixed targeted rebuild. Untargeted
    visual work remains conservative and forces the existing full visual path.
    Line-plot set, append, and clear commands now identify their widget as the
    visual target rather than requesting an anonymous visual rebuild.

    Identical 90-frame live profiles verify the rendering correction:

    - Both runs applied all 34 packed append commands (17 ticks × two series).
    - Before the fix, targeted line-plot rebuilds were **0** despite 107
      targeted checks.
    - After the fix, targeted line-plot rebuilds rose to **10**, with 19 total
      line-plot rebuild samples covering mixed and broader telemetry batches.
    - The ending queue remained empty and wall FPS remained at 60.0.
    - Frame CPU work improved slightly from 2.04 ms to 1.97 ms average, so the
      correctness repair did not reintroduce the old full-tree bottleneck.

    Two native regression tests cover mixed text/visual target retention and
    the full-rebuild fallback for anonymous visual work. The complete native
    suite passes with 799 tests, 12 ignored, and zero failures. The release
    extension was rebuilt and installed for manual verification.
47. **Retained tooltip targets remove quadratic hover hit testing.**
    Synthetic hover dispatch now reports separate dropdown-hit, widget-hit,
    state-read, dependency, state-update, and rebuild distributions. The first
    instrumented 30-step CATHODE run showed that widget hit testing consumed
    7.51 ms of the 8.94 ms average dispatch, while the complete targeted
    text/primitive rebuild averaged only 1.41 ms.

    The hover predicate previously called a recursive rich-tooltip lookup for
    many visited widget candidates. On a 1,628-node tree, this repeatedly
    rescanned the same tree during one hit test. DragonGUI already maintains a
    retained set of simple and rich tooltip target ids for overlay dependency
    checks. Hover hit testing now receives that set and performs constant-time
    membership checks, preserving reverse-z traversal, modal boundaries,
    clipping, disabled-state filtering, interactive widgets, and authored
    hover styles.

    Identical 30-step alternating tooltip-free profiles produced:

    - Widget hit average: 7.51 ms → **0.389 ms** (**94.8% reduction**).
    - Widget hit p95: 9.45 ms → **0.586 ms**.
    - Total dispatch average: 8.94 ms → **2.22 ms** (**75.2% reduction**).
    - Total dispatch p95: 11.08 ms → **3.30 ms**.
    - All 30 targets resolved with zero misses and zero mismatches in both
      runs.
    - Overlay passes remained zero, confirming the tooltip-free gate still
      holds.

    Cached-target tests cover both positive rich-tooltip resolution and the
    negative case where a noninteractive widget is absent from the retained
    set. The profiler snapshot test covers every new bounded stage timing.
48. **Hover movement no longer synchronizes unrelated transition families.**
    The hover-specific targeted rebuild now skips full-tree focus, checked,
    active, open, and expanded transition synchronization. Those scans remain
    enabled for interactions that can change the corresponding states. On the
    CATHODE workload this is a deliberately small final optimization: rebuild
    average moved from 1.78 ms to 1.73 ms and total dispatch from 2.22 ms to
    2.17 ms. The result confirms that the remaining hover cost is useful
    text/primitive work rather than another large accidental scan.
49. **Explicit keyed task backpressure bounds replaceable telemetry without
    weakening stream semantics.**
    `call_soon_threadsafe(..., coalesce_key=...)` now retains only the newest
    pending callback per key in an ordered O(1)-indexed queue. Unkeyed events
    remain lossless FIFO work, and drains are bounded by both 100 callbacks and
    6 ms. Scheduler diagnostics report enqueue, execute, replacement, pending,
    and high-water counts.

    CATHODE separates every-tick plot/log/map streams from replaceable
    scope/bar/label/LED/clock snapshots. A 1,200-frame combined live and
    120-hover acceptance profile produced:

    - 60.01 wall FPS with 1.10 ms average CPU frame work.
    - Zero native queue depth, zero fairness yields, and zero suppressed
      warnings at exit.
    - Python task queue high-water of 15.
    - 171 Python callbacks enqueued, 157 executed, 13 obsolete snapshots
      coalesced, and one shutdown-race callback still pending in the captured
      pre-close snapshot.
    - All 85 lossless stream callbacks executed, producing all 170 packed
      line-plot append commands.
    - All 120 hover targets resolved with zero misses or mismatches; dispatch
      averaged 1.70 ms.
    - 1,200 successful text-atlas trims with zero text preparation errors.

    This meets the current responsiveness, queue, stream-correctness, and
    sustained mixed-interaction budgets. Additional GPU tracing and large-tree
    research remain useful infrastructure but are deferred until a real
    application exposes another bottleneck.

### Deferred Research Targets

1. Break down native fallback value application and the remaining sparse style
   merge work. They are now the largest primitive cascade substages at 3.84 ms
   and 4.39 ms respectively.
2. Add GPU timestamp queries, trace export, and larger synthetic tree matrices
   only when a shipping workload justifies the additional instrumentation.

Raw reports:

- `artifacts/performance/cathode-default-static.json`
- `artifacts/performance/cathode-default-live.json`
- `artifacts/performance/cathode-small-static.json`
- `artifacts/performance/cathode-small-live.json`
- `artifacts/performance/cathode-default-live-batched.json`
- `artifacts/performance/cathode-live-no-scope-kind-fastpath.json`
- `artifacts/performance/cathode-default-live-kind-fastpath.json`
- `artifacts/performance/cathode-default-live-paint-command.json`
- `artifacts/performance/cathode-default-live-paint-command-60f.json`
- `artifacts/performance/cathode-default-live-deferred-stream-rebuilds.json`
- `artifacts/performance/cathode-default-live-targeted-command-text.json`
- `artifacts/performance/cathode-default-live-table-overlay-text.json`
- `artifacts/performance/cathode-default-live-targeted-interactions.json`
- `artifacts/performance/cathode-default-live-overlay-primitives.json`
- `artifacts/performance/cathode-default-live-retained-leaf-primitives.json`
- `artifacts/performance/cathode-default-live-retained-leaf-primitives-60f.json`
- `artifacts/performance/cathode-default-live-targeted-line-plot-gating-60f.json`
- `artifacts/performance/cathode-default-live-targeted-animation-primitives-60f.json`
- `artifacts/performance/cathode-default-live-retained-animation-subtrees-60f.json`
- `artifacts/performance/cathode-default-live-partial-primitive-uploads-60f.json`
- `artifacts/performance/cathode-default-static-lazy-fallback-provenance.json`
- `artifacts/performance/cathode-default-live-lazy-fallback-provenance-60f.json`
- `artifacts/performance/subsystem-matrix-initial/matrix-summary.json`
- `artifacts/performance/subsystem-matrix-initial/matrix-summary.md`
- `artifacts/performance/subsystem-matrix-after-text-cache/matrix-summary.json`
- `artifacts/performance/subsystem-matrix-after-text-cache/matrix-summary.md`
- `artifacts/performance/cathode-default-live-layout-text-cache-60f.json`
- `artifacts/performance/cathode-default-live-provenance-key-cache-60f.json`
- `artifacts/performance/subsystem-provenance-key-cache/primitives.json`
- `artifacts/performance/subsystem-shared-provenance-map/primitives.json`
- `artifacts/performance/subsystem-presized-provenance-map/primitives.json`
- `artifacts/performance/subsystem-inline-provenance-chains/primitives.json`
- `artifacts/performance/cathode-default-live-inline-provenance-60f.json`
- `artifacts/performance/subsystem-bulk-cascade-timing/primitives.json`
- `artifacts/performance/cathode-default-live-bulk-cascade-timing-60f.json`
- `artifacts/performance/subsystem-sparse-style-merge/primitives.json`
- `artifacts/performance/subsystem-empty-style-fastpath/primitives.json`
- `artifacts/performance/cathode-default-live-sparse-style-merge-60f.json`
- `artifacts/performance/subsystem-deferred-authored-provenance/primitives.json`
- `artifacts/performance/cathode-default-live-deferred-authored-provenance-60f.json`
- `artifacts/performance/subsystem-selective-attribute-snapshots/primitives.json`
- `artifacts/performance/cathode-default-live-selective-attribute-snapshots-60f.json`
- `artifacts/performance/cathode-synthetic-hover-1.json`
- `artifacts/performance/cathode-synthetic-hover-2.json`
- `artifacts/performance/cathode-synthetic-hover-final.json`
- `artifacts/performance/cathode-synthetic-hover-no-tooltip-before.json`
- `artifacts/performance/cathode-synthetic-hover-no-tooltip-after.json`
- `artifacts/performance/cathode-synthetic-hover-30-old-overlays.json`
- `artifacts/performance/cathode-synthetic-hover-30-cached-overlay-gate.json`
- `artifacts/performance/cathode-synthetic-hover-cached-tooltip-safety.json`
- `artifacts/performance/cathode-channel-trace-regression.json`
- `artifacts/performance/cathode-channel-trace-fixed.json`
- `artifacts/performance/cathode-hover-stages-before-hit-cache.json`
- `artifacts/performance/cathode-hover-stages-after-hit-cache.json`
- `artifacts/performance/cathode-hover-after-transition-scan-gate.json`
- `artifacts/performance/cathode-keyed-backpressure-diagnostics-90f.json`
- `artifacts/performance/cathode-final-sustained-live-hover-1200f.json`
- `artifacts/performance/cathode-default-static-container-cache.json`
- `artifacts/performance/cathode-default-live-container-cache-60f.json`
- `artifacts/performance/cathode-default-live-atlas-trim-120f.json`
- `artifacts/performance/cathode-default-static-cascade-counters.json`
- `artifacts/performance/cathode-static-selector-label-cache-1.json`
- `artifacts/performance/cathode-static-selector-label-cache-2.json`
- `artifacts/performance/cathode-static-selector-label-cache-3.json`
- `artifacts/performance/cathode-static-cascade-substages.json`
- `artifacts/performance/cathode-static-provenance-substage.json`
- `artifacts/performance/cathode-static-shared-provenance-1.json`
- `artifacts/performance/cathode-static-shared-provenance-2.json`
- `artifacts/performance/cathode-static-shared-provenance-3.json`
- `artifacts/performance/cathode-static-cached-provenance-1.json`
- `artifacts/performance/cathode-static-cached-provenance-2.json`
- `artifacts/performance/cathode-static-cached-provenance-3.json`
- `artifacts/performance/cathode-live-cached-provenance-60f.json`
- `artifacts/performance/matrix/matrix-summary.json`
- `artifacts/performance/matrix/matrix-summary.md`
- `artifacts/performance/matrix-after-transition-text/matrix-summary.json`
- `artifacts/performance/matrix-after-transition-text/matrix-summary.md`
- `artifacts/performance/cathode-live-instrumented-no-dynamic-text.json`
- `artifacts/performance/cathode-live-partial-text-no-dynamic-text.json`
- `artifacts/performance/cathode-live-partial-text-default.json`
- `artifacts/performance/cathode-extension-line-clip-validation.json`

---

# Phase 1 — Repeatable Measurement

**Status:** In progress

## Completed

- [x] Add a fresh-process CATHODE profiling harness.
- [x] Capture static versus live behavior.
- [x] Capture full versus reduced widget counts.
- [x] Report Python, command, framework, renderer, and queue metrics.
- [x] Save machine-readable JSON reports.
- [x] Establish a release-build baseline.
- [x] Add independently selectable live-update group ablations.
- [x] Add quick, full, and nightly fresh-process matrix presets.
- [x] Write machine-readable and Markdown matrix summaries.
- [x] Compact selector diagnostics in persisted profile reports.
- [x] Report text atlas lifecycle and prepare-error counters.
- [x] Report nodes visited, rule candidates, matched declarations, provenance
      entries, and selector snapshot counts for the latest cascade.

## Remaining

- [x] Add a command-line matrix runner that summarizes multiple fresh-process
      reports into one comparison table.
- [ ] Record machine, adapter, build identity, Python version, and package path.
- [ ] Separate startup frames from steady-state frame statistics.
- [x] Record bounded p50/p95/p99, minimum, and maximum timing values.
- [ ] Add queue high-water, oldest-command age, production rate, and
      consumption rate.
- [ ] Add first-application-frame wall time directly to the runtime snapshot.
- [ ] Add a lightweight native snapshot path so profiling does not first build
      the full tree, layout, computed-style, and selector diagnostics.
- [x] Add profiling presets for short local, full local, and nightly workloads.
- [x] Add comparable fresh-process CSS-, text-, primitive-, table-, and
      scatter-heavy subsystem workloads and a matrix summary runner.
- [x] Add deterministic widget-centered synthetic hover profiling with handler
      and injection-to-presentation latency distributions.

---

# Phase 2 — Eliminate Redundant Whole-Tree Rebuilds

**Status:** In progress

## Completed

- [x] Route live class, sidebar-state, badge-level, and LED-state style
      invalidation through the deferred dirty accumulator.
- [x] Route `ReplaceNode` and `ReplaceChildren` rebuilds through the deferred
      dirty accumulator.
- [x] Ensure one command batch performs at most one final full rebuild for
      these commands.
- [x] Run the full native suite after the change.
- [x] Rebuild the release extension.
- [x] Reprofile the default live workload.
- [x] Audit runtime command handlers for direct rebuild calls.
- [x] Route line-plot, histogram, toast, and resource-release commands through
      the shared deferred dirty accumulator.
- [x] Add stable widget ownership and an owner index for retained base-tree
      text entries.
- [x] Recollect only affected widget subtrees for text-affecting CSS animation
      frames.
- [x] Route background/border-only keyframes directly to primitive rebuilding.
- [x] Preserve full text rebuild fallbacks for overlay and dataframe-table
      ordering that is not yet independently patchable.
- [x] Carry widget ids through deferred text invalidation for local property,
      style, histogram, and extension display-list commands.
- [x] Normalize overlapping targets to the shallowest dirty roots.
- [x] Preserve global fallbacks for untargeted, stale, unsupported, and overly
      broad text batches.
- [x] Add command-targeted text rebuild telemetry and profile the default live
      workload.
- [x] Split retained base, dataframe-table, virtual-overlay, and ephemeral
      scatter text into explicit renderer ranges.
- [x] Route dataframe data commands through table-only retained replacement.
- [x] Route toast show, dismissal, and expiration through overlay-only retained
      replacement.
- [x] Verify the overlay-only path with a live toast smoke test.
- [x] Route direct hover, focus, editing, selection, menu, dropdown, table,
      plot, drag/drop, and activation interactions through targeted retained
      replacement.
- [x] Normalize interaction roots and preserve stale, unsupported, and
      over-limit global fallbacks.
- [x] Expose interaction-specific targeted rebuild telemetry.
- [x] Retain overlay text and primitives for hover changes whose old and new
      targets have no tooltip, dropdown, or popup dependency.
- [x] Run the full native suite and reprofile the default live workload after
      migrating direct interaction paths.
- [x] Retain the base primitive range during overlay-only toast/menu/tooltip
      rebuilds.
- [x] Expose full versus overlay-only primitive rebuild telemetry and verify
      the path with painted base content in a live toast smoke.
- [x] Record retained primitive ranges for safe leaf widgets and explicit
      no-primitive text leaves.
- [x] Splice changed leaf ranges while maintaining following leaf and overlay
      offsets.
- [x] Preserve full primitive fallbacks for containers, scroll owners,
      transformed ancestors, tab attachment, and unsupported empty-to-painted
      transitions.
- [x] Add retained primitive attempt/completion/fallback telemetry, native
      range tests, and reusable success/fallback GUI smokes.
- [x] Skip the dedicated line-plot renderer when retained primitive targets do
      not contain a line plot.
- [x] Preserve dedicated line-renderer rebuilds for direct line-plot targets
      and targeted ancestors containing line plots.
- [x] Add targeted line-plot check/rebuild/skip telemetry and a positive live
      line-plot smoke.
- [x] Extend retained primitive ownership from leaves to safe complete
      subtrees, including descendant ranges and trailing scroll chrome.
- [x] Normalize overlapping animation targets and independently track visual
      versus text-affecting animation owners.
- [x] Route CSS animation and simultaneous toast/animation invalidation through
      retained subtree primitive replacement.
- [x] Retain source-to-pipeline routes during primitive splitting and patch
      same-shape retained subtree changes directly into affected GPU slices.
- [x] Preserve full split/upload fallbacks for count changes, pipeline-kind
      changes, invalid routes, and overlay regeneration.
- [x] Expose partial-buffer patch/fallback/byte telemetry, add native routing
      regressions, rebuild the release extension, and reprofile CATHODE.

## Remaining

- [ ] Add native regression tests that count one full rebuild per deferred
      command batch.
- [ ] Refactor stylesheet replacement/clearing so stylesheet parsing,
      invalidation, and the final full rebuild also participate in command
      batching without an eager cascade.
- [ ] Report which commands escalated a batch to `Text`, `Layout`, or `Full`.
- [x] Report dirty request counts by command and dirty level.
- [x] Report requested versus executed dirty rebuilds and deferred merge count.

---

# Phase 3 — Paint-Only Extension Updates

**Status:** Complete
**Priority:** P0

## Problem

`CrtScope.set_values()` calls `PaintWidget.repaint()`. The repaint serializes
and replaces the entire extension node. Although batching now prevents multiple
full passes in one batch, each telemetry frame still looks like a structural
replacement and forces CSS cascade plus layout.

## Required Work

- [x] Add a dedicated extension display-list update command.
- [x] Preserve extension node identity, style, callbacks, layout, and retained
      widget state.
- [x] Update only the display-list property.
- [x] Classify shape-only updates as `Dirty::Visual` and text-bearing updates
      as `Dirty::Text`.
- [x] Coalesce pending paint snapshots by extension widget id.
- [x] Keep a structural replacement fallback when image resource discovery or
      native compatibility requires it.
- [x] Keep a structural replacement fallback when measure/layout metadata
      changes.
- [x] Change `PaintWidget.repaint()` to use the paint-only command when safe.
- [x] Add Python and native coalescing regression tests.
- [x] Rebuild the release extension and profile short and sustained workloads.

## Acceptance Criteria

- Scope telemetry does not increment style or layout pass counts.
- Repaint command cost remains below the visual rebuild budget.
- Multiple pending scope frames retain only the newest display list.

All acceptance criteria pass in the current CATHODE profiles. Image-bearing
display lists intentionally retain the structural path because the image
renderer discovers and owns resources during layout rebuilds.

---

# Phase 4 — Incremental Style Invalidation

**Status:** In progress
**Priority:** P0

## Problem

An LED state change marks styles dirty because DragonGUI supports attribute
selectors such as `LED[state="on"]`. The runtime currently resolves this by
reapplying every stylesheet to all 1,628 widgets.

## Required Work

- [ ] Build selector dependency metadata when stylesheets are parsed:
  - Type, id, class, attribute, pseudo-state, and part dependencies
  - Descendant/child/sibling dependencies
  - `:has(...)` and structural selector dependencies
- [ ] For a local attribute or pseudo-state change, identify the smallest safe
      invalidation region.
- [ ] Recompute only the target for selectors without relational dependencies.
- [ ] Include ancestors, descendants, or siblings only when selector metadata
      requires them.
- [ ] Fall back to a whole-tree cascade for unknown or complex dependencies.
- [ ] Compare old and new computed styles to derive the actual dirty level:
  - Paint-only change → `Visual`
  - Text shaping change → `Text`
  - Geometry change → `Layout`
- [x] Cache container-query contexts and skip reevaluation when relevant
      container dimensions are unchanged.

## Completed

- [x] Add conservative stylesheet queries for attribute dependencies.
- [x] Narrow dependency queries by changed widget kind while retaining
      ancestor, descendant, and nested selector dependencies.
- [x] Classify LED `state` updates as visual-only when no applicable selector
      can observe the attribute.
- [x] Preserve full invalidation for direct `LED[state=...]` and relational
      dependencies such as `Panel:has(LED[state=...])`.
- [x] Add native regression coverage for global, direct-kind, nested, and
      unrelated-kind dependencies.
- [x] Reprofile LED updates independently from scope repaint.
- [x] Cache stable container-query contexts.
- [x] Reevaluate changed container contexts against the newly computed layout
      rather than the previous layout.

## Acceptance Criteria

- Toggling several LEDs does not traverse the entire widget tree when no
  relational selector depends on their state.
- Attribute-selector and `:has(...)` semantics remain correct.
- The default live profile spends less than 10 ms per telemetry tick in style
  invalidation on the current development machine.

---

# Phase 5 — Backpressure and Latest-State Semantics

**Status:** In progress
**Priority:** P0

This phase shares scope with
`plans/runtime-startup-backpressure-remediation.md`.

- [ ] Add keyed Python task coalescing.
- [ ] Schedule CATHODE telemetry with `coalesce_key="telemetry"`.
- [ ] Coalesce generic pending `SetProp` by `(widget_id, property)`.
- [x] Coalesce paint snapshots by widget id within command batches while
      preserving structural replacement barriers.
- [x] Preserve line-plot append ordering and every appended point, including
      mixed text/visual deferred rebuild batches.
- [ ] Treat structural mutations and event commands as ordering barriers.
- [ ] Expose produced, applied, coalesced, and dropped counts.

## Acceptance Criteria

- The producer cannot accumulate obsolete complete telemetry frames.
- Queue high-water remains bounded during a 60-second default live run.
- Stream commands preserve their documented semantics.

---

# Phase 6 — Style Engine Scaling

**Status:** In progress
**Priority:** P1

- [x] Index candidate rules by type, id, class, and key.
- [ ] Extend candidate indexing to attribute and pseudo-state dependencies.
- [ ] Avoid testing rules that cannot match a node.
- [ ] Cache stable selector matches across paint-only updates.
- [ ] Cache parsed generated-content and resolved custom-property chains.
- [ ] Profile container query evaluation separately from base cascade.
- [x] Add counters for nodes visited, candidates considered, matched
      declarations, provenance entries, and selector snapshots.
- [ ] Add computed-style change counts.
- [x] Separate selector-match, declaration-application, property-application,
      fallback, inheritance, provenance, and snapshot timings.
- [x] Replace per-declaration clocks with bulk provenance/property timing and
      expose style-merge and part-filter substages.
- [x] Merge sparse default/inline overlays in place without cloning untouched
      gradients, shadows, strings, vectors, or generated content.
- [x] Materialize only attributes referenced by applicable selectors instead
      of cloning every exposed widget attribute for every node.
- [x] Share immutable authored-declaration provenance across shorthand-expanded
      properties and all nodes matched by the same rule.
- [x] Cache normalized shorthand expansions and pseudo/part provenance keys
      across all nodes matched by the same declaration in one cascade.
- [x] Compact per-node provenance with shared property keys, pre-sized hash
      maps, and inline short candidate chains.
- [x] Materialize non-inherited native-fallback provenance only for diagnostic
      snapshots while retaining inheritance-critical text provenance in the
      live computed style.
- [x] Defer non-inherited authored stylesheet provenance to an on-demand
      diagnostic recascade while retaining inheritance-critical text
      provenance in the live computed style.
- [ ] Benchmark 100, 1,000, 2,000, and 5,000-node synthetic trees.

---

# Phase 7 — Frame and GPU Profiling

**Status:** In progress
**Priority:** P1

The present evidence says GPU work is not dominant, but the library still needs
better tools for workloads where it is.

- [ ] Separate first-frame and steady-state frame distributions.
- [x] Record bounded p50/p95/p99, minimum, and maximum for frame and framework
      stages.
- [ ] Add rolling p50/p95/p99/max for prepare, encode, submit, present, and
      total frame time.
- [ ] Add optional WGPU timestamp queries for major render passes where the
      adapter supports them.
- [ ] Report buffer uploads and bytes per frame.
- [ ] Report text glyph uploads and atlas churn.
- [x] Trim the glyph atlas after every presented frame.
- [x] Report atlas trim count, custom-glyph count, and prepare failures.
- [ ] Report primitive batch and draw-call counts per frame.
- [ ] Report skipped redraws and frames with no visual changes.
- [ ] Support trace export readable by Perfetto or Chrome tracing.

---

# Phase 8 — CPU Sampling and Trace Correlation

**Status:** Not started
**Priority:** P1

- [ ] Add optional native tracing spans around command, cascade, layout, text,
      primitive, and render stages.
- [ ] Export stable thread names and frame/command sequence ids.
- [ ] Document Windows sampling with an appropriate Rust/Python profiler.
- [ ] Profile Python producer work separately from the native event-loop thread.
- [ ] Correlate Python callback id, native command sequence, drain slice, and
      presented frame.
- [ ] Ensure profiling can be disabled with negligible release overhead.

---

## Performance Budgets

Initial budgets for this development machine:

- Static steady-state CPU frame work: below 8 ms average.
- Visual-only telemetry update: below 4 ms.
- Paint-only extension update: below 6 ms.
- Incremental LED state style update: below 2 ms per LED group.
- Full-tree style cascade at 1,628 widgets: tracked, but not acceptable per
  telemetry tick.
- Command drain slice: below 8 ms for ordinary state updates.
- Queue high-water after coalescing: below 128 commands.
- Ten-frame default live profile: at least 30 wall FPS after startup work is
  excluded.

These are diagnostic targets and should be recalibrated after percentile and
steady-state instrumentation is available.

The current ten-frame and 60-frame profiles exceed the FPS target. It remains
the regression floor until startup and steady-state distributions are reported
separately.

---

## Definition of Done

1. CATHODE profiling is repeatable and produces compact comparable reports.
2. Static, live, startup, command, style, layout, and GPU costs are separately
   observable.
3. Paint-only updates do not perform structural replacement.
4. Local style-state changes do not automatically cascade the whole tree.
5. Obsolete snapshot work is coalesced while stream ordering is preserved.
6. Queue depth remains bounded under the default live workload.
7. Performance changes have regression tests and measured before/after reports.
8. The default live workload remains responsive without weakening its stress
   scale.
