# DragonGUI Library Overview

DragonGUI is a Python application toolkit for GPU-native data tools. The core
idea is simple: Python describes the application, while Rust owns the hot path:
windowing, input, layout, rendering, text, data uploads, retained widget state,
and custom GPU widgets.

The library is currently pre-alpha. It already supports a usable set of native
widgets, live updates from Python callbacks and background threads, structured
inline styles, a CSS styling subset, themes, reactive Python components, GPU
scatter rendering, and DataFrame-style tables.

## What DragonGUI Does

DragonGUI lets Python developers build desktop data applications without
writing Rust:

```python
import dragongui as dg

app = dg.App(theme=dg.Theme.dark())
win = dg.Window("My Tool", width=1200, height=800)

with dg.HLayout():
    with dg.Panel("Controls", style={"width": 320, "padding": 14, "gap": 10}):
        col = dg.Dropdown(items=["x", "y", "z"], value="x")
        dg.Button("Plot")
    dg.Scatter3D(frame, x="x", y="y", z="z")

app.run(win)
```

Python code builds the widget tree and handles user callbacks. The native Rust
backend runs the event loop, computes layout, handles input, draws widgets, and
updates GPU resources.

## Product Position

DragonGUI targets the same broad slot as Dear PyGui: native desktop tools for
data-heavy Python applications. The intended differentiation is:

- Modern `wgpu` rendering instead of an OpenGL-first stack.
- Rust-owned runtime state and rendering.
- Python API ergonomics for scientists and tool builders.
- First-class DragonSci-style GPU data widgets.
- Native DataFrame and large-data workflows without pushing every operation
  through Python frame by frame.

DragonGUI is not a browser, not a webview wrapper, and not a JavaScript UI
framework. It aims for web-like layout and styling flexibility through native
Rust systems.

## Architecture

The current architecture is:

```text
Python app and widgets
    -> optional reactive component runtime
    -> typed document / VNode diff / command patches
    -> PyO3 native bridge
    -> Rust command queue and retained widget tree
    -> Taffy layout, widget state, resource registries
    -> wgpu primitive, text, table, and scatter renderers
    -> native window through winit
```

Python is not in the render loop. Python runs when:

- The application creates the initial widget tree.
- A callback fires.
- Component state changes.
- A background thread schedules work with `call_soon_threadsafe`.
- A live widget method queues a native update.

Rust owns:

- The native window and event loop.
- Hit testing, hover, active, focus, scrolling, and keyboard input.
- Layout computation.
- Text shaping and glyph rendering.
- Primitive widget rendering.
- Scatter, table, and image rendering.
- GPU buffers and retained resources.
- Command draining and redraw invalidation.

## Python Frontend

The Python package exposes a flat `import dragongui as dg` API.

Core application objects:

- `App`
- `Theme`
- `Window`
- `BackendUnavailableError`
- `backend_info()`
- `native_backend_available()`

The user can build UIs with context managers:

```python
win = dg.Window("Tool")
with dg.HLayout():
    with dg.Panel("Controls"):
        dg.Button("Run", on_click=run)
        dg.Checkbox("Enabled", checked=True)
    dg.DataFrameTable(df)
```

Widgets can also be created as explicit children where useful:

```python
panel = dg.Panel("Controls")
panel.add(dg.Button("Run", on_click=run, parent=None))
```

Each widget serializes to a typed document for startup. Live widgets also keep
handles after `app.run(...)` begins, so Python methods can enqueue native
updates instead of rebuilding the whole app.

## Widget Catalog

### Top-Level

| Widget | Purpose |
| --- | --- |
| `Window` | Top-level native application window. Holds the root widget tree. |
| `FileDialog` | Native open/save/folder picker helpers with synchronous or callback-based use. Top-level helpers are also exported as `open_file_dialog`, `open_files_dialog`, `save_file_dialog`, and `pick_folder_dialog`. |
| `toast` / `App.toast` | Live app helper for non-blocking native notification overlays. Supports per-toast level, duration, opacity, corner position, radius, and padding, and can be styled with `Toast` CSS selectors. Returns a `ToastHandle` for update or dismiss. |

### Layout And Structure

| Widget | Purpose |
| --- | --- |
| `HLayout` | Horizontal flex container. |
| `VLayout` | Vertical flex container. |
| `Panel` | Framed titled container for controls and grouped content. |
| `Collapsible` | Expandable/collapsible vertical section for dense control groups. |
| `Modal` | Centered overlay container for blocking alert/confirm workflows; prefer `show()` and `close()`. |
| `MenuBar` | Horizontal application menu strip. |
| `ContextMenu` | Targeted right-click popup menu container. |
| `Sidebar` | Side navigation/container region. |
| `StatusBar` | Bottom/status strip container. |
| `Separator` | Visual divider. |
| `Spacer` | Fixed or flexible blank layout space. |

### Navigation

| Widget | Purpose |
| --- | --- |
| `Tabs` | Tab strip container with live `set_value(...)` route switching. |
| `Tab` | Individual tab item with optional badge text/count. |
| `Pages` | Page container paired with navigation state and live `set_value(...)` route switching. |
| `Page` | Individual page inside `Pages`. |
| `NavItem` | Sidebar/page navigation item with optional badge text/count. |
| `Menu` | Top-level menu inside a `MenuBar`. |
| `MenuItem` | Clickable row inside `Menu` or `ContextMenu`. |
| `Tooltip` | Rich hover overlay attached to a target widget. |

### Basic Controls

| Widget | Purpose |
| --- | --- |
| `Label` | Text display. |
| `Badge` | Compact status/count pill with semantic levels. |
| `Tag` | Compact bordered status label with semantic levels. |
| `Button` | Clickable command control with `on_click` and optional badge text/count. |
| `TextInput` | Editable single-line text input with `on_change`. |
| `TextArea` | Editable multiline text input with rows, wrapping, internal scrolling, and `on_change`. |
| `NumberInput` | Editable numeric input with min/max/step and `on_change`. |
| `Slider` | Numeric drag control with range and `on_change`. |
| `ProgressBar` | Non-interactive progress indicator with live `set_value`. |
| `Dropdown` | Select one value from a list. |
| `Checkbox` | Boolean toggle. |
| `ColorPicker` | Composite RGB/RGBA picker with slider channels, swatch preview, and optional `set_value(..., notify=True)`. |

### Data And GPU Widgets

| Widget | Purpose |
| --- | --- |
| `Scatter3D` | GPU-rendered 3D point cloud/scatter widget. |
| `DataFrameTable` | Native virtualized table for DataFrame-like data. |
| `Image` | Textured image quad renderer for local PNG/JPEG files. |

## Widget Behavior

Current interaction support includes:

- Button click callbacks.
- Inline badges on buttons, tabs, and navigation items.
- Standalone `Badge` and `Tag` status widgets.
- Checkbox toggling.
- Dropdown open/select behavior.
- Number input typing, steppers, clamping, and callbacks.
- Slider drag and value callbacks.
- Composite ColorPicker updates with integer RGB/RGBA callbacks; programmatic `set_value` is silent unless `notify=True`.
- Progress bar live value updates.
- Static text tooltips on hovered widgets through the common `tooltip` prop.
- Rich tooltip overlays with arbitrary DragonGUI children.
- Modal overlays with background input blocking and Escape close.
- Toast notification overlays for non-blocking status feedback.
- Collapsible sections with pointer and keyboard toggling.
- Menu bar popups, menu item callbacks, and target-based context menus.
- File open/save/folder dialogs through top-level helpers or `FileDialog`.
- PNG/JPEG image display with fit modes and styled placeholders.
- Text input and multiline text area editing, internal TextArea scrolling, caret positioning, and keyboard input.
- Tab focus navigation and programmatic tab route switching.
- Enter/Space activation for keyboard-focused controls.
- Sidebar/page navigation and programmatic page route switching.
- Table scrolling and cell/header selection.
- Scatter orbit, pan, and zoom.

Scatter interaction:

- Left drag orbits the camera.
- Middle or right drag pans.
- Mouse wheel zooms.
- Resize updates the scatter viewport and camera aspect.

## Reactive Component Frontend

DragonGUI includes an experimental reactive Python layer.

Exports:

- `@component`
- `ComponentCtx`
- `ComponentInstance`
- `StateSlot`
- `VNode`
- `Patch`
- `ResourceRef`

The component model uses keyed state instead of React-style positional hooks:

```python
@dg.component
def Tool(ctx, df):
    x = ctx.state("x", "x")

    return dg.Panel(children=[
        dg.Dropdown(items=df.columns, value=x.value, on_change=x.set),
        dg.Scatter3D(df, x=x.value, y="y", z="z", key="main-scatter"),
    ])
```

State rules:

- State keys must be unique within a component instance.
- Updating state rerenders the owning component subtree.
- Child component state is preserved when parent path, component type, and key
  remain stable.
- Large resources are compared by handle/object identity, not by deep value.

The component system produces VNode diffs and sends patches through the live
native command queue.

## Live Updates And Command Queue

DragonGUI supports runtime mutation after startup. Python can enqueue native
updates from UI callbacks or background threads.

Important APIs:

- `App.call_soon_threadsafe(fn)`
- `App.debug_snapshot(timeout_ms=1000)`
- `Widget.set_style(style)`
- `Container.replace_children(children)`
- `Scatter3D.set_points(frame, x=..., y=..., z=...)`
- `DataFrameTable.set_frame(frame)`
- `App.set_buffer_resource(...)`
- `App.release_resource(resource_id)`

The native command queue supports commands such as:

- Set widget property.
- Set style patch.
- Replace children.
- Replace a node.
- Set packed scatter point data.
- Set table data.
- Set table data columns.
- Set/release buffer resources.
- Invalidate layout, text, visual, GPU data, or full state.
- Request debug snapshots.
- Drain scheduled Python tasks.

The queue wakes the event loop when work is scheduled, drains before redraw,
marks dirty state, and requests a redraw when needed.

## Styling And Theming

DragonGUI supports two styling paths:

- Structured inline `style={...}` dictionaries for local widget overrides.
- A DragonGUI CSS subset loaded with `app.stylesheet(...)` or
  `app.load_stylesheet(...)` for app-wide design systems.

Inline styles keep the highest precedence over stylesheets, so existing apps
remain compatible.

Example:

```python
dg.Panel(
    "Controls",
    class_="controls",
    style={
        "width": 340,
        "padding": 14,
        "gap": 10,
        "background": "surface",
        "border_color": "border",
        "border_radius": 6,
    },
)
```

CSS example:

```python
app.stylesheet("""
Panel.controls {
    width: 340px;
    padding: 14px;
    gap: 10px;
    background: surface;
    border: 1px solid border;
    border-radius: 6px;
}

Panel.controls > Button {
    height: 34px;
}

Button:hover {
    background: accent_mix_20;
}
""")
```

Supported CSS selectors include type, class, id, key, attribute, descendant,
direct-child, structural, selector-function, and widget-part selectors, along
with public pseudo-states such as `:hover`, `:active`, `:focus`, and
`:disabled`. Text properties inherit down the widget tree.

See `docs/css-styling.md` for the full supported CSS subset.

Supported layout style concepts include:

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

Supported visual style concepts include:

- `background`
- `foreground`
- `border_color`
- `border_width`
- `border_radius`
- `opacity`
- `accent`
- `track_color`
- `thumb_color`

Supported text style concepts include:

- `font_size`
- `font_family`
- `font_weight`
- `color`
- `text_align`

Pseudo-state styles are structural:

```python
dg.Button(
    "Run",
    style={
        "background": "surface_alt",
        "hover": {"background": "accent_mix_20"},
        "active": {"background": "accent_dark"},
        "focus": {"border_color": "focus"},
        "disabled": {"opacity": 0.5},
    },
)
```

Current built-in themes:

- `Theme.dark(...)`
- `Theme.light(...)`

Theme tokens include:

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

The native renderer also resolves derived tokens such as accent mix and accent
dark variants.

Python `Theme` color fields accept literal CSS-style color strings: hex forms
including alpha, `transparent`, common named colors, `rgb()/rgba()`, and
`hsl()/hsla()`, and `hwb()`, plus `lab()`, `lch()`, `oklab()`, `oklch()`,
`color(srgb ...)`, and `color(srgb-linear ...)`.

## Rust Backend

The native backend is a PyO3 extension module built with maturin.

Major backend systems:

| Area | Files | Responsibility |
| --- | --- | --- |
| Python bridge | `native/src/lib.rs`, `native/src/app.rs` | Exposes native APIs to Python. |
| Command queue | `native/src/commands.rs` | Thread-safe command bridge and runtime events. |
| Runtime | `native/src/runtime.rs` | Window/event loop, command drain, redraw, native state. |
| Document parsing | `native/src/document.rs` | Converts Python widget documents into typed Rust nodes. |
| Layout | `native/src/layout.rs` | Taffy-based layout and widget rect computation. |
| Events | `native/src/events.rs` | Hit testing, focus, active/hover state, widget interactions. |
| Styles | `native/src/style.rs`, `native/src/css_style.rs`, `native/src/theme.rs`, `native/src/framework.dg.css` | Inline style parsing, CSS parsing/cascade, pseudo-state merging, token resolution. |
| Primitive rendering | `native/src/primitives/` | Instanced rounded rectangles, controls, table shapes, overlays. |
| Text rendering | `native/src/text/` | Text layout, glyph caching, widget labels, table/dropdown text. |
| Tables | `native/src/table.rs`, `native/src/resources.rs` | Table metrics, visible ranges, data resources. |
| Scatter | `native/src/scatter/` | GPU 3D scatter widget, camera, colormaps, point shader. |

Native dependencies currently include:

- `pyo3`
- `winit`
- `wgpu`
- `taffy`
- `glyphon`
- `glam`
- `bytemuck`
- `serde` / `serde_json`
- `base64`
- `lightningcss`
- `image`
- `rfd`
- `pollster`
- `thiserror`

## Rendering Pipeline

The backend renders with `wgpu`. The exact ordering is implementation-specific,
but the rendering model is:

1. Clear the window surface.
2. Draw primitive widget geometry.
3. Draw custom GPU widgets such as `Scatter3D` within their layout rects and
   clip regions.
4. Draw table and overlay primitives.
5. Draw text, caret, dropdown text, table text, and other foreground elements.

Primitive widgets are mostly instanced rectangles with color, radius, and
border-like effects. Text rendering uses a Rust text pipeline and buffer cache.
Scatter uses its own GPU pipeline and camera state.

## Data Model

DragonGUI has two data paths:

### Startup Document Data

The initial app document can include bounded metadata or compact serialized
payloads. This is used for startup compatibility and simple examples.

### Live Resource Data

After the app is running, large data should flow through resource handles and
native commands, not through full document resend.

Current live data support includes:

- Packed float32 scatter point buffers.
- Table resource updates.
- Generic buffer resource registration.
- Resource release commands.
- Optional widget ownership for generic buffers; app-owned buffers persist until
  explicit release, while widget-owned buffers are purged when the owner leaves
  the retained tree.

For Python data, the package includes helpers that can summarize DataFrame-like
objects, extract bounded table samples, and pack NumPy-accessible columns.

## DataFrameTable

`DataFrameTable` displays tabular data in the native renderer and can emit
selection callbacks when a user clicks or keyboard-selects a cell.

Current capabilities:

- Accepts DataFrame-like objects.
- Extracts column names, dtypes, shape, and bounded samples.
- Supports live table replacement.
- Supports selected cell/header state.
- Emits `on_select` with `TableSelection(row_index, column_index, column, value)`.
- Supports scrolling by row and column.
- Supports keyboard cell navigation with Arrow keys, PageUp/PageDown,
  Home/End, and Enter/Space.
- Renders headers, grid/row backgrounds, visible cells, and selected regions.

The table is designed around visible rows/columns rather than drawing every row
in Python.

## Scatter3D

`Scatter3D` is the current flagship GPU widget.

Current capabilities:

- Renders 3D point data with `wgpu`.
- Uses a retained camera for orbit, pan, and zoom.
- Supports colormaps and point shader logic derived from DragonSci work.
- Supports live colormap changes through `Scatter3D.set_colormap(...)`.
- Can receive live point updates from Python.
- Emits `on_pick` with `ScatterPick(index, x, y, z)` for point clicks.
- Renders inside the same native window/layout tree as the rest of the UI.

This is the core example of DragonGUI's intended moat: data widgets that are
native GPU scene nodes, not embedded browser canvases.

## Debugging And Introspection

`App.debug_snapshot()` returns a JSON-compatible snapshot from the native
runtime.

The snapshot is intended to expose:

- Retained widget tree.
- Layout rects.
- Computed style state.
- Focus, hover, and active state.
- Dirty flags.
- Frame timing.
- GPU upload timing where available.
- Command queue/resource state.

This is the first observability layer. A full visual inspector is not part of
the current runtime.

## Packaging And Development

The package is organized for PyPI through `pyproject.toml` and maturin.

Package facts:

- Python package name: `dragongui`
- Native extension module: `dragongui._dragongui`
- Rust crate: `dragongui-native`
- Python source directory: `python`
- Native source directory: `native`
- Python requirement: `>=3.11`

Optional dependency groups:

- `dataframe`: pandas and polars support.
- `dev`: maturin, pytest, ruff.

The repository also includes local development helpers such as `start.bat` and
examples that can run against the in-repo Python package before wheel
installation.

## Examples

Current examples cover:

- Full feature showcase: `examples/all_features_demo.py`
- CSS all-features demo: `examples/all_features_css_demo.py`
- CSS design system demo: `examples/css_design_system_demo.py`
- CSS showcase: `examples/css_showcase.py`
- Scatter tool: `examples/scatter_tool.py`
- Table tool: `examples/table_tool.py`
- Live table updates: `examples/live_table_tool.py`
- Streaming scatter updates: `examples/streaming_scatter_tool.py`
- Live style updates: `examples/live_style_tool.py`
- Live prop updates: `examples/live_update_tool.py`
- Live child replacement: `examples/live_children_tool.py`
- Component counter: `examples/component_counter_tool.py`
- Nested components: `examples/component_nested_tool.py`
- Component node swapping: `examples/component_node_swap_tool.py`
- Multipage navigation: `examples/multipage_tool.py`
- Style showcase: `examples/style_showcase.py`
- Debug snapshot demo: `examples/debug_snapshot_tool.py`

The reactive engine plan also includes API-first examples under
`plans/10-reactive-native-engine/api-examples/`.

## Current Limits

DragonGUI intentionally does not currently include:

- Browser DOM.
- HTML parser.
- Webview frontend.
- JavaScript runtime.
- Browser-complete CSS behavior.
- Descendant selectors, media queries, transitions, or scoped CSS variables.
- Full visual inspector.
- Broad native desktop platform coverage such as system trays, clipboard APIs,
  drag-and-drop, printing, dock/taskbar integration, and accessibility.

Some of these are planned or documented separately, but they are not part of the
current core runtime.

Known engineering areas that still need continued audit and hardening:

- Cross-platform CI and packaging breadth.
- More concurrency tests around shutdown and background updates.
- More visual regression testing for layout/text clipping.
- More performance profiling for large table and high-frequency update paths.
- Resource lifetime tracking as large data workflows expand.

## Summary

DragonGUI currently provides:

- A Python widget API.
- A Python reactive component layer with keyed state.
- A PyO3 bridge into a Rust native backend.
- A retained Rust widget/render tree.
- Taffy-based layout.
- Structured inline styling and theme tokens.
- CSS stylesheets with cascade, specificity, pseudo-states, and text
  inheritance.
- Native input/focus/hover/active behavior.
- Native tooltip overlays.
- Native modal overlays.
- `wgpu` rendering.
- Native text rendering through `glyphon`.
- Primitive widgets and navigation widgets.
- GPU scatter rendering.
- DataFrame table rendering.
- Live updates through a thread-safe command queue.
- Debug snapshots for runtime inspection.

The long-term direction is a Python-first application toolkit with web-like
layout/styling flexibility and Rust/GPU performance for serious data tools.
