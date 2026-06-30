# Navigation And Multipage Plan

## Objective

Add first-class navigation for applications that need multiple tools, views, or
data workflows in one native window.

The goal is not to build a browser router. The first version should cover the
application patterns DragonGUI is targeting:

- Tabbed analysis panels.
- A left navigation sidebar with pages.
- Tool pages that preserve state when hidden.
- Keyboard-friendly switching between pages.
- No Python per-frame rendering and no full document resend when switching.

## API Shape

Start with two public patterns: tabs for compact local view switching, and
pages plus navigation items for full app shells.

```python
import dragongui as dg

app = dg.App()
win = dg.Window("Analysis Workbench", width=1400, height=900)

with dg.Tabs(value="scatter") as tabs:
    with dg.Tab("Scatter", value="scatter"):
        dg.Scatter3D(df, x="x", y="y", z="z")

    with dg.Tab("Table", value="table"):
        dg.DataFrameTable(df)

    with dg.Tab("Settings", value="settings"):
        dg.Checkbox("Use GPU point sprites", checked=True)

app.run(win)
```

For larger applications:

```python
app = dg.App()
win = dg.Window("DragonGUI Data Tool", width=1400, height=900)

with dg.HLayout():
    with dg.Sidebar(width=220):
        dg.NavItem("Scatter", page="scatter")
        dg.NavItem("Table", page="table")
        dg.NavItem("Settings", page="settings")

    with dg.Pages(value="scatter") as pages:
        with dg.Page("scatter"):
            dg.Scatter3D(df, x="x", y="y", z="z")

        with dg.Page("table"):
            dg.DataFrameTable(df)

        with dg.Page("settings"):
            dg.Label("Rendering")
            dg.Checkbox("Use GPU point sprites", checked=True)
```

Public widgets:

- `Tabs(value=None, on_change=None, disabled=False)`
- `Tab(label, value=None, disabled=False)`
- `Pages(value=None, on_change=None)`
- `Page(value, title=None)`
- `Sidebar(width=220)`
- `NavItem(label, page, disabled=False)`

Keep `value` explicit and stable. Labels are display text; values are routing
keys. If a value is omitted for `Tab`, derive it from the label once at
construction time.

## Behavior

Tabs:

- Draw a tab strip and exactly one active content region.
- Clicking a tab activates it.
- Arrow keys move focus between tabs.
- Enter or Space activates the focused tab.
- Home and End jump to first and last enabled tabs.
- Hidden tab content is not rendered or hit-tested.
- Widget state inside hidden tabs is preserved.

Pages and sidebar navigation:

- `Pages` owns the active page key.
- `NavItem(page=...)` activates the matching page.
- Multiple navigation surfaces can point at the same `Pages` container later,
  but v1 can assume one `Pages` container per window.
- Hidden page content is not rendered or hit-tested.
- Widget state inside hidden pages is preserved.
- If the active page is removed or invalid, select the first enabled page.

Focus:

- Tab traversal only includes widgets inside the active tab/page.
- Switching pages moves focus to the first focusable child in that page, unless
  the page previously had focused content.
- Focus rings must work on tab headers and nav items.

Callbacks:

- `Tabs(..., on_change=fn)` receives the active tab value.
- `Pages(..., on_change=fn)` receives the active page value.
- `NavItem` should not need its own callback for basic routing.

## Startup Document Schema

Navigation nodes are normal widget tree nodes:

```python
{
    "id": "dg-12",
    "type": "tabs",
    "props": {"value": "scatter", "events": ["change"]},
    "children": [
        {
            "id": "dg-13",
            "type": "tab",
            "props": {"label": "Scatter", "value": "scatter"},
            "children": [...]
        }
    ]
}
```

For app pages:

```python
{
    "type": "pages",
    "props": {"value": "scatter"},
    "children": [
        {"type": "page", "props": {"value": "scatter", "title": "Scatter"}}
    ]
}
```

`NavItem` carries `page`, not a child tree:

```python
{"type": "nav_item", "props": {"label": "Scatter", "page": "scatter"}}
```

## Runtime State Model

Navigation active state belongs to Rust after startup:

- `WidgetState.active_tabs: HashMap<tabs_id, value>`
- `WidgetState.active_pages: HashMap<pages_id, value>`
- `WidgetState.page_focus: HashMap<page_value, focused_widget_id>`

Python widget objects are handles. Setting `tabs.value` or `pages.value` after
`app.run()` must enqueue a command instead of resending the full tree.

Initial commands:

```python
{"target": "dg-12", "op": "tabs.set_active", "payload": {"value": "table"}}
{"target": "dg-20", "op": "pages.set_active", "payload": {"value": "settings"}}
```

If the command queue is not complete when the first navigation slice starts,
implement internal Rust switching first and expose callback state updates
through the existing callback bridge.

## Native Implementation Work

Python:

- Add `Tabs`, `Tab`, `Pages`, `Page`, `Sidebar`, and `NavItem` classes.
- Validate child nesting:
  - `Tab` must live directly under `Tabs`.
  - `Page` must live directly under `Pages`.
  - `NavItem.page` must be a non-empty string.
- Validate duplicate tab/page values inside the same container.
- Register `on_change` only for `Tabs` and `Pages` when provided.
- Add `examples/multipage_tool.py`.

Rust document parsing:

- Add `WidgetKind::Tabs`, `Tab`, `Pages`, `Page`, `Sidebar`, `NavItem`.
- Parse `label`, `value`, `page`, and `disabled`.
- Build a navigation index during `WidgetState::from_tree`.
- Validate invalid active values and fall back to the first enabled child.

Layout:

- `Tabs` layout is a vertical container:
  - fixed-height tab strip.
  - active tab child gets the content rect.
- `Pages` lays out only the active `Page` child in its full rect.
- Inactive `Tab` and `Page` children receive no rects for hit testing.
- `Sidebar` is a vertical panel with fixed width.
- `NavItem` uses normal control height.

Rendering:

- Draw tab strip backgrounds, active tab underline or fill, hover, pressed, and
  focus states.
- Draw sidebar surface and nav item active/hover/focus states.
- Render labels through the existing text renderer.
- Do not draw inactive page contents.

Events:

- Hit-test tab headers separately from tab content.
- Hit-test nav items as normal controls.
- Clicking a tab or nav item updates active state and rebuilds layout.
- Page changes emit `ChangeValue::Text(value)`.
- Keyboard support:
  - ArrowLeft/ArrowRight for horizontal tab strips.
  - ArrowUp/ArrowDown for sidebar nav.
  - Home/End for first/last item.
  - Enter/Space to activate.

## Milestone Slices

Current implementation:

- N0 is implemented: Python widgets, schema serialization, validation, callback
  registration, and tests.
- N1 is implemented for eager tabs: native parsing, layout, rendering, mouse
  activation, text, focus, and keyboard activation.
- N2 is implemented for eager pages and sidebar navigation.
- N3 is partially implemented: Tab/Shift+Tab only traverse visible content,
  tab headers support Left/Right/Home/End, and nav items support
  Up/Down/Home/End.
- N4 is still pending because the general command queue is not in place yet;
  switching currently mutates Rust state internally and emits existing
  `on_change` callbacks.

### N0: Python API And Schema

- Add Python widget classes.
- Serialize valid startup documents.
- Add validation tests.
- Add a dev-fallback `multipage_tool.py` document demo.

Acceptance:

- `python -m pytest` covers nesting, duplicate values, and document shape.
- No native rendering changes required yet.

### N1: Native Tabs

- Parse `Tabs` and `Tab`.
- Layout and render tab strip plus active tab content.
- Mouse click switches active tab.
- Hidden tab content is not rendered or hit-tested.

Acceptance:

- A demo can switch between scatter, table, and settings tabs.
- Scatter camera controls only work when the scatter tab is active.

### N2: Pages And Sidebar

- Parse `Pages`, `Page`, `Sidebar`, and `NavItem`.
- Sidebar item click switches page.
- Active nav item is visually distinct.

Acceptance:

- A demo can switch between full application pages.
- Table scroll state and text input state survive page switches.

### N3: Keyboard And Focus

- Add tab/nav keyboard navigation.
- Confine focus traversal to active content.
- Restore per-page focus where practical.

Acceptance:

- Tab, Shift+Tab, arrows, Home/End, Enter, and Space behave predictably.
- Focus ring never appears on hidden page content.

### N4: Command Integration

- Add `tabs.set_active` and `pages.set_active` commands.
- Python-side handles update their `.value` from change callbacks.
- Background threads can request navigation through the app command queue once
  that queue exists.

Acceptance:

- Navigation changes do not reserialize the full app document.
- `Tabs(..., on_change=...)` and `Pages(..., on_change=...)` fire once per
  activation.

### N5: Polish

- Add disabled tab/nav item styling.
- Add closeable tabs only if there is a concrete app need.
- Add optional lazy pages after the eager version is stable.

Acceptance:

- Visual styling is consistent with dark and light themes.
- Nested tabs work inside active pages.

## Risks And Decisions

- Eager page trees are simpler and preserve state, but they allocate all widgets
  up front. Use eager trees for v1; add lazy pages later.
- Routing by string keys is enough. Avoid URL-style routing until there is a
  concrete need.
- Do not make `NavItem` own page content. Keeping navigation and page content
  separate makes sidebars, top nav, and command-driven routing share one state
  model.
- Hidden content must not be part of hit testing. This is the most important
  correctness rule because scatter/table controls can otherwise receive input
  through inactive pages.

## Acceptance Criteria

- One example demonstrates both `Tabs` and `Sidebar` plus `Pages`.
- Inactive pages do not render, receive pointer input, or participate in focus.
- State inside inactive pages survives switching.
- Navigation works by mouse and keyboard.
- `on_change` callbacks receive the active value.
- Runtime switching is implemented as targeted state mutation, not full
  document replacement.
