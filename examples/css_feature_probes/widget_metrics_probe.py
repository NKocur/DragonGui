from __future__ import annotations

import math
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

from probe_helpers import probe_grid

try:
    import numpy as np
except ImportError:  # pragma: no cover
    np = None


class ScatterFrame:
    """Small helical point cloud for scatter metric cases."""

    def __init__(self, rows: int = 600) -> None:
        self.shape = (rows, 4)
        if np is not None:
            t = np.linspace(0.0, 1.0, rows, dtype=np.float32)
            theta = t * np.float32(math.tau * 4.0)
            r = np.float32(0.4) + t * np.float32(2.0)
            self.x = np.cos(theta) * r
            self.y = np.sin(theta) * r
            self.z = (t - np.float32(0.5)) * np.float32(2.0)
        else:
            t_vals = [i / max(1, rows - 1) for i in range(rows)]
            self.x = [math.cos(t * math.tau * 4.0) * (0.4 + t * 2.0) for t in t_vals]
            self.y = [math.sin(t * math.tau * 4.0) * (0.4 + t * 2.0) for t in t_vals]
            self.z = [(t - 0.5) * 2.0 for t in t_vals]

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


class MetricsFrame:
    columns = ("id", "region", "owner", "score", "delta", "status")
    dtypes = ("int64", "str", "str", "float64", "float64", "str")

    def __init__(self, rows: int = 80) -> None:
        self.shape = (rows, len(self.columns))
        self.id = list(range(1000, 1000 + rows))
        self.region = [("North", "West", "South", "East")[i % 4] for i in range(rows)]
        self.owner = [("Avery", "Morgan", "Riley", "Quinn", "Taylor")[i % 5] for i in range(rows)]
        self.score = [round(72.0 + (i % 18) * 1.37, 2) for i in range(rows)]
        self.delta = [round(((i % 9) - 4) * 0.18, 2) for i in range(rows)]
        self.status = [("queued", "active", "review", "done")[i % 4] for i in range(rows)]

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
        overflow-y: auto;
        padding-right: 20px;
        padding-bottom: 48px;
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

    GridLayout.grid {
        width: 100%;
        gap: 12px;
        height: auto;
    }

    Panel {
        background:
            radial-gradient(circle at 18% 20%, rgba(90, 169, 255, 0.13), transparent 58%),
            rgba(18, 25, 39, 0.94);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 14px;
        padding: 14px;
        gap: 10px;
        box-shadow: 0 12px 30px rgba(0, 0, 0, 0.25);
    }

    Panel.case {
        min-width: 0;
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
        line-height: 1.18;
        padding: 10px;
    }

    TextArea.rows-two {
        text-area-rows: 2;
    }

    TextArea.rows-five {
        text-area-rows: 5;
    }

    TextArea.tight {
        text-area-rows: 4;
        line-height: 1.05;
        padding: 7px;
    }

    TextArea.comfortable {
        text-area-rows: 4;
        line-height: 1.32;
        padding: 12px;
    }

    DataFrameTable {
        background: rgba(3, 8, 18, 0.36);
        border: 1px solid rgba(90, 169, 255, 0.28);
        border-radius: 12px;
        color: rgba(232, 240, 255, 0.84);
    }

    DataFrameTable::header {
        background: rgba(90, 169, 255, 0.20);
        color: white;
        font-weight: 850;
        text-transform: uppercase;
        letter-spacing: 0.08em;
    }

    DataFrameTable::row {
        background: rgba(255, 255, 255, 0.025);
    }

    DataFrameTable::row-selected {
        background: rgba(116, 221, 176, 0.20);
    }

    DataFrameTable::grid-line {
        background: rgba(255, 255, 255, 0.10);
    }

    DataFrameTable.compact-table {
        height: 214px;
        font-size: 11px;
        table-row-height: 22px;
        table-header-height: 28px;
        table-column-width: 112px;
        table-index-width: 48px;
    }

    DataFrameTable.roomy-table {
        height: 274px;
        font-size: 13px;
        table-row-height: 32px;
        table-header-height: 42px;
        table-column-width: 150px;
        table-index-width: 76px;
    }

    Panel.scatter-case {
        height: 340px;
    }

    Scatter3D {
        width: 100%;
        flex-grow: 1;
        border-radius: 10px;
    }

    Scatter3D.tiny-points {
        scatter-point-size: 2px;
    }

    Scatter3D.large-points {
        scatter-point-size: 10px;
    }

    Panel.scroll-scatter-case {
        height: 500px;
        overflow-y: auto;
    }

    Panel.scroll-scatter-case::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    Panel.scroll-scatter-case::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.72);
        border-radius: 999px;
    }

    Scatter3D.scroll-plot {
        scatter-point-size: 6px;
        height: 320px;
    }
    """
)


frame = MetricsFrame()
scatter_frame = ScatterFrame()
win = dg.Window("CSS Widget Metrics Probe", width=940, height=720)

with dg.VLayout(class_="root"):
    dg.Label("Widget metrics", class_="title")
    dg.Label(
        "This probe isolates DragonGUI-specific measurement CSS for TextArea rows "
        "and DataFrameTable header, row, column, and index sizing.",
        class_="caption",
    )

    with probe_grid(gap=12):
        with dg.Panel("TextArea rows", class_="case"):
            dg.Label("CSS text-area-rows overrides constructor rows.", class_="case-title")
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

        with dg.Panel("TextArea typography metrics", class_="case"):
            dg.Label("Same row count, different padding and line-height.", class_="case-title")
            dg.TextArea(
                "tight line-height\nkeeps four rows compact\nwithout clipping\nor caret drift",
                rows=2,
                class_="tight",
            )
            dg.TextArea(
                "comfortable line-height\nuses the same row count\nbut takes more space\nand reads airier",
                rows=2,
                class_="comfortable",
            )
            dg.Label("PASS: both have four rows, but different physical heights.", class_="pass")

    with probe_grid(gap=12):
        with dg.Panel("Scatter3D tiny points", class_="case scatter-case"):
            dg.Label("CSS scatter-point-size: 2px — viridis colormap.", class_="case-title")
            dg.Scatter3D(scatter_frame, x="x", y="y", z="z", colormap="viridis", class_="tiny-points")
            dg.Label("PASS: points are visibly small and z-colored.", class_="pass")

        with dg.Panel("Scatter3D large points", class_="case scatter-case"):
            dg.Label("CSS scatter-point-size: 10px — plasma colormap.", class_="case-title")
            dg.Scatter3D(scatter_frame, x="x", y="y", z="z", colormap="plasma", class_="large-points")
            dg.Label("PASS: points are visibly larger and use a different colormap.", class_="pass")

    with probe_grid(gap=12):
        with dg.Panel("Scatter3D inside scrollable panel", class_="case scroll-scatter-case"):
            dg.Label("Scroll this panel — the plot viewport should not shrink.", class_="case-title")
            dg.Scatter3D(scatter_frame, x="x", y="y", z="z", colormap="turbo", class_="scroll-plot")
            dg.Label("Scrolled content below the scatter plot.", class_="caption")
            dg.Label("More content to force scroll overflow.", class_="caption")
            dg.Label("PASS: scrolling clips the scatter but does not change its viewport size.", class_="pass")

        with dg.Panel("Compact table metrics", class_="case"):
            dg.Label("Small rows, narrow columns, and a narrow index gutter.", class_="case-title")
            dg.DataFrameTable(frame, page_size=48, sample_rows=24, class_="compact-table")
            dg.Label("PASS: many rows are visible and header height is compact.", class_="pass")

        with dg.Panel("Roomy table metrics", class_="case"):
            dg.Label("Larger rows, taller header, wider columns, and wider index.", class_="case-title")
            dg.DataFrameTable(frame, page_size=48, sample_rows=24, class_="roomy-table")
            dg.Label("PASS: fewer rows are visible and columns/index are wider.", class_="pass")


if __name__ == "__main__":
    print(app.run(win))
