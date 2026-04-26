from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


@dg.component
def NodeSwapTool(ctx: dg.ComponentCtx) -> dg.Window:
    alternate = ctx.state("alternate", False)

    if alternate.value:
        win = dg.Window("DragonGUI Node Swap B", width=760, height=430, id="swap-window-b", key="swap-window-b")
        with dg.HLayout(parent=win, style={"padding": 14, "gap": 16}):
            with dg.Panel(
                "Replacement root",
                width=320,
                style={"padding": 14, "gap": 10, "border_color": "success", "accent": "success"},
            ):
                dg.Label(
                    "The root widget identity changed.",
                    style={"color": "success", "font_size": 18, "font_weight": "bold"},
                )
                dg.Button("Swap Back", id="swap-back", key="swap-back", on_click=lambda: alternate.set(False))
            with dg.Panel("What to test", style={"padding": 14, "gap": 10}):
                dg.Label("Click Swap Back.")
                dg.Label("Replaces the retained root again.")
                dg.Label("Callbacks should still work.")
    else:
        win = dg.Window("DragonGUI Node Swap A", width=760, height=430, id="swap-window-a", key="swap-window-a")
        with dg.HLayout(parent=win, style={"padding": 14, "gap": 16}):
            with dg.Panel(
                "Primary root",
                width=320,
                style={"padding": 14, "gap": 10, "border_color": "#3fc7ff", "accent": "#3fc7ff"},
            ):
                dg.Label(
                    "This component starts with root A.",
                    style={"color": "#b9f6ff", "font_size": 18, "font_weight": "bold"},
                )
                dg.Button("Swap Root Node", id="swap-root", key="swap-root", on_click=lambda: alternate.set(True))
            with dg.Panel("What this validates", style={"padding": 14, "gap": 10}):
                dg.Label("This exercises VDOM REPLACE_NODE.")
                dg.Label("Installs a new root id.")
                dg.Label("Relayouts without restarting.")

    return win


if __name__ == "__main__":
    print(dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33")).run(NodeSwapTool()))
