"""NodeGraph editor probe.

Exercises the canvas-backed node editor: templates, typed ports, validation,
events, history, navigation, persistence, and the Python-side agent models.
"""

from __future__ import annotations

import copy
import json
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


TEMPLATES = dg.multi_agent_node_templates()


def template_data(template_id: str, **config: object) -> dict[str, object]:
    """Copy template inspector metadata while allowing probe-specific defaults."""

    template = next(template for template in TEMPLATES if template.id == template_id)
    data = copy.deepcopy(template.data or {})
    node_config = data.setdefault("config", {})
    if isinstance(node_config, dict):
        node_config.update(config)
    return data

NODES = [
    dg.NodeGraphNode(
        "terminal_executable_text",
        "Terminal Executable",
        -270,
        55,
        outputs=(dg.NodeGraphPort("text", "text", port_type="text"),),
        subtitle="text -> terminal command",
        status="ready",
        color="#7aa2f7",
        width=230,
        data=template_data(
            "text_input",
            text="cmd.exe",
            output_mode="manual",
        ),
    ),
    dg.NodeGraphNode(
        "terminal_args_text",
        "Terminal Args",
        -270,
        145,
        outputs=(dg.NodeGraphPort("text", "text", port_type="text"),),
        subtitle="JSON list -> terminal args",
        status="ready",
        color="#7aa2f7",
        width=230,
        data=template_data(
            "text_input",
            text="[]",
            output_mode="manual",
        ),
    ),
    dg.NodeGraphNode(
        "terminal_command_text",
        "Terminal Stdin Text",
        -270,
        235,
        outputs=(dg.NodeGraphPort("text", "text", port_type="text"),),
        subtitle="text -> terminal stdin",
        status="ready",
        color="#7aa2f7",
        width=230,
        data=template_data(
            "text_input",
            text="echo DragonGUI runtime probe",
            output_mode="manual",
        ),
    ),
    dg.NodeGraphNode(
        "gui_prompt_source",
        "GUI Prompt Source",
        -270,
        345,
        outputs=(dg.NodeGraphPort("value", "text", port_type="text"),),
        subtitle="GUI field -> terminal stdin",
        status="ready",
        color="#7aa2f7",
        width=230,
        data=template_data(
            "widget_source",
            widget_id="runtime-prompt-input",
            widget_type="text_input",
            port_profile="text",
            format="text",
        ),
    ),
    dg.NodeGraphNode(
        "implementer_terminal",
        "Implementer Terminal",
        20,
        130,
        inputs=(
            dg.NodeGraphPort("command", "command", port_type="text"),
            dg.NodeGraphPort("args", "args", port_type="text"),
            dg.NodeGraphPort("stdin", "terminal_input", port_type="terminal_input"),
        ),
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
        "terminal_output_display",
        "Terminal Output Display",
        310,
        260,
        inputs=(dg.NodeGraphPort("value", "terminal_output", port_type="terminal_output"),),
        outputs=(dg.NodeGraphPort("value", "terminal_output", port_type="terminal_output"),),
        subtitle="stdout -> GUI log",
        status="watching",
        color="#2ac3de",
        width=250,
        data={
            "node_type": "widget_sink",
            "template_id": "widget_sink",
            "default_status": "watching",
            "config": {
                "widget_id": "terminal-output-log",
                "widget_type": "log_view",
                "port_profile": "terminal_output",
                "update_mode": "append",
                "format": "text",
            },
        },
    ),
    dg.NodeGraphNode(
        "message_indicator",
        "Parsed Message Display",
        610,
        90,
        inputs=(dg.NodeGraphPort("value", "message", port_type="message"),),
        outputs=(dg.NodeGraphPort("value", "message", port_type="message"),),
        subtitle="message -> GUI log",
        status="watching",
        color="#2ac3de",
        width=250,
        data={
            "node_type": "widget_sink",
            "template_id": "widget_sink",
            "default_status": "watching",
            "config": {
                "widget_id": "runtime-message-indicator",
                "widget_type": "log_view",
                "port_profile": "message",
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
        -300,
        48,
        1220,
        430,
        purpose="terminal stdout -> GUI log and parser -> message log",
        trigger="manual_probe",
        color="#43c6ac",
        data={
            "runtime_scope": "manual_probe",
            "owns": ["implementer_terminal"],
            "refs": ["terminal-output-log", "runtime-message-indicator"],
            "action_id": "run-section-action-button",
            "section_command": "run",
        },
    ),
]
EDGES = [
    dg.NodeGraphEdge(
        "terminal_executable_text",
        "text",
        "implementer_terminal",
        "command",
        label="text -> terminal command",
        color=dg.node_graph_port_type_color("text"),
        id="edge-text-terminal-command",
    ),
    dg.NodeGraphEdge(
        "terminal_args_text",
        "text",
        "implementer_terminal",
        "args",
        label="text -> terminal args",
        color=dg.node_graph_port_type_color("text"),
        id="edge-text-terminal-args",
    ),
    dg.NodeGraphEdge(
        "terminal_command_text",
        "text",
        "implementer_terminal",
        "stdin",
        label="text -> terminal stdin",
        color=dg.node_graph_port_type_color("text"),
        id="edge-text-terminal-stdin",
    ),
    dg.NodeGraphEdge(
        "gui_prompt_source",
        "value",
        "implementer_terminal",
        "stdin",
        label="GUI prompt -> terminal stdin",
        color=dg.node_graph_port_type_color("text"),
        id="edge-gui-prompt-terminal-stdin",
    ),
    dg.NodeGraphEdge(
        "implementer_terminal",
        "stdout",
        "parser",
        "in",
        label="terminal_output",
        color=dg.node_graph_port_type_color("terminal_output"),
        id="edge-terminal-parser",
    ),

    dg.NodeGraphEdge(
        "implementer_terminal",
        "stdout",
        "terminal_output_display",
        "value",
        label="stdout -> terminal output log",
        color=dg.node_graph_port_type_color("terminal_output"),
        id="edge-terminal-output-display",
    ),
    dg.NodeGraphEdge(
        "parser",
        "message",
        "message_indicator",
        "value",
        label="message -> parsed message log",
        color=dg.node_graph_port_type_color("message"),
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
        gap: 10px;
    }

    Panel.canvas {
        flex-grow: 1;
        min-width: 0;
        min-height: 0;
        height: 100%;
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

    Collapsible.diagnostics {
        width: 100%;
        flex-grow: 0;
    }

    LogView {
        width: 100%;
        flex-grow: 0;
        min-height: 0;
        height: 230px;
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
history_label: dg.Label | None = None
event_log: dg.LogView | None = None
terminal_output_log: dg.LogView | None = None
reviewer_terminal_output_log: dg.LogView | None = None
runtime_indicator: dg.LogView | None = None
runtime_view_panel: dg.Panel | None = None
runtime_prompt_input: dg.TextInput | None = None
runtime_status_label: dg.Label | None = None
selected_node_id: str | None = None
runtime_session: dg.NodeGraphRuntimeSession | None = None
runtime_terminal_id = "implementer-session"
runtime_view_signature: tuple[object, ...] | None = None

def log(line: object = "") -> None:
    if event_log is not None:
        event_log.append_line(line)



def sync_runtime_prompt_input(value: str) -> None:
    # The runtime callback wrapper has already mirrored the native value onto the widget.
    _ = value


def register_graph_widget_targets() -> None:
    if graph is None:
        return
    formats = ("text", "json", "repr", "message_body")
    if terminal_output_log is not None:
        graph.register_binding_target(
            id="terminal-output-log",
            label="Terminal Output",
            widget_type="log_view",
            widget=terminal_output_log,
            supported_update_modes=("append", "set"),
            default_update_mode="append",
            supported_port_profiles=("terminal_output", "text"),
            default_port_profile="terminal_output",
            supported_formats=formats,
        )
    if reviewer_terminal_output_log is not None:
        graph.register_binding_target(
            id="reviewer-terminal-output-log",
            label="Reviewer Terminal Output",
            widget_type="log_view",
            widget=reviewer_terminal_output_log,
            supported_update_modes=("append", "set"),
            default_update_mode="append",
            supported_port_profiles=("terminal_output", "text"),
            default_port_profile="terminal_output",
            supported_formats=formats,
        )
    if runtime_indicator is not None:
        graph.register_binding_target(
            id="runtime-message-indicator",
            label="Parsed Message Log",
            widget_type="log_view",
            widget=runtime_indicator,
            supported_update_modes=("append", "set"),
            default_update_mode="append",
            supported_port_profiles=("message", "json", "text"),
            default_port_profile="message",
            supported_formats=formats,
        )
    if runtime_prompt_input is not None:
        graph.register_binding_target(
            id="runtime-prompt-input",
            label="Runtime Prompt Input",
            widget_type="text_input",
            widget=runtime_prompt_input,
            supported_update_modes=("set",),
            default_update_mode="set",
            supported_port_profiles=("text",),
            default_port_profile="text",
            supported_formats=("text", "json", "repr", "raw"),
        )
    if event_log is not None:
        graph.register_binding_target(
            id="node-probe-event-log",
            label="Event Log",
            widget_type="log_view",
            widget=event_log,
            supported_update_modes=("append", "set"),
            default_update_mode="append",
            supported_port_profiles=("event", "json", "text", "message", "terminal_output"),
            default_port_profile="event",
            supported_formats=formats,
        )


def register_runtime_widget_targets() -> None:
    if graph is None or runtime_session is None:
        return
    for target in graph.widget_targets:
        if target.widget is not None:
            runtime_session.register_widget(target.id, target.widget)



def refresh_runtime_status() -> None:
    if runtime_status_label is None:
        return
    if runtime_session is None:
        if graph is not None:
            runtime_status_label.text = graph.managed_runtime_status_text()
        else:
            runtime_status_label.text = "Runtime: idle"
        return
    last_event = runtime_session.events[-1].event if runtime_session.events else None
    parts = [
        "Runtime: active",
        f"status {runtime_session.status}",
        f"widgets {len(runtime_session.widget_ids())}",
        f"handles {len(runtime_session.handles)}",
        f"events {len(runtime_session.events)}",
    ]
    if last_event:
        parts.append(f"last {last_event}")
    runtime_status_label.text = " | ".join(parts)


def run_runtime_smoke_action(action_id: str, command: str) -> object | None:
    global runtime_session
    if graph is None:
        return None
    if runtime_session is None:
        runtime_session = graph.runtime_session(session_id="probe-runtime")
        register_runtime_widget_targets()
        log("runtime session created for section action")
    register_runtime_widget_targets()
    event = runtime_session.run_section_command("runtime-smoke", command)
    log(f"section action {action_id} command={command} event={event.event}")
    if event.data:
        log(f"section action data {json.dumps(event.data, sort_keys=True)}")
    log_runtime_tail()
    render_runtime_view(selected_node_id or "implementer_terminal")
    return event


def register_graph_action_targets() -> None:
    if graph is None:
        return
    graph.register_binding_target(
        "run-section-action-button",
        label="Run Section Action Button",
        action_type="button",
        callback=run_runtime_smoke_action,
        supported_commands=("run", "stop", "reset", "replay"),
        default_command="run",
    )


def run_runtime_smoke_section_action() -> None:
    if graph is None:
        return
    try:
        graph.run_section_action("runtime-smoke")
    except Exception as exc:
        log(f"section action failed: {exc}")

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
    history = graph.history_state()
    if history_label is not None:
        history_label.set_value(
            "History: "
            f"undo={history['undo_depth']} redo={history['redo_depth']} dirty={history['dirty']}"
        )


def on_graph_event(payload: dict[str, object]) -> None:
    global selected_node_id
    event = str(payload.get("event", ""))
    if event == "node_selected":
        node = payload.get("node")
        selected_node_id = None if node is None else str(node)
    elif event == "node_created":
        node = payload.get("node")
        if isinstance(node, dict):
            selected_node_id = None if node.get("id") is None else str(node.get("id"))
    elif event in {"selection_cleared", "edge_selected"}:
        selected_node_id = None
    elif event == "editor_action":
        action = str(payload.get("action", ""))
        if action == "snapshot":
            log_snapshot()
        elif action == "run_section":
            run_runtime_smoke_section_action()
        elif action == "send_input":
            send_runtime_input()
        elif action == "send_prompt":
            send_runtime_prompt_input()
        elif action == "stop_terminal":
            stop_runtime_terminal()
        elif action == "cleanup_runtime":
            cleanup_runtime_session()
        else:
            log(f"editor action ignored: {action}")

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
    register_runtime_widget_targets()
    snapshot = runtime_session.snapshot()
    log(
        "runtime session "
        f"valid={snapshot['valid']} objects={len(snapshot['objects'])} events={len(snapshot['events'])}"
    )
    refresh_runtime_status()
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
        register_runtime_widget_targets()
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
    before_sequence = runtime_session.events[-1].sequence if runtime_session.events else 0
    runtime_session.run_node("terminal_command_text")
    new_events = [event for event in runtime_session.events if event.sequence > before_sequence]
    outputs = runtime_session.port_values("terminal_command_text", "text")
    skipped = [event for event in new_events if event.event == "node_run_skipped"]
    failures = [event for event in new_events if event.event == "edge_conversion_failed"]
    applied = [event for event in new_events if event.event == "edge_conversion_applied"]
    if outputs:
        log(f"terminal input source emitted: {outputs[-1]!r}")
    if skipped:
        log(f"terminal input source skipped: {skipped[-1].data.get('reason') if skipped[-1].data else 'unknown'}")
    elif failures:
        log(f"terminal stdin conversion blocked: {failures[-1].data.get('reason') if failures[-1].data else 'unknown'}")
    elif applied:
        log(f"terminal stdin delivered via graph edge={applied[-1].data.get('delivered') if applied[-1].data else None}")
    else:
        log("terminal input source ran; no terminal conversion result recorded")
    log_runtime_tail()

def send_runtime_prompt_input() -> None:
    if runtime_session is None:
        log("runtime GUI prompt skipped: create/attach runtime first")
        return
    before_sequence = runtime_session.events[-1].sequence if runtime_session.events else 0
    runtime_session.run_node("gui_prompt_source")
    new_events = [event for event in runtime_session.events if event.sequence > before_sequence]
    outputs = runtime_session.port_values("gui_prompt_source", "value")
    reads = [event for event in new_events if event.event == "widget_read"]
    skipped = [event for event in new_events if event.event == "node_run_skipped"]
    failures = [event for event in new_events if event.event in {"widget_read_failed", "edge_conversion_failed"}]
    applied = [event for event in new_events if event.event == "edge_conversion_applied"]
    if reads and outputs:
        log(f"GUI prompt source read: {outputs[-1]!r}")
    if skipped:
        log(f"GUI prompt source skipped: {skipped[-1].data.get('reason') if skipped[-1].data else 'unknown'}")
    elif failures:
        log(f"GUI prompt delivery blocked: {failures[-1].data.get('reason') if failures[-1].data else 'unknown'}")
    elif applied:
        log(f"GUI prompt delivered via graph edge={applied[-1].data.get('delivered') if applied[-1].data else None}")
    else:
        log("GUI prompt source ran; no terminal conversion result recorded")
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
    if terminal_output_log is not None:
        terminal_output_log.clear()
        terminal_output_log.append_line("")
    if reviewer_terminal_output_log is not None:
        reviewer_terminal_output_log.clear()
        reviewer_terminal_output_log.append_line("")
    if runtime_indicator is not None:
        runtime_indicator.clear()
        runtime_indicator.append_line("Parsed message sink idle.")
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
    refresh_runtime_status()


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
            show_editor_chrome=True,
            editor_title="NodeGraph",
            editor_actions=(
                {"id": "snapshot", "label": "Snapshot", "icon": "S", "separator_before": True},
                {"id": "run_section", "label": "Run section", "icon": "Run", "wide": True, "separator_before": True},
                {"id": "send_input", "label": "Send input", "icon": "Input", "primary": True},
                {"id": "send_prompt", "label": "Send prompt", "icon": "Prompt", "wide": True},
                {"id": "stop_terminal", "label": "Stop terminal", "icon": "Stop", "wide": True},
                {"id": "cleanup_runtime", "label": "Cleanup runtime", "icon": "X", "separator_before": True},
            ),
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
        with dg.FlowLayout(gap=6, row_gap=6):
            dg.Tag("typed ports", level="success")
            dg.Tag("palette templates", level="success")
            dg.Tag("undo/redo", level="neutral")
            dg.Tag("fit/zoom/grid/minimap", level="neutral")
            dg.Tag("invalid links reject", level="warning")
        history_label = dg.Label("History: loading", class_="status")
        dg.Label("Runtime Checks", class_="section")
        runtime_status_label = dg.Label("Runtime: idle", class_="status")
        runtime_prompt_input = dg.TextInput(
            "echo DragonGUI prompt source",
            id="runtime-prompt-input",
            placeholder="Command text from GUI field...",
            on_change=sync_runtime_prompt_input,
            style={"width": "100%", "height": 34, "flex_shrink": 0},
        )
        with dg.Collapsible("Manual Runtime Diagnostics", expanded=False, class_="diagnostics"):
            with dg.FlowLayout(gap=6, row_gap=6):
                dg.Button("Runtime Session", on_click=create_runtime_session)
                dg.Button("Attach Terminal", on_click=attach_runtime_terminal)
                dg.Button("Start Terminal", on_click=start_runtime_terminal)
                dg.Button("Model Smoke", on_click=run_model_smoke)
                dg.Button("Text Flow Demo", on_click=run_text_flow_demo)
                dg.Button("Inject Plain", on_click=inject_runtime_plain_output)
                dg.Button("Inject Envelope", on_click=inject_runtime_envelope)
                dg.Button("Clear Indicator", on_click=clear_runtime_indicator)
        runtime_view_panel = dg.Panel("Runtime View", class_="runtime-view")
        with runtime_view_panel:
            dg.Label("No runtime session", class_="status")
            dg.Label("Click Runtime Session, then Attach Terminal.", class_="muted")
        dg.Label("Terminal Output", class_="section")
        terminal_output_log = dg.LogView(
            [],
            id="terminal-output-log",
            follow=True,
            rows=4,
            wrap=False,
        )
        dg.Label("Reviewer Terminal Output", class_="section")
        reviewer_terminal_output_log = dg.LogView(
            [],
            id="reviewer-terminal-output-log",
            follow=True,
            rows=4,
            wrap=False,
        )
        dg.Label("Parsed Message Log", class_="section")
        runtime_indicator = dg.LogView(
            ["Parsed message sink idle."],
            id="runtime-message-indicator",
            follow=True,
            rows=4,
            wrap=True,
        )
        dg.Label("Event Log", class_="section")
        event_log = dg.LogView(
            [
                "NodeGraph probe ready.",
                "Default path: Terminal Command Text.text -> Terminal.stdin, then Terminal.stdout -> display/parser.",
                "Use Inject Plain for raw terminal output; Inject Envelope also feeds the parsed message log.",
            ],
            id="node-probe-event-log",
            follow=True,
            rows=6,
            wrap=True,
        )
        register_graph_widget_targets()
        register_graph_action_targets()

refresh_runtime_status()
refresh_state()

if __name__ == "__main__":
    print(app.run(win))








