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
        background:
            radial-gradient(circle at 14% 16%, rgba(255, 211, 106, 0.56) 0%, rgba(255, 211, 106, 0.18) 24%, transparent 54%),
            radial-gradient(circle at 84% 18%, rgba(255, 101, 132, 0.54) 0%, rgba(255, 101, 132, 0.16) 28%, transparent 58%),
            radial-gradient(circle at 20% 82%, rgba(116, 221, 176, 0.52) 0%, rgba(116, 221, 176, 0.14) 26%, transparent 58%),
            radial-gradient(circle at 82% 78%, rgba(90, 169, 255, 0.58) 0%, rgba(90, 169, 255, 0.18) 28%, transparent 60%),
            linear-gradient(135deg, #111827 0%, #25314b 48%, #0d1320 100%);
        background-noise: 0.018;
        color: rgba(245, 248, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.scroll-root {
        height: 620px;
        overflow-y: auto;
        padding-right: 18px;
        padding-bottom: 84px;
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
        background: rgba(18, 25, 39, 0.64);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 16px;
        box-shadow: 0 12px 30px rgba(0, 0, 0, 0.26);
        padding: 12px;
        gap: 8px;
    }

    Panel.case {
        width: 356px;
        min-height: 162px;
        overflow: visible;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 800;
    }

    Label.caption {
        color: rgba(245, 248, 255, 0.74);
        line-height: 1.16;
    }

    Label.case-title {
        color: rgba(245, 248, 255, 0.96);
        font-weight: 800;
    }

    Label.pass {
        color: #74ddb0;
        font-weight: 800;
    }

    Panel.backdrop-base {
        background: rgba(255, 255, 255, 0.045);
        border-color: rgba(255, 255, 255, 0.18);
    }

    Panel.blur {
        backdrop-filter: blur(28px);
    }

    Panel.bright {
        backdrop-filter: brightness(180%);
        background: rgba(255, 255, 255, 0.075);
        border-color: rgba(255, 255, 255, 0.32);
    }

    Panel.saturated {
        backdrop-filter: saturate(3.0);
        background: rgba(90, 169, 255, 0.075);
        border-color: rgba(116, 221, 176, 0.34);
    }

    Panel.combo {
        backdrop-filter: blur(32px) brightness(145%) saturate(2.1);
        background: rgba(255, 255, 255, 0.095);
        border-color: rgba(255, 255, 255, 0.36);
    }

    Panel.none {
        backdrop-filter: none;
    }

    Panel.rounded-clip {
        border-radius: 28px;
        backdrop-filter: blur(30px) brightness(135%) saturate(1.8);
        background: rgba(255, 255, 255, 0.095);
        overflow: hidden;
    }

    Panel.moving-reference {
        background:
            radial-gradient(circle at 20% 20%, rgba(255, 211, 106, 0.42), transparent 52%),
            radial-gradient(circle at 80% 68%, rgba(116, 221, 176, 0.34), transparent 58%),
            linear-gradient(135deg, rgba(90, 169, 255, 0.28), rgba(13, 19, 32, 0.38));
        background-noise: 0.018;
    }

    Badge {
        background: rgba(255, 255, 255, 0.12);
        border: 1px solid rgba(255, 255, 255, 0.24);
        border-radius: 999px;
        color: white;
        padding: 5px 12px;
    }
    """
)


win = dg.Window("CSS Backdrop Filter Probe", width=820, height=660)

with dg.VLayout(class_="scroll-root"):
    dg.Label("Backdrop filter", class_="title")
    dg.Label(
        "DragonGUI currently renders backdrop-filter as a first-slice frosted tint/noise treatment. "
        "It should clip cleanly to rounded panels and stay stable while scrolling.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("No filter baseline", class_="case backdrop-base none"):
            dg.Label("backdrop-filter: none", class_="case-title")
            dg.Label("Baseline translucent panel over the window gradient.")
            dg.Badge("baseline")
            dg.Label("PASS: no frosted treatment beyond the panel background.", class_="caption")

        with dg.Panel("Blur", class_="case backdrop-base blur"):
            dg.Label("blur(12px)", class_="case-title")
            dg.Label("Should show the subtle frosted tint/noise treatment.")
            dg.Badge("blur")
            dg.Label("PASS: effect is stable, not resized while scrolling.", class_="pass")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Brightness", class_="case backdrop-base bright"):
            dg.Label("brightness(118%)", class_="case-title")
            dg.Label("Exaggerated brightness value should be clearly lighter.")
            dg.Badge("bright")
            dg.Label("PASS: brightened tint stays clipped to the panel.", class_="pass")

        with dg.Panel("Saturation", class_="case backdrop-base saturated"):
            dg.Label("saturate(1.55)", class_="case-title")
            dg.Label("Exaggerated saturation should read cooler and more colorful.")
            dg.Badge("saturate")
            dg.Label("PASS: saturation treatment should be subtle and consistent.", class_="caption")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Combined filter", class_="case combo"):
            dg.Label("blur + brightness + saturate", class_="case-title")
            dg.Label("Combined filter list should parse and apply as one frosted surface.")
            dg.Badge("combo")
            dg.Label("PASS: this should be the strongest frosted card.", class_="pass")

        with dg.Panel("Rounded clipping", class_="case rounded-clip"):
            dg.Label("Large radius clip", class_="case-title")
            dg.Label("The treatment must respect the larger corner radius.")
            dg.Badge("rounded")
            dg.Label("PASS: no square tint leaks outside rounded corners.", class_="pass")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Backdrop over local gradient", class_="case moving-reference"):
            dg.Label("Reference gradient panel", class_="case-title")
            dg.Label("This card has its own vivid gradient and no backdrop-filter.")
            dg.Badge("reference")
            dg.Label("Use this as the visual comparison target.", class_="caption")

        with dg.Panel("Scroll stability", class_="case backdrop-base combo"):
            dg.Label("Scroll this card", class_="case-title")
            dg.Label("The filter shape should not shrink or drift as it approaches viewport edges.")
            dg.Badge("scroll check")
            dg.Label("PASS: rounded shape and tint remain stable while scrolling.", class_="pass")


if __name__ == "__main__":
    print(app.run(win))
