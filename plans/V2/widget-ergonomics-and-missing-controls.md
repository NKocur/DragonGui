# Widget Ergonomics And Missing Controls Plan

## Objective

Fill the next set of high-impact DragonGUI widget gaps for real data tools:
file picking convenience APIs, non-blocking feedback, collapsible control
groups, multiline text editing, compact status badges, and clearer callback/API
contracts.

This plan is intentionally pragmatic. It prioritizes small widgets and API
repairs that make existing examples and downstream tools easier to build
without forcing users into custom layout workarounds.

## Current Baseline

Already available:

- Blocking/overlay dialogs through `Modal`, plus `alert(...)` and
  `confirm(...)` helpers.
- Native file dialogs through `dg.FileDialog.open_file(...)`,
  `open_files(...)`, `save_file(...)`, and `pick_folder(...)`.
- Single-line text editing through `TextInput`.
- Navigation through `Pages`, `Page`, `Tabs`, `Tab`, `NavItem`.
- Data widgets through `DataFrameTable` and `Scatter3D`.
- Simple text tooltips through the common `tooltip=` widget argument.

Missing or weak:

- No top-level `dg.open_file_dialog(...)` / `dg.save_file_dialog(...)`
  convenience API.
- No native non-blocking toast/notification surface.
- No collapsible/accordion container for dense sidebars.
- No multiline text editor.
- No badge/count bubble support on common navigation/action widgets.
- No rich tooltip widget with arbitrary children.
- Modal open/close API has one extra method shape.
- `ColorPicker.set_value(...)` suppresses callbacks implicitly.
- `DataFrameTable` and `Scatter3D` have no interaction callbacks.
- `Pages.on_change` docs should state the exact contract.

## Priority Order

1. File dialog convenience API.
2. Toast/notification overlay.
3. Collapsible container.
4. TextArea.
5. Badge/tag support.
6. API contract cleanup.
7. Rich tooltip widget.
8. Data selection/picking callbacks.

## CSS And Styling Baseline

Each new widget should support the existing common styling surface:

- `id`
- `key`
- `class_`
- `style`
- `tooltip`
- CSS type selectors
- `:hover`, `:active`, `:focus`, `:disabled` where meaningful
- debug snapshot visibility

Only add CSS parts when a widget has stable internal renderer pieces worth
styling separately.

## Phase 1: File Dialog Convenience API

### Goal

Make file dialogs discoverable at the top-level API without requiring users to
know about `FileDialog`.

### Proposed API

```python
path = dg.open_file_dialog(
    title="Open CSV",
    filters=[("CSV", ["csv"]), ("All files", ["*"])],
)

paths = dg.open_files_dialog(title="Open data files")
path = dg.save_file_dialog(title="Export report")
folder = dg.pick_folder_dialog(title="Choose output folder")
```

Callback form should mirror `FileDialog`:

```python
dg.open_file_dialog(
    title="Open CSV",
    filters=[("CSV", ["csv"])],
    on_select=lambda path: status.set_value(path or "Canceled"),
    app=app,
)
```

### Implementation Notes

- Keep `FileDialog` as the underlying implementation.
- Add top-level functions in `python/dragongui/dialogs.py`.
- Export them from `python/dragongui/__init__.py`.
- Reuse existing `_backend.open_file_dialog`, `open_files_dialog`,
  `save_file_dialog`, and `pick_folder_dialog`.
- Preserve synchronous return when no callback is supplied.
- Preserve threaded callback behavior when `on_select` is supplied.

### Acceptance

- `dg.open_file_dialog(...)` works the same as `dg.FileDialog.open_file(...)`.
- `dg.save_file_dialog(...)` works the same as `dg.FileDialog.save_file(...)`.
- Existing `FileDialog` tests still pass.
- New Python tests cover top-level exports and callback dispatch.
- `docs/widgets.md` and `docs/library-overview.md` mention both APIs.

## Phase 2: Toast / Notification Overlay

Status: implemented in the first slice with `dg.toast(...)`, `App.toast(...)`,
`ToastHandle.update(...)`, `ToastHandle.dismiss()`, native command dispatch,
top-right native rendering, timeout expiry, debug snapshot state, tests, and
`examples/toast_tool.py`. Full CSS selector styling for native toasts remains a
future enhancement.

### Goal

Provide non-blocking ephemeral feedback for common operations:

- "Export complete"
- "Error saving file"
- "3 rows updated"
- "Connected"

### Proposed API

```python
dg.toast("Export complete")
dg.toast("Error saving file", level="error", duration=5000)
app.toast("Saved report.csv", level="success")
```

Optional handle form:

```python
toast = app.toast("Uploading...", level="info", duration=None)
toast.update("Upload complete", level="success", duration=2000)
toast.dismiss()
```

### Levels

| Level | Meaning |
| --- | --- |
| `info` | Neutral status. |
| `success` | Completed action. |
| `warning` | Recoverable issue. |
| `error` | Failed action. |

### Implementation Notes

- Prefer an app-level command queue API, because toasts are transient runtime
  overlays rather than document-tree widgets.
- Native runtime stores a small toast list with ids, message, level, created
  time, duration, and dismiss state.
- Primitive/text renderers draw toasts above normal content in a corner stack.
- Start with simple text-only toasts.
- Add `Toast::close`/dismiss hit target later if needed; first slice may be
  timeout-only.

### Styling

Add CSS type selector:

```css
Toast {
    background: surface_alt;
    border-color: border;
    border-radius: 8px;
    color: text;
}

Toast.error {
    background: danger;
    color: white;
}
```

If native toasts are not part of the widget tree, expose a small theme/style
slot or a dedicated toast style in debug snapshot. Do not block implementation
on full CSS integration.

### Acceptance

- Toasts can be triggered while the app is running.
- Multiple toasts stack without overlapping.
- Duration auto-dismiss works.
- `duration=None` creates a persistent toast until dismissed.
- Smoke demo shows success/warning/error toasts.
- Debug snapshot includes active toasts.

## Phase 3: Collapsible / Accordion Container

Status: implemented in the first slice with `dg.Collapsible(...)`, native
layout/state/rendering, pointer and keyboard toggling, `set_expanded(...)`,
`expand()`, `collapse()`, `toggle()`, `on_change`, CSS parts, tests, docs, and
`examples/collapsible_tool.py`. Animation and richer disclosure glyph styling
remain future refinements.

### Goal

Support dense sidebars and advanced settings without forcing every option to be
visible at once.

### Proposed API

```python
with dg.Collapsible("Advanced Settings", expanded=False):
    dg.Checkbox("Normalize data")
    dg.Slider(0.5)
```

Optional callback:

```python
dg.Collapsible(
    "Filters",
    expanded=True,
    on_change=lambda expanded: status.set_value(str(expanded)),
)
```

### Behavior

- Header row is always visible.
- Children participate in layout only when expanded.
- Header click toggles expanded state.
- Keyboard focus/activation should follow Button-like semantics.
- Disabled collapsible prevents toggling.

### Implementation Notes

- Python `Collapsible` should be a `Container`.
- Native `WidgetKind::Collapsible` needs state collection for expanded state.
- Layout should skip children when collapsed.
- Runtime command support should allow `set_expanded(...)`.
- CSS parts:

| Part | Meaning |
| --- | --- |
| `header` | Clickable header row. |
| `indicator` | Chevron/plus/minus glyph. |
| `body` | Child content region. |

### Acceptance

- Collapsed content consumes no height.
- Expanded content lays out exactly like a normal vertical container.
- Toggle works with pointer and keyboard.
- `set_expanded()` updates live native state.
- CSS can style header and indicator.
- Existing container layout tests are unaffected.

## Phase 4: TextArea / Multiline Text Input

Status: implemented in the first slice with `dg.TextArea(...)`, native
`WidgetKind::TextArea`, shared text state/callbacks, newline insertion, row-based
preferred height, optional wrapping, clipping, docs, tests, and a smoke example.
Internal scrolling and richer selection behavior remain out of scope for this
slice.

### Goal

Support multiline editing for notes, logs, SQL queries, prompts, and other data
tool text workflows.

### Proposed API

```python
editor = dg.TextArea(
    value="select * from table",
    rows=6,
    placeholder="SQL query...",
    on_change=lambda text: ...
)
```

Options:

| Argument | Purpose |
| --- | --- |
| `value` | Initial text. |
| `placeholder` | Empty-state text. |
| `rows` | Preferred visible line count. |
| `on_change` | Callback with full text. |
| `disabled` | Read-only/disabled interaction. |
| `wrap` | Whether long lines wrap. Default `True`. |

### Implementation Notes

- Add `WidgetKind::TextArea`.
- Start with plain text only; no rich text, syntax highlighting, or selection
  styling in the first slice.
- Reuse text input state where possible, but support newline insertion,
  multi-line caret metrics, vertical scrolling, and text clipping.
- If internal scrolling is too much for the first slice, support fixed visible
  rows and clipped content, then add scroll in the next pass.

### Acceptance

- Newlines can be entered and serialized.
- `set_value()` updates live text.
- `rows` affects intrinsic/preferred height.
- Placeholder appears only when empty.
- Text does not draw outside the widget bounds.
- `on_change` emits the full multiline value.

## Phase 5: Badge / Tag Support

Status: implemented in the first slice with inline `badge` support on
`Button`, `Tab`, and `NavItem`, live `set_badge(value)`, native badge pill/text
rendering, CSS `badge` parts, docs, tests, and a smoke example. Standalone
`Badge` / `Tag` widgets remain optional follow-up work.

### Goal

Allow compact status/count communication on navigation and action widgets.

### Proposed APIs

Inline badge argument on selected widgets:

```python
dg.Button("Filters", badge=3)
dg.NavItem("Errors", page="errors", badge=12)
dg.Tab("Jobs", badge="new")
```

Standalone badge/tag widget:

```python
dg.Badge("beta", level="info")
dg.Tag("running")
```

### Initial Scope

Add badge support to:

- `Button`
- `NavItem`
- `Tab`

Optional standalone `Badge` can follow if inline badges prove useful.

### Implementation Notes

- Badge values accept `str | int | None`.
- `None` hides the badge.
- Runtime text layer measures and draws badge text.
- Primitive layer draws badge pill background.
- Live `set_badge(value)` updates the badge.
- CSS parts:

| Widget | Part |
| --- | --- |
| `Button` | `badge` |
| `NavItem` | `badge` |
| `Tab` | `badge` |

### Acceptance

- Badge count/text appears without shifting the main label unexpectedly.
- `set_badge(None)` hides the badge.
- Badges fit single/two/three digit counts.
- CSS can style badge background/text.
- Existing Button/NavItem/Tab behavior remains unchanged when no badge is set.

## Phase 6: API Contract Cleanup

Status: implemented. Modal docs now prefer `show()` / `close()` while retaining
`set_open(...)` as a compatibility alias, `ColorPicker.set_value(...)` supports
`notify=False` by default and `notify=True` for callback emission, and
`Pages.on_change` docs/tests state that callbacks fire for native route changes
rather than construction-time state.

### Modal Open/Close

Current API:

```python
modal.set_open(True)
modal.show()
modal.close()
```

Plan:

- Keep `show()` and `close()` as the documented primary API.
- Keep `set_open(open: bool)` as a compatibility alias for now.
- Update docs to discourage direct `set_open(...)` unless callers are bridging
  boolean state.
- Do not remove `set_open(...)` until a future breaking-change window.

Acceptance:

- Docs list `show()` / `close()` first.
- Tests cover that `set_open(True)` and `show()` serialize/enqueue the same
  native prop update.

### ColorPicker.set_value

Current behavior suppresses `on_change` implicitly.

Proposed API:

```python
picker.set_value((255, 0, 0), notify=False)
picker.set_value((255, 0, 0), notify=True)
```

Plan:

- Add `notify: bool = False` keyword.
- Preserve current behavior by default.
- When `notify=True`, invoke `on_change` after internal sliders/labels update.
- Document that user interaction always invokes `on_change`.

Acceptance:

- Existing tests keep passing.
- New tests cover `notify=False` and `notify=True`.

### Pages.on_change Contract

Plan:

- State clearly: `Pages.on_change` fires when native interaction changes the
  active page route.
- Programmatic construction with `value=...` does not call `on_change`.
- Future `Pages.set_value(...)` should optionally support `notify=False`,
  matching ColorPicker style.

Acceptance:

- `docs/widgets.md` removes ambiguous "can emit" wording.
- Tests cover native callback registration for `Pages`.

## Phase 7: Rich Tooltip Widget

Status: implemented in the first slice with `dg.Tooltip(target=...)`, native
hover targeting, overlay layout, arbitrary child rendering, focus/hit-test
exclusion for tooltip content, docs, tests, and a smoke example.

### Goal

Support complex hover content such as previews, metrics, mini legends, and
small charts.

### Proposed API

```python
with dg.Tooltip(target=button):
    dg.Label("Rows: 1,240")
    dg.ProgressBar(0.72)
```

### Deferral Rationale

Simple `tooltip="..."` already handles the common case. Rich tooltips require
hover tracking, overlay layout, clipping, and arbitrary child rendering in a
popup surface. Defer until toast/collapsible/text area are done.

### Acceptance

- Tooltip can target a widget instance or id.
- Tooltip content is arbitrary DragonGUI children.
- Tooltip positions within the window bounds.
- Tooltip does not steal focus.

## Phase 8: Data Interaction Callbacks

### DataFrameTable Selection

Status: implemented in the first Phase 8 slice with
`DataFrameTable(on_select=...)`, `TableSelection`, native click emission,
selected-cell debug snapshot details, docs, and the table demo update.

Proposed API:

```python
dg.DataFrameTable(
    frame,
    on_select=lambda row, column, value: ...
)
```

Potential callback payload:

```python
TableSelection(
    row_index: int,
    column_index: int,
    column: str,
    value: object | str,
)
```

Acceptance:

- Clicked row/cell selection emits a callback.
- Selection is represented in debug snapshot.
- Keyboard/table navigation can be added later.

### Scatter3D Point Picking

Status: implemented with `Scatter3D(on_pick=...)`, `ScatterPick`, CPU
projection-based click picking against the retained point buffer, docs, and
tests. This avoids an ID-buffer pass for the first interaction model.

Proposed API:

```python
dg.Scatter3D(
    frame,
    x="x",
    y="y",
    z="z",
    on_pick=lambda point: ...
)
```

Potential callback payload:

```python
ScatterPick(
    index: int,
    x: float,
    y: float,
    z: float,
)
```

Acceptance:

- Pointer click near a rendered point returns the nearest point within a
  configured radius.
- Callback includes original row/point index.
- No meaningful performance regression for normal rendering.

## Documentation Tasks

Update:

- `docs/widgets.md`
- `docs/library-overview.md`
- relevant examples:
  - `examples/all_features_demo.py`
  - `examples/all_features_css_demo.py`
  - `examples/multipage_tool.py`

Add examples as features land:

- `examples/toast_tool.py`
- `examples/collapsible_tool.py`
- `examples/text_area_tool.py`

## Testing Strategy

### Python Tests

- Top-level dialog functions delegate to `FileDialog`.
- Toast API serializes/enqueues runtime commands.
- `Collapsible` validates children and expanded state.
- `TextArea` serializes multiline text and live `set_value`.
- Badge values serialize and live `set_badge`.
- ColorPicker `notify` behavior.
- Modal show/close/set_open compatibility.

### Rust Tests

- New widget kind parsing.
- Layout behavior for collapsed containers.
- TextArea text state and multiline input editing.
- Toast command parsing, timeout, and debug snapshot.
- Badge primitive/text emission.
- DataFrameTable and Scatter3D callback event payloads when those phases land.

### Smoke Tests

- Add new demos to a targeted smoke tool or extend existing demo smoke coverage.
- Ensure no CSS warnings and no layout audit issues.

## Risks

### Runtime Overlay Complexity

Toasts and rich tooltips are overlays, not normal layout children. Keep toast
text-only first to avoid building a general overlay layout system too early.

### TextArea Editing Scope Creep

Multiline text can quickly turn into a full editor. Keep the first version to
plain text editing, caret movement, newlines, clipping, and optional wrapping.

### API Compatibility

Avoid removing existing methods such as `Modal.set_open(...)`. Prefer
documenting primary APIs and preserving compatibility aliases.

### Data Picking Performance

Scatter point picking can be expensive if implemented naively. Defer until the
interaction model is clear and use spatial/ID buffers only if needed.

## Success Criteria

This roadmap is successful when:

- Users can open/save files through discoverable top-level APIs.
- Apps can show non-blocking feedback without building custom panels.
- Dense control panels can hide advanced sections.
- Users can edit multiline text.
- Navigation/actions can show counts or status tags.
- Modal, ColorPicker, and Pages behavior is explicitly documented.
- Future table/scatter interaction callbacks have a clear target shape.
