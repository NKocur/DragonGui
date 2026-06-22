"""Static mockup for a modular multi-agent terminal cockpit.

This sketch shows the UI shape for a future DragonGUI workflow where any number
of PTY-backed agent terminals can exchange structured messages through a parser
and routing queue. It intentionally uses fake terminal/log content so the
interface can be reviewed before real agent orchestration is wired in.
"""

from __future__ import annotations

from dataclasses import dataclass
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


@dataclass(frozen=True)
class AgentSpec:
    id: str
    name: str
    role: str
    command: str
    cwd: str
    status: str
    level: str
    tags: tuple[str, ...]
    lines: tuple[str, ...]


AGENTS = (
    AgentSpec(
        id="implementer",
        name="Implementer",
        role="Makes code changes and prepares review requests.",
        command="codex --sandbox workspace-write",
        cwd="C:\\workspace\\demo",
        status="writing",
        level="success",
        tags=("code", "diff", "handoff"),
        lines=(
            "PS C:\\workspace\\demo> codex",
            "codex> role: implementer",
            "codex> reading task ticket DG-142",
            "INFO  changed files: python/dragongui/terminal.py, tests/test_python_api.py",
            "INFO  ran focused tests: 2 passed",
            "@to reviewer",
            "@type review_request",
            "@id DG-142-R1",
            "Please review the terminal bridge startup ordering and xterm repaint path.",
            "@end",
        ),
    ),
    AgentSpec(
        id="reviewer",
        name="Reviewer",
        role="Reviews diffs for regressions, edge cases, and missing tests.",
        command="codex --review",
        cwd="C:\\workspace\\demo",
        status="idle",
        level="info",
        tags=("review", "risk", "tests"),
        lines=(
            "PS C:\\workspace\\demo> codex",
            "codex> role: reviewer",
            "codex> waiting for routed review_request messages",
            "INFO  policy: prioritize regressions, missing tests, and edge cases",
            "INFO  subscribed channels: reviewer, user, all",
            "READY idle",
        ),
    ),
    AgentSpec(
        id="tester",
        name="Tester",
        role="Runs focused checks and reports failures back to the implementer.",
        command="codex --task tests",
        cwd="C:\\workspace\\demo",
        status="ready",
        level="warning",
        tags=("pytest", "ci", "failure_report"),
        lines=(
            "PS C:\\workspace\\demo> codex",
            "codex> role: tester",
            "INFO  subscribed channels: tester, all",
            "READY waiting for @type test_request",
        ),
    ),
    AgentSpec(
        id="planner",
        name="Planner",
        role="Breaks vague goals into routed tasks and keeps scope from drifting.",
        command="codex --task planning",
        cwd="C:\\workspace\\demo",
        status="watching",
        level="neutral",
        tags=("plan", "scope", "blocked"),
        lines=(
            "PS C:\\workspace\\demo> codex",
            "codex> role: planner",
            "INFO  tracking open envelopes and operator decisions",
            "READY watching transcript",
        ),
    ),
)

ROUTER_LINES = (
    "09:18:22 parse implementer stdout -> envelope DG-142-R1",
    "09:18:22 route candidate: implementer -> reviewer",
    "09:18:22 held for approval: review_request",
    "09:18:23 safety scan: no secrets detected",
    "09:18:23 waiting for operator approval",
)

TRANSCRIPT_LINES = (
    "user -> implementer: Build a terminal wrapper for Codex/Claude CLI.",
    "implementer -> reviewer: DG-142-R1 review_request queued.",
    "router: message held because manual approval is enabled.",
)


app = dg.App(theme=dg.Theme.dark(accent="#43c6ac", focus="#f8c14a", radius=6))
app.stylesheet(
    """
    Window {
        background: #101318;
        color: rgba(245, 248, 252, 0.94);
        padding: 12px;
        gap: 10px;
        font-size: 13px;
    }

    Splitter.root {
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 0;
    }

    VLayout.stack {
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 0;
        gap: 10px;
    }

    FlowLayout.toolbar,
    FlowLayout.badge-row,
    FlowLayout.actions {
        width: 100%;
        height: auto;
        min-height: 30px;
        gap: 6px;
        row-gap: 6px;
        flex-shrink: 0;
    }

    FlowLayout.agent-grid {
        width: 100%;
        flex-grow: 1;
        min-height: 0;
        gap: 10px;
        row-gap: 10px;
        overflow-y: auto;
        padding-right: 6px;
    }

    Panel {
        background: #171c24;
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 8px;
        padding: 10px;
        gap: 8px;
        min-width: 0;
        min-height: 0;
    }

    Panel.sidebar,
    Panel.queue {
        width: 100%;
        height: 100%;
        flex-shrink: 1;
        overflow-y: auto;
    }

    Panel.agent-card {
        width: calc(50% - 5px);
        min-width: 300px;
        height: 292px;
        padding: 8px;
        flex-shrink: 1;
    }

    Panel.bus {
        height: 250px;
        flex-shrink: 0;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
        height: 26px;
    }

    Label.section {
        color: rgba(245, 248, 252, 0.88);
        font-weight: 800;
        height: 20px;
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

    Badge,
    Tag {
        font-size: 12px;
        font-weight: 800;
        padding: 7px 8px;
    }

    Button {
        min-width: 74px;
        border-radius: 6px;
        font-weight: 800;
    }

    Button.primary {
        background: #43c6ac;
        color: #07110f;
    }

    Button.warn {
        border-color: rgba(248, 193, 74, 0.55);
        color: #ffe2a0;
    }

    Button.danger {
        border-color: rgba(255, 109, 121, 0.54);
        color: #ffc6cd;
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

    LogView.router {
        background: #0a0d13;
    }

    TextArea,
    CodeEditor,
    Dropdown {
        width: 100%;
        background: rgba(255, 255, 255, 0.055);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 6px;
        color: rgba(245, 248, 252, 0.92);
    }

    TextArea.message,
    CodeEditor.protocol {
        font-family: "Consolas";
        font-size: 12px;
        line-height: 18px;
    }

    Separator {
        background: rgba(255, 255, 255, 0.12);
    }
    """
)

win = dg.Window("Multi-Agent Cockpit Mockup", width=1480, height=860)

state = {"approved": 0, "paused": False, "pending_target": "reviewer", "pending_type": "review_request"}
agent_logs: dict[str, dg.LogView] = {}


def stamp(message: str) -> str:
    return f"09:18:{24 + state['approved']:02d} {message}"


def set_status(message: str) -> None:
    status.set_value(message)


def append_agent(agent_id: str, line: str) -> None:
    log = agent_logs.get(agent_id)
    if log is not None:
        log.append_line(line)


def approve_route() -> None:
    target = str(state["pending_target"])
    message_type = str(state["pending_type"])
    state["approved"] += 1
    route_count.set_value(str(state["approved"]))
    router_log.append_line(stamp(f"approved DG-142 -> {target} stdin"))
    append_agent(target, f"@from router DG-142 {message_type} received")
    transcript_log.append_line(f"router -> {target}: approved DG-142 {message_type}.")
    set_status(f"Approved one queued message for {target}")


def queue_review_request() -> None:
    state["pending_target"] = "reviewer"
    state["pending_type"] = "review_request"
    append_agent("implementer", "@to reviewer")
    append_agent("implementer", "@type review_request")
    append_agent("implementer", "@id DG-142-R2")
    append_agent("implementer", "Please review the latest parser contract mockup.")
    append_agent("implementer", "@end")
    router_log.append_line(stamp("parse implementer stdout -> envelope DG-142-R2"))
    router_log.append_line(stamp("held for approval: review_request -> reviewer"))
    set_status("Queued a fake implementer -> reviewer review_request")


def queue_test_request() -> None:
    state["pending_target"] = "tester"
    state["pending_type"] = "test_request"
    append_agent("reviewer", "@to tester")
    append_agent("reviewer", "@type test_request")
    append_agent("reviewer", "@id DG-142-T1")
    append_agent("reviewer", "Run the focused terminal wrapper tests and report failures.")
    append_agent("reviewer", "@end")
    router_log.append_line(stamp("parse reviewer stdout -> envelope DG-142-T1"))
    router_log.append_line(stamp("held for approval: test_request -> tester"))
    set_status("Queued a fake reviewer -> tester test_request")


def ping_agent(agent_id: str) -> None:
    append_agent(agent_id, "@from operator ping")
    router_log.append_line(stamp(f"operator ping queued for {agent_id}"))
    set_status(f"Pinged {agent_id}")


def toggle_pause() -> None:
    state["paused"] = not state["paused"]
    routing_badge.set_value("paused" if state["paused"] else "manual")
    routing_badge.set_level("warning" if state["paused"] else "info")
    router_log.append_line(stamp("routing paused" if state["paused"] else "routing resumed"))
    set_status("Routing paused" if state["paused"] else "Routing resumed in manual approval mode")


def clear_mock_logs() -> None:
    router_log.clear()
    transcript_log.clear()
    set_status("Cleared router and transcript mock logs")


def render_badges(spec: AgentSpec) -> None:
    with dg.FlowLayout(class_="badge-row", gap=6, row_gap=6):
        dg.Badge(spec.status, level=spec.level)
        dg.Tag(spec.command, level="neutral")
        for tag in spec.tags:
            dg.Tag(tag, level="neutral")


def render_agent_card(spec: AgentSpec) -> None:
    with dg.Panel(spec.name, class_="agent-card"):
        render_badges(spec)
        dg.Label(spec.role, class_="muted")
        agent_logs[spec.id] = dg.LogView(spec.lines, rows=8, follow=True, max_lines=500, wrap=False)
        with dg.FlowLayout(class_="actions", gap=6, row_gap=6):
            dg.Button("Ping", on_click=lambda agent_id=spec.id: ping_agent(agent_id))
            dg.Button("Focus", on_click=lambda name=spec.name: set_status(f"Selected {name}"))
            dg.Tag(f"cwd: {spec.cwd}", level="neutral")


def render_agent_grid(specs: tuple[AgentSpec, ...]) -> None:
    with dg.FlowLayout(class_="agent-grid", gap=10, row_gap=10):
        for spec in specs:
            render_agent_card(spec)


def render_sidebar(specs: tuple[AgentSpec, ...]) -> None:
    global routing_badge, route_count, status
    with dg.Panel("Workspace", class_="sidebar"):
        dg.Label("Agent Router Lab", class_="title")
        dg.Label("Mock interface for routing structured messages between terminal-backed coding agents.", class_="muted")
        with dg.FlowLayout(class_="badge-row", gap=6, row_gap=6):
            routing_badge = dg.Badge("manual", level="info")
            dg.Badge(f"{len(specs)} agents", level="success")
            route_count = dg.Badge("0", level="neutral")
        dg.Separator()
        dg.Label("Run Mode", class_="section")
        dg.Checkbox("Manual approval before forwarding", checked=True)
        dg.Checkbox("Pause on @needs user", checked=True)
        dg.Checkbox("Redact suspected secrets", checked=True)
        dg.Checkbox("Capture raw PTY transcript", checked=True)
        dg.Separator()
        dg.Label("Agent Roster", class_="section")
        for spec in specs:
            dg.Tag(f"{spec.name}: {spec.status}", level=spec.level)
        dg.Separator()
        dg.Label("Status", class_="section")
        status = dg.Label("Mock workspace loaded", class_="status")
        dg.ProgressBar(0.42, show_value=True)


def render_message_bus() -> None:
    global router_log, transcript_log
    with dg.Panel("Message Bus", class_="bus"):
        with dg.HLayout(style={"width": "100%", "height": "100%", "gap": 10, "min_width": 0, "min_height": 0}):
            with dg.VLayout(style={"width": "50%", "min_width": 0, "min_height": 0, "gap": 8}):
                dg.Label("Router Events", class_="section")
                router_log = dg.LogView(ROUTER_LINES, rows=8, follow=True, class_="router")
            with dg.VLayout(style={"width": "50%", "min_width": 0, "min_height": 0, "gap": 8}):
                dg.Label("Cross-Agent Transcript", class_="section")
                transcript_log = dg.LogView(TRANSCRIPT_LINES, rows=8, follow=True, wrap=True)


def render_queue_panel() -> None:
    with dg.Panel("Queue + Artifacts", class_="queue"):
        dg.Label("Pending Envelope", class_="section")
        dg.TextArea(
            "@to reviewer\n"
            "@from implementer\n"
            "@type review_request\n"
            "@id DG-142-R1\n"
            "Please review the terminal bridge startup ordering and xterm repaint path.\n"
            "@end",
            rows=7,
            wrap=True,
            class_="message",
        )
        with dg.FlowLayout(class_="actions", gap=6, row_gap=6):
            dg.Button("Approve", class_="primary", on_click=approve_route)
            dg.Button("Edit")
            dg.Button("Reject", class_="danger")
        dg.Separator()
        dg.Label("Routing Rules", class_="section")
        dg.CodeEditor(
            "review_request -> reviewer\n"
            "test_request -> tester\n"
            "request_changes -> implementer\n"
            "needs_user -> operator_queue\n"
            "done -> transcript + task_board",
            language="text",
            rows=6,
            wrap=False,
            disabled=True,
            class_="protocol",
        )
        dg.Separator()
        dg.Label("Task Board", class_="section")
        dg.Tag("DG-142  waiting review", level="warning")
        dg.Tag("DG-143  parser contract draft", level="info")
        dg.Tag("DG-144  transcript export", level="neutral")
        dg.Separator()
        dg.Label("Artifacts", class_="section")
        dg.LogView(
            (
                "review_findings.md        queued",
                "implementation_summary.md updated",
                "terminal_transcript.json  recording",
                "routing_events.jsonl      recording",
            ),
            rows=6,
            follow=False,
            wrap=False,
        )


with dg.Splitter(
    orientation="horizontal",
    sizes=(250, "1fr", 330),
    min_sizes=(220, 520, 280),
    gutter_size=8,
    class_="root",
):
    with dg.Pane(min_size=220):
        render_sidebar(AGENTS)

    with dg.Pane(min_size=520):
        with dg.VLayout(class_="stack"):
            with dg.FlowLayout(class_="toolbar", gap=6, row_gap=6):
                dg.Button("Approve Route", class_="primary", on_click=approve_route)
                dg.Button("Queue Review", on_click=queue_review_request)
                dg.Button("Queue Test", on_click=queue_test_request)
                dg.Button("Pause", class_="warn", on_click=toggle_pause)
                dg.Button("Clear", class_="danger", on_click=clear_mock_logs)
                dg.Tag("add agents by editing AGENTS", level="neutral")
            render_agent_grid(AGENTS)
            render_message_bus()

    with dg.Pane(min_size=280):
        render_queue_panel()


if __name__ == "__main__":
    print(app.run(win))