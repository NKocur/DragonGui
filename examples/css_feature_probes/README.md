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
