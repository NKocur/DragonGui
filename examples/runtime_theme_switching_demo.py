from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


NEXUS = (
    dg.Theme.dark(accent="#8b7cff", radius=10, spacing=6),
    """
    Window { background: #171b2c; }
    Panel.demo-card { background: #252b43; border: 1px solid #46506e; }
    Button.primary { background: accent; color: #17152a; }
    """,
)

WINDOWS_311 = (
    dg.Theme.light(
        background="#c0c0c0",
        surface="#c0c0c0",
        surface_alt="#ffffff",
        text="#000000",
        muted_text="#404040",
        accent="#000080",
        border="#808080",
        radius=0,
        spacing=5,
        font_size=12,
    ),
    """
    Window { background: #c0c0c0; }
    Panel.demo-card {
        background: #c0c0c0;
        border-top: 2px solid white;
        border-left: 2px solid white;
        border-right: 2px solid #404040;
        border-bottom: 2px solid #404040;
    }
    Button { border-radius: 0; }
    Button.primary { background: #000080; color: white; }
    """,
)

MAC_90S = (
    dg.Theme.light(
        background="#d9d9d9",
        surface="#dedede",
        surface_alt="#ffffff",
        text="#000000",
        muted_text="#505050",
        accent="#333366",
        border="#606060",
        radius=4,
        spacing=6,
        font_size=12,
    ),
    """
    Window { background: #d9d9d9; }
    Panel.demo-card { background: white; border: 1px solid black; }
    Button { border: 1px solid black; border-radius: 5px; }
    Button.primary { background: white; color: black; border-width: 2px; }
    """,
)

STYLES = {
    "Nexus": NEXUS,
    "Windows 3.11": WINDOWS_311,
    "Mac OS 9": MAC_90S,
}


def main() -> None:
    app = dg.App(title="Runtime theme switching", theme=NEXUS[0])
    app.set_stylesheet("appearance", NEXUS[1])
    status: dg.Label | None = None

    def apply_style(name: str) -> None:
        theme, css = STYLES[name]
        app.set_theme(theme)
        app.set_stylesheet("appearance", css)
        if status is not None:
            status.text = f"Active style: {name}"

    with dg.Window("Live Theme Switching", width=820, height=520) as window:
        with dg.VLayout(style={"padding": 16, "gap": 12}):
            dg.Label("Runtime theme and named stylesheet switching")
            dg.Label(
                "The widget tree and interaction state stay intact while the "
                "appearance sheet is replaced in place."
            )
            with dg.HLayout(style={"gap": 8}):
                for style_name in STYLES:
                    dg.Button(
                        style_name,
                        class_="primary" if style_name == "Nexus" else None,
                        on_click=lambda name=style_name: apply_style(name),
                    )
            with dg.Panel("State preservation", class_="demo-card"):
                with dg.VLayout(style={"padding": 12, "gap": 8}):
                    dg.TextInput("Edit this text, then switch styles")
                    dg.Slider(value=42, min=0, max=100)
                    dg.Checkbox("Keep this selection", checked=True)
                    status = dg.Label("Active style: Nexus")

    app.run(window)


if __name__ == "__main__":
    main()
