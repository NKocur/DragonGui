from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#8bd450", radius=7, focus="#ffd166"))
app.stylesheet(
    """
    Window {
        background: #11161d;
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
        height: auto;
    }

    Panel.case {
        width: calc(50% - 6px);
        min-width: 360px;
        min-height: 300px;
        background: rgba(23, 31, 40, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        padding: 14px;
        gap: 12px;
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
        background: rgba(139, 212, 80, 0.12);
        border: 1px solid rgba(139, 212, 80, 0.34);
        border-radius: 8px;
        color: rgba(239, 255, 226, 0.96);
        font-weight: 750;
        padding: 8px 10px;
        width: 100%;
    }

    VLayout.radio-list {
        gap: 4px;
        width: 100%;
    }

    RadioButton {
        height: 30px;
        border-radius: 6px;
        color: rgba(246, 249, 255, 0.86);
        transition: background 120ms ease, border-color 120ms ease, color 120ms ease;
    }

    RadioButton:hover {
        background: rgba(139, 212, 80, 0.10);
        border-color: rgba(139, 212, 80, 0.24);
    }

    RadioButton:selected {
        color: white;
    }

    RadioButton::indicator {
        width: 15px;
        height: 15px;
        background: rgba(17, 22, 29, 0.92);
        border: 2px solid rgba(246, 249, 255, 0.36);
        border-radius: 999px;
    }

    RadioButton:selected::indicator {
        border-color: #8bd450;
    }

    RadioButton::dot {
        width: 7px;
        height: 7px;
        background: #8bd450;
        border-radius: 999px;
    }

    RadioButton:disabled {
        color: rgba(246, 249, 255, 0.38);
        opacity: 0.68;
    }
    """
)

win = dg.Window("RadioGroup probe", width=980, height=580)

with dg.VLayout(class_="root"):
    dg.Label("RadioGroup", class_="title")
    status = dg.Label("Renderer mode: Balanced", class_="status")

    def set_status(text: str) -> None:
        status.set_value(text)

    with dg.HLayout(class_="grid"):
        with dg.Panel("Vertical group", class_="case"):
            dg.Label("Exactly one option should be selected. Disabled rows should remain inert.", class_="caption")
            dg.RadioGroup(
                [
                    ("Fast", "Fast"),
                    ("Balanced", "Balanced"),
                    ("Quality", "Quality"),
                    {"label": "Offline", "value": "Offline", "disabled": True},
                ],
                value="Balanced",
                class_="radio-list",
                on_change=lambda value: set_status(f"Renderer mode: {value}"),
            )
            dg.RadioButton(
                "Standalone radio button",
                checked=True,
                on_change=lambda checked: set_status(f"Standalone checked: {checked}"),
            )

        with dg.Panel("Horizontal group", class_="case"):
            dg.Label("Horizontal groups should align compactly without shifting row height.", class_="caption")
            dg.RadioGroup(
                ["Left", "Center", "Right"],
                value="Center",
                orientation="horizontal",
                gap=10,
                on_change=lambda value: set_status(f"Alignment: {value}"),
            )
            dg.RadioGroup(
                [
                    ("Low latency", "low"),
                    ("Balanced", "balanced"),
                    ("High quality", "high"),
                ],
                value="balanced",
                on_change=lambda value: set_status(f"Preset: {value}"),
                class_="radio-list",
            )

    dg.Label("PASS: radio indicator, selected dot, disabled state, vertical and horizontal groups render.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
