# Python API And Update Protocol Plan

## Objective

Keep Python ergonomic and small:

- Python describes the initial app structure.
- Rust runs the UI.
- Python callbacks and state updates use handles and a command queue.
- Python never renders per frame.

## API Principles

- Keep the public import flat: `import dragongui as dg`.
- Context managers build startup layout trees.
- Widgets have stable ids.
- User callbacks are normal Python callables.
- Background thread updates go through an explicit queue API.
- Data widgets accept familiar Python objects directly.
- Theme is token-based, not a CSS engine.

## Near-Term Public API

```python
app = dg.App(theme=dg.Theme.dark(accent="#4ea1ff"))
win = dg.Window("Tool", width=1200, height=800)

with dg.HLayout():
    with dg.Panel("Controls", width=280):
        col = dg.Dropdown(items=df.columns)
        dg.Button("Plot", on_click=lambda: scatter.set_points(df, x=col.value))

    scatter = dg.Scatter3D(df, x="x", y="y", z="z")

app.run(win)
```

## Startup Document Schema

The Rust backend should receive a typed startup document with this shape:

```python
{
    "schema": 1,
    "type": "app",
    "title": "DragonGUI",
    "theme": {"mode": "dark", "accent": "#4ea1ff"},
    "window": {
        "id": "dg-1",
        "type": "window",
        "props": {"title": "Tool", "width": 1200, "height": 800},
        "children": [...]
    }
}
```

This document is for startup and structural initialization. It must not become
the runtime state update mechanism.

## Runtime Update Protocol

Add explicit commands before callbacks and background updates become complex:

```python
scatter.set_points(df, x="x", y="y", z="z")
app.call_soon_threadsafe(lambda: scatter.set_points(next_df, x="x", y="y", z="z"))
app.set_theme(dg.Theme.light(accent="#006adc"))
```

Internally these should become targeted commands:

```python
{
    "schema": 1,
    "target": "dg-7",
    "op": "scatter.set_points",
    "payload": {"source": "...", "x": "x", "y": "y", "z": "z"}
}
```

Initial command types:

- `widget.set_prop`
- `scatter.set_points`
- `scatter.set_camera`
- `theme.set`
- `app.stop`
- `callback.invoke`

Design constraints:

- Do not reserialize the whole widget tree for `set_points` or simple prop
  updates.
- Commands must be safe to enqueue from background Python threads.
- The Rust UI thread is the only owner of live UI state.
- Python widget objects are handles after `app.run()` starts, not the canonical
  renderer state.

## API Work Items

- Add explicit callback registry instead of serializing callback existence only.
- Add `WidgetHandle` semantics for Rust-owned widgets after app start.
- Add `app.call_soon_threadsafe(fn)` for background-thread UI updates.
- Add `app.stop()` for controlled shutdown.
- Add `StateVar[T]` only after the command queue exists.
- Add validation errors for invalid layout nesting and duplicate ids.
- Add stable document and command schema versioning.
- Add `Theme` and theme serialization.

## Theme API

First version:

```python
theme = dg.Theme(
    mode="dark",
    background="#101214",
    surface="#181b1f",
    text="#eef1f4",
    muted_text="#9aa4ad",
    accent="#4ea1ff",
    border="#30363d",
    radius=6,
    spacing=8,
    font_size=13,
)
```

Convenience constructors:

- `dg.Theme.dark()`
- `dg.Theme.light()`

Keep the theme deliberately small. Retrofitting theme later would touch every
widget, so the API should exist by M3 even if only primitive colors are used.

## Acceptance Criteria

- Existing examples keep working.
- A missing native backend still supports dev fallback.
- Duplicate widget ids fail early in Python.
- Callback registration can be inspected in tests without opening a window.
- Python API remains importable without optional pandas, Polars, NumPy, or
  DragonSci.
- Runtime mutations use commands rather than full document replacement.
