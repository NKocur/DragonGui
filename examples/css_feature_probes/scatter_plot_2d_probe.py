from __future__ import annotations

import math
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

from probe_helpers import probe_app, probe_header

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual visual probe requirement
    raise SystemExit("scatter_plot_2d_probe.py requires NumPy") from exc


class Scatter2DFrame:
    columns = ("x", "y", "score", "group", "radius")
    dtypes = ("float32", "float32", "float32", "str", "float32")

    def __init__(self, rows: int = 65_000, *, phase: float = 0.0) -> None:
        self.shape = (rows, len(self.columns))
        i = np.arange(rows, dtype=np.float32)
        t = (i + np.float32(0.5)) / np.float32(max(rows, 1))
        golden_angle = np.float32(math.pi * (3.0 - math.sqrt(5.0)))
        theta = i * golden_angle + np.float32(phase)
        radius = np.sqrt(t) * np.float32(3.35)
        ripple = np.sin(theta * np.float32(5.0) + np.float32(phase)) * np.float32(0.055)
        self.x = np.cos(theta) * (radius + ripple)
        self.y = np.sin(theta) * (radius + ripple)
        angle_score = (np.sin(theta * np.float32(0.18)) + np.float32(1.0)) * np.float32(0.5)
        self.score = (angle_score * np.float32(0.45) + t * np.float32(0.55)).astype(np.float32)
        self.radius = radius.astype(np.float32)
        self.group = np.where(t > 0.67, "outer", np.where(t > 0.34, "middle", "inner"))

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


def make_cluster_frame(seed: int = 7, rows: int = 12_000) -> Scatter2DFrame:
    rng = np.random.default_rng(seed)
    frame = Scatter2DFrame(0)
    frame.shape = (rows, len(frame.columns))
    centers = np.array([[-1.8, -0.9], [-0.25, 1.0], [1.45, -0.3]], dtype=np.float32)
    labels = np.array(["alpha", "beta", "gamma"], dtype=object)
    groups = rng.integers(0, len(centers), size=rows)
    spread = np.array([0.36, 0.24], dtype=np.float32)
    points = centers[groups] + rng.normal(0.0, spread, size=(rows, 2)).astype(np.float32)
    frame.x = points[:, 0]
    frame.y = points[:, 1]
    frame.score = (points[:, 0] * 0.35 + points[:, 1] * 0.65).astype(np.float32)
    frame.radius = np.sqrt(points[:, 0] ** 2 + points[:, 1] ** 2).astype(np.float32)
    frame.group = labels[groups]
    return frame


phase = {"value": 0.0}
dense_frame = Scatter2DFrame()
cluster_frame = make_cluster_frame()

app, win = probe_app("ScatterPlot2D Probe", width=980, height=720)
app.stylesheet(
    """
    Window {
        background: #101521;
        color: rgba(246, 249, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
    }

    Label.title {
        font-size: 20px;
        font-weight: 850;
        color: #ffffff;
    }

    Label.caption {
        color: rgba(235, 241, 255, 0.72);
        line-height: 1.2;
    }

    Label.status {
        padding: 8px 10px;
        width: 100%;
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 8px;
        color: rgba(231, 255, 245, 0.96);
        font-weight: 740;
    }

    HLayout.row {
        width: 100%;
        gap: 12px;
        height: 286px;
    }

    Panel.case {
        flex-grow: 1;
        flex-shrink: 1;
        flex-basis: 0;
        min-width: 0;
        height: 286px;
        padding: 12px;
        gap: 8px;
        background: rgba(18, 26, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        overflow: hidden;
    }

    Scatter3D.scatter-plot-2d {
        width: 100%;
        height: 230px;
        scatter-point-size: 3px;
        scatter-point-style: circle;
        scatter-grid-visible: true;
        background: #0b1020;
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 8px;
    }

    Scatter3D.scatter-plot-2d.dense {
        scatter-point-size: 3px;
        scatter-point-style: gaussian;
    }

    Scatter3D.scatter-plot-2d.clusters {
        scatter-point-size: 5px;
    }

    HLayout.controls {
        width: 100%;
        gap: 8px;
        height: 36px;
    }

    Button {
        height: 32px;
        padding-left: 12px;
        padding-right: 12px;
        font-weight: 760;
    }
    """
)

status: dg.Label | None = None


def set_status(text: str) -> None:
    if status is not None:
        status.set_value(text)


def refresh_dense() -> None:
    phase["value"] += 0.55
    dense_plot.set_points(Scatter2DFrame(phase=phase["value"]), x="x", y="y", scalars="score", fit=True)
    set_status(f"Dense plot refreshed at phase {phase['value']:.2f}.")


def refresh_clusters() -> None:
    seed = int(phase["value"] * 31 + 11)
    cluster_plot.set_points(make_cluster_frame(seed=seed), x="x", y="y", color="group", fit=True)
    set_status(f"Cluster plot refreshed with seed {seed}.")


def fit_both() -> None:
    dense_plot.fit()
    cluster_plot.fit()
    set_status("Both plots fit to their current bounds.")


with win:
    with dg.VLayout(class_="root"):
        probe_header(
            "ScatterPlot2D",
            "Flat 2D scatter API using the high-throughput packed scatter renderer.",
        )
        status = dg.Label(
            "ScatterPlot2D uses the packed point path with a flat XY camera.",
            class_="status",
        )
        with dg.HLayout(class_="row"):
            with dg.Panel("Dense scalar color", class_="case"):
                dense_plot = dg.ScatterPlot2D(
                    dense_frame,
                    x="x",
                    y="y",
                    scalars="score",
                    colormap="turbo",
                    scalar_bar=True,
                    scalar_bar_title="score",
                    point_size=2.0,
                    auto_quality=True,
                    quality_target_fps=30.0,
                    hover=["group", "score"],
                    class_="dense",
                )
            with dg.Panel("Categorical clusters", class_="case"):
                cluster_plot = dg.ScatterPlot2D(
                    cluster_frame,
                    x="x",
                    y="y",
                    color="group",
                    legend=True,
                    legend_position="bottom-left",
                    point_size=5.0,
                    hover=["group", "radius"],
                    class_="clusters",
                )
        with dg.HLayout(class_="controls"):
            dg.Button("Refresh dense", on_click=refresh_dense)
            dg.Button("Refresh clusters", on_click=refresh_clusters)
            dg.Button("Fit both", on_click=fit_both)


if __name__ == "__main__":
    print(app.run(win))
