# DragonGUI Visual Audit Report

Generated: 2026-07-24 15:39:37 Eastern Daylight Time

This is a manifest-driven visual audit. `pass` means the saved screenshot state was visually reviewed against `artifacts/SPEC.md` and no obvious defect was recorded. `needs_manual_interaction` means the static capture was reviewed, but important hover/open/drag/focus or animated states still need manual or automated interaction coverage.

## Summary

- pass: 1
- needs_manual_interaction: 0
- fail: 0
- blocked: 0

## Targets

### Layout Flex Stress (`layout-flex-stress`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\layout_flex_stress_probe.py`
- Features: `HLayout`, `VLayout`, `FlowLayout`, `Fixed and flexible children`, `Long labels`
- Screenshots: `screenshots/layout-flex-stress-640x480@1x.png`
- Debug snapshots: `snapshots/layout-flex-stress-640x480@1x.json`, `snapshots/layout-flex-stress-640x480@1x-resize-0-start-640x480.json`, `snapshots/layout-flex-stress-640x480@1x-resize-1-640x480.json`, `snapshots/layout-flex-stress-640x480@1x-resize-2-1024x768.json`, `snapshots/layout-flex-stress-640x480@1x-resize-3-640x480.json`
- Logs: `logs/layout-flex-stress-640x480@1x.stdout.txt`, `logs/layout-flex-stress-640x480@1x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target layout-flex-stress --sizes 640x480 --scales 1`
