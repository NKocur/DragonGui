# Implementation Notes: Reopened Adjacent Scatter Interaction Fix

## Changed In This Pass

- Fixed stale scatter interaction state in `native/src/runtime.rs`.
  - Added `cancel_scatter_interaction()` to clear `active_scatter_id`, orbit/pan/selection drag state, press position, temporary selection overlays, and interaction LOD.
  - Called it on `WindowEvent::CursorLeft` and `WindowEvent::Focused(false)` so a pointer-down in one plot followed by leaving the window cannot keep routing later movement to that stale scatter.
- Expanded native debug snapshots for scatter isolation checks.
  - Runtime snapshots now include `active_scatter_id`.
  - Per-scatter snapshots now include viewport clip/offset/size, data bounds, actor counts, hover state, and selection state.
- Added `examples/css_feature_probes/adjacent_scatter_interaction_probe.py`.
  - Builds adjacent explicit-id plots: `adjacent-left-scatter` (`Scatter3D`, 720 points, turbo, scalar bar) and `adjacent-right-scatter` (`ScatterPlot2D`, 360 points, viridis).
  - Uses Windows mouse input to exercise hover transitions, left/right clicks, gutter click, left drag, drag-outside/release, right drag, and wheel over both plots.
  - Prints import paths and emits `ADJACENT_SCATTER_INTERACTION_PASS` only after stable per-scatter signatures remain distinct and `active_scatter_id` is cleared.
- Registered the probe as `adjacent-scatter-interaction` in `examples/css_feature_probes/visual_audit_manifest.json`.
- Added `tools/visual_audit.py` validation for the adjacent interaction target.
  - The target now fails if stdout has a fail marker, lacks a pass marker, or omits import path diagnostics.
  - Added unit coverage in `tests/test_visual_audit.py`.
- Kept the prior startup-framing fix intact.
  - Base `Scatter3D` startup uploads still request `fit=true`.
  - `ScatterPlot2D` still uses its explicit 2D `FitScatterCamera` path.
- Rebuilt and copied the native release extension to `python/dragongui/_dragongui.pyd`.
- Reinstalled the package into global Python 3.13 with `pip install --upgrade --force-reinstall .`.

## Reproduction And Verification

- New interaction audit:
  - `adjacent-scatter-interaction`: `pass`
  - stdout records workspace imports:
    - `J:\Projects\DragonFrame\python\dragongui\__init__.py`
    - `J:\Projects\DragonFrame\python\dragongui\_dragongui.pyd`
  - after interaction playback:
    - `active_scatter_id`: `None`
    - left: `720` points, `turbo`, original bounds, no selection
    - right: `360` points, `viridis`, original bounds, no selection
- Existing visual audits after rebuild:
  - `all-features-professional-explore`: `pass` for `1440x900`, `1180x760`, `390x720`, `320x640`
  - `layout-plot-embedding`: `pass`
  - `scatter3d`: `needs_manual_interaction`
  - `scatter-plot-2d`: `needs_manual_interaction`
- Import proof after reinstall:
  - bare global Python imports `C:\Users\nashk\AppData\Local\Programs\Python\Python313\Lib\site-packages\dragongui\_dragongui.pyd`
  - workspace-prepended Python imports `J:\Projects\DragonFrame\python\dragongui\_dragongui.pyd`
  - both report native backend available.

## Remaining

- The required combined Python suite still has three unrelated failures in `tests/test_python_api.py`:
  - app shell/body default style expectations
  - workbench layout/main default style expectations
  - node graph selected terminal runtime bridge replacement
- `scatter3d` and `scatter-plot-2d` visual-audit targets still retain their expected `needs_manual_interaction` status for broader manual states outside this adjacent-plot regression.

---

# Prior Implementation Notes: Scatter Startup Fit And Audit Guard

## Changed In This Pass

- Fixed base `Scatter3D` startup framing in `python/dragongui/widgets.py`.
  - `Scatter3D._queue_startup_resources()` now sends the initial primary point upload with `fit=True`.
  - `ScatterPlot2D` opts out of that base upload fit and keeps its existing 2D-specific `FitScatterCamera` + parallel XY sync sequence, avoiding a duplicate fit.
- Added regression coverage in `tests/test_python_api.py`.
  - `test_scatter3d_startup_upload_requests_initial_fit` verifies startup `Scatter3D` point uploads carry the fit flag.
  - `test_scatterplot2d_startup_keeps_single_2d_fit` verifies the 2D plot still uses its explicit camera fit path.
- Added a professional Explore visual-audit guard in `tools/visual_audit.py`.
  - `all-features-professional-explore` now fails if a nonempty `SetScatterPointsPacked` startup command has neither `fit=true` nor a same-target `FitScatterCamera`.
  - This catches the prior false-pass state where `dg-126` uploaded points with `fit=false` while only right-side `dg-146` received `FitScatterCamera`.
- Added visual-audit unit coverage in `tests/test_visual_audit.py` for the old bad command history and the fixed command history.

## Verification From This Pass

- Regenerated Explore audit for `1440x900` and `1180x760`: `all-features-professional-explore` reports `pass`.
- Regenerated plot probes:
  - `layout-plot-embedding`: `pass`
  - `scatter3d`: `needs_manual_interaction`
  - `scatter-plot-2d`: `needs_manual_interaction`
- The refreshed `all-features-professional-explore-1180x760` log now records:
  - left primary scatter `dg-126`: `SetScatterPointsPacked ... fit=true`
  - right preview scatter `dg-146`: `SetScatterPointsPacked ... fit=false` followed by `FitScatterCamera`

## Remaining

- I did not find evidence of Python payload/cache reuse across adjacent plot widget ids. Cached payloads are per widget object and native command history records distinct target ids.
- Native hit testing already checks scatter viewport and scissor bounds in `ScatterWidget::contains_point`; no native code change was made in this pass.
- Static visual audit still does not simulate click/drag/wheel interactions between adjacent plots. The new guard covers startup framing command isolation, but fully automated adjacent-pointer interaction playback remains future work.
- The worktree contains prior splitter/table changes and visual audit artifact churn from the earlier pass; this pass did not revert or normalize those unrelated changes.

---

# Prior Implementation Notes: Splitter Pane Sizing Pass

## Changed

- Fixed fractional pane sizing in `python/dragongui/widgets.py`.
  - `Splitter(sizes=(0.7, 0.3))` now serializes as `["0.7fr", "0.3fr"]` instead of fixed pixel sizes.
  - `Pane(size=0.62)` now behaves as a flex/proportional pane (`size=None`, `flex=0.62`) instead of a fixed `0.62px` pane.
  - `Splitter.set_sizes(...)` now enqueues live `flex` changes when switching panes to `fr` sizes.
- Hardened native pane layout in `native/src/layout.rs`.
  - Legacy documents with `pane_size` in the fractional range `0 < n < 1` are treated as flex weights instead of fixed pixels.
  - Added `fractional_pane_sizes_distribute_splitter_space`.
- Updated `examples/all_features_professional_demo.py`.
  - Replaced ambiguous fractional `Pane(size=...)` calls with explicit `Pane(flex=...)` values.
- Fixed `DataFrameTable` support for `list[dict]` row data in `python/dragongui/dataframe.py`.
  - The professional demo table now reports real columns/cells instead of `columns=0`.
  - Added `test_dataframe_table_accepts_row_mapping_sequences`.
- Added visual-audit splitter utilization validation in `tools/visual_audit.py`.
  - Professional-demo targets now fail if a horizontal splitter leaves a large unexplained unused span.
  - Added unit tests for underused and filled splitter snapshots.

## Root Cause

The desktop/tablet panel failures were mostly caused by ambiguous pane sizing:

- The demo used values like `Pane(size=0.7)` and `Pane(size=0.3)` intending proportions.
- The Python API serialized numeric pane sizes as fixed pixels.
- Native layout treated those as tiny fixed sizes, disabled flex growth, then clamped panes to `min_size`.
- Splitters therefore used only the pane minimum widths and left hundreds of pixels blank.

There was a second visible issue on the data page:

- The demo passed `list[dict]` rows to `DataFrameTable`.
- The dataframe adapter did not infer columns for that shape, so the table rendered row indices with no real data columns.

## Before / After Geometry

From the spec/current evidence before this pass:

```text
data-1440x900:    splitter w=1190, pane widths 360 + 280, ~545px unused
explore-1180x760: splitter w=930,  pane widths 360 + 260, ~307px unused
overview-1180x760: splitter w=930, pane widths 320 + 260, large unused right side
```

After the fix:

```text
data-1440x900:    splitter w=1190, pane widths 743 + 444, unused 0
explore-1180x760: splitter w=930,  pane widths 479 + 448, unused 0
overview-1180x760: splitter w=930, pane widths 535 + 392, unused 0
reports-1180x760: splitter w=930, pane widths 538 + 389, unused 0
```

The data page table now reports `7` columns and displays route/owner/state/latency/error/throughput values in `all-features-professional-data-1440x900.png`.

## Python 3.13 Import Finding

Global Python 3.13 without path setup imports an installed package:

```text
C:\Users\nashk\AppData\Local\Programs\Python\Python313\Lib\site-packages\dragongui\__init__.py
C:\Users\nashk\AppData\Local\Programs\Python\Python313\Lib\site-packages\dragongui\_dragongui.pyd
```

With `sys.path.insert(0, "python")`, it imports the workspace package:

```text
J:\Projects\DragonFrame\python\dragongui\__init__.py
J:\Projects\DragonFrame\python\dragongui\_dragongui.pyd
```

`examples/all_features_professional_demo.py` already inserts the workspace `python` folder when run directly as a script, so `python examples/all_features_professional_demo.py` should use the local code. A bare `python -c "import dragongui"` will not.

## Visual Audit Output

- Regenerated all professional-demo screenshots/snapshots/logs.
- Regenerated `artifacts/visual_audit/contact_sheets/all-features-professional-contact-sheet.png`.
- Updated `artifacts/visual_audit/report.json` and `artifacts/visual_audit/REPORT.md`.
- Current report summary: `76` entries, `55 pass`, `21 needs_manual_interaction`, `0 fail`, `0 blocked`.
- Professional-demo validator sweep: `0` scroll violations, `0` splitter violations.

## Remaining

- `all-features-professional-explore-1180x760.png` still shows chart/rendering composition issues inside the now-correctly-sized scatter pane. That is separate from splitter sizing.
- Mobile views remain dense by design; prior scroll reachability remains intact.
- Full `tests/test_python_api.py` still has three unrelated failures in app-shell/workbench defaults and node-graph runtime bridge behavior.
