from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8))
app.stylesheet(
    """
    :root {
        --surface: rgba(18, 25, 39, 0.94);
        --surface-alt: rgba(35, 49, 73, 0.94);
        --border: rgba(255, 255, 255, 0.13);
        --muted: rgba(245, 248, 255, 0.72);
        --text: rgba(245, 248, 255, 0.94);
        --blue: #5aa9ff;
        --green: #74ddb0;
        --yellow: #ffd36a;
        --red: #ff6584;
    }

    Window {
        background: #0d1320;
        color: var(--text);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    Panel {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: 12px;
        padding: 14px;
        gap: 8px;
    }

    Panel.case {
        width: 330px;
        min-height: 118px;
    }

    Label.title {
        font-size: 20px;
        font-weight: 800;
        color: var(--blue);
    }

    Label.caption {
        color: var(--muted);
        line-height: 1.12;
    }

    Label.case-title {
        font-weight: 800;
        color: var(--text);
    }

    Label.state {
        display: none;
        padding: 8px;
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 8px;
        font-weight: 800;
    }

    Label.inactive {
        padding: 8px;
        border: 1px solid rgba(255, 211, 106, 0.30);
        border-radius: 8px;
        color: var(--yellow);
        background: rgba(255, 211, 106, 0.10);
    }

    @media (min-width: 700px) {
        Panel.wide {
            border-color: var(--green);
            background: rgba(116, 221, 176, 0.12);
        }

        Label.wide-active {
            display: block;
            color: var(--green);
            background: rgba(116, 221, 176, 0.12);
        }

        Label.wide-inactive {
            display: none;
        }
    }

    @media (max-width: 640px) {
        Window {
            padding: 12px;
        }

        Panel.narrow {
            border-color: var(--blue);
            background: rgba(90, 169, 255, 0.12);
        }

        Label.narrow-active {
            display: block;
            color: var(--blue);
            background: rgba(90, 169, 255, 0.12);
        }

        Label.narrow-inactive {
            display: none;
        }
    }

    @media (orientation: landscape) {
        Panel.landscape {
            border-color: var(--green);
            background: rgba(116, 221, 176, 0.12);
        }

        Label.landscape-active {
            display: block;
            color: var(--green);
            background: rgba(116, 221, 176, 0.12);
        }

        Label.landscape-inactive {
            display: none;
        }
    }

    @media (orientation: portrait) {
        Panel.portrait {
            border-color: var(--blue);
            background: rgba(90, 169, 255, 0.12);
        }

        Label.portrait-active {
            display: block;
            color: var(--blue);
            background: rgba(90, 169, 255, 0.12);
        }

        Label.portrait-inactive {
            display: none;
        }
    }

    @media (pointer: fine) and (hover: hover) {
        Panel.input {
            border-color: var(--green);
            background: rgba(116, 221, 176, 0.12);
        }

        Label.input-active {
            display: block;
            color: var(--green);
            background: rgba(116, 221, 176, 0.12);
        }

        Label.input-inactive {
            display: none;
        }
    }

    @media (prefers-color-scheme: dark) {
        Panel.scheme {
            border-color: var(--blue);
            background: rgba(90, 169, 255, 0.12);
        }

        Label.dark-active {
            display: block;
            color: var(--blue);
            background: rgba(90, 169, 255, 0.12);
        }
    }

    @media (prefers-color-scheme: light) {
        Panel.scheme {
            border-color: var(--green);
            background: rgba(116, 221, 176, 0.12);
        }

        Label.light-active {
            display: block;
            color: var(--green);
            background: rgba(116, 221, 176, 0.12);
        }
    }

    @media (prefers-reduced-motion: no-preference) {
        Panel.motion {
            border-color: var(--green);
            background: rgba(116, 221, 176, 0.12);
        }

        Label.motion-active {
            display: block;
            color: var(--green);
            background: rgba(116, 221, 176, 0.12);
        }

        Label.motion-inactive {
            display: none;
        }
    }

    @media (max-width: 480px), (min-height: 580px) {
        Panel.query-list {
            border-color: var(--blue);
            background: rgba(90, 169, 255, 0.12);
        }

        Label.query-list-active {
            display: block;
            color: var(--blue);
            background: rgba(90, 169, 255, 0.12);
        }

        Label.query-list-inactive {
            display: none;
        }
    }
    """
)


win = dg.Window("CSS @media Probe", width=760, height=620)

with dg.VLayout(style={"gap": 12}):
    dg.Label("@media query matching", class_="title")
    dg.Label(
        "Resize this window to check live media re-evaluation. Green or blue "
        "state boxes are active media rules; yellow boxes are intentionally inactive.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="case wide"):
            dg.Label("min-width: 700px", class_="case-title")
            dg.Label("Active at the default 760px window width.", class_="state wide-active")
            dg.Label("Resize wider than 700px to activate.", class_="inactive wide-inactive")

        with dg.Panel(class_="case narrow"):
            dg.Label("max-width: 640px", class_="case-title")
            dg.Label("Active after narrowing the window below 640px.", class_="state narrow-active")
            dg.Label("Currently inactive until the window is narrow.", class_="inactive narrow-inactive")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="case landscape"):
            dg.Label("orientation: landscape", class_="case-title")
            dg.Label("Landscape rule is active.", class_="state landscape-active")
            dg.Label("Resize to a wider-than-tall window to activate.", class_="inactive landscape-inactive")

        with dg.Panel(class_="case portrait"):
            dg.Label("orientation: portrait", class_="case-title")
            dg.Label("Portrait rule is active.", class_="state portrait-active")
            dg.Label("Resize to a taller-than-wide window to activate.", class_="inactive portrait-inactive")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="case input"):
            dg.Label("pointer + hover", class_="case-title")
            dg.Label("Desktop fine pointer and hover rule is active.", class_="state input-active")
            dg.Label("This should only stay yellow on non-hover/coarse input.", class_="inactive input-inactive")

        with dg.Panel(class_="case query-list"):
            dg.Label("comma query list", class_="case-title")
            dg.Label("Active from min-height: 580px at the default size.", class_="state query-list-active")
            dg.Label("Inactive only if both width > 480px and height < 580px.", class_="inactive query-list-inactive")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="case scheme"):
            dg.Label("prefers-color-scheme", class_="case-title")
            dg.Label("Dark color scheme rule is active.", class_="state dark-active")
            dg.Label("Light color scheme rule is active.", class_="state light-active")
            dg.Label("Exactly one of the two scheme labels should be visible.", class_="caption")

        with dg.Panel(class_="case motion"):
            dg.Label("prefers-reduced-motion", class_="case-title")
            dg.Label("no-preference rule is active.", class_="state motion-active")
            dg.Label("This should stay yellow if the platform reports reduced motion.", class_="inactive motion-inactive")


if __name__ == "__main__":
    print(app.run(win))
