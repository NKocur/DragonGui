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
    }

    Sidebar.allocation-sidebar {
        padding: 12px;
        gap: 8px;
    }

    Panel.content-sized-status {
        background: rgba(77, 211, 177, 0.10);
        border: 2px solid #4dd3b1;
        padding: 10px;
        gap: 7px;
    }

    .main-region {
        background: rgba(91, 155, 255, 0.08);
    }

    @media (max-width: 500px) {
        Sidebar.allocation-sidebar {
            width: 100%;
        }
    }
    """
)


with dg.Window("Sidebar Flex Allocation Baseline", width=900, height=680) as win:
    with dg.AppShell():
        with dg.Sidebar(title="Allocation", width=220, class_="allocation-sidebar"):
            dg.Label("Navigation")
            dg.NavItem("Overview", page="overview")
            dg.NavItem("Diagnostics", page="diagnostics")
            dg.Separator()
            with dg.Panel("Status should size to content", class_="content-sized-status"):
                dg.LED(True)
                dg.Label("All services nominal")
                dg.ProgressBar(0.82, show_value=False)

        with dg.WorkbenchLayout(class_="main-region"):
            with dg.Toolbar():
                dg.Badge("main region", level="info")
            with dg.Body(gap=12):
                dg.Label("Required workbench", style={"font_size": 24, "font_weight": 850})
                dg.Label(
                    "At desktop size the green sidebar panel should remain compact. "
                    "At phone size this blue workbench must retain usable width or "
                    "the audit must report a starved subtree."
                )
                with dg.Panel("Visible content"):
                    for index in range(1, 7):
                        dg.Label(f"Workbench content row {index}")
            with dg.StatusBar():
                dg.Badge("reachable", level="success")


if __name__ == "__main__":
    print(app.run(win))
