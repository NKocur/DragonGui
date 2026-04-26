from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33"))
win = dg.Window(
    "DragonGUI Style Showcase",
    width=980,
    height=640,
    style={"background": "background"},
)

with dg.HLayout(style={"gap": 18, "padding": 14}):
    with dg.Panel(
        "Styled controls",
        class_="control-panel",
        style={
            "width": 340,
            "padding": 16,
            "gap": 12,
            "background": "#18202f",
            "border_color": "#3fc7ff",
            "border_radius": 12,
            "accent": "#3fc7ff",
        },
    ):
        dg.Label(
            "Per-widget text styles",
            style={
                "color": "#b9f6ff",
                "font_size": 18,
                "font_weight": "bold",
                "height": 36,
            },
        )
        dg.Button(
            "Hover / Press",
            style={
                "background": "#26334a",
                "border_color": "#4d5f7c",
                "border_radius": 10,
                "color": "#f4fbff",
                "text_align": "center",
                "font_weight": 700,
                "hover": {"background": "accent_mix_20", "border_color": "accent"},
                "active": {"background": "accent_dark"},
            },
        )
        dg.TextInput(
            "Styled text input",
            style={
                "background": "#101826",
                "border_color": "#557399",
                "border_radius": 8,
                "color": "#c9f2ff",
                "font_family": "monospace",
                "focus": {"border_color": "focus"},
            },
        )
        dg.Dropdown(
            ["surface", "accent", "success"],
            style={
                "background": "#24314a",
                "border_color": "#557399",
                "hover": {"background": "accent_mix_20"},
            },
        )
        dg.Checkbox(
            "Custom accent checkbox",
            checked=True,
            style={"accent": "#43d48f", "border_color": "#43d48f"},
        )
        dg.Slider(
            0.62,
            style={
                "accent": "#ffbf47",
                "border_color": "#755f2d",
                "focus": {"border_color": "focus"},
            },
        )

    with dg.VLayout(style={"gap": 14, "flex_grow": 1}):
        with dg.Panel(
            "Layout overrides",
            style={
                "height": 210,
                "padding": 18,
                "gap": 10,
                "background": "#201b2d",
                "border_color": "#9b7bff",
                "border_radius": 14,
                "accent": "#9b7bff",
            },
        ):
            dg.Label(
                "This panel has custom height, padding, gap, border, radius, and text size.",
                style={"font_size": 16, "font_weight": 600, "color": "#ddd1ff", "height": 34},
            )
            with dg.HLayout(style={"gap": 10, "height": 42}):
                dg.Button(
                    "180px",
                    style={
                        "width": 180,
                        "background": "#2a3650",
                        "text_align": "center",
                    },
                )
                dg.Button(
                    "Flexible",
                    style={
                        "flex_grow": 1,
                        "background": "#2d2d46",
                        "color": "accent",
                        "text_align": "center",
                        "font_family": "monospace",
                    },
                )
            dg.Separator(style={"background": "#9b7bff"})
            dg.Label("The row spacing comes from style={\"gap\": 10}.")

        with dg.Panel(
            "Token colors",
            style={
                "padding": 18,
                "gap": 10,
                "background": "surface",
                "border_color": "border",
                "border_radius": 10,
            },
        ):
            dg.Button("Danger token", style={"background": "danger", "border_color": "danger"})
            dg.Button("Warning token", style={"background": "warning", "border_color": "warning"})
            dg.Button("Success token", style={"background": "success", "border_color": "success"})
            dg.Label(
                "Theme tokens and text color resolve in Rust.",
                style={"color": "muted_text", "text_align": "right", "font_weight": "light"},
            )

print(app.run(win))
