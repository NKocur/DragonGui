from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8))
app.stylesheet(
    """
    VLayout.demo-root {
        padding: 18px;
        gap: 14px;
        background: #101722;
    }

    Panel.scheme-card {
        padding: 18px;
        gap: 10px;
        background: rgba(18, 25, 39, 0.96);
        border-width: 1px;
        border-color: rgba(255, 255, 255, 0.14);
        border-radius: 12px;
        box-shadow: 0 16px 44px rgba(0, 0, 0, 0.30);
    }

    Label.title {
        font-size: 18px;
        font-weight: 700;
        color: #f4f7fb;
    }

    Label.note {
        color: rgba(232, 238, 247, 0.72);
        line-height: 1.25;
    }

    Label.state-dark,
    Label.state-light {
        display: none;
        padding: 10px;
        border-radius: 8px;
        font-weight: 700;
    }

    @media (prefers-color-scheme: dark) {
        Label.state-dark {
            display: block;
            background: rgba(90, 169, 255, 0.16);
            border-color: rgba(90, 169, 255, 0.40);
            border-width: 1px;
            color: #d8ebff;
        }
    }

    @media (prefers-color-scheme: light) {
        VLayout.demo-root {
            background: #edf2f8;
        }

        Panel.scheme-card {
            background: rgba(255, 255, 255, 0.96);
            border-color: rgba(40, 74, 115, 0.16);
            box-shadow: 0 14px 34px rgba(40, 74, 115, 0.16);
        }

        Label.title {
            color: #172131;
        }

        Label.note {
            color: rgba(23, 33, 49, 0.68);
        }

        Label.state-light {
            display: block;
            background: rgba(23, 120, 93, 0.12);
            border-color: rgba(23, 120, 93, 0.30);
            border-width: 1px;
            color: #165f4c;
        }
    }
    """
)

win = dg.Window("DragonGUI prefers-color-scheme Demo", width=680, height=360)

with dg.VLayout(class_="demo-root"):
    with dg.Panel("Platform Color Scheme", class_="scheme-card"):
        dg.Label("prefers-color-scheme media query", class_="title")
        dg.Label(
            "DragonGUI now uses the OS window theme when winit reports one. "
            "Change your OS app theme while this window is open to test live updates.",
            class_="note",
        )
        dg.Label("Dark media rule is active", class_="state-dark")
        dg.Label("Light media rule is active", class_="state-light")

print(app.run(win))
