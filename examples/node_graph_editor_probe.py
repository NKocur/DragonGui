"""NodeGraph editor probe.

A canvas-backed node editor prototype for agent-routing workflows. Drag nodes,
connect pins, add nodes, and delete selected graph items.
"""

from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


NODES = [
    dg.NodeGraphNode(
        "implementer",
        "Implementer",
        20,
        70,
        outputs=(dg.NodeGraphPort("stdout", "stdout"), dg.NodeGraphPort("done", "done")),
        subtitle="codex terminal",
        status="writing",
        color="#43c6ac",
    ),
    dg.NodeGraphNode(
        "parser",
        "Message Parser",
        270,
        40,
        inputs=(dg.NodeGraphPort("raw", "raw"),),
        outputs=(dg.NodeGraphPort("envelope", "envelope"), dg.NodeGraphPort("needs_user", "needs_user")),
        subtitle="@to / @type / @id / @end",
        status="watching",
        color="#7aa2f7",
        width=210,
    ),
    dg.NodeGraphNode(
        "approval",
        "Approval Gate",
        500,
        80,
        inputs=(dg.NodeGraphPort("in", "candidate"),),
        outputs=(dg.NodeGraphPort("approved", "approved"), dg.NodeGraphPort("rejected", "rejected")),
        subtitle="manual or policy checked",
        status="held",
        color="#f8c14a",
        width=210,
    ),
    dg.NodeGraphNode(
        "reviewer",
        "Reviewer",
        760,
        45,
        inputs=(dg.NodeGraphPort("stdin", "stdin"),),
        outputs=(dg.NodeGraphPort("findings", "findings"), dg.NodeGraphPort("tests", "test_request")),
        subtitle="diff and risk review",
        status="idle",
        color="#b48ead",
    ),
    dg.NodeGraphNode(
        "tester",
        "Tester",
        760,
        245,
        inputs=(dg.NodeGraphPort("stdin", "stdin"),),
        outputs=(dg.NodeGraphPort("report", "report"),),
        subtitle="focused checks",
        status="ready",
        color="#ff9f6e",
    ),
    dg.NodeGraphNode(
        "artifacts",
        "Transcript + Artifacts",
        500,
        310,
        inputs=(dg.NodeGraphPort("event", "event"), dg.NodeGraphPort("report", "report")),
        outputs=(dg.NodeGraphPort("export", "export"),),
        subtitle="jsonl, markdown, task state",
        status="recording",
        color="#9ece6a",
        width=230,
    ),
]

EDGES = [
    dg.NodeGraphEdge("implementer", "stdout", "parser", "raw", "stdout"),
    dg.NodeGraphEdge("parser", "envelope", "approval", "in", "candidate"),
    dg.NodeGraphEdge("approval", "approved", "reviewer", "stdin", "review_request"),
    dg.NodeGraphEdge("reviewer", "tests", "tester", "stdin", "test_request", color="#ff9f6e"),
    dg.NodeGraphEdge("reviewer", "findings", "artifacts", "event", "findings", color="#9ece6a"),
    dg.NodeGraphEdge("tester", "report", "artifacts", "report", "test_report", color="#9ece6a"),
    dg.NodeGraphEdge("implementer", "done", "artifacts", "event", "done", color="#9ece6a"),
]


app = dg.App(theme=dg.Theme.dark(accent="#43c6ac", focus="#f8c14a", radius=7))
app.stylesheet(
    """
    Window {
        background: #101318;
        color: rgba(245, 248, 252, 0.94);
        padding: 12px;
        gap: 10px;
        font-size: 13px;
    }

    HLayout.root {
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 0;
        gap: 10px;
    }

    Panel.canvas {
        flex-grow: 1;
        min-width: 0;
        min-height: 0;
        padding: 10px;
        gap: 8px;
    }

    HtmlReport.node-graph {
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 8px;
        background: #0d1117;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.muted {
        color: rgba(245, 248, 252, 0.64);
        line-height: 1.16;
    }

    Label.status {
        width: 100%;
        background: rgba(67, 198, 172, 0.12);
        border: 1px solid rgba(67, 198, 172, 0.34);
        border-radius: 6px;
        color: rgba(226, 255, 248, 0.96);
        font-weight: 750;
        padding: 7px 9px;
    }

    LogView {
        width: 100%;
        flex-grow: 1;
        min-height: 0;
        background: #070a0f;
        border: 1px solid rgba(255, 255, 255, 0.11);
        border-radius: 6px;
        color: rgba(232, 239, 248, 0.90);
        font-family: "Consolas";
        font-size: 12px;
        line-height: 18px;
        padding: 8px 10px;
    }
    """
)

win = dg.Window("NodeGraph Editor Probe", width=1280, height=760)
status_label: dg.Label | None = None
selection_label: dg.Label | None = None
event_log: dg.LogView | None = None


def on_select(node_id: str | None) -> None:
    if selection_label is not None:
        selection_label.set_value(f"Selected: {node_id or 'none'}")
    if event_log is not None:
        event_log.append_line(f"select {node_id or 'none'}")


def on_move(node_id: str, x: float, y: float) -> None:
    if status_label is not None:
        status_label.set_value(f"Moved {node_id} to {x:.0f}, {y:.0f}")


with dg.VLayout(class_="root"):
    with dg.Panel("Node Canvas", class_="canvas"):
        with dg.FlowLayout(gap=8, row_gap=6, style={"width": "100%", "height": "auto", "flex_shrink": 0}):
            dg.Label("NodeGraph", class_="title")
            selection_label = dg.Label("Selected: none", class_="status", style={"width": 180})
            status_label = dg.Label("Ready", class_="status", style={"width": 220})
            dg.Tag("drag headers", level="success")
            dg.Tag("drag pins to connect", level="neutral")
            dg.Tag("dbl-click add", level="warning")
            dg.Tag("Delete removes", level="neutral")
            dg.Tag("Ctrl+D duplicates", level="neutral")
        dg.NodeGraph(NODES, EDGES, on_node_select=on_select, on_node_move=on_move, enable_zoom=True, show_port_labels=True, show_subtitles=False, width=1060, height=610, class_="node-graph")

if __name__ == "__main__":
    print(app.run(win))








