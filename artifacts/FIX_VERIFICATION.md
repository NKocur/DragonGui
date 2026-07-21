# Fix Verification: Adjacent Scatter Composite Overlay

## Verdict

Approved for this fix loop. The installed workspace extension matches the rebuilt
native library, the original adjacent-scatter reproduction no longer shows the
overlay failure, the professional demo Explore capture no longer shows the left
plot composited into the right plot, and the focused regression tests pass.

## Binary Install Check

Command:

```powershell
Get-FileHash native\target\release\_dragongui.dll, python\dragongui\_dragongui.pyd | Format-Table -AutoSize
```

Result:

- `native\target\release\_dragongui.dll`: `BDB3964C49594B499D8BB3AABDFABA4940F29B4995328CF811C158283802766B`
- `python\dragongui\_dragongui.pyd`: `BDB3964C49594B499D8BB3AABDFABA4940F29B4995328CF811C158283802766B`

The rebuilt native library was copied into the Python package used by the probes.

## Reproduction Runs

Original reproduction command:

```powershell
$py='C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe'
& $py tools\visual_audit.py --target adjacent-scatter-interaction --sizes 1120x720 --append --wait-ms 2500 --timeout-ms 16000
```

Result:

- Printed `adjacent-scatter-interaction: pass`.
- Process exited `1` because append mode reused the shared
  `artifacts\visual_audit\report.json`, which already contains unrelated failing
  targets. The adjacent-scatter target itself passed.

Isolated reproduction command:

```powershell
$py='C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe'
& $py tools\visual_audit.py --target adjacent-scatter-interaction --sizes 1120x720 --out artifacts\visual_audit_fix_verification --wait-ms 2500 --timeout-ms 16000
```

Result:

- Exit code `0`.
- Printed `adjacent-scatter-interaction: pass`.
- Screenshot inspected: `artifacts\visual_audit_fix_verification\screenshots\adjacent-scatter-interaction-1120x720.png`.
- The left 3D scatter plot is present in the left viewport and the right 2D
  scatter remains in the right viewport. I did not see the previous blank-left /
  right-overlay failure.

Professional demo Explore command:

```powershell
$py='C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe'
& $py tools\visual_audit.py --target all-features-professional-explore --sizes 1440x900 --out artifacts\visual_audit_professional_explore_verification --wait-ms 2500 --timeout-ms 20000
```

Result:

- Exit code `0`.
- Printed `all-features-professional-explore: pass`.
- Screenshot inspected:
  `artifacts\visual_audit_professional_explore_verification\screenshots\all-features-professional-explore-1440x900.png`.
- The large left `Scatter3D` and the smaller right/control-panel `ScatterPlot2D`
  are both visible in their own rectangles with no visible plot overlay.

## Regression Tests

Commands:

```powershell
cargo fmt --manifest-path native/Cargo.toml --check
cargo check --manifest-path native/Cargo.toml
$py='C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe'
& $py -m pytest tests\test_visual_audit.py -q
& $py -m pytest tests\test_all_features_professional_demo.py -q
```

Results:

- `cargo fmt --manifest-path native/Cargo.toml --check`: passed.
- `cargo check --manifest-path native/Cargo.toml`: passed.
- `tests\test_visual_audit.py`: `15 passed in 0.26s`.
- `tests\test_all_features_professional_demo.py`: `2 passed in 0.19s`.

## Test Coverage Added By The Fix

The fix added `test_adjacent_scatter_interaction_screenshot_requires_plot_pixels`
in `tests\test_visual_audit.py`. This is the necessary coverage for this bug
class because the earlier state-signature checks could pass while the pixels were
drawn into the wrong viewport. The test constructs adjacent scatter viewport
metadata and verifies that both plot rectangles contain plot pixels; it fails for
the previous blank-left/overlaid-right screenshot pattern.

The visual audit runner now also validates the adjacent scatter screenshot for
plot pixels when running `adjacent-scatter-interaction`.

## Remaining Failures / Caveats

- No remaining failures found in the focused verification commands.
- The original append-mode visual-audit command still exits nonzero due to
  unrelated pre-existing failures in the shared report file, not because the
  adjacent-scatter target failed.
- I verified the automated click/interaction probe and the professional Explore
  capture. I did not perform an open-ended manual GUI session beyond inspecting
  the regenerated screenshots.
