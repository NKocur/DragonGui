# Raspberry Pi Release Checklist

Use this checklist before publishing an aarch64 Linux wheel as Raspberry Pi 5
supported. Dev-fallback CI and arm64 cloud runner builds are useful, but they do
not validate the V3D GPU, Mesa stack, desktop session, or real input behavior.

## Baseline

Record the exact hardware and OS:

- Raspberry Pi model and RAM size.
- Power supply and cooling.
- Display resolution and scaling factor.
- Desktop session: Wayland or X11.
- Raspberry Pi OS image date.
- Kernel version from `uname -a`.
- Mesa/OpenGL renderer from `glxinfo -B`.
- Vulkan summary from `vulkaninfo --summary`.
- Python version.
- DragonGUI version and wheel filename.

## Clean Install

On a clean Pi OS image:

```bash
python -m venv .venv-dragongui
. .venv-dragongui/bin/activate
python -m pip install --upgrade pip
python -m pip install dragongui
python -c "import dragongui as dg; print(dg.backend_info())"
```

Pass criteria:

- Import succeeds.
- `backend_info()["native"]` is true.
- `backend_info()["platform"]["profile"]` is `pi`.
- `backend_info()["features"]["webview"]` is false on Linux.

## Minimal Native Smoke

```bash
export DRAGONGUI_PROFILE=pi
export DRAGONGUI_SMOKE_FRAMES=3
python - <<'PY'
import dragongui as dg

app = dg.App()
win = dg.Window("Pi Smoke", width=420, height=280)
with dg.Panel("Smoke"):
    dg.Label("Raspberry Pi")
    dg.Button("OK")
result = app.run(win)
print(result)
assert result["status"] == "ok"
PY
```

Pass criteria:

- Window opens and closes automatically.
- Renderer is `wgpu`.
- No panic, device creation failure, or black-window hang.

## Core Interaction Pass

Manually verify:

- Button click.
- Text input and backspace.
- Dropdown open/select.
- Checkbox toggle.
- Slider drag.
- Mouse wheel over scrollable content.
- Table scroll and selection.
- Scatter rotate, right-drag pan, middle-drag pan, and wheel zoom.
- LinePlot toolbar fit/pan/zoom/box zoom.
- Window close.

## Probe Set

Run with `DRAGONGUI_PROFILE=pi`:

```bash
python examples/css_feature_probes/line_plot_probe.py
python examples/css_feature_probes/thread_monitor_probe.py
python examples/css_feature_probes/html_report_probe.py
python examples/css_feature_probes/scatter3d_frame_benchmark_probe.py
python examples/scatter_perf_lab.py
python examples/all_features_v3_demo.py
```

Pass criteria:

- No process crash.
- Unsupported embedded `HtmlReport` shows a clear fallback and external open
  path.
- Thread monitor remains usable but does not dominate frame time.
- LinePlot remains responsive at the Pi retained cap.
- Scatter 25k-50k is interactive; 100k is usable with reduced quality/LOD.
- Full V3 demo starts within the recorded budget and remains navigable.

## Performance Record

Record:

- Temperature before/after heavy probes: `vcgencmd measure_temp`.
- Startup time for the V3 demo.
- Scatter benchmark output at 25k, 50k, and 100k points.
- Streaming comparison for direct handoff vs. UI callback handoff.
- Any command-drain or payload-cap warnings.

## Release Gate

Do not publish the Pi wheel as supported unless:

- Clean install passes.
- Minimal native smoke passes.
- Core interaction pass is complete.
- Required probes do not crash.
- Unsupported features fail gracefully.
- Supported OS/kernel/Mesa/Python baseline is written into
  [raspberry-pi.md](raspberry-pi.md).
- Known limitations are documented in the release notes.
