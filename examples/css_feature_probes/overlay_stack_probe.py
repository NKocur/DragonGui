from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


def log(message: str) -> None:
    print(message, flush=True)


app = dg.App(theme=dg.Theme.dark(accent="#63b3ff", radius=8, focus="#ffd36a"))

BASE_CSS = """
Window {
    background: #0a0f1d;
    color: rgba(248, 250, 252, 0.94);
    padding: 0;
    gap: 0;
    font-size: 14px;
}

MenuBar {
    background: rgba(6, 11, 21, 0.98);
    border-bottom: 1px solid rgba(255, 255, 255, 0.13);
    color: rgba(248, 250, 252, 0.82);
    padding-left: 8px;
}

Menu {
    background: transparent;
    border: 0;
    border-radius: 6px;
    color: rgba(248, 250, 252, 0.82);
    padding: 6px 10px;
}

Menu:hover,
Menu:open {
    background: rgba(99, 179, 255, 0.16);
    color: white;
}

MenuItem {
    color: rgba(248, 250, 252, 0.90);
    padding: 7px 10px;
}

MenuItem:hover {
    background: rgba(99, 179, 255, 0.16);
}

VLayout.root {
    width: 100%;
    height: calc(100% - 34px);
    padding: 16px;
    padding-right: 28px;
    padding-bottom: 80px;
    gap: 14px;
    overflow-y: auto;
}

VLayout.root::scrollbar-track,
Panel::scrollbar-track {
    width: 8px;
    padding: 2px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 999px;
}

VLayout.root::scrollbar-thumb,
Panel::scrollbar-thumb {
    width: 6px;
    background: linear-gradient(180deg, #63b3ff, #7dd3fc);
    border-radius: 999px;
}

Label.title {
    color: white;
    font-size: 20px;
    font-weight: 850;
}

Label.caption {
    color: rgba(248, 250, 252, 0.70);
    line-height: 1.14;
}

Label.status {
    width: 100%;
    background: rgba(125, 211, 252, 0.12);
    border: 1px solid rgba(125, 211, 252, 0.32);
    border-radius: 10px;
    color: rgba(224, 242, 254, 0.96);
    font-weight: 800;
    padding: 8px 10px;
}

Label.case-title {
    color: white;
    font-weight: 850;
}

HLayout.grid {
    width: 100%;
    height: auto;
    gap: 14px;
}

HLayout.button-row {
    height: 40px;
    gap: 10px;
    align-items: center;
}

Panel {
    background:
        radial-gradient(circle at 10% 8%, rgba(99, 179, 255, 0.12), transparent 48%),
        rgba(17, 24, 39, 0.96);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 14px;
    padding: 14px;
    gap: 10px;
    box-shadow: 0 14px 32px rgba(0, 0, 0, 0.28);
}

Panel.case {
    width: calc(50% - 7px);
    min-width: 390px;
    min-height: 300px;
}

Panel.theme-strip {
    width: 100%;
    min-height: 118px;
}

Panel.edge-zone {
    position: relative;
    min-height: 300px;
}

Panel.scroll-host {
    width: 100%;
    height: 250px;
    overflow-y: auto;
    padding-right: 28px;
    padding-bottom: 16px;
}

Button {
    min-width: 112px;
    border-radius: 10px;
    font-weight: 820;
}

Button.primary {
    background: #63b3ff;
    border-color: rgba(99, 179, 255, 0.72);
    color: #06101f;
}

Button.anchor {
    position: absolute;
    width: 160px;
    background: rgba(99, 179, 255, 0.16);
    border: 1px solid rgba(99, 179, 255, 0.38);
    color: white;
}

Button.top-left {
    left: 14px;
    top: 48px;
}

Button.top-right {
    right: 14px;
    top: 48px;
}

Button.bottom-left {
    left: 14px;
    bottom: 16px;
}

Button.bottom-right {
    right: 14px;
    bottom: 16px;
}

Button::badge {
    background: #ffd36a;
    color: #111827;
    border-radius: 999px;
    font-weight: 900;
    padding: 2px 7px;
}

Dropdown {
    width: 100%;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 10px;
    color: rgba(248, 250, 252, 0.94);
}

Dropdown::menu,
ContextMenu,
Tooltip {
    background: rgba(10, 16, 30, 0.98);
    border: 1px solid rgba(125, 211, 252, 0.32);
    border-radius: 12px;
    color: rgba(248, 250, 252, 0.94);
    box-shadow: 0 20px 42px rgba(0, 0, 0, 0.42);
}

Dropdown::item-hover {
    background: rgba(99, 179, 255, 0.16);
}

Dropdown::item-selected {
    background: rgba(99, 179, 255, 0.28);
    color: white;
    font-weight: 850;
}

Tooltip {
    padding: 12px;
    padding-bottom: 16px;
    gap: 8px;
}

Modal {
    background: #0f172a;
    border: 2px solid #1e3a8a;
    border-radius: 16px;
    color: rgba(248, 250, 252, 0.94);
    padding: 16px;
    padding-bottom: 18px;
    gap: 10px;
    box-shadow: 0 26px 70px rgba(0, 0, 0, 0.50);
}

Modal::titlebar {
    background: linear-gradient(90deg, #1e3a8a, #0e7490);
    color: white;
    border-top-left-radius: 14px;
    border-top-right-radius: 14px;
    padding: 8px 16px;
}

Modal::scrim {
    background: rgba(2, 6, 23, 0.54);
    backdrop-filter: blur(4px) saturate(1.1);
}

.filler {
    width: 100%;
    background: rgba(255, 255, 255, 0.045);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    padding: 8px 10px;
}
"""

THEMES = {
    "Glass": """
        Panel {
            background:
                radial-gradient(circle at 8% 12%, rgba(125, 211, 252, 0.20), transparent 46%),
                rgba(15, 23, 42, 0.72);
            backdrop-filter: blur(10px) saturate(1.18);
            border-color: rgba(226, 232, 240, 0.24);
        }

        Dropdown::menu,
        ContextMenu,
        Tooltip {
            background: rgba(15, 23, 42, 0.86);
            backdrop-filter: blur(14px) saturate(1.22);
            border-color: rgba(226, 232, 240, 0.30);
        }
    """,
    "Paper": """
        Window {
            background: #edf2f7;
            color: #172033;
        }

        MenuBar {
            background: #ffffff;
            border-bottom-color: #cbd5e1;
            color: #172033;
        }

        Menu {
            color: #334155;
        }

        Menu:hover,
        Menu:open {
            background: #dbeafe;
            color: #0f172a;
        }

        Panel,
        Dropdown::menu,
        ContextMenu,
        Tooltip {
            background: #ffffff;
            border-color: #b6c3d1;
            color: #172033;
            box-shadow: 0 16px 32px rgba(33, 56, 84, 0.16);
        }

        Label.title,
        Label.case-title {
            color: #0f172a;
        }

        Label.caption {
            color: rgba(15, 23, 42, 0.68);
        }

        Dropdown {
            background: #f8fafc;
            border-color: #9fb0c4;
            color: #0f172a;
        }

        Button.primary {
            background: #2563eb;
            color: white;
        }

        Modal {
            background: #ffffff;
            border-color: #1d4ed8;
            color: #172033;
        }

        Modal::scrim {
            background: rgba(15, 23, 42, 0.30);
            backdrop-filter: blur(3px);
        }
    """,
    "Alarm": """
        Window {
            background: #210707;
            color: #fff7ed;
        }

        MenuBar {
            background: #2a0909;
            border-bottom-color: rgba(251, 191, 36, 0.38);
        }

        Panel,
        Dropdown::menu,
        ContextMenu,
        Tooltip {
            background:
                radial-gradient(circle at 10% 12%, rgba(251, 191, 36, 0.22), transparent 44%),
                #2a0909;
            border-color: #fb923c;
            color: #fff7ed;
            box-shadow: 0 18px 44px rgba(127, 29, 29, 0.36);
        }

        Button.primary,
        Button::badge {
            background: #facc15;
            color: #431407;
        }

        Dropdown {
            background: rgba(67, 20, 7, 0.84);
            border-color: #fdba74;
            color: #fff7ed;
        }

        Modal {
            background: #2a0909;
            border-color: #f97316;
            color: #fff7ed;
        }

        Modal::titlebar {
            background: linear-gradient(90deg, #991b1b, #f97316);
        }
    """,
}

app.stylesheet(BASE_CSS)

win = dg.Window("CSS Overlay Stack Probe", width=960, height=720)

with dg.MenuBar(height=34):
    with dg.Menu("File"):
        dg.MenuItem("New overlay test", on_click=lambda: log("menu:file:new"))
        dg.MenuItem("Save snapshot", on_click=lambda: log("menu:file:save"))
        dg.MenuItem("Disabled export", disabled=True)
    with dg.Menu("View"):
        dg.MenuItem("Show layers", on_click=lambda: log("menu:view:layers"))
        dg.MenuItem("Reset theme", on_click=lambda: log("menu:view:reset"))
    with dg.Menu("Help"):
        dg.MenuItem("Overlay rules", on_click=lambda: log("menu:help:rules"))

with dg.VLayout(class_="root"):
    dg.Label("Overlay stack", class_="title")
    dg.Label(
        "Stress-test overlay ordering, clipping, backdrop readability, and live theme swaps. "
        "Open the modal, fire toasts, right-click edge targets, and open the dropdown while scrolled.",
        class_="caption",
    )

    status = dg.Label("Interact with an overlay control to update this status line.", class_="status")
    modal = dg.Modal("Overlay stack modal", open=False, width=440, height=220)

    def set_status(message: str) -> None:
        status.set_value(message)

    def apply_theme(name: str) -> None:
        app.stylesheet(f"{BASE_CSS}\n{THEMES[name]}")
        set_status(f"Theme switched: {name}")

    def show_toast(level: str) -> None:
        set_status(f"{level.title()} toast requested")
        dg.toast(f"{level.title()} overlay toast", level=level, duration=1800)

    with dg.Panel("Theme and overlay actions", class_="theme-strip"):
        dg.Label("Switch themes while overlays are open. Surfaces should restyle without losing z-order.", class_="case-title")
        with dg.HLayout(class_="button-row"):
            for theme_name in THEMES:
                dg.Button(theme_name, on_click=lambda name=theme_name: apply_theme(name))
            dg.Button("Open modal", class_="primary", on_click=modal.show)
            dg.Button("Success toast", on_click=lambda: show_toast("success"))
            dg.Button("Error toast", on_click=lambda: show_toast("error"))

    with dg.HLayout(class_="grid"):
        with dg.Panel("Modal, toast, and tooltip", class_="case"):
            dg.Label("Modal scrim should separate underlying text. Toasts should sit above page content.", class_="case-title")
            with dg.HLayout(class_="button-row"):
                inspect = dg.Button(
                    "Hover details",
                    badge="?",
                    class_="primary",
                    on_click=lambda: set_status("Tooltip target clicked"),
                    tooltip="Simple tooltip also works on this control.",
                )
                dg.Button("Open modal", on_click=modal.show)
                dg.Button("Info toast", on_click=lambda: show_toast("info"))
            dg.Label(
                "This dense text intentionally sits under modal and toast overlays so opacity, "
                "backdrop, and text separation issues are easy to spot.",
                class_="caption filler",
            )
            with dg.Tooltip(target=inspect, width=310, height=142):
                dg.Label("Rich tooltip", class_="case-title")
                dg.Label("The tooltip is a real overlay container with child widgets.", class_="caption")
                dg.ProgressBar(0.72, show_value=True)

        with dg.Panel("Dropdown inside scroll container", class_="case"):
            dg.Label("Open the dropdown, then scroll this panel. The dropdown should stay readable and above panel content.", class_="case-title")
            with dg.Panel("Scrollable dropdown host", class_="scroll-host"):
                for index in range(1, 5):
                    dg.Label(f"Filler row {index}: visible before dropdown.", class_="filler")
                dg.Dropdown(
                    ("Nearest edge", "Inside scroll panel", "Above modal", "Under tooltip"),
                    value="Inside scroll panel",
                    on_change=lambda value: set_status(f"Dropdown selected: {value}"),
                )
                for index in range(5, 13):
                    dg.Label(f"Filler row {index}: scroll behind overlay.", class_="filler")

    with dg.HLayout(class_="grid"):
        with dg.Panel("Context menu edge clamp", class_="case edge-zone"):
            dg.Label("Right-click each edge target. Menus should clamp inside the window.", class_="caption")
            top_left = dg.Button("Top left", class_="anchor top-left", on_click=lambda: set_status("Top left clicked"))
            top_right = dg.Button("Top right", class_="anchor top-right", on_click=lambda: set_status("Top right clicked"))
            bottom_left = dg.Button("Bottom left", class_="anchor bottom-left", on_click=lambda: set_status("Bottom left clicked"))
            bottom_right = dg.Button("Bottom right", class_="anchor bottom-right", on_click=lambda: set_status("Bottom right clicked"))

        with dg.Panel("Scrollable background density", class_="case"):
            dg.Label("Use this content to check overlay readability during root scrolling.", class_="case-title")
            for index in range(1, 10):
                dg.Label(f"Background row {index}: overlay text should not mix with this text.", class_="filler")

    with modal:
        dg.Label("Modal content should render above menus, page text, and scroll content.", class_="case-title")
        dg.Label("Try firing a toast while this modal is open; both surfaces should remain legible.", class_="caption")
        with dg.HLayout(class_="button-row"):
            dg.Button("Toast on modal", on_click=lambda: show_toast("warning"))
            dg.Button("Close", class_="primary", on_click=modal.close)


for target, name in [
    (top_left, "top-left"),
    (top_right, "top-right"),
    (bottom_left, "bottom-left"),
    (bottom_right, "bottom-right"),
]:
    with dg.ContextMenu(target=target, width=230, parent=win):
        dg.MenuItem(f"Inspect {name}", on_click=lambda n=name: set_status(f"Inspect {n}"))
        dg.MenuItem("Show toast", on_click=lambda n=name: show_toast("info"))
        dg.MenuItem("Disabled menu item", disabled=True)


if __name__ == "__main__":
    print(app.run(win))
