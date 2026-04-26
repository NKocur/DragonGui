"""Runnable example: nested components preserve local DataFrame UI state."""

from __future__ import annotations

import math
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual example guard
    raise SystemExit("This example requires NumPy") from exc


ROWS = 60_000


class DemoFrame:
    columns = ("x", "y", "z", "signal", "score", "phase")
    dtypes = ("float32", "float32", "float32", "float32", "float32", "float32")
    shape = (ROWS, len(columns))

    def __init__(self) -> None:
        t = np.linspace(0.0, 1.0, ROWS, dtype=np.float32)
        theta = t * np.float32(math.tau * 10.0)
        self.x = (t - np.float32(0.5)) * np.float32(8.0)
        self.y = np.sin(theta) * np.float32(3.0)
        self.z = np.cos(theta * np.float32(0.5)) * np.float32(3.0)
        self.signal = np.sin(theta * np.float32(0.25)).astype(np.float32)
        self.score = np.cos(theta * np.float32(0.125)).astype(np.float32)
        self.phase = theta.astype(np.float32)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


@dg.component
def ColumnPicker(ctx: dg.ComponentCtx, frame: DemoFrame, axis: str, default: str) -> dg.Panel:
    selected = ctx.state("selected", default)

    panel = dg.Panel(
        f"{axis.upper()} axis",
        id=f"{axis}-picker-panel",
        key=f"{axis}-picker-panel",
        style={"padding": 10, "gap": 8},
        parent=None,
    )
    dg.Label(f"Local state: {selected.value}", id=f"{axis}-label", key=f"{axis}-label", parent=panel)
    dg.Dropdown(
        frame.columns,
        value=str(selected.value),
        on_change=selected.set,
        id=f"{axis}-dropdown",
        key=f"{axis}-dropdown",
        parent=panel,
    )
    return panel


@dg.component
def PlotPage(ctx: dg.ComponentCtx, frame: DemoFrame, tone: str) -> dg.VLayout:
    show_table = ctx.state("show_table", True)

    root = dg.VLayout(id="plot-page", key="plot-page", style={"gap": 10}, parent=None)
    with dg.HLayout(parent=root, style={"gap": 10}):
        ColumnPicker(frame, "x", "x", key="x-picker")
        ColumnPicker(frame, "y", "y", key="y-picker")
        ColumnPicker(frame, "z", "z", key="z-picker")
    dg.Checkbox(
        "Show table",
        checked=bool(show_table.value),
        on_change=show_table.set,
        id="show-table",
        key="show-table",
        parent=root,
    )
    with dg.HLayout(parent=root, style={"gap": 10, "flex": 1}):
        dg.Scatter3D(
            frame,
            x="x",
            y="y",
            z="z",
            id="nested-scatter",
            key="nested-scatter",
            style={"flex": 2, "border_color": "success" if tone == "green" else "#3fc7ff"},
        )
        if show_table.value:
            dg.DataFrameTable(frame, id="nested-table", key="nested-table", page_size=80, style={"flex": 1})
        else:
            dg.Spacer(id="table-spacer", key="table-spacer", style={"flex": 1})
    return root


@dg.component
def AppShell(ctx: dg.ComponentCtx, frame: DemoFrame) -> dg.Window:
    page = ctx.state("page", "plot")
    tone = ctx.state("tone", "blue")

    def toggle_page() -> None:
        page.set("data" if page.value == "plot" else "plot")

    def toggle_tone() -> None:
        tone.set("green" if tone.value == "blue" else "blue")

    win = dg.Window("Nested DataFrame Components", width=1200, height=760, id="nested-window", key="nested-window")
    with dg.HLayout(parent=win, style={"padding": 14, "gap": 16}):
        with dg.Panel("Shell state", width=260, style={"padding": 14, "gap": 10}):
            dg.Label(f"Parent page: {page.value}", id="page-label", key="page-label")
            dg.Label(f"Parent tone: {tone.value}", id="tone-label", key="tone-label")
            dg.Button("Toggle Page", id="toggle-page", key="toggle-page", on_click=toggle_page)
            dg.Button("Toggle Parent Tone", id="toggle-tone", key="toggle-tone", on_click=toggle_tone)
        if page.value == "plot":
            PlotPage(frame, str(tone.value), key="plot-page-component")
        else:
            dg.DataFrameTable(frame, id="data-page-table", key="data-page-table", page_size=100, style={"flex": 1})
    return win


def main() -> None:
    app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33"))
    print(app.run(AppShell(DemoFrame())))


if __name__ == "__main__":
    main()
