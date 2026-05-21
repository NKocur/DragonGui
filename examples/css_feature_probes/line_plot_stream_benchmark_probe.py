from __future__ import annotations

import json
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

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual benchmark requirement
    raise SystemExit("line_plot_stream_benchmark_probe.py requires NumPy") from exc


INITIAL_POINTS = max(2, int(os.environ.get("DRAGONGUI_LINE_BENCH_INITIAL_POINTS", "16384")))
BATCH_POINTS = max(1, int(os.environ.get("DRAGONGUI_LINE_BENCH_BATCH_POINTS", "256")))
BATCHES = max(1, int(os.environ.get("DRAGONGUI_LINE_BENCH_BATCHES", "80")))
MAX_POINTS = max(1, int(os.environ.get("DRAGONGUI_LINE_BENCH_MAX_POINTS", "65536")))
WINDOW_SIZE = float(os.environ.get("DRAGONGUI_LINE_BENCH_WINDOW_SIZE", "4096"))
MODE = os.environ.get("DRAGONGUI_LINE_BENCH_MODE", "widget").strip().lower()
if MODE not in {"widget", "packed"}:
    raise SystemExit("DRAGONGUI_LINE_BENCH_MODE must be widget or packed")


class SignalFrame:
    columns = ("time", "value")
    dtypes = ("float32", "float32")

    def __init__(self, x: np.ndarray, y: np.ndarray) -> None:
        self.time = x
        self.value = y
        self.shape = (len(x), 2)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


def make_values(start: int, count: int) -> tuple[np.ndarray, np.ndarray]:
    x = np.arange(start, start + count, dtype=np.float32)
    phase = x * np.float32(0.018)
    slow = np.sin(phase * np.float32(0.31)) * np.float32(0.42)
    fast = np.sin(phase * np.float32(2.7)) * np.float32(0.11)
    spike = np.exp(-((np.mod(x, 997.0) - 34.0) ** 2) * np.float32(0.0025))
    y = (np.sin(phase) + slow + fast + spike * np.float32(0.9)).astype(np.float32)
    return x, y


def pack_xy(x: np.ndarray, y: np.ndarray) -> memoryview:
    payload = np.empty((len(x), 2), dtype="<f4")
    payload[:, 0] = x
    payload[:, 1] = y
    return memoryview(payload.view(np.uint8).reshape(-1))


initial_x, initial_y = make_values(0, INITIAL_POINTS)
initial_frame = SignalFrame(initial_x, initial_y)
batches = [make_values(INITIAL_POINTS + index * BATCH_POINTS, BATCH_POINTS) for index in range(BATCHES)]
packed_batches = [pack_xy(x, y) for x, y in batches]

app = dg.App(theme=dg.Theme.dark(accent="#66d9a8", radius=6))
win = dg.Window("LinePlot Streaming Benchmark", width=1120, height=720)

app.stylesheet(
    """
    Window {
        background: #11151d;
        color: rgba(245, 248, 255, 0.94);
        padding: 14px;
        gap: 10px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 10px;
    }

    FlowLayout.controls {
        width: 100%;
        gap: 8px;
        row-gap: 8px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.metrics {
        width: 100%;
        background: rgba(102, 217, 168, 0.10);
        border: 1px solid rgba(102, 217, 168, 0.28);
        border-radius: 8px;
        color: rgba(238, 255, 248, 0.96);
        font-family: "Consolas";
        padding: 8px 10px;
    }

    Button {
        width: auto;
        min-width: 96px;
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 8px;
        color: rgba(245, 248, 255, 0.92);
        padding: 7px 10px;
    }

    Button.primary {
        background: rgba(102, 217, 168, 0.22);
        border-color: rgba(102, 217, 168, 0.55);
        color: white;
        font-weight: 800;
    }

    LinePlot {
        width: 100%;
        flex-grow: 1;
        min-height: 500px;
        background: rgba(4, 9, 16, 0.70);
        border: 1px solid rgba(102, 217, 168, 0.28);
        border-radius: 8px;
        padding: 10px;
    }
    """
)

state_lock = threading.Lock()
stop_event = threading.Event()
runner_thread: threading.Thread | None = None
append_times: list[float] = []

state = {
    "running": False,
    "submitted": 0,
    "wall_ms": 0.0,
    "updates": 0,
    "renderer_frame_ms": 0.0,
    "renderer_frames": 0,
    "renderer_work_ms": 0.0,
    "renderer_prepare_ms": 0.0,
    "renderer_acquire_ms": 0.0,
    "renderer_encode_ms": 0.0,
    "renderer_submit_ms": 0.0,
    "renderer_present_ms": 0.0,
    "renderer_wall_fps": 0.0,
    "renderer_present_mode": "default",
    "renderer_frame_latency": 0,
    "queue_depth": 0,
    "native_total_ms": 0.0,
    "native_decode_ms": 0.0,
    "native_bounds_ms": 0.0,
    "native_trim_ms": 0.0,
    "native_window_ms": 0.0,
    "native_count_ms": 0.0,
    "payload_kb": 0.0,
    "points": INITIAL_POINTS,
    "visible_points": 0,
    "primitive_rects": 0,
    "primitive_simple": 0,
    "primitive_lines": 0,
    "primitive_complex": 0,
    "primitive_batches": 0,
    "primitive_split_collapsed": False,
    "primitive_bytes": 0,
    "primitive_upload_ms": 0.0,
    "primitive_emit_ms": 0.0,
    "line_renderer_enabled": False,
    "line_renderer_aa_width": 0.0,
    "line_renderer_max_segments": 0,
    "line_renderer_decimation_mode": "off",
    "line_renderer_series": 0,
    "line_renderer_source_points": 0,
    "line_renderer_decimated_series": 0,
    "line_renderer_points": 0,
    "line_renderer_segments": 0,
    "line_renderer_bytes": 0,
    "line_renderer_decimate_ms": 0.0,
    "line_renderer_emit_ms": 0.0,
    "line_renderer_upload_ms": 0.0,
    "line_renderer_encode_ms": 0.0,
}


def percentile(values: list[float], pct: float) -> float:
    if not values:
        return 0.0
    index = min(len(values) - 1, max(0, int(round((len(values) - 1) * pct))))
    return sorted(values)[index]


def line_metrics_from_snapshot(snapshot: dict[str, object]) -> dict[str, object]:
    gpu = snapshot.get("gpu")
    resources = gpu.get("resources") if isinstance(gpu, dict) else None
    metrics = resources.get("line_plot") if isinstance(resources, dict) else None
    return metrics if isinstance(metrics, dict) else {}


def primitive_metrics_from_snapshot(snapshot: dict[str, object]) -> dict[str, object]:
    gpu = snapshot.get("gpu")
    renderer = gpu.get("renderer") if isinstance(gpu, dict) else None
    primitives = renderer.get("primitives") if isinstance(renderer, dict) else None
    return primitives if isinstance(primitives, dict) else {}


def line_renderer_metrics_from_snapshot(snapshot: dict[str, object]) -> dict[str, object]:
    gpu = snapshot.get("gpu")
    renderer = gpu.get("renderer") if isinstance(gpu, dict) else None
    line_renderer = renderer.get("line_plot_renderer") if isinstance(renderer, dict) else None
    return line_renderer if isinstance(line_renderer, dict) else {}


def apply_snapshot_metrics(snapshot: dict[str, object]) -> None:
    runtime = snapshot.get("runtime")
    gpu = snapshot.get("gpu")
    renderer = gpu.get("renderer") if isinstance(gpu, dict) else None
    line_metrics = line_metrics_from_snapshot(snapshot)
    primitive = primitive_metrics_from_snapshot(snapshot)
    line_renderer = line_renderer_metrics_from_snapshot(snapshot)
    with state_lock:
        if isinstance(runtime, dict):
            state["queue_depth"] = int(runtime.get("command_queue_depth") or 0)
            state["renderer_frame_ms"] = float(runtime.get("frame_ms") or 0.0)
            state["renderer_frames"] = int(runtime.get("frames_rendered") or 0)
            state["renderer_work_ms"] = float(
                runtime.get("frame_work_ms_avg", runtime.get("frame_work_ms")) or 0.0
            )
            state["renderer_prepare_ms"] = float(
                runtime.get("frame_prepare_ms_avg", runtime.get("frame_prepare_ms")) or 0.0
            )
            state["renderer_acquire_ms"] = float(
                runtime.get("frame_acquire_ms_avg", runtime.get("frame_acquire_ms")) or 0.0
            )
            state["renderer_encode_ms"] = float(
                runtime.get("frame_encode_ms_avg", runtime.get("frame_encode_ms")) or 0.0
            )
            state["renderer_submit_ms"] = float(
                runtime.get("frame_submit_ms_avg", runtime.get("frame_submit_ms")) or 0.0
            )
            state["renderer_present_ms"] = float(
                runtime.get("frame_present_ms_avg", runtime.get("frame_present_ms")) or 0.0
            )
            state["renderer_wall_fps"] = float(runtime.get("wall_fps") or 0.0)
        if isinstance(renderer, dict):
            state["renderer_present_mode"] = str(renderer.get("present_mode") or "default")
            state["renderer_frame_latency"] = int(
                renderer.get("desired_maximum_frame_latency") or 0
            )
        state["updates"] = int(line_metrics.get("updates") or 0)
        state["native_total_ms"] = float(line_metrics.get("last_total_native_ms") or 0.0)
        state["native_decode_ms"] = float(line_metrics.get("last_decode_ms") or 0.0)
        state["native_bounds_ms"] = float(line_metrics.get("last_bounds_ms") or 0.0)
        state["native_trim_ms"] = float(line_metrics.get("last_trim_ms") or 0.0)
        state["native_window_ms"] = float(line_metrics.get("last_window_ms") or 0.0)
        state["native_count_ms"] = float(line_metrics.get("last_count_ms") or 0.0)
        state["payload_kb"] = float(line_metrics.get("last_payload_bytes") or 0.0) / 1000.0
        state["points"] = int(line_metrics.get("last_point_count") or state["points"])
        state["visible_points"] = int(line_metrics.get("last_visible_point_count") or 0)
        state["primitive_rects"] = int(primitive.get("rect_count") or 0)
        state["primitive_simple"] = int(primitive.get("simple_count") or 0)
        state["primitive_lines"] = int(primitive.get("line_count") or 0)
        state["primitive_complex"] = int(primitive.get("complex_count") or 0)
        state["primitive_batches"] = int(primitive.get("base_batches") or 0) + int(
            primitive.get("overlay_batches") or 0
        )
        state["primitive_split_collapsed"] = bool(primitive.get("split_collapsed"))
        state["primitive_bytes"] = int(primitive.get("buffer_bytes") or 0)
        state["primitive_upload_ms"] = float(primitive.get("last_upload_ms") or 0.0)
        state["primitive_emit_ms"] = float(primitive.get("last_emit_ms") or 0.0)
        state["line_renderer_enabled"] = bool(line_renderer.get("enabled"))
        state["line_renderer_aa_width"] = float(line_renderer.get("aa_width") or 0.0)
        state["line_renderer_max_segments"] = int(line_renderer.get("max_segments_per_series") or 0)
        state["line_renderer_decimation_mode"] = str(line_renderer.get("decimation_mode") or "off")
        state["line_renderer_series"] = int(line_renderer.get("series_count") or 0)
        state["line_renderer_source_points"] = int(line_renderer.get("source_point_count") or 0)
        state["line_renderer_decimated_series"] = int(
            line_renderer.get("decimated_series_count") or 0
        )
        state["line_renderer_points"] = int(line_renderer.get("point_count") or 0)
        state["line_renderer_segments"] = int(line_renderer.get("segment_count") or 0)
        state["line_renderer_bytes"] = int(line_renderer.get("buffer_bytes") or 0)
        state["line_renderer_decimate_ms"] = float(line_renderer.get("last_decimate_ms") or 0.0)
        state["line_renderer_emit_ms"] = float(line_renderer.get("last_emit_ms") or 0.0)
        state["line_renderer_upload_ms"] = float(line_renderer.get("last_upload_ms") or 0.0)
        state["line_renderer_encode_ms"] = float(line_renderer.get("last_encode_ms") or 0.0)


def update_metrics_label() -> None:
    with state_lock:
        values = dict(state)
        samples = list(append_times)
    avg_append = statistics.fmean(samples) if samples else 0.0
    p95_append = percentile(samples, 0.95)
    metrics.set_value(
        "\n".join(
            [
                f"mode: {MODE}    submitted: {values['submitted']}/{BATCHES}    queue depth: {values['queue_depth']}",
                f"points: {values['points']:,} retained    visible: {values['visible_points']:,}    window: {WINDOW_SIZE:g}",
                f"wall: {values['wall_ms']:.2f} ms    append avg/p95: {avg_append:.3f}/{p95_append:.3f} ms",
                f"renderer: {values['renderer_frame_ms']:.3f} ms/frame across {values['renderer_frames']} frames    wall fps: {values['renderer_wall_fps']:.1f}",
                f"frame mode: {values['renderer_present_mode']}    latency: {values['renderer_frame_latency']}    stage avg: work {values['renderer_work_ms']:.3f} ms    prepare {values['renderer_prepare_ms']:.3f} ms    acquire {values['renderer_acquire_ms']:.3f} ms    encode {values['renderer_encode_ms']:.3f} ms    submit {values['renderer_submit_ms']:.3f} ms    present {values['renderer_present_ms']:.3f} ms",
                f"native append: total {values['native_total_ms']:.3f} ms    decode {values['native_decode_ms']:.3f} ms    bounds {values['native_bounds_ms']:.3f} ms    trim {values['native_trim_ms']:.3f} ms    window {values['native_window_ms']:.3f} ms    count {values['native_count_ms']:.3f} ms",
                f"payload: {values['payload_kb']:.1f} KB    primitive rects: {values['primitive_rects']:,}    simple: {values['primitive_simple']:,}    line: {values['primitive_lines']:,}    complex: {values['primitive_complex']:,}",
                f"primitive batches: {values['primitive_batches']:,}    split collapsed: {values['primitive_split_collapsed']}",
                f"primitive buffer: {values['primitive_bytes'] / 1000.0:.1f} KB    emit {values['primitive_emit_ms']:.3f} ms    upload {values['primitive_upload_ms']:.3f} ms",
                f"line renderer: {values['line_renderer_enabled']}    aa: {values['line_renderer_aa_width']:.2f}    max seg: {values['line_renderer_max_segments']:,}    decim: {values['line_renderer_decimation_mode']}    series: {values['line_renderer_series']:,}    source: {values['line_renderer_source_points']:,}    points: {values['line_renderer_points']:,}    segments: {values['line_renderer_segments']:,}",
                f"line renderer buffer: {values['line_renderer_bytes'] / 1000.0:.1f} KB    decim {values['line_renderer_decimate_ms']:.3f} ms    emit {values['line_renderer_emit_ms']:.3f} ms    upload {values['line_renderer_upload_ms']:.3f} ms    encode {values['line_renderer_encode_ms']:.3f} ms",
            ]
        )
    )


def submit_batch(index: int) -> bool:
    x, y = batches[index]
    start = time.perf_counter()
    if MODE == "packed":
        handle = getattr(app, "_handle", None)
        if handle is None:
            return False
        handle.enqueue_append_line_plot_points_packed(
            line_plot.id,
            "value",
            packed_batches[index],
            max_points=MAX_POINTS,
        )
    else:
        line_plot.append_points(x, y, max_points=MAX_POINTS)
    elapsed_ms = (time.perf_counter() - start) * 1000.0
    with state_lock:
        state["submitted"] += 1
        append_times.append(elapsed_ms)
    return True


def run_benchmark() -> None:
    bench_t0 = time.perf_counter()
    with state_lock:
        state["running"] = True
        state["submitted"] = 0
        state["wall_ms"] = 0.0
        append_times.clear()
    for index in range(BATCHES):
        if stop_event.is_set():
            break
        if not submit_batch(index):
            break
    with state_lock:
        submitted = int(state["submitted"])
    if submitted:
        try:
            snapshot = app.debug_snapshot(timeout_ms=5000)
            apply_snapshot_metrics(snapshot)
        except RuntimeError:
            pass
    wall_ms = (time.perf_counter() - bench_t0) * 1000.0
    with state_lock:
        state["wall_ms"] = wall_ms
        state["running"] = False
    update_metrics_label()
    print_summary()


def start_benchmark() -> None:
    global runner_thread
    with state_lock:
        if state["running"]:
            return
    runner_thread = threading.Thread(target=run_benchmark, daemon=True)
    runner_thread.start()
    update_metrics_label()


def print_summary() -> None:
    with state_lock:
        values = dict(state)
        samples = list(append_times)
    summary = {
        "mode": MODE,
        "initial_points": INITIAL_POINTS,
        "batch_points": BATCH_POINTS,
        "batches": BATCHES,
        "max_points": MAX_POINTS,
        "window_size": WINDOW_SIZE,
        "split_env": os.environ.get("DRAGONGUI_PRIMITIVE_SPLIT", "default"),
        "present_mode_env": os.environ.get("DRAGONGUI_PRESENT_MODE", "default"),
        "surface_frame_latency_env": os.environ.get(
            "DRAGONGUI_SURFACE_FRAME_LATENCY", "default"
        ),
        "line_renderer_env": os.environ.get("DRAGONGUI_LINE_PLOT_RENDERER", "default"),
        "line_renderer_aa_env": os.environ.get("DRAGONGUI_LINE_PLOT_AA_WIDTH", "default"),
        "line_renderer_decimation_env": os.environ.get(
            "DRAGONGUI_LINE_PLOT_DECIMATION", "default"
        ),
        "line_renderer_max_segments_env": os.environ.get(
            "DRAGONGUI_LINE_PLOT_MAX_SEGMENTS", "default"
        ),
        "submitted": values["submitted"],
        "wall_ms": round(values["wall_ms"], 3),
        "append_avg_ms": round(statistics.fmean(samples), 4) if samples else 0.0,
        "append_p95_ms": round(percentile(samples, 0.95), 4),
        "native_updates": values["updates"],
        "native_total_ms": round(values["native_total_ms"], 4),
        "native_decode_ms": round(values["native_decode_ms"], 4),
        "native_bounds_ms": round(values["native_bounds_ms"], 4),
        "native_trim_ms": round(values["native_trim_ms"], 4),
        "native_window_ms": round(values["native_window_ms"], 4),
        "native_count_ms": round(values["native_count_ms"], 4),
        "renderer_frame_ms": round(values["renderer_frame_ms"], 4),
        "renderer_frames": values["renderer_frames"],
        "renderer_work_ms": round(values["renderer_work_ms"], 4),
        "renderer_prepare_ms": round(values["renderer_prepare_ms"], 4),
        "renderer_acquire_ms": round(values["renderer_acquire_ms"], 4),
        "renderer_encode_ms": round(values["renderer_encode_ms"], 4),
        "renderer_submit_ms": round(values["renderer_submit_ms"], 4),
        "renderer_present_ms": round(values["renderer_present_ms"], 4),
        "renderer_wall_fps": round(values["renderer_wall_fps"], 4),
        "renderer_present_mode": values["renderer_present_mode"],
        "renderer_frame_latency": values["renderer_frame_latency"],
        "points": values["points"],
        "visible_points": values["visible_points"],
        "primitive_rects": values["primitive_rects"],
        "primitive_simple": values["primitive_simple"],
        "primitive_lines": values["primitive_lines"],
        "primitive_complex": values["primitive_complex"],
        "primitive_batches": values["primitive_batches"],
        "primitive_split_collapsed": values["primitive_split_collapsed"],
        "primitive_kb": round(values["primitive_bytes"] / 1000.0, 3),
        "primitive_emit_ms": round(values["primitive_emit_ms"], 4),
        "primitive_upload_ms": round(values["primitive_upload_ms"], 4),
        "line_renderer_enabled": values["line_renderer_enabled"],
        "line_renderer_aa_width": round(values["line_renderer_aa_width"], 4),
        "line_renderer_max_segments": values["line_renderer_max_segments"],
        "line_renderer_decimation_mode": values["line_renderer_decimation_mode"],
        "line_renderer_series": values["line_renderer_series"],
        "line_renderer_source_points": values["line_renderer_source_points"],
        "line_renderer_decimated_series": values["line_renderer_decimated_series"],
        "line_renderer_points": values["line_renderer_points"],
        "line_renderer_segments": values["line_renderer_segments"],
        "line_renderer_kb": round(values["line_renderer_bytes"] / 1000.0, 3),
        "line_renderer_decimate_ms": round(values["line_renderer_decimate_ms"], 4),
        "line_renderer_emit_ms": round(values["line_renderer_emit_ms"], 4),
        "line_renderer_upload_ms": round(values["line_renderer_upload_ms"], 4),
        "line_renderer_encode_ms": round(values["line_renderer_encode_ms"], 4),
    }
    print("LinePlot streaming benchmark:", json.dumps(summary, sort_keys=True), flush=True)


with dg.VLayout(class_="root"):
    dg.Label("LinePlot streaming benchmark", class_="title")
    with dg.FlowLayout(class_="controls"):
        dg.Button(f"Run {BATCHES}", class_="primary", on_click=start_benchmark)
        dg.Button("Print", on_click=print_summary)
    metrics = dg.Label("Ready.", class_="metrics")
    line_plot = dg.LinePlot(
        initial_frame,
        x="time",
        y="value",
        label="value",
        color="#66d9a8",
        x_label="sample",
        y_label="signal",
        line_width=2.0,
        window_size=WINDOW_SIZE,
        max_points=MAX_POINTS,
        show_grid=True,
        show_axes=True,
        show_ticks=True,
        show_toolbar=False,
        show_legend=False,
        interaction="pan",
    )

if os.environ.get("DRAGONGUI_BENCH_AUTOSTART") == "1":
    timer = threading.Timer(0.15, lambda: app.call_soon_threadsafe(start_benchmark))
    timer.daemon = True
    timer.start()

try:
    result = app.run(win)
    if os.environ.get("DRAGONGUI_BENCH_AUTOSTART") == "1":
        apply_snapshot_metrics(result.get("debug_snapshot", {}))
        print_summary()
    else:
        print(result)
finally:
    stop_event.set()
