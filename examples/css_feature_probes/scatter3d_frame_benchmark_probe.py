from __future__ import annotations

import math
import os
import statistics
import sys
import threading
import time
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

from probe_helpers import probe_grid

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual visual probe requirement
    raise SystemExit("scatter3d_frame_benchmark_probe.py requires NumPy") from exc


REQUESTED_POINTS = max(125_000, int(os.environ.get("DRAGONGUI_BENCH_POINTS", "125000")))
FRAME_COUNT = max(1, int(os.environ.get("DRAGONGUI_BENCH_FRAMES", "50")))
FRAME_INTERVAL_MS = max(0.0, float(os.environ.get("DRAGONGUI_BENCH_FRAME_INTERVAL_MS", "0")))
VISUAL_INTERVAL_MS = max(
    1.0, float(os.environ.get("DRAGONGUI_BENCH_VISUAL_INTERVAL_MS", "5"))
)
COMMAND_DRAIN_TIMEOUT_S = max(
    0.1, float(os.environ.get("DRAGONGUI_BENCH_COMMAND_DRAIN_TIMEOUT_S", "5"))
)

LIDAR_ROWS = max(64, int(math.sqrt(REQUESTED_POINTS / 2.0)))
LIDAR_COLS = max(1, math.ceil(REQUESTED_POINTS / LIDAR_ROWS))
POINTS = LIDAR_ROWS * LIDAR_COLS

FIELD_U, FIELD_V = np.meshgrid(
    np.linspace(-1.0, 1.0, LIDAR_COLS, dtype=np.float32),
    np.linspace(-1.0, 1.0, LIDAR_ROWS, dtype=np.float32),
)
FIELD_U = FIELD_U.ravel()
FIELD_V = FIELD_V.ravel()


def pack_xyz_payload(x: object, y: object, z: object) -> np.ndarray:
    out = np.empty((POINTS, 3), dtype="<f4")
    out[:, 0] = np.asarray(x, dtype=np.float32)
    out[:, 1] = np.asarray(y, dtype=np.float32)
    out[:, 2] = np.asarray(z, dtype=np.float32)
    return out.view(np.uint8).reshape(-1)


class BenchmarkFrame:
    columns = ("x", "y", "z")
    dtypes = ("float32", "float32", "float32")

    def __init__(self, phase: float) -> None:
        self.shape = (POINTS, 3)
        p = np.float32(phase)
        sweep_center = np.sin(p * np.float32(0.72)) * np.float32(0.78)
        sweep = np.exp(-((FIELD_U - sweep_center) ** 2) * np.float32(46.0))
        cross_sweep = np.exp(
            -((FIELD_V - np.cos(p * np.float32(0.58)) * np.float32(0.62)) ** 2)
            * np.float32(28.0)
        )
        scan_wave = np.sin(FIELD_U * np.float32(math.tau * 2.0) + p)
        row_wave = np.cos(FIELD_V * np.float32(math.tau * 3.0) - p * np.float32(0.65))
        diagonal_wave = np.sin(
            (FIELD_U * np.float32(1.4) + FIELD_V * np.float32(0.9)) * np.float32(math.tau)
            + p * np.float32(1.8)
        )
        range_surface = (
            np.float32(18.0)
            + scan_wave * np.float32(0.38)
            + row_wave * np.float32(0.22)
            + diagonal_wave * np.float32(0.78)
            + sweep * np.float32(4.6)
            - cross_sweep * np.float32(2.8)
        )
        self.x = (
            FIELD_U * np.float32(14.0)
            + np.sin(FIELD_V * np.float32(math.tau) + p) * np.float32(0.34)
            + np.sin(p * np.float32(0.52)) * np.float32(0.55)
        )
        self.y = (
            FIELD_V * np.float32(5.5)
            + np.cos(FIELD_U * np.float32(math.tau) - p) * np.float32(0.24)
            + np.cos(p * np.float32(0.47)) * np.float32(0.28)
        )
        self.z = range_surface.astype(np.float32)
        self.payload = pack_xyz_payload(self.x, self.y, self.z)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


def build_frames() -> tuple[list[BenchmarkFrame], float]:
    start = time.perf_counter()
    frames = [BenchmarkFrame(index * 0.55) for index in range(FRAME_COUNT)]
    return frames, (time.perf_counter() - start) * 1000.0


BENCHMARK_FRAMES, PREBUILD_MS = build_frames()

app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #0e1422;
        color: rgba(246, 249, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
        overflow-y: auto;
        padding-right: 22px;
        padding-bottom: 40px;
    }

    VLayout.root::scrollbar-track,
    Panel::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb,
    Panel::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.72);
        border-radius: 999px;
    }

    GridLayout.grid {
        width: 100%;
        gap: 12px;
    }

    FlowLayout.controls {
        width: 100%;
        gap: 10px;
        row-gap: 8px;
    }

    Panel {
        background: rgba(18, 25, 39, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 12px;
        padding: 14px;
        gap: 10px;
        max-width: 100%;
        box-shadow: 0 12px 28px rgba(0, 0, 0, 0.24);
    }

    Panel.plot {
        height: 560px;
        overflow: hidden;
    }

    Label.title {
        color: white;
        font-size: 21px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(246, 249, 255, 0.72);
        line-height: 1.14;
    }

    Label.metric {
        width: 100%;
        background: rgba(90, 169, 255, 0.10);
        border: 1px solid rgba(90, 169, 255, 0.24);
        border-radius: 10px;
        color: rgba(235, 244, 255, 0.96);
        font-family: "Consolas";
        padding: 8px 10px;
    }

    Label.good {
        color: #74ddb0;
        font-weight: 800;
    }

    Label.interval {
        color: rgba(246, 249, 255, 0.82);
        font-weight: 800;
    }

    Slider.interval {
        width: 100%;
        accent: #5aa9ff;
        track-color: rgba(255, 255, 255, 0.18);
        thumb-color: #d8ecff;
    }

    Button {
        width: auto;
        min-width: 96px;
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 8px;
        color: rgba(246, 249, 255, 0.92);
        padding: 7px 10px;
    }

    Button.primary {
        background: rgba(90, 169, 255, 0.22);
        border-color: rgba(90, 169, 255, 0.55);
        color: white;
        font-weight: 800;
    }

    Scatter3D {
        width: 100%;
        flex-grow: 1;
        min-height: 420px;
        background: rgba(3, 8, 18, 0.54);
        border: 1px solid rgba(90, 169, 255, 0.26);
        border-radius: 12px;
        scatter-point-size: 2px;
        scatter-point-style: circle;
    }
    """
)


win = dg.Window("Scatter3D Frame Benchmark Probe", width=1180, height=800)

state_lock = threading.Lock()
stop_event = threading.Event()
runner_thread: threading.Thread | None = None

state = {
    "running": False,
    "mode": "idle",
    "submitted": 0,
    "frame_index": 0,
    "native_updates": 0,
    "set_ms": 0.0,
    "submit_latency_ms": 0.0,
    "completion_ms": 0.0,
    "wall_ms": 0.0,
    "fps": 0.0,
    "renderer_frame_ms": 0.0,
    "renderer_frames": 0,
    "native_pack_ms": 0.0,
    "native_decode_ms": 0.0,
    "native_bounds_ms": 0.0,
    "native_upload_ms": 0.0,
    "native_primary_upload_ms": 0.0,
    "native_lod_ms": 0.0,
    "native_grid_ms": 0.0,
    "native_overlay_ms": 0.0,
    "native_total_ms": 0.0,
    "payload_mb": 0.0,
    "queue_depth": 0,
    "visual_interval_ms": VISUAL_INTERVAL_MS,
}
set_times: list[float] = []
submit_latencies: list[float] = []


def percentile(values: list[float], pct: float) -> float:
    if not values:
        return 0.0
    index = min(len(values) - 1, max(0, int(round((len(values) - 1) * pct))))
    return sorted(values)[index]


def update_metrics_label() -> None:
    with state_lock:
        values = dict(state)
        set_sample = list(set_times)
        latency_sample = list(submit_latencies)
    avg_set = statistics.fmean(set_sample) if set_sample else 0.0
    p95_set = percentile(set_sample, 0.95)
    avg_latency = statistics.fmean(latency_sample) if latency_sample else 0.0
    submitted = int(values["submitted"])
    progress = (
        f"played: {submitted}    source frames: {FRAME_COUNT}    current: {values['frame_index']}"
        if values["mode"] == "visual playback"
        else f"submitted: {submitted}/{FRAME_COUNT}"
    )
    metrics.set_value(
        "\n".join(
            [
                f"points/frame: {POINTS:,} ({LIDAR_ROWS}x{LIDAR_COLS})    prebuilt frames: {FRAME_COUNT}",
                f"prebuild: {PREBUILD_MS:.2f} ms    fast interval: {FRAME_INTERVAL_MS:.1f} ms    visual interval: {values['visual_interval_ms']:.1f} ms",
                f"{progress}    native updates: {values['native_updates']}",
                f"mode: {values['mode']}    running: {values['running']}    queue depth: {values['queue_depth']}",
                f"wall: {values['wall_ms']:.2f} ms    data updates: {values['fps']:.1f}/s",
                f"renderer: {values['renderer_frame_ms']:.2f} ms/frame across {values['renderer_frames']} presented frames",
                f"burst completion: {values['completion_ms']:.2f} ms    last enqueue: {values['set_ms']:.2f} ms",
                f"avg enqueue: {avg_set:.2f} ms    p95 enqueue: {p95_set:.2f} ms",
                f"submit latency: {values['submit_latency_ms']:.2f} ms    avg latency: {avg_latency:.2f} ms",
                f"native pack: {values['native_pack_ms']:.2f} ms    decode+bounds: {values['native_decode_ms']:.2f}+{values['native_bounds_ms']:.2f} ms",
                f"upload total: {values['native_upload_ms']:.2f} ms    primary: {values['native_primary_upload_ms']:.2f} ms    lod: {values['native_lod_ms']:.2f} ms",
                f"chrome: grid {values['native_grid_ms']:.2f} ms    overlays {values['native_overlay_ms']:.2f} ms    native total: {values['native_total_ms']:.2f} ms",
                f"payload: {values['payload_mb']:.2f} MB",
            ]
        )
    )


def debug_snapshot(timeout_ms: int = 1000) -> dict[str, object]:
    return app.debug_snapshot(timeout_ms=timeout_ms)


def scatter_metrics_from_snapshot(snapshot: dict[str, object]) -> dict[str, object]:
    gpu = snapshot.get("gpu")
    if not isinstance(gpu, dict):
        return {}
    resources = gpu.get("resources")
    if not isinstance(resources, dict):
        return {}
    scatter_metrics = resources.get("scatter")
    return scatter_metrics if isinstance(scatter_metrics, dict) else {}


def apply_snapshot_metrics(snapshot: dict[str, object]) -> int:
    runtime = snapshot.get("runtime")
    depth = 0
    if isinstance(runtime, dict):
        depth = int(runtime.get("command_queue_depth") or 0)
    scatter_metrics = scatter_metrics_from_snapshot(snapshot)
    updates = int(scatter_metrics.get("updates") or 0)
    with state_lock:
        state["queue_depth"] = depth
        state["renderer_frame_ms"] = float(runtime.get("frame_ms") or 0.0) if isinstance(runtime, dict) else 0.0
        state["renderer_frames"] = int(runtime.get("frames_rendered") or 0) if isinstance(runtime, dict) else 0
        state["native_updates"] = updates
        state["native_pack_ms"] = float(scatter_metrics.get("last_pack_ms") or 0.0)
        state["native_decode_ms"] = float(scatter_metrics.get("last_decode_ms") or 0.0)
        state["native_bounds_ms"] = float(scatter_metrics.get("last_bounds_ms") or 0.0)
        state["native_upload_ms"] = float(scatter_metrics.get("last_upload_ms") or 0.0)
        state["native_primary_upload_ms"] = float(scatter_metrics.get("last_primary_upload_ms") or 0.0)
        state["native_lod_ms"] = float(scatter_metrics.get("last_lod_ms") or 0.0)
        state["native_grid_ms"] = float(scatter_metrics.get("last_grid_ms") or 0.0)
        state["native_overlay_ms"] = float(scatter_metrics.get("last_overlay_ms") or 0.0)
        state["native_total_ms"] = float(scatter_metrics.get("last_total_native_ms") or 0.0)
        state["payload_mb"] = float(scatter_metrics.get("last_payload_bytes") or 0.0) / 1_000_000.0
    return updates


def submit_frame(frame: BenchmarkFrame, submitted_at: float, *, coalesce: bool = True) -> bool:
    handle = getattr(app, "_handle", None)
    if handle is None:
        return False
    start = time.perf_counter()
    handle.enqueue_set_scatter_points_packed(
        scatter.id,
        frame.payload,
        pack_ms=0.0,
        enqueue_epoch_ms=time.time() * 1000.0,
        colormap=scatter.colormap,
        payload_format="xyz_f32_v0",
        coalesce=coalesce,
    )
    set_ms = (time.perf_counter() - start) * 1000.0
    submit_latency_ms = (time.perf_counter() - submitted_at) * 1000.0
    with state_lock:
        state["submitted"] += 1
        state["frame_index"] = int((state["submitted"] - 1) % FRAME_COUNT)
        state["set_ms"] = set_ms
        state["submit_latency_ms"] = submit_latency_ms
        set_times.append(set_ms)
        submit_latencies.append(submit_latency_ms)
    return True


def run_fast_batch() -> None:
    batch_start = time.perf_counter()
    for frame_index, frame in enumerate(BENCHMARK_FRAMES, start=1):
        if stop_event.is_set():
            break
        with state_lock:
            if not state["running"]:
                break
        submitted_at = time.perf_counter()
        if not submit_frame(frame, submitted_at, coalesce=False):
            break
        with state_lock:
            state["frame_index"] = frame_index
        if FRAME_INTERVAL_MS > 0.0 and stop_event.wait(FRAME_INTERVAL_MS / 1000.0):
            break
        wall_ms = (time.perf_counter() - batch_start) * 1000.0
        with state_lock:
            state["wall_ms"] = wall_ms
    submitted = 0
    with state_lock:
        submitted = int(state["submitted"])
    if submitted:
        try:
            # One snapshot after the burst acts as a completion fence because it is enqueued
            # behind the SetScatterPointsPacked commands. Per-frame snapshots distort the
            # benchmark and can keep the native drain loop busy enough to hit its batch cap.
            snapshot = debug_snapshot(
                timeout_ms=max(1000, int(COMMAND_DRAIN_TIMEOUT_S * 1000.0))
            )
            apply_snapshot_metrics(snapshot)
        except RuntimeError:
            pass
    completion_ms = (time.perf_counter() - batch_start) * 1000.0
    with state_lock:
        state["completion_ms"] = completion_ms
        state["wall_ms"] = completion_ms
        state["fps"] = submitted / (completion_ms / 1000.0) if completion_ms > 0.0 else 0.0
        state["running"] = False
        state["mode"] = "idle"
    update_metrics_label()


def run_visual_playback() -> None:
    batch_start = time.perf_counter()
    next_metrics_at = batch_start + 0.35
    frame_index = 0
    while not stop_event.is_set():
        with state_lock:
            if not state["running"]:
                break
        submitted_at = time.perf_counter()
        if not submit_frame(BENCHMARK_FRAMES[frame_index], submitted_at, coalesce=True):
            break
        frame_index = (frame_index + 1) % FRAME_COUNT
        wall_ms = (time.perf_counter() - batch_start) * 1000.0
        with state_lock:
            submitted = int(state["submitted"])
            state["frame_index"] = frame_index
            state["wall_ms"] = wall_ms
            state["fps"] = submitted / (wall_ms / 1000.0) if wall_ms > 0.0 else 0.0
        now = time.perf_counter()
        if now >= next_metrics_at:
            next_metrics_at = now + 0.35
            try:
                apply_snapshot_metrics(debug_snapshot(timeout_ms=1000))
            except RuntimeError:
                pass
            update_metrics_label()
        elapsed_ms = (time.perf_counter() - submitted_at) * 1000.0
        with state_lock:
            visual_interval_ms = float(state["visual_interval_ms"])
        delay_ms = max(1.0, visual_interval_ms - elapsed_ms)
        if stop_event.wait(delay_ms / 1000.0):
            break
    try:
        apply_snapshot_metrics(debug_snapshot(timeout_ms=1000))
    except RuntimeError:
        pass
    completion_ms = (time.perf_counter() - batch_start) * 1000.0
    with state_lock:
        submitted = int(state["submitted"])
        state["completion_ms"] = completion_ms
        state["wall_ms"] = completion_ms
        state["fps"] = submitted / (completion_ms / 1000.0) if completion_ms > 0.0 else 0.0
        state["running"] = False
        state["mode"] = "idle"
    update_metrics_label()


def start_benchmark(*, visual: bool = False) -> None:
    global runner_thread
    mode = "visual playback" if visual else "fast benchmark"
    with state_lock:
        if state["running"]:
            return
        state.update(
            running=True,
            mode=mode,
            submitted=0,
            frame_index=0,
            wall_ms=0.0,
            fps=0.0,
            completion_ms=0.0,
        )
        set_times.clear()
        submit_latencies.clear()
    stop_event.clear()
    runner_thread = threading.Thread(
        target=run_visual_playback if visual else run_fast_batch,
        daemon=True,
    )
    runner_thread.start()
    update_metrics_label()


def stop_benchmark() -> None:
    with state_lock:
        state["running"] = False
        state["mode"] = "idle"
    update_metrics_label()


def reset_benchmark() -> None:
    stop_benchmark()
    with state_lock:
        state.update(
            mode="idle",
            submitted=0,
            frame_index=0,
            native_updates=0,
            set_ms=0.0,
            submit_latency_ms=0.0,
            completion_ms=0.0,
            wall_ms=0.0,
            fps=0.0,
        )
        set_times.clear()
        submit_latencies.clear()
    update_metrics_label()


def set_visual_interval(value: float) -> None:
    interval_ms = max(1.0, float(value))
    with state_lock:
        state["visual_interval_ms"] = interval_ms
    visual_interval_label.set_value(f"Visual interval: {interval_ms:.1f} ms")
    update_metrics_label()


def print_metrics() -> None:
    with state_lock:
        values = dict(state)
        set_sample = list(set_times)
        latency_sample = list(submit_latencies)
    summary = {
        "points": POINTS,
        "frames": values["submitted"],
        "prebuilt_frames": FRAME_COUNT,
        "interval_ms": FRAME_INTERVAL_MS,
        "visual_interval_ms": round(values["visual_interval_ms"], 3),
        "wall_ms": round(values["wall_ms"], 3),
        "data_updates_per_s": round(values["fps"], 3),
        "renderer_frame_ms": round(values["renderer_frame_ms"], 3),
        "renderer_frames": values["renderer_frames"],
        "last_enqueue_ms": round(values["set_ms"], 3),
        "avg_enqueue_ms": round(statistics.fmean(set_sample), 3) if set_sample else 0.0,
        "p95_enqueue_ms": round(percentile(set_sample, 0.95), 3),
        "avg_submit_latency_ms": round(statistics.fmean(latency_sample), 3)
        if latency_sample
        else 0.0,
        "native_updates": values["native_updates"],
        "native_total_ms": round(values["native_total_ms"], 3),
        "payload_mb": round(values["payload_mb"], 3),
    }
    print("Scatter3D prebuilt frame benchmark:", summary, flush=True)


with dg.VLayout(class_="root"):
    dg.Label("Scatter3D prebuilt frame benchmark", class_="title")
    dg.Label(
        "Prebuilds flat LiDAR-style scan frames with 125k+ points, then submits that batch to Scatter3D when Run is clicked.",
        class_="caption",
    )

    with probe_grid(gap=12, min_column_width=360, class_="grid"):
        with dg.Panel("Controls"):
            with dg.FlowLayout(class_="controls", gap=10, row_gap=8):
                dg.Button(
                    f"Run fast {FRAME_COUNT}",
                    class_="primary",
                    on_click=lambda: start_benchmark(visual=False),
                )
                dg.Button("Play visual", on_click=lambda: start_benchmark(visual=True))
                dg.Button("Stop", on_click=stop_benchmark)
                dg.Button("Reset", on_click=reset_benchmark)
                dg.Button("Fit", on_click=lambda: scatter.fit())
                dg.Button("Print", on_click=print_metrics)
            visual_interval_label = dg.Label(
                f"Visual interval: {VISUAL_INTERVAL_MS:.1f} ms",
                class_="interval",
            )
            dg.Slider(
                VISUAL_INTERVAL_MS,
                min=1,
                max=80,
                step=1,
                class_="interval",
                on_change=set_visual_interval,
                tooltip="Delay between submitted frames during Play visual.",
            )
            metrics = dg.Label("Ready.", class_="metric")
            dg.Label(
                "Run fast measures throughput and may only present the final frame. Play visual loops paced frames until Stop.",
                class_="good",
            )

        with dg.Panel("Live plot", class_="plot"):
            scatter = dg.Scatter3D(
                BENCHMARK_FRAMES[0],
                x="x",
                y="y",
                z="z",
                colormap="turbo",
                axis_x="horizontal",
                axis_y="vertical",
                axis_z="range",
            )

if os.environ.get("DRAGONGUI_BENCH_AUTOSTART") == "1":
    def _autostart_benchmark() -> None:
        visual = os.environ.get("DRAGONGUI_BENCH_AUTOSTART_VISUAL") == "1"
        try:
            app.call_soon_threadsafe(lambda: start_benchmark(visual=visual))
        except RuntimeError:
            pass

    _timer = threading.Timer(0.15, _autostart_benchmark)
    _timer.daemon = True
    _timer.start()

try:
    print(app.run(win))
finally:
    stop_event.set()
