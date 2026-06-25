# Nodal System Plan

## Source Of Truth

`primitive-node-vocabulary.md` is the current detailed plan for the nodal
system. This README is the short orientation layer: what exists now, what is
still missing, and where work should resume.

## Objective

Build DragonGUI's `NodeGraph` from a useful visual editor into a nodal
orchestration system for terminal sessions, agent workflows, message routing,
approval gates, testing loops, artifact capture, and reusable workflow regions.

The near-term goal has shifted from basic graph editing to runtime integration:
the editor can now create, edit, persist, and report graph changes. The next
goal is to connect the graph to real runtime objects without losing the current
non-destructive editing and probing behavior.

## Current Implementation

Implemented in the current prototype:

- `NodeGraph`, `NodeGraphNode`, `NodeGraphPort`, `NodeGraphEdge`, and
  `NodeGraphSection` Python APIs.
- Canvas-backed graph rendering inside `HtmlReport`.
- Node dragging, canvas panning, mouse-wheel zoom, edge creation, edge
  selection, node selection, deletion, duplication, and basic node creation.
- Versioned graph persistence through `NodeGraph.to_graph_data()`,
  `NodeGraph.set_graph_data(...)`, and `NodeGraph.from_graph_data(...)`.
- Python-side event bridge for canvas mutations through `on_graph_event`.
- Undo/redo history for canvas-side graph mutations.
- Section regions with drag-to-create, move, resize, delete, rename,
  serialization, membership detection, and group movement for contained nodes.
- In-canvas property editor for node and section metadata.
- Primitive palette templates for Terminal Session, Text Input, Append Text,
  Extract Between Markers, Envelope Parser, Message Router, Approval Gate, Log,
  and Probe.
- Template-driven `property_fields` and `config_schema` metadata for editable
  node configuration.
- Typed port metadata and first-pass connection validation support.
- Static runtime object registry helpers that derive declared runtime objects
  and missing references from graph node configuration.
- Static runtime binding helpers that map graph nodes and sections to runtime
  binding records.
- Live `NodeGraphRuntimeSession` state that owns transient runtime handles,
  status changes, validation snapshots, and schema-versioned runtime events
  separately from graph layout data.
- Non-destructive `NodeGraph.run_text_flow()` for primitive text/message flows.
- Probe demo that runs parser -> router -> log without launching real terminals
  or agents.
- Opt-in Terminal Session bridge creation through runtime sessions. Creating a
  runtime session still does not launch a process; terminal bridges are attached
  only when explicitly requested.
- Terminal Session runtime commands for bridge start, stop, stdin input, and
  cleanup, with normalized runtime events such as `terminal_started`,
  `terminal_stdout`, and `terminal_stdin`.
- `node_graph_editor_probe.py` exposes runtime controls for creating a runtime
  session, attaching/starting/stopping the terminal bridge, sending input, and
  inspecting runtime events.
- Runtime-view binding records resolve graph nodes to observable live views.
- `Terminal` can attach to an existing `TerminalBridge`, so a Terminal Session
  node can be watched without creating a second bridge/process.
- `node_graph_editor_probe.py` includes a Runtime View panel that resolves the
  selected node or Terminal Session node and shows the live attached terminal
  view when available.
- Runtime edge transport records emitted port values and propagates
  `terminal_stdout` values across connected graph edges.
- Delivered runtime edge values can execute safe downstream primitive nodes
  (`parser`/`envelope_parser`, `message_router`, `log`, and `probe`) and emit
  their outputs back into the runtime event stream.
- Widget Sink / UI Indicator nodes can bind graph values to registered
  DragonGUI widget IDs such as labels, logs, text boxes, and LEDs.

## Where We Left Off

The editor foundation is in place. The runtime contract and live handle registry
now exist. Terminal nodes can be observed through a live runtime view, terminal
stdout can move across graph edges, and safe primitive nodes can execute from
delivered edge values.

The next practical slice is section commands and richer runtime observation:

- Add widget-ID picker support for Widget Sink node configuration.
- Add section-level run, stop, reset, and replay commands.
- Add non-terminal detail views for parser/router/log/probe/approval nodes.
- Keep `run_text_flow()` as the safe non-destructive probe path.

## Not Yet Implemented

- Live agent sessions bound to terminal or process nodes.
- Widget-ID picker support for Widget Sink node configuration.
- Section-level runtime commands.
- Non-terminal runtime detail views for parser/router/approval/log/probe nodes.
- Collapsed section rendering with section-level inputs and outputs.
- Save-as-template support for reusable role, startup, review, testing,
  approval, and shutdown regions.
- Browser/WebView-level interaction tests for shift-drag section creation,
  resize handles, and property editor interaction.

## Recommended Next Work

1. Add widget-ID picker support for Widget Sink node configuration.
2. Add section run/stop/reset commands once one real runtime object can execute.
3. Bind live terminal events into `AgentSession` records for terminal-backed
   agent nodes.
4. Add non-terminal runtime detail views for parser/router/approval/log/probe
   nodes.
5. Save the first reusable subgraph templates only after the runtime contract is
   stable.

## Useful Local Checks

Run focused nodal tests with:

```powershell
python -m pytest tests\test_python_api.py -q -k "node_graph"
```

Run the interactive probe with:

```powershell
python examples\node_graph_editor_probe.py
```
