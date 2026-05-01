from __future__ import annotations

import math
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError:  # pragma: no cover - manual visual probe fallback
    np = None


class ProbeFrame:
    columns = ("x", "y", "z", "score", "phase", "segment", "enabled")
    dtypes = ("float32", "float32", "float32", "float32", "float32", "str", "bool")

    def __init__(self, rows: int = 1200) -> None:
        self.shape = (rows, len(self.columns))
        if np is not None:
            t = np.linspace(0.0, 1.0, rows, dtype=np.float32)
            theta = t * np.float32(math.tau * 5.5)
            radius = np.float32(0.35) + t * np.float32(2.6)
            wave = np.sin(theta * np.float32(0.64)).astype(np.float32)
            self.x = np.cos(theta) * radius
            self.y = np.sin(theta) * radius
            self.z = (t - np.float32(0.5)) * np.float32(2.2)
            self.score = wave
            self.phase = t.astype(np.float32)
            self.segment = np.where(t > 0.66, "outer", np.where(t > 0.33, "middle", "inner"))
            self.enabled = (np.arange(rows) % 3) != 0
        else:
            t_values = [i / max(1, rows - 1) for i in range(rows)]
            self.x = [math.cos(t * math.tau * 5.5) * (0.35 + t * 2.6) for t in t_values]
            self.y = [math.sin(t * math.tau * 5.5) * (0.35 + t * 2.6) for t in t_values]
            self.z = [(t - 0.5) * 2.2 for t in t_values]
            self.score = [math.sin(t * math.tau * 5.5 * 0.64) for t in t_values]
            self.phase = t_values
            self.segment = ["outer" if t > 0.66 else "middle" if t > 0.33 else "inner" for t in t_values]
            self.enabled = [(i % 3) != 0 for i in range(rows)]

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


frame = ProbeFrame()

app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #0c111d;
        color: rgba(246, 249, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
        overflow-y: auto;
        padding-right: 24px;
    }

    VLayout.root::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.72);
        border-radius: 999px;
    }

    HLayout.grid {
        gap: 12px;
        height: auto;
    }

    Panel {
        background:
            radial-gradient(circle at 12% 18%, rgba(90, 169, 255, 0.12), transparent 54%),
            rgba(18, 25, 40, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 14px;
        padding: 14px;
        gap: 10px;
        box-shadow: 0 12px 30px rgba(0, 0, 0, 0.24);
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(246, 249, 255, 0.72);
        line-height: 1.12;
    }

    Label.status {
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 10px;
        color: rgba(229, 255, 244, 0.95);
        font-weight: 750;
        padding: 8px 10px;
        width: 100%;
    }

    Label.case-title {
        color: white;
        font-weight: 850;
    }

    Label.pass {
        color: #74ddb0;
        font-weight: 800;
    }

    DataFrameTable.probe-table {
        height: 285px;
        background: rgba(3, 8, 18, 0.44);
        border: 1px solid rgba(90, 169, 255, 0.32);
        border-radius: 13px;
        color: rgba(232, 240, 255, 0.86);
        font-size: 12px;
        table-row-height: 27px;
        table-header-height: 36px;
        table-column-width: 124px;
        table-index-width: 62px;
    }

    DataFrameTable.probe-table::header {
        background:
            linear-gradient(180deg, rgba(90, 169, 255, 0.28), rgba(90, 169, 255, 0.13));
        color: white;
        font-weight: 850;
        text-transform: uppercase;
        letter-spacing: 0.07em;
    }

    DataFrameTable.probe-table::row {
        background: rgba(255, 255, 255, 0.025);
    }

    DataFrameTable.probe-table::row-selected {
        background: rgba(255, 211, 106, 0.24);
        color: white;
    }

    DataFrameTable.probe-table::grid-line {
        background: rgba(255, 255, 255, 0.11);
    }

    Panel.scatter-case {
        width: calc(50% - 6px);
        min-width: 360px;
        height: 278px;
        padding: 12px;
        gap: 8px;
        box-shadow: none;
        overflow: hidden;
    }

    Scatter3D {
        height: 188px;
        background:
            radial-gradient(circle at 18% 22%, rgba(116, 221, 176, 0.16), transparent 56%),
            rgba(3, 8, 18, 0.50);
        border: 1px solid rgba(116, 221, 176, 0.30);
        border-radius: 14px;
    }

    Scatter3D.small-points {
        scatter-point-size: 4px;
    }

    Scatter3D.large-points {
        scatter-point-size: 8px;
    }
    """
)

win = dg.Window("CSS Data Widgets Probe", width=1000, height=760)

with dg.VLayout(class_="root"):
    dg.Label("Data widgets", class_="title")
    dg.Label(
        "This probe checks DataFrameTable parts and Scatter3D renderer metrics, "
        "including selection/pick callbacks, rounded clipping, and point-size styling.",
        class_="caption",
    )

    status = dg.Label("Click a table cell or pick a scatter point.", class_="status")

    def show_selection(selection: dg.TableSelection) -> None:
        status.set_value(
            f"Table selection: row {selection.row_index}, {selection.column} = {selection.value}"
        )

    def show_pick(point: dg.ScatterPick) -> None:
        status.set_value(
            f"Scatter pick: point {point.index} at ({point.x:.2f}, {point.y:.2f}, {point.z:.2f})"
        )

    with dg.Panel("DataFrameTable styling and selection"):
        dg.Label("Header, row, selected-row, and grid-line parts should all be visibly styled.", class_="case-title")
        dg.DataFrameTable(
            frame,
            page_size=48,
            sample_rows=32,
            on_select=show_selection,
            class_="probe-table",
        )
        dg.Label("PASS: selected cells update the status label and use the selected-row style.", class_="pass")

    with dg.HLayout(class_="grid"):
        with dg.Panel("Scatter3D compact points", class_="scatter-case"):
            dg.Label("Small viridis points", class_="case-title")
            dg.Scatter3D(
                frame,
                x="x",
                y="y",
                z="z",
                colormap="viridis",
                on_pick=show_pick,
                class_="small-points",
            )
            dg.Label("PASS: plot clips cleanly to the rounded panel.", class_="pass")

        with dg.Panel("Scatter3D large points", class_="scatter-case"):
            dg.Label("Large magma points", class_="case-title")
            dg.Scatter3D(
                frame,
                x="x",
                y="y",
                z="z",
                colormap="magma",
                on_pick=show_pick,
                class_="large-points",
            )
            dg.Label("PASS: point size and colormap differ from the left plot.", class_="pass")


if __name__ == "__main__":
    print(app.run(win))
