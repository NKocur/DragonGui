# CSS Styling System Plan

## Objective

Add a CSS-like styling system to DragonGUI so applications can look and feel
substantially different without requiring per-widget inline style dictionaries
everywhere.

The goal is not to become a browser. The goal is to let users style DragonGUI
with a familiar CSS subset that maps cleanly onto the systems DragonGUI already
has:

- `WidgetNode` tree metadata.
- `class_`, `key`, and explicit widget ids.
- `NodeStyle`.
- Taffy layout.
- Theme tokens.
- Runtime pseudo-state such as hover, active, focus, and disabled.
- Primitive/text/table/scatter renderers.

The first implementation should compile CSS into DragonGUI's existing computed
style model. It should not replace the current `style={...}` API; inline styles
remain supported and should keep the highest precedence.

## Product Goal

DragonGUI apps should be able to move from "all apps look mostly like the
default theme" to "this app has its own design system" through a stylesheet:

```css
:root {
    --accent: #ff6b35;
    --surface: #f5f0e8;
    --border: #c4b8a0;
    --radius: 2px;
}

Panel.controls {
    width: 340px;
    padding: 14px;
    gap: 10px;
    background: var(--surface);
    border-color: var(--border);
    border-radius: var(--radius);
}

Button {
    border-radius: var(--radius);
    font-weight: 600;
}

Button:hover {
    background: accent_mix_20;
}

Button.danger {
    background: danger;
    color: white;
}
```

Python usage:

```python
app = dg.App()
app.stylesheet("""
Button {
    border-radius: 4px;
}

.plot {
    flex: 1;
}
""")
```

or:

```python
app.load_stylesheet("styles/app.dg.css")
```

## Non-Goals For V1

- No browser DOM.
- No HTML parser.
- No JavaScript.
- No full CSS cascade compatibility promise.
- No media queries in the first version.
- No animations/transitions in the first version.
- No CSS grid beyond what DragonGUI explicitly maps into Taffy later.
- No arbitrary browser CSS property support.
- No `calc()` requirement in the first version.
- No stylesheet hot reload until the static API is stable.
- No custom layout engine beyond Taffy.

DragonGUI CSS is a styling language for DragonGUI widgets, not general web CSS.

## Dependency Choice

Use `lightningcss` for parsing.

Reasons:

- Rust native.
- Browser-grade parser.
- Built on the CSS parsing infrastructure used by Firefox/Servo-adjacent
  projects.
- Parses stylesheets into structured rules/properties instead of leaving
  everything as raw strings.
- Avoids building and maintaining a custom CSS parser.

The hard part is still DragonGUI-specific:

- Matching selectors against `WidgetNode`.
- Applying pseudo-state from `WidgetState`.
- Resolving cascade precedence.
- Mapping CSS declarations into `NodeStyle`.
- Caching computed styles.
- Invalidation when stylesheets/theme/runtime state change.

## Licensing Policy

`lightningcss` is MPL-2.0. MPL-2.0 is file-level copyleft, not project-level
copyleft. DragonGUI can use it as an unmodified Cargo dependency while keeping
DragonGUI's own code MIT.

Policy:

- Do not vendor or modify `lightningcss` source unless absolutely necessary.
- If DragonGUI ever modifies MPL-licensed files, those modified files stay
  MPL-2.0.
- Add third-party license notices to distributed wheels/sdists.
- Add a release task using `cargo-about` or an equivalent tool to generate a
  dependency license notice file.

Deliverable before implementing CSS parser integration:

- Add `THIRD_PARTY_NOTICES.md` or `LICENSES/`.
- Ensure wheels include third-party notices.
- Add a release/check task or documented command for dependency license notices.

## Architecture

The CSS system should sit between document parsing and layout/rendering:

```text
Python widgets / components
    -> WidgetNode tree with id, key, class_, kind, inline style
    -> StylesheetStore
    -> selector matching
    -> cascade resolver
    -> computed NodeStyle per widget
    -> Taffy layout + renderers
```

The current direct `NodeStyle::from_json(...)` path becomes one input into the
computed style resolver, not the only style source.

All `lightningcss` types must be isolated behind one DragonGUI module. The rest
of the runtime should consume DragonGUI-owned types only:

```rust
enum DgStyleDeclaration {
    Layout(DgLayoutDeclaration),
    Visual(DgVisualDeclaration),
    Text(DgTextDeclaration),
    Widget(DgWidgetDeclaration),
    CustomProperty { name: String, value: DgCssValue },
}

struct DgStyleRule {
    selector: DgSelector,
    declarations: Vec<DgStyleDeclaration>,
    specificity: Specificity,
    origin: StylesheetOrigin,
    source_order: u32,
}
```

`lightningcss` parses CSS into its own AST. DragonGUI immediately lowers that
AST into `DgStyleRule` and `DgStyleDeclaration`. Selector matching, cascade,
computed style, tests, and renderers should not depend directly on
`lightningcss` AST types. If the dependency API changes, only the lowering
module should break.

## Style Origins

Use explicit origins, lowest to highest precedence:

1. Framework defaults.
2. Theme stylesheet / theme token defaults.
3. User stylesheet.
4. Inline `style={...}` props.
5. Runtime pseudo-state rules at the same origin specificity level.

The exact cascade key should be:

```text
important flag,
origin precedence,
specificity,
source order
```

Inline styles can be represented as an origin with higher precedence than user
stylesheet rules.

`:root` custom properties are processed before normal rule declaration
resolution:

1. Parse the full stylesheet.
2. Collect all `:root` custom property declarations by origin/source order.
3. Build the global variable map.
4. Resolve `var(--name)` references while lowering other declarations into
   `DgStyleDeclaration`.
5. Match selectors and apply cascade.

Scoped custom properties are not part of V1.

## Supported Selector Subset

Start with a deliberately small subset.

### Type Selectors

```css
Button { ... }
Panel { ... }
DataFrameTable { ... }
Scatter3D { ... }
```

Type names map to `WidgetKind` display names, not Python class internals.

Initial type names:

- `Window`
- `HLayout`
- `VLayout`
- `Panel`
- `Sidebar`
- `StatusBar`
- `Tabs`
- `Tab`
- `Pages`
- `Page`
- `NavItem`
- `Label`
- `Button`
- `TextInput`
- `Slider`
- `Dropdown`
- `Checkbox`
- `Separator`
- `Spacer`
- `Scatter3D`
- `DataFrameTable`

### Class Selectors

```css
.controls { ... }
Panel.controls { ... }
```

Maps to Python `class_="controls"`.

V1 should support one or more whitespace-separated classes:

```python
dg.Panel(class_="controls primary")
```

This is the normal CSS model and the implementation cost is low. The existing
`class_` argument remains a string, but selector matching splits it on
whitespace.

### ID Selectors

```css
#main-scatter { ... }
```

Maps to explicit widget `id`, not `key`.

This means autogenerated ids technically work but should not be recommended for
CSS. For stylesheet targeting, users should pass explicit ids or classes.

### Key Selectors

Do not overload CSS `#id` for DragonGUI `key`.

If key targeting is needed later, use an attribute-like selector:

```css
[key="main-scatter"] { ... }
```

Defer this until the basic selector subset works.

### Child Combinator

```css
Sidebar > NavItem { ... }
.toolbar > Button { ... }
```

Support direct parent-child matching.

Do not support general descendant matching initially:

```css
Panel Button { ... } /* defer */
```

The child combinator is easier to implement, easier to reason about, and avoids
unexpected broad matches in nested data-tool layouts.

The selector matching input should still carry an ancestor chain, not only a
single parent id, so later descendant selectors do not require a data model
rewrite:

```rust
struct StyleElement<'a> {
    id: &'a str,
    key: Option<&'a str>,
    classes: &'a [&'a str],
    kind: WidgetKind,
    ancestors: &'a [StyleAncestor<'a>],
    pseudo: PseudoState,
}
```

CSS V1 only uses the nearest ancestor for `parent > child`, but the full chain
is available for future selectors.

### Pseudo-States

```css
Button:hover { ... }
Button:active { ... }
Button:focus { ... }
Button:disabled { ... }
```

Supported pseudo-states:

- `:hover`
- `:active`
- `:focus`
- `:disabled`

Potential later pseudo-states:

- `:checked`
- `:selected`
- `:open`

### Root

```css
:root {
    --accent: #4a90d9;
}
```

`:root` applies to application/theme token definition, not a rendered widget.

## Supported Properties

Only support properties that DragonGUI can map into existing layout/rendering
behavior.

### Layout Properties

CSS names should map to existing style keys:

| CSS | DragonGUI Style |
| --- | --- |
| `display` | `display` |
| `flex-direction` | `flex_direction` |
| `flex` | `flex` |
| `flex-grow` | `flex_grow` |
| `flex-shrink` | `flex_shrink` |
| `width` | `width` |
| `height` | `height` |
| `min-width` | `min_width` |
| `min-height` | `min_height` |
| `max-width` | `max_width` |
| `max-height` | `max_height` |
| `padding` | padding shorthand |
| `padding-left` | `padding_left` |
| `padding-right` | `padding_right` |
| `padding-top` | `padding_top` |
| `padding-bottom` | `padding_bottom` |
| `margin` | margin shorthand |
| `gap` | `gap` |

Supported units for V1:

- unitless numbers as logical pixels
- `px`
- percentages only where Taffy already handles them safely

### Visual Properties

| CSS | DragonGUI Style |
| --- | --- |
| `background` | `background` |
| `background-color` | `background` |
| `foreground` | `foreground` |
| `color` | text `color` / foreground |
| `border-color` | `border_color` |
| `border-width` | `border_width` |
| `border-radius` | `border_radius` |
| `opacity` | `opacity` |
| `accent` | `accent` |
| `track-color` | `track_color` |
| `thumb-color` | `thumb_color` |

V1 should explicitly support the common border shorthand:

```css
Button {
    border: 1px solid border;
}
```

Supported shorthand form:

```text
border: <width> solid <color>
```

Only `solid` is supported. Other border styles should produce a clear warning
and be ignored rather than silently doing nothing.

### Text Properties

| CSS | DragonGUI Style |
| --- | --- |
| `font-size` | `font_size` |
| `font-family` | `font_family` |
| `font-weight` | `font_weight` |
| `text-align` | `text_align` |

Text properties are the only inherited properties in V1.

### Widget-Specific Properties

| CSS | DragonGUI Style |
| --- | --- |
| `table-row-height` | `table_row_height` |
| `table-header-height` | `table_header_height` |

Use DragonGUI-specific dashed property names for custom widget behavior. Do not
pretend these are standard browser CSS properties.

## Theme Tokens And CSS Variables

DragonGUI already has theme tokens:

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
- `focus`
- `disabled`
- `radius`
- `spacing`
- `font_size`

CSS should support both DragonGUI token names and CSS custom properties:

```css
Button {
    background: surface_alt;
    border-color: border;
}

:root {
    --button-radius: 6px;
}

Button {
    border-radius: var(--button-radius);
}
```

V1 variable behavior:

- `:root` custom properties are global.
- No scoped custom properties in the first version.
- `var(--name)` resolves from global variables.
- Unknown variables produce a clear warning and fall back to a visible safe
  value.

## Inline Styles

Existing inline styles remain supported:

```python
dg.Button("Run", style={"background": "accent"})
```

Inline styles should:

- Override stylesheet rules.
- Continue to support nested pseudo-state maps.
- Be represented internally as a high-precedence style origin.

This preserves all current apps and examples.

## Computed Style Model

Add computed style as a separate concept from parsed node style:

```rust
struct ComputedStyle {
    layout: LayoutStyle,
    visual: VisualStyle,
    text: TextStyle,
    widget: WidgetSpecificStyle,
}
```

Existing `NodeStyle` can either become `ComputedStyle` or remain the parsed
inline style while a new computed field is introduced.

Recommended path:

1. Keep `WidgetNode.style` as parsed inline style for compatibility.
2. Add a computed style map in runtime/layout:

```rust
HashMap<WidgetId, NodeStyle>
```

3. Teach layout/renderers to read computed style first.
4. Fall back to `node.style` until the migration is complete.

This avoids a large immediate rewrite of every renderer.

## Selector Matching Design

Create a DragonGUI selector input model:

```rust
struct StyleElement<'a> {
    id: &'a str,
    key: Option<&'a str>,
    classes: &'a [&'a str],
    kind: WidgetKind,
    ancestors: &'a [StyleAncestor<'a>],
    pseudo: PseudoState,
}
```

Pseudo state comes from `WidgetState`:

- `hovered`
- `pressed`
- `focused`
- disabled state from node props/state
- later selected/open/checked

For each widget:

1. Build parent chain metadata.
2. Match stylesheet selectors.
3. Collect matching declarations.
4. Sort/fold by cascade key.
5. Apply inherited text values.
6. Apply inline style last.
7. Store computed style.

## Caching And Invalidation

Computed styles should be cached.

Dirty causes:

- Stylesheet changed: recompute all computed styles.
- Theme changed: recompute all computed styles.
- Widget tree changed: recompute for affected subtree; V1 may recompute all.
- Class/id/key changed: recompute affected subtree; V1 may recompute all.
- Hover/active/focus changed: recompute only old/new affected widgets; V1 may
  recompute visuals for all.
- Inline style changed: recompute that widget; if layout/text inherited values
  changed, recompute subtree.

V1 can over-invalidate. Correctness matters more than perfect granularity.

Recommended dirty classes:

- `Style`
- `Layout`
- `Text`
- `Visual`
- `Full`

Or map stylesheet invalidation into the existing dirty flags conservatively:

- layout-affecting property: `Layout`
- text-affecting property: `Text`
- visual-only property: `Visual`
- unknown: `Full`

## Python API

Add to `App`:

```python
app.stylesheet(css: str) -> None
app.load_stylesheet(path: str | Path) -> None
app.clear_stylesheets() -> None
```

`origin` is an internal cascade concept, not a public V1 argument. User-loaded
stylesheets always use the user origin. Framework and theme origins are
reserved for DragonGUI internals.

For live apps:

- `app.stylesheet(...)` enqueues a native stylesheet update.
- `app.load_stylesheet(...)` reads the file in Python and enqueues the CSS
  string.
- Native parser errors raise in synchronous calls before enqueue when possible,
  or return a clear error from the command bridge.

For non-running apps:

- Stylesheets are stored on the `App` and included in the startup document.

Document format addition:

```json
{
    "stylesheets": [
        {
            "origin": "user",
            "source": "Button { border-radius: 4px; }"
        }
    ]
}
```

## Native Commands

Add commands:

```rust
Command::SetStylesheet {
    origin: StylesheetOrigin,
    css: String,
}

Command::ClearStylesheets {
    origin: StylesheetOrigin,
}
```

Optional later:

```rust
Command::SetFrameworkStylesheet { css: String }
Command::SetThemeStylesheet { css: String }
```

## Framework Defaults

Do not externalize framework defaults first. That is a later milestone.

Initial CSS should layer on top of current defaults. Once selector/cascade works
and examples prove the behavior, move hardcoded default widget styles into a
lowest-precedence built-in stylesheet.

This is the point where users can fully reshape the visual identity:

```css
Button {
    border-radius: 0;
}

Panel {
    border-radius: 0;
    border-color: transparent;
}
```

## Milestone Plan

### CSS0: Planning And License Gate

Status: this document.

Deliverables:

- Define CSS subset.
- Define dependency and licensing policy.
- Define cascade order.
- Define API shape.
- Add third-party notice policy.
- Add a release/check task or documented command for dependency license notices.

Acceptance:

- CSS implementation cannot be considered shippable until dependency notices
  are included in wheel/sdist packaging.

### CSS1: Internal Declaration Model

Goal: define DragonGUI's CSS-independent style IR before parser integration.

Deliverables:

- Add `DgStyleDeclaration`.
- Add `DgStyleRule`.
- Add `DgSelector`.
- Add `DgCssValue`.
- Add `StylesheetOrigin`.
- Add `Specificity`.
- Add property support matrix in code comments/tests.

Acceptance:

- Unit tests can construct declarations without `lightningcss`.
- Supported properties are represented explicitly.
- Unsupported properties have an explicit warning/error path.
- Cascade tests can run against hand-built `DgStyleRule` values.

### CSS2: Parser And Stylesheet Store

Goal: parse and store CSS without affecting rendering.

Deliverables:

- Add `lightningcss` dependency.
- Add `style_sheet.rs` or `css_style.rs`.
- Add `StylesheetStore`.
- Parse CSS string into internal `DgStyleRule` / `DgStyleDeclaration` records.
- Store source order.
- Store origin.
- Add parser error reporting.
- Collect `:root` custom properties before lowering normal declarations.

Acceptance:

- Unit test parses valid DragonGUI CSS.
- Unit test rejects invalid CSS with useful error.
- Unit test proves `:root` variables resolve before normal rules.
- No rendering behavior changes yet.

### CSS3: Declaration Mapping

Goal: map parsed CSS declarations into DragonGUI style patches.

Deliverables:

- Convert supported CSS property names into `NodeStyle` fields.
- Support colors, theme token identifiers, numbers, `px`, and simple strings.
- Support root custom properties.
- Ignore unsupported properties with debug warnings.

Acceptance:

- `Button { border-radius: 4px; }` maps to visual radius.
- `Panel { padding: 14px; gap: 10px; }` maps to layout style.
- `Button { border: 1px solid border; }` maps to border width and color.
- `Button { border: 1px solid border; }` resolves `border` as a theme token
  inside the shorthand path, not only in longhand `border-color`.
- `:root { --accent: #ff6b35; }` resolves for later declarations.
- Unsupported property warnings include selector/property/source position.

### CSS4: Basic Selector Matching And Minimal Python API

Goal: match type, class, id, and direct child selectors.

Deliverables:

- Add widget tree style metadata.
- Match type selectors.
- Match one or more classes from whitespace-separated `class_`.
- Match explicit id selectors.
- Match `parent > child`.
- Use ancestor chain data even though V1 only supports direct child matching.
- Compute selector specificity.
- Add minimal `App.stylesheet(css)` for startup documents only.

Acceptance:

- `Button` matches all buttons.
- `.controls` matches widgets with `class_="controls"`.
- `.primary` matches widgets with `class_="controls primary"`.
- `Panel.controls > Button` matches only direct child buttons.
- Specificity tests prove id beats class beats type.
- A Python example can include a stylesheet in the startup document.

### CSS5: Cascade Resolver

Goal: produce computed styles for each widget.

Deliverables:

- Collect matching rules per widget.
- Sort/fold declarations by cascade key.
- Apply user stylesheets over defaults.
- Apply inline styles last.
- Add computed style map.
- Teach layout/renderers to use computed style where available.

Acceptance:

- User stylesheet changes button colors globally.
- Inline `style={...}` overrides stylesheet.
- Later rules with same specificity override earlier rules.
- Layout-affecting CSS triggers layout rebuild.
- Specificity regression tests cover:
  - id beats class
  - class beats type
  - same-specificity last rule wins
  - inline style beats stylesheet

### CSS6: Pseudo-State Selectors

Goal: support interactive CSS selectors.

Deliverables:

- Match `:hover`, `:active`, `:focus`, `:disabled`.
- Integrate with `WidgetState`.
- Recompute affected styles on hover/focus/active changes.
- Map pseudo-state effects into visual/text/layout dirty flags.

Acceptance:

- `Button:hover { background: accent_mix_20; }` works.
- `TextInput:focus { border-color: focus; }` works.
- `Button:disabled { opacity: 0.5; }` works.
- Hover changes do not force unnecessary full layout rebuilds.

### CSS7: Text Inheritance

Goal: make typography work like users expect.

Deliverables:

- Inherit text properties down the tree.
- Supported inherited properties:
  - `color`
  - `font-size`
  - `font-family`
  - `font-weight`
  - `text-align`
- Do not inherit layout properties.

Acceptance:

- `Panel { color: muted_text; }` affects labels/buttons inside unless
  overridden.
- `Button { color: text; }` can override inherited panel color.
- Changing parent text style invalidates descendant text.

### CSS8: Full Python API

Goal: expose stylesheet loading to users.

Deliverables:

- `App.load_stylesheet(path)`.
- `App.clear_stylesheets()`.
- Include startup stylesheets in app document.
- Add live stylesheet update commands.

Acceptance:

- CSS can be loaded before `app.run`.
- CSS can be updated while app is running.
- Parser errors are surfaced clearly.
- Existing inline-style examples still work.

### CSS9: Demo And Visual Regression Set

Goal: prove the system is useful.

Deliverables:

- Add `examples/css_showcase.py`.
- Add at least three visual themes:
  - default dark override
  - compact light data-tool theme
  - square high-contrast theme
- Add snapshot/debug output showing computed style for a selected widget.

Acceptance:

- Same widget tree can look materially different with only CSS changes.
- Each theme changes at least:
  - `border-radius`
  - app/panel background
  - button background or border color
- `debug_snapshot()` for a representative button confirms the expected computed
  style for each theme.
- Buttons, panels, tabs, nav items, inputs, table, and scatter container all
  reflect stylesheet changes.

### CSS10: Framework Defaults As CSS

Goal: make the whole default design system overridable.

Deliverables:

- Create built-in `framework.dg.css`.
- Move current widget baseline styling into framework stylesheet where
  practical.
- Keep Rust constants only for non-style geometry or fallback safety.

Acceptance:

- `Button { border-radius: 0; }` fully overrides default button radius.
- `Panel { background: transparent; }` fully overrides default panel fill.
- Existing examples look the same before and after externalization when no user
  stylesheet is loaded.
- If the built-in framework stylesheet fails to parse or load, DragonGUI falls
  back to hardcoded Rust constants instead of crashing or rendering a blank UI.

## Testing Strategy

### Unit Tests

- Parse valid CSS.
- Reject invalid CSS.
- Map declarations into style fields.
- Resolve theme tokens and variables.
- Compute selector specificity.
- Match selectors against widget trees.
- Cascade source order correctly.
- Inline style precedence.
- Selector specificity regression matrix.
- Pseudo-state matching.
- Text inheritance.

### Integration Tests

- Build an app with stylesheet before `run`.
- Send live stylesheet update command.
- Verify debug snapshot includes stylesheets and computed styles.
- Verify layout changes from CSS affect Taffy rects.
- Verify visual-only changes avoid full layout rebuild where possible.

### Manual Visual Tests

- `examples/css_showcase.py`.
- `examples/all_features_demo.py` with a stylesheet loaded.
- Narrow window tests for layout/clipping.
- Light and dark theme variants.

## Debug Snapshot Additions

Extend `debug_snapshot()` with:

```json
{
    "stylesheets": {
        "framework_rules": 42,
        "theme_rules": 12,
        "user_rules": 18,
        "last_error": null
    },
    "computed_styles": {
        "button-1": {
            "matched_rules": ["Button", ".toolbar > Button"],
            "layout_dirty": false,
            "visual_dirty": true
        }
    }
}
```

Do not dump huge stylesheet ASTs by default.

## Risks

### Scope Creep Into Browser CSS

CSS is huge. Users will expect browser behavior if the feature is described too
loosely.

Mitigation:

- Call it "DragonGUI CSS subset".
- Document supported selectors and properties.
- Warn on unsupported properties.

### Style Invalidation Bugs

Incorrect dirty flags can produce stale layout or stale visuals.

Mitigation:

- Start conservative.
- Prefer over-invalidation to stale UI.
- Add debug snapshot dirty reasons.

### Selector Matching Performance

Naive matching across every rule and every widget is probably fine for hundreds
of widgets, but could become expensive for large trees.

Mitigation:

- Start simple.
- Cache computed styles.
- Add indexing by type/class/id later if profiles show need.

### Lightning CSS API Churn

The crate may have API changes, especially around alpha versions.

Mitigation:

- Pin an exact compatible version.
- Wrap all usage in one DragonGUI module.
- Keep internal rule representation independent from the external AST.

### Licensing Notices

Dependency license notices can be forgotten during packaging.

Mitigation:

- Add release CI check.
- Generate notices automatically.

## Success Criteria

The CSS effort succeeds when:

- A user can load a stylesheet from Python.
- Type, class, id, direct-child, and pseudo-state selectors work.
- CSS maps into DragonGUI layout, visual, and text style fields.
- Inline styles still work and override stylesheets.
- Theme tokens and root variables work.
- Text style inheritance works.
- Computed styles are cached and inspectable.
- The all-features demo can look materially different with only CSS changes.
- Release artifacts include third-party license notices.

## Long-Term Extensions

Possible future work:

- Descendant selectors.
- Attribute selectors for `key`.
- `:checked`, `:selected`, `:open`.
- Scoped variables.
- Media queries for window size.
- CSS transitions for color/opacity.
- Hot reload file watcher.
- Developer style inspector.
- Theme packages.
- CSS modules or local scoping for reusable component libraries.
