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
- **30+ widgets** including panels, buttons, text inputs, sliders, dropdowns,
  checkboxes, number inputs, progress bars, menus, modals, tabs, pages,
  navigation, color pickers, images, scatter plots, and virtualized tables.
- **CSS styling** with type, class, id, direct-child, and pseudo-state
  selectors, custom variables, text inheritance, and framework defaults.
- **Reactive components** with keyed state, VNode diffing, and targeted native
  patches.
- **Live updates** from Python callbacks and background threads through a
  thread-safe command queue.
- **GPU scatter** rendering 500K+ points with orbit, pan, zoom, and colormaps.
- **Virtualized DataFrame tables** with column buffers, scrolling, selection,
  and sorting.
- **File dialogs**, native image display, and debug snapshots.

## Widget Catalog

| Category | Widgets |
| --- | --- |
| Layout | `Window`, `HLayout`, `VLayout`, `Panel`, `Sidebar`, `StatusBar`, `Separator`, `Spacer` |
| Navigation | `Tabs`, `Tab`, `Pages`, `Page`, `NavItem`, `MenuBar`, `Menu`, `MenuItem`, `ContextMenu` |
| Controls | `Label`, `Button`, `TextInput`, `NumberInput`, `Slider`, `Dropdown`, `Checkbox`, `ProgressBar`, `ColorPicker` |
| Overlays | `Modal`, `alert()`, `confirm()` |
| Data/GPU | `Scatter3D`, `DataFrameTable`, `Image` |
| Utilities | `FileDialog`, `Theme` |

## Repository Layout

```
python/dragongui/    Python package
native/src/          Rust native extension (PyO3 + maturin)
tests/               82 tests across API, components, and VDOM
examples/            19 runnable demos
plans/               Implementation plans and V2 roadmap
docs/                Library overview and CSS documentation
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
| `scatter_tool.py` | Full scatter + table + controls demo |
| `all_features_demo.py` | Every widget type on one screen |
| `css_design_system_demo.py` | 4 CSS themes on the same widget tree |
| `css_showcase.py` | CSS selectors, variables, and pseudo-states |
| `streaming_scatter_tool.py` | Background thread pushing live scatter data |
| `live_update_tool.py` | Live prop updates from callbacks |
| `live_table_tool.py` | Live DataFrame table replacement |
| `component_counter_tool.py` | Reactive component with keyed state |
| `component_nested_tool.py` | Nested components with state isolation |
| `multipage_tool.py` | Sidebar, pages, tabs, and status bar |
| `debug_snapshot_tool.py` | Runtime introspection while running |

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
| `taffy` | Flexbox layout |
| `glyphon` | Text shaping and rendering |
| `lightningcss` | CSS parsing |
| `rfd` | Native file dialogs |
| `image` | PNG/JPEG decoding |

## Requirements

- Python >= 3.11
- Rust toolchain (for building the native extension)
- Windows, macOS, or Linux with GPU support

Optional: `pandas` or `polars` for DataFrame integration.

## License

MIT. Third-party dependency notices are included in
[THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md).

## Plans

Detailed plans live in [plans/](./plans/). Active V2 work includes the
[CSS styling system](./plans/V2/css-styling-system.md) and
[widget parts](./plans/V2/css-styling-system-part-2-widget-parts.md).
