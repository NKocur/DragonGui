# WebView Lifecycle In Tabs

## Symptom

`HtmlReport`-backed widgets such as `NodeGraph` and `Terminal` can be hosted
inside `Tabs` or `Pages`. During the binding playground tab work, the `GUI
Objects` tab initially showed scrollbars but no visible/clickable controls.

The primary visible failure was caused by native `Tabs` clipping: active tab
body controls had layout rectangles, but inherited the tab header rectangle as
their clip.

## Current Status

Fixed:

- Active `Tab` children now receive a body-area clip below the tab header.
- Inactive `Tab` and `Page` descendants are pruned from layout maps.
- WebView sync runs immediately after layout recompute.
- Inactive-but-still-mounted `HtmlReport` WebViews are preserved, hidden, and
  moved offscreen. WebViews whose widgets are removed from the tree are closed.

Verified:

- `node_graph_binding_playground_probe.py` now shows the `GUI Objects` tab
  controls with the `NodeGraph` restored.
- Native regression tests cover inactive tab/page layout pruning and active tab
  body controls retaining nonzero clips.

## Open Question

Closing inactive WebView2 controllers is robust, but may be heavier than needed
when switching between tabs that contain `NodeGraph`, `Terminal`, or other
`HtmlReport` widgets.

For stateful WebView widgets, closing inactive controllers also creates an
unacceptable UX risk: the `NodeGraph` editor can appear to clear or reset when
the user switches away from its tab and returns. A node editor cannot depend on
DOM-only state that is destroyed by normal tab navigation.

We need to decide whether the permanent library behavior should be:

- close inactive WebViews, keeping correctness and input safety simple; or
- hide/reposition inactive WebViews, preserving page state but requiring
  reliable z-order/input behavior across tab switches.

## Local Research Notes

`NodeGraph` state is split between Python and the WebView DOM:

- Python owns durable graph contents for nodes, edges, sections, node
  properties, section properties, runtime bindings, selected node, undo/redo
  stacks, dirty state, and the last viewport reported by canvas events.
- The generated HTML is rebuilt from Python-owned nodes, edges, sections,
  templates, targets, and selected node.
- The WebView DOM still owns transient editor state: selected edge, selected
  section, open property editor, in-progress text edits, node picker state,
  JavaScript-side undo/redo availability, and the live canvas viewport used at
  startup.
- Python tracks viewport through `viewport_changed`, but the generated HTML
  currently initializes `viewX`, `viewY`, and `zoom` from hardcoded defaults
  instead of hydrating from Python's `navigation_state()`.

This means destroying and recreating the WebView can rebuild the graph content,
but it is not a seamless editor restore today.

## Proposed Solution

Use a two-layer policy:

1. Preserve inactive WebView controllers during normal navigation. Implemented.
   - Collect all `HtmlReport` ids that still exist in the widget tree.
   - Collect the active/visible `HtmlReport` ids after `Tabs`/`Pages` filtering.
   - For ids that still exist but are inactive, hide and move the WebView
     offscreen, but do not close the controller.
   - For ids that no longer exist anywhere in the widget tree, close and remove
     the controller.

2. Make stateful WebViews recoverable when recreation is unavoidable. Still
   open.
   - Add `navigation_state` to the `NodeGraph` HTML config and initialize
     `state.viewX`, `state.viewY`, and `state.zoom` from it.
   - Add explicit Python-owned selected edge and selected section state, or a
     general editor selection state, so selection survives recreation.
   - Decide whether property editor open state and in-progress edits should be
     recoverable. At minimum, committed graph data must survive.
   - Hydrate the JavaScript history controls from Python `history_state()`, or
     route undo/redo toolbar actions to Python so the Python stacks remain
     authoritative.

This avoids clearing `NodeGraph` when switching tabs while keeping a safe
fallback for real widget removal.

## Follow-Up Work

- Add focused runtime tests or a probe for WebView-containing tabs.
- Measure whether closing/recreating WebViews causes unacceptable latency or
  state loss for `NodeGraph` and `Terminal` widgets.
- Audit `NodeGraph` state ownership. Graph data, viewport, selection, section
  metadata, inspector edits, and runtime binding metadata should be recoverable
  from the DragonGUI widget state, not only from the live WebView DOM.
- Define a WebView rehydration path so a recreated `NodeGraph` WebView restores
  the same graph state after tab switches.
- Verify the inactive-but-mounted WebView preserve policy with `NodeGraph` and
  `Terminal` tabs.
- Document the final lifecycle policy in the widgets reference once decided.
