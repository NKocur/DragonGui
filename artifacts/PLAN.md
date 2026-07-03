# Plan: Verify and Finish Text Selection Highlight Geometry

## Goal
Fix DragonFrame/DragonGUI text selection geometry so logs, edit fields, text areas, code editors, and wrapped text behave consistently:
- Selection highlight must cover selected glyphs only, not extend far past the text.
- Multi-line and wrapped selections must paint one or more rectangles on every selected visual line.
- Mouse drag hit-testing and the text cursor indicator must move to the correct wrapped visual line.
- Existing selection state, copy, paste, delete, and select-all behavior should remain unchanged unless a state bug is proven.

## Workspace Findings
- The current workspace already contains a shaped-text implementation attempt in modified files:
  - `native/src/text/mod.rs`
  - `native/src/primitives/mod.rs`
  - `native/src/runtime.rs`
  - `native/src/events.rs`
- `native/src/text/mod.rs` now has:
  - `shaped_text_selection_rects`
  - `shaped_text_cursor_at_point`
  - `shaped_text_buffer`
  - caret/glyph helpers around `caret_xy_for_buffer`
- `native/src/primitives/mod.rs::emit_text_selection_rects` now calls `crate::text::shaped_text_selection_rects` instead of using fixed character-width column math.
- `native/src/runtime.rs::text_cursor_at_point` now calls `crate::text::shaped_text_cursor_at_point` for `TextInput`, `NumberInput`, `TextArea`, `CodeEditor`, and `LogView`.
- Existing artifacts report that automated selection-specific tests passed and reviewer approval was granted, but manual GUI smoke testing was not completed.
- The repository is currently dirty. Do not revert user or prior-agent changes.

## Dependency and Environment Findings
- `native/Cargo.toml` pins `glyphon = "0.11.0"`.
- `native/Cargo.lock` resolves `cosmic-text`.
- `native/Cargo.toml` uses `pyo3 = "0.23"` with `abi3-py312`; Rust tests on Windows/MSVC need a Python 3.12+ import library on the linker path.
- Prior test artifacts show default `python` resolved to MSYS2 Python and linking failed until Python 3.13 and `Python313\libs` were provided.

## Current Research Notes
- `glyphon` 0.11.0 re-exports COSMIC Text types, including `Buffer`, `Cursor`, `LayoutRun`, `LayoutGlyph`, `FontSystem`, `Wrap`, and related layout/editing APIs: https://docs.rs/glyphon/0.11.0/glyphon/
- COSMIC Text is the authoritative layout/shaping layer behind glyphon, covering shaping, font discovery/fallback, layout, rasterization, and editing: https://docs.rs/cosmic-text
- Current COSMIC Text `LayoutRun::highlight(cursor_start, cursor_end)` returns pixel spans for selected ranges within a visual run and can return multiple spans for mixed bidirectional text: https://docs.rs/cosmic-text/latest/cosmic_text/struct.LayoutRun.html
- COSMIC Text `LayoutRun::cursor_position`, `cursor_from_glyph_left`, and `cursor_from_glyph_right` are relevant for aligning caret and hit-test geometry with rendered glyph positions: https://docs.rs/cosmic-text/latest/cosmic_text/struct.LayoutRun.html
- COSMIC Text `Buffer` exposes `layout_runs`, `hit`, `cursor_position`, wrapping, sizing, and `shape_until_scroll`; layout must be shaped before reading run/hit geometry: https://docs.rs/cosmic-text/latest/cosmic_text/struct.Buffer.html

## Recommended Direction
The right technical direction remains: use shaped COSMIC Text layout as the single source of truth for selection rectangles and hit-testing. The current workspace appears to have implemented that. The next agent should verify it against the newest user symptom, especially "text cursor indicator doesn't change lines when wrapped", then make only targeted corrections.

## Concrete Steps
1. Re-read current implementation before editing:
   - `native/src/text/mod.rs` around `shaped_text_selection_rects`, `shaped_text_cursor_at_point`, `shaped_text_buffer`, `caret_xy_for_buffer`, and tests.
   - `native/src/primitives/mod.rs::emit_text_selection_rects`.
   - `native/src/runtime.rs::text_cursor_at_point`, `begin_text_selection`, and `update_text_selection`.
2. Confirm shaped selection spans:
   - Multi-line select-all emits multiple rectangles.
   - A short full-line selection stops at actual shaped width.
   - Proportional strings such as `iiii WWWW` do not use equal-width assumptions.
   - Wrapped text emits multiple visual-line rectangles even without newline characters.
   - Non-wrapping horizontal scroll shifts selection rectangles.
3. Specifically investigate wrapped cursor movement:
   - Drag or click across a wrapped logical line and confirm `shaped_text_cursor_at_point` returns cursor byte offsets from the visual run under the pointer.
   - Confirm the painted caret position uses the same visual wrapped run as hit-testing.
   - If the caret still stays on the first logical line, inspect whether `caret_xy_for_buffer` returns the first matching logical line run instead of the visual run containing the cursor.
   - If needed, add a helper that resolves cursor position from all `layout_runs()` and uses affinity/run range to choose the correct wrapped visual run.
4. Manual GUI smoke test required:
   - `TextInput`: select short text, select-all, drag both directions.
   - `NumberInput`: same selection checks with stepper offsets.
   - `TextArea`: multi-line select-all and drag from one line to another.
   - Wrapped `TextArea` or `LogView`: one long line wraps; click/drag on second/third visual line; caret and highlight must move there.
   - Non-wrapping text area: horizontally scroll, then select visible text.
   - `CodeEditor`: verify gutter offset does not shift highlight/caret.
5. Automated validation:
   - `cargo fmt --manifest-path native/Cargo.toml -- --check`
   - `cargo check --manifest-path native/Cargo.toml`
   - Configure Python 3.13 for PyO3/MSVC on this machine:
     - `PYO3_PYTHON=C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe`
     - Prefix `LIB` with `C:\Users\nashk\AppData\Local\Programs\Python\Python313\libs`
   - Run targeted tests:
     - `cargo test --manifest-path native/Cargo.toml shaped -- --nocapture`
     - `cargo test --manifest-path native/Cargo.toml text_area_selection_rect_honors_horizontal_scroll_when_unwrapped -- --nocapture`
   - Run full suite if time allows:
     - `cargo test --manifest-path native/Cargo.toml`
6. If tests or manual smoke reveal the wrapped caret bug persists:
   - Add a regression test for a long wrapped logical line where hit-testing on the second visual run produces a later cursor and caret y-position greater than the first run's `line_top`.
   - Keep edits localized to `native/src/text/mod.rs` and integration call sites.
   - Avoid changing selection state in `events.rs` unless the byte ranges are demonstrably wrong.

## Role Handoffs

### Tester
Verify the current shaped-text implementation against the user-visible wrapped caret and highlight behavior. Prioritize manual smoke testing for wrapped text because automated artifacts already cover most geometry math but did not complete real GUI validation. Use the Python 3.13 linker setup noted above before running native tests.

### Implementer
If Tester reproduces remaining wonky highlight or wrapped-caret behavior, inspect `shaped_text_cursor_at_point` and `caret_xy_for_buffer` first. The likely bug class is choosing the first logical line run instead of the correct visual wrapped run for a byte offset or pointer y-coordinate. Use COSMIC Text layout runs and cursor APIs rather than reintroducing fixed character-width math.

### Reviewer
Review any follow-up patch for:
- Visual geometry correctness across wrapped visual runs.
- Byte-index handling at UTF-8 boundaries.
- Consistent coordinate systems between painting, hit-testing, scroll offsets, and caret rendering.
- Gutter offset preservation in code editor.
- Avoidance of unrelated layout/chart/test-environment churn.

### Archivist
After successful targeted tests and manual smoke testing, update `artifacts/TEST_RESULTS.md` and `artifacts/SUMMARY.md` with exact commands, outcomes, remaining unrelated full-suite failures, and any manual cases verified.

## Risks
- Wrapped cursor placement may still diverge if caret rendering and hit-testing use different run-selection rules.
- `LayoutRun::highlight` is the best available API for BiDi/mixed text; fallback glyph interpolation may still be imperfect for ligatures, grapheme clusters, and complex scripts.
- The geometry helpers currently construct a fresh/default `FontSystem` and default alias map; custom runtime font aliases may still diverge from rendered text.
- Full native tests have known unrelated failures in prior artifacts. Do not block this fix on unrelated layout/chart failures unless they newly implicate text selection.
- Windows test runs can fail during linking if PyO3 sees the wrong Python installation.

## Stop Conditions
- Stop and ask for direction if fixing wrapped caret placement requires a broad text editing/state rewrite or public Python API changes.
- Stop if the selected byte ranges are unstable or invalid across ordinary edit operations; that would be a separate state-model bug.
- Stop if custom font parity requires sharing renderer-owned font systems through a large architectural change; document it as a follow-up unless the user explicitly prioritizes custom fonts.
- Stop if manual GUI smoke cannot be run in the current environment; record the blocker and complete automated verification.
