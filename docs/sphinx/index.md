# DragonGUI Documentation

DragonGUI is a Python application toolkit for GPU-native data tools. These docs
are the longer-form companion to the in-runtime `dragongui.help` manual.

```{toctree}
:maxdepth: 2
:caption: User Guide

quickstart
layout
widgets
styling
plots
live-updates
performance
troubleshooting
```

```{toctree}
:maxdepth: 2
:caption: API Reference

api/index
```

```{toctree}
:maxdepth: 1
:caption: Project Notes

context-depth-audit
notes
```

## Local Help

DragonGUI also ships a compact built-in manual:

```python
import dragongui as dg

print(dg.help())
print(dg.help("widgets.plots"))
print(dg.help.search("scatter streaming"))
```

Use the Sphinx docs for the full guide and API reference. Use `dg.help` when an
agent or running application needs concise, structured context at runtime.
