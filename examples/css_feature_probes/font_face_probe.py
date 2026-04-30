from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8))
app.stylesheet(
    """
    @font-face {
        font-family: "Probe UI";
        src: local("Segoe UI");
    }

    @font-face {
        font-family: "Probe Mono File";
        src: url("file:///C:/Windows/Fonts/consola.ttf") format("truetype");
    }

    @font-face {
        font-family: "Probe Serif Collection";
        src: url("file:///C:/Windows/Fonts/cambria.ttc") format("collection");
    }

    :root {
        --surface: rgba(18, 25, 39, 0.94);
        --surface-alt: rgba(35, 49, 73, 0.94);
        --border: rgba(255, 255, 255, 0.13);
        --muted: rgba(245, 248, 255, 0.72);
        --text: rgba(245, 248, 255, 0.94);
        --blue: #5aa9ff;
        --green: #74ddb0;
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

    Panel.sample {
        width: 690px;
        background: var(--surface-alt);
    }

    Label.title {
        font-family: "Probe UI";
        font-size: 22px;
        font-weight: 800;
        color: var(--blue);
    }

    Label.caption {
        color: var(--muted);
        line-height: 1.12;
    }

    Label.sample-title {
        font-family: "Probe UI";
        font-size: 13px;
        font-weight: 800;
        color: var(--green);
        text-transform: uppercase;
        letter-spacing: 0.06em;
    }

    Label.sample-text {
        font-size: 23px;
        line-height: 1.08;
        padding: 8px;
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 8px;
        background: rgba(255, 255, 255, 0.045);
    }

    Label.ui-face {
        font-family: "Probe UI";
    }

    Label.mono-face {
        font-family: "Probe Mono File";
        font-variant-numeric: tabular-nums;
    }

    Label.serif-face {
        font-family: "Probe Serif Collection";
        font-style: italic;
    }

    Label.fallback-face {
        font-family: "Missing Probe Face";
    }
    """
)


win = dg.Window("CSS @font-face Probe", width=760, height=620)

with dg.VLayout(style={"gap": 12}):
    dg.Label("@font-face loading", class_="title")
    dg.Label(
        "This probe maps CSS family names to local Windows fonts. The UI, mono, "
        "and serif samples should look visibly different. The final row is an "
        "intentional missing-family fallback for comparison.",
        class_="caption",
    )

    with dg.Panel(class_="sample"):
        dg.Label('local("Segoe UI") as "Probe UI"', class_="sample-title")
        dg.Label(
            "Dashboard Revenue 1234567890 ABCDEFG abcdefg",
            class_="sample-text ui-face",
        )

    with dg.Panel(class_="sample"):
        dg.Label('file URL consola.ttf as "Probe Mono File"', class_="sample-title")
        dg.Label(
            "Dashboard Revenue 1234567890 ABCDEFG abcdefg",
            class_="sample-text mono-face",
        )

    with dg.Panel(class_="sample"):
        dg.Label('file URL cambria.ttc as "Probe Serif Collection"', class_="sample-title")
        dg.Label(
            "Dashboard Revenue 1234567890 ABCDEFG abcdefg",
            class_="sample-text serif-face",
        )

    with dg.Panel(class_="sample"):
        dg.Label("missing family fallback", class_="sample-title")
        dg.Label(
            "Dashboard Revenue 1234567890 ABCDEFG abcdefg",
            class_="sample-text fallback-face",
        )


if __name__ == "__main__":
    print(app.run(win))
