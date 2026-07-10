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
        width: 100%;
        height: 100%;
        min-height: 520px;
        overflow-y: auto;
        padding-right: 18px;
        padding-bottom: 84px;
        gap: 12px;
    }

    VLayout.scroll-root::scrollbar-track {
        width: 8px;
        padding: 8px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.scroll-root::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.72);
        border-radius: 999px;
    }

    Panel {
        background: rgba(18, 25, 39, 0.92);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 16px;
        box-shadow: 0 12px 30px rgba(0, 0, 0, 0.24);
        padding: 12px;
        gap: 8px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 800;
    }

    Label.caption {
        color: rgba(245, 248, 255, 0.74);
        line-height: 1.12;
    }

    Label.case-title {
        color: rgba(245, 248, 255, 0.96);
        font-weight: 800;
    }

    Label.pass {
        color: #74ddb0;
        font-weight: 800;
    }

    Badge {
        background: rgba(255, 255, 255, 0.10);
        border: 1px solid rgba(255, 255, 255, 0.18);
        border-radius: 999px;
        color: white;
        padding: 5px 10px;
    }

    Panel.percent-case {
        width: calc(100% - 24px);
        min-width: 300px;
        min-height: 150px;
        padding: calc(1% + 10px);
        gap: calc(1% + 4px);
        background:
            radial-gradient(circle at 20% 18%, rgba(116, 221, 176, 0.18), transparent 62%),
            linear-gradient(145deg, rgba(90, 169, 255, 0.13), rgba(255, 255, 255, 0.045));
    }

    Panel.percent-child {
        width: 72%;
        min-width: 220px;
        max-width: 680px;
        min-height: 44px;
        padding: 8px;
        background: rgba(116, 221, 176, 0.10);
        border-color: rgba(116, 221, 176, 0.28);
        box-shadow: none;
    }

    HLayout.percent-row {
        gap: calc(1% + 6px);
    }

    Button.half {
        width: 50%;
        min-width: 112px;
    }

    Button.calc-width {
        width: calc(50% - 20px);
        min-width: 128px;
    }

    Panel.auto-case {
        width: auto;
        min-width: 440px;
        max-width: 620px;
        min-height: auto;
        margin-left: auto;
        margin-right: auto;
        gap: 12px;
    }

    Panel.named-grid {
        display: grid;
        width: calc(100% - 24px);
        min-height: 250px;
        grid-template-columns: fit-content(150px) repeat(2, minmax(120px, 1fr));
        grid-template-rows: fit-content(46px) minmax(72px, auto) 48px;
        grid-template-areas:
            "side main main"
            "side stats detail"
            "side foot foot";
        column-gap: calc(1% + 8px);
        row-gap: calc(1% + 8px);
        padding: calc(1% + 10px);
    }

    Panel.grid-item {
        min-height: 42px;
        padding: 10px;
        border-radius: 10px;
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.10);
    }

    Panel.area-side {
        grid-area: side;
        background: rgba(90, 169, 255, 0.14);
        border-color: rgba(90, 169, 255, 0.30);
    }

    Panel.area-main {
        grid-area: main;
        background: rgba(116, 221, 176, 0.12);
        border-color: rgba(116, 221, 176, 0.30);
    }

    Panel.area-stats {
        grid-area: stats;
        background: rgba(255, 211, 106, 0.12);
        border-color: rgba(255, 211, 106, 0.30);
    }

    Panel.area-detail {
        grid-area: detail;
        background: rgba(255, 142, 163, 0.12);
        border-color: rgba(255, 142, 163, 0.30);
    }

    Panel.area-foot {
        grid-area: foot;
        background: rgba(199, 210, 254, 0.12);
        border-color: rgba(199, 210, 254, 0.30);
    }

    Panel.auto-fit-grid {
        display: grid;
        width: calc(100% - 24px);
        min-height: 220px;
        grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
        grid-template-rows: repeat(auto-fill, 44px);
        grid-auto-flow: row dense;
        gap: 10px;
        padding: 12px;
    }

    Panel.tile {
        min-height: 40px;
        padding: 8px;
        border-radius: 10px;
        background: rgba(255, 255, 255, 0.055);
        border-color: rgba(255, 255, 255, 0.12);
        box-shadow: none;
    }

    Panel.tile:nth-child(odd) {
        background: rgba(90, 169, 255, 0.12);
        border-color: rgba(90, 169, 255, 0.28);
    }

    Panel.tile-wide {
        grid-column: 1 / span 2;
        background: rgba(116, 221, 176, 0.12);
        border-color: rgba(116, 221, 176, 0.28);
    }

    Panel.fit-row {
        display: grid;
        width: calc(100% - 24px);
        grid-template-columns: fit-content(210px) minmax(140px, 1fr) minmax(22%, auto);
        gap: calc(1% + 8px);
        padding: 12px;
    }

    Panel.fit-row > Panel {
        min-height: 64px;
        padding: 10px;
        border-radius: 10px;
        box-shadow: none;
    }

    @media (max-width: 520px) {
        Window {
            padding: 10px;
        }

        VLayout.scroll-root {
            padding-right: 8px;
            padding-bottom: 28px;
            gap: 10px;
        }

        Panel.percent-case,
        Panel.named-grid,
        Panel.auto-fit-grid,
        Panel.fit-row {
            width: auto;
            min-height: auto;
            padding: 10px;
            gap: 8px;
        }

        Panel.auto-case {
            width: auto;
            min-width: 0;
            margin-left: 0;
            margin-right: 0;
        }

        Panel.auto-case Badge {
            width: 96px;
            min-width: 0;
        }

        Panel.percent-child {
            min-width: 0;
            width: 100%;
        }

        Button.half,
        Button.calc-width {
            min-width: 92px;
        }

        Panel.named-grid {
            grid-template-columns: 1fr;
            grid-template-rows: repeat(5, auto);
            grid-template-areas:
                "side"
                "main"
                "stats"
                "detail"
                "foot";
        }

        Panel.fit-row {
            grid-template-columns: 1fr;
        }

        Panel.auto-fit-grid {
            grid-template-columns: 1fr;
            grid-template-rows: repeat(auto-fill, 40px);
        }

        Panel.tile-wide {
            grid-column: auto;
        }

        Panel.grid-item,
        Panel.fit-row > Panel {
            min-height: auto;
            padding: 8px;
        }
    }
    """
)


win = dg.Window("CSS Responsive Layout Probe", width=860, height=700)

with dg.VLayout(class_="scroll-root"):
    dg.Label("Responsive layout", class_="title")
    dg.Label(
        "Resize the window. Percent, calc(), auto sizing, grid tracks, named areas, "
        "and dense auto-placement should adapt without clipped labels or overlapping panels.",
        class_="caption",
    )

    with dg.Panel("Percent and calc sizing", class_="percent-case"):
        dg.Label("Full-width parent with percent children", class_="case-title")
        dg.Label("The parent panel should grow with the window. The green child is 72% wide.")
        with dg.Panel(class_="percent-child"):
            dg.Label("72% child, clamped by min/max", class_="case-title")
        with dg.HLayout(class_="percent-row"):
            dg.Button("50% button", class_="half")
            dg.Button("calc(50% - 20px)", class_="calc-width")
        dg.Label("PASS: buttons resize with the panel and keep readable text.", class_="pass")

    with dg.Panel("Auto sizing", class_="auto-case"):
        dg.Label("Centered auto-width panel", class_="case-title")
        dg.Label(
            "This panel uses width:auto, min/max width, and auto side margins.",
            class_="caption",
        )
        with dg.HLayout(style={"gap": 8}):
            dg.Badge("auto")
            dg.Badge("min/max")
            dg.Badge("margin auto")
        dg.Label("PASS: the panel remains centered while respecting its max width.", class_="pass")

    with dg.Panel("Named grid areas", class_="named-grid"):
        with dg.Panel(class_="grid-item area-side"):
            dg.Label("side", class_="case-title")
            dg.Label("fit-content column")
        with dg.Panel(class_="grid-item area-main"):
            dg.Label("main", class_="case-title")
            dg.Label("spans two flexible columns")
        with dg.Panel(class_="grid-item area-stats"):
            dg.Label("stats", class_="case-title")
            dg.Label("minmax track")
        with dg.Panel(class_="grid-item area-detail"):
            dg.Label("detail", class_="case-title")
            dg.Label("minmax track")
        with dg.Panel(class_="grid-item area-foot"):
            dg.Label("footer", class_="case-title")
            dg.Label("named area across both columns")

    with dg.Panel("Auto-fit and dense placement", class_="auto-fit-grid"):
        for index in range(1, 10):
            tile_class = "tile tile-wide" if index in {2, 7} else "tile"
            with dg.Panel(class_=tile_class):
                dg.Label(f"tile {index}", class_="case-title")

    with dg.Panel("fit-content and minmax tracks", class_="fit-row"):
        with dg.Panel():
            dg.Label("fit-content(210px)", class_="case-title")
            dg.Label("This column hugs content until the cap.")
        with dg.Panel():
            dg.Label("minmax(140px, 1fr)", class_="case-title")
            dg.Label("This column absorbs extra space.")
        with dg.Panel():
            dg.Label("minmax(22%, auto)", class_="case-title")
            dg.Label("Percent minimum plus auto max.")


if __name__ == "__main__":
    print(app.run(win))
