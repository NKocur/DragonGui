# DragonGUI Visual Audit Report

Generated: 2026-07-24 15:49:49 Eastern Daylight Time

This is a manifest-driven visual audit. `pass` means the saved screenshot state was visually reviewed against `artifacts/SPEC.md` and no obvious defect was recorded. `needs_manual_interaction` means the static capture was reviewed, but important hover/open/drag/focus or animated states still need manual or automated interaction coverage.

## Summary

- pass: 3
- needs_manual_interaction: 0
- fail: 2
- blocked: 0

## Targets

### Layout Panel Bounds (`layout-panel-bounds`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\layout_panel_bounds_probe.py`
- Features: `Panel`, `Collapsible`, `ScrollArea`, `Nested panels`
- Screenshots: `screenshots/layout-panel-bounds-640x480@1x.png`
- Debug snapshots: `snapshots/layout-panel-bounds-640x480@1x.json`, `snapshots/layout-panel-bounds-640x480@1x-resize-0-start-640x480.json`, `snapshots/layout-panel-bounds-640x480@1x-resize-1-640x480.json`, `snapshots/layout-panel-bounds-640x480@1x-resize-2-1024x768.json`, `snapshots/layout-panel-bounds-640x480@1x-resize-3-640x480.json`
- Logs: `logs/layout-panel-bounds-640x480@1x.stdout.txt`, `logs/layout-panel-bounds-640x480@1x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target layout-panel-bounds --sizes 640x480 --scales 1`

### Layout Grid And Masonry (`layout-grid-masonry`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\layout_grid_masonry_probe.py`
- Features: `GridLayout`, `CSS grid`, `Responsive collapse`, `Nested grids`
- Screenshots: `screenshots/layout-grid-masonry-640x480@1x.png`
- Debug snapshots: `snapshots/layout-grid-masonry-640x480@1x.json`, `snapshots/layout-grid-masonry-640x480@1x-resize-0-start-640x480.json`, `snapshots/layout-grid-masonry-640x480@1x-resize-1-640x480.json`, `snapshots/layout-grid-masonry-640x480@1x-resize-2-1024x768.json`, `snapshots/layout-grid-masonry-640x480@1x-resize-3-640x480.json`
- Logs: `logs/layout-grid-masonry-640x480@1x.stdout.txt`, `logs/layout-grid-masonry-640x480@1x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target layout-grid-masonry --sizes 640x480 --scales 1`

### Layout Overlay Collision (`layout-overlay-collision`)

- Status: `fail`
- Priority: `high`
- Probe: `examples\css_feature_probes\layout_overlay_collision_probe.py`
- Features: `Modal`, `Tooltip`, `Dropdown`, `ContextMenu`, `CommandPalette`, `Z-index`
- Screenshots: `screenshots/layout-overlay-collision-640x480@1x.png`
- Debug snapshots: `snapshots/layout-overlay-collision-640x480@1x.json`, `snapshots/layout-overlay-collision-640x480@1x-resize-0-start-640x480.json`, `snapshots/layout-overlay-collision-640x480@1x-resize-1-640x480.json`, `snapshots/layout-overlay-collision-640x480@1x-resize-2-1024x768.json`, `snapshots/layout-overlay-collision-640x480@1x-resize-3-640x480.json`
- Logs: `logs/layout-overlay-collision-640x480@1x.stdout.txt`, `logs/layout-overlay-collision-640x480@1x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API. Additional run: Layout relational check failed: layout-overlay-collision-640x480@1x.json: overlay dg-48 escapes the root; layout-overlay-collision-640x480@1x-resize-0-start-640x480.json: overlay dg-48 escapes the root; layout-overlay-collision-640x480@1x-resize-1-640x480.json: overlay dg-48 escapes the root; layout-overlay-collision-640x480@1x-resize-3-640x480.json: overlay dg-48 escapes the root
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`, `native/src/text/mod.rs`
- Reproduction: `python tools/visual_audit.py --target layout-overlay-collision --sizes 640x480 --scales 1`

### Overflow And Scrollbars (`overflow-scrollbar`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\overflow_scrollbar_probe.py`
- Features: `ScrollArea`, `Overflow clipping`, `Scrollbar parts`
- Screenshots: `screenshots/overflow-scrollbar-640x480@1x.png`
- Debug snapshots: `snapshots/overflow-scrollbar-640x480@1x.json`, `snapshots/overflow-scrollbar-640x480@1x-resize-0-start-640x480.json`, `snapshots/overflow-scrollbar-640x480@1x-resize-1-640x480.json`, `snapshots/overflow-scrollbar-640x480@1x-resize-2-1024x768.json`, `snapshots/overflow-scrollbar-640x480@1x-resize-3-640x480.json`
- Logs: `logs/overflow-scrollbar-640x480@1x.stdout.txt`, `logs/overflow-scrollbar-640x480@1x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target overflow-scrollbar --sizes 640x480 --scales 1`

### Responsive Layout (`responsive-layout`)

- Status: `fail`
- Priority: `high`
- Probe: `examples\css_feature_probes\responsive_layout_probe.py`
- Features: `Percent sizing`, `calc()`, `Grid tracks`, `Named grid areas`
- Screenshots: `screenshots/responsive-layout-640x480@1x.png`
- Debug snapshots: `snapshots/responsive-layout-640x480@1x.json`, `snapshots/responsive-layout-640x480@1x-resize-0-start-640x480.json`, `snapshots/responsive-layout-640x480@1x-resize-1-640x480.json`, `snapshots/responsive-layout-640x480@1x-resize-2-1024x768.json`, `snapshots/responsive-layout-640x480@1x-resize-3-640x480.json`
- Logs: `logs/responsive-layout-640x480@1x.stdout.txt`, `logs/responsive-layout-640x480@1x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API. Additional run: Layout relational check failed: responsive-layout-640x480@1x.json: native unreachable-root-overflow: unreachable-root-overflow-y: dg-1 window clips dg-2 v_layout with no scroll owner; responsive-layout-640x480@1x-resize-0-start-640x480.json: native unreachable-root-overflow: unreachable-root-overflow-y: dg-1 window clips dg-2 v_layout with no scroll owner; responsive-layout-640x480@1x-resize-1-640x480.json: native unreachable-root-overflow: unreachable-root-overflow-y: dg-1 window clips dg-2 v_layout with no scroll owner; responsive-layout-640x480@1x-resize-3-640x480.json: native unreachable-root-overflow: unreachable-root-overflow-y: dg-1 window clips dg-2 v_layout with no scroll owner
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target responsive-layout --sizes 640x480 --scales 1`
