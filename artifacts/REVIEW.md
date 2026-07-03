# Review: Wrapped Text Selection and Caret Geometry

## Approval Status
Approved.

The follow-up patch addresses the remaining wrapped-caret bug described in `artifacts/TEST_RESULTS.md`. `native/src/text/mod.rs::caret_xy_for_buffer` now filters candidate layout runs by the visual run glyph byte range before resolving caret coordinates, so a cursor inside a later wrapped visual run is no longer claimed by the first visual run of the same logical line.

## Findings
No blocking findings remain for the stated goal.

## Reviewed Changes
- `native/src/text/mod.rs`
  - `caret_xy_for_buffer` now uses visual-run byte range filtering.
  - `visual_run_byte_range` was added as a localized helper.
  - The regression test `shaped_caret_xy_uses_wrapped_visual_run` now passes.
- `native/src/primitives/mod.rs`
  - Selection painting remains routed through shaped text rectangles.
  - Wrapped text ignores horizontal scroll while unwrapped text preserves horizontal scroll.
  - Code editor gutter offset remains applied before selection/caret geometry.
- `native/src/runtime.rs`
  - Mouse hit-testing remains routed through shaped text cursor resolution with matching padding, scroll, wrap, and gutter coordinate handling.
- `native/src/events.rs`
  - Only dead-code allowances were added for old state helpers; the selection state model was not rewritten.

## Validation Rerun
All commands below passed in this review session:

```powershell
cargo fmt --manifest-path native/Cargo.toml -- --check
```

```powershell
$env:LIB='C:\Users\nashk\AppData\Local\Programs\Python\Python313\libs;' + $env:LIB
$env:PYO3_PYTHON='C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe'
cargo check --manifest-path native/Cargo.toml
```

```powershell
$env:LIB='C:\Users\nashk\AppData\Local\Programs\Python\Python313\libs;' + $env:LIB
$env:PYO3_PYTHON='C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe'
cargo test --manifest-path native/Cargo.toml shaped_caret_xy_uses_wrapped_visual_run -- --nocapture
```

```powershell
$env:LIB='C:\Users\nashk\AppData\Local\Programs\Python\Python313\libs;' + $env:LIB
$env:PYO3_PYTHON='C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe'
cargo test --manifest-path native/Cargo.toml shaped -- --nocapture
```

```powershell
$env:LIB='C:\Users\nashk\AppData\Local\Programs\Python\Python313\libs;' + $env:LIB
$env:PYO3_PYTHON='C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe'
cargo test --manifest-path native/Cargo.toml text_area_selection_rect_honors_horizontal_scroll_when_unwrapped -- --nocapture
```

Targeted outcomes:
- Wrapped caret regression: passed, 1 passed.
- Shaped text tests: passed, 6 passed.
- Horizontal-scroll selection primitive test: passed, 1 passed.

## Gaps
- Manual GUI smoke testing is still not completed. The release checklist should still cover `TextInput`, `NumberInput`, multi-line `TextArea`, wrapped `TextArea` or `LogView`, horizontally scrolled unwrapped text, proportional strings such as `iiii WWWW`, and `CodeEditor` gutter selection.
- Full `cargo test --manifest-path native/Cargo.toml` was not rerun in this review pass. Prior artifacts list unrelated layout/chart failures.
- `shaped_text_selection_rects` is called with already-normalized selection ranges in production, but its own local reversed-range normalization is imperfect. This is non-blocking for current call sites because `WidgetState::normalized_text_selection` normalizes drag direction before painting, but a direct helper test for reversed ranges would be useful.
- The PyO3/MSVC test environment still requires explicitly setting Python 3.13 and its `libs` directory.

## Risks
- The geometry helpers still construct a fresh/default `FontSystem` and default alias map, so custom loaded font aliases may diverge from renderer geometry.
- BiDi, ligatures, and complex scripts depend on COSMIC Text highlight behavior with local fallbacks; additional coverage would reduce risk if those cases are important.
- If future call sites invoke `shaped_text_selection_rects` directly with reversed ranges, selection painting could be empty until the helper's local normalization is corrected.

## Recommendation
Proceed to archivist/release handoff for the automated fix. Manual GUI smoke testing remains the main unverified item.

[[APPROVED]]
[[COMMANDDOCK_DONE]]
