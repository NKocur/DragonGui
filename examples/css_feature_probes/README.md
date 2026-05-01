# CSS Feature Probes

Small, focused DragonGUI examples for checking one CSS capability at a time.

Each file in this folder should isolate one behavior so visual regressions are
easy to spot. Prefer tiny windows, a few widgets, and obvious labels over large
showcase layouts.

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
19. `core_widgets_probe.py` - buttons, badges, text inputs, checkbox, dropdown, numeric controls, ColorPicker, Collapsible, Tooltip, Modal, and Toast.
20. `navigation_widgets_probe.py` - MenuBar/Menu/MenuItem, Sidebar/NavItem, Tabs/Tab, Pages/Page, badges, disabled states, selected states, and ContextMenu.
21. `menu_overlays_probe.py` - MenuBar open states, menu item states, context menu edge placement, overlay z-order, and scroll-container clipping.
22. `layout_containers_probe.py` - HLayout/VLayout sizing, titled Panel spacing, absolute children, nested panels, scrollable panels, Spacer, and Separator.
23. `form_controls_probe.py` - TextInput, TextArea, Checkbox, Dropdown, Button badges, Slider, NumberInput, ProgressBar, disabled states, and form-control parts.
24. `overlay_stack_probe.py` - modal, toast, tooltip, dropdown, context menu, MenuBar, scrim styling, edge clamping, and overlay theme stress.
