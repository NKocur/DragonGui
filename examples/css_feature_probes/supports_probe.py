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
        --border: rgba(255, 255, 255, 0.13);
        --muted: rgba(245, 248, 255, 0.72);
        --pass: #74ddb0;
        --pass-bg: rgba(116, 221, 176, 0.12);
        --blue: #5aa9ff;
        --blue-bg: rgba(90, 169, 255, 0.12);
        --fail-bg: rgba(255, 101, 132, 0.20);
    }

    Window {
        background: #0d1320;
        color: rgba(245, 248, 255, 0.94);
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

    Label.title {
        font-size: 20px;
        font-weight: 800;
        color: var(--blue);
    }

    Label.caption {
        color: var(--muted);
        line-height: 1.15;
    }

    Label.case-title {
        font-weight: 800;
        color: rgba(245, 248, 255, 0.94);
    }

    Panel.case {
        width: 310px;
        min-height: 104px;
    }

    Panel.pass {
        background: var(--pass-bg);
        border-color: rgba(116, 221, 176, 0.44);
    }

    Panel.expected-false {
        background: rgba(255, 211, 106, 0.10);
        border-color: rgba(255, 211, 106, 0.34);
    }

    @supports (display: grid) {
        Panel.declaration-pass {
            border-color: var(--pass);
            background: var(--pass-bg);
        }

        Label.declaration-pass::before {
            content: "PASS ";
            color: var(--pass);
            font-weight: 800;
        }
    }

    @supports (display: inline-grid) {
        Panel.declaration-fail {
            background: var(--fail-bg);
            border-color: #ff6584;
        }

        Label.declaration-fail::before {
            content: "UNEXPECTED ";
            color: #ff6584;
            font-weight: 800;
        }
    }

    @supports selector(Panel > Button.primary) {
        Panel.selector-pass {
            border-color: var(--blue);
            background: var(--blue-bg);
        }

        Label.selector-pass::before {
            content: "PASS ";
            color: var(--blue);
            font-weight: 800;
        }
    }

    @supports selector(Widget.unknown) {
        Panel.selector-fail {
            background: var(--fail-bg);
            border-color: #ff6584;
        }
    }

    @supports font-format(ttf) {
        Panel.font-format-pass {
            border-color: var(--pass);
            background: var(--pass-bg);
        }

        Label.font-format-pass::before {
            content: "PASS ";
            color: var(--pass);
            font-weight: 800;
        }
    }

    @supports font-format(woff2) {
        Panel.font-format-fail {
            background: var(--fail-bg);
            border-color: #ff6584;
        }
    }

    @supports at-rule(@media) {
        Panel.at-rule-pass {
            border-color: var(--blue);
            background: var(--blue-bg);
        }

        Label.at-rule-pass::before {
            content: "PASS ";
            color: var(--blue);
            font-weight: 800;
        }
    }

    @supports at-rule(@container) {
        Panel.container-at-rule-pass {
            border-color: var(--pass);
            background: var(--pass-bg);
        }

        Label.container-at-rule-pass::before {
            content: "PASS ";
            color: var(--pass);
            font-weight: 800;
        }
    }

    @supports font-tech(features-opentype) {
        Panel.font-tech-pass {
            border-color: var(--pass);
            background: var(--pass-bg);
        }

        Label.font-tech-pass {
            font-variant-numeric: tabular-nums;
        }

        Label.font-tech-pass::before {
            content: "PASS ";
            color: var(--pass);
            font-weight: 800;
        }
    }

    @supports font-tech(color-COLRv1) {
        Panel.font-tech-fail {
            background: var(--fail-bg);
            border-color: #ff6584;
        }
    }
    """
)


win = dg.Window("CSS @supports Probe", width=760, height=650)

with dg.VLayout(style={"gap": 12}):
    dg.Label("@supports feature queries", class_="title")
    dg.Label(
        "Green/blue panels are expected to pass. Yellow panels should stay yellow; "
        "if any turn red, an unsupported query incorrectly matched.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="case declaration-pass"):
            dg.Label("Declaration query", class_="case-title declaration-pass")
            dg.Label("(display: grid) should match.", class_="caption")

        with dg.Panel(class_="case expected-false declaration-fail"):
            dg.Label("Unsupported declaration", class_="case-title declaration-fail")
            dg.Label("(display: inline-grid) should not match.", class_="caption")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="case selector-pass"):
            dg.Label("Selector query", class_="case-title selector-pass")
            dg.Button("Primary button", class_="primary")
            dg.Label("selector(Panel > Button.primary) should match.", class_="caption")

        with dg.Panel(class_="case expected-false selector-fail"):
            dg.Label("Unsupported selector", class_="case-title")
            dg.Label("selector(Widget.unknown) should not match.", class_="caption")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="case font-format-pass"):
            dg.Label("Font format", class_="case-title font-format-pass")
            dg.Label("font-format(ttf) should match.", class_="caption")

        with dg.Panel(class_="case expected-false font-format-fail"):
            dg.Label("Unsupported font format", class_="case-title")
            dg.Label("font-format(woff2) should not match yet.", class_="caption")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="case at-rule-pass"):
            dg.Label("At-rule", class_="case-title at-rule-pass")
            dg.Label("at-rule(@media) should match.", class_="caption")

        with dg.Panel(class_="case container-at-rule-pass"):
            dg.Label("Container at-rule", class_="case-title container-at-rule-pass")
            dg.Label("at-rule(@container) should match.", class_="caption")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="case font-tech-pass"):
            dg.Label("Font tech", class_="case-title font-tech-pass")
            dg.Label("features-opentype 1234567890 should match.", class_="caption font-tech-pass")

        with dg.Panel(class_="case expected-false font-tech-fail"):
            dg.Label("Unsupported font tech", class_="case-title")
            dg.Label("color-COLRv1 should not match yet.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
