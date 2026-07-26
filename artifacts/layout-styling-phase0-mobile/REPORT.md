# DragonGUI Visual Audit Report

Generated: 2026-07-24 18:56:12 Eastern Daylight Time

This is a manifest-driven visual audit. `pass` means the saved screenshot state was visually reviewed against `artifacts/SPEC.md` and no obvious defect was recorded. `needs_manual_interaction` means the static capture was reviewed, but important hover/open/drag/focus or animated states still need manual or automated interaction coverage.

## Summary

- pass: 1
- needs_manual_interaction: 0
- fail: 0
- blocked: 0

## Targets

### Sidebar Flex Allocation Baseline (`sidebar-flex-allocation`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\sidebar_flex_allocation_probe.py`
- Features: `Sidebar`, `Panel`, `AppShell`, `WorkbenchLayout`, `starved subtree`
- Screenshots: `screenshots/sidebar-flex-allocation-390x720@1x.png`
- Debug snapshots: `snapshots/sidebar-flex-allocation-390x720@1x.json`, `snapshots/sidebar-flex-allocation-390x720@1x-resize-0-start-390x720.json`, `snapshots/sidebar-flex-allocation-390x720@1x-resize-1-390x720.json`, `snapshots/sidebar-flex-allocation-390x720@1x-resize-2-900x680.json`, `snapshots/sidebar-flex-allocation-390x720@1x-resize-3-390x720.json`
- Logs: `logs/sidebar-flex-allocation-390x720@1x.stdout.txt`, `logs/sidebar-flex-allocation-390x720@1x.stderr.txt`
- Notes: Phase 0 baseline for content-sized sidebar children and zero-width workbench detection. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target sidebar-flex-allocation --sizes 390x720 --scales 1`
