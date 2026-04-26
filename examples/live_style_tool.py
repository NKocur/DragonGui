from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33"))
win = dg.Window("DragonGUI Live Style Demo", width=820, height=460)

state = {"index": 0}

styles = [
    {
        "panel": {
            "background": "#172235",
            "border_color": "#3fc7ff",
            "border_radius": 12,
            "padding": 16,
            "gap": 12,
        },
        "preview": {
            "width": 190,
            "height": 46,
            "background": "#24314a",
            "border_color": "#3fc7ff",
            "border_radius": 10,
            "color": "#e9fbff",
            "font_size": 14,
            "text_align": "center",
        },
        "label": {"color": "#b9f6ff", "font_size": 16, "font_weight": "bold"},
        "message": "Cool blue style",
    },
    {
        "panel": {
            "background": "#2b1722",
            "border_color": "danger",
            "border_radius": 4,
            "padding": 20,
            "gap": 16,
        },
        "preview": {
            "width": 250,
            "height": 54,
            "background": "danger",
            "border_color": "#ffb1c3",
            "border_radius": 6,
            "color": "#fff6f8",
            "font_size": 16,
            "font_weight": "bold",
            "text_align": "center",
        },
        "label": {"color": "#ffd3dc", "font_size": 18, "font_weight": "bold"},
        "message": "Danger style with layout change",
    },
    {
        "panel": {
            "background": "#17251d",
            "border_color": "success",
            "border_radius": 16,
            "padding": 14,
            "gap": 10,
        },
        "preview": {
            "width": 220,
            "height": 42,
            "background": "success",
            "border_color": "#c7f7d6",
            "border_radius": 18,
            "color": "#062111",
            "font_size": 14,
            "font_weight": 700,
            "text_align": "center",
        },
        "label": {"color": "#c8f7d4", "font_size": 15, "font_family": "monospace"},
        "message": "Success style",
    },
]


def apply_style() -> None:
    state["index"] = (state["index"] + 1) % len(styles)
    current = styles[state["index"]]
    preview_panel.set_style(current["panel"])
    preview_button.set_style(current["preview"])
    preview_label.set_style(current["label"])
    status.set_value(current["message"])


def clear_style() -> None:
    preview_panel.set_style(None)
    preview_button.set_style(None)
    preview_label.set_style(None)
    status.set_value("Styles cleared")


with dg.HLayout(style={"gap": 16, "padding": 14}):
    with dg.Panel("Live style controls", width=300, style={"padding": 14, "gap": 10}):
        status = dg.TextInput("Ready", placeholder="Last action")
        dg.Button("Cycle Live Style", on_click=apply_style)
        dg.Button("Clear Style", on_click=clear_style)
        dg.Label("Watch live style patches.")

    with dg.Panel("Live preview", style=styles[0]["panel"]) as preview_panel:
        preview_label = dg.Label("Styled label", style=styles[0]["label"])
        preview_button = dg.Button("Preview button", style=styles[0]["preview"])
        dg.Label("Patches visual, text, and layout keys.")


print(app.run(win))
