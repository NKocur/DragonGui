# Implementation Notes: Dramatic Default GUI Polish Pass

## Initial Defects Documented

- `artifacts/visual_audit/screenshots/app-shell-workbench-390x720.png`
  - Heading was clipped (`AppShell + Body + ...`).
  - Summary cards preserved desktop row assumptions and clipped/wasted label/body text in the narrow viewport.
- `artifacts/visual_audit/screenshots/responsive-layout-390x720.png`
  - Narrow capture showed cramped nested scrolling and fixed grid/panel sizing that hid content behind narrow regions.
- `artifacts/visual_audit/screenshots/layout-flex-stress-390x720.png`
  - Dense rows with long labels and mixed fixed/flexible controls were too squeezed on mobile.
- The first pass made defaults more compact, but the visual change was subtle. The screenshots still read as broad filled cards with similar surfaces and weak hierarchy.

## Changed

- Made default dark/light theme changes more visible in `python/dragongui/theme.py` and `native/src/theme.rs`.
  - Dark defaults now use a deeper app background, clearer panel/surface separation, brighter foreground text, cyan accent/focus tokens, `13px` default text, `5px` spacing, and `3px` radius.
  - Light defaults keep parity on density/radius while using crisper border/accent tokens.
  - Native `Theme::control_height()` now resolves to `25px` for the default theme.
- Reworked default framework styling in `native/src/framework.dg.css`.
  - App chrome, sidebars, workbench areas, status bars, toolbars, tabs, nav rows, tables, property rows, inputs, and buttons now use tighter metrics and stronger hierarchy.
  - Controls are flatter and denser, with smaller icon buttons, clearer active/focus states, compact nav/tab rows, and data-table defaults closer to a professional tool surface.
  - Added default class styling for common app-shell/workbench/property-grid arrangements so apps get a cleaner baseline without custom visual CSS.
- Tightened native metrics in `native/src/style.rs`, `native/src/layout.rs`, and `native/src/table.rs`.
  - Checkbox, toggle, slider, stepper, tab, table header/row, index-column, and data-column defaults are smaller and more data-dense.
  - Table defaults now use a `25px` header, `21px` rows, `48px` index column, and `116px` data columns.
- Fixed probe layouts where local CSS was creating screenshot-visible defects while preserving the stress content.
  - `examples/css_feature_probes/app_shell_workbench_probe.py`: mobile title gets adequate height, summary cards stack, card text wraps/ellipsizes inside bounds, and narrow workbench spacing is reduced.
  - `examples/css_feature_probes/responsive_layout_probe.py`: mobile panels use auto height, single-column named-grid areas, reduced padding/gaps, and scroll-owned overflow.
  - `examples/css_feature_probes/layout_flex_stress_probe.py`: mobile stress rows stack instead of forcing side-by-side cards and use tighter label/control metrics.
- Fixed the blocking toolbar review finding.
  - `python/dragongui/widgets.py`: `SearchBox` now reserves a default `164px` minimum width so its composed icon/input/clear children do not paint over following siblings when the wrapper is squeezed.
  - `examples/css_feature_probes/toolbar_probe.py`: the overfull single toolbar row is now split into icon-command and search/action toolbar rows, preserving Toolbar coverage without clipping controls on mobile.
- Added `examples/css_feature_probes/default_polish_stress_probe.py`.
  - Uses defaults as much as possible, with only minimal responsive layout CSS.
  - Covers AppShell/sidebar, toolbar/status density, nav badges, long labels, summary cards, table, property grid, and nested scroll text.
  - Stacks the shell at `390px` and `320px` so long nav labels are visible instead of clipped.
- Added `default-polish-stress` to `examples/css_feature_probes/visual_audit_manifest.json`.
- Regenerated focused visual-audit screenshots, snapshots, logs, `artifacts/visual_audit/REPORT.md`, and `artifacts/visual_audit/report.json`.

## Visual Review After Fix

- `app-shell-workbench-390x720.png`: heading and summary card copy are now visible; summary cards stack and remaining rows are scroll-owned.
- `responsive-layout-390x720.png`: first viewport no longer shows text hidden by nested fixed regions; lower content is reachable through the main scroll.
- `layout-flex-stress-390x720.png`: dense controls stay inside panels; below-fold action rows are scroll-owned rather than overlapping.
- `default-polish-stress-390x720.png` and `default-polish-stress-320x640.png`: long sidebar labels, toolbar controls, status badges, summary cards, and table/property panels render without static overlap or clipped labels in the first viewport.
- `toolbar-desktop-1.png` and `toolbar-390x720.png`: SearchBox has nonzero width and no longer overlaps or clips the `Deploy`/disabled controls.
- Current visual-audit report summary: `68` entries, `47 pass`, `21 needs_manual_interaction`, `0 fail`, `0 blocked`.

## Validation

- Used Python 3.12 because the spec's previously referenced Python 3.13 path is not present in this workspace:
  - `C:\Users\nashk\AppData\Local\Programs\Python\Python312\python.exe`
- Passed:
  - `python -m py_compile examples/css_feature_probes/default_polish_stress_probe.py examples/css_feature_probes/app_shell_workbench_probe.py examples/css_feature_probes/responsive_layout_probe.py examples/css_feature_probes/layout_flex_stress_probe.py`
  - `cargo fmt --manifest-path native/Cargo.toml -- --check`
  - `cargo check --manifest-path native/Cargo.toml`
  - `cargo test --manifest-path native/Cargo.toml dark_theme_defaults_are_compact_and_neutral -- --nocapture`
  - `cargo test --manifest-path native/Cargo.toml default_metrics_are_compact_for_dense_data -- --nocapture`
  - `cargo test --manifest-path native/Cargo.toml framework_defaults_install_and_remain_lower_precedence_than_user_css -- --nocapture`
  - `python tools/visual_audit.py --target default-polish-stress --sizes desktop,390x720,320x640 --append --wait-ms 1200 --timeout-ms 12000`
  - `python -m py_compile python/dragongui/widgets.py examples/css_feature_probes/toolbar_probe.py`
  - `python tools/visual_audit.py --target toolbar --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000`
- Earlier in this pass, the focused required visual targets were rerun and recorded in the report:
  - `app-shell-workbench`: `pass`
  - `responsive-layout`: `pass`
  - `layout-flex-stress`: `pass`
  - `layout-panel-bounds`: `pass`
  - `layout-grid-masonry`: `pass`
  - `layout-scrollable-composites`: `pass`
  - `form-controls`: `pass`
  - `property-grid`: `pass`
  - `toolbar`: `pass`
  - `data-table-upgrades`: `pass`
  - `core-widgets`, `navigation-widgets`, `tree-view`: `needs_manual_interaction`

## Remaining

- Full native `cargo test --manifest-path native/Cargo.toml` was not run.
- Python 3.13 validation was not run because that interpreter path is absent here.
- Python pytest was not rerun in this second pass; Python 3.12 does not have `pytest` installed, and a previous Python 3.11 run had four unrelated failures outside this theme/layout polish work.
- The `21 needs_manual_interaction` visual-audit targets still need hover/open/focus/overlay interaction coverage.
- Some long content intentionally continues below the first mobile viewport; that is expected where the owning scroll area is visible and content does not overlap or paint outside bounds.
