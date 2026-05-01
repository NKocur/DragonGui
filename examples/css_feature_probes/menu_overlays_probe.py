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
        background: #0b101b;
        color: rgba(246, 249, 255, 0.94);
        padding: 0;
        gap: 0;
        font-size: 14px;
    }

    MenuBar {
        background: rgba(7, 12, 22, 0.98);
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
        background: rgba(90, 169, 255, 0.17);
        color: white;
    }

    VLayout.root {
        width: 100%;
        height: calc(100% - 34px);
        padding: 16px;
        padding-right: 24px;
        padding-bottom: 72px;
        gap: 14px;
        overflow-y: auto;
    }

    VLayout.root::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb {
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

    Panel {
        background:
            radial-gradient(circle at 14% 12%, rgba(90, 169, 255, 0.12), transparent 48%),
            rgba(18, 25, 40, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 14px;
        padding: 14px;
        gap: 10px;
    }

    Panel.edge-grid {
        width: 100%;
        height: 420px;
        position: relative;
    }

    Button.anchor {
        background: rgba(90, 169, 255, 0.16);
        border: 1px solid rgba(90, 169, 255, 0.38);
        border-radius: 10px;
        color: white;
        font-weight: 820;
        width: 176px;
    }

    Button.top-left {
        position: absolute;
        left: 16px;
        top: 18px;
    }

    Button.top-right {
        position: absolute;
        right: 16px;
        top: 18px;
    }

    Button.bottom-left {
        position: absolute;
        left: 16px;
        bottom: 18px;
    }

    Button.bottom-right {
        position: absolute;
        right: 16px;
        bottom: 18px;
    }

    Panel.overlap-card {
        width: 100%;
        height: 190px;
        background:
            linear-gradient(135deg, rgba(116, 221, 176, 0.16), rgba(90, 169, 255, 0.08)),
            rgba(18, 25, 40, 0.96);
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


win = dg.Window("CSS Menu Overlays Probe", width=900, height=680)

with dg.MenuBar(height=34):
    with dg.Menu("File"):
        dg.MenuItem("New project", on_click=lambda: log("file:new"))
        dg.MenuItem("Open recent", on_click=lambda: log("file:recent"))
        dg.MenuItem("Export disabled", disabled=True)
    with dg.Menu("Edit"):
        dg.MenuItem("Undo", on_click=lambda: log("edit:undo"))
        dg.MenuItem("Redo", on_click=lambda: log("edit:redo"))
        dg.MenuItem("Preferences", on_click=lambda: log("edit:preferences"))
    with dg.Menu("Window"):
        dg.MenuItem("Zoom to fit", on_click=lambda: log("window:zoom"))
        dg.MenuItem("Bring all to front", disabled=True)
    with dg.Menu("Disabled", disabled=True):
        dg.MenuItem("Hidden disabled action")

with dg.VLayout(class_="root"):
    dg.Label("Menu overlays", class_="title")
    dg.Label(
        "Open each top menu and right-click each anchor. Menus should stay above content, "
        "avoid clipping near window edges, and render disabled rows clearly.",
        class_="caption",
    )

    with dg.Panel("Context menu edge placement", class_="edge-grid"):
        top_left = dg.Button(
            "Top left target",
            class_="anchor top-left",
            on_click=lambda: log("target:top-left"),
        )
        top_right = dg.Button(
            "Top right target",
            class_="anchor top-right",
            on_click=lambda: log("target:top-right"),
        )
        bottom_left = dg.Button(
            "Bottom left target",
            class_="anchor bottom-left",
            on_click=lambda: log("target:bottom-left"),
        )
        bottom_right = dg.Button(
            "Bottom right target",
            class_="anchor bottom-right",
            on_click=lambda: log("target:bottom-right"),
        )
        dg.Label("PASS: context menus should clamp inside the window, not under the edges.", class_="pass")

    with dg.Panel("Overlay z-order", class_="overlap-card"):
        dg.Label("Open a top menu over this panel.")
        dg.Label("The menu should render above this text and the panel surface.")
        dg.Label("PASS: overlay text remains readable and is not mixed with panel text.", class_="pass")

    with dg.Panel("Scrollable background content"):
        for index in range(1, 9):
            dg.Label(f"Scroll filler row {index}: menu overlays should remain clipped to the window, not this panel.")


for target, name in [
    (top_left, "top-left"),
    (top_right, "top-right"),
    (bottom_left, "bottom-left"),
    (bottom_right, "bottom-right"),
]:
    with dg.ContextMenu(target=target, width=230, parent=win):
        dg.MenuItem(f"Inspect {name}", on_click=lambda n=name: log(f"context:{n}:inspect"))
        dg.MenuItem("Duplicate target", on_click=lambda n=name: log(f"context:{n}:duplicate"))
        dg.MenuItem("Disabled action", disabled=True)


if __name__ == "__main__":
    print(app.run(win))
