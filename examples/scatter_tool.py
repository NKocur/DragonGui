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
    """500k-point demo data set with extra metadata for widget/table testing.

    When NumPy is available, actual float32 xyz arrays are generated and
    passed to the Rust renderer via the scatter document.  Without NumPy,
    the columns exist only as metadata and the Rust side falls back to its
    own deterministic LCG generator.
    """

    columns = ("x", "y", "z", "signal", "score", "phase", "row_id", "enabled")
    dtypes = ("float32", "float32", "float32", "float32", "float32", "float32", "int64", "bool")
    shape = (500_000, len(columns))

    def __init__(self) -> None:
        if _NUMPY:
            rng = np.random.default_rng(42)  # deterministic seed
            data = (rng.standard_normal((500_000, 6)) * 3.0).astype(np.float32)
            self.x = data[:, 0]
            self.y = data[:, 1]
            self.z = data[:, 2]
            self.signal = data[:, 3]
            self.score = data[:, 4]
            self.phase = data[:, 5]
            self.row_id = np.arange(500_000, dtype=np.int64)
            self.enabled = (self.row_id % 2) == 0


df = DemoFrame()

app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33"))
win = dg.Window("DragonGUI Scatter Demo", width=1200, height=800)


def log_event(name: str, value: object) -> None:
    print(f"{name}: {value}", flush=True)


def make_slider_logger(step: float = 0.05):
    last_bucket: int | None = None

    def log_slider(value: float) -> None:
        nonlocal last_bucket
        bucket = round(value / step)
        if bucket != last_bucket:
            last_bucket = bucket
            log_event("slider", round(bucket * step, 3))

    return log_slider


def redacted_document(doc: dict) -> dict:
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


with dg.HLayout():
    with dg.Panel("Controls", width=340):
        dg.Label("Widget checks")
        x_col = dg.Dropdown(
            items=("x", "signal", "score", "phase"),
            value="x",
            on_change=lambda value: log_event("x column", value),
        )
        y_col = dg.Dropdown(
            items=("y", "signal", "score", "phase"),
            value="y",
            on_change=lambda value: log_event("y column", value),
        )
        z_col = dg.Dropdown(
            items=("z", "signal", "score", "phase"),
            value="z",
            on_change=lambda value: log_event("z column", value),
        )

        def replot_selected() -> None:
            x = x_col.value or "x"
            y = y_col.value or "y"
            z = z_col.value or "z"
            log_event("plot selected xyz", f"{x}, {y}, {z}")
            scatter.set_points(df, x=x, y=y, z=z)

        dg.Button(
            "Plot selected XYZ",
            on_click=replot_selected,
        )
        dg.Checkbox(
            "Use GPU point sprites",
            checked=True,
            on_change=lambda checked: log_event("point sprites", checked),
        )
        dg.TextInput(
            "edit me",
            placeholder="type here",
            on_change=lambda value: log_event("text input", value),
        )
        dg.Slider(
            0.55,
            min=0.0,
            max=1.0,
            step=0.05,
            on_change=make_slider_logger(),
        )
        dg.Dropdown(
            items=("Viridis", "Magma", "Plasma", "Cividis"),
            value="Viridis",
            on_change=lambda value: log_event("palette", value),
        )
        dg.Button("Disabled button", disabled=True)
        dg.TextInput("disabled text", disabled=True)
        dg.Checkbox("Disabled checkbox", checked=True, disabled=True)

    with dg.VLayout():
        scatter = dg.Scatter3D(df, x="x", y="y", z="z")
        dg.Label("DataFrameTable: wheel scroll, Shift+wheel columns, click headers/cells")
        dg.DataFrameTable(df, page_size=80)

try:
    result = app.run(win)
except dg.BackendUnavailableError:
    print("DragonGUI source import works.")
    print("Native backend is not built yet, so this run prints the UI document instead.")
    print("Build the backend later with: python -m maturin build --target x86_64-pc-windows-gnu")
    pprint(redacted_document(app.document(win)))
else:
    if result.get("renderer") == "dev-fallback":
        print("DragonGUI dev fallback is active.")
        print("Native backend is not built yet, so this run prints the UI document instead.")
        pprint(redacted_document(result["document"]))
