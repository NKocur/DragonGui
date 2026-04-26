# Reactive Native Engine Proposal

## Objective

DragonGUI should become a Python application toolkit with frontend flexibility
approaching HTML/CSS, while keeping Rust on every hot path: layout, text,
input, rendering, data uploads, and custom GPU widgets.

The target is not a browser, a webview, or a full CSS engine. The target is a
native retained renderer inspired by Flutter, React Native Fabric, GPUI, Xilem,
and Iced:

```text
Python keyed components and callbacks
    -> typed virtual tree and shallow diff
    -> Rust command queue
    -> retained Rust widget/render tree
    -> style resolution and Taffy layout
    -> prepaint hitboxes and scene items
    -> wgpu render passes and DragonSci custom GPU widgets
```

Python must never participate in the frame loop. Python runs when state changes,
callbacks fire, or background work schedules an update. Rust owns the event
loop, focus, hover/active state, text input, scrolling, layout, render
invalidation, GPU buffers, and presentation.

## Product Position

This effort should support the long-term product claim:

> Python ergonomics, web-like layout/styling flexibility, and native Rust/GPU
> performance for data tools.

The moat is still DragonSci-class native data widgets. A `Scatter3D` widget
should be a first-class GPU scene node that receives a layout rect, clip rect,
input events, and controlled render-pass access. This is the part a webview
architecture cannot reproduce cleanly.

## Non-Goals For V1

- No real HTML parser.
- No browser DOM.
- No webview frontend.
- No JavaScript runtime.
- No full CSS selector engine.
- No stylesheet cascade in the first version.
- No React-style positional hooks in the first version.
- No Python per-frame rendering.
- No full visual inspector in the first version.

These can be revisited later, but they should not block the first architecture
slice.

## Current Repo Assessment

DragonGUI already has several pieces of this architecture:

- Python widget objects serialize to a typed document.
- Rust parses a `WidgetNode` tree.
- Rust owns the `winit` event loop.
- Rust uses `wgpu` for rendering.
- Rust uses `taffy` for layout.
- Rust owns mutable widget state through `WidgetState`.
- Rust has separate primitive, text, scatter, and table renderers.

The missing foundation is live mutation after startup. Today Python builds a
document once, Rust consumes it, and Python callbacks can mutate Python widget
objects without updating the native UI unless that behavior is already mirrored
inside Rust state. This is the first limitation to fix.

## Architecture Decisions

### 1. Command Queue First

The first milestone is a Rust-owned command queue that can be safely written
from Python callbacks and background threads.

Initial command shape:

```rust
enum Command {
    SetProp { id: WidgetId, prop: PropName, value: PropValue },
    SetStyle { id: WidgetId, style: StylePatch },
    SetScatterPoints { id: WidgetId, data: BufferHandle, columns: ScatterColumns },
    ReplaceChildren { id: WidgetId, children: Vec<WidgetNode> },
    SetTheme { theme: ThemePatch },
    Invalidate { id: WidgetId, dirty: Dirty },
}
```

Python-facing API examples:

```python
app.call_soon_threadsafe(lambda: progress.set_value(0.72))
scatter.set_points(df, x="signal", y="score", z="phase")
button.set_style({"background": "accent"})
```

The command queue immediately enables live `Scatter3D.set_points()`, progress
updates, status updates, modal open/close, image updates, and future component
patches.

### 2. Flush Strategy

Use a wake-and-flush strategy, not frame-only polling.

Required behavior:

- Commands may be enqueued from the UI callback path.
- Commands may be enqueued from a Python background thread.
- Enqueueing a command wakes the event loop through a thread-safe proxy.
- Rust drains the queue before handling the next redraw.
- Draining commands marks dirty regions and requests a redraw if needed.

Frame-start flushing is simpler, but it can add avoidable latency for streaming
data tools. A wake-and-flush path is more complex, but it matches the intended
use case: background work pushing data, progress, status, and scatter updates
into a live native UI.

### 3. Stable Identity

Auto ids are fine for static examples, but reactive rerendering requires stable
identity.

V1 should add explicit `key` support to every widget:

```python
dg.TextInput(key="filter-name")
dg.Scatter3D(df, x="x", y="y", z="z", key="main-scatter")
```

Identity rule:

```text
same parent path + same widget type + same key = same retained element
```

If no key is provided, DragonGUI can keep generating ids for static use. The
component system should strongly encourage keys for repeated or stateful nodes.

### 4. Structured Inline Styles Before CSS Strings

V1 should use structured inline style props:

```python
dg.Panel(
    class_="controls",
    style={
        "width": 340,
        "padding": 12,
        "gap": 8,
        "background": "surface",
        "border_radius": 6,
    },
)
```

In V1, `class_` is a semantic label only. It is included in debug snapshots,
retained tree metadata, and future stylesheet targeting, but it does not resolve
styles by itself until a selector engine exists. Inline `style={...}` remains
the only V1 styling mechanism.

This should map directly into:

- Taffy layout fields.
- Primitive/text renderer style tokens.
- Widget-specific drawing parameters.

Do not implement `app.styles("""...""")` in V1. CSS selectors, specificity,
inheritance, pseudo-state matching, and cascade resolution are a separate
subsystem. They should come after the renderer has a structured style model.

### 5. Typed VNode Diff, Not Dict Diff

The Python component layer should diff typed virtual nodes, not arbitrary dicts.

Proposed internal shape:

```python
VNode(
    type="dropdown",
    key="x-column",
    props={"items": columns, "value": "x"},
    style={"width": 180},
    children=[],
)
```

Diff identity:

```text
same component owner + same parent path + same type + same key
```

Diff behavior:

- Props are shallow-compared.
- Children are keyed where possible.
- Large data values are resource handles, not deeply compared.
- DataFrame, NumPy, Arrow, image, and scatter buffers compare by handle/object
  identity plus explicit version markers.

This avoids accidentally deep-comparing million-row data structures on every
state change.

### 6. Keyed Component State

Do not start with React-style positional hooks. Python does not have macros, and
call-order hooks create invisible constraints.

Use keyed component state:

```python
@dg.component
def Tool(ctx, df):
    x = ctx.state("x", "x")

    return dg.Panel(children=[
        dg.Dropdown(df.columns, value=x.value, on_change=x.set),
        dg.Scatter3D(df, x=x.value, y="y", z="z", key="main-scatter"),
    ])
```

State rule:

```text
state key must be unique within a component instance
```

Rerender rule for V1:

```text
when ctx.state(...).set(value) is called, rerender the owning component and its
subtree
```

Child component state is preserved by keyed identity. A parent rerender may
re-run child component render functions with new props, but it must not discard
the child component's internal `ctx.state(...)` values when the child keeps the
same parent path, component type, and key.

This is simple, predictable, and debuggable. Users who put all state at the app
root may rerender a larger subtree, but that is acceptable for V1 and easy to
explain.

### 7. Coarse Dirty Flags

Use coarse invalidation first:

```rust
enum Dirty {
    Layout,
    Text,
    Visual,
    GpuData,
    Full,
}
```

Patch application decides the dirty mask:

- `padding`, `gap`, `width`, `height`: `Layout`
- text value or label: `Text`
- colors, borders, radius, hover/active: `Visual`
- scatter/table/image buffer changes: `GpuData`
- structural replacement: `Full`

Do not over-optimize invalidation before profiling real apps.

### 8. Debug Snapshot Before Inspector

The first observability tool should be a JSON-compatible snapshot:

```python
snapshot = app.debug_snapshot()
```

It should include:

- retained widget tree
- layout rects
- computed styles
- focus/hover/active state
- dirty flags
- frame time
- GPU upload time
- command queue depth

A full visual inspector can become a later milestone once external users need
it. Treating inspector as a separate product-quality system keeps V1 realistic.

## Style Model V1

Supported style keys should be intentionally small:

Layout:

- `display`
- `flex_direction`
- `flex`
- `flex_grow`
- `flex_shrink`
- `width`
- `height`
- `min_width`
- `min_height`
- `max_width`
- `max_height`
- `padding`
- `padding_left`
- `padding_right`
- `padding_top`
- `padding_bottom`
- `margin`
- `gap`

Visual:

- `background`
- `foreground`
- `border_color`
- `border_width`
- `border_radius`
- `opacity`

Text:

- `font_size`
- `font_family`
- `font_weight`
- `color`
- `text_align`

Widget-specific:

- `accent`
- `track_color`
- `thumb_color`
- `table_row_height`
- `table_header_height`

Pseudo-state styling can be represented structurally before selectors exist:

```python
dg.Button(
    "Run",
    style={
        "background": "surface_alt",
        "hover": {"background": "accent_mix_20"},
        "active": {"background": "accent_dark"},
        "disabled": {"opacity": 0.5},
    },
)
```

This gives web-like behavior without a selector engine.

## Data And Resource Model

Large data must not flow through JSON after startup.

Resource types:

- `BufferHandle`
- `DataFrameHandle`
- `ArrowTableHandle`
- `ImageHandle`
- `TextureHandle`
- `ScatterPointBufferHandle`

Initial implementation can use Python object identity and explicit version
updates. Later implementations should add:

- Python buffer protocol for NumPy-style arrays.
- Arrow C Data Interface for DataFrames.
- Rust-side resource lifetime tracking.
- Explicit release commands for large GPU resources.

## Theme Token Resolution

Structured styles can reference theme tokens:

```python
dg.Panel(style={"background": "surface", "border_color": "border"})
dg.Button(style={"hover": {"background": "accent_mix_20"}})
```

R3 must include token resolution before it ships. The initial resolver can be
small and conservative:

- Resolve built-in tokens from the current `Theme`: `background`, `surface`,
  `surface_alt`, `text`, `muted_text`, `accent`, `border`, `danger`, `warning`,
  `success`, `focus`, and `disabled`.
- Resolve a small set of derived tokens such as `accent_mix_20` and
  `accent_dark`.
- Unknown tokens should produce a clear debug warning and fall back to a safe
  visible color, not panic.
- `SetTheme` commands invalidate visual/text/layout state according to which
  tokens changed. V1 may conservatively mark the full tree dirty.

The first implementation may keep the theme API equivalent to today's
`dg.Theme.dark()` / `dg.Theme.light()` object. Custom stylesheet variables and
runtime user-defined tokens are a later feature.

## Milestone Plan

### R0: Architecture Proposal

Status: this document.

Deliverables:

- Define V1 architecture.
- Record flush strategy.
- Record rerender scope.
- Record postponed features.
- Write API-first example programs before implementation starts.

Acceptance:

- Proposal reviewed before implementation.
- Open decisions are explicit.
- Three future-facing examples exist under `api-examples/`:
  - simple scatter component
  - nested DataFrame/component state
  - background scatter updates

### R1: Stable Identity And Command Queue

Goal: support live native UI mutations without full document resend.

Deliverables:

- Add `key` to Python widgets.
- Add stable Rust `WidgetId`/identity mapping.
- Add thread-safe command queue.
- Add event-loop wake path.
- Add command draining before redraw.
- Add coarse dirty flag plumbing.
- Add `app.call_soon_threadsafe(...)`.

Acceptance:

- A Python callback can enqueue a native update.
- A Python background thread can enqueue a native update.
- Commands wake the window without waiting for unrelated input.
- Existing examples still work with generated ids.
- Commands targeting destroyed widgets, closed windows, or stale handles are
  dropped with a debug log and do not panic.

### R2: Live Scatter Updates

Goal: make `Scatter3D.set_points()` update a running native widget.

Deliverables:

- Store live native widget handles on Python widget objects after `app.run()`
  starts.
- Convert `Scatter3D.set_points()` into a command when the widget is live.
- Avoid JSON/base64 for repeated point updates.
- Add resource handle path for point buffers.

Acceptance:

- Clicking "Plot selected XYZ" changes the rendered scatter without restarting.
- Background thread can push new scatter data.
- Upload time is measured separately from frame time.
- Existing startup data path remains valid.
- If a live scatter handle is stale, closed, or destroyed, `set_points()` returns
  a clear Python error in synchronous contexts and logs/drops the command from
  background contexts.

### R3: Structured Inline Styles

Goal: add web-like layout and visual flexibility without CSS selectors.

Deliverables:

- Add `style` and `class_` props to Python widgets.
- Define `class_` as a semantic/debug label only in V1.
- Add `NodeStyle` in Rust.
- Parse structured style maps from Python.
- Resolve theme token strings into concrete colors/values.
- Map layout style keys into Taffy.
- Map visual style keys into primitive/text rendering.
- Add pseudo-state nested style maps for hover/active/focus/disabled.

Acceptance:

- Examples can change padding, gap, width, colors, borders, and radius per
  widget.
- Hardcoded widget layout constants are reduced where style props exist.
- Hover/active style can be customized without changing Rust code.
- Theme tokens such as `surface`, `accent`, and `border` resolve consistently in
  Rust.

### R4: Typed VNode And Patch Diff

Goal: make structural updates explicit and cheap.

Deliverables:

- Add internal Python `VNode`.
- Add shallow prop/style diff.
- Add keyed child diff.
- Emit command patches from diff results.
- Treat DataFrames and buffers as opaque resources.

Acceptance:

- Replacing a panel child emits targeted patches.
- Updating a text prop emits `Text` dirty only.
- Updating style color emits `Visual` dirty only.
- Large data objects are not deeply compared.

### R5: Keyed Component Runtime

Goal: add Python components and keyed state on top of the patch system.

Deliverables:

- Add `@dg.component`.
- Add component context object.
- Add `ctx.state(key, default)`.
- Rerender owning component subtree on state set.
- Bind callbacks to state setters.
- Make the prewritten `api-examples/` component examples runnable or update the
  proposal if implementation reveals a better API.

Acceptance:

- A dropdown state change rerenders its owning component subtree.
- Component state survives unrelated sibling rerenders.
- Child component state survives parent rerenders when child identity is stable.
- Duplicate state keys in one component produce a clear error.
- Component examples do not require manual document rebuilding.

### R6: Debug Snapshot

Goal: make the retained tree observable.

Deliverables:

- Add `app.debug_snapshot()`.
- Include tree, layout rects, computed styles, state, dirty flags, frame timing,
  and command queue depth.
- Add redaction for large resource handles.

Acceptance:

- Snapshot can be printed from examples.
- Snapshot explains why a widget was relaid out or redrawn.
- Snapshot does not serialize large buffers.

### R7: Stylesheet And Selector Research

Goal: decide whether CSS-like string stylesheets are worth adding.

Deliverables:

- Prototype selector parsing separately from the main renderer.
- Support class, id, type, and pseudo-state selectors in isolation.
- Measure cost on a large widget tree.
- Decide whether to keep structural style maps as the primary API.

Acceptance:

- No selector engine enters the main runtime until it has a measured design.
- The inline style system remains fully usable without stylesheets.

## Risks

### Command Queue Threading

PyO3 callbacks, Python background threads, and the winit main thread must not
share mutable renderer state directly.

Mitigation:

- Commands are data-only.
- Python owns Python objects.
- Rust UI thread owns native widget state.
- Cross-thread communication uses a queue plus event-loop wake proxy.

### Component API Permanence

The component API will become user-facing and hard to change.

Mitigation:

- Start with `ctx.state(key, default)`, not `use_state`.
- Write examples before implementation.
- Keep positional hook sugar out of V1.

### Style Scope Creep

CSS-like syntax can balloon into a browser project.

Mitigation:

- Structured styles first.
- No selectors in V1.
- No cascade until computed style exists.

### Dirty Flag Misclassification

Wrong dirty flags can cause stale UI or unnecessary rebuilds.

Mitigation:

- Start coarse.
- Prefer over-invalidating to stale rendering.
- Add debug snapshot dirty reasons.

### Resource Lifetime

Large buffers and GPU resources can leak if handles are not released.

Mitigation:

- Central Rust resource registry.
- Reference-counted handles or explicit release commands.
- Debug snapshot includes resource counts.

## Success Criteria

This effort succeeds when:

- Python can update a running UI without full document resend.
- `Scatter3D.set_points()` works on a live widget.
- Background threads can safely update progress/status/data widgets.
- Per-widget structured styles influence layout and drawing.
- Python components can own keyed state and rerender only their subtree.
- Large data is represented by handles, not deep diffs or JSON.
- Debug snapshots make layout, style, state, and dirty behavior inspectable.
- No user-facing feature requires Python to run every frame.
