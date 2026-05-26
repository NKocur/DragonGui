from __future__ import annotations

import math
import random
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


class Sparkline(dg.PaintWidget):
    def __init__(
        self,
        values: list[float],
        *,
        stroke: str = "accent",
        fill: str = "surface",
        **kwargs: object,
    ) -> None:
        self.values = list(values)
        self.stroke = stroke
        self.fill = fill
        super().__init__(extension_type="sparkline", **kwargs)

    def measure(self, constraints: dg.MeasureConstraints) -> dg.Size:
        return constraints.clamp(dg.Size(220, 68))

    def paint(self, ctx: dg.PaintContext) -> None:
        ctx.rounded_rect(0, 0, ctx.width, ctx.height, radius=9, fill=self.fill)
        if len(self.values) < 2:
            return

        pad_x = 10.0
        pad_y = 8.0
        lo = min(self.values)
        hi = max(self.values)
        span = hi - lo or 1.0
        plot_w = max(1.0, ctx.width - pad_x * 2.0)
        plot_h = max(1.0, ctx.height - pad_y * 2.0)
        step = plot_w / (len(self.values) - 1)
        points = [
            [
                pad_x + index * step,
                pad_y + plot_h - ((value - lo) / span) * plot_h,
            ]
            for index, value in enumerate(self.values)
        ]
        ctx.polyline(points, stroke=self.stroke, width=2.25)
        ctx.circle(points[-1][0], points[-1][1], 3.6, fill=self.stroke)
        ctx.text(10, 7, f"{self.values[-1]:+.2f}", fill="text", font_size=11, font_weight=750)


def wave(seed: int, count: int = 40) -> list[float]:
    rng = random.Random(seed)
    phase = rng.random() * math.tau
    return [
        math.sin(phase + index * 0.24) * 0.6
        + math.sin(phase * 0.5 + index * 0.07) * 0.25
        + rng.uniform(-0.08, 0.08)
        for index in range(count)
    ]


app = dg.App(theme=dg.Theme.dark(accent="#7dd3fc", radius=8, focus="#fbbf24"))
app.stylesheet(
    """
    Window {
        background: #10151f;
        color: #eef4ff;
        padding: 18px;
        gap: 14px;
        font-size: 14px;
    }

    Label.title {
        font-size: 20px;
        font-weight: 850;
        color: white;
    }

    Label.caption {
        color: rgba(238, 244, 255, 0.68);
    }

    GridLayout.spark-grid {
        width: 100%;
        flex-grow: 1;
        min-height: 0;
        gap: 12px;
        row-gap: 12px;
    }

    Panel.card {
        min-height: 0;
        padding: 12px;
        gap: 8px;
        background: rgba(20, 29, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 9px;
    }

    ExtensionWidget.spark {
        width: 100%;
        height: 68px;
        border: 1px solid rgba(125, 211, 252, 0.24);
        border-radius: 9px;
        background: rgba(11, 18, 28, 0.92);
    }
    """
)

win = dg.Window("PaintWidget Sparkline Probe", width=920, height=560)

with dg.VLayout(class_="root", style={"width": "100%", "height": "100%", "gap": 12}):
    dg.Label("PaintWidget Sparkline Probe", class_="title")
    dg.Label(
        "Pure Python widgets record rect, polyline, and circle commands into a native display list.",
        class_="caption",
    )
    with dg.GridLayout(columns=3, min_column_width=240, class_="spark-grid"):
        for index, label in enumerate(("loss", "accuracy", "throughput", "gpu util", "latency", "queue")):
            with dg.Panel(label.title(), class_="card"):
                Sparkline(
                    wave(index),
                    stroke=("#7dd3fc", "#86efac", "#fbbf24", "#f472b6", "#c4b5fd", "#fb7185")[index],
                    class_="spark",
                )
                dg.Badge(f"{sum(wave(index)) / 40:+.2f}", level="info")


if __name__ == "__main__":
    app.run(win)
