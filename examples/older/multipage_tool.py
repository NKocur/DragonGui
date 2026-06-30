from __future__ import annotations

import sys
from pathlib import Path
from pprint import pprint

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np

    _NUMPY = True
except ImportError:
    _NUMPY = False


class DemoFrame:
    columns = ("x", "y", "z", "signal", "row_id")
    dtypes = ("float32", "float32", "float32", "float32", "int64")
    shape = (100_000, len(columns))

    def __init__(self) -> None:
        if _NUMPY:
            t = np.linspace(0.0, 16.0, self.shape[0], dtype=np.float32)
            self.x = np.sin(t) * 4.0
            self.y = np.cos(t * 0.7) * 4.0
            self.z = np.sin(t * 1.3) * np.cos(t * 0.2) * 4.0
            self.signal = np.sin(t * 2.0).astype(np.float32)
            self.row_id = np.arange(self.shape[0], dtype=np.int64)


def log(name: str, value: object) -> None:
    print(f"{name}: {value}", flush=True)


def redacted_document(doc: object) -> object:
    if isinstance(doc, dict):
        return {
            key: "<packed float32 xyz data>"
            if key == "data_b64" and value is not None
            else "<sampled table cells>"
            if key == "cells" and value
            else redacted_document(value)
            for key, value in doc.items()
        }
    if isinstance(doc, list):
        return [redacted_document(item) for item in doc]
    return doc


df = DemoFrame()
app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33"))
win = dg.Window("DragonGUI Multipage Demo", width=1280, height=820)

with dg.HLayout():
    with dg.Sidebar(width=220):
        dg.Label("Workspace")
        with dg.HLayout(style={"gap": 6, "height": 24}):
            dg.Badge("live", level="success")
            dg.Tag("v2", level="info")
        dg.Separator()
        dg.NavItem("Explore", page="explore", badge="3")
        dg.NavItem("Data", page="data", badge=12)
        dg.NavItem("Settings", page="settings")
        dg.Spacer()
        dg.Separator()
        dg.Label("DragonGUI M8")

    with dg.Pages(value="explore", on_change=lambda value: log("page", value)):
        with dg.Page("explore", title="Explore"):
            with dg.Tabs(value="scatter", on_change=lambda value: log("tab", value)):
                with dg.Tab("Scatter", value="scatter", badge="gpu"):
                    dg.Scatter3D(
                        df,
                        x="x",
                        y="y",
                        z="z",
                        on_pick=lambda point: log("point", point.index),
                    )
                with dg.Tab("Table", value="table"):
                    dg.DataFrameTable(
                        df,
                        page_size=80,
                        on_select=lambda selection: log(
                            "table",
                            f"{selection.row_index}:{selection.column}={selection.value}",
                        ),
                    )
                with dg.Tab("Controls", value="controls"):
                    dg.Label("Tab state survives switching")
                    dg.Separator()
                    dg.TextInput("editable value", placeholder="type here")
                    dg.Checkbox("Enable GPU points", checked=True)
                    dg.Button("Show Toast", on_click=lambda: dg.toast("Multipage toast", level="success"))
                    dg.Spacer(height=8)
                    dg.Slider(0.35, min=0, max=1, step=0.05)

        with dg.Page("data", title="Data"):
            dg.Label("Full-page table")
            dg.Separator()
            dg.DataFrameTable(
                df,
                page_size=100,
                on_select=lambda row, column, value: log(
                    "table", f"{row}:{column}={value}"
                ),
            )

        with dg.Page("settings", title="Settings"):
            dg.Label("Application settings")
            dg.Separator()
            dg.Dropdown(("Viridis", "Magma", "Plasma", "Cividis"), value="Viridis")
            dg.Checkbox("Show diagnostics", checked=False)
            dg.TextInput("", placeholder="Project note")
            with dg.Collapsible("Advanced note", expanded=False):
                dg.TextArea("Multiline\nruntime\nsettings", rows=3)

with dg.StatusBar():
    dg.Label("Ready")
    dg.Spacer()
    dg.Label(f"{df.shape[0]:,} rows")
    dg.Separator(orientation="vertical")
    dg.Label("W0 widgets")

try:
    result = app.run(win)
except dg.BackendUnavailableError:
    print("DragonGUI source import works.")
    print("Native backend is not built yet, so this run prints the UI document instead.")
    pprint(redacted_document(app.document(win)))
else:
    if result.get("renderer") == "dev-fallback":
        print("DragonGUI dev fallback is active.")
        pprint(redacted_document(result["document"]))
