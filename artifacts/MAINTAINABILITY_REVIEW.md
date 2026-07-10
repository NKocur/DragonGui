# Maintainability Review

## Findings

### 1. Badge snapshot validation is target-specific logic inside the generic visual-audit runner

- Pattern: God files / multi-responsibility functions
- Location: `tools/visual_audit.py:331`, `tools/visual_audit.py:354`, `tools/visual_audit.py:404`
- Severity: minor
- Description: The new badge bounds check is useful and conservative, but it adds a target-id branch and badge-specific snapshot parsing directly to the generic visual-audit runner. This is acceptable for one probe-specific guard, but if more probes add semantic validators, `run_target()` and the surrounding script will accumulate target-specific behavior and become harder to change safely.
- Recommended fix: No immediate blocker. If another target needs semantic validation, introduce a small validator registry such as `TARGET_VALIDATORS = {"badge-layout": validate_badge_layout_snapshot}` and keep target-specific helpers grouped away from process/report orchestration.

## Reuse Audit Answers

- Did it reuse the existing domain model? Yes. The badge fix uses existing `WidgetNode.props.badge`, existing widget kinds, existing `NodeStyle`/part styling, and the existing visual-audit manifest/debug-snapshot model.
- Did it duplicate validation/formatting/parsing/API access? No significant production duplication remains. The text and primitive renderers now share `inline_badge_layout_for_text()` instead of duplicating badge geometry. The Python visual-audit bounds check necessarily approximates the renderer for a conservative probe assertion, but it does not replace the native helper tests.
- Is the code in the right layer? Yes for the badge code. Shared badge sizing/geometry lives with style sizing helpers, renderers consume it, and visual-audit snapshot validation stays in the harness. The broader layout min-width policy is now documented where it is implemented and covered by focused layout tests.
- Does each new abstraction earn its existence? Yes. `InlineBadgeLayout` and `inline_badge_layout_for_text()` isolate the policy that labels reserve preferred badge width while visible pills clip to available parent width.
- Was dependency direction preserved? Yes. There are no backward imports, new service layers, or dependency inversions.
- Did it delete obsolete code, or only add more? It removed the duplicated local badge geometry from text/primitives and replaced it with a shared helper. It added tests and a conservative visual-audit validator.
- Could a future developer understand why this change exists without asking the AI? Yes. `artifacts/IMPLEMENTATION_NOTES.md`, the inline helper comment in `native/src/style.rs`, and the min-width comment in `native/src/layout.rs` explain the intent.

## Overall Verdict

Clean.

The badge-specific maintainability issues have been addressed: geometry policy is shared, label reservation is explicit, the disabled badge test now checks containment independently, and the broad min-width policy is documented and tested. The only remaining note is organizational: avoid letting `tools/visual_audit.py` grow a new target-specific branch for every future semantic probe.
