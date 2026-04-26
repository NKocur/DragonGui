# DragonGUI

DragonGUI is a Python application toolkit for GPU-native data tools.

The target position is simple: a Dear PyGui-style workflow with a modern Rust
backend built for `wgpu`, first-class DragonSci widgets, and direct DataFrame
integration.

This repository is organized as a PyPI-ready `maturin` package:

- Python API: `python/dragongui`
- Rust native extension: `native/src/lib.rs`
- Package metadata: `pyproject.toml`
- Rust metadata: `native/Cargo.toml`
- Tests: `tests`
- Examples: `examples`

## API Shape

```python
import dragongui as dg

app = dg.App()
win = dg.Window("My Tool", width=1200, height=800)

with dg.HLayout():
    with dg.Panel():
        col = dg.Dropdown(items=["x", "y", "z"])
        dg.Button("Plot", on_click=lambda: scatter.set_points(df, x=col.value))

    scatter = dg.Scatter3D(df, x="x", y="y", z="z")

app.run(win)
```

Python describes structure and handles callbacks. Layout, rendering, event
dispatch, and high-volume widget loops are owned by Rust.

## Development

Run the example from the source tree without installing the package:

```powershell
.\start.bat
```

The launcher sets `PYTHONPATH` to `python/` and enables
`DRAGONGUI_DEV_FALLBACK=1`, so examples can serialize their UI document before
the native backend exists.

Install the package in editable mode:

```bash
python -m pip install -e ".[dev]"
```

Build the native extension:

```bash
python -m maturin develop --manifest-path native/Cargo.toml
```

Run tests:

```bash
python -m pytest
```

Build a wheel (Windows/MSYS2, x86_64):

```bash
python -m maturin build --release --manifest-path native/Cargo.toml --out dist --target x86_64-pc-windows-gnu
```

Build a source distribution:

```bash
python -m maturin sdist --manifest-path native/Cargo.toml --out dist
```

## Roadmap

Detailed implementation plans live in [plans/](./plans/README.md).

1. Native `winit` window and `wgpu` clear.
2. DragonSci scatter embedded in that window.
3. Layout and primitive drawing without text.
4. Events, callbacks, command updates, and text rendering.
5. Full basic widget set and token-based theming.
6. Virtualized DataFrame table for pandas and Polars.
7. Packaging, docs, CI, and Dear PyGui benchmarks.
