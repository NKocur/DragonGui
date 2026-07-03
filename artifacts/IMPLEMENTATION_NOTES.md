# Implementation Notes

## Changed
- Fixed the wrapped caret regression in `native/src/text/mod.rs::caret_xy_for_buffer`.
  - The helper now filters candidate `layout_runs()` by the visual run's glyph byte range before resolving the caret x/y.
  - Wrapped visual runs that share the same logical line no longer let the first run claim cursors that belong to later wrapped runs.
  - Added `visual_run_byte_range` as a small local helper for the run byte-span calculation.
- Added shaped text geometry helpers in `native/src/text/mod.rs`:
  - `shaped_text_selection_rects` builds a COSMIC Text/glyphon buffer with the widget text font, line height, width, and wrap settings, then emits selection spans from `LayoutRun::highlight` with a caret-position fallback.
  - `shaped_text_cursor_at_point` uses the same shaped glyph positions for mouse hit-testing instead of line/column estimates.
- Replaced `native/src/primitives/mod.rs::emit_text_selection_rects` character-width math with shaped selection spans.
  - Multi-line and wrapped selections now emit per-visual-run rectangles.
  - Short full-line selections now stop at shaped text width instead of extending across the field.
  - Non-wrapping multiline selection still applies horizontal scroll; wrapping ignores horizontal scroll like rendering.
  - Code editor gutter offset is still handled by the caller before painting.
- Replaced `native/src/runtime.rs::text_cursor_at_point` column math with shaped hit-testing so drag endpoints match the painted selection geometry.
- Fixed the reviewed single-line hit-test regression by using the same vertically centered line-box origin for `TextInput` and `NumberInput` that painting uses.
- Tightened selection run clipping to use min/max glyph byte offsets within each shaped run instead of assuming first/last visual glyph order matches logical byte order.
- Left existing text state/copy/paste behavior intact. The old line/column state utilities are still present and marked as retained dead code.
- Added targeted unit tests in `native/src/text/mod.rs` for:
  - multi-line select-all producing multiple rects
  - short full-line selection not filling the field
  - proportional glyph width differences
  - wrapped logical lines producing multiple visual rects
  - hit-testing using shaped positions
  - caret y-position using the correct wrapped visual run
- Added a primitive-layer test for non-wrapping text area horizontal scroll shifting the selection rect by the scroll amount.

## Validation
- `cargo fmt --manifest-path native/Cargo.toml -- --check` passed.
- With `PYO3_PYTHON=C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe` and `LIB` prefixed with `C:\Users\nashk\AppData\Local\Programs\Python\Python313\libs`:
  - `cargo check --manifest-path native/Cargo.toml` passed.
  - `cargo test --manifest-path native/Cargo.toml shaped_caret_xy_uses_wrapped_visual_run -- --nocapture` passed.
  - `cargo test --manifest-path native/Cargo.toml shaped -- --nocapture` passed, 6 passed.
  - `cargo test --manifest-path native/Cargo.toml text_area_selection_rect_honors_horizontal_scroll_when_unwrapped -- --nocapture` passed, 1 passed.

## Remaining
- Full `cargo test --manifest-path native/Cargo.toml` was not rerun in this implementer pass; prior artifacts show unrelated layout/chart failures.
- Manual visual smoke test is still needed for text input, number input, text area, log view, code editor gutter selection, wrapped text, horizontal scroll, and proportional strings.
- The transient geometry helpers still use default font aliases, so custom loaded font aliases may differ from the renderer until geometry can share the renderer's live font system/alias map. This remains explicitly limited to default/system-font parity for now.
