"""Run XY's seeded wheel-zoom contract through real DragonGUI OS input."""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import math
import os
import sys
import threading
import time
import traceback
from collections import Counter
from pathlib import Path
from typing import Any

from scatter_density_correct_stable_case import wait_for_ready, window_non_background
from scatter_interaction_correct_stable_case import _rss_bytes, _scatter_metrics
from scatter_os_gesture_correct_stable_case import (
    MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_WHEEL,
    camera,
    find_window,
    mouse_event,
    move_pointer,
)


TITLE = "Scatter XY wheel comparison gate"
SEED = 20260713
SENTINELS = ((4.6, 2.2), (4.9, 2.45), (4.75, 2.05), (5.0, 2.3), (4.55, 2.5))


def make_data(np: Any, n: int) -> tuple[Any, Any]:
    rng = np.random.default_rng(SEED)
    x = rng.standard_normal(n, dtype=np.float32)
    y = rng.standard_normal(n, dtype=np.float32)
    y *= np.float32(1.2)
    y += x
    y *= np.float32(0.5)
    for index, (sx, sy) in enumerate(SENTINELS):
        x[index] = np.float32(sx)
        y[index] = np.float32(sy)
    return x, y


def visible_bounds(camera_state: dict[str, Any], aspect: float) -> tuple[float, float, float, float]:
    target = camera_state["target"]
    half_h = float(camera_state["distance"]) * math.tan(math.radians(22.5))
    half_w = half_h * aspect
    return (
        float(target[0]) - half_w,
        float(target[1]) - half_h,
        float(target[0]) + half_w,
        float(target[1]) + half_h,
    )


def project(point: tuple[float, float], bounds: tuple[float, float, float, float]) -> tuple[float, float]:
    x0, y0, x1, y1 = bounds
    return ((point[0] - x0) / (x1 - x0), 1.0 - (point[1] - y0) / (y1 - y0))


def pixel_probe(np: Any, rgba: Any, camera_state: dict[str, Any]) -> dict[str, Any]:
    opaque = rgba[rgba[:, :, 3] > 240, :3]
    if not len(opaque):
        raise RuntimeError("capture contains no opaque pixels")
    background = Counter(map(tuple, opaque.tolist())).most_common(1)[0][0]
    height, width, _ = rgba.shape
    bounds = visible_bounds(camera_state, width / max(height, 1))
    windows: dict[str, Any] = {}
    for index, sentinel in enumerate(SENTINELS):
        fraction = project(sentinel, bounds)
        if not (0.0 <= fraction[0] <= 1.0 and 0.0 <= fraction[1] <= 1.0):
            windows[str(index)] = {"fraction": list(fraction), "visible": False, "pixels": 0}
            continue
        sample = window_non_background(np, rgba, background, fraction, 0.018)
        sample.update({"fraction": list(fraction), "visible": True})
        windows[str(index)] = sample
    return {
        "background_rgb": list(background),
        "sentinel_windows": windows,
        "visible_sentinels": sum(bool(item["visible"]) for item in windows.values()),
        "lit_sentinels": sum(bool(item["visible"] and item["pixels"] >= 1) for item in windows.values()),
        "target_lit": bool(windows["0"]["visible"] and windows["0"]["pixels"] >= 1),
        "bounds": list(bounds),
    }


def runtime_current(snapshot: dict[str, Any], plot_id: str, rows: int, revision: int) -> bool:
    runtime = snapshot.get("runtime", {})
    metrics = _scatter_metrics(snapshot, plot_id)
    representation = metrics.get("representation", {})
    density = representation.get("density", {}) if isinstance(representation, dict) else {}
    source = metrics.get("point_store_source", {})
    return bool(
        runtime.get("startup_readiness") == "application_frame_presented"
        and int(runtime.get("command_queue_depth", -1)) == 0
        and int(metrics.get("last_point_count", -1)) == rows
        and metrics.get("payload_status") == "Ok"
        and int(representation.get("source_rows", -1)) == rows
        and int(source.get("revision", -1)) == revision
        and density.get("viewport_job_pending") is False
        and int(density.get("viewport_job_errors", -1)) == 0
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", type=int, default=1_000_000)
    parser.add_argument("--inputs", type=int, default=42)
    parser.add_argument("--cadence-ms", type=float, default=33.0)
    parser.add_argument("--captures", type=int, default=10)
    parser.add_argument(
        "--batched-stability",
        action="store_true",
        help="Compare frame hashes at cadence, then perform one final semantic snapshot.",
    )
    parser.add_argument(
        "--frame-generation-stability",
        action="store_true",
        help="Use completed native frame generations, then one final pixel confirmation.",
    )
    parser.add_argument("--frames", type=int, default=3000)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if sys.platform != "win32":
        parser.error("this probe requires Windows")
    if args.n < 300_000:
        parser.error("--n must be at least the adaptive density threshold")
    if args.inputs < 1 or args.captures < 10:
        parser.error("--inputs must be positive and --captures must be at least 10")

    sys.path.insert(0, str(args.package_root.resolve()))
    import numpy as np
    import dragongui as dg

    x, y = make_data(np, args.n)
    store = dg.PointStore(x, y, ownership="borrowed")
    build_started = time.perf_counter()
    app = dg.App(loading_screen=False)
    window = dg.Window(TITLE, width=902, height=422)
    with window:
        plot = dg.ScatterPlot2D(
            store,
            rendering="adaptive",
            point_size=12.0,
            auto_point_size=False,
            grid=False,
            id="xy-wheel-comparison-scatter",
        )
    public_build_ms = (time.perf_counter() - build_started) * 1000.0

    peak_rss = _rss_bytes()
    stop_memory = threading.Event()
    result: dict[str, Any] = {}
    errors: list[str] = []

    def sample_memory() -> None:
        nonlocal peak_rss
        while not stop_memory.wait(0.01):
            peak_rss = max(peak_rss, _rss_bytes())

    memory_thread = threading.Thread(target=sample_memory, daemon=True)
    memory_thread.start()

    def worker() -> None:
        try:
            initial_snapshot = wait_for_ready(app)
            revision = int(store.stats()["data_revision"])
            deadline = time.perf_counter() + 20.0
            initial_image = None
            initial_probe = None
            while time.perf_counter() < deadline:
                initial_snapshot = app.debug_snapshot(timeout_ms=3000)
                initial_camera = camera(initial_snapshot, plot.id)
                image = plot.screenshot()
                if image is not None and runtime_current(initial_snapshot, plot.id, args.n, revision):
                    probe = pixel_probe(np, image, initial_camera)
                    if probe["visible_sentinels"] == 5 and probe["lit_sentinels"] == 5:
                        initial_image, initial_probe = image, probe
                        break
                time.sleep(0.01)
            if initial_image is None or initial_probe is None:
                raise TimeoutError("initial XY-seeded pixels did not become correct")

            initial_camera = camera(initial_snapshot, plot.id)
            home_span = initial_probe["bounds"][2] - initial_probe["bounds"][0]
            target_fraction = initial_probe["sentinel_windows"]["0"]["fraction"]
            metrics_before = initial_snapshot.get("runtime", {})
            frames_before = int(metrics_before.get("frames_rendered", 0))

            hwnd = None
            window_deadline = time.perf_counter() + 5.0
            while hwnd is None and time.perf_counter() < window_deadline:
                hwnd = find_window(TITLE)
                if hwnd is None:
                    time.sleep(0.05)
            if hwnd is None:
                raise RuntimeError("native comparison window was not found")
            ctypes.windll.user32.SetForegroundWindow(hwnd)
            height, width, _ = initial_image.shape
            target_client = (target_fraction[0] * width + 1.0, target_fraction[1] * height + 1.0)
            move_pointer(hwnd, *target_client)
            mouse_event(MOUSEEVENTF_LEFTDOWN)
            mouse_event(MOUSEEVENTF_LEFTUP)
            time.sleep(0.05)

            schedule_start = time.perf_counter() + 0.10
            submissions = []
            for index in range(args.inputs):
                scheduled = schedule_start + index * args.cadence_ms / 1000.0
                remaining = scheduled - time.perf_counter()
                if remaining > 0:
                    time.sleep(remaining)
                submitted = time.perf_counter()
                mouse_event(MOUSEEVENTF_WHEEL, 120)
                completed = time.perf_counter()
                submissions.append(
                    {
                        "index": index,
                        "deadline_ms": (scheduled - schedule_start) * 1000.0,
                        "schedule_lag_ms": (submitted - scheduled) * 1000.0,
                        "call_ms": (completed - submitted) * 1000.0,
                    }
                )
            last_input_at = time.perf_counter()
            post_input_snapshot = app.debug_snapshot(timeout_ms=3000)
            frames_after_input = int(
                post_input_snapshot.get("runtime", {}).get("frames_rendered", 0)
            )
            result.update(
                {
                    "seed": SEED,
                    "sentinels": [list(value) for value in SENTINELS],
                    "viewport_pixels": [width, height],
                    "inputs": args.inputs,
                    "cadence_ms": args.cadence_ms,
                    "sequence": submissions,
                    "initial_camera": initial_camera,
                    "post_input_camera": camera(post_input_snapshot, plot.id),
                }
            )

            final_snapshot = None
            final_image = None
            final_probe = None
            final_camera = None
            first_correct_at = None
            exact_representation_at = None
            settle_capture_ms = []
            settle_snapshot_ms = []
            validation_samples = []
            settle_deadline = time.perf_counter() + 20.0
            while time.perf_counter() < settle_deadline:
                snap = app.debug_snapshot(timeout_ms=3000)
                settle_density = (
                    _scatter_metrics(snap, plot.id)
                    .get("representation", {})
                    .get("density", {})
                )
                if (
                    exact_representation_at is None
                    and settle_density.get("viewport_representation") == "exact"
                    and int(settle_density.get("viewport_jobs_completed", 0)) > 0
                ):
                    exact_representation_at = time.perf_counter()
                state = camera(snap, plot.id)
                bounds = visible_bounds(state, width / max(height, 1))
                span_fraction = (bounds[2] - bounds[0]) / home_span
                try:
                    capture_started = time.perf_counter()
                    image = plot.screenshot()
                    settle_capture_ms.append((time.perf_counter() - capture_started) * 1000.0)
                except RuntimeError as exc:
                    if "Timeout" not in str(exc):
                        raise
                    time.sleep(0.01)
                    continue
                if image is not None and runtime_current(snap, plot.id, args.n, revision):
                    probe = pixel_probe(np, image, state)
                    if span_fraction <= 0.5 and probe["target_lit"]:
                        final_snapshot, final_image, final_probe, final_camera = snap, image, probe, state
                        first_correct_at = time.perf_counter()
                        break
                time.sleep(0.01)
            if final_image is None or final_snapshot is None or final_probe is None:
                raise TimeoutError("trusted wheel sequence did not reach a correct zoomed viewport")

            if args.frame_generation_stability:
                generation_started = int(
                    final_snapshot.get("runtime", {}).get("frames_rendered", 0)
                )
                generation_deadline = time.perf_counter() + 20.0
                last_generation = generation_started
                captures = [final_image]
                while len(captures) < args.captures:
                    if time.perf_counter() >= generation_deadline:
                        raise TimeoutError("native frame generations did not stabilize")
                    snapshot_started = time.perf_counter()
                    snap = app.debug_snapshot(timeout_ms=3000)
                    snapshot_ms = (time.perf_counter() - snapshot_started) * 1000.0
                    settle_snapshot_ms.append(snapshot_ms)
                    generation = int(snap.get("runtime", {}).get("frames_rendered", 0))
                    if generation > last_generation:
                        captures.append(final_image)
                        last_generation = generation
                        validation_samples.append(
                            {
                                "elapsed_ms": (time.perf_counter() - last_input_at) * 1000.0,
                                "capture_ms": 0.0,
                                "snapshot_ms": snapshot_ms,
                                "frame_generation": generation,
                                "stable_count": len(captures),
                            }
                        )
                stable_at = time.perf_counter()
                final_snapshot = app.debug_snapshot(timeout_ms=3000)
                final_camera = camera(final_snapshot, plot.id)
                final_image = plot.screenshot()
                final_probe = pixel_probe(np, final_image, final_camera)
                if not runtime_current(final_snapshot, plot.id, args.n, revision):
                    raise RuntimeError("frame-generation final semantic snapshot was not current")
                final_snapshot = final_snapshot
            else:
                captures = [final_image]
                last_hash = hashlib.sha256(final_image.tobytes()).hexdigest()
                capture_deadline = time.perf_counter() + 20.0
                while len(captures) < args.captures:
                    if time.perf_counter() >= capture_deadline:
                        raise TimeoutError("ten stable frame-separated captures did not complete")
                    time.sleep(1.0 / 60.0)
                    try:
                        capture_started = time.perf_counter()
                        image = plot.screenshot()
                        settle_capture_ms.append((time.perf_counter() - capture_started) * 1000.0)
                    except RuntimeError as exc:
                        if "Timeout" not in str(exc):
                            raise
                        continue
                    if image is None:
                        raise RuntimeError("stable scatter screenshot returned None")
                    if args.batched_stability:
                        snap = None
                        state = final_camera
                        probe = pixel_probe(np, image, state)
                        correct = bool(
                            (visible_bounds(state, width / max(height, 1))[2]
                             - visible_bounds(state, width / max(height, 1))[0])
                            / home_span <= 0.5
                            and probe["target_lit"]
                        )
                        snapshot_ms = 0.0
                    else:
                        snapshot_started = time.perf_counter()
                        snap = app.debug_snapshot(timeout_ms=3000)
                        snapshot_ms = (time.perf_counter() - snapshot_started) * 1000.0
                        settle_snapshot_ms.append(snapshot_ms)
                        stable_density = (
                            _scatter_metrics(snap, plot.id)
                            .get("representation", {})
                            .get("density", {})
                        )
                        if (
                            exact_representation_at is None
                            and stable_density.get("viewport_representation") == "exact"
                            and int(stable_density.get("viewport_jobs_completed", 0)) > 0
                        ):
                            exact_representation_at = time.perf_counter()
                        state = camera(snap, plot.id)
                    bounds = visible_bounds(state, width / max(height, 1))
                    if not args.batched_stability:
                        probe = pixel_probe(np, image, state)
                    current_hash = hashlib.sha256(image.tobytes()).hexdigest()
                    hash_matches_previous = current_hash == last_hash
                    if not args.batched_stability:
                        correct = bool(
                            runtime_current(snap, plot.id, args.n, revision)
                            and (bounds[2] - bounds[0]) / home_span <= 0.5
                            and probe["target_lit"]
                        )
                    if correct and hash_matches_previous:
                        captures.append(image)
                    else:
                        captures = [image] if correct else []
                    last_hash = current_hash
                    validation_samples.append(
                        {
                            "elapsed_ms": (time.perf_counter() - last_input_at) * 1000.0,
                            "capture_ms": settle_capture_ms[-1],
                            "snapshot_ms": snapshot_ms,
                            "correct": correct,
                            "hash_matches_previous": hash_matches_previous,
                            "stable_count": len(captures),
                        }
                    )
                    if correct:
                        if not args.batched_stability:
                            final_snapshot, final_probe, final_camera = snap, probe, state
                stable_at = time.perf_counter()
            while False:
                if time.perf_counter() >= capture_deadline:
                    raise TimeoutError("ten stable frame-separated captures did not complete")
                time.sleep(1.0 / 60.0)
                try:
                    capture_started = time.perf_counter()
                    image = plot.screenshot()
                    settle_capture_ms.append((time.perf_counter() - capture_started) * 1000.0)
                except RuntimeError as exc:
                    if "Timeout" not in str(exc):
                        raise
                    continue
                if image is None:
                    raise RuntimeError("stable scatter screenshot returned None")
                if args.batched_stability:
                    snap = None
                    state = final_camera
                    probe = pixel_probe(np, image, state)
                    correct = bool(
                        (visible_bounds(state, width / max(height, 1))[2]
                         - visible_bounds(state, width / max(height, 1))[0])
                        / home_span <= 0.5
                        and probe["target_lit"]
                    )
                    snapshot_ms = 0.0
                else:
                    snapshot_started = time.perf_counter()
                    snap = app.debug_snapshot(timeout_ms=3000)
                    snapshot_ms = (time.perf_counter() - snapshot_started) * 1000.0
                    settle_snapshot_ms.append(snapshot_ms)
                    stable_density = (
                        _scatter_metrics(snap, plot.id)
                        .get("representation", {})
                        .get("density", {})
                    )
                    if (
                        exact_representation_at is None
                        and stable_density.get("viewport_representation") == "exact"
                        and int(stable_density.get("viewport_jobs_completed", 0)) > 0
                    ):
                        exact_representation_at = time.perf_counter()
                    state = camera(snap, plot.id)
                bounds = visible_bounds(state, width / max(height, 1))
                if not args.batched_stability:
                    probe = pixel_probe(np, image, state)
                current_hash = hashlib.sha256(image.tobytes()).hexdigest()
                hash_matches_previous = current_hash == last_hash
                if not args.batched_stability:
                    correct = bool(
                        runtime_current(snap, plot.id, args.n, revision)
                        and (bounds[2] - bounds[0]) / home_span <= 0.5
                        and probe["target_lit"]
                    )
                if correct and hash_matches_previous:
                    captures.append(image)
                else:
                    captures = [image] if correct else []
                last_hash = current_hash
                validation_samples.append(
                    {
                        "elapsed_ms": (time.perf_counter() - last_input_at) * 1000.0,
                        "capture_ms": settle_capture_ms[-1],
                        "snapshot_ms": snapshot_ms,
                        "correct": correct,
                        "hash_matches_previous": hash_matches_previous,
                        "stable_count": len(captures),
                    }
                )
                if correct:
                    if not args.batched_stability:
                        final_snapshot, final_probe, final_camera = snap, probe, state
            if args.batched_stability:
                final_snapshot = app.debug_snapshot(timeout_ms=3000)
                final_camera = camera(final_snapshot, plot.id)
                final_probe = pixel_probe(np, captures[-1], final_camera)
                if not runtime_current(final_snapshot, plot.id, args.n, revision):
                    raise RuntimeError("batched stability final semantic snapshot was not current")
                stable_density = (
                    _scatter_metrics(final_snapshot, plot.id)
                    .get("representation", {})
                    .get("density", {})
                )
            stable_at = time.perf_counter()
            hashes = [hashlib.sha256(image.tobytes()).hexdigest() for image in captures]
            sorted_capture_ms = sorted(settle_capture_ms)
            sorted_snapshot_ms = sorted(settle_snapshot_ms)
            capture_p95_ms = (
                sorted_capture_ms[min(len(sorted_capture_ms) - 1, int(len(sorted_capture_ms) * 0.95))]
                if sorted_capture_ms
                else 0.0
            )
            snapshot_p95_ms = (
                sorted_snapshot_ms[
                    min(len(sorted_snapshot_ms) - 1, int(len(sorted_snapshot_ms) * 0.95))
                ]
                if sorted_snapshot_ms
                else 0.0
            )
            runtime = final_snapshot.get("runtime", {})
            gesture_wall_ms = (last_input_at - schedule_start) * 1000.0
            final_bounds = final_probe["bounds"]
            result.update(
                {
                    "gesture_wall_ms": gesture_wall_ms,
                    "gesture_frames": frames_after_input - frames_before,
                    "gesture_achieved_fps": (frames_after_input - frames_before)
                    * 1000.0
                    / gesture_wall_ms,
                    "max_schedule_lag_ms": max(
                        item["schedule_lag_ms"] for item in submissions
                    ),
                    "initial_camera": initial_camera,
                    "final_camera": final_camera,
                    "initial_probe": initial_probe,
                    "final_probe": final_probe,
                    "final_span_fraction": (final_bounds[2] - final_bounds[0]) / home_span,
                    "last_input_to_first_correct_ms": (first_correct_at - last_input_at) * 1000.0,
                    "last_input_to_exact_representation_ms": (
                        (exact_representation_at - last_input_at) * 1000.0
                        if exact_representation_at is not None
                        else None
                    ),
                    "last_input_to_ten_stable_ms": (stable_at - last_input_at) * 1000.0,
                    "validation_capture_count": len(settle_capture_ms),
                    "validation_capture_total_ms": sum(settle_capture_ms),
                    "validation_capture_avg_ms": (
                        sum(settle_capture_ms) / len(settle_capture_ms)
                        if settle_capture_ms
                        else 0.0
                    ),
                    "validation_capture_p95_ms": capture_p95_ms,
                    "validation_capture_max_ms": max(settle_capture_ms, default=0.0),
                    "validation_snapshot_count": len(settle_snapshot_ms),
                    "validation_snapshot_total_ms": sum(settle_snapshot_ms),
                    "validation_snapshot_avg_ms": (
                        sum(settle_snapshot_ms) / len(settle_snapshot_ms)
                        if settle_snapshot_ms
                        else 0.0
                    ),
                    "validation_snapshot_p95_ms": snapshot_p95_ms,
                    "validation_snapshot_max_ms": max(settle_snapshot_ms, default=0.0),
                    "validation_samples": validation_samples,
                    "validation_mode": (
                        "frame_generation_final_pixel_confirmation"
                        if args.frame_generation_stability
                        else
                        "batched_final_semantic_snapshot"
                        if args.batched_stability
                        else "per_sample_semantic_snapshot"
                    ),
                    "hashes": hashes,
                    "unique_hashes": len(set(hashes)),
                    "command_queue_depth": runtime.get("command_queue_depth"),
                    "frame_timings": runtime.get("frame_timings"),
                    "representation": _scatter_metrics(final_snapshot, plot.id).get("representation"),
                }
            )
        except Exception as exc:
            errors.append(f"{type(exc).__name__}: {exc}\n{traceback.format_exc()}")
        finally:
            try:
                if app._handle is not None:
                    app._handle.request_exit()
            except (AttributeError, RuntimeError):
                pass

    thread = threading.Thread(target=worker, daemon=True)
    thread.start()
    previous_smoke = os.environ.get("DRAGONGUI_SMOKE_FRAMES")
    os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(max(1, args.frames))
    try:
        run_result = app.run(window)
    finally:
        stop_memory.set()
        memory_thread.join(timeout=2.0)
        if previous_smoke is None:
            os.environ.pop("DRAGONGUI_SMOKE_FRAMES", None)
        else:
            os.environ["DRAGONGUI_SMOKE_FRAMES"] = previous_smoke
    thread.join(timeout=20.0)
    if thread.is_alive():
        errors.append("TimeoutError: comparison worker did not finish")

    valid = bool(
        not errors
        and result.get("final_span_fraction", 1.0) <= 0.5
        and (result.get("final_probe") or {}).get("target_lit") is True
        and result.get("unique_hashes") == 1
        and result.get("command_queue_depth") == 0
    )
    payload = {
        "status": "ok" if valid else "invalid",
        "contract": "XY seed/sentinels and 900x420 viewport; 42 real Windows wheel inputs at 33 ms; target lit; zoom proof; ten stable frames",
        "rows": args.n,
        "public_build_ms": public_build_ms,
        "gesture": result,
        "peak_rss_bytes": peak_rss,
        "frame_ms": run_result.get("frame_ms"),
        "errors": errors,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload))
    if not valid:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
