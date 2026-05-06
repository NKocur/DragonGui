from __future__ import annotations

import math
import os
import platform
import sys
import threading
import time
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - visual lab dependency
    raise SystemExit("scatter_perf_lab.py requires NumPy") from exc


def _auto_pi_profile() -> bool:
    if not (
        sys.platform.startswith("linux")
        and platform.machine().lower() in {"aarch64", "arm64"}
    ):
        return False
    try:
        model = Path("/proc/device-tree/model").read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return True
    return "raspberry pi" in model.lower()


def _demo_profile() -> str:
    requested = os.environ.get("DRAGONGUI_PROFILE", "auto").strip().lower()
    if requested in {"pi", "rpi", "raspberry-pi", "raspberry_pi"}:
        return "pi"
    if requested == "desktop":
        return "desktop"
    return "pi" if _auto_pi_profile() else "desktop"


DG_PROFILE = _demo_profile()
IS_PI_PROFILE = DG_PROFILE == "pi"
INITIAL_POINTS = 40_000 if IS_PI_PROFILE else 125_000
INITIAL_LOD_THRESHOLD = 25_000 if IS_PI_PROFILE else 50_000
MAX_LOD_THRESHOLD = 100_000 if IS_PI_PROFILE else 500_000
WORKLOADS = (
    (("25k", 25_000), ("50k", 50_000), ("100k", 100_000))
    if IS_PI_PROFILE
    else (("125k", 125_000), ("300k", 300_000), ("1M", 1_000_000))
)


class ScatterFrame:
    columns = ("x", "y", "z")
    dtypes = ("float32", "float32", "float32")

    def __init__(self, n: int, *, phase: float = 0.0) -> None:
        self.shape = (n, 3)
        side = max(2, int(math.sqrt(n)))
        rows = int(math.ceil(n / side))
        u, v = np.meshgrid(
            np.linspace(-1.0, 1.0, side, dtype=np.float32),
            np.linspace(-1.0, 1.0, rows, dtype=np.float32),
        )
        u = u.ravel()[:n]
        v = v.ravel()[:n]
        p = np.float32(phase)
        spiral = np.sin((u * 2.0 + v * 1.2) * np.float32(math.tau) + p)
        ridge = np.exp(-((u - np.sin(p) * 0.45) ** 2) * np.float32(12.0))
        self.x = (u * np.float32(18.0) + spiral * np.float32(0.35)).astype(np.float32)
        self.y = (v * np.float32(10.0) + np.cos(u * np.float32(math.tau) + p) * np.float32(0.24)).astype(np.float32)
        self.z = (
            np.float32(14.0)
            + np.sin(u * np.float32(math.tau * 3.0) + p) * np.float32(1.8)
            + np.cos(v * np.float32(math.tau * 2.0) - p) * np.float32(1.1)
            + ridge * np.float32(5.0)
        ).astype(np.float32)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #0d1320;
        color: rgba(246, 249, 255, 0.94);
        padding: 16px;
        gap: 12px;
        font-size: 14px;
    }

    HLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
    }

    Panel.controls {
        width: 330px;
        min-width: 330px;
        padding: 12px;
        gap: 9px;
        overflow-y: auto;
        background: linear-gradient(145deg, rgba(24, 35, 58, 0.98), rgba(11, 17, 30, 0.98));
        border: 1px solid rgba(118, 151, 204, 0.30);
        border-radius: 10px;
    }

    Panel.controls::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    Panel.controls::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.70);
        border-radius: 999px;
    }

    Panel.plot {
        padding: 10px;
        gap: 8px;
        min-width: 520px;
        min-height: 520px;
        flex-grow: 1;
        background: rgba(7, 11, 21, 0.84);
        border: 1px solid rgba(118, 151, 204, 0.26);
        border-radius: 10px;
    }

    Scatter3D.lab {
        width: 100%;
        height: 100%;
        min-height: 500px;
        background: #050914;
        border-radius: 8px;
        scatter-point-style: circle;
    }

    Label.title {
        font-size: 18px;
        font-weight: 700;
    }

    Label.subtle {
        color: rgba(211, 224, 245, 0.72);
        line-height: 1.2;
        text-wrap: wrap;
    }

    Label.readout {
        font-family: monospace;
        font-size: 12px;
        line-height: 1.22;
        color: rgba(235, 243, 255, 0.88);
        text-wrap: wrap;
    }

    HLayout.row {
        gap: 8px;
        height: 34px;
    }

    Button {
        height: 30px;
        text-align: center;
    }
    """
)

win = dg.Window("Scatter Perf Lab", width=1180, height=760)

state = {
    "points": INITIAL_POINTS,
    "point_size": 3.6,
    "lod_enabled": False,
    "lod_threshold": INITIAL_LOD_THRESHOLD,
    "lod_factor": 4,
    "auto_point_size": True,
    "interactive_render_scale": 1.0,
    "auto_quality": False,
    "quality_target_fps": 10.0,
    "frame_driver": True,
    "frame_driver_fps": 60.0,
    "streaming": False,
    "stream_hz_target": 8.0,
    "stream_phase": 0.0,
    "stream_updates": 0,
    "stream_last_update": 0.0,
    "stream_window": [],
    "stream_build_ms": 0.0,
    "stream_generate_ms": 0.0,
    "stream_pack_ms": 0.0,
    "stream_enqueue_ms": 0.0,
    "stream_producer_ms": 0.0,
    "stream_late_ms": 0.0,
    "last_stats_text": "",
    "update_mode": "set_points",
}
state_lock = threading.Lock()
stop_event = threading.Event()

initial_frame = ScatterFrame(state["points"])
scatter: dg.Scatter3D
live_frame: dg.ScatterLiveFrame | None = None
status: dg.Label
stats: dg.Label
point_size_slider: dg.Slider
threshold_slider: dg.Slider
factor_slider: dg.Slider
render_scale_slider: dg.Slider
target_fps_slider: dg.Slider
frame_driver_fps_slider: dg.Slider
stream_hz_slider: dg.Slider


def set_status(text: str) -> None:
    status.set_value(text)


def reset_live_frame() -> None:
    global live_frame
    if live_frame is not None:
        live_frame.remove()
        live_frame = None


def ensure_live_frame(point_size: float) -> dg.ScatterLiveFrame:
    global live_frame
    if live_frame is None:
        scatter.clear()
        with state_lock:
            capacity = int(state["points"])
        live_frame = scatter.create_live_frame(
            capacity=capacity,
            x="x",
            y="y",
            z="z",
            point_size=point_size,
            colormap="turbo",
        )
    return live_frame


def replace_scatter_frame(
    frame: ScatterFrame | None,
    *,
    point_size: float,
    fit: bool,
    payload: dg.ScatterPayload | None = None,
) -> None:
    with state_lock:
        mode = str(state["update_mode"])
    if mode == "live_frame":
        live = ensure_live_frame(point_size)
        if payload is not None:
            live.replace_prepared(payload, fit=fit)
        elif frame is not None:
            live.replace(frame, point_size=point_size, fit=fit)
    else:
        if frame is None:
            return
        reset_live_frame()
        scatter.set_points(frame, x="x", y="y", z="z", point_size=point_size, fit=fit)


def load_workload(count: int, *, streaming: bool = False) -> None:
    with state_lock:
        state["points"] = count
        state["streaming"] = streaming
        state["stream_phase"] = 0.0
        state["stream_updates"] = 0
        state["stream_last_update"] = 0.0
        state["stream_window"] = []
        state["stream_build_ms"] = 0.0
        state["stream_generate_ms"] = 0.0
        state["stream_pack_ms"] = 0.0
        state["stream_enqueue_ms"] = 0.0
        state["stream_producer_ms"] = 0.0
        state["stream_late_ms"] = 0.0
    set_status(f"Building {count:,} points...")
    frame = ScatterFrame(count)
    replace_scatter_frame(frame, point_size=float(state["point_size"]), fit=True)
    set_status(("Streaming " if streaming else "Static ") + f"{count:,} points")


def set_update_mode(mode: str) -> None:
    if mode not in ("set_points", "live_frame"):
        return
    with state_lock:
        state["update_mode"] = mode
        count = int(state["points"])
        point_size = float(state["point_size"])
        phase = float(state["stream_phase"])
    set_status(f"Switching update path to {mode}...")
    frame = ScatterFrame(count, phase=phase)
    replace_scatter_frame(frame, point_size=point_size, fit=True)
    set_status(f"Update path: {mode}")


def apply_quality(
    name: str,
    *,
    auto_point_size: bool,
    lod_enabled: bool,
    threshold: int,
    factor: int,
    point_size: float,
    render_scale: float,
    auto_quality: bool,
    target_fps: float,
) -> None:
    with state_lock:
        state["auto_point_size"] = auto_point_size
        state["lod_enabled"] = lod_enabled
        state["lod_threshold"] = threshold
        state["lod_factor"] = factor
        state["point_size"] = point_size
        state["interactive_render_scale"] = render_scale
        state["auto_quality"] = auto_quality
        state["quality_target_fps"] = target_fps
    scatter.set_auto_point_size(auto_point_size)
    scatter.set_lod(lod_enabled, threshold=threshold, factor=factor)
    scatter.set_point_size(point_size)
    scatter.set_interactive_render_scale(render_scale)
    scatter.set_auto_quality(auto_quality, target_fps=target_fps)
    point_size_slider.set_value(point_size)
    threshold_slider.set_value(threshold)
    factor_slider.set_value(factor)
    render_scale_slider.set_value(render_scale)
    target_fps_slider.set_value(target_fps)
    set_status(f"Preset: {name}")


def set_point_size(value: float) -> None:
    with state_lock:
        state["point_size"] = float(value)
    scatter.set_point_size(float(value))


def set_lod_threshold(value: float) -> None:
    with state_lock:
        state["lod_threshold"] = int(value)
        enabled = bool(state["lod_enabled"])
        factor = int(state["lod_factor"])
    scatter.set_lod(enabled, threshold=int(value), factor=factor)


def set_lod_factor(value: float) -> None:
    factor = max(1, int(round(value)))
    with state_lock:
        state["lod_factor"] = factor
        enabled = bool(state["lod_enabled"])
        threshold = int(state["lod_threshold"])
    scatter.set_lod(enabled, threshold=threshold, factor=factor)


def set_lod_enabled(enabled: bool) -> None:
    with state_lock:
        state["lod_enabled"] = bool(enabled)
        threshold = int(state["lod_threshold"])
        factor = int(state["lod_factor"])
    scatter.set_lod(bool(enabled), threshold=threshold, factor=factor)


def set_auto_point_size(enabled: bool) -> None:
    with state_lock:
        state["auto_point_size"] = bool(enabled)
    scatter.set_auto_point_size(bool(enabled))


def set_render_scale(value: float) -> None:
    value = max(0.25, min(1.0, float(value)))
    with state_lock:
        state["interactive_render_scale"] = value
    scatter.set_interactive_render_scale(value)


def set_auto_quality(enabled: bool) -> None:
    with state_lock:
        state["auto_quality"] = bool(enabled)
        target_fps = float(state["quality_target_fps"])
    scatter.set_auto_quality(bool(enabled), target_fps=target_fps)


def set_quality_target_fps(value: float) -> None:
    target_fps = max(1.0, float(value))
    with state_lock:
        state["quality_target_fps"] = target_fps
        enabled = bool(state["auto_quality"])
    scatter.set_auto_quality(enabled, target_fps=target_fps)


def set_frame_driver(enabled: bool) -> None:
    with state_lock:
        state["frame_driver"] = bool(enabled)


def set_frame_driver_fps(value: float) -> None:
    with state_lock:
        state["frame_driver_fps"] = max(1.0, float(value))


def set_stream_hz(value: float) -> None:
    with state_lock:
        state["stream_hz_target"] = max(0.5, float(value))


def copy_stats_to_clipboard() -> None:
    with state_lock:
        text = str(state["last_stats_text"])
    if not text.strip():
        set_status("No stats to copy yet")
        return
    try:
        import tkinter as tk

        root = tk.Tk()
        root.withdraw()
        root.clipboard_clear()
        root.clipboard_append(text)
        root.update()
        root.destroy()
        set_status("Copied stats to clipboard")
    except Exception as exc:
        print(text)
        set_status(f"Clipboard failed; printed stats ({exc})")


def scatter_metrics(snapshot: dict[str, object]) -> dict[str, object]:
    gpu = snapshot.get("gpu")
    if not isinstance(gpu, dict):
        return {}
    resources = gpu.get("resources")
    if not isinstance(resources, dict):
        return {}
    scatters = resources.get("scatters")
    if isinstance(scatters, dict):
        selected = scatters.get(scatter.id)
        if isinstance(selected, dict):
            return selected
    selected = resources.get("scatter")
    return selected if isinstance(selected, dict) else {}


def update_stats(snapshot: dict[str, object]) -> None:
    runtime = snapshot.get("runtime")
    frame_avg_ms = float(runtime.get("frame_ms_avg") or runtime.get("frame_ms") or 0.0) if isinstance(runtime, dict) else 0.0
    cpu_frame_ms = float(runtime.get("cpu_frame_ms") or runtime.get("last_frame_ms") or 0.0) if isinstance(runtime, dict) else 0.0
    frame_work_ms = float(runtime.get("frame_work_ms") or 0.0) if isinstance(runtime, dict) else 0.0
    frame_acquire_ms = float(runtime.get("frame_acquire_ms") or 0.0) if isinstance(runtime, dict) else 0.0
    frame_encode_ms = float(runtime.get("frame_encode_ms") or 0.0) if isinstance(runtime, dict) else 0.0
    frame_submit_ms = float(runtime.get("frame_submit_ms") or 0.0) if isinstance(runtime, dict) else 0.0
    frame_present_ms = float(runtime.get("frame_present_ms") or 0.0) if isinstance(runtime, dict) else 0.0
    wall_fps = float(runtime.get("wall_fps") or 0.0) if isinstance(runtime, dict) else 0.0
    frames_rendered = int(runtime.get("frames_rendered") or 0) if isinstance(runtime, dict) else 0
    frame_window_count = int(runtime.get("frame_window_count") or 0) if isinstance(runtime, dict) else 0
    with state_lock:
        stream_updates = int(state["stream_updates"])
        stream_window = list(state["stream_window"])
        stream_enabled = bool(state["streaming"])
        stream_hz_target = float(state["stream_hz_target"])
        stream_build_ms = float(state["stream_build_ms"])
        stream_generate_ms = float(state["stream_generate_ms"])
        stream_pack_ms = float(state["stream_pack_ms"])
        stream_enqueue_ms = float(state["stream_enqueue_ms"])
        stream_producer_ms = float(state["stream_producer_ms"])
        stream_late_ms = float(state["stream_late_ms"])
        update_mode = str(state["update_mode"])
    stream_hz = 0.0
    if len(stream_window) >= 2:
        span = max(0.000_001, stream_window[-1] - stream_window[0])
        stream_hz = (len(stream_window) - 1) / span
    metrics = scatter_metrics(snapshot)
    lod = metrics.get("lod", {})
    lod_active = bool(lod.get("active")) if isinstance(lod, dict) else False
    total = int(metrics.get("last_point_count") or state["points"])
    drawn = int(metrics.get("effective_draw_point_count") or total)
    point_scale = float(metrics.get("point_size_scale") or 1.0)
    render_scale = float(metrics.get("active_render_scale") or 1.0)
    interactive_render_scale = float(metrics.get("interactive_render_scale") or state["interactive_render_scale"])
    auto_quality = bool(metrics.get("auto_quality"))
    quality_level = int(metrics.get("quality_level") or 0)
    target_frame_ms = float(metrics.get("quality_target_frame_ms") or 0.0)
    target_fps = 1000.0 / target_frame_ms if target_frame_ms > 0 else 0.0
    stats_text = "\n".join(
        [
            f"FPS wall: {wall_fps:5.1f}    frame total: {cpu_frame_ms:5.2f} ms",
            f"CPU work: {frame_work_ms:5.2f} ms    avg total: {frame_avg_ms:5.2f} ms",
            f"Acquire: {frame_acquire_ms:5.2f} ms    encode: {frame_encode_ms:5.2f} ms",
            f"Submit: {frame_submit_ms:5.2f} ms    present: {frame_present_ms:5.2f} ms",
            f"Frames: {frames_rendered:,}    rolling window: {frame_window_count}",
            f"Stream: {'on' if stream_enabled else 'off'}    updates: {stream_updates:,}    update Hz: {stream_hz:4.1f} / {stream_hz_target:4.1f}",
            f"Update path: {update_mode}",
            f"Producer build: {stream_build_ms:5.2f} ms    enqueue: {stream_enqueue_ms:5.2f} ms",
            f"Generate: {stream_generate_ms:5.2f} ms    pack: {stream_pack_ms:5.2f} ms",
            f"Producer total: {stream_producer_ms:5.2f} ms    late: {stream_late_ms:5.2f} ms",
            f"Drawn: {drawn:,} / {total:,}    point scale: {point_scale:.2f}",
            f"Render scale: {render_scale:.2f} active / {interactive_render_scale:.2f} interaction",
            f"Auto quality: {'on' if auto_quality else 'off'}    level: {quality_level}    target: {target_fps:4.1f} fps",
            f"Frame driver: {'on' if state['frame_driver'] else 'off'} @ {float(state['frame_driver_fps']):4.0f} Hz",
            f"LOD: {'active' if lod_active else 'idle'}    threshold: {state['lod_threshold']:,}    factor: {state['lod_factor']}",
            f"Native total: {float(metrics.get('last_total_native_ms') or 0.0):5.2f} ms    render encode: {float(metrics.get('last_render_encode_ms') or 0.0):5.2f} ms",
            f"Pack: {float(metrics.get('last_pack_ms') or 0.0):5.2f} ms    upload: {float(metrics.get('last_upload_ms') or 0.0):5.2f} ms    LOD build: {float(metrics.get('last_lod_ms') or 0.0):5.2f} ms",
        ]
    )
    with state_lock:
        state["last_stats_text"] = stats_text
    stats.set_value(stats_text)


def stats_worker() -> None:
    while not stop_event.wait(0.45):
        try:
            snapshot = app.debug_snapshot(timeout_ms=500)
        except Exception:
            continue
        app.call_soon_threadsafe(lambda s=snapshot: update_stats(s))


def streaming_worker() -> None:
    next_update = time.perf_counter()
    while not stop_event.is_set():
        with state_lock:
            enabled = bool(state["streaming"])
            count = int(state["points"])
            state["stream_phase"] = float(state["stream_phase"]) + 0.22
            phase = float(state["stream_phase"])
            point_size = float(state["point_size"])
            target_hz = max(0.5, float(state["stream_hz_target"]))
            mode = str(state["update_mode"])
        if not enabled:
            next_update = time.perf_counter()
            stop_event.wait(0.05)
            continue
        now = time.perf_counter()
        if now < next_update:
            stop_event.wait(min(next_update - now, 0.05))
            continue
        producer_t0 = time.perf_counter()
        late_ms = max(0.0, (producer_t0 - next_update) * 1000.0)
        build_t0 = time.perf_counter()
        generate_t0 = time.perf_counter()
        frame = ScatterFrame(count, phase=phase)
        generate_ms = (time.perf_counter() - generate_t0) * 1000.0
        prepared_payload = None
        pack_ms = 0.0
        if mode == "live_frame":
            pack_t0 = time.perf_counter()
            prepared_payload = dg.Scatter3D.prepare_points(
                frame,
                x="x",
                y="y",
                z="z",
                point_size=point_size,
                colormap="turbo",
            )
            pack_ms = (time.perf_counter() - pack_t0) * 1000.0
        build_ms = (time.perf_counter() - build_t0) * 1000.0
        update_time = time.perf_counter()
        with state_lock:
            state["stream_updates"] = int(state["stream_updates"]) + 1
            state["stream_last_update"] = update_time
            window = list(state["stream_window"])
            window.append(update_time)
            cutoff = update_time - 1.0
            state["stream_window"] = [stamp for stamp in window if stamp >= cutoff]
        enqueue_t0 = time.perf_counter()
        app.call_soon_threadsafe(
            lambda f=frame, p=prepared_payload, ps=point_size: replace_scatter_frame(
                f,
                point_size=ps,
                fit=False,
                payload=p,
            )
        )
        enqueue_ms = (time.perf_counter() - enqueue_t0) * 1000.0
        producer_ms = (time.perf_counter() - producer_t0) * 1000.0
        with state_lock:
            state["stream_build_ms"] = build_ms
            state["stream_generate_ms"] = generate_ms
            state["stream_pack_ms"] = pack_ms
            state["stream_enqueue_ms"] = enqueue_ms
            state["stream_producer_ms"] = producer_ms
            state["stream_late_ms"] = late_ms
        next_update = max(next_update + (1.0 / target_hz), time.perf_counter())


def redraw_worker() -> None:
    next_frame = time.perf_counter()
    while not stop_event.is_set():
        with state_lock:
            enabled = bool(state["frame_driver"])
            fps = max(1.0, float(state["frame_driver_fps"]))
        if not enabled:
            next_frame = time.perf_counter()
            stop_event.wait(0.05)
            continue
        now = time.perf_counter()
        if now >= next_frame:
            try:
                app.request_redraw()
            except RuntimeError:
                pass
            next_frame = now + (1.0 / fps)
        stop_event.wait(min(max(0.0, next_frame - time.perf_counter()), 0.05))


with win:
    with dg.HLayout(class_="root"):
        with dg.Panel("Perf Lab", class_="controls"):
            dg.Label("Scatter A/B Lab", class_="title")
            dg.Label(
                "Use presets and workloads to compare adaptive point size and interaction LOD. Internal optimizations stay out of the public API.",
                class_="subtle",
            )
            status = dg.Label("Ready", class_="subtle")

            dg.Label("Presets", class_="title")
            with dg.HLayout(class_="row"):
                dg.Button(
                    "Desktop",
                    on_click=lambda: apply_quality(
                        "Desktop defaults",
                        auto_point_size=True,
                        lod_enabled=False,
                        threshold=min(200_000, MAX_LOD_THRESHOLD),
                        factor=8,
                        point_size=3.6,
                        render_scale=1.0,
                        auto_quality=False,
                        target_fps=60.0,
                    ),
                )
                dg.Button(
                    "Pi",
                    on_click=lambda: apply_quality(
                        "Pi defaults",
                        auto_point_size=True,
                        lod_enabled=True,
                        threshold=INITIAL_LOD_THRESHOLD,
                        factor=4,
                        point_size=3.0,
                        render_scale=0.65,
                        auto_quality=True,
                        target_fps=10.0,
                    ),
                )
            with dg.HLayout(class_="row"):
                dg.Button(
                    "All off",
                    on_click=lambda: apply_quality(
                        "All off",
                        auto_point_size=False,
                        lod_enabled=False,
                        threshold=MAX_LOD_THRESHOLD,
                        factor=1,
                        point_size=4.0,
                        render_scale=1.0,
                        auto_quality=False,
                        target_fps=60.0,
                    ),
                )
                dg.Button(
                    "Max quality",
                    on_click=lambda: apply_quality(
                        "Max quality",
                        auto_point_size=False,
                        lod_enabled=False,
                        threshold=MAX_LOD_THRESHOLD,
                        factor=1,
                        point_size=5.0,
                        render_scale=1.0,
                        auto_quality=False,
                        target_fps=60.0,
                    ),
                )

            dg.Label("Update path", class_="title")
            with dg.HLayout(class_="row"):
                dg.Button("set_points", on_click=lambda: set_update_mode("set_points"))
                dg.Button("live frame", on_click=lambda: set_update_mode("live_frame"))

            dg.Label("Workloads", class_="title")
            with dg.HLayout(class_="row"):
                for label, count in WORKLOADS:
                    dg.Button(label, on_click=lambda count=count: load_workload(count))
            with dg.HLayout(class_="row"):
                for label, count in WORKLOADS:
                    dg.Button(
                        f"{label} stream",
                        on_click=lambda count=count: load_workload(count, streaming=True),
                    )

            dg.Label("Quality knobs", class_="title")
            dg.Checkbox("Adaptive point size", checked=True, on_change=set_auto_point_size)
            dg.Checkbox("Interaction LOD", checked=False, on_change=set_lod_enabled)
            dg.Checkbox("Auto quality budget", checked=False, on_change=set_auto_quality)
            dg.Label("Point size", class_="subtle")
            point_size_slider = dg.Slider(3.6, min=1.0, max=8.0, step=0.2, on_change=set_point_size)
            dg.Label("LOD threshold", class_="subtle")
            threshold_slider = dg.Slider(
                INITIAL_LOD_THRESHOLD,
                min=10_000,
                max=MAX_LOD_THRESHOLD,
                step=10_000,
                on_change=set_lod_threshold,
            )
            dg.Label("LOD factor", class_="subtle")
            factor_slider = dg.Slider(4, min=1, max=16, step=1, on_change=set_lod_factor)
            dg.Label("Interaction render scale", class_="subtle")
            render_scale_slider = dg.Slider(1.0, min=0.25, max=1.0, step=0.05, on_change=set_render_scale)
            dg.Label("Auto target FPS", class_="subtle")
            target_fps_slider = dg.Slider(10.0, min=5.0, max=60.0, step=1.0, on_change=set_quality_target_fps)
            dg.Label("Frame driver", class_="title")
            dg.Checkbox("Request redraws", checked=True, on_change=set_frame_driver)
            dg.Label("Requested frame rate", class_="subtle")
            frame_driver_fps_slider = dg.Slider(60.0, min=5.0, max=120.0, step=5.0, on_change=set_frame_driver_fps)
            dg.Label("Stream update Hz", class_="subtle")
            stream_hz_slider = dg.Slider(8.0, min=1.0, max=60.0, step=1.0, on_change=set_stream_hz)

            dg.Label("Stats", class_="title")
            dg.Button("Copy stats", on_click=copy_stats_to_clipboard)
            stats = dg.Label("Waiting for first snapshot...", class_="readout")

        with dg.Panel("Scatter viewport", class_="plot"):
            scatter = dg.Scatter3D(
                initial_frame,
                x="x",
                y="y",
                z="z",
                colormap="turbo",
                point_size=3.6,
                auto_point_size=True,
                interactive_render_scale=1.0,
                auto_quality=False,
                quality_target_fps=10.0,
                grid=True,
                major_planes=True,
                scalar_bar=True,
                scalar_bar_title="z",
                orientation_axes=True,
                class_="lab",
                key="scatter-perf-lab",
            )


def main() -> None:
    threading.Thread(target=stats_worker, daemon=True).start()
    threading.Thread(target=streaming_worker, daemon=True).start()
    threading.Thread(target=redraw_worker, daemon=True).start()
    try:
        print(app.run(win))
    finally:
        stop_event.set()


if __name__ == "__main__":
    main()
