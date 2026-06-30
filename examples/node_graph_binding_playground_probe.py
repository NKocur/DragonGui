"""Blank NodeGraph binding playground probe.

A small sandbox for testing the shared GUI binding registry without the larger
runtime demo graph. Start with an empty node editor, add generic nodes from the
canvas palette, then assign them to the real controls on the right.
"""

from __future__ import annotations

import json
import os
import sys
from collections.abc import Mapping
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


TEMPLATES = dg.multi_agent_node_templates()
NO_GRAPH = os.environ.get("DRAGONGUI_BINDING_PROBE_NO_GRAPH", "").strip().lower() in {
    "1",
    "true",
    "yes",
    "on",
}
ACTION_BUTTON_TARGETS = (
    ("run-section-action-button", "Run Section Action Button"),
    ("run-selection-button", "Run Selection Button"),
    ("send-prompt-button", "Send Prompt Button"),
    ("clear-logs-button", "Clear Logs Button"),
    ("snapshot-button", "Snapshot Button"),
)

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
        min-width: 0;
        min-height: 0;
        gap: 10px;
    }
    Tabs.root {
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 0;
    }
    Tab.editor-tab, Tab.objects-tab, Tab.terminal-tab {
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 0;
    }
    VLayout.editor-page {
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 0;
        gap: 0;
    }
    Panel.canvas {
        width: 100%;
        flex-grow: 1;
        min-width: 0;
        min-height: 0;
        height: 100%;
        padding: 10px;
        gap: 8px;
    }
    Panel.side {
        width: 100%;
        min-width: 0;
        max-width: none;
        height: 100%;
        min-height: 0;
        overflow-y: auto;
        overflow-x: hidden;
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
    HtmlReport.play-terminal {
        width: 100%;
        height: 280px;
        min-height: 220px;
        flex-shrink: 0;
        border: 1px solid rgba(255, 255, 255, 0.11);
        border-radius: 6px;
        background: #070a0f;
    }
    Button {
        padding: 7px 9px;
    }
    """
)

win = dg.Window(
    "NodeGraph Binding Playground [NO GRAPH]" if NO_GRAPH else "NodeGraph Binding Playground",
    width=1480,
    height=860,
)

graph: dg.NodeGraph | None = None
runtime_status_label: dg.Label | None = None
target_status_label: dg.Label | None = None
selected_section_id: str | None = None
prompt_input: dg.TextInput | None = None
review_input: dg.TextInput | None = None
scratch_input: dg.TextInput | None = None
main_output_log: dg.LogView | None = None
review_output_log: dg.LogView | None = None
message_log: dg.LogView | None = None
event_log: dg.LogView | None = None
playground_terminal: dg.Terminal | None = None
action_slot_buttons: dict[str, dg.Button] = {}


def log(line: object = "") -> None:
    if event_log is not None:
        event_log.append_line(str(line))


def set_status(text: str) -> None:
    log(text)


def on_playground_tab_change(value: str) -> None:
    log(f"tab changed: {value}")
    refresh_target_status()


def refresh_counts() -> None:
    return


def refresh_selection() -> None:
    return


def refresh_runtime_status() -> None:
    if graph is None or runtime_status_label is None:
        return
    runtime_status_label.text = graph.managed_runtime_status_text()


def refresh_target_status() -> None:
    if graph is None or target_status_label is None:
        return
    action_ids = graph.action_target_ids()
    widget_ids = graph.widget_target_ids()
    target_status_label.text = (
        f"Discovered Actions: {len(action_ids)} | Widgets: {len(widget_ids)}"
    )


def force_refresh_targets() -> None:
    if graph is None:
        return
    graph.refresh_binding_targets_from_host(win)
    refresh_target_status()
    log(f"refreshed action targets: {', '.join(graph.action_target_ids())}")
    set_status("binding targets refreshed")


def on_select(node_id: str) -> None:
    global selected_section_id
    selected_section_id = None
    refresh_selection()
    set_status(f"selected node {node_id}")


def on_move(node_id: str, x: float, y: float) -> None:
    log(f"Moved {node_id}: {x:.0f}, {y:.0f}")


def on_graph_event(event: dict[str, object]) -> None:
    global selected_section_id
    event_name = str(event.get("event", "event"))
    if event_name in {
        "node_created",
        "node_duplicated",
        "node_deleted",
        "edge_created",
        "edge_deleted",
        "section_created",
        "section_deleted",
        "property_editor_saved",
        "graph_changed",
    }:
        refresh_counts()
    if event_name == "section_selected":
        selected_section_id = str(event.get("section") or "") or None
    elif event_name in {"node_selected", "selection_cleared"}:
        selected_section_id = None
    if event_name in {"node_selected", "section_selected", "selection_cleared"}:
        refresh_selection()
    if event_name == "editor_action":
        action = str(event.get("action", ""))
        if action == "run_node":
            run_selected_node_runtime()
        elif action == "run_section":
            run_selected_section_runtime()
        elif action == "cleanup_runtime":
            cleanup_runtime()
        elif action == "refresh_targets":
            force_refresh_targets()
        elif action == "snapshot":
            snapshot_graph()
        elif action == "reset_graph":
            reset_blank_graph()
        else:
            set_status(f"editor action ignored: {action}")
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


def log_section_runtime_diagnostic(section: dg.NodeGraphSection) -> None:
    if graph is None:
        return
    try:
        node_ids = graph.section_nodes(section.id)
    except Exception as exc:
        node_ids = ()
        log(f"section {section.id} node lookup failed: {exc}")
    binding = graph.runtime_binding()
    section_binding = binding.section_binding(section.id)
    terminal_nodes: list[dict[str, object]] = []
    for node in graph.nodes:
        data = node.data or {}
        if not isinstance(data, Mapping):
            continue
        if str(data.get("node_type", data.get("template_id", ""))) != "terminal":
            continue
        config = data.get("config") if isinstance(data.get("config"), Mapping) else {}
        terminal_nodes.append(
            {
                "id": node.id,
                "in_section": node.id in node_ids,
                "inputs": [port.id for port in node.inputs],
                "session_id": str(config.get("session_id", "")),
                "terminal_widget_id": str(config.get("terminal_widget_id", "")),
            }
        )
    log(
        "section diagnostic "
        + json.dumps(
            {
                "section": section.id,
                "section_nodes": list(node_ids),
                "binding_nodes": list(section_binding.node_ids) if section_binding else [],
                "terminal_nodes": terminal_nodes,
                "widget_targets": list(graph.widget_target_ids()),
            },
            sort_keys=True,
        )
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
        log_section_runtime_diagnostic(section)
        try:
            event = graph.run_section_runtime(section.id, command_s)
        except Exception as exc:
            log(f"action slot {action_id} section {section.id} failed: {exc}")
            continue
        events.append(event.event)
        log(
            "section result "
            + json.dumps(
                {
                    "action_id": action_id,
                    "section": section.id,
                    "command": command_s,
                    "event": event.event,
                    "data": event.data or {},
                },
                sort_keys=True,
            )
        )
        log_runtime_tail()
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


def send_direct_terminal_test() -> None:
    if playground_terminal is None:
        set_status("direct terminal test skipped: no terminal widget")
        return
    delivered = playground_terminal.send_line("echo direct terminal widget test")
    log(f"direct terminal widget send delivered={delivered}")
    set_status(f"direct terminal widget send delivered={delivered}")


def clear_logs() -> None:
    for target in (main_output_log, review_output_log, message_log, event_log):
        if target is not None:
            target.clear()
    set_status("logs cleared")


def dump_bindings() -> None:
    if graph is None:
        return
    graph.refresh_binding_targets_from_host(win)
    payload = {
        "binding_targets": graph.binding_target_ids(),
        "widget_targets": graph.widget_target_ids(),
        "action_targets": graph.action_target_ids(),
    }
    log(json.dumps(payload, sort_keys=True))
    set_status("binding registry dumped to event log")


def log_runtime_tail() -> None:
    if graph is None or graph.managed_runtime is None:
        return
    events = graph.managed_runtime.snapshot().get("events", [])
    tail = events[-6:] if isinstance(events, list) else []
    for event in tail:
        log(f"runtime event {json.dumps(event, sort_keys=True)}")


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
    if playground_terminal is not None:
        graph.register_binding_target(
            "playground-terminal",
            label="Playground Terminal",
            target_type="terminal",
            widget_type="terminal",
            widget=playground_terminal,
            supported_update_modes=("append", "set"),
            default_update_mode="append",
            supported_port_profiles=("terminal_output", "text"),
            default_port_profile="terminal_output",
            supported_formats=("terminal_text", "text", "repr"),
            data={"runtime_object_id": "playground-terminal"},
        )
    for action_id, label in ACTION_BUTTON_TARGETS:
        graph.register_binding_target(
            action_id,
            label=label,
            target_type="button",
            action_type="button",
            callback=binding_action,
            supported_commands=("run", "stop", "reset", "replay"),
            default_command="run",
        )
    for action_id, fallback_label in ACTION_SLOT_TARGETS:
        button = action_slot_buttons.get(action_id)
        graph.register_binding_target(
            action_id,
            label=button.text if button is not None else fallback_label,
            target_type="button",
            action_type="button",
            callback=action_slot_callback,
            supported_commands=("run", "stop", "reset", "replay"),
            default_command="run",
        )
    dump_bindings()
    refresh_runtime_status()


with dg.Tabs(value="objects", id="binding-playground-tabs", class_="root", on_change=on_playground_tab_change):
    with dg.Tab("Node Editor", value="editor", class_="editor-tab"):
        with dg.VLayout(class_="editor-page"):
            with dg.Panel("Blank Node Editor", class_="canvas"):
                if NO_GRAPH:
                    dg.Label("NodeGraph disabled for layout isolation", class_="section")
                    dg.Label(
                        "Set DRAGONGUI_BINDING_PROBE_NO_GRAPH=0 or unset it to restore the editor.",
                        class_="muted",
                    )
                    dg.Button("Native placeholder button")
                    dg.LogView(
                        ["No HtmlReport/WebView was created in this run."],
                        rows=6,
                        follow=True,
                        wrap=True,
                        class_="play-log",
                    )
                else:
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
                        show_editor_chrome=True,
                        editor_title="Binding Playground",
                        editor_actions=(
                            {"id": "run_node", "label": "Run node", "icon": "Node", "wide": True, "separator_before": True},
                            {"id": "run_section", "label": "Run section", "icon": "Section", "wide": True},
                            {"id": "cleanup_runtime", "label": "Cleanup runtime", "icon": "X", "separator_before": True},
                            {"id": "refresh_targets", "label": "Refresh targets", "icon": "Refresh", "wide": True},
                            {"id": "snapshot", "label": "Snapshot", "icon": "S", "separator_before": True},
                            {"id": "reset_graph", "label": "Reset graph", "icon": "Reset", "wide": True},
                        ),
                        width=1000,
                        height=680,
                        class_="node-graph",
                    )
        

    with dg.Tab("GUI Objects", value="objects", class_="objects-tab"):
        with dg.Panel("Bindable GUI Controls", class_="side"):
            dg.Label("How to test", class_="section")
            dg.Label(
                "Add Widget Source, Widget Sink, Text Input, Parser, Terminal, or Section nodes in the editor tab, "
                "then assign them to these real GUI targets.",
                class_="muted",
            )
            dg.Label("Runtime Status", class_="section")
            runtime_status_label = dg.Label("Runtime: idle", class_="status")
            target_status_label = dg.Label("Discovered Actions: loading", class_="status")
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
                dg.Button("Run Section Target", id="run-section-action-button", on_click=lambda: binding_action("run-section-action-button", "run"))
                action_slot_buttons["action-slot-a"] = dg.Button("Action Slot A", id="action-slot-a", on_click=lambda: run_action_slot("action-slot-a"))
                action_slot_buttons["action-slot-b"] = dg.Button("Action Slot B", id="action-slot-b", on_click=lambda: run_action_slot("action-slot-b"))
                action_slot_buttons["action-slot-c"] = dg.Button("Action Slot C", id="action-slot-c", on_click=lambda: run_action_slot("action-slot-c"))
                action_slot_buttons["action-slot-d"] = dg.Button("Action Slot D", id="action-slot-d", on_click=lambda: run_action_slot("action-slot-d"))
                dg.Button("Run Selection", id="run-selection-button", on_click=lambda: binding_action("run-selection-button", "run"))
                dg.Button("Send Prompt", id="send-prompt-button", on_click=lambda: binding_action("send-prompt-button", "run"))
                dg.Button("Direct Terminal Test", id="direct-terminal-test-button", on_click=send_direct_terminal_test)
                dg.Button("Append Samples", id="append-samples-button", on_click=append_sample_outputs)
                dg.Button("Snapshot Target", id="snapshot-button", on_click=lambda: binding_action("snapshot-button", "run"))
                dg.Button("Dump Bindings", id="dump-bindings-button", on_click=dump_bindings)
                dg.Button("Clear Logs", id="clear-logs-button", on_click=clear_logs)
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
    

    with dg.Tab("Terminal", value="terminal", class_="terminal-tab"):
        with dg.Panel("Terminal Target", class_="side"):
            dg.Label("Terminal Widget", class_="section")
            dg.Label(
                "This WebView-backed terminal is isolated from the native GUI controls tab for local layering diagnostics.",
                class_="muted",
            )
            playground_terminal = dg.Terminal(
                "cmd.exe",
                id="playground-terminal",
                title="Playground Terminal",
                prefer_pty=True,
                height=420,
                class_="play-terminal",
            )

register_binding_targets()
refresh_counts()
refresh_selection()
refresh_runtime_status()
if graph is not None:
    graph.refresh_binding_targets_from_host(win)
    log(f"startup action targets: {', '.join(graph.action_target_ids())}")
refresh_target_status()

if __name__ == "__main__":
    print(app.run(win))

