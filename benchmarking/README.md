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
