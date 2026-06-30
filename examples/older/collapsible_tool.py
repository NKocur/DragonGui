from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


app = dg.App()
win = dg.Window("DragonGUI Collapsible Demo", width=860, height=520)
status: dg.Label | None = None


def on_advanced(expanded: bool) -> None:
    if status is not None:
        status.set_value("Advanced open" if expanded else "Advanced closed")
    dg.toast("Advanced settings expanded" if expanded else "Advanced settings collapsed")


with dg.HLayout(style={"padding": 18, "gap": 16}):
    with dg.Panel("Controls", width=320, style={"padding": 14, "gap": 10}):
        dg.Checkbox("Enable processing", checked=True)
        dg.Slider(0.65)
        with dg.Collapsible(
            "Advanced Settings",
            expanded=False,
            on_change=on_advanced,
            style={
                "parts": {
                    "header": {"background": "surface_alt"},
                    "indicator": {"color": "accent"},
                    "body": {"background": "surface"},
                }
            },
        ) as advanced:
            dg.NumberInput(42, min=0, max=100, step=1)
            dg.Dropdown(["Nearest", "Linear", "Cubic"], value="Linear")
            dg.Checkbox("Normalize before export")
        dg.Separator()
        dg.Button("Open Advanced", on_click=advanced.expand)

    with dg.Panel("Preview", style={"padding": 14, "gap": 10}):
        dg.Label("Collapsible children consume no height while closed.")
        dg.Label("Use Tab to focus the header, then Enter or Space to toggle.")
        status = dg.Label("Advanced closed")
        dg.Label("Current state updates through on_change.")
        dg.Label("Styled header, indicator, and body parts are active.")


if __name__ == "__main__":
    print(app.run(win))
