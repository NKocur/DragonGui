from __future__ import annotations

import math
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - visual demo requirement
    raise SystemExit("scatter_colorbar_demo.py requires NumPy") from exc


SHORT_TITLE = "Temperature (K)"
LONG_TITLE = "Time-averaged normalized particle temperature gradient across the chamber"


class HelixFrame:
    columns = ("x", "y", "z", "temperature")
    dtypes = ("float32", "float32", "float32", "float32")

    def __init__(self, rows: int = 2_400) -> None:
        self.shape = (rows, len(self.columns))
        t = np.linspace(0.0, 1.0, rows, dtype=np.float32)
        theta = t * np.float32(math.tau * 7.0)
        radius = np.float32(0.5) + t * np.float32(2.4)

        self.x = np.cos(theta) * radius
        self.y = np.sin(theta) * radius
        self.z = (t - np.float32(0.5)) * np.float32(5.0)
        ripple = np.sin(theta * np.float32(0.65)) * np.float32(35.0)
        self.temperature = np.float32(280.0) + t * np.float32(640.0) + ripple

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


app = dg.App(theme=dg.Theme.dark())
window = dg.Window("Scatter3D Colorbar Label Demo", width=1180, height=780)

app.stylesheet(
    """
    Window {
        background: #0c111b;
        color: #edf3ff;
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 0;
        gap: 12px;
    }

    HLayout.heading {
        width: 100%;
        align-items: center;
        gap: 12px;
    }

    Label.title {
        font-size: 22px;
        font-weight: 800;
        color: #ffffff;
    }

    Label.caption {
        color: rgba(225, 234, 250, 0.68);
    }

    Badge.fix {
        background: rgba(78, 205, 154, 0.14);
        border: 1px solid rgba(78, 205, 154, 0.42);
        color: #8ce4bb;
    }

    Panel.stage {
        width: 100%;
        min-width: 0;
        min-height: 0;
        flex-grow: 1;
        padding: 12px;
        gap: 8px;
        background: #111927;
        border: 1px solid #26344a;
        border-radius: 12px;
        overflow: hidden;
    }

    Scatter3D.colorbar-plot {
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 360px;
        flex-grow: 1;
        background: #09101a;
        border-radius: 8px;
    }

    FlowLayout.controls {
        width: 100%;
        gap: 8px;
        row-gap: 8px;
        align-items: center;
    }

    Label.status {
        width: 100%;
        padding: 8px 10px;
        color: #b8c9e5;
        background: rgba(112, 155, 255, 0.08);
        border: 1px solid rgba(112, 155, 255, 0.20);
        border-radius: 8px;
    }
    """
)

frame = HelixFrame()

with window:
    with dg.VLayout(class_="root"):
        with dg.HLayout(class_="heading"):
            with dg.VLayout(style={"gap": 3, "flex_grow": 1}):
                dg.Label("Scatter3D colorbar", class_="title")
                dg.Label(
                    "Use the buttons below to compare normal, long, and hidden colorbar titles.",
                    class_="caption",
                )
            dg.Badge("bounded + ellipsis", level="success", class_="fix")

        with dg.Panel("Colorbar spacing and long-title behavior", class_="stage"):
            scatter = dg.Scatter3D(
                frame,
                x="x",
                y="y",
                z="z",
                scalars="temperature",
                colormap="turbo",
                clim=(240.0, 960.0),
                scalar_bar=True,
                scalar_bar_vmin=240.0,
                scalar_bar_vmax=960.0,
                scalar_bar_colormap="turbo",
                scalar_bar_title=LONG_TITLE,
                grid=True,
                major_planes=True,
                orientation_axes=True,
                point_size=3.4,
                hover=["temperature"],
                class_="colorbar-plot",
            )

        status = dg.Label(
            "Long title active: it should stay inside the plot and end with an ellipsis when needed.",
            class_="status",
        )

        def show_short_title() -> None:
            scatter.show_scalar_bar(
                True,
                vmin=240.0,
                vmax=960.0,
                colormap="turbo",
                title=SHORT_TITLE,
            )
            status.set_value("Short title active: check the gap above the colorbar.")

        def show_long_title() -> None:
            scatter.show_scalar_bar(
                True,
                vmin=240.0,
                vmax=960.0,
                colormap="turbo",
                title=LONG_TITLE,
            )
            status.set_value(
                "Long title active: it should stay inside the plot and end with an ellipsis when needed."
            )

        def use_viridis() -> None:
            scatter.set_colormap("viridis")
            scatter.show_scalar_bar(
                True,
                vmin=240.0,
                vmax=960.0,
                colormap="viridis",
                title=LONG_TITLE,
            )
            status.set_value("Viridis colorbar active with the long-title stress case.")

        def hide_colorbar() -> None:
            scatter.show_scalar_bar(False)
            status.set_value("Colorbar hidden.")

        with dg.FlowLayout(class_="controls"):
            dg.Button("Short title", on_click=show_short_title)
            dg.Button("Long title", class_="primary", on_click=show_long_title)
            dg.Button("Viridis", on_click=use_viridis)
            dg.Button("Hide colorbar", on_click=hide_colorbar)
            dg.Button("Fit view", on_click=scatter.fit)
            dg.Button("Isometric", on_click=scatter.view_isometric)


if __name__ == "__main__":
    print(app.run(window))
