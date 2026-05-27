# Raspberry Pi Touchscreen Gesture Support Plan

## Goal

Add practical touchscreen support for DragonGUI on Raspberry Pi touch displays, especially HDMI + USB touch monitors used as dashboard/tablet displays.

The first target is the WASP dashboard:

- Touch buttons, sliders, checkboxes, and compact controls reliably.
- Use one-finger Scatter3D rotation.
- Use two-finger Scatter3D pinch zoom.
- Optionally use two-finger midpoint drag for Scatter3D pan.
- Preserve existing mouse and wheel behavior.

## Current State

- The runtime handles `MouseInput`, `CursorMoved`, and `MouseWheel` in `native/src/runtime.rs`.
- Buttons and most UI controls are driven by pointer-style mouse press/release behavior.
- Scatter3D currently uses mouse drag for orbit/pan/selection and mouse wheel for zoom.
- There is no explicit `WindowEvent::Touch` handling yet.
- Touchscreens connected over HDMI still need USB touch input. HDMI only carries video.

## Non-Goals For First Pass

- Full multi-touch support for every widget.
- OS-level virtual keyboard support.
- Gesture recognition for LinePlot/Histogram beyond possible future pinch zoom.
- Inertial scrolling or kinetic pan physics.
- Complex gesture customization in Python API.

## Design

### 1. Add Touch State

Add a small touch tracking structure in `native/src/runtime.rs`.

Track active touches by `TouchId`:

- `id`
- current position in logical pixels
- previous position
- start position
- phase/state
- start time

Also track a current gesture:

- `target_widget_id`
- `target_kind`
- `mode`: `UiTap`, `ScatterOrbit`, `ScatterPinch`, `ScrollDrag`, etc.
- initial two-finger distance for pinch
- previous two-finger midpoint
- active scatter id, if any

Keep this isolated from mouse state so touch cannot leave stale `orbit_active`, `pressed_id`, or scrollbar drag state behind.

### 2. Handle `WindowEvent::Touch`

Add a new match arm in `DragonRuntime::window_event`:

- `TouchPhase::Started`
- `TouchPhase::Moved`
- `TouchPhase::Ended`
- `TouchPhase::Cancelled`

Convert physical touch coordinates to the same logical coordinate space used by mouse events. Confirm whether winit already provides logical coordinates for this backend; if not, divide by scale factor.

### 3. One-Finger UI Tap Path

For one-finger start outside Scatter3D:

- perform the same hit testing used by mouse press
- set pressed visual state
- on touch end, activate if released over the same widget

Reuse as much existing mouse press/release code as possible by extracting helper methods:

- `handle_primary_press(pos)`
- `handle_primary_release(pos)`
- `handle_primary_move(pos)`

This avoids duplicating button, dropdown, slider, table, number-input, menu, and modal behavior.

### 4. One-Finger Scatter3D Orbit

For one-finger start over Scatter3D:

- set active scatter id
- enable interaction LOD
- store start position
- on move, call the same camera orbit path currently used by mouse drag:
  - `runtime.widget.camera.orbit(delta)`
  - `runtime.widget.update_camera(...)`
  - refresh grid/overlays
  - emit `camera_changed`

Use a movement threshold before treating the touch as orbit so a tap can still pick a point if picking is enabled.

Suggested threshold:

- `4 px` movement before orbit starts
- same or slightly larger than current mouse click movement check

### 5. Two-Finger Scatter3D Pinch Zoom

When two active touches are both over the same Scatter3D:

- switch gesture mode to `ScatterPinch`
- calculate initial distance between touches
- calculate current distance on every move
- convert distance ratio to camera zoom

Suggested mapping:

- `ratio = current_distance / previous_distance`
- zoom amount can be `ln(ratio) * sensitivity`
- use existing `camera.zoom(...)` path if possible

Positive/negative direction must be tested on actual hardware because touch gesture sign is easy to invert.

### 6. Two-Finger Scatter3D Pan

After pinch zoom is stable, add midpoint pan:

- previous midpoint = average of two touch positions
- current midpoint = average of two touch positions
- midpoint delta drives `camera.pan(delta)`

This should be optional internally at first. If pinch + pan together feels jumpy, split behavior:

- two-finger distance change above threshold -> zoom
- two-finger midpoint move above threshold with stable distance -> pan

### 7. Touch Scrolling

For scrollable panels and sidebars:

- one-finger drag over scroll container should scroll content
- direction should match normal touch behavior: finger drag down moves content down, which means scroll offset usually decreases

Implementation path:

- reuse `scroll_container_at_pos`
- call existing `scroll_container(...)`
- add a small movement threshold so taps on buttons inside a scroll panel still click

This is important because the WASP dashboard uses narrow scrollable panels.

### 8. Control Hit Targets

Touch hardware will expose small-control usability issues even if events work correctly.

Recommended touch mode CSS/profile:

- buttons: minimum `32 px` height
- sliders: minimum `32 px` height
- checkboxes: minimum `28-32 px` row height
- scrollbar thumbs: wider hit area or touch drag support independent of visual width

For the WASP dashboard, add a later `--touch-ui` flag or CSS class rather than changing the compact Pi layout unconditionally.

## Implementation Steps

### Step 1: Input Refactor

- Extract mouse primary press/release/move logic into helper methods.
- Keep behavior identical for mouse.
- Add unit tests where possible around state transitions that do not require winit events.

Status: `pending`

### Step 2: Basic Touch Tap

- Add `WindowEvent::Touch` handling.
- Convert `Started`/`Ended` into the same press/release helper path for single-touch UI.
- Test buttons, checkboxes, sliders, dropdowns, and scrollbars on touchscreen hardware.

Status: `pending`

### Step 3: One-Finger Scatter Orbit

- Route one-finger movement over Scatter3D to existing orbit logic.
- Preserve tap-to-pick if movement stays below threshold.
- Ensure interaction LOD begins on gesture start and ends on touch release/cancel.

Status: `pending`

### Step 4: Two-Finger Scatter Pinch Zoom

- Track two active touches.
- Add pinch distance calculation.
- Apply zoom through existing camera update path.
- Tune sensitivity on actual Pi touchscreen.

Status: `pending`

### Step 5: Two-Finger Scatter Pan

- Track midpoint movement.
- Apply camera pan.
- Tune thresholds so pinch and pan do not fight each other.

Status: `pending`

### Step 6: Touch Scroll Containers

- Add one-finger drag scrolling for scrollable containers.
- Ensure child buttons still receive taps when movement is below drag threshold.
- Test the left navigation and WASP right-side dashboard panels.

Status: `pending`

### Step 7: Touch UI Mode

- Add optional WASP/example touch sizing mode.
- Candidate CLI flag: `--touch-ui`.
- Increase compact button/slider/control heights only when enabled.

Status: `pending`

### Step 8: Bench And Regression Pass

- Verify no measurable overhead when no touch events occur.
- Run native tests.
- Smoke run the WASP dashboard.
- Test on actual HDMI + USB touchscreen.

Status: `pending`

## Files Likely To Change

- `native/src/runtime.rs`
  - winit touch event handling
  - touch gesture state
  - extracted pointer helpers
- `native/src/events.rs`
  - optional persistent touch/gesture state helpers if runtime state grows too large
- `native/src/scatter/mod.rs`
  - likely no first-pass changes if existing camera methods are enough
- `examples/rpi_v4l2_phase_scatter.py`
  - optional `--touch-ui` layout mode
- `plans/RPi/touchscreen-gesture-support-plan.md`
  - implementation tracker

## Test Plan

Automated:

- `cargo test --manifest-path native/Cargo.toml runtime --lib`
- `cargo test --manifest-path native/Cargo.toml scatter --lib`
- `cargo check --manifest-path native/Cargo.toml --features pyo3/extension-module,pi`
- `python -m py_compile examples/rpi_v4l2_phase_scatter.py`

Manual hardware:

- Tap every visible button in WASP dashboard.
- Tap/drag sliders.
- Scroll left navigation.
- Scroll right dashboard panel.
- One-finger rotate Scatter3D.
- Two-finger pinch zoom Scatter3D.
- Two-finger pan Scatter3D.
- Confirm mouse still works after touch interactions.
- Confirm touch cancel/release does not leave the plot stuck rotating.

## Risks

- Winit touch coordinate units may differ by backend; verify X11/XWayland and Wayland behavior.
- Some HDMI touch monitors emulate mouse events instead of real touch events. In that case basic taps may already work, but two-finger gestures may not be available.
- Compact WASP controls may be too small for touch even if event handling is correct.
- Gesture state can conflict with existing mouse state if not isolated carefully.
- Scatter picking, orbit, and touch tap need clear movement thresholds to avoid accidental picks during rotate.

## Open Questions

- Which exact touchscreen hardware will be used?
- Will the Pi run X11/XWayland, Wayland, or a kiosk compositor?
- Should one-finger drag over Scatter3D rotate or pan by default?
- Should two-finger rotate around midpoint change yaw/roll, or should yaw/roll stay mouse-only?
- Does WASP need touch support only for Scatter3D, or also LinePlot/Histogram zoom later?
