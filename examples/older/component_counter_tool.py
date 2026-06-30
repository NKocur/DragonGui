from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


@dg.component
def CounterTool(ctx: dg.ComponentCtx) -> dg.Window:
    count = ctx.state("count", 0)
    tone = ctx.state("tone", "blue")

    palette = {
        "blue": {
            "panel": {"border_color": "#3fc7ff", "accent": "#3fc7ff"},
            "label": {"color": "#b9f6ff", "font_size": 20, "font_weight": "bold"},
        },
        "green": {
            "panel": {"border_color": "success", "accent": "success"},
            "label": {"color": "success", "font_size": 20, "font_weight": "bold"},
        },
        "yellow": {
            "panel": {"border_color": "warning", "accent": "warning"},
            "label": {"color": "warning", "font_size": 20, "font_weight": "bold"},
        },
    }
    current = palette[str(tone.value)]

    def increment() -> None:
        count.set(int(count.value) + 1)

    def cycle_tone() -> None:
        order = ["blue", "green", "yellow"]
        tone.set(order[(order.index(str(tone.value)) + 1) % len(order)])

    win = dg.Window("DragonGUI Component Counter", width=760, height=430, id="component-window")
    with dg.HLayout(id="root-row", key="root-row", parent=win, style={"padding": 14, "gap": 16}):
        with dg.Panel(
            "Component state",
            id="controls",
            key="controls",
            width=320,
            style={"padding": 14, "gap": 10, **current["panel"]},
        ):
            dg.Label(
                f"Count: {count.value}",
                id="count-label",
                key="count-label",
                style=current["label"],
            )
            dg.Button("Increment", id="increment", key="increment", on_click=increment)
            dg.Button("Cycle Tone", id="cycle-tone", key="cycle-tone", on_click=cycle_tone)

        with dg.Panel("What to test", id="notes", key="notes", style={"padding": 14, "gap": 10}):
            dg.Label(
                "Increment sends a live patch.",
                id="note-1",
                key="note-1",
            )
            dg.Label(
                "Cycle Tone patches style.",
                id="note-2",
                key="note-2",
            )
            dg.Label(
                "Python runs on state changes.",
                id="note-3",
                key="note-3",
            )

    return win


if __name__ == "__main__":
    print(dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33")).run(CounterTool()))
