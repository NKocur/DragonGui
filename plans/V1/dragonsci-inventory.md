# DragonSci Inventory

Completed during M2 exploration (2026-04-24).

## Package Location

- Python executable: `C:\msys64\mingw64\bin\python3.11.exe` (Python 3.11 env)
- `dragonsci.__file__`: `C:\Users\nashk\AppData\Local\...\dragonsci\__init__.py`
- Source repo: `j:/Projects/DragonSci/src/` — Rust source tree

## Reusable Pieces

| Module | File | Status |
|--------|------|--------|
| Camera orbit/pan/zoom | `src/camera.rs` (124 lines) | **Copied** to `native/src/scatter/camera.rs` |
| Colormaps (12 maps) | `src/colormap.rs` (172 lines) | **Copied** to `native/src/scatter/colormap.rs` |
| Billboard point shader | `src/shaders/points.wgsl` (67 lines) | **Copied** to `native/src/scatter/points.wgsl` |
| Scatter pipeline | `src/scatter.rs` | Re-implemented in `native/src/scatter/mod.rs` for wgpu 29 |
| Point buffer upload | `src/scatter.rs::set_points` | Re-implemented |
| Colormaps | pure data | Shared by copy |
| Benchmarks | `benches/` | Not yet ported |

## Standalone Assumptions To Remove

| Assumption | Present? | Resolved by |
|------------|----------|-------------|
| Owns window | Yes — Tkinter parent passed at init | DragonGUI owns the window; scatter accepts `Device`/`Queue` |
| Owns event loop | No (Tkinter owns it) | N/A |
| Owns `wgpu::Device` | Yes | DragonGUI passes borrowed `&Device` |
| Owns `wgpu::Surface` | Yes | DragonGUI owns surface; scatter renders into provided pass |
| Assumes Tkinter parent | Yes | Removed; DragonGUI owns the OS window via winit |
| Assumes full-window viewport | Yes | DragonGUI passes full window for now; viewport/scissor TBD in M4 |

## Shared Crate Boundary Decision

**Shared crate: not created for M2.**

- **Blocker**: DragonSci uses `wgpu 24`; DragonGUI uses `wgpu 29`.  The wgpu
  types are not compatible across major versions (Surface, Device, Queue, etc.),
  so the scatter pipeline code cannot be shared as a compiled crate at this time.

- **Temporary duplication accepted**: `camera.rs` and `colormap.rs` are pure
  math/data with no wgpu dependency and were copied verbatim.  `points.wgsl`
  is copied verbatim.  `scatter/mod.rs` is a new wgpu-29 implementation of
  the same pipeline, following the same shader interface.

- **Future path**: When DragonSci is upgraded to wgpu 29, extract a
  `dragonsci-render` workspace crate (pure Rust, no Python bindings) and have
  both packages depend on it.

## Notes

- `PointInstance` struct: `{position:[f32;3], size:f32, color:[f32;3], alpha:f32}` (32 bytes, `Pod+Zeroable`)
- `Uniforms` struct: `{view_proj:[[f32;4];4], screen_size:[f32;2], style:u32, _pad:f32}` (80 bytes)
- Shader uses `VertexStepMode::Instance` with `draw(0..6, 0..N)` — one quad per instance
- Camera `fit(center, radius, aspect)` initialises a sensible view for any point cloud
- Style values: 0=circle (default), 1=square, 2=gaussian
