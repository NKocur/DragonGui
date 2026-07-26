from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App()
app.stylesheet(
    """
    Window {
        background: #111923;
        color: #eff8ff;
        padding: 18px;
        gap: 14px;
    }

    .origin-card {
        flex-grow: 0;
        background: rgba(255, 255, 255, 0.055);
        border: 1px solid rgba(255, 255, 255, 0.14);
        padding: 12px;
        gap: 8px;
    }

    .stylesheet-width {
        width: 360px;
        border: 2px solid #4dd3b1;
    }

    .inline-wins {
        width: 360px;
        border: 2px solid #5b9bff;
    }
    """
)


with dg.Window("Cascade Origin Baseline", width=760, height=520) as win:
    with dg.VLayout(style={"width": "100%", "height": "100%", "gap": 12}):
        dg.Label("Widget defaults versus authored styles", style={"font_size": 22, "font_weight": 850})
        dg.Label(
            "Green should resolve to the application stylesheet width. Blue should "
            "resolve to the explicit 220px inline width."
        )
        with dg.Panel("Application stylesheet", class_="origin-card"):
            dg.SearchBox(
                placeholder="Expected stylesheet width: 360px",
                class_="stylesheet-width",
            )
        with dg.Panel("Author inline style", class_="origin-card"):
            dg.SearchBox(
                placeholder="Expected inline width: 220px",
                class_="inline-wins",
                style={"width": 220},
            )
        with dg.Panel("Cascade contract", class_="origin-card"):
            dg.Label(
                "Target precedence: native fallback < framework CSS < widget default "
                "< theme CSS < application CSS < author inline style."
            )


if __name__ == "__main__":
    print(app.run(win))
