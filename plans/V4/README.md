# V4 Widget Expansion Roadmap

## Objective

V4 should turn DragonGUI from a strong data-tool shell into a broader
desktop-tool framework. The focus is not widget count for its own sake; the
focus is the missing controls that unlock inspectors, editors, dashboards,
file browsers, debug tools, and operational apps.

This roadmap is based on the gap between the current DragonGUI widget set and
the common Dear ImGui widget patterns: selectable lists, tree nodes, drag
numeric controls, split panes, tool buttons, richer table controls, and
drag-and-drop interactions.

## Priority Order

1. `Selectable` and `SelectableList`
2. `RadioButton` and `RadioGroup`
3. `TreeView` and `TreeNode`
4. `DragNumber`, vector drags, and range sliders
5. `Splitter` and `ResizablePane`
6. `IconButton`, `ImageButton`, `SmallButton`, and `ArrowButton`
7. `PropertyGrid` / inspector
8. `SearchBox` and `CommandPalette`
9. `DataFrameTable` upgrades
10. Drag-and-drop API

## Secondary Backlog

- `ToggleSwitch`
- `DateInput`, `TimeInput`, and `DateTimeInput`
- `CodeEditor` and `LogView`
- `Breadcrumbs`
- `Toolbar`
- `LoadingSpinner`
- `ScatterPlot2D`
- `Heatmap`
- `BarChart`

## Common Requirements

Every new widget should support:

- `id`
- `key`
- `class_`
- `style`
- `tooltip`
- disabled state where meaningful
- CSS type selectors
- relevant pseudo states such as `:hover`, `:active`, `:focus`, `:selected`
- debug snapshot visibility
- smoke demo coverage

## Implementation Guidance

- Prefer reusing the existing document tree, layout, command queue, primitive,
  text, and renderer patterns.
- Add native widget kinds only when the control needs native state, hit testing,
  keyboard interaction, or custom rendering.
- Keep first versions useful and narrow. Defer advanced interactions unless
  the simple widget would feel broken without them.
- Add callback payload dataclasses on the Python side when callbacks need more
  than one value.
- Make live setters command-driven and optional-callback notification explicit
  with `notify=False` defaults.

## Documentation And Demo Plan

Add V4 examples as features land:

- `examples/selectable_list_tool.py`
- `examples/radio_group_tool.py`
- `examples/tree_view_tool.py`
- `examples/inspector_tool.py`
- `examples/splitter_tool.py`
- `examples/command_palette_tool.py`
- `examples/drag_drop_tool.py`

Update:

- `docs/widgets.md`
- `docs/library-overview.md`
- `examples/all_features_v3_demo.py` or a new V4 demo when enough widgets land

