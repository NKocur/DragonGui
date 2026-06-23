"""NodeGraph editor probe.

Exercises the canvas-backed node editor: templates, typed ports, validation,
events, history, navigation, persistence, and the Python-side agent models.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


TEMPLATES = dg.multi_agent_node_templates()

NODES = [
    dg.NodeGraphNode(
        "implementer_terminal",
        "Implementer Terminal",
        20,
        90,
        inputs=(dg.NodeGraphPort("stdin", "terminal_input", port_type="terminal_input"),),
        outputs=(
            dg.NodeGraphPort("stdout", "terminal_output", port_type="terminal_output"),
            dg.NodeGraphPort("error", "error", port_type="error"),
        ),
        subtitle="PTY transcript source",
        status="running",
        color="#43c6ac",
        width=230,
        data={"node_type": "terminal", "template_id": "terminal", "default_status": "running"},
    ),
    dg.NodeGraphNode(
        "parser",
        "Envelope Parser",
        310,
        80,
        inputs=(dg.NodeGraphPort("in", "terminal_output", port_type="terminal_output"),),
        outputs=(
            dg.NodeGraphPort("message", "message", port_type="message"),
            dg.NodeGraphPort("error", "error", port_type="error"),
        ),
        subtitle="@to / @type / @id / @end",
        status="watching",
        color="#9ece6a",
        width=220,
        data={"node_type": "parser", "template_id": "parser", "default_status": "idle"},
    ),
    dg.NodeGraphNode(
        "reviewer_agent",
        "Reviewer Agent",
        610,
        40,
        inputs=(
            dg.NodeGraphPort("in", "message", port_type="message"),
            dg.NodeGraphPort("approval", "approval_result", port_type="approval_result"),
        ),
        outputs=(
            dg.NodeGraphPort("out", "message", port_type="message"),
            dg.NodeGraphPort("approval_request", "approval_request", port_type="approval_request"),
            dg.NodeGraphPort("test_request", "test_request", port_type="test_request"),
            dg.NodeGraphPort("artifact", "artifact", port_type="artifact"),
            dg.NodeGraphPort("error", "error", port_type="error"),
        ),
        subtitle="review and route",
        status="idle",
        color="#7aa2f7",
        width=235,
        data={
            "node_type": "agent",
            "template_id": "agent",
            "default_status": "idle",
            "session": {"agent_type": "codex", "capabilities": {"terminal": True}},
        },
    ),
    dg.NodeGraphNode(
        "approval",
        "Approval Gate",
        910,
        30,
        inputs=(dg.NodeGraphPort("request", "approval_request", port_type="approval_request"),),
        outputs=(
            dg.NodeGraphPort("result", "approval_result", port_type="approval_result"),
            dg.NodeGraphPort("error", "error", port_type="error"),
        ),
        subtitle="human checkpoint",
        status="waiting",
        color="#e0af68",
        width=220,
        data={"node_type": "approval_gate", "template_id": "approval_gate"},
    ),
    dg.NodeGraphNode(
        "tester",
        "Tester",
        910,
        245,
        inputs=(dg.NodeGraphPort("request", "test_request", port_type="test_request"),),
        outputs=(
            dg.NodeGraphPort("report", "test_report", port_type="test_report"),
            dg.NodeGraphPort("error", "error", port_type="error"),
        ),
        subtitle="focused checks",
        status="ready",
        color="#bb9af7",
        width=210,
        data={"node_type": "tester", "template_id": "tester"},
    ),
    dg.NodeGraphNode(
        "artifacts",
        "Artifacts",
        610,
        335,
        inputs=(dg.NodeGraphPort("in", "artifact", port_type="artifact"),),
        outputs=(dg.NodeGraphPort("out", "artifact", port_type="artifact"),),
        subtitle="snapshots and reports",
        status="recording",
        color="#f7768e",
        width=220,
        data={"node_type": "artifact", "template_id": "artifact"},
    ),
]

EDGES = [
    dg.NodeGraphEdge(
        "implementer_terminal",
        "stdout",
        "parser",
        "in",
        label="terminal_output",
        color="#9ece6a",
        id="edge-terminal-parser",
    ),
    dg.NodeGraphEdge(
        "parser",
        "message",
        "reviewer_agent",
        "in",
        label="message",
        color="#7aa2f7",
        id="edge-parser-reviewer",
    ),
    dg.NodeGraphEdge(
        "reviewer_agent",
        "approval_request",
        "approval",
        "request",
        label="approval_request",
        color="#e0af68",
        id="edge-reviewer-approval",
    ),
    dg.NodeGraphEdge(
        "approval",
        "result",
        "reviewer_agent",
        "approval",
        label="approval_result",
        color="#e0af68",
        id="edge-approval-reviewer",
    ),
    dg.NodeGraphEdge(
        "reviewer_agent",
        "test_request",
        "tester",
        "request",
        label="test_request",
        color="#bb9af7",
        id="edge-reviewer-tester",
    ),
    dg.NodeGraphEdge(
        "reviewer_agent",
        "artifact",
        "artifacts",
        "in",
        label="artifact",
        color="#f7768e",
        id="edge-reviewer-artifacts",
    ),
]

SAVED_GRAPH_DATA = dg.NodeGraph(NODES, EDGES, templates=TEMPLATES, parent=None).to_graph_data()
LOADED_GRAPH = dg.NodeGraph.from_graph_data(SAVED_GRAPH_DATA, templates=TEMPLATES, parent=None)


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

    Panel.side {
        width: 360px;
        min-width: 320px;
        height: 100%;
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

    Label.section {
        color: rgba(245, 248, 252, 0.88);
        font-weight: 800;
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

    Button {
        padding: 7px 9px;
    }
    """
)

win = dg.Window("NodeGraph Editor Probe", width=1480, height=820)

graph: dg.NodeGraph | None = None
status_label: dg.Label | None = None
selection_label: dg.Label | None = None
history_label: dg.Label | None = None
nav_label: dg.Label | None = None
counts_label: dg.Label | None = None
event_log: dg.LogView | None = None
selected_node_id: str | None = None


def log(line: object = "") -> None:
    if event_log is not None:
        event_log.append_line(line)


def refresh_state() -> None:
    if graph is None:
        return
    data = graph.to_graph_data()
    history = graph.history_state()
    nav = graph.navigation_state()
    if counts_label is not None:
        counts_label.set_value(f"Graph: {len(data['nodes'])} nodes / {len(data['edges'])} edges")
    if history_label is not None:
        history_label.set_value(
            "History: "
            f"undo={history['undo_depth']} redo={history['redo_depth']} dirty={history['dirty']}"
        )
    if nav_label is not None:
        nav_label.set_value(
            f"Viewport: x={nav['x']:.1f} y={nav['y']:.1f} zoom={nav['zoom']:.2f}"
        )


def on_graph_event(payload: dict[str, object]) -> None:
    global selected_node_id
    event = str(payload.get("event", ""))
    if event == "node_selected":
        node = payload.get("node")
        selected_node_id = None if node is None else str(node)
        if selection_label is not None:
            selection_label.set_value(f"Selected: {selected_node_id or 'none'}")
    elif event in {"selection_cleared", "edge_selected"}:
        selected_node_id = None
        if selection_label is not None:
            selection_label.set_value(
                f"Selected: edge {payload.get('edge')}" if event == "edge_selected" else "Selected: none"
            )

    if status_label is not None:
        if event == "connection_rejected":
            status_label.set_value(f"Rejected: {payload.get('reason')}")
        elif event == "viewport_changed":
            status_label.set_value(f"Navigation: {payload.get('action', 'viewport')}")
        else:
            status_label.set_value(f"Event: {event}")

    interesting = {
        key: payload[key]
        for key in ("event", "node", "edge", "reason", "action", "history", "viewport")
        if key in payload
    }
    log(json.dumps(interesting, sort_keys=True))
    refresh_state()


def on_select(node_id: str | None) -> None:
    log(f"legacy on_node_select {node_id or 'none'}")


def on_move(node_id: str, x: float, y: float) -> None:
    log(f"legacy on_node_move {node_id} {x:.0f},{y:.0f}")


def rename_selected() -> None:
    if graph is None:
        return
    target = selected_node_id or (graph.to_graph_data()["nodes"][0]["id"] if graph.to_graph_data()["nodes"] else None)
    if target is None:
        log("rename skipped: no node")
        return
    graph.update_node(target, title=f"{target} updated", status="edited", color="#2ac3de", notify=True)
    refresh_state()


def undo_graph() -> None:
    if graph is not None:
        log(f"python undo -> {graph.undo(notify=True)}")
        refresh_state()


def redo_graph() -> None:
    if graph is not None:
        log(f"python redo -> {graph.redo(notify=True)}")
        refresh_state()


def request_fit() -> None:
    if graph is not None:
        log(f"python fit request -> {graph.fit_to_view()}")
        refresh_state()


def log_snapshot() -> None:
    if graph is None:
        return
    data = graph.to_graph_data()
    log(f"snapshot schema={data['schema_version']} nodes={len(data['nodes'])} edges={len(data['edges'])}")
    log(json.dumps(graph.history_state(), sort_keys=True))


def run_model_smoke() -> None:
    parser = dg.AgentEnvelopeParser()
    messages = parser.feed(
        "@to reviewer\n"
        "@from implementer\n"
        "@type review_request\n"
        "@id DG-probe-1\n"
        "@priority high\n"
        "Please inspect the NodeGraph probe.\n"
        "@end\n"
    )
    queue = dg.AgentRouterQueue()
    for message in messages:
        queue.enqueue(message, hold=True, reason="operator approval")
    session = dg.AgentSession("session-probe", "reviewer_agent", "codex", command=("codex", "--ask-for-approval"))
    session.apply_terminal_event(dg.TerminalEvent("bridge_started", timestamp=1.0))
    session.apply_terminal_event(dg.TerminalEvent("session_started", session_id=7, timestamp=2.0))
    session.apply_terminal_event(dg.TerminalEvent("output", session_id=7, data="ready", timestamp=3.0))
    log("model smoke:")
    log(json.dumps(parser.drain_events(), sort_keys=True))
    log(json.dumps(queue.snapshot()["items"], sort_keys=True))
    log(json.dumps(session.snapshot()["record"], sort_keys=True))


with dg.HLayout(class_="root"):
    with dg.Panel("Node Canvas", class_="canvas"):
        with dg.FlowLayout(gap=8, row_gap=6, style={"width": "100%", "height": "auto", "flex_shrink": 0}):
            dg.Label("NodeGraph", class_="title")
            selection_label = dg.Label("Selected: none", class_="status", style={"width": 190})
            status_label = dg.Label("Ready", class_="status", style={"width": 280})
            counts_label = dg.Label("Graph: loading", class_="status", style={"width": 210})
            dg.Tag("typed ports", level="success")
            dg.Tag("palette templates", level="success")
            dg.Tag("undo/redo", level="neutral")
            dg.Tag("fit/zoom/grid/minimap", level="neutral")
            dg.Tag("invalid links reject", level="warning")
        graph = dg.NodeGraph(
            LOADED_GRAPH.nodes,
            LOADED_GRAPH.edges,
            templates=TEMPLATES,
            on_graph_event=on_graph_event,
            on_node_select=on_select,
            on_node_move=on_move,
            enable_zoom=True,
            show_port_labels=True,
            show_subtitles=True,
            width=1080,
            height=660,
            class_="node-graph",
        )

    with dg.Panel("Probe Controls", class_="side"):
        dg.Label("Canvas Checks", class_="section")
        dg.Label(
            "Use the palette at top-left, double-click to create, drag typed pins, press Enter/F2 "
            "to rename, Ctrl+Z/Ctrl+Y for history, F to fit, +/- to zoom, G for grid.",
            class_="muted",
        )
        history_label = dg.Label("History: loading", class_="status")
        nav_label = dg.Label("Viewport: loading", class_="status")
        with dg.FlowLayout(gap=6, row_gap=6):
            dg.Button("Rename Selected", on_click=rename_selected)
            dg.Button("Undo", on_click=undo_graph)
            dg.Button("Redo", on_click=redo_graph)
            dg.Button("Fit Request", on_click=request_fit)
            dg.Button("Snapshot", on_click=log_snapshot)
            dg.Button("Model Smoke", on_click=run_model_smoke)
        dg.Label("Event Log", class_="section")
        event_log = dg.LogView(
            [
                "NodeGraph probe ready.",
                "Try dragging Agent.message -> Rule.message.",
                "Try dragging Terminal.stdout -> Agent.message to see type rejection.",
            ],
            rows=24,
            wrap=False,
        )

refresh_state()

if __name__ == "__main__":
    print(app.run(win))
