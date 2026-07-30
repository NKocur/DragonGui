from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


CLIENT_CHROME_CSS = """
Window {
    background: #111522;
}

Window::titlebar {
    background: #1d2335;
    border-bottom: 1px solid #38415d;
}

Window::title {
    color: #e8ebf5;
    font-weight: 600;
}

Window::minimize,
Window::maximize,
Window::close {
    background: transparent;
    color: #d8dceb;
    border: 0;
    border-radius: 0;
}

Window::resize-border {
    width: 6px;
}

Window[window-state="maximized"]::maximize {
    background: #343d59;
}

WindowMinimize:hover,
WindowMaximize:hover {
    background: #343d59;
}

WindowClose:hover {
    background: #d94b64;
    color: white;
}

Panel.chrome-card {
    background: #1a2030;
    border: 1px solid #343d59;
}

Button.primary {
    background: accent;
    color: #111522;
}
"""


def main() -> None:
    app = dg.App(
        title="Client window chrome",
        theme=dg.Theme.dark(accent="#8fa8ff", radius=8, spacing=8),
    )
    app.set_stylesheet("client-chrome-demo", CLIENT_CHROME_CSS)

    with dg.Window(
        "DragonGUI Client Chrome Preview",
        width=940,
        height=620,
        decorations="client",
        id="client-chrome-window",
    ) as window:
        with dg.VLayout(style={"padding": 18, "gap": 14}):
            dg.Label("Styleable client-side window decorations")
            dg.Label(
                "Drag the title or empty titlebar area. The retained controls "
                "minimize, toggle maximize, and close the native window. On "
                "Windows and Linux, drag any edge or corner to resize. "
                "Double-click the titlebar to maximize or restore. macOS "
                "safely falls back to native window decorations."
            )
            with dg.HLayout(style={"gap": 12, "flex_grow": 1}):
                with dg.Panel("Chrome behavior", class_="chrome-card"):
                    with dg.VLayout(style={"padding": 14, "gap": 9}):
                        dg.Badge("opt-in")
                        dg.Label("Native decorations remain the default.")
                        dg.Label(
                            "Right-click the titlebar or press Alt+Space for the "
                            "Windows system menu."
                        )
                        dg.Label(
                            "Window::resize-border controls the DPI-scaled resize "
                            "hit region."
                        )
                        dg.Label("Titlebar double-click follows Windows user settings.")
                        dg.Checkbox(
                            "Application controls remain interactive",
                            checked=True,
                            id="client-chrome-checkbox",
                        )
                with dg.Panel("CSS surface", class_="chrome-card"):
                    with dg.VLayout(style={"padding": 14, "gap": 9}):
                        dg.Label("Window::titlebar / Window::title")
                        dg.Label("Window::minimize / ::maximize / ::close")
                        dg.Label("Window::resize-border (structural)")
                        dg.Label("[window-state='maximized'] state selector")
                        dg.Label("Named button metadata + document-order Tab traversal")
                        dg.Button(
                            "Normal application button",
                            class_="primary",
                            id="client-chrome-app-button",
                        )

    app.run(window)


if __name__ == "__main__":
    main()
