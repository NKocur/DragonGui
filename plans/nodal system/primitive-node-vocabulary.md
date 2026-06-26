# Primitive Node Vocabulary

## Purpose

The nodal system should stay flexible enough to build many agent workflows without hard-coding roles such as Implementer, Reviewer, or Tester into the core widget. Those roles should be templates, presets, or saved subgraphs assembled from smaller nodes.

The first useful target is a multi-agent terminal orchestration graph where terminal-backed CLIs such as Codex, Claude, PowerShell, or custom scripts can exchange structured messages. The node vocabulary should therefore focus on small, reusable units: terminal I/O, text parsing, text shaping, routing, gating, memory, and observability.

## Design Principle

Prefer generic capability nodes over role-specific nodes.

A `Reviewer` should be a saved graph or template like:

```text
Terminal(Codex reviewer)
  -> Extract Between Markers
  -> Envelope Parser
  -> Router
```

An `Implementer` should also be composition:

```text
Router
  -> Append Text(implementation instructions)
  -> Terminal(Codex implementer)
```

This keeps the editor useful for Codex-to-Codex, Claude-to-Codex, human-in-the-loop, test automation, local scripts, data processing, and future non-agent workflows.

## Primitive Node Categories

### Terminal And Process I/O

#### Terminal Session

Runs or attaches to a command-line process.

Inputs:

- `stdin`: text to send to the process.
- `control`: start, stop, restart, interrupt, clear, focus.
- `cwd`: optional working directory update.
- `env`: optional environment update.

Outputs:

- `stdout`: raw output stream.
- `stderr`: raw error stream.
- `transcript`: combined terminal transcript.
- `status`: idle, starting, running, exited, errored.
- `exit_code`: process exit code when available.

Notes:

- This is the basic wrapper for Codex CLI, Claude CLI, PowerShell, Python scripts, test runners, and helper tools.
- It should expose both raw stream output and normalized transcript events.

#### Command Runner

Runs a one-shot command and emits the result.

Inputs:

- `command`
- `args`
- `stdin`
- `cwd`
- `env`

Outputs:

- `stdout`
- `stderr`
- `exit_code`
- `duration`
- `success`

Use when a persistent terminal session is unnecessary.

#### File Input

Reads file content or watches a file for changes.

Inputs:

- `path`
- `read_trigger`

Outputs:

- `text`
- `path`
- `changed`
- `error`

#### File Output

Writes or appends text to a file.

Inputs:

- `path`
- `text`
- `mode`: write, append, append_line.

Outputs:

- `written`
- `path`
- `error`

Useful for transcripts, logs, generated summaries, and test artifacts.

#### Clipboard

Reads from or writes to the system clipboard.

Inputs:

- `write_text`
- `read_trigger`

Outputs:

- `text`
- `written`

## Text Processing Nodes

### Append Text

Adds configured text after an incoming message.

Inputs:

- `text`
- `appendix`

Outputs:

- `text`

Example use:

```text
agent message -> Append Text(review rubric) -> terminal stdin
```

### Prepend Text

Adds configured text before an incoming message.

Inputs:

- `text`
- `prefix`

Outputs:

- `text`

Useful for system-style instructions, role setup, and safety reminders.

### Template Text

Interpolates structured values into a template.

Inputs:

- `values`: structured object or multiple named inputs.
- `template`: optional dynamic template override.

Outputs:

- `text`

Example template:

```text
Review this change:
{{body}}

Focus areas:
{{criteria}}
```

### Extract Between Markers

Extracts text between configurable start and end markers.

Inputs:

- `text`
- `start_marker`
- `end_marker`

Outputs:

- `match`: first extracted text.
- `matches`: all extracted spans.
- `before`
- `after`
- `found`: boolean.

Example:

```text
start_marker = "@to reviewer"
end_marker = "@end"
```

This is a core primitive for agent message protocols.

### Regex Extract

Extracts text with a regular expression.

Inputs:

- `text`
- `pattern`
- `flags`

Outputs:

- `match`
- `matches`
- `groups`
- `named_groups`
- `found`

### Split Text

Splits text by delimiter, lines, paragraphs, JSONL records, or regex.

Inputs:

- `text`
- `mode`
- `delimiter`

Outputs:

- `items`
- `count`

### Join Text

Combines multiple text inputs or list items.

Inputs:

- `items`
- `separator`

Outputs:

- `text`

### Replace Text

Find/replace or regex replace.

Inputs:

- `text`
- `find`
- `replace`
- `regex`: boolean.

Outputs:

- `text`
- `changed`

### Clean Text

Normalizes terminal or agent output.

Inputs:

- `text`

Outputs:

- `text`

Options:

- Strip ANSI escape codes.
- Normalize line endings.
- Trim whitespace.
- Collapse repeated blank lines.
- Remove control characters.

## Message Protocol Nodes

### Envelope Builder

Builds a structured text envelope from fields.

Inputs:

- `to`
- `from`
- `type`
- `body`
- `id`
- `reply_to`
- `priority`
- `fields`

Outputs:

- `text`
- `message`

Example output:

```text
@to reviewer
@from implementer
@type review_request
@id DG-142
Please review this patch.
@end
```

### Envelope Parser

Parses marker-based envelopes into structured data.

Inputs:

- `text`

Outputs:

- `message`
- `to`
- `from`
- `type`
- `body`
- `id`
- `fields`
- `malformed`
- `duplicate`

Notes:

- Should support partial messages from streaming terminal output.
- Should remember incomplete input until the end marker appears.

### Message Router

Routes messages by structured fields.

Inputs:

- `message`
- `rules`

Outputs:

- `default`
- dynamic outputs by target, type, tag, or rule name.

Examples:

- Route `to=reviewer` to reviewer terminal.
- Route `type=test_request` to test runner.
- Route `type=question` to user prompt.

### Message Queue

Buffers messages until a target is ready.

Inputs:

- `message`
- `release`
- `hold`
- `clear`

Outputs:

- `next`
- `queued_count`
- `held_count`
- `status`

### Deduplicate

Drops repeated message IDs or repeated content.

Inputs:

- `message`
- `key`

Outputs:

- `unique`
- `duplicate`

## Control Flow Nodes

### Gate

Passes input only when a condition is true.

Inputs:

- `value`
- `condition`

Outputs:

- `passed`
- `blocked`

### Approval Gate

Pauses until the user approves, rejects, or edits a payload.

Inputs:

- `message`
- `summary`
- `risk`

Outputs:

- `approved`
- `rejected`
- `edited`
- `needs_user`

### Switch

Routes input to one of several outputs based on rules.

Inputs:

- `value`
- `rules`

Outputs:

- `case_*`
- `default`

### Merge

Combines multiple incoming streams into one stream.

Inputs:

- `in_*`

Outputs:

- `out`

Options:

- Preserve source metadata.
- Interleave by arrival time.
- Wait for all inputs before emitting.

### Delay

Waits before forwarding input.

Inputs:

- `value`
- `duration`

Outputs:

- `value`

### Throttle Or Debounce

Limits rapid repeated messages.

Inputs:

- `value`
- `interval`

Outputs:

- `value`
- `dropped_count`

### Retry

Retries an operation after failure or timeout.

Inputs:

- `request`
- `success`
- `failure`

Outputs:

- `retry`
- `failed`
- `attempt_count`

### Manual Trigger

Button-like node that emits an event when clicked.

Inputs:

- Optional payload.

Outputs:

- `triggered`

## Agent Workflow Nodes

### Instruction Pack

Stores reusable instructions or role constraints.

Inputs:

- Optional override text.

Outputs:

- `instructions`

Examples:

- Reviewer checklist.
- Implementer rules.
- Test policy.
- Safety policy.

### Context Builder

Combines files, prior messages, instructions, and current task text.

Inputs:

- `task`
- `instructions`
- `files`
- `history`
- `terminal_output`

Outputs:

- `prompt`
- `metadata`

### Result Classifier

Classifies agent output into useful workflow categories.

Inputs:

- `text`
- `message`

Outputs:

- `question`
- `review_request`
- `test_request`
- `approval_request`
- `findings`
- `done`
- `unknown`

### Task State

Tracks current task status.

Inputs:

- `event`
- `message`

Outputs:

- `status`
- `reason`
- `history`

Suggested statuses:

- `idle`
- `working`
- `waiting_review`
- `waiting_tests`
- `needs_user`
- `blocked`
- `done`
- `failed`

### Conversation Memory

Keeps rolling conversation or transcript context.

Inputs:

- `message`
- `reset`

Outputs:

- `history`
- `summary`
- `recent`

Options:

- Fixed message window.
- Character/token budget.
- Summary compaction.

## Human Interaction Nodes

### User Prompt

Asks the user for text input.

Inputs:

- `question`
- `default`

Outputs:

- `answer`
- `cancelled`

### Choice

Presents discrete options.

Inputs:

- `question`
- `choices`

Outputs:

- `selected`
- output per choice.

### Inspector

Lets the user view and edit a message before it continues.

Inputs:

- `message`

Outputs:

- `approved`
- `edited`
- `cancelled`

### Notification

Shows a toast, badge, sound, or log entry.

Inputs:

- `level`
- `title`
- `message`

Outputs:

- `shown`

## Observability Nodes

### Log

Displays incoming values in the UI.

Inputs:

- `value`

Outputs:

- `value`

### Transcript Recorder

Records every message passing through a connection.

Inputs:

- `event`
- `message`

Outputs:

- `recorded`
- `path`

### Probe

Shows the current value on a wire without changing it.

Inputs:

- `value`

Outputs:

- `value`

### Counter

Counts events, messages, failures, retries, or approvals.

Inputs:

- `event`
- `reset`

Outputs:

- `count`

### Trace

Records route decisions and node execution order.

Inputs:

- `event`

Outputs:

- `trace`

## Sections And Runtime Object Identity

The graph editor should support visual sections: dotted rectangular regions that group nodes by purpose. A section is partly layout, partly execution scope, and partly a bridge to ordinary GUI controls.

Sections let a workflow say:

```text
[ Initialization ]
  Create Terminal(session_id="implementer")
  Send startup instructions
  Wait for ready

[ Review Loop ]
  Terminal Ref(session_id="implementer")
  Envelope Parser
  Message Router
```

The important idea is that the terminal is a named runtime object. Nodes in later sections can refer to the same terminal by ID instead of creating another process.

### Section Region

A section is a drawn region behind normal nodes.

Suggested properties:

- `section_id`
- `title`
- `purpose`
- `trigger`
- `enabled`
- `color`
- `collapsed`
- `locked`
- `description`

Suggested interactions:

- Draw as a dotted or lightly tinted rectangle behind contained nodes.
- Drag the section to move all contained nodes.
- Resize the section bounds.
- Collapse to a compact header while preserving internal nodes.
- Lock layout to prevent accidental movement.
- Save a section as a reusable template.
- Expose section-level inputs and outputs when collapsed.

Membership can start as geometry-based: nodes whose center point is inside the rectangle belong to that section. Later, explicit membership can be stored in graph data for stability when nodes overlap section boundaries.

### Section Triggers And Action Targets

A section can be connected to an action or control in the surrounding GUI. This
should use the same split as assignable widget targets: the graph stores stable
IDs and command metadata, while the host app registers live buttons, menu items,
or callbacks at runtime.

Binding chain:

```text
DragonGUI button/menu/control
  -> NodeGraph action target registry
  -> section action binding
  -> section runtime command
```

Common triggers:

- App startup.
- Button click.
- Manual graph trigger.
- Timer or interval.
- Message received.
- Terminal ready.
- Test completed.
- Approval accepted.
- Shutdown.

Target registration should be app-owned and transient:

```python
graph_editor.register_action_target(
    id="initialize-agents",
    label="Initialize Agents",
    action_type="button",
    callback=initialize_agents,
    supported_commands=("run", "reset"),
)
```

The registered target record should describe the live GUI action without making
the graph depend on the button or callback instance:

```json
{
  "action_id": "initialize-agents",
  "label": "Initialize Agents",
  "action_type": "button",
  "supported_commands": ["run", "stop", "reset", "replay"],
  "default_command": "run"
}
```

Section inspector behavior:

- Sections should show an `Action Target` dropdown in the section inspector / info
  menu.
- The dropdown should list registered action targets with friendly labels, while
  saving only the stable `action_id`.
- Choosing a target should fill `action_id` and any expected `action_type`
  metadata together.
- The section command dropdown should offer `run`, `stop`, `reset`, and `replay`,
  filtered or ranked by the selected action target.
- Missing or unregistered action targets should be shown as inspector warnings
  and should also appear in validation/runtime events.
- The editor should support freeform IDs for advanced/manual graph authoring,
  but the normal path should be dropdown-first.

Example GUI binding:

```text
Button("Initialize Agents") -> Run Section("initialization")
Button("Run Review") -> Run Section("review-loop")
Button("Stop All") -> Run Section("shutdown")
```

Saved section data should remain portable:

```json
{
  "runtime_scope": "initialization",
  "trigger": "manual",
  "action_id": "initialize-agents",
  "section_command": "run"
}
```

This gives the visual graph a direct relationship to the app controls around it
without saving transient GUI objects or callbacks in graph data.

### Runtime Object IDs

Some nodes should create or bind to named runtime objects. These IDs let separate sections share durable state.

Examples:

- `Terminal Session(session_id="implementer")`
- `Terminal Ref(session_id="implementer")`
- `Memory Store(store_id="review-history")`
- `Queue(queue_id="reviewer-inbox")`
- `File Watcher(watcher_id="workspace-files")`
- `Transcript Recorder(recorder_id="main-transcript")`

A creator node is responsible for starting or owning the object. A reference node is responsible for interacting with an existing object.

Example terminal lifecycle:

```text
[ Initialization ]
  Terminal Session(session_id="codex-implementer", command="codex")

[ Work Loop ]
  Terminal Ref(session_id="codex-implementer") -> stdin
  Terminal Ref(session_id="codex-implementer") -> stdout

[ Shutdown ]
  Stop Terminal(session_id="codex-implementer")
```

### Object Registry

The runtime should maintain an object registry keyed by stable IDs.

The registry should track:

- `object_id`
- `object_type`
- `owner_node_id`
- `status`
- `created_at`
- `last_used_at`
- `config`
- `runtime_handle`

The graph document should save IDs and configuration, but not unsafe runtime handles or secrets.

### Section Execution Semantics

A section does not need to become a full subgraph engine immediately. The first version can simply provide organization and trigger metadata. Later versions can add execution semantics.

Possible semantics:

- Run all trigger nodes inside the section.
- Enable or disable nodes inside the section as a group.
- Treat section boundaries as scope for variables and object IDs.
- Stop, pause, or reset all runtime objects owned by a section.
- Replay all events that passed through a section.

### Useful Section Templates

#### Initialization

Starts terminals, loads files, initializes memory, creates queues, and sends startup prompts.

#### Message Parsing

Watches terminal output, strips noise, extracts envelopes, validates message shape, and emits structured messages.

#### Routing

Routes messages to reviewer, implementer, tester, user, queues, or logs.

#### Review Loop

Adds review instructions, forwards implementation summaries, receives findings, and sends responses back to the implementer.

#### Testing

Runs test commands, captures results, builds test result envelopes, and routes failures back to the implementer.

#### Approval

Holds risky or ambiguous actions until the user approves, rejects, or edits the payload.

#### Shutdown

Saves transcripts, stops terminals, flushes queues, writes final summaries, and clears runtime handles.

### Current Editor Support

The editor now has first-pass section support in the core `NodeGraph` widget:

- `NodeGraphSection` data with `id`, `title`, bounds, `purpose`, `trigger`, `color`, `collapsed`, `locked`, and custom `data`.
- Section serialization through the graph data schema alongside nodes and edges.
- Dotted, lightly tinted section regions drawn behind nodes and edges.
- Shift-drag on empty canvas creates a section region.
- Sections can be selected, moved, resized, deleted, and renamed.
- Section rename uses the styled in-canvas editor instead of a browser prompt.
- Section edits flow through the graph event bridge and undo/redo history.
- Geometry-based section membership is derived from node center points.
- Dragging a section moves the nodes that belonged to it at drag start.
- `section_moved` events include moved node positions, and Python exposes `section_nodes(section_id)`.
- The in-canvas property editor can edit node title/subtitle/status/color/runtime ID and section title/purpose/trigger/color/runtime ID/locked/collapsed fields.
- Node templates can provide `property_fields` schemas that add custom inspector fields stored in node `data`.
- Edge waypoints can be added by double-clicking a wire, dragged as routing dots, serialized in edge `data.waypoints`, and synced through undo/redo history.

### Required Editor Features

To support sections well, the node editor still needs:

- Visual membership affordances for selected or hovered sections.
- Optional collapsed section rendering with exposed section-level inputs and outputs.
- Save-as-template support for reusable initialization, parsing, routing, review, testing, approval, and shutdown regions.

Implemented editor pieces:

- Section creation by drag-to-draw rectangle.
- Section selection, move, resize, and delete.
- Section title editing.
- Section serialization in graph data.
- Layering rules so sections draw behind nodes and edges.
- Node membership detection by center-point inclusion.
- Group movement for contained nodes.
- Section movement events that include moved node positions.
- Node and section metadata editing through the in-canvas property editor.
- The in-canvas property editor and I-menu fields are canvas-drawn editors, not native DragonGUI `TextInput` widgets. They keep their own keyboard handling and explicitly refocus the graph canvas when a field or select popup is clicked.
- Schema-driven custom property fields for primitive node templates.
- Primitive palette templates for Terminal Session, Text Input, Append Text, Extract Between Markers, Envelope Parser, Message Router, Approval Gate, Log, and Probe.
- Primitive templates now publish a `config_schema` with typed editable fields stored under node `data.config` when edited.
- Python static runtime object registry support can derive named objects from node config, list refs, and report missing refs.
- Python static runtime binding support maps graph nodes and sections into binding records with validation.
- Wire organization waypoints are persisted on edges and can be edited directly on the canvas.

### Required Runtime Features

Implemented runtime pieces:

- Non-destructive `NodeGraph.run_text_flow()` support for primitive text/message flows.
- First-pass execution for Text Input, Append Text, Extract Between Markers, Envelope Parser, Message Router, Log, and Probe nodes.
- A probe button that runs a parser -> router -> log demo without launching real terminals or agents.
- `NodeGraphRuntimeEvent`, `NodeGraphRuntimeHandle`, and `NodeGraphRuntimeSession` provide the live runtime execution contract, transient handle registry, validation snapshot, and event log.
- Runtime sessions can create and attach a `TerminalBridge` for a `terminal_session` object without starting it by default.
- Runtime sessions can start/stop terminal handles, send stdin, normalize terminal lifecycle/stdout/stdin events, and clean up attached handles.
- `node_graph_editor_probe.py` includes runtime controls for the Terminal Session path.
- `NodeGraphRuntimeViewBinding` resolves graph nodes to observable runtime views.
- `Terminal` can attach to an existing `TerminalBridge`, and the probe includes a Runtime View panel for the attached Terminal Session.
- `NodeGraphRuntimeEdgeBinding` and runtime port-value storage propagate `terminal_stdout` values across connected graph edges.
- Delivered runtime edge values can execute safe downstream primitive nodes (`parser`/`envelope_parser`, `message_router`, `log`, and `probe`) and emit their outputs back into the runtime event stream.
- Widget Sink / UI Indicator nodes let graph values update registered DragonGUI widgets by stable widget ID without saving transient widget instances in graph data.
- Widget Source nodes let graph values originate from registered DragonGUI widgets such as `TextInput`, `TextArea`, or `CodeEditor` by stable widget ID.
- Assignable widget targets can be registered by host apps and selected from Widget Sink and Widget Source inspector dropdowns using friendly labels while saving portable widget IDs.
- `node_graph_editor_probe.py` demonstrates terminal stdout assignment through configured `Widget Sink` nodes and GUI prompt input through a configured `Widget Source` targeting the `Runtime Prompt Input` TextInput.
- The probe treats `Runtime Prompt Input` as an ordinary host GUI edit field registered by stable widget ID. The graph can read it through `Widget Source`, and the `Send GUI Prompt` control runs that source node to deliver the current GUI text to the Terminal Session path.
- NodeGraph wire and socket colors are associated with port data types, so terminal output, messages, text, JSON, errors, artifacts, and control/status flows remain visually consistent across graphs.
- Runtime sessions can run source nodes directly with `run_node()`, starting with `Text Input` emitting its configured text into the live graph.
- Runtime sessions can run section-contained source nodes with `run_section()`, emitting a section summary event while keeping non-source nodes skipped rather than destructive.

The implemented runtime pieces are intentionally non-destructive. They validate
graph metadata, own transient live handles, and run local text/message
transforms from delivered runtime values, but they do not launch terminals
automatically or execute sections.

To make sections executable, the runtime will need:

- Object creator and reference node distinction.
- Additional runtime detail views from graph node -> runtime state -> observable widget or detail panel.
- Assignable action targets backed by a transient runtime action registry.
- Section inspector dropdowns for action target and section command selection.
- Lifecycle handling for startup and shutdown sections.
- Section-level run, stop, reset, and replay commands.
- GUI action binding to section commands.

### Widget Sink / Widget Source Binding

Some graph nodes should connect ordinary DragonGUI widgets that already exist in
the surrounding GUI to the graph runtime. This is separate from runtime views: a
runtime view is a detail surface for a selected node, while widget binding is a
portable connection between graph data and stable GUI widget IDs anywhere in the
app.

There are two directions:

- `Widget Sink`: graph value -> existing GUI widget.
- `Widget Source`: existing GUI widget value -> graph value.

Widget Sink binding chain:

```text
NodeGraph output port
  -> Widget Sink node input
  -> NodeGraphRuntimeSession widget registry
  -> existing DragonGUI widget instance
```

Widget Source binding chain:

```text
Existing DragonGUI widget instance
  -> NodeGraphRuntimeSession widget registry
  -> Widget Source node output
  -> connected graph input ports
```

### GUI Edit Fields And In-Canvas Edit Fields

There are two different kinds of editable text involved:

- Host GUI edit fields are real DragonGUI widgets such as `TextInput`, `TextArea`, and `CodeEditor`. These can be registered as assignable widget targets and selected by a `Widget Source` node. The graph stores the stable `widget_id`; the live app supplies the actual widget instance at runtime.
- Node editor inspector fields, command-palette search text, and rename fields are drawn inside the `NodeGraph` canvas. They are not registered widget sources and do not become graph values directly. They exist to edit graph metadata, so they use the canvas keyboard/focus path and save changes back into graph data.

Example host GUI field binding:

```text
TextInput(id="runtime-prompt-input")
  -> Widget Source(widget_id="runtime-prompt-input", port_profile="text")
  -> Terminal Session.stdin through text -> terminal_input conversion
```

This distinction matters for user expectations: typing in a normal GUI field should feel like a normal DragonGUI input and can feed the graph; typing in the I-menu/property editor changes node configuration.

Required concepts:

- Widget instances are transient runtime handles and must not be serialized into
  graph data.
- Graph data stores only a stable `widget_id`, an expected `widget_type`, a
  `port_profile`, direction-specific options such as `update_mode`, and formatting hints.
- The runtime session owns a widget registry populated by the application or
  probe before execution.
- Safe first update modes should include `set`, `append`, and `state`.
- Safe first widget targets should include `label`, `badge`, `log_view`,
  `text_input`, `text_area`, `code_editor`, and `led`.
- The editor picker lists registered assignable widget targets and writes the
  selected stable ID into Widget Sink or Widget Source node config.

### Assignable Widget Targets

The editor should make widget binding feel like choosing a destination, not
typing an implementation detail. A DragonGUI application can expose ordinary UI
components as assignable graph targets, and Widget Sink / UI Indicator or Widget Source nodes can
choose from those targets in the node inspector.

Target registration should be app-owned and transient:

```python
graph_editor.register_widget_target(
    id="event-log",
    label="Event Log",
    widget_type="log_view",
    widget=event_log,
    supported_update_modes=("append", "set"),
    supported_port_profiles=("text", "terminal_output", "message", "json"),
)
```

The registered target record should describe the live widget without making the
graph depend on the widget instance:

```json
{
  "widget_id": "event-log",
  "label": "Event Log",
  "widget_type": "log_view",
  "supported_update_modes": ["append", "set"],
  "default_update_mode": "append",
  "supported_port_profiles": ["text", "terminal_output", "message", "json"],
  "default_port_profile": "text",
  "supported_formats": ["text", "json", "repr", "message_body"]
}
```

Inspector behavior:

- Widget Sink and Widget Source nodes should show a `Widget` dropdown in the node inspector / info
  menu.
- The dropdown should list registered targets with friendly labels, while still
  saving only the stable `widget_id`.
- Choosing a target should fill `widget_id` and `widget_type` together.
- Update mode options should be filtered or ranked by the selected widget type.
- Port profile options should describe the incoming graph value type for sinks
  and the outgoing graph value type for sources, and can be suggested by the selected widget target.
- Format options should remain explicit because they describe how graph values
  are rendered into widgets or how widget values are read into graph values.
- Missing, unregistered, or type-mismatched targets should be shown as warnings
  in the inspector and should also emit runtime validation/update events.
- The editor should support freeform IDs for advanced/manual graph authoring,
  but the normal path should be dropdown-first.

Compatibility examples:

- `log_view`: `append`, `set`; good for terminal output, event streams, parsed
  messages, and debug probes.
- `label`, `badge`, `text_input`, `text_area`, `code_editor`: `set`; good for
  latest status, selected message, prompt drafts, and generated artifacts.
- `led`: `state`; good for boolean or enum-like runtime state.

Saved graph data should continue to store only portable assignment metadata:

```json
{
  "node_type": "widget_sink",
  "config": {
    "widget_id": "event-log",
    "widget_type": "log_view",
    "port_profile": "message",
    "update_mode": "append",
    "format": "message_body"
  }
}
```

This keeps the graph reusable across apps while allowing each host GUI to decide
which live components are available as display targets or input sources.

### Configurable Port Profiles

Some primitive nodes should remain generic in behavior while letting the user pick
which data type they accept or emit. This avoids creating special-purpose menu
nodes such as `Terminal Output Display`, `Message Display`, or `JSON Display`
when those are really configured versions of the same primitive.

The first targets are `Widget Sink` and `Widget Source`:

- The add-node menu should expose one generic `Widget Sink` primitive and one generic `Widget Source` primitive.
- The inspector should provide an `Accepted Value Type`, `Emitted Value Type`, or `Port Profile`
  dropdown with options such as `text`, `terminal_output`, `message`, `json`,
  `artifact`, `status`, and `error` where compatible.
- Choosing a profile should update the visible input/output pin labels, port
  types, socket colors, wire compatibility, and default formatting hints.
- Choosing a registered widget target can suggest a profile, but the graph should
  still save the explicit profile so it remains portable.
- The nodes should remain `node_type="widget_sink"` or `node_type="widget_source"`; graph data should not grow
  new hard-coded node types for every display target or input source.

Example saved config:

```json
{
  "node_type": "widget_sink",
  "config": {
    "widget_id": "terminal-output-log",
    "widget_type": "log_view",
    "port_profile": "terminal_output",
    "update_mode": "append",
    "format": "text"
  }
}
```

This pattern should also apply to other generic primitives over time:

- `Text Input` can emit plain `text`, terminal command text, or prompt text
  without becoming separate node types.
- `Formatter` can format `message`, `json`, `artifact`, or plain `text` into
  `text`.
- `Router` can route `message`, `event`, `status`, or generic `json` values
  with the same core routing behavior.
- `Log` and `Probe` can observe any value type while preserving the incoming
  type on their output pin.

Port profiles are different from adaptive conversion edges. A port profile says
"this generic node is currently shaped for this data type." An adaptive edge says
"these two different port types can connect through a saved conversion rule."

Example node config:

```json
{
  "node_type": "widget_sink",
  "config": {
    "widget_id": "runtime-indicator",
    "widget_type": "log_view",
    "port_profile": "json",
    "update_mode": "append",
    "format": "json"
  }
}
```

### Runtime View Binding

Applicable node types should be observable while they run. The graph should not
special-case Terminal nodes forever; it needs a general pathway from graph node
to runtime object to view.

Binding chain:

```text
NodeGraphNode
  -> NodeGraphNodeBinding
  -> NodeGraphRuntimeObject / NodeGraphRuntimeHandle
  -> NodeGraphRuntimeViewBinding
  -> DragonGUI widget or detail panel
```

Required concepts:

- Each runtime-capable node can declare `view_type`, such as `terminal`,
  `event_log`, `queue`, `approval`, `parser_trace`, `artifact_list`,
  `test_results`, or `metrics`.
- A runtime view attaches to an existing runtime handle instead of creating a
  second process/session.
- View state is transient runtime UI state, not saved graph layout data.
- The selected-node inspector or detail panel should resolve the selected node's
  runtime object and show the matching live view when available.
- Nodes without a dedicated view still expose a generic event/config/status
  detail view.

First useful view bindings:

- Terminal Session -> `Terminal` widget attached to the existing
  `TerminalBridge`.
- Envelope Parser -> parser event trace and parsed message preview.
- Message Router -> queue/routing table with delivered/held/failed states.
- Approval Gate -> pending approval payload, approve/reject/edit controls, and
  decision history.
- Log / Probe -> latest values and event timeline.
- Artifact / Recorder -> artifact manifest and transcript chunks.
- Tester / Command Runner -> command status, stdout/stderr, and test report.

## First Useful Subgraph Templates

### Codex Implementer And Reviewer

```text
Terminal(Codex implementer)
  -> Clean Text
  -> Extract Between Markers
  -> Envelope Parser
  -> Message Router
  -> Append Text(review instructions)
  -> Terminal(Codex reviewer)

Terminal(Codex reviewer)
  -> Clean Text
  -> Extract Between Markers
  -> Envelope Parser
  -> Message Router
  -> Terminal(Codex implementer)
```

### Human Approval Before Terminal Input

```text
Envelope Parser
  -> Inspector
  -> Approval Gate
  -> Terminal stdin
```

### Test Request Flow

```text
Envelope Parser(type=test_request)
  -> Command Runner(pytest)
  -> Clean Text
  -> Envelope Builder(type=test_result)
  -> Router
```

### Artifact Capture

```text
Terminal transcript
  -> Clean Text
  -> Transcript Recorder
  -> File Output
```

## Runtime Metadata Each Node Should Expose

Every executable node should eventually expose:

- `node_id`
- `node_type`
- `status`
- `last_input_at`
- `last_output_at`
- `last_error`
- `run_count`
- `error_count`
- `latency_ms`
- `config`
- `ports`

This lets the graph editor become both a visual editor and a live workflow monitor.

## Port Typing Direction

Suggested base port types should match the implementation names used by
`NodeGraphPort.port_type` and the data-type color map:

- `text`
- `stream:text`
- `text:list`
- `message`
- `message:list`
- `json`
- `file:path`
- `terminal_input`
- `terminal_output`
- `terminal_error`
- `event`
- `control`
- `status`
- `bool`
- `number`
- `approval_request`
- `approval_result`
- `test_request`
- `test_report`
- `artifact`
- `error`

Strict typing should help prevent obvious mistakes, but the editor should allow explicit conversion nodes instead of blocking advanced workflows. Wire and socket colors should continue to be derived from these port types.

### Adaptive Conversion Edges

Some common connections should be easy even when the source and target port types
are not identical. The editor should allow a small, explicit set of safe
conversions and store the conversion on the edge rather than mutating either
node's port type.

First supported conversion:

```text
Text Input(text) -> Terminal Session(stdin: terminal_input)
```

Saved edge data should describe the conversion:

```json
{
  "conversion": "text_to_terminal_input",
  "newline": true
}
```

Runtime behavior:

- When a `text` value reaches a `terminal_input` target through this conversion,
  the runtime sends the converted text to the target terminal session's stdin.
- The node remains a generic Text Input node, so the same text source can still
  feed parsers, logs, prompt templates, or message builders.
- The edge inspector can later expose conversion options such as newline, raw
  text, trim whitespace, prefix, and suffix.
- Advanced users can still use explicit converter nodes when they want conversion
  steps to be visible in the graph.

Future safe conversions may include:

- `text -> message` through an envelope parser.
- `message -> text` using body or template formatting.
- `json -> text` using JSON serialization.
- `terminal_output -> text` using transcript cleaning.


## Near-Term Implementation Order

1. [x] Keep the current node editor focused on editing and persisting graph structure.
2. [x] Add section regions with drag-to-create, resize, move, title editing, and serialization.
3. [x] Add section membership detection and group movement for contained nodes.
4. [x] Add a property inspector or metadata editor for node and section configuration fields.
5. [x] Add schema-driven property fields for primitive templates.
6. [x] Add primitive templates for Terminal, Text Input, Append Text, Extract Between Markers, Envelope Parser, Message Router, Approval Gate, Log, and Probe.
7. [x] Add node configuration schema so each node template can define editable fields.
8. [x] Add runtime object IDs and a static object registry that derives declared objects and missing references from graph config.
9. [x] Add a static runtime binding layer that maps graph nodes and sections to Python binding records.
10. [x] Build a small non-destructive demo where text flows through parser/router/log nodes without launching real agents.
11. [x] Define the live graph runtime execution contract: event payloads, validation snapshots, lifecycle status, and serializable event logs.
12. [x] Promote static object metadata into a live runtime session registry with transient handles and status.
13. [x] Add opt-in Terminal Session bridge creation without automatic process launch.
14. [x] Add Terminal Session runtime commands for start, stop, stdin input, normalized stdout/stdin events, and cleanup.
15. [x] Add probe controls for creating runtime sessions and exercising Terminal Session commands.
16. [x] Add a general runtime-view binding contract for node types with useful live views.
17. [x] Let `Terminal` attach to an existing `TerminalBridge` so Terminal Session nodes can be observed directly.
18. [x] Add a probe detail panel that resolves the selected node's runtime view when available.
19. [x] Route terminal stdout events across connected graph edges.
20. [ ] Add stderr edge transport when `TerminalBridge` exposes stderr separately from combined PTY output.
21. [x] Execute downstream parser/router/log/probe nodes from delivered runtime edge values.
22. [x] Add Widget Sink / UI Indicator nodes that update registered DragonGUI widgets from graph values.
23. [x] Add assignable widget target registration and inspector dropdown support for Widget Sink node config.
24. [x] Add adaptive conversion edges, starting with `text -> terminal_input`.
25. [x] Add configurable port/type profiles for generic primitives, starting with Widget Sink accepted value type.
26. [x] Add Widget Source nodes so registered GUI edit fields can emit graph values without Python code changes. The probe demonstrates a real `TextInput` host widget feeding a Terminal Session path by stable widget ID.
27. [x] Keep canvas-drawn node editor fields documented separately from real GUI edit fields; property/I-menu fields edit graph metadata and maintain their own canvas focus handling.
28. [x] Add edge routing waypoints so complex graphs can organize traces without adding functional nodes.
29. [ ] Add assignable action targets plus section-level run, stop, reset, and replay commands. `run_node()` and first-pass `run_section()` are now implemented; action target registration and stop/reset/replay commands remain.
30. [ ] Save common role setups as section or subgraph templates, not as hard-coded primitive nodes.

## Open Questions

- Should node execution be pull-based, push-based, or a hybrid event stream?
- How should partial streaming terminal output be represented on edges?
- Should message envelopes use plain marker text first, JSON first, or both?
- How much state should live in the graph document versus separate runtime session records?
- How should secrets and environment variables be represented without leaking into saved graph files?
- Should subgraphs be editable inline, opened as tabs, or represented as collapsed macro nodes?
- Should section membership be purely geometry-based, explicit in graph data, or both?
- Should runtime object IDs be globally unique across the whole graph or scoped by section?
- Should runtime views be registered by node type, runtime object type, or explicit node `view_type`?
- Should action targets bind directly to section commands, emit graph events, or both?


