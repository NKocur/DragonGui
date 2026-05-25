from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

from probe_helpers import probe_app, probe_grid, probe_header


class WideFrame:
    columns = (
        "index_key",
        "route",
        "owner",
        "state",
        "latency_p50_ms",
        "latency_p95_ms",
        "queue_depth",
        "enabled",
    )
    dtypes = ("str", "str", "str", "str", "float64", "float64", "int64", "bool")

    def __init__(self, rows: int = 64) -> None:
        self.shape = (rows, len(self.columns))
        self.index_key = [f"sample-{idx:04d}" for idx in range(rows)]
        self.route = [
            f"plant.zone-{idx % 7}.cell-{idx % 5}.stream-primary-long-name-{idx:02d}"
            for idx in range(rows)
        ]
        self.owner = [("ingest", "model", "export", "review")[idx % 4] for idx in range(rows)]
        self.state = [("ready", "active", "queued", "blocked", "backfill")[idx % 5] for idx in range(rows)]
        self.latency_p50_ms = [round(3.1 + (idx * 1.73) % 24.0, 2) for idx in range(rows)]
        self.latency_p95_ms = [round(11.0 + (idx * 3.21) % 77.0, 2) for idx in range(rows)]
        self.queue_depth = [(idx * 17) % 240 for idx in range(rows)]
        self.enabled = [idx % 6 != 2 for idx in range(rows)]

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


CODE_SAMPLE = """from __future__ import annotations

def route_packet(packet, registry, telemetry_writer):
    trace = registry.lookup(packet.source, packet.destination, packet.priority)
    telemetry_writer.emit("routing.trace", {"trace_id": packet.trace_id, "source": packet.source, "destination": packet.destination, "priority": packet.priority, "route": trace.name})
    if trace.requires_review and packet.retry_count > 3:
        return "manual-review-required-for-very-long-routing-state"
    return trace.next_hop

for packet in incoming_packets:
    route_packet(packet, registry, telemetry_writer)
"""


LOG_LINES = [
    f"{level} scroll-composite event={idx:03d} route=plant.zone-{idx % 7}.cell-{idx % 5}.pipeline payload=alpha.beta.gamma.delta.{idx:03d}"
    for idx, level in enumerate(("INFO", "DEBUG", "WARN", "INFO", "ERROR", "INFO") * 8, start=1)
]


app, win = probe_app("Layout Scrollable Composites Probe", width=980, height=740)
app.stylesheet(
    """
    Window {
        background: #0f141c;
        color: rgba(245, 248, 255, 0.94);
        padding: 16px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        overflow-y: auto;
        overflow-x: hidden;
        padding-right: 24px;
        padding-bottom: 34px;
        gap: 12px;
    }

    VLayout.root::scrollbar-track,
    ScrollArea::scrollbar-track,
    DataFrameTable::scrollbar-track,
    CodeEditor::scrollbar-track,
    LogView::scrollbar-track,
    SelectableList::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb,
    ScrollArea::scrollbar-thumb,
    DataFrameTable::scrollbar-thumb,
    CodeEditor::scrollbar-thumb,
    LogView::scrollbar-thumb,
    SelectableList::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.72);
        border-radius: 999px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(245, 248, 255, 0.70);
        line-height: 1.12;
    }

    Label.status {
        width: 100%;
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 8px;
        color: rgba(229, 255, 244, 0.96);
        font-weight: 800;
        padding: 8px 10px;
    }

    GridLayout.grid {
        width: 100%;
        height: auto;
        gap: 12px;
        row-gap: 12px;
    }

    Panel.case {
        width: 100%;
        height: 352px;
        min-width: 0;
        min-height: 0;
        background: rgba(22, 30, 41, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 10px;
        padding: 12px;
        gap: 10px;
    }

    Panel.short {
        height: 310px;
    }

    ScrollArea.fill-scroll {
        width: 100%;
        flex: 1;
        min-height: 0;
        padding-right: 22px;
        padding-bottom: 20px;
        gap: 8px;
    }

    ScrollArea.both-axis {
        overflow-x: auto;
        overflow-y: auto;
    }

    HLayout.wide-row {
        width: 760px;
        flex-shrink: 0;
        min-height: 34px;
        gap: 8px;
        align-items: center;
    }

    HLayout.field-row,
    HLayout.command-row {
        width: 100%;
        min-width: 0;
        min-height: 34px;
        gap: 8px;
        align-items: center;
    }

    Label.field-label {
        width: 108px;
        flex-shrink: 0;
        color: rgba(245, 248, 255, 0.70);
        font-weight: 750;
    }

    TextInput.fill,
    Dropdown.fill,
    DragNumber.fill {
        flex: 1;
        min-width: 0;
        height: 34px;
    }

    TextArea.fill {
        width: 100%;
        flex: 1;
        min-height: 72px;
    }

    Button,
    SmallButton {
        flex-shrink: 0;
    }

    Button.shrink,
    SmallButton.shrink {
        min-width: 0;
        flex-shrink: 1;
    }

    DataFrameTable.wide-table {
        width: 100%;
        flex: 1;
        min-height: 0;
        background: rgba(5, 9, 14, 0.72);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 8px;
        color: rgba(230, 238, 248, 0.90);
        font-size: 12px;
        table-row-height: 26px;
        table-header-height: 34px;
        table-column-width: 150px;
        table-index-width: 56px;
    }

    DataFrameTable.wide-table::header {
        background: rgba(90, 169, 255, 0.18);
        color: white;
        font-weight: 850;
    }

    DataFrameTable.wide-table::grid-line {
        background: rgba(255, 255, 255, 0.12);
    }

    CodeEditor.code,
    LogView.log {
        width: 100%;
        flex: 1;
        min-height: 0;
        background: rgba(5, 9, 14, 0.72);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 8px;
        color: rgba(230, 238, 248, 0.90);
        font-family: "Consolas";
        font-size: 12px;
        line-height: 18px;
        padding: 8px 10px;
    }

    CodeEditor.code::gutter {
        width: 50px;
        background: rgba(255, 255, 255, 0.055);
        border-color: rgba(255, 255, 255, 0.13);
    }

    CodeEditor.code::line-number {
        color: rgba(245, 248, 255, 0.44);
        font-family: "Consolas";
        font-size: 12px;
        font-variant-numeric: tabular-nums;
    }

    TreeView.compact-tree,
    VLayout.command-results,
    SelectableList.metric-list {
        width: 100%;
        min-width: 0;
        gap: 3px;
    }

    TreeNode,
    Selectable {
        min-height: 30px;
        border-radius: 6px;
        color: rgba(245, 248, 255, 0.86);
    }

    TreeNode:hover,
    Selectable:hover {
        background: rgba(90, 169, 255, 0.10);
        border-color: rgba(90, 169, 255, 0.26);
    }

    TreeNode:selected,
    Selectable:selected {
        background: rgba(90, 169, 255, 0.18);
        border-color: rgba(90, 169, 255, 0.62);
        color: white;
    }

    TreeNode::indicator,
    Selectable::indicator {
        width: 4px;
        height: 16px;
        background: #5aa9ff;
        border-radius: 999px;
    }

    PropertyGrid,
    VLayout.property-grid {
        width: 100%;
        min-width: 0;
        gap: 6px;
    }

    HLayout.property-row {
        width: 100%;
        min-width: 0;
        min-height: 32px;
        gap: 8px;
    }

    Label.property-label {
        color: rgba(245, 248, 255, 0.70);
        font-weight: 750;
    }

    HLayout.property-editor {
        flex: 1;
        min-width: 0;
        gap: 8px;
    }

    Collapsible.property-section {
        width: 100%;
        min-width: 0;
        background: rgba(255, 255, 255, 0.035);
        border: 1px solid rgba(255, 255, 255, 0.09);
        border-radius: 8px;
        padding: 8px;
        gap: 6px;
    }

    HLayout.search-box {
        height: 38px;
        flex-shrink: 0;
        background: rgba(255, 255, 255, 0.055);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 8px;
        padding: 4px;
    }

    HLayout.search-box TextInput {
        min-width: 0;
        background: transparent;
        border-color: transparent;
        height: 28px;
        padding-left: 4px;
        padding-right: 4px;
    }

    IconButton.search-box-icon,
    IconButton.search-box-clear {
        width: 28px;
        height: 28px;
        border-radius: 6px;
        padding: 5px;
        background: transparent;
        border-color: transparent;
    }
    """
)

frame = WideFrame()
properties = {
    "Route": "plant.zone-3.cell-2.pipeline.long-property-name",
    "Enabled": True,
    "Mode": "Auto review",
    "Gain": 0.42,
    "Window": (12, 86),
    "Notes": "Long multiline notes should stay inside the property editor and scroll with the property region.",
}
schema = {
    "Mode": {"type": "select", "options": ["Auto review", "Manual hold", "Disabled"]},
    "Gain": {"type": "float", "min": 0.0, "max": 1.0, "step": 0.01},
    "Window": {"type": "range", "min": 0, "max": 100, "step": 2},
    "Notes": {"type": "multiline", "rows": 4},
}
command_items = [
    ("Open route diagnostics", "open-route"),
    ("Export selected table rows", "export"),
    ("Toggle code wrapping", "wrap"),
    ("Clear live log buffer", "clear-log"),
    ("Reset nested scroll offsets", "reset-scroll"),
    ("Show long command result that should clip cleanly", "long-command"),
    ("Disabled command result", "disabled"),
]


def mark(message: str) -> None:
    status.set_value(message)


with dg.VLayout(class_="root"):
    probe_header(
        "Scrollable composites",
        "Scroll each fixed card. Tables, text widgets, lists, property rows, and nested scroll areas should keep their headers, gutters, and text anchored to their own viewport.",
    )
    status = dg.Label("Ready: scroll owners are intentionally nested and constrained.", class_="status")

    with probe_grid(gap=12, row_gap=12, min_column_width=390):
        with dg.Panel("Wide table in a fixed card", class_="case"):
            dg.Label("Rows and columns should scroll inside the table, not push the panel wider.", class_="caption")
            dg.DataFrameTable(
                frame,
                page_size=64,
                sample_rows=64,
                sortable=True,
                resizable_columns=True,
                on_select=lambda selection: mark(
                    f"Table row {selection.row_index}: {selection.column} = {selection.value}"
                ),
                on_sort=lambda sort: mark(
                    f"Sorted {'index' if sort.is_index else sort.column} {sort.direction}"
                ),
                class_="wide-table",
            )

        with dg.Panel("Log and code scroll owners", class_="case"):
            dg.Label("The log and code gutter should stay clipped to this card while the card height stays fixed.", class_="caption")
            with dg.HLayout(style={"width": "100%", "flex": 1, "min_height": 0, "gap": 8}):
                dg.LogView(LOG_LINES, follow=False, rows=8, wrap=False, class_="log")
                dg.CodeEditor(CODE_SAMPLE, language="python", rows=8, wrap=False, class_="code")

        with dg.Panel("Tree and selectable list", class_="case short"):
            dg.Label("Two scrollable list widgets share one card without either bleeding into the other.", class_="caption")
            with dg.HLayout(style={"width": "100%", "flex": 1, "min_height": 0, "gap": 10}):
                with dg.ScrollArea(axis="y", class_="fill-scroll"):
                    with dg.TreeView(class_="compact-tree", on_select=lambda node_id: mark(f"Tree: {node_id}")):
                        with dg.TreeNode("Plant", node_id="plant", expanded=True):
                            for zone in range(1, 8):
                                with dg.TreeNode(f"Zone {zone}", node_id=f"zone-{zone}", expanded=zone <= 2):
                                    for cell in range(1, 5):
                                        dg.TreeNode(f"Cell {cell} / capture stream", node_id=f"zone-{zone}/cell-{cell}", leaf=True)
                with dg.ScrollArea(axis="y", class_="fill-scroll"):
                    dg.SelectableList(
                        [f"Metric channel {idx:02d} / long scroll label" for idx in range(1, 25)],
                        selection_mode="multiple",
                        selected={"Metric channel 01 / long scroll label", "Metric channel 07 / long scroll label"},
                        class_="metric-list",
                        on_change=lambda values: mark(f"Metrics selected: {len(values)}"),
                    )

        with dg.Panel("Property grid in a scroll body", class_="case short"):
            dg.Label("Generated property editors should shrink to the scroll viewport width.", class_="caption")
            with dg.ScrollArea(axis="y", class_="fill-scroll"):
                dg.PropertyGrid(
                    properties,
                    schema=schema,
                    sections={
                        "Routing": ["Route", "Enabled", "Mode"],
                        "Tuning": ["Gain", "Window"],
                        "Notes": ["Notes"],
                    },
                    label_width=118,
                    on_change=lambda change: mark(f"{change.key}: {change.value!r}"),
                )
                for index in range(1, 8):
                    with dg.HLayout(class_="field-row"):
                        dg.Label(f"Extra {index}", class_="field-label")
                        dg.TextInput(f"additional.scroll.property.{index}.long-value", class_="fill")
                        dg.Badge("ok", level="success")

        with dg.Panel("Command results and horizontal scroll", class_="case"):
            dg.Label("Search results scroll vertically; the wide rows below expose horizontal scrollbars.", class_="caption")
            filter_state = {"query": "", "selected": "open-route"}

            def refresh_commands() -> None:
                query = filter_state["query"].strip().lower()
                rows = []
                for label, value in command_items:
                    if query and query not in label.lower() and query not in value:
                        continue

                    def select_command(selected: bool, value: str = value) -> None:
                        if not selected:
                            return
                        filter_state["selected"] = value
                        mark(f"Command: {value}")
                        refresh_commands()

                    rows.append(
                        dg.Selectable(
                            label,
                            value=value,
                            selected=value == filter_state["selected"],
                            disabled=value == "disabled",
                            toggle=False,
                            on_select=select_command,
                            parent=None,
                        )
                    )
                if not rows:
                    rows.append(dg.Label("No command rows match the filter.", class_="caption", parent=None))
                if command_results.is_live:
                    command_results.replace_children(rows)
                    return
                command_results.children = []
                for row in rows:
                    command_results.add(row)

            def filter_commands(value: str) -> None:
                filter_state["query"] = value
                mark(f"Filter: {value or 'all'}")
                refresh_commands()

            dg.SearchBox(
                placeholder="Filter command rows...",
                on_change=filter_commands,
            )
            with dg.ScrollArea(axis="y", class_="fill-scroll"):
                command_results = dg.VLayout(class_="command-results")
                refresh_commands()
            with dg.ScrollArea(axis="both", class_="fill-scroll both-axis"):
                for index in range(1, 7):
                    with dg.HLayout(class_="wide-row"):
                        dg.Label(f"Wide row {index:02d}", class_="field-label")
                        dg.TextInput(f"horizontal.overflow.payload.{index}.alpha.beta.gamma.delta.epsilon", class_="fill")
                        dg.Tag("overflow-x", level="info")
                        dg.SmallButton("Act", on_click=lambda index=index: mark(f"Wide row {index}"))

        with dg.Panel("Nested scroll body", class_="case"):
            dg.Label("The outer scroll area contains fixed inner scroll areas; wheel and clipping should remain local.", class_="caption")
            with dg.ScrollArea(axis="y", class_="fill-scroll"):
                for section in range(1, 5):
                    with dg.Panel(f"Inner scroller {section}", class_="short", style={"height": 162, "width": "100%", "min_width": 0}):
                        with dg.ScrollArea(axis="y", class_="fill-scroll"):
                            for row in range(1, 10):
                                with dg.HLayout(class_="field-row"):
                                    dg.Label(f"S{section}.{row:02d}", class_="field-label")
                                    dg.TextInput(f"nested.scroll.owner.{section}.{row}.value", class_="fill")
                                    dg.Badge("local", level="info")

    dg.Label(
        "PASS TARGET: scrollbars are visible and usable, headers/gutters stay anchored, and every scrolled child clips to its owning region.",
        class_="caption",
    )


if __name__ == "__main__":
    print(app.run(win))
