# Quickstart

This is the smallest useful DragonGUI application shape:

```python
import dragongui as dg

app = dg.App()

with dg.Window("DragonGUI App", width=1000, height=700) as win:
    with dg.HLayout():
        with dg.Panel("Controls", width=280):
            name = dg.TextInput("Dataset A")
            dg.Button("Run")

        with dg.Panel("Output"):
            dg.Label("Ready")

app.run(win)
```

## Construction Model

Widgets are ordinary Python objects. Container context managers attach child
widgets to the current parent:

```python
with dg.Panel("Controls"):
    dg.Button("Apply")
    dg.Slider(0.5)
```

Pass `parent=None` when you need to construct a widget without attaching it to
the current context.

## Live Updates

After `app.run(...)` binds the native runtime, use widget live methods for
updates instead of rebuilding large trees:

```python
progress = dg.ProgressBar(0.0)

def on_step(value: float) -> None:
    progress.set_value(value)
```

For worker threads, prefer `app.call_soon_threadsafe(...)` or documented
thread-safe enqueue methods.

