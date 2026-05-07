# Raspberry Pi 5 Port

## Goal

Ship a `dragongui` build that runs on a Raspberry Pi 5 (aarch64, Pi OS
bookworm or newer, V3D GPU) without forking the codebase. Desktop builds
stay byte-for-byte unchanged; the Pi build is gated behind a Cargo
feature and a separate CI wheel.

## Non-goals

- Feature parity with desktop. The Pi's V3D GPU cannot match a discrete
  GPU on point-cloud throughput, MSAA, or compute paths.
- 32-bit Pi (armv7) support. Pi 5 is aarch64-only and earlier Pis are
  out of scope for this plan.
- Headless / framebuffer-only operation. The plan assumes a Wayland or
  X11 session.

## Constraints we have to design around

- **GPU:** V3D VII, Vulkan ~1.2 with gaps. wgpu often picks GL as a
  fallback. Compute shaders are limited; storage-buffer size in the
  vertex stage is small.
- **Drivers:** mesa version on Pi OS lags upstream. Behaviour changes
  between point releases — we must pin a known-good combo in install
  docs.
- **Memory:** 4 GB / 8 GB / 16 GB SKUs. The Rust + lightningcss build
  needs ≥8 GB to compile comfortably; users on 4 GB should install a
  prebuilt wheel, not build from source.
- **No published aarch64-linux wheel today.** This plan introduces one.

## Full-port acceptance criteria

A full Raspberry Pi port is ready when:

- `pip install dragongui` installs a working aarch64 Linux wheel on the
  supported Pi OS baseline.
- The package imports without extra user build steps.
- A minimal `Window` with native controls renders under the default Pi desktop
  session.
- The core widget set renders correctly: labels, buttons, inputs, menus,
  layouts, scroll areas, tabs/pages, tables, line plots, histograms, images,
  and scatter.
- The full V3 demo has a Pi profile that starts in a documented time budget
  and remains interactive.
- Heavy widgets degrade deliberately instead of failing with adapter/device,
  shader, or out-of-memory errors.
- Unsupported features, especially embedded `HtmlReport`, have clear fallbacks
  or clear errors.
- Release docs include supported OS image, kernel, mesa, Python, and install
  dependencies.
- A Pi 5 smoke checklist is part of release gating.

## Supported platform matrix

Initial supported target:

- Raspberry Pi 5.
- 64-bit Raspberry Pi OS Bookworm or newer.
- Python 3.11+.
- Desktop session through Wayland or X11.
- Mesa/V3D baseline pinned in docs after hardware validation.
- 8 GB or 16 GB RAM recommended for source builds.

Explicitly unsupported in the first release:

- 32-bit Pi OS.
- Headless framebuffer-only mode.
- SSH-only GUI forwarding as a supported path.
- Pi 4 or earlier performance guarantees.
- Embedded WebView/Plotly rendering.

## Code audit snapshot

Audited against the current tree on 2026-05-06:

- [native/Cargo.toml](../../native/Cargo.toml) currently has only
  `default`, `gpu`, and `pi` features. The `pi` feature is off by default.
- Windows-specific WebView2 dependencies are already behind
  `cfg(windows)`, so they should not be pulled into an aarch64 Linux build.
- [native/src/runtime.rs](../../native/src/runtime.rs) creates the wgpu
  instance/device in `WgpuState::new`; Pi profile selection now uses GL/GLES by
  default, `PowerPreference::LowPower`, and downlevel limits.
- The startup loading screen is already implemented and rendered before
  startup scatter resources are decoded/uploaded. The Pi work is validation,
  defaults, and diagnostics, not first implementation.
- [native/src/html_report_webview.rs](../../native/src/html_report_webview.rs)
  already has a non-Windows fallback manager that reports
  `platform: unsupported`, `enabled: false`. The Python `HtmlReport` also
  has an `open_external()` path.
- The current WGSL scan shows ordinary `textureSample` usage only; no
  `textureSampleLevel`, `f16`, or WGSL storage-buffer bindings were found.
  Buffer-size pressure still matters because scatter uses large vertex buffers.
- `ScatterWidget::new` defaults `lod_threshold` to `200_000`. The raw
  `point_instance_v1` fast path intentionally declines while active LOD needs
  CPU-side points.
- `LinePlot` already caps emitted line segments per visible series at
  `4096`; the Pi profile now caps retained data at 50k points per series.
- [pyproject.toml](../../pyproject.toml) includes `plans/*.md` and
  `plans/V2/*.md`, `plans/RPi/*.md`, and `plans/V3/*.md` in the sdist.

## Implementation status

Implemented so far:

- Central native runtime profile in
  [native/src/runtime_profile.rs](../../native/src/runtime_profile.rs) with
  `DRAGONGUI_PROFILE=desktop|pi|auto`.
- `backend_info()` and `app.debug_snapshot()` platform diagnostics include OS,
  architecture, profile selection, WebView availability, adapter info, wgpu
  limits, and effective heavy-widget caps.
- `DRAGONGUI_LOG=debug` prints native startup profile/backend/adapter/features/
  downlevel/limit diagnostics to stderr.
- `rpi_setup_and_run.sh diag` captures the display session, GL/EGL/Vulkan
  summaries, DragonGUI backend info, and one-frame backend probes.
- Pi profile device selection uses GL/GLES backend selection by default,
  low-power adapter preference, and downlevel limits.
- Scatter startup/live/actor/stream payloads validate point counts against the
  Pi profile cap and device max buffer size before allocating GPU buffers.
- Scatter stream ring uploads use contiguous span writes instead of one
  `write_buffer` call per point.
- Scatter startup/live/raw/actor/LOD uploads now use bounded chunked
  `queue.write_buffer` calls for large buffers.
- LinePlot startup and live updates enforce a 50k retained-point Pi cap and
  trim before decoding dropped points where the payload path allows it.
- DataFrameTable uses Pi-safe defaults of `page_size=64` and
  `sample_rows=512`; native parsing also reapplies those caps for non-Python
  clients.
- DataFrameTable packed column buffers are capped to 10k rows under the Pi
  profile, and native resources accept partial column buffers instead of
  requiring full-frame buffers.
- V3 demo and scatter performance lab have Pi-scaled dataset/workload defaults.
- V3 demo and scatter performance lab are import-safe, so profile constants can
  be tested without launching windows or background threads.
- Targeted Python tests cover Pi-profile line plot caps, table defaults, table
  column-buffer caps, HtmlReport serialization/fallback flags, and Pi demo
  constants.
- Targeted Rust tests cover profile selection, scatter cap errors, scatter
  payload stride validation, and partial table column buffers.
- Initial Raspberry Pi install and troubleshooting notes are in
  [docs/raspberry-pi.md](../../docs/raspberry-pi.md).
- Hardware release gating checklist is in
  [docs/raspberry-pi-release-checklist.md](../../docs/raspberry-pi-release-checklist.md).
- Widget-level Pi caps are documented in
  [docs/widgets.md](../../docs/widgets.md).
- Current implementation progress and pre-hardware audit are documented in
  [raspberry-pi-5-port-progress-audit.md](raspberry-pi-5-port-progress-audit.md).

## Where the code needs to change

### 1. Cargo feature flag

[native/Cargo.toml](../../native/Cargo.toml)

- `pi` feature exists under `[features]`. Off by default. Enabling it
  flips the runtime backend and limit selection (see section 2).
- No new dependencies are required for the Pi target. `rfd` 0.15
  already uses xdg-portal on Linux. The Windows-only deps are correctly
  gated by `cfg(windows)` and will not be pulled in.

[pyproject.toml](../../pyproject.toml)

- Pi CI passes the combined feature set explicitly as
  `--features pyo3/extension-module,pi` against
  `manifest-path = "native/Cargo.toml"`.

### 2. wgpu instance, adapter, and device

[native/src/runtime.rs](../../native/src/runtime.rs)

Implemented in `WgpuState::new`:

- Pi profile uses `InstanceDescriptor { backends: Backends::GL, .. }`
  because V3D's Vulkan path can expose an adapter but fail logical device
  creation on required baseline features. Vulkan remains available for explicit
  comparison with `DRAGONGUI_WGPU_BACKEND=vulkan`.
- When Pi GL is active, the runtime defaults the winit window backend to X11 so
  wgpu GL can attach to an X11/XWayland surface instead of an incompatible
  native Wayland surface. Override with `DRAGONGUI_WINDOW_BACKEND=auto|x11|wayland`.
- Pi profile uses `power_preference: LowPower`.
- Pi profile replaces `Limits::default()` with
  `Limits::downlevel_defaults().using_resolution(adapter.limits())`.
  Without this, device creation fails on V3D.
- Keep `force_fallback_adapter: false` — the V3D adapter is the real
  one we want.
- Implemented: capture `adapter.get_info()` and `adapter.limits()` after
  adapter selection and store them in the runtime/debug snapshot.
- Implemented: `DRAGONGUI_WGPU_BACKEND=auto|vulkan|gl|vulkan,gl|...` can force
  backend selection for triage when a mesa regression appears, and the request
  is visible in backend/debug snapshots.

### 3. Shader audit

[native/src/primitives/rect.wgsl](../../native/src/primitives/rect.wgsl)
and every other `.wgsl` under [native/src/](../../native/src/).

Watch for and fix:

- `textureSampleLevel` with non-zero LOD on textures without mip chains.
- Storage buffers bound to the vertex stage if future widgets introduce them.
  The current scatter pressure is large vertex-buffer allocation and upload
  size, not WGSL storage-buffer binding.
- `f16` or packed types — replace with `f32`.

Action: build with `pi` feature, run each example in
[examples/css_feature_probes/](../../examples/css_feature_probes/) on
real hardware, capture validation errors, fix iteratively. Keep this audit in
the plan even though the current shader scan is clean, because future widgets
can introduce downlevel-incompatible WGSL.

### 4. Heavy widgets — lower the ceiling

[native/src/scatter/mod.rs](../../native/src/scatter/mod.rs)

- Implemented: Pi profile caps max scatter payloads at 100 000 points, lowers
  default LOD threshold, caps interactive render scale, and validates point
  counts against both profile caps and `device.limits().max_buffer_size`.
- Implemented: payloads over the Pi or device cap return deterministic errors
  before GPU allocation.
- Implemented: large startup/live/raw/actor/LOD uploads are chunked into bounded
  `queue.write_buffer` calls.
- Implemented: ring stream uploads write contiguous spans instead of issuing one
  `write_buffer` call per point.
- Still to validate on hardware: whether MSAA needs to be explicitly disabled
  on every Pi path, and whether raw streaming should be the documented default
  for all high-frequency Pi streams.

[native/src/table.rs](../../native/src/table.rs)

- Implemented in the Python/native table pipeline: Pi profile defaults use
  `page_size=64`, `sample_rows=512`, and 10k-row packed column buffers. Native
  resources now accept partial column buffers.
- Still to validate on hardware: whether table keyboard/scroll interaction
  needs a smaller row prefetch window than desktop.

[native/src/primitives/mod.rs](../../native/src/primitives/mod.rs)

- Implemented: `LinePlot` startup and live update paths enforce a 50k retained
  point cap per series under the Pi profile. The renderer still down-samples to
  4096 segments per visible series.
- Still to validate on hardware: 50k retained points, multi-series plots, hover
  searches, and live append workloads.

These should continue to flow through the central runtime profile/cap
structures, with hot paths reading precomputed caps rather than scattering
ad-hoc Raspberry Pi checks.

### 5. WebView widget

[native/src/html_report_webview.rs](../../native/src/html_report_webview.rs)
is `cfg(windows)` for the embedded WebView backend. On Pi, the current
non-Windows manager exists and reports a stable unsupported snapshot:
`platform: unsupported`, `enabled: false`, with a reason string.

[python/dragongui/widgets.py](../../python/dragongui/widgets.py)

- Implemented: the Python-side `HtmlReport` placeholder plus `open_external()`
  fallback is the first Pi story.
- Implemented: V3 demo and probe surface the unsupported embedded backend and
  keep the external-open path available.
- Implemented: Python serialization tests cover `external_fallback`,
  `allow_scripts`, and `allow_remote`. A non-Windows native snapshot test is
  present behind `cfg(not(windows))`.
- Leave a Linux embedded webview backend as a later milestone unless the
  product decision changes. A hard `NotImplementedError` would be a regression
  from the current fallback behavior.

### 6. File dialogs

No code change needed. Document that
`xdg-desktop-portal-gtk` (or `-gnome` / `-kde`) must be installed on
the Pi for `rfd` to work.

### 7. Runtime capability detection

Implemented: native capability snapshots allow the Python layer and debug tools
to see what backend actually started:

- OS, architecture, and DragonGUI build profile.
- wgpu backend selected: Vulkan, GL, or other.
- adapter name, vendor, device type, and driver info when available.
- effective wgpu limits.
- whether Pi profile caps are active.
- whether WebView is available.

This is exposed through `app.debug_snapshot()` under the `runtime.platform`
snapshot:

```json
{
  "runtime": {
    "platform": {
      "os": "linux",
      "arch": "aarch64",
      "profile": "pi",
      "wgpu_backend": "Vulkan",
      "adapter": "V3D 7.1.7",
      "downlevel_limits": true
    }
  }
}
```

This makes remote debugging possible when a user reports "it opens black" or
"scatter is slow" from a Pi.

Implemented: [native/src/lib.rs](../../native/src/lib.rs) `backend_info()` now
includes platform, build profile, supported features, and WebView availability
so users can get useful diagnostics before running a full app.

`DRAGONGUI_DEV_FALLBACK=1` is still useful for Python/package smoke tests on
CI, but it does not exercise the native event loop, wgpu, windowing, or V3D.
The Pi release gate must not count dev-fallback tests as hardware validation.

### 8. Pi profile and widget defaults

Implemented: Pi behavior is centralized behind a profile rather than scattered
Raspberry Pi checks across the codebase.

Implemented shape:

- Native `RuntimeProfile::Desktop | RuntimeProfile::Pi`.
- Python-visible debug snapshot field.
- Environment override for testing: `DRAGONGUI_PROFILE=desktop|pi|auto`.

Pi profile defaults:

- Implemented: lower scatter point ceilings and real device limit checks.
- Implemented: lower streaming benchmark/demo defaults.
- Implemented: lower table startup sample sizes and packed column buffers.
- Implemented: real device limits are respected at runtime even when the `pi`
  feature is not enabled. A generic aarch64 Linux machine can still have a
  downlevel adapter.
- Still to validate/tune on hardware: scalar-bar refresh frequency, heavy grid
  labels, large hover metadata, high-frequency background stats, and whether
  direct scatter stream handoff should become the documented Pi default.

The important rule: desktop behavior must remain unchanged unless the Pi
profile is active.

### 9. Pi-safe examples and demo profiles

Implemented: the V3 demo keeps desktop-scale defaults unless the Pi profile is
active:

- `POINT_ROWS = 125_000`
- `TABLE_ROWS = 50_000`
- `STREAM_FRAME_COUNT = 24`

The examples now select smaller constants through `DRAGONGUI_PROFILE=pi`
instead of asking Pi users to edit source:

```python
DG_PROFILE = os.environ.get("DRAGONGUI_PROFILE", "auto")
if DG_PROFILE == "pi":
    POINT_ROWS = 40_000
    TABLE_ROWS = 10_000
    STREAM_FRAME_COUNT = 8
```

Current first Pi demo targets:

- Overview/demo scatter: 40k points.
- Scatter performance lab buttons: 25k, 50k, 100k.
- Stream frame count: 8 frames.
- HtmlReport page uses fallback content and an "open externally" path.
- Still to validate/tune on hardware: stream interval and whether the thread
  monitor should be disabled by default or just refreshed more slowly.

Implemented: under `DRAGONGUI_PROFILE=pi`, the scatter performance lab exposes
Pi workloads rather than desktop-scale 300k/1M buttons.

### 10. Startup loading screen

The loading screen API and native startup renderer already exist:

- `python/dragongui/app.py` exposes `LoadingScreen`.
- `native/src/document.rs` parses `loading_screen`.
- `native/src/runtime.rs` renders a startup frame and includes loading-screen
  timing in the debug snapshot.

Pi port tasks:

- Validate that the window becomes visible with the loading screen under both
  the documented Wayland path and X11 fallback.
- Ensure heavy startup resource work remains after the loading frame is
  presented, especially scatter decode/upload and large table resources.
- Include loading-screen timing in Pi benchmark artifacts.
- Use Pi-specific loading text in the V3 demo only through normal public API,
  not a special native branch.

### 11. Input, windowing, text, and fonts

Validate platform behavior beyond rendering:

- Mouse wheel/trackpad scroll.
- Middle/right mouse handling for Scatter3D pan and context menus.
- Keyboard input for text fields and shortcuts.
- IME/non-US keyboard behavior is best-effort but should not crash.
- HiDPI and fractional scaling under the Pi desktop.
- Default font selection. If system font discovery differs from Windows, docs
  should list expected fallback packages such as DejaVu/Noto fonts.
- Clipboard support for copy/paste and diagnostics.
- Native file dialogs with `rfd`; the tkinter fallback in `_backend.py` only
  applies when the native backend is missing, not when the native backend is
  present but portal services are absent.

### 12. Logging and troubleshooting

Implemented: [docs/raspberry-pi.md](../../docs/raspberry-pi.md) includes a Pi
troubleshooting section with:

- how to print `app.debug_snapshot()`
- how to force GL or Vulkan if wgpu backend selection is wrong
- how to check mesa/Vulkan packages
- common errors and fixes:
  - device creation failed
  - surface format unavailable
  - black window
  - missing xdg portal for dialogs
  - out-of-memory during source build
  - WebView unavailable
  - wrong backend selected
  - thermal throttling

Implemented: `DRAGONGUI_LOG=debug` prints native startup profile/backend/adapter
diagnostics to stderr. Hardware validation can still decide whether deeper wgpu
trace logging is needed.

## Build & distribution

### Local build (developer Pi)

Pi OS bookworm, 8 GB+ RAM:

```
sudo apt update
sudo apt install build-essential cmake pkg-config libvulkan1 \
                 mesa-vulkan-drivers mesa-utils vulkan-tools \
                 libxkbcommon-dev libwayland-dev libxcb1-dev \
                 xdg-desktop-portal xdg-desktop-portal-gtk \
                 python3-dev python3-pip python3-venv curl

export DRAGONGUI_USB_ROOT=/media/$USER/DragonUSB
mkdir -p "$DRAGONGUI_USB_ROOT/rustup" "$DRAGONGUI_USB_ROOT/cargo" \
         "$DRAGONGUI_USB_ROOT/cargo-target" "$DRAGONGUI_USB_ROOT/pip-cache"
export RUSTUP_HOME="$DRAGONGUI_USB_ROOT/rustup"
export CARGO_HOME="$DRAGONGUI_USB_ROOT/cargo"
export CARGO_TARGET_DIR="$DRAGONGUI_USB_ROOT/cargo-target"
export PIP_CACHE_DIR="$DRAGONGUI_USB_ROOT/pip-cache"
export PATH="$CARGO_HOME/bin:$PATH"

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
  sh -s -- -y --no-modify-path --default-toolchain stable
rustup default stable

python3 -m venv .venv
. .venv/bin/activate
pip install --upgrade pip maturin numpy plotly
maturin build --release --manifest-path native/Cargo.toml --features pyo3/extension-module,pi
pip install "$CARGO_TARGET_DIR"/wheels/dragongui-*.whl
cp "$(find .venv -name '_dragongui*.so' -print -quit)" python/dragongui/
```

Implemented convenience path: `bash rpi_setup_and_run.sh full` wraps the package
install, USB-backed Rust setup, wheel build/install, native extension copy, and
short smoke run. The script defaults `DRAGONGUI_WGPU_BACKEND=gl` so an otherwise
valid Pi Vulkan adapter cannot crash startup before the GL/GLES path is tested,
and defaults `DRAGONGUI_WINDOW_BACKEND=x11` so GL uses an X11/XWayland surface.

For 4 GB Pi hardware, source builds are not a supported first path. Use the
prebuilt wheel.

Validate that `lightningcss` builds successfully on aarch64 Linux and document
expected build memory/time for developer source builds.

### CI wheel

GitHub Actions:

- New job `pi-arm64-wheel` on `ubuntu-24.04-arm` runner (native aarch64,
  no QEMU).
- Uses `PyO3/maturin-action` with
  `args: --release --manifest-path native/Cargo.toml --features pyo3/extension-module,pi`
  and `manylinux: "2_28"`.
- Produces `dragongui-<ver>-cp311-abi3-manylinux_2_28_aarch64.whl`.
- Publishes to PyPI alongside the desktop wheels — same package name,
  different platform tag, so `pip install dragongui` Just Works on a Pi.

Fallback if `ubuntu-24.04-arm` is unavailable: use `ubuntu-22.04-arm`, or
`cross` with the
`aarch64-unknown-linux-gnu` target. Slower, but no QEMU emulation of
the wheel itself.

Additional CI checks before publishing the wheel:

- `cargo check --manifest-path native/Cargo.toml --target aarch64-unknown-linux-gnu --features pi`
- Python API tests with `DRAGONGUI_DEV_FALLBACK=1` and `DRAGONGUI_PROFILE=pi`
- `maturin build --release --features pyo3/extension-module,pi` on native aarch64
- Import smoke from the built wheel, not from the source tree

If the `pi` feature is passed through maturin differently, capture the exact
command in this plan and in release docs.

### Packaging includes

Implemented in [pyproject.toml](../../pyproject.toml) package include patterns:

- `plans/RPi/*.md` ships this plan in the sdist
- `plans/V3/*.md` remains included for V3 plans
- current Pi docs live under `docs/` and are already part of the docs set

### Install docs

Implemented in [docs/raspberry-pi.md](../../docs/raspberry-pi.md):

- supported Pi OS image and mesa/kernel baseline
- required apt packages
- Python version guidance
- wheel install command
- source build command for developers
- how to run a smoke example
- expected limitations
- how to capture support info:
  `uname -a`, `python --version`, `vulkaninfo --summary`,
  `glxinfo -B`, `vcgencmd measure_temp`, and `app.debug_snapshot()`

## Testing

- **Smoke test on real hardware.** Required. There is no substitute for
  V3D-on-mesa behaviour. Maintain a Pi 5 in CI or a manual pre-release
  checklist.
- **Examples to run:**
  [examples/all_features_v3_demo.py](../../examples/all_features_v3_demo.py),
  every probe under
  [examples/css_feature_probes/](../../examples/css_feature_probes/).
- **Pinned baseline:** record the kernel + mesa + Pi OS image used
  during validation in this plan as the supported baseline. Update it
  per release.

### Test matrix

Run on the supported Pi baseline:

- import smoke:
  `python -c "import dragongui as dg; print(dg.__file__)"`
- minimal window smoke:
  label + button + 3 smoke frames
- CSS/layout probes:
  all probes under `examples/css_feature_probes/` that do not require embedded
  WebView
- heavy widget probes:
  line plot, histogram, table, scatter
- full app smoke:
  V3 demo with `DRAGONGUI_PROFILE=pi`
- interaction manual pass:
  click, scroll, drag, text input, context menu, scatter rotate/pan/zoom

Targeted automated tests before hardware validation:

- Done: native runtime profile selection from `DRAGONGUI_PROFILE`
- Done: `backend_info()` includes platform/profile/WebView availability
- Done: non-Windows `HtmlReport` snapshot remains stable and non-crashing
  through a `cfg(not(windows))` native test
- Done: V3 demo/scatter lab constants switch under `DRAGONGUI_PROFILE=pi`
- Done: line plot retained-point caps and append trimming
- Done: table default/sample/column-buffer caps under `DRAGONGUI_PROFILE=pi`
- Done: scatter payload cap errors are deterministic and user-readable
- Done: isolated backend override parsing tests without constructing a full
  runtime
- Done: isolated wgpu limit selection helper tests for desktop vs.
  Pi/downlevel adapters without constructing a full runtime

### Performance targets

Initial targets should be conservative and measured on real hardware:

- Basic controls: interactive at desktop refresh rate for normal windows.
- LinePlot: 50k visible points should remain usable.
- Scatter3D: 25k-50k points should be smooth enough for interactive orbit.
- Scatter3D: 100k points should render with reduced quality/LOD.
- Streaming: direct handoff should visibly outperform UI callback handoff.
- Full V3 demo Pi profile: starts reliably and remains navigable.

These are acceptance targets, not marketing claims. Record actual numbers in a
benchmark artifact after hardware testing.

Benchmark protocol:

- record cooling, power supply, Pi RAM SKU, desktop session, resolution, and
  scaling factor
- record kernel, mesa, Vulkan/GL renderer, and Python versions
- run from a clean boot or after documenting thermal state
- capture temperatures before and after heavy scatter/line-plot runs

### Release gate

A release can include the Pi wheel only if:

- the wheel installs on a clean Pi OS image
- minimal smoke passes
- Pi-profile V3 demo smoke passes
- no required probe crashes the process
- unsupported features fail gracefully
- the supported baseline is documented

## Phased rollout

1. **Phase 1 — boots.** Feature flag, downlevel limits, GL fallback.
   Goal: a window opens with a `Button` and `Label` on the Pi.
2. **Phase 2 — widgets.** Shader audit; all 45+ widgets render
   correctly. No scatter / table tuning yet.
3. **Phase 3 — heavy widgets.** Scatter and table ceilings tuned;
   `HtmlReport` falls back cleanly.
4. **Phase 4 — Pi profiles.** V3 demo, scatter perf lab, and probes have
   Pi-safe defaults.
5. **Phase 5 — distribution.** CI wheel published to PyPI; install docs +
   supported-baseline note added to README.
6. **Phase 6 — maintenance.** Add Pi smoke-test step to release checklist;
   track wgpu / winit / mesa upgrades against real hardware.

Phases 1–3 are the engineering work. Phase 1 alone tells us whether
this is a 1-week job or a 1-month job.

## Risks

- **V3D Vulkan regressions across mesa versions.** Mitigation: pin a
  baseline, document it, retest on upgrades.
- **wgpu major releases drop downlevel paths.** Mitigation: the Pi
  build pins wgpu carefully; bumping wgpu requires a Pi smoke test.
- **CI runner availability for aarch64 Linux.** Mitigation: `cross`
  fallback documented above.
- **User confusion about reduced ceilings.** Mitigation: Pi-specific
  section in [docs/widgets.md](../../docs/widgets.md) listing the
  caps (max points, reduced interactive quality, no embedded `HtmlReport`).
- **Thermal throttling changes results.** Mitigation: document cooling and
  power-supply assumptions for benchmark runs.
- **Desktop session differences.** Mitigation: validate at least the default Pi
  OS desktop path first; treat other compositors as best-effort until tested.
- **Too many runtime branches.** Mitigation: centralize Pi behavior behind a
  profile/config module and constants.

## Open questions

- Should Pi profile activation be build-time only, runtime auto-detected, or
  both? Current recommendation: build-time feature for limits, runtime/env
  profile for examples and docs.
- Do we support both Wayland and X11 equally in the first release, or document
  one as the validated path?
- Should `HtmlReport.open_external()` be the official Pi story, or should a
  Linux webview backend become part of a later full-port milestone?
- Do we ship one wheel that auto-detects the Pi at runtime, or keep a
  separate `pi`-feature wheel? Current plan: separate wheel, same
  package name, selected by platform tag. Simpler, no runtime cost on
  desktop. Revisit if PyPI tag resolution causes friction.
- Pi 4 support? Out of scope here, but most of this plan also applies
  if someone wants to extend it later — the limits would need to drop
  further.
