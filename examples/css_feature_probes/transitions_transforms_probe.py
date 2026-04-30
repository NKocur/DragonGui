from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8))
app.stylesheet(
    """
    Window {
        background: #0d1320;
        color: rgba(245, 248, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.scroll-root {
        height: 620px;
        overflow-y: auto;
        padding-right: 18px;
        padding-bottom: 18px;
        gap: 12px;
    }

    VLayout.scroll-root::scrollbar-track {
        width: 8px;
        padding: 8px;
        background: rgba(255, 255, 255, 0.09);
        border-radius: 999px;
    }

    VLayout.scroll-root::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.72);
        border-radius: 999px;
    }

    Panel {
        background: rgba(18, 25, 39, 0.94);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 14px;
        box-shadow: 0 12px 30px rgba(0, 0, 0, 0.26);
        padding: 12px;
        gap: 8px;
    }

    Panel.case {
        width: 356px;
        min-height: 156px;
        overflow: visible;
    }

    Label.title {
        color: #5aa9ff;
        font-size: 20px;
        font-weight: 800;
    }

    Label.caption {
        color: rgba(245, 248, 255, 0.72);
        line-height: 1.16;
    }

    Label.case-title {
        color: rgba(245, 248, 255, 0.95);
        font-weight: 800;
    }

    Label.pass {
        color: #74ddb0;
        font-weight: 800;
    }

    Button,
    TextInput,
    Dropdown,
    Checkbox,
    Badge {
        transition-property:
            background,
            border-color,
            color,
            opacity,
            outline-color,
            outline-width,
            outline-offset,
            translate,
            scale,
            rotate,
            box-shadow;
        transition-duration: 220ms;
        transition-timing-function: cubic-bezier(0.16, 1, 0.3, 1);
    }

    Button {
        height: 34px;
        background: rgba(255, 255, 255, 0.06);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 10px;
        color: white;
        font-weight: 800;
        box-shadow: 0 4px 10px rgba(0, 0, 0, 0.18);
    }

    Button.lift:hover {
        background: rgba(90, 169, 255, 0.30);
        border-color: rgba(90, 169, 255, 0.72);
        translate: 0 -3px;
        scale: 1.03;
        box-shadow: 0 14px 26px rgba(39, 113, 207, 0.34);
    }

    Button.lift:active {
        background: rgba(90, 169, 255, 0.42);
        translate: 0 1px;
        scale: 0.97;
        box-shadow: 0 3px 8px rgba(0, 0, 0, 0.28);
    }

    Button.shorthand:hover {
        background: rgba(116, 221, 176, 0.22);
        border-color: rgba(116, 221, 176, 0.62);
        transform: translateY(-2px) scale(1.02) rotate(1deg);
    }

    Button.rotate:hover {
        background: rgba(255, 211, 106, 0.20);
        border-color: rgba(255, 211, 106, 0.62);
        rotate: -2deg;
    }

    TextInput {
        height: 34px;
        background: rgba(255, 255, 255, 0.07);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 10px;
        color: white;
        outline: 1px solid transparent;
        outline-offset: 1px;
    }

    TextInput:focus {
        background: rgba(90, 169, 255, 0.14);
        border-color: rgba(90, 169, 255, 0.72);
        outline-color: rgba(90, 169, 255, 0.58);
        outline-width: 2px;
        outline-offset: 3px;
        translate: 0 -1px;
    }

    Panel.card-motion {
        transition-property: background, border-color, translate, scale, box-shadow;
        transition-duration: 260ms;
        transition-timing-function: ease-out;
    }

    Panel.card-motion:hover {
        background: rgba(90, 169, 255, 0.12);
        border-color: rgba(90, 169, 255, 0.54);
        translate: 0 -2px;
        scale: 1.015;
        box-shadow: 0 18px 38px rgba(18, 91, 172, 0.30);
    }

    Panel.child-motion {
        transition: transform 220ms ease-out;
    }

    Panel.child-motion:hover {
        transform: translateX(4px) scale(1.01);
    }

    Badge.spin {
        background: rgba(255, 211, 106, 0.18);
        border: 1px solid rgba(255, 211, 106, 0.48);
        color: #ffd36a;
        padding: 4px 10px;
        border-radius: 999px;
    }

    Badge.spin:hover {
        background: rgba(255, 211, 106, 0.32);
        color: white;
        rotate: 4deg;
        scale: 1.06;
    }

    Dropdown {
        height: 34px;
        background: rgba(255, 255, 255, 0.07);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 10px;
        color: white;
    }

    Dropdown:open {
        background: rgba(255, 211, 106, 0.15);
        border-color: rgba(255, 211, 106, 0.68);
        translate: 0 -1px;
        scale: 1.01;
    }

    Checkbox:checked {
        accent: #74ddb0;
        color: #74ddb0;
        translate: 2px 0;
    }
    """
)


win = dg.Window("CSS Transitions And Transforms Probe", width=820, height=660)

with dg.VLayout(class_="scroll-root"):
    dg.Label("Transitions and transforms", class_="title")
    dg.Label(
        "Hover, press, focus, open, and checked states should animate smoothly. "
        "Transforms should move borders, fills, shadows, text, and child content together.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Button motion", class_="case"):
            dg.Label("Hover lift and active press", class_="case-title")
            dg.Button("Hover lift", class_="lift")
            dg.Button("Press me", class_="lift")
            dg.Label("PASS: hover should lift; mouse-down should compress.", class_="pass")

        with dg.Panel("Transform shorthand", class_="case"):
            dg.Label("transform: translate scale rotate", class_="case-title")
            dg.Button("Shorthand transform", class_="shorthand")
            dg.Button("Rotate longhand", class_="rotate")
            dg.Label("Text should follow translate and scale. Rotation is surface-only.", class_="caption")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Focus and open states", class_="case"):
            dg.Label("Focus input and open dropdown", class_="case-title")
            dg.TextInput("Focus outline transition", placeholder="Focus me")
            dg.Dropdown(["Closed", "Open transition", "Third item"], value="Closed")
            dg.Label("PASS: focus/open should interpolate outline, color, and motion.", class_="pass")

        with dg.Panel("Card transform", class_="case card-motion"):
            dg.Label("Hover the whole card", class_="case-title")
            dg.Label("The panel surface, border, shadow, and labels should move together.")
            with dg.Panel(class_="child-motion", style={"padding": 8, "gap": 6}):
                dg.Label("Nested child")
                dg.Badge("hover badge", class_="spin")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Stateful controls", class_="case"):
            dg.Label("Checked state transition", class_="case-title")
            dg.Checkbox("Toggle checked motion", checked=True)
            dg.Checkbox("Toggle me", checked=False)
            dg.Label("PASS: checked text/accent and small translate should animate.", class_="pass")

        with dg.Panel("Mixed timing", class_="case"):
            dg.Label("Different timing functions", class_="case-title")
            dg.Button("Cubic-bezier lift", class_="lift")
            dg.Button(
                "Ease shorthand",
                class_="shorthand",
                style={
                    "transition_duration": 320,
                    "transition_timing_function": "ease-in-out",
                },
            )
            dg.Label("The second button should feel slower than the first.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
