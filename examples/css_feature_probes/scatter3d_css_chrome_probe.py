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
except ImportError as exc:  # pragma: no cover - manual visual probe requirement
    raise SystemExit("scatter3d_css_chrome_probe.py requires NumPy") from exc


class ScatterFrame:
    columns = ("x", "y", "z", "group")
    dtypes = ("float32", "float32", "float32", "str")

    def __init__(self, rows: int = 900) -> None:
        self.shape = (rows, len(self.columns))
        t = np.linspace(0.0, 1.0, rows, dtype=np.float32)
        theta = t * np.float32(math.tau * 4.5)
        radius = np.float32(0.35) + t * np.float32(2.4)
        self.x = np.cos(theta) * radius
        self.y = np.sin(theta) * radius
        self.z = (t - np.float32(0.5)) * np.float32(2.6)
        self.group = np.where(t > 0.66, "outer", np.where(t > 0.33, "middle", "inner"))

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


frame = ScatterFrame()
legend_entries = [
    ("inner", 0.25, 0.62, 1.0),
    ("middle", 0.44, 0.84, 0.56),
    ("outer", 1.0, 0.66, 0.28),
]

app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8))
app.stylesheet(
    """
    Window {
        background: #0e1422;
        color: rgba(244, 247, 252, 0.96);
        padding: 16px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
    }

    Panel.case {
        min-height: 520px;
        padding: 12px;
        gap: 10px;
        background: #151d2d;
        border: 1px solid rgba(139, 164, 203, 0.24);
        border-radius: 8px;
    }

    Label.title {
        font-size: 16px;
        font-weight: 700;
        color: #f4f7fc;
    }

    Label.caption {
        color: rgba(216, 226, 242, 0.74);
    }

    Scatter3D {
        height: 380px;
        border-radius: 8px;
        background: #0a0f1a;
        border: 1px solid rgba(139, 164, 203, 0.22);
    }

    Scatter3D.css-chrome {
        scatter-point-size: 6px;
        scatter-point-style: square;
        scatter-grid-visible: true;
        scatter-grid-planes: all;
        scatter-legend-position: bottom-left;
        scatter-orientation-axes: true;
    }

    Scatter3D.css-minimal {
        scatter-point-size: 3px;
        scatter-point-style: circle;
        scatter-grid-visible: false;
        scatter-grid-planes: none;
        scatter-legend-position: top-right;
        scatter-orientation-axes: false;
    }
    """
)

win = dg.Window("Scatter3D CSS Chrome Probe", width=1040, height=760)

with dg.VLayout(class_="root"):
    dg.Label("Scatter3D CSS chrome", class_="title")
    dg.Label(
        "Left plot gets grid, planes, legend position, orientation axes, point size, and point style from CSS.",
        class_="caption",
    )

    with probe_grid(gap=12):
        with dg.Panel("CSS chrome enabled", class_="case"):
            dg.Scatter3D(
                frame,
                x="x",
                y="y",
                z="z",
                color="group",
                legend=True,
                legend_entries=legend_entries,
                class_="css-chrome",
            )
            dg.Label("PASS: square points, grid/planes, bottom-left legend, and orientation axes are visible.", class_="caption")

        with dg.Panel("CSS chrome disabled", class_="case"):
            dg.Scatter3D(
                frame,
                x="x",
                y="y",
                z="z",
                color="group",
                grid=True,
                major_planes=True,
                orientation_axes=True,
                legend=True,
                legend_entries=legend_entries,
                class_="css-minimal",
            )
            dg.Label("PASS: CSS removes grid/planes/orientation axes and keeps the legend at top-right.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
