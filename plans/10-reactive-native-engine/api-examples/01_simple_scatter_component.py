"""Runnable example: one component controls a live scatter widget."""

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
    raise SystemExit("This example requires NumPy for scatter point packing") from exc


ROWS = 180_000


class DemoFrame:
    columns = ("x", "y", "z", "signal", "score", "phase")
    dtypes = ("float32", "float32", "float32", "float32", "float32", "float32")
    shape = (ROWS, len(columns))

    def __init__(self, phase: float = 0.0) -> None:
        t = np.linspace(0.0, 1.0, ROWS, dtype=np.float32)
        theta = t * np.float32(math.tau * 14.0 + phase)
        radius = np.float32(1.0) + np.float32(3.5) * t
        self.x = np.cos(theta) * radius
        self.y = np.sin(theta) * radius
        self.z = (t - np.float32(0.5)) * np.float32(8.0)
        self.signal = np.sin(theta * np.float32(0.5)).astype(np.float32)
        self.score = np.cos(theta * np.float32(0.25)).astype(np.float32)
        self.phase = np.full(ROWS, np.float32(phase), dtype=np.float32)


@dg.component
def ScatterTool(ctx: dg.ComponentCtx, frame: DemoFrame) -> dg.Window:
    x = ctx.state("x", "x")
    y = ctx.state("y", "y")
    z = ctx.state("z", "z")
    generation = ctx.state("generation", 0)

    def replot() -> None:
        next_generation = int(generation.value) + 1
        generation.set(next_generation)
        scatter.set_points(
            DemoFrame(phase=next_generation * 0.35),
            x=str(x.value),
            y=str(y.value),
            z=str(z.value),
        )

    win = dg.Window("Reactive Scatter Component", width=1100, height=720, id="scatter-window", key="scatter-window")
    with dg.HLayout(parent=win, style={"padding": 14, "gap": 16}):
        with dg.Panel("Controls", width=320, style={"padding": 14, "gap": 10}):
            dg.Dropdown(frame.columns, value=str(x.value), on_change=x.set, id="x-axis", key="x-axis")
            dg.Dropdown(frame.columns, value=str(y.value), on_change=y.set, id="y-axis", key="y-axis")
            dg.Dropdown(frame.columns, value=str(z.value), on_change=z.set, id="z-axis", key="z-axis")
            dg.Button("Plot Selected Columns", id="plot", key="plot", on_click=replot)
            dg.Label(f"Generation {generation.value}", id="generation", key="generation")

        scatter = dg.Scatter3D(
            frame,
            x=str(x.value),
            y=str(y.value),
            z=str(z.value),
            id="main-scatter",
            key="main-scatter",
            style={"flex": 1, "border_color": "border", "border_width": 1},
        )

    return win


def main() -> None:
    app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33"))
    print(app.run(ScatterTool(DemoFrame())))


if __name__ == "__main__":
    main()
