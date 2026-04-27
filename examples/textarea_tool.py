from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


app = dg.App()
win = dg.Window("DragonGUI TextArea Demo", width=860, height=520)
status: dg.Label | None = None
editor: dg.TextArea | None = None


def on_notes_change(value: str) -> None:
    if status is not None:
        lines = value.count("\n") + 1 if value else 0
        status.set_value(f"{len(value)} chars, {lines} lines")


def load_sample() -> None:
    if editor is not None:
        editor.set_value("select *\nfrom orders\nwhere total > 100\norder by created_at desc")


with dg.HLayout(style={"padding": 18, "gap": 16}):
    with dg.Panel("Editor", width=500, style={"padding": 14, "gap": 10}):
        editor = dg.TextArea(
            "Try typing multiple lines here.",
            rows=8,
            placeholder="SQL query, notes, prompt, or log text...",
            on_change=on_notes_change,
            style={"width": 460},
        )
        with dg.HLayout(style={"gap": 8}):
            dg.Button("Load Sample", on_click=load_sample)
            dg.Button("Notify", on_click=lambda: dg.toast("TextArea value updated"))

    with dg.Panel("Details", style={"padding": 14, "gap": 10}):
        dg.Label("TextArea preserves newlines and reports the full value.")
        dg.Label("Rows set the preferred height; text is clipped to the field.")
        status = dg.Label("31 chars, 1 lines")
        dg.Separator()
        dg.TextArea(
            "",
            rows=4,
            placeholder="Placeholder text appears while empty.",
            wrap=True,
        )
        dg.TextArea(
            "Disabled\nmultiline value",
            rows=3,
            disabled=True,
        )


if __name__ == "__main__":
    print(app.run(win))
