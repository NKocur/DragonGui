# Summary: Wrapped Text Selection and Caret Geometry

## Final Outcome
The text selection and wrapped-caret fix is approved for archival/release handoff.

Selection painting now uses shaped COSMIC Text/glyphon geometry instead of fixed character-width estimates. This addresses the reported highlight problems where multi-line or wrapped selections were incomplete, full-line selections extended past actual text, and proportional-font selections did not follow selected glyph widths.

The follow-up wrapped-caret regression is also fixed. `native/src/text/mod.rs::caret_xy_for_buffer` now filters candidate layout runs by visual-run byte range before resolving the caret position, so a cursor inside a later wrapped visual line is no longer claimed by the first visual run of the same logical line.

## Important Decisions
- Kept existing text state, selection byte ranges, copy, paste, delete, and select-all behavior intact.
- Centralized visual selection geometry in shaped text helpers in `native/src/text/mod.rs`.
- Replaced primitive-layer selection rectangle math in `native/src/primitives/mod.rs` with shaped selection spans.
- Updated mouse hit-testing in `native/src/runtime.rs` to use shaped glyph positions so drag endpoints match painted highlights.
- Preserved code editor gutter offset handling at the caller/primitive integration layer.
- Added only dead-code allowances for retained older line/column state helpers instead of rewriting the state model.

## Validation
Post-fix tester verification passed for the text-selection and wrapped-caret scope:
- `cargo fmt --manifest-path native/Cargo.toml -- --check`: passed.
- `cargo check --manifest-path native/Cargo.toml`: passed with Python 3.13 PyO3/MSVC setup.
- `cargo test --manifest-path native/Cargo.toml shaped_caret_xy_uses_wrapped_visual_run -- --nocapture`: passed, 1 passed.
- `cargo test --manifest-path native/Cargo.toml shaped -- --nocapture`: passed, 6 passed.
- `cargo test --manifest-path native/Cargo.toml text_area_selection_rect_honors_horizontal_scroll_when_unwrapped -- --nocapture`: passed, 1 passed.

Full native suite result:
- `cargo test --manifest-path native/Cargo.toml`: failed in five apparently unrelated layout/chart tests.
- Summary: 568 passed, 5 failed, 12 ignored.
- Failing tests:
  - `layout::tests::calc_style_width_lowers_when_expression_is_single_unit`
  - `layout::tests::flow_layout_auto_width_controls_do_not_reserve_wrapped_height`
  - `layout::tests::horizontal_scroll_offset_moves_children_and_preserves_clip`
  - `layout::tests::percent_style_width_uses_parent_space`
  - `primitives::tests::bar_chart_value_labels_auto_contrast_and_accept_style_part_color`

## Remaining Follow-Ups
- Manual GUI smoke testing remains the main release gap.
- Manual checklist: `TextInput`, `NumberInput`, multi-line `TextArea`, wrapped `TextArea`, wrapped `LogView`, horizontally scrolled non-wrapping text, `CodeEditor` gutter selection, and proportional strings such as `iiii WWWW`.
- Triage the five full-suite layout/chart failures separately unless they are already known baseline failures.
- Keep the PyO3/MSVC native test setup documented: `PYO3_PYTHON=C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe` and `LIB` prefixed with `C:\Users\nashk\AppData\Local\Programs\Python\Python313\libs`.
- Track custom-font parity as a follow-up: transient geometry helpers currently use a fresh/default font system and alias map rather than the renderer's live font context.
- Add deeper BiDi, ligature, and complex-script coverage if those are expected first-class visual cases.

## Status
Reviewer approved the change. Automated coverage now passes for the selection-highlight issues and the wrapped-caret failure mode. Manual GUI smoke testing is still uncompleted, and the remaining full-suite failures are outside the completed text-selection geometry fix.
