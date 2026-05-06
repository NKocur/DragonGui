from __future__ import annotations

import argparse
import dataclasses
import json
import math
import os
import statistics
import sys
import threading
import time
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
PYTHON_DIR = ROOT / "python"
if str(PYTHON_DIR) not in sys.path:
    sys.path.insert(0, str(PYTHON_DIR))

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - benchmark dependency
    raise SystemExit("scatter_stream_compare.py requires NumPy") from exc


def _summary(values: list[float]) -> dict[str, float]:
    if not values:
        return {"avg": 0.0, "p50": 0.0, "p95": 0.0, "max": 0.0}
    ordered = sorted(values)
    p95_index = min(len(ordered) - 1, max(0, math.ceil(len(ordered) * 0.95) - 1))
    return {
        "avg": statistics.fmean(values),
        "p50": statistics.median(values),
        "p95": ordered[p95_index],
        "max": max(values),
    }


def _hz_from_timestamps(timestamps: list[float]) -> float:
    if len(timestamps) < 2:
        return 0.0
    span = timestamps[-1] - timestamps[0]
    if span <= 0:
        return 0.0
    return (len(timestamps) - 1) / span


class FrameFactory:
    def __init__(self, points: int) -> None:
        self.points = max(1, int(points))
        side = max(2, int(math.sqrt(self.points)))
        rows = int(math.ceil(self.points / side))
        u, v = np.meshgrid(
            np.linspace(-1.0, 1.0, side, dtype=np.float32),
            np.linspace(-1.0, 1.0, rows, dtype=np.float32),
        )
        self.u = u.ravel()[: self.points]
        self.v = v.ravel()[: self.points]

    def build(self, phase: float) -> np.ndarray:
        p = np.float32(phase)
        u = self.u
        v = self.v
        spiral = np.sin((u * 2.0 + v * 1.2) * np.float32(math.tau) + p)
        ridge = np.exp(-((u - np.sin(p) * 0.45) ** 2) * np.float32(12.0))
        out = np.empty((self.points, 3), dtype=np.float32)
        out[:, 0] = u * np.float32(18.0) + spiral * np.float32(0.35)
        out[:, 1] = v * np.float32(10.0) + np.cos(u * np.float32(math.tau) + p) * np.float32(0.24)
        out[:, 2] = (
            np.float32(14.0)
            + np.sin(u * np.float32(math.tau * 3.0) + p) * np.float32(1.8)
            + np.cos(v * np.float32(math.tau * 2.0) - p) * np.float32(1.1)
            + ridge * np.float32(5.0)
        )
        return out


class ArrayFrame:
    columns = ("x", "y", "z")
    dtypes = ("float32", "float32", "float32")

    def __init__(self, points: np.ndarray) -> None:
        self.points = points
        self.shape = points.shape

    def __getitem__(self, column: str) -> object:
        if column == "x":
            return self.points[:, 0]
        if column == "y":
            return self.points[:, 1]
        if column == "z":
            return self.points[:, 2]
        raise KeyError(column)


def _scatter_metrics(snapshot: dict[str, Any], scatter_id: str) -> dict[str, Any]:
    gpu = snapshot.get("gpu")
    if not isinstance(gpu, dict):
        return {}
    resources = gpu.get("resources")
    if not isinstance(resources, dict):
        return {}
    scatters = resources.get("scatters")
    if isinstance(scatters, dict):
        selected = scatters.get(scatter_id)
        if isinstance(selected, dict):
            return selected
    selected = resources.get("scatter")
    return selected if isinstance(selected, dict) else {}


def run_dragongui_benchmark(args: argparse.Namespace) -> dict[str, Any]:
    import dragongui as dg

    if not dg.native_backend_available():
        return {
            "backend": "dragongui",
            "status": "skipped",
            "reason": "native DragonGUI backend is not available",
        }

    factory = FrameFactory(args.points)

    def prepare_scatter_payload(frame: ArrayFrame) -> dg.ScatterPayload:
        kwargs: dict[str, Any] = {}
        if args.dragongui_payload_format == "instances":
            kwargs["scalars"] = "z"
        return dg.Scatter3D.prepare_points(
            frame,
            x="x",
            y="y",
            z="z",
            point_size=args.point_size,
            colormap="turbo",
            **kwargs,
        )

    prebuilt_frames: list[ArrayFrame] = []
    prebuilt_payloads: list[dg.ScatterPayload] = []
    if args.dragongui_workload in ("prebuilt-frames", "prebuilt-payloads"):
        for i in range(max(1, int(args.prebuild_count))):
            frame = ArrayFrame(factory.build(i * 0.22))
            prebuilt_frames.append(frame)
            if args.dragongui_workload == "prebuilt-payloads":
                prebuilt_payloads.append(prepare_scatter_payload(frame))
    app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))
    win = dg.Window("DragonGUI Scatter Stream Benchmark", width=args.width, height=args.height)
    with win:
        scatter = dg.Scatter3D(
            None,
            x="x",
            y="y",
            z="z",
            colormap="turbo",
            point_size=args.point_size,
            auto_point_size=args.auto_point_size,
            lod=args.lod,
            lod_threshold=args.lod_threshold,
            lod_factor=args.lod_factor,
            interactive_render_scale=args.render_scale,
            class_="benchmark",
        )

    stop_event = threading.Event()
    producer_build_ms: list[float] = []
    producer_generate_ms: list[float] = []
    producer_pack_ms: list[float] = []
    ui_update_ms: list[float] = []
    completed_update_times: list[float] = []
    producer_late_ms: list[float] = []
    lock = threading.Lock()
    live_frame: dg.ScatterLiveFrame | None = None

    def wait_for_live_app() -> bool:
        deadline = time.perf_counter() + 5.0
        while time.perf_counter() < deadline and not stop_event.is_set():
            if getattr(app, "_handle", None) is not None:
                return True
            time.sleep(0.005)
        return False

    def producer() -> None:
        if not wait_for_live_app():
            return
        interval = 1.0 / max(0.1, float(args.target_hz))
        end_time = time.perf_counter() + float(args.duration)
        next_update = time.perf_counter()
        index = 0
        while not stop_event.is_set() and time.perf_counter() < end_time:
            now = time.perf_counter()
            if now < next_update:
                stop_event.wait(min(next_update - now, 0.01))
                continue
            late_ms = max(0.0, (now - next_update) * 1000.0)
            phase = index * 0.22
            build_t0 = time.perf_counter()
            generate_t0 = time.perf_counter()
            if args.dragongui_workload in ("prebuilt-frames", "prebuilt-payloads"):
                frame = prebuilt_frames[index % len(prebuilt_frames)]
            else:
                frame = ArrayFrame(factory.build(phase))
            generate_ms = (time.perf_counter() - generate_t0) * 1000.0
            prepared_payload = None
            pack_ms = 0.0
            if args.dragongui_update_mode == "live-frame":
                if args.dragongui_workload == "prebuilt-payloads":
                    prepared_payload = dataclasses.replace(
                        prebuilt_payloads[index % len(prebuilt_payloads)],
                        pack_ms=0.0,
                    )
                else:
                    pack_t0 = time.perf_counter()
                    prepared_payload = prepare_scatter_payload(frame)
                    pack_ms = (time.perf_counter() - pack_t0) * 1000.0
            build_ms = (time.perf_counter() - build_t0) * 1000.0

            def apply_frame(
                f: ArrayFrame = frame,
                payload: dg.ScatterPayload | None = prepared_payload,
                build: float = build_ms,
                generate: float = generate_ms,
                pack: float = pack_ms,
                late: float = late_ms,
            ) -> None:
                nonlocal live_frame
                update_t0 = time.perf_counter()
                if args.dragongui_update_mode == "live-frame":
                    if live_frame is None:
                        scatter.clear()
                        live_frame = scatter.create_live_frame(
                            capacity=args.points,
                            x="x",
                            y="y",
                            z="z",
                            point_size=args.point_size,
                            colormap="turbo",
                        )
                    assert payload is not None
                    live_frame.replace_prepared(
                        payload,
                        fit=live_frame.replaces == 0,
                        update_metadata=False,
                    )
                else:
                    if live_frame is not None:
                        live_frame.remove()
                        live_frame = None
                    scatter.set_points(f, x="x", y="y", z="z", point_size=args.point_size, fit=False)
                update_ms = (time.perf_counter() - update_t0) * 1000.0
                with lock:
                    producer_build_ms.append(build)
                    producer_generate_ms.append(generate)
                    producer_pack_ms.append(pack)
                    ui_update_ms.append(update_ms)
                    producer_late_ms.append(late)
                    completed_update_times.append(time.perf_counter())

            try:
                app.call_soon_threadsafe(apply_frame)
            except RuntimeError:
                break
            index += 1
            next_update = max(next_update + interval, time.perf_counter())

    def frame_driver() -> None:
        if not wait_for_live_app():
            return
        interval = 1.0 / max(1.0, float(args.frame_hz))
        next_frame = time.perf_counter()
        while not stop_event.is_set():
            now = time.perf_counter()
            if now >= next_frame:
                try:
                    app.request_redraw()
                except RuntimeError:
                    break
                next_frame = now + interval
            stop_event.wait(min(max(0.0, next_frame - time.perf_counter()), 0.01))

    old_smoke = os.environ.get("DRAGONGUI_SMOKE_FRAMES")
    smoke_frames = max(30, int(min(float(args.frame_hz), 60.0) * (float(args.duration) + 1.0)))
    os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(smoke_frames)
    start = time.perf_counter()
    threading.Thread(target=producer, daemon=True).start()
    threading.Thread(target=frame_driver, daemon=True).start()
    try:
        result = app.run(win)
    finally:
        stop_event.set()
        if old_smoke is None:
            os.environ.pop("DRAGONGUI_SMOKE_FRAMES", None)
        else:
            os.environ["DRAGONGUI_SMOKE_FRAMES"] = old_smoke
    wall_s = time.perf_counter() - start

    snapshot = result.get("debug_snapshot", {})
    metrics = _scatter_metrics(snapshot if isinstance(snapshot, dict) else {}, scatter.id)
    runtime = (snapshot.get("runtime") if isinstance(snapshot, dict) else {}) or {}
    with lock:
        build_values = list(producer_build_ms)
        generate_values = list(producer_generate_ms)
        pack_values = list(producer_pack_ms)
        update_values = list(ui_update_ms)
        update_times = list(completed_update_times)
        late_values = list(producer_late_ms)

    native_updates = int(metrics.get("updates") or 0)
    accepted_update_hz = _hz_from_timestamps(update_times)
    native_update_hz = native_updates / max(0.001, wall_s)
    coalesced_updates = max(0, len(update_times) - native_updates)

    return {
        "backend": "dragongui",
        "status": "ok",
        "update_mode": args.dragongui_update_mode,
        "workload": args.dragongui_workload,
        "payload_format": args.dragongui_payload_format,
        "prebuild_count": int(args.prebuild_count),
        "points": int(args.points),
        "duration_s": float(args.duration),
        "target_update_hz": float(args.target_hz),
        "completed_updates": len(update_times),
        "achieved_update_hz": accepted_update_hz,
        "accepted_update_hz": accepted_update_hz,
        "native_update_hz": native_update_hz,
        "coalesced_updates": coalesced_updates,
        "wall_s": wall_s,
        "producer_build_ms": _summary(build_values),
        "producer_generate_ms": _summary(generate_values),
        "producer_pack_ms": _summary(pack_values),
        "ui_update_ms": _summary(update_values),
        "producer_late_ms": _summary(late_values),
        "present_fps": float(runtime.get("wall_fps") or 0.0) if isinstance(runtime, dict) else 0.0,
        "frame_work_ms": float(runtime.get("frame_work_ms") or 0.0) if isinstance(runtime, dict) else 0.0,
        "frame_total_ms": float(runtime.get("last_frame_ms") or 0.0) if isinstance(runtime, dict) else 0.0,
        "command_queue_depth": int(runtime.get("command_queue_depth") or 0)
        if isinstance(runtime, dict)
        else 0,
        "native": {
            "updates": native_updates,
            "pack_ms": float(metrics.get("last_pack_ms") or 0.0),
            "decode_ms": float(metrics.get("last_decode_ms") or 0.0),
            "bounds_ms": float(metrics.get("last_bounds_ms") or 0.0),
            "upload_ms": float(metrics.get("last_upload_ms") or 0.0),
            "grid_ms": float(metrics.get("last_grid_ms") or 0.0),
            "overlay_ms": float(metrics.get("last_overlay_ms") or 0.0),
            "total_ms": float(metrics.get("last_total_native_ms") or 0.0),
            "render_encode_ms": float(metrics.get("last_render_encode_ms") or 0.0),
            "effective_drawn": int(metrics.get("effective_draw_point_count") or 0),
            "point_scale": float(metrics.get("point_size_scale") or 0.0),
        },
    }


def run_vispy_benchmark(args: argparse.Namespace) -> dict[str, Any]:
    try:
        from vispy import app as vispy_app
        from vispy import scene
    except Exception as exc:
        return {"backend": "vispy", "status": "skipped", "reason": repr(exc)}

    factory = FrameFactory(args.points)
    producer_build_ms: list[float] = []
    ui_update_ms: list[float] = []
    completed_update_times: list[float] = []
    draw_times: list[float] = []

    canvas = scene.SceneCanvas(
        keys=None,
        size=(int(args.width), int(args.height)),
        bgcolor=(0.02, 0.03, 0.06, 1.0),
        show=True,
        title="VisPy Scatter Stream Benchmark",
    )
    view = canvas.central_widget.add_view()
    view.camera = scene.TurntableCamera(fov=45.0, distance=34.0, elevation=24.0, azimuth=35.0)
    first_points = factory.build(0.0)
    markers = scene.visuals.Markers(parent=view.scene)
    markers.set_data(
        first_points,
        face_color=(0.25, 0.64, 1.0, 0.82),
        edge_color=None,
        size=float(args.point_size),
    )

    start_time = time.perf_counter()
    next_update = start_time
    interval = 1.0 / max(0.1, float(args.target_hz))
    index = 0

    @canvas.events.draw.connect
    def _on_draw(_event: object) -> None:
        draw_times.append(time.perf_counter())

    def on_timer(_event: object) -> None:
        nonlocal next_update, index
        now = time.perf_counter()
        if now - start_time >= float(args.duration):
            timer.stop()
            canvas.close()
            vispy_app.quit()
            return
        if now < next_update:
            return
        build_t0 = time.perf_counter()
        points = factory.build(index * 0.22)
        build_ms = (time.perf_counter() - build_t0) * 1000.0
        update_t0 = time.perf_counter()
        markers.set_data(
            points,
            face_color=(0.25, 0.64, 1.0, 0.82),
            edge_color=None,
            size=float(args.point_size),
        )
        canvas.update()
        update_ms = (time.perf_counter() - update_t0) * 1000.0
        producer_build_ms.append(build_ms)
        ui_update_ms.append(update_ms)
        completed_update_times.append(time.perf_counter())
        index += 1
        next_update = max(next_update + interval, time.perf_counter())

    timer = vispy_app.Timer(interval=0.001, connect=on_timer, start=True)
    try:
        vispy_app.run()
    except Exception as exc:
        return {"backend": "vispy", "status": "error", "reason": repr(exc)}

    return {
        "backend": "vispy",
        "status": "ok",
        "points": int(args.points),
        "duration_s": float(args.duration),
        "target_update_hz": float(args.target_hz),
        "completed_updates": len(completed_update_times),
        "achieved_update_hz": _hz_from_timestamps(completed_update_times),
        "wall_s": time.perf_counter() - start_time,
        "producer_build_ms": _summary(producer_build_ms),
        "ui_update_ms": _summary(ui_update_ms),
        "producer_late_ms": _summary([]),
        "present_fps": _hz_from_timestamps(draw_times),
        "draw_events": len(draw_times),
    }


def print_result(result: dict[str, Any]) -> None:
    backend = result.get("backend", "unknown")
    status = result.get("status", "unknown")
    print(f"\n[{backend}] {status}")
    if status != "ok":
        print(f"  reason: {result.get('reason')}")
        return
    print(f"  points              : {result['points']:,}")
    print(f"  target_update_hz    : {result['target_update_hz']:.1f}")
    if result["backend"] == "dragongui":
        print(f"  accepted_update_hz  : {result.get('accepted_update_hz', result['achieved_update_hz']):.1f}")
        print(f"  native_update_hz    : {result.get('native_update_hz', 0.0):.1f}")
    else:
        print(f"  achieved_update_hz  : {result['achieved_update_hz']:.1f}")
    print(f"  completed_updates   : {result['completed_updates']}")
    print(f"  present_fps         : {result.get('present_fps', 0.0):.1f}")
    print(f"  producer_build avg  : {result['producer_build_ms']['avg']:.2f} ms")
    print(f"  producer_build p95  : {result['producer_build_ms']['p95']:.2f} ms")
    if result["backend"] == "dragongui":
        print(f"  producer_gen avg    : {result['producer_generate_ms']['avg']:.2f} ms")
        print(f"  producer_pack avg   : {result['producer_pack_ms']['avg']:.2f} ms")
    print(f"  ui_update avg       : {result['ui_update_ms']['avg']:.2f} ms")
    print(f"  ui_update p95       : {result['ui_update_ms']['p95']:.2f} ms")
    if result["backend"] == "dragongui":
        native = result.get("native", {})
        print(f"  update_mode         : {result.get('update_mode', 'set-points')}")
        print(f"  workload            : {result.get('workload', 'generate')}")
        print(f"  payload_format      : {result.get('payload_format', 'xyz')}")
        print(f"  frame_work_ms       : {result.get('frame_work_ms', 0.0):.2f}")
        print(f"  command_queue_depth : {result.get('command_queue_depth', 0)}")
        print(f"  native_updates      : {native.get('updates', 0)}")
        print(f"  coalesced_updates   : {result.get('coalesced_updates', 0)}")
        print(f"  native pack/upload  : {native.get('pack_ms', 0.0):.2f} / {native.get('upload_ms', 0.0):.2f} ms")
        print(f"  native decode       : {native.get('decode_ms', 0.0):.2f} ms")
        print(f"  native bounds       : {native.get('bounds_ms', 0.0):.2f} ms")
        print(f"  native grid/overlay : {native.get('grid_ms', 0.0):.2f} / {native.get('overlay_ms', 0.0):.2f} ms")
        print(f"  native total        : {native.get('total_ms', 0.0):.2f} ms")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Compare DragonGUI and VisPy scatter streaming.")
    parser.add_argument("--backend", choices=("both", "dragongui", "vispy"), default="both")
    parser.add_argument("--points", type=int, default=125_000)
    parser.add_argument("--duration", type=float, default=3.0)
    parser.add_argument("--target-hz", type=float, default=60.0)
    parser.add_argument("--frame-hz", type=float, default=120.0)
    parser.add_argument("--width", type=int, default=1100)
    parser.add_argument("--height", type=int, default=760)
    parser.add_argument("--point-size", type=float, default=3.0)
    parser.add_argument("--auto-point-size", action=argparse.BooleanOptionalAction, default=True)
    parser.add_argument("--lod", action=argparse.BooleanOptionalAction, default=False)
    parser.add_argument("--lod-threshold", type=int, default=50_000)
    parser.add_argument("--lod-factor", type=int, default=4)
    parser.add_argument("--render-scale", type=float, default=1.0)
    parser.add_argument(
        "--dragongui-update-mode",
        choices=("set-points", "live-frame"),
        default="set-points",
        help="DragonGUI update path: replace primary scene or retained live actor.",
    )
    parser.add_argument(
        "--dragongui-workload",
        choices=("generate", "prebuilt-frames", "prebuilt-payloads"),
        default="generate",
        help=(
            "DragonGUI producer workload: generate every frame, reuse generated "
            "frames, or reuse already-packed payloads."
        ),
    )
    parser.add_argument(
        "--prebuild-count",
        type=int,
        default=8,
        help="Number of frames/payloads to prebuild for non-generate DragonGUI workloads.",
    )
    parser.add_argument(
        "--dragongui-payload-format",
        choices=("xyz", "instances"),
        default="xyz",
        help=(
            "Payload prepared for DragonGUI live-frame updates. 'xyz' is compact "
            "and native-colormapped; 'instances' is GPU-shaped point_instance_v1 "
            "and can use the native direct-upload fast path."
        ),
    )
    parser.add_argument("--json-out", type=Path, default=None)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    results: list[dict[str, Any]] = []
    if args.backend in {"both", "dragongui"}:
        results.append(run_dragongui_benchmark(args))
    if args.backend in {"both", "vispy"}:
        results.append(run_vispy_benchmark(args))

    for result in results:
        print_result(result)

    payload = {"results": results}
    if args.json_out is not None:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        print(f"\nwrote {args.json_out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
