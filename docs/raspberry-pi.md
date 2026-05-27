# Raspberry Pi 5

DragonGUI has a Raspberry Pi 5 profile for aarch64 Linux builds. The profile is
intended for Raspberry Pi OS Bookworm or newer with the standard desktop
session. It is not a headless framebuffer target.

This port is still pending real hardware validation. The code now has the
runtime switches and conservative widget caps needed for the first Pi smoke
pass, but the supported Mesa/kernel baseline must be recorded after testing on
actual Pi 5 hardware.

## Supported Target

- Raspberry Pi 5.
- 64-bit Raspberry Pi OS Bookworm or newer.
- Python 3.11+.
- Wayland or X11 desktop session.
- 8 GB or 16 GB RAM recommended for source builds.

Unsupported for the first release:

- 32-bit Pi OS.
- Pi 4 or earlier performance guarantees.
- SSH-only GUI forwarding as a supported path.
- Embedded `HtmlReport` WebView. Use `HtmlReport.open_external()` on Linux.

`HtmlReport` remains useful on Pi as a report launcher/source preview surface,
but embedded rendering is reported as unsupported in the native debug snapshot.
Keep `external_fallback=True` and expose an external-open action for Pi-facing
apps.

## Install Dependencies

For a developer source build on Pi OS:

```bash
sudo apt update
sudo apt install build-essential cmake pkg-config libvulkan1 \
                 mesa-vulkan-drivers mesa-utils vulkan-tools \
                 libxkbcommon-dev libwayland-dev libxcb1-dev \
                 xdg-desktop-portal xdg-desktop-portal-gtk \
                 python3-dev python3-pip python3-venv curl
```

For 4 GB Pi hardware, prefer a prebuilt wheel once published. Building the Rust
extension and `lightningcss` stack from source may be memory constrained.

## Setup Helper

For a repo checked out on a USB drive, the helper keeps Rust, Cargo build
artifacts, and the pip cache on that drive. It also copies the installed native
extension into `python/dragongui/`, which is required when running examples from
the source tree.

From the DragonGUI repo root:

```bash
bash rpi_setup_and_run.sh deps
bash rpi_setup_and_run.sh check-deps
bash rpi_setup_and_run.sh build-smoke
```

Then run the V3 demo normally:

```bash
bash rpi_setup_and_run.sh run
```

Run an individual V3 page directly:

```bash
DRAGONGUI_DEMO_PAGE=scatter bash rpi_setup_and_run.sh run
DRAGONGUI_DEMO_PAGE=lineplots bash rpi_setup_and_run.sh run
DRAGONGUI_DEMO_PAGE=histograms bash rpi_setup_and_run.sh run
DRAGONGUI_DEMO_PAGE=data bash rpi_setup_and_run.sh run
```

Run the validated page-fit smoke for one route:

```bash
DRAGONGUI_DEMO_PAGE=data DRAGONGUI_SMOKE_FRAMES=3 \
  bash rpi_setup_and_run.sh smoke benchmarking/rpi_v3_viewport_fit_check.py
```

If startup fails before a window appears, capture a full display/GPU report:

```bash
bash rpi_setup_and_run.sh diag
```

The helper defaults to `DRAGONGUI_WGPU_BACKEND=gl` and
`DRAGONGUI_WINDOW_BACKEND=x11`. On Wayland desktops this uses XWayland for the
window because wgpu's GL backend may not be compatible with a native Wayland
surface on Pi.

The script infers the USB root from a path like
`/media/xymbu/DragonUSB/DragonGui-RPi`. If needed, force it explicitly:

```bash
DRAGONGUI_USB_ROOT=/media/xymbu/DragonUSB bash rpi_setup_and_run.sh build-smoke
```

## Build From Source

```bash
sudo apt install curl
export DRAGONGUI_USB_ROOT=/media/$USER/DragonUSB
mkdir -p "$DRAGONGUI_USB_ROOT/rustup" \
         "$DRAGONGUI_USB_ROOT/cargo" \
         "$DRAGONGUI_USB_ROOT/cargo-target" \
         "$DRAGONGUI_USB_ROOT/pip-cache"
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
python -m pip install --upgrade pip maturin numpy plotly
maturin build --release --manifest-path native/Cargo.toml \
  --features pyo3/extension-module,pi
python -m pip install "$CARGO_TARGET_DIR"/wheels/dragongui-*.whl
cp "$(find .venv -name '_dragongui*.so' -print -quit)" python/dragongui/
```

For local diagnostics without building a wheel:

```bash
export DRAGONGUI_PROFILE=pi
cargo check --manifest-path native/Cargo.toml --features pi
```

The CI wheel job uses a native arm64 GitHub-hosted runner and builds with
`PyO3/maturin-action` using `manylinux: "2_28"`. That wheel tag should remain
compatible with Raspberry Pi OS Bookworm's glibc baseline while avoiding QEMU
for the build itself.

## Runtime Profile

The runtime profile is selected by `DRAGONGUI_PROFILE`:

- `auto`: Pi profile on `linux/aarch64` or when the native `pi` feature is
  enabled; desktop elsewhere.
- `pi`: force Pi caps and defaults.
- `desktop`: force desktop defaults.

The Pi profile currently applies:

- wgpu GL/GLES backend selection by default.
- low-power adapter preference.
- downlevel wgpu limits.
- scatter point cap: 200,000 points.
- scatter LOD threshold: 50,000 points.
- scatter interactive render scale: 0.75.
- line plot retained cap: 50,000 points per series.
- line plot primitive segment budget: 1,536 segments per series.
- line plot dense dashed/dotted styles simplify to solid during primitive
  emission.
- histogram render budget: 384 rendered bar buckets.
- compact histogram tick cap: 4 ticks for compact histogram rects.
- table page size cap: 64 rows.
- table startup sample cap: 512 rows.
- table packed column buffer cap: 10,000 rows.
- compact table metrics: 26 px header, 22 px rows, 48 px index column, and
  112 px data columns unless explicit table style values override them.
- compact PieChart labels: small Pi plots raise the automatic slice-label
  threshold and cap visible slice/legend labels to keep compact panels readable.

Backend triage can force wgpu backend selection:

- `DRAGONGUI_WGPU_BACKEND=auto`: use the DragonGUI Pi default, currently GL/GLES.
- `DRAGONGUI_WGPU_BACKEND=vulkan`: force Vulkan for driver comparison.
- `DRAGONGUI_WGPU_BACKEND=gl`: force OpenGL/GLES.
- `DRAGONGUI_WGPU_BACKEND=vulkan,gl`: allow only Vulkan or GL.

The active request appears in `backend_info()["platform"]["wgpu_backend_override"]`
and in `app.debug_snapshot()["runtime"]["platform"]`.

The Pi helper also supports `DRAGONGUI_WINDOW_BACKEND=auto|x11|wayland`. Use
`x11` with `DRAGONGUI_WGPU_BACKEND=gl`, and use `wayland` when explicitly
testing the Vulkan path.

For startup diagnostics, set `DRAGONGUI_LOG=debug`. This prints the selected
profile, requested wgpu backends, adapter name/backend, driver, available
adapter features, downlevel capabilities, and required buffer limit to stderr
during native startup.

## Smoke Test

Run a minimal app first:

```bash
export DRAGONGUI_PROFILE=pi
export DRAGONGUI_SMOKE_FRAMES=3
python examples/all_features_v3_demo.py
```

Then run targeted probes:

```bash
export DRAGONGUI_PROFILE=pi
python examples/css_feature_probes/line_plot_probe.py
python examples/css_feature_probes/thread_monitor_probe.py
python examples/css_feature_probes/scatter3d_frame_benchmark_probe.py
```

To capture a compact widget baseline with the native performance counters:

```bash
DRAGONGUI_SMOKE_FRAMES=3 \
  bash rpi_setup_and_run.sh smoke benchmarking/rpi_widget_probe.py
```

The probe defaults to `DRAGONGUI_PROFILE=pi`, `DRAGONGUI_WGPU_BACKEND=gl`,
`DRAGONGUI_WINDOW_BACKEND=x11`, and `DRAGONGUI_SMOKE_FRAMES=10` when those
variables are not already set. It prints snapshot values for primitive/text
counts, primitive/text rebuild time, dirty kind, queue depth, and drained command
count without enforcing thresholds before real Pi baseline data exists. If
`native_performance_counters` is `false`, rebuild/copy the native extension with
`bash rpi_setup_and_run.sh build-smoke` before recording the baseline.

Use [raspberry-pi-release-checklist.md](raspberry-pi-release-checklist.md) for
the full pre-release gate.

## Troubleshooting

Print backend support info:

```python
import dragongui as dg

print(dg.backend_info())
```

For an active app, capture the debug snapshot:

```python
snapshot = app.debug_snapshot()
print(snapshot["runtime"]["platform"])
```

Useful system commands for support reports:

```bash
bash rpi_setup_and_run.sh diag
uname -a
python --version
vulkaninfo --summary
glxinfo -B
vcgencmd measure_temp
```

If the window opens black or device creation fails:

- Verify `mesa-vulkan-drivers`, `libvulkan1`, and `vulkan-tools` are installed.
- Run through `bash rpi_setup_and_run.sh run`, which defaults to
  `DRAGONGUI_WGPU_BACKEND=gl` and `DRAGONGUI_WINDOW_BACKEND=x11`.
- If GL fails, try `DRAGONGUI_WGPU_BACKEND=vulkan bash rpi_setup_and_run.sh run`
  to check the Vulkan path separately.
- Run once with `DRAGONGUI_LOG=debug` and include stderr in the support report.
- Capture `backend_info()` and `app.debug_snapshot()` output.

If file dialogs do not open:

- Verify `xdg-desktop-portal` and a desktop-specific portal backend such as
  `xdg-desktop-portal-gtk` are installed and running.

If startup is slow:

- Use `DRAGONGUI_PROFILE=pi`.
- Keep scatter examples at 25k to 200k points.
- Keep table samples small.
- Prefer prepacked scatter and line plot payloads for live updates.
