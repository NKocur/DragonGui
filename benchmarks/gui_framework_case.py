"""Run one isolated cross-framework GUI benchmark case and emit JSON.

Each invocation owns exactly one GUI event loop. The parent matrix runner
launches this module in fresh processes so imports, toolkit initialization,
native allocations, and event-loop state cannot leak between samples.
"""

from __future__ import annotations

import argparse
import ctypes
import importlib.metadata
import json
import os
from pathlib import Path
import platform
import statistics
import sys
import time
from typing import Any, Callable

from gui_benchmark_validation import (
    ValidationRecorder,
    add_common_runtime_checks,
    find_tree_node,
)


ROOT = Path(__file__).resolve().parents[1]
TARGET_FRAME_S = 1.0 / 60.0


def _percentile(values: list[float], percentile: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = min(len(ordered) - 1, max(0, round((len(ordered) - 1) * percentile)))
    return ordered[index]


def _timings(values: list[float]) -> dict[str, float | int]:
    return {
        "count": len(values),
        "mean_ms": statistics.fmean(values) if values else 0.0,
        "median_ms": statistics.median(values) if values else 0.0,
        "p95_ms": _percentile(values, 0.95),
        "p99_ms": _percentile(values, 0.99),
        "max_ms": max(values, default=0.0),
    }


def _memory_bytes() -> dict[str, int | None]:
    """Return current working-set and private/unique process memory when available."""
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
        get_process_memory_info.argtypes = [
            ctypes.c_void_p,
            ctypes.c_void_p,
            ctypes.c_ulong,
        ]
        get_process_memory_info.restype = ctypes.c_int
        process = get_current_process()
        if get_process_memory_info(
            process, ctypes.byref(counters), counters.cb
        ):
            return {
                "rss_bytes": int(counters.working_set_size),
                "private_bytes": int(counters.private_usage),
            }
        return {"rss_bytes": None, "private_bytes": None}
    try:
        import psutil

        process = psutil.Process()
        rss = int(process.memory_info().rss)
        full = process.memory_full_info()
        private = getattr(full, "uss", None)
        return {
            "rss_bytes": rss,
            "private_bytes": int(private) if private is not None else None,
        }
    except (ImportError, OSError):
        pass
    try:
        import resource

        value = int(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss)
        rss = value if sys.platform == "darwin" else value * 1024
        return {"rss_bytes": rss, "private_bytes": None}
    except (ImportError, OSError):
        return {"rss_bytes": None, "private_bytes": None}


def _rss_bytes() -> int | None:
    """Return current resident memory, preserving the existing benchmark helper API."""
    return _memory_bytes()["rss_bytes"]


def _pace(frame_started: float) -> None:
    remaining = TARGET_FRAME_S - (time.perf_counter() - frame_started)
    if remaining > 0:
        time.sleep(remaining)


def _version(distribution: str, fallback: str) -> str:
    try:
        return importlib.metadata.version(distribution)
    except importlib.metadata.PackageNotFoundError:
        return fallback


def _base_report(
    framework: str,
    version: str,
    args: argparse.Namespace,
    import_ms: float,
) -> dict[str, Any]:
    return {
        "schema": 1,
        "framework": framework,
        "framework_version": version,
        "python": platform.python_version(),
        "platform": platform.platform(),
        "rows": args.rows,
        "logical_controls": args.rows * 4,
        "row_containers": args.rows,
        "frames_requested": args.frames,
        "update_frames_requested": args.updates,
        "updates_per_frame": args.rows * 2 if args.updates else 0,
        "import_ms": import_ms,
    }


def run_dragongui(args: argparse.Namespace) -> dict[str, Any]:
    sys.path.insert(0, str(ROOT / "python"))
    started = time.perf_counter()
    import dragongui as dg
    import_ms = (time.perf_counter() - started) * 1000.0

    os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(args.frames)
    started = time.perf_counter()
    app = dg.App(loading_screen=False)
    window = dg.Window("GUI framework benchmark", width=1000, height=720)
    labels: list[Any] = []
    progress: list[Any] = []
    with dg.ScrollArea(axis="y", style={"height": "100%", "gap": 4}):
        for index in range(args.rows):
            with dg.HLayout(style={"height": 32, "gap": 6}):
                labels.append(dg.Label(f"Row {index:04d}", style={"width": 130}))
                dg.TextInput(f"Value {index}", style={"width": 280})
                dg.Button("Action", style={"width": 100})
                progress.append(
                    dg.ProgressBar(
                        (index % 100) / 100.0,
                        style={"width": 220},
                    )
                )
    build_ms = (time.perf_counter() - started) * 1000.0

    update_apply_samples: list[float] = []
    completed_updates = 0
    update_start_after_run_ms: float | None = None
    import threading

    if args.updates:
        def producer() -> None:
            nonlocal completed_updates, update_start_after_run_ms
            deadline = time.perf_counter() + 15.0
            while time.perf_counter() < deadline:
                try:
                    readiness = app.debug_snapshot(timeout_ms=2000).get("runtime", {}).get(
                        "startup_readiness"
                    )
                except (RuntimeError, TimeoutError):
                    readiness = None
                if readiness == "application_frame_presented":
                    break
                time.sleep(0.01)
            update_start_after_run_ms = (time.perf_counter() - run_started) * 1000.0
            for iteration in range(args.updates):
                def apply(iteration: int = iteration) -> None:
                    nonlocal completed_updates
                    apply_started = time.perf_counter()
                    for index, label in enumerate(labels):
                        label.set_value(f"Row {index:04d} / tick {iteration:04d}")
                    value = (iteration % 100) / 100.0
                    for bar in progress:
                        bar.set_value(value)
                    update_apply_samples.append((time.perf_counter() - apply_started) * 1000.0)
                    completed_updates += 1

                app.call_soon_threadsafe(apply)
                time.sleep(TARGET_FRAME_S)

        threading.Thread(target=producer, name="gui-benchmark-updates", daemon=True).start()

    rss_before_run = _rss_bytes()
    rss_runtime_samples: list[int] = []
    stop_sampling = threading.Event()

    def sample_memory() -> None:
        time.sleep(0.25)
        while not stop_sampling.is_set():
            value = _rss_bytes()
            if value is not None:
                rss_runtime_samples.append(value)
            stop_sampling.wait(0.25)

    sampler = threading.Thread(target=sample_memory, name="gui-benchmark-rss", daemon=True)
    sampler.start()
    run_started = time.perf_counter()
    result = app.run(window)
    run_wall_ms = (time.perf_counter() - run_started) * 1000.0
    stop_sampling.set()
    sampler.join(timeout=1.0)
    snapshot = result.get("debug_snapshot") or {}
    runtime = snapshot.get("runtime") or {}
    gpu = snapshot.get("gpu") or {}
    renderer = gpu.get("renderer") or {}
    framework_metrics = gpu.get("framework") or {}
    stylesheets = gpu.get("stylesheets") or {}
    diagnostics = (gpu.get("layout") or {}).get("diagnostics") or {}
    issue_count = sum(bool(entry.get("issues")) for entry in diagnostics.values())
    validation = ValidationRecorder()
    add_common_runtime_checks(
        validation,
        snapshot,
        expected_widgets=2 + args.rows * 5,
        minimum_frames=args.frames,
    )
    validation.equal(
        "all requested mutation frames completed",
        completed_updates,
        args.updates,
        source="Python benchmark producer",
    )
    if args.rows and args.updates:
        expected_text = f"Row 0000 / tick {args.updates - 1:04d}"
        validation.equal(
            "Python label reached final mutation",
            labels[0].text,
            expected_text,
            source="Python widget state",
        )
        native_label = find_tree_node(gpu.get("tree"), labels[0].id)
        validation.equal(
            "native label reached final mutation",
            (native_label or {}).get("props", {}).get("text"),
            expected_text,
            source="native retained tree",
        )
    validation_report = validation.report()
    report = _base_report("DragonGUI", _version("dragongui", "unknown"), args, import_ms)
    report.update(
        {
            "build_ms": build_ms,
            "first_event_ms": None,
            "run_wall_ms": run_wall_ms,
            "rss_before_run_bytes": rss_before_run,
            "rss_after_run_bytes": _rss_bytes(),
            "rss_runtime_median_bytes": _percentile(rss_runtime_samples, 0.5),
            "rss_runtime_peak_bytes": max(rss_runtime_samples, default=0),
            "rss_runtime_last_bytes": rss_runtime_samples[-1] if rss_runtime_samples else 0,
            "active_frame_ms": runtime.get("frame_timings", {}).get("work", {}),
            "completed_update_frames": completed_updates,
            "update_start_after_run_ms": update_start_after_run_ms,
            "update_apply_ms": _timings(update_apply_samples),
            "native": {
                "frames_rendered": runtime.get("frames_rendered"),
                "wall_fps": runtime.get("wall_fps"),
                "frame_work_ms_avg": runtime.get("frame_work_ms_avg"),
                "command_drain": runtime.get("command_drain"),
                "python_scheduler": runtime.get("python"),
                "style_reapply": framework_metrics.get("style_reapply"),
                "layout_compute": framework_metrics.get("layout_compute"),
                "text_rebuild": framework_metrics.get("text_rebuild"),
                "widget_count": renderer.get("widget_count"),
                "layout_issue_count": issue_count,
                "cascade": stylesheets.get("last_cascade"),
            },
            "validation": validation_report,
        }
    )
    return report


def run_pyqt6(args: argparse.Namespace) -> dict[str, Any]:
    os.environ.setdefault("QT_ENABLE_HIGHDPI_SCALING", "1")
    started = time.perf_counter()
    from PyQt6 import QtCore, QtWidgets
    import_ms = (time.perf_counter() - started) * 1000.0

    started = time.perf_counter()
    app = QtWidgets.QApplication.instance() or QtWidgets.QApplication([])
    window = QtWidgets.QMainWindow()
    window.setWindowTitle("GUI framework benchmark")
    window.resize(1000, 720)
    scroll = QtWidgets.QScrollArea()
    scroll.setWidgetResizable(True)
    content = QtWidgets.QWidget()
    layout = QtWidgets.QVBoxLayout(content)
    layout.setContentsMargins(8, 8, 8, 8)
    layout.setSpacing(4)
    labels: list[Any] = []
    progress: list[Any] = []
    for index in range(args.rows):
        row = QtWidgets.QWidget()
        row_layout = QtWidgets.QHBoxLayout(row)
        row_layout.setContentsMargins(0, 0, 0, 0)
        row_layout.setSpacing(6)
        label = QtWidgets.QLabel(f"Row {index:04d}")
        label.setFixedWidth(130)
        field = QtWidgets.QLineEdit(f"Value {index}")
        field.setFixedWidth(280)
        button = QtWidgets.QPushButton("Action")
        button.setFixedWidth(100)
        bar = QtWidgets.QProgressBar()
        bar.setRange(0, 100)
        bar.setValue(index % 100)
        bar.setFixedWidth(220)
        row_layout.addWidget(label)
        row_layout.addWidget(field)
        row_layout.addWidget(button)
        row_layout.addWidget(bar)
        row_layout.addStretch(1)
        layout.addWidget(row)
        labels.append(label)
        progress.append(bar)
    layout.addStretch(1)
    scroll.setWidget(content)
    window.setCentralWidget(scroll)
    build_ms = (time.perf_counter() - started) * 1000.0

    first_started = time.perf_counter()
    window.show()
    app.processEvents(QtCore.QEventLoop.ProcessEventsFlag.AllEvents)
    window.repaint()
    app.processEvents(QtCore.QEventLoop.ProcessEventsFlag.AllEvents)
    first_event_ms = (time.perf_counter() - first_started) * 1000.0
    rss = _rss_bytes()
    rss_runtime_samples = [rss] if rss is not None else []
    frame_samples: list[float] = []
    update_samples: list[float] = []
    run_started = time.perf_counter()
    for frame in range(args.frames):
        frame_started = time.perf_counter()
        if frame < args.updates:
            update_started = time.perf_counter()
            for index, label in enumerate(labels):
                label.setText(f"Row {index:04d} / tick {frame:04d}")
            value = frame % 100
            for bar in progress:
                bar.setValue(value)
            update_samples.append((time.perf_counter() - update_started) * 1000.0)
        scroll.viewport().update()
        window.repaint()
        app.processEvents(QtCore.QEventLoop.ProcessEventsFlag.AllEvents)
        if frame % 15 == 0:
            value = _rss_bytes()
            if value is not None:
                rss_runtime_samples.append(value)
        frame_samples.append((time.perf_counter() - frame_started) * 1000.0)
        _pace(frame_started)
    run_wall_ms = (time.perf_counter() - run_started) * 1000.0
    window.close()
    app.processEvents()
    report = _base_report("PyQt6", _version("PyQt6", "unknown"), args, import_ms)
    report.update(
        {
            "build_ms": build_ms,
            "first_event_ms": first_event_ms,
            "run_wall_ms": run_wall_ms,
            "rss_before_run_bytes": rss,
            "rss_after_run_bytes": _rss_bytes(),
            "rss_runtime_median_bytes": _percentile(rss_runtime_samples, 0.5),
            "rss_runtime_peak_bytes": max(rss_runtime_samples, default=0),
            "rss_runtime_last_bytes": rss_runtime_samples[-1] if rss_runtime_samples else 0,
            "active_frame_ms": _timings(frame_samples),
            "completed_update_frames": min(args.frames, args.updates),
            "update_apply_ms": _timings(update_samples),
        }
    )
    return report


def run_dearpygui(args: argparse.Namespace) -> dict[str, Any]:
    sys.path.insert(0, str(ROOT / "artifacts" / "benchmark-deps"))
    started = time.perf_counter()
    import dearpygui.dearpygui as dpg
    import_ms = (time.perf_counter() - started) * 1000.0

    started = time.perf_counter()
    dpg.create_context()
    dpg.create_viewport(title="GUI framework benchmark", width=1000, height=720)
    labels: list[int | str] = []
    progress: list[int | str] = []
    with dpg.window(tag="primary", label="GUI framework benchmark"):
        with dpg.child_window(autosize_x=True, autosize_y=True):
            for index in range(args.rows):
                with dpg.group(horizontal=True, horizontal_spacing=6):
                    labels.append(dpg.add_text(f"Row {index:04d}", wrap=130))
                    dpg.add_input_text(default_value=f"Value {index}", width=280)
                    dpg.add_button(label="Action", width=100)
                    progress.append(
                        dpg.add_progress_bar(
                            default_value=(index % 100) / 100.0,
                            width=220,
                        )
                    )
    dpg.set_primary_window("primary", True)
    dpg.setup_dearpygui()
    dpg.set_viewport_vsync(False)
    build_ms = (time.perf_counter() - started) * 1000.0

    first_started = time.perf_counter()
    dpg.show_viewport()
    dpg.render_dearpygui_frame()
    first_event_ms = (time.perf_counter() - first_started) * 1000.0
    rss = _rss_bytes()
    rss_runtime_samples = [rss] if rss is not None else []
    frame_samples: list[float] = []
    update_samples: list[float] = []
    run_started = time.perf_counter()
    for frame in range(args.frames):
        frame_started = time.perf_counter()
        if frame < args.updates:
            update_started = time.perf_counter()
            for index, label in enumerate(labels):
                dpg.set_value(label, f"Row {index:04d} / tick {frame:04d}")
            value = (frame % 100) / 100.0
            for bar in progress:
                dpg.set_value(bar, value)
            update_samples.append((time.perf_counter() - update_started) * 1000.0)
        dpg.render_dearpygui_frame()
        if frame % 15 == 0:
            value = _rss_bytes()
            if value is not None:
                rss_runtime_samples.append(value)
        frame_samples.append((time.perf_counter() - frame_started) * 1000.0)
        _pace(frame_started)
    run_wall_ms = (time.perf_counter() - run_started) * 1000.0
    after = _rss_bytes()
    dpg.destroy_context()
    report = _base_report("Dear PyGui", _version("dearpygui", "unknown"), args, import_ms)
    report.update(
        {
            "build_ms": build_ms,
            "first_event_ms": first_event_ms,
            "run_wall_ms": run_wall_ms,
            "rss_before_run_bytes": rss,
            "rss_after_run_bytes": after,
            "rss_runtime_median_bytes": _percentile(rss_runtime_samples, 0.5),
            "rss_runtime_peak_bytes": max(rss_runtime_samples, default=0),
            "rss_runtime_last_bytes": rss_runtime_samples[-1] if rss_runtime_samples else 0,
            "active_frame_ms": _timings(frame_samples),
            "completed_update_frames": min(args.frames, args.updates),
            "update_apply_ms": _timings(update_samples),
        }
    )
    return report


def run_tkinter(args: argparse.Namespace) -> dict[str, Any]:
    started = time.perf_counter()
    import tkinter as tk
    from tkinter import ttk
    import_ms = (time.perf_counter() - started) * 1000.0

    started = time.perf_counter()
    root = tk.Tk()
    root.title("GUI framework benchmark")
    root.geometry("1000x720")
    canvas = tk.Canvas(root, highlightthickness=0)
    scrollbar = ttk.Scrollbar(root, orient="vertical", command=canvas.yview)
    content = ttk.Frame(canvas, padding=8)
    canvas.configure(yscrollcommand=scrollbar.set)
    canvas.pack(side="left", fill="both", expand=True)
    scrollbar.pack(side="right", fill="y")
    canvas.create_window((0, 0), window=content, anchor="nw")
    labels: list[Any] = []
    progress: list[Any] = []
    for index in range(args.rows):
        row = ttk.Frame(content)
        row.pack(fill="x", pady=2)
        label = ttk.Label(row, text=f"Row {index:04d}", width=18)
        field = ttk.Entry(row, width=38)
        field.insert(0, f"Value {index}")
        button = ttk.Button(row, text="Action", width=14)
        bar = ttk.Progressbar(row, maximum=100, value=index % 100, length=220)
        label.pack(side="left", padx=(0, 6))
        field.pack(side="left", padx=(0, 6))
        button.pack(side="left", padx=(0, 6))
        bar.pack(side="left")
        labels.append(label)
        progress.append(bar)
    content.update_idletasks()
    canvas.configure(scrollregion=canvas.bbox("all"))
    build_ms = (time.perf_counter() - started) * 1000.0

    first_started = time.perf_counter()
    root.update_idletasks()
    root.update()
    first_event_ms = (time.perf_counter() - first_started) * 1000.0
    rss = _rss_bytes()
    rss_runtime_samples = [rss] if rss is not None else []
    frame_samples: list[float] = []
    update_samples: list[float] = []
    run_started = time.perf_counter()
    for frame in range(args.frames):
        frame_started = time.perf_counter()
        if frame < args.updates:
            update_started = time.perf_counter()
            for index, label in enumerate(labels):
                label.configure(text=f"Row {index:04d} / tick {frame:04d}")
            value = frame % 100
            for bar in progress:
                bar.configure(value=value)
            update_samples.append((time.perf_counter() - update_started) * 1000.0)
        root.update_idletasks()
        root.update()
        if frame % 15 == 0:
            value = _rss_bytes()
            if value is not None:
                rss_runtime_samples.append(value)
        frame_samples.append((time.perf_counter() - frame_started) * 1000.0)
        _pace(frame_started)
    run_wall_ms = (time.perf_counter() - run_started) * 1000.0
    after = _rss_bytes()
    root.destroy()
    report = _base_report("Tkinter", f"Tk {tk.TkVersion}", args, import_ms)
    report.update(
        {
            "build_ms": build_ms,
            "first_event_ms": first_event_ms,
            "run_wall_ms": run_wall_ms,
            "rss_before_run_bytes": rss,
            "rss_after_run_bytes": after,
            "rss_runtime_median_bytes": _percentile(rss_runtime_samples, 0.5),
            "rss_runtime_peak_bytes": max(rss_runtime_samples, default=0),
            "rss_runtime_last_bytes": rss_runtime_samples[-1] if rss_runtime_samples else 0,
            "active_frame_ms": _timings(frame_samples),
            "completed_update_frames": min(args.frames, args.updates),
            "update_apply_ms": _timings(update_samples),
        }
    )
    return report


RUNNERS: dict[str, Callable[[argparse.Namespace], dict[str, Any]]] = {
    "dragongui": run_dragongui,
    "pyqt6": run_pyqt6,
    "dearpygui": run_dearpygui,
    "tkinter": run_tkinter,
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--framework", required=True, choices=sorted(RUNNERS))
    parser.add_argument("--rows", type=int, default=50)
    parser.add_argument("--frames", type=int, default=60)
    parser.add_argument("--updates", type=int, default=0)
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    args.rows = max(0, args.rows)
    args.frames = max(3, args.frames)
    args.updates = min(args.frames, max(0, args.updates))
    report = RUNNERS[args.framework](args)
    payload = json.dumps(report, indent=2, sort_keys=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
    print(payload)
    validation = report.get("validation")
    if args.framework == "dragongui" and validation and not validation.get("passed", False):
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
