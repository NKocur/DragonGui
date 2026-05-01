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
        min-height: 560px;
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
        box-shadow: 0 6px 15px rgba(0, 0, 0, 0.18);
        padding: 12px;
        gap: 8px;
    }

    Panel.case {
        width: 360px;
        min-height: 228px;
    }

    Panel.shadow-case {
        gap: 14px;
    }

    Panel.sample {
        width: 148px;
        height: 76px;
        padding: 10px;
        border-radius: 14px;
        background: rgba(255, 255, 255, 0.06);
        box-shadow: none;
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

    Label.sample-label {
        color: rgba(245, 248, 255, 0.92);
        font-weight: 800;
        font-size: 12px;
    }

    Label.pass {
        color: #74ddb0;
        font-weight: 800;
    }

    Panel.solid-border {
        border-width: 2px;
        border-style: solid;
        border-color: rgba(90, 169, 255, 0.84);
    }

    Panel.no-border {
        border-width: 3px;
        border-style: none;
        border-color: rgba(255, 101, 132, 0.90);
    }

    Panel.hidden-border {
        border-width: 3px;
        border-style: hidden;
        border-color: rgba(255, 211, 106, 0.90);
    }

    Panel.rounded-border {
        border-width: 3px;
        border-style: solid;
        border-color: rgba(116, 221, 176, 0.86);
        border-radius: 26px;
    }

    Panel.outline-solid {
        outline: 3px solid rgba(90, 169, 255, 0.78);
        outline-offset: 4px;
        border-color: rgba(255, 255, 255, 0.20);
    }

    Panel.outline-tight {
        outline-color: rgba(255, 211, 106, 0.86);
        outline-width: 2px;
        outline-style: solid;
        outline-offset: 0px;
        border-color: rgba(255, 255, 255, 0.20);
    }

    Panel.outline-none {
        outline: 4px solid rgba(255, 101, 132, 0.90);
        outline-style: none;
        border-color: rgba(255, 101, 132, 0.45);
    }

    Panel.outset-shadow {
        box-shadow: 0 9px 17px rgba(0, 0, 0, 0.34);
    }

    Panel.multi-shadow {
        box-shadow:
            0 1px 4px rgba(0, 0, 0, 0.22),
            0 10px 24px 2px rgba(18, 91, 172, 0.28);
    }

    Panel.inset-shadow {
        box-shadow:
            inset 0 1px 0 rgba(255, 255, 255, 0.22),
            inset 0 -9px 17px rgba(0, 0, 0, 0.28);
    }

    Panel.shadow-scroll {
        width: calc(100% - 24px);
        height: 210px;
        overflow-y: auto;
        padding: 18px;
        padding-bottom: 70px;
    }

    Panel.shadow-scroll::scrollbar-track {
        width: 8px;
        padding: 8px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    Panel.shadow-scroll::scrollbar-thumb {
        width: 6px;
        background: rgba(255, 211, 106, 0.76);
        border-radius: 999px;
    }

    Panel.scroll-card {
        width: 640px;
        height: 78px;
        flex-shrink: 0;
        border-radius: 18px;
        border-color: rgba(90, 169, 255, 0.44);
        box-shadow:
            0 9px 18px rgba(0, 0, 0, 0.34),
            22px 0 21px rgba(90, 169, 255, 0.22);
        background: rgba(35, 49, 73, 0.94);
    }
    """
)


win = dg.Window("CSS Border Outline Shadow Probe", width=850, height=700)


def sample(class_name: str, label: str) -> None:
    with dg.Panel(class_=f"sample {class_name}"):
        dg.Label(label, class_="sample-label", wrap=False)


with dg.VLayout(class_="scroll-root"):
    dg.Label("Border, outline, and shadow paint", class_="title")
    dg.Label(
        "This probe isolates uniform border styles, paint-only outlines, inset/outset shadows, "
        "multi-layer shadows, and shadow clipping inside scroll containers.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Borders", class_="case"):
            dg.Label("border-style", class_="case-title")
            with dg.HLayout(style={"gap": 10}):
                sample("solid-border", "solid")
                sample("rounded-border", "rounded")
            with dg.HLayout(style={"gap": 10}):
                sample("no-border", "none")
                sample("hidden-border", "hidden")
            dg.Label("PASS: none/hidden remove the border ring.", class_="pass")

        with dg.Panel("Outlines", class_="case"):
            dg.Label("outline and outline-offset", class_="case-title")
            with dg.HLayout(style={"gap": 10}):
                sample("outline-solid", "offset")
                sample("outline-tight", "tight")
            with dg.HLayout(style={"gap": 10}):
                sample("outline-none", "none")
            dg.Label("PASS: outlines follow rounded corners cleanly.", class_="pass")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Shadows", class_="case shadow-case"):
            dg.Label("outset, inset, and layers", class_="case-title")
            with dg.HLayout(style={"gap": 22}):
                sample("outset-shadow", "outset")
                sample("multi-shadow", "multi")
            with dg.HLayout(style={"gap": 22}):
                sample("inset-shadow", "inset")
            dg.Label("PASS: shadows do not cover text.", class_="pass")

        with dg.Panel("Rounded shadow edges", class_="case shadow-case"):
            dg.Label("rounded surfaces", class_="case-title")
            sample("rounded-border multi-shadow", "rounded + shadow")
            sample("outline-solid inset-shadow", "outline + inset")
            dg.Label("PASS: rings and shadows are continuous at corners.", class_="pass")

    with dg.Panel("Scroll clipping", class_="shadow-scroll"):
        dg.Label("Scroll this panel", class_="case-title")
        dg.Label(
            "Outset shadows should clip to the scroll viewport without shrinking to the visible part of the card.",
            class_="caption",
        )
        for index in range(1, 6):
            with dg.Panel(class_="scroll-card"):
                dg.Label(f"shadow card {index}", class_="case-title")
                dg.Label("Rounded surface with large outset shadow.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
