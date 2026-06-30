from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


app = dg.App()
app.stylesheet(
    """
    Tooltip {
        background: surface_alt;
        border-color: accent;
        border-radius: 8px;
        padding: 12px;
        gap: 8px;
    }
    """
)

win = dg.Window("DragonGUI Tooltip Demo", width=860, height=520)

with dg.HLayout(style={"padding": 18, "gap": 16}):
    with dg.Panel("Controls", width=320, style={"padding": 14, "gap": 10}):
        inspect = dg.Button("Inspect Dataset", badge=4, tooltip="Static tooltip text still works.")
        export = dg.Button("Export CSV", tooltip="Simple text tooltip")
        normalize = dg.Checkbox(
            "Normalize values",
            checked=True,
            tooltip="Scale each numeric column before previewing results.",
        )
        sensitivity = dg.Slider(0.68, tooltip="Static slider tooltip")

    with dg.Panel("Preview", style={"padding": 14, "gap": 10}):
        chart = dg.ProgressBar(0.72, show_value=True)
        dg.Label("Hover the controls to see static and rich tooltip overlays.")
        dg.TextArea("customer_id,total,status\n1001,219.50,ready\n1002,88.25,queued", rows=6)

with dg.Tooltip(target=inspect, width=280, height=132, style={"gap": 6}):
    dg.Label("Dataset summary", style={"font_weight": 700})
    dg.Label("Rows: 1,240")
    dg.Label("Columns: 18")
    dg.ProgressBar(0.72, show_value=True)

with dg.Tooltip(target=chart, width=300, height=112, style={"gap": 6}):
    dg.Label("Export readiness", style={"font_weight": 700})
    dg.Label("Validation passed for 72% of rows.")
    dg.Button("Tooltip content is display-only", disabled=True)

with dg.Tooltip(target=sensitivity, width=280, height=104, style={"gap": 6}):
    dg.Label("Sensitivity", style={"font_weight": 700})
    dg.Label("Current threshold: 0.68")
    dg.Label("Higher values make filtering stricter.")


if __name__ == "__main__":
    print(app.run(win))
