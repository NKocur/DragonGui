from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#76b7ff", radius=10))
app.stylesheet(
    """
    :root {
        --surface: #172230;
        --text: rgba(248, 251, 255, 0.96);
        --muted: rgba(225, 234, 245, 0.78);
        --line: rgba(255, 255, 255, 0.16);
    }

    Window {
        background: #0d141f;
        color: var(--text);
        padding: 20px;
        gap: 14px;
        font-size: 14px;
    }

    Label.title {
        color: #76b7ff;
        font-size: 21px;
        font-weight: 800;
    }

    Label.caption {
        color: var(--muted);
    }

    Panel.pattern-card {
        width: 340px;
        height: 154px;
        padding: 16px;
        gap: 8px;
        border: 1px solid var(--line);
        border-radius: 16px;
        box-shadow: 0 14px 30px rgba(0, 0, 0, 0.28);
    }

    Panel.checker {
        background-color: #172230;
        background-image:
            dg-pattern(checker, rgba(118, 183, 255, 0.18), transparent, 18px);
    }

    Panel.pinstripe {
        background-color: #1b2634;
        background-image:
            dg-pattern(pinstripe, rgba(255, 255, 255, 0.13), transparent, 12px);
    }

    Panel.dot {
        background-color: #132821;
        background-image:
            dg-pattern(stipple, rgba(116, 221, 176, 0.52), transparent, 14px);
    }

    Panel.hatch {
        background-color: #2b2018;
        background-image:
            dg-pattern(diagonal-hatch, rgba(255, 211, 106, 0.30), transparent, 16px);
        transform: rotate(-1deg);
    }

    Label.card-title {
        color: white;
        font-size: 17px;
        font-weight: 800;
    }

    Label.card-copy {
        color: rgba(245, 249, 255, 0.82);
        line-height: 1.12;
    }
    """
)


win = dg.Window("CSS Background Pattern Probe", width=760, height=470)

with dg.VLayout(style={"gap": 14}):
    dg.Label("Procedural background patterns", class_="title")
    dg.Label(
        "One bounded GPU paint per card; patterns retain rounded clipping, opacity, "
        "DPI-aware tile sizing, layers, and transforms.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 14}):
        with dg.Panel(class_="pattern-card checker"):
            dg.Label("checker", class_="card-title")
            dg.Label(
                "18 px logical tile layered over a solid background color.",
                class_="card-copy",
            )

        with dg.Panel(class_="pattern-card pinstripe"):
            dg.Label("pinstripe", class_="card-title")
            dg.Label(
                "A restrained vertical stripe without repeated CPU geometry.",
                class_="card-copy",
            )

    with dg.HLayout(style={"gap": 14}):
        with dg.Panel(class_="pattern-card dot"):
            dg.Label("dot / stipple", class_="card-title")
            dg.Label(
                "The stipple alias resolves to the same bounded dot paint.",
                class_="card-copy",
            )

        with dg.Panel(class_="pattern-card hatch"):
            dg.Label("diagonal-hatch", class_="card-title")
            dg.Label(
                "Rounded clipping remains correct after a small CSS rotation.",
                class_="card-copy",
            )


if __name__ == "__main__":
    print(app.run(win))
