"""Validate fixed-schedule scatter interaction recovery and stable pixels."""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
import sys
import threading
import time
import traceback
from collections import Counter
from pathlib import Path
from typing import Any, Callable

from scatter_density_correct_stable_case import wait_for_ready, window_non_background


def _rss_bytes() -> int:
    """Return current resident working-set bytes without optional dependencies."""
    if sys.platform == "win32":
        class ProcessMemoryCountersEx(ctypes.Structure):
            _fields_ = [
                ("cb", ctypes.c_ulong),
                ("page_fault_count", ctypes.c_ulong),
                ("peak_working_set_size", ctypes.c_size_t),
                ("working_set_size", ctypes.c_size_t),
                ("quota_peak_paged_pool_usage", ctypes.c_size_t),
                ("quota_paged_pool_usage", ctypes.c_size_t),
                ("quota_peak_non_paged_pool_usage", ctypes.c_size_t),
                ("quota_non_paged_pool_usage", ctypes.c_size_t),
                ("pagefile_usage", ctypes.c_size_t),
                ("peak_pagefile_usage", ctypes.c_size_t),
                ("private_usage", ctypes.c_size_t),
            ]

        counters = ProcessMemoryCountersEx()
        counters.cb = ctypes.sizeof(counters)
        get_current_process = ctypes.windll.kernel32.GetCurrentProcess
        get_current_process.restype = ctypes.c_void_p
        get_process_memory_info = ctypes.windll.psapi.GetProcessMemoryInfo
        get_process_memory_info.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_ulong]
        get_process_memory_info.restype = ctypes.c_int
        if get_process_memory_info(
            get_current_process(), ctypes.byref(counters), counters.cb
        ):
            return int(counters.working_set_size)
        return 0
    try:
        import resource

        value = int(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss)
        return value if sys.platform == "darwin" else value * 1024
    except (ImportError, OSError):
        return 0


def _scatter_metrics(snapshot: dict[str, Any], plot_id: str) -> dict[str, Any]:
    gpu = snapshot.get("gpu", {})
    resources = gpu.get("resources", {}) if isinstance(gpu, dict) else {}
    scatters = resources.get("scatters", {}) if isinstance(resources, dict) else {}
    value = scatters.get(plot_id, {}) if isinstance(scatters, dict) else {}
    return value if isinstance(value, dict) else {}


def _final_runtime_ready(
    snapshot: dict[str, Any], plot_id: str, rows: int, revision: int
) -> bool:
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
        and representation.get("policy_requested") == "adaptive"
        and representation.get("policy_effective") == "density"
        and density.get("scope") == "full"
        and density.get("source_rows_conserved") is True
        and int(density.get("finite_source_rows", -1)) == rows
        and int(density.get("represented_source_rows", -1)) == rows
        and int(density.get("source_revision", -1)) == revision
        and int(source.get("revision", -1)) == revision
        and density.get("viewport_job_pending") is False
        and int(density.get("viewport_job_errors", -1)) == 0
    )


def _pixel_probe(np: Any, rgba: Any) -> dict[str, Any]:
    opaque_rgb = rgba[rgba[:, :, 3] > 240, :3]
    if not len(opaque_rgb):
        raise RuntimeError("capture contains no opaque pixels")
    background = Counter(map(tuple, opaque_rgb.tolist())).most_common(1)[0][0]
    windows = {
        "bottom_left": window_non_background(np, rgba, background, (0.158, 0.841), 0.035),
        "top_left": window_non_background(np, rgba, background, (0.158, 0.158), 0.035),
        "bottom_right": window_non_background(np, rgba, background, (0.841, 0.841), 0.035),
        "top_right": window_non_background(np, rgba, background, (0.841, 0.158), 0.035),
        "center": window_non_background(np, rgba, background, (0.5, 0.5), 0.055),
    }
    return {
        "background_rgb": list(background),
        "sentinel_windows": windows,
        "sentinel_windows_valid": all(item["pixels"] >= 4 for item in windows.values()),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", type=int, default=1_000_000)
    parser.add_argument("--captures", type=int, default=10)
    parser.add_argument("--frames", type=int, default=3000)
    parser.add_argument("--step-ms", type=float, default=320.0)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.n < 300_000:
        parser.error("--n must be at least the adaptive density entry threshold")
    if args.captures < 10:
        parser.error("--captures must be at least 10")

    sys.path.insert(0, str(args.package_root.resolve()))
    import numpy as np
    import dragongui as dg

    peak_rss = _rss_bytes()
    stop_memory = threading.Event()

    def sample_memory() -> None:
        nonlocal peak_rss
        while not stop_memory.wait(0.01):
            peak_rss = max(peak_rss, _rss_bytes())

    memory_thread = threading.Thread(target=sample_memory, name="scatter-interaction-rss", daemon=True)
    memory_thread.start()
    x = np.linspace(-0.55, 0.55, args.n, dtype=np.float32)
    y = np.sin(x * np.float32(35.0), dtype=np.float32) * np.float32(0.45)
    x[:5] = (-1.0, -1.0, 1.0, 1.0, 0.0)
    y[:5] = (-1.0, 1.0, -1.0, 1.0, 0.0)
    store = dg.PointStore(x, y, ownership="borrowed")

    build_started_at = time.perf_counter()
    app = dg.App(loading_screen=False)
    window = dg.Window("Scatter fixed-schedule interaction gate", width=720, height=720)
    selection_event = threading.Event()
    selection_payload: dict[str, Any] = {}

    def on_selection(payload: dict[str, Any]) -> None:
        selection_payload.clear()
        selection_payload.update(payload)
        selection_event.set()

    with window:
        plot = dg.ScatterPlot2D(
            store,
            rendering="adaptive",
            point_size=12.0,
            auto_point_size=False,
            grid=False,
            id="interaction-correct-stable-scatter",
        )
        plot.enable_rectangle_picking(on_selection)
    public_build_ms = (time.perf_counter() - build_started_at) * 1000.0

    result: dict[str, Any] = {}
    producer_errors: list[str] = []
    render_started_at = time.perf_counter()

    def worker() -> None:
        try:
            wait_for_ready(app)
            initial_correct_image: Any = None
            initial_correct_snapshot: dict[str, Any] | None = None
            first_nonblank_ms: float | None = None
            initial_deadline = time.perf_counter() + 15.0
            initial_revision = int(store.stats()["data_revision"])
            while time.perf_counter() < initial_deadline:
                snapshot = app.debug_snapshot(timeout_ms=3000)
                image = plot.screenshot()
                if image is not None:
                    opaque = image[image[:, :, 3] > 240, :3]
                    if len(opaque):
                        background = Counter(map(tuple, opaque.tolist())).most_common(1)[0][0]
                        delta = np.max(
                            np.abs(opaque.astype(np.int16) - np.asarray(background, dtype=np.int16)),
                            axis=1,
                        )
                        if first_nonblank_ms is None and bool(np.any(delta >= 18)):
                            first_nonblank_ms = (time.perf_counter() - render_started_at) * 1000.0
                    if (
                        image.shape == (718, 718, 4)
                        and _final_runtime_ready(snapshot, plot.id, args.n, initial_revision)
                    ):
                        probe = _pixel_probe(np, image)
                        if probe["sentinel_windows_valid"]:
                            initial_correct_image = image
                            initial_correct_snapshot = snapshot
                            break
                time.sleep(0.01)
            if initial_correct_image is None or initial_correct_snapshot is None:
                raise TimeoutError("initial full-density pixels did not become correct before timeout")
            initial_correct_ms = (time.perf_counter() - render_started_at) * 1000.0
            baseline_frames = int(
                initial_correct_snapshot.get("runtime", {}).get("frames_rendered", 0)
            )
            initial_camera = plot.get_camera()
            if initial_camera is None:
                raise RuntimeError("initial scatter camera was unavailable")
            handle = app._handle
            if handle is None:
                raise RuntimeError("live app handle was unavailable")

            schedule_start = time.perf_counter() + 0.10
            step_s = max(args.step_ms, 20.0) / 1000.0
            submissions: list[dict[str, Any]] = []
            home_checkpoint_states: list[dict[str, Any]] = []
            updated_y: Any = None

            def zoom() -> None:
                plot.set_parallel_scale(0.52, 0.52)

            def pan() -> None:
                state = dict(initial_camera)
                state["target"] = [0.16, -0.11, 0.0]
                state["parallel"] = True
                plot.set_camera(state)

            def resize_away() -> None:
                handle.request_window_resize(860, 620)

            def mutate() -> None:
                nonlocal updated_y
                updated_y = np.ascontiguousarray(y + np.float32(0.002), dtype=np.float32)
                updated_y[:5] = (-1.0, 1.0, -1.0, 1.0, 0.0)
                store.replace_column("y", updated_y, ownership="moved")
                plot.set_points(store)

            def resize_home() -> None:
                handle.request_window_resize(720, 720)

            def rectangle_zoom() -> None:
                plot.fit((-0.42, -0.28, 0.48, 0.36))

            def restore() -> None:
                plot.fit()

            def home_checkpoint() -> None:
                state = plot.get_camera()
                if state is None:
                    raise RuntimeError("home checkpoint camera state was unavailable")
                home_checkpoint_states.append(state)

            selection_submitted_at = 0.0

            def select_top_left_sentinel() -> None:
                nonlocal selection_submitted_at
                selection_submitted_at = time.perf_counter()
                plot.select_rectangle((0.13, 0.13, 0.19, 0.19))

            actions: list[tuple[str, Callable[[], None], bool]] = [
                ("zoom", zoom, True),
                ("pan", pan, True),
                ("resize_860x620", resize_away, True),
                ("source_revision_update", mutate, True),
                ("rectangle_zoom_bounds", rectangle_zoom, True),
                ("resize_720x720", resize_home, True),
                ("home_fit_1", restore, True),
                ("home_checkpoint_1", home_checkpoint, False),
                ("zoom_after_home", zoom, True),
                ("home_fit_2", restore, True),
                ("home_checkpoint_2", home_checkpoint, False),
                ("pan_after_home", pan, True),
                ("home_fit_3", restore, True),
                ("select_top_left_sentinel", select_top_left_sentinel, True),
            ]
            last_input_at = schedule_start
            for index, (name, action, is_input) in enumerate(actions):
                deadline = schedule_start + step_s * index
                remaining = deadline - time.perf_counter()
                if remaining > 0:
                    time.sleep(remaining)
                submitted = time.perf_counter()
                action()
                completed = time.perf_counter()
                if is_input:
                    last_input_at = completed
                submissions.append(
                    {
                        "action": name,
                        "deadline_ms": (deadline - schedule_start) * 1000.0,
                        "schedule_lag_ms": (submitted - deadline) * 1000.0,
                        "call_ms": (completed - submitted) * 1000.0,
                    }
                )

            if not selection_event.wait(timeout=10.0):
                raise TimeoutError("programmatic rectangle selection callback did not arrive")
            selection_received_at = time.perf_counter()
            expected_selected_indices = [1]
            raw_primary_indices = list(
                (selection_payload.get("actors") or {}).get("0", [])
            )
            if raw_primary_indices != expected_selected_indices:
                raise RuntimeError(
                    "programmatic selection returned actor-0 rows "
                    f"{raw_primary_indices!r}, expected {expected_selected_indices!r}"
                )
            if plot.selected_indices != expected_selected_indices:
                raise RuntimeError(
                    "selected_indices did not preserve exact source rows: "
                    f"{plot.selected_indices!r}"
                )
            if plot.selected_index_values is not None:
                raise RuntimeError("unlabelled PointStore selection unexpectedly produced labels")

            expected_revision = int(store.stats()["data_revision"])
            recovery_deadline = last_input_at + 15.0
            first_correct: Any = None
            first_correct_at = 0.0
            final_snapshot: dict[str, Any] | None = None
            final_probe: dict[str, Any] | None = None
            recovery_polls = 0
            while time.perf_counter() < recovery_deadline:
                recovery_polls += 1
                snapshot = app.debug_snapshot(timeout_ms=3000)
                recovery_metrics = _scatter_metrics(snapshot, plot.id)
                recovery_representation = recovery_metrics.get("representation", {})
                recovery_density = (
                    recovery_representation.get("density", {})
                    if isinstance(recovery_representation, dict)
                    else {}
                )
                result["recovery_diagnostic"] = {
                    "polls": recovery_polls,
                    "command_queue_depth": snapshot.get("runtime", {}).get(
                        "command_queue_depth"
                    ),
                    "dimensions": recovery_metrics.get("dimensions"),
                    "payload_status": recovery_metrics.get("payload_status"),
                    "last_point_count": recovery_metrics.get("last_point_count"),
                    "policy_requested": recovery_representation.get("policy_requested"),
                    "policy_effective": recovery_representation.get("policy_effective"),
                    "density_scope": recovery_density.get("scope"),
                    "density_source_revision": recovery_density.get("source_revision"),
                    "store_source_revision": (
                        recovery_metrics.get("point_store_source") or {}
                    ).get("revision"),
                    "viewport_job_pending": recovery_density.get("viewport_job_pending"),
                    "viewport_job_errors": recovery_density.get("viewport_job_errors"),
                    "camera": recovery_metrics.get("camera"),
                }
                if _final_runtime_ready(snapshot, plot.id, args.n, expected_revision):
                    image = plot.screenshot()
                    if image is not None and image.shape == (718, 718, 4):
                        probe = _pixel_probe(np, image)
                        if probe["sentinel_windows_valid"]:
                            first_correct = image
                            first_correct_at = time.perf_counter()
                            final_snapshot = snapshot
                            final_probe = probe
                            break
                time.sleep(0.01)
            if first_correct is None or final_snapshot is None or final_probe is None:
                raise TimeoutError("final full-density pixels did not become correct before timeout")

            captures = [first_correct]
            capture_ms: list[float] = []
            while len(captures) < args.captures:
                t0 = time.perf_counter()
                image = plot.screenshot()
                capture_ms.append((time.perf_counter() - t0) * 1000.0)
                if image is None:
                    raise RuntimeError("scatter screenshot API returned None")
                captures.append(image)
            stable_at = time.perf_counter()
            hashes = [hashlib.sha256(image.tobytes()).hexdigest() for image in captures]
            stable_snapshot = app.debug_snapshot(timeout_ms=3000)
            final_camera = plot.get_camera()
            if final_camera is None:
                raise RuntimeError("final home camera state was unavailable")
            metrics = _scatter_metrics(stable_snapshot, plot.id)
            representation = metrics.get("representation", {})
            density = representation.get("density", {}) if isinstance(representation, dict) else {}
            runtime = stable_snapshot.get("runtime", {})
            schedule_lags = [entry["schedule_lag_ms"] for entry in submissions]
            first = captures[0]
            interaction_seconds = max(stable_at - schedule_start, 0.001)
            presented_frames = max(int(runtime.get("frames_rendered", 0)) - baseline_frames, 0)
            expected_60hz_frames = interaction_seconds * 60.0
            result.update(
                {
                    "public_build_ms": public_build_ms,
                    "first_nonblank_ms": first_nonblank_ms,
                    "initial_first_correct_ms": initial_correct_ms,
                    "sequence": submissions,
                    "schedule_max_lag_ms": max(schedule_lags),
                    "schedule_mean_lag_ms": sum(schedule_lags) / len(schedule_lags),
                    "recovery_polls": recovery_polls,
                    "last_input_to_first_correct_ms": (first_correct_at - last_input_at) * 1000.0,
                    "last_input_to_ten_stable_ms": (stable_at - last_input_at) * 1000.0,
                    "width": int(first.shape[1]),
                    "height": int(first.shape[0]),
                    "channels": int(first.shape[2]),
                    "hashes": hashes,
                    "unique_hashes": len(set(hashes)),
                    "home_checkpoint_states": home_checkpoint_states,
                    "final_home_camera": final_camera,
                    "home_restore_unique_states": len(
                        {
                            json.dumps(state, sort_keys=True)
                            for state in home_checkpoint_states + [final_camera]
                        }
                    ),
                    "selection": {
                        "normalized_rect": [0.13, 0.13, 0.19, 0.19],
                        "expected_source_indices": expected_selected_indices,
                        "payload_actor_0_indices": raw_primary_indices,
                        "selected_indices": list(plot.selected_indices),
                        "selected_index_values": plot.selected_index_values,
                        "callback_latency_ms": (
                            selection_received_at - selection_submitted_at
                        )
                        * 1000.0,
                        "exact_source_row_verified": True,
                    },
                    "capture_ms_after_first": capture_ms,
                    **final_probe,
                    "source_revision": density.get("source_revision"),
                    "policy_requested": representation.get("policy_requested"),
                    "policy_effective": representation.get("policy_effective"),
                    "density_scope": density.get("scope"),
                    "source_rows": representation.get("source_rows"),
                    "render_rows": representation.get("render_rows"),
                    "represented_source_rows": density.get("represented_source_rows"),
                    "viewport_jobs_started": density.get("viewport_jobs_started"),
                    "viewport_jobs_completed": density.get("viewport_jobs_completed"),
                    "viewport_jobs_stale": density.get("viewport_jobs_stale"),
                    "viewport_job_errors": density.get("viewport_job_errors"),
                    "command_queue_depth": runtime.get("command_queue_depth"),
                    "frame_timings": runtime.get("frame_timings"),
                    "presentation": {
                        "measurement_seconds": interaction_seconds,
                        "frames_presented": presented_frames,
                        "achieved_hz": presented_frames / interaction_seconds,
                        "target_hz": 60.0,
                        "missed_presentation_estimate_percent": max(
                            0.0,
                            (expected_60hz_frames - presented_frames)
                            / expected_60hz_frames
                            * 100.0,
                        ),
                        "note": "60 Hz estimate from native presented-frame delta over the fixed-schedule interaction window",
                    },
                    "gpu_memory": metrics.get("gpu_memory"),
                }
            )
        except Exception as exc:
            producer_errors.append(
                f"{type(exc).__name__}: {exc}\n{traceback.format_exc()}"
            )
        finally:
            try:
                if app._handle is not None:
                    app._handle.request_exit()
            except (AttributeError, RuntimeError):
                pass

    thread = threading.Thread(target=worker, name="scatter-interaction-correct-stable", daemon=True)
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
        producer_errors.append("TimeoutError: interaction worker did not finish")

    valid = bool(
        not producer_errors
        and result.get("unique_hashes") == 1
        and result.get("home_restore_unique_states") == 1
        and (result.get("selection") or {}).get("exact_source_row_verified") is True
        and result.get("sentinel_windows_valid") is True
        and result.get("source_revision") == 2
        and result.get("density_scope") == "full"
        and result.get("command_queue_depth") == 0
    )
    payload = {
        "status": "ok" if valid else "invalid",
        "contract": "fixed-deadline zoom, pan, rectangle-bounds zoom, resize, source mutation, and three home restores; identical restored camera states; current full-density revision and ten stable captures",
        "rows": args.n,
        "captures_requested": args.captures,
        "step_ms": args.step_ms,
        "selection": "normalized public rectangle selection with exact source-row verification",
        "interaction": result,
        "peak_rss_bytes": peak_rss,
        "producer_errors": producer_errors,
        "frame_ms": run_result.get("frame_ms"),
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload))
    if not valid:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
