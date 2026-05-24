from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=7, focus="#ffd166"))
app.stylesheet(
    """
    Window {
        background: #10141b;
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

    HLayout.content {
        width: 100%;
        flex-grow: 1;
        flex-shrink: 1;
        min-height: 0;
        gap: 12px;
    }

    Panel.case {
        width: calc(50% - 6px);
        height: 100%;
        min-height: 0;
        min-width: 360px;
        background: rgba(22, 31, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        padding: 14px;
        gap: 10px;
    }

    ScrollArea.property-scroll {
        width: 100%;
        flex-grow: 1;
        flex-shrink: 1;
        min-height: 0;
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
        background: rgba(90, 169, 255, 0.12);
        border: 1px solid rgba(90, 169, 255, 0.34);
        border-radius: 8px;
        color: rgba(232, 244, 255, 0.96);
        font-weight: 750;
        padding: 8px 10px;
        width: 100%;
    }

    VLayout.property-grid {
        gap: 6px;
        width: 100%;
    }

    HLayout.property-row {
        min-height: 32px;
        gap: 10px;
        width: 100%;
    }

    Label.property-label {
        color: rgba(246, 249, 255, 0.70);
        font-weight: 650;
    }

    HLayout.property-editor {
        gap: 8px;
        flex: 1;
        min-width: 0;
    }

    Collapsible.property-section {
        width: 100%;
        background: rgba(255, 255, 255, 0.035);
        border: 1px solid rgba(255, 255, 255, 0.09);
        border-radius: 8px;
        padding: 8px;
        gap: 6px;
    }

    Collapsible.property-section::header {
        color: white;
        font-weight: 800;
    }

    TextInput,
    Dropdown,
    DragNumber {
        width: 100%;
    }

    RangeSlider {
        width: 100%;
        height: 32px;
    }
    """
)

win = dg.Window("PropertyGrid probe", width=920, height=560)

values = {
    "Name": "Sensor A",
    "Enabled": True,
    "Mode": "Auto",
    "Gain": 0.25,
    "Window": (20, 80),
    "Color": "#66ccff",
    "Notes": "Mounted on test stand",
}

schema = {
    "Mode": {"type": "select", "options": ["Auto", "Manual", "Disabled"]},
    "Gain": {"type": "float", "min": 0.0, "max": 1.0, "step": 0.01},
    "Window": {"type": "range", "min": 0, "max": 100, "step": 5},
    "Color": {"type": "color"},
    "Notes": {"type": "multiline", "rows": 3},
}

with dg.VLayout(class_="root"):
    dg.Label("PropertyGrid", class_="title")
    status = dg.Label("Ready", class_="status")

    def on_property_change(change: dg.PropertyChange) -> None:
        status.set_value(f"{change.key}: {change.old_value!r} -> {change.value!r}")

    with dg.HLayout(class_="content"):
        with dg.Panel("Schema generated", class_="case"):
            dg.Label("Rows are generated from a dict plus optional schema and sections.", class_="caption")
            with dg.ScrollArea(axis="y", class_="property-scroll"):
                grid = dg.PropertyGrid(
                    values,
                    schema=schema,
                    sections={
                        "Device": ["Name", "Enabled", "Mode"],
                        "Tuning": ["Gain", "Window", "Color"],
                        "Metadata": ["Notes"],
                    },
                    on_change=on_property_change,
                    label_width=112,
                )

            def normalize_gain() -> None:
                grid.set_value("Gain", 0.5, notify=True)

            dg.SmallButton("Normalize gain", on_click=normalize_gain)

        with dg.Panel("Manual rows", class_="case"):
            dg.Label("Manual rows accept any editor widget while preserving label alignment.", class_="caption")
            with dg.PropertyGrid(label_width=118):
                dg.Property(
                    "Exposure",
                    dg.DragNumber(
                        12.5,
                        min=0,
                        max=30,
                        step=0.5,
                        on_change=lambda value: status.set_value(f"Exposure: {value:.1f} ms"),
                    ),
                )
                dg.Property(
                    "Threshold",
                    dg.RangeSlider(
                        (0.2, 0.8),
                        min=0,
                        max=1,
                        step=0.05,
                        on_change=lambda value: status.set_value(
                            f"Threshold: {value[0]:.2f} - {value[1]:.2f}"
                        ),
                    ),
                )
                with dg.Property("Actions"):
                    dg.IconButton("play", tooltip="Run", on_click=lambda: status.set_value("Action: run"))
                    dg.IconButton("save", tooltip="Save", on_click=lambda: status.set_value("Action: save"))
                    dg.SmallButton("Reset", on_click=lambda: status.set_value("Action: reset"))

    dg.Label("PASS: generated sections, aligned manual rows, editors, programmatic set_value, and change payloads work.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
