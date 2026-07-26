from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


def build_app() -> tuple[dg.App, dg.Window]:
    app = dg.App(theme=dg.Theme.dark())

    with dg.Window("Responsive DragonGUI Application", width=1100, height=720) as window:
        with dg.AppShell(class_="template-shell"):
            sidebar = dg.Sidebar(
                title="Northstar",
                width=224,
                collapsed_width=64,
                id="template-sidebar",
            )
            with sidebar:
                dg.Label("WORKSPACE", class_="section-label", wrap=False)
                dg.NavItem("Overview", page="overview", badge="live")
                dg.NavItem("Activity", page="activity", badge="12")
                dg.NavItem("Settings", page="settings")

            with dg.WorkbenchLayout(gap=8, padding=10):
                with dg.Toolbar():
                    dg.IconButton(
                        "menu",
                        tooltip="Toggle navigation",
                        on_click=sidebar.toggle_collapsed,
                        id="template-sidebar-toggle",
                    )
                    dg.ToolbarSeparator()
                    dg.SearchBox(
                        "",
                        placeholder="Search the workspace",
                        grow=True,
                        id="template-search",
                    )
                    dg.Button("Create", class_="primary")

                with dg.Body():
                    with dg.Pages(value="overview", id="template-pages"):
                        with dg.Page("overview", title="Overview"):
                            with dg.ScrollArea(axis="y", gap=12, id="overview-scroll"):
                                dg.Label("OPERATIONS / OVERVIEW", class_="section-label")
                                dg.Label("Responsive workspace", class_="page-title")
                                dg.Label(
                                    "The shell, navigation, grid, and scroll owner adapt "
                                    "without application-level geometry workarounds.",
                                    class_="muted",
                                )
                                with dg.GridLayout(
                                    columns={"default": 3, 900: 2, 620: 1},
                                    min_column_width=200,
                                    gap=12,
                                ):
                                    for title, value, detail in (
                                        ("Availability", "99.98%", "Healthy across all regions"),
                                        ("Active jobs", "184", "Six awaiting review"),
                                        ("Latency", "42 ms", "Within the service target"),
                                    ):
                                        with dg.Panel(title, class_="metric-card"):
                                            dg.Label(value, class_="metric-value", wrap=False)
                                            dg.Label(detail, class_="muted")

                                with dg.Panel("Recent activity"):
                                    for item in (
                                        "Calibration completed for line 4",
                                        "Review task assigned to Operations",
                                        "Telemetry export finished",
                                    ):
                                        dg.Label(item)

                        with dg.Page("activity", title="Activity"):
                            with dg.ScrollArea(axis="y", gap=10):
                                dg.Label("Activity", class_="page-title")
                                for index in range(18):
                                    dg.Label(f"Event {index + 1:02d} · system check completed")

                        with dg.Page("settings", title="Settings"):
                            with dg.ScrollArea(axis="y", gap=10):
                                dg.Label("Settings", class_="page-title")
                                with dg.Panel("Notifications"):
                                    dg.Checkbox("Operational alerts", checked=True)
                                    dg.Checkbox("Weekly summary", checked=False)
                                with dg.Panel("Refresh policy"):
                                    dg.Dropdown(("Automatic", "Every 5 minutes", "Manual"))

                with dg.StatusBar():
                    dg.Badge("ready", level="success")
                    dg.Label("Responsive template")

    app.stylesheet(
        """
        Window {
            background: #0b141d;
        }

        AppShell.template-shell {
            background: transparent;
        }

        Sidebar {
            background: #101f2b;
            border-right: 1px solid rgba(123, 215, 190, 0.20);
        }

        Toolbar,
        StatusBar {
            background: #122330;
        }

        ScrollArea {
            padding: 8px 10px 18px 2px;
        }

        Panel.metric-card {
            min-height: 118px;
        }

        Button.primary {
            background: #72dbbd;
            color: #071914;
            font-weight: 750;
        }

        .section-label {
            color: #72dbbd;
            font-size: 10px;
            font-weight: 800;
            letter-spacing: 1px;
        }

        .page-title {
            font-size: 24px;
            font-weight: 800;
        }

        .metric-value {
            font-size: 26px;
            font-weight: 850;
        }

        .muted {
            color: rgba(220, 235, 242, 0.68);
        }

        @media (max-width: 620px) {
            .page-title {
                font-size: 19px;
            }

            Toolbar {
                padding: 4px;
            }
        }
        """
    )
    return app, window


def main() -> int:
    app, window = build_app()
    print(app.run(window))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
