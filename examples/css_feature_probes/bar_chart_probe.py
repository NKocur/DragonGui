from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))
    sys.path.insert(0, str(Path(__file__).resolve().parent))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual visual probe requirement
    raise SystemExit("bar_chart_probe.py requires NumPy") from exc

from probe_helpers import probe_card, probe_grid, probe_header


class SalesFrame:
    columns = ("segment", "region", "sales", "cost", "tickets")
    dtypes = ("str", "str", "float32", "float32", "float32")

    def __init__(self, rows: int = 420) -> None:
        self.shape = (rows, len(self.columns))
        rng = np.random.default_rng(18)
        segments = np.array(["Core", "Growth", "Trial", "Partner", "Enterprise"], dtype=object)
        regions = np.array(["East", "West", "North", "South"], dtype=object)
        self.segment = rng.choice(segments, rows, p=[0.32, 0.24, 0.18, 0.14, 0.12]).tolist()
        self.region = rng.choice(regions, rows).tolist()
        base = np.array([52, 38, 21, 30, 66], dtype=np.float32)
        segment_index = np.array([segments.tolist().index(label) for label in self.segment])
        self.sales = (base[segment_index] + rng.normal(0, 7.0, rows)).clip(2, None).astype(np.float32)
        self.cost = (self.sales * rng.uniform(0.42, 0.68, rows)).astype(np.float32)
        self.tickets = rng.poisson(lam=np.maximum(self.sales / 7.5, 1.0), size=rows).astype(np.float32)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


frame = SalesFrame()

app = dg.App(theme=dg.Theme.dark(accent="#69b7ff", radius=8, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #0b1020;
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
        padding-right: 20px;
    }

    VLayout.root::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb {
        width: 6px;
        background: rgba(105, 183, 255, 0.76);
        border-radius: 999px;
    }

    Label.status {
        width: 100%;
        padding: 8px 10px;
        background: rgba(105, 183, 255, 0.12);
        border: 1px solid rgba(105, 183, 255, 0.34);
        border-radius: 8px;
        color: rgba(231, 245, 255, 0.96);
        font-weight: 740;
    }

    Label.caption {
        color: rgba(214, 224, 242, 0.76);
        line-height: 1.25;
        text-wrap: wrap;
    }

    GridLayout.grid {
        width: 100%;
        gap: 12px;
        row-gap: 12px;
    }

    Panel.case {
        min-height: 285px;
        padding: 12px;
        gap: 10px;
        background: linear-gradient(145deg, rgba(26, 37, 62, 0.96), rgba(13, 19, 34, 0.98));
        border: 1px solid rgba(121, 155, 205, 0.28);
        border-radius: 10px;
        box-shadow: 0 14px 34px rgba(0, 0, 0, 0.26);
    }

    BarChart {
        min-height: 210px;
        background: rgba(6, 10, 20, 0.58);
        border: 1px solid rgba(130, 165, 215, 0.24);
        border-radius: 8px;
    }

    BarChart.sales {
        color: #69b7ff;
    }

    BarChart.grouped {
        color: #76e0b1;
    }

    BarChart.horizontal {
        color: #ffbf69;
    }

    BarChart.dense {
        color: #ed7d9a;
    }

    Badge {
        width: auto;
        padding: 5px 9px;
    }
    """
)

win = dg.Window("Bar Chart Widget Probe", width=1040, height=760)
status: dg.Label | None = None


def set_status(text: str) -> None:
    if status is not None:
        status.set_value(text)


def hover_status(bar: dg.BarChartBar | None) -> None:
    if bar is None:
        set_status("Hover a bar to inspect category, series, and value.")
        return
    set_status(f"{bar.category} / {bar.series}: {bar.value:.4g}")


months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"]
sales_values = [34, 42, 39, 48, 57, 63]
grouped_values = [
    [42, 51, 47, 55, 61],
    [28, 32, 30, 35, 39],
]
dense_labels = [f"S{i:02d}" for i in range(1, 25)]
dense_values = (np.sin(np.linspace(0.0, np.pi * 3.2, len(dense_labels))) * 18.0 + 42.0).round(1)

with dg.VLayout(class_="root"):
    probe_header(
        "BarChart widget probe",
        "Categorical bars using the same native chart chrome, toolbar treatment, and dark dashboard styling as Histogram.",
    )
    status = dg.Label("Hover a bar to inspect category, series, and value.", class_="status")
    with dg.HLayout(style={"gap": 8, "height": "auto"}):
        dg.Badge("single", level="info")
        dg.Badge("grouped", level="success")
        dg.Badge("horizontal", level="warning")
        dg.Badge("dense", level="danger")

    with probe_grid(min_column_width=440, gap=12, row_gap=12):
        with probe_card("Monthly revenue"):
            dg.Label("Direct labels and values rendered as a compact vertical bar chart.", class_="caption")
            dg.BarChart(
                labels=months,
                values=sales_values,
                label="Revenue",
                x_label="month",
                y_label="revenue",
                colors=["#69b7ff"],
                show_toolbar=True,
                on_hover=hover_status,
                class_="sales",
            )

        with probe_card("Grouped segment summary"):
            dg.Label("Frame data aggregated by category with sales and cost side-by-side.", class_="caption")
            dg.BarChart(
                frame,
                category="segment",
                value=["sales", "cost"],
                aggregate="mean",
                x_label="segment",
                y_label="avg value",
                colors=["#76e0b1", "#ffbf69"],
                show_toolbar=True,
                on_hover=hover_status,
                class_="grouped",
            )

        with probe_card("Horizontal workload"):
            dg.Label("Horizontal orientation for longer category names.", class_="caption")
            dg.BarChart(
                labels=["Ingest", "Transform", "Validate", "Export", "Archive"],
                values=[78, 64, 52, 38, 24],
                orientation="horizontal",
                x_label="jobs",
                y_label="stage",
                colors=["#ffbf69"],
                show_toolbar=True,
                on_hover=hover_status,
                class_="horizontal",
            )

        with probe_card("Dense categorical set"):
            dg.Label("Many categories stay readable by dropping crowded value labels first.", class_="caption")
            dg.BarChart(
                labels=dense_labels,
                values=dense_values,
                x_label="sensor",
                y_label="score",
                colors=["#ed7d9a"],
                show_toolbar=True,
                on_hover=hover_status,
                class_="dense",
                bar_gap=1.0,
            )


if __name__ == "__main__":
    print(app.run(win))
