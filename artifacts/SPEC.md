# Spec: Dramatic Default GUI Polish And Layout Defect Pass

## Goal

The previous default visual-polish pass was too subtle. Do a stronger second pass that makes DragonGUI's default UI visibly cleaner, more professional, and more data-dense, while also fixing remaining screenshot-visible text clipping and overlap problems.

This pass should produce an obvious visual difference in the saved screenshots without requiring application authors to add custom CSS. The goal is not merely a token tweak; it is a coordinated default visual system and layout robustness pass.

## Source Of Truth

Read these before editing:

- `artifacts/IMPLEMENTATION_NOTES.md`
- `artifacts/CODE_REVIEW.md`
- `artifacts/visual_audit/REPORT.md`
- `artifacts/visual_audit/report.json`
- `artifacts/visual_audit/contact_sheets/contact-sheet-01.png`
- `artifacts/visual_audit/contact_sheets/contact-sheet-02.png`
- `artifacts/visual_audit/contact_sheets/contact-sheet-03.png`
- `artifacts/visual_audit/contact_sheets/contact-sheet-04.png`
- `artifacts/visual_audit/contact_sheets/contact-sheet-05.png`
- `artifacts/visual_audit/contact_sheets/contact-sheet-06.png`
- representative screenshots and snapshots in `artifacts/visual_audit/screenshots/` and `artifacts/visual_audit/snapshots/`

Important current state:

- The first pass changed dark/light tokens, radius, spacing, framework CSS, table metrics, and some native constants.
- User feedback: the changes are not noticeable enough. The next pass should be more dramatic.
- The audit report still shows `46 pass`, `21 needs_manual_interaction`, `0 fail`, `0 blocked`, but screenshots/contact sheets still show visible layout problems. Do not trust the status summary alone.
- `artifacts/visual_audit/screenshots/app-shell-workbench-390x720.png` visibly clips the title and summary-card text (`AppShell + Body + ...`, `Owner`, and wrapped body copy).
- `artifacts/visual_audit/screenshots/responsive-layout-390x720.png` shows cramped/nested scroll behavior and content cut off behind narrow regions.
- Contact sheets still show some captures labeled `[fail]` from prior visual state (`app-shell-workbench`, `layout-flex-stress`, `responsive-layout`) even when the current report now marks some targets pass. Inspect the actual current screenshots and snapshots, not only generated status.
- Several probes use local CSS with large gutters/cards. Product defaults still need to look better, but probe-local CSS can also be adjusted where it is part of the visual-audit presentation and masks or creates default-layout problems.

## Required Outcomes

1. A visibly different default look:
   - More refined hierarchy between app background, shell chrome, panels, controls, tables, and plot/data surfaces.
   - Less "same filled rounded rectangle everywhere."
   - Less bulky card/panel framing in dense workbench layouts.
   - Stronger selected/focus/active states that still feel professional.
   - Denser controls and rows where text remains readable.

2. Screenshot-visible layout fixes:
   - No clipped headings, card labels, button labels, nav labels, tab labels, or status-bar text in required audit screenshots.
   - No overlapping panel/card/control content in desktop or `390x720` captures.
   - Long text should wrap, ellipsize, or yield layout space predictably instead of painting outside its box or vanishing.
   - Narrow AppShell/workbench layouts must adapt instead of preserving desktop card rows that become unreadable.

3. Better visual audit coverage:
   - Add or update a default-only polish probe that does not hide defaults behind heavy custom CSS.
   - Add targeted stress cases for long text, narrow cards, nested scroll areas, nav/sidebar compression, summary cards, tables, and toolbar/status bar density.
   - Mark real static clipping/overlap as `fail` in the report notes until fixed; do not leave screenshot-obvious defects as `pass` or only `needs_manual_interaction`.

## Concrete Visual Direction

Make the change noticeable. Acceptable examples:

- Move from uniformly filled card blocks to a cleaner workbench style: darker app background, quieter sidebar/status chrome, flatter panels, restrained borders, stronger section headers, and denser rows.
- Reduce reliance on bordered cards. Use border only for real containers or interactive controls; use contrast, spacing, or subtle header treatment for hierarchy.
- Make primary controls feel crisper: lower vertical padding, consistent heights, clearer hover/active/selected fills, and tighter icon button metrics.
- Make data widgets look more like professional tools: compact rows, subdued grid lines, clear headers, stable column widths, and less large empty padding.
- Make navigation look like navigation: compact nav rows, clear active indication, less card-like sidebar buttons, tabs that fit labels/badges without chunky blocks.
- Make mobile/narrow layouts intentionally different: stack summary cards vertically or allow wrapping, reduce sidebar/content competition, and prevent text from being clipped inside fixed-height cards.

Avoid:

- Merely changing one or two colors.
- Increasing decorative gradients, large shadows, or playful rounding.
- Making all components smaller without solving text fit.
- Hiding defects by cropping screenshots, shortening test text, or removing stress cases.

## Required Targets To Inspect And Fix

High priority screenshots/probes:

- `app-shell-workbench`
  - Fix mobile title clipping and summary-card text clipping.
  - Ensure desktop does not waste most of the workbench area as empty body space unless content is genuinely scrollable and useful.
  - Reconsider default AppShell/sidebar/status metrics and probe layout behavior at narrow widths.
- `responsive-layout`
  - Fix narrow capture nested scrolling/cutoff and ensure percent/calc/grid examples keep readable labels.
  - If the probe's local CSS is causing bad visual presentation, update the probe while preserving real stress coverage.
- `layout-flex-stress`
  - Recheck mobile and desktop for overlap/cutoff with long labels and mixed fixed/flexible children.
- `layout-panel-bounds`
  - Recheck nested panel contents and scroll ownership.
- `layout-grid-masonry`
  - Recheck grid dense placement, panel labels, and narrow layout.
- `layout-scrollable-composites`
  - Recheck tables, code/log/text areas, and nested scrollbars for density without content clipping.
- `navigation-widgets`
  - Recheck mobile sidebar/tabs/nav labels, menu rows, badge alignment, and active states.
- `core-widgets`, `form-controls`, `property-grid`, `tree-view`, `toolbar`, `data-table-upgrades`
  - Recheck default controls for polish, clipping, and density.

Add a new explicit default stress target if needed:

- Suggested file: `examples/css_feature_probes/default_polish_stress_probe.py`
- Suggested manifest id: `default-polish-stress`
- Sizes: `[[1180, 760], [900, 640], [390, 720], [320, 640]]`
- Should use default theme/framework styles as much as possible.
- Include:
  - app shell with sidebar, top/body/status chrome
  - summary cards with long labels and values
  - toolbar with icon buttons, text buttons, separators, search/input
  - tabs with long labels and badges
  - nav list with long labels and badges
  - property grid and table together
  - nested panel/scroll area with long wrapped text
  - mobile/narrow stress cases with the same content

## Affected Files And Modules

Primary visual/default styling:

- `native/src/framework.dg.css`
  - This should be a larger second pass, not a minor adjustment.
  - Rework default component hierarchy, state styles, table/nav/tab/sidebar/status/toolbar parts, panel/control contrast, scrollbar defaults, and density.
- `python/dragongui/theme.py`
  - Update default dark and light tokens if needed for a more visible and professional palette.
- `native/src/theme.rs`
  - Keep native defaults in parity with Python defaults.

Primary layout/text robustness:

- `native/src/layout.rs`
  - Fix narrow AppShell/workbench/card/flex/grid behavior that preserves desktop assumptions too aggressively.
  - Ensure intrinsic sizes and shrink rules do not create clipped headings, controls, badges, or summary card text.
- `native/src/text/mod.rs`
  - Text bounds, wrapping, ellipsis, clipping, line height, baseline alignment, and narrow available-width behavior.
  - Long labels should not paint outside parent rects or disappear when width is small.
- `native/src/primitives/mod.rs`
  - Paint defaults, state fills, focus/selection, scrollbars, table/control chrome, and clipping consistency.
- `native/src/table.rs`
  - DataFrameTable density, header/row metrics, grid line visibility, selected row contrast, and column width defaults.
- `native/src/style.rs` and `native/src/css_style.rs`
  - Only if CSS/default-part support is needed for the stronger visual system or layout fixes.

Probe/audit files:

- `examples/css_feature_probes/app_shell_workbench_probe.py`
- `examples/css_feature_probes/responsive_layout_probe.py`
- `examples/css_feature_probes/layout_flex_stress_probe.py`
- `examples/css_feature_probes/layout_panel_bounds_probe.py`
- `examples/css_feature_probes/layout_grid_masonry_probe.py`
- `examples/css_feature_probes/layout_scrollable_composites_probe.py`
- `examples/css_feature_probes/navigation_widgets_probe.py`
- `examples/css_feature_probes/core_widgets_probe.py`
- `examples/css_feature_probes/form_controls_probe.py`
- `examples/css_feature_probes/property_grid_probe.py`
- `examples/css_feature_probes/tree_view_probe.py`
- `examples/css_feature_probes/toolbar_probe.py`
- `examples/css_feature_probes/data_table_upgrades_probe.py`
- `examples/css_feature_probes/visual_audit_manifest.json`
- `tools/visual_audit.py` if report status handling needs to flag static visual defects more accurately.

Test/artifact files:

- `tests/test_visual_audit.py`
- Native layout/text/table tests where practical.
- `artifacts/IMPLEMENTATION_NOTES.md`
- `artifacts/TEST_RESULTS.md`
- `artifacts/visual_audit/REPORT.md`
- `artifacts/visual_audit/report.json`

## Implementation Guidance

Start with visual diagnosis:

1. Open current screenshots for all high-priority targets above at desktop and `390x720`.
2. Compare snapshots against visible clipping/overlap. Identify whether each issue is caused by:
   - product layout/text behavior,
   - product default CSS/theme,
   - probe-local CSS/layout,
   - visual-audit status classification.
3. Write a short defect list in `artifacts/IMPLEMENTATION_NOTES.md` before or during implementation. Include screenshot names.

Then implement in this order:

1. Fix true clipping/overlap bugs first.
2. Add or update the default-only stress probe so product defaults are visible.
3. Make the stronger visual-system changes.
4. Re-run visual audits and inspect screenshots manually.
5. Iterate until the required screenshots show a clear improvement and no obvious static text clipping/overlap.

Specific requirements:

- Do not shorten labels or remove long-text stress cases as the fix.
- Use wrapping or ellipsis intentionally. If a widget has a fixed height, either reserve enough height for expected wrapped text or make the layout choose ellipsis.
- For summary cards and horizontal rows on mobile, prefer responsive stacking/wrapping over clipped columns.
- If local probe CSS is intentionally demo-like but makes the product look bad in audit artifacts, tune the probe presentation while keeping the same semantic stress.
- Add tests for any layout/text fix that can be asserted without visual inspection.
- If a visual issue is only detectable by screenshot, document before/after screenshots in `artifacts/IMPLEMENTATION_NOTES.md`.

## Expected Behavior

After this pass:

- The default theme should look obviously cleaner and more modern than the current screenshots.
- Default workbench screens should fit more content in the same viewport without feeling like every element is a separate card.
- Mobile `app-shell-workbench-390x720.png` should not clip the heading, summary card labels, summary card body text, or status bar content.
- Mobile `responsive-layout-390x720.png` should not show text hidden behind nested scrollbars or cut-off content in the first viewport.
- `layout-flex-stress`, `layout-panel-bounds`, `layout-grid-masonry`, and `layout-scrollable-composites` should not show obvious text overlap or child painting outside parent bounds.
- Nav/sidebar/tab/toolbars should read as compact app chrome and not as chunky demo cards.
- Tables and property grids should show meaningful density and remain readable.
- Visual audit statuses should match reality: static screenshot defects should be recorded as `fail` until fixed.

## Validation Required

Use the available local Python interpreter. Previous notes found Python 3.13 absent and Python 3.12 available at:

```powershell
$py = 'C:\Users\nashk\AppData\Local\Programs\Python\Python312\python.exe'
$env:LIB='C:\Users\nashk\AppData\Local\Programs\Python\Python312\libs;' + $env:LIB
$env:PYO3_PYTHON=$py
```

Minimum checks:

```powershell
cargo fmt --manifest-path native/Cargo.toml -- --check
cargo check --manifest-path native/Cargo.toml
& $py -m py_compile tools/visual_audit.py python/dragongui/*.py
```

Run focused native/Python tests added for this pass. Also rerun any existing tests touched by layout/theme/table/style changes.

If native code changes affect the Python extension used by the visual harness, rebuild and copy:

```powershell
cargo build --manifest-path native/Cargo.toml --release
Copy-Item native/target/release/_dragongui.dll python/dragongui/_dragongui.pyd -Force
```

Required visual audit rerun:

```powershell
& $py tools/visual_audit.py --target app-shell-workbench --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000
& $py tools/visual_audit.py --target responsive-layout --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000
& $py tools/visual_audit.py --target layout-flex-stress --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000
& $py tools/visual_audit.py --target layout-panel-bounds --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000
& $py tools/visual_audit.py --target layout-grid-masonry --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000
& $py tools/visual_audit.py --target layout-scrollable-composites --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000
& $py tools/visual_audit.py --target navigation-widgets --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000
& $py tools/visual_audit.py --target core-widgets --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000
& $py tools/visual_audit.py --target form-controls --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000
& $py tools/visual_audit.py --target property-grid --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000
& $py tools/visual_audit.py --target tree-view --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000
& $py tools/visual_audit.py --target toolbar --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000
& $py tools/visual_audit.py --target data-table-upgrades --sizes desktop,mobile --append --wait-ms 1200 --timeout-ms 12000
```

If `default-polish-stress` is added:

```powershell
& $py tools/visual_audit.py --target default-polish-stress --sizes desktop,mobile,320x640 --append --wait-ms 1200 --timeout-ms 12000
```

Manual screenshot review is required after the commands. Record the reviewed screenshot names and remaining defects in `artifacts/IMPLEMENTATION_NOTES.md` and update `artifacts/TEST_RESULTS.md`.

## Edge Cases

- Long headings in narrow AppShell bodies.
- Summary cards with short labels but long descriptions.
- Three or more cards in a row at `390x720`.
- Sidebar plus content plus status bar on mobile/narrow widths.
- Long nav item labels, tab labels, badges, menu labels, table headers, property names, and toolbar button labels.
- Nested scroll areas inside panels.
- Mixed fixed/flexible children in `HLayout`, `VLayout`, `FlowLayout`, and grid layouts.
- Text wrapped to multiple lines inside fixed-height widgets.
- High DPI rounding on 1px borders, baselines, and scrollbars.
- Disabled/selected/focused/open/hover states after visual-system changes.
- Light theme parity for token changes.

## Out Of Scope

- A new public widget API.
- Replacing the entire layout engine.
- Rewriting plot rendering algorithms.
- Making a custom one-off demo theme unrelated to product defaults.
- Removing local CSS support or breaking user stylesheet precedence.
- Hiding problems by deleting stress content, shortening labels, or reducing audit coverage.
- Treating manual interaction-only states as complete without either manual review notes or automated interaction coverage.
