# Raspberry Pi Optimization Plan

Last updated: 2026-05-13

## Goal

Make DragonGUI feel deliberate and stable on Raspberry Pi 5 after the initial
port. The first port focused on getting the native backend running, protecting
Scatter3D, and adding basic LinePlot caps. This plan covers the next layer:
reducing CPU/GPU work in the rest of the widget stack, removing desktop-first
layout assumptions, and making Pi degradation explicit instead of accidental.

## Current State

Implemented or partially implemented:

- `DRAGONGUI_PROFILE=pi` selects conservative native runtime defaults.
- Pi profile defaults to GL/GLES and X11/XWayland for the current Pi path.
- Scatter3D has the most complete Pi work: point caps, lower LOD threshold,
  interactive render scale, chunked uploads, and V3 demo workload reductions.
- LinePlot has retained-point caps, live append coalescing, and deferred rebuilds
  for packed set/append paths.
- DataFrameTable has profile caps for page size, sample rows, and packed column
  buffer rows.
- The V3 demo has Pi-specific sizing, workload, font, and layout adjustments.

Known gaps:

- LinePlot rendering still emits many rectangle primitives per visible segment.
- Histogram and PieChart are mostly desktop-first retained primitive/text paths.
- Several live interaction paths still call immediate rebuilds.
- Text/primitive rebuilds are often coupled, which is expensive on Pi.
- Layout components such as `Badge`/`Tag` still rely on intrinsic desktop-style
  widths that can overflow compact scrollable regions.
- Python-side heavy-widget work can still be expensive before native caps reject
  or trim data.

## Constraints

- Optimize for Raspberry Pi 5, 64-bit Raspberry Pi OS, V3D GPU.
- Keep desktop behavior unchanged unless the change is an obvious correctness
  improvement.
- Prefer `RuntimeProfileSelection` and `DRAGONGUI_PROFILE=pi` over broad
  platform checks.
- Avoid adding new heavy dependencies.
- Keep all degradations visible and documented: lower caps, simplified styles,
  disabled effects, or lower-fidelity rendering should be intentional.

## Phase 0: Measurement and Guardrails

Before making larger changes, add repeatable probes so improvements can be
judged on Pi hardware.

Tasks:

- Add a small Pi benchmark script or extend the existing scatter performance lab
  to cover:
  - LinePlot streaming.
  - LinePlot hover.
  - Histogram rendering.
  - DataFrameTable scroll.
  - Page/sidebar scroll.
- Extend `app.debug_snapshot()` with per-frame counters that are cheap to collect:
  - primitive instance count.
  - text entry count.
  - last rebuild dirty kind.
  - last primitive rebuild time.
  - last text rebuild time.
  - command queue depth / drained command count.
  - queue latency / coalesced or dropped command count when available.
  - last heavy-widget upload byte count when available.
- Make these counters available for a later optional V3 demo status/debug display.
- Record the visual smoke needs for V3 screenshots at 800x480 and 1280x720;
  implement the full visual smoke checklist in Slice 13.
- Add documentation for a standard Pi test matrix:
  - `DRAGONGUI_WGPU_BACKEND=gl`
  - default Pi profile
  - 800x480 and 1280x720 windows
  - streaming on/off
  - auto stats on/off

Implementation details:

- Add a focused script under `benchmarking/`, for example
  `benchmarking/rpi_widget_probe.py`.
  - Reuse the existing data generators in `examples/all_features_v3_demo.py`
    where possible instead of inventing new datasets.
  - Keep each scenario short and explicit: start window, warm up, run one
    workload for N seconds, print debug snapshot summary.
  - Run with the same env as the helper script:
    `DRAGONGUI_PROFILE=pi`, `DRAGONGUI_WGPU_BACKEND=gl`,
    `DRAGONGUI_WINDOW_BACKEND=x11`.
- Add native counters to `WgpuState` in `native/src/runtime.rs`.
  - Candidate fields:
    - `last_dirty: Option<Dirty>`
    - `last_primitive_rebuild_ms: f64`
    - `last_text_rebuild_ms: f64`
    - `last_primitive_instance_count: usize`
    - `last_text_entry_count: usize`
    - `last_drained_command_count: usize`
  - Wrap `rebuild_primitives()` and `rebuild_text()` with `Instant::now()`.
  - Store `prims.rect_count` after `PrimitivesRenderer::rebuild`; the primitive
    renderer already maintains this count.
  - Store text entry count after `TextRendererDg::rebuild`. If no public count
    exists, add a cheap count field in `native/src/text/mod.rs`.
- Add these counters to the existing snapshot path in `native/src/runtime.rs`.
  - The debug snapshot functions already emit platform/profile/layout/style
    details. Add a compact `"performance"` or `"last_frame"` object rather than
    scattering fields across the top level.
- For queue depth/drained count:
  - Inspect `CommandBridge` / command drain code in `native/src/commands.rs` and
    `native/src/runtime.rs`.
  - Count how many commands are drained per runtime tick after
    `coalesce_runtime_command_batch`.
  - Do not add logging by default; expose the count in `debug_snapshot()`.
- Do not add V3 demo status UI in Slice 1. Slice 4 owns optional low-frequency
  demo display after passive queue/backpressure fields exist.

Acceptance:

- A Pi tester can run one command and capture comparable before/after numbers.
- Debug snapshots show whether a problem is primitive count, text count, queue
  depth, layout rebuilds, or data upload.

## Phase 1: LinePlot Correctness and Rebuild Control

LinePlot is the next highest-value target after Scatter3D because streaming
updates are frequent and visible.

Tasks:

- Fix queue-level append coalescing barriers.
  - Current queue merging should not merge a later append across same-widget
    `SetProp`, `SetStyle`, `ReplaceNode`, `ReplaceChildren`, `ClearLinePlotSeries`,
    or `SetLinePlotDataPacked` commands.
  - Safer first version: only merge with the immediately previous compatible
    append, or stop scanning at any command for the same line plot id.
- Route `ClearLinePlotSeries` through `rebuild_for_dirty(Dirty::Visual)` instead
  of calling `rebuild_primitives()` directly.
- Route LinePlot toolbar actions through `rebuild_for_dirty`.
  - Actions that affect labels/ticks/legend should return `Dirty::Text`.
  - Pure visual actions should return `Dirty::Visual`.
- Add tests for:
  - append coalescing does not cross `SetProp(window_size)`.
  - append coalescing does not cross `SetStyle`.
  - clear-series defers during a command batch.
  - toolbar dirty classification is correct.

Implementation details:

- Queue-level coalescing lives in `native/src/commands.rs` inside
  `CommandBridge::push`.
  - The current `AppendLinePlotPointsPacked` scan walks backward through queued
    commands and merges with any earlier compatible append.
  - Add a helper such as:
    - `fn command_targets_widget(command: &Command, id: &str) -> bool`
    - `fn command_is_line_plot_append_barrier(command: &Command, id: &str, series: &str) -> bool`
  - Conservative first fix:
    - Merge only if the first relevant command encountered while scanning
      backward is a compatible append.
    - Stop at any command targeting the same line plot id.
    - This keeps ordering correct even if it misses some merge opportunities.
- Batch-level coalescing lives in `native/src/runtime.rs` in
  `coalesce_adjacent_line_plot_appends`.
  - It already clears append indexes on any non-append command. Keep that
    behavior as the safer model.
  - Add tests near the existing `command_batch_coalesces_*` tests for both
    queue-level and batch-level behavior.
- `ClearLinePlotSeries` is handled in `native/src/runtime.rs` around the command
  application match.
  - Replace direct `gpu.rebuild_primitives()` with
    `gpu.rebuild_for_dirty(Dirty::Visual)`.
  - Keep the recorded dirty kind as `Dirty::Visual`.
- LinePlot toolbar handling is in `WgpuState::activate_line_plot_toolbar`.
  - Change it to compute a dirty value from the action:
    - `Fit`, pan/zoom mode, box mode, grid: `Dirty::Visual`.
    - `Axes`: `Dirty::Text`, because ticks/axis text and primitives change.
  - Call `self.rebuild_for_dirty(dirty)` instead of `self.rebuild_primitives()`.
- Tests:
  - Add command-queue tests in `native/src/commands.rs` if the queue internals
    are test-accessible; otherwise add focused runtime tests near the existing
    coalescing tests in `native/src/runtime.rs`.
  - Include a case: append A, `SetProp(window_size)`, append B must remain three
    ordered commands or at least must not merge B before the prop.

Acceptance:

- Fast V3 line streaming preserves command ordering.
- A burst of line commands causes at most one final rebuild per drained batch.
- Existing line plot behavior remains unchanged on desktop.

## Phase 2: LinePlot Rendering Fast Path

The current LinePlot renderer maps points and emits rectangle primitives for
segments. This is acceptable for small plots, but it is a poor fit for Pi when
streaming multiple series.

Tasks:

- Add Pi-aware segment budget controls.
  - Move `LINE_PLOT_MAX_SEGMENTS_PER_SERIES` into profile-aware configuration.
  - Lower the Pi budget when plots are small or streaming.
- Add a solid-line fast path.
  - On Pi, prefer solid lines for high-frequency streams or provide a
    `line_plot_simplify_style=true` profile option.
  - Avoid dashed/dotted subdivision for high segment counts.
- Evaluate a dedicated GPU line plot renderer.
  - Store line points in a vertex buffer instead of retained rect instances.
  - Render with a line-strip or quad-line pipeline.
  - Keep rectangle fallback for desktop or unsupported cases.
- Add simple downsampling by screen x bucket.
  - For each pixel column or small x bucket, keep min/max y and endpoints.
  - This preserves spikes better than uniform stride.
  - Apply only when visible point count exceeds a profile threshold.
- Add a Pi-aware hover fast path.
  - Avoid scanning every visible point on every pointer movement when plots are
    streaming or densely sampled.
  - Reuse the same x-bucket representation where possible, or add a compact
    per-series hover index that is rebuilt only when line data or plot bounds
    change.

Implementation details:

- Start with the retained primitive path in `native/src/primitives/mod.rs`.
  - `emit_line_plot_series` currently computes visible range, then applies a
    uniform stride based on `LINE_PLOT_MAX_SEGMENTS_PER_SERIES`.
  - Replace the uniform stride with a helper that yields representative points:
    `line_plot_simplified_points(points, plot, bounds, budget)`.
  - First implementation can return a temporary `Vec<[f32; 2]>`; optimize later
    if allocation shows up in the Phase 0 counters.
- Add profile-aware budget plumbing.
  - `native/src/runtime_profile.rs` should expose something like
    `line_plot_segment_budget()`.
  - Store the effective profile or budget where primitive emission can see it.
    If `PrimitivesRenderer::rebuild` does not currently receive the runtime
    profile, pass a lightweight rendering profile into the primitives rebuild.
  - Keep the existing desktop budget unchanged.
- Solid-line simplification:
  - In `push_styled_line_segment`, dashed/dotted styles can emit many small
    segments.
  - Add a Pi/high-count branch before pattern subdivision:
    - if profile says simplify styles and visible segment count is high, call
      `push_line_segment` as solid.
  - Make this profile-driven, not a permanent change to desktop output.
- Screen-bucket downsampling:
  - Map visible points into x pixel buckets using `map_line_plot_point`.
  - For each bucket, retain first point, min-y point, max-y point, and last
    point in x order.
  - Feed those representatives to the existing segment emission path.
  - This should preserve spikes better than `step_by(stride)`.
- Dedicated GPU renderer:
  - Treat this as a later subproject after budget/downsampling.
  - Likely files:
    - new `native/src/line_plot/` module or a section alongside primitives.
    - new WGSL shader for line quads.
    - runtime-owned GPU buffers keyed by line plot widget id.
  - Do not start here unless measurement proves retained primitives are still
    the bottleneck after downsampling.
- Hover lookup:
  - `WgpuState::nearest_line_plot_point` in `native/src/runtime.rs` currently
    scans visible points and maps candidates during pointer movement.
  - First safe improvement: cap candidate checks under Pi profile by searching
    a small x-window around the pointer using sorted x values / visible range
    bounds, then choose the nearest point from that bounded window.
  - If data is not sorted by x, fall back to the current scan or build an
    explicit hover index when line data is updated.

Acceptance:

- Three V3 demo line plots can stream at desktop-ish tick rates on Pi without
  queue buildup.
- Dashed/dotted line styles degrade gracefully instead of causing frame spikes.
- Visual output remains acceptable for inspection and trend monitoring.

## Phase 3: Text and Primitive Rebuild Separation

Text shaping and primitive rebuilds are both expensive on Pi. Several state
changes currently rebuild more than they need.

Tasks:

- Audit all direct calls to:
  - `rebuild_primitives()`
  - `rebuild_text()`
  - `rebuild_visuals()`
- Replace direct calls with `rebuild_for_dirty()` where batching is safe.
- Split text-only changes from primitive-only changes where possible.
  - Hover readouts may require text but not all primitives.
  - Grid toggles require primitives.
  - Legend/axis/tick toggles require text and primitives.
- Consider separate dirty states:
  - `Dirty::PrimitiveOnly`
  - `Dirty::TextOnly`
  - or keep the current enum but narrow call sites.

Implementation details:

- First do a mechanical call-site audit.
  - `grep -n "rebuild_primitives();\\|rebuild_text();\\|rebuild_visuals();" native/src/runtime.rs`
  - For each call, classify why it exists:
    - data changed
    - text changed
    - layout changed
    - hover/readout changed
    - toolbar/control state changed
- Prefer replacing direct calls with `rebuild_for_dirty`.
  - The batching hook already exists in `WgpuState::rebuild_for_dirty`.
  - It respects `defer_rebuilds` and merges dirty levels.
- Be careful with `Dirty::Text`.
  - Today `Dirty::Text` calls `rebuild_visuals()`, which rebuilds both text and
    primitives.
  - Do not rename it until all call sites are understood. A safer intermediate
    step is to add instrumentation from Phase 0 and prove which calls are hot.
- Candidate narrow changes:
  - Hover readouts that only alter text should avoid rebuilding primitive rects.
  - Selection rectangles and grid/axis toggles still need primitive rebuilds.
  - Widget state transitions may need both until transition ownership is split.
- If adding new dirty states:
  - Update `dirty_rank`, `merge_dirty`, `dirty_name`, runtime command logging,
    and tests near the existing dirty tests in `native/src/runtime.rs`.
  - Preserve the ordering rule: a stronger dirty kind must dominate weaker
    pending dirty kinds.

Acceptance:

- Pointer hover and toolbar interactions no longer cause unnecessary full visual
  rebuilds.
- Debug snapshots make rebuild type visible.
- No stale text or stale primitive artifacts after interactions.

## Phase 4: Histogram Optimization

Histogram currently computes bins in Python and renders one primitive per visible
bar. It is not a streaming path yet, but it can become expensive with larger bin
counts or frequent data changes.

Tasks:

- Add a native live histogram update command for packed bins.
  - Update edges/counts without replacing the whole widget tree.
  - Coalesce repeated histogram updates by id.
- Add Pi profile caps:
  - max bins rendered.
  - max labels/ticks.
  - optional toolbar default off for compact Pi plots.
- Add a visible-bin rendering budget.
  - Merge bins that map to the same pixel column when bin count exceeds plot
    width.
  - Preserve max count in merged bins.
- Consider moving basic numeric histogram binning to native for packed numeric
  columns.
  - Python can still compute by default.
  - Native path should be opt-in or profile-driven.

Implementation details:

- Python widget entry point is `Histogram` in `python/dragongui/widgets.py`.
  - `_compute_histogram_bins` currently computes edges/counts in Python.
  - `Histogram.set_data()` recomputes bins but raises for live widgets.
- Add a live command path similar to LinePlot.
  - Python side:
    - add `enqueue_set_histogram_bins_packed` to `python/dragongui/runtime.py`
      and native Python bindings in `native/src/commands.rs`.
    - in `Histogram.set_data()`, if live, compute bins and enqueue packed
      edges/counts instead of raising.
  - Native command:
    - add `Command::SetHistogramBinsPacked { id, edges, counts, ... }`.
    - parse/apply in `native/src/runtime.rs` by updating
      `node.props.histogram.edges` and `node.props.histogram.counts`.
    - return/use `Dirty::Visual` or `Dirty::Text` depending on whether labels
      and bounds changed. Initial safe value: `Dirty::Text`.
  - Coalescing:
    - in `CommandBridge::push`, remove older pending histogram bin updates for
      the same id.
    - in `coalesce_runtime_command_batch`, keep only the last histogram update
      per id.
- Visible-bin budget belongs in `emit_histogram` in
  `native/src/primitives/mod.rs`.
  - When `counts.len() > plot_width_px` or over a Pi cap, aggregate bins before
    emitting rects.
  - For count/probability/percent modes, use max or sum depending on intended
    visual semantics. Start with max for performance/trend visibility.
  - For density/cumulative modes, document and test the chosen behavior.
- Add profile caps to `RuntimeProfileSelection`.
  - Suggested helpers:
    - `histogram_max_bins_rendered()`
    - `histogram_tick_count_cap()`
    - `histogram_toolbar_default()`
- Tests:
  - Python: live `Histogram.set_data()` enqueues instead of raising.
  - Rust: repeated histogram updates coalesce; rendered bin budget caps rect
    count under Pi profile.

Acceptance:

- Histogram pages remain interactive on Pi with multiple histograms visible.
- Repeated histogram data updates coalesce like line/scatter updates.
- Pi bin caps are documented.

## Phase 5: DataFrameTable Pi Compact Mode

Tables have data caps, but the visual defaults still assume a desktop display.

Tasks:

- Add Pi profile defaults for table metrics:
  - row height.
  - header height.
  - index width.
  - column width.
- Add optional text density mode:
  - smaller row text.
  - fewer visible columns by default.
  - truncate with ellipsis instead of expensive large text bounds.
- Audit table scroll and sort paths.
  - Keep partial-buffer sorting behavior explicit.
  - Consider disabling typed sort or showing a limited-sort indicator when only a
    prefix of rows is buffered.
- Add tests for profile-specific table metrics.

Implementation details:

- Table metrics are in `native/src/table.rs`.
  - `metrics()` currently uses theme defaults: control height, row height,
    `index_w = 64`, `col_w = 140`.
  - Add either:
    - `metrics_for_profile(theme, sf, profile)`, or
    - profile-derived defaults stored on the table node/style before layout.
  - Prefer a profile-aware helper over hard-coding Pi checks in table rendering.
  - Choose one path and keep it shared; duplicating profile math in primitive
    and text renderers risks grid/text drift.
- Runtime profile helpers can live in `native/src/runtime_profile.rs`.
  - Suggested helpers:
    - `table_row_height() -> Option<f32>`
    - `table_header_height() -> Option<f32>`
    - `table_index_width() -> Option<f32>`
    - `table_column_width() -> Option<f32>`
- Wire profile into table text and primitive paths.
  - Primitive table drawing is in `native/src/primitives/mod.rs` under
    `WidgetKind::DataFrameTable`.
  - Text emission for table cells is in `native/src/text/mod.rs` under
    `WidgetKind::DataFrameTable`.
  - Both must use the same metrics helper to avoid text/grid mismatch.
- Python API:
  - Keep explicit user style values authoritative. Pi defaults should only apply
    when the user did not set table row/column metrics.
  - Optional: expose a `density="compact"` style/example, but do not require a
    new public API until the native defaults are proven.
- Partial-buffer sorting:
  - Sorting and table state live in `native/src/table.rs`,
    `native/src/resources.rs`, and related runtime handlers.
  - Add a flag in debug snapshot or table props indicating partial column
    buffers when `rows > buffered_rows`.
  - If disabling typed sort for partial buffers, do it only under Pi profile or
    only when the selected sort column is partial.

Acceptance:

- Tables fit useful information in an 800x480 Pi window.
- Scrolling tables does not cause visible stalls.
- Partial buffer behavior is documented and not misleading.

## Phase 6: Layout and Compact UI Correctness

Recent Pi work exposed layout assumptions around scrollbars and intrinsic widths.
These should be fixed in the library, not only patched in the V3 demo.

Tasks:

- Fix `Badge`/`Tag` behavior in constrained flow layouts.
  - Support `max_width`.
  - Clip or ellipsize text within the badge.
  - Ensure flow wrapping respects scrollbar gutter and parent content width.
- Keep `Sidebar` and `Page` scroll behavior covered by tests.
- Add regression tests for:
  - sidebar scrollbars do not overlap `NavItem`.
  - long badge/tag does not paint under sidebar scrollbar.
  - compact `FlowLayout` wraps or clips correctly.
- Add 800x480 viewport acceptance checks.
  - no primary panel is clipped without a scroll path.
  - left navigation scrolls independently.
  - content areas respect available height after title bars/toolbars.
  - fullscreen uses the monitor's logical size, not desktop assumptions.
- Add a font consistency audit.
  - Button labels, panel/frame titles, navigation, DataFrameTable values,
    Histogram labels, Badge/Tag text, and generic widget text should all resolve
    through the same theme/default font family unless explicitly styled.
- Add Pi compact theme defaults:
  - sidebar width.
  - control panel width.
  - plot minimum heights.
  - scrollbar gutter.
  - reduced font weights that resolve consistently on Pi.

Implementation details:

- `Sidebar` and `Page` scroll behavior is now library behavior.
  - Keep `is_scroll_container_kind` in `native/src/layout.rs` covered by tests
    for `Panel`, `Page`, and `Sidebar`.
  - Ensure primitive scrollbar emission in `native/src/primitives/mod.rs` works
    for all scroll container kinds, not just panels.
- Badge/Tag intrinsic width behavior is in `native/src/layout.rs`.
  - `apply_intrinsic_leaf_width` assigns intrinsic min-width for Badge/Tag in
    `FlowLayout`.
  - Add support for `max_width` by respecting `node.style.layout.max_width` /
    `max_width_value` when calculating intrinsic width.
  - If max width is lower than text width, let layout use the max width and rely
    on text clipping/ellipsis.
- Badge/Tag text rendering is in `native/src/text/mod.rs`; fill/border drawing
  is in `native/src/primitives/mod.rs`.
  - Add clipping bounds for standalone Badge/Tag text.
  - The text renderer already has clipping support, but not a clear reusable
    ellipsis path. Clip text to the badge rect as the first pass; add ellipsis
    later only if it is needed and tested.
  - Keep the badge background rect equal to the laid-out widget rect.
- FlowLayout wrapping:
  - Flow intrinsic height calculation is in `apply_flow_layout_intrinsic_height`.
  - Ensure it uses content width after padding and scrollbar gutter.
  - Add tests with a scrollable Sidebar containing a FlowLayout and a long Tag.
- Demo cleanup:
  - After library fix, remove sidebar badge width/renaming/padding workarounds
    from `examples/all_features_v3_demo.py`.
  - Keep only demo-level choices such as which badges to show.

Acceptance:

- The V3 demo does not require one-off badge/sidebar hacks.
- Compact layouts fail by clipping/scrolling predictably, not by painting through
  scrollbars.

## Phase 7: PieChart and HtmlReport Policy

These are less urgent than line plots and layout, but they need Pi-specific
policy.

Tasks:

- PieChart:
  - cap labels/legend rows on compact rects.
  - disable slice labels by default on small Pi plots.
  - keep legends clipped to the chart rect.
- HtmlReport:
  - keep embedded WebView unsupported on Pi unless a Linux webview backend is
    intentionally added.
  - make external fallback prominent in docs and demos.
  - avoid expensive inline HTML snapshots in routine Pi debug paths.

Implementation details:

- PieChart primitive and text paths are in `native/src/primitives/mod.rs` and
  `native/src/text/mod.rs`.
  - `pie_chart_text_labels` emits title, slice labels, and legend labels.
  - Add compact-rect checks before slice labels and legend labels:
    - if rect width/height below threshold, skip slice labels.
    - cap legend rows by available legend height.
  - Make thresholds profile-aware only if desktop behavior would change.
- HtmlReport behavior is split across:
  - `native/src/html_report_webview.rs`
  - `native/src/runtime.rs`
  - `python/dragongui/widgets.py`
  - `docs/raspberry-pi.md`
  - V3 demo `HtmlReport` panel.
  - Keep non-Windows embedded backend as unsupported unless a Linux webview
    dependency is intentionally selected.
  - Add clearer debug snapshot fields if needed:
    - `embedded_supported`
    - `external_fallback_available`
    - `last_open_external_status`
- Avoid routine heavy HTML snapshots.
  - Audit `debug_snapshot()` and V3 demo snapshot/report actions so they do not
    serialize large inline HTML unless explicitly requested.

Acceptance:

- Pie charts remain readable in compact grids.
- HtmlReport behavior is explicit and does not look like a broken renderer.

## Phase 8: Documentation and Demo Cleanup

Tasks:

- Update `docs/raspberry-pi.md` with the real validated command path:
  - `DRAGONGUI_PROFILE=pi`
  - `DRAGONGUI_WGPU_BACKEND=gl`
  - `DRAGONGUI_WINDOW_BACKEND=x11`
  - optional `DRAGONGUI_DEMO_WIDTH` / `DRAGONGUI_DEMO_HEIGHT`.
- Update the older RPi progress audit with actual hardware findings:
  - V3D shader location limit.
  - GL/X11 default.
  - line plot streaming fixes.
  - compact layout work.
- Keep V3 demo Pi constants minimal.
  - Move reusable Pi defaults into the library profile where possible.
  - Keep demo-only workload choices in the demo.
- Treat `rpi_setup_and_run.sh` as a supported Pi entry point.
  - Check Python environment creation.
  - Check native build mode and missing package diagnostics.
  - Confirm GL/X11 defaults match the docs.
  - Confirm rerunning the script is idempotent.

Implementation details:

- Update `docs/raspberry-pi.md`.
  - Prefer commands that match `rpi_setup_and_run.sh`.
  - Include direct V3 run commands for both default and explicit 800x480 runs.
  - Document when to use `DRAGONGUI_WGPU_BACKEND=vulkan` only as a diagnostic,
    not the default.
- Update `plans/RPi/raspberry-pi-5-port-progress-audit.md`.
  - Mark the original pre-hardware status as stale.
  - Add a hardware validation section for:
    - V3D shader inter-stage output limit.
    - GL/X11 path.
    - line plot deferred/coalesced work.
    - sidebar/page scroll work.
    - remaining layout/font issues.
- Demo cleanup in `examples/all_features_v3_demo.py`.
  - Remove constants once the library owns them:
    - generic compact table metrics.
    - generic plot minimum heights.
    - scrollbar/badge layout workarounds.
  - Keep constants that are truly demo workload choices:
    - point count.
    - table rows.
    - stream frame count.
    - line stream interval/batch size.
- Add a short note to the plan or docs explaining the split:
  - library Pi defaults are reusable behavior.
  - demo Pi constants are workload choices for the showcase.

Acceptance:

- A fresh Pi tester can follow docs without knowing the development history.
- Demo workarounds do not hide missing library behavior.

## Gap Audit

This pass looks for missing implementation coverage in the plan rather than
repeating the code-side safety audit.

High-priority gaps to add to implementation work:

- **Font consistency is not fully owned.**
  - The plan mentions reduced font weights in compact theme defaults, but the
    actual bug reports were broader: panel titles, widget text, data values,
    histogram labels, frame panel text, and buttons resolved to different fonts.
  - Add a small font audit task to Phase 6:
    - trace all text emission paths in `native/src/text/mod.rs`;
    - verify each path receives the same resolved theme/default font family;
    - include Button, Frame/Panel title, DataFrameTable, Histogram, Badge/Tag,
      and navigation text in a Pi snapshot test or visual checklist.
  - Document required Pi font packages or bundled font fallback in
    `docs/raspberry-pi.md` if the library depends on system fonts.
- **Viewport/aspect-ratio behavior needs explicit acceptance criteria.**
  - The plan covers Sidebar/Page scroll and compact defaults, but it does not
    define what "fits on the Pi screen" means after fullscreen/window scaling.
  - Add a Phase 6 acceptance check for 800x480:
    - no primary panel is clipped without a scroll path;
    - left navigation scrolls independently;
    - content areas respect available height after title bars/toolbars;
    - fullscreen uses the monitor's logical size, not desktop assumptions from
      the development machine.
- **Queue backpressure is under-specified.**
  - The runtime already has queue depth and queue latency signals. Phase 0 should
    expose those consistently, then later phases should define limits.
  - Add a follow-up task after Phase 1:
    - cap or coalesce high-frequency commands before they can build unbounded
      queue latency;
    - expose dropped/coalesced command counts in the debug snapshot;
    - make the V3 demo show queue buildup clearly when streaming is too fast.
- **GPU upload and memory budgets are not explicit outside Scatter3D.**
  - Scatter3D has chunking/LOD work, but retained primitives, text atlases,
    table buffers, histogram bins, and line point buffers can still create Pi
    memory pressure.
  - Add Phase 0 counters or debug fields for:
    - primitive buffer bytes;
    - text entry count / atlas pressure if available;
    - line/histogram/table retained data sizes;
    - last upload byte count for heavy widget updates.
- **Screenshot-level validation is missing.**
  - Unit tests can catch layout math, but the current Pi issues were visual:
    overlap, wrong fonts, clipped panels, and scrollbar collisions.
  - Add a lightweight visual smoke checklist or script:
    - launch V3 at 800x480 and 1280x720;
    - capture screenshots of Grid, Scatter3D, LinePlot, Histogram, Table, and
      HtmlReport panels;
    - compare manually at first, then automate only if the screenshots stabilize.
- **Build/install path is only in documentation cleanup.**
  - The plan should treat `rpi_setup_and_run.sh` as part of the supported Pi
    surface, not just docs.
  - Add checks for:
    - Python environment creation;
    - native build mode used on Pi;
    - GL/X11 env defaults;
    - missing system package diagnostics;
    - whether rerunning the script is idempotent.
- **Input behavior is not covered.**
  - Pi deployments may use touchscreens, trackpads, or mouse wheel scrolling.
  - Add a compact interaction checklist:
    - sidebar wheel/drag scroll;
    - table scroll;
    - plot pan/zoom;
    - toolbar button hit targets at 800x480;
    - hover behavior when no precise mouse hover exists.
- **Failure diagnostics need a user-facing path.**
  - The original crashes around deferred/limit behavior should produce useful
    signals when they reappear.
  - Add a debug snapshot field or log line for:
    - last dirty/deferred rebuild state;
    - command queue depth/latency;
    - last widget id/type that triggered a heavy rebuild;
    - profile caps currently in effect.

Lower-priority gaps to keep visible:

- Define a target frame budget for Pi, even if approximate: for example 30 FPS
  steady interaction, with a known lower target during heavy table/histogram
  updates.
- Decide whether 800x480 is the primary supported display or only a minimum
  smoke target. This affects sidebar width, plot minimum height, table density,
  and button hit sizes.
- Decide how much visual degradation is acceptable by widget. Line plots can
  simplify line style under Pi; tables and histograms may need different
  degradation rules.
- Add a profile-cap inventory so Python and Rust do not grow separate,
  inconsistent Pi defaults.

## Patch Scope Audit

Use these as implementation slices. Each slice should be small enough to review
on its own and should avoid mixing correctness fixes with larger performance
rewrites.

### Slice 1: Pi Measurement Snapshot

Purpose:

- Add cheap performance counters before changing behavior.

Likely files:

- `native/src/runtime.rs`
- `native/src/text/mod.rs`
- `native/src/primitives/mod.rs`
- `python/dragongui/app.py`
- `python/dragongui/runtime.py`
- `examples/debug_snapshot_tool.py`

Scope:

- Add rebuild timing, last dirty kind, primitive count, text count, queue depth,
  queue latency, drained count, and optional upload bytes to debug snapshot.
- Reuse existing `PrimitivesRenderer::rect_count`.
- Add only a cheap text count accessor/field.
- Do not add logging spam or visual demo UI in this slice.

Tests/checks:

- Rust debug snapshot tests in `native/src/runtime.rs` / `native/src/commands.rs`.
- Python `app.debug_snapshot()` shape check if a test harness exists.
- Manual run of `examples/debug_snapshot_tool.py`.

Risk:

- Low if counters are passive and default output shape remains backward
  compatible.

### Slice 2: Repeatable Pi Probe

Purpose:

- Create a repeatable command for before/after measurements.

Likely files:

- new `benchmarking/rpi_widget_probe.py`
- `examples/all_features_v3_demo.py` only for reusable data helpers if needed
- `docs/raspberry-pi.md`

Scope:

- Probe LinePlot streaming, LinePlot hover, Histogram render, DataFrameTable
  scroll, and Page/Sidebar scroll.
- Print compact snapshot summaries.
- Avoid asserting performance thresholds until real Pi baseline numbers exist.

Tests/checks:

- Run on dev machine to verify it starts and exits.
- Run on Pi with GL/X11 env once Slice 1 counters exist.

Risk:

- Low. Keep it outside library runtime behavior.

### Slice 3: LinePlot Command Ordering

Purpose:

- Fix correctness before making LinePlot faster.

Likely files:

- `native/src/commands.rs`
- `native/src/runtime.rs`

Scope:

- Prevent queue-level append coalescing from crossing same-widget barriers.
- Route `ClearLinePlotSeries` through `rebuild_for_dirty(Dirty::Visual)`.
- Route LinePlot toolbar actions through `rebuild_for_dirty`.
- Do not change rendering/downsampling in this slice.

Tests/checks:

- Command queue tests for append, barrier, append ordering.
- Runtime batch coalescing tests near existing `command_batch_coalesces_*`
  tests.
- Manual V3 line stream sanity check.

Risk:

- Medium. Command ordering is central, but the conservative merge rule is safer
  than aggressive coalescing.

### Slice 4: Queue Backpressure Visibility

Purpose:

- Make overload visible before dropping/coalescing more aggressively.

Likely files:

- `native/src/commands.rs`
- `native/src/runtime.rs`
- `examples/all_features_v3_demo.py`

Scope:

- Count coalesced commands and queue latency consistently.
- Add optional low-frequency V3 status display for queue buildup.
- Do not drop commands yet unless there is already a safe coalescing rule.

Tests/checks:

- Unit tests for coalesced count changes.
- Manual high-frequency LinePlot stream to confirm queue buildup is visible.

Risk:

- Low to medium. Passive metrics are low risk; dropping commands should be a
  later, separate slice.

### Slice 5: Rebuild Call-Site Cleanup

Purpose:

- Reduce unnecessary immediate rebuilds without changing rendering output.

Likely files:

- `native/src/runtime.rs`

Scope:

- Audit direct calls to `rebuild_primitives()`, `rebuild_text()`, and
  `rebuild_visuals()`.
- Replace only clearly batch-safe calls with `rebuild_for_dirty`.
- Leave new dirty enum variants for a later slice unless measurement proves the
  current dirty levels are the bottleneck.

Tests/checks:

- Existing runtime dirty tests.
- Add targeted tests for the changed call sites.
- Manual toolbar/hover checks in V3.

Risk:

- Medium. Incorrect dirty classification can cause stale visuals, so keep each
  call-site group small.

### Slice 6: Sidebar/Page/Badge Layout Correctness

Purpose:

- Fix visible Pi layout overflow in the library.

Likely files:

- `native/src/layout.rs`
- `native/src/primitives/mod.rs`
- `native/src/text/mod.rs`
- `examples/all_features_v3_demo.py` only to remove workarounds after library
  fixes land

Scope:

- Preserve Sidebar/Page scroll behavior.
- Ensure scrollbar gutter affects content width.
- Add Badge/Tag `max_width` handling in intrinsic sizing.
- Clip Badge/Tag text to the laid-out rect.
- Do not redesign the V3 sidebar in this slice.

Tests/checks:

- Layout tests for Sidebar/Page scroll containers.
- Badge/Tag long text test with scrollbar gutter.
- Text clipping test for Badge/Tag.
- Manual 800x480 V3 screenshot.

Risk:

- Medium. Layout changes can affect desktop, so keep desktop snapshots/manual
  smoke checks in the loop.

### Slice 7: Font Consistency

Purpose:

- Resolve remaining Pi font mismatches across text paths.

Likely files:

- `native/src/text/mod.rs`
- `native/src/theme.rs`
- `native/src/runtime.rs`
- `native/src/css_style.rs` if CSS font resolution is involved
- `docs/raspberry-pi.md`
- `examples/all_features_v3_demo.py` for visual confirmation only

Scope:

- Trace Button, Frame/Panel title, navigation, DataFrameTable values,
  Histogram labels, Badge/Tag, and generic Label text.
- Ensure each path uses the resolved theme/default font unless explicitly
  styled.
- Document required Pi font package/fallback if the runtime relies on system
  fonts.
- Do not change typography scale or layout metrics in this slice.

Tests/checks:

- Text renderer tests that inspect emitted font family where possible.
- Visual screenshot checklist at 800x480 and desktop size.

Risk:

- Medium. Font changes are visible and may alter layout; keep this separate from
  layout sizing changes.

### Slice 8: 800x480 Viewport and Fullscreen Fit

Purpose:

- Make the Pi display target explicit and testable.

Likely files:

- `native/src/runtime.rs`
- `native/src/layout.rs`
- `examples/all_features_v3_demo.py`
- `docs/raspberry-pi.md`

Scope:

- Verify fullscreen/window sizing uses monitor logical size.
- Ensure main content has a scroll path when height is insufficient.
- Keep navigation scroll independent.
- Move reusable compact defaults into profile/theme only after library behavior
  is correct.

Tests/checks:

- Manual V3 800x480 and 1280x720 screenshots.
- Layout tests for Page/Sidebar available height where possible.

Risk:

- Medium. This touches user-facing layout behavior; avoid bundling with font
  fixes.

### Slice 9: LinePlot Rendering Fast Path

Purpose:

- Reduce primitive count and pointer hover cost after correctness is fixed.

Likely files:

- `native/src/primitives/mod.rs`
- `native/src/runtime_profile.rs`
- `native/src/runtime.rs`

Scope:

- Add profile-aware segment budget.
- Add screen-x bucket downsampling.
- Add solid-style simplification only under Pi/high-count conditions.
- Add bounded hover lookup in `nearest_line_plot_point`.
- Do not start the dedicated GPU line renderer in this slice.

Tests/checks:

- Unit tests for downsampling preserving endpoints/min/max.
- Primitive count comparison from Slice 1 counters.
- Manual V3 stream and hover checks on Pi.

Risk:

- Medium to high. Rendering simplification can change visuals; gate through Pi
  profile and keep desktop defaults unchanged.

### Slice 10: Histogram Live Packed Updates

Purpose:

- Avoid full widget replacement and prepare Histogram for Pi-friendly updates.

Likely files:

- `python/dragongui/widgets.py`
- `python/dragongui/runtime.py`
- `native/src/commands.rs`
- `native/src/runtime.rs`
- `native/src/document.rs`
- `native/src/primitives/mod.rs`

Scope:

- Add `enqueue_set_histogram_bins_packed`.
- Add native `SetHistogramBinsPacked` command and coalescing.
- Update live `Histogram.set_data()` to enqueue packed bins.
- Initial dirty level can be `Dirty::Text` for correctness.
- Keep native binning optional and out of this slice.

Tests/checks:

- Python live histogram set_data no longer raises.
- Rust command coalescing tests.
- Manual V3 histogram update check.

Risk:

- Medium. Crosses Python/native boundary; keep the command payload simple and
  test invalid edges/counts.

### Slice 11: Histogram Render Budget

Purpose:

- Reduce primitive/text load for dense histograms.

Likely files:

- `native/src/primitives/mod.rs`
- `native/src/text/mod.rs`
- `native/src/runtime_profile.rs`

Scope:

- Cap rendered bins under Pi profile.
- Merge bins that map to the same pixel column.
- Cap ticks/labels in compact plots.
- Document semantics for count/probability/percent/density/cumulative modes.

Tests/checks:

- Primitive count tests under Pi profile.
- Visual checks for each histogram mode.

Risk:

- Medium. Bin aggregation semantics are easy to get subtly wrong, so keep it
  separate from live command work.

### Slice 12: DataFrameTable Compact Metrics

Purpose:

- Make tables usable on 800x480 without text/grid drift.

Likely files:

- `native/src/table.rs`
- `native/src/runtime_profile.rs`
- `native/src/primitives/mod.rs`
- `native/src/text/mod.rs`
- `native/src/runtime.rs`
- `python/dragongui/widgets.py` only if public API/docs need a density option

Scope:

- Add one shared profile-aware metrics path.
- Keep explicit user CSS/style values authoritative.
- Add partial-buffer debug indication before changing sort behavior.
- Do not redesign table virtualization in this slice.

Tests/checks:

- `metrics_for_node` profile tests.
- Primitive/text table alignment tests if feasible.
- Manual table scroll check on Pi.

Risk:

- Medium. Metrics affect layout and rendering; shared helper is mandatory.

### Slice 13: Visual Smoke and Input Checklist

Purpose:

- Catch the issues unit tests miss.

Likely files:

- new `benchmarking/rpi_visual_smoke.md` or script under `benchmarking/`
- `docs/raspberry-pi.md`
- possibly `tools/` if there is an existing smoke-test pattern to reuse

Scope:

- Define screenshot list for Grid, Scatter3D, LinePlot, Histogram, Table,
  PieChart, HtmlReport, and navigation.
- Define input checklist for wheel/drag/touch/toolbar hit targets.
- Start manual; automate later only if stable enough.

Tests/checks:

- Checklist itself is the output.

Risk:

- Low.

### Slice 14: Pi Setup Script and Docs

Purpose:

- Make the supported run path reliable and repeatable.

Likely files:

- `rpi_setup_and_run.sh`
- `docs/raspberry-pi.md`
- `docs/raspberry-pi-release-checklist.md`
- `plans/RPi/raspberry-pi-5-port-progress-audit.md`

Scope:

- Confirm env defaults match working Pi path.
- Add missing package diagnostics.
- Check rerun/idempotency behavior.
- Add direct V3 demo commands.
- Do not bundle runtime rendering fixes here.

Tests/checks:

- Run script on Pi.
- Run docs commands manually.

Risk:

- Low to medium depending on install behavior. Avoid destructive cleanup.

### Slice 15: PieChart and HtmlReport Policy

Purpose:

- Close lower-priority compact widget and unsupported-backend gaps.

Likely files:

- `native/src/primitives/mod.rs`
- `native/src/text/mod.rs`
- `native/src/html_report_webview.rs`
- `native/src/runtime.rs`
- `python/dragongui/widgets.py`
- `docs/raspberry-pi.md`
- `examples/all_features_v3_demo.py`

Scope:

- Cap/clip PieChart labels and legends in compact plots.
- Make HtmlReport embedded unsupported state and external fallback explicit.
- Avoid heavy HTML snapshot generation unless explicitly requested.

Tests/checks:

- Visual PieChart compact checks.
- HtmlReport debug snapshot/manual external fallback check.

Risk:

- Low to medium. Keep webview dependency decisions out of this slice.

### Slices To Avoid

- Do not combine font consistency with layout sizing changes.
- Do not combine Histogram live command plumbing with bin aggregation semantics.
- Do not start a dedicated GPU LinePlot renderer until Slice 9 proves retained
  primitives are still the bottleneck.
- Do not remove V3 demo workarounds until the library fix and screenshot check
  for that specific workaround have landed.
- Do not add broad Pi-only conditionals when a profile helper or existing style
  path can carry the behavior.

## Test Coverage Audit

The repo already has native Rust tests embedded in `native/src/*.rs` and Python
tests under `tests/`. Use those first. Add manual Pi checks only where unit
tests cannot see GPU/backend/font/display behavior.

Baseline commands:

- Native unit tests: run from `native/` with `cargo test`.
- Python unit tests: run from repo root with `pytest`.
- Python tests run in dev fallback mode through `tests/conftest.py`, so they
  should not open a native window.
- Pi validation should use:
  - `DRAGONGUI_PROFILE=pi`
  - `DRAGONGUI_WGPU_BACKEND=gl`
  - `DRAGONGUI_WINDOW_BACKEND=x11`

### Coverage by Slice

1. **Pi Measurement Snapshot**
   - Rust:
     - add snapshot shape tests near `WgpuState::debug_snapshot_value` in
       `native/src/runtime.rs` where feasible;
     - add bridge/snapshot completion tests in `native/src/commands.rs` only if
       command-side fields change.
   - Python:
     - add or update `tests/test_python_api.py` for `App.debug_snapshot()` shape
       only if the Python wrapper behavior changes.
   - Manual/dev:
     - run `examples/debug_snapshot_tool.py`.
   - Pi:
     - confirm counters are nonzero on V3 demo after rendering LinePlot,
       Histogram, and DataFrameTable.

2. **Repeatable Pi Probe**
   - Rust: none required unless new native snapshot fields are added.
   - Python:
     - add a lightweight import/argument smoke test if the probe has CLI args.
   - Manual/dev:
     - run the probe in a short mode and confirm it exits.
   - Pi:
     - capture baseline output for LinePlot stream, hover, Histogram, table
       scroll, and sidebar/page scroll.

3. **LinePlot Command Ordering**
   - Rust:
     - add tests in `native/src/commands.rs` for append + same-widget barrier +
       append;
     - add tests near existing `command_batch_coalesces_*` tests in
       `native/src/runtime.rs`;
     - verify append does not cross `SetProp`, `SetStyle`, `ReplaceNode`,
       `ReplaceChildren`, `ClearLinePlotSeries`, or `SetLinePlotDataPacked`.
   - Python: none unless public runtime methods change.
   - Manual/dev:
     - run V3 LinePlot stream with fast tick settings.
   - Pi:
     - verify stream remains ordered and no deferred-limit crash returns.

4. **Queue Backpressure Visibility**
   - Rust:
     - test coalesced/drained/latency counters on controlled command batches.
   - Python:
     - add snapshot-field assertions only if exposed through a stable Python
       shape.
   - Manual/dev:
     - stress LinePlot stream and verify queue buildup appears in snapshot/demo
       status.
   - Pi:
     - record queue latency before and after LinePlot rendering optimizations.

5. **Rebuild Call-Site Cleanup**
   - Rust:
     - add targeted dirty/deferred tests in `native/src/runtime.rs` for each
       changed command or toolbar path;
     - verify `flush_deferred_rebuilds` still preserves strongest dirty kind.
   - Python: none expected.
   - Manual/dev:
     - check toolbar actions, hover, clear series, and style changes for stale
       visuals.
   - Pi:
     - compare rebuild counters before/after during V3 line/histogram controls.

6. **Sidebar/Page/Badge Layout Correctness**
   - Rust:
     - add layout tests in `native/src/layout.rs` for Sidebar/Page scroll
       containers, scrollbar gutter, Badge/Tag `max_width`, and long badge in
       constrained `FlowLayout`;
     - add text clipping tests in `native/src/text/mod.rs`;
     - add primitive scrollbar geometry tests in `native/src/primitives/mod.rs`
       only if primitive emission changes.
   - Python: none unless demo workarounds become public API changes.
   - Manual/dev:
     - screenshot V3 navigation with long badges.
   - Pi:
     - verify no badge/tag text paints under the sidebar scrollbar at 800x480.

7. **Font Consistency**
   - Rust:
     - add text emission tests in `native/src/text/mod.rs` where emitted entries
       expose font family/style;
     - add CSS parsing tests in `native/src/css_style.rs` only if font
       resolution changes there.
   - Python: none expected.
   - Manual/dev:
     - screenshot Button, Panel/Frame titles, navigation, DataFrameTable values,
       Histogram labels, Badge/Tag text.
   - Pi:
     - verify these paths resolve to the same visible font or documented
       fallback.

8. **800x480 Viewport and Fullscreen Fit**
   - Rust:
     - add layout tests for Page/Sidebar available height where possible.
   - Python:
     - add V3 demo config tests only if env parsing/default sizing logic changes.
   - Manual/dev:
     - run V3 at explicit 800x480 and 1280x720.
   - Pi:
     - test fullscreen and explicit 800x480; every clipped area must have a
       usable scroll path.

9. **LinePlot Rendering Fast Path**
   - Rust:
     - unit test downsampling helper for endpoints, min/max y preservation, and
       empty/single-point inputs;
     - test profile budget helpers in `native/src/runtime_profile.rs`;
     - test hover bounded lookup against current nearest-point behavior for
       sorted data and fallback behavior for unsorted data.
   - Python: none expected.
   - Manual/dev:
     - compare debug primitive count before/after.
   - Pi:
     - verify fast V3 streams do not build queue latency and hover remains
       correct enough for inspection.

10. **Histogram Live Packed Updates**
    - Rust:
      - add command enum/coalescing tests in `native/src/commands.rs`;
      - add runtime apply tests if histogram props can be updated without a full
        event loop.
    - Python:
      - add `tests/test_python_api.py` coverage that live `Histogram.set_data()`
        enqueues instead of raising;
      - test invalid edges/counts remain rejected.
    - Manual/dev:
      - run a small live histogram update tool or V3 histogram action.
    - Pi:
      - verify repeated histogram updates do not visibly stall or grow queue
        depth.

11. **Histogram Render Budget**
    - Rust:
      - add primitive emission tests for Pi bin caps;
      - add mode-specific tests for count, probability, percent, density, and
        cumulative semantics if aggregation changes values.
    - Python: none expected unless public props/options are added.
    - Manual/dev:
      - visual check dense histograms before/after.
    - Pi:
      - verify histogram pages remain interactive with multiple histograms.

12. **DataFrameTable Compact Metrics**
    - Rust:
      - extend `native/src/table.rs` tests around `metrics_for_node`;
      - add profile-specific metrics tests;
      - add text/primitive alignment tests if helper APIs make that feasible.
    - Python:
      - add API tests only if a public `density` option is added.
    - Manual/dev:
      - inspect table scroll and column/header alignment.
    - Pi:
      - verify useful rows/columns fit at 800x480 and scrolling does not stall.

13. **Visual Smoke and Input Checklist**
    - Rust/Python: none unless a script is added.
    - Manual/dev:
      - capture V3 screenshots for Grid, Scatter3D, LinePlot, Histogram, Table,
        PieChart, HtmlReport, and navigation at 800x480 and 1280x720.
    - Pi:
      - repeat screenshots and test wheel/drag/touch/toolbar hit targets.

14. **Pi Setup Script and Docs**
    - Rust/Python: none expected.
    - Manual/dev:
      - shellcheck if available; otherwise review script paths and env defaults.
    - Pi:
      - run `rpi_setup_and_run.sh` from a clean-ish checkout;
      - rerun it to confirm idempotency;
      - run the direct V3 command documented in `docs/raspberry-pi.md`.

15. **PieChart and HtmlReport Policy**
    - Rust:
      - add PieChart compact label/legend tests in `native/src/text/mod.rs` or
        `native/src/primitives/mod.rs`;
      - add HtmlReport support/fallback snapshot tests if fields change.
    - Python:
      - add widget serialization/API tests only if public behavior changes.
    - Manual/dev:
      - check compact PieChart and HtmlReport fallback messaging.
    - Pi:
      - verify HtmlReport does not appear as a broken embedded renderer and
        external fallback is clear.

### Coverage Gaps to Close Before Coding

- There is no automated screenshot comparison yet. Treat visual smoke as manual
  until the V3 layout stabilizes.
- There is no reliable off-Pi substitute for Pi font resolution, fullscreen
  logical sizing, V3D limits, or GL/X11 behavior.
- Performance thresholds are not defined yet. Slice 1 and Slice 2 must collect
  baseline numbers before later slices can assert pass/fail performance budgets.
- Avoid adding brittle tests against exact pixel text positions until fonts and
  compact metrics are stable.

## Regression Risk Audit

This audit names the behavior each slice could accidentally break. The default
rule is: desktop output and public Python behavior stay unchanged unless the
change is a correctness fix, and Pi-specific degradations must be profile-gated
or explicitly documented.

### Cross-Cutting Regression Rules

- Prefer `RuntimeProfileSelection` / `DRAGONGUI_PROFILE=pi` for Pi-only behavior.
- Keep `DRAGONGUI_PROFILE=desktop` and unset profile behavior as the compatibility
  baseline.
- Keep user-specified styles and widget props authoritative over profile
  defaults.
- Avoid changing public Python method signatures unless the plan explicitly says
  to add a new API.
- If a degradation changes visual semantics, expose it in debug snapshot or docs.
- Prefer passive counters before active throttling or dropping commands.

### Risk Matrix by Slice

1. **Pi Measurement Snapshot**
   - Could break:
     - debug snapshot consumers if fields move or top-level shape changes;
     - frame time if counters allocate or lock heavily.
   - Guardrail:
     - add counters under a nested `"performance"` / `"last_frame"` object;
     - keep existing fields intact;
     - avoid cloning renderer/text buffers.
   - Rollback:
     - remove snapshot fields without affecting runtime behavior.

2. **Repeatable Pi Probe**
   - Could break:
     - nothing in library runtime if kept as an external script;
     - developer workflow if it imports V3 demo and triggers side effects.
   - Guardrail:
     - keep probe datasets/helpers isolated or import-safe;
     - add a short mode for local smoke testing.
   - Rollback:
     - disable/remove probe script; no library impact.

3. **LinePlot Command Ordering**
   - Could break:
     - append throughput if coalescing becomes too conservative;
     - command ordering if a barrier is missed;
     - clear/set interactions if dirty classification is wrong.
   - Guardrail:
     - correctness beats merge rate;
     - stop scanning at any same-widget command unless proven safe;
     - add tests for each barrier type.
   - Rollback:
     - keep batch-level coalescing and revert only queue-level merge changes.

4. **Queue Backpressure Visibility**
   - Could break:
     - snapshot compatibility if queue fields are renamed;
     - demo performance if status updates become too frequent.
   - Guardrail:
     - make V3 display low-frequency/optional;
     - avoid dropping commands in this slice.
   - Rollback:
     - disable demo display while keeping passive snapshot counters.

5. **Rebuild Call-Site Cleanup**
   - Could break:
     - stale text, stale primitives, or missed redraws;
     - interaction state if dirty kinds are too weak;
     - desktop behavior if direct rebuilds were relied on for immediate output.
   - Guardrail:
     - change one call-site group at a time;
     - use stronger dirty kind when uncertain;
     - verify toolbar, hover, selection, clear, style, and data paths manually.
   - Rollback:
     - revert individual call-site changes without reverting instrumentation.

6. **Sidebar/Page/Badge Layout Correctness**
   - Could break:
     - desktop intrinsic sizes for Badge/Tag;
     - FlowLayout wrapping;
     - scrollbar hit testing if gutter math changes;
     - existing demos that rely on current pill widths.
   - Guardrail:
     - preserve desktop defaults except overflow correctness;
     - only apply `max_width` when explicitly set or constrained;
     - add layout tests for both constrained and unconstrained badges.
   - Rollback:
     - keep Sidebar/Page scroll fixes and revert Badge/Tag intrinsic changes
       separately if needed.

7. **Font Consistency**
   - Could break:
     - layout widths/heights because text metrics change;
     - explicit CSS/user font choices;
     - desktop typography if default resolution changes globally.
   - Guardrail:
     - do not override explicit `font_family` / `font_weight`;
     - fix inconsistent fallback resolution rather than forcing a new font;
     - keep this separate from layout metric changes.
   - Rollback:
     - gate Pi fallback font behavior through profile/docs while keeping bug
       fixes for obviously wrong text paths.

8. **800x480 Viewport and Fullscreen Fit**
   - Could break:
     - desktop window sizing;
     - fullscreen behavior on non-Pi monitors;
     - V3 demo layout assumptions.
   - Guardrail:
     - keep monitor logical-size fixes platform-neutral only if correct;
     - keep Pi compact defaults profile/demo scoped;
     - verify both explicit window size and fullscreen.
   - Rollback:
     - revert demo constants separately from library fullscreen/layout fixes.

9. **LinePlot Rendering Fast Path**
   - Could break:
     - visual fidelity, spikes, dashed/dotted style rendering, hover precision;
     - desktop output if budget/simplification is not profile-gated;
     - data interpretation if downsampling loses extrema.
   - Guardrail:
     - keep desktop budget unchanged;
     - preserve endpoints, min-y, max-y per bucket;
     - simplify styles only under Pi/high-count conditions;
     - retain current path as fallback.
   - Rollback:
     - expose/keep profile flag to disable downsampling or style simplification.

10. **Histogram Live Packed Updates**
    - Could break:
      - public Python behavior if `Histogram.set_data()` semantics change too
        broadly;
      - native command parsing if invalid edges/counts slip through;
      - labels/bounds if dirty kind is too weak.
    - Guardrail:
      - preserve current non-live behavior;
      - validate edges/counts on Python and native sides;
      - use `Dirty::Text` initially for correctness.
    - Rollback:
      - keep command type but make live `set_data()` fall back to the old error
        until native behavior is fixed.

11. **Histogram Render Budget**
    - Could break:
      - statistical meaning of density/cumulative modes;
      - visual comparison with desktop;
      - tick/label readability.
    - Guardrail:
      - make bin aggregation profile-gated;
      - document per-mode aggregation semantics;
      - start with count-like modes before changing density/cumulative.
    - Rollback:
      - profile flag/env option to disable histogram aggregation.

12. **DataFrameTable Compact Metrics**
    - Could break:
      - desktop table dimensions;
      - CSS table metric overrides;
      - text/grid alignment;
      - table sort/selection visibility.
    - Guardrail:
      - one shared metrics helper for layout/primitive/text paths;
      - explicit style values win over profile defaults;
      - do not change sorting behavior in the compact metrics slice.
    - Rollback:
      - disable Pi metric overrides while keeping partial-buffer diagnostics.

13. **Visual Smoke and Input Checklist**
    - Could break:
      - no runtime behavior if kept as checklist/script;
      - CI reliability if visual checks are made mandatory too early.
    - Guardrail:
      - keep manual until screenshot output stabilizes.
    - Rollback:
      - remove from automated checks, keep as docs/checklist.

14. **Pi Setup Script and Docs**
    - Could break:
      - user environments if script modifies global state;
      - repeat runs if setup is not idempotent;
      - docs if commands drift from real script behavior.
    - Guardrail:
      - avoid destructive cleanup;
      - check before installing/building where possible;
      - keep env defaults in one visible place.
    - Rollback:
      - revert script changes independently of runtime changes.

15. **PieChart and HtmlReport Policy**
    - Could break:
      - desktop PieChart label behavior if compact rules are not gated;
      - HtmlReport expectations if unsupported embedded backend is presented as
        a regression;
      - debug snapshot size if HTML content is serialized routinely.
    - Guardrail:
      - compact PieChart changes should be rect/profile-aware;
      - keep external HtmlReport fallback explicit;
      - avoid adding Linux webview dependencies in this slice.
    - Rollback:
      - disable compact PieChart label caps or fallback messaging separately.

### Highest-Risk Changes

- LinePlot downsampling/style simplification.
- Histogram aggregation semantics.
- Font fallback changes.
- Shared layout changes for Badge/Tag and scroll gutters.
- Dirty rebuild reclassification.

These should receive the most manual validation and should not be bundled with
unrelated cleanup.

## Performance Budget Audit

These are initial implementation budgets, not final product claims. Slice 1 and
Slice 2 must capture real Pi baselines before any number below becomes a hard
pass/fail gate.

Current Pi profile caps already present in code:

- Scatter3D max points: `200_000`.
- LinePlot max retained points per series/path: `50_000`.
- DataFrameTable page size: `64`.
- DataFrameTable sample rows: `512`.
- DataFrameTable column buffer rows: profile-capped in the native runtime.

### Budget Classes

- **Hard compatibility budget:** must hold immediately.
  - Desktop defaults stay unchanged.
  - Explicit user styles/props override Pi defaults.
  - No unbounded command queue growth from normal V3 demo interactions.
- **Initial Pi target:** desired starting target, can be adjusted after hardware
  measurement.
- **Observed baseline:** must be filled in by Slice 1/Slice 2 on the actual Pi.

### Initial Runtime Targets

- Idle frame pacing:
  - Initial Pi target: stable redraws without continuous rebuilds when the UI is
    idle.
  - Measurement: rebuild counters should remain flat while idle.
- Interactive target:
  - Initial Pi target: 30 FPS-feeling interaction for sidebar scroll, table
    scroll, LinePlot hover, and plot toolbar use.
  - Observed baseline needed: actual frame time from Pi probe.
- Heavy update target:
  - Initial Pi target: visible progress without crashes or queue runaway during
    LinePlot streaming and repeated Histogram updates.
  - Observed baseline needed: queue latency and rebuild timings during V3 fast
    stream.

### Initial Counter Budgets

These should be exposed by debug snapshot before enforcement:

- Command queue depth:
  - Initial Pi target: returns toward zero between normal stream ticks.
  - Warning threshold placeholder: sustained depth above `2 * drained_per_tick`.
  - Hard failure signal: monotonically growing depth during steady V3 stream.
- Queue latency:
  - Initial Pi target: below one visible interaction interval for normal updates.
  - Placeholder warning threshold: sustained latency above `250 ms`.
  - Hard failure signal: latency climbs until user input feels seconds behind.
- Primitive rebuild time:
  - Initial Pi target: under one 30 FPS frame budget for common interactions
    after optimization.
  - Placeholder warning threshold: sustained rebuilds above `33 ms`.
  - Observed baseline needed per page: Grid, LinePlot, Histogram, Table.
- Text rebuild time:
  - Initial Pi target: text rebuilds should not occur for pure visual changes.
  - Placeholder warning threshold: sustained text rebuilds above `16 ms`.
- Primitive instance count:
  - Initial Pi target: LinePlot and Histogram optimizations should reduce counts
    compared with baseline on the same view.
  - Placeholder warning threshold: any single panel producing tens of thousands
    of retained rects should be investigated.
- Text entry count:
  - Initial Pi target: compact views should cap labels/ticks/legend text rather
    than emitting offscreen text.
  - Placeholder warning threshold: text count grows with hidden/offscreen table
    rows or dense chart ticks.
- Upload bytes:
  - Initial Pi target: streaming updates upload incremental data when possible.
  - Placeholder warning threshold: repeated full-buffer uploads during normal
    LinePlot append or Histogram update.

### Widget Budgets

- Scatter3D:
  - Keep current Pi point cap at `200_000` unless measurement shows a need to
    lower it.
  - Budget focus: avoid regressing the existing fast path.
- LinePlot:
  - Keep retained point cap at `50_000` until rendering counters prove a lower
    cap is necessary.
  - Add separate rendered segment budget under Pi profile.
  - Initial rendered segment target: no more segments than can be inspected at
    screen resolution; start with screen-x bucket representatives when visible
    points exceed plot width.
  - Hover target: bounded lookup should inspect a small x-window, not every
    visible point, for sorted data.
- Histogram:
  - Add rendered-bin budget separate from stored bin data.
  - Initial target: rendered bars should not exceed plot pixel width.
  - Tick/label target: compact Pi plots should cap ticks/labels before text
    entry count becomes the bottleneck.
- DataFrameTable:
  - Keep page size cap at `64` initially.
  - Compact metric target: enough rows visible at 800x480 to make scrolling
    useful, without text/grid mismatch.
  - Sorting target: no misleading full-table sort claim when only partial
    buffers are available.
- Sidebar/Page/Badge:
  - Layout budget is correctness-oriented:
    - no content painted under scrollbar;
    - no clipped primary panel without scroll;
    - no long badge forcing the navigation wider than the Pi viewport.
- PieChart/HtmlReport:
  - Lower priority. Budget is readability and avoiding expensive hidden work:
    cap labels on compact charts and avoid routine large HTML snapshots.

### Enforcement Order

1. Add counters and probes.
2. Record Pi baseline numbers at 800x480 and 1280x720.
3. Set warning thresholds in docs/debug output.
4. Optimize one widget path at a time.
5. Only then consider hard limits, dropped updates, or automatic degradation.

### Open Budget Decisions

- Final target FPS: 30 FPS is the initial practical target, but actual Pi
  baseline may suggest separate targets for scrolling, streaming, and heavy
  chart rebuilds.
- Whether 800x480 is the primary supported size or minimum smoke size.
- Whether Pi profile should automatically simplify line styles, or whether this
  should be an explicit profile/widget option.
- Whether histogram aggregation should use max, sum, or mode-specific semantics
  for each display mode.
- Whether table compact metrics should be profile-only or exposed as a public
  density option later.

## Dependency and Sequencing Audit

This audit turns the priority list into implementation order. The goal is to
avoid starting risky optimizations before measurement, correctness, and rollback
paths are in place.

### Hard Blockers

- Slice 1, Pi Measurement Snapshot, blocks:
  - Slice 2 Pi probe usefulness.
  - Slice 4 queue backpressure thresholds.
  - Slice 9 LinePlot rendering proof.
  - Slice 11 Histogram render-budget proof.
  - Slice 12 DataFrameTable compact-metric proof.
- Slice 3, LinePlot Command Ordering, blocks:
  - Slice 9 LinePlot rendering fast path.
  - Any command dropping/throttling beyond passive backpressure metrics.
- Slice 6, Sidebar/Page/Badge Layout Correctness, blocks:
  - removing V3 demo badge/sidebar workarounds.
  - treating 800x480 screenshots as meaningful pass/fail artifacts.
- Slice 7, Font Consistency, blocks:
  - brittle screenshot comparisons involving text.
  - final Pi font/package documentation.
- Slice 8, 800x480 Viewport and Fullscreen Fit, blocks:
  - final visual smoke acceptance at Pi size.
  - final V3 compact-layout cleanup.
- Slice 10, Histogram Live Packed Updates, blocks:
  - Slice 11 only for live histogram update performance. Slice 11 can still
    optimize startup/static histogram rendering independently.

### Recommended Implementation Order

1. Slice 1: Pi Measurement Snapshot.
2. Slice 2: Repeatable Pi Probe.
3. Slice 3: LinePlot Command Ordering.
4. Slice 4: Queue Backpressure Visibility.
5. Slice 5: Rebuild Call-Site Cleanup.
6. Slice 6: Sidebar/Page/Badge Layout Correctness.
7. Slice 7: Font Consistency.
8. Slice 8: 800x480 Viewport and Fullscreen Fit.
9. Slice 9: LinePlot Rendering Fast Path.
10. Slice 10: Histogram Live Packed Updates.
11. Slice 11: Histogram Render Budget.
12. Slice 12: DataFrameTable Compact Metrics.
13. Slice 13: Visual Smoke and Input Checklist.
14. Slice 14: Pi Setup Script and Docs.
15. Slice 15: PieChart and HtmlReport Policy.

### Work That Can Run In Parallel

- Slice 2 probe scaffolding can start while Slice 1 counters are being added, as
  long as final probe output waits for Slice 1 fields.
- Slice 13 visual checklist can be drafted at any time, but screenshots should
  be recaptured after Slices 6, 7, and 8.
- Slice 14 documentation/script review can start early, but final docs should
  wait for validated env defaults, direct V3 command, and known limitations.
- Slice 15 PieChart/HtmlReport policy can run after Slice 1 counters if it does
  not touch layout/font work.
- DataFrameTable compact metrics can be designed while LinePlot work is in
  progress, but should land after Slice 1 counters and the shared metrics path
  is clear.

### Work To Delay

- Do not implement LinePlot downsampling or style simplification before Slice 3
  command correctness is done.
- Do not implement command dropping/throttling before Slice 4 proves queue
  latency/depth behavior with passive counters.
- Do not remove V3 demo layout workarounds before the corresponding library
  fix has a screenshot/manual check.
- Do not automate screenshot comparisons before font consistency and 800x480
  layout have stabilized.
- Do not add a dedicated GPU LinePlot renderer until retained primitive
  downsampling has been measured and proven insufficient.
- Do not change Histogram aggregation semantics in the same patch as live
  histogram command plumbing.

### Milestones

- **Milestone A: Measurable Baseline**
  - Complete Slices 1 and 2.
  - Output: debug counters, probe command, first Pi baseline numbers.
- **Milestone B: Correct Streaming Foundation**
  - Complete Slices 3, 4, and 5.
  - Output: ordered LinePlot commands, visible queue pressure, fewer unnecessary
    rebuilds.
- **Milestone C: Compact UI Foundation**
  - Complete Slices 6, 7, and 8.
  - Output: scrollable Pi-sized UI, consistent fonts, stable 800x480 behavior.
- **Milestone D: Widget Performance Pass**
  - Complete Slices 9, 10, 11, and 12.
  - Output: optimized LinePlot, Histogram, and DataFrameTable behavior under Pi
    profile with desktop defaults intact.
- **Milestone E: Validation and Release Readiness**
  - Complete Slices 13, 14, and 15.
  - Output: visual/input checklist, reliable Pi run path, updated docs, explicit
    lower-priority widget policy.

### Sequencing Risks

- Starting with layout/font work before measurement may make performance
  regressions harder to attribute.
- Starting with LinePlot rendering before command correctness may hide ordering
  bugs behind faster rendering.
- Combining Slices 6, 7, and 8 into one patch makes it hard to tell whether a
  screenshot change came from layout, font metrics, or viewport sizing.
- Delaying Slice 14 too long can leave docs and `rpi_setup_and_run.sh` out of
  sync with the real validated command path.

## API Compatibility Audit

This audit separates public/semi-public behavior from internal native details.
The implementation should prefer additive APIs and profile-gated defaults.

### Public and Semi-Public Surfaces

- Python widgets in `python/dragongui/widgets.py`.
- Runtime handles in `python/dragongui/runtime.py`.
- `App.debug_snapshot()` in `python/dragongui/app.py` and native snapshot JSON.
- Environment variables:
  - `DRAGONGUI_PROFILE`
  - `DRAGONGUI_WGPU_BACKEND`
  - `DRAGONGUI_WINDOW_BACKEND`
  - `DRAGONGUI_DEV_FALLBACK`
  - `DRAGONGUI_SMOKE_FRAMES`
  - HtmlReport-related env vars.
- CSS/style properties:
  - `font_family` / `font-family`
  - `font_weight` / `font-weight`
  - `max_width` / `max-width`
  - table metric props such as `table-row-height`,
    `table-header-height`, `table-column-width`, and `table-index-width`.
- Demo and setup entry points:
  - `examples/all_features_v3_demo.py`
  - `rpi_setup_and_run.sh`
  - `docs/raspberry-pi.md`

### Compatibility Rules

- Do not rename existing Python methods, widget constructor arguments, or style
  keys.
- Add new `enqueue_*` methods only as additive runtime helpers.
- Keep debug snapshot additions under new nested objects or new fields; do not
  remove or repurpose existing fields.
- Keep existing env var values valid. New env values should have clear fallback
  behavior.
- Desktop/unset profile behavior must remain the baseline.
- Pi profile defaults may lower caps or simplify visuals only when documented
  and visible in profile/debug output.
- Explicit user CSS/style/widget props must override profile defaults.

### Slice-Specific API Notes

1. **Pi Measurement Snapshot**
   - Snapshot fields are semi-public because examples and diagnostics can read
     them.
   - Add fields under `"performance"` / `"last_frame"` or another compact nested
     object.
   - Avoid moving existing platform/profile/layout fields.

2. **Repeatable Pi Probe**
   - Probe script is developer tooling, not library API.
   - If it accepts CLI args, keep defaults stable enough for docs.

3. **LinePlot Command Ordering**
   - Native command coalescing is internal.
   - Public behavior should only become more correct: appends must preserve
     ordering around prop/style/tree changes.

4. **Queue Backpressure Visibility**
   - Queue metrics in snapshot are semi-public diagnostics.
   - Do not add command dropping as hidden behavior in this slice.
   - If a future throttle/drop policy is added, expose counters and profile/docs.

5. **Rebuild Call-Site Cleanup**
   - Internal behavior only.
   - Public API should not change; visible output should remain equivalent.

6. **Sidebar/Page/Badge Layout Correctness**
   - `Badge` and `Tag` constructor behavior should remain compatible.
   - `max_width` support should follow existing style semantics.
   - Do not make badge text truncation a required Python API change.

7. **Font Consistency**
   - Explicit `font_family` and CSS font declarations must keep winning.
   - If a Pi font package or fallback is required, document it rather than
     silently changing all desktop typography.

8. **800x480 Viewport and Fullscreen Fit**
   - Window sizing behavior is user-visible.
   - Keep explicit width/height env vars and widget props authoritative.
   - Any fullscreen logical-size fix should be correctness-oriented, not a Pi
     special case unless necessary.

9. **LinePlot Rendering Fast Path**
   - Rendering simplification should not change Python LinePlot API.
   - If users need control, add profile/widget options additively.
   - Desktop line style output should remain unchanged by default.

10. **Histogram Live Packed Updates**
    - `Histogram.set_data()` currently raises for live widgets. Changing live
      behavior to work is additive, but non-live behavior must remain unchanged.
    - Add `enqueue_set_histogram_bins_packed` as additive handle API.
    - Validate packed edges/counts consistently with existing `HistogramBins`
      validation.

11. **Histogram Render Budget**
    - Stored histogram data should remain unchanged; only rendered bins should
      be capped/merged under Pi profile.
    - If aggregation affects visual semantics, expose/document profile behavior.

12. **DataFrameTable Compact Metrics**
    - Existing CSS table metric overrides must remain authoritative.
    - Pi compact metrics should apply only when no explicit user metric is set.
    - Do not add a public `density` option until native defaults are proven.

13. **Visual Smoke and Input Checklist**
    - Checklist/script is not public API unless documented as a supported tool.

14. **Pi Setup Script and Docs**
    - `rpi_setup_and_run.sh` becomes a supported entry point once documented.
    - Keep script idempotent and avoid global/destructive changes.
    - Docs must match script env defaults.

15. **PieChart and HtmlReport Policy**
    - PieChart compact label caps should be profile/rect-aware, not a desktop
      API change.
    - HtmlReport unsupported embedded backend should be explicit, but existing
      external fallback behavior should remain available.

### API Compatibility Checks Before Each Slice Lands

- Does `pytest` still pass in dev fallback mode?
- Do existing widget constructors serialize the same document shape unless the
  slice intentionally adds a field?
- Does `DRAGONGUI_PROFILE=desktop` preserve current caps/defaults?
- Does `DRAGONGUI_PROFILE=pi` expose its caps/degradations in debug/profile
  output?
- If a debug snapshot field was added, was it additive?
- If a user style prop exists for the same behavior, does it override the
  profile default?

## Rollback and Feature Flag Audit

This audit decides which planned changes need an explicit switch or fallback.
Use profile helpers for normal Pi defaults. Use environment variables only for
diagnostics, temporary emergency escape hatches, or existing backend selection.

### Flag Placement Rules

- `RuntimeProfileSelection`:
  - preferred home for stable Pi defaults and caps;
  - values should appear in debug/profile output.
- Widget props or CSS/style:
  - use when the behavior is a durable user-facing choice;
  - explicit user values must override profile defaults.
- Environment variables:
  - use sparingly for backend selection, diagnostics, smoke runs, or temporary
    rollback switches;
  - document any env var that users are expected to set.
- Internal fallback path:
  - use for implementation safety when no user-visible control is needed.

### Required Rollbacks / Gates

- **LinePlot rendered segment budget**
  - Control: `RuntimeProfileSelection` helper such as
    `line_plot_segment_budget()`.
  - Default: desktop `None`; Pi `Some(...)` after measurement.
  - Rollback: set Pi helper to `None` or very high budget.
  - Test: desktop profile keeps current primitive output budget.

- **LinePlot style simplification**
  - Control: profile helper such as `line_plot_simplify_styles()`.
  - Default: desktop `false`; Pi `false` until measurement proves dashed/dotted
    subdivision is a bottleneck, then `true` only above a high segment count.
  - Rollback: helper returns `false`.
  - Test: desktop dashed/dotted rendering unchanged.

- **LinePlot screen-bucket downsampling**
  - Control: profile helper plus internal fallback to current stride/simple path.
  - Default: desktop disabled; Pi enabled only when visible point count exceeds
    threshold.
  - Rollback: disable helper or threshold.
  - Test: endpoints and min/max y preserved; desktop unchanged.

- **LinePlot bounded hover lookup**
  - Control: internal fallback to current full scan for unsorted data or failed
    assumptions.
  - Default: enabled only when data/order assumptions are safe.
  - Rollback: fall back to full scan.
  - Test: nearest point matches current behavior for sorted test data.

- **Command dropping/throttling**
  - Control: do not implement until passive queue metrics exist.
  - Default: no dropping.
  - Rollback: keep dropping disabled; retain coalescing only.
  - Test: command order and callback semantics unchanged.

- **Histogram live packed updates**
  - Control: additive command path.
  - Default: live `Histogram.set_data()` uses packed update when native support
    exists.
  - Rollback: live `set_data()` can return to old RuntimeError while keeping the
    command code disabled/unused.
  - Test: non-live `set_data()` behavior unchanged.

- **Histogram rendered-bin aggregation**
  - Control: profile helper such as `histogram_max_bins_rendered()` and
    optional aggregation-mode helper.
  - Default: desktop disabled; Pi disabled until baseline, then profile-capped.
  - Rollback: helper returns `None`.
  - Test: stored bin data unchanged; only rendered output is capped.

- **Histogram tick/label caps**
  - Control: profile helper such as `histogram_tick_count_cap()`.
  - Default: desktop `None`; Pi cap after visual validation.
  - Rollback: helper returns `None`.
  - Test: explicit user `tick_count` behavior remains predictable and documented.

- **DataFrameTable compact metrics**
  - Control: profile helpers for row/header/index/column metrics.
  - Default: desktop `None`; Pi values only when no explicit CSS/style metric is
    set.
  - Rollback: helpers return `None`.
  - Test: `table-row-height`, `table-header-height`, `table-column-width`, and
    `table-index-width` still override profile defaults.

- **Font fallback changes**
  - Control: prefer documented Pi package/fallback over a new public flag.
  - Default: explicit font styles win; fallback only fills missing/default font
    paths.
  - Rollback: revert fallback resolution while keeping explicit font handling.
  - Test: user `font_family` remains authoritative.

- **Badge/Tag clipping and `max_width`**
  - Control: existing style semantics, not a new flag.
  - Default: only constrained/explicit max-width badges clip.
  - Rollback: revert Badge/Tag intrinsic width changes separately from
    Sidebar/Page scroll behavior.
  - Test: unconstrained badges keep current pill sizing.

- **800x480 compact demo/library defaults**
  - Control: profile/demo constants.
  - Default: desktop unchanged; Pi compact defaults only under Pi profile or V3
    demo Pi mode.
  - Rollback: remove demo constants or set profile helpers to `None`.
  - Test: explicit user window width/height stays authoritative.

- **PieChart compact label caps**
  - Control: rect/profile-aware helper.
  - Default: desktop unchanged; Pi/compact rect caps only after visual check.
  - Rollback: disable cap helper.
  - Test: desktop labels/legend rows unchanged.

- **HtmlReport embedded fallback**
  - Control: existing platform support and HtmlReport env vars.
  - Default: no new Linux webview dependency; unsupported embedded state remains
    explicit with external fallback.
  - Rollback: disable new messaging/snapshot fields independently.
  - Test: external fallback remains available.

### Temporary Env Vars To Consider Only If Needed

Avoid adding these unless implementation risk justifies an emergency switch:

- `DRAGONGUI_PI_DISABLE_LINE_DOWNSAMPLING`
- `DRAGONGUI_PI_DISABLE_HISTOGRAM_AGGREGATION`
- `DRAGONGUI_PI_DISABLE_COMPACT_TABLE_METRICS`
- `DRAGONGUI_PI_DISABLE_FONT_FALLBACK`

If added, document them as temporary diagnostics and include them in debug
snapshot/profile output.

### Rollback Checks Before Each Risky Slice Lands

- Can the behavior be disabled without reverting unrelated fixes?
- Is the disabled/default path exactly the old desktop behavior?
- Is the enabled path visible in debug snapshot/profile output?
- Does an explicit user style/widget prop override the profile default?
- Is the rollback covered by at least one test or manual validation step?

## Pi Hardware Validation Audit

This audit separates what can be trusted from local tests from what must be
verified on actual Raspberry Pi hardware. Local tests can prove logic; Pi tests
must prove backend, display, font, input, and frame pacing behavior.

### Local-Only Validation

Can be done on the development machine:

- Rust unit tests with `cargo test` from `native/`.
- Python unit tests with `pytest` from the repo root.
- Document serialization and public Python API behavior.
- Command coalescing/order tests.
- Layout math tests that do not depend on system fonts or real monitor size.
- Runtime profile helper tests for `desktop` and `pi`.
- Debug snapshot shape tests.
- Script static review for `rpi_setup_and_run.sh`.

### Must Validate on Raspberry Pi

Requires actual Pi 5 hardware and desktop session:

- GL/GLES backend creation with `DRAGONGUI_WGPU_BACKEND=gl`.
- X11/XWayland window path with `DRAGONGUI_WINDOW_BACKEND=x11`.
- Vulkan comparison path with `DRAGONGUI_WGPU_BACKEND=vulkan`.
- Native Wayland path when explicitly testing Vulkan/Wayland.
- V3D shader/inter-stage limits and downlevel wgpu limits.
- Actual frame pacing during LinePlot streaming, hover, table scroll, and
  sidebar/page scroll.
- Fullscreen logical size and explicit 800x480 behavior.
- Font fallback/installed font behavior.
- Touchscreen, trackpad, mouse wheel, and toolbar hit targets.
- Real memory/build pressure during source build.
- File dialogs/portal behavior.
- HtmlReport external fallback behavior on Linux desktop.

### Required Pi Test Matrix

Minimum validation matrix:

- 800x480 explicit window:
  - `DRAGONGUI_PROFILE=pi`
  - `DRAGONGUI_WGPU_BACKEND=gl`
  - `DRAGONGUI_WINDOW_BACKEND=x11`
- 1280x720 explicit window with the same GL/X11 defaults.
- Fullscreen/default V3 demo run with the same GL/X11 defaults.
- One Vulkan comparison run:
  - `DRAGONGUI_WGPU_BACKEND=vulkan`
  - `DRAGONGUI_WINDOW_BACKEND=wayland` or `auto`.
- One diagnostic run:
  - `bash rpi_setup_and_run.sh diag`

### Validation Commands

Primary helper paths:

- `bash rpi_setup_and_run.sh build-smoke`
- `bash rpi_setup_and_run.sh run`
- `bash rpi_setup_and_run.sh diag`

Direct V3 command after setup:

- `DRAGONGUI_PROFILE=pi DRAGONGUI_WGPU_BACKEND=gl DRAGONGUI_WINDOW_BACKEND=x11 python examples/all_features_v3_demo.py`

Support commands to capture with failures:

- `uname -a`
- `python --version`
- `vulkaninfo --summary`
- `glxinfo -B`
- `vcgencmd measure_temp`
- `backend_info()`
- `app.debug_snapshot()`
- stderr from a run with `DRAGONGUI_LOG=debug`

### Hardware Acceptance Checklist

- Startup:
  - native backend imports from the expected source checkout / venv.
  - active profile is `pi`.
  - selected backend and window backend match the test case.
  - no black window or device creation failure.
- Layout:
  - left navigation scrolls independently at 800x480.
  - main panel content has a scroll path when taller than the screen.
  - badges/tags do not paint under scrollbars.
  - no primary panel is cut off without scroll.
- Fonts:
  - navigation, buttons, panel/frame titles, table numbers, histogram labels,
    and badges visibly use the same intended fallback/default unless explicitly
    styled.
- LinePlot:
  - fast stream does not recreate the deferred-limit crash.
  - queue latency does not grow without bound.
  - hover remains responsive enough for inspection.
- Histogram:
  - histogram panel opens without startup crash.
  - repeated updates or auto stats do not create runaway queue/rebuild pressure.
- DataFrameTable:
  - table scroll is usable at 800x480.
  - header/grid/text alignment remains correct.
- Scatter3D:
  - current fast path remains responsive and visually correct.
- Input:
  - mouse wheel / trackpad scroll works in navigation and table.
  - toolbar hit targets are usable on the Pi display.
  - touchscreen behavior is recorded if hardware is available.
- Setup:
  - `rpi_setup_and_run.sh` can be rerun without destructive cleanup.
  - `diag` produces enough information for support.

### Pi Validation Records to Keep

For each hardware validation pass, record:

- Pi model and RAM.
- Raspberry Pi OS version and kernel.
- Desktop session type.
- Monitor resolution and scale factor.
- Backend/window backend pair.
- Mesa/Vulkan/OpenGL renderer versions.
- Whether running from USB or SD.
- Build mode: source build or installed wheel.
- Debug snapshot performance counters.
- Screenshots for the visual smoke panels.
- Any env vars beyond the standard Pi defaults.

### Hardware Validation Blockers

- Do not declare the V3 demo Pi-ready until GL/X11 800x480 and fullscreen runs
  are validated on hardware.
- Do not make automated screenshot assertions mandatory until Pi font behavior
  is stable.
- Do not lock final performance budgets until Slice 1/Slice 2 counters have
  been collected on actual Pi hardware.
- Do not document Vulkan as the primary path unless hardware validation proves
  it is better than GL/X11.

## Documentation Readiness Audit

This audit maps each user-visible change to the docs or script text that must
be updated before the related milestone is considered complete.

### Documentation Targets

- `docs/raspberry-pi.md`
  - supported OS/kernel/Mesa baseline after hardware validation;
  - validated GL/X11 command path;
  - direct V3 demo commands, including explicit 800x480 run;
  - active Pi profile caps and degradations;
  - troubleshooting and support report instructions;
  - font package/fallback notes if needed;
  - HtmlReport external fallback policy.
- `docs/raspberry-pi-release-checklist.md`
  - final pre-release hardware matrix;
  - performance counters to record;
  - visual/input checklist;
  - pass/fail gates for LinePlot, Histogram, DataFrameTable, Scatter3D, and
    layout.
- `plans/RPi/raspberry-pi-5-port-progress-audit.md`
  - mark older pre-hardware assumptions as stale;
  - add actual Pi findings after validation;
  - record decisions made from hardware data.
- `rpi_setup_and_run.sh`
  - usage text for supported commands and env overrides;
  - default GL/X11 behavior;
  - diagnostics command expectations.
- `examples/all_features_v3_demo.py`
  - keep only user-visible status/debug text that helps validation;
  - avoid in-app explanatory docs unless needed for a supported fallback such as
    HtmlReport external open.
- `docs/widgets.md` / `docs/widgets-reference.md`
  - update only if public widget behavior or documented props change.

### Documentation by Milestone

- **Milestone A: Measurable Baseline**
  - Document new debug snapshot performance fields in `docs/raspberry-pi.md`.
  - Add the probe command and expected output shape.
  - Add baseline-recording fields to `docs/raspberry-pi-release-checklist.md`.
- **Milestone B: Correct Streaming Foundation**
  - Document queue latency/depth fields if exposed.
  - Update troubleshooting for deferred-limit/queue buildup symptoms.
  - Record LinePlot command-ordering fix in progress audit.
- **Milestone C: Compact UI Foundation**
  - Document validated 800x480 behavior and fullscreen notes.
  - Document Pi font fallback/package requirements if discovered.
  - Remove or update any docs that imply sidebar/badge workarounds are demo-only.
- **Milestone D: Widget Performance Pass**
  - Document LinePlot rendered segment/downsampling behavior if enabled.
  - Document Histogram rendered-bin/tick caps and aggregation semantics.
  - Document DataFrameTable compact metrics and partial-buffer behavior.
  - Ensure all profile caps appear in debug/profile output and docs.
- **Milestone E: Validation and Release Readiness**
  - Update release checklist with actual hardware pass/fail criteria.
  - Update Pi guide with the validated OS/kernel/Mesa/Python baseline.
  - Update progress audit with final hardware findings and remaining limitations.
  - Confirm `rpi_setup_and_run.sh --help` usage matches the docs.

### Documentation Compatibility Rules

- Do not document temporary rollback env vars as normal user controls unless we
  decide they are supported.
- If a profile default changes visual output, document it as a Pi-profile
  degradation, not as a general desktop behavior change.
- If behavior is only validated on GL/X11, call that the validated path and
  describe Vulkan/Wayland as diagnostic or experimental.
- Keep direct commands copy-pasteable from the repo root.
- Keep stale “pending hardware validation” text only where validation is still
  genuinely pending.

### Documentation Gates Before Implementation Is Complete

- Every new debug snapshot field has a short description or is clearly marked as
  diagnostic/internal.
- Every new Pi profile cap has a doc entry and appears in profile/debug output.
- Every user-visible fallback has a doc path:
  - HtmlReport external fallback;
  - compact font fallback/package;
  - unsupported embedded webview on Linux;
  - GL/X11 validated backend path.
- Every release/support report asks for enough data:
  - `rpi_setup_and_run.sh diag`;
  - `backend_info()`;
  - `app.debug_snapshot()`;
  - stderr with `DRAGONGUI_LOG=debug`;
  - screenshot when layout/font/rendering is involved.

## Code Ownership and Conflict Audit

Several slices touch the same large native modules. This audit defines the
intended ownership for each high-churn file so implementation patches stay
small and reviewable.

### High-Conflict Files

- `native/src/runtime.rs`
  - touched by measurement, command application, dirty rebuilds, LinePlot hover,
    table state, debug snapshot, window/backend handling, and HtmlReport policy.
- `native/src/commands.rs`
  - touched by command coalescing, new live Histogram commands, and debug
    snapshot bridge behavior.
- `native/src/primitives/mod.rs`
  - touched by primitive counts, LinePlot rendering, Histogram rendering,
    Badge/Tag drawing, table grid drawing, and PieChart drawing.
- `native/src/text/mod.rs`
  - touched by text counts, font consistency, Badge/Tag clipping, Histogram
    labels, table text, and PieChart labels.
- `native/src/layout.rs`
  - touched by Sidebar/Page scroll, Badge/Tag intrinsic sizing, scrollbar gutter,
    and 800x480 layout behavior.
- `native/src/runtime_profile.rs`
  - touched by every new Pi cap/profile helper.
- `python/dragongui/runtime.py`
  - touched by additive live update handle methods and debug snapshot wrappers.
- `python/dragongui/widgets.py`
  - touched by Histogram live behavior, profile-aware Python fallbacks, and any
    future public widget options.
- `examples/all_features_v3_demo.py`
  - touched by probes/status display, Pi workload constants, visual validation,
    and removal of workarounds.
- `docs/raspberry-pi.md`, `docs/raspberry-pi-release-checklist.md`,
  `plans/RPi/raspberry-pi-5-port-progress-audit.md`, and
  `rpi_setup_and_run.sh`
  - touched by setup, validation, support, and milestone docs.

### Ownership by File

- `native/src/runtime.rs`
  - Slice 1 owns passive debug/performance counters.
  - Slice 3 owns LinePlot command application changes.
  - Slice 4 owns queue/backpressure snapshot fields and demo-facing metrics.
  - Slice 5 owns direct rebuild call-site changes.
  - Slice 8 owns window/fullscreen sizing checks.
  - Slice 9 owns `nearest_line_plot_point` hover optimization.
  - Slice 10 owns native application of Histogram packed updates.
  - Slice 12 owns table debug/partial-buffer indicators.
  - Rule: do not mix these groups in one patch unless the second group is a
    direct test fix for the first.
- `native/src/commands.rs`
  - Slice 3 owns LinePlot queue-level coalescing barriers.
  - Slice 4 owns passive queue/coalesced/drained metrics.
  - Slice 10 owns new Histogram command variants/bindings/coalescing.
  - Rule: do not combine LinePlot coalescing changes with Histogram command
    additions.
- `native/src/primitives/mod.rs`
  - Slice 1 may only read/export existing `rect_count` behavior.
  - Slice 6 owns Badge/Tag primitive clipping/background correctness.
  - Slice 9 owns LinePlot primitive emission/downsampling/style simplification.
  - Slice 11 owns Histogram primitive bin budgeting.
  - Slice 12 owns DataFrameTable grid metrics alignment.
  - Slice 15 owns PieChart primitive compact behavior.
  - Rule: each widget renderer change should be its own patch.
- `native/src/text/mod.rs`
  - Slice 1 owns cheap text entry count.
  - Slice 6 owns Badge/Tag text clipping.
  - Slice 7 owns font resolution consistency.
  - Slice 11 owns Histogram label/tick caps.
  - Slice 12 owns DataFrameTable text metrics alignment.
  - Slice 15 owns PieChart label/legend caps.
  - Rule: do not combine font fallback work with text clipping or table metrics.
- `native/src/layout.rs`
  - Slice 6 owns Sidebar/Page scroll, scrollbar gutter, and Badge/Tag
    constrained intrinsic sizing.
  - Slice 8 owns viewport/fullscreen available-height behavior.
  - Rule: layout correctness for scroll containers should land before V3 demo
    workaround removal.
- `native/src/runtime_profile.rs`
  - Slice 9 owns LinePlot segment/downsampling/style helpers.
  - Slice 11 owns Histogram bin/tick cap helpers.
  - Slice 12 owns table compact metric helpers.
  - Slice 15 owns PieChart compact helpers if needed.
  - Rule: each profile helper must be tested for desktop `None`/disabled and Pi
    enabled values, and must appear in debug/profile output if user-visible.
- `python/dragongui/runtime.py`
  - Slice 10 owns Histogram live packed update handle methods.
  - Slice 1 owns debug snapshot wrapper changes only if Python shape changes.
  - Rule: new `enqueue_*` methods are additive; do not rename existing handles.
- `python/dragongui/widgets.py`
  - Slice 10 owns `Histogram.set_data()` live behavior.
  - Slice 12 may own a future `DataFrameTable` density option only if native
    defaults are proven first.
  - Rule: do not add public widget options for internal Pi defaults until
    profile behavior is validated.
- `examples/all_features_v3_demo.py`
  - Slice 4 owns optional low-frequency queue/status display.
  - Slice 6 owns removing badge/sidebar workarounds after library fixes.
  - Slice 8 owns Pi viewport/demo sizing constants.
  - Slice 13 owns visual smoke/checklist integration.
  - Rule: do not remove a workaround in the same patch that introduces the
    library fix unless the screenshot/manual check is included.

### Conflict Avoidance Rules

- Before editing a high-conflict file, identify the slice ownership in the patch
  description.
- Avoid opportunistic cleanup in high-conflict files.
- Keep formatting-only churn out of behavioral patches.
- In `runtime.rs`, prefer small helper functions over large match rewrites when
  adding counters or dirty classification.
- In `primitives/mod.rs` and `text/mod.rs`, isolate widget-specific helpers so
  LinePlot, Histogram, Table, Badge/Tag, and PieChart changes do not interleave.
- In `examples/all_features_v3_demo.py`, separate validation/status additions
  from UI layout/workaround removals.
- Docs can be updated with the slice they describe, but broad docs refresh
  belongs to Slice 14.

### Patch Review Checklist

- Does the patch touch more than one high-conflict file? If yes, is that
  required by the slice?
- Does the patch combine two slices that the plan says to keep separate?
- Are unrelated demo/docs changes mixed into a native behavior patch?
- Are profile helper changes accompanied by tests and debug/profile visibility?
- If `runtime.rs` changed, is the change instrumentation, command application,
  dirty rebuild, viewport, or widget state? It should not be several at once.

## Implementation Readiness Audit

This is the final pre-coding gate for Milestone A. It defines exactly what the
first implementation pass should do and what evidence is needed before moving
to LinePlot command correctness.

### Implementation Status

- Slice 1, Pi Measurement Snapshot: completed.
  - Started: 2026-05-13.
  - Completed: 2026-05-13.
  - Implemented passive `runtime.performance` and `gpu.performance` counters for
    last applied dirty flag, primitive/text rebuild timing, primitive/text
    counts, command queue depth, and last drained command count.
  - Verification: `rustfmt --edition 2021 native/src/runtime.rs
    native/src/text/mod.rs` passed.
  - Verification: `cargo check --manifest-path native/Cargo.toml` passed after
    using the explicit rustup toolchain path; only pre-existing dead-code/read
    warnings were reported.
  - Verification: `bash rpi_setup_and_run.sh build-smoke
    benchmarking/rpi_widget_probe.py` rebuilt/copied the native extension and
    confirmed `native_performance_counters: True` through `app.debug_snapshot()`.
- Slice 2, Repeatable Pi Probe: completed.
  - Started: 2026-05-13.
  - Completed: 2026-05-13.
  - Added standalone `benchmarking/rpi_widget_probe.py` with 800x480/1280x720
    command-line sizing, smoke-frame default exit, JSON output, and compact
    LinePlot/Histogram/DataFrameTable/sidebar-scroll coverage.
  - Added Pi docs commands for the probe.
  - Verification: `python -m py_compile benchmarking/rpi_widget_probe.py`
    passed.
  - Verification: `python benchmarking/rpi_widget_probe.py --help` passed.
  - Verification: short GUI smoke run passed with `--width 640 --height 400
    --rows 128 --append-points 16 --frames 3 --json`.
  - Verification: `bash rpi_setup_and_run.sh build-smoke
    benchmarking/rpi_widget_probe.py` passed after rebuilding/copying the native
    extension. The probe reported `native_performance_counters: True`,
    primitive count `4295`, text count `33`, and populated primitive/text
    rebuild timings in the 800x480 smoke run.
- Phase 1, LinePlot Correctness and Rebuild Control: completed.
  - Started: 2026-05-13.
  - Completed: 2026-05-13.
  - Changed queue-level LinePlot append coalescing to the conservative safe path:
    only immediately adjacent compatible appends merge. Appends no longer merge
    across `SetProp`, `SetStyle`, `DebugSnapshot`, or any other intervening
    command.
  - Routed `ClearLinePlotSeries` through `rebuild_for_dirty(Dirty::Visual)`.
  - Routed LinePlot toolbar actions through dirty classification:
    `Fit`/`Pan`/`Zoom`/`Box`/`Grid` are `Dirty::Visual`; `Axes` is
    `Dirty::Text`.
  - Verification: `rustfmt --edition 2021 native/src/runtime.rs
    native/src/commands.rs` passed.
  - Verification: `cargo test --manifest-path native/Cargo.toml line_plot`
    passed: 6 passed, 0 failed.
  - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
    only pre-existing dead-code/read warnings.
  - Verification: rebuilt/copied the native extension with
    `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh build-smoke
    benchmarking/rpi_widget_probe.py`; final snapshot reported
    `native_performance_counters: True`.
  - Verification: `python benchmarking/rpi_widget_probe.py --width 800 --height
    480 --rows 2048 --append-points 256 --frames 10 --json` passed with both
    live and final snapshots reporting `native_performance_counters: true`.
- Phase 2, LinePlot Rendering Fast Path: completed.
  - Started: 2026-05-13.
  - Completed: 2026-05-14.
  - Added profile-aware retained LinePlot primitive rendering controls:
    desktop keeps the existing 4096 segment budget, while Pi uses a 1536 segment
    budget and simplifies dense dashed/dotted styles to solid during primitive
    emission.
  - Replaced uniform stride downsampling with screen-x bucket simplification for
    over-budget series. Each bucket preserves first, min-y, max-y, and last
    representatives to keep spikes visible.
  - Exposed `line_plot_segment_budget` and `line_plot_simplify_styles` in the
    runtime platform snapshot.
  - Updated Raspberry Pi docs with the new LinePlot profile caps.
  - Verification: `rustfmt --edition 2021 native/src/primitives/mod.rs
    native/src/runtime.rs native/src/runtime_profile.rs` passed.
  - Verification: `cargo test --manifest-path native/Cargo.toml line_plot`
    passed: 9 passed, 0 failed.
  - Verification: `cargo test --manifest-path native/Cargo.toml env_can_force`
    passed: 2 passed, 0 failed.
  - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
    only pre-existing dead-code/read warnings.
  - Verification: rebuilt/copied the native extension with
    `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh build-smoke
    benchmarking/rpi_widget_probe.py`; final snapshot reported
    `native_performance_counters: True` and primitive count about `1030`.
  - Verification: `python benchmarking/rpi_widget_probe.py --width 800 --height
    480 --rows 2048 --append-points 256 --frames 10 --json` passed with live
    and final snapshots reporting `native_performance_counters: true`,
    `queue_depth: 0`, primitive count `1030`, and final primitive rebuild time
    about `0.90ms`.
- Phase 3, Text and Primitive Rebuild Separation: in progress.
  - Started: 2026-05-14.
  - First implementation slice completed: converted hot LinePlot/Histogram
    interaction rebuilds to the existing dirty batching path without adding new
    dirty enum variants.
  - Histogram toolbar actions now classify dirty work like LinePlot toolbar
    actions: `Fit`/`Pan`/`Zoom`/`Box`/`Grid` are `Dirty::Visual`; `Axes` is
    `Dirty::Text`.
  - LinePlot and Histogram bounds changes now use `Dirty::Text` so tick/axis
    text and primitive visuals rebuild through the deferred dirty path.
  - LinePlot and Histogram selection-rectangle updates now use `Dirty::Visual`
    so primitive-only interaction overlays avoid text rebuilds.
  - LinePlot hover changes now use `Dirty::Text` through the deferred dirty path.
  - Verification: `rustfmt --edition 2021 native/src/runtime.rs` passed.
  - Verification: `cargo test --manifest-path native/Cargo.toml
    toolbar_actions_classify_dirty_work` passed: 2 passed, 0 failed.
  - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
    only pre-existing dead-code/read warnings.
  - Verification: rebuilt/copied the native extension with
    `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh build-smoke
    benchmarking/rpi_widget_probe.py`; final snapshot reported counters enabled,
    queue depth `0`, and primitive count `1030`.
  - Verification: `python benchmarking/rpi_widget_probe.py --width 800 --height
    480 --rows 2048 --append-points 256 --frames 10 --json` passed with live
    and final snapshots reporting `native_performance_counters: true`,
    `queue_depth: 0`, and primitive count `1030`.
  - Remaining Phase 3 work: finish the direct rebuild call-site audit before
    converting non-plot interactions, because popup/menu/toast/table/text-area
    paths have different stale-text and stale-primitive risks.
  - Direct rebuild call-site audit completed: 2026-05-14.
    - `WgpuState::rebuild_visuals()` itself intentionally calls
      `rebuild_text()` then `rebuild_primitives()`. Keep this as the current
      meaning of `Dirty::Text` until a separate `TextOnly`/`PrimitiveOnly` dirty
      split is designed.
    - `rebuild_for_dirty(Dirty::Visual)` still directly calls
      `rebuild_primitives()`. Keep this; it is the batching boundary, not a
      bypass.
    - Popup/menu/dropdown paths (`close_popups`, `open_context_menu_at`,
      dropdown escape/selection, menu activation) rebuild both text and
      primitives because overlay text and overlay rectangles move together.
      Candidate follow-up: convert these to `rebuild_for_dirty(Dirty::Text)`,
      not primitive-only.
    - Text editing/caret paths (`focus_widget`, `scroll_text_area`,
      `handle_text_input_key`, `handle_number_input_key`, IME commit, number
      validation) rebuild both because caret geometry, text buffers, and input
      primitives are coupled. Candidate follow-up: keep direct until a
      `TextOnly` dirty state or a text-area-specific caret invalidation exists.
    - Table paths (`select_table_cell`, `move_table_selection`,
      `move_table_selection_to_col_edge`, `toggle_table_sort`, `scroll_table`)
      currently rebuild text and primitives. Selection changes may be
      primitive-only, but table scroll/sort changes visible cell text. Candidate
      follow-up: split table selection from table viewport/data changes.
    - Slider paths (`update_slider_drag`, keyboard slider adjustment) call
      `rebuild_primitives()` and appear primitive-only. Candidate follow-up:
      route through `rebuild_for_dirty(Dirty::Visual)` so deferred command
      batches can coalesce them.
    - Toast paths (`ShowToast`, `DismissToast`, expiration in
      `about_to_wait`) rebuild visuals because toast text and primitives are
      coupled. Candidate follow-up: convert command-path show/dismiss to
      `Dirty::Text`; keep animation expiration direct until animation dirty
      semantics are reviewed.
    - Style transition / CSS animation tick paths rebuild visuals every frame
      when animated values change. Keep direct for now; this is frame-timed
      animation work and should be handled by a dedicated animation dirty
      policy.
    - Scroll-container and panel-scrollbar paths call `apply_layout()`, not
      `rebuild_visuals()`, because visible rects, clipping, and scrollbar
      geometry change. Keep as layout work.
    - Scatter interaction paths mostly update scatter-owned GPU overlays or
      camera buffers directly, not retained primitives. Keep out of Phase 3
      unless a separate scatter overlay audit identifies stale retained UI.
  - Recommended next Phase 3 implementation slices:
    - Slice 3A: route slider direct primitive rebuilds through
      `rebuild_for_dirty(Dirty::Visual)`.
    - Slice 3B: route popup/menu/dropdown open/close command paths through
      `rebuild_for_dirty(Dirty::Text)` and add smoke coverage for dropdown/menu
      text.
    - Slice 3C: split table selection rebuilds from table scroll/sort rebuilds;
      only selection is a candidate for `Dirty::Visual`.
    - Slice 3D: design `Dirty::TextOnly`/`Dirty::PrimitiveOnly` only after the
      above slices and probe data show remaining rebuild pressure.
  - Slice 3A, Slider Dirty Path: completed.
    - Started: 2026-05-14.
    - Completed: 2026-05-14.
    - Slider mouse drag and keyboard arrow adjustment now route primitive-only
      updates through `rebuild_for_dirty(Dirty::Visual)` instead of direct
      `rebuild_primitives()` calls.
    - Verification: `rustfmt --edition 2021 native/src/runtime.rs` passed.
    - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
      only pre-existing dead-code/read warnings.
    - Verification: rebuilt/copied the native extension with
      `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh build-smoke
      benchmarking/rpi_widget_probe.py`; final snapshot reported counters
      enabled, queue depth `0`, and primitive count `1030`.
    - Verification: `python benchmarking/rpi_widget_probe.py --width 800
      --height 480 --rows 2048 --append-points 256 --frames 10 --json` passed
      with live and final snapshots reporting `native_performance_counters:
      true`, `queue_depth: 0`, and primitive count `1030`.
  - Slice 3B, Popup/Menu/Dropdown Dirty Path: completed.
    - Started: 2026-05-14.
    - Completed: 2026-05-14.
    - Popup close/open, context menu open, dropdown activation, dropdown option
      selection, and dropdown keyboard navigation/escape paths now route through
      `rebuild_for_dirty(Dirty::Text)` instead of direct visual rebuild calls.
    - Verification: `rustfmt --edition 2021 native/src/runtime.rs` passed.
    - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
      only pre-existing dead-code/read warnings.
    - Verification: rebuilt/copied the native extension with
      `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh build-smoke
      benchmarking/rpi_widget_probe.py`; final snapshot reported counters
      enabled, queue depth `0`, primitive count `1036`, and primitive rebuild
      time `1.243743 ms`.
    - Verification: `python benchmarking/rpi_widget_probe.py --width 800
      --height 480 --rows 2048 --append-points 256 --frames 10 --json` passed
      with live and final snapshots reporting `native_performance_counters:
      true`, `queue_depth: 0`, live primitive count `1030`, and final primitive
      count `1036`.
  - Slice 3C, Table Selection Dirty Path: completed.
    - Started: 2026-05-14.
    - Completed: 2026-05-14.
    - Added explicit table dirty classifiers: selection-only paths route through
      `rebuild_for_dirty(Dirty::Visual)`, while table scroll/sort paths route
      through `rebuild_for_dirty(Dirty::Text)` because visible cell text can
      change.
    - Verification: `rustfmt --edition 2021 native/src/runtime.rs` passed.
    - Verification: `cargo test --manifest-path native/Cargo.toml
      table_interactions_classify_dirty_work` passed: 1 passed, 0 failed.
    - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
      only pre-existing dead-code/read warnings.
    - Verification: rebuilt/copied the native extension with
      `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh build-smoke
      benchmarking/rpi_widget_probe.py`; live and final snapshots reported
      counters enabled, queue depth `0`, and primitive count `1030`.
    - Verification: `python benchmarking/rpi_widget_probe.py --width 800
      --height 480 --rows 2048 --append-points 256 --frames 10 --json` passed
      with live and final snapshots reporting `native_performance_counters:
      true`, `queue_depth: 0`, and primitive count `1030`.
  - Slice 3D, Dirty State Split Decision: completed.
    - Started: 2026-05-14.
    - Completed: 2026-05-14.
    - Decision: do not add `Dirty::TextOnly` or a new primitive-only dirty enum
      in this pass. Existing `Dirty::Visual` already represents primitive-only
      work, and the remaining text-heavy call sites also affect caret geometry,
      overlays, focus/pressed styles, toast/style animation primitives, or
      layout-coupled text.
    - The latest Pi probes after Slices 3A-3C report `queue_depth: 0`,
      primitive count `1030`, and sub-millisecond to low-millisecond primitive
      rebuild timing, so a dirty enum split is not the current bottleneck.
    - Follow-up: revisit `Dirty::TextOnly` only after Slice 4 queue visibility
      and later visual smoke checks show a repeated hot text-only path with no
      primitive/caret/layout dependency.
- Slice 4, Queue Backpressure Visibility: completed.
  - Started: 2026-05-14.
  - Completed: 2026-05-14.
  - Added passive queue visibility fields without changing command dropping or
    application behavior.
  - Runtime snapshots now expose raw drained command count, applied drained
    command count, last drain coalesced count, total queue-side coalesced count,
    queue oldest age, and max observed queue depth.
  - `benchmarking/rpi_widget_probe.py` now includes those queue fields in its
    compact summary output.
  - Added command queue tests for queue depth/max-depth stats and queue-side
    LinePlot append coalesced counts.
  - Verification: `rustfmt --edition 2021 native/src/runtime.rs
    native/src/commands.rs` passed.
  - Verification: `python -m py_compile benchmarking/rpi_widget_probe.py`
    passed.
  - Verification: `cargo test --manifest-path native/Cargo.toml queue_stats`
    passed: 2 passed, 0 failed.
  - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
    only pre-existing dead-code/read warnings.
  - Verification: rebuilt/copied the native extension with
    `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh build-smoke
    benchmarking/rpi_widget_probe.py`; final snapshot reported queue depth `0`,
    queue oldest age `0.0 ms`, max observed queue depth `6`, and all coalesced
    count fields present.
  - Verification: `python benchmarking/rpi_widget_probe.py --width 800
    --height 480 --rows 2048 --append-points 256 --frames 10 --json` passed
    with live and final snapshots reporting `queue_depth: 0`,
    `queue_max_depth_observed: 6`, `last_coalesced_command_count: 0`, and
    `total_queue_coalesced_command_count: 0`.
  - Deferred: command dropping and V3 queue/status UI are not added yet because
    the current Pi probe still shows no queue buildup. Revisit if a
    high-frequency workload pushes queue depth or oldest age above the eventual
    Slice 4 thresholds.
- Slice 5, Rebuild Call-Site Cleanup: completed.
  - Started: 2026-05-14.
  - Completed: 2026-05-14.
  - Completed through the Phase 3 implementation slices and final audit.
  - Hot LinePlot, Histogram, slider, popup/menu/dropdown, and table selection
    paths now route through `rebuild_for_dirty` where batching is safe.
  - Remaining direct `rebuild_visuals()` calls are intentionally retained for
    coupled text/caret, focus/pressed style, toast/style animation, hover
    tooltip, and layout-related paths.
  - No new dirty enum variants were added; Slice 3D recorded the decision to
    defer `Dirty::TextOnly` until measurements show a repeated text-only
    bottleneck.
  - Verification: covered by Phase 3 targeted dirty tests, `cargo check`,
    build-smoke probe, and longer 800x480 probe.
- Slice 6, Sidebar/Page/Badge Layout Correctness: completed.
  - Started: 2026-05-14.
  - Completed: 2026-05-14.
  - Completed first patch: Badge/Tag intrinsic sizing now respects
    `max_width`, and FlowLayout intrinsic wrapping now uses child max-width
    caps. This keeps long sidebar badges, such as `HtmlReport`, inside the
    scrollable content gutter instead of underneath the scrollbar.
  - Added regression coverage for standalone Badge max-width and Sidebar
    FlowLayout badge clearance from the vertical scrollbar gutter.
  - Verification: `rustfmt --edition 2021 native/src/layout.rs` passed.
  - Verification: `cargo test --manifest-path native/Cargo.toml badge` passed:
    9 passed, 0 failed.
  - Verification: `cargo test --manifest-path native/Cargo.toml
    sidebar_flow_badges_respect_scrollbar_gutter_and_max_width` passed: 1
    passed, 0 failed.
  - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
    only pre-existing dead-code/read warnings.
  - Verification: rebuilt/copied the native extension with
    `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh build-smoke
    benchmarking/rpi_widget_probe.py`; live and final snapshots reported
    counters enabled, queue depth `0`, max observed queue depth `6`, and
    primitive count `1030`.
  - Verification: `python benchmarking/rpi_widget_probe.py --width 800
    --height 480 --rows 2048 --append-points 256 --frames 10 --json` passed
    with live and final snapshots reporting `native_performance_counters: true`,
    `queue_depth: 0`, max observed queue depth `6`, and primitive count `1030`.
  - Added Sidebar scroll ownership coverage: Sidebar now has an explicit
    regression test proving it owns its vertical scroll range independently from
    main content.
  - Added Sidebar primitive scrollbar coverage proving Sidebar emits the same
    vertical scrollbar indicator path as other scroll containers.
  - Verification: `cargo test --manifest-path native/Cargo.toml
    sidebar_scrolls_independently_from_main_content` passed: 1 passed, 0 failed.
  - Verification: `cargo test --manifest-path native/Cargo.toml
    sidebar_scroll_container_emits_vertical_scrollbar_indicator` passed: 1
    passed, 0 failed.
  - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
    only pre-existing dead-code/read warnings.
  - Verification: `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh smoke
    examples/all_features_v3_demo.py` passed at the Pi default 800x480 path.
    Snapshot reported Pi profile, GL backend, frame count `3`, queue depth `0`,
    and native performance counters.
  - Added `benchmarking/rpi_v3_sidebar_geometry_check.py` as a repeatable V3
    sidebar geometry check. It runs the real V3 demo at the Pi 800x480 path and
    verifies the sidebar badge rects stay clear of the scrollbar gutter and the
    sidebar owns a vertical scroll range.
  - Verification: `python -m py_compile
    benchmarking/rpi_v3_sidebar_geometry_check.py` passed.
  - Verification: `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh smoke
    benchmarking/rpi_v3_sidebar_geometry_check.py` passed. The V3 sidebar
    reported `HtmlReport` ending at x=`156.0`, gutter-safe right x=`164.5`,
    and sidebar vertical scroll range `336.0`.
  - Demo cleanup decision: no V3 sidebar constants were removed in this slice.
    `SIDEBAR_BADGES_STYLE` remains a normal demo layout choice, not a
    scrollbar-overlap workaround; the library fix and geometry check now own the
    bug regression.
- Slice 7, Font Consistency: completed.
  - Started: 2026-05-14.
  - Completed: 2026-05-14.
  - Completed first patch: plot overlay labels now pass the owning
    LinePlot/Histogram/PieChart widget's resolved font family into the overlay
    text path instead of falling back to generic sans-serif. This covers chart
    ticks, legends, readouts, and toolbar/readout overlay labels that were
    visibly different from normal panel/navigation text on Pi.
  - Completed second patch: rotated plot axis-label custom glyphs now resolve
    the owning widget's named font family through the glyphon font database
    before falling back to the previous Segoe/Arial/DejaVu/Liberation list. The
    glyph cache key now includes the resolved font family so cached axis labels
    cannot be reused across different families.
  - Completed third patch: true Scatter3D projected labels now pass the owning
    Scatter3D widget's resolved font family into the shared overlay label path.
  - Explicit terminal-theme monospace CSS and embedded HtmlReport/Plotly HTML
    font declarations remain intentional and are outside the normal DragonGUI
    panel/widget typography path.
  - Verification: `rustfmt --edition 2021 native/src/runtime.rs
    native/src/text/mod.rs` passed.
  - Verification: `cargo test --manifest-path native/Cargo.toml
    font_family` passed: 3 passed, 0 failed.
  - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
    only pre-existing dead-code/read warnings.
  - Verification before the final Scatter3D label wiring:
    `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh build-smoke
    benchmarking/rpi_widget_probe.py` rebuilt and installed the native wheel.
    The final probe snapshot reported Pi profile, GL backend, 800x480, queue
    depth `0`, primitive count `1036`, text count `33`, and native performance
    counters. The live mid-run snapshot still timed out, matching the prior
    known probe behavior.
  - Verification before the final Scatter3D label wiring:
    `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh smoke
    examples/all_features_v3_demo.py` passed. Snapshot reported status `ok`,
    Pi profile, GL backend, 800x480, queue depth `0`, text entry count `53`,
    and computed styles for normal widgets/buttons using `Piboto`.
  - Verification after the final Scatter3D label wiring:
    `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh build-smoke
    benchmarking/rpi_widget_probe.py` rebuilt and installed the native wheel.
    The live and final probe snapshots reported Pi profile, GL backend, 800x480,
    native performance counters, queue depth `0`, max observed queue depth `6`,
    primitive count `1030`, and text count `33`.
- Slice 8, 800x480 Viewport and Fullscreen Fit: completed.
  - Started: 2026-05-14.
  - Completed: 2026-05-14.
  - Added `DRAGONGUI_DEMO_PAGE` support to the V3 demo so individual pages can
    be launched directly for repeatable Pi viewport checks without manual
    navigation.
  - Added `benchmarking/rpi_v3_viewport_fit_check.py`. It runs the real V3 demo
    at the Pi 800x480 path and verifies the active page clip stays inside the
    window, active-page direct content has a scroll path when needed, and active
    page descendants do not extend outside the page clip unless they are inside
    an actual scroll container.
  - The viewport check now derives its expected logical window size from the V3
    demo module after `DRAGONGUI_DEMO_WIDTH` / `DRAGONGUI_DEMO_HEIGHT` are
    applied, so the same check can be reused for 800x480 and 1280x720.
  - First Slice 8 bug found: the Histogram page forced two histogram-grid
    columns at 800x480, causing the right column to extend past the page clip
    with no horizontal scroll path.
  - First Slice 8 fix: the V3 Histogram page now uses one histogram-grid column
    under the Pi profile while keeping the two-column desktop layout unchanged.
  - Second Slice 8 bug found: `Page` scroll ranges only considered direct
    children. A compact page containing a `GridLayout` could have tall panel
    descendants extend below the page while the page still reported no scroll
    range because the direct grid child fit inside the page.
  - Second Slice 8 fix: scroll content bounds now include non-fixed descendant
    extents, while stopping at nested scroll containers so inner `ScrollArea`
    widgets continue to own their own overflow.
  - Added layout regression tests for page scroll ranges that come from
    overflowing grid descendants and for stopping parent scroll accounting at a
    nested scroll container.
  - Fullscreen audit finding: the native runtime currently creates windows from
    explicit logical `AppSpec` width/height only; no public fullscreen flag is
    wired through Python/AppSpec/winit yet. Current Pi fit validation therefore
    covers the explicit 800x480 logical window path and
    `DRAGONGUI_DEMO_WIDTH`/`DRAGONGUI_DEMO_HEIGHT`, not a true monitor
    fullscreen mode.
  - Verification: `python -m py_compile examples/all_features_v3_demo.py
    benchmarking/rpi_v3_viewport_fit_check.py` passed.
  - Verification: `rustfmt --edition 2021 native/src/layout.rs` passed.
  - Verification: `cargo test --manifest-path native/Cargo.toml scroll_range`
    passed: 9 passed, 0 failed.
  - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
    only pre-existing dead-code/read warnings.
  - Verification after native layout fix:
    `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh build-smoke
    benchmarking/rpi_widget_probe.py` rebuilt and installed the native wheel;
    final snapshot reported Pi profile, GL backend, 800x480, native counters,
    queue depth `0`, primitive count `1036`, and text count `33`.
  - Verification: `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh smoke
    benchmarking/rpi_v3_viewport_fit_check.py` passed for the default Overview
    page at 800x480.
  - Verification: `DRAGONGUI_DEMO_PAGE=lineplots DRAGONGUI_SMOKE_FRAMES=3 bash
    rpi_setup_and_run.sh smoke benchmarking/rpi_v3_viewport_fit_check.py`
    passed at 800x480.
  - Verification: `DRAGONGUI_DEMO_PAGE=histograms DRAGONGUI_SMOKE_FRAMES=3 bash
    rpi_setup_and_run.sh smoke benchmarking/rpi_v3_viewport_fit_check.py`
    failed before the one-column Pi histogram-grid fix and passed after it.
  - Verification: `DRAGONGUI_DEMO_PAGE=scatter DRAGONGUI_SMOKE_FRAMES=3 bash
    rpi_setup_and_run.sh smoke benchmarking/rpi_v3_viewport_fit_check.py`
    passed at 800x480.
  - Verification: remaining 800x480 V3 page sweep passed after the descendant
    scroll-range fix: `piecharts`, `controls`, `data`, `runtime`, `debug`,
    `styling`, and `layout`. The Controls page failed before the native fix and
    passed after it with active page `scroll_max_y: 328.0`.
  - Verification: 1280x720 checks passed for Overview, Histograms, and Controls.
    On the current Pi/window-manager setup, a requested `1280x720` logical
    window produced an actual inner size of `1280x658`; the check now treats
    smaller window-manager-constrained inner sizes as valid and verifies layout
    against the actual inner size.
  - Verification: `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh smoke
    examples/all_features_v3_demo.py` passed after the demo initial-page and
    viewport/layout changes.
  - Decision: do not add a public fullscreen/logical-monitor API in this slice.
    The Pi clipping/scroll bugs are fixed through layout behavior and verified
    against explicit logical window sizes. A true fullscreen mode should be a
    separate API slice because it crosses Python app options, `AppSpec`, winit
    window creation, and documentation.
  - Optional follow-up: broaden the 1280x720 viewport sweep to every V3 page.
- Slice 10, Histogram Live Packed Updates: completed.
  - Started: 2026-05-14.
  - Completed: 2026-05-14.
  - Added additive Python handle/runtime APIs for
    `enqueue_set_histogram_bins_packed`.
  - Updated live `Histogram.set_data()` to recompute bins and enqueue packed
    float32 edges/counts instead of raising. Non-live serialization still uses
    the existing `edges`/`counts` props.
  - Added native `SetHistogramBinsPacked`, queue-level coalescing,
    runtime-batch coalescing, byte payload validation, startup-rule-compatible
    edge/count validation, and `Dirty::Text` rebuild on successful application.
  - Added focused Python API coverage for live histogram updates and Rust tests
    for histogram command coalescing plus bin validation.
  - Verification: `python -m py_compile python/dragongui/widgets.py
    python/dragongui/runtime.py tests/test_python_api.py` passed.
  - Verification: `cargo test --manifest-path native/Cargo.toml histogram
    --lib` passed: 5 passed, 0 failed.
  - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
    only pre-existing dead-code/read warnings.
  - Verification: direct `.venv` Python smoke passed for live
    `Histogram.set_data()` and for the compiled `_NativeCommandSender`
    histogram command/invalid-payload validation.
  - Verification: `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh
    build-smoke benchmarking/rpi_widget_probe.py` rebuilt and installed the
    native wheel and exited successfully. The final probe snapshot reported Pi
    profile, GL backend, 800x480, native counters, `last_dirty: text`,
    primitive count `1030`, text count `33`, and queue depth `0`. The probe's
    optional `live_after_append` snapshot still timed out as it did before, but
    the final smoke completed successfully.
  - Test environment note: `pytest` is not installed in either `/usr/bin/python`
    or `.venv`, so the new pytest case was not run through pytest in this
    environment; equivalent direct `.venv` smoke coverage was run.
  - Scope remains command plumbing only. Histogram render budgeting and bin
    aggregation stay deferred to Slice 11.
- Slice 11, Histogram Render Budget: completed.
  - Started: 2026-05-14.
  - Completed: 2026-05-14.
  - Added profile-gated histogram render helpers in
    `native/src/runtime_profile.rs` and `native/src/primitives/mod.rs`.
  - Pi profile now reports `histogram_bin_budget: Some(384)` and
    `histogram_compact_tick_count: Some(4)`; desktop reports `None` for both.
  - Primitive rendering now caps dense histogram bars by screen bucket under the
    Pi profile and leaves desktop rendering unchanged.
  - Histogram tick generation and text labels use the compact Pi tick cap only
    for compact histogram rects. Larger Pi histogram rects keep the requested
    tick count.
  - Render semantics for merged bins: merged bar height uses the maximum source
    bin height in that screen bucket. This is an envelope/downsampling rule, not
    statistical re-binning; it preserves visible peaks, keeps the original
    y-axis bounds meaningful, works for density/probability/percent/cumulative
    data without rewriting values, and leaves stored histogram props unchanged.
  - Added Rust tests for profile helper defaults, render-bin budgeting without
    data mutation, compact tick gating, and emitted bar primitive counts.
  - Verification: `rustfmt --edition 2021 native/src/runtime_profile.rs
    native/src/primitives/mod.rs native/src/runtime.rs` passed.
  - Verification: `cargo test --manifest-path native/Cargo.toml histogram
    --lib` passed: 8 passed, 0 failed.
  - Verification: `cargo test --manifest-path native/Cargo.toml
    runtime_profile --lib` passed: 5 passed, 0 failed.
  - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
    only pre-existing dead-code/read warnings.
  - Verification: `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh
    build-smoke benchmarking/rpi_widget_probe.py` rebuilt and installed the
    native wheel and exited successfully. Final probe snapshot reported Pi
    profile, GL backend, 800x480, primitive count `1030`, text count `33`, and
    queue depth `0`; the demo histogram surface is not dense enough for the new
    bin cap to change this top-level count.
  - Verification: `DRAGONGUI_DEMO_PAGE=histograms DRAGONGUI_SMOKE_FRAMES=3 bash
    rpi_setup_and_run.sh smoke benchmarking/rpi_v3_viewport_fit_check.py`
    passed at 800x480.
- Slice 12, DataFrameTable Compact Metrics: completed.
  - Started: 2026-05-14.
  - Completed: 2026-05-14.
  - Added one shared profile-aware table metrics helper in `native/src/table.rs`.
  - Pi profile now enables compact table metrics: header `26`, row `22`, index
    `48`, and column `112` logical pixels before scale factor. Desktop uses the
    previous metrics.
  - Primitive grid rendering, text placement, table hit testing, and visible
    row/column calculations now use the same metrics profile path.
  - Explicit table CSS/style values remain authoritative over Pi compact
    defaults.
  - Added `table_compact_metrics` to runtime profile/debug output.
  - Added Rust tests for compact profile defaults, CSS override precedence,
    table text clipping/alignment, and table primitive behavior.
  - Verification: `rustfmt --edition 2021 native/src/table.rs
    native/src/runtime_profile.rs native/src/primitives/mod.rs
    native/src/text/mod.rs native/src/runtime.rs` passed.
  - Verification: `cargo test --manifest-path native/Cargo.toml table --lib`
    passed: 23 passed, 0 failed.
  - Verification: `cargo test --manifest-path native/Cargo.toml
    runtime_profile --lib` passed: 5 passed, 0 failed.
  - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
    only pre-existing dead-code/read warnings.
  - Verification: `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh
    build-smoke benchmarking/rpi_widget_probe.py` rebuilt and installed the
    native wheel and exited successfully. Final probe snapshot reported Pi
    profile, GL backend, 800x480, primitive count `1030`, text count `33`, and
    queue depth `0`.
  - Verification: `DRAGONGUI_DEMO_PAGE=data DRAGONGUI_SMOKE_FRAMES=3 bash
    rpi_setup_and_run.sh smoke benchmarking/rpi_v3_viewport_fit_check.py`
    passed at 800x480.
  - No public Python `density` option is planned in this slice.
- Slice 13, Visual Smoke and Input Checklist: completed.
  - Started: 2026-05-14.
  - Completed: 2026-05-14.
  - Added `benchmarking/rpi_visual_smoke.md` with the Pi baseline record,
    automated 800x480 and representative 1280x720 V3 route sweeps, manual
    screenshot list, widget input checklist, pass criteria, and failure notes.
  - Linked the detailed visual smoke checklist from
    `docs/raspberry-pi-release-checklist.md`.
  - Checklist remains manual by design; automation should wait until screenshots
    are stable enough to avoid brittle false failures.
- Slice 14, Pi Setup Script and Docs: completed.
  - Started: 2026-05-14.
  - Completed: 2026-05-14.
  - Added a non-destructive `check-deps` command to `rpi_setup_and_run.sh` that
    reports missing build/runtime commands and pkg-config libraries, and included
    it in `diag`.
  - Updated `docs/raspberry-pi.md` with the validated direct V3 page commands,
    page-fit smoke command, current histogram/table Pi profile caps, and the
    supported `rpi_setup_and_run.sh` path.
  - Updated `docs/raspberry-pi-release-checklist.md` to call out `check-deps`,
    the build-smoke probe, and the detailed visual/input smoke checklist.
  - Verification: `bash -n rpi_setup_and_run.sh` passed.
  - Verification: `bash rpi_setup_and_run.sh check-deps` ran and correctly
    reported missing local packages/tools (`cmake`, `glxinfo`,
    `wayland-client`, `xkbcommon`) with the remediation command
    `bash rpi_setup_and_run.sh deps`. This is expected in the current shell and
    confirms the diagnostic path works.
  - Runtime verification reuses the successful Slice 12
    `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh build-smoke
    benchmarking/rpi_widget_probe.py` run after the latest native changes.
- Slice 15, PieChart and HtmlReport Policy: completed.
  - Started: 2026-05-14.
  - Completed: 2026-05-14.
  - Added a Pi profile switch for compact PieChart labels and exposed it in the
    runtime platform/debug profile output.
  - Small Pi PieCharts now raise the automatic slice-label threshold and cap
    visible slice/legend labels so compact panels do not become text-heavy.
  - Desktop PieChart behavior remains unchanged.
  - Embedded HtmlReport unsupported snapshots now explicitly report that an
    external fallback is available on unsupported targets.
  - Updated Raspberry Pi docs with the HtmlReport external-fallback policy and
    compact PieChart profile behavior.
  - Verification: `rustfmt --edition 2021 native/src/runtime_profile.rs
    native/src/primitives/mod.rs native/src/runtime.rs
    native/src/html_report_webview.rs` passed.
  - Verification: `cargo test --manifest-path native/Cargo.toml pie_chart
    --lib` passed: 3 passed, 0 failed.
  - Verification: `cargo test --manifest-path native/Cargo.toml html_report
    --lib` passed: 1 passed, 0 failed.
  - Verification: `cargo check --manifest-path native/Cargo.toml` passed with
    only pre-existing dead-code/read warnings.
  - Verification: `DRAGONGUI_SMOKE_FRAMES=3 bash rpi_setup_and_run.sh
    build-smoke benchmarking/rpi_widget_probe.py` rebuilt and installed the
    native wheel after the Slice 15 native changes. Final probe snapshot
    reported Pi profile, GL backend, 800x480, primitive count `1030`, text
    count `33`, and queue depth `0`.
  - Verification: `DRAGONGUI_DEMO_PAGE=piecharts DRAGONGUI_SMOKE_FRAMES=3 bash
    rpi_setup_and_run.sh smoke benchmarking/rpi_v3_viewport_fit_check.py`
    passed at 800x480 after the rebuild.

### Start With Slice 1

First patch scope:

- Add passive debug/performance counters only.
- Do not alter command behavior, rendering behavior, layout, fonts, or V3 demo
  UI.
- Keep all debug snapshot changes additive.

Exact native fields to add or expose:

- `last_dirty`
- `last_primitive_rebuild_ms`
- `last_text_rebuild_ms`
- `last_primitive_instance_count`
- `last_text_entry_count`
- `last_drained_command_count`
- existing queue depth / queue latency fields if not already exposed in the same
  snapshot object

Exact implementation notes:

- Read primitive count from `PrimitivesRenderer::rect_count`.
- Add a cheap `TextRendererDg` count field/accessor after text rebuild.
- Time primitive/text rebuilds with `Instant`.
- Store counters in `WgpuState`; expose them under a nested debug object such as
  `"performance"` or `"last_frame"`.
- Do not clone primitive/text buffers to compute counts.

Files expected in Slice 1:

- `native/src/runtime.rs`
- `native/src/text/mod.rs`
- possibly `python/dragongui/app.py` or `python/dragongui/runtime.py` only if
  Python wrapper shape changes
- `examples/debug_snapshot_tool.py` only if it needs to print new fields

Tests/checks for Slice 1:

- `cargo test` from `native/`, or focused native tests if the full test run is
  too slow.
- `pytest` from the repo root if Python wrapper behavior changes.
- Run `examples/debug_snapshot_tool.py` manually.

Slice 1 acceptance:

- Existing snapshot fields still exist.
- New counters appear under a nested object.
- Counters are passive; no behavior changes are visible.
- Primitive count uses existing `rect_count`.
- Text count does not require expensive reconstruction.

### Then Slice 2

Second patch scope:

- Add a repeatable probe script or checklist that uses Slice 1 counters.
- Keep it outside core runtime behavior.
- Avoid hard performance assertions until Pi hardware baseline exists.

Expected probe output:

- scenario name;
- profile/backend/window backend;
- primitive count;
- text count;
- last dirty kind;
- primitive rebuild time;
- text rebuild time;
- queue depth / drained command count;
- queue latency if available;
- short notes for LinePlot streaming, hover, Histogram render, table scroll, and
  page/sidebar scroll.

Files expected in Slice 2:

- `benchmarking/rpi_widget_probe.py`
- `docs/raspberry-pi.md` for the command if the probe is useful immediately
- optionally `examples/all_features_v3_demo.py` only if extracting import-safe
  data helpers is necessary

Tests/checks for Slice 2:

- Local short-mode run exits cleanly.
- Probe output is stable enough to paste into an issue/support note.
- Pi run records first baseline at 800x480 and 1280x720 when hardware is
  available.

Slice 2 acceptance:

- One local command produces a compact baseline summary.
- One Pi command can be used later without editing the script.
- No pass/fail thresholds are enforced before hardware data exists.

### Milestone A Go / No-Go

Move on to Slice 3 only when:

- Slice 1 counters exist and are visible in `app.debug_snapshot()`.
- Slice 2 can collect a baseline summary.
- `cargo test` passes for the touched native areas.
- `pytest` passes if Python wrapper files changed.
- Debug snapshot additions are documented or scheduled in the docs audit.

Do not move on if:

- counters require expensive buffer cloning;
- new snapshot fields replace or rename old fields;
- the probe cannot run without opening the full V3 demo unnecessarily;
- queue depth/latency is still invisible when diagnosing stream overload.

### First Implementation Checklist

- Confirm current worktree state before editing.
- Implement Slice 1 only.
- Run focused tests.
- Inspect debug snapshot output.
- Then implement Slice 2.
- Run local short-mode probe.
- Record remaining Pi-only validation as pending, not failed.

## Final Plan Consistency Audit

Final pass completed on 2026-05-13.

Corrections made:

- Phase 0 no longer asks Slice 1 to add V3 demo status UI. Passive counters come
  first; optional demo display belongs to Slice 4.
- Phase 0 no longer implies the full visual smoke checklist must land before
  measurement. Screenshot needs are recorded early, but the visual smoke
  checklist belongs to Slice 13.
- The priority list now keeps Histogram live command plumbing and Histogram
  render budgeting as separate steps, matching the patch scope and risk audits.

Consistency checks:

- The phase plan, patch slices, sequencing audit, and implementation readiness
  audit all start with passive measurement and probe work.
- LinePlot rendering work remains blocked by command-ordering correctness.
- Layout, font, and viewport fixes remain separate slices.
- Histogram live update plumbing remains separate from render aggregation
  semantics.
- Desktop behavior preservation is repeated consistently across constraints,
  regression risk, API compatibility, rollback, and review checklists.
- GL/X11 is consistently treated as the validated Pi default path, with Vulkan
  kept as comparison/diagnostic unless hardware proves otherwise.

No remaining blocking inconsistencies were found. Future edits should update the
patch scope, sequencing, test coverage, rollback, and documentation sections
together when a slice changes.

## Priority Order

1. Pi measurement/debug counters, queue/upload diagnostics, and a repeatable
   benchmark command.
2. LinePlot coalescing barrier correctness.
3. Queue backpressure visibility and high-frequency command limits.
4. Direct rebuild audit and `rebuild_for_dirty` cleanup.
5. Sidebar/Page/Badge layout correctness.
6. Font consistency.
7. 800x480 viewport and fullscreen fit.
8. LinePlot segment-budget/downsampling/hover fast path.
9. Histogram live packed update/coalescing path.
10. Histogram render budget.
11. DataFrameTable compact metrics.
12. Visual smoke screenshots and Pi input checklist.
13. `rpi_setup_and_run.sh` and documentation refresh.
14. PieChart/HtmlReport policy cleanup.

## Risks

- GPU line rendering can become a larger renderer project. Start with segment
  budgets and solid-style simplification before committing to a full new
  pipeline.
- Measurement can become its own project. Keep Phase 0 intentionally small:
  counters and one repeatable script are enough to start.
- Too many Pi-specific branches can make desktop and Pi behavior diverge. Prefer
  profile knobs and tests over ad hoc `cfg` checks.
- Lowering visual fidelity too aggressively can make the library feel broken
  instead of optimized. Degradations should be visible in debug snapshots and
  documented.
- Python-side and native-side profile logic can drift. Long term, expose a
  single profile/cap helper to Python.

## Reaudit Notes

Reaudited immediately after writing on 2026-05-13.

Reaudited again against the code after adding implementation details on
2026-05-13.

Final safety audit against the current code on 2026-05-13.

Findings:

- **Priority mismatch:** Measurement was described as Phase 0 but listed fifth
  in priority order. Corrected so counters/probes come first. Without this,
  later optimization work would be hard to evaluate objectively.
- **Measurement must stay cheap:** Primitive count is already tracked by
  `PrimitivesRenderer::rect_count`, while text count needs a small renderer
  field/accessor. The plan avoids reconstructing counts from debug snapshots or
  cloning buffers.
- **Renderer scope risk:** A dedicated GPU LinePlot renderer may be too large
  for the first optimization pass. The plan now emphasizes segment budgets,
  solid-style simplification, and screen-bucket downsampling before a new
  pipeline.
- **LinePlot correctness comes before speed:** The command queue coalescing path
  in `native/src/commands.rs` can still merge appends across same-widget command
  barriers. The implementation notes prioritize preserving command order before
  adding more aggressive rendering optimizations.
- **Layout fix should precede demo cleanup:** Sidebar/Page/Badge fixes are kept
  ahead of documentation and demo cleanup because the V3 demo should not keep
  accumulating one-off layout workarounds.
- **Badge/Tag overflow is a library issue:** The plan now points at
  `apply_intrinsic_leaf_width`, Badge/Tag text clipping, and FlowLayout content
  width calculations instead of treating the V3 demo sidebar as the real fix.
- **Histogram native binning is speculative:** The plan keeps native histogram
  binning as optional after packed bin updates. The first concrete target is
  coalesced packed edges/counts, not moving all binning into Rust.
- **DataFrameTable scope is bounded:** The plan avoids broad table redesign and
  focuses on profile metrics, scroll smoothness, and partial-buffer clarity.
- **Text/grid table alignment matters:** Table primitive and text paths use the
  same metric assumptions today. The implementation notes now require a shared
  profile-aware metrics helper so compact Pi metrics do not desynchronize grid
  lines and cell text.
- **Primitive count already exists:** `PrimitivesRenderer` already exposes
  `rect_count`, so Phase 0 should read that field instead of adding a second
  primitive counter.
- **Text count needs a small extension:** `TextRendererDg` stores text entries
  privately and does not expose a cheap count. A count field/accessor is the
  smallest safe change.
- **LinePlot hover is part of the performance path:** The current hover lookup
  scans visible points in `nearest_line_plot_point`. Phase 2 now includes a
  bounded/indexed hover lookup so pointer movement does not become the next Pi
  bottleneck after rendering is optimized.
- **Histogram live updates cross the Python/native boundary:** There is no
  existing histogram enqueue path analogous to line plots. Phase 4 correctly
  requires both `python/dragongui/runtime.py` handle methods and native command
  bindings in `native/src/commands.rs`.
- **Badge ellipsis should not be first-pass scope:** Badge/Tag text clipping is
  already present, while a reusable ellipsis path is not obvious. The plan now
  chooses clipping first and treats ellipsis as a later tested enhancement.

Remaining open questions:

- What primitive/text count is acceptable for a steady 30 FPS Pi target?
- Should Pi defaults target 800x480 first, or should 1280x720 be the primary
  supported demo resolution?
- Should line style simplification be automatic under the Pi profile, or exposed
  as a documented widget/profile option?
- Should partial table sorting be disabled under Pi profile, or only marked as
  limited when the column buffer is partial?

## Reaudit Checklist

Use this checklist after each phase:

- Does the change preserve desktop defaults?
- Does it work with `DRAGONGUI_PROFILE=desktop` and `DRAGONGUI_PROFILE=pi`?
- Does it reduce one of: primitive count, text count, queue depth, upload size,
  rebuild count, or layout overflow?
- Is there a focused test or probe for the behavior?
- Is the V3 demo relying on a workaround that should live in the library?
- Is the degradation documented and intentional?
