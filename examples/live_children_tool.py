from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33"))
win = dg.Window("DragonGUI Live Children Demo", width=860, height=480)

state = {"page": 0}


def make_summary_children() -> list[dg.Label | dg.Separator | dg.Spacer | dg.Button]:
    return [
        dg.Label("Dataset Summary", parent=None, style={"font_size": 18, "font_weight": "bold"}),
        dg.Separator(parent=None),
        dg.Label("Rows: 500,000", parent=None),
        dg.Label("Columns: x, y, z, signal, score, phase", parent=None),
        dg.Spacer(height=8, parent=None),
        dg.Label("Inserted with replace_children(...).", parent=None),
        dg.Button(
            "Dynamic Summary Action",
            parent=None,
            on_click=lambda: status.set_value("Summary dynamic callback fired"),
        ),
    ]


def make_status_children() -> list[dg.Label | dg.Separator | dg.Spacer | dg.Button]:
    return [
        dg.Label("Pipeline Status", parent=None, style={"font_size": 18, "font_weight": "bold"}),
        dg.Separator(parent=None),
        dg.Label("Load: complete", parent=None, style={"color": "success"}),
        dg.Label("Normalize: complete", parent=None, style={"color": "success"}),
        dg.Label("Cluster: waiting", parent=None, style={"color": "warning"}),
        dg.Spacer(height=8, parent=None),
        dg.Label("New callbacks register on insert.", parent=None),
        dg.Button(
            "Dynamic Status Action",
            parent=None,
            on_click=lambda: status.set_value("Status dynamic callback fired"),
        ),
    ]


def swap_children() -> None:
    state["page"] = 1 - state["page"]
    if state["page"] == 0:
        content.replace_children(make_summary_children())
        status.set_value("Showing dataset summary")
    else:
        content.replace_children(make_status_children())
        status.set_value("Showing pipeline status")


with dg.HLayout(style={"gap": 16, "padding": 14}):
    with dg.Panel("Controls", width=300, style={"padding": 14, "gap": 10}):
        status = dg.TextInput("Ready", placeholder="Last action")
        dg.Button("Swap Static Children", on_click=swap_children)
        dg.Label("Preview children update live.")

    with dg.Panel(
        "ReplaceChildren target",
        style={"padding": 18, "gap": 10, "border_color": "accent", "border_radius": 12},
    ) as content:
        for child in make_summary_children():
            content.add(child)


print(app.run(win))
