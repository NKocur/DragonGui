from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#7dd3fc", radius=7, focus="#ffd166"))
app.stylesheet(
    """
    Window {
        background: #0f131a;
        color: rgba(246, 249, 255, 0.94);
        padding: 16px;
        gap: 10px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 10px;
    }

    Label.title {
        color: white;
        font-size: 18px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(246, 249, 255, 0.70);
        line-height: 1.12;
    }

    Label.status {
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.095);
        border-radius: 7px;
        color: rgba(236, 244, 255, 0.92);
        font-weight: 750;
        padding: 7px 10px;
        width: 100%;
    }

    Splitter.workspace {
        width: 100%;
        height: 442px;
    }

    Splitter.nested {
        width: 100%;
        height: 100%;
    }

    Splitter::gutter {
        width: 3px;
        height: 3px;
        background: transparent;
    }

    Panel.pane-card {
        width: 100%;
        height: 100%;
        min-height: 80px;
        background: #151b24;
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 8px;
        padding: 12px;
        gap: 9px;
    }

    Panel.preview {
        background: #111821;
    }

    Panel.navigator {
        background: #121922;
    }

    Panel.inspector {
        background: #171b22;
    }

    Label.metric {
        background: rgba(255, 255, 255, 0.050);
        border: 1px solid rgba(255, 255, 255, 0.095);
        border-radius: 7px;
        color: white;
        font-weight: 800;
        padding: 7px 9px;
        width: 100%;
    }

    Selectable {
        height: 30px;
        border-radius: 7px;
        padding: 6px 9px;
    }

    ProgressBar {
        width: 100%;
        height: 18px;
    }

    RangeSlider {
        width: 100%;
        height: 30px;
    }
    """
)

win = dg.Window("Splitter probe", width=1060, height=620)

with dg.VLayout(class_="root"):
    dg.Label("Splitter", class_="title")
    dg.Label("Workspace layout: navigator / preview / inspector", class_="status")

    with dg.Splitter(
        orientation="horizontal",
        sizes=(240, "1fr", 260),
        min_sizes=(160, 260, 180),
        gutter_size=8,
        class_="workspace",
    ):
        with dg.Pane():
            with dg.Panel("Navigator", class_="pane-card navigator"):
                dg.Label("Scenes", class_="metric")
                dg.Selectable("Overview", selected=True)
                dg.Selectable("Sensors")
                dg.Selectable("Streaming plots")
                dg.Selectable("Exports")

        with dg.Pane():
            with dg.Splitter(
                orientation="vertical",
                sizes=("1fr", 150),
                min_sizes=(170, 110),
                gutter_size=8,
                class_="nested",
            ):
                with dg.Pane():
                    with dg.Panel("Preview", class_="pane-card preview"):
                        dg.Label("Frame time", class_="metric")
                        dg.ProgressBar(0.68)
                        dg.RangeSlider((20, 84), min=0, max=100, step=2)
                        dg.Label("Render: 8.4 ms", class_="caption")

                with dg.Pane():
                    with dg.Panel("Log", class_="pane-card"):
                        dg.Label("Layout stable", class_="metric")
                        dg.Label("Pane children keep their own layout while resizing.", class_="caption")

        with dg.Pane():
            with dg.Panel("Inspector", class_="pane-card inspector"):
                dg.Label("Transform", class_="metric")
                dg.DragVector((0, 1.5, -2.0), labels=("X", "Y", "Z"), component_width=90)
                dg.Label("Bounds", class_="metric")
                dg.RangeSlider((-1.5, 2.5), min=-5, max=5, step=0.25, class_="warn")

    dg.Label("PASS: splitter gutters render and adjacent panes resize without clipping children.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
