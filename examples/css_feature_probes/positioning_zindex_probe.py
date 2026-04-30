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
        padding-bottom: 72px;
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
        min-height: 168px;
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

    Badge,
    Button {
        border-radius: 999px;
        color: white;
    }

    Badge {
        background: rgba(255, 255, 255, 0.10);
        border: 1px solid rgba(255, 255, 255, 0.18);
        padding: 5px 12px;
    }

    Button {
        height: 34px;
        background: rgba(255, 255, 255, 0.06);
        border: 1px solid rgba(255, 255, 255, 0.16);
        font-weight: 800;
    }

    Panel.relative-demo {
        min-height: 176px;
    }

    Badge.relative-offset {
        position: relative;
        left: 24px;
        top: -6px;
        z-index: 2;
        background: rgba(116, 221, 176, 0.24);
        border-color: rgba(116, 221, 176, 0.56);
    }

    Badge.relative-base {
        background: rgba(255, 255, 255, 0.10);
        border-color: rgba(255, 255, 255, 0.18);
    }

    HLayout.stack-lane {
        height: 86px;
        overflow: visible;
    }

    Badge.stack-low,
    Badge.stack-high,
    Badge.stack-middle {
        position: relative;
        width: 112px;
        height: 46px;
        border-radius: 14px;
    }

    Badge.stack-low {
        left: 0;
        top: 0;
        z-index: 1;
        background: rgba(90, 169, 255, 0.30);
        border-color: rgba(90, 169, 255, 0.70);
    }

    Badge.stack-middle {
        left: -42px;
        top: 18px;
        z-index: 2;
        background: rgba(255, 211, 106, 0.30);
        border-color: rgba(255, 211, 106, 0.70);
    }

    Badge.stack-high {
        left: -84px;
        top: 8px;
        z-index: 4;
        background: rgba(255, 101, 132, 0.34);
        border-color: rgba(255, 101, 132, 0.74);
    }

    Panel.positioned-card {
        position: relative;
        min-height: 150px;
        overflow: visible;
    }

    Badge.corner-pin {
        position: absolute;
        top: 10px;
        right: 10px;
        z-index: 3;
        background: rgba(255, 211, 106, 0.26);
        border-color: rgba(255, 211, 106, 0.62);
    }

    Button.absolute-action {
        position: absolute;
        right: 12px;
        bottom: 12px;
        z-index: 2;
        width: 132px;
        background: rgba(90, 169, 255, 0.20);
        border-color: rgba(90, 169, 255, 0.58);
    }

    Panel.clip-host {
        height: 150px;
        overflow: hidden;
        position: relative;
    }

    Badge.clipped-pin {
        position: absolute;
        right: -18px;
        bottom: 12px;
        z-index: 2;
        background: rgba(255, 101, 132, 0.30);
        border-color: rgba(255, 101, 132, 0.70);
    }

    Badge.fixed-dock {
        position: fixed;
        right: 18px;
        bottom: 18px;
        z-index: 8;
        background: rgba(90, 169, 255, 0.30);
        border-color: rgba(90, 169, 255, 0.70);
        box-shadow: 0 12px 26px rgba(0, 0, 0, 0.30);
    }
    """
)


win = dg.Window("CSS Positioning And Z-Index Probe", width=820, height=660)

with dg.VLayout(class_="scroll-root"):
    dg.Label("Positioning and z-index", class_="title")
    dg.Label(
        "Relative offsets should paint shifted without changing layout. Absolute "
        "children should pin inside their parent. The fixed badge should stay "
        "pinned to the viewport while this page scrolls.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Relative position", class_="case relative-demo"):
            dg.Label("position: relative keeps layout space", class_="case-title")
            dg.Badge("normal badge", class_="relative-base")
            dg.Badge("offset badge", class_="relative-offset")
            dg.Button("button after offset")
            dg.Label("PASS: second badge paints right/up; button spacing remains normal.", class_="caption")

        with dg.Panel("Sibling z-index", class_="case"):
            dg.Label("Higher z-index should paint on top", class_="case-title")
            with dg.HLayout(class_="stack-lane"):
                dg.Badge("z 1", class_="stack-low")
                dg.Badge("z 2", class_="stack-middle")
                dg.Badge("z 4", class_="stack-high")
            dg.Label("PASS: red z 4 badge should be visually above the others.", class_="pass")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Absolute child", class_="case positioned-card"):
            dg.Label("position: absolute inside positioned parent", class_="case-title")
            dg.Label("Corner badge and action button are removed from normal flow.")
            dg.Badge("PIN", class_="corner-pin")
            dg.Button("Pinned action", class_="absolute-action")
            dg.Label("PASS: badge top-right; button bottom-right.", class_="pass")

        with dg.Panel("Clipping", class_="case clip-host"):
            dg.Label("Absolute child clipped by overflow", class_="case-title")
            dg.Label("The pink badge intentionally extends past the right edge.")
            dg.Badge("clipped", class_="clipped-pin")
            dg.Label("PASS: overflow hidden clips the badge at the panel edge.", class_="caption")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Scroll reference", class_="case"):
            dg.Label("Scroll down and back up", class_="case-title")
            dg.Label("The fixed viewport badge should not move with this content.")
            dg.Button("normal flow button")
            dg.Label("PASS: fixed dock stays bottom-right of the window.", class_="pass")

        with dg.Panel("Extra content", class_="case"):
            dg.Label("More scroll content", class_="case-title")
            dg.Button("Row 1")
            dg.Button("Row 2")
            dg.Button("Row 3")
            dg.Label("This card exists to make scroll behavior obvious.", class_="caption")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Bottom content", class_="case"):
            dg.Label("More scroll room", class_="case-title")
            dg.Button("bottom row")
            dg.Label("The fixed dock should still be visible here.", class_="pass")

        with dg.Panel("Static baseline", class_="case"):
            dg.Label("No positioning", class_="case-title")
            dg.Badge("static")
            dg.Button("static button")
            dg.Label("PASS: this card should use normal flow only.", class_="pass")

    dg.Badge("fixed viewport dock", class_="fixed-dock")


if __name__ == "__main__":
    print(app.run(win))
