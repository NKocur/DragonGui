"""Blank NodeGraph binding playground probe.

A small sandbox for testing the shared GUI binding registry without the larger
runtime demo graph. Start with an empty node editor, add generic nodes from the
canvas palette, then assign them to the real controls on the right.
"""

from __future__ import annotations

import json
import sys
from collections.abc import Mapping
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


TEMPLATES = dg.multi_agent_node_templates()
ACTION_SLOT_TARGETS = (
    ("action-slot-a", "Action Slot A"),
    ("action-slot-b", "Action Slot B"),
    ("action-slot-c", "Action Slot C"),
    ("action-slot-d", "Action Slot D"),
)


app = dg.App(theme=dg.Theme.dark(accent="#43c6ac", focus="#f8c14a", radius=7))
app.stylesheet(
    """
    Window {
        background: #111722;
        color: rgba(245, 248, 252, 0.94);
        padding: 10px;
        gap: 10px;
    }
    HLayout.root {
        width: 100%;
        height: 100%;
        gap: 10px;
    }
    Panel.canvas {
        flex-grow: 1;
        min-width: 760px;
        min-height: 0;
        height: 100%;
        padding: 10px;
        gap: 8px;
    }
    Panel.side {
        width: 430px;
        min-width: 390px;
        max-width: 470px;
        height: 100%;
        padding: 10px;
        gap: 8px;
    }
    HtmlReport.node-graph {
        width: 100%;
        flex-grow: 1;
        min-height: 640px;
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 8px;
        background: #0d1117;
    }
    Label.section {
        margin-top: 10px;
        color: #dce7f4;
        font-weight: 800;
    }
    Label.muted {
        color: #9aa8b8;
        font-size: 12px;
        line-height: 1.35;
    }
    Label.status {
        color: #b8c7d9;
        font-size: 12px;
    }
    TextInput.mini-field {
        width: 100%;
        height: 34px;
        flex-shrink: 0;
    }
    LogView.play-log {
        width: 100%;
        flex-shrink: 0;
        background: #070a0f;
        border: 1px solid rgba(255, 255, 255, 0.11);
        border-radius: 6px;
        font-family: "Consolas";
        font-size: 12px;
    }
    Button {
        padding: 7px 9px;
    }
    """
)

win = dg.Window("NodeGraph Binding Playground", width=1480, height=860)

graph: dg.NodeGraph | None = None
status_label: dg.Label | None = None
counts_label: dg.Label | None = None
selection_label: dg.Label | None = None
runtime_status_label: dg.Label | None = None
selected_section_id: str | None = None
prompt_input: dg.TextInput | None = None
review_input: dg.TextInput | None = None
scratch_input: dg.TextInput | None = None
main_output_log: dg.LogView | None = None
review_output_log: dg.LogView | None = None
message_log: dg.LogView | None = None
event_log: dg.LogView | None = None


def log(line: object = "") -> None:
    if event_log is not None:
        event_log.append_line(str(line))


def set_status(text: str) -> None:
    if status_label is not None:
        status_label.text = text
    log(text)


def refresh_counts() -> None:
    if graph is None or counts_label is None:
        return
    counts_label.text = (
        f"Graph: {len(graph.nodes)} nodes, {len(graph.edges)} edges, "
        f"{len(graph.sections)} sections"
    )


def refresh_selection() -> None:
    if graph is None or selection_label is None:
        return
    if graph.selected_node:
        selected = f"node {graph.selected_node}"
    elif selected_section_id:
        selected = f"section {selected_section_id}"
    else:
        selected = "none"
    selection_label.text = f"Selected: {selected}"


def refresh_runtime_status() -> None:
    if graph is None or runtime_status_label is None:
        return
    runtime_status_label.text = graph.managed_runtime_status_text()


def on_select(node_id: str) -> None:
    global selected_section_id
    selected_section_id = None
    refresh_selection()
    set_status(f"selected node {node_id}")


def on_move(node_id: str, x: float, y: float) -> None:
    if status_label is not None:
        status_label.text = f"Moved {node_id}: {x:.0f}, {y:.0f}"


def on_graph_event(event: dict[str, object]) -> None:
    global selected_section_id
    event_name = str(event.get("event", "event"))
    if event_name in {
        "node_added",
        "node_deleted",
        "edge_added",
        "edge_deleted",
        "section_added",
        "section_deleted",
        "property_editor_saved",
    }:
        refresh_counts()
    if event_name == "section_selected":
        selected_section_id = str(event.get("section") or "") or None
    elif event_name in {"node_selected", "selection_cleared"}:
        selected_section_id = None
    if event_name in {"node_selected", "section_selected", "selection_cleared"}:
        refresh_selection()
    if event_name not in {"viewport_changed", "node_moved"}:
        log(json.dumps(event, sort_keys=True))


def binding_action(action_id: str, command: str) -> object:
    message = f"action target fired: {action_id} command={command}"
    set_status(message)
    if message_log is not None:
        message_log.append_line(message)
    return {"action_id": action_id, "command": command}


def run_selected_node_runtime() -> None:
    if graph is None:
        return
    if not graph.selected_node:
        set_status("select a node before running it")
        return
    try:
        event = graph.run_node_runtime(graph.selected_node)
    except Exception as exc:
        set_status(f"node runtime failed: {exc}")
        return
    log(f"node {graph.selected_node} runtime event: {event.event}")
    refresh_runtime_status()
    set_status(f"node {graph.selected_node} ran with managed runtime")


def run_selected_section_runtime() -> None:
    if graph is None:
        return
    if not selected_section_id:
        set_status("select a section before running its action")
        return
    try:
        result = graph.run_section_runtime(selected_section_id)
    except Exception as exc:
        set_status(f"section action failed: {exc}")
        return
    log(f"section {selected_section_id} runtime event: {result.event}")
    refresh_runtime_status()
    set_status(f"section {selected_section_id} ran with managed runtime")


def section_action_config(section: dg.NodeGraphSection) -> tuple[str, str]:
    data = section.data or {}
    if not isinstance(data, Mapping):
        return "", "run"
    config = data.get("config")
    source = config if isinstance(config, Mapping) else data
    action_id = str(source.get("action_id", "") or "").strip()
    command = str(source.get("section_command", "") or "run").strip() or "run"
    return action_id, command


def sections_for_action_target(action_id: str) -> tuple[dg.NodeGraphSection, ...]:
    if graph is None:
        return ()
    target_id = str(action_id or "").strip()
    return tuple(
        section
        for section in graph.sections
        if section_action_config(section)[0] == target_id
    )


def run_action_slot(action_id: str, command: str | None = None) -> object:
    if graph is None:
        return {"action_id": action_id, "ran": 0}
    sections = sections_for_action_target(action_id)
    if not sections:
        set_status(f"no sections assigned to {action_id}")
        return {"action_id": action_id, "ran": 0}
    events: list[str] = []
    for section in sections:
        _, section_command = section_action_config(section)
        command_s = str(command or section_command or "run").strip() or "run"
        try:
            event = graph.run_section_runtime(section.id, command_s)
        except Exception as exc:
            log(f"action slot {action_id} section {section.id} failed: {exc}")
            continue
        events.append(event.event)
        log(f"action slot {action_id} ran section {section.id} command={command_s} event={event.event}")
    refresh_runtime_status()
    set_status(f"{action_id} ran {len(events)} assigned section(s)")
    return {"action_id": action_id, "ran": len(events), "events": events}


def action_slot_callback(action_id: str, command: str) -> object:
    return run_action_slot(action_id, command)


def cleanup_runtime() -> None:
    if graph is None:
        return
    result = graph.cleanup_managed_runtime()
    log(f"managed runtime cleanup: {json.dumps(result, sort_keys=True)}")
    refresh_runtime_status()
    set_status("managed runtime cleaned up")


def append_sample_outputs() -> None:
    if main_output_log is not None:
        main_output_log.append_line("Main output sample: hello from the playground.")
    if review_output_log is not None:
        review_output_log.append_line("Reviewer output sample: this would be agent/reviewer text.")
    if message_log is not None:
        message_log.append_line("Parsed message sample: @type report")
    set_status("sample output appended")


def clear_logs() -> None:
    for target in (main_output_log, review_output_log, message_log, event_log):
        if target is not None:
            target.clear()
    set_status("logs cleared")


def dump_bindings() -> None:
    if graph is None:
        return
    payload = {
        "binding_targets": graph.binding_target_ids(),
        "widget_targets": graph.widget_target_ids(),
        "action_targets": graph.action_target_ids(),
    }
    log(json.dumps(payload, sort_keys=True))
    set_status("binding registry dumped to event log")


def snapshot_graph() -> None:
    if graph is None:
        return
    data = graph.to_graph_data()
    log(
        json.dumps(
            {
                "nodes": len(data.get("nodes", [])),
                "edges": len(data.get("edges", [])),
                "sections": len(data.get("sections", [])),
                "viewport": data.get("viewport"),
            },
            sort_keys=True,
        )
    )
    set_status("graph snapshot written to event log")


def reset_blank_graph() -> None:
    global selected_section_id
    if graph is None:
        return
    graph.set_nodes([])
    graph.set_edges([])
    graph.set_sections([])
    selected_section_id = None
    refresh_counts()
    refresh_selection()
    refresh_runtime_status()
    set_status("graph cleared")


def register_binding_targets() -> None:
    if graph is None:
        return
    formats = ("text", "json", "repr", "message_body", "raw")
    if prompt_input is not None:
        graph.register_binding_target(
            "prompt-input",
            label="Prompt Input",
            target_type="text_input",
            widget_type="text_input",
            widget=prompt_input,
            supported_update_modes=("set",),
            default_update_mode="set",
            supported_port_profiles=("text", "terminal_input"),
            default_port_profile="text",
            supported_formats=formats,
        )
    if review_input is not None:
        graph.register_binding_target(
            "review-input",
            label="Reviewer Prompt Input",
            target_type="text_input",
            widget_type="text_input",
            widget=review_input,
            supported_update_modes=("set",),
            default_update_mode="set",
            supported_port_profiles=("text", "message"),
            default_port_profile="text",
            supported_formats=formats,
        )
    if scratch_input is not None:
        graph.register_binding_target(
            "scratch-input",
            label="Scratch Text Input",
            target_type="text_input",
            widget_type="text_input",
            widget=scratch_input,
            supported_update_modes=("set",),
            default_update_mode="set",
            supported_port_profiles=("text", "json"),
            default_port_profile="text",
            supported_formats=formats,
        )
    if main_output_log is not None:
        graph.register_binding_target(
            "main-output-log",
            label="Main Output Log",
            target_type="log",
            widget_type="log_view",
            widget=main_output_log,
            supported_update_modes=("append", "set"),
            default_update_mode="append",
            supported_port_profiles=("terminal_output", "text", "json", "message"),
            default_port_profile="text",
            supported_formats=formats,
        )
    if review_output_log is not None:
        graph.register_binding_target(
            "review-output-log",
            label="Reviewer Output Log",
            target_type="log",
            widget_type="log_view",
            widget=review_output_log,
            supported_update_modes=("append", "set"),
            default_update_mode="append",
            supported_port_profiles=("terminal_output", "text", "json", "message"),
            default_port_profile="text",
            supported_formats=formats,
        )
    if message_log is not None:
        graph.register_binding_target(
            "message-log",
            label="Parsed Message Log",
            target_type="log",
            widget_type="log_view",
            widget=message_log,
            supported_update_modes=("append", "set"),
            default_update_mode="append",
            supported_port_profiles=("message", "json", "text", "event"),
            default_port_profile="message",
            supported_formats=formats,
        )
    if event_log is not None:
        graph.register_binding_target(
            "event-log",
            label="Event Log",
            target_type="event_log",
            widget_type="log_view",
            widget=event_log,
            supported_update_modes=("append", "set"),
            default_update_mode="append",
            supported_port_profiles=("event", "json", "text", "message", "terminal_output"),
            default_port_profile="event",
            supported_formats=formats,
        )
    for action_id, label in (
        ("run-section-action-button", "Run Section Action Button"),
        ("run-selection-button", "Run Selection Button"),
        ("send-prompt-button", "Send Prompt Button"),
        ("clear-logs-button", "Clear Logs Button"),
        ("snapshot-button", "Snapshot Button"),
    ):
        graph.register_binding_target(
            action_id,
            label=label,
            target_type="button",
            action_type="button",
            callback=binding_action,
            supported_commands=("run", "stop", "reset", "replay"),
            default_command="run",
        )
    for action_id, label in ACTION_SLOT_TARGETS:
        graph.register_binding_target(
            action_id,
            label=label,
            target_type="button",
            action_type="button",
            callback=action_slot_callback,
            supported_commands=("run", "stop", "reset", "replay"),
            default_command="run",
        )
    dump_bindings()
    refresh_runtime_status()


with dg.HLayout(class_="root"):
    with dg.Panel("Blank Node Editor", class_="canvas"):
        with dg.FlowLayout(gap=8, row_gap=6, style={"width": "100%", "height": "auto", "flex_shrink": 0}):
            dg.Label("Binding Playground", class_="section")
            selection_label = dg.Label("Selected: none", class_="status", style={"width": 180})
            counts_label = dg.Label("Graph: loading", class_="status", style={"width": 230})
            status_label = dg.Label("Ready", class_="status", style={"width": 330})
            dg.Tag("blank graph", level="neutral")
            dg.Tag("real GUI targets", level="success")
            dg.Tag("double-click to add", level="warning")
        graph = dg.NodeGraph(
            [],
            [],
            sections=[],
            templates=TEMPLATES,
            on_graph_event=on_graph_event,
            on_node_select=on_select,
            on_node_move=on_move,
            enable_zoom=True,
            show_port_labels=True,
            show_subtitles=True,
            width=1000,
            height=680,
            class_="node-graph",
        )

    with dg.Panel("Bindable GUI Controls", class_="side"):
        dg.Label("How to test", class_="section")
        dg.Label(
            "Double-click the blank editor to add Widget Source, Widget Sink, Text Input, Parser, "
            "Terminal, or Section nodes. The Run buttons create/clean up runtime automatically.",
            class_="muted",
        )
        dg.Label("Runtime Status", class_="section")
        runtime_status_label = dg.Label("Runtime: idle", class_="status")
        dg.Label("Text Sources", class_="section")
        prompt_input = dg.TextInput(
            "echo hello from prompt input",
            id="prompt-input",
            placeholder="Prompt text source...",
            on_change=lambda value: set_status(f"Prompt Input changed: {value}"),
            class_="mini-field",
        )
        review_input = dg.TextInput(
            "Please review the implementation.",
            id="review-input",
            placeholder="Reviewer prompt source...",
            on_change=lambda value: set_status(f"Reviewer Prompt changed: {value}"),
            class_="mini-field",
        )
        scratch_input = dg.TextInput(
            "{\"kind\": \"scratch\", \"ok\": true}",
            id="scratch-input",
            placeholder="Scratch JSON/text source...",
            on_change=lambda value: set_status(f"Scratch changed: {value}"),
            class_="mini-field",
        )
        dg.Label("Action Buttons", class_="section")
        with dg.FlowLayout(gap=6, row_gap=6):
            dg.Button("Run Section", on_click=run_selected_section_runtime)
            dg.Button("Run Node", on_click=run_selected_node_runtime)
            dg.Button("Cleanup Runtime", on_click=cleanup_runtime)
            dg.Button("Action Slot A", on_click=lambda: run_action_slot("action-slot-a"))
            dg.Button("Action Slot B", on_click=lambda: run_action_slot("action-slot-b"))
            dg.Button("Action Slot C", on_click=lambda: run_action_slot("action-slot-c"))
            dg.Button("Action Slot D", on_click=lambda: run_action_slot("action-slot-d"))
            dg.Button("Run Selection", on_click=lambda: binding_action("run-selection-button", "run"))
            dg.Button("Send Prompt", on_click=lambda: binding_action("send-prompt-button", "run"))
            dg.Button("Append Samples", on_click=append_sample_outputs)
            dg.Button("Snapshot", on_click=snapshot_graph)
            dg.Button("Dump Bindings", on_click=dump_bindings)
            dg.Button("Clear Logs", on_click=clear_logs)
            dg.Button("Reset Graph", on_click=reset_blank_graph)
        dg.Label("Main Output Log", class_="section")
        main_output_log = dg.LogView(
            ["Main output log ready."],
            id="main-output-log",
            rows=4,
            follow=True,
            wrap=True,
            class_="play-log",
        )
        dg.Label("Reviewer Output Log", class_="section")
        review_output_log = dg.LogView(
            ["Reviewer output log ready."],
            id="review-output-log",
            rows=4,
            follow=True,
            wrap=True,
            class_="play-log",
        )
        dg.Label("Parsed Message Log", class_="section")
        message_log = dg.LogView(
            ["Parsed message log ready."],
            id="message-log",
            rows=4,
            follow=True,
            wrap=True,
            class_="play-log",
        )
        dg.Label("Event Log", class_="section")
        event_log = dg.LogView(
            ["Binding playground ready."],
            id="event-log",
            rows=6,
            follow=True,
            wrap=True,
            class_="play-log",
        )
        register_binding_targets()

refresh_counts()
refresh_selection()
refresh_runtime_status()

if __name__ == "__main__":
    print(app.run(win))
