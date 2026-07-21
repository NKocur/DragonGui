# DragonGUI Visual Audit Report

Generated: 2026-07-12 23:36:31 Eastern Daylight Time

This is a manifest-driven visual audit. `pass` means the saved screenshot state was visually reviewed against `artifacts/SPEC.md` and no obvious defect was recorded. `needs_manual_interaction` means the static capture was reviewed, but important hover/open/drag/focus or animated states still need manual or automated interaction coverage.

## Summary

- pass: 1
- needs_manual_interaction: 0
- fail: 0
- blocked: 0

## Targets

### All Features Professional Demo - Explore (`all-features-professional-explore`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\all_features_professional_demo_probe.py`
- Features: `Professional demo`, `Scatter3D`, `ScatterPlot2D`, `Splitter`, `Pane`, `Toolbar`, `SearchBox`
- Screenshots: `screenshots/all-features-professional-explore-1440x900.png`
- Debug snapshots: `snapshots/all-features-professional-explore-1440x900.json`
- Logs: `logs/all-features-professional-explore-1440x900.stdout.txt`, `logs/all-features-professional-explore-1440x900.stderr.txt`
- Notes: Route-specific capture for the professional all-features demo 3D explore page. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`, `native/src/scatter/mod.rs`, `native/src/table.rs`
- Reproduction: `python tools/visual_audit.py --target all-features-professional-explore --sizes 1440x900`
