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

## Install Dependencies

For a developer source build on Pi OS:

```bash
sudo apt install build-essential cmake pkg-config libvulkan1 \
                 mesa-vulkan-drivers mesa-utils vulkan-tools \
                 libxkbcommon-dev libwayland-dev libxcb1-dev \
                 xdg-desktop-portal xdg-desktop-portal-gtk \
                 python3.11-dev python3-pip python3-venv
```

For 4 GB Pi hardware, prefer a prebuilt wheel once published. Building the Rust
extension and `lightningcss` stack from source may be memory constrained.

## Build From Source

```bash
python -m pip install maturin
maturin build --release --manifest-path native/Cargo.toml \
  --features pyo3/extension-module,pi
python -m pip install target/wheels/dragongui-*.whl
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

- wgpu Vulkan-or-GL backend selection.
- low-power adapter preference.
- downlevel wgpu limits.
- scatter point cap: 100,000 points.
- scatter LOD threshold: 50,000 points.
- scatter interactive render scale: 0.75.
- line plot retained cap: 50,000 points per series.
- table page size cap: 64 rows.
- table startup sample cap: 512 rows.
- table packed column buffer cap: 10,000 rows.

Backend triage can force wgpu backend selection:

- `DRAGONGUI_WGPU_BACKEND=auto`: use DragonGUI/wgpu defaults.
- `DRAGONGUI_WGPU_BACKEND=vulkan`: force Vulkan.
- `DRAGONGUI_WGPU_BACKEND=gl`: force OpenGL/GLES.
- `DRAGONGUI_WGPU_BACKEND=vulkan,gl`: allow only Vulkan or GL.

The active request appears in `backend_info()["platform"]["wgpu_backend_override"]`
and in `app.debug_snapshot()["runtime"]["platform"]`.

For startup diagnostics, set `DRAGONGUI_LOG=debug`. This prints the selected
profile, requested wgpu backends, adapter name/backend, driver, and required
buffer limit to stderr during native startup.

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
uname -a
python --version
vulkaninfo --summary
glxinfo -B
vcgencmd measure_temp
```

If the window opens black or device creation fails:

- Verify `mesa-vulkan-drivers`, `libvulkan1`, and `vulkan-tools` are installed.
- Try `DRAGONGUI_WGPU_BACKEND=gl` before launch. If GL fails, try
  `DRAGONGUI_WGPU_BACKEND=vulkan`.
- Run once with `DRAGONGUI_LOG=debug` and include stderr in the support report.
- Capture `backend_info()` and `app.debug_snapshot()` output.

If file dialogs do not open:

- Verify `xdg-desktop-portal` and a desktop-specific portal backend such as
  `xdg-desktop-portal-gtk` are installed and running.

If startup is slow:

- Use `DRAGONGUI_PROFILE=pi`.
- Keep scatter examples at 25k to 100k points.
- Keep table samples small.
- Prefer prepacked scatter and line plot payloads for live updates.
