from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


def log(message: str) -> None:
    print(message, flush=True)


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #0c111d;
        color: rgba(246, 249, 255, 0.94);
        padding: 0;
        gap: 0;
        font-size: 14px;
    }

    MenuBar {
        background: rgba(7, 12, 22, 0.96);
        border-bottom: 1px solid rgba(255, 255, 255, 0.12);
        color: rgba(246, 249, 255, 0.86);
        padding-left: 8px;
    }

    Menu {
        background: transparent;
        border: 0;
        border-radius: 6px;
        color: rgba(246, 249, 255, 0.82);
        padding: 6px 10px;
    }

    Menu:hover {
        background: rgba(255, 255, 255, 0.08);
        color: white;
    }

    Menu:open {
        background: rgba(90, 169, 255, 0.16);
        color: white;
    }

    HLayout.shell {
        width: 100%;
        height: calc(100% - 34px);
        gap: 0;
    }

    Sidebar.nav {
        background:
            radial-gradient(circle at 12% 6%, rgba(90, 169, 255, 0.18), transparent 45%),
            rgba(12, 18, 30, 0.98);
        border-right: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 0;
        padding: 14px;
        gap: 8px;
        overflow-y: auto;
    }

    Sidebar.nav::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    Sidebar.nav::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.68);
        border-radius: 999px;
    }

    VLayout.content {
        width: calc(100% - 238px);
        height: 100%;
        padding: 16px;
        gap: 12px;
        overflow-y: auto;
        padding-right: 24px;
        padding-bottom: 64px;
    }

    VLayout.content::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.content::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.72);
        border-radius: 999px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(246, 249, 255, 0.72);
        line-height: 1.12;
    }

    Label.nav-title {
        color: white;
        font-size: 16px;
        font-weight: 850;
    }

    NavItem {
        color: rgba(246, 249, 255, 0.78);
    }

    NavItem::item {
        background: transparent;
        border-radius: 10px;
        padding: 9px 10px;
    }

    NavItem:hover::item {
        background: rgba(255, 255, 255, 0.07);
    }

    NavItem:selected::item {
        background: rgba(90, 169, 255, 0.18);
        border: 1px solid rgba(90, 169, 255, 0.36);
    }

    NavItem::accent {
        background: #5aa9ff;
        border-radius: 999px;
        width: 4px;
    }

    NavItem:disabled {
        color: rgba(246, 249, 255, 0.34);
    }

    Badge,
    Tag {
        font-weight: 850;
    }

    HLayout.badges {
        height: 28px;
        gap: 6px;
        align-items: center;
    }

    Panel.case {
        background:
            radial-gradient(circle at 14% 12%, rgba(90, 169, 255, 0.13), transparent 50%),
            rgba(18, 25, 40, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 14px;
        padding: 14px;
        gap: 10px;
        box-shadow: none;
    }

    VLayout.panel-stack {
        width: 100%;
        height: auto;
        gap: 14px;
    }

    Panel.half {
        width: 100%;
        height: 260px;
    }

    Pages {
        width: 100%;
        height: 1160px;
    }

    Tabs {
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 12px;
    }

    Tabs::header {
        background: rgba(255, 255, 255, 0.055);
        border-bottom: 1px solid rgba(255, 255, 255, 0.10);
    }

    Tab::tab {
        background: transparent;
        border-radius: 9px 9px 0 0;
        padding: 8px 12px;
    }

    Tab:selected::tab {
        background: rgba(90, 169, 255, 0.18);
        color: white;
    }

    Tab::accent {
        background: #74ddb0;
        height: 3px;
        border-radius: 999px 999px 0 0;
    }

    Page {
        padding: 12px;
        gap: 10px;
    }

    Button.context-target {
        background: rgba(90, 169, 255, 0.16);
        border: 1px solid rgba(90, 169, 255, 0.34);
        border-radius: 10px;
        color: white;
        font-weight: 800;
        width: 180px;
    }

    Label.pass {
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 10px;
        color: rgba(229, 255, 244, 0.96);
        font-weight: 800;
        padding: 8px 10px;
        width: 100%;
    }
    """
)


win = dg.Window("CSS Navigation Widgets Probe", width=980, height=700)

with dg.MenuBar(height=34):
    with dg.Menu("File"):
        dg.MenuItem("New workspace", on_click=lambda: log("menu:new"))
        dg.MenuItem("Open recent", on_click=lambda: log("menu:recent"))
        dg.MenuItem("Disabled export", disabled=True)
    with dg.Menu("View"):
        dg.MenuItem("Refresh", on_click=lambda: log("menu:refresh"))
        dg.MenuItem("Toggle inspector", on_click=lambda: log("menu:inspector"))
    with dg.Menu("Help", disabled=True):
        dg.MenuItem("Disabled help")

with dg.HLayout(class_="shell"):
    with dg.Sidebar(title="Navigator", width=238, class_="nav"):
        dg.Label("Navigation", class_="nav-title")
        with dg.HLayout(class_="badges"):
            dg.Badge("live", level="success")
            dg.Tag("v2", level="info")
        dg.Separator()
        dg.NavItem("Overview", page="overview", badge="3")
        dg.NavItem("Analytics", page="analytics", badge=12)
        dg.NavItem("Reports", page="reports")
        dg.NavItem("Settings", page="settings", badge="new")
        dg.NavItem("Disabled", page="disabled", disabled=True)
        dg.Separator()
        for index in range(1, 11):
            dg.NavItem(
                f"Overflow item {index}",
                page=f"overflow-{index}",
                badge=index if index % 3 == 0 else None,
            )
        dg.Spacer(height=12)
        dg.Label("PASS: sidebar scrolls without covering nav text.", class_="caption")

    with dg.VLayout(class_="content"):
        dg.Label("Navigation widgets", class_="title")
        dg.Label(
            "This probe isolates app navigation controls: MenuBar/Menu/MenuItem, Sidebar/NavItem, "
            "Tabs/Tab, Pages/Page, badges, disabled states, selected states, and ContextMenu.",
            class_="caption",
        )

        with dg.Pages(value="analytics", on_change=lambda value: log(f"page:{value}")):
            with dg.Page("overview", title="Overview"):
                with dg.Panel("Overview page", class_="case"):
                    dg.Label("Select Analytics in the sidebar to test the active nav state.")
                    dg.Label("PASS: inactive pages remain hidden.")

            with dg.Page("analytics", title="Analytics"):
                with dg.VLayout(class_="panel-stack"):
                    with dg.Panel("Tabs and badges", class_="case half"):
                        with dg.Tabs(value="summary", on_change=lambda value: log(f"tab:{value}")):
                            with dg.Tab("Summary", value="summary", badge="4"):
                                dg.Label("Selected tab should have a cohesive top shape.")
                                dg.Label("The badge should fit without clipping the tab label.")
                                dg.Label("PASS: selected tab, badge, and accent align cleanly.", class_="pass")
                            with dg.Tab("Queue", value="queue", badge=9):
                                dg.Label("Queue tab content")
                            with dg.Tab("Disabled", value="disabled-tab", disabled=True):
                                dg.Label("Disabled tab content should not become active.")

                    with dg.Panel("Context menu target", class_="case half"):
                        target = dg.Button(
                            "Right click me",
                            class_="context-target",
                            on_click=lambda: log("context target clicked"),
                        )
                        dg.Label("Right-click the button to check ContextMenu placement and style.")
                        dg.Label("PASS: context menu opens above content and keeps readable spacing.", class_="pass")

                    with dg.Panel("Menu bar behavior", class_="case half"):
                        dg.Label("Top menus should look like menus, not standalone rounded buttons.")
                        dg.Label("Open states should feel attached to the menu bar.")
                        dg.Label("Disabled Help should read visibly disabled.", class_="pass")

                    with dg.Panel("Page route state", class_="case half"):
                        dg.Label("Sidebar selection should track the active Pages value.")
                        dg.Label("Nav badges should remain inside the row at narrow widths.")
                        dg.Label("PASS: selected NavItem has a full-row shape, not a detached strip.", class_="pass")

            with dg.Page("reports", title="Reports"):
                with dg.Panel("Reports page", class_="case"):
                    dg.Label("Reports route selected.")

            with dg.Page("settings", title="Settings"):
                with dg.Panel("Settings page", class_="case"):
                    dg.Label("Settings route selected.")

            with dg.Page("disabled", title="Disabled"):
                with dg.Panel("Disabled page", class_="case"):
                    dg.Label("This route should not be reachable from the disabled nav item.")


with dg.ContextMenu(target=target, width=220, parent=win):
    dg.MenuItem("Inspect route", on_click=lambda: log("context:inspect"))
    dg.MenuItem("Duplicate tab", on_click=lambda: log("context:duplicate"))
    dg.MenuItem("Disabled action", disabled=True)


if __name__ == "__main__":
    print(app.run(win))
