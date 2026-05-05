from __future__ import annotations

import math
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))
    sys.path.insert(0, str(Path(__file__).resolve().parent))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual visual probe requirement
    raise SystemExit("histogram_probe.py requires NumPy") from exc

from probe_helpers import probe_card, probe_grid, probe_header


class HistogramFrame:
    columns = ("latency_ms", "score", "revenue", "residual")
    dtypes = ("float32", "float32", "float32", "float32")

    def __init__(self, rows: int = 3000) -> None:
        self.shape = (rows, len(self.columns))
        rng = np.random.default_rng(42)
        fast = rng.normal(42.0, 8.5, int(rows * 0.72))
        slow = rng.normal(86.0, 18.0, rows - len(fast))
        self.latency_ms = np.clip(np.concatenate([fast, slow]), 0.0, 160.0).astype(np.float32)
        score = rng.beta(4.2, 1.8, rows)
        self.score = score.astype(np.float32)
        self.revenue = rng.lognormal(mean=3.35, sigma=0.58, size=rows).astype(np.float32)
        x = np.linspace(-math.tau, math.tau, rows, dtype=np.float32)
        self.residual = (
            np.sin(x * np.float32(1.7)) * np.float32(0.38)
            + rng.normal(0.0, 0.16, rows)
        ).astype(np.float32)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


frame = HistogramFrame()

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

    Label.title {
        font-size: 21px;
        font-weight: 700;
        color: #f7fbff;
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

    Histogram {
        min-height: 210px;
        background: rgba(6, 10, 20, 0.58);
        border: 1px solid rgba(130, 165, 215, 0.24);
        border-radius: 8px;
    }

    Histogram.latency {
        color: #69b7ff;
    }

    Histogram.density {
        color: #76e0b1;
    }

    Histogram.percent {
        color: #ffbf69;
    }

    Histogram.cumulative {
        color: #ed7d9a;
    }

    Badge {
        width: auto;
        padding: 5px 9px;
    }
    """
)

win = dg.Window("Histogram Widget Probe", width=1040, height=760)

with dg.VLayout(class_="root"):
    probe_header(
        "Histogram widget probe",
        "Static first-slice histograms: pre-binned Python data rendered as native bars with grid and axis chrome.",
    )
    with dg.HLayout(style={"gap": 8, "height": "auto"}):
        dg.Badge("count", level="info")
        dg.Badge("density", level="success")
        dg.Badge("percent", level="warning")
        dg.Badge("cumulative", level="danger")

    with probe_grid(min_column_width=440, gap=12, row_gap=12):
        with probe_card("Latency distribution"):
            dg.Label("Bimodal response-time data with a fixed 0-160 ms range.", class_="caption")
            dg.Histogram(
                frame,
                value="latency_ms",
                bins=32,
                range=(0.0, 160.0),
                label="Latency",
                x_label="latency (ms)",
                y_label="requests",
                color="#69b7ff",
                show_toolbar=True,
                class_="latency",
            )

        with probe_card("Density normalization"):
            dg.Label("Score values normalized so total bin area is one.", class_="caption")
            dg.Histogram(
                frame,
                value="score",
                bins=24,
                range=(0.0, 1.0),
                mode="density",
                x_label="score",
                y_label="density",
                color="#76e0b1",
                show_toolbar=True,
                class_="density",
            )

        with probe_card("Explicit bin edges"):
            dg.Label("Log-normal revenue data using manually supplied thresholds.", class_="caption")
            dg.Histogram(
                frame,
                value="revenue",
                bin_edges=(0, 12, 20, 32, 50, 80, 125, 200),
                mode="percent",
                x_label="revenue",
                y_label="share (%)",
                color="#ffbf69",
                show_toolbar=True,
                class_="percent",
                bar_gap=2.0,
            )

        with probe_card("Cumulative distribution"):
            dg.Label("Residual values accumulated from left to right.", class_="caption")
            dg.Histogram(
                frame,
                value="residual",
                bins=28,
                range=(-1.2, 1.2),
                mode="probability",
                cumulative=True,
                x_label="residual",
                y_label="cumulative probability",
                color="#ed7d9a",
                show_toolbar=True,
                class_="cumulative",
                bar_gap=1.5,
            )


if __name__ == "__main__":
    print(app.run(win))
