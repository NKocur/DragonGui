from __future__ import annotations

import math
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual demo guard
    raise SystemExit("live_table_tool.py requires NumPy") from exc


ROWS = 20_000


class TableFrame:
    columns = ("row", "x", "signal", "score", "label")
    dtypes = ("int64", "float32", "float32", "float32", "str")
    shape = (ROWS, len(columns))

    def __init__(self, phase: float, label: str) -> None:
        row = np.arange(ROWS)
        t = np.linspace(0.0, 1.0, ROWS, dtype=np.float32)
        theta = t * np.float32(math.tau * 10.0 + phase)
        self.row = row
        self.x = (t - np.float32(0.5)) * np.float32(10.0)
        self.signal = np.sin(theta).astype(np.float32)
        self.score = np.cos(theta * np.float32(0.35)).astype(np.float32)
        self.label = np.array([f"{label}-{idx % 8}" for idx in range(ROWS)], dtype=object)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33"))
win = dg.Window("DragonGUI Live Table Demo", width=1100, height=720)

state = {"generation": 0}


def update_table(label: str) -> None:
    state["generation"] += 1
    phase = state["generation"] * 0.45
    table.set_frame(TableFrame(phase, label), sample_rows=2048)
    status.set_value(f"{label} generation {state['generation']}")


def upload_demo_buffer() -> None:
    payload = np.arange(256, dtype=np.uint8)
    app.set_buffer_resource("demo-buffer", payload, kind="uint8-demo")
    status.set_value("uploaded demo-buffer resource")


def release_demo_buffer() -> None:
    app.release_resource("demo-buffer")
    status.set_value("released demo-buffer resource")


with dg.HLayout(style={"padding": 14, "gap": 16}):
    with dg.Panel("Table controls", width=300, style={"padding": 14, "gap": 10}):
        status = dg.TextInput("ready", placeholder="status", key="status")
        dg.Button("Load Alpha", on_click=lambda: update_table("alpha"))
        dg.Button("Load Beta", on_click=lambda: update_table("beta"))
        dg.Button("Load Gamma", on_click=lambda: update_table("gamma"))
        dg.Separator()
        dg.Button("Upload Buffer Resource", on_click=upload_demo_buffer)
        dg.Button("Release Buffer Resource", on_click=release_demo_buffer)
        dg.Label("In-place table updates.")
        dg.Label("Scroll past row 2048.")
        dg.Label("Retained table resource.")
        dg.Label("Buffers appear in snapshots.")

    table = dg.DataFrameTable(TableFrame(0.0, "initial"), id="live-table", key="live-table", page_size=80)


if __name__ == "__main__":
    print(app.run(win))
