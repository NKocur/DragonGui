"""Measure one DragonGUI fixed-baseline initialization stage.

This script is intentionally small because the matrix runner launches it in a
fresh process for every sample.  It separates normal Python startup, the native
extension, the public package, document serialization, and a live WGPU window.
"""

from __future__ import annotations

import argparse
import gc
import importlib
import json
import os
from pathlib import Path
import sys
import threading
import time
import types
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
PYTHON_ROOT = ROOT / "python"
PACKAGE_ROOT = PYTHON_ROOT / "dragongui"
STAGES = ("stdlib", "native", "package", "document", "window")
WINDOW_PROFILES = (
    "minimal",
    "large-window",
    "numpy-large-window",
    "indicators-24",
    "indicators-320",
    "lines-4",
    "telemetry-stage1",
)


def _memory_bytes() -> dict[str, int | None]:
    if sys.platform == "win32":
        import ctypes

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
        full = process.memory_full_info()
        return {
            "rss_bytes": int(process.memory_info().rss),
            "private_bytes": int(getattr(full, "uss", 0)) or None,
        }
    except (ImportError, OSError):
        return {"rss_bytes": None, "private_bytes": None}


def _settled_memory() -> dict[str, int | None]:
    gc.collect()
    time.sleep(0.15)
    return _memory_bytes()


def _load_native_only() -> Any:
    # Loading a package extension normally executes dragongui/__init__.py first.
    # A minimal package shell lets this stage measure only the compiled module.
    package = types.ModuleType("dragongui")
    package.__path__ = [str(PACKAGE_ROOT)]
    package.__package__ = "dragongui"
    sys.modules["dragongui"] = package
    return importlib.import_module("dragongui._dragongui")


def _load_package() -> Any:
    sys.path.insert(0, str(PYTHON_ROOT))
    return importlib.import_module("dragongui")


def _build_window_profile(
    dg: Any,
    profile: str,
    *,
    window_width: int | None = None,
    window_height: int | None = None,
) -> Any:
    compact = profile == "minimal"
    width = int(window_width if window_width is not None else (320 if compact else 1500))
    height = int(window_height if window_height is not None else (240 if compact else 960))
    window = dg.Window(
        f"DG baseline: {profile}",
        width=width,
        height=height,
        style={"overflow": "hidden"},
    )
    if compact or profile in {"large-window", "numpy-large-window"}:
        if profile == "numpy-large-window":
            import numpy  # noqa: F401 - the import footprint is the profile
        dg.Label("baseline")
        return window

    indicator_count = 0
    line_count = 0
    if profile == "indicators-24":
        indicator_count = 24
    elif profile == "indicators-320":
        indicator_count = 320
    elif profile == "lines-4":
        line_count = 4
    elif profile == "telemetry-stage1":
        indicator_count = 24
        line_count = 4

    with dg.VLayout(style={"height": "100%", "padding": 6, "gap": 6}):
        dg.Label(f"{line_count} traces · {indicator_count} indicators", wrap=False)
        with dg.HLayout(style={"flex_grow": 1, "min_height": 0, "gap": 6}):
            if line_count:
                import numpy as np

                with dg.ScrollArea(
                    axis="y",
                    style={"flex_grow": 1, "min_width": 0, "height": "100%", "gap": 6},
                ):
                    with dg.HLayout(style={"height": 850, "gap": 6}):
                        x = np.linspace(0.0, 20.0, 1_024, dtype=np.float32)
                        for channel in range(line_count):
                            y = np.sin(x * (0.3 + channel * 0.02)).astype(np.float32)
                            dg.LinePlot(
                                {"x": x, "y": y},
                                x="x",
                                y="y",
                                label=f"TM trace {channel:02d}",
                                max_points=1_024,
                                show_toolbar=False,
                                style={"width": "25%", "height": "100%"},
                            )
            if indicator_count:
                with dg.ScrollArea(
                    axis="y",
                    style={"width": 300, "height": "100%", "gap": 2, "padding": 3},
                ):
                    for index in range(indicator_count):
                        value = float((index * 13) % 101)
                        with dg.HLayout(style={"height": 22, "gap": 4, "align_items": "center"}):
                            dg.LED(index % 5 != 0, size=9)
                            dg.Label(f"TM-{index:03d}  {value:05.1f}", wrap=False, style={"width": 118})
                            dg.ProgressBar(value / 100.0, style={"width": 142, "height": 8})
    return window


def _live_window_memory(
    dg: Any,
    timeout: float,
    profile: str,
    *,
    window_width: int | None = None,
    window_height: int | None = None,
) -> tuple[dict[str, int | None], dict[str, Any]]:
    app = dg.App(loading_screen=False)
    window = _build_window_profile(
        dg,
        profile,
        window_width=window_width,
        window_height=window_height,
    )
    state: dict[str, Any] = {"ready": False, "error": None, "memory": None}

    def sample_when_ready() -> None:
        deadline = time.monotonic() + timeout
        try:
            while time.monotonic() < deadline:
                handle = getattr(app, "_handle", None)
                try:
                    if handle is not None and handle.latency_probe(timeout_ms=1000):
                        time.sleep(0.5)
                        state["memory"] = _settled_memory()
                        state["ready"] = True
                        handle.request_exit()
                        return
                except (RuntimeError, TimeoutError):
                    # The handle exists slightly before the event loop begins
                    # servicing its command queue.  Readiness is therefore a
                    # bounded retry, not a single probe.
                    pass
                time.sleep(0.02)
            state["error"] = "window readiness timeout"
            app.request_exit()
        except Exception as exc:  # pragma: no cover - diagnostic guard
            state["error"] = f"{type(exc).__name__}: {exc}"
            app.request_exit()

    worker = threading.Thread(target=sample_when_ready, name="dg-baseline-sampler", daemon=True)
    worker.start()
    run_result = app.run(window)
    worker.join(timeout=1.0)
    memory = state["memory"] or _settled_memory()
    details = {
        "window_ready": state["ready"],
        "error": state["error"],
        "run_result_type": type(run_result).__name__,
        "window_profile": profile,
        "window_width_requested": window_width,
        "window_height_requested": window_height,
        "effective_memory_hint": (
            (run_result.get("debug_snapshot") or {})
            .get("gpu", {})
            .get("renderer", {})
            .get("memory_hint")
            if isinstance(run_result, dict)
            else None
        ),
        "renderer": (
            (run_result.get("debug_snapshot") or {}).get("gpu", {}).get("renderer", {})
            if isinstance(run_result, dict)
            else {}
        ),
    }
    return memory, details


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", choices=STAGES, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=15.0)
    parser.add_argument("--memory-hint", choices=("performance", "memory-usage"))
    parser.add_argument("--window-profile", choices=WINDOW_PROFILES, default="minimal")
    parser.add_argument("--window-width", type=int)
    parser.add_argument("--window-height", type=int)
    args = parser.parse_args()
    if args.window_width is not None and args.window_width < 64:
        parser.error("--window-width must be at least 64")
    if args.window_height is not None and args.window_height < 64:
        parser.error("--window-height must be at least 64")

    if args.memory_hint:
        os.environ["DRAGONGUI_WGPU_MEMORY_HINT"] = args.memory_hint

    started = time.perf_counter()
    bootstrap = _settled_memory()
    details: dict[str, Any] = {}

    if args.stage == "stdlib":
        stage_memory = _settled_memory()
    elif args.stage == "native":
        native = _load_native_only()
        details["native_module"] = native.__name__
        stage_memory = _settled_memory()
    else:
        dg = _load_package()
        details["backend"] = dg.backend_info()
        if args.stage == "package":
            stage_memory = _settled_memory()
        else:
            app = dg.App(loading_screen=False)
            window = dg.Window("DG baseline", width=320, height=240)
            if args.stage == "document":
                document = app.document(window)
                details["document_window_type"] = document["window"]["type"]
                stage_memory = _settled_memory()
            else:
                del app, window
                stage_memory, live_details = _live_window_memory(
                    dg,
                    args.timeout,
                    args.window_profile,
                    window_width=args.window_width,
                    window_height=args.window_height,
                )
                details.update(live_details)

    report = {
        "schema": 1,
        "benchmark": "dragongui_fixed_baseline_memory_case",
        "stage": args.stage,
        "memory_hint_requested": args.memory_hint,
        "window_profile": args.window_profile,
        "pid": os.getpid(),
        "python": sys.version,
        "bootstrap_memory": bootstrap,
        "stage_memory": stage_memory,
        "elapsed_ms": (time.perf_counter() - started) * 1000.0,
        "details": details,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(report, sort_keys=True))
    return 0 if not details.get("error") else 2


if __name__ == "__main__":
    raise SystemExit(main())
