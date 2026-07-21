# DragonGUI Visual Audit Report

Generated: 2026-07-12 23:36:07 Eastern Daylight Time

This is a manifest-driven visual audit. `pass` means the saved screenshot state was visually reviewed against `artifacts/SPEC.md` and no obvious defect was recorded. `needs_manual_interaction` means the static capture was reviewed, but important hover/open/drag/focus or animated states still need manual or automated interaction coverage.

## Summary

- pass: 0
- needs_manual_interaction: 0
- fail: 1
- blocked: 0

## Targets

### Adjacent Scatter Interaction Isolation (`adjacent-scatter-interaction`)

- Status: `fail`
- Priority: `high`
- Probe: `examples\css_feature_probes\adjacent_scatter_interaction_probe.py`
- Features: `Scatter3D`, `ScatterPlot2D`, `hit testing`, `mouse interaction`, `debug_snapshot`
- Screenshots: `screenshots/adjacent-scatter-interaction-1120x720.png`
- Debug snapshots: `snapshots/adjacent-scatter-interaction-1120x720.json`
- Logs: `logs/adjacent-scatter-interaction-1120x720.stdout.txt`, `logs/adjacent-scatter-interaction-1120x720.stderr.txt`
- Notes: Automated two-plot interaction probe for adjacent scatter state isolation. Additional run: Probe exited with code 1; native screenshot was still written. Additional run: Adjacent scatter interaction check failed: adjacent-scatter-interaction-1120x720.stdout.txt: missing adjacent scatter interaction pass marker Additional run: Automated two-plot interaction probe for adjacent scatter state isolation. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`, `native/src/scatter/mod.rs`, `native/src/table.rs`
- Reproduction: `python tools/visual_audit.py --target adjacent-scatter-interaction --sizes 1120x720`
