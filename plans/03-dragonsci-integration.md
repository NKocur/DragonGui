# DragonSci Scatter Integration Plan

## Objective

Use DragonSci as DragonGUI's first differentiated capability.

The goal is not to rebuild `Scatter3D` from scratch. DragonSci already has the
valuable pieces: `wgpu` pipeline, point buffers, camera controls, colormaps, and
large scatter performance. DragonGUI should extract or share that core and
embed it inside its native window.

The installed Python 3.11 package is named `dragonsci` and should be used as
the behavior reference while the shared Rust boundary is defined.

## Work Items

### 1. Inventory DragonSci

Find:

- Where the installed Python 3.11 `dragonsci` package lives.
- Whether the source Rust crate is available locally.
- Which modules own `wgpu` device/surface creation.
- Which modules own scatter pipeline creation.
- Which modules own camera input.
- Which modules own colormaps and point buffer upload.
- Whether the current implementation assumes a standalone/Tkinter window.

Expected output:

- A short `plans/dragonsci-inventory.md` or code comment documenting reusable
  modules and blockers.

### 2. Define Shared Crate Boundary

Preferred shape:

```text
dragonsci-render/
  src/
    lib.rs
    scatter.rs
    camera.rs
    colormap.rs
    buffers.rs
```

DragonSci and DragonGUI should both depend on this crate.

The shared crate should not own:

- Python bindings.
- Tkinter integration.
- `winit` event loop.
- Top-level window creation.

The shared crate should own:

- Scatter pipeline setup.
- GPU point buffers.
- Camera math.
- Camera input interpretation where possible.
- Colormap tables and mapping.
- Render-pass drawing of scatter points into a provided `wgpu` target.

### 3. Adapt Embedded Rendering

DragonGUI owns the `wgpu::Device`, `wgpu::Queue`, `wgpu::Surface`, and
`winit` window. DragonSci's scatter core should render into a rectangle inside
DragonGUI's frame.

Required changes from standalone mode:

- Accept a borrowed `Device` and `Queue` instead of creating them internally.
- Accept render target format from DragonGUI.
- Accept viewport/scissor rectangle from layout.
- Render into DragonGUI's render pass or a subpass-compatible abstraction.
- Use DragonGUI input routing for camera events.
- Avoid owning the event loop.

### 4. Data Path

Stage the data path:

1. Generated float32 xyz points in Rust for the first embedded demo.
2. NumPy numeric columns from Python.
3. pandas and Polars numeric columns through NumPy or Arrow.
4. Arrow zero-copy path for Polars and Arrow-backed pandas.

Do not block the embedded renderer on perfect DataFrame interop.

## Python API

Keep the API stable while implementation changes underneath:

```python
scatter = dg.Scatter3D(df, x="x", y="y", z="z")
scatter.set_points(df, x="new_x", y="new_y", z="new_z")
```

Runtime updates must use the command queue:

- `scatter.set_points` enqueues `scatter.set_points`.
- Camera changes enqueue `scatter.set_camera`.
- The full widget document is not resent.

## Acceptance Criteria

- `examples/scatter_tool.py` opens a DragonGUI window with an embedded scatter.
- 500k generated points render interactively.
- Camera orbit, pan, zoom, and reset work inside the DragonGUI window.
- DragonGUI shares DragonSci scatter code, or a documented blocker explains why
  temporary duplication was necessary.
- Upload time and steady-state frame time are captured for the first benchmark.
