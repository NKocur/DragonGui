# WASP DragonGUI Phase Scatter

Files:

- `dragongui-0.1.0-cp39-abi3-manylinux_2_39_aarch64.whl`: Raspberry Pi
  aarch64 DragonGUI wheel for Python 3.9+.
- `rpi_v4l2_phase_scatter.py`: replacement GUI script for the V4L2 phase
  scatter workflow.
- `run_wasp_phase_scatter.sh`: local launcher that creates/uses `.venv`, installs
  the wheel plus NumPy, and starts the script.

Install and run:

```bash
bash run_wasp_phase_scatter.sh
```

Run with the camera stream starting immediately:

```bash
bash run_wasp_phase_scatter.sh --autostart
```

Useful options:

```bash
bash run_wasp_phase_scatter.sh --device /dev/video0 --sample-step 5 --threshold 1.6 --z-scale 80 --payload xyz --autostart
```

`--payload xyz` is the default fast path. It sends compact x/y/z points and lets
the native renderer color from Z. Use `--payload scalar` only when you need
Python to precompute per-point RGB colors from the raw phase scalar.

If the camera is not attached, start without `--autostart`, open
`Debug fake points`, and use `Wave`, `Rings`, `Noise`, or `Ramp` to test the
Scatter3D layout and live update path.

The default V4L2 pixel format is `Y12 ` with the trailing FourCC space. Override
it only if the camera reports a different format:

```bash
v4l2-ctl -d /dev/video0 --list-formats-ext
bash run_wasp_phase_scatter.sh --pixelformat "Y12 " --autostart
```
