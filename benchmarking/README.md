# Scatter Streaming Benchmarks

This folder contains comparison scripts for DragonGUI scatter streaming against
external Python plotting backends.

## DragonGUI vs VisPy

Run from the repository root:

```powershell
C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe benchmarking\scatter_stream_compare.py --points 1000000 --duration 3 --target-hz 60
```

The script always runs the DragonGUI benchmark. It runs the VisPy benchmark only
when VisPy and a compatible GUI backend are installed.

Useful options:

```powershell
--backend dragongui
--backend vispy
--backend both
--points 125000
--points 1000000
--duration 3
--target-hz 60
--json-out benchmarking\last_scatter_compare.json
```

Read the metrics as two different budgets:

- `producer_build_ms`: Python time spent building a new point frame.
- `ui_update_ms`: time spent calling the plotting widget update API.
- `achieved_update_hz`: completed data updates per second.
- `present_fps`: rendered/presented frames per second when the backend exposes it.

DragonGUI also reports native packet/upload timings from its debug snapshot.
VisPy reports draw-event timing from its canvas callbacks when available.

## V4 Widget Probe Baselines

Run the static v4 widget probes in smoke mode and collect frame, stage,
primitive, and widget-count metrics:

```powershell
python benchmarking\v4_widget_benchmark.py --frames 20 --json-out benchmarking\v4_widget_baseline.json
```

Useful options:

```powershell
--quick
--probe heatmap
--probe bar_chart
--frames 60
--repeat 3
--json-out benchmarking\v4_widget_baseline_after.json
```

For optimization work, sort by `frame_work_ms_avg`, `frame_prepare_ms_avg`,
`frame_encode_ms_avg`, `primitive_rects`, and `primitive_complex`. The headline
`frame_ms` can include presentation timing, so it is a weaker bottleneck signal
for short static probe runs.

## Dynamic Scatter Camera Benchmark

Run the paced camera-motion benchmark when checking scatter interaction costs:

```powershell
python benchmarking\scatter_dynamic_camera_benchmark.py --points 65000 --camera-updates 90 --interval-ms 16 --repeat 3 --json-out benchmarking\scatter_dynamic_camera_paced.json
```

The static v4 benchmark can show cache hits after the camera stops. This dynamic
benchmark forces camera changes at a paced interval and reports aggregate scatter
render timing plus cache hit rate.
