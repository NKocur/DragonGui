from __future__ import annotations

import time
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))
    sys.path.insert(0, str(Path(__file__).resolve().parent))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual visual probe requirement
    raise SystemExit("startup_loading_probe.py requires NumPy") from exc


class StartupScatterFrame:
    columns = ("x", "y", "z")
    dtypes = ("float32", "float32", "float32")

    def __init__(self, rows: int = 220_000) -> None:
        self.shape = (rows, len(self.columns))
        rng = np.random.default_rng(11)
        theta = rng.uniform(0.0, np.pi * 8.0, rows).astype(np.float32)
        radius = rng.normal(0.8, 0.16, rows).astype(np.float32)
        self.x = np.cos(theta) * radius
        self.y = np.sin(theta) * radius
        self.z = np.linspace(-1.0, 1.0, rows, dtype=np.float32)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


app = dg.App(
    theme=dg.Theme.dark(accent="#42a5ff", radius=8),
    loading_screen=dg.LoadingScreen(
        title="Preparing DragonGUI",
        message="Building the Python document, then loading native resources...",
        background="#07111f",
        text="#f8fafc",
        accent="#42a5ff",
        show_progress=True,
        min_duration_ms=450,
    ),
)

app.stylesheet(
    """
    Window {
        background: #0b1020;
        color: #f8fafc;
        padding: 18px;
        gap: 12px;
    }

    Panel {
        padding: 12px;
        gap: 10px;
        background: rgba(255, 255, 255, 0.055);
        border: 1px solid rgba(148, 163, 184, 0.28);
        border-radius: 10px;
    }

    Scatter3D {
        min-height: 420px;
        border-radius: 8px;
        background: #050914;
    }

    Label.caption {
        color: rgba(226, 232, 240, 0.78);
        text-wrap: wrap;
    }
    """
)


def build_window() -> dg.Window:
    # Simulate application-side startup work that would normally happen before
    # app.run() and block any native frame from appearing.
    time.sleep(0.45)
    win = dg.Window("Startup Loading Probe", width=980, height=720)
    with win:
        with dg.VLayout(style={"width": "100%", "height": "100%", "gap": 12}):
            with dg.Panel("Startup Loading Screen"):
                dg.Label(
                    "This probe uses run_with_loading() so the loading screen appears before the real Python window is constructed.",
                    class_="caption",
                )
                dg.Scatter3D(
                    StartupScatterFrame(),
                    x="x",
                    y="y",
                    z="z",
                    colormap="turbo",
                )
    return win


if __name__ == "__main__":
    app.run_with_loading(build_window, title="Startup Loading Probe", width=980, height=720)
