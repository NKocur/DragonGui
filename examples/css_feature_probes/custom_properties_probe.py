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
        --brand: #5aa9ff;
        --success: #74ddb0;
        --warning: #ffd36a;
        --radius: 12px;
        --gap: 10px;
        --shadow-color: rgba(0, 0, 0, 0.28);
        --card-width: 260px;
    }

    Window {
        background: #0d1320;
        color: rgba(245, 248, 255, 0.94);
        padding: 18px;
        gap: var(--gap);
        font-size: 14px;
    }

    Panel {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        box-shadow: 0 10px 24px var(--shadow-color);
        padding: 14px;
        gap: 8px;
    }

    Label.title {
        font-size: 20px;
        font-weight: 800;
        color: var(--brand);
    }

    Label.caption {
        color: rgba(245, 248, 255, 0.72);
        line-height: 1.18;
    }

    Panel.root-vars {
        width: var(--card-width);
        background: var(--surface-alt);
        border-color: var(--brand);
    }

    Panel.fallback-vars {
        width: var(--missing-width, 260px);
        background: var(--missing-bg, rgba(255, 211, 106, 0.11));
        border-color: var(--missing-border, var(--warning));
    }

    Panel.selector-local {
        --local-bg:
            radial-gradient(circle at 20% 18%, rgba(116, 221, 176, 0.20), transparent 56%),
            linear-gradient(135deg, rgba(90, 169, 255, 0.16), rgba(255, 255, 255, 0.06));
        --local-border: rgba(116, 221, 176, 0.48);
        --local-radius: 16px;
        width: var(--card-width);
        background: var(--local-bg);
        border-color: var(--local-border);
        border-radius: var(--local-radius);
    }

    Panel.embedded-vars {
        width: var(--card-width);
        border: 2px solid var(--success);
        box-shadow:
            0 10px 24px var(--shadow-color),
            inset 0 1px 0 var(--success);
        background:
            linear-gradient(135deg, var(--brand), transparent),
            var(--surface);
    }

    Panel.calc-vars {
        width: calc(var(--card-width) + 60px);
        min-width: calc(var(--card-width) - 40px);
        padding: calc(var(--gap) + 8px);
        background: rgba(90, 169, 255, 0.10);
        border-color: rgba(90, 169, 255, 0.42);
    }

    Panel.no-inherit-parent {
        --child-color: #ff6584;
        width: var(--card-width);
        border-color: rgba(255, 101, 132, 0.48);
    }

    Label.no-inherit-child {
        color: var(--child-color, rgba(245, 248, 255, 0.72));
    }
    """
)


win = dg.Window("CSS Custom Properties Probe", width=760, height=620)

with dg.VLayout(style={"gap": 12}):
    dg.Label("Custom properties / var()", class_="title")
    dg.Label(
        "This probe isolates root variables, fallbacks, selector-local variables, "
        "embedded var() values, calc(), and the current no-inheritance limitation.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="root-vars"):
            dg.Label(":root variables", class_="title")
            dg.Label("Uses --surface-alt, --brand, --radius, and --card-width.", class_="caption")

        with dg.Panel(class_="fallback-vars"):
            dg.Label("Fallback variables", class_="title")
            dg.Label("Missing variables fall back to width, background, and border values.", class_="caption")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="selector-local"):
            dg.Label("Selector-local variables", class_="title")
            dg.Label("Variables declared in this rule are reused by the same rule block.", class_="caption")

        with dg.Panel(class_="embedded-vars"):
            dg.Label("Embedded var()", class_="title")
            dg.Label("var() is used inside border, shadow, and layered background values.", class_="caption")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="calc-vars"):
            dg.Label("calc() with var()", class_="title")
            dg.Label("Width, min-width, and padding use variables inside calc().", class_="caption")

        with dg.Panel(class_="no-inherit-parent"):
            dg.Label("No inherited custom vars", class_="title")
            dg.Label(
                "This text should use the fallback color, because --child-color "
                "does not inherit into a separate child selector yet.",
                class_="caption no-inherit-child",
            )


if __name__ == "__main__":
    print(app.run(win))

