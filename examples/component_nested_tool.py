from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


@dg.component
def MetricCard(ctx: dg.ComponentCtx, title: str, accent: str) -> dg.Panel:
    local_count = ctx.state("local_count", 0)

    panel = dg.Panel(
        title,
        id=f"{title.lower()}-card",
        key=f"{title.lower()}-card",
        parent=None,
        style={"padding": 14, "gap": 10, "border_color": accent, "accent": accent},
    )
    dg.Label(
        f"{title}: {local_count.value}",
        id=f"{title.lower()}-value",
        key="value",
        parent=panel,
        style={"font_size": 18, "font_weight": "bold", "color": accent},
    )
    dg.Button(
        f"Increment {title}",
        id=f"{title.lower()}-button",
        key="button",
        parent=panel,
        on_click=lambda: local_count.set(int(local_count.value) + 1),
    )
    dg.Label(
        "Child component state.",
        id=f"{title.lower()}-note",
        key="note",
        parent=panel,
        style={"color": "muted_text"},
    )
    return panel


@dg.component
def NestedTool(ctx: dg.ComponentCtx) -> dg.Window:
    parent_count = ctx.state("parent_count", 0)
    tone = ctx.state("tone", "blue")

    accents = {
        "blue": ("#3fc7ff", "Blue"),
        "green": ("success", "Green"),
        "yellow": ("warning", "Yellow"),
    }
    accent, tone_label = accents[str(tone.value)]

    def bump_parent() -> None:
        parent_count.set(int(parent_count.value) + 1)

    def cycle_tone() -> None:
        order = ["blue", "green", "yellow"]
        tone.set(order[(order.index(str(tone.value)) + 1) % len(order)])

    win = dg.Window("DragonGUI Nested Component Demo", width=960, height=540, id="nested-window")
    with dg.VLayout(id="root", key="root", parent=win, style={"padding": 14, "gap": 14}):
        with dg.Panel(
            "Parent component",
            id="parent-panel",
            key="parent-panel",
            style={"padding": 14, "gap": 10, "border_color": accent, "accent": accent},
        ):
            dg.Label(
                f"Parent rerenders: {parent_count.value}",
                id="parent-count",
                key="parent-count",
                style={"font_size": 18, "font_weight": "bold", "color": accent},
            )
            dg.Label(
                f"Current tone: {tone_label}",
                id="tone-label",
                key="tone-label",
                style={"color": accent},
            )
            with dg.HLayout(id="parent-actions", key="parent-actions", style={"gap": 8}):
                dg.Button("Rerender", id="parent-button", key="parent-button", on_click=bump_parent)
                dg.Button("Tone", id="tone-button", key="tone-button", on_click=cycle_tone)

        with dg.HLayout(id="cards", key="cards", style={"gap": 14, "flex_grow": 1}):
            MetricCard("Alpha", accent, key="alpha")
            MetricCard("Beta", accent, key="beta")

    return win


if __name__ == "__main__":
    print(dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33")).run(NestedTool()))
