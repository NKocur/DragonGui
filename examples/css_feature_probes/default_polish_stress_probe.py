from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App()
app.stylesheet(
    """
    @media (max-width: 420px) {
        .default-shell {
            flex-direction: column;
        }

        Sidebar.default-sidebar {
            width: 100%;
            height: 208px;
            flex-shrink: 0;
        }
    }
    """
)

rows = [
    {
        "route": f"plant.zone-{i % 4}.cell-{i % 7}.capture-stream",
        "owner": ["ingest", "model", "export", "review"][i % 4],
        "latency_ms": round(2.1 + i * 1.37, 2),
        "state": ["ready", "queued", "running", "blocked"][i % 4],
    }
    for i in range(36)
]

properties = {
    "Route": "plant.zone-3.cell-2.pipeline.long-property",
    "Enabled": True,
    "Mode": "Auto review",
    "Threshold": 0.82,
    "Owner": "Workbench operations",
}

schema = {
    "Mode": {"type": "choice", "options": ["Auto review", "Manual hold", "Diagnostics"]},
    "Threshold": {"type": "number", "min": 0, "max": 1, "step": 0.01},
}


with dg.Window(title="Default Polish Stress", width=1180, height=760) as win:
    with dg.AppShell(class_="default-shell"):
        with dg.Sidebar(title="Audit", width=132, class_="default-sidebar"):
            dg.Label("Default UI")
            dg.NavItem("Overview", page="overview", badge="12")
            dg.NavItem("Long route analytics channel", page="analytics", badge="7")
            dg.NavItem("Reports and exports", page="reports")
            dg.NavItem("Disabled destination", page="disabled", disabled=True)
            dg.Separator()
            dg.Badge("online", level="success")

        with dg.WorkbenchLayout(gap=6, padding=8):
            with dg.Toolbar():
                dg.IconButton("play", tooltip="Run")
                dg.IconButton("pause", tooltip="Pause")
                dg.ToolbarSeparator()
                dg.SmallButton("Dep")
                dg.SmallButton("Run")

            with dg.Body(scroll="y", gap=8, style={"height": 0}):
                dg.TextInput("filter routes, owners, status", style={"width": "100%"})
                dg.Label(
                    "Default polish stress: long labels, dense cards, tables, tabs, navigation, and nested scroll areas"
                )
                with dg.FlowLayout(gap=8, row_gap=8, style={"width": "100%"}):
                    for label, value, detail in [
                        ("Open incidents", "12", "Queue pressure and owner handoff"),
                        ("Long-running route", "plant-zone-west", "Should wrap or ellipsize without overlap"),
                        ("Owner", "Workbench operations", "Summary text stays inside the card"),
                    ]:
                        with dg.Panel(style={"width": 170, "flex_shrink": 0}):
                            dg.Label(label)
                            dg.Label(value, style={"font_weight": 800})
                            dg.Label(detail)

                with dg.Tabs(value="summary"):
                    with dg.Tab("Summary", value="summary", badge="4"):
                        dg.Label("Selected tab content remains compact.")
                    with dg.Tab("Extremely long queue label", value="queue", badge="18"):
                        dg.Label("Long tab labels should keep badges contained.")
                    with dg.Tab("Disabled destination", value="disabled", disabled=True):
                        dg.Label("Disabled tab.")

                with dg.FlowLayout(gap=8, row_gap=8, style={"width": "100%"}):
                    with dg.Panel("Table", style={"width": 170, "flex_shrink": 0, "height": 230}):
                        dg.DataFrameTable(rows, page_size=36, sample_rows=36, style={"width": "100%", "height": 178})
                    with dg.Panel("Properties", style={"width": 170, "flex_shrink": 0, "height": 230, "overflow_y": "auto"}):
                        dg.PropertyGrid(properties, schema=schema, label_width=92)

                with dg.Panel("Nested scroll and long text", style={"height": 220, "overflow_y": "auto"}):
                    for index in range(1, 9):
                        with dg.Panel(style={"width": "100%"}):
                            dg.Label(f"Workbench row {index}")
                            dg.Label(
                                "This default-only row keeps long body copy visible inside nested scroll ownership "
                                "without requiring application CSS."
                            )

            with dg.StatusBar(height=28):
                dg.Badge("ok", level="success")
                dg.Badge("theme")


if __name__ == "__main__":
    print(app.run(win))
