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
        --hero-bg:
            radial-gradient(circle at 18% 20%, rgba(116, 221, 176, 0.24) 0%, rgba(116, 221, 176, 0.07) 40%, transparent 70%),
            radial-gradient(circle at 82% 28%, rgba(90, 169, 255, 0.22) 0%, rgba(90, 169, 255, 0.06) 36%, transparent 68%),
            linear-gradient(135deg, rgba(78, 129, 196, 0.38) 0%, rgba(44, 58, 91, 0.28) 52%, rgba(17, 24, 38, 0.96) 100%);
        --inputs-offset: 560px;
        --inputs-min: 220px;
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
        background:
            radial-gradient(circle at 24% 18%, rgba(255, 255, 255, 0.13) 0%, rgba(255, 255, 255, 0.04) 28%, transparent 62%),
            radial-gradient(circle at 82% 70%, rgba(116, 221, 176, 0.10) 0%, transparent 58%),
            linear-gradient(150deg, rgba(54, 75, 116, 0.24) 0%, rgba(18, 25, 39, 0.94) 100%);
        background-noise: 0.016;
        box-shadow: var(--soft-shadow);
    }

    Panel.inputs {
        width: calc(100% - var(--inputs-offset));
        min-width: calc(var(--inputs-min) + 40px);
    }

    Panel.grid-demo {
        display: grid;
        grid-template-columns: 150px 1fr 1fr;
        grid-template-rows: 58px 76px;
        column-gap: 10px;
        row-gap: 10px;
        overflow: visible;
        padding: 10px;
    }

    Panel.grid-tile {
        background: rgba(255, 255, 255, 0.07);
        border-color: rgba(255, 255, 255, 0.12);
        border-radius: 10px;
        box-shadow: none;
        padding: 10px;
    }

    Panel.grid-sidebar {
        grid-column: 1;
        grid-row: 1 / span 2;
        background:
            radial-gradient(circle at 22% 18%, rgba(255, 211, 106, 0.18) 0%, rgba(255, 211, 106, 0.05) 34%, transparent 68%),
            radial-gradient(circle at 80% 82%, rgba(90, 169, 255, 0.16) 0%, transparent 62%),
            linear-gradient(145deg, rgba(90, 169, 255, 0.20) 0%, rgba(116, 221, 176, 0.10) 48%, rgba(18, 25, 39, 0.80) 100%);
        background-noise: 0.018;
    }

    Panel.grid-main {
        grid-column: 2 / span 2;
        grid-row: 1;
    }

    Panel.grid-left {
        grid-column: 2;
        grid-row: 2;
        position: relative;
        overflow: visible;
    }

    Panel.grid-right {
        grid-column: 3;
        grid-row: 2;
    }

    Panel.hero {
        background: var(--hero-bg);
        background-noise: 0.02;
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

    :is(Label, Badge).callout {
        color: rgba(235, 244, 255, 0.94);
    }

    :where(.metric-value) {
        font-variant-numeric: tabular-nums;
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

    Button.motion {
        background: rgba(90, 169, 255, 0.16);
        border-color: rgba(90, 169, 255, 0.38);
        transition-property: background, border-color, transform;
        transition-duration: 220ms;
        transition-timing-function: ease-out;
    }

    Button:is(:hover, :focus) {
        background: linear-gradient(180deg, rgba(90, 169, 255, 0.34), rgba(90, 169, 255, 0.16));
        border-color: rgba(90, 169, 255, 0.60);
    }

    Button.motion:hover {
        background: rgba(90, 169, 255, 0.42);
        border-color: rgba(90, 169, 255, 0.72);
        transform: translateY(-2px) scale(1.02);
    }

    Button.ghost {
        background: transparent;
        box-shadow: none;
        color: rgba(230, 238, 255, 0.80);
    }

    Button:not(:disabled):not(.ghost) {
        border-width: 2px;
    }

    TextInput, Dropdown, NumberInput {
        background: rgba(255, 255, 255, 0.07);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 12px;
        color: white;
        transition-property: background, border-color, transform;
        transition-duration: 180ms;
        transition-timing-function: ease-out;
    }

    Dropdown:open {
        background: rgba(90, 169, 255, 0.24);
        border-color: rgba(90, 169, 255, 0.72);
        transform: translateY(-1px);
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

    Tab {
        transition-property: background, color, border-color;
        transition-duration: 180ms;
        transition-timing-function: ease-out;
    }

    Tab:selected::accent {
        background: #5aa9ff;
    }

    Badge.info {
        background: rgba(90, 169, 255, 0.24);
        border-color: rgba(90, 169, 255, 0.50);
        color: white;
    }

    Badge.offset-demo {
        position: relative;
        top: 0;
        left: 6px;
        z-index: 2;
        background: rgba(116, 221, 176, 0.22);
        border-color: rgba(116, 221, 176, 0.48);
        color: white;
    }

    Badge.stack-demo {
        position: relative;
        top: 0;
        left: 0;
        z-index: 1;
        background: rgba(255, 255, 255, 0.10);
        border-color: rgba(255, 255, 255, 0.18);
        color: rgba(235, 244, 255, 0.88);
    }

    Badge.pin-demo {
        position: absolute;
        top: 8px;
        right: 8px;
        z-index: 3;
        background: rgba(255, 211, 106, 0.22);
        border-color: rgba(255, 211, 106, 0.55);
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
        with dg.VLayout(style={"gap": 3}):
            dg.Label(
                "Text controls, rgba()/hsl(), transparent fills, var() fallbacks, "
                "box-shadow and softly blended multi-stop gradient backgrounds.",
                class_="caption callout",
            )
            dg.Label(
                "State selectors, selector chains, key selectors, structural child "
                "selectors, and dynamic :is() / :where() / :not().",
                class_="caption callout",
            )
            dg.Label(
                "Hover/open/selected transitions, paint-only transforms, percent "
                "and var() calc sizing, CSS Grid, overflow, relative offsets, and absolute pins.",
                class_="caption callout",
            )
        with dg.HLayout(style={"gap": 10, "height": 38}):
            dg.Button("Run Workflow", key="primary-action")
            dg.Button("Hover Motion", class_="motion")
            dg.Button("View CSS", class_="ghost")
            dg.Badge("new", level="info")

    with dg.HLayout(style={"gap": 14, "flex": 1}):
        with dg.Panel("Inputs", class_="compact inputs"):
            dg.Label("Controls", class_="kicker")
            dg.TextInput("Revenue forecast Q3", key="forecast-input")
            dg.Dropdown(["Operations", "Finance", "Support"], value="Finance")
            dg.NumberInput(72, min=0, max=100)
            dg.ProgressBar(0.72, label="72% confidence")

        with dg.Panel("Status", class_="compact"):
            dg.Label("Live Metrics", class_="kicker")
            dg.Label(
                "1,248.50",
                class_="metric-value",
                style={"font_size": 30, "font_weight": 800},
            )
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
            with dg.Panel("Grid slice", class_="grid-demo"):
                with dg.Panel(class_="grid-tile grid-sidebar"):
                    dg.Label("Sidebar", class_="kicker")
                    dg.Label("span 2", class_="caption")
                with dg.Panel(class_="grid-tile grid-main"):
                    dg.Label("Main track", class_="caption")
                with dg.Panel(class_="grid-tile grid-left"):
                    dg.Label("1fr", class_="caption")
                    dg.Badge("pin", level="warning", class_="pin-demo")
                with dg.Panel(class_="grid-tile grid-right"):
                    dg.Label("1fr", class_="caption")
                    with dg.HLayout(style={"gap": 0, "height": 22}):
                        dg.Badge("base", level="info", class_="stack-demo")
                        dg.Badge("offset", level="success", class_="offset-demo")


print(app.run(win))
