# DragonGUI Visual Audit Report

Generated: 2026-07-24 16:16:00 Eastern Daylight Time

This is a manifest-driven visual audit. `pass` means the saved screenshot state was visually reviewed against `artifacts/SPEC.md` and no obvious defect was recorded. `needs_manual_interaction` means the static capture was reviewed, but important hover/open/drag/focus or animated states still need manual or automated interaction coverage.

## Summary

- pass: 1
- needs_manual_interaction: 0
- fail: 0
- blocked: 0

## Targets

### Layout Panel Bounds (`layout-panel-bounds`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\layout_panel_bounds_probe.py`
- Features: `Panel`, `Collapsible`, `ScrollArea`, `Nested panels`
- Screenshots: `screenshots/layout-panel-bounds-390x720@1.5x.png`
- Debug snapshots: `snapshots/layout-panel-bounds-390x720@1.5x.json`, `snapshots/layout-panel-bounds-390x720@1.5x-resize-0-start-390x720.json`, `snapshots/layout-panel-bounds-390x720@1.5x-resize-1-640x480.json`, `snapshots/layout-panel-bounds-390x720@1.5x-resize-2-1024x768.json`, `snapshots/layout-panel-bounds-390x720@1.5x-resize-3-390x720.json`
- Logs: `logs/layout-panel-bounds-390x720@1.5x.stdout.txt`, `logs/layout-panel-bounds-390x720@1.5x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target layout-panel-bounds --sizes 390x720 --scales 1.5`
