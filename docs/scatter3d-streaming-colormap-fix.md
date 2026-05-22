# Scatter3D Streaming Colormap Fix

## Symptom

In the V3 demo, changing the Scatter3D colormap worked when the plot was not
streaming. During prepared-frame streaming, the point cloud and scalar bar could
either fail to update, flicker between the selected colormap and `turbo`, or
briefly change and then snap back to `turbo`.

The final observed failure was:

- The widget selected colormap changed to something like `plasma`.
- Native debug state could briefly report the scalar bar as `plasma`.
- A few frames later, both the point cloud and scalar bar returned to `turbo`.

## Root Cause

There were two related state ownership problems.

### 1. Stream Frames Carried Stale Colormap State

Prepared stream frames were cached as compact `xyz_f32_v0` payloads. Those
payloads only contain positions, so their colors are produced by native decode
or by the compact shader using a colormap name.

The V3 demo initially built the stream with `turbo` payload metadata. When the
user changed the widget colormap while streaming, queued or cached frames could
still submit `turbo` as the frame colormap.

That made old frames compete with live colormap changes.

### 2. Native Retained Chrome Reapplied the Startup Colormap

The deeper issue was in native retained state.

`SetScatterScalarBar` updated the live `ScatterWidget`:

- visible
- range
- log scale
- colormap
- title

But it did not update the retained `WidgetNode.props` tree.

Later, `sync_scatter_style_overrides()` rebuilt `ScatterChromeState` from the
retained widget tree by calling `scatter_chrome_from_node()`. That retained tree
still had the original constructor prop:

```text
scalar_bar_colormap = "turbo"
```

So the chrome sync path overwrote the live scalar bar with stale retained state.
This is why the bug survived several stream-side fixes: the live command was
correct, but another native path was reapplying old props afterward.

## Fix

### Python Stream Ownership

`ScatterFrameStream` now tracks an explicit colormap override for compact
payloads.

When `Scatter3D.set_colormap()` runs, it updates the active stream before
submitting the native point and scalar-bar commands:

```text
active stream override -> widget colormap -> native point upload -> native scalar bar
```

Compact stream payloads are retagged with the override, and callback-handoff
frames recompute the current colormap when the callback runs instead of relying
on an older captured value.

The V3 demo also now clears stream payload cache on colormap changes and retags
cached compact frames by current colormap.

### Native Scalar Bar Ownership

Native point uploads no longer own the scalar-bar colormap.

For compact `xyz_f32_v0` frames:

- If the scalar bar is visible, native point upload uses the scalar bar's current
  colormap as the effective point colormap.
- The frame upload does not rewrite the scalar bar colormap.
- `SetScatterScalarBar` remains the owner of scalar-bar chrome.

This prevents ordinary stream frames from repainting the scalar bar back to an
older frame colormap.

### Retained Tree Synchronization

`SetScatterScalarBar` now also updates the retained widget-tree props through
`set_scatter_scalar_bar_props()`.

That helper updates both the parsed fields and raw props:

- `scatter_scalar_bar_visible`
- `scatter_scalar_bar_vmin`
- `scatter_scalar_bar_vmax`
- `scatter_scalar_bar_log_scale`
- `scatter_scalar_bar_colormap`
- `scatter_scalar_bar_title`
- matching `raw_props` entries such as `scalar_bar_colormap`

Now, when `sync_scatter_style_overrides()` later rebuilds chrome from the
retained tree, it sees the same colormap as the live widget and does not revert
to startup state.

### Command Coalescing

Pending `SetScatterScalarBar` commands are coalesced by scatter id in both the
command queue and runtime command batch. This keeps rapid colormap changes from
replaying stale scalar-bar commands after a newer one has already been queued.

## Files Changed

- `python/dragongui/widgets.py`
- `examples/all_features_v3_demo.py`
- `native/src/runtime.rs`
- `native/src/commands.rs`
- `tests/test_python_api.py`

Key implementation changes:

- Added active stream ownership to `Scatter3D.stream_prepared_frames()`.
- Added `ScatterFrameStream.set_colormap()`.
- Retagged compact stream payloads when the active colormap changes.
- Made callback-handoff frames use the latest compact colormap at callback time.
- Made compact native point uploads follow the visible scalar bar's colormap.
- Added `set_scatter_scalar_bar_props()` to keep retained props in sync.
- Updated `SetScatterScalarBar` to patch retained props before refreshing live
  widget chrome.
- Added scalar-bar command coalescing.
- Added V3 demo diagnostics for widget/native colormap state and recent
  colormap commands.

## Verification

Focused Python tests:

```powershell
C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe -m pytest tests\test_python_api.py -k "scatter_frame_stream or scatter_enqueue_compact_prepared_override_updates_scalar_bar_colormap or scatter_enqueue_compact_prepared_without_metadata_uses_widget_colormap or scatter_set_colormap_updates_tracking_scalar_bar"
```

Result:

```text
8 passed
```

Focused native scalar-bar tests:

```powershell
$env:PYO3_PYTHON='C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe'
cargo test --manifest-path native\Cargo.toml scalar_bar --lib
```

Result:

```text
7 passed
```

Broader native scatter tests:

```powershell
$env:PYO3_PYTHON='C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe'
cargo test --manifest-path native\Cargo.toml scatter --lib
```

Result:

```text
61 passed; 1 ignored
```

Manual verification:

- Rebuilt native.
- Ran the V3 demo.
- Started Scatter3D prepared-frame streaming.
- Changed the colormap while streaming.
- Confirmed the point cloud and scalar bar stayed on the selected colormap and
  no longer flickered or reverted to `turbo`.

