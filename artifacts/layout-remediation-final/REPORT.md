# DragonGUI Visual Audit Report

Generated: 2026-07-24 15:41:20 Eastern Daylight Time

This is a manifest-driven visual audit. `pass` means the saved screenshot state was visually reviewed against `artifacts/SPEC.md` and no obvious defect was recorded. `needs_manual_interaction` means the static capture was reviewed, but important hover/open/drag/focus or animated states still need manual or automated interaction coverage.

## Summary

- pass: 3
- needs_manual_interaction: 0
- fail: 5
- blocked: 0

## Targets

### Layout Flex Stress (`layout-flex-stress`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\layout_flex_stress_probe.py`
- Features: `HLayout`, `VLayout`, `FlowLayout`, `Fixed and flexible children`, `Long labels`
- Screenshots: `screenshots/layout-flex-stress-desktop-1@1x.png`, `screenshots/layout-flex-stress-desktop-2@1.5x.png`
- Debug snapshots: `snapshots/layout-flex-stress-desktop-1@1x.json`, `snapshots/layout-flex-stress-desktop-1@1x-resize-0-start-820x680.json`, `snapshots/layout-flex-stress-desktop-1@1x-resize-1-640x480.json`, `snapshots/layout-flex-stress-desktop-1@1x-resize-2-1024x768.json`, `snapshots/layout-flex-stress-desktop-1@1x-resize-3-820x680.json`, `snapshots/layout-flex-stress-desktop-2@1.5x.json`, `snapshots/layout-flex-stress-desktop-2@1.5x-resize-0-start-820x680.json`, `snapshots/layout-flex-stress-desktop-2@1.5x-resize-1-640x480.json`, `snapshots/layout-flex-stress-desktop-2@1.5x-resize-2-1024x768.json`, `snapshots/layout-flex-stress-desktop-2@1.5x-resize-3-820x680.json`
- Logs: `logs/layout-flex-stress-desktop-1@1x.stdout.txt`, `logs/layout-flex-stress-desktop-1@1x.stderr.txt`, `logs/layout-flex-stress-desktop-2@1.5x.stdout.txt`, `logs/layout-flex-stress-desktop-2@1.5x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target layout-flex-stress --sizes desktop-1 --scales 1`; `python tools/visual_audit.py --target layout-flex-stress --sizes desktop-2 --scales 1.5`

### Layout Panel Bounds (`layout-panel-bounds`)

- Status: `fail`
- Priority: `high`
- Probe: `examples\css_feature_probes\layout_panel_bounds_probe.py`
- Features: `Panel`, `Collapsible`, `ScrollArea`, `Nested panels`
- Screenshots: `screenshots/layout-panel-bounds-desktop-1@1x.png`, `screenshots/layout-panel-bounds-desktop-2@1.5x.png`
- Debug snapshots: `snapshots/layout-panel-bounds-desktop-1@1x.json`, `snapshots/layout-panel-bounds-desktop-1@1x-resize-0-start-940x720.json`, `snapshots/layout-panel-bounds-desktop-1@1x-resize-1-640x480.json`, `snapshots/layout-panel-bounds-desktop-1@1x-resize-2-1024x768.json`, `snapshots/layout-panel-bounds-desktop-1@1x-resize-3-940x720.json`, `snapshots/layout-panel-bounds-desktop-2@1.5x.json`, `snapshots/layout-panel-bounds-desktop-2@1.5x-resize-0-start-940x720.json`, `snapshots/layout-panel-bounds-desktop-2@1.5x-resize-1-640x480.json`, `snapshots/layout-panel-bounds-desktop-2@1.5x-resize-2-1024x768.json`, `snapshots/layout-panel-bounds-desktop-2@1.5x-resize-3-940x720.json`
- Logs: `logs/layout-panel-bounds-desktop-1@1x.stdout.txt`, `logs/layout-panel-bounds-desktop-1@1x.stderr.txt`, `logs/layout-panel-bounds-desktop-2@1.5x.stdout.txt`, `logs/layout-panel-bounds-desktop-2@1.5x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API. Additional run: Layout relational check failed: layout-panel-bounds-desktop-1@1x.json: fixed panel dg-99 clip escapes its bounds; layout-panel-bounds-desktop-1@1x-resize-0-start-940x720.json: fixed panel dg-99 clip escapes its bounds; layout-panel-bounds-desktop-1@1x-resize-1-640x480.json: fixed panel dg-74 clip escapes its bounds; layout-panel-bounds-desktop-1@1x-resize-1-640x480.json: fixed panel dg-92 clip escapes its bounds Additional run: Captured with native DragonGUI window screenshot API. Additional run: Layout relational check failed: layout-panel-bounds-desktop-2@1.5x.json: fixed panel dg-99 clip escapes its bounds; layout-panel-bounds-desktop-2@1.5x-resize-0-start-940x720.json: fixed panel dg-99 clip escapes its bounds; layout-panel-bounds-desktop-2@1.5x-resize-1-640x480.json: fixed panel dg-74 clip escapes its bounds; layout-panel-bounds-desktop-2@1.5x-resize-1-640x480.json: fixed panel dg-92 clip escapes its bounds
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target layout-panel-bounds --sizes desktop-1 --scales 1`; `python tools/visual_audit.py --target layout-panel-bounds --sizes desktop-2 --scales 1.5`

### Layout Grid And Masonry (`layout-grid-masonry`)

- Status: `fail`
- Priority: `high`
- Probe: `examples\css_feature_probes\layout_grid_masonry_probe.py`
- Features: `GridLayout`, `CSS grid`, `Responsive collapse`, `Nested grids`
- Screenshots: `screenshots/layout-grid-masonry-desktop-1@1x.png`, `screenshots/layout-grid-masonry-desktop-2@1.5x.png`
- Debug snapshots: `snapshots/layout-grid-masonry-desktop-1@1x.json`, `snapshots/layout-grid-masonry-desktop-1@1x-resize-0-start-1020x760.json`, `snapshots/layout-grid-masonry-desktop-1@1x-resize-1-640x480.json`, `snapshots/layout-grid-masonry-desktop-1@1x-resize-2-1024x768.json`, `snapshots/layout-grid-masonry-desktop-1@1x-resize-3-1020x760.json`, `snapshots/layout-grid-masonry-desktop-2@1.5x.json`, `snapshots/layout-grid-masonry-desktop-2@1.5x-resize-0-start-1020x760.json`, `snapshots/layout-grid-masonry-desktop-2@1.5x-resize-1-640x480.json`, `snapshots/layout-grid-masonry-desktop-2@1.5x-resize-2-1024x768.json`, `snapshots/layout-grid-masonry-desktop-2@1.5x-resize-3-1020x760.json`
- Logs: `logs/layout-grid-masonry-desktop-1@1x.stdout.txt`, `logs/layout-grid-masonry-desktop-1@1x.stderr.txt`, `logs/layout-grid-masonry-desktop-2@1.5x.stdout.txt`, `logs/layout-grid-masonry-desktop-2@1.5x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API. Additional run: Layout relational check failed: layout-grid-masonry-desktop-1@1x-resize-1-640x480.json: .card-grid children dg-85 and dg-112 overlap; layout-grid-masonry-desktop-1@1x-resize-1-640x480.json: .card-grid children dg-94 and dg-124 overlap; layout-grid-masonry-desktop-1@1x-resize-1-640x480.json: .card-grid children dg-112 and dg-139 overlap; layout-grid-masonry-desktop-1@1x-resize-1-640x480.json: .card-grid children dg-124 and dg-148 overlap Additional run: Captured with native DragonGUI window screenshot API. Additional run: Layout relational check failed: layout-grid-masonry-desktop-2@1.5x-resize-1-640x480.json: .card-grid children dg-85 and dg-112 overlap; layout-grid-masonry-desktop-2@1.5x-resize-1-640x480.json: .card-grid children dg-94 and dg-124 overlap; layout-grid-masonry-desktop-2@1.5x-resize-1-640x480.json: .card-grid children dg-112 and dg-139 overlap; layout-grid-masonry-desktop-2@1.5x-resize-1-640x480.json: .card-grid children dg-124 and dg-148 overlap
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target layout-grid-masonry --sizes desktop-1 --scales 1`; `python tools/visual_audit.py --target layout-grid-masonry --sizes desktop-2 --scales 1.5`

### Layout Overlay Collision (`layout-overlay-collision`)

- Status: `fail`
- Priority: `high`
- Probe: `examples\css_feature_probes\layout_overlay_collision_probe.py`
- Features: `Modal`, `Tooltip`, `Dropdown`, `ContextMenu`, `CommandPalette`, `Z-index`
- Screenshots: `screenshots/layout-overlay-collision-desktop-1@1x.png`, `screenshots/layout-overlay-collision-desktop-2@1.5x.png`
- Debug snapshots: `snapshots/layout-overlay-collision-desktop-1@1x.json`, `snapshots/layout-overlay-collision-desktop-1@1x-resize-0-start-980x740.json`, `snapshots/layout-overlay-collision-desktop-1@1x-resize-1-640x480.json`, `snapshots/layout-overlay-collision-desktop-1@1x-resize-2-1024x768.json`, `snapshots/layout-overlay-collision-desktop-1@1x-resize-3-980x740.json`, `snapshots/layout-overlay-collision-desktop-2@1.5x.json`, `snapshots/layout-overlay-collision-desktop-2@1.5x-resize-0-start-980x740.json`, `snapshots/layout-overlay-collision-desktop-2@1.5x-resize-1-640x480.json`, `snapshots/layout-overlay-collision-desktop-2@1.5x-resize-2-1024x768.json`, `snapshots/layout-overlay-collision-desktop-2@1.5x-resize-3-980x740.json`
- Logs: `logs/layout-overlay-collision-desktop-1@1x.stdout.txt`, `logs/layout-overlay-collision-desktop-1@1x.stderr.txt`, `logs/layout-overlay-collision-desktop-2@1.5x.stdout.txt`, `logs/layout-overlay-collision-desktop-2@1.5x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API. Additional run: Layout relational check failed: layout-overlay-collision-desktop-1@1x.json: scroll_max_x owner dg-16 has no rect; layout-overlay-collision-desktop-1@1x.json: scroll_max_x owner dg-84 has no rect; layout-overlay-collision-desktop-1@1x.json: scroll_max_y owner dg-16 has no rect; layout-overlay-collision-desktop-1@1x.json: scroll_max_y owner dg-84 has no rect Additional run: Captured with native DragonGUI window screenshot API. Additional run: Layout relational check failed: layout-overlay-collision-desktop-2@1.5x.json: scroll_max_x owner dg-16 has no rect; layout-overlay-collision-desktop-2@1.5x.json: scroll_max_x owner dg-84 has no rect; layout-overlay-collision-desktop-2@1.5x.json: scroll_max_y owner dg-16 has no rect; layout-overlay-collision-desktop-2@1.5x.json: scroll_max_y owner dg-84 has no rect
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`, `native/src/text/mod.rs`
- Reproduction: `python tools/visual_audit.py --target layout-overlay-collision --sizes desktop-1 --scales 1`; `python tools/visual_audit.py --target layout-overlay-collision --sizes desktop-2 --scales 1.5`

### Layout Scrollable Composites (`layout-scrollable-composites`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\layout_scrollable_composites_probe.py`
- Features: `ScrollArea`, `DataFrameTable`, `LogView`, `CodeEditor`, `TreeView`, `PropertyGrid`
- Screenshots: `screenshots/layout-scrollable-composites-desktop-1@1x.png`, `screenshots/layout-scrollable-composites-desktop-2@1.5x.png`
- Debug snapshots: `snapshots/layout-scrollable-composites-desktop-1@1x.json`, `snapshots/layout-scrollable-composites-desktop-1@1x-resize-0-start-980x740.json`, `snapshots/layout-scrollable-composites-desktop-1@1x-resize-1-640x480.json`, `snapshots/layout-scrollable-composites-desktop-1@1x-resize-2-1024x768.json`, `snapshots/layout-scrollable-composites-desktop-1@1x-resize-3-980x740.json`, `snapshots/layout-scrollable-composites-desktop-2@1.5x.json`, `snapshots/layout-scrollable-composites-desktop-2@1.5x-resize-0-start-980x740.json`, `snapshots/layout-scrollable-composites-desktop-2@1.5x-resize-1-640x480.json`, `snapshots/layout-scrollable-composites-desktop-2@1.5x-resize-2-1024x768.json`, `snapshots/layout-scrollable-composites-desktop-2@1.5x-resize-3-980x740.json`
- Logs: `logs/layout-scrollable-composites-desktop-1@1x.stdout.txt`, `logs/layout-scrollable-composites-desktop-1@1x.stderr.txt`, `logs/layout-scrollable-composites-desktop-2@1.5x.stdout.txt`, `logs/layout-scrollable-composites-desktop-2@1.5x.stderr.txt`
- Notes: No visual issue recorded by automated first pass.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`, `native/src/text/mod.rs`
- Reproduction: `python tools/visual_audit.py --target layout-scrollable-composites --sizes desktop-1 --scales 1`; `python tools/visual_audit.py --target layout-scrollable-composites --sizes desktop-2 --scales 1.5`

### Layout Plot Embedding (`layout-plot-embedding`)

- Status: `pass`
- Priority: `low`
- Probe: `examples\css_feature_probes\layout_plot_embedding_probe.py`
- Features: `LinePlot`, `ScatterPlot2D`, `Heatmap`, `Histogram`, `BarChart`, `DataFrameTable`, `Splitter`
- Screenshots: `screenshots/layout-plot-embedding-desktop-1@1x.png`, `screenshots/layout-plot-embedding-desktop-2@1.5x.png`
- Debug snapshots: `snapshots/layout-plot-embedding-desktop-1@1x.json`, `snapshots/layout-plot-embedding-desktop-1@1x-resize-0-start-1120x780.json`, `snapshots/layout-plot-embedding-desktop-1@1x-resize-1-640x480.json`, `snapshots/layout-plot-embedding-desktop-1@1x-resize-2-1024x768.json`, `snapshots/layout-plot-embedding-desktop-1@1x-resize-3-1120x780.json`, `snapshots/layout-plot-embedding-desktop-2@1.5x.json`, `snapshots/layout-plot-embedding-desktop-2@1.5x-resize-0-start-1120x780.json`, `snapshots/layout-plot-embedding-desktop-2@1.5x-resize-1-640x480.json`, `snapshots/layout-plot-embedding-desktop-2@1.5x-resize-2-1024x768.json`, `snapshots/layout-plot-embedding-desktop-2@1.5x-resize-3-1120x780.json`
- Logs: `logs/layout-plot-embedding-desktop-1@1x.stdout.txt`, `logs/layout-plot-embedding-desktop-1@1x.stderr.txt`, `logs/layout-plot-embedding-desktop-2@1.5x.stdout.txt`, `logs/layout-plot-embedding-desktop-2@1.5x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API.
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`, `native/src/scatter/mod.rs`, `native/src/table.rs`
- Reproduction: `python tools/visual_audit.py --target layout-plot-embedding --sizes desktop-1 --scales 1`; `python tools/visual_audit.py --target layout-plot-embedding --sizes desktop-2 --scales 1.5`

### Overflow And Scrollbars (`overflow-scrollbar`)

- Status: `fail`
- Priority: `high`
- Probe: `examples\css_feature_probes\overflow_scrollbar_probe.py`
- Features: `ScrollArea`, `Overflow clipping`, `Scrollbar parts`
- Screenshots: `screenshots/overflow-scrollbar-desktop-1@1x.png`, `screenshots/overflow-scrollbar-desktop-2@1.5x.png`
- Debug snapshots: `snapshots/overflow-scrollbar-desktop-1@1x.json`, `snapshots/overflow-scrollbar-desktop-1@1x-resize-0-start-780x700.json`, `snapshots/overflow-scrollbar-desktop-1@1x-resize-1-640x480.json`, `snapshots/overflow-scrollbar-desktop-1@1x-resize-2-1024x768.json`, `snapshots/overflow-scrollbar-desktop-1@1x-resize-3-780x700.json`, `snapshots/overflow-scrollbar-desktop-2@1.5x.json`, `snapshots/overflow-scrollbar-desktop-2@1.5x-resize-0-start-780x700.json`, `snapshots/overflow-scrollbar-desktop-2@1.5x-resize-1-640x480.json`, `snapshots/overflow-scrollbar-desktop-2@1.5x-resize-2-1024x768.json`, `snapshots/overflow-scrollbar-desktop-2@1.5x-resize-3-780x700.json`
- Logs: `logs/overflow-scrollbar-desktop-1@1x.stdout.txt`, `logs/overflow-scrollbar-desktop-1@1x.stderr.txt`, `logs/overflow-scrollbar-desktop-2@1.5x.stdout.txt`, `logs/overflow-scrollbar-desktop-2@1.5x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Captured with native DragonGUI window screenshot API. Additional run: Layout relational check failed: overflow-scrollbar-desktop-1@1x.json: native unreachable-root-overflow: unreachable-root-overflow-x: dg-1 window clips dg-22 label with no scroll owner; overflow-scrollbar-desktop-1@1x-resize-0-start-780x700.json: native unreachable-root-overflow: unreachable-root-overflow-x: dg-1 window clips dg-22 label with no scroll owner; overflow-scrollbar-desktop-1@1x-resize-1-640x480.json: native unreachable-root-overflow: unreachable-root-overflow-x: dg-1 window clips dg-22 label with no scroll owner; overflow-scrollbar-desktop-1@1x-resize-1-640x480.json: native unreachable-root-overflow: unreachable-root-overflow-y: dg-1 window clips dg-25 h_layout with no scroll owner Additional run: Captured with native DragonGUI window screenshot API. Additional run: Layout relational check failed: overflow-scrollbar-desktop-2@1.5x.json: native unreachable-root-overflow: unreachable-root-overflow-x: dg-1 window clips dg-22 label with no scroll owner; overflow-scrollbar-desktop-2@1.5x-resize-0-start-780x700.json: native unreachable-root-overflow: unreachable-root-overflow-x: dg-1 window clips dg-22 label with no scroll owner; overflow-scrollbar-desktop-2@1.5x-resize-1-640x480.json: native unreachable-root-overflow: unreachable-root-overflow-x: dg-1 window clips dg-22 label with no scroll owner; overflow-scrollbar-desktop-2@1.5x-resize-1-640x480.json: native unreachable-root-overflow: unreachable-root-overflow-y: dg-1 window clips dg-25 h_layout with no scroll owner
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target overflow-scrollbar --sizes desktop-1 --scales 1`; `python tools/visual_audit.py --target overflow-scrollbar --sizes desktop-2 --scales 1.5`

### Responsive Layout (`responsive-layout`)

- Status: `fail`
- Priority: `high`
- Probe: `examples\css_feature_probes\responsive_layout_probe.py`
- Features: `Percent sizing`, `calc()`, `Grid tracks`, `Named grid areas`
- Screenshots: _none_
- Debug snapshots: _none_
- Logs: `logs/responsive-layout-desktop-1@1x.stdout.txt`, `logs/responsive-layout-desktop-1@1x.stderr.txt`, `logs/responsive-layout-desktop-2@1.5x.stdout.txt`, `logs/responsive-layout-desktop-2@1.5x.stderr.txt`
- Notes: No visual issue recorded by automated first pass. Additional run: Probe exited before a window was detected with code 1; see logs. Additional run: Layout relational check failed: responsive-layout-desktop-1@1x.json: could not read layout snapshot: [Errno 2] No such file or directory: 'C:\\Users\\nkocur\\Desktop\\Projects\\Python\\Full Dragon\\DragonGui\\artifacts\\layout-remediation-final\\snapshots\\responsive-layout-desktop-1@1x.json'; responsive-layout-desktop-1@1x.json: could not read layout snapshot: [Errno 2] No such file or directory: 'C:\\Users\\nkocur\\Desktop\\Projects\\Python\\Full Dragon\\DragonGui\\artifacts\\layout-remediation-final\\snapshots\\responsive-layout-desktop-1@1x.json' Additional run: Probe exited before a window was detected with code 1; see logs. Additional run: Layout relational check failed: responsive-layout-desktop-2@1.5x.json: could not read layout snapshot: [Errno 2] No such file or directory: 'C:\\Users\\nkocur\\Desktop\\Projects\\Python\\Full Dragon\\DragonGui\\artifacts\\layout-remediation-final\\snapshots\\responsive-layout-desktop-2@1.5x.json'; responsive-layout-desktop-2@1.5x.json: could not read layout snapshot: [Errno 2] No such file or directory: 'C:\\Users\\nkocur\\Desktop\\Projects\\Python\\Full Dragon\\DragonGui\\artifacts\\layout-remediation-final\\snapshots\\responsive-layout-desktop-2@1.5x.json'
- Suspected modules: `native/src/layout.rs`, `native/src/primitives/mod.rs`, `native/src/runtime.rs`
- Reproduction: `python tools/visual_audit.py --target responsive-layout --sizes desktop-1 --scales 1`; `python tools/visual_audit.py --target responsive-layout --sizes desktop-2 --scales 1.5`
