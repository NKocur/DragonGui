# Native Backend Plan

## Objective

Replace the current Rust scaffold with a real native runtime:

Python builds an initial widget document, Rust owns the window, event loop,
layout, rendering, and high-volume widget work.

The startup document is not the long-term update mechanism. After startup,
Python sends targeted commands through handles and a UI-thread queue.

## Proposed Rust Layout

```text
native/src/
  lib.rs              # PyO3 module exports
  app.rs              # run_app entrypoint and top-level orchestration
  document.rs         # typed startup document parsed from Python dicts
  commands.rs         # handle-based runtime mutations
  runtime.rs          # winit event loop, window lifecycle
  renderer.rs         # wgpu device, surface, frame rendering
  layout.rs           # taffy integration
  theme.rs            # theme tokens and defaults
  text.rs             # cosmic-text/glyphon integration, added after M3
  events.rs           # input events, hit testing, callback events
  error.rs            # Rust errors converted to Python exceptions
  dragonsci/
    mod.rs            # embedded DragonSci scatter adapter
  widgets/
    mod.rs
    button.rs
    checkbox.rs
    dropdown.rs
    label.rs
    panel.rs
    slider.rs
    text_input.rs
    scatter3d.rs
    dataframe_table.rs
```

## Dependencies To Add

M1:

- `winit`: native windows and input events.
- `wgpu`: GPU device, surface, render passes.
- `serde` and `serde_json`: typed startup document parsing from Python.
- `thiserror`: maintainable error types.
- `pollster`: simple blocking setup for async `wgpu` initialization.

M2 and later:

- DragonSci shared renderer crate or local adapter.
- `bytemuck`: safe buffer casts for GPU vertices.
- `taffy`: flexbox layout.
- `cosmic-text` or `glyphon`: Unicode shaping and GPU text rendering.

## M1 First Code Slice

Implement the smallest useful native path:

1. In Python, send `App.document(win)` into `_dragongui.run_app`.
2. In Rust, parse only `window.props.title`, `width`, and `height`.
3. Start a `winit` event loop.
4. Create a `wgpu` surface for the window.
5. Clear each frame to a neutral background.
6. Exit cleanly on close.

Do not implement widgets, text, or DragonSci in M1.

## macOS Smoke Test Requirement

Add macOS coverage as soon as M1 lands. `winit` has platform-specific event
loop rules, and macOS main-thread behavior should not be deferred until wheel
release work.

Minimum M1 CI coverage:

- Windows: build native module, import `dragongui`, run non-window smoke tests.
- macOS: build native module and run a window creation smoke path on the main
  thread when CI supports a graphical session, or a documented skip with
  compile coverage if it does not.
- Linux: compile native module; window smoke can wait until CI has a display
  strategy such as xvfb or Wayland headless support.

## Acceptance Criteria

- `app.run(dg.Window("DragonGUI Scatter Demo", width=1200, height=800))` opens
  a 1200x800 native window.
- The window presents frames without validation errors.
- Resizing reconfigures the surface.
- Closing the window returns control without a Python traceback.
- If native startup fails, Python receives a useful exception message.
- M1 CI proves Windows and macOS do not drift before M2 starts.

## Risks

- `winit` event loop ownership is strict, especially on macOS where UI work
  must be on the main thread.
- Python callbacks cannot directly mutate Rust UI state from arbitrary threads.
  Use a command queue.
- Text rendering can grow quickly. Keep M1 and M3 text-free.
