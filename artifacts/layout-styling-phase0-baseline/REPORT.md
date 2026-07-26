# DragonGUI Visual Audit Report

Generated: 2026-07-24 18:55:59 Eastern Daylight Time

This is a manifest-driven visual audit. `pass` means the saved screenshot state was visually reviewed against `artifacts/SPEC.md` and no obvious defect was recorded. `needs_manual_interaction` means the static capture was reviewed, but important hover/open/drag/focus or animated states still need manual or automated interaction coverage.

## Summary

- pass: 4
- needs_manual_interaction: 0
- fail: 0
- blocked: 0

## Targets

### Semantic CSS Identity Baseline (`semantic-css-identity`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\semantic_css_identity_probe.py`
- Features: `SearchBox`, `Toolbar`, `AppShell`, `WorkbenchLayout`, `public CSS type selectors`
- Screenshots: `screenshots/semantic-css-identity-desktop-1@1x.png`
- Debug snapshots: `snapshots/semantic-css-identity-desktop-1@1x.json`
- Logs: `logs/semantic-css-identity-desktop-1@1x.stdout.txt`, `logs/semantic-css-identity-desktop-1@1x.stderr.txt`
- Notes: Phase 0 baseline for public composite type selectors versus native render kinds. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target semantic-css-identity --sizes desktop-1 --scales 1`

### Cascade Origin Baseline (`cascade-origin`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\cascade_origin_probe.py`
- Features: `widget defaults`, `application stylesheet`, `author inline style`, `computed style`
- Screenshots: `screenshots/cascade-origin-desktop-1@1x.png`
- Debug snapshots: `snapshots/cascade-origin-desktop-1@1x.json`
- Logs: `logs/cascade-origin-desktop-1@1x.stdout.txt`, `logs/cascade-origin-desktop-1@1x.stderr.txt`
- Notes: Phase 0 baseline for separating widget defaults from authored inline styles. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target cascade-origin --sizes desktop-1 --scales 1`

### Sidebar Flex Allocation Baseline (`sidebar-flex-allocation`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\sidebar_flex_allocation_probe.py`
- Features: `Sidebar`, `Panel`, `AppShell`, `WorkbenchLayout`, `starved subtree`
- Screenshots: `screenshots/sidebar-flex-allocation-desktop-1@1x.png`
- Debug snapshots: `snapshots/sidebar-flex-allocation-desktop-1@1x.json`, `snapshots/sidebar-flex-allocation-desktop-1@1x-resize-0-start-900x680.json`, `snapshots/sidebar-flex-allocation-desktop-1@1x-resize-1-390x720.json`, `snapshots/sidebar-flex-allocation-desktop-1@1x-resize-2-900x680.json`, `snapshots/sidebar-flex-allocation-desktop-1@1x-resize-3-900x680.json`
- Logs: `logs/sidebar-flex-allocation-desktop-1@1x.stdout.txt`, `logs/sidebar-flex-allocation-desktop-1@1x.stderr.txt`
- Notes: Phase 0 baseline for content-sized sidebar children and zero-width workbench detection. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target sidebar-flex-allocation --sizes desktop-1 --scales 1`

### Responsive Grid Orphan Baseline (`responsive-grid-orphan`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\responsive_grid_orphan_probe.py`
- Features: `GridLayout`, `min_column_width`, `responsive columns`, `orphan balancing`
- Screenshots: `screenshots/responsive-grid-orphan-desktop-1@1x.png`
- Debug snapshots: `snapshots/responsive-grid-orphan-desktop-1@1x.json`, `snapshots/responsive-grid-orphan-desktop-1@1x-resize-0-start-1024x640.json`, `snapshots/responsive-grid-orphan-desktop-1@1x-resize-1-640x480.json`, `snapshots/responsive-grid-orphan-desktop-1@1x-resize-2-1024x768.json`, `snapshots/responsive-grid-orphan-desktop-1@1x-resize-3-1024x640.json`
- Logs: `logs/responsive-grid-orphan-desktop-1@1x.stdout.txt`, `logs/responsive-grid-orphan-desktop-1@1x.stderr.txt`
- Notes: Phase 0 baseline for explicit responsive columns and optional final-row balancing. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target responsive-grid-orphan --sizes desktop-1 --scales 1`
