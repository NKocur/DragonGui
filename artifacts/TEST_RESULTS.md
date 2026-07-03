# Test Results: Wrapped Text Selection and Caret Geometry

## Environment
- Workspace: `J:\Projects\DragonFrame`
- Date: 2026-07-03
- Shell: PowerShell
- PyO3/MSVC setup used for Rust tests:
  - `PYO3_PYTHON=C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe`
  - `LIB=C:\Users\nashk\AppData\Local\Programs\Python\Python313\libs;...`

## Summary
Post-fix verification passed for the selection/caret scope. The wrapped-caret regression that previously reproduced the user symptom now passes.

Covered and passing:
- Shaped multi-line selection rectangles.
- Short full-line selection stopping at shaped text width.
- Proportional glyph selection width.
- Wrapped logical line selection emitting multiple visual-line rects.
- Shaped hit-testing using glyph positions.
- Wrapped caret y-position using the correct visual run.
- Non-wrapping `TextArea` horizontal scroll shifting selection geometry.

Full native suite still fails in five apparently unrelated layout/chart tests.

## Workspace Inspection

```powershell
git status --short
```

Result:
```text
 M native/src/events.rs
 M native/src/primitives/mod.rs
 M native/src/runtime.rs
 M native/src/text/mod.rs
?? artifacts/
```

```powershell
rg -n "visual_run_byte_range|shaped_caret_xy_uses_wrapped_visual_run|fn caret_xy_for_buffer|pub\(crate\) fn shaped_text_cursor_at_point|shaped_text_selection_rects" native/src/text/mod.rs native/src/primitives/mod.rs native/src/runtime.rs
```

Result: confirmed the current fix and tests are present:
- `native/src/text/mod.rs::caret_xy_for_buffer`
- `native/src/text/mod.rs::visual_run_byte_range`
- `native/src/text/mod.rs::shaped_text_cursor_at_point`
- `native/src/text/mod.rs::shaped_text_selection_rects`
- `native/src/text/mod.rs::shaped_caret_xy_uses_wrapped_visual_run`
- `native/src/primitives/mod.rs` selection painting calls `shaped_text_selection_rects`

Implementation spot-check:
- `caret_xy_for_buffer` now obtains `visual_run_byte_range(&run, line_start)`.
- It skips wrapped visual runs where `cursor < run_start || cursor > run_end`.
- This prevents the first visual run of a wrapped logical line from claiming cursors that belong to later visual runs.

## Commands and Results

```powershell
cargo fmt --manifest-path native/Cargo.toml -- --check
```

Result: passed.

```powershell
$env:LIB='C:\Users\nashk\AppData\Local\Programs\Python\Python313\libs;' + $env:LIB
$env:PYO3_PYTHON='C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe'
cargo check --manifest-path native/Cargo.toml
```

Result: passed.

```powershell
$env:LIB='C:\Users\nashk\AppData\Local\Programs\Python\Python313\libs;' + $env:LIB
$env:PYO3_PYTHON='C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe'
cargo test --manifest-path native/Cargo.toml shaped_caret_xy_uses_wrapped_visual_run -- --nocapture
```

Result: passed.

Summary:
```text
running 1 test
test text::tests::shaped_caret_xy_uses_wrapped_visual_run ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 584 filtered out
```

```powershell
$env:LIB='C:\Users\nashk\AppData\Local\Programs\Python\Python313\libs;' + $env:LIB
$env:PYO3_PYTHON='C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe'
cargo test --manifest-path native/Cargo.toml shaped -- --nocapture
```

Result: passed.

Summary:
```text
running 6 tests
test text::tests::shaped_caret_xy_uses_wrapped_visual_run ... ok
test text::tests::shaped_selection_wraps_long_logical_line_into_visual_rects ... ok
test text::tests::shaped_selection_short_full_line_does_not_fill_control_width ... ok
test text::tests::shaped_selection_select_all_multiline_emits_per_line_rects ... ok
test text::tests::shaped_selection_uses_proportional_glyph_widths ... ok
test text::tests::shaped_hit_testing_uses_glyph_positions ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 579 filtered out
```

```powershell
$env:LIB='C:\Users\nashk\AppData\Local\Programs\Python\Python313\libs;' + $env:LIB
$env:PYO3_PYTHON='C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe'
cargo test --manifest-path native/Cargo.toml text_area_selection_rect_honors_horizontal_scroll_when_unwrapped -- --nocapture
```

Result: passed.

Summary:
```text
running 1 test
test primitives::tests::text_area_selection_rect_honors_horizontal_scroll_when_unwrapped ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 584 filtered out
```

```powershell
$env:LIB='C:\Users\nashk\AppData\Local\Programs\Python\Python313\libs;' + $env:LIB
$env:PYO3_PYTHON='C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe'
cargo test --manifest-path native/Cargo.toml
```

Result: failed in five tests that do not appear related to text selection.

Summary:
```text
test result: FAILED. 568 passed; 5 failed; 12 ignored; 0 measured; 0 filtered out
```

Failures:
```text
layout::tests::calc_style_width_lowers_when_expression_is_single_unit
layout::tests::flow_layout_auto_width_controls_do_not_reserve_wrapped_height
layout::tests::horizontal_scroll_offset_moves_children_and_preserves_clip
layout::tests::percent_style_width_uses_parent_space
primitives::tests::bar_chart_value_labels_auto_contrast_and_accept_style_part_color
```

Failure details:
```text
layout::tests::calc_style_width_lowers_when_expression_is_single_unit
assertion `left == right` failed
left: 340.0
right: 280.0

layout::tests::flow_layout_auto_width_controls_do_not_reserve_wrapped_height
auto-width button should grow beyond min-width for longer labels: Rect { x: 82.0, y: 0.0, w: 74.0, h: 32.0 }

layout::tests::horizontal_scroll_offset_moves_children_and_preserves_clip
assertion failed: scroll_container_max_x(root.children.first().unwrap(), &unscrolled) > 0.0

layout::tests::percent_style_width_uses_parent_space
assertion `left == right` failed
left: 590.0
right: 400.0

primitives::tests::bar_chart_value_labels_auto_contrast_and_accept_style_part_color
styled pie legend label
```

## Manual Smoke Status
Manual GUI smoke testing was not completed in this tester session. The automated geometry coverage now directly covers the wrapped-caret failure mode and the previous selection rectangle issues, but a real visual pass is still recommended before release.

Recommended manual cases:
- `TextInput`: select short text, select all, drag both directions.
- `NumberInput`: same checks with stepper offsets.
- `TextArea`: multi-line select-all and drag across lines.
- Wrapped `TextArea`: one long logical line wraps; click/drag on second and third visual lines; caret and highlight should move there.
- Wrapped `LogView`: same wrapped-line caret/highlight behavior.
- Non-wrapping text area: horizontally scroll, then select visible text.
- `CodeEditor`: verify gutter offset does not shift highlight or caret.
- Proportional string such as `iiii WWWW`: selection should follow shaped glyph width.

## Risk Notes
- The full native suite failures are unchanged in class from prior artifacts and are outside the text-selection scope.
- The PyO3/MSVC test environment still needs explicit Python 3.13 and `Python313\libs`; default `python` may link incorrectly.
- The shaped geometry helpers still construct a fresh/default font system and alias map, so custom loaded font aliases may diverge from renderer geometry.
- Additional BiDi, ligature, and complex-script coverage would be useful if those are expected to be first-class visual cases.

## Next Steps
1. Proceed to Archivist/release handoff for the automated text-selection and wrapped-caret fix.
2. Run the manual GUI smoke checklist above before release.
3. Triage the five full-suite layout/chart failures separately unless they are already known baseline failures.
4. Keep the Python 3.13 PyO3/MSVC setup documented for future native test runs.
