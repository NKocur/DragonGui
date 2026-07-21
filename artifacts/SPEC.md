# Spec: Fix Scatter Composite Render-Target Overlay In 3D Explore

## Goal

The professional demo's `3D Sensor Explore` tab still has a user-visible plot bug. There are two plots: a large `Scatter3D` on the left and a smaller `ScatterPlot2D` preview on the right. The user reports:

- clicking the left plot puts the data in the right place;
- clicking anywhere else causes the whole left plot area to be overlaid with the right plot.

The prior approved pass fixed stale scatter interaction state and added an adjacent interaction probe, but that probe only checked per-widget debug state signatures. It did not catch this visual compositing failure. The next change must focus on scatter render-target/composite isolation, not just data/pointer state.

## Source Of Truth

Read these before editing:

- `artifacts/IMPLEMENTATION_NOTES.md`
- `artifacts/TEST_RESULTS.md`
- `artifacts/CODE_REVIEW.md`
- `artifacts/visual_audit/REPORT.md`
- `artifacts/visual_audit/report.json`
- `artifacts/visual_audit/screenshots/all-features-professional-explore-1180x760.png`
- `artifacts/visual_audit/screenshots/all-features-professional-explore-1440x900.png`
- `artifacts/visual_audit/snapshots/all-features-professional-explore-1180x760.json`
- `artifacts/visual_audit/logs/all-features-professional-explore-1180x760.stdout.txt`
- `artifacts/visual_audit/screenshots/adjacent-scatter-interaction-1120x720.png`
- `artifacts/visual_audit/logs/adjacent-scatter-interaction-1120x720.stdout.txt`
- `examples/all_features_professional_demo.py`
- `examples/css_feature_probes/adjacent_scatter_interaction_probe.py`
- `examples/css_feature_probes/visual_audit_manifest.json`
- `native/src/runtime.rs`
- `native/src/scatter/mod.rs`
- `native/src/scatter/composite.wgsl`
- `tools/visual_audit.py`
- `tests/test_visual_audit.py`
- `tests/test_python_api.py`

## Current Evidence

The latest artifacts show these fixes are already present and should be preserved:

- `Scatter3D` startup uploads now request `fit=true`.
- `cancel_scatter_interaction()` clears stale active scatter state on cursor leave and focus loss.
- `adjacent-scatter-interaction` probe passes by checking point counts, bounds, colormaps, actor counts, and `active_scatter_id`.
- The native extension was rebuilt/reinstalled and hashes match across:
  - `native\target\release\_dragongui.dll`
  - `python\dragongui\_dragongui.pyd`
  - global Python 3.13 `site-packages\dragongui\_dragongui.pyd`

The remaining symptom is visual: the right plot image appears over the larger left plot after most clicks. That points to render composition/cache isolation.

Strong suspect in `native/src/runtime.rs`:

- Scatter widgets may render into per-widget `ScatterRenderTarget`s and then composite those textures to the swapchain.
- `ScatterCompositeRenderer` owns a single uniform buffer/bind group for `target_rect`.
- During frame encoding, `self.scatter_compositor.update_uniforms(...)` is called once per scatter, then `render(...)` is called for that scatter.
- Because command buffers reference the same uniform buffer, later `queue.write_buffer` calls can make all composite draws observe the last written `target_rect` instead of the per-draw rectangle.
- This can cause one scatter texture, often the right-side plot, to be drawn over another plot's rectangle after cached/offscreen rendering is activated by interaction/redraw.

Other possible but lower-priority causes to check:

- offscreen render target reuse with the wrong texture bind group;
- render target cache validity not including viewport offset/clip when it should;
- composite pass missing scissor/clip state;
- direct render vs cached render switching after clicks or interaction render scale changes;
- visual audit probe not comparing pixels after each click, so state checks pass while the visible surface is wrong.

## Required Outcomes

1. Fix scatter composite rectangle isolation.
   - Each scatter composite draw must use that scatter's own target rectangle.
   - Do not share a mutable uniform buffer in a way that lets later scatter draws overwrite earlier draw parameters.
   - Acceptable fixes include per-draw/per-target uniform buffers, dynamic uniform offsets, push constants if supported, or baking target rect into per-scatter composite bind state.
   - The fix must work when two or more scatters are visible and when cached/offscreen render targets are used.

2. Preserve per-widget render target isolation.
   - A scatter must composite its own color texture and never a neighbor's texture.
   - Render target cache keys/validity must include dimensions and any state needed for correct composition.
   - Viewport offset/clip changes must not cause a stale cached texture to draw into the wrong screen rect.

3. Add visual/pixel regression coverage.
   - Extend `adjacent_scatter_interaction_probe.py` or add a new probe that captures screenshots after:
     - startup;
     - click left plot;
     - click right plot;
     - click gutter/non-plot area;
     - click controls/outside the plot area on the professional Explore page if feasible.
   - Pixel-check that the left plot region does not become dominated by the right plot's color/shape after non-left clicks.
   - Pixel-check that the right plot stays within its own viewport and does not appear across the left plot rectangle.
   - State-only assertions are not enough.

4. Validate in the professional demo, not only the synthetic probe.
   - Reproduce the user's sequence on `all-features-professional-explore`: left plot click is okay; then click elsewhere and ensure no right-plot overlay appears on the left plot.
   - Capture screenshots before and after each click.
   - Record import paths and native binary hash in `artifacts/TEST_RESULTS.md`.

5. Keep prior fixes intact.
   - Startup `Scatter3D` framing remains correct.
   - `ScatterPlot2D` keeps its 2D fit/parallel projection behavior.
   - `cancel_scatter_interaction()` cleanup remains in place.
   - Adjacent plot point counts, bounds, colormaps, and actor state remain isolated.

## Affected Files And Modules

Primary:

- `native/src/runtime.rs`
  - `ScatterCompositeRenderer`
  - `ScatterRenderTarget`
  - per-scatter render/composite loop around `visible_scatter_order`
  - debug snapshot render metrics.
- `native/src/scatter/composite.wgsl`
  - composite shader inputs if target rect handling changes.
- `native/src/scatter/mod.rs`
  - offscreen render target sizing/scissor behavior and viewport clip reporting.
- `examples/css_feature_probes/adjacent_scatter_interaction_probe.py`
  - add pixel/screenshot checks after each interaction.
- `tools/visual_audit.py`
  - validate the new visual pass/fail markers and/or pixel-diff outputs.
- `tests/test_visual_audit.py`
  - unit coverage for the new validator behavior.

Secondary:

- `examples/all_features_professional_demo.py` and `examples/css_feature_probes/all_features_professional_demo_probe.py`, only for instrumentation or deterministic interaction hooks.
- `tests/test_python_api.py`, only if public Python behavior needs additional assertions.
- `artifacts/visual_audit/*`, regenerated screenshots/snapshots/logs for the relevant targets.

## Implementation Guidance

Start by confirming the render-path transition. In `native/src/runtime.rs`, inspect whether the failing click changes either scatter from direct render to cached render-target composite. The current loop computes `render_to_target`, may create/update `runtime.render_target`, calls `self.scatter_compositor.update_uniforms(...)`, then renders the composite using a shared compositor bind group.

If the shared uniform buffer is confirmed:

- make composite uniform data stable per draw;
- do not call `queue.write_buffer` repeatedly to the same uniform buffer for multiple draws in one command buffer unless the draw uses distinct dynamic offsets or distinct buffers;
- consider storing the composite uniform buffer/bind group on `ScatterRenderTarget`, since the target rect is per scatter/per frame;
- if target rect can change while the texture stays cached, update that scatter's own composite uniforms without affecting others.

Also consider adding scissor to the composite pass for the target rect. Even with correct uniforms, scissoring the composite draw to the scatter viewport gives a second guard against texture bleed.

Do not solve this by disabling caching globally unless used temporarily to prove the root cause. If caching is disabled as a workaround, document the performance tradeoff and add a follow-up note.

## Expected Behavior

After the fix:

- On `3D Sensor Explore`, clicking the left plot, right plot, controls, gutter, toolbar, or blank page area never draws the right plot over the left plot.
- The left `Scatter3D` remains in its own large viewport.
- The right `ScatterPlot2D` remains in its smaller preview viewport.
- Both direct render and cached/offscreen composite paths are visually correct.
- Adjacent interaction debug state remains isolated.
- The rebuilt/reinstalled global Python 3.13 package imports the fixed native extension.

## Validation Required

Minimum commands:

```powershell
$py = 'C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe'
$env:PYO3_PYTHON = $py
cargo fmt --manifest-path native\Cargo.toml -- --check
cargo test --manifest-path native\Cargo.toml scatter -- --nocapture
cargo test --manifest-path native\Cargo.toml command_batch_coalesces_scatter_updates -- --nocapture
cargo check --manifest-path native\Cargo.toml
& $py -m py_compile tools\visual_audit.py examples\all_features_professional_demo.py examples\css_feature_probes\adjacent_scatter_interaction_probe.py
& $py -m pytest tests\test_visual_audit.py tests\test_vdom.py -q
```

If native code changes, rebuild, copy, and reinstall:

```powershell
cargo build --manifest-path native\Cargo.toml --release --features pyo3/extension-module
Copy-Item -LiteralPath native\target\release\_dragongui.dll -Destination python\dragongui\_dragongui.pyd -Force
& $py -m pip install --upgrade --force-reinstall .
Copy-Item -LiteralPath native\target\release\_dragongui.dll -Destination python\dragongui\_dragongui.pyd -Force
```

Record binary hashes and imports:

```powershell
Get-FileHash native\target\release\_dragongui.dll, python\dragongui\_dragongui.pyd
& $py -c "import dragongui as dg, dragongui._dragongui as native; print(dg.__file__); print(native.__file__); print(dg.native_backend_available(), dg.backend_info())"
& $py -c "import sys; sys.path.insert(0, 'python'); import dragongui as dg, dragongui._dragongui as native; print(dg.__file__); print(native.__file__); print(dg.native_backend_available(), dg.backend_info())"
```

Required visual/probe reruns:

```powershell
& $py tools\visual_audit.py --target adjacent-scatter-interaction --sizes 1120x720 --append --wait-ms 2500 --timeout-ms 16000
& $py tools\visual_audit.py --target all-features-professional-explore --sizes 1440x900,1180x760 --append --wait-ms 1400 --timeout-ms 16000
& $py tools\visual_audit.py --target layout-plot-embedding --target scatter3d --target scatter-plot-2d --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 14000
```

Manual or automated professional demo check:

- launch the exact command the user uses, or record why that command is unavailable;
- print `dragongui.__file__` and `dragongui._dragongui.__file__` from inside that run;
- go to `3D Sensor Explore`;
- click left plot, then right plot, then controls/gutter/blank area;
- save screenshots after each click;
- verify no right-plot overlay appears in the left plot.

Document results in `artifacts/IMPLEMENTATION_NOTES.md` and `artifacts/TEST_RESULTS.md`.

## Edge Cases

- Two scatters using cached render targets in the same frame.
- One scatter direct-rendered and one scatter composited from cache.
- Interaction render scale below `1.0`.
- Auto-quality toggling render scale during drag.
- Full-scale cache hit after a direct render.
- Viewport offset changes while cached texture remains valid.
- Different plot sizes and aspect ratios.
- Adjacent plots with and without a gutter.
- Nested right plot inside a controls panel.
- More than two visible scatter widgets.
- Scroll/clipped scatter widgets.
- High-DPI scale factors.
- Empty or one-point scatter data.
- Reinstall/build path mismatch after native changes.

## Out Of Scope

- Removing the right preview plot.
- Disabling all scatter caching as the final solution without justification.
- Rewriting the professional demo layout.
- Replacing the scatter renderer wholesale.
- Broad redesign of chart widgets outside scatter compositing.
- Closing the loop based only on debug state signatures while the screenshot can still be wrong.
