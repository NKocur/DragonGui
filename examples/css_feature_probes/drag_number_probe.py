from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#4fd1a5", radius=7, focus="#ffd166"))
app.stylesheet(
    """
    Window {
        background: #10141b;
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

    HLayout.grid {
        gap: 12px;
        width: 100%;
    }

    Panel.case {
        width: calc(50% - 6px);
        min-width: 360px;
        min-height: 300px;
        background: rgba(22, 31, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        padding: 14px;
        gap: 10px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(246, 249, 255, 0.70);
        line-height: 1.12;
    }

    Label.status {
        background: rgba(79, 209, 165, 0.12);
        border: 1px solid rgba(79, 209, 165, 0.34);
        border-radius: 8px;
        color: rgba(232, 255, 247, 0.96);
        font-weight: 750;
        padding: 8px 10px;
        width: 100%;
    }

    DragNumber {
        height: 30px;
        width: 128px;
        background: rgba(8, 13, 20, 0.62);
        border: 1px solid rgba(255, 255, 255, 0.15);
        border-radius: 7px;
        color: rgba(246, 249, 255, 0.92);
        font-weight: 760;
    }

    DragNumber:hover {
        background: rgba(79, 209, 165, 0.12);
        border-color: rgba(79, 209, 165, 0.52);
    }

    DragNumber:active {
        background: rgba(79, 209, 165, 0.20);
        border-color: rgba(79, 209, 165, 0.78);
    }

    DragNumber::field {
        border-radius: 7px;
    }

    DragNumber::value {
        color: white;
        font-weight: 820;
    }

    DragNumber::grip {
        width: 18px;
        height: 3px;
        background: rgba(79, 209, 165, 0.74);
    }

    DragNumber.warning::grip {
        background: #ffd166;
    }

    DragNumber.drag-vector-value::grip {
        width: 1px;
        height: 1px;
        background: rgba(79, 209, 165, 0);
    }

    Label.drag-vector-label {
        width: 24px;
        height: 30px;
        background: rgba(79, 209, 165, 0.13);
        border: 1px solid rgba(79, 209, 165, 0.30);
        border-radius: 6px;
        color: rgba(232, 255, 247, 0.94);
        font-size: 12px;
        font-weight: 900;
        text-align: center;
    }
    """
)

win = dg.Window("DragNumber probe", width=900, height=500)

with dg.VLayout(class_="root"):
    dg.Label("DragNumber", class_="title")
    status = dg.Label("Gain: 0.50 | Position: (0.0, 1.0, -2.0)", class_="status")

    values = {"gain": 0.5, "exposure": 1.0, "position": (0.0, 1.0, -2.0)}

    def refresh_status() -> None:
        x, y, z = values["position"]
        status.set_value(
            f"Gain: {values['gain']:.2f} | Exposure: {values['exposure']:.2f} | Position: ({x:.1f}, {y:.1f}, {z:.1f})"
        )

    with dg.HLayout(class_="grid"):
        with dg.Panel("Single values", class_="case"):
            dg.Label("Drag horizontally on a value; keyboard focus supports arrow keys.", class_="caption")

            def set_gain(value: float) -> None:
                values["gain"] = value
                refresh_status()

            dg.DragNumber(0.5, min=0, max=1, step=0.05, speed=0.005, on_change=set_gain)

            def set_exposure(value: float) -> None:
                values["exposure"] = value
                refresh_status()

            dg.DragNumber(
                1.0,
                min=-4,
                max=4,
                step=0.25,
                speed=0.02,
                on_change=set_exposure,
                class_="warning",
            )
            dg.DragNumber(42, min=0, max=100, step=1, speed=0.25, disabled=True)

        with dg.Panel("Vector", class_="case"):
            dg.Label("DragVector composes labelled DragNumber controls for compact numeric groups.", class_="caption")

            def set_position(value: tuple[float, ...]) -> None:
                values["position"] = value
                refresh_status()

            dg.DragVector(
                (0, 1, -2),
                labels=("X", "Y", "Z"),
                min=(-10, -10, -10),
                max=(10, 10, 10),
                step=0.5,
                speed=0.05,
                component_gap=6,
                component_width=92,
                on_change=set_position,
            )

    dg.Label("PASS: drag number field, grip part, hover, active, focus, disabled, and vector composition render.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
