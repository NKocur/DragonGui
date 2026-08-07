"""Measure one fresh-process DragonGUI source-ready-to-first-present workload."""

from __future__ import annotations

import argparse
import importlib.metadata
import json
import os
import sys
import time
from pathlib import Path
from typing import Any


def scatter_metrics(snapshot: dict[str, Any], widget_id: str) -> dict[str, Any]:
    gpu = snapshot.get("gpu", {})
    resources = gpu.get("resources", {}) if isinstance(gpu, dict) else {}
    scatters = resources.get("scatters", {}) if isinstance(resources, dict) else {}
    value = scatters.get(widget_id, {}) if isinstance(scatters, dict) else {}
    return value if isinstance(value, dict) else {}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workload", choices=("empty", "exact-100k", "adaptive-1m"), required=True)
    parser.add_argument("--frames", type=int, default=3)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    sys.path.insert(0, str(args.package_root.resolve()))
    import numpy as np
    import dragongui as dg

    n = {"empty": 0, "exact-100k": 100_000, "adaptive-1m": 1_000_000}[args.workload]
    x = y = None
    store = None
    if n:
        x = np.linspace(-1.0, 1.0, n, dtype=np.float32)
        y = np.sin(x * np.float32(20.0), dtype=np.float32)
        store = dg.PointStore(x, y, ownership="borrowed")

    source_ready_t0 = time.perf_counter()
    build_t0 = time.perf_counter()
    app = dg.App(loading_screen=False)
    window = dg.Window("DragonGUI startup gate", width=900, height=600)
    plot = None
    if store is not None:
        with window:
            plot = dg.ScatterPlot2D(
                store,
                rendering="adaptive" if args.workload == "adaptive-1m" else "exact",
                auto_point_size=False,
                id="startup-scatter",
            )
    public_build_ms = (time.perf_counter() - build_t0) * 1000.0

    previous_smoke = os.environ.get("DRAGONGUI_SMOKE_FRAMES")
    os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(max(1, args.frames))
    try:
        result = app.run(window)
    finally:
        run_return_ms = (time.perf_counter() - source_ready_t0) * 1000.0
        if previous_smoke is None:
            os.environ.pop("DRAGONGUI_SMOKE_FRAMES", None)
        else:
            os.environ["DRAGONGUI_SMOKE_FRAMES"] = previous_smoke

    snapshot = result.get("debug_snapshot", {})
    runtime = snapshot.get("runtime", {}) if isinstance(snapshot, dict) else {}
    gpu = snapshot.get("gpu", {}) if isinstance(snapshot, dict) else {}
    renderer = gpu.get("renderer", {}) if isinstance(gpu, dict) else {}
    native_startup = runtime.get("startup_phases", {}) if isinstance(runtime, dict) else {}
    python_runtime = runtime.get("python", {}) if isinstance(runtime, dict) else {}
    python_startup = python_runtime.get("startup", {}) if isinstance(python_runtime, dict) else {}
    pre_native_ms = float(python_startup.get("pre_native_total_ms", 0.0))
    first_present_ms = float(native_startup.get("first_application_present_ms", 0.0))
    source_ready_to_first_present_ms = public_build_ms + pre_native_ms + first_present_ms

    metrics = scatter_metrics(snapshot, plot.id) if plot is not None else {}
    representation = metrics.get("representation", {}) if isinstance(metrics, dict) else {}
    readiness_valid = runtime.get("startup_readiness") == "application_frame_presented"
    if args.workload == "empty":
        workload_valid = int(snapshot.get("gpu", {}).get("renderer", {}).get("scatter_count", -1)) == 0
    elif args.workload == "exact-100k":
        workload_valid = (
            int(metrics.get("last_point_count", -1)) == n
            and metrics.get("payload_status") == "Ok"
            and representation.get("policy_effective") == "exact"
            and int(representation.get("render_rows", -1)) == n
        )
    else:
        workload_valid = (
            int(metrics.get("last_point_count", -1)) == n
            and metrics.get("payload_status") == "Ok"
            and representation.get("policy_effective") == "density"
            and 0 < int(representation.get("render_rows", 0)) < n
        )
    timings_valid = pre_native_ms >= 0.0 and first_present_ms > 0.0
    optional_renderers_deferred = (
        renderer.get("line_plot_renderer") is None
        and renderer.get("image_renderer_ready") is False
    )

    payload = {
        "status": (
            "ok"
            if readiness_valid
            and workload_valid
            and timings_valid
            and optional_renderers_deferred
            else "invalid"
        ),
        "workload": args.workload,
        "version": importlib.metadata.version("dragongui"),
        "package_path": str(Path(dg.__file__).resolve()),
        "rows": n,
        "source_bytes": 0 if x is None or y is None else int(x.nbytes + y.nbytes),
        "public_build_ms": public_build_ms,
        "python_pre_native_ms": pre_native_ms,
        "native_first_application_present_ms": first_present_ms,
        "source_ready_to_first_application_present_ms": source_ready_to_first_present_ms,
        "run_return_ms": run_return_ms,
        "native_startup": native_startup,
        "python_startup": python_startup,
        "representation": representation,
        "optional_renderers": {
            "line_plot_ready": renderer.get("line_plot_renderer") is not None,
            "image_ready": renderer.get("image_renderer_ready"),
        },
        "validation": {
            "readiness": readiness_valid,
            "workload": workload_valid,
            "timings": timings_valid,
            "optional_renderers_deferred": optional_renderers_deferred,
        },
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload))
    if payload["status"] != "ok":
        raise SystemExit(1)


if __name__ == "__main__":
    main()
