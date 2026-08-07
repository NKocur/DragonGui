"""Validate bounded density/adaptive scatter pixels and stable readback."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import threading
import time
from collections import Counter
from pathlib import Path
from typing import Any


def wait_for_ready(app: Any, timeout_s: float = 15.0) -> dict[str, Any]:
    deadline = time.perf_counter() + timeout_s
    while time.perf_counter() < deadline:
        try:
            snapshot = app.debug_snapshot(timeout_ms=3000)
        except RuntimeError:
            time.sleep(0.01)
            continue
        if snapshot.get("runtime", {}).get("startup_readiness") == "application_frame_presented":
            return snapshot
        time.sleep(0.01)
    raise TimeoutError("application frame was not presented before capture")


def window_non_background(
    np: Any,
    rgba: Any,
    background: tuple[int, int, int],
    center: tuple[float, float],
    radius: float,
) -> dict[str, Any]:
    height, width, _ = rgba.shape
    cx = int(round(center[0] * width))
    cy = int(round(center[1] * height))
    rx = max(2, int(round(radius * width)))
    ry = max(2, int(round(radius * height)))
    x0, x1 = max(0, cx - rx), min(width, cx + rx + 1)
    y0, y1 = max(0, cy - ry), min(height, cy + ry + 1)
    rgb = rgba[y0:y1, x0:x1, :3].astype(np.int16)
    bg = np.asarray(background, dtype=np.int16)
    distance = np.max(np.abs(rgb - bg), axis=2)
    changed = distance >= 18
    return {
        "rect": [x0, y0, x1, y1],
        "pixels": int(changed.sum()),
        "max_channel_delta": int(distance.max()) if distance.size else 0,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=("density", "adaptive"), required=True)
    parser.add_argument("--n", type=int, default=1_000_000)
    parser.add_argument("--captures", type=int, default=10)
    parser.add_argument("--frames", type=int, default=260)
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

    # Keep the bulk distribution away from the extrema so each corner sentinel
    # occupies an isolated density cell and remains independently observable.
    x = np.linspace(-0.55, 0.55, args.n, dtype=np.float32)
    y = np.sin(x * np.float32(35.0), dtype=np.float32) * np.float32(0.45)
    x[:5] = (-1.0, -1.0, 1.0, 1.0, 0.0)
    y[:5] = (-1.0, 1.0, -1.0, 1.0, 0.0)
    store = dg.PointStore(x, y, ownership="borrowed")

    app = dg.App(loading_screen=False)
    window = dg.Window(f"Scatter {args.mode} correct-and-stable gate", width=720, height=720)
    with window:
        plot = dg.ScatterPlot2D(
            store,
            rendering=args.mode,
            point_size=12.0,
            auto_point_size=False,
            grid=False,
            id="density-correct-stable-scatter",
        )

    capture_result: dict[str, Any] = {}
    producer_errors: list[str] = []

    def capture_worker() -> None:
        try:
            wait_for_ready(app)
            time.sleep(0.15)
            captures = []
            capture_ms = []
            for _ in range(args.captures):
                t0 = time.perf_counter()
                image = plot.screenshot()
                capture_ms.append((time.perf_counter() - t0) * 1000.0)
                if image is None:
                    raise RuntimeError("scatter screenshot API returned None")
                captures.append(image)
            final_snapshot = app.debug_snapshot(timeout_ms=3000)
            first = captures[0]
            height, width, channels = first.shape
            hashes = [hashlib.sha256(image.tobytes()).hexdigest() for image in captures]
            opaque_rgb = first[first[:, :, 3] > 240, :3]
            if not len(opaque_rgb):
                raise RuntimeError("capture contains no opaque pixels")
            background = Counter(map(tuple, opaque_rgb.tolist())).most_common(1)[0][0]

            # Camera fit for a square domain in a square viewport places extrema
            # at approximately 15.8% and 84.1%, as established by the exact gate.
            windows = {
                "bottom_left": window_non_background(
                    np, first, background, (0.158, 0.841), 0.035
                ),
                "top_left": window_non_background(
                    np, first, background, (0.158, 0.158), 0.035
                ),
                "bottom_right": window_non_background(
                    np, first, background, (0.841, 0.841), 0.035
                ),
                "top_right": window_non_background(
                    np, first, background, (0.841, 0.158), 0.035
                ),
                "center": window_non_background(
                    np, first, background, (0.5, 0.5), 0.055
                ),
            }
            windows_valid = all(item["pixels"] >= 4 for item in windows.values())

            gpu = final_snapshot.get("gpu", {})
            resources = gpu.get("resources", {}) if isinstance(gpu, dict) else {}
            scatters = resources.get("scatters", {}) if isinstance(resources, dict) else {}
            metrics = scatters.get(plot.id, {}) if isinstance(scatters, dict) else {}
            representation = metrics.get("representation", {}) if isinstance(metrics, dict) else {}
            density = representation.get("density", {}) if isinstance(representation, dict) else {}
            store_source = metrics.get("point_store_source", {}) if isinstance(metrics, dict) else {}
            runtime = final_snapshot.get("runtime", {})
            expected_revision = int(store.stats()["data_revision"])
            runtime_valid = (
                runtime.get("startup_readiness") == "application_frame_presented"
                and int(metrics.get("last_point_count", -1)) == args.n
                and metrics.get("payload_status") == "Ok"
                and representation.get("policy_requested") == args.mode
                and representation.get("policy_effective") == "density"
                and 0 < int(representation.get("render_rows", 0)) <= 65_536
                and density.get("source_rows_conserved") is True
                and int(density.get("finite_source_rows", -1)) == args.n
                and int(density.get("represented_source_rows", -1)) == args.n
                and int(density.get("source_revision", -1)) == expected_revision
                and int(store_source.get("revision", -1)) == expected_revision
                and int(density.get("viewport_job_errors", -1)) == 0
            )
            capture_result.update(
                {
                    "width": width,
                    "height": height,
                    "channels": channels,
                    "rgba_bytes": int(first.nbytes),
                    "hashes": hashes,
                    "unique_hashes": len(set(hashes)),
                    "capture_ms": capture_ms,
                    "background_rgb": list(background),
                    "sentinel_windows": windows,
                    "sentinel_windows_valid": windows_valid,
                    "runtime_valid": runtime_valid,
                    "source_revision": density.get("source_revision"),
                    "policy_requested": representation.get("policy_requested"),
                    "policy_effective": representation.get("policy_effective"),
                    "source_rows": representation.get("source_rows"),
                    "render_rows": representation.get("render_rows"),
                    "represented_source_rows": density.get("represented_source_rows"),
                    "representative_fingerprint": density.get("representative_fingerprint"),
                }
            )
        except Exception as exc:
            producer_errors.append(f"{type(exc).__name__}: {exc}")

    worker = threading.Thread(target=capture_worker, name="density-correct-stable", daemon=True)
    worker.start()
    previous_smoke = os.environ.get("DRAGONGUI_SMOKE_FRAMES")
    os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(max(1, args.frames))
    try:
        run_result = app.run(window)
    finally:
        if previous_smoke is None:
            os.environ.pop("DRAGONGUI_SMOKE_FRAMES", None)
        else:
            os.environ["DRAGONGUI_SMOKE_FRAMES"] = previous_smoke
    worker.join(timeout=15.0)
    if worker.is_alive():
        producer_errors.append("TimeoutError: capture worker did not finish")

    valid = (
        not producer_errors
        and capture_result.get("unique_hashes") == 1
        and capture_result.get("sentinel_windows_valid") is True
        and capture_result.get("runtime_valid") is True
    )
    payload = {
        "status": "ok" if valid else "invalid",
        "contract": "isolated extrema plus central distribution; bounded density conservation; fixed projection windows; ten identical captures",
        "mode": args.mode,
        "rows": args.n,
        "captures_requested": args.captures,
        "capture": capture_result,
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
