# Code Review: Reopened Adjacent Scatter Interaction Fix

## Findings

No blocking findings.

The current pass addresses the prior review blocker: it adds an adjacent scatter interaction probe, adds native scatter-interaction cleanup for stale active state, expands debug snapshots so plot state can be compared per id, and verifies the rebuilt extension is installed into both the workspace package and global Python 3.13.

## Approval Status

Approved.

## Reviewed Changes

- `native/src/runtime.rs`
  - Adds detailed per-scatter debug state: viewport clip/offset/size, bounds, actor counts, hover metadata, selection state, and runtime `active_scatter_id`.
  - Adds `cancel_scatter_interaction()` and calls it on `WindowEvent::CursorLeft` and `WindowEvent::Focused(false)`.
  - The cleanup clears `active_scatter_id`, orbit/pan/selection state, press position, temporary selection overlays, and interaction LOD for the active scatter.
- `examples/css_feature_probes/adjacent_scatter_interaction_probe.py`
  - Creates explicit adjacent ids, distinct data ranges, distinct point counts, and distinct colormaps.
  - Exercises hover transitions, left/right/gutter clicks, left drag, drag-outside/release, right drag, and wheel over both plots.
  - Emits import-path diagnostics and `ADJACENT_SCATTER_INTERACTION_PASS` only after stable per-scatter signatures remain distinct and `active_scatter_id` is cleared.
- `tools/visual_audit.py`
  - Adds validation for the adjacent interaction target by checking fail/pass markers and import diagnostics.
  - Keeps the prior professional Explore startup-fit guard.
- `tests/test_visual_audit.py`
  - Adds unit coverage for the adjacent interaction log validator.
- Packaging/install verification
  - `artifacts/TEST_RESULTS.md` now records rebuild, reinstall, import paths, and matching native binary hashes.

## Verification Reviewed

From `artifacts/TEST_RESULTS.md` and direct spot checks:

- Python compile: passed.
- `cargo fmt`: passed.
- `cargo test ... scatter`: `62 passed`, `1 ignored`.
- `cargo test ... command_batch_coalesces_scatter_updates`: `2 passed`.
- `cargo check`: passed.
- Combined Python suite: `432 passed`, `3 failed`; the failures are the same unrelated existing failures documented in the artifact.
- `adjacent-scatter-interaction`: `pass`.
- `all-features-professional-explore`: `pass`.
- `layout-plot-embedding`: `pass`.
- `scatter3d` and `scatter-plot-2d`: expected `needs_manual_interaction`.
- Native binary hashes match for:
  - `python\dragongui\_dragongui.pyd`
  - `native\target\release\_dragongui.dll`
  - `C:\Users\nashk\AppData\Local\Programs\Python\Python313\Lib\site-packages\dragongui\_dragongui.pyd`
  - SHA-256: `8A97760CA59ED92AA6F2CA07DDA60AE9CF46BC7EFE608A1A67FC76B696A57E87`

The adjacent probe log confirms workspace imports and stable distinct plot signatures:

```text
dragongui_import=J:\Projects\DragonFrame\python\dragongui\__init__.py
dragongui_native_import=J:\Projects\DragonFrame\python\dragongui\_dragongui.pyd
ADJACENT_SCATTER_INTERACTION_PASS
active_scatter_id: null
left: 720 points, turbo
right: 360 points, viridis
```

## Risks And Notes

- `git status` shows the new adjacent probe and several professional demo files as untracked. They are present in the workspace and were used by the visual audit, but final integration must include intentional untracked source/artifact files or the manifest target will point at a missing script.
- The adjacent probe is Windows-specific because it drives Win32 mouse input via `ctypes`. That matches this reported environment, but cross-platform interaction coverage would need a different driver.
- The probe validates stable data/colormap/bounds/actor signatures and cleared active scatter state. It does not try to assert every expected camera delta from each drag/wheel operation; that is acceptable for the reported data-switch/combine defect, but deeper camera-isolation assertions would still be useful future coverage.
- The three full-suite Python failures remain residual suite noise outside this fix.
- If the user still sees the issue, the next required diagnostic is the exact launch command plus `dragongui.__file__` and `dragongui._dragongui.__file__` printed from inside that run.

[[APPROVED]]
[[COMMANDDOCK_DONE]]
