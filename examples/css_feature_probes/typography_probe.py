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
        box-shadow: 0 12px 30px rgba(0, 0, 0, 0.24);
        padding: 12px;
        gap: 8px;
    }

    Panel.case {
        width: 360px;
        min-height: 168px;
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

    Label.upper {
        text-transform: uppercase;
        letter-spacing: 0.12em;
        color: #ffd36a;
        font-weight: 800;
    }

    Label.lower {
        text-transform: lowercase;
        color: #74ddb0;
        font-weight: 800;
    }

    Label.capitalize {
        text-transform: capitalize;
        color: #c7d2fe;
        font-weight: 800;
    }

    Label.tight {
        line-height: 1.02;
        color: rgba(245, 248, 255, 0.92);
    }

    Label.loose {
        line-height: 1.55;
        color: rgba(245, 248, 255, 0.92);
    }

    Label.italic {
        font-style: italic;
        color: rgba(245, 248, 255, 0.80);
    }

    Label.tabular,
    Badge.tabular {
        font-variant-numeric: tabular-nums;
        font-family: "Segoe UI";
    }

    Label.metric {
        font-size: 18px;
        color: #74ddb0;
        font-weight: 800;
    }

    Label.ellipsis {
        width: 230px;
        text-overflow: ellipsis;
        color: rgba(245, 248, 255, 0.92);
    }

    Label.clip {
        width: 230px;
        text-overflow: clip;
        color: rgba(245, 248, 255, 0.72);
    }

    Button.long {
        width: 240px;
        text-overflow: ellipsis;
    }

    Panel.wrap-case {
        width: calc(100% - 24px);
        min-height: 160px;
    }

    Label.wrap-demo {
        width: 420px;
        line-height: 1.18;
        color: rgba(245, 248, 255, 0.88);
    }

    Label.no-wrap-demo {
        width: 420px;
        text-overflow: ellipsis;
        color: rgba(255, 211, 106, 0.92);
    }

    Badge {
        background: rgba(255, 255, 255, 0.10);
        border: 1px solid rgba(255, 255, 255, 0.18);
        border-radius: 999px;
        color: white;
        padding: 5px 10px;
    }
    """
)


win = dg.Window("CSS Typography Probe", width=850, height=700)

long_text = (
    "Quarterly operating forecast changed after late-arriving usage data "
    "and should wrap cleanly inside this fixed-width label."
)

with dg.VLayout(class_="scroll-root"):
    dg.Label("Typography", class_="title")
    dg.Label(
        "This probe isolates display-only text styling: casing, tracking, line height, "
        "italic style, tabular numbers, ellipsis, and wrapping behavior.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Transform and spacing", class_="case"):
            dg.Label("Text transform", class_="case-title")
            dg.Label("report status", class_="upper")
            dg.Label("WARNING MIXED CASE", class_="lower")
            dg.Label("monthly active accounts", class_="capitalize")
            dg.Label("PASS: casing changes without changing source strings.", class_="pass")

        with dg.Panel("Line height and italic", class_="case"):
            dg.Label("Line-height comparison", class_="case-title")
            dg.Label("Tight line one\nTight line two", class_="tight")
            dg.Label("Loose line one\nLoose line two", class_="loose")
            dg.Label("Italic caption style should render slanted.", class_="italic")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Tabular numbers", class_="case"):
            dg.Label("Aligned dashboard values", class_="case-title")
            dg.Label("001.25   124.90   987.10", class_="metric tabular")
            dg.Label("111.25   444.90   222.10", class_="metric tabular")
            with dg.HLayout(style={"gap": 8}):
                dg.Badge("09:41", class_="tabular")
                dg.Badge("12.80", class_="tabular")
                dg.Badge("98.05", class_="tabular")
            dg.Label("PASS: numeric columns should feel evenly spaced.", class_="pass")

        with dg.Panel("Overflow", class_="case"):
            dg.Label("Ellipsis vs clip", class_="case-title")
            dg.Label("This label should end with an ellipsis marker", class_="ellipsis", wrap=False)
            dg.Label("This label clips abruptly without an ellipsis marker", class_="clip", wrap=False)
            dg.Button("Long button label should ellipsize here", class_="long")
            dg.Label("PASS: ellipsis is visible where enabled.", class_="pass")

    with dg.Panel("Wrapping behavior", class_="wrap-case"):
        dg.Label("Wrapped and non-wrapped labels", class_="case-title")
        dg.Label(long_text, class_="wrap-demo")
        dg.Label(long_text, class_="no-wrap-demo", wrap=False)
        dg.Label(
            "PASS: the first paragraph wraps into multiple lines; the second remains single-line with ellipsis.",
            class_="pass",
        )


if __name__ == "__main__":
    print(app.run(win))
