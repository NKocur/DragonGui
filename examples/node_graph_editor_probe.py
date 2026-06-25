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
        data={
            "node_type": "terminal",
            "template_id": "terminal",
            "default_status": "running",
            "runtime_object": "terminal_session",
            "config": {
                "session_id": "implementer-session",
                "command": "cmd.exe",
                "args": [],
                "prefer_pty": True,
            },
        },
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
        "message_indicator",
        "Message Indicator",
        610,
        90,
        inputs=(dg.NodeGraphPort("value", "message", port_type="message"),),
        outputs=(dg.NodeGraphPort("value", "message", port_type="message"),),
        subtitle="widget sink",
        status="watching",
        color="#2ac3de",
        width=230,
        data={
            "node_type": "widget_sink",
            "template_id": "widget_sink",
            "default_status": "watching",
            "config": {
                "widget_id": "runtime-message-indicator",
                "widget_type": "log_view",
                "update_mode": "append",
                "format": "json",
            },
        },
    ),
]
SECTIONS = [
    dg.NodeGraphSection(
        "runtime-smoke",
        "Runtime Smoke Test",
        -18,
        48,
        890,
        230,
        purpose="terminal stdout -> parser -> widget sink",
        trigger="manual_probe",
        color="#43c6ac",
        data={"runtime_scope": "manual_probe", "owns": ["implementer_terminal"], "refs": ["runtime-message-indicator"]},
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
        "message_indicator",
        "value",
        label="message indicator",
        color="#2ac3de",
        id="edge-parser-indicator",
    ),
]

SAVED_GRAPH_DATA = dg.NodeGraph(NODES, EDGES, sections=SECTIONS, templates=TEMPLATES, parent=None).to_graph_data()
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
        height: 230px;
        gap: 10px;
    }

    Panel.canvas {
        flex-grow: 0;
        min-width: 0;
        min-height: 0;
        height: 230px;
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
        flex-grow: 0;
        min-height: 0;`r`n        height: 230px;
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
runtime_indicator: dg.LogView | None = None
runtime_view_panel: dg.Panel | None = None
selected_node_id: str | None = None
runtime_session: dg.NodeGraphRuntimeSession | None = None
runtime_terminal_id = "implementer-session"
runtime_view_signature: tuple[object, ...] | None = None

def log(line: object = "") -> None:
    if event_log is not None:
        event_log.append_line(line)


def add_toolbar_node() -> None:
    global selected_node_id
    if graph is None:
        return
    node = graph.create_node_from_template(TEMPLATES[0].id, 120, 120, notify=True)
    selected_node_id = node.id
    log(f"toolbar created {node.id} from {TEMPLATES[0].id} at 120,120")
    refresh_state()


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
    elif event == "node_created":
        node = payload.get("node")
        if isinstance(node, dict):
            selected_node_id = None if node.get("id") is None else str(node.get("id"))
            if selection_label is not None:
                selection_label.set_value(f"Selected: {selected_node_id or 'none'}")
    elif event == "node_picker_opened":
        position = payload.get("position")
        if isinstance(position, dict) and status_label is not None:
            status_label.set_value(f"Choose node for {float(position.get('x', 0.0)):.0f},{float(position.get('y', 0.0)):.0f}")
    elif event == "node_picker_selected":
        if status_label is not None:
            status_label.set_value(f"Added template: {payload.get('template')}")
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
    if event == "node_deleted":
        deleted_node = payload.get("node")
        shown_node = runtime_view_signature[1] if runtime_view_signature is not None and len(runtime_view_signature) > 1 else None
        if deleted_node is not None and str(deleted_node) == shown_node:
            render_runtime_view(str(deleted_node))


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



def run_text_flow_demo() -> None:
    demo = dg.NodeGraph(
        [
            {
                "id": "input",
                "title": "Demo Envelope",
                "x": 0,
                "y": 0,
                "outputs": [dg.NodeGraphPort("text", "text", port_type="text")],
                "data": {
                    "node_type": "text_input",
                    "config": {
                        "text": (
                            "@to reviewer\n"
                            "@from implementer\n"
                            "@type review_request\n"
                            "@id DG-flow-1\n"
                            "Please inspect the non-destructive flow demo.\n"
                            "@end\n"
                        )
                    },
                },
            },
            {
                "id": "parser",
                "title": "Envelope Parser",
                "x": 240,
                "y": 0,
                "inputs": [dg.NodeGraphPort("text", "text", port_type="text")],
                "outputs": [dg.NodeGraphPort("message", "message", port_type="message")],
                "data": {"node_type": "envelope_parser"},
            },
            {
                "id": "router",
                "title": "Message Router",
                "x": 480,
                "y": 0,
                "inputs": [dg.NodeGraphPort("message", "message", port_type="message")],
                "outputs": [
                    dg.NodeGraphPort("reviewer", "reviewer", port_type="message"),
                    dg.NodeGraphPort("default", "default", port_type="message"),
                ],
                "data": {
                    "node_type": "message_router",
                    "config": {
                        "rules": [{"field": "to", "equals": "reviewer", "output": "reviewer"}],
                        "default_target": "default",
                    },
                },
            },
            {
                "id": "log",
                "title": "Log",
                "x": 720,
                "y": 0,
                "inputs": [dg.NodeGraphPort("value", "value", port_type="json")],
                "outputs": [dg.NodeGraphPort("value", "value", port_type="json")],
                "data": {"node_type": "log"},
            },
        ],
        [
            dg.NodeGraphEdge("input", "text", "parser", "text"),
            dg.NodeGraphEdge("parser", "message", "router", "message"),
            dg.NodeGraphEdge("router", "reviewer", "log", "value"),
        ],
        parent=None,
    )
    run = demo.run_text_flow()
    routed = run.port_values("router", "reviewer")
    log(f"text flow demo valid={run.valid} routed={len(routed)} logged={len(run.port_values('log', 'value'))}")
    if routed:
        message = routed[0]
        log(f"text flow message {message.get('id')} -> {message.get('to')}: {message.get('body')}")
    events = [str(entry["event"]) for entry in run.log]
    counts = {name: events.count(name) for name in sorted(set(events))}
    trace = " -> ".join(event for event in events if event != "emit")
    log(f"text flow trace {trace}")
    log(f"text flow event counts {json.dumps(counts, sort_keys=True)}")


def create_runtime_session() -> None:
    global runtime_session
    if graph is None:
        return
    runtime_session = graph.runtime_session(session_id="probe-runtime")
    if runtime_indicator is not None:
        runtime_session.register_widget(runtime_indicator)
    snapshot = runtime_session.snapshot()
    log(
        "runtime session "
        f"valid={snapshot['valid']} objects={len(snapshot['objects'])} events={len(snapshot['events'])}"
    )
    for obj in snapshot["objects"]:
        if isinstance(obj, dict):
            log(
                "runtime object "
                f"{obj.get('object_id')} type={obj.get('object_type')} status={obj.get('status')}"
            )


def attach_runtime_terminal() -> None:
    global runtime_session
    if graph is None:
        return
    if runtime_session is None:
        runtime_session = graph.runtime_session(session_id="probe-runtime")
        if runtime_indicator is not None:
            runtime_session.register_widget(runtime_indicator)
    bridge = runtime_session.create_terminal_bridge(runtime_terminal_id, start=False)
    log(f"terminal bridge attached {bridge.command.label} status={bridge.status}")
    render_runtime_view("implementer_terminal")
    log_runtime_tail()


def start_runtime_terminal() -> None:
    if runtime_session is None:
        log("runtime start skipped: create/attach runtime first")
        return
    before = runtime_session.object_handle(runtime_terminal_id)
    had_view = before is not None and before.handle_attached
    bridge = runtime_session.start_terminal_session(runtime_terminal_id)
    log(f"terminal start requested status={bridge.status}")
    if had_view:
        log("terminal view already attached; keeping existing surface")
    else:
        render_runtime_view("implementer_terminal")
    log_runtime_tail()


def send_runtime_input() -> None:
    if runtime_session is None:
        log("runtime input skipped: create/attach runtime first")
        return
    try:
        delivered = runtime_session.send_terminal_input(runtime_terminal_id, "echo DragonGUI runtime probe", newline=True)
    except RuntimeError as exc:
        log(f"runtime input blocked: {exc}")
        return
    log(f"terminal stdin delivered={delivered}")
    log_runtime_tail()


def inject_runtime_plain_output() -> None:
    if runtime_session is None:
        log("runtime plain output skipped: create/attach runtime first")
        return
    runtime_session.apply_terminal_event(
        runtime_terminal_id,
        {
            "event": "output",
            "data": "plain runtime output\n",
        },
    )
    log("injected plain terminal stdout")
    log_runtime_tail()


def inject_runtime_envelope() -> None:
    if runtime_session is None:
        log("runtime envelope skipped: create/attach runtime first")
        return
    runtime_session.apply_terminal_event(
        runtime_terminal_id,
        {
            "event": "output",
            "data": (
                "@to reviewer_agent\n"
                "@from implementer_terminal\n"
                "@type review_request\n"
                "@id probe-live-1\n"
                "Please inspect the live runtime path.\n"
                "@end\n"
            ),
        },
    )
    log("injected terminal stdout envelope")
    log_runtime_tail()


def clear_runtime_indicator() -> None:
    if runtime_indicator is not None:
        runtime_indicator.clear()
        runtime_indicator.append_line("Widget sink idle.")
    log("runtime indicator cleared")


def stop_runtime_terminal() -> None:
    if runtime_session is None:
        log("runtime stop skipped: create/attach runtime first")
        return
    stopped = runtime_session.stop_runtime_object(runtime_terminal_id)
    log(f"terminal stop requested stopped={stopped}")
    log_runtime_tail()


def cleanup_runtime_session() -> None:
    if runtime_session is None:
        log("runtime cleanup skipped: create runtime first")
        return
    log(f"runtime cleanup {json.dumps(runtime_session.cleanup(), sort_keys=True)}")
    log_runtime_tail()


def log_runtime_tail() -> None:
    if runtime_session is None:
        return
    snapshot = runtime_session.snapshot()
    objects = snapshot.get("objects", [])
    if objects:
        log(f"runtime objects {json.dumps(objects, sort_keys=True)}")
    port_values = snapshot.get("port_values", {})
    if port_values:
        counts = {
            key: len(value)
            for key, value in port_values.items()
            if isinstance(value, list)
        }
        log(f"runtime port values {json.dumps(counts, sort_keys=True)}")
    events = snapshot.get("events", [])
    tail = events[-4:] if isinstance(events, list) else []
    for event in tail:
        log(f"runtime event {json.dumps(event, sort_keys=True)}")


def render_runtime_view(node_id: str | None = None) -> None:
    global runtime_view_signature
    if runtime_view_panel is None:
        return
    target = node_id or selected_node_id or "implementer_terminal"
    if runtime_session is None:
        signature = ("empty",)
        if runtime_view_signature == signature:
            return
        runtime_view_signature = signature
        runtime_view_panel.replace_children(
            [
                dg.Label("No runtime session", class_="status", parent=None),
                dg.Label("Click Runtime Session, then Attach Terminal.", class_="muted", parent=None),
            ]
        )
        return
    binding = runtime_session.view_binding(target)
    if binding is None:
        signature = ("missing", target)
        if runtime_view_signature == signature:
            return
        runtime_view_signature = signature
        runtime_view_panel.replace_children(
            [
                dg.Label(f"No runtime view for {target}", class_="status", parent=None),
                dg.LogView([json.dumps(runtime_session.validate(), sort_keys=True)], rows=5, wrap=True, parent=None),
            ]
        )
        return
    handle = runtime_session.object_handle(binding.object_id) if binding.object_id is not None else None
    if binding.view_type == "terminal" and handle is not None and handle.handle is not None:
        signature = ("terminal", binding.node_id, binding.object_id, id(handle.handle))
        if runtime_view_signature == signature:
            return
        runtime_view_signature = signature
        runtime_view_panel.replace_children(
            [
                dg.Label(f"{binding.title or binding.node_id} -> {binding.object_id}", class_="status", parent=None),
                dg.Terminal(
                    bridge=handle.handle,
                    title=str(binding.title or binding.object_id or "Terminal"),
                    height=220,
                    parent=None,
                ),
            ]
        )
        return
    signature = ("detail", binding.node_id, binding.view_type, binding.object_id, binding.available, binding.reason)
    if runtime_view_signature == signature:
        return
    runtime_view_signature = signature
    runtime_view_panel.replace_children(
        [
            dg.Label(
                f"{binding.title or binding.node_id}: {binding.view_type}",
                class_="status",
                parent=None,
            ),
            dg.LogView(
                [
                    json.dumps(binding.to_dict(), sort_keys=True),
                    "Attach or start the runtime object to show its live view.",
                ],
                follow=True,
                rows=6,
                wrap=True,
                parent=None,
            ),
        ]
    )


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
            sections=LOADED_GRAPH.sections,
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
            "Double-click empty canvas to choose a node from the command palette. Drag typed pins, "
            "press Enter/F2 to rename, Ctrl+Z/Ctrl+Y for history, F to fit, +/- to zoom, G for grid.",
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
            dg.Button("Text Flow Demo", on_click=run_text_flow_demo)
            dg.Button("Add Terminal Node", on_click=add_toolbar_node)
        dg.Label("Runtime Checks", class_="section")
        with dg.FlowLayout(gap=6, row_gap=6):
            dg.Button("Runtime Session", on_click=create_runtime_session)
            dg.Button("Attach Terminal", on_click=attach_runtime_terminal)
            dg.Button("Start Terminal", on_click=start_runtime_terminal)
            dg.Button("Send Input", on_click=send_runtime_input)
            dg.Button("Inject Plain", on_click=inject_runtime_plain_output)
            dg.Button("Inject Envelope", on_click=inject_runtime_envelope)
            dg.Button("Clear Indicator", on_click=clear_runtime_indicator)
            dg.Button("Stop Terminal", on_click=stop_runtime_terminal)
            dg.Button("Cleanup Runtime", on_click=cleanup_runtime_session)
        runtime_view_panel = dg.Panel("Runtime View", class_="runtime-view")
        with runtime_view_panel:
            dg.Label("No runtime session", class_="status")
            dg.Label("Click Runtime Session, then Attach Terminal.", class_="muted")
        dg.Label("Runtime Indicator", class_="section")
        runtime_indicator = dg.LogView(
            ["Widget sink idle."],
            id="runtime-message-indicator",
            follow=True,
            rows=4,
            wrap=True,
        )
        dg.Label("Event Log", class_="section")
        event_log = dg.LogView(
            [
                "NodeGraph probe ready.",
                "Default path: Terminal.stdout -> Parser.in -> Message Indicator.value.",
                "Use Inject Plain for edge transport, Inject Envelope for parser + widget sink.",
            ],
            follow=True,
            rows=6,
            wrap=True,
        )

refresh_state()

if __name__ == "__main__":
    print(app.run(win))





