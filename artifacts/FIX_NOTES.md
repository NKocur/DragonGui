# Fix Notes: Adjacent Scatter Composite Overlay

## Root Cause

The remaining professional demo overlay bug was caused by `ScatterCompositeRenderer`
owning one mutable composite uniform buffer for `target_rect`. Every cached scatter
render target had a bind group pointing at that same buffer, so multiple offscreen
scatter composites encoded in one frame could all observe the last-written placement
rectangle.

That made the left plot texture render into the right plot rectangle when two
cached scatter widgets were visible.

## Code Changes

- Moved the mutable composite placement uniform out of `ScatterCompositeRenderer`
  and into each `ScatterRenderTarget` in `native/src/runtime.rs`.
- Kept the compositor pipeline, bind-group layout, and sampler shared.
- Changed each scatter render target bind group to bind its own
  `composite_uniform_buffer`.
- Changed the render loop to call `target.update_composite_uniforms(...)` before
  drawing that target's composite bind group.
- Added pixel-level validation for the `adjacent-scatter-interaction` visual audit
  in `tools/visual_audit.py`. The validator uses scatter viewport rectangles from
  the debug snapshot and checks that both visible plot viewports contain plot
  pixels, catching the previous state-only blind spot.
- Added a focused unit test in `tests/test_visual_audit.py` that fails when the
  left adjacent scatter viewport is blank.

## Rebuild

The native library was rebuilt and copied into the workspace Python package path:

```powershell
$env:LIB = "C:\Users\nashk\AppData\Local\Programs\Python\Python313\libs;$env:LIB"
$env:PYO3_PYTHON = "C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe"
cargo build --manifest-path native/Cargo.toml --release
Copy-Item native\target\release\_dragongui.dll python\dragongui\_dragongui.pyd -Force
```

The rebuilt DLL and package extension both have SHA256:

```text
BDB3964C49594B499D8BB3AABDFABA4940F29B4995328CF811C158283802766B
```

Review follow-up: the Python 3.13 installed `site-packages` extension was also
stale and has now been replaced with the rebuilt binary:

```text
C:\Users\nashk\AppData\Local\Programs\Python\Python313\Lib\site-packages\dragongui\_dragongui.pyd
BDB3964C49594B499D8BB3AABDFABA4940F29B4995328CF811C158283802766B
```

A plain Python 3.13 import now resolves to the installed package and fixed
extension:

```text
C:\Users\nashk\AppData\Local\Programs\Python\Python313\Lib\site-packages\dragongui\__init__.py
C:\Users\nashk\AppData\Local\Programs\Python\Python313\Lib\site-packages\dragongui\_dragongui.pyd
```

## Verification

Passed:

```powershell
cargo fmt --manifest-path native/Cargo.toml --check
cargo check --manifest-path native/Cargo.toml
py -3.13 -m pytest tests/test_visual_audit.py -q
```

Passed in isolated target output:

```powershell
py -3.13 tools\visual_audit.py --target adjacent-scatter-interaction --sizes 1120x720 --out artifacts\visual_audit_fix_check --wait-ms 2500 --timeout-ms 16000
```

The existing append-mode command also printed the target as pass:

```powershell
py -3.13 tools\visual_audit.py --target adjacent-scatter-interaction --sizes 1120x720 --append --wait-ms 2500 --timeout-ms 16000
```

That append-mode command still returned nonzero because the existing combined
`artifacts/visual_audit/report.json` already contains unrelated failing targets.

Visual inspection of the regenerated
`artifacts/visual_audit/screenshots/adjacent-scatter-interaction-1120x720.png`
shows the 3D scatter in the left panel and the 2D scatter in the right panel,
with no left texture composited into the right rectangle.

Review follow-up verification from the installed package path:

- Cleared `PYTHONPATH`.
- Added only `examples` to `sys.path` so `all_features_professional_demo` could
  be imported without adding the workspace `python` package.
- Confirmed `dragongui` and `_dragongui.pyd` imported from Python 3.13
  `site-packages`.
- Launched the professional demo on the `3D Explore` route.
- Captured `artifacts/site_packages_professional_explore/screenshot.png` and
  `artifacts/site_packages_professional_explore/snapshot.json`.
- The screenshot shows the large left 3D scatter and the right 2D scatter in
  separate panels with no overlay.
- The snapshot shows two distinct scatter widgets:
  - `dg-126`: 4000 points, `turbo`, viewport `[223, 256]` size `[664, 608]`
  - `dg-146`: 4000 points, `viridis`, viewport `[899, 502]` size `[505, 208]`
  - `active_scatter_id` is `null`.

## Intentionally Left Alone

- The prior `active_scatter_id` interaction-cancel changes were not reverted or
  altered; they address a separate interaction-state issue.
- The static cached target invalidation behavior in `set_layout_rect()` was left
  intact. It is safe once composite placement state is target-local.
- Unrelated existing visual audit/report artifacts and other dirty worktree files
  were not cleaned or modified.
