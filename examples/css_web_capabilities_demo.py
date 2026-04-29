from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8))
app.stylesheet(
    """
    @font-face {
        font-family: "Dragon Demo UI";
        src: url("C:/Windows/Fonts/segoeui.ttf") format("truetype");
    }

    :root {
        --card-shadow: 0 2px 10px rgba(0, 0, 0, 0.18), 0 18px 44px rgba(0, 0, 0, 0.34);
        --soft-shadow: 0 1px 5px rgba(0, 0, 0, 0.16), 0 8px 24px rgba(0, 0, 0, 0.22);
        --hero-bg:
            radial-gradient(circle at 18% 20%, rgba(116, 221, 176, 0.24) 0%, rgba(116, 221, 176, 0.07) 40%, transparent 70%),
            radial-gradient(circle at 82% 28%, rgba(90, 169, 255, 0.22) 0%, rgba(90, 169, 255, 0.06) 36%, transparent 68%),
            linear-gradient(135deg, rgba(78, 129, 196, 0.38) 0%, rgba(44, 58, 91, 0.28) 52%, rgba(17, 24, 38, 0.96) 100%);
        --panel-border: rgba(255, 255, 255, 0.10);
        --button-shadow-color: rgba(0, 0, 0, 0.20);
        --inputs-offset: 560px;
        --inputs-min: 220px;
    }

    @keyframes live-pulse {
        from {
            opacity: 0.70;
            scale: 0.97;
        }

        50% {
            opacity: 1;
            scale: 1.05;
        }

        to {
            opacity: 0.82;
            scale: 1;
        }
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
        border: 1px solid var(--panel-border);
        border-radius: 18px;
        box-shadow: var(--card-shadow);
        padding: 18px;
        gap: 12px;
    }

    Panel::scrollbar-track {
        width: 5px;
        padding: 14px;
        background: rgba(255, 255, 255, 0.10);
        border-radius: 999px;
    }

    Panel::scrollbar-thumb {
        width: 5px;
        background: rgba(90, 169, 255, 0.66);
        border-radius: 999px;
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
        grid-template-columns: fit-content(190px) repeat(1, repeat(2, minmax(150px, 1fr)));
        grid-template-rows: fit-content(58px) 76px;
        grid-template-areas:
            "sidebar main main"
            "sidebar left right";
        column-gap: calc(1% + 6px);
        row-gap: calc(1% + 6px);
        overflow: visible;
        padding: calc(1% + 8px);
    }

    Panel.grid-tile {
        background: rgba(255, 255, 255, 0.07);
        border-color: rgba(255, 255, 255, 0.12);
        border-radius: 10px;
        box-shadow:
            inset 0 1px 0 rgba(255, 255, 255, 0.12),
            inset 0 -16px 28px rgba(0, 0, 0, 0.14);
        padding: calc(1% + 6px);
    }

    Panel.grid-demo > * {
        opacity: 0.96;
    }

    Panel.grid-demo > *:nth-child(3n + 1) {
        border-color: rgba(255, 211, 106, 0.36);
    }

    Panel.grid-demo > *:nth-child(2 of Panel.grid-demo > Panel.grid-tile) {
        background: rgba(90, 169, 255, 0.12);
        border-color: rgba(90, 169, 255, 0.36);
    }

    @supports (display: grid) and (selector(Panel.grid-demo > Panel.grid-tile)) {
        Panel.grid-demo {
            border-color: rgba(116, 221, 176, 0.30);
        }
    }

    @supports (backdrop-filter: blur(12px)) {
        Panel.grid-sidebar {
            backdrop-filter: blur(12px);
        }
    }

    Panel.grid-sidebar {
        grid-area: sidebar;
        background:
            radial-gradient(circle at 22% 18%, rgba(255, 211, 106, 0.18) 0%, rgba(255, 211, 106, 0.05) 34%, transparent 68%),
            radial-gradient(circle at 80% 82%, rgba(90, 169, 255, 0.16) 0%, transparent 62%),
            linear-gradient(145deg, rgba(90, 169, 255, 0.20) 0%, rgba(116, 221, 176, 0.10) 48%, rgba(18, 25, 39, 0.80) 100%);
        background-noise: 0.018;
    }

    Panel.grid-main {
        grid-area: main;
    }

    Panel.grid-main::after {
        content: attr(title);
        width: 118px;
        padding: 8px;
        color: rgba(116, 221, 176, 0.92);
        font-size: 11px;
        font-weight: 800;
        text-align: right;
        text-transform: uppercase;
        letter-spacing: 0.08em;
    }

    Panel.grid-left {
        grid-area: left;
        position: relative;
        overflow: visible;
    }

    Panel.grid-right {
        grid-area: right;
    }

    Panel.horizontal-strip {
        width: 280px;
        height: 96px;
        overflow-x: auto;
        overflow-y: hidden;
        padding: 12px;
        gap: 0;
    }

    HLayout.strip-row {
        width: 420px;
        height: 36px;
        gap: 8px;
        flex-shrink: 0;
    }

    Button.strip-item {
        width: 128px;
        flex-shrink: 0;
    }

    Panel.hero {
        background: var(--hero-bg);
        background-noise: 0.02;
        backdrop-filter: blur(18px);
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
        font-family: "Dragon Demo UI";
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
        box-shadow: 0 7px 16px var(--button-shadow-color);
    }

    Button.motion {
        background: rgba(90, 169, 255, 0.16);
        border-color: rgba(90, 169, 255, 0.38);
        transition-property: background, border-color, translate, scale;
        transition-duration: 220ms;
        transition-timing-function: cubic-bezier(0.16, 1, 0.3, 1);
    }

    Button:is(:hover, :focus) {
        background: linear-gradient(180deg, rgba(90, 169, 255, 0.34), rgba(90, 169, 255, 0.16));
        border-color: rgba(90, 169, 255, 0.60);
    }

    Button.motion:hover {
        background: rgba(90, 169, 255, 0.42);
        border-color: rgba(90, 169, 255, 0.72);
        translate: 0 -2px;
        scale: 1.02;
    }

    Button.ghost {
        background: transparent;
        border: none;
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
        transition-property: background, border-color, translate;
        transition-duration: 180ms;
        transition-timing-function: ease-out;
    }

    Dropdown:open {
        background: rgba(90, 169, 255, 0.24);
        border-color: rgba(90, 169, 255, 0.72);
        translate: 0 -1px;
    }

    Collapsible {
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 12px;
        transition-property: background, border-color;
        transition-duration: 180ms;
        transition-timing-function: ease-out;
    }

    Collapsible:collapsed {
        background: rgba(90, 169, 255, 0.10);
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

    Badge.live {
        background: rgba(116, 221, 176, 0.24);
        border-color: rgba(116, 221, 176, 0.62);
        color: white;
        animation: 1400ms cubic-bezier(0.16, 1, 0.3, 1) infinite alternate both running live-pulse;
    }

    Badge.paused {
        background: rgba(116, 221, 176, 0.16);
        border-color: rgba(116, 221, 176, 0.38);
        color: rgba(232, 245, 255, 0.76);
        animation: 1400ms cubic-bezier(0.16, 1, 0.3, 1) infinite alternate both paused live-pulse;
    }

    Badge[level] {
        border-width: 1px;
    }

    Badge[text="FIXED" i] {
        border-color: rgba(116, 221, 176, 0.70);
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

    Badge.fixed-demo {
        position: fixed;
        right: 18px;
        bottom: 18px;
        z-index: 8;
        background: rgba(90, 169, 255, 0.28);
        border-color: rgba(90, 169, 255, 0.62);
        color: white;
        box-shadow: 0 10px 24px rgba(0, 0, 0, 0.28);
    }

    ProgressBar {
        background: rgba(255, 255, 255, 0.08);
        accent: #5aa9ff;
        border-radius: 999px;
    }

    @media (max-width: 760px) {
        Window {
            padding: 12px;
            gap: 10px;
        }

        Panel {
            padding: 12px;
        }

        Panel.inputs {
            width: auto;
            min-width: 220px;
        }

        Label.headline {
            font-size: 20px;
        }
    }

    @media (min-width: 1100px) {
        :root {
            --grid-min: 180px;
        }

        Label.headline {
            font-size: 28px;
        }

        Panel.grid-demo {
            grid-template-columns: fit-content(220px) repeat(1, repeat(2, minmax(var(--grid-min), 1fr)));
        }
    }

    @media (orientation: landscape) {
        Panel.horizontal-strip {
            width: 320px;
        }
    }

    @media (min-aspect-ratio: 4/3) {
        Panel.grid-demo {
            grid-template-columns: fit-content(220px) repeat(1, repeat(2, minmax(160px, 1fr)));
        }
    }

    @media (min-resolution: 1dppx) {
        Button.ghost {
            border-color: rgba(255, 255, 255, 0.20);
        }
    }

    @media (color-gamut: srgb) {
        Badge.live {
            border-color: rgba(255, 255, 255, 0.24);
        }
    }

    @media (update: fast) {
        Badge.live {
            box-shadow: 0 0 0 1px rgba(116, 221, 176, 0.24), 0 8px 20px rgba(116, 221, 176, 0.16);
        }
    }

    @media (scripting: none) {
        Badge.info {
            border-width: 1px;
            border-color: rgba(90, 169, 255, 0.70);
        }
    }

    @media (forced-colors: none) {
        Button.ghost {
            color: rgba(235, 244, 255, 0.92);
        }
    }

    @media (prefers-contrast: no-preference) {
        Panel.hero {
            box-shadow:
                0 2px 10px rgba(0, 0, 0, 0.18),
                0 18px 44px rgba(0, 0, 0, 0.34);
        }
    }

    @media (inverted-colors: none) {
        Badge.paused {
            border-color: rgba(116, 221, 176, 0.46);
        }
    }

    @media (dynamic-range: standard) {
        Badge.info {
            box-shadow: 0 0 0 1px rgba(90, 169, 255, 0.18);
        }
    }

    @media (video-dynamic-range: standard) {
        Panel.status-card {
            border-color: rgba(255, 255, 255, 0.16);
        }
    }

    @media (pointer: fine) and (hover: hover) {
        Button.motion:hover {
            box-shadow:
                0 8px 18px rgba(90, 169, 255, 0.28),
                0 0 0 1px rgba(255, 255, 255, 0.12);
        }
    }

    @media (prefers-color-scheme: dark) {
        Panel.hero {
            border-color: rgba(116, 221, 176, 0.28);
        }
    }

    @media (prefers-reduced-motion: reduce) {
        Badge.live {
            animation-play-state: paused;
        }
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
                "selectors with filtered :nth-child(), attribute presence/string/case selectors, "
                "and dynamic :is() / :where() / :not().",
                class_="caption callout",
            )
            dg.Label(
                "Hover/open/selected transitions with custom easing, keyframe animation, generated content, frosted backdrop-filter surfaces, paint-only transforms, percent "
                "and var() calc sizing/spacing, media-scoped CSS variables, CSS Grid, overflow, relative offsets, absolute pins, and fixed badges.",
                class_="caption callout",
            )
            dg.Label(
                "Resize the window or change the OS app theme to exercise width, height, orientation, aspect-ratio, resolution, pointer/hover, update, scripting, forced-colors, contrast, inverted-colors, dynamic-range, video-dynamic-range, color-gamut, color-scheme, and reduced-motion @media rules.",
                class_="caption callout",
            )
        with dg.HLayout(style={"gap": 10, "height": 38}):
            dg.Button("Run Workflow", key="primary-action")
            dg.Button("Hover Motion", class_="motion")
            dg.Button("View CSS", class_="ghost")
            dg.Badge("live", level="success", class_="live")
            dg.Badge("paused", level="info", class_="paused")

    with dg.HLayout(style={"gap": 14, "flex": 1}):
        with dg.Panel("Inputs", class_="compact inputs"):
            dg.Label("Controls", class_="kicker")
            dg.TextInput("Revenue forecast Q3", key="forecast-input")
            dg.Dropdown(["Operations", "Finance", "Support"], value="Finance")
            dg.NumberInput(72, min=0, max=100)
            dg.ProgressBar(0.72, label="72% confidence")

        with dg.Panel("Status", class_="compact status-card"):
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
            with dg.Panel("Horizontal overflow", class_="horizontal-strip"):
                with dg.HLayout(class_="strip-row"):
                    dg.Button("Forecast", class_="strip-item")
                    dg.Button("Pipeline", class_="strip-item")
                    dg.Button("Capacity", class_="strip-item")
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

    dg.Badge("fixed", level="info", class_="fixed-demo")


print(app.run(win))
