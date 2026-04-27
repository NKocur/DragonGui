# CSS Styling System Part 2: Widget Parts Plan

## Objective

Extend DragonGUI CSS so users can style named internal parts of composite
widgets, not only the outer widget rectangle.

The first CSS implementation made app-wide styling possible through type,
class, id, direct-child, pseudo-state, variables, text inheritance, and
framework defaults. That is enough for many layout, color, typography, and
theme changes, but it is not enough for widgets whose visual structure is made
from multiple internal pieces.

Example problem:

```css
NumberInput {
    border-radius: 10px;
}
```

This rounds the outer number input. It does not give the up/down stepper region
its own corner, divider, fill, hover, or pressed styling. The result can look
wrong because the renderer still draws internal parts with hardcoded geometry
and colors.

Part 2 adds CSS-addressable widget parts so users can write:

```css
NumberInput {
    border-radius: 10px;
}

NumberInput::stepper {
    background: surface_alt;
    border-color: border;
}

NumberInput::stepper-up {
    border-top-right-radius: 10px;
}

NumberInput::stepper-down {
    border-bottom-right-radius: 10px;
}
```

## Product Goal

DragonGUI CSS should let applications change the visual language of native
widgets, including their internal controls, without requiring widget-specific
Python props or forking the renderer.

Users should be able to:

- Make number steppers square, pill-shaped, flat, or invisible.
- Style dropdown chevrons and popup rows.
- Style checkbox boxes separately from row labels.
- Style slider tracks, fills, and thumbs independently.
- Style progress bar track/fill/text.
- Style table headers, rows, selected rows, and cells.
- Style tab headers independently from tab containers.

This is what makes CSS more than a color/theme layer.

## Non-Goals For This Slice

- No arbitrary pseudo-element compatibility with browser CSS.
- No user-defined renderer parts.
- No arbitrary nested widgets for parts.
- No CSS transitions or animations.
- No per-cell table CSS selectors in the first slice.
- No layout participation for pseudo-elements as independent Taffy nodes.
- No dynamic generation of new hit-test regions from CSS.

Widget parts are styling hooks for renderer-owned pieces. They do not become
real widgets and they do not get callbacks.

## Syntax

Use DragonGUI pseudo-element style selectors:

```css
NumberInput::stepper { ... }
Dropdown::chevron { ... }
Checkbox::box { ... }
Slider::thumb { ... }
DataFrameTable::header { ... }
```

Recommended V1 syntax:

```text
WidgetSelector::part-name
```

Examples:

```css
Button.primary::label {
    color: white;
}

Panel.controls > NumberInput::stepper {
    background: #111827;
}

Dropdown:hover::chevron {
    color: accent;
}
```

Do not use browser `::part(name)` in the first implementation. The shorter
`::stepper` syntax is easier for DragonGUI users and maps directly to known
renderer parts.

Potential later compatibility alias:

```css
NumberInput::part(stepper) { ... }
```

## Selector Semantics

A part selector has two pieces:

1. A normal DragonGUI widget selector.
2. A part name.

The widget selector decides which widget the rule applies to. The part name
decides which internal renderer piece receives the declaration.

```css
Panel.rail > NumberInput::stepper-up { ... }
```

This means:

- Match `NumberInput` widgets that are direct children of `Panel.rail`.
- Store the declarations under that widget's `stepper-up` part style.
- The `NumberInput` renderer reads that part style when drawing the upper
  stepper button.

Pseudo-states may apply before the part:

```css
NumberInput:hover::stepper { ... }
NumberInput:focus::stepper { ... }
Dropdown:disabled::chevron { ... }
```

Part pseudo-states are widget-state driven in the first version. A future slice
can add part-specific hover/pressed states, such as hovering only the up
stepper.

Part 2 also promotes `:checked` for `Checkbox` styling because checkbox part
styling is incomplete without a state-specific indicator hook:

```css
Checkbox:checked::indicator {
    background: accent;
}
```

`:selected` and `:open` remain future pseudo-states.

## Style Model

Add part styles to `NodeStyle`.

Recommended shape:

```rust
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartStyle {
    pub layout: PartLayoutStyle,
    pub visual: VisualStyle,
    pub text: TextStyle,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodePartStyles {
    pub parts: BTreeMap<String, PartStyle>,
    pub hover: BTreeMap<String, PartStyle>,
    pub active: BTreeMap<String, PartStyle>,
    pub focus: BTreeMap<String, PartStyle>,
    pub disabled: BTreeMap<String, PartStyle>,
}

pub struct NodeStyle {
    ...
    pub parts: NodePartStyles,
}
```

Alternative shape:

```rust
BTreeMap<WidgetPart, PartStateStyles>
```

This is more type-safe and better long term, but the initial CSS parser already
works with string selectors and warning paths. Use a typed enum internally only
if the implementation stays clean.

## Part Layout Properties

Most part styling is visual/text only. Some parts need simple geometry knobs.
Do not expose full Taffy layout for parts.

Supported part layout fields:

| Field | Purpose |
| --- | --- |
| `width` | stepper width, chevron box width, thumb width |
| `height` | track height, thumb height, progress fill height |
| `padding` | popup row/cell text inset where applicable |
| `gap` | reserved for later composite parts |

CSS properties:

```css
NumberInput::stepper {
    width: 34px;
}

Slider::track {
    height: 8px;
}

Slider::thumb {
    width: 18px;
    height: 18px;
}
```

Part layout fields are interpreted by each renderer. They do not create Taffy
nodes.

## Per-Corner Radius

Per-corner radius support is required for this feature.

The motivating `NumberInput` case cannot be solved cleanly with a single
uniform `border-radius`. The upper stepper needs a rounded top-right corner and
a square left edge. The lower stepper needs a rounded bottom-right corner and a
square left edge.

Part 2 should add a shared corner-radius model:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CornerRadii {
    pub top_left: Option<f32>,
    pub top_right: Option<f32>,
    pub bottom_right: Option<f32>,
    pub bottom_left: Option<f32>,
}
```

Recommended integration:

- Keep `border_radius` as the existing uniform radius shortcut.
- Add optional per-corner overrides to visual style or part visual style.
- `border-radius` sets all corners.
- `border-top-left-radius`, `border-top-right-radius`,
  `border-bottom-right-radius`, and `border-bottom-left-radius` override
  individual corners.
- Teach the primitive rectangle pipeline to render per-corner radii.

This is a blocker for CSSP4. If per-corner radius is not implemented, the
NumberInput stepper examples should not ship because they imply behavior the
renderer cannot produce.

## Supported Part Properties

Start with properties already supported by `VisualStyle` and `TextStyle`:

- `background`
- `background-color`
- `foreground`
- `color`
- `border-color`
- `border-width`
- `border-radius`
- `border-top-left-radius`
- `border-top-right-radius`
- `border-bottom-right-radius`
- `border-bottom-left-radius`
- `opacity`
- `accent`
- `track-color`
- `thumb-color`
- `font-size`
- `font-family`
- `font-weight`
- `text-align`
- `width`
- `height`
- `padding`

Unsupported properties should warn through the existing stylesheet warning
system.

## Initial Part Catalog

Implement parts in priority order. Do not expose every theoretical internal
piece at once.

### P1: NumberInput

High priority because rounded outer controls make the internal up/down buttons
look bad today.

Parts:

| Part | Renderer Meaning |
| --- | --- |
| `field` | text/value field region |
| `stepper` | entire right stepper column |
| `stepper-up` | upper half of stepper |
| `stepper-down` | lower half of stepper |
| `stepper-divider` | horizontal divider between up/down |
| `divider` | vertical divider between value field and stepper |
| `caret` | text caret |

Example:

```css
NumberInput {
    border-radius: 10px;
}

NumberInput::stepper {
    width: 32px;
    background: surface_alt;
}

NumberInput::divider,
NumberInput::stepper-divider {
    background: border;
}

NumberInput::stepper-up {
    border-top-right-radius: 10px;
}

NumberInput::stepper-down {
    border-bottom-right-radius: 10px;
}
```

Implementation notes:

- `number_stepper_width(...)` should first check `NumberInput::stepper.width`.
- Stepper text color should come from `stepper-up` / `stepper-down` text color,
  then `stepper`, then widget text.
- Stepper backgrounds should use part backgrounds before widget background.
- Radius for right corners should come from part radius if provided.

### P2: Dropdown

Parts:

| Part | Renderer Meaning |
| --- | --- |
| `field` | selected value field |
| `chevron` | arrow indicator |
| `menu` | popup container |
| `item` | popup row |
| `item-selected` | selected popup row |
| `item-hover` | hovered popup row |

Example:

```css
Dropdown::chevron {
    color: accent;
    width: 22px;
}

Dropdown::menu {
    background: surface;
    border-radius: 8px;
}

Dropdown::item-hover {
    background: accent_mix_20;
}
```

Implementation notes:

- Dropdown overlay renderer must read the parent widget's part styles.
- Popup row hover should use `item-hover` before hardcoded hover fill.
- Text color for selected/hover rows should come from matching parts.

### P3: Checkbox

Parts:

| Part | Renderer Meaning |
| --- | --- |
| `row` | full clickable row highlight |
| `box` | checkbox square |
| `indicator` | checked mark/fill |
| `label` | label text |

Example:

```css
Checkbox::box {
    border-radius: 2px;
    background: surface_alt;
}

Checkbox:checked::indicator {
    background: accent;
}
```

Part 2 should add `:checked` for `Checkbox` as part of CSSP5. The pseudo-state
is widget-state driven, matching the existing hover/focus/disabled model.

### P4: Slider

Parts:

| Part | Renderer Meaning |
| --- | --- |
| `track` | full slider track |
| `fill` | filled track segment |
| `thumb` | draggable thumb |

Example:

```css
Slider::track {
    height: 6px;
    background: border;
}

Slider::fill {
    background: accent;
}

Slider::thumb {
    width: 18px;
    height: 18px;
    border-color: surface;
}
```

Implementation notes:

- Track height should come from `track.height`.
- Thumb width/height should come from `thumb.width` / `thumb.height`.
- Thumb border/fill should come from `thumb` visual style.

### P5: ProgressBar

Parts:

| Part | Renderer Meaning |
| --- | --- |
| `track` | outer/empty track |
| `fill` | progress fill |
| `label` | centered label |

Example:

```css
ProgressBar::track {
    background: surface_alt;
}

ProgressBar::fill {
    background: success;
}

ProgressBar::label {
    color: text;
    font-weight: 700;
}
```

### P6: Tabs And Navigation

Parts:

| Widget | Part | Renderer Meaning |
| --- | --- | --- |
| `Tabs` | `header` | tab strip background |
| `Tab` | `tab` | tab button body |
| `Tab` | `accent` | active tab accent |
| `NavItem` | `item` | full nav row |
| `NavItem` | `accent` | active nav accent bar |

Example:

```css
Tab::accent {
    background: accent;
    height: 3px;
}

NavItem::accent {
    width: 4px;
    border-radius: 4px;
}
```

### P7: DataFrameTable

Parts:

| Part | Renderer Meaning |
| --- | --- |
| `header` | header row |
| `row` | body row |
| `row-selected` | selected row/cell fill |
| `grid-line` | row/column separators |

Example:

```css
DataFrameTable::header {
    background: surface_alt;
    color: text;
    font-weight: 700;
}

DataFrameTable::grid-line {
    background: border;
}
```

Defer these table parts until a profile shows the extra lookups are cheap
enough or the renderer has a cached part-style handle:

- `surface`
- `header-cell`
- `row-alt`
- `cell`

Do not implement per-column/per-row selectors in this slice.

## Parser Design

Extend `DgSelector` with an optional part:

```rust
pub struct DgSelector {
    ...
    pub part: Option<DgWidgetPart>,
}
```

Recommended part representation:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum DgWidgetPart {
    Named(String),
}
```

or a typed enum:

```rust
pub enum DgWidgetPart {
    NumberField,
    NumberStepper,
    NumberStepperUp,
    NumberStepperDown,
    DropdownChevron,
    DropdownMenu,
    ...
}
```

Recommended V1 implementation:

- Parse as string.
- Store as `WidgetPartName`.
- Validate selector syntax only.
- Validate part names against the matched widget kind during cascade.

This avoids overfitting the type system before the part catalog stabilizes.

## Validation Timing

Part-name validation should happen during stylesheet application/cascade, not
while parsing the stylesheet.

The parser should validate only syntax:

```css
NumberInput::stepper { ... }
.numeric::stepper { ... }
```

Both selectors are syntactically valid and should lower into `DgSelector` with
`part = "stepper"`. During cascade, after the selector has matched an actual
`WidgetNode`, DragonGUI checks whether that widget kind supports the requested
part.

This matters for class/id selectors where the widget kind is unknown at parse
time:

```css
.numeric::stepper { ... }
```

If `.numeric` matches a `NumberInput`, the rule applies. If it matches a
`Button`, DragonGUI records a warning and ignores that part declaration for
that widget.

Warnings should be reported through the existing stylesheet/debug-snapshot
warning path and should be de-duplicated by `(rule, widget_kind, part)` so a
large tree does not produce hundreds of identical warnings.

## Cascade Semantics

Part declarations use the same cascade key:

```text
important flag,
origin precedence,
specificity,
source order
```

Specificity includes the widget selector and pseudo-state. The part name does
not add specificity beyond behaving like a pseudo-element.

Example:

```css
NumberInput::stepper { background: surface; }
.toolbar NumberInput::stepper { background: surface_alt; } /* later descendant support */
NumberInput.danger::stepper { background: danger; }
```

For current V1 selectors:

```css
Panel.toolbar > NumberInput::stepper { ... }
```

has the same specificity as `Panel.toolbar > NumberInput`.

Renderer field resolution must be explicit and consistent:

```text
part pseudo-state field
> base part field
> widget pseudo-state field
> base widget field
> inherited text field, when applicable
> theme default
```

For visual fields this means:

```text
parts.hover["stepper"].visual.background
> parts.base["stepper"].visual.background
> node.style.hover.background
> node.style.visual.background
> renderer/theme fallback
```

For text fields this means:

```text
parts.hover["stepper-up"].text.color
> parts.base["stepper-up"].text.color
> node.style.text.color
> inherited parent text color
> theme.text
```

Layout-affecting part fields such as `width` and `height` are base-state only
in the first part CSS slice. A rule such as
`NumberInput:hover::stepper { width: 40px; }` should warn and ignore the
stateful layout field. This prevents hover/focus from changing hit-test
geometry and creating layout instability.

## Inline Style API

Do not require Python users to use CSS for part styling. Add inline support in
the existing style dict:

```python
dg.NumberInput(
    42,
    style={
        "border_radius": 10,
        "parts": {
            "stepper": {"background": "surface_alt", "width": 32},
            "stepper_up": {"border_top_right_radius": 10},
            "stepper_down": {"border_bottom_right_radius": 10},
        },
    },
)
```

For consistency with CSS, accept both dashed and snake-case part names:

- `stepper-up`
- `stepper_up`

The style JSON parser should normalize both to the same internal part key.

Parser behavior:

- The flat style parser must skip the top-level `parts` key when parsing normal
  widget style fields.
- Each nested part style uses the same declaration parser as CSS lowering where
  possible.
- Invalid inline part names should produce a clear Python-side validation error
  before the document is sent to native code when feasible.
- Native parsing should still validate and warn defensively, because live style
  commands can also carry part dictionaries.

## Renderer Integration

Add helpers:

```rust
fn part_visual<'a>(
    node: &'a WidgetNode,
    state: &WidgetState,
    part: &str,
) -> Cow<'a, VisualStyle>
```

```rust
fn part_text<'a>(
    node: &'a WidgetNode,
    state: &WidgetState,
    part: &str,
) -> Cow<'a, TextStyle>
```

These helpers should:

1. Start with the part base style.
2. Merge active pseudo-state part style for hover/active/focus/disabled.
3. Fall back to the widget-level style when part fields are absent.
4. Avoid allocation on the common no-part-style path.

Renderer code should not manually inspect the part maps everywhere. Keep part
resolution centralized.

These helpers must be available to both `primitives/mod.rs` and `text/mod.rs`.
Several visible parts are text-layer elements, not primitive rectangles:

- `NumberInput::stepper-up` / `stepper-down` for `+` and `-`.
- `Dropdown::chevron` for the arrow glyph.
- `Checkbox::indicator` if the checked mark remains text-based.
- `ProgressBar::label`.
- `DataFrameTable::header` and body text.

The text renderer should use `part_text(...)` for these pieces and the
primitive renderer should use `part_visual(...)` for their backgrounds,
borders, fills, and dividers. This keeps background and text styling for a
part in the same cascade model.

## Hit Testing

Initial part styling should not change hit-test behavior except for part layout
fields that already correspond to existing hit-test geometry.

Allowed:

- `NumberInput::stepper { width: 36px; }` changes the stepper hit area because
  the stepper width already affects runtime hit testing.
- `Slider::thumb { width: 20px; }` may affect drag affordance if the existing
  hit test depends on thumb geometry.

Not allowed in this slice:

- Creating new clickable areas.
- Making only a styled subpart receive callbacks.
- Giving parts separate focus targets.

## Debug Snapshot Additions

Extend computed style snapshots:

```json
{
    "computed_styles": {
        "number-1": {
            "matched_rules": ["NumberInput", "NumberInput::stepper"],
            "style": { "...": "..." },
            "parts": {
                "stepper": {
                    "matched_rules": ["NumberInput::stepper"],
                    "visual": { "background": { "token": "surface_alt" } },
                    "layout": { "width": 32 }
                }
            }
        }
    }
}
```

Do not dump empty part styles for every widget. Only include parts with
non-empty computed styles or matched rules.

## CSS Demo Updates

Update `examples/css_design_system_demo.py` to make part styling obvious:

- Rounded number inputs whose steppers still look correct.
- Square/dense steppers in dense mode.
- Pill slider thumbs in presentation mode.
- Neon slider track/fill/thumb in mission-control mode.
- Table header/grid/selection restyling.
- Dropdown popup hover rows with strong visual difference.

Add a small focused example:

```text
examples/css_widget_parts_demo.py
```

It should contain:

- NumberInput
- Dropdown
- Checkbox
- Slider
- ProgressBar
- DataFrameTable

The demo should make part changes visually obvious without requiring a huge
window.

## Milestone Plan

### CSSP0: Design And Part Catalog

Deliverables:

- Finalize part selector syntax.
- Finalize initial part allowlist.
- Add this plan to `plans/V2`.

Acceptance:

- NumberInput part names are fixed before implementation starts.
- Unsupported part behavior is specified.

### CSSP1: Internal Part Style Model

Deliverables:

- Add `PartStyle`.
- Add part storage to `NodeStyle`.
- Add `CornerRadii` / per-corner radius support to the visual model.
- Update primitive rectangle rendering so per-corner radii can be drawn.
- Add style snapshot serialization for parts.
- Add helper methods for checking/merging part styles.

Acceptance:

- Unit tests can construct a `NodeStyle` with part styles.
- `border-top-right-radius` and related properties are represented in the
  internal style model.
- Part styles merge predictably.
- Empty part styles do not bloat debug snapshots.

### CSSP2: CSS Parser Part Selectors

Deliverables:

- Parse `Widget::part-name` selectors.
- Store `part` on `DgSelector`.
- Validate part selector syntax.
- Defer widget-kind-aware part-name validation to cascade/application time.
- Parse per-corner radius declarations.
- Preserve existing selector specificity behavior.

Acceptance:

- `NumberInput::stepper { background: red; }` parses.
- `Panel.controls > NumberInput::stepper { ... }` parses.
- `.numeric::stepper { ... }` parses even though the widget kind is unknown
  until cascade.
- Unsupported browser pseudo-elements warn clearly.

### CSSP3: Cascade Part Declarations

Deliverables:

- Validate matched part names against the matched widget kind.
- Apply matched part declarations into `node.style.parts`.
- Apply pseudo-state part declarations into part hover/active/focus/disabled
  slots.
- Add matched rule reporting for part selectors.
- De-duplicate unsupported part warnings by `(rule, widget_kind, part)`.

Acceptance:

- Widget-level CSS and part-level CSS can coexist.
- `Button::stepper { ... }` warns because `Button` has no `stepper` part.
- Inline widget style still wins over stylesheet widget style.
- Inline part style wins over stylesheet part style.
- Pseudo-state part rules are stored and used only during that state.
- Part field resolution follows:
  `part pseudo > part base > widget pseudo > widget base > inherited/theme`.

### CSSP4: NumberInput Renderer Integration

Deliverables:

- Use part styles for:
  - `field`
  - `stepper`
  - `stepper-up`
  - `stepper-down`
  - `stepper-divider`
  - `divider`
  - `caret`
- Allow `stepper.width` to affect geometry and hit testing.
- Use per-corner radii so rounded outer controls produce correct stepper
  corners without rounding the divider edge.
- Add renderer tests where possible.

Acceptance:

- Rounded `NumberInput` steppers look correct.
- Square/dense `NumberInput` steppers look correct.
- Up/down text or icons remain centered.
- Existing number input behavior remains unchanged without part CSS.

### CSSP5: Dropdown, Checkbox, Slider, ProgressBar

Deliverables:

- Add `:checked` for `Checkbox`.
- Dropdown part styles for `field`, `chevron`, `menu`, `item`,
  `item-selected`, `item-hover`.
- Checkbox part styles for `row`, `box`, `indicator`, `label`.
- Slider part styles for `track`, `fill`, `thumb`.
- ProgressBar part styles for `track`, `fill`, `label`.

Acceptance:

- Dropdown popup hover rows are CSS-controllable.
- Checkbox box radius and indicator color are CSS-controllable.
- `Checkbox:checked::indicator` works.
- Slider track height and thumb size are CSS-controllable.
- ProgressBar fill and label are CSS-controllable.

### CSSP6: Table And Navigation Parts

Deliverables:

- DataFrameTable parts:
  - `header`
  - `row`
  - `row-selected`
  - `grid-line`
- Tabs/NavItem parts:
  - `header`
  - `tab`
  - `accent`
  - `item`

Acceptance:

- Table header, body row, selected row/cell, and grid lines can be styled
  separately.
- Active tab/nav accent styling is CSS-controllable.

### CSSP7: Inline Part Styles

Deliverables:

- Parse `style={"parts": {...}}`.
- Normalize dashed and snake-case part names.
- Merge inline part styles with highest precedence.

Acceptance:

- Python inline part style works without a stylesheet.
- Inline part style overrides CSS part rules.
- Invalid inline part names produce a clear warning or validation error.

### CSSP8: Demos, Docs, And Smoke Audit

Deliverables:

- Add `examples/css_widget_parts_demo.py`.
- Update `examples/css_design_system_demo.py`.
- Update `docs/css-styling.md`.
- Extend `tools/smoke_css_demos.py` to include the widget-parts demo.
- Add layout audit coverage for part-driven geometry.

Acceptance:

- CSS demos pass with `warnings=0`.
- CSS demos pass with `layout_issues=0`.
- The widget-parts demo visibly shows more than color/rounding changes.

## Testing Strategy

### Rust Unit Tests

- Parse part selectors.
- Reject/warn unknown part selectors.
- Specificity of part selectors.
- Cascade ordering for part rules.
- Pseudo-state part rule storage.
- `:checked` matching for checkboxes.
- Per-corner radius parsing and style serialization.
- Inline part override precedence.
- Part style snapshot serialization.
- NumberInput stepper width affects geometry helper.
- Stateful part layout fields warn and are ignored.

### Python Tests

- `App.stylesheet()` serializes part CSS.
- Inline `style={"parts": ...}` serializes correctly.
- Live `set_style(parts=...)` updates retained inline style if supported.

### Smoke Tests

- `tools/smoke_css_demos.py --strict-layout`.
- New `examples/css_widget_parts_demo.py`.
- Existing CSS design-system demo.
- All-features CSS demo.

### Manual Visual Checks

- NumberInput rounded and square styles.
- Dropdown item hover styling.
- Slider track/fill/thumb styling.
- Table header/body/grid styling.
- High DPI scaling.
- Small-window dense layout.

## Risks

### Too Many Part Names

If every renderer detail becomes public CSS API, future renderer changes become
harder.

Mitigation:

- Start with a small documented part catalog.
- Treat undocumented internals as private.
- Add parts only when they map to stable user-facing concepts.

### CSS That Breaks Interaction Geometry

Changing widths/heights of internal pieces can desync drawing and hit testing.

Mitigation:

- Only expose layout fields for existing hit-test regions.
- Add tests for NumberInput stepper hit testing when `stepper.width` changes.
- Keep default behavior unchanged when no part CSS is present.

### Style Model Bloat

Part maps can make every `NodeStyle` heavier.

Mitigation:

- Use empty maps by default.
- Avoid per-widget allocations when no part styles exist.
- Do not include empty parts in debug snapshots.

### Pseudo-State Ambiguity

`NumberInput:hover::stepper-up` might be read as hovering the up button, but the
first implementation may only know the whole widget is hovered.

Mitigation:

- Document that pseudo-states are widget-level in the first part CSS slice.
- Add part-specific state later only where hit testing already tracks it.

## Success Criteria

This work succeeds when:

- Users can style internal widget parts with CSS.
- `NumberInput` steppers look correct with rounded outer controls.
- Dropdown popup rows, checkbox boxes, slider thumbs/tracks, progress fills, and
  table headers are CSS-controllable.
- Inline part styles work for users who do not want a stylesheet.
- Existing apps look unchanged without part CSS.
- Debug snapshots show applied part rules clearly.
- CSS smoke demos pass with zero warnings and zero layout issues.

## Long-Term Extensions

Possible later work:

- Browser-compatible `::part(name)` alias.
- `:selected` and `:open`.
- Part-specific hover/pressed state for steppers and dropdown rows.
- Deferred table parts: `surface`, `header-cell`, `row-alt`, and `cell`.
- Per-column table selectors.
- Per-row state selectors.
- Theme packages that define widget parts.
- Style inspector showing widget and part matches.

## Follow-On Containment Work

Widget parts make renderer-owned pieces styleable, but they do not replace a
general clipping model. The current implementation handles the common rounded
containment cases directly in renderer code:

- `Panel::accent` renders as a fill slice clipped to the panel's inner rounded
  shape.
- `DataFrameTable` clips header, rows, selection, grid lines, and border to the
  table's rounded shape.
- `Image` clips texture content to the rounded content box inside its border.
- Dropdown and menu item fills are clipped to their rounded popup bounds.

Remaining containment features should be separate V2 follow-ons:

- `overflow: hidden` for containers such as `Panel`, implemented with a real
  renderer mask/stencil rather than ad hoc child clipping.
- Scroll containers for dense panels and lists.
- Rounded `Scatter3D` clipping. Scatter currently uses rectangular viewport and
  scissor state because it is depth-buffered 3D content. Rounded clipping should
  be implemented with a stencil/mask pass or shader-level viewport mask.
- Text clipping to rounded masks. Table text currently uses corner-aware bounds
  as a practical approximation, not true curved glyph clipping.
