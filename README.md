# DragonGUI

A Python application toolkit for GPU-native data tools.

Python describes the application. Rust owns the hot path: windowing, input,
layout, text, rendering, GPU widgets, and retained widget state. Python never
runs in the frame loop.

**Status:** pre-alpha. The native backend, widget set, reactive component
system, live update pipeline, CSS styling, and GPU data widgets are functional.
The API is not yet stable.

## Quick Look

```python
import dragongui as dg

app = dg.App(theme=dg.Theme.dark())
app.stylesheet("""
    Panel.controls { width: 340px; padding: 14px; gap: 10px; }
    Button:hover   { background: accent_mix_20; }
""")

win = dg.Window("My Tool", width=1200, height=800)

with dg.HLayout():
    with dg.Panel("Controls", class_="controls"):
        col = dg.Dropdown(items=["x", "y", "z"], value="x")
        dg.Slider(0.5, min=0, max=1)
        dg.Button("Plot", on_click=lambda: scatter.set_points(df, x=col.value))

    scatter = dg.Scatter3D(df, x="x", y="y", z="z")

app.run(win)
```

## What It Does

- **Native rendering** with `wgpu` through a Rust backend built on `winit`,
  `taffy`, and `glyphon`.
- **45+ widgets** including panels, inputs, sliders, dropdowns, checkboxes,
  number inputs, progress bars, menus, modals, tabs, pages, navigation,
  color pickers, images, badges, LEDs, collapsibles, tooltips, and more.
- **CSS styling** with type, class, id, attribute, pseudo-state, pseudo-function
  (`:not()`, `:is()`, `:has()`, `:nth-child()`), and widget-part (`::thumb`,
  `::stepper`, `::badge`, etc.) selectors, custom variables, and media queries.
- **Reactive components** with keyed state slots, VNode diffing, and targeted
  native patches.
- **Live updates** from Python callbacks and background threads via a
  thread-safe command queue.
- **GPU scatter** — 3D point clouds with 500K+ points, orbit/pan/zoom, 14
  colormaps, scalar bar, categorical legend, orientation axes, 3D labels, line
  and box overlays, multi-actor layers, lasso/rectangle selection, hover
  tooltips, LOD, and real-time frame streaming.
- **Virtualized DataFrame tables** with column buffers, scrolling, selection,
  and sorting.
- **Toast notifications**, file dialogs, native image display, and debug
  snapshots.

## Widget Catalog

| Category | Widgets |
| --- | --- |
| Layout | `Window`, `HLayout`, `VLayout`, `Panel`, `ScrollArea`, `GridLayout`, `FlowLayout`, `Sidebar`, `StatusBar`, `Separator`, `Spacer` |
| Navigation | `Tabs`, `Tab`, `Pages`, `Page`, `NavItem`, `MenuBar`, `Menu`, `MenuItem`, `ContextMenu` |
| Container | `Collapsible`, `Modal`, `Tooltip` |
| Controls | `Button`, `TextInput`, `TextArea`, `NumberInput`, `Slider`, `Dropdown`, `Checkbox`, `ColorPicker` |
| Display | `Label`, `Badge`, `Tag`, `LED`, `Image`, `ProgressBar` |
| Data / GPU | `Scatter3D`, `DataFrameTable` |
| Overlays | `alert()`, `confirm()`, `toast()` |
| Utilities | `FileDialog`, `Theme` |

## 3D Scatter

`Scatter3D` renders large point clouds via wgpu with a wide API for interactive
data exploration.

**Creating a plot:**
```python
scatter = dg.Scatter3D(
    df, x="px", y="py", z="pz",
    colormap="viridis",   # or scalars=df["intensity"]
    point_size=3.0,
    grid=True,
    on_pick=lambda hit: print(hit.index),
)
```

**Updating live:**
```python
scatter.set_points(new_df, x="px", y="py", z="pz")
scatter.set_colormap("plasma")
```

**Chrome elements:**
```python
scatter.show_grid(True)
scatter.show_grid_planes(major=True, minor=True)
scatter.set_axes("X (m)", "Y (m)", "Z (m)")
scatter.show_legend(entries=[("Cluster A", 0.2, 0.6, 1.0)])
scatter.show_scalar_bar(vmin=0.0, vmax=1.0, colormap="inferno", title="Intensity")
scatter.show_orientation_axes(True)
scatter.set_background(0.05, 0.05, 0.08)
```

**3D annotations:**
```python
scatter.add_label("origin", 0, 0, 0, "O", r=1, g=1, b=0, size=14)
scatter.add_lines("bbox_top", segments=[[x0,y0,z1, x1,y0,z1, ...]], r=1, g=0.5, b=0)
scatter.add_box("hull", xmin, xmax, ymin, ymax, zmin, zmax, r=0.3, g=0.8, b=1.0)
```

**Multiple independent point layers (actors):**
```python
scatter.add_points("layer_b", df_b, x="x", y="y", z="z", colormap="plasma")
scatter.update_actor("layer_b", df_new, x="x", y="y", z="z")
scatter.remove_actor("layer_b")
```

**Real-time frame streaming:**
```python
payloads = [scatter.prepare_points(df, x, y, z) for df in frames]
stream = scatter.stream_prepared_frames(payloads, interval_ms=40, loop=True)
stream.start()
# ... later:
stream.stop()
print(stream.metrics)  # ScatterStreamMetrics(produced, submitted, ui_callbacks, errors)
```

**Selection and hover:**
```python
scatter = dg.Scatter3D(df, ..., on_pick=lambda hit: print(hit.index, hit.point))
# After interaction:
hits = scatter.selected            # list[ScatterHit]
indices = scatter.selected_indices # list[int]
```

**Available colormaps:** `viridis`, `plasma`, `inferno`, `magma`, `coolwarm`,
`hot`, `gray`, `turbo`, `cividis`, `blues`, `greens`, `reds`

**Camera linking:**
```python
dg.link_cameras(scatter_a, scatter_b)   # programmatic camera changes propagate
dg.unlink_cameras(scatter_a, scatter_b)
```

## Components

Reactive components with keyed state and VNode diffing:

```python
@dg.component
def Counter(ctx: dg.ComponentCtx, initial: int = 0):
    count = ctx.state("count", initial)

    win = dg.Window("Counter")
    with dg.Panel():
        dg.Label(f"Count: {count.value}")
        dg.Button("Increment", on_click=lambda: count.set(count.value + 1))
    return win

app.run(Counter(initial=5))
```

- `@dg.component` — wraps a render function as a reusable component.
- `ctx.state(key, default)` → `StateSlot` — stable slot per key; `.value` reads,
  `.set(v)` writes and triggers a diff+patch cycle.
- `ctx.app` — access `AppHandle` (toast, `call_soon_threadsafe`, etc.) while live.

## CSS Styling

```python
app.stylesheet("""
    /* Type, class, ID, attribute selectors */
    Panel              { background: #1a1a2e; border-radius: 8px; }
    Button.primary     { background: accent; color: white; }
    #save              { width: 120px; }
    [disabled]         { opacity: 0.4; }

    /* Pseudo-states */
    Button:hover       { background: accent_mix_20; }
    Checkbox:checked   { color: accent; }
    Collapsible:expanded > Panel { padding: 12px; }
    Dropdown:open      { border-color: accent; }

    /* Pseudo-functions */
    Button:not(.ghost) { border: 1px solid border; }
    Panel:has(Slider)  { padding-left: 16px; }
    :nth-child(2n)     { background: surface_alt; }

    /* Widget parts (sub-element selectors) */
    Slider::thumb      { background: accent; width: 14px; }
    NumberInput::stepper { background: surface_alt; }
    Checkbox::box      { border-radius: 4px; }
    Tabs::tab:hover    { color: accent; }
    ProgressBar::fill  { background: linear-gradient(...); }

    /* Media queries */
    @media (max-width: 800px) { Panel.controls { display: none; } }
    @media (prefers-color-scheme: dark) { Window { background: #0d0d0d; } }
""")
```

Supported pseudo-states: `:hover`, `:active`, `:focus`, `:checked`, `:disabled`,
`:selected`, `:expanded`, `:open`

CSS parts vary by widget — see [docs/css-styling.md](./docs/css-styling.md) for
the full per-widget part list.

## Theming

```python
# Built-in presets
app = dg.App(theme=dg.Theme.dark())
app = dg.App(theme=dg.Theme.light())

# Override specific tokens
theme = dg.Theme.dark(
    accent="#3fc7ff",
    radius=8.0,
    font_size=14.0,
)

# Reference tokens in stylesheets
app.stylesheet("Button { background: accent_mix_20; border-color: border; }")
```

Theme tokens available as CSS values: `background`, `surface`, `surface_alt`,
`text`, `muted_text`, `accent`, `border`, `danger`, `warning`, `success`,
`focus`, `disabled` — plus computed mix variants (e.g. `accent_mix_20`).

## Live Updates

```python
# From callbacks (main thread)
slider = dg.Slider(0.5, on_change=lambda v: label.set_props(text=f"{v:.2f}"))

# From background threads
import threading
def worker():
    while True:
        data = fetch_data()
        app.call_soon_threadsafe(lambda d=data: scatter.set_points(d))
        time.sleep(1.0)

threading.Thread(target=worker, daemon=True).start()
```

**Toasts:**
```python
handle = app.toast("Loading...", level="info")
# ... later:
handle.update("Done!", level="success")
handle.dismiss()
```

Toast positions: `top-right`, `top-left`, `bottom-right`, `bottom-left`

## Repository Layout

```
python/dragongui/    Python package
native/src/          Rust native extension (PyO3 + maturin)
tests/               264 tests across API, components, and VDOM
examples/            28 runnable demos
plans/               Implementation plans and roadmap
docs/                CSS and widget reference documentation
```

## Development

Run an example from the source tree without installing:

```powershell
.\start.bat
```

Install in editable mode:

```bash
pip install -e ".[dev]"
```

Build the native extension:

```bash
maturin develop --manifest-path native/Cargo.toml
```

Run tests:

```bash
python -m pytest
```

Build a release wheel (Windows/MSYS2):

```bash
maturin build --release --manifest-path native/Cargo.toml --out dist --target x86_64-pc-windows-gnu
```

## Examples

| Example | What It Shows |
| --- | --- |
| `all_features_v3_demo.py` | Full demo: scatter, table, streaming, overlays, chrome, CSS |
| `all_features_demo.py` | All widget types on one screen |
| `all_features_css_demo.py` | Widget set with full CSS theme |
| `streaming_scatter_tool.py` | Background thread pushing live scatter frames |
| `scatter_tool.py` | Scatter + table + controls |
| `table_tool.py` | DataFrameTable with selection and sorting |
| `css_design_system_demo.py` | Four CSS themes on the same widget tree |
| `css_showcase.py` | CSS selectors, variables, and pseudo-states |
| `css_theme_gallery.py` | Theme gallery with live switcher |
| `css_widget_parts_demo.py` | Widget parts and sub-element selectors |
| `css_web_capabilities_demo.py` | CSS media queries and responsive layout |
| `css_julius_style_demo.py` | Custom design system demo |
| `color_scheme_media_tool.py` | `prefers-color-scheme` media query |
| `component_counter_tool.py` | Reactive component with keyed state |
| `component_nested_tool.py` | Nested components with state isolation |
| `component_node_swap_tool.py` | Component node identity and swap |
| `multipage_tool.py` | Sidebar, pages, tabs, and status bar |
| `live_update_tool.py` | Live prop updates from callbacks |
| `live_table_tool.py` | Live DataFrame table replacement |
| `live_children_tool.py` | Dynamic child insertion and removal |
| `live_style_tool.py` | Live CSS stylesheet updates |
| `collapsible_tool.py` | Collapsible sections |
| `badge_tool.py` | Badge and Tag widget variants |
| `tooltip_tool.py` | Tooltip overlay |
| `textarea_tool.py` | Multi-line text input |
| `toast_tool.py` | Toast notifications |
| `meridian.py` | Full-featured application demo |
| `debug_snapshot_tool.py` | Runtime introspection snapshot |

## Architecture

```
Python widgets / components
    -> typed document / VNode diff / command patches
    -> PyO3 native bridge
    -> Rust command queue and retained widget tree
    -> CSS cascade, Taffy layout, widget state
    -> wgpu primitive, text, image, scatter, and table renderers
    -> native window (winit)
```

## Native Stack

| Crate | Purpose |
| --- | --- |
| `pyo3` | Python bindings (abi3, stable ABI) |
| `winit` | Native window and event loop |
| `wgpu` | GPU rendering |
| `taffy` | Flexbox/grid layout |
| `glyphon` | Text shaping and rendering |
| `lightningcss` | CSS parsing |
| `rfd` | Native file dialogs |
| `image` | PNG/JPEG decoding |

## Requirements

- Python >= 3.11
- Rust toolchain (for building the native extension)
- Windows, macOS, or Linux with GPU support

Optional: `pandas` or `polars` for DataFrame integration. `numpy` for
`sample_colormap_numpy`.

## License

MIT. Third-party dependency notices are included in
[THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md).

## Documentation

- [docs/widgets-reference.md](./docs/widgets-reference.md) — all widget options
- [docs/css-styling.md](./docs/css-styling.md) — selectors, parts, and variables
- [docs/css-capabilities-reference.md](./docs/css-capabilities-reference.md) — full CSS feature list
- [docs/layout.md](./docs/layout.md) — layout system and taffy integration
- [docs/library-overview.md](./docs/library-overview.md) — architecture deep-dive
- [plans/](./plans/) — implementation roadmap
