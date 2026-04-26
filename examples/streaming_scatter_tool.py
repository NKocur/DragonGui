from __future__ import annotations

import math
import sys
import threading
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual demo guard
    raise SystemExit("streaming_scatter_tool.py requires NumPy for live point packing") from exc


POINTS = 220_000


class PointFrame:
    columns = ("x", "y", "z", "signal", "score", "phase")
    dtypes = ("float32", "float32", "float32", "float32", "float32", "float32")
    shape = (POINTS, len(columns))

    def __init__(self, phase: float, mode: str) -> None:
        t = np.linspace(0.0, 1.0, POINTS, dtype=np.float32)
        theta = t * np.float32(math.tau * 18.0 + phase)
        if mode == "helix":
            radius = np.float32(1.0) + np.float32(3.0) * t
            self.x = np.cos(theta) * radius
            self.y = np.sin(theta) * radius
            self.z = (t - np.float32(0.5)) * np.float32(9.0)
        elif mode == "wave":
            self.x = (t - np.float32(0.5)) * np.float32(10.0)
            self.y = np.sin(theta) * np.float32(3.5)
            self.z = np.cos(theta * np.float32(0.37)) * np.float32(3.5)
        else:
            rng = np.random.default_rng(int(phase * 1000) + 7)
            cloud = rng.standard_normal((POINTS, 3)).astype(np.float32)
            self.x = cloud[:, 0] * 3.0
            self.y = cloud[:, 1] * 3.0
            self.z = cloud[:, 2] * 3.0
        self.signal = np.sin(theta).astype(np.float32)
        self.score = np.cos(theta).astype(np.float32)
        self.phase = np.full(POINTS, np.float32(phase), dtype=np.float32)


app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33"))
win = dg.Window("DragonGUI Streaming Scatter Demo", width=1100, height=720)

stream_stop = threading.Event()
stream_thread: threading.Thread | None = None
state = {"phase": 0.0, "mode": "helix"}
initial = PointFrame(0.0, "helix")


def push_frame(mode: str | None = None) -> None:
    if mode is not None:
        state["mode"] = mode
    state["phase"] += 0.35
    frame = PointFrame(state["phase"], state["mode"])
    scatter.set_points(frame, x="x", y="y", z="z")
    status.set_value(f"{state['mode']} phase {state['phase']:.2f}")


def start_stream() -> None:
    global stream_thread
    if stream_thread is not None and stream_thread.is_alive():
        return

    stream_stop.clear()

    def worker() -> None:
        while not stream_stop.wait(0.5):
            try:
                app.call_soon_threadsafe(push_frame)
            except RuntimeError:
                break

    stream_thread = threading.Thread(target=worker, daemon=True)
    stream_thread.start()
    status.set_value("streaming every 500ms")


def stop_stream() -> None:
    stream_stop.set()
    status.set_value("stream stopped")


def print_scatter_metrics() -> None:
    snapshot = app.debug_snapshot()
    scatter_metrics = snapshot["gpu"]["resources"]["scatter"]
    print(
        "Scatter metrics:",
        {
            "updates": scatter_metrics["updates"],
            "points": scatter_metrics["last_point_count"],
            "payload_mb": round(scatter_metrics["last_payload_bytes"] / 1_000_000, 2),
            "pack_ms": round(scatter_metrics["last_pack_ms"], 3),
            "queue_latency_ms": round(scatter_metrics["last_queue_latency_ms"], 3),
            "decode_ms": round(scatter_metrics["last_decode_ms"], 3),
            "upload_ms": round(scatter_metrics["last_upload_ms"], 3),
            "native_total_ms": round(scatter_metrics["last_total_native_ms"], 3),
        },
    )


with dg.HLayout():
    with dg.Panel("Scatter stream", width=310):
        status = dg.TextInput("ready", placeholder="status", key="status")
        dg.Button("Switch To Helix", on_click=lambda: push_frame("helix"))
        dg.Button("Switch To Wave", on_click=lambda: push_frame("wave"))
        dg.Button("Switch To Cloud", on_click=lambda: push_frame("cloud"))
        dg.Button("Start Stream", on_click=start_stream)
        dg.Button("Stop Stream", on_click=stop_stream)
        dg.Button(
            "Print Metrics",
            on_click=lambda: threading.Thread(target=print_scatter_metrics, daemon=True).start(),
        )
        dg.Label("Drag/wheel the plot to orbit and zoom.")

    scatter = dg.Scatter3D(initial, x="x", y="y", z="z", key="stream-scatter")

try:
    print(app.run(win))
finally:
    stream_stop.set()
