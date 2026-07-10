# Test Results: Dramatic Default GUI Polish Pass

## Environment

- Workspace: `C:\Users\nashk\Documents\Projects\DragonGui`
- Date: 2026-07-10
- Shell: PowerShell
- Python used: `C:\Users\nashk\AppData\Local\Programs\Python\Python312\python.exe`
- Cargo used: `C:\Users\nashk\.cargo\bin\cargo.exe`

## Summary

Focused validation for the stronger default visual polish pass passed.

Current `artifacts/visual_audit/report.json`:

- Entries: `68`
- `pass`: `47`
- `needs_manual_interaction`: `21`
- `fail`: `0`
- `blocked`: `0`

## Commands And Results

```powershell
$py='C:\Users\nashk\AppData\Local\Programs\Python\Python312\python.exe'
& $py -m py_compile examples\css_feature_probes\default_polish_stress_probe.py examples\css_feature_probes\app_shell_workbench_probe.py examples\css_feature_probes\responsive_layout_probe.py examples\css_feature_probes\layout_flex_stress_probe.py
& $py -m py_compile python\dragongui\widgets.py examples\css_feature_probes\toolbar_probe.py
```

Result: passed.

```powershell
$cargo='C:\Users\nashk\.cargo\bin\cargo.exe'
$py='C:\Users\nashk\AppData\Local\Programs\Python\Python312\python.exe'
$env:LIB='C:\Users\nashk\AppData\Local\Programs\Python\Python312\libs;' + $env:LIB
$env:PYO3_PYTHON=$py
& $cargo fmt --manifest-path native\Cargo.toml -- --check
& $cargo check --manifest-path native\Cargo.toml
```

Result: passed.

```powershell
& $cargo test --manifest-path native\Cargo.toml dark_theme_defaults_are_compact_and_neutral -- --nocapture
& $cargo test --manifest-path native\Cargo.toml default_metrics_are_compact_for_dense_data -- --nocapture
& $cargo test --manifest-path native\Cargo.toml framework_defaults_install_and_remain_lower_precedence_than_user_css -- --nocapture
```

Result: passed.

```text
theme::tests::dark_theme_defaults_are_compact_and_neutral ... ok
table::tests::default_metrics_are_compact_for_dense_data ... ok
css_style::tests::framework_defaults_install_and_remain_lower_precedence_than_user_css ... ok
```

Focused visual audit rerun:

```powershell
& $py tools\visual_audit.py --target default-polish-stress --sizes desktop,390x720,320x640 --append --wait-ms 1200 --timeout-ms 12000
& $py tools\visual_audit.py --target toolbar --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000
```

Result: passed.

```text
default-polish-stress: pass
toolbar: pass
```

Report consistency check:

```powershell
& $py -c "import json; from collections import Counter; data=json.load(open('artifacts/visual_audit/report.json', encoding='utf-8')); print(len(data), Counter(i.get('status') for i in data))"
```

Result:

```text
68 Counter({'pass': 47, 'needs_manual_interaction': 21})
```

## Visual Inspection Notes

- `app-shell-workbench-390x720.png`: pass. Heading, summary labels, body copy, and status bar are visible; vertical overflow is scroll-owned.
- `responsive-layout-390x720.png`: pass. Percent/calc and grid examples remain readable without nested static clipping in the first viewport.
- `layout-flex-stress-390x720.png`: pass. Dense controls remain inside panel bounds; below-fold rows are reached by scrolling.
- `default-polish-stress-390x720.png`: pass. Narrow shell stacks, long nav labels fit, and summary/table/property content stays contained.
- `default-polish-stress-320x640.png`: pass. Narrowest shell stacks and status/toolbar/nav labels remain visible; remaining content overflow is scroll-owned.
- `toolbar-desktop-1.png`: pass. SearchBox wrapper now has nonzero width and the search/icon/input/clear controls no longer paint over `Deploy` or the disabled button.
- `toolbar-390x720.png`: pass. Toolbar controls are split into icon and search/action rows so the required mobile capture does not clip controls at the panel edge.

## Not Run

- Full `cargo test --manifest-path native/Cargo.toml`.
- Full all-target visual audit from scratch.
- Python pytest in this second pass. Python 3.12 lacks `pytest`; previous Python 3.11 pytest validation had unrelated failures documented in prior notes.
- Manual interaction coverage for the `21 needs_manual_interaction` visual targets.
