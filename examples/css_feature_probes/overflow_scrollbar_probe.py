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

    VLayout.root {
        width: 100%;
        height: 100%;
        min-height: 0;
        overflow-y: auto;
        overflow-x: hidden;
        padding-right: 12px;
        padding-bottom: 18px;
    }

    Panel {
        background: rgba(18, 25, 39, 0.94);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 14px;
        box-shadow: 0 14px 34px rgba(0, 0, 0, 0.28);
        padding: 14px;
        gap: 8px;
    }

    Label.title {
        color: #5aa9ff;
        font-size: 20px;
        font-weight: 800;
    }

    Label.section-title {
        font-weight: 800;
        color: rgba(245, 248, 255, 0.95);
    }

    Label.caption {
        color: rgba(245, 248, 255, 0.72);
        line-height: 1.12;
    }

    Panel.case {
        width: 330px;
        height: 205px;
        padding: 14px;
    }

    Panel.hidden-case {
        height: 220px;
    }

    Panel.vertical-scroll {
        overflow-y: auto;
        overflow-x: hidden;
        height: 220px;
        padding-right: 26px;
        padding-bottom: 20px;
    }

    Panel.hidden-clip {
        overflow: hidden;
    }

    Panel.both-scroll {
        width: 700px;
        height: 205px;
        overflow: auto;
        padding: 14px;
    }

    HLayout.horizontal-scroll {
        width: 700px;
        height: 112px;
        overflow-x: auto;
        overflow-y: hidden;
        gap: 10px;
        padding: 12px;
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 12px;
    }

    HLayout.wide-row {
        width: 920px;
        gap: 10px;
        flex-shrink: 0;
    }

    Panel.tile {
        width: 145px;
        height: 64px;
        flex-shrink: 0;
        padding: 10px;
        border-radius: 10px;
        background:
            radial-gradient(circle at 20% 18%, rgba(116, 221, 176, 0.18), transparent 62%),
            linear-gradient(145deg, rgba(90, 169, 255, 0.14), rgba(255, 255, 255, 0.05));
        border-color: rgba(90, 169, 255, 0.30);
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.13);
    }

    Panel.clip-card {
        width: 286px;
        height: 84px;
        padding: 12px;
        background: rgba(255, 211, 106, 0.13);
        border-color: rgba(255, 211, 106, 0.50);
        border-radius: 12px;
        box-shadow:
            0 10px 24px rgba(0, 0, 0, 0.20),
            42px 10px 30px rgba(255, 211, 106, 0.28);
        overflow: hidden;
    }

    Label.spill-text {
        width: 520px;
        color: rgba(255, 237, 181, 0.92);
        font-weight: 800;
        text-overflow: clip;
    }

    Panel.cell {
        width: 132px;
        height: 38px;
        flex-shrink: 0;
        padding: 8px;
        overflow: hidden;
        border-radius: 8px;
        background: rgba(255, 255, 255, 0.06);
        border-color: rgba(255, 255, 255, 0.10);
        box-shadow: none;
    }

    Panel.cell:nth-child(odd) {
        background: rgba(90, 169, 255, 0.10);
        border-color: rgba(90, 169, 255, 0.24);
    }

    Panel::scrollbar-track,
    HLayout::scrollbar-track {
        width: 8px;
        padding: 8px;
        background: rgba(255, 255, 255, 0.09);
        border-radius: 999px;
    }

    Panel::scrollbar-thumb,
    HLayout::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.72);
        border-radius: 999px;
    }

    HLayout.horizontal-scroll::scrollbar-thumb {
        background: rgba(116, 221, 176, 0.72);
    }

    Panel.both-scroll::scrollbar-thumb {
        background: rgba(255, 211, 106, 0.76);
    }

    Button {
        height: 30px;
    }
    """
)


win = dg.Window("CSS Overflow And Scrollbar Probe", width=780, height=700)

with dg.VLayout(class_="root", style={"gap": 12}):
    dg.Label("Overflow and scrollbar parts", class_="title")
    dg.Label(
        "This probe isolates overflow clipping, vertical scroll, horizontal scroll, "
        "both-axis scroll, and ::scrollbar-track / ::scrollbar-thumb styling.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="case vertical-scroll"):
            dg.Label("Vertical auto", class_="section-title")
            dg.Label("Wheel inside this panel.", class_="caption")
            for index in range(1, 10):
                dg.Button(f"Scrollable action {index}")

        with dg.Panel("Hidden clip", class_="case hidden-case hidden-clip"):
            dg.Label("The wide text and glow should clip with no scrollbar.", class_="caption")
            with dg.Panel(class_="clip-card"):
                dg.Label("Clipped wide child", class_="section-title")
                dg.Label("THIS LONG STRIP SHOULD DISAPPEAR AT THE CARD EDGE", class_="spill-text")
            dg.Label("This trailing label should remain fully visible below the card.", class_="caption")

    dg.Label("Horizontal overflow", class_="section-title")
    with dg.HLayout(class_="horizontal-scroll"):
        for index in range(1, 8):
            with dg.Panel(class_="tile"):
                dg.Label(f"Tile {index}", class_="section-title")
                dg.Label("Drag the bottom scrollbar.", class_="caption")

    with dg.Panel("Both axes", class_="both-scroll"):
        dg.Label("This panel should show vertical and horizontal scrollbar indicators.", class_="caption")
        for row in range(1, 8):
            with dg.HLayout(class_="wide-row"):
                for col in range(1, 7):
                    with dg.Panel(class_="cell"):
                        dg.Label(f"R{row} C{col}")


if __name__ == "__main__":
    print(app.run(win))
