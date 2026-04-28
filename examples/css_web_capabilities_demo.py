from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8))
app.stylesheet(
    """
    :root {
        --card-shadow: 0 18px 44px rgba(0, 0, 0, 0.34);
        --soft-shadow: 0 8px 24px rgba(0, 0, 0, 0.22);
        --hero-bg: linear-gradient(135deg, rgba(90, 169, 255, 0.22), rgba(18, 25, 39, 0.94));
    }

    Window {
        background: hsl(222, 28%, 9%);
        color: rgba(245, 248, 255, 0.92);
        font-size: 14px;
        padding: 18px;
        gap: 14px;
    }

    Panel {
        background: rgba(18, 25, 39, 0.92);
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 18px;
        box-shadow: var(--card-shadow);
        padding: 18px;
        gap: 12px;
    }

    Panel.compact {
        background: radial-gradient(circle, rgba(90, 169, 255, 0.13), rgba(18, 25, 39, 0.92));
        box-shadow: var(--soft-shadow);
    }

    Panel.hero {
        background: var(--hero-bg);
    }

    Window > VLayout > Panel.hero > HLayout > Button[key="primary-action"] {
        border-color: rgba(90, 169, 255, 0.75);
    }

    Panel.hero > HLayout > Button:first-child {
        background: linear-gradient(180deg, rgba(90, 169, 255, 0.34), rgba(255, 255, 255, 0.08));
    }

    Panel.compact > Label:first-child {
        color: rgba(160, 188, 230, 0.94);
    }

    Panel.compact > Label:nth-child(2) {
        font-variant-numeric: tabular-nums;
    }

    Panel.compact Label.caption {
        color: rgba(210, 220, 238, 0.72);
    }

    Label.kicker {
        color: rgba(160, 188, 230, 0.88);
        font-size: 11px;
        font-weight: 700;
        text-transform: uppercase;
        letter-spacing: 0.16em;
    }

    Label.headline {
        color: white;
        font-size: 24px;
        font-weight: 800;
        line-height: 1.12;
    }

    Label.caption {
        color: rgba(210, 220, 238, 0.68);
        line-height: 1.45;
    }

    Button {
        background: linear-gradient(180deg, rgba(255, 255, 255, 0.12), rgba(255, 255, 255, 0.06));
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 12px;
        color: white;
        font-weight: 700;
        box-shadow: 0 7px 16px rgba(0, 0, 0, 0.20);
    }

    Button:hover {
        background: linear-gradient(180deg, rgba(90, 169, 255, 0.34), rgba(90, 169, 255, 0.16));
        border-color: rgba(90, 169, 255, 0.60);
    }

    Button.ghost {
        background: transparent;
        box-shadow: none;
        color: rgba(230, 238, 255, 0.80);
    }

    TextInput, Dropdown, NumberInput {
        background: rgba(255, 255, 255, 0.07);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 12px;
        color: white;
    }

    Dropdown:open {
        background: linear-gradient(180deg, rgba(90, 169, 255, 0.24), rgba(255, 255, 255, 0.07));
        border-color: rgba(90, 169, 255, 0.72);
    }

    Collapsible {
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 12px;
    }

    Collapsible:collapsed {
        border-color: rgba(90, 169, 255, 0.38);
    }

    Collapsible:collapsed::indicator {
        color: rgba(160, 188, 230, 0.74);
    }

    Collapsible:expanded::header {
        background: rgba(90, 169, 255, 0.18);
        color: white;
    }

    Tab:selected {
        background: rgba(90, 169, 255, 0.18);
        color: white;
    }

    Tab:selected::accent {
        background: #5aa9ff;
    }

    Badge.info {
        background: rgba(90, 169, 255, 0.24);
        border-color: rgba(90, 169, 255, 0.50);
        color: white;
    }

    ProgressBar {
        background: rgba(255, 255, 255, 0.08);
        accent: #5aa9ff;
        border-radius: 999px;
    }
    """
)

win = dg.Window("CSS Web Capabilities", width=920, height=640)

with dg.VLayout(style={"gap": 14}):
    with dg.Panel(class_="hero"):
        dg.Label("Part 3 CSS", class_="kicker")
        dg.Label("Typography, translucent colors, and soft elevation", class_="headline")
        dg.Label(
            "This screen uses text-transform, letter-spacing, line-height, rgba(), "
            "hsl(), transparent backgrounds, var() fallbacks, box-shadow, and "
            "linear/radial gradient backgrounds plus state selectors, selector "
            "chains, key selectors, and structural child selectors.",
            class_="caption",
        )
        with dg.HLayout(style={"gap": 10, "height": 38}):
            dg.Button("Run Workflow", key="primary-action")
            dg.Button("View CSS", class_="ghost")
            dg.Badge("new", level="info")

    with dg.HLayout(style={"gap": 14, "flex": 1}):
        with dg.Panel("Inputs", class_="compact", style={"width": 320}):
            dg.Label("Controls", class_="kicker")
            dg.TextInput("Revenue forecast Q3", key="forecast-input")
            dg.Dropdown(["Operations", "Finance", "Support"], value="Finance")
            dg.NumberInput(72, min=0, max=100)
            dg.ProgressBar(0.72, label="72% confidence")

        with dg.Panel("Status", class_="compact"):
            dg.Label("Live Metrics", class_="kicker")
            dg.Label("1,248.50", style={"font_size": 30, "font_weight": 800})
            dg.Label(
                "Tabular numeric rendering keeps dashboard values stable while "
                "the card shadow separates the panel from the window surface.",
                class_="caption",
            )
            with dg.Tabs(value="metrics"):
                with dg.Tab("Metrics", value="metrics"):
                    dg.Label("Selected tab uses Tab:selected styling.", class_="caption")
                with dg.Tab("Events", value="events"):
                    dg.Label("Inactive tab keeps the base styling.", class_="caption")
            with dg.Collapsible("State selector details", expanded=False):
                dg.Label("Expanded headers use Collapsible:expanded::header.", class_="caption")


print(app.run(win))
