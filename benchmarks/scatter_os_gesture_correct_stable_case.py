"""Replay real Windows scatter gestures and require correct stable recovery."""

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
from pathlib import Path
from typing import Any, Callable

from scatter_interaction_correct_stable_case import (
    _final_runtime_ready,
    _pixel_probe,
    _rss_bytes,
    _scatter_metrics,
)
from scatter_density_correct_stable_case import wait_for_ready


TITLE = "Scatter Windows gesture correctness gate"
VK_HOME = 0x24
VK_SHIFT = 0x10
KEYEVENTF_KEYUP = 0x0002
KEYEVENTF_EXTENDEDKEY = 0x0001
MOUSEEVENTF_LEFTDOWN = 0x0002
MOUSEEVENTF_LEFTUP = 0x0004
MOUSEEVENTF_WHEEL = 0x0800
SWP_NOZORDER = 0x0004
SWP_NOACTIVATE = 0x0010


class Point(ctypes.Structure):
    _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]


class Rect(ctypes.Structure):
    _fields_ = [
        ("left", ctypes.c_long),
        ("top", ctypes.c_long),
        ("right", ctypes.c_long),
        ("bottom", ctypes.c_long),
    ]


def find_window(title: str = TITLE) -> int | None:
    user32 = ctypes.windll.user32
    pid = os.getpid()
    found = ctypes.c_void_p(0)

    @ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)
    def enum_proc(hwnd: int, _param: int) -> bool:
        if not user32.IsWindowVisible(hwnd):
            return True
        window_pid = ctypes.c_ulong()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(window_pid))
        if window_pid.value != pid:
            return True
        length = user32.GetWindowTextLengthW(hwnd)
        if length <= 0:
            return True
        text = ctypes.create_unicode_buffer(length + 1)
        user32.GetWindowTextW(hwnd, text, length + 1)
        if title in text.value:
            found.value = hwnd
            return False
        return True

    user32.EnumWindows(enum_proc, 0)
    return int(found.value) if found.value else None


def client_to_screen(hwnd: int, x: float, y: float) -> tuple[int, int]:
    point = Point(int(round(x)), int(round(y)))
    if not ctypes.windll.user32.ClientToScreen(hwnd, ctypes.byref(point)):
        raise RuntimeError("ClientToScreen failed")
    return int(point.x), int(point.y)


def move_pointer(hwnd: int, x: float, y: float) -> None:
    screen_x, screen_y = client_to_screen(hwnd, x, y)
    if not ctypes.windll.user32.SetCursorPos(screen_x, screen_y):
        raise RuntimeError("SetCursorPos failed")


def mouse_event(flags: int, data: int = 0) -> None:
    ctypes.windll.user32.mouse_event(flags, 0, 0, data, 0)


def drag(hwnd: int, start: tuple[float, float], end: tuple[float, float]) -> None:
    move_pointer(hwnd, *start)
    mouse_event(MOUSEEVENTF_LEFTDOWN)
    time.sleep(0.012)
    move_pointer(hwnd, *end)
    time.sleep(0.012)
    mouse_event(MOUSEEVENTF_LEFTUP)


def key_press(vk: int, *, extended: bool = False) -> None:
    scan_code = ctypes.windll.user32.MapVirtualKeyW(vk, 0)
    flags = KEYEVENTF_EXTENDEDKEY if extended else 0
    ctypes.windll.user32.keybd_event(vk, scan_code, flags, 0)
    time.sleep(0.008)
    ctypes.windll.user32.keybd_event(vk, scan_code, flags | KEYEVENTF_KEYUP, 0)


def camera(snapshot: dict[str, Any], plot_id: str) -> dict[str, Any]:
    value = _scatter_metrics(snapshot, plot_id).get("camera")
    if not isinstance(value, dict):
        raise RuntimeError("scatter camera snapshot was unavailable")
    return value


def viewport(snapshot: dict[str, Any], plot_id: str) -> tuple[list[float], list[float]]:
    value = _scatter_metrics(snapshot, plot_id).get("viewport")
    if not isinstance(value, dict):
        raise RuntimeError("scatter viewport snapshot was unavailable")
    offset = value.get("offset")
    size = value.get("size")
    if not isinstance(offset, list) or not isinstance(size, list):
        raise RuntimeError("scatter viewport snapshot was malformed")
    return [float(offset[0]), float(offset[1])], [float(size[0]), float(size[1])]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", type=int, default=1_000_000)
    parser.add_argument("--captures", type=int, default=10)
    parser.add_argument("--frames", type=int, default=3000)
    parser.add_argument("--step-ms", type=float, default=250.0)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if sys.platform != "win32":
        parser.error("this probe requires Windows")
    if args.n < 300_000:
        parser.error("--n must be at least the adaptive density threshold")
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

    memory_thread = threading.Thread(target=sample_memory, daemon=True)
    memory_thread.start()

    x = np.linspace(-0.55, 0.55, args.n, dtype=np.float32)
    y = np.sin(x * np.float32(35.0), dtype=np.float32) * np.float32(0.45)
    x[:5] = (-1.0, -1.0, 1.0, 1.0, 0.0)
    y[:5] = (-1.0, 1.0, -1.0, 1.0, 0.0)
    store = dg.PointStore(x, y, ownership="borrowed")

    selection_event = threading.Event()
    selection_payload: dict[str, Any] = {}

    def on_selection(payload: dict[str, Any]) -> None:
        selection_payload.clear()
        selection_payload.update(payload)
        selection_event.set()

    app = dg.App(loading_screen=False)
    window = dg.Window(TITLE, width=720, height=720)
    with window:
        plot = dg.ScatterPlot2D(
            store,
            rendering="adaptive",
            point_size=12.0,
            auto_point_size=False,
            grid=False,
            id="os-gesture-scatter",
        )

    result: dict[str, Any] = {}
    errors: list[str] = []

    def worker() -> None:
        try:
            initial_snapshot = wait_for_ready(app)
            initial_revision = int(store.stats()["data_revision"])
            deadline = time.perf_counter() + 15.0
            while not _final_runtime_ready(initial_snapshot, plot.id, args.n, initial_revision):
                if time.perf_counter() >= deadline:
                    raise TimeoutError("initial full-density state did not settle")
                time.sleep(0.01)
                initial_snapshot = app.debug_snapshot(timeout_ms=3000)
            initial_camera = camera(initial_snapshot, plot.id)
            initial_offset, initial_size = viewport(initial_snapshot, plot.id)
            center = (
                initial_offset[0] + initial_size[0] * 0.5,
                initial_offset[1] + initial_size[1] * 0.5,
            )

            hwnd = None
            window_deadline = time.perf_counter() + 5.0
            while time.perf_counter() < window_deadline and hwnd is None:
                hwnd = find_window()
                if hwnd is None:
                    time.sleep(0.05)
            if hwnd is None:
                raise RuntimeError("native benchmark window was not found")
            if not ctypes.windll.user32.SetForegroundWindow(hwnd):
                # A foreground call may report zero when the window is already active.
                if ctypes.windll.user32.GetForegroundWindow() != hwnd:
                    raise RuntimeError("benchmark window could not receive foreground focus")

            original_rect = Rect()
            if not ctypes.windll.user32.GetWindowRect(hwnd, ctypes.byref(original_rect)):
                raise RuntimeError("GetWindowRect failed")
            original_width = int(original_rect.right - original_rect.left)
            original_height = int(original_rect.bottom - original_rect.top)

            step_s = max(args.step_ms, 40.0) / 1000.0
            schedule_start = time.perf_counter() + 0.10
            submissions: list[dict[str, Any]] = []
            checkpoints: dict[str, Any] = {}
            selection_submitted_at = 0.0
            updated_y: Any = None

            def wheel_zoom() -> None:
                move_pointer(hwnd, *center)
                mouse_event(MOUSEEVENTF_LEFTDOWN)
                mouse_event(MOUSEEVENTF_LEFTUP)
                time.sleep(0.025)
                mouse_event(MOUSEEVENTF_WHEEL, 120)

            def zoom_checkpoint() -> None:
                snap = app.debug_snapshot(timeout_ms=3000)
                checkpoints["zoom_camera"] = camera(snap, plot.id)

            def shift_pan() -> None:
                ctypes.windll.user32.keybd_event(VK_SHIFT, 0, 0, 0)
                try:
                    drag(hwnd, center, (center[0] + 70.0, center[1] + 35.0))
                finally:
                    ctypes.windll.user32.keybd_event(VK_SHIFT, 0, KEYEVENTF_KEYUP, 0)

            def pan_checkpoint() -> None:
                snap = app.debug_snapshot(timeout_ms=3000)
                checkpoints["pan_camera"] = camera(snap, plot.id)

            def resize_away() -> None:
                if not ctypes.windll.user32.SetWindowPos(
                    hwnd,
                    0,
                    original_rect.left,
                    original_rect.top,
                    original_width + 140,
                    max(480, original_height - 100),
                    SWP_NOZORDER | SWP_NOACTIVATE,
                ):
                    raise RuntimeError("SetWindowPos resize-away failed")

            def mutate() -> None:
                nonlocal updated_y
                updated_y = np.ascontiguousarray(y + np.float32(0.002), dtype=np.float32)
                updated_y[:5] = (-1.0, 1.0, -1.0, 1.0, 0.0)
                store.replace_column("y", updated_y, ownership="moved")
                plot.set_points(store)

            def resize_home() -> None:
                if not ctypes.windll.user32.SetWindowPos(
                    hwnd,
                    0,
                    original_rect.left,
                    original_rect.top,
                    original_width,
                    original_height,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                ):
                    raise RuntimeError("SetWindowPos restore failed")

            def home_key() -> None:
                ctypes.windll.user32.SetForegroundWindow(hwnd)
                key_press(VK_HOME, extended=True)

            def home_checkpoint() -> None:
                snap = app.debug_snapshot(timeout_ms=3000)
                checkpoints["home_camera"] = camera(snap, plot.id)

            def enable_rectangle_mode() -> None:
                plot.enable_rectangle_picking(on_selection)

            def select_sentinel() -> None:
                nonlocal selection_submitted_at
                snap = app.debug_snapshot(timeout_ms=3000)
                offset, size = viewport(snap, plot.id)
                start = (offset[0] + size[0] * 0.13, offset[1] + size[1] * 0.13)
                end = (offset[0] + size[0] * 0.19, offset[1] + size[1] * 0.19)
                selection_submitted_at = time.perf_counter()
                drag(hwnd, start, end)

            actions: list[tuple[str, Callable[[], None], bool]] = [
                ("os_wheel_zoom", wheel_zoom, True),
                ("zoom_camera_checkpoint", zoom_checkpoint, False),
                ("os_shift_left_pan", shift_pan, True),
                ("pan_camera_checkpoint", pan_checkpoint, False),
                ("os_resize_away", resize_away, True),
                ("source_revision_update", mutate, True),
                ("os_resize_restore", resize_home, True),
                ("os_home_key", home_key, True),
                ("home_camera_checkpoint", home_checkpoint, False),
                ("enable_rectangle_mode", enable_rectangle_mode, False),
                ("os_rectangle_select", select_sentinel, True),
            ]
            last_input_at = schedule_start
            for index, (name, action, is_input) in enumerate(actions):
                action_deadline = schedule_start + step_s * index
                remaining = action_deadline - time.perf_counter()
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
                        "deadline_ms": (action_deadline - schedule_start) * 1000.0,
                        "schedule_lag_ms": (submitted - action_deadline) * 1000.0,
                        "call_ms": (completed - submitted) * 1000.0,
                    }
                )

            zoom_camera = checkpoints.get("zoom_camera") or {}
            pan_camera = checkpoints.get("pan_camera") or {}
            home_camera = checkpoints.get("home_camera") or {}
            if zoom_camera.get("distance") == initial_camera.get("distance"):
                raise RuntimeError("OS wheel did not change scatter camera distance")
            if pan_camera.get("target") == zoom_camera.get("target"):
                raise RuntimeError("OS Shift+drag did not change scatter camera target")
            if home_camera != initial_camera:
                raise RuntimeError(
                    f"OS Home did not restore fitted camera: {home_camera!r} != {initial_camera!r}"
                )
            if not selection_event.wait(timeout=10.0):
                raise TimeoutError("OS rectangle selection callback did not arrive")
            selection_received_at = time.perf_counter()
            raw_indices = list((selection_payload.get("actors") or {}).get("0", []))
            if raw_indices != [1] or plot.selected_indices != [1]:
                raise RuntimeError(
                    f"OS rectangle selected {raw_indices!r}/{plot.selected_indices!r}, expected [1]"
                )

            expected_revision = int(store.stats()["data_revision"])
            recovery_deadline = time.perf_counter() + 15.0
            first_correct: Any = None
            final_snapshot: dict[str, Any] | None = None
            final_probe: dict[str, Any] | None = None
            first_correct_at = 0.0
            while time.perf_counter() < recovery_deadline:
                snap = app.debug_snapshot(timeout_ms=3000)
                if _final_runtime_ready(snap, plot.id, args.n, expected_revision):
                    image = plot.screenshot()
                    if image is not None and image.shape == (718, 718, 4):
                        probe = _pixel_probe(np, image)
                        if probe["sentinel_windows_valid"]:
                            first_correct = image
                            final_snapshot = snap
                            final_probe = probe
                            first_correct_at = time.perf_counter()
                            break
                time.sleep(0.01)
            if first_correct is None or final_snapshot is None or final_probe is None:
                raise TimeoutError("OS gesture sequence did not recover correct full-density pixels")

            captures = [first_correct]
            while len(captures) < args.captures:
                image = plot.screenshot()
                if image is None:
                    raise RuntimeError("stable scatter screenshot returned None")
                captures.append(image)
            stable_at = time.perf_counter()
            hashes = [hashlib.sha256(image.tobytes()).hexdigest() for image in captures]
            final_metrics = _scatter_metrics(final_snapshot, plot.id)
            representation = final_metrics.get("representation", {})
            density = representation.get("density", {})
            runtime = final_snapshot.get("runtime", {})
            result.update(
                {
                    "sequence": submissions,
                    "initial_camera": initial_camera,
                    "zoom_camera": zoom_camera,
                    "pan_camera": pan_camera,
                    "home_camera": home_camera,
                    "wheel_changed_distance": True,
                    "pan_changed_target": True,
                    "home_restored_camera": True,
                    "selection": {
                        "payload_actor_0_indices": raw_indices,
                        "selected_indices": list(plot.selected_indices),
                        "callback_latency_ms": (
                            selection_received_at - selection_submitted_at
                        )
                        * 1000.0,
                        "exact_source_row_verified": True,
                    },
                    "last_input_to_first_correct_ms": (
                        first_correct_at - last_input_at
                    )
                    * 1000.0,
                    "last_input_to_ten_stable_ms": (stable_at - last_input_at) * 1000.0,
                    "hashes": hashes,
                    "unique_hashes": len(set(hashes)),
                    **final_probe,
                    "source_revision": density.get("source_revision"),
                    "density_scope": density.get("scope"),
                    "source_rows": representation.get("source_rows"),
                    "render_rows": representation.get("render_rows"),
                    "represented_source_rows": density.get("represented_source_rows"),
                    "command_queue_depth": runtime.get("command_queue_depth"),
                    "frame_timings": runtime.get("frame_timings"),
                    "gpu_memory": final_metrics.get("gpu_memory"),
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

    thread = threading.Thread(target=worker, name="scatter-os-gesture-gate", daemon=True)
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
        errors.append("TimeoutError: OS gesture worker did not finish")

    valid = bool(
        not errors
        and result.get("wheel_changed_distance") is True
        and result.get("pan_changed_target") is True
        and result.get("home_restored_camera") is True
        and (result.get("selection") or {}).get("exact_source_row_verified") is True
        and result.get("unique_hashes") == 1
        and result.get("sentinel_windows_valid") is True
        and result.get("density_scope") == "full"
        and result.get("source_revision") == 2
        and result.get("represented_source_rows") == args.n
        and result.get("command_queue_depth") == 0
    )
    payload = {
        "status": "ok" if valid else "invalid",
        "contract": "real Windows wheel, Shift+left pan, resize, Home, and rectangle-select events; exact source row; final full-density ten-frame stability",
        "rows": args.n,
        "captures_requested": args.captures,
        "step_ms": args.step_ms,
        "gesture": result,
        "peak_rss_bytes": peak_rss,
        "errors": errors,
        "frame_ms": run_result.get("frame_ms"),
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload))
    if not valid:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
