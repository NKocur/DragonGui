# DragonGUI

A Python application toolkit for GPU-native data tools.

Python describes the application. Rust owns the hot path: windowing, input,
layout, text, rendering, GPU widgets, and retained widget state. The native
renderer does not call Python on every frame.

**Status:** stable. Version 1.0.0 is the first production release. Public APIs
follow semantic versioning from this release forward. `NodeGraph` remains an
experimental preview outside the stable API guarantee; see
[Experimental APIs](#experimental-apis).

## Installation

DragonGUI requires Python 3.12 or 3.13. Install the prebuilt wheel from PyPI:

```bash
python -m pip install dragongui
```

Normal wheel installation does not require Rust. A Rust toolchain is needed
only when pip must build from the source distribution or when developing
DragonGUI itself.

Optional integrations can be installed independently:

```bash
python -m pip install "dragongui[numpy]"     # NumPy plotting/colormap paths
python -m pip install "dragongui[pandas]"    # pandas integration
python -m pip install "dragongui[polars]"    # Polars integration
python -m pip install "dragongui[dataframe]" # pandas and Polars together
python -m pip install "dragongui[terminal]"  # Windows ConPTY support
```

Version 1.0.0 publishes wheels for Windows x86-64, Linux x86-64, macOS Intel,
and macOS Apple Silicon. Other architectures may build from source but are not
part of the supported 1.0.0 binary matrix.

## Quick Look

```python
import dragongui as dg

app = dg.App(theme=dg.Theme.dark())
app.stylesheet("""
    Panel.controls { width: 340px; padding: 14px; gap: 10px; }
    Button:hover   { background: accent_mix_20; }
""")

points = {
    "x": [-1.0, 0.0, 1.0],
    "y": [0.2, 0.8, 0.3],
    "z": [0.0, 0.6, 1.0],
}

with dg.Window("My Tool", width=1200, height=800) as win:
    with dg.FlexLayout(direction="row", gap=12, style={"padding": 12}):
        with dg.Panel("Controls", class_="controls"):
            palette = dg.Dropdown(["viridis", "plasma", "turbo"], value="viridis")
            dg.Button("Apply", on_click=lambda: scatter.set_colormap(palette.value))

        scatter = dg.Scatter3D(points, x="x", y="y", z="z", grid=True)

app.run(win)
```

## What It Does

- **Native rendering** with `wgpu` through a Rust backend built on `winit`,
  `taffy`, and `glyphon`; rendering and retained widget state remain native.
- **80+ widgets** — layout containers, buttons, text/number/date-time inputs,
  sliders, dropdowns, checkboxes, radios, toggles, trees, menus, modals, tabs,
  pages, navigation, breadcrumbs, a command palette, color pickers, images,
  badges, LEDs, collapsibles, tooltips, progress/loading indicators,
  drag-and-drop, and a custom-drawing `PaintWidget`.
- **Charts and data widgets** — GPU `Scatter3D`, `ScatterPlot2D`, `LinePlot`,
  `Histogram`, `BarChart`, `Heatmap`, `PieChart`, a virtualized `DataFrameTable`,
  an `HtmlReport` webview, and telemetry-oriented `LimitsBar` indicators.
- An optional embedded **Terminal**, multi-agent session helpers, and a thread
  monitor. `NodeGraph` exists as an experimental preview only.
- **CSS styling** with type, class, id, attribute, pseudo-state, pseudo-function
  (`:not()`, `:is()`, `:has()`, `:nth-child()`), and widget-part (`::thumb`,
  `::stepper`, `::badge`, etc.) selectors, custom variables, `@media`,
  `@supports`, `@font-face`, container queries, gradients, shadows, outlines,
  transitions, animations, and backdrop filters.
- **Reactive components** with keyed state slots, VNode diffing, and targeted
  native patches.
- **Live updates** from Python callbacks and background threads via a
  thread-safe command queue.
- **GPU scatter** — 3D point clouds with 500K+ points, orbit/pan/zoom, 12
  colormaps, scalar bar, categorical legend, orientation axes, 3D labels, line
  and box overlays, multi-actor layers, lasso/rectangle selection, hover
  tooltips, LOD, and real-time frame streaming.
- **Toast notifications**, file dialogs, native image display, and debug
  snapshots.

## Widget Catalog

| Category | Widgets |
| --- | --- |
| Layout & shell | `Window`, `Body`, `AppShell`, `HLayout`, `VLayout`, `FlexLayout`, `Panel`, `ScrollArea`, `GridLayout`, `FlowLayout`, `WorkbenchLayout`, `WorkbenchMain`, `Sidebar`, `StatusBar`, `Toolbar`, `ToolbarSeparator`, `Splitter`, `Pane`, `Separator`, `Spacer` |
| Navigation | `Tabs`, `Tab`, `Pages`, `Page`, `NavItem`, `MenuBar`, `Menu`, `MenuItem`, `ContextMenu`, `Breadcrumbs`, `CommandPalette`, `SearchBox` |
| Containers & overlays | `Collapsible`, `Modal`, `Tooltip`, `PropertyGrid`, `Property` |
| Buttons | `Button`, `SmallButton`, `IconButton`, `ImageButton`, `ArrowButton` |
| Text & code input | `TextInput`, `TextArea`, `NumberInput`, `DragNumber`, `CodeEditor`, `LogView` |
| Date & time | `DateInput`, `TimeInput`, `DateTimeInput` |
| Selection controls | `Slider`, `RangeSlider`, `Dropdown`, `Checkbox`, `RadioButton`, `RadioGroup`, `ToggleSwitch`, `Selectable`, `SelectableList`, `ColorPicker` |
| Display | `Label`, `Badge`, `Tag`, `LED`, `Image`, `ProgressBar`, `LimitsBar`, `LoadingSpinner` |
| Trees | `TreeView`, `TreeNode` |
| Charts & plots | `Scatter3D`, `ScatterPlot2D`, `LinePlot`, `Histogram`, `BarChart`, `Heatmap`, `PieChart` |
| Tables & reports | `DataFrameTable`, `HtmlReport` |
| Drag & drop | `DragSource`, `DropTarget`, `DropZone`, `DragVector` |
| Custom & advanced | `PaintWidget`, `ExtensionWidget`, `Terminal`; experimental `NodeGraph` |
| Overlays & utilities | `alert()`, `confirm()`, `toast()`, `FileDialog`, `Theme`, `ThreadMonitor` |

## 3D Scatter

`Scatter3D` renders large point clouds via wgpu with a wide API for interactive
data exploration.

**Creating a plot:**

```python
points = {"px": [0.0, 1.0], "py": [0.2, 0.8], "pz": [0.4, 0.6]}
scatter = dg.Scatter3D(
    points, x="px", y="py", z="pz",
    colormap="viridis",
    point_size=3.0,
    grid=True,
    on_pick=lambda hit: print(hit.index),
)
```

**Updating live:**

```python
new_points = {"px": [0.1, 1.1], "py": [0.3, 0.9], "pz": [0.5, 0.7]}
scatter.set_points(new_points, x="px", y="py", z="pz")
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
label_handle = scatter.add_label((0.0, 0.0, 0.0), "Origin", color=(1.0, 1.0, 0.0))
line_handle = scatter.add_lines(
    [[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]],
    color=(1.0, 0.5, 0.0),
)
box_handle = scatter.add_box(
    (-1.0, -1.0, -1.0, 1.0, 1.0, 1.0),  # xmin, ymin, zmin, xmax, ymax, zmax
    color=(0.3, 0.8, 1.0),
)
```

**Multiple independent point layers (actors):**

```python
layer = scatter.add_points(points_b, x="x", y="y", z="z", colormap="plasma")
scatter.update_actor(layer, points_new, x="x", y="y", z="z")
scatter.remove_actor(layer)
```

**Real-time frame streaming:**

```python
payloads = [
    scatter.prepare_points(frame, x="x", y="y", z="z")
    for frame in frames
]
stream = scatter.stream_prepared_frames(payloads, interval_ms=40, loop=True)
stream.start()
# ... later:
stream.stop()
print(stream.metrics)  # production, submission, callback, and error counters
```

**Selection and hover:**

```python
scatter = dg.Scatter3D(
    points,
    x="x",
    y="y",
    z="z",
    on_pick=lambda hit: print(hit.index, hit.point),
)
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

## Experimental APIs

`NodeGraph` and its runtime/binding helpers are incomplete previews. They are
exposed for development and internal probes, but are **not ready for usage in
applications or production code**. Their behavior, serialization, and public
API may change or be removed without compatibility guarantees. The built-in
manual repeats this warning on every NodeGraph-related entry.

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
    ProgressBar::fill  {
        background: linear-gradient(90deg, #38bdf8 0%, #8b5cf6 100%);
    }

    /* Effects: gradients, shadows, outlines */
    Panel.card        { background: linear-gradient(135deg, #1a1a2e, #0d0d1a); }
    Button:focus      { outline: 2px solid focus; outline-offset: 2px; }
    Modal             { box-shadow: 0 18px 48px rgba(0, 0, 0, 0.5); }

    /* Transitions and animations */
    Button            { transition: background 120ms ease, border-color 120ms; }
    Panel.busy        { animation: pulse 1.2s ease-in-out infinite; }

    /* At-rules: media, feature, and font queries + container queries */
    @media (max-width: 800px) { Panel.controls { display: none; } }
    @supports (backdrop-filter: blur(8px)) { Modal { backdrop-filter: blur(8px); } }
    @font-face { font-family: "Mono"; src: url("./mono.ttf"); }
    @container (min-width: 600px) { Panel.grid { grid-template-columns: 1fr 1fr; } }
""")
```

Supported pseudo-states: `:hover`, `:active`, `:focus`, `:checked`, `:disabled`,
`:selected`, `:expanded`, `:collapsed`, `:open`

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

## Layout Essentials

DragonGUI uses native flex/grid layout. For application shells, prefer
`AppShell` with `Body`, or `WorkbenchLayout` with `WorkbenchMain`, so fixed
chrome and flexible content receive explicit roles. Use `gap` for spacing
between siblings and `padding` for space inside a container.

Give each scrollable region one clear `ScrollArea` owner. Flexible descendants
that must shrink should use `min_width: 0` and/or `min_height: 0`; otherwise
their intrinsic content size can force overflow. The
[layout guide](./docs/layout.md) covers sizing, clipping, responsive grids, and
diagnostic patterns.

## Live Updates

```python
# From callbacks (main thread)
label = dg.Label("0.50")
slider = dg.Slider(0.5, on_change=lambda value: label.set_value(f"{value:.2f}"))

# From background threads
import threading
import time

def worker():
    for step in range(101):
        app.call_soon_threadsafe(
            lambda value=step / 100: progress.set_value(value),
            coalesce_key="readme.progress.latest",
        )
        time.sleep(0.05)

threading.Thread(target=worker, daemon=True).start()
```

Reuse a stable `coalesce_key` for replaceable snapshots so slow rendering skips
obsolete pending state. Omit the key for lossless events and append-only plot or
log streams, where every callback must remain FIFO.

Group related changes into one visual update when they must appear together:

```python
with app.update_batch():
    progress.set_value(0.72)
    limits.set_value(72.0)
    status.set_value("Nominal")
```

`LimitsBar` is designed for telemetry displays. The thresholds must remain in
ascending order; omitted thresholds default to 10%, 25%, 75%, and 90% of the
configured domain.

```python
limits = dg.LimitsBar(
    62.0,
    min=0.0,
    max=100.0,
    red_low=10.0,
    yellow_low=25.0,
    yellow_high=75.0,
    red_high=90.0,
)
limits.set_value(68.0)
```

**Toasts:**
```python
handle = app.toast("Loading...", level="info")
# ... later:
handle.update("Done!", level="success")
handle.dismiss()
```

Toast positions: `top-right`, `top-left`, `bottom-right`, `bottom-left`

## Built-in Help

The installed package includes a searchable manual generated and audited
against the public API:

```python
import dragongui as dg

print(dg.help())
print(dg.help.reference.widgets.limits_bar())
print(dg.help.search("scatter streaming"))
print(dg.help.find_symbol("NumberInput"))
```

Sections are callable (`dg.help("widgets.plots")`) and can also be exported as
structured data with `dg.help.to_dict()`. Treat the NodeGraph warnings in this
manual as authoritative.

## Repository Layout

```
python/dragongui/          Python package and built-in help manual
native/src/                Rust native extension (PyO3 + maturin)
tests/                     Python and integration tests
examples/                  Current demos and experimental NodeGraph probes
examples/css_feature_probes/  Focused, one-feature-per-file probes
examples/older/            Legacy single-feature demos and tools
plans/                     Implementation plans and roadmap
docs/                      Markdown and Sphinx documentation
```

The Rust native extension carries its own unit-test suite in addition to the
Python tests above.

## Development

Create and activate a virtual environment, then install the project in editable
mode. Building the native extension requires a Rust toolchain.

```powershell
py -3.12 -m venv .venv
.\.venv\Scripts\Activate.ps1
python -m pip install -e ".[dev]"
```

On macOS or Linux, activate with `source .venv/bin/activate` instead. Run a
current example after installation:

```bash
python examples/responsive_app_template.py
```

For an explicit native rebuild during development, run this **inside the active
virtual environment**:

```bash
python -m maturin develop --manifest-path native/Cargo.toml
```

Run tests:

```bash
python -m pytest
python -m pytest tests/test_python_api.py -k builtin_help
```

Build a release wheel for the current host:

```bash
python -m maturin build --release --manifest-path native/Cargo.toml --out dist
```

Confirm which backend the interpreter loaded with
`python -c "import dragongui as dg; print(dg.backend_info())"`.

## Examples

Run any current example after the editable install described in
[Development](#development).

**Flagship demos** (`examples/`):

| Example | What It Shows |
| --- | --- |
| `responsive_app_template.py` | Small modern starter using `AppShell`, responsive `Sidebar`, routed pages, adaptive grids, and explicit scrolling |
| `aurora_command_center_demo.py` | Sleek responsive command center used to stress layout, styling, diagnostics, and visual-audit states |
| `nexus_studio_stress_demo.py` | Five-workspace flagship stress demo with dense dashboards, plots, tables, workflows, controls, diagnostics, overlays, and responsive audit states |
| `cathode_ops_stress_demo.py` | Six-workspace vintage-CRT stress demo: scanline/phosphor CSS with three palettes, custom `PaintWidget` scope and core map, a load-bank page, and live worker-thread updates |
| `theme_forge_stress_demo.py` | Large multi-theme CSS, layout, paint, chrome, and live-update stress application |
| `client_window_chrome_demo.py` | Client-decorated window behavior and chrome styling |
| `icon_theme_demo.py` | Icon theme registration and widget icon rendering |
| `live_update_batch_demo.py` | Batched and coalesced live telemetry updates |
| `live_update_producer_safety_probe.py` | Producer-thread safety and queue-pressure behavior |

**Feature probes** (`examples/css_feature_probes/`) — 80+ focused,
one-feature-per-file programs used for visual verification. A sampling:

| Area | Probes |
| --- | --- |
| Widgets | `core_widgets_probe`, `form_controls_probe`, `limits_bar_probe`, `tree_view_probe`, `toggle_switch_probe`, `radio_group_probe`, `date_time_inputs_probe`, `command_palette_probe`, `breadcrumbs_probe`, `toolbar_probe` |
| Charts & data | `bar_chart_probe`, `histogram_probe`, `heatmap_probe`, `pie_chart_probe`, `line_plot_probe`, `scatter3d_probe`, `scatter_plot_2d_probe`, `data_table_upgrades_probe`, `html_report_probe` |
| CSS features | `gradients_probe`, `border_outline_shadow_probe`, `transitions_transforms_probe`, `animations_probe`, `backdrop_filter_probe`, `container_queries_probe`, `supports_probe`, `font_face_probe`, `selectors_probe`, `typography_probe` |
| Layout | `layout_containers_probe`, `layout_grid_masonry_probe`, `responsive_layout_probe`, `overflow_scrollbar_probe`, `positioning_zindex_probe`, `splitter_probe` |
| Custom drawing | `paint_widget_events_probe`, `paint_widget_sparkline_probe`, `custom_composite_widget_probe` |

**Legacy demos and tools** (`examples/older/`) — earlier demos and tools,
including `all_features_v3_demo.py`, `all_features_professional_demo.py`,
scatter/table tools, live-update tools, CSS theme galleries, component demos, a
terminal wrapper, the `meridian` application demo, and more.

**Experimental NodeGraph probes** (`examples/node_graph_*.py`) exercise
unfinished APIs for library development only. They are not application
templates; see [Experimental APIs](#experimental-apis).

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

- Python >= 3.12
- A Rust toolchain when building from source (not when installing a wheel)
- Windows, macOS, or Linux with a GPU backend supported by `wgpu`

CI runs Python 3.12 and 3.13 tests, Rust checks on Windows, macOS, and Linux,
supply-chain checks, and release metadata validation. The release workflow
builds and smoke-tests Windows x86-64, Linux x86-64, macOS Intel, and macOS
Apple Silicon wheels before publishing.

Optional extras: `pip install "dragongui[dataframe]"` for `pandas`/`polars`
DataFrame integration, `[terminal]` for the embedded terminal (`pywinpty` on
Windows), `[numpy]` for NumPy plotting paths, `[pandas]` or `[polars]` for one
dataframe engine, `[dev]` for build/test tooling, and `[docs]` for the Sphinx
docs.

## License

MIT. Third-party dependency notice policy and release-generation instructions
are included in [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md).

## Documentation

- [docs/widgets-reference.md](./docs/widgets-reference.md) — all widget options
- [docs/css-styling.md](./docs/css-styling.md) — selectors, parts, and variables
- [docs/css-capabilities-reference.md](./docs/css-capabilities-reference.md) — full CSS feature list
- [docs/layout.md](./docs/layout.md) — layout system and taffy integration
- [docs/library-overview.md](./docs/library-overview.md) — architecture deep-dive
- [docs/sphinx/index.md](./docs/sphinx/index.md) — structured user guide source
- [plans/](./plans/) — implementation roadmap
