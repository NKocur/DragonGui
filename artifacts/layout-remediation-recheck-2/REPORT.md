# DragonGUI Visual Audit Report

Generated: 2026-07-24 15:50:35 Eastern Daylight Time

This is a manifest-driven visual audit. `pass` means the saved screenshot state was visually reviewed against `artifacts/SPEC.md` and no obvious defect was recorded. `needs_manual_interaction` means the static capture was reviewed, but important hover/open/drag/focus or animated states still need manual or automated interaction coverage.

## Summary

- pass: 1
- needs_manual_interaction: 1
- fail: 0
- blocked: 0

## Targets

### Layout Overlay Collision (`layout-overlay-collision`)

- Status: `needs_manual_interaction`
- Priority: `medium`
- Probe: `examples\css_feature_probes\layout_overlay_collision_probe.py`
- Features: `Modal`, `Tooltip`, `Dropdown`, `ContextMenu`, `CommandPalette`, `Z-index`
- Screenshots: `screenshots/layout-overlay-collision-640x480@1x.png`
- Debug snapshots: `snapshots/layout-overlay-collision-640x480@1x.json`, `snapshots/layout-overlay-collision-640x480@1x-resize-0-start-640x480.json`, `snapshots/layout-overlay-collision-640x480@1x-resize-1-640x480.json`, `snapshots/layout-overlay-collision-640x480@1x-resize-2-1024x768.json`, `snapshots/layout-overlay-collision-640x480@1x-resize-3-640x480.json`
- Logs: `logs/layout-overlay-collision-640x480@1x.stdout.txt`, `logs/layout-overlay-collision-640x480@1x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`, `native/src/text/mod.rs`
- Reproduction: `python tools/visual_audit.py --target layout-overlay-collision --sizes 640x480 --scales 1`

### Responsive Layout (`responsive-layout`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\responsive_layout_probe.py`
- Features: `Percent sizing`, `calc()`, `Grid tracks`, `Named grid areas`
- Screenshots: `screenshots/responsive-layout-640x480@1x.png`
- Debug snapshots: `snapshots/responsive-layout-640x480@1x.json`, `snapshots/responsive-layout-640x480@1x-resize-0-start-640x480.json`, `snapshots/responsive-layout-640x480@1x-resize-1-640x480.json`, `snapshots/responsive-layout-640x480@1x-resize-2-1024x768.json`, `snapshots/responsive-layout-640x480@1x-resize-3-640x480.json`
- Logs: `logs/responsive-layout-640x480@1x.stdout.txt`, `logs/responsive-layout-640x480@1x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target responsive-layout --sizes 640x480 --scales 1`
