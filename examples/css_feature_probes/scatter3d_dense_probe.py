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
    raise SystemExit("scatter3d_dense_probe.py requires NumPy") from exc


class DenseScatter3DFrame:
    columns = ("x", "y", "z", "score", "group", "radius")
    dtypes = ("float32", "float32", "float32", "float32", "str", "float32")

    def __init__(self, rows: int = 65_000, *, phase: float = 0.0) -> None:
        self.shape = (rows, len(self.columns))
        i = np.arange(rows, dtype=np.float32)
        t = (i + np.float32(0.5)) / np.float32(max(rows, 1))
        golden_angle = np.float32(math.pi * (3.0 - math.sqrt(5.0)))
        theta = i * golden_angle + np.float32(phase)

        z_unit = np.float32(1.0) - np.float32(2.0) * t
        shell_radius = np.sqrt(np.maximum(np.float32(0.0), np.float32(1.0) - z_unit * z_unit))
        fill = np.sqrt(np.mod(i * np.float32(0.7548777), np.float32(1.0)))
        radius = (shell_radius * fill) * np.float32(3.1)
        ripple = np.sin(theta * np.float32(4.0) + np.float32(phase)) * np.float32(0.045)

        self.x = np.cos(theta) * (radius + ripple)
        self.y = np.sin(theta) * (radius + ripple)
        self.z = z_unit * np.float32(2.65) + np.sin(theta * np.float32(0.37)) * np.float32(0.18)
        self.radius = np.sqrt(self.x * self.x + self.y * self.y + self.z * self.z).astype(np.float32)
        wave = (np.sin(theta * np.float32(0.16) + self.z) + np.float32(1.0)) * np.float32(0.5)
        self.score = (wave * np.float32(0.32) + t * np.float32(0.38) + fill * np.float32(0.30)).astype(np.float32)
        self.group = np.where(
            self.radius > 2.45,
            "shell",
            np.where(self.z > 0.9, "upper", np.where(self.z < -0.9, "lower", "core")),
        )

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


phase = {"value": 0.0}
dense_frame = DenseScatter3DFrame()

app, win = probe_app("Dense Scatter3D Probe", width=980, height=760)
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
        width: 100%;
        padding: 8px 10px;
        background: rgba(90, 169, 255, 0.13);
        border: 1px solid rgba(90, 169, 255, 0.36);
        border-radius: 8px;
        color: rgba(232, 244, 255, 0.96);
        font-weight: 740;
    }

    Panel.stage {
        flex-grow: 1;
        min-height: 0;
        padding: 12px;
        gap: 10px;
        background: rgba(18, 26, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        overflow: hidden;
    }

    Scatter3D.dense-3d {
        width: 100%;
        height: 100%;
        min-height: 470px;
        scatter-point-size: 3px;
        scatter-point-style: gaussian;
        scatter-grid-visible: true;
        scatter-grid-planes: all;
        scatter-orientation-axes: true;
        background: #0b1020;
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 8px;
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
    phase["value"] += 0.45
    dense_plot.set_points(
        DenseScatter3DFrame(phase=phase["value"]),
        x="x",
        y="y",
        z="z",
        scalars="score",
        fit=True,
    )
    set_status(f"Dense 3D plot refreshed at phase {phase['value']:.2f}.")


def view_iso() -> None:
    dense_plot.view_isometric()
    set_status("View snapped to isometric. Left-drag should still orbit.")


def view_xy() -> None:
    dense_plot.view_xy()
    set_status("View snapped to XY. This is still a 3D scatter, so left-drag should orbit.")


def view_xz() -> None:
    dense_plot.view_xz()
    set_status("View snapped to XZ. This is useful for checking real z depth.")


def fit_plot() -> None:
    dense_plot.fit()
    set_status("Dense 3D plot fit to its current bounds.")


with win:
    with dg.VLayout(class_="root"):
        probe_header(
            "Dense Scatter3D",
            "A true 3D version of the dense scalar plot for checking depth, orbit controls, and packed point rendering.",
        )
        status = dg.Label(
            "Dense 3D scalar scatter: left-drag should orbit, right/middle drag should pan.",
            class_="status",
        )
        with dg.Panel("Dense scalar volume", class_="stage"):
            dense_plot = dg.Scatter3D(
                dense_frame,
                x="x",
                y="y",
                z="z",
                scalars="score",
                colormap="turbo",
                scalar_bar=True,
                scalar_bar_title="score",
                grid=True,
                major_planes=True,
                orientation_axes=True,
                point_size=2.5,
                auto_quality=True,
                quality_target_fps=30.0,
                hover=["group", "score", "radius"],
                class_="dense-3d",
            )
        with dg.HLayout(class_="controls"):
            dg.Button("Refresh", on_click=refresh_dense)
            dg.Button("Isometric", on_click=view_iso)
            dg.Button("XY", on_click=view_xy)
            dg.Button("XZ", on_click=view_xz)
            dg.Button("Fit", on_click=fit_plot)


if __name__ == "__main__":
    print(app.run(win))
