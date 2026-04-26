# Layout, Widgets, And Theme Plan

## Objective

Avoid making "layout + text + all widgets" one blocking milestone.

This work is split into three stages:

1. Layout and primitive drawing with no text.
2. Events and text.
3. Full basic widget behavior and theming.

## M3: Layout And Primitive Drawing, No Text

Deliverables:

- Typed Rust widget document.
- `taffy` layout for `HLayout`, `VLayout`, fixed sizes, grow, shrink, and gaps.
- Primitive renderer for rectangles, borders, rounded corners, separators, and
  simple filled shapes.
- Plot region placement for the embedded DragonSci scatter.
- Initial theme tokens wired into primitive colors.

Explicitly out of scope:

- Text rendering.
- Text input.
- Dropdown menus.
- Widget keyboard behavior.

Acceptance criteria:

- A left panel and scatter region render from the Python tree.
- The left panel can show button/checkbox/dropdown shells without labels.
- Resize recomputes layout without flicker or panic.
- Theme background, surface, accent, border, and radius affect drawing.

## M4: Events, Callbacks, Updates, And Text

Deliverables:

- Hit testing based on layout boxes.
- Mouse hover, press, release, click, and drag routing.
- Python callback registry.
- UI-thread command queue.
- `Button`, `Checkbox`, and `Slider` interactions.
- Text rendering using `cosmic-text` or `glyphon`.
- Basic label drawing and button/control labels.

Text integration checklist:

- Font discovery or bundled default font.
- Unicode shaping.
- Glyph atlas management.
- HiDPI scaling.
- Text color from theme tokens.
- Cache invalidation when text or font size changes.

Acceptance criteria:

- Button callback fires exactly once per click.
- Slider drag updates the Rust value and Python handle.
- Checkbox toggles and emits a callback.
- Labels render clearly on Windows and macOS at common scale factors.

## M5: Full Basic Widget Set And Theming

Deliverables:

- `Label`
- `Button`
- `TextInput`
- `Slider`
- `Dropdown`
- `Checkbox`
- `Panel`
- `HLayout`
- `VLayout`
- Light and dark themes.
- Theme token overrides from Python.
- Focus, hover, active, disabled, and validation states.

Theme tokens:

- `background`
- `surface`
- `surface_alt`
- `text`
- `muted_text`
- `accent`
- `border`
- `danger`
- `warning`
- `success`
- `radius`
- `spacing`
- `font_size`

Acceptance criteria:

- The scatter tool uses real controls rather than placeholders.
- Theme changes apply consistently across widgets.
- No widget depends on Python per-frame rendering.
- The widget set is enough to build a small internal data tool.
