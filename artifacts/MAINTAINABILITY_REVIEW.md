# Maintainability Review: Professional All Features Demo

## Findings

### 1. Fixture cache ignores later output directory changes

- Pattern: Happy-path state management in a broad example file
- Location: `examples/all_features_professional_demo.py:270`, `examples/all_features_professional_demo.py:946`
- Severity: minor
- Description: `ensure_demo_fixtures(output_dir)` only checks whether the global fixture objects are populated. If fixtures were already prepared for one directory, a later `build_app(other_dir)` returns the existing globals and keeps `REPORT_PATH` / `IMAGE_PATH` pointed at the first directory. The current app path and tests are fine because tests call `prepare_demo_fixtures(tmp_path, ...)` before `build_app(tmp_path)`, but the output-dir parameter looks stronger than it is.
- Recommended fix: No blocker for this demo. If this helper becomes reusable, track the prepared output directory and rebuild when `Path(output_dir)` changes, or remove the `output_dir` parameter from `ensure_demo_fixtures()` and require callers that need a specific directory to call `prepare_demo_fixtures(...)` explicitly.

No blocking or major maintainability findings remain.

## Reuse Audit Answers

- Did it reuse the existing domain model? Yes. The demo uses DragonGUI's public widgets and runtime APIs directly (`App`, `Window`, `Pages`, `DataFrameTable`, plot widgets, overlays, dialogs, resource APIs) instead of inventing a parallel framework.
- Did it duplicate validation/formatting/parsing/API access? No significant duplication was introduced. The bespoke `write_png` / `write_report` helpers generate demo assets, not alternate DragonGUI validation or API wrappers.
- Is the code in the right layer? Yes for an example script. Fixture generation is now explicit startup/test setup instead of import-time side effect, and UI composition remains in the demo.
- Does each new abstraction earn its existence? Yes. `DemoState` is a practical holder for live widget references in a single-file example, and `prepare_demo_fixtures` / `ensure_demo_fixtures` now isolate asset/dataset preparation clearly enough for tests and startup.
- Was dependency direction preserved? Yes. The new code imports the public `dragongui` API, standard library modules, and NumPy only; it does not reach into native internals or add backward dependencies.
- Did it delete obsolete code, or only add more? It intentionally adds a new professional demo and leaves `examples/all_features_v3_demo.py` intact. The follow-up removed the previous import-time side-effect behavior rather than layering more code over it.
- Could a future developer understand why this change exists without asking the AI? Yes. `artifacts/SPEC.md`, `artifacts/IMPLEMENTATION_NOTES.md`, the focused tests, and the page/function organization make the intent clear.

## Overall Verdict

Clean.

The earlier major maintainability issues are addressed: importing the demo is inert, fixture generation is explicit, background stream scheduling failures now keep diagnostic context, and focused tests cover import inertness plus small fixture/app construction. The remaining output-directory cache caveat is minor and acceptable for a single demo.

## Validation Checked

- `python -m py_compile examples\all_features_professional_demo.py tests\test_all_features_professional_demo.py`: passed.
- `python -m pytest tests\test_all_features_professional_demo.py -q`: `2 passed`.
