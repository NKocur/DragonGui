# V4 Layout Torture Suite

Status: started. Added `layout_flex_stress_probe.py`,
`layout_panel_bounds_probe.py`, `layout_grid_masonry_probe.py`, and
`layout_overlay_collision_probe.py`, `layout_scrollable_composites_probe.py`,
and `layout_plot_embedding_probe.py`.

## Objective

Build a set of standalone stress probes that expose layout, clipping, overflow,
and overlay problems before they show up in the all-features demo.

The goal is framework hardening, not nicer demo-specific tuning. The probes
should intentionally combine constrained widths, long text, mixed widget sizes,
nested containers, scroll regions, overlays, and plots so weak layout behavior
is easy to reproduce.

## Motivation

Recent V4 work exposed repeated framework-level layout failures:

- Widgets and badges running off the right edge of panels.
- Composite rows overlapping or requiring hand-tuned widths.
- Property-grid and split-panel content needing too much manual sizing.
- Two-column card layouts wasting vertical space when card heights differ.
- Tooltips hiding text behind them instead of drawing as true overlays.
- Plots and tables behaving poorly inside constrained panels.

These are signals that the framework needs stronger default layout behavior and
better test coverage for real app compositions.

## Probe Suite

### 1. Flex Shrink/Grow Stress

Proposed file: `examples/css_feature_probes/layout_flex_stress_probe.py`

- Long labels next to inputs, buttons, badges, and icon buttons.
- Nested horizontal and vertical rows.
- Narrow panels and resized windows.
- Widgets with `flex_grow`, `flex_shrink`, `min_width`, and default sizing.

Acceptance:

- Text and controls stay inside their panel.
- Rows do not overlap.
- Important controls remain usable at narrow widths.
- Sensible behavior requires minimal per-widget style overrides.

### 2. Panel Content Bounds

Proposed file: `examples/css_feature_probes/layout_panel_bounds_probe.py`

- Mixed controls inside panels: inputs, selectors, badges, progress, logs, and
  short tables.
- Fixed-height and auto-height panels.
- Panel padding, title bands, scrollbars, and nested groups.

Acceptance:

- Panel content respects padding and available width.
- Children do not bleed through rounded borders.
- Scrollbars appear when content is too tall.
- Panel title/header space does not collide with content.

### 3. Grid And Masonry Stress

Proposed file: `examples/css_feature_probes/layout_grid_masonry_probe.py`

- Two-column cards with different heights.
- Masonry and non-masonry grid modes.
- Nested cards, split panes, property grids, and toolbar rows.
- Window widths that force one-column fallback.

Acceptance:

- Masonry avoids unnecessary vertical gaps when enabled.
- Standard grids align predictably when masonry is disabled.
- Column widths stay stable and responsive.
- Cards do not overlap during resize.

### 4. Overlay Collision Stress

Proposed file: `examples/css_feature_probes/layout_overlay_collision_probe.py`

- Static tooltips, rich tooltips, dropdowns, menu popups, command palette,
  modal, toast, and drag ghost over dense text.
- Targets near each edge of the window.
- Overlays above scroll areas and tables.

Acceptance:

- Overlays draw above content without deleting underlying text.
- Overlay placement clamps to the window.
- Rich tooltip children render and clip correctly.
- Modal and command palette chrome does not overlap body content.

### 5. Scrollable Composites

Proposed file: `examples/css_feature_probes/layout_scrollable_composites_probe.py`

- Tables, log view, code editor, tree view, selectable lists, property grid, and
  command results inside fixed-height panels.
- Nested scroll areas where possible.
- Horizontal overflow cases for tables and long code lines.

Acceptance:

- Scrollbars are visible and usable.
- Text stays anchored while scrolling.
- Headers/gutters do not drift.
- Widgets remain clipped to their owning scroll region.

### 6. Plot Embedding Stress

Proposed file: `examples/css_feature_probes/layout_plot_embedding_probe.py`

- Line plot, 2D scatter, 3D scatter, heatmap, bar chart, histogram, and table in
  panels and grids.
- Small, medium, and wide plot containers.
- Auto-fit buttons and hover readouts in constrained layouts.

Acceptance:

- Plots start with valid bounds and visible data.
- Hover readouts overlay cleanly.
- Axis labels do not overlap unrelated text.
- Plot controls remain usable in cards and split panes.

## Framework Targets

Likely fixes discovered by these probes should land in the framework, not the
probe styling, when they affect normal app layout:

- Better default `min_width: 0` behavior for flex/grid children that should
  shrink inside panels.
- More predictable panel content width and scroll clipping.
- First-class masonry grid behavior for uneven card columns.
- Consistent overlay layering for text and primitives.
- More robust default sizing for composite widgets.
- Plot widgets that produce useful initial bounds in constrained containers.

## Execution Order

1. Build `layout_flex_stress_probe.py`. Done.
2. Fix the first clear flex/panel overflow issue found there. Next.
3. Build `layout_panel_bounds_probe.py`. Done.
4. Fix the first clear panel bounds issue found there. Done.
5. Add grid/masonry stress once panel bounds are stable. Done.
6. Fix the first clear grid/masonry issue found there. Done.
7. Add overlay collision stress. Done.
8. Add scrollable composites. Done.
9. Add plot embedding stress. Done.
10. Fold stable examples back into the all-features V3/V4 tab only after the
   framework behavior holds up in standalone probes.

## Acceptance

This plan is complete when:

- All six probes exist and can be smoke-run.
- The most obvious layout failures are fixed at framework level.
- The V4 tab can use the same defaults without broad demo-specific sizing hacks.
- New V4 widgets have a clear stress-probe path before being added to the large
  demo.
