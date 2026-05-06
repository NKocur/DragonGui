# V3 Library Startup Loading Screen

DragonGUI should provide a library-level loading screen that appears when any
DragonGUI app starts, before heavy startup resources make the first real UI
frame available. This should be a native runtime feature, not a demo-specific
widget pattern, because normal widgets are not visible until after the widget
tree has been parsed, resource payloads have been applied, and the first real
frame has rendered.

The first useful version should render a small native loading frame immediately
after the native window and GPU surface are ready, then continue with startup
resource loading and swap to the real UI once ready.

## Current State

- There is no public loading or splash screen API.
- Users can control the first real frame background through `Theme.background`
  and window styling.
- The native runtime creates the window, initializes WGPU, applies startup
  resources, drains queued live startup commands, and then requests the first
  real redraw.
- In live mode, Python widgets queue startup resources before `run_document`.
  Native drains those commands in `DragonApp::resumed` before the first redraw.
- In non-live startup documents, some heavy startup data can be decoded/uploaded
  inside `WgpuState::new`.
- A normal `Label("Loading...")` does not solve startup stalls because it is
  part of the real widget tree and cannot be shown before resource loading.

## Goals

- Add one library-wide startup loading screen API on `dg.App`.
- Render a native loading frame before expensive startup command drain and
  large widget resource uploads.
- Support a polished default that works without app-specific configuration.
- Let applications customize title, message, background, text color, accent
  color, spinner/progress visibility, and minimum display duration.
- Allow apps to disable it completely.
- Keep the first implementation simple and deterministic: static loading
  content first, progress reporting later.
- Make startup behavior visible in debug snapshots for probes and tests.
- Avoid adding any dependency on the normal widget tree, layout engine, CSS
  cascade, or app callbacks for the loading frame.

## Non-Goals

- Showing a loading screen while Python code is still building the app document
  before `app.run()` is called.
- Full async app initialization in the first pass.
- Accurate per-widget progress in the first pass.
- User-defined arbitrary loading widgets in the first pass.
- Replacing widget-level loading states for data that loads after the app is
  already visible.
- Solving platform window-manager delays before the native window is created.

## Proposed Python API

Default behavior:

```python
app = dg.App()
```

The default should be enabled only when it can help. A reasonable first policy
is to render it whenever startup resources or queued startup commands exist,
with a small minimum display duration to avoid a single-frame flicker.

Disable globally:

```python
app = dg.App(loading_screen=False)
```

Customize:

```python
app = dg.App(
    loading_screen=dg.LoadingScreen(
        title="Loading dashboard",
        message="Preparing plots and tables...",
        background="#0b1020",
        text="#f8fafc",
        accent="#42a5ff",
        show_spinner=True,
        min_duration_ms=160,
    )
)
```

Dataclass shape:

```python
@dataclass(slots=True)
class LoadingScreen:
    enabled: bool = True
    title: str = "Loading"
    message: str | None = None
    background: str | tuple[float, float, float, float] | None = None
    text: str | tuple[float, float, float, float] | None = None
    accent: str | tuple[float, float, float, float] | None = None
    show_spinner: bool = True
    show_progress: bool = False
    min_duration_ms: int = 120
```

`App` constructor:

```python
class App:
    def __init__(
        self,
        title: str = "DragonGUI",
        theme: Theme | None = None,
        metadata: dict[str, Any] | None = None,
        loading_screen: bool | LoadingScreen | None = None,
    )
```

Suggested interpretation:

- `None`: use the library default.
- `False`: disable.
- `True`: use default loading screen explicitly.
- `LoadingScreen(...)`: enabled unless `enabled=False`.

## Serialized Document

Add a top-level document field:

```json
{
  "schema": 1,
  "type": "app",
  "title": "DragonGUI",
  "loading_screen": {
    "enabled": true,
    "title": "Loading",
    "message": null,
    "background": "#0b1020",
    "text": "#f8fafc",
    "accent": "#42a5ff",
    "show_spinner": true,
    "show_progress": false,
    "min_duration_ms": 120
  },
  "window": {}
}
```

Native parsing should convert this to `LoadingScreenSpec` on `AppSpec`.

## Native Architecture

### Startup Phases

Introduce an explicit startup phase in `DragonApp`:

```rust
enum StartupPhase {
    CreatingWindow,
    LoadingVisible,
    ApplyingStartupResources,
    Ready,
}
```

The first milestone can use this as internal state and expose only aggregate
debug data.

### Split Heavy Initialization

`WgpuState::new` currently mixes several jobs:

- create WGPU instance, surface, adapter, device, queue, and surface config
- create depth texture and renderers
- parse/use startup widget data
- build scatter runtimes and upload initial scatter buffers
- initialize resources and widget state

To show a loading screen before expensive work, split this into stages:

1. `WgpuState::new_base(window, spec_base)`
   - creates surface/device/queue/config
   - resolves theme
   - creates minimal render support needed for the loading frame
   - does not upload heavy widget resources

2. `gpu.render_loading_screen(&loading_spec)`
   - clears the surface to the loading background
   - draws minimal native loading visuals
   - presents immediately

3. `gpu.finish_startup(spec)`
   - creates full primitive/image/text renderers
   - initializes widget state/resources
   - initializes scatter runtimes and uploads startup buffers
   - applies layout and rebuilds visuals

4. Drain queued startup commands.

5. Request the first real app redraw.

For live mode, step 4 is the critical improvement. Startup resource commands
queued by Python should be drained only after the loading frame has been
presented.

For non-live startup documents, step 3 is the critical improvement. Heavy
payloads serialized directly into the startup document should also move after
the loading frame.

### Loading Renderer

The loading renderer should not depend on the widget tree. Recommended first
implementation:

- color clear pass using the configured loading background
- small centered accent spinner or progress bar using simple WGPU primitives
- optional title/message text using the same text renderer if available

If text renderer setup turns out to be too expensive or coupled, the first pass
can render only background plus spinner and add text in the second milestone.

### Minimum Duration

Avoid flicker by tracking `loading_presented_at`.

Before swapping to the real UI:

- if `elapsed < min_duration_ms`, wait until the deadline
- otherwise request the real redraw immediately

This should not run if the loading screen is disabled.

## Debug Snapshot

Add startup loading information to the runtime snapshot:

```json
{
  "runtime": {
    "loading_screen": {
      "enabled": true,
      "shown": true,
      "frames": 1,
      "present_ms": 4.2,
      "startup_resource_ms": 83.7,
      "min_duration_ms": 120
    }
  }
}
```

This gives probes a stable way to verify that the loading frame was actually
presented.

## Milestones

### Milestone 1: API and Static Native Loading Frame

- Add `LoadingScreen` Python dataclass.
- Add `loading_screen` parameter to `dg.App`.
- Serialize top-level `loading_screen`.
- Parse native `LoadingScreenSpec`.
- Render a static native loading frame after base GPU init.
- Drain live startup commands after the loading frame.
- Add debug snapshot fields.
- Add a focused probe that forces visible startup work.

### Milestone 2: Move Heavy Startup Payload Work After Loading Frame

- Split `WgpuState::new` so scatter startup decode/upload happens after the
  loading frame.
- Ensure startup document payloads for Scatter3D, LinePlot, DataFrameTable,
  Histogram, HtmlReport, and images do not block the loading frame.
- Preserve existing run result fields such as `upload_ms`.
- Add regression smoke tests for heavy startup documents.

### Milestone 3: Progress Reporting

- Add internal progress stages:
  - GPU ready
  - resources queued
  - widget resources loaded
  - first layout built
  - first app frame ready
- Optionally expose `show_progress=True`.
- Consider `app.set_loading_status(...)` for applications that perform native
  runtime tasks after the event loop is already running.

### Milestone 4: Polish and Defaults

- Tune default visuals against dark and light themes.
- Default title/message should avoid app-specific language:
  - title: `Loading`
  - message: omitted by default
- Decide whether default loading appears only when startup work is detected or
  always appears for at least one frame.
- Add documentation examples.

## Probe Plan

Create `examples/css_feature_probes/startup_loading_probe.py`.

Probe cases:

- default loading screen with a deliberately large startup payload
- custom loading screen colors/title/message
- disabled loading screen
- minimum duration behavior

Useful test hooks:

- `DRAGONGUI_SMOKE_FRAMES=3`
- `DRAGONGUI_STARTUP_DELAY_MS=250` test-only environment variable, if needed
- debug snapshot assertion that `runtime.loading_screen.shown` is true

The probe should not depend on screenshots for pass/fail, but screenshots are
useful for manual visual review.

## Test Plan

Python:

- `LoadingScreen` serializes correctly.
- `App(loading_screen=False)` serializes disabled state.
- invalid colors or negative `min_duration_ms` raise clear errors.
- default app document includes the default loading policy.

Native:

- document parsing accepts missing, disabled, and custom loading config.
- `DragonApp` records loading debug fields.
- disabled loading skips the loading render path.
- min duration does not block smoke tests when disabled.

Integration:

- startup loading probe exits cleanly under smoke frames.
- heavy startup LinePlot and Scatter3D probes still render real content after
  the loading frame.
- all existing app examples still run with default settings.

## Risks

- Rendering from inside `resumed` before the normal redraw path must be handled
  carefully so surface acquisition/present errors are not duplicated.
- Some platforms may not show a newly-created window until the event loop
  returns. If that happens, schedule the loading render through a redraw event
  before applying heavy resources instead of presenting synchronously.
- Splitting `WgpuState::new` can expose ordering assumptions around resource
  registry sync, widget state creation, scatter startup, image startup, and text
  renderer creation.
- A default loading screen could add visible flicker for very fast apps. The
  default policy should be conservative or use a small delay threshold.

## Open Questions

- Should the loading screen be enabled by default for every app, or only when
  startup resources are detected?
- Should the default copy use `Loading`, the app title, or no text?
- Should progress be percentage-based, stage-based, or omitted until the runtime
  has reliable per-resource accounting?
- Should Python document-building helpers get a separate userland pattern for
  pre-`app.run()` work, since native loading cannot cover that phase?

