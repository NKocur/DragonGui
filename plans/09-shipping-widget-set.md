# Shipping Widget Set Plan

## Objective

Round out DragonGUI from a promising data-tool shell into something credible
for real desktop applications before packaging broadly.

The current widget set can build demos and simple tools. The next set should
cover the everyday controls users expect in data applications:

- Numeric input for precise parameters.
- Progress and status feedback for long operations.
- Layout polish primitives.
- Tooltips and contextual help.
- Modals and menus.
- File and image workflows.
- Color selection.

This plan deliberately separates low-risk immediate widgets from features that
need the command queue or operating-system integration.

## Priority Tiers

### Tier 1: Must-Have Before Shipping

These should land before a TestPyPI/public pre-alpha release:

- `NumberInput`
- `ProgressBar`
- `Separator`
- `Spacer`
- `Tooltip`
- `Modal` / dialog

### Tier 2: Expected By Real Tool Builders

These can follow immediately after Tier 1, but should still be planned before a
serious public release:

- `MenuBar`, `Menu`, `MenuItem`
- `ContextMenu`
- `Collapsible`
- `FileDialog`
- `ColorPicker`
- `Image`
- `StatusBar`

## Public API Shape

### Number Input

```python
gain = dg.NumberInput(
    value=0.001,
    min=0.0,
    max=1.0,
    step=0.001,
    precision=4,
    on_change=lambda value: print(value),
)
```

Behavior:

- Supports int and float values.
- Text entry for precise values.
- Increment/decrement buttons.
- Up/Down arrows adjust by `step`.
- Shift+Up/Down adjust by `step * 10`.
- Ctrl+Up/Down adjust by `step * 0.1`.
- Optional drag-to-change can come after typed input is stable.
- Values clamp to min/max on commit.
- Invalid text keeps focus and uses danger styling until fixed.

Document props:

```python
{
    "type": "number_input",
    "props": {
        "value": 0.001,
        "min": 0.0,
        "max": 1.0,
        "step": 0.001,
        "precision": 4,
        "disabled": False,
        "events": ["change"]
    }
}
```

### Progress Bar

```python
progress = dg.ProgressBar(value=0.72, label="Loading...")
```

Behavior:

- `value` normalized to `0.0..1.0`.
- Optional label.
- Optional indeterminate mode later.
- Background-thread updates require the command queue:
  `app.call_soon_threadsafe(lambda: progress.set_value(next_value))`.

Document props:

```python
{"type": "progress_bar", "props": {"value": 0.72, "label": "Loading..."}}
```

### Separator And Spacer

```python
dg.Separator()
dg.Spacer(height=8)
dg.Spacer(width=12)
```

Behavior:

- `Separator` draws a thin theme border line.
- Orientation is inferred from parent layout by default, with explicit
  `orientation="horizontal" | "vertical"` override.
- `Spacer` reserves fixed logical space without rendering.

### Tooltip

```python
dg.Button("Run", tooltip="Run the analysis on selected rows")
dg.DataFrameTable(df, tooltip="Right-click rows for table actions")
```

Behavior:

- `tooltip` is a common prop supported by every widget.
- Hover delay before opening.
- Tooltip follows the hovered widget, not the cursor.
- Tooltip overlays everything else and is clipped to the window.
- Basic text-only tooltips first; rich tooltips can come later.

### Modal / Dialog

```python
dg.modal(
    "Delete selected rows?",
    title="Confirm Delete",
    buttons=["Delete", "Cancel"],
    danger="Delete",
    on_close=lambda button: print(button),
)
```

Declarative form for static startup dialogs:

```python
with dg.Modal("confirm_delete", title="Confirm Delete", open=False):
    dg.Label("Delete selected rows?")
    dg.Button("Delete")
    dg.Button("Cancel")
```

Behavior:

- Modal overlay blocks input to the rest of the UI.
- Escape closes if `closable=True`.
- Enter activates default button if configured.
- Dialog open/close must be command-driven after startup.
- `dg.alert`, `dg.confirm`, and `dg.prompt` can be convenience wrappers after
  the core modal exists.

### Menu Bar

```python
with dg.MenuBar():
    with dg.Menu("File"):
        dg.MenuItem("Open...", on_click=open_file)
        dg.MenuItem("Save", on_click=save)
        dg.MenuItem("Exit", on_click=app.stop)
    with dg.Menu("Help"):
        dg.MenuItem("About", on_click=show_about)
```

Behavior:

- Top-level fixed-height menu bar.
- Menus open on click, close on outside click/Escape.
- Menu items fire click callbacks.
- Separators work inside menus.
- Keyboard navigation can be staged after mouse behavior.

### Context Menu

```python
with dg.ContextMenu(target=table):
    dg.MenuItem("Copy row", on_click=copy_row)
    dg.MenuItem("Filter by value", on_click=filter_value)
```

Behavior:

- Opens on right-click over target widget.
- Anchors at cursor position.
- Uses same menu item renderer as `MenuBar`.
- Target-specific payloads can come later; first version just fires callbacks.

### Collapsible

```python
with dg.Collapsible("Advanced Settings", expanded=False):
    dg.NumberInput(value=0.001, step=0.001)
    dg.Checkbox("Use robust fit")
```

Behavior:

- Header toggles expanded state.
- Hidden contents are not rendered or hit-tested.
- State survives page/tab switches.
- Keyboard activation with Enter/Space.

### File Dialog

```python
dg.FileDialog.open_file(
    title="Open CSV",
    filters=[("CSV", "*.csv"), ("All files", "*.*")],
    on_select=load_csv,
)
```

Behavior:

- Use Rust `rfd` crate for native file dialogs.
- Support open file, open multiple, save file, pick folder.
- Dialog invocation must be asynchronous from the UI perspective.
- Return `None` on cancel.
- Keep this as an API service rather than a widget tree node for v1.

### Color Picker

```python
dg.ColorPicker(value=(255, 100, 0), alpha=True, on_change=set_color)
```

Behavior:

- RGBA value model.
- Text/number fields for channels can reuse `NumberInput`.
- Small swatch preview.
- HSV rectangle can come after simple RGB sliders.

### Image Display

```python
dg.Image(path="chart.png", fit="contain")
dg.Image(array=rgba_array, width=512, height=512)
```

Behavior:

- Path-backed image using the `image` Rust crate.
- NumPy RGBA upload path later.
- Fit modes: `contain`, `cover`, `stretch`, `none`.
- Optional nearest/linear filtering.
- Resize-aware layout.

### Status Bar

```python
with dg.StatusBar():
    dg.Label("Ready")
    dg.Spacer()
    dg.Label("500,000 rows")
```

Behavior:

- Fixed-height bottom strip.
- Usually direct child of `Window` or app shell.
- Supports labels, progress bars, and compact buttons.
- Does not need special event behavior beyond layout.

## Native Architecture

### Document Model

Add `WidgetKind` variants:

- `NumberInput`
- `ProgressBar`
- `Separator`
- `Spacer`
- `Modal`
- `MenuBar`
- `Menu`
- `MenuItem`
- `ContextMenu`
- `Collapsible`
- `ColorPicker`
- `Image`
- `StatusBar`

Extend `NodeProps` with:

- `tooltip: Option<String>`
- `label: Option<String>` or reuse `text`
- `orientation: Option<String>`
- `width`, `height`, `min_width`, `min_height`
- numeric fields: `value`, `min`, `max`, `step`, `precision`
- modal/menu fields: `open`, `closable`, `default`
- image fields: `path`, `fit`, `filter`

### Layout

- `Spacer`: fixed width/height, zero rendering.
- `Separator`: 1 logical pixel line with parent-orientation-aware sizing.
- `ProgressBar`: fixed control height.
- `NumberInput`: fixed control height.
- `StatusBar`: fixed bottom strip; initially just a horizontal layout with
  fixed height.
- `Modal`: overlay layout independent from normal taffy tree.
- `MenuBar`: fixed top strip; menus use overlay layout.
- `Collapsible`: header height when closed, header + children when expanded.
- `Image`: growable leaf with optional aspect ratio.

### Rendering

Reuse primitive rectangle and text renderers where possible:

- `NumberInput`: text box + stepper button regions.
- `ProgressBar`: track + filled region + optional centered label.
- `Separator`: line.
- `Tooltip`: overlay surface + text.
- `Modal`: dim overlay + centered panel + children.
- `MenuBar`/menus/context menus: surfaces, hover/active rows, text.
- `Collapsible`: header row + chevron + child content.
- `ColorPicker`: swatch and RGB slider first; HSV later.
- `Image`: dedicated textured-quad renderer.

### Events

Add hit-test support for:

- Number input text area and stepper buttons.
- Modal overlay and modal child controls.
- Menu popups and context menus.
- Tooltip hover delay.
- Collapsible header.
- Color picker sliders/swatches.

Extend `WidgetState` with:

- `number_values`
- `number_text`
- `number_invalid`
- `progress_values`
- `open_modal`
- `open_menu`
- `open_context_menu`
- `collapsible_expanded`
- `tooltip_hover`
- `tooltip_open_since`
- `image_handles`

## Command Queue Dependency

These widgets should not all block on the command queue, but updates from
background work require it.

Can be implemented before the command queue:

- `Separator`
- `Spacer`
- `NumberInput` startup value and direct interaction
- `ProgressBar` startup value
- `Tooltip` static text
- `Collapsible`
- Basic `MenuBar` / `MenuItem`
- Static `Image(path=...)`
- `StatusBar`

Should wait for or land with the command queue:

- `ProgressBar.set_value` from background threads.
- Modal open/close from callbacks.
- File dialog result callbacks.
- Image array updates.
- Status bar updates from background operations.
- Programmatic menu/context menu open.

## Implementation Slices

Current implementation:

- W0 is implemented: `Separator`, `Spacer`, and `StatusBar` exist in the
  Python API, serialize into the document, lay out in Rust, render with native
  primitives, and are covered in the multipage demo.

### W0: Micro Layout Widgets

Deliver:

- `Separator`
- `Spacer`
- Basic `StatusBar`
- Demo coverage in `examples/multipage_tool.py`

Acceptance:

- Control panels can be visually grouped without fake labels or empty widgets.
- Spacing is stable on resize and HiDPI.
- No event behavior needed.

### W1: Number Input

Deliver:

- Python `NumberInput`.
- Rust state and rendering.
- Text entry, clamping, stepper buttons.
- Keyboard adjustment.
- `on_change` callback.

Acceptance:

- Typing `0.001` works.
- Increment/decrement buttons clamp to min/max.
- Invalid entry uses danger styling and does not emit.
- Python handle updates through the callback wrapper.

### W2: Progress Bar And Command Queue Hook

Deliver:

- Python `ProgressBar`.
- Rust rendering.
- Initial static values.
- Command API path for `progress.set_value(value)`.

Acceptance:

- A background thread can update a progress bar without touching renderer
  state directly.
- Progress text and filled region update without full document replacement.

### W3: Tooltips

Deliver:

- Common `tooltip` prop on all widgets.
- Rust hover tracking and delayed overlay.
- Text rendering in overlay.

Acceptance:

- Tooltip appears over buttons, dropdowns, labels, table, and scatter widgets.
- Tooltip does not steal clicks.
- Tooltip stays within window bounds.

### W4: Modal Dialogs

Deliver:

- `Modal` container.
- `dg.alert`, `dg.confirm` convenience APIs.
- Overlay input blocking.
- Open/close commands.

Acceptance:

- A destructive button can show a confirmation modal.
- Background UI behind the modal is not hit-tested.
- Escape/default button behavior is correct.

### W5: Menus And Context Menus

Deliver:

- `MenuBar`, `Menu`, `MenuItem`.
- `ContextMenu(target=...)`.
- Shared popup menu renderer.
- Mouse interaction first, keyboard second.

Acceptance:

- File/Edit/Help style menu bar works.
- Right-clicking a table opens a context menu.
- Menu items fire callbacks exactly once.

### W6: File Dialog

Deliver:

- Add Rust `rfd` dependency.
- `FileDialog.open_file`, `open_files`, `save_file`, `pick_folder`.
- Callback result path through command queue.

Acceptance:

- Example opens a CSV path and displays the selected path in the UI.
- Cancel is distinguishable from selecting an empty path.
- No renderer freeze while dialog is open.

### W7: Color Picker

Deliver:

- `ColorPicker` with RGBA channel controls.
- Swatch preview.
- `on_change` callback.

Acceptance:

- Can edit RGB and alpha values.
- Emits normalized or integer values consistently.
- Theme/sample chart color can be changed in an example.

### W8: Image Display

Deliver:

- Textured quad renderer.
- `Image(path=...)`.
- Fit modes.
- Later: `Image(array=...)` NumPy upload.

Acceptance:

- PNG/JPEG images display in a layout cell.
- Image resizes correctly.
- Missing path shows a styled error placeholder, not a panic.

## Testing Strategy

Python tests:

- Serialization shape for every widget.
- Validation for invalid numeric ranges, invalid file dialog filters, duplicate
  menu item ids where relevant.
- Callback registration for `NumberInput`, `ColorPicker`, menus, dialogs.

Rust tests:

- Layout rects for spacer/separator/status bar/collapsible.
- State transitions for number input, progress, modal, menu, collapsible.
- Hit testing for popups and modal input blocking.

Smoke examples:

- `examples/widget_gallery.py`: every shipping widget on one screen.
- `examples/long_task_tool.py`: progress bar + status bar + background update.
- `examples/file_image_tool.py`: file dialog + image display.

## Acceptance Criteria For Shipping

- The widget gallery demonstrates all Tier 1 widgets.
- Long-running task demo updates progress from a background thread.
- Destructive action demo uses a modal confirmation.
- Tooltips work on at least five different widget classes.
- Menus and context menus work in the DataFrame demo.
- File dialog can open a local CSV path.
- Image widget displays a PNG and handles missing files gracefully.
- All new widgets follow dark/light theme tokens.
- Hidden overlays and inactive modal backgrounds do not receive input.
- No new widget requires Python per-frame rendering.
