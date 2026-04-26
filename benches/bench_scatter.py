"""
Benchmark: DragonGUI scatter — 500k-point upload and steady-state frame time.

Usage (from the repo root):
    python benches/bench_scatter.py

Requires the native extension to be built first:
    python -m maturin build --target x86_64-pc-windows-gnu
    (start.bat copies the result into python/dragongui/ automatically)

Output example:
    points       = 500000
    upload_ms    = 45.23
    frame_ms     = 2.81  (avg over 100 frames)
    fps          = 356
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

# Load from source tree when run directly (same as start.bat dev workflow).
if __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

if not dg.native_backend_available():
    print(
        "Native backend not available. Build it first:\n"
        "  python -m maturin build --target x86_64-pc-windows-gnu\n"
        "  (or run start.bat which copies the extension automatically)"
    )
    sys.exit(1)

SMOKE_FRAMES = 100

app = dg.App()
win = dg.Window("DragonGUI Scatter Benchmark", width=1200, height=800)
_scatter = dg.Scatter3D(None, x="x", y="y", z="z")

os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(SMOKE_FRAMES)
result = app.run(win)

upload_ms = result.get("upload_ms", float("nan"))
frame_ms = result.get("frame_ms", float("nan"))
fps = 1000.0 / frame_ms if frame_ms > 0 else float("nan")

print(f"points       = 500000")
print(f"upload_ms    = {upload_ms:.2f}")
print(f"frame_ms     = {frame_ms:.3f}  (avg over {SMOKE_FRAMES} frames)")
print(f"fps          = {fps:.0f}")
