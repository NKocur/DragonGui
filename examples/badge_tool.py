from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


app = dg.App()
app.stylesheet(
    """
    Button::badge,
    Tab::badge,
    NavItem::badge {
        background: warning;
        color: background;
        border-radius: 999px;
        font-size: 11px;
    }

    Button.danger::badge {
        background: danger;
        color: text;
    }
    """
)

win = dg.Window("DragonGUI Badge Demo", width=860, height=520)
filter_button: dg.Button | None = None
jobs_tab: dg.Tab | None = None
errors_nav: dg.NavItem | None = None
status: dg.Label | None = None


def clear_badges() -> None:
    if filter_button is not None:
        filter_button.set_badge(None)
    if jobs_tab is not None:
        jobs_tab.set_badge(None)
    if errors_nav is not None:
        errors_nav.set_badge(None)
    if status is not None:
        status.set_value("Badges cleared")


def refresh_badges() -> None:
    if filter_button is not None:
        filter_button.set_badge(8)
    if jobs_tab is not None:
        jobs_tab.set_badge("new")
    if errors_nav is not None:
        errors_nav.set_badge(12)
    if status is not None:
        status.set_value("Badges refreshed")
    dg.toast("Badge counts refreshed")


with dg.HLayout(style={"padding": 18, "gap": 16}):
    with dg.Sidebar(title="Workspace", width=220, style={"padding": 12, "gap": 8}):
        dg.NavItem("Overview", page="overview")
        errors_nav = dg.NavItem("Errors", page="errors", badge=12)
        dg.NavItem("Exports", page="exports", badge="3")

    with dg.VLayout(style={"gap": 12}):
        with dg.HLayout(style={"gap": 8, "height": 40}):
            filter_button = dg.Button("Filters", badge=3, on_click=refresh_badges)
            dg.Button("Sync", badge="!", class_="danger", on_click=lambda: dg.toast("Sync failed", level="error"))
            dg.Button("Clear", on_click=clear_badges)

        with dg.Tabs(value="jobs"):
            with dg.Tab("Jobs", value="jobs", badge="new") as tab:
                jobs_tab = tab
                with dg.Panel("Jobs", style={"padding": 14, "gap": 10}):
                    dg.Label("Badges reserve space at the right edge of the control.")
                    status = dg.Label("Badges ready")
                    dg.ProgressBar(0.72, show_value=True)
            with dg.Tab("Logs", value="logs", badge=99):
                with dg.Panel("Logs", style={"padding": 14, "gap": 10}):
                    dg.TextArea("Build started\nRunning checks\nWaiting for export", rows=7)


if __name__ == "__main__":
    print(app.run(win))
