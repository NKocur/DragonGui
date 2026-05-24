# V4 SearchBox And CommandPalette

## Objective

Add fast filtering and command execution controls for larger tools.

## Proposed API

```python
dg.SearchBox(
    placeholder="Filter rows...",
    on_change=lambda query: ...
)
```

Command palette:

```python
dg.CommandPalette(
    commands=[
        dg.Command("open", "Open File...", on_run=open_file),
        dg.Command("export", "Export Report", on_run=export),
    ],
    open=False,
)
```

## Behavior

- SearchBox is a TextInput variant with clear button and search icon.
- CommandPalette opens as an overlay, filters commands, and runs selected
  command on Enter.
- Keyboard shortcut binding can be phase 2.
- Recent commands can be phase 2.

## Native Work

- SearchBox can extend TextInput with clear-button hit testing.
- CommandPalette needs modal-like overlay focus capture.
- Reuse text input state, selectable list rendering, and keyboard navigation.

## Python Work

- Add `SearchBox`.
- Add `Command` dataclass.
- Add `CommandPalette`.
- Add `app.open_command_palette()` convenience if useful.

## Acceptance

- SearchBox emits live query changes.
- Clear button resets query and emits callback.
- Palette filters commands case-insensitively.
- Enter runs the selected command.
- Escape closes the palette.

