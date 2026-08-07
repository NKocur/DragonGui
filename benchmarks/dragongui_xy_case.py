"""Measure one installed-DragonGUI 2D scatter case against the XY workload.

This intentionally imports ``dragongui`` from the active environment rather
than from this repository's ``python`` directory.  Input generation is outside
the render clock, matching XY's published scatter benchmark.
"""

from __future__ import annotations

import argparse
import importlib.metadata
import json
import os
import sys
import threading
import time
from pathlib import Path
from typing import Any

import psutil


SEED = 20260713
SENTINELS = [(4.6, 2.2), (4.9, 2.45), (4.75, 2.05), (5.0, 2.3), (4.55, 2.5)]


class Frame:
    columns = ("x", "y")
    dtypes = ("float32", "float32")

    def __init__(self, x: Any, y: Any) -> None:
        self.x = x
        self.y = y
        self.shape = (len(x), 2)

    def __getitem__(self, column: str) -> Any:
        return getattr(self, column)


def make_data(np: Any, n: int) -> tuple[Any, Any]:
    rng = np.random.default_rng(SEED)
    x = rng.standard_normal(n, dtype=np.float32)
    y = rng.standard_normal(n, dtype=np.float32)
    y *= np.float32(1.2)
    y += x
    y *= np.float32(0.5)
    for index, (sx, sy) in enumerate(SENTINELS[:n]):
        x[index] = np.float32(sx)
        y[index] = np.float32(sy)
    return x, y


def find_scatter_metrics(snapshot: dict[str, Any], widget_id: str) -> dict[str, Any]:
    gpu = snapshot.get("gpu")
    resources = gpu.get("resources") if isinstance(gpu, dict) else None
    if not isinstance(resources, dict):
        return {}
    scatters = resources.get("scatters")
    if isinstance(scatters, dict) and isinstance(scatters.get(widget_id), dict):
        return scatters[widget_id]
    scatter = resources.get("scatter")
    return scatter if isinstance(scatter, dict) else {}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", type=int, required=True)
    parser.add_argument("--mode", choices=("adaptive", "exact"), required=True)
    parser.add_argument("--frames", type=int, default=20)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument(
        "--loading-screen",
        action="store_true",
        help="Include DragonGUI's startup loading frame and renderer reuse path.",
    )
    parser.add_argument(
        "--package-root",
        type=Path,
        help="Use a source/build package root for before-vs-after development runs.",
    )
    args = parser.parse_args()

    if args.package_root is not None:
        sys.path.insert(0, str(args.package_root.resolve()))

    import_t0 = time.perf_counter()
    import numpy as np
    import dragongui as dg
    import_ms = (time.perf_counter() - import_t0) * 1000.0

    package_path = Path(dg.__file__).resolve()
    source_package = Path(__file__).resolve().parents[1] / "python" / "dragongui"
    if args.package_root is None and source_package in package_path.parents:
        raise RuntimeError(f"benchmark must use installed dragongui, got {package_path}")
    if not dg.native_backend_available():
        raise RuntimeError("installed dragongui wheel has no native backend")

    process = psutil.Process()
    stop_sampling = threading.Event()
    peak_rss = process.memory_info().rss

    def sample_memory() -> None:
        nonlocal peak_rss
        while not stop_sampling.wait(0.01):
            try:
                peak_rss = max(peak_rss, process.memory_info().rss)
            except psutil.NoSuchProcess:
                return

    sampler = threading.Thread(target=sample_memory, daemon=True)
    sampler.start()
    x, y = make_data(np, args.n)
    source_rss = process.memory_info().rss

    build_t0 = time.perf_counter()
    app = dg.App(loading_screen=args.loading_screen)
    window = dg.Window("DragonGUI XY benchmark", width=900, height=420)
    with window:
        scatter = dg.ScatterPlot2D(
            Frame(x, y),
            x="x",
            y="y",
            point_size=2.0,
            opacity=1.0,
            auto_point_size=False,
            lod=args.mode == "adaptive",
            lod_threshold=200_000,
            lod_factor=8,
            id="xy-benchmark-scatter",
        )
    public_build_ms = (time.perf_counter() - build_t0) * 1000.0

    previous_smoke = os.environ.get("DRAGONGUI_SMOKE_FRAMES")
    os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(max(1, args.frames))
    render_t0 = time.perf_counter()
    try:
        result = app.run(window)
    finally:
        render_wall_ms = (time.perf_counter() - render_t0) * 1000.0
        if previous_smoke is None:
            os.environ.pop("DRAGONGUI_SMOKE_FRAMES", None)
        else:
            os.environ["DRAGONGUI_SMOKE_FRAMES"] = previous_smoke
        stop_sampling.set()
        sampler.join(timeout=2)
        peak_rss = max(peak_rss, process.memory_info().rss)

    snapshot = result.get("debug_snapshot", {})
    runtime = snapshot.get("runtime", {}) if isinstance(snapshot, dict) else {}
    gpu = snapshot.get("gpu", {}) if isinstance(snapshot, dict) else {}
    renderer = gpu.get("renderer", {}) if isinstance(gpu, dict) else {}
    metrics = find_scatter_metrics(snapshot, scatter.id)
    point_count = metrics.get(
        "last_point_count",
        metrics.get("point_count", metrics.get("points", metrics.get("instances"))),
    )
    readiness = runtime.get("startup_readiness") if isinstance(runtime, dict) else None
    effective_draw_point_count = metrics.get("effective_draw_point_count")
    payload_status = metrics.get("payload_status")
    validation = {
        "application_frame_presented": readiness == "application_frame_presented",
        "native_update_seen": int(metrics.get("updates") or 0) >= 1,
        "payload_ok": payload_status == "Ok",
        "effective_draw_matches_source": effective_draw_point_count == args.n,
        "point_count": point_count,
        "effective_draw_point_count": effective_draw_point_count,
        "payload_status": payload_status,
        "source_rows": args.n,
    }

    payload = {
        "status": (
            "ok"
            if all(
                (
                    validation["application_frame_presented"],
                    validation["native_update_seen"],
                    validation["payload_ok"],
                    validation["effective_draw_matches_source"],
                )
            )
            else "invalid"
        ),
        "library": "dragongui",
        "version": importlib.metadata.version("dragongui"),
        "package_path": str(package_path),
        "python": sys.version.split()[0],
        "n": args.n,
        "mode": args.mode,
        "mode_detail": (
            "stride LOD factor 8 above 200k" if args.mode == "adaptive" else "all points"
        ),
        "source_bytes": int(x.nbytes + y.nbytes),
        "import_ms": import_ms,
        "public_build_ms": public_build_ms,
        "render_wall_ms": render_wall_ms,
        "frames_requested": args.frames,
        "loading_screen": args.loading_screen,
        "frame_ms": result.get("frame_ms"),
        "startup": (
            runtime.get("startup_phases") if isinstance(runtime, dict) else None
        ) or result.get("startup"),
        "frame_timings": runtime.get("frame_timings") if isinstance(runtime, dict) else None,
        "scatter_metrics": metrics,
        "renderer": renderer,
        "source_rss_bytes": source_rss,
        "peak_rss_bytes": peak_rss,
        "validation": validation,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload))


if __name__ == "__main__":
    main()
