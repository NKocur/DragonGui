"""Measure native PointStore source-chunk frustum rejection for Scatter3D."""

from __future__ import annotations

import argparse
import json
import os
import sys
import threading
import time
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", type=int, required=True)
    parser.add_argument("--order", choices=("coherent", "random"), required=True)
    parser.add_argument("--chunk-rows", type=int, default=1_048_576)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))
    import numpy as np
    import dragongui as dg

    if args.order == "coherent":
        x = np.linspace(-100.0, 100.0, args.n, dtype=np.float32)
    else:
        x = np.random.default_rng(20260806).uniform(
            -100.0, 100.0, args.n
        ).astype(np.float32)
    y = np.sin(x * np.float32(0.15), dtype=np.float32)
    z = np.cos(x * np.float32(0.11), dtype=np.float32)
    store = dg.PointStore(x, y, z=z, ownership="moved")

    app = dg.App(loading_screen=False)
    window = dg.Window("PointStore native chunk culling", width=900, height=600)
    with window:
        scatter = dg.Scatter3D(
            store,
            x="x",
            y="y",
            z="z",
            lod=False,
            auto_point_size=False,
            id="chunk-culling-scatter",
        )

    camera_applied = threading.Event()
    worker_error: list[str] = []

    def focus_camera() -> None:
        deadline = time.monotonic() + 10.0
        while app._handle is None and time.monotonic() < deadline:
            time.sleep(0.005)
        if app._handle is None:
            worker_error.append("app handle was not bound")
            return

        def apply() -> None:
            scatter.set_camera(
                {
                    "target": [-90.0, 0.0, 0.0],
                    "distance": 5.0,
                    "yaw": 0.0,
                    "pitch": 0.0,
                    "parallel": True,
                }
            )
            camera_applied.set()

        app.call_soon_threadsafe(apply)

    worker = threading.Thread(target=focus_camera, daemon=True)
    worker.start()
    previous_smoke = os.environ.get("DRAGONGUI_SMOKE_FRAMES")
    previous_chunk_rows = os.environ.get("DRAGONGUI_SCATTER_POINT_STORE_CHUNK_ROWS")
    os.environ["DRAGONGUI_SMOKE_FRAMES"] = "120"
    os.environ["DRAGONGUI_SCATTER_POINT_STORE_CHUNK_ROWS"] = str(args.chunk_rows)
    try:
        result = app.run(window)
    finally:
        if previous_smoke is None:
            os.environ.pop("DRAGONGUI_SMOKE_FRAMES", None)
        else:
            os.environ["DRAGONGUI_SMOKE_FRAMES"] = previous_smoke
        if previous_chunk_rows is None:
            os.environ.pop("DRAGONGUI_SCATTER_POINT_STORE_CHUNK_ROWS", None)
        else:
            os.environ["DRAGONGUI_SCATTER_POINT_STORE_CHUNK_ROWS"] = previous_chunk_rows
    worker.join(timeout=1.0)

    snapshot = result.get("debug_snapshot", {})
    scatter_snapshot = (
        snapshot.get("gpu", {})
        .get("resources", {})
        .get("scatters", {})
        .get("chunk-culling-scatter", {})
    )
    representation = scatter_snapshot.get("representation", {})
    chunk_count = int(representation.get("chunk_count", 0))
    visible_chunks = int(representation.get("visible_chunk_count", 0))
    visible_rows = int(representation.get("visible_candidate_rows", 0))
    expected_rejection = args.order == "coherent"
    checks = {
        "camera_applied": camera_applied.is_set(),
        "worker_clean": not worker_error,
        "payload_ok": scatter_snapshot.get("payload_status") == "Ok",
        "all_source_rows_retained": representation.get("source_rows") == args.n,
        "source_chunks": chunk_count > 1,
        "stable_offsets": representation.get("chunk_source_offsets", [])
        == list(range(0, args.n, args.chunk_rows)),
        "expected_rejection": (visible_chunks < chunk_count) == expected_rejection,
    }
    payload = {
        "status": "ok" if all(checks.values()) else "invalid",
        "n": args.n,
        "order": args.order,
        "chunk_rows": args.chunk_rows,
        "frame_ms": result.get("frame_ms"),
        "chunk_count": chunk_count,
        "visible_chunk_count": visible_chunks,
        "culled_chunk_count": chunk_count - visible_chunks,
        "visible_candidate_rows": visible_rows,
        "candidate_ratio": visible_rows / args.n,
        "representation": representation,
        "checks": checks,
        "worker_error": worker_error,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload))
    if payload["status"] != "ok":
        raise SystemExit(1)


if __name__ == "__main__":
    main()
