# Nodal System Plan

## Objective

Build DragonGUI's `NodeGraph` from a useful visual prototype into a complete
node editor and, eventually, a nodal orchestration system for agent workflows,
terminal processes, message routing, approval gates, testing loops, and artifact
capture.

The near-term goal is a polished editor widget that can create, edit, persist,
and report graph changes. The longer-term goal is a reusable nodal system where
nodes are not only drawings, but typed workflow components with runtime state.

## Current Status

Implemented in the current prototype:

- `NodeGraph`, `NodeGraphNode`, `NodeGraphPort`, and `NodeGraphEdge` Python API.
- Canvas-backed graph rendering inside `HtmlReport`.
- Node dragging by header.
- Background panning.
- Mouse-wheel zoom.
- Automatic node width based on title, subtitles, status, and pin labels.
- Visible input and output pin labels.
- Tuned node header height, selection outline, and pin-label spacing.
- Output-to-input edge creation by dragging between pins.
- Edge selection.
- Node body/header selection.
- Delete or Backspace removes the selected node or edge.
- Double-clicking empty canvas adds a basic node.
- Ctrl+D duplicates the selected node.
- Escape clears selection or cancels the active drag.
- Focused Python tests verify the widget serializes as an interactive canvas
  editor and exposes the expected editing functions.
- `examples/node_graph_editor_probe.py` demonstrates the current editor.

Known limitations:

- Edits currently live inside the canvas runtime and are not synchronized back to
  Python.
- There is no save/load format yet.
- There is no undo/redo stack.
- New nodes are generic and not created from a palette.
- Ports are untyped, so connection validation is minimal.
- There is no side panel for editing node or port properties.
- Existing Python callbacks passed to `NodeGraph` are accepted for compatibility
  but not wired to canvas-side edits yet.
- The current implementation is canvas-backed rather than native Rust-rendered.

## Immediate Next Slice

### 1. Persistence

Add graph import/export so users can keep edits across sessions.

Required pieces:

- `NodeGraph.to_graph_data()` returning a serializable schema.
- `NodeGraph.from_graph_data(...)` or `set_graph_data(...)`.
- Versioned JSON schema with `schema_version`.
- Nodes with IDs, titles, positions, width hints, colors, statuses, inputs,
  outputs, and custom data.
- Edges with IDs, source node/port, target node/port, label, color, and custom
  data.
- Example save/load buttons in the probe or a dedicated example.

### 2. Python Event Bridge

Canvas edits need to become DragonGUI events.

Events to report:

- `node_selected`
- `edge_selected`
- `node_moved`
- `node_created`
- `node_deleted`
- `node_duplicated`
- `edge_created`
- `edge_deleted`
- `graph_changed`

Implementation questions:

- Decide whether `HtmlReport` can emit structured events directly.
- If not, add a small WebView-to-Python event bridge for trusted local widget
  scripts.
- Keep events compact and schema-stable.
- Let Python callbacks update labels, logs, side panels, and saved graph state.

### 3. Editor State API

Expose a clear Python-side API for graph state updates.

Candidate methods:

```python
graph.set_graph_data(data)
graph.graph_data()
graph.add_node(node)
graph.remove_node(node_id)
graph.update_node(node_id, **fields)
graph.add_edge(edge)
graph.remove_edge(edge_id)
graph.select_node(node_id | None)
graph.select_edge(edge_id | None)
graph.fit_to_view()
```

## Core Editing Roadmap

### Selection

- Multi-select nodes with Shift-click.
- Box selection by dragging empty canvas with a modifier.
- Select all with Ctrl+A.
- Clear selection with Escape.
- Selection handles or clear visual affordances for selected edges.

### Creation

- Replace generic double-click node creation with a node palette.
- Add context menu on canvas.
- Add context menu on nodes and ports.
- Add node templates for common agent workflow components.

### Movement And Layout

- Snap nodes to grid.
- Optional alignment guides.
- Keyboard nudging with arrow keys.
- Fit graph to view.
- Center selected node.
- Basic auto-layout for workflow graphs.

### Connections

- Drag from output pin to input pin to create edges. Already implemented.
- Allow dragging an existing edge endpoint to reroute it.
- Highlight compatible target pins while dragging.
- Prevent invalid duplicate edges. Basic duplicate prevention exists.
- Define per-port connection limits.
- Define whether cycles are allowed per graph mode.

### Editing

- Rename nodes.
- Rename ports.
- Add/remove ports.
- Edit node color and status.
- Edit edge label and color.
- Duplicate selected subgraphs, not only one node.
- Copy/paste selected nodes and edges.

### History

- Undo/redo for node movement, creation, deletion, duplication, and edge edits.
- Batch drag operations into a single undo entry.
- Keep history local to the graph widget but expose dirty state to Python.

## Runtime-Oriented Nodal System

The editor should stay general, but the multi-agent use case needs richer node
semantics.

Candidate node types:

- Agent node: command, model, role, prompt, working directory, environment.
- Terminal node: process session, stdin, stdout, stderr, exit status.
- Parser node: message framing, routing tags, JSON extraction, transcript scan.
- Approval gate node: manual approval, policy approval, blocked state.
- Tester node: command runner, test request queue, report output.
- Artifact node: transcript, file references, markdown summaries, JSONL logs.
- Human input node: prompts the user and routes the answer.
- Rule node: conditional routing based on message type, status, or content.

Runtime states:

- `idle`
- `running`
- `waiting`
- `needs_user`
- `blocked`
- `failed`
- `done`

Runtime features:

- Start/stop/restart node runtime.
- Per-node logs.
- Per-edge message counters.
- Live edge pulses or activity markers.
- Record graph execution as a replayable transcript.
- Route outputs from one terminal/agent to another through typed messages.

## Terminal-Backed Multi-Agent Requirements

The first real application for the nodal system should be a graph-driven
multi-agent terminal cockpit. In that mode, graph nodes are connected to live
terminal sessions running tools such as Codex CLI, Claude Code, shells, test
runners, parsers, and approval gates.

This needs more than the generic editor features above.

### Terminal Control Surface

The current `Terminal` widget is a visual PTY wrapper. Multi-agent orchestration
also needs a programmatic control layer.

Required capabilities:

- Start, stop, restart, and dispose terminal sessions from Python.
- Send text or key sequences to a session without requiring manual typing.
- Subscribe to stdout/stderr/PTY output as a structured stream.
- Keep an append-only transcript with timestamps and session IDs.
- Track process state: starting, running, exited, crashed, disconnected.
- Expose exit code and last activity time.
- Support command, args, cwd, environment, title, and startup prompt metadata.
- Allow terminal sessions to be hidden, shown, or opened in a detail pane.

Likely `TerminalBridge` work:

- Add output callbacks or an event queue.
- Add a safe `send_text(...)` / `send_bytes(...)` API.
- Add transcript recording separate from xterm rendering.
- Add process lifecycle events.
- Keep PTY output capture independent from the browser terminal surface, so the
  router does not have to scrape rendered xterm text.

### Agent Session Model

A graph node should bind to an agent session, not directly to a widget instance.

A session record should include:

- `session_id`
- node ID owning the session
- agent type, such as `codex`, `claude`, `shell`, `tester`, or `parser`
- command, args, cwd, environment, and startup state
- current status and status reason
- transcript cursor positions used by parsers
- capabilities, such as can edit files, can run tests, can request approval
- safety policy, such as requires approval before sending messages or commands

This keeps the graph, terminal widget, and runtime orchestration decoupled.

### Message Envelope Protocol

Agents need a reliable way to talk through terminals. The plan mentions parser
nodes, but the protocol itself should be specified.

A first envelope can stay text-based so it works inside Codex/Claude terminals:

```text
@to reviewer
@from implementer
@type review_request
@id DG-142-R2
@reply_to DG-142
Please review the latest patch and report risks.
@end
```

Required fields:

- `to`
- `from`
- `type`
- `id`
- body

Useful optional fields:

- `reply_to`
- `priority`
- `requires_approval`
- `timeout_seconds`
- `artifact_refs`
- `thread_id`

Parser requirements:

- Incremental parsing from a growing terminal transcript.
- Recovery from malformed or partial envelopes.
- Deduplication by message ID.
- Clear error events for rejected messages.
- Ability to parse multiple envelopes from one output chunk.
- Configurable message markers in case a specific CLI or prompt needs a
  different format.

### Router And Queue Semantics

The graph should not directly shove text from one terminal into another. It
needs a message bus between terminal output and terminal input.

Required router behavior:

- Queue messages by target agent or node.
- Route messages based on graph edges and message type.
- Support manual approval gates before forwarding.
- Support paused routing.
- Track delivery state: queued, held, delivered, failed, expired.
- Avoid loops with message IDs, hop counts, or graph policy.
- Preserve ordering where needed, especially within a thread.
- Allow replaying or resending a held/failed message.
- Show pending queues in the UI.

### Human Approval And Intervention

The multi-agent system should assume a human operator stays in control.

Required controls:

- Approve/reject held messages.
- Edit a message before forwarding.
- Inject a message into any agent terminal.
- Pause/resume the router.
- Pause/resume an individual agent session.
- Kill/restart a misbehaving terminal process.
- Mark a message or session as resolved.

### Workspace And Safety Model

Multiple coding agents may operate on the same files, so the plan needs a safety
model before this becomes more than a demo.

Questions to answer early:

- Do agents share one working directory or get separate worktrees?
- Can reviewer/tester agents write files, or are they read-only by policy?
- Which commands require human approval?
- How are secrets and environment variables redacted from transcripts?
- How are destructive commands detected or blocked?
- How are terminal transcripts stored if they may contain private data?

Initial recommendation:

- Let the implementer own writes.
- Keep reviewer/tester sessions read-oriented by default.
- Add explicit approval before routing commands that ask another agent to mutate
  files or run broad/destructive commands.
- Record all routed messages and operator approvals.

### Graph-To-Runtime Binding

The editor graph needs a runtime binding layer.

Needed concepts:

- A saved graph is a template.
- A running graph instance owns sessions, queues, transcripts, and runtime state.
- Node IDs in the template map to session IDs in the running instance.
- Runtime state should be saveable separately from graph layout.
- The UI should show whether a node is only configured, currently running, or
  disconnected from its session.

### UI Shape For The First App

The node editor alone is not enough. The cockpit needs coordinated views.

Likely first layout:

- Main graph canvas for orchestration.
- Inspector panel for the selected node, edge, session, or message.
- Terminal detail panel for the selected agent node.
- Message queue panel for held and pending messages.
- Transcript/artifact panel for graph-level history.
- Toolbar for run, pause, save, load, fit, approve all, and emergency stop.

### Observability And Replay

Debugging multi-agent systems requires a good event log.

Record:

- Terminal session lifecycle events.
- Transcript chunks with timestamps.
- Parsed envelopes.
- Routing decisions.
- Approval decisions.
- Delivered input text.
- Node status transitions.
- Errors and parser failures.

Export formats:

- JSONL event log.
- Markdown transcript summary.
- Graph JSON snapshot.
- Artifact manifest.

### CLI-Specific Notes

Codex CLI and Claude Code are interactive tools. The terminal wrapper should
avoid relying on fragile prompt scraping when possible.

Potential issues to handle:

- ANSI escape sequences and alternate-screen terminal UIs.
- Bracketed paste behavior.
- Prompts that repaint in place.
- Long-running operations with streaming output.
- Terminal resize events.
- Login/auth flows that must remain manual.
- Different conventions for accepting pasted multi-line messages.

The router should treat CLI sessions as opaque terminals plus transcript streams,
then layer message parsing on top of captured PTY output.
## Port And Edge Typing

Ports should eventually carry types so the editor can prevent bad connections.

Potential port types:

- `text`
- `message`
- `terminal_input`
- `terminal_output`
- `approval_request`
- `approval_result`
- `test_request`
- `test_report`
- `artifact`
- `error`

Connection validation should check:

- Source is an output and target is an input.
- Source and target types are compatible.
- Target accepts another incoming edge.
- Graph mode allows or disallows cycles.

## UX Polish

- Better edge hit targets and selected-edge affordance.
- Minimap for large graphs.
- Toolbar buttons for zoom in, zoom out, fit, undo, redo, save, load.
- Keyboard shortcut reference in docs rather than visible in-app instructions.
- Optional grid visibility and snap toggle.
- Node icons or badges.
- Compact status indicators in node headers.
- Better empty-state behavior.

## Testing Plan

Python tests:

- Serialization and schema round-trip.
- Data validation for nodes, ports, and edges.
- Graph mutation API methods.
- Event payload shape once the bridge exists.

Browser/canvas behavior tests, if available later:

- Drag node updates graph state.
- Drag output to input creates an edge.
- Delete selected node removes attached edges.
- Undo/redo restores graph state.
- Fit-to-view computes sensible viewport values.

Manual probe checks:

- Node dragging feels stable at multiple zoom levels.
- Pin labels align and remain readable.
- Edge creation is obvious and forgiving.
- Selection borders and edge highlights are visually clean.
- New nodes can be created without disturbing existing layout.

## Open Questions

- Should `NodeGraph` remain canvas-backed long term, or should mature behavior be
  ported into a native Rust-rendered widget?
- Should graph editing state be owned by JavaScript first, Python first, or a
  synchronized shared model?
- How much of the agent runtime belongs in DragonGUI versus in an application
  layer built on DragonGUI?
- Should node templates be plain Python dataclasses, JSON schema objects, or
  registered component classes?
- How strict should graph validation be by default?

## Suggested Implementation Order

1. Add graph data export/import and schema versioning.
2. Add Python event bridge for canvas-side graph changes.
3. Add terminal session control APIs: send input, capture output, lifecycle events.
4. Add an agent session model and transcript/event log.
5. Add the first message envelope parser and router queue.
6. Add undo/redo history.
7. Add a simple node palette and property editor.
8. Add typed ports and connection validation.
9. Add runtime-oriented node templates for the multi-agent workflow app.
10. Add layout tools, minimap, and larger graph navigation polish.

