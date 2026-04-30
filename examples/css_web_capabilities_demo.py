from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError:
    np = None


app = dg.App(theme=dg.Theme.dark(accent="oklch(72% 0.14 245)", radius=8))
app.stylesheet(
    """
    @font-face {
        font-family: "Dragon Demo UI";
        src: url("file:///C:/Windows/Fonts/segoeui.ttf") format("truetype");
    }

    :root {
        --card-shadow: 0 2px 10px rgba(0, 0, 0, 0.18), 0 18px 44px rgba(0, 0, 0, 0.34);
        --soft-shadow: 0 1px 5px rgba(0, 0, 0, 0.16), 0 8px 24px rgba(0, 0, 0, 0.22);
        --hero-bg:
            radial-gradient(circle at 18% 20%, rgba(116, 221, 176, 0.24) 0%, rgba(116, 221, 176, 0.07) 40%, transparent 70%),
            radial-gradient(circle at 82% 28%, rgba(90, 169, 255, 0.22) 0%, rgba(90, 169, 255, 0.06) 36%, transparent 68%),
            linear-gradient(135deg, rgba(78, 129, 196, 0.38) 0%, rgba(44, 58, 91, 0.28) 52%, transparent 100%);
        --panel-border: lch(100% 0 0 / 10%);
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
        background: color(srgb 0.353 0.663 1 / 66%);
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

    Panel.scoped-vars {
        --scope-bg:
            radial-gradient(circle at 18% 22%, rgba(116, 221, 176, 0.18), transparent 58%),
            linear-gradient(135deg, rgba(90, 169, 255, 0.16), rgba(255, 255, 255, 0.055));
        --scope-border: rgba(116, 221, 176, 0.34);
        --scope-radius: 12px;
        background: var(--scope-bg);
        border-color: var(--scope-border);
        border-radius: var(--scope-radius);
        padding: 10px;
        box-shadow: inset 0 1px 0 var(--scope-border);
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
        grid-auto-flow: row dense;
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

    Panel.grid-demo > *:nth-child(2 of Panel.grid-tile:first-child, Panel.grid-tile:last-child) {
        border-radius: 14px;
    }

    Panel.grid-demo > *:nth-last-child(2) {
        border-width: 2px;
    }

    Panel.grid-demo > *:nth-last-child(1 of Panel.grid-tile) {
        box-shadow:
            inset 0 1px 0 rgba(255, 255, 255, 0.13),
            0 0 0 1px rgba(116, 221, 176, 0.18),
            inset 0 -16px 28px rgba(116, 221, 176, 0.10);
    }

    Panel.grid-tile:has(> Badge.pin-demo) {
        border-color: rgba(255, 211, 106, 0.62);
        box-shadow:
            inset 0 1px 0 rgba(255, 255, 255, 0.12),
            0 0 0 1px rgba(255, 211, 106, 0.14),
            inset 0 -16px 28px rgba(0, 0, 0, 0.14);
    }

    Panel.grid-tile:has(> HLayout:last-child) {
        background: rgba(116, 221, 176, 0.11);
        border-color: rgba(116, 221, 176, 0.36);
    }

    Panel.grid-tile:has(> Label:only-child) {
        background: rgba(90, 169, 255, 0.14);
        border-color: rgba(90, 169, 255, 0.48);
    }

    Panel.empty-marker:empty {
        height: 6px;
        padding: 0;
        background: rgba(255, 211, 106, 0.34);
        border-color: rgba(255, 211, 106, 0.50);
        border-radius: 999px;
    }

    Panel.grid-tile:has(> Panel.empty-marker:empty) {
        border-color: rgba(255, 211, 106, 0.72);
    }

    Panel.grid-demo:has(Panel.grid-right > HLayout) {
        box-shadow:
            0 0 0 1px rgba(116, 221, 176, 0.10),
            0 14px 34px rgba(0, 0, 0, 0.20);
    }

    Panel.grid-left:has(+ Panel.grid-right) {
        box-shadow:
            0 0 0 1px rgba(90, 169, 255, 0.24),
            inset 0 1px 0 rgba(255, 255, 255, 0.14),
            inset 0 -16px 28px rgba(90, 169, 255, 0.12);
    }

    Panel.grid-demo:has(Panel.grid-right:has(Badge.offset-demo)) {
        border-width: 2px;
    }

    Panel.grid-demo:has(Panel.grid-right:has(> HLayout) > Label) {
        border-color: rgba(116, 221, 176, 0.44);
    }

    @supports (display: grid) and (selector(Panel.grid-demo > Panel.grid-tile)) {
        Panel.grid-demo {
            border-color: rgba(116, 221, 176, 0.30);
        }
    }

    @supports (backdrop-filter: blur(12px) brightness(112%) saturate(1.1)) {
        Panel.grid-sidebar {
            backdrop-filter: blur(12px) brightness(112%) saturate(1.1);
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

    Panel.auto-flow-grid {
        display: grid;
        grid-template-columns: repeat(3, minmax(58px, 1fr));
        grid-template-rows: repeat(2, 36px);
        grid-auto-flow: row dense;
        gap: 8px;
        padding: 10px;
        overflow: hidden;
    }

    Panel.auto-flow-grid > Panel.flow-tile {
        min-width: 0;
        height: 36px;
        padding: 6px;
        border-radius: 9px;
        background: rgba(255, 255, 255, 0.08);
        border-color: rgba(255, 255, 255, 0.13);
        box-shadow: none;
    }

    Panel.flow-wide {
        grid-column: auto / span 2;
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

    HLayout.layout-strip {
        width: 280px;
        height: 44px;
        overflow-x: auto;
        overflow-y: hidden;
        gap: 8px;
        flex-shrink: 0;
    }

    HLayout.layout-strip::scrollbar-track {
        width: 5px;
        padding: 9px;
        background: rgba(255, 255, 255, 0.09);
        border-radius: 999px;
    }

    HLayout.layout-strip::scrollbar-thumb {
        width: 5px;
        background: rgba(116, 221, 176, 0.68);
        border-radius: 999px;
    }

    Sidebar.scroll-demo {
        width: 260px;
        height: 92px;
        overflow-y: auto;
        padding: 12px;
        gap: 6px;
        background: rgba(90, 169, 255, 0.10);
        border-color: rgba(90, 169, 255, 0.30);
        border-radius: 12px;
    }

    Sidebar.scroll-demo::scrollbar-track {
        width: 5px;
        padding: 10px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    Sidebar.scroll-demo::scrollbar-thumb {
        width: 5px;
        background: rgba(255, 211, 106, 0.70);
        border-radius: 999px;
    }

    Sidebar.scroll-demo Label {
        color: rgba(232, 240, 255, 0.78);
        font-size: 12px;
    }

    Button.shadow-clip-item {
        width: 210px;
        flex-shrink: 0;
        background: rgba(116, 221, 176, 0.18);
        border-color: rgba(116, 221, 176, 0.36);
        box-shadow: 0 12px 26px rgba(0, 0, 0, 0.34);
    }

    Button.layout-strip-item {
        width: 116px;
        flex-shrink: 0;
    }

    Button.layout-strip-item:nth-child(2) {
        margin-left: 12px;
    }

    Panel.hero {
        background-color: rgba(17, 24, 38, 0.96);
        background-image: var(--hero-bg);
        background-noise: 0.02;
        backdrop-filter: blur(18px) brightness(108%) saturate(1.08);
    }

    Modal.css-scrim-demo {
        background:
            radial-gradient(circle at 24% 16%, rgba(90, 169, 255, 0.18), transparent 58%),
            linear-gradient(180deg, rgba(18, 25, 39, 0.98), rgba(12, 17, 28, 0.98));
        border-color: rgba(116, 221, 176, 0.34);
        border-radius: 18px;
        box-shadow: 0 22px 58px rgba(0, 0, 0, 0.44);
        padding: 18px;
        gap: 12px;
    }

    Modal.css-scrim-demo::scrim {
        background: rgba(4, 8, 16, 0.66);
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
        outline: 1px solid transparent;
        outline-offset: 1px;
        transition-property: background, border-color, translate, scale, outline-color, outline-offset;
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
        outline-color: rgba(90, 169, 255, 0.28);
        outline-offset: 3px;
        translate: 0 -2px;
        scale: 1.02;
    }

    Button.motion:active {
        background: rgba(90, 169, 255, 0.52);
        border-color: rgba(154, 210, 255, 0.92);
        outline-color: rgba(154, 210, 255, 0.38);
        outline-offset: 2px;
        translate: 0 0;
        scale: 0.98;
    }

    Button.ghost {
        background: transparent;
        border-style: none;
        box-shadow: none;
        color: hwb(216 90% 0% / 80%);
    }

    Button:not(:disabled):not(.ghost) {
        border-style: solid;
        border-width: 2px;
    }

    TextInput, TextArea, Dropdown, NumberInput {
        background: rgba(255, 255, 255, 0.07);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 12px;
        color: white;
        transition-property: background, border-color, translate;
        transition-duration: 180ms;
        transition-timing-function: ease-out;
    }

    TextArea.rows-demo {
        text-area-rows: 3;
        color: rgba(232, 240, 255, 0.86);
        line-height: 1.35;
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

    Panel.status-card:has(> Checkbox:checked) {
        border-color: rgba(116, 221, 176, 0.46);
    }

    Panel.status-card:has(> Collapsible:collapsed) {
        box-shadow:
            0 0 0 1px rgba(255, 211, 106, 0.10),
            0 18px 44px rgba(0, 0, 0, 0.34);
    }

    Checkbox {
        background: transparent;
        border: 1px solid transparent;
        border-radius: 10px;
        transition-property: background, border-color;
        transition-duration: 180ms;
        transition-timing-function: ease-out;
    }

    Checkbox:checked {
        background: rgba(116, 221, 176, 0.11);
        border-color: rgba(116, 221, 176, 0.38);
    }

    Panel.status-card > *:nth-child(1 of Checkbox:checked) {
        background: rgba(116, 221, 176, 0.11);
        border-color: rgba(116, 221, 176, 0.38);
    }

    Panel.status-card > *:nth-child(1 of Collapsible:collapsed) {
        border-color: rgba(255, 211, 106, 0.42);
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
        animation-name: live-pulse, unused-pulse;
        animation-duration: 1400ms, 400ms;
        animation-timing-function: steps(4, end), linear;
        animation-iteration-count: infinite, 1;
        animation-direction: alternate, normal;
        animation-fill-mode: both, none;
        animation-play-state: running, paused;
    }

    Badge.paused {
        background: rgba(116, 221, 176, 0.16);
        border-color: rgba(116, 221, 176, 0.38);
        color: rgba(232, 245, 255, 0.76);
        animation: 1400ms step-start infinite alternate both paused live-pulse;
        animation-play-state: paused, running;
    }

    Badge.finite {
        background: rgba(255, 209, 102, 0.20);
        border-color: rgba(255, 209, 102, 0.52);
        outline: 1px solid rgba(255, 209, 102, 0.36);
        outline-offset: 3px;
        color: white;
        animation: 900ms -450ms ease-out 2.5 alternate both live-pulse;
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

    DataFrameTable.web-table {
        height: 142px;
        background: rgba(255, 255, 255, 0.055);
        border: 1px solid rgba(90, 169, 255, 0.24);
        border-radius: 12px;
        color: rgba(232, 240, 255, 0.84);
        font-size: 12px;
        table-row-height: 24px;
        table-header-height: 30px;
        table-column-width: 116px;
        table-index-width: 42px;
    }

    DataFrameTable.web-table::header {
        background: rgba(90, 169, 255, 0.18);
        color: rgba(245, 248, 255, 0.94);
        font-weight: 800;
        text-transform: uppercase;
        letter-spacing: 0.08em;
    }

    DataFrameTable.web-table::row-selected {
        background: rgba(116, 221, 176, 0.18);
    }

    DataFrameTable.web-table::grid-line {
        background: rgba(255, 255, 255, 0.09);
    }

    Scatter3D.web-scatter {
        height: 132px;
        background: rgba(3, 8, 18, 0.36);
        border: 1px solid rgba(116, 221, 176, 0.28);
        border-radius: 12px;
        scatter-point-size: 6px;
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

    @media (-webkit-device-pixel-ratio >= 1) {
        Button.motion {
            border-color: rgba(116, 221, 176, 0.30);
        }
    }

    @media (device-width >= 900px) and (device-aspect-ratio >= 4/3) {
        Panel.status-card {
            border-radius: 14px;
        }
    }

    @media (horizontal-viewport-segments: 1) and (vertical-viewport-segments: 1) {
        Badge.info {
            outline: 1px solid rgba(90, 169, 255, 0.18);
            outline-offset: 2px;
        }
    }

    @media (color-gamut: srgb) {
        Badge.live {
            border-color: rgba(255, 255, 255, 0.24);
        }
    }

    @media (video-color-gamut: srgb) {
        Panel.status-card {
            border-color: rgba(90, 169, 255, 0.20);
        }
    }

    @media (color >= 8) and (monochrome: 0) {
        Panel.status-card {
            outline: 1px solid rgba(90, 169, 255, 0.12);
            outline-offset: 2px;
        }
    }

    @media (color-index: 0) {
        Badge.paused {
            opacity: 0.96;
        }
    }

    @media (scan: progressive) and (environment-blending: opaque) {
        Panel.hero {
            outline: 1px solid rgba(255, 255, 255, 0.10);
            outline-offset: 3px;
        }
    }

    @media (grid: 0) {
        Badge.live {
            outline: 1px solid rgba(116, 221, 176, 0.18);
            outline-offset: 2px;
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

    @media (display-mode: standalone) {
        Button.ghost {
            outline: 1px solid rgba(255, 255, 255, 0.14);
            outline-offset: 2px;
        }
    }

    @media (overflow-block: scroll) and (overflow-inline: scroll) {
        Panel.grid-demo {
            outline: 1px solid rgba(116, 221, 176, 0.16);
            outline-offset: 2px;
        }
    }

    @media (pointer: fine) and (hover: hover) {
        Button.motion:hover {
            box-shadow:
                0 8px 18px rgba(90, 169, 255, 0.28),
                0 0 0 1px rgba(255, 255, 255, 0.12);
        }
    }

    @media (nav-controls: none) {
        Button.ghost {
            background: rgba(255, 255, 255, 0.055);
        }
    }

    @supports font-format(woff) {
        Label.caption {
            letter-spacing: 0.015em;
        }
    }

    @supports font-format(ttf) {
        Label.kicker {
            font-style: italic;
        }
    }

    @supports at-rule(@media) {
        Badge.info {
            border-radius: 9px;
        }
    }

    @supports font-tech(features-opentype) {
        Label.value {
            font-variant-numeric: tabular-nums;
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

    @media (prefers-reduced-transparency: no-preference) {
        Panel.hero {
            backdrop-filter: blur(10px) brightness(105%) saturate(1.04);
        }
    }

    @media (prefers-reduced-data: no-preference) {
        Badge.live {
            animation-play-state: running;
        }
    }
    """
)


class DashboardFrame:
    columns = ("metric", "value", "delta")
    dtypes = ("str", "str", "str")
    shape = (4, 3)

    metric = ("Revenue", "Pipeline", "Capacity", "Risk")
    value = ("1,248", "84%", "72%", "Low")
    delta = ("+8%", "+3%", "-2%", "flat")

    def __getitem__(self, column: str) -> tuple[str, ...]:
        return getattr(self, column)


class ScatterFrame:
    def __init__(self) -> None:
        if np is None:
            raise RuntimeError("ScatterFrame requires NumPy")
        t = np.linspace(0.0, 1.0, 1200, dtype=np.float32)
        theta = t * np.float32(9.0 * 3.14159)
        radius = np.float32(0.35) + t * np.float32(1.8)
        self.x = np.cos(theta) * radius
        self.y = np.sin(theta) * radius
        self.z = (t - np.float32(0.5)) * np.float32(2.0)
        self.columns = ("x", "y", "z")
        self.dtypes = ("float32", "float32", "float32")
        self.shape = (len(self.x), 3)


win = dg.Window("CSS Web Capabilities", width=920, height=640)

with dg.VLayout(style={"gap": 14}):
    with dg.Panel(class_="hero"):
        dg.Label("Part 3 CSS", class_="kicker")
        dg.Label("Typography, translucent colors, and soft elevation", class_="headline")
        with dg.VLayout(style={"gap": 3}):
            dg.Label(
                "Text controls, rgba()/hsl()/hwb(), transparent fills, var() fallbacks, "
                "box-shadow and softly blended multi-stop gradient backgrounds.",
                class_="caption callout",
            )
            dg.Label(
                "State selectors, selector chains, key selectors, structural child "
                "selectors with :empty, :only-child, :nth-last-child(), and structural filtered :nth-child(), attribute presence/string/case selectors, "
                "dynamic :is() / :where() / :not(), data-backed state filters, direct-child/sibling/nested target and ancestor-chain :has(), and descendant-chain :has().",
                class_="caption callout",
            )
            dg.Label(
                "Hover/open/selected transitions with custom and step easing, keyframe animation, generated content, frosted backdrop-filter surfaces, paint-only transforms, percent "
                "and var() calc sizing/spacing, per-side margins, media-scoped CSS variables, CSS Grid, overflow, relative offsets, absolute pins, and fixed badges.",
                class_="caption callout",
            )
            dg.Label(
                "Resize the window or change the OS app theme to exercise width, height, orientation, aspect-ratio, resolution, device size, viewport segments, color depth, scan/grid, environment-blending, pointer/hover, nav-controls, overflow-block/inline, update, scripting, forced-colors, contrast, inverted-colors, dynamic-range, video color/dynamic range, display-mode, color-gamut, color-scheme, reduced-motion, reduced-transparency, and reduced-data @media rules.",
                class_="caption callout",
            )
        with dg.HLayout(style={"gap": 10, "height": 38}):
            dg.Button("Run Workflow", key="primary-action")
            dg.Button("Hover Motion", class_="motion")
            dg.Button("View CSS", class_="ghost", on_click=lambda: css_modal.show())
            dg.Badge("live", level="success", class_="live")
            dg.Badge("paused", level="info", class_="paused")
            dg.Badge("2.5x", level="warning", class_="finite")

    with dg.HLayout(style={"gap": 14, "flex": 1}):
        with dg.Panel("Inputs", class_="compact inputs"):
            dg.Label("Controls", class_="kicker")
            dg.TextInput("Revenue forecast Q3", key="forecast-input")
            dg.TextArea(
                "CSS sets this TextArea to three visible rows.",
                placeholder="Notes",
                rows=1,
                class_="rows-demo",
            )
            dg.Dropdown(["Operations", "Finance", "Support"], value="Finance")
            dg.NumberInput(72, min=0, max=100)
            dg.ProgressBar(0.72, label="72% confidence")

        with dg.Panel("Status", class_="compact status-card"):
            dg.Label("Live Metrics", class_="kicker")
            dg.Label(
                "1,248.50",
                class_="metric-value",
                style={
                    "font_size": 30,
                    "font_weight": 800,
                    "color": "lab(78% -12 -34)",
                },
            )
            dg.Label(
                "Tabular numeric rendering keeps dashboard values stable while "
                "the card shadow separates the panel from the window surface.",
                class_="caption",
            )
            with dg.Panel(class_="scoped-vars"):
                dg.Label("Selector-local CSS variables", class_="caption")
            dg.DataFrameTable(
                DashboardFrame(),
                page_size=8,
                sample_rows=4,
                class_="web-table",
            )
            if np is not None:
                dg.Scatter3D(
                    ScatterFrame(),
                    x="x",
                    y="y",
                    z="z",
                    colormap="turbo",
                    class_="web-scatter",
                )
            with dg.Tabs(value="metrics"):
                with dg.Tab("Metrics", value="metrics"):
                    dg.Label("Selected tab uses Tab:selected styling.", class_="caption")
                with dg.Tab("Events", value="events"):
                    dg.Label("Inactive tab keeps the base styling.", class_="caption")
            dg.Checkbox("Prop-backed :has() and nth-child filter", checked=True)
            with dg.Collapsible("State selector details", expanded=False):
                dg.Label("Expanded headers use Collapsible:expanded::header.", class_="caption")
            with dg.Panel("Horizontal overflow", class_="horizontal-strip"):
                with dg.HLayout(class_="strip-row"):
                    dg.Button("Forecast", class_="strip-item")
                    dg.Button("Pipeline", class_="strip-item")
                    dg.Button("Capacity", class_="strip-item")
            with dg.HLayout(class_="layout-strip"):
                dg.Button("Layout A", class_="layout-strip-item")
                dg.Button("Layout B", class_="layout-strip-item")
                dg.Button("Layout C", class_="layout-strip-item")
            with dg.Panel(class_="auto-flow-grid"):
                with dg.Panel(class_="flow-tile flow-wide"):
                    dg.Label("span 2", class_="caption")
                with dg.Panel(class_="flow-tile flow-wide"):
                    dg.Label("wide", class_="caption")
                with dg.Panel(class_="flow-tile"):
                    dg.Label("dense", class_="caption")
                with dg.Panel(class_="flow-tile"):
                    dg.Label("fill", class_="caption")
                with dg.Panel(class_="flow-tile"):
                    dg.Label("tail", class_="caption")
            with dg.Sidebar(title="Scrollable nav", width=260, class_="scroll-demo"):
                dg.Label("Sidebar scrollbar part")
                dg.Label("Track and thumb use Sidebar::scrollbar-*")
                dg.Label(
                    "The same parts now apply to Pages, Page, Modal, and Collapsible"
                )
                dg.Button("Clipped shadow", class_="shadow-clip-item")
                dg.Label("Mouse wheel, track click, and thumb drag share the scroll path")
                dg.Label("This line forces vertical overflow")
            with dg.Panel("Grid slice", class_="grid-demo"):
                with dg.Panel(class_="grid-tile grid-sidebar"):
                    dg.Label("Sidebar", class_="kicker")
                    dg.Label("span 2", class_="caption")
                with dg.Panel(class_="grid-tile grid-main"):
                    dg.Label("Main track", class_="caption")
                with dg.Panel(class_="grid-tile grid-left"):
                    dg.Label("1fr", class_="caption")
                    dg.Panel(class_="empty-marker")
                    dg.Badge("pin", level="warning", class_="pin-demo")
                with dg.Panel(class_="grid-tile grid-right"):
                    dg.Label("1fr", class_="caption")
                    with dg.HLayout(style={"gap": 0, "height": 22}):
                        dg.Badge("base", level="info", class_="stack-demo")
                        dg.Badge("offset", level="success", class_="offset-demo")

    dg.Badge("fixed", level="info", class_="fixed-demo")

css_modal = dg.Modal(
    "Modal scrim part",
    open=False,
    width=420,
    height=230,
    class_="css-scrim-demo",
    parent=win,
)
with css_modal:
    dg.Label("Modal::scrim", class_="kicker")
    dg.Label(
        "The full-window scrim behind this modal is styled through a CSS part.",
        class_="caption",
    )
    dg.Button("Close", class_="motion", on_click=css_modal.close)


print(app.run(win))
