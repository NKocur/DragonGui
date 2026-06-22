# CSS Feature Probes

Small, focused DragonGUI examples for checking one CSS capability at a time.

Each file in this folder should isolate one behavior so visual regressions are
easy to spot. Prefer tiny windows, a few widgets, and obvious labels over large
showcase layouts.

## Layout Pattern

Use `probe_helpers.probe_grid()` for card grids instead of `HLayout` plus
per-card `width: calc(50% - gap)` rules. The helper builds a responsive
`GridLayout(columns=2, min_column_width=390)` so cards use two columns when
there is room and collapse to one column when the window is narrow.

```python
from probe_helpers import probe_grid

with probe_grid(gap=12):
    with dg.Panel("Case A", class_="case"):
        ...
    with dg.Panel("Case B", class_="case"):
        ...
```

Keep `Panel.case` rules focused on visual styling and minimum height. Avoid
fixed card widths and `min-width` values unless a probe is specifically testing
overflow behavior.

Suggested order:

1. `custom_properties_probe.py` - custom properties and `var()` resolution.
2. `supports_probe.py` - `@supports` feature queries.
3. `media_probe.py` - `@media` query matching and live resize updates.
4. `font_face_probe.py` - `@font-face` loading from local family and font files.
5. `gradients_probe.py` - gradients, layered backgrounds, and background noise.
6. `overflow_scrollbar_probe.py` - overflow clipping, scroll containers, and scrollbar parts.
7. `selectors_probe.py` - selector matching, selector functions, and pseudo-states.
8. `transitions_transforms_probe.py` - state transitions, timing functions, and paint-only transforms.
9. `animations_probe.py` - `@keyframes`, animation timing, fill modes, and play state.
10. `positioning_zindex_probe.py` - relative, absolute, fixed positioning, clipping, and z-index.
11. `backdrop_filter_probe.py` - first-slice backdrop-filter tinting, rounded clipping, and scroll stability.
12. `generated_content_probe.py` - `::before`, `::after`, quoted content, `attr(...)`, clipping, and reserved inline space.
13. `responsive_layout_probe.py` - percent and `calc()` sizing, auto sizing, grid tracks, named areas, and dense auto-placement.
14. `typography_probe.py` - text transform, letter spacing, line height, italic style, tabular numbers, ellipsis, and wrapping.
15. `widget_metrics_probe.py` - DragonGUI-specific TextArea rows and DataFrameTable metrics.
16. `color_syntax_probe.py` - named colors, transparent, hex alpha, rgb/hsl/hwb, Lab/LCH/Oklab/Oklch, and `color(...)`.
17. `border_outline_shadow_probe.py` - border styles, rounded outlines, outline offset, inset/outset shadows, and scroll clipping.
18. `data_widgets_probe.py` - DataFrameTable parts, table selection, Scatter3D picking, colormap differences, and point sizing.
19. `scatter3d_probe.py` - dedicated Scatter3D packet formats, point styles, live updates, picking, z-order, clipping, and camera controls.
20. `scatter3d_frame_benchmark_probe.py` - repeated 125k+ point Scatter3D frame replacement and per-frame timing.
21. `core_widgets_probe.py` - buttons, badges, text inputs, checkbox, dropdown, numeric controls, ColorPicker, Collapsible, Tooltip, Modal, and Toast.
22. `navigation_widgets_probe.py` - MenuBar/Menu/MenuItem, Sidebar/NavItem, Tabs/Tab, Pages/Page, badges, disabled states, selected states, and ContextMenu.
23. `menu_overlays_probe.py` - MenuBar open states, menu item states, context menu edge placement, overlay z-order, and scroll-container clipping.
24. `layout_containers_probe.py` - HLayout/VLayout sizing, titled Panel spacing, absolute children, nested panels, scrollable panels, Spacer, and Separator.
25. `form_controls_probe.py` - TextInput, TextArea, Checkbox, Dropdown, Button badges, Slider, NumberInput, ProgressBar, disabled states, and form-control parts.
26. `overlay_stack_probe.py` - modal, toast, tooltip, dropdown, context menu, MenuBar, scrim styling, edge clamping, and overlay theme stress.
27. `histogram_probe.py` - first-slice Histogram rendering, count/density/percent/cumulative modes, explicit bin edges, and native bar chrome.
28. `startup_loading_probe.py` - native startup loading screen display, custom copy/colors, minimum duration, and delayed startup handoff.
29. `pie_chart_probe.py` - dashboard-style PieChart donut cards with center metrics, toolbar chrome, legends, and themed slice colors.
30. `scatter3d_css_chrome_probe.py` - Scatter3D CSS grid visibility, grid planes, legend position, orientation axes, point size, and point style.
31. `container_queries_probe.py` - first-slice named `@container` width queries with `container-type: inline-size`.
32. `primitive_benchmark_probe.py` - dense primitive rect rendering benchmark with split-path diagnostics.
33. `line_plot_stream_benchmark_probe.py` - LinePlot append streaming benchmark with native, primitive, decimation, and dedicated line renderer diagnostics.
34. `toggle_switch_probe.py` - ToggleSwitch state, callbacks, disabled state, label position, and track/thumb parts.
35. `date_time_inputs_probe.py` - DateInput, TimeInput, and DateTimeInput validation, normalization, callbacks, and invalid styling.
36. `code_editor_probe.py` - CodeEditor multiline editing, fixed-width text, line numbers, gutter styling, and disabled state.
37. `log_view_probe.py` - LogView append streams, visible-line rendering, follow mode, scrolling, and severity row parts.
38. `breadcrumbs_probe.py` - Breadcrumbs path navigation, collapsed middle segments, current styling, and selection callbacks.
39. `toolbar_probe.py` - Toolbar and ToolbarSeparator grouped actions, embedded controls, vertical layout, and disabled states.
40. `loading_spinner_probe.py` - LoadingSpinner animated, paused, disabled, labeled, and CSS part states.
41. `scatter_plot_2d_probe.py` - ScatterPlot2D scalar and categorical packed point rendering with flat camera controls.
42. `scatter3d_dense_probe.py` - Dense 3D scalar volume rendering, depth behavior, orbit controls, view snaps, and packed point rendering.
43. `heatmap_probe.py` - Heatmap packed matrix rendering, labels, scalar bars, and hover readout.
44. `bar_chart_probe.py` - BarChart categorical rendering, grouped series, horizontal orientation, native toolbar, and hover readout.
45. `layout_flex_stress_probe.py` - long labels, mixed fixed/flexible controls, nested rows, wrapped chips, and constrained cards.
46. `layout_panel_bounds_probe.py` - fixed and auto-height panels, scroll bodies, nested groups, logs, and constrained tables.
47. `layout_grid_masonry_probe.py` - masonry packing, standard grid alignment, nested cards, split panes, and responsive fallback.
48. `layout_overlay_collision_probe.py` - tooltips, menus, dropdowns, modal, command palette, toasts, and drag ghost overlay collision stress.
49. `layout_scrollable_composites_probe.py` - tables, logs, code editor, tree/list widgets, property rows, command results, and nested scroll regions.
50. `layout_plot_embedding_probe.py` - line, scatter, heatmap, histogram, bar, and table widgets embedded in constrained cards and split panes.
51. `custom_composite_widget_probe.py` - V5 composite widget state/keys plus the native ExtensionWidget layout/CSS foundation.
52. `paint_widget_sparkline_probe.py` - V5 PaintWidget display-list primitives, theme-token colors, scaling, and sparkline rendering.
53. `paint_widget_events_probe.py` - V5 PaintWidget click hit-testing, callback routing, live repaint, and ordinary-widget updates.
