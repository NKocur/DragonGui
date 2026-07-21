# Fix Review: Adjacent Scatter Composite Overlay

## Approval Status

Approved.

The fix now addresses both the native root cause and the prior review blocker around the installed Python package. The workspace binary, release DLL, and Python 3.13 site-packages extension all match the rebuilt fixed hash.

## Findings

No blocking findings remain.

The previous review blocker is resolved. Current direct hash check:

```text
native\target\release\_dragongui.dll
BDB3964C49594B499D8BB3AABDFABA4940F29B4995328CF811C158283802766B

python\dragongui\_dragongui.pyd
BDB3964C49594B499D8BB3AABDFABA4940F29B4995328CF811C158283802766B

C:\Users\nashk\AppData\Local\Programs\Python\Python313\Lib\site-packages\dragongui\_dragongui.pyd
BDB3964C49594B499D8BB3AABDFABA4940F29B4995328CF811C158283802766B
```

A plain `py -3.13` import with `PYTHONPATH` cleared now resolves to the installed package and loads the fixed extension:

```text
C:\Users\nashk\AppData\Local\Programs\Python\Python313\Lib\site-packages\dragongui\__init__.py
C:\Users\nashk\AppData\Local\Programs\Python\Python313\Lib\site-packages\dragongui\_dragongui.pyd
BDB3964C49594B499D8BB3AABDFABA4940F29B4995328CF811C158283802766B
```

## Root-Cause Review

The native compositor change in `native/src/runtime.rs` matches the diagnosis rather than masking symptoms:

- `ScatterCompositeRenderer` no longer owns one shared mutable `uniform_buffer`.
- `ScatterRenderTarget` now owns `composite_uniform_buffer`.
- Each render target bind group binds that target's own uniform buffer and texture view.
- The render loop updates the current target's own composite uniforms immediately before drawing that target.

That removes the failure mode where multiple cached scatter composite draws in one frame could all observe the last-written `target_rect` and place a left plot texture inside the right plot rectangle.

The prior `active_scatter_id` cleanup remains in place. It is adjacent interaction hardening, but the actual overlay fix is the per-target compositor state.

## Verification Review

Adequate verification was performed for this fix loop:

- Source-tree adjacent scatter audit passed in isolated output.
- Source-tree professional Explore visual audit passed.
- Installed-package professional Explore launch path was verified after clearing `PYTHONPATH` and importing `dragongui` from Python 3.13 `site-packages`.
- Installed-path screenshot inspected at `artifacts/site_packages_professional_explore/screenshot.png`; it shows the left 3D scatter and right 2D scatter in separate panels with no visible overlay.
- Installed-path snapshot confirms distinct scatter widgets:
  - `dg-126`: 4000 points, `turbo`, viewport `[223, 256]`, size `[664, 608]`
  - `dg-146`: 4000 points, `viridis`, viewport `[899, 502]`, size `[505, 208]`
  - `active_scatter_id=null`
- Reported focused tests passed:
  - `cargo fmt --manifest-path native/Cargo.toml --check`
  - `cargo check --manifest-path native/Cargo.toml`
  - `py -3.13 -m pytest tests/test_visual_audit.py -q`
  - `py -3.13 -m pytest tests/test_all_features_professional_demo.py -q`

The installed-path Explore snapshot did not itself hit the cached composite path in that frame (`last_render_cache_hit=false`, `last_render_composite_ms=0.0` for both scatters). That is acceptable because the adjacent scatter audit specifically exercises the cached/composite path, and all import paths now load the same fixed binary hash.

## Risks And Notes

- The screenshot validator added for `adjacent-scatter-interaction` catches the prior blank-left failure, but it remains a coarse content-presence check. A stronger future guard would compare left/right color signatures to detect cross-viewport leakage explicitly.
- The working tree still contains broad unrelated generated artifacts and pre-existing changes. Final integration should intentionally include only the compositor fix, focused probes/tests, and needed demo/audit assets.
- The append-mode visual audit can still return nonzero due to unrelated pre-existing failures in the shared report file; that is not evidence this scatter fix failed.

[[APPROVED]]
[[COMMANDDOCK_DONE]]
