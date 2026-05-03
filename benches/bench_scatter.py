"""
Benchmark: DragonGUI Scatter3D real payload upload and steady-state frame time.

Usage, from the repo root:
    python benches/bench_scatter.py

Requires the native extension to be built first:
    python -m maturin build --target x86_64-pc-windows-gnu
    start.bat copies the result into python/dragongui/ automatically.

Environment:
    DRAGONGUI_BENCH_POINTS=500000
    DRAGONGUI_SMOKE_FRAMES=100

Output example:
    points            = 500000
    payload_mb        = 6.00
    python_pack_ms    = 3.12
    native_decode_ms  = 5.44
    native_upload_ms  = 1.67
    native_total_ms   = 7.25
    frame_ms          = 26.11  (avg over 100 frames)
    fps               = 38
"""
from __future__ import annotations

import os
import sys
import threading
import time
from pathlib import Path

# Load from source tree when run directly, matching the start.bat dev workflow.
if __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:
    raise SystemExit("bench_scatter.py requires NumPy") from exc

if not dg.native_backend_available():
    print(
        "Native backend not available. Build it first:\n"
        "  python -m maturin build --target x86_64-pc-windows-gnu\n"
        "  or run start.bat which copies the extension automatically."
    )
    sys.exit(1)

POINTS = max(1, int(os.environ.get("DRAGONGUI_BENCH_POINTS", "500000")))
SMOKE_FRAMES = max(1, int(os.environ.get("DRAGONGUI_SMOKE_FRAMES", "100")))


class Frame:
    columns = ("x", "y", "z")
    dtypes = ("float32", "float32", "float32")

    def __init__(self, points: int) -> None:
        self.shape = (points, 3)
        t = np.linspace(0.0, np.float32(160.0), points, dtype=np.float32)
        radius = np.linspace(np.float32(0.3), np.float32(12.0), points, dtype=np.float32)
        self.x = np.cos(t) * radius
        self.y = np.sin(t) * radius
        self.z = np.linspace(np.float32(-8.0), np.float32(8.0), points, dtype=np.float32)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


frame = Frame(POINTS)
pack_start = time.perf_counter()
payload = np.empty((POINTS, 3), dtype="<f4")
payload[:, 0] = frame.x
payload[:, 1] = frame.y
payload[:, 2] = frame.z
payload_bytes = payload.nbytes
payload_u8 = payload.view(np.uint8).reshape(-1)
python_pack_ms = (time.perf_counter() - pack_start) * 1000.0

app = dg.App()
win = dg.Window("DragonGUI Scatter Benchmark", width=1200, height=800)
scatter = dg.Scatter3D(None, x="x", y="y", z="z", colormap="turbo")


def submit_payload() -> None:
    handle = getattr(app, "_handle", None)
    if handle is None:
        return
    handle.enqueue_set_scatter_points_packed(
        scatter.id,
        payload_u8,
        pack_ms=python_pack_ms,
        enqueue_epoch_ms=time.time() * 1000.0,
        colormap=scatter.colormap,
        payload_format="xyz_f32_v0",
        coalesce=False,
    )


timer = threading.Timer(0.05, submit_payload)
timer.daemon = True
timer.start()

os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(SMOKE_FRAMES)
result = app.run(win)

snapshot = result.get("debug_snapshot", {})
gpu = snapshot.get("gpu", {}) if isinstance(snapshot, dict) else {}
resources = gpu.get("resources", {}) if isinstance(gpu, dict) else {}
scatter_metrics = resources.get("scatter", {}) if isinstance(resources, dict) else {}
native_decode_ms = scatter_metrics.get("last_decode_ms", float("nan"))
native_upload_ms = scatter_metrics.get("last_upload_ms", result.get("upload_ms", float("nan")))
native_total_ms = scatter_metrics.get("last_total_native_ms", float("nan"))
native_updates = scatter_metrics.get("updates", 0)
frame_ms = result.get("frame_ms", float("nan"))
fps = 1000.0 / frame_ms if frame_ms > 0 else float("nan")

print(f"points            = {POINTS}")
print(f"payload_mb        = {payload_bytes / 1_000_000.0:.2f}")
print(f"python_pack_ms    = {python_pack_ms:.2f}")
print(f"native_decode_ms  = {float(native_decode_ms):.2f}")
print(f"native_upload_ms  = {float(native_upload_ms):.2f}")
print(f"native_total_ms   = {float(native_total_ms):.2f}")
print(f"native_updates    = {int(native_updates)}")
print(f"frame_ms          = {frame_ms:.3f}  (avg over {SMOKE_FRAMES} frames)")
print(f"fps               = {fps:.0f}")
