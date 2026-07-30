# Widgets

DragonGUI includes controls, data widgets, plots, navigation widgets, and
extension points.

## Common Controls

- Text and status: `Label`, `Badge`, `Tag`, `LED`, `StatusBar`, `ProgressBar`,
  `LoadingSpinner`.
- Buttons: `Button`, `SmallButton`, `IconButton`, `ImageButton`,
  `ArrowButton`.
- Choices: `Checkbox`, `RadioButton`, `RadioGroup`, `ToggleSwitch`,
  `Dropdown`, `Selectable`, `SelectableList`.
- Numeric inputs: `Slider`, `RangeSlider`, `NumberInput`, `DragNumber`,
  `DragVector`.
- Text and date inputs: `TextInput`, `TextArea`, `SearchBox`, `CodeEditor`,
  `LogView`, `DateInput`, `TimeInput`, `DateTimeInput`.
- Composite controls: `ColorPicker`, `PropertyGrid`, `Property`.

Composite widgets expose their own public CSS type and their public base-type
chain. For example, `ColorPicker` can be styled through `ColorPicker` for its
outer contract and also participates in intentional `Panel` rules.
- Commands and navigation controls: `CommandPalette`, `Breadcrumbs`,
  `TreeView`, `TreeNode`.

`SearchBox` has an explicit composite-sizing contract: its default preferred
width is 340 logical pixels, it shrinks to a 180-pixel minimum, and
`grow=True` consumes remaining row or toolbar space. Use `width=None` for an
intrinsic starting size, `max_width` to cap growth, and `clearable=False` to
release the clear-button slot. Inline `style` sizing overrides these defaults.

`IconButton` uses semantic native-vector names. `resolve_icon(name)` reports
the canonical built-in identity, alias use, and whether the request will fall
back to `more`; `BUILTIN_ICONS` and `ICON_ALIASES` expose the current catalog.
Use `IconResource` and `IconStroke` with `App.set_icon_theme()` to replace
monochrome geometry before startup or live while retaining CSS tint and
layout. Live `IconButton.set_icon()` changes use the same retained registry.

## Data And Visualization

- `DataFrameTable`
- `LinePlot`
- `Scatter3D`
- `ScatterPlot2D`
- `Heatmap`
- `Histogram`
- `BarChart`
- `PieChart`
- `Image`
- `HtmlReport`

## Containers And Navigation

- Root and structural containers: `Window`, `Panel`, `Pane`, `ScrollArea`,
  `Splitter`, `Separator`, `Spacer`.
- Layout containers: `FlexLayout`, `HLayout`, `VLayout`, `FlowLayout`,
  `GridLayout`.
- Page navigation: `Tabs`, `Tab`, `Pages`, `Page`, `Sidebar`, `NavItem`.
- Menus and toolbars: `MenuBar`, `Menu`, `MenuItem`, `ContextMenu`,
  `Toolbar`, `ToolbarSeparator`.
- Overlays: `Modal`, `Tooltip`, `Collapsible`.

## Drag, Drop, And Extensions

- Drag/drop widgets: `DragSource`, `DropTarget`, `DropZone`.
- Extension points: `ExtensionWidget`, `PaintWidget`.

For generated API signatures, see [Widgets API](api/widgets).
