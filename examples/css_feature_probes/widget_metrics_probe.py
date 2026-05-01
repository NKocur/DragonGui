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
    columns = ("x", "y", "z", "score", "phase", "segment")
    dtypes = ("float32", "float32", "float32", "float32", "float32", "str")

    def __init__(self, rows: int = 900) -> None:
        self.shape = (rows, len(self.columns))
        if np is not None:
            t = np.linspace(0.0, 1.0, rows, dtype=np.float32)
            theta = t * np.float32(math.tau * 4.5)
            radius = np.float32(0.45) + t * np.float32(2.4)
            self.x = np.cos(theta) * radius
            self.y = np.sin(theta) * radius
            self.z = (t - np.float32(0.5)) * np.float32(2.4)
            self.score = np.sin(theta * np.float32(0.72)).astype(np.float32)
            self.phase = t.astype(np.float32)
            self.segment = np.where(t > 0.66, "outer", np.where(t > 0.33, "middle", "inner"))
        else:
            t_values = [i / max(1, rows - 1) for i in range(rows)]
            self.x = [math.cos(t * math.tau * 4.5) * (0.45 + t * 2.4) for t in t_values]
            self.y = [math.sin(t * math.tau * 4.5) * (0.45 + t * 2.4) for t in t_values]
            self.z = [(t - 0.5) * 2.4 for t in t_values]
            self.score = [math.sin(t * math.tau * 4.5 * 0.72) for t in t_values]
            self.phase = t_values
            self.segment = ["outer" if t > 0.66 else "middle" if t > 0.33 else "inner" for t in t_values]

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8))
app.stylesheet(
    """
    Window {
        background: #0d1320;
        color: rgba(245, 248, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
    }

    Panel {
        background: rgba(18, 25, 39, 0.94);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 14px;
        padding: 14px;
        gap: 10px;
        box-shadow: 0 12px 30px rgba(0, 0, 0, 0.25);
    }

    Panel.case {
        box-shadow: none;
        background:
            radial-gradient(circle at 18% 20%, rgba(90, 169, 255, 0.14), transparent 58%),
            rgba(255, 255, 255, 0.045);
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(245, 248, 255, 0.74);
        line-height: 1.12;
    }

    Label.case-title {
        color: white;
        font-weight: 850;
    }

    Label.pass {
        color: #74ddb0;
        font-weight: 800;
    }

    TextArea {
        width: 100%;
        background: rgba(255, 255, 255, 0.07);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 10px;
        color: rgba(245, 248, 255, 0.88);
        line-height: 1.22;
    }

    TextArea.rows-two {
        text-area-rows: 2;
    }

    TextArea.rows-five {
        text-area-rows: 5;
    }

    DataFrameTable.metrics-table {
        height: 226px;
        background: rgba(3, 8, 18, 0.36);
        border: 1px solid rgba(90, 169, 255, 0.28);
        border-radius: 12px;
        color: rgba(232, 240, 255, 0.84);
        font-size: 12px;
        table-row-height: 26px;
        table-header-height: 34px;
        table-column-width: 132px;
        table-index-width: 68px;
    }

    DataFrameTable.metrics-table::header {
        background: rgba(90, 169, 255, 0.20);
        color: white;
        font-weight: 850;
        text-transform: uppercase;
        letter-spacing: 0.08em;
    }

    DataFrameTable.metrics-table::grid-line {
        background: rgba(255, 255, 255, 0.10);
    }

    Panel.scatter-case {
        width: 430px;
        height: 210px;
        padding: 10px;
        gap: 8px;
        box-shadow: none;
    }

    Scatter3D {
        height: 150px;
        background: rgba(3, 8, 18, 0.48);
        border: 1px solid rgba(116, 221, 176, 0.28);
        border-radius: 14px;
    }

    Scatter3D.large-points {
        scatter-point-size: 8px;
    }
    """
)


frame = ProbeFrame()
win = dg.Window("CSS Widget Metrics Probe", width=920, height=700)

with dg.VLayout(class_="root"):
    dg.Label("Widget metrics", class_="title")
    dg.Label(
        "This probe isolates DragonGUI-specific CSS that feeds widget measurement "
        "or renderer metrics: TextArea rows, DataFrameTable sizes, and Scatter3D point size.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12, "height": 235}):
        with dg.Panel("TextArea rows", class_="case", width=310):
            dg.Label("CSS overrides constructor rows", class_="case-title")
            dg.TextArea(
                "Constructor rows=1, CSS text-area-rows: 2.\nSecond visible row should fit.",
                rows=1,
                class_="rows-two",
            )
            dg.TextArea(
                "Constructor rows=1, CSS text-area-rows: 5.\nLine 2\nLine 3\nLine 4\nLine 5",
                rows=1,
                class_="rows-five",
            )
            dg.Label("PASS: lower field is much taller than the upper field.", class_="pass")

        with dg.Panel("Table metrics", class_="case"):
            dg.Label("Header, row, column, and index sizing come from CSS.", class_="case-title")
            dg.DataFrameTable(frame, page_size=32, sample_rows=18, class_="metrics-table")
            dg.Label("PASS: index is wide, header is tall, and columns are uniform.", class_="pass")

    with dg.Panel("Scatter3D point metrics", class_="case"):
        with dg.Panel("8px points", class_="scatter-case"):
            dg.Scatter3D(frame, x="x", y="y", z="z", colormap="magma", class_="large-points")
            dg.Label("PASS: spiral uses visibly large point sprites.", class_="pass")
            dg.Label("PASS: plot keeps its scale while this panel scrolls.", class_="pass")


if __name__ == "__main__":
    print(app.run(win))
