from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#62d2a2", radius=7, focus="#ffd166"))
app.stylesheet(
    """
    Window {
        background: #101820;
        color: rgba(246, 249, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
    }

    HLayout.grid {
        gap: 12px;
        width: 100%;
        height: auto;
    }

    Panel.case {
        width: calc(50% - 6px);
        min-width: 360px;
        min-height: 300px;
        background: rgba(21, 32, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        padding: 14px;
        gap: 10px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(246, 249, 255, 0.70);
        line-height: 1.12;
    }

    Label.status {
        background: rgba(98, 210, 162, 0.12);
        border: 1px solid rgba(98, 210, 162, 0.34);
        border-radius: 8px;
        color: rgba(230, 255, 244, 0.96);
        font-weight: 750;
        padding: 8px 10px;
        width: 100%;
    }

    VLayout.list {
        gap: 3px;
        width: 100%;
    }

    Selectable {
        height: 32px;
        border-radius: 6px;
        color: rgba(246, 249, 255, 0.88);
        transition: background 120ms ease, border-color 120ms ease, color 120ms ease;
    }

    Selectable:hover {
        background: rgba(98, 210, 162, 0.10);
        border-color: rgba(98, 210, 162, 0.24);
    }

    Selectable:selected {
        background: rgba(98, 210, 162, 0.18);
        border-color: rgba(98, 210, 162, 0.64);
        color: white;
    }

    Selectable::indicator {
        width: 4px;
        height: 16px;
        background: #62d2a2;
        border-radius: 999px;
    }

    Selectable:disabled {
        color: rgba(246, 249, 255, 0.40);
        opacity: 0.68;
    }
    """
)

win = dg.Window("SelectableList probe", width=980, height=620)

with dg.VLayout(class_="root"):
    dg.Label("SelectableList", class_="title")
    status = dg.Label("Selected renderer: GPU", class_="status")

    def set_status(text: str) -> None:
        status.set_value(text)

    with dg.HLayout(class_="grid"):
        with dg.Panel("Single select", class_="case"):
            dg.Label("Single-select rows should keep one selected item and show a stable highlight.", class_="caption")
            dg.SelectableList(
                [
                    ("CPU renderer", "CPU"),
                    ("GPU renderer", "GPU"),
                    {"label": "Remote renderer", "value": "Remote"},
                    {"label": "Unavailable renderer", "value": "Disabled", "disabled": True},
                ],
                value="GPU",
                class_="list",
                on_change=lambda value: set_status(f"Selected renderer: {value}"),
            )
            dg.Selectable(
                "Standalone toggle row",
                selected=True,
                on_select=lambda selected: set_status(f"Standalone selected: {selected}"),
            )

        with dg.Panel("Multiple select", class_="case"):
            dg.Label("Multiple-select rows should toggle independently and preserve item order.", class_="caption")
            dg.SelectableList(
                ["Frame time", "GPU uploads", "Layout pass", "Text atlas", "Draw batches"],
                selection_mode="multiple",
                selected={"Frame time", "Draw batches"},
                class_="list",
                max_height=190,
                on_change=lambda values: set_status("Metrics: " + ", ".join(values)),
            )

    dg.Label("PASS: selected, hover, focus, disabled, single-select, and multi-select states render.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
