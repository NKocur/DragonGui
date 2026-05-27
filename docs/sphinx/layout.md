# Layout

DragonGUI layouts are native, retained widget trees. Prefer predictable
containers and stable dimensions over rebuilding large subtrees.

## Core Containers

- `Window`: top-level application surface.
- `Panel`: titled or untitled framed content area.
- `HLayout` and `VLayout`: horizontal and vertical stacking.
- `GridLayout`: responsive dashboard grids and masonry layouts.
- `FlowLayout`: wrapping rows for chips, tags, and compact command groups.
- `ScrollArea`: explicit scroll container for overflow content.
- `Splitter` and `Pane`: resizable split views.
- `Pages`, `Tabs`, and `Sidebar`: navigation structures.
- `Spacer` and `Separator`: lightweight spacing and visual dividers.

## Practical Rules

- Use `Panel(width=...)` for fixed sidebars.
- Use `GridLayout(masonry=True)` for cards with different natural heights.
- Put large tables and plots in stable containers so layout changes do not force
  unnecessary work.
- Prefer live setters for changing values, labels, plot data, and visibility.

See also the existing layout design notes in `../layout.md`.
