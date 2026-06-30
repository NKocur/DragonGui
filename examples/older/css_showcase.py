from __future__ import annotations

import math
import sys
from pathlib import Path
from types import SimpleNamespace

import numpy as np

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


def sample_frame() -> SimpleNamespace:
    t = np.linspace(0.0, math.tau * 2.0, 2400, dtype=np.float32)
    return SimpleNamespace(
        columns=("x", "y", "z"),
        shape=(t.size, 3),
        x=np.sin(t),
        y=np.cos(t * 0.73),
        z=np.sin(t * 0.37) * np.cos(t),
    )


app = dg.App(theme=dg.Theme.dark(accent="#4da3ff", radius=5))
app.stylesheet(
    """
    Window {
        background: #050816;
    }

    Panel {
        padding: 16px;
        gap: 12px;
        background: #101a3a;
        border: 1px solid #00d5ff;
        border-radius: 14px;
        accent: #ff42c7;
        color: #d7fbff;
    }

    Panel.controls {
        width: 340px;
        background: #1a0d35;
        border-color: #ff42c7;
    }

    Panel.controls > Button {
        border-radius: 10px;
    }

    Label {
        color: #d7fbff;
    }

    Button {
        background: #111f4a;
        border: 1px solid #ff42c7;
        border-radius: 10px;
        color: #ffffff;
        font-weight: 700;
    }

    Button:hover {
        background: #0d3d66;
        border-color: #00d5ff;
        color: #ffffff;
    }

    Button.primary {
        background: #ff42c7;
        border-color: #ff42c7;
        color: #050816;
    }

    TextInput, Dropdown, NumberInput {
        background: #071027;
        border: 1px solid #00d5ff;
        border-radius: 10px;
        color: #ffffff;
    }

    TextInput:focus, NumberInput:focus {
        border-color: #6dff8f;
    }

    Slider {
        accent: #ff42c7;
        track-color: #26366e;
        thumb-color: #6dff8f;
    }

    Checkbox {
        accent: #6dff8f;
        color: #ffffff;
    }

    ProgressBar {
        accent: #6dff8f;
        background: #071027;
        border-color: #00d5ff;
        border-radius: 10px;
    }

    Tab {
        border-radius: 10px;
        accent: #ff42c7;
        color: #ffffff;
    }

    Scatter3D {
        border-color: #00d5ff;
        border-width: 1px;
    }

    .plot {
        flex-grow: 1;
    }
    """
)

win = dg.Window("DragonGUI CSS Showcase", width=1120, height=720)
frame = sample_frame()

with dg.HLayout(style={"gap": 12, "padding": 12}):
    with dg.Panel("CSS controls", class_="controls"):
        dg.Label("Selectors, pseudo-states, and inherited text.")
        dg.Button("Primary action", class_="primary")
        dg.Button("Secondary action")
        dg.TextInput("CSS text input")
        dg.Dropdown(["viridis", "magma", "plasma"], value="viridis")
        dg.NumberInput(42, min=0, max=100)
        dg.Slider(0.58)
        dg.Checkbox("Inherited text with CSS accent", checked=True)
        dg.ProgressBar(0.64, label="64% complete")

    with dg.VLayout(class_="plot", style={"gap": 12}):
        with dg.Panel("Styled data view", class_="plot", style={"height": 460}):
            dg.Scatter3D(frame, x="x", y="y", z="z", colormap="viridis")

        with dg.Tabs(value="overview"):
            dg.Tab("Overview", value="overview")
            dg.Tab("Details", value="details")
            dg.Tab("Export", value="export")

        with dg.Panel("Direct child selector"):
            dg.Label("The buttons in the left panel get a direct-child selector.")
            with dg.HLayout(style={"gap": 10, "height": 38}):
                dg.Button("Inline override", style={"background": "#5a3b7a"})
                dg.Button("CSS default")


print(app.run(win))
