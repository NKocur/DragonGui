# Implementation Notes: Badge Layout Pass

## Changed

- Added a dedicated badge visual-audit target:
  - `examples/css_feature_probes/badge_layout_probe.py`
  - manifest id: `badge-layout`
  - sizes: `900x640`, `390x720`, `320x640`
  - category: `widgets`
  - covers standalone `Badge` / `Tag`, inline `Button` / `SmallButton` badges, `Tab` badges, `NavItem` badges, styled `::badge` parts, long badge values, constrained parents, disabled states, and narrow/mobile cases.
- Updated inline badge geometry in `native/src/text/mod.rs` and `native/src/primitives/mod.rs`.
  - The previous inline `badge_rect()` returned `None` when a badge's preferred x position was at or left of the parent x. In narrow controls this made non-empty badges disappear.
  - Inline badge rects now cap badge width to the available right-side space inside the parent and allow the badge to sit flush against the parent left edge.
  - Text and primitive badge rect behavior now flows through a shared helper instead of duplicated local implementations.
- Consolidated inline badge sizing and geometry policy in `native/src/style.rs`.
  - Added `InlineBadgeLayout` and `inline_badge_layout_for_text()`.
  - The helper explicitly returns both `reserved_width` and the clipped `visible_rect`.
  - Policy: labels reserve the badge's preferred width plus gap so labels yield first; the visible pill is clipped to available parent width so narrow controls keep a badge affordance.
- Updated badge sizing helpers in `native/src/style.rs`.
  - Inline `::badge` preferred width now accounts for part padding and border width when no explicit part width is set.
  - Inline `::badge` preferred height now accounts for part padding and border width when no explicit part height is set.
  - Standalone badge/tag intrinsic width now includes styled border width.
- Updated CSS part layout application in `native/src/css_style.rs`.
  - Non-uniform part padding shorthands such as `Button::badge { padding: 5px 9px; }` are no longer dropped.
  - Because `PartLayoutStyle` currently stores a single padding value, non-uniform part padding is preserved as the maximum resolved edge. This conservatively retains the horizontal padding value used by the badge probe and badge sizing helper.
- Updated label text bounds for `Button`, `SmallButton`, `Tab`, and `NavItem` in `native/src/text/mod.rs`.
  - When reserved badge width consumes most of a parent, the main label clip right edge is clamped so text bounds do not invert.
  - The label can now collapse/clamp before the badge disappears.
- Added focused native regression tests:
  - `style::tests::inline_badge_layout_reserves_preferred_width_but_clips_visible_rect`
  - `style::tests::inline_badge_layout_accounts_for_part_padding_and_border`
  - `text::tests::narrow_button_badge_text_rect_stays_inside_parent`
  - `text::tests::narrow_tab_and_nav_badge_text_rects_stay_inside_parent`
  - `text::tests::button_label_clip_is_not_inverted_when_badge_consumes_width`
  - `text::tests::constrained_standalone_badge_and_tag_text_clip_to_pill_rect`
  - `primitives::tests::narrow_button_badge_pill_rect_stays_inside_parent`
  - `primitives::tests::narrow_tab_and_nav_badge_pill_rects_stay_inside_parent`
  - `primitives::tests::disabled_button_still_emits_inline_badge_pill_inside_parent`
  - `css_style::tests::non_uniform_part_padding_is_preserved_for_badge_sizing`
- Added targeted layout-policy tests documenting the non-badge `native/src/layout.rs` changes that were already present from the prior visual-audit pass:
  - `layout::tests::nonzero_logical_min_width_does_not_opt_out_of_intrinsic_leaf_width`
  - `layout::tests::percent_min_width_remains_parent_relative_constraint`
  - `layout::tests::calc_min_width_does_not_opt_out_of_intrinsic_leaf_width`
- Tightened `disabled_button_still_emits_inline_badge_pill_inside_parent` so it independently asserts the emitted badge-sized primitive has positive dimensions and stays within the disabled button parent.
- Added a conservative `badge-layout` snapshot bounds check to `tools/visual_audit.py`.
  - It walks inline badge nodes in the saved debug snapshot and fails the target if the computed visible badge rect has no area or exceeds its parent rect.
  - Native unit tests remain the authoritative coverage for exact styled padding/border sizing because the debug snapshot does not expose every part layout detail needed for exact renderer parity.
- Rebuilt and copied the native Python extension:
  - `native/target/release/_dragongui.dll`
  - `python/dragongui/_dragongui.pyd`
- Updated visual-audit artifacts:
  - `artifacts/visual_audit/report.json`
  - `artifacts/visual_audit/REPORT.md`
  - new badge screenshots and snapshots under `artifacts/visual_audit/screenshots/` and `artifacts/visual_audit/snapshots/`

## Observed Issues

- The new `badge-layout` probe reproduced the risky narrow cases from the spec: badge preferred width can exceed the available control width for buttons, tabs, and nav items.
- Before the fix, the native code path would drop the inline badge entirely when the computed badge x was at or before the parent left edge.
- The maintainability review found the first implementation duplicated that rule in the text and primitive renderers; the follow-up moved the rule to `native/src/style.rs`.
- The later maintainability review found the older broad layout policy changes were still undocumented in this badge-focused pass; this follow-up added targeted tests and an inline comment documenting the `min-width: 0` opt-out policy.
- The code review found the badge probe's non-uniform `::badge` padding was authored in CSS but dropped by the part layout application path. This follow-up preserves that padding conservatively and adds a CSS-level regression.
- After the fix, the `320x640` badge-layout capture shows narrow button, tab, and nav badges still visible and clipped inside their parents.
- The regenerated `badge-layout-320x640` snapshot shows `Button.styled::badge` with computed `layout.padding: 9.0`, confirming the probe now exercises the styled padding path.
- Existing mobile `navigation-widgets` remains generally cramped in the content pane, but the badge-specific failure is not present in the rerun: nav and tab badges remain visible within their controls.

## Validation

- `C:\Users\nashk\AppData\Local\Programs\Python\Python313\python.exe -m py_compile examples/css_feature_probes/badge_layout_probe.py tools/visual_audit.py` passed.
- `cargo fmt --manifest-path native/Cargo.toml -- --check` passed.
- `cargo check --manifest-path native/Cargo.toml` passed with the Python 3.13 PyO3/MSVC environment.
- Focused native tests passed:
  - `inline_badge_layout_reserves_preferred_width_but_clips_visible_rect`
  - `inline_badge_layout_accounts_for_part_padding_and_border`
  - `narrow_button_badge_text_rect_stays_inside_parent`
  - `narrow_tab_and_nav_badge_text_rects_stay_inside_parent`
  - `button_label_clip_is_not_inverted_when_badge_consumes_width`
  - `constrained_standalone_badge_and_tag_text_clip_to_pill_rect`
  - `narrow_button_badge_pill_rect_stays_inside_parent`
  - `narrow_tab_and_nav_badge_pill_rects_stay_inside_parent`
  - `disabled_button_still_emits_inline_badge_pill_inside_parent`
  - `non_uniform_part_padding_is_preserved_for_badge_sizing`
  - `nonzero_logical_min_width_does_not_opt_out_of_intrinsic_leaf_width`
  - `percent_min_width_remains_parent_relative_constraint`
  - `calc_min_width_does_not_opt_out_of_intrinsic_leaf_width`
- Visual audit reruns passed:
  - `badge-layout --sizes desktop,mobile,320x640` after the CSS part-padding fix
  - `core-widgets --sizes desktop,mobile`
  - `navigation-widgets --sizes desktop,mobile`
  - `typography --sizes desktop,mobile`
- Current report summary:
  - entries: 67
  - `pass`: 46
  - `needs_manual_interaction`: 21
  - `fail`: 0
  - `blocked`: 0
- PNG header and dimensions checked for the new badge screenshots:
  - `badge-layout-desktop-1.png`: `900x640`
  - `badge-layout-390x720.png`: `390x720`
  - `badge-layout-320x640.png`: `320x640`

## Remaining

- Full `cargo test --manifest-path native/Cargo.toml` was not run.
- The 21 `needs_manual_interaction` visual targets still require interaction-state coverage.
- The `navigation-widgets` mobile probe is still visually dense/cramped outside the badge-specific behavior; that appears to be broader probe/layout pressure, not the disappearing inline badge bug.
- Earlier broad visual-audit layout changes in `native/src/layout.rs` remain in the working tree from the prior pass; this follow-up documented their min-width behavior with targeted tests but did not split them into a separate branch/change.
- The visual badge probe now has a conservative snapshot bounds check, but it is not a pixel-perfect renderer assertion. Exact badge sizing policy is still covered by native helper/text/primitive tests.
- The working tree still contains pre-existing dirty/generated files from earlier workflow passes; this pass did not revert unrelated changes.
