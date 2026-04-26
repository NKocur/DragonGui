# Reactive Engine Demo Checkpoints

Every feature slice in this effort should leave behind a runnable GUI demo that
makes the change visible. Passing tests alone is not enough for UI work.

## Current Demos

### R1: Live Basic Control Updates

File:

```powershell
python examples\live_update_tool.py
```

What it should show:

- `Apply Python Update` changes a text input, slider, dropdown, and checkbox.
- `Start Background Updates` changes those controls every 500ms through
  `app.call_soon_threadsafe(...)`.
- The UI updates without rebuilding or restarting the native window.

### R2: Live Scatter Updates

File:

```powershell
python examples\streaming_scatter_tool.py
```

What it should show:

- A button calls `scatter.set_points(...)` while the window is already running.
- The rendered point cloud visibly changes without restarting.
- A background thread can stream new point data at a fixed interval.
- `Print Metrics` prints the latest live scatter pack, queue, decode, upload,
  and native apply timings from `app.debug_snapshot()`.

### R3: Structured Inline Styles

File:

```powershell
python examples\style_showcase.py
```

What it should show:

- Padding, gap, width, color, border, radius, accent, font size, font family,
  font weight, and text alignment are set from Python.
- Hover, active, focus, and disabled pseudo-state styles are visible.
- Dark/light theme token resolution is easy to compare.

Current limitation:

- This R3 slice supports text color, font size, font family, font weight, and
  left/center/right text alignment.

### R4: Live Style Patches

File:

```powershell
python examples\live_style_tool.py
```

What it should show:

- `Cycle Live Style` updates a panel, label, and button while the window is
  running.
- The patch exercises visual, text, and layout style keys.
- `Clear Style` removes the live styles without rebuilding the window.

### R4: Live Static Child Replacement

File:

```powershell
python examples\live_children_tool.py
```

What it should show:

- `Swap Static Children` replaces the preview panel's child subtree while the
  window is running.
- The replacement includes labels, separators, spacers, and per-widget styles.
- New callback-bearing widgets inserted by `replace_children(...)` should work;
  use the dynamic action buttons in the demo to verify callback registration.

### R5: Root Components And Keyed State

File:

```powershell
python examples\component_counter_tool.py
```

What it should show:

- `Increment` updates keyed component state, rerenders the root component, and
  patches a live label.
- `Cycle Tone` updates state that changes style, then applies live style
  patches.
- Stable widget ids/keys keep the retained native nodes intact across rerenders.

### R5: Nested Components And State

File:

```powershell
python examples\component_nested_tool.py
```

What it should show:

- `Increment Alpha` and `Increment Beta` update child component local state.
- `Rerender Parent` updates parent state without resetting the child counts.
- `Cycle Parent Tone` passes new props to both child components while their
  keyed state remains intact.

Current limitation:

- Nested component calls require explicit `key=...`.

### R5/R4 Hardening: Node Replacement

File:

```powershell
python examples\component_node_swap_tool.py
```

What it should show:

- `Swap Root Node` changes the component root identity and applies a live
  `REPLACE_NODE` patch.
- `Swap Back` proves callbacks in the replacement tree are registered and still
  work.
- The retained native tree relayouts without restarting the window.

### R6: Debug Snapshot

File:

```powershell
python examples\debug_snapshot_tool.py
```

What it should show:

- A background thread can call `app.debug_snapshot()` while the window is live.
- The final `App.run(...)` result includes a `debug_snapshot` dictionary.
- The snapshot includes widget tree, layout rects, runtime state, command queue
  depth, theme tokens, and frame/upload timing.

Current limitation:

- Do not call `app.debug_snapshot()` from a UI callback yet; it can timeout
  because the event loop is blocked by that callback.

### Data Slice: Live Table Updates

File:

```powershell
python examples\live_table_tool.py
```

What it should show:

- `Load Alpha/Beta/Gamma` calls `DataFrameTable.set_frame(...)` while the
  window is running.
- The table updates in place without restarting the native window.
- The live path sends bounded table metadata and sample cells, not the full
  DataFrame.

## Required Future Demos

### R5 Later: Data-Aware Nested Components

Add or update a demo where:

- A keyed component owns local state.
- A parent rerender preserves child component state.
- A dropdown controls a live scatter widget through component state.

Recommended file:

```powershell
python examples\component_showcase.py
```
