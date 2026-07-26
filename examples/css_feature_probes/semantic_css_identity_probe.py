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
        background: #101722;
        color: #eef7ff;
        font-size: 14px;
    }

    AppShell.semantic-shell {
        background: rgba(77, 211, 177, 0.08);
    }

    WorkbenchLayout.semantic-workbench {
        background: rgba(91, 155, 255, 0.10);
        padding: 18px;
    }

    Toolbar.semantic-toolbar {
        background: rgba(77, 211, 177, 0.20);
        border: 2px solid #4dd3b1;
        padding: 8px;
    }

    SearchBox.semantic-search {
        width: 360px;
        border: 2px solid #ffcf66;
    }

    .class-control {
        width: 360px;
        border: 2px solid #5b9bff;
    }

    Panel.explanation {
        flex-grow: 0;
        background: rgba(255, 255, 255, 0.05);
        padding: 12px;
        gap: 8px;
    }
    """
)


with dg.Window("Semantic CSS Identity Baseline", width=960, height=620) as win:
    with dg.AppShell(class_="semantic-shell"):
        with dg.Sidebar(title="Identity", width=190):
            dg.Label("Public widget types")
            dg.NavItem("Composite selectors", page="identity")

        with dg.WorkbenchLayout(class_="semantic-workbench"):
            with dg.Toolbar(class_="semantic-toolbar"):
                dg.SearchBox(
                    placeholder="SearchBox type selector should make this 360px",
                    class_="semantic-search",
                )
                dg.Spacer()
                dg.Badge("baseline", level="warning")

            with dg.Body(gap=12):
                dg.Label(
                    "Public composite selector baseline",
                    style={"font_size": 22, "font_weight": 850},
                )
                dg.Label(
                    "The gold SearchBox rule, green Toolbar rule, blue WorkbenchLayout "
                    "rule, and AppShell rule should match the public Python widget types."
                )
                with dg.Panel("Class-only control", class_="explanation"):
                    dg.Label(
                        "This control uses only a class selector. It establishes that the "
                        "stylesheet loaded even when public composite type selectors fail."
                    )
                    dg.SearchBox(
                        placeholder="Class selector control: expected 360px",
                        class_="class-control",
                    )
                with dg.Panel("Expected current baseline", class_="explanation"):
                    dg.Label(
                        "Before semantic CSS identity is implemented, SearchBox, Toolbar, "
                        "AppShell, and WorkbenchLayout selectors have no matching native type."
                    )


if __name__ == "__main__":
    print(app.run(win))
