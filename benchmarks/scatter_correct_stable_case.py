"""Validate planted scatter pixels and ten byte-identical diagnostic captures."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import threading
import time
from pathlib import Path
from typing import Any, Callable


class Frame:
    columns = ("x", "y")
    dtypes = ("float32", "float32")

    def __init__(self, x: Any, y: Any) -> None:
        self.x = x
        self.y = y
        self.shape = (len(x), 2)

    def __getitem__(self, name: str) -> Any:
        return getattr(self, name)


def wait_for_ready(app: Any, timeout_s: float = 15.0) -> dict[str, Any]:
    deadline = time.perf_counter() + timeout_s
    while time.perf_counter() < deadline:
        try:
            snapshot = app.debug_snapshot(timeout_ms=3000)
        except RuntimeError:
            time.sleep(0.01)
            continue
        runtime = snapshot.get("runtime", {})
        if runtime.get("startup_readiness") == "application_frame_presented":
            return snapshot
        time.sleep(0.01)
    raise TimeoutError("application frame was not presented before capture")


def color_mask(np: Any, rgba: Any, predicate: Callable[[Any, Any, Any], Any]) -> Any:
    rgb = rgba[:, :, :3].astype(np.int16)
    return predicate(rgb[:, :, 0], rgb[:, :, 1], rgb[:, :, 2]) & (rgba[:, :, 3] > 200)


def mask_summary(np: Any, mask: Any, width: int, height: int) -> dict[str, Any]:
    ys, xs = np.nonzero(mask)
    return {
        "pixels": int(len(xs)),
        "centroid": [float(xs.mean()), float(ys.mean())] if len(xs) else None,
        "normalized_centroid": (
            [float(xs.mean() / width), float(ys.mean() / height)] if len(xs) else None
        ),
    }


def in_window(summary: dict[str, Any], x_range: tuple[float, float], y_range: tuple[float, float]) -> bool:
    point = summary.get("normalized_centroid")
    return (
        int(summary.get("pixels", 0)) >= 8
        and isinstance(point, list)
        and x_range[0] <= float(point[0]) <= x_range[1]
        and y_range[0] <= float(point[1]) <= y_range[1]
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--frames", type=int, default=240)
    parser.add_argument("--captures", type=int, default=10)
    args = parser.parse_args()
    if args.captures < 10:
        parser.error("--captures must be at least 10")

    sys.path.insert(0, str(args.package_root.resolve()))
    import numpy as np
    import dragongui as dg

    # Bottom-left red, top-left green, bottom-right blue, top-right yellow,
    # center magenta. Broad fixed windows tolerate aspect-preserving camera fit.
    x = np.asarray([-1.0, -1.0, 1.0, 1.0, 0.0], dtype=np.float32)
    y = np.asarray([-1.0, 1.0, -1.0, 1.0, 0.0], dtype=np.float32)
    colors = np.asarray(
        [
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [1.0, 1.0, 0.0, 1.0],
            [1.0, 0.0, 1.0, 1.0],
        ],
        dtype=np.float32,
    )

    app = dg.App(loading_screen=False)
    window = dg.Window("Scatter correct-and-stable gate", width=720, height=720)
    with window:
        plot = dg.ScatterPlot2D(
            Frame(x, y),
            colors=colors,
            point_size=20.0,
            auto_point_size=False,
            grid=False,
            rendering="exact",
            id="correct-stable-scatter",
        )

    capture_result: dict[str, Any] = {}
    producer_error: list[str] = []

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
            masks = {
                "bottom_left_red": color_mask(
                    np, first, lambda r, g, b: (r > 180) & (g < 100) & (b < 100)
                ),
                "top_left_green": color_mask(
                    np, first, lambda r, g, b: (g > 150) & (r < 120) & (b < 120)
                ),
                "bottom_right_blue": color_mask(
                    np, first, lambda r, g, b: (b > 180) & (r < 100) & (g < 120)
                ),
                "top_right_yellow": color_mask(
                    np, first, lambda r, g, b: (r > 150) & (g > 150) & (b < 120)
                ),
                "center_magenta": color_mask(
                    np, first, lambda r, g, b: (r > 150) & (b > 150) & (g < 120)
                ),
            }
            summaries = {
                name: mask_summary(np, mask, width, height) for name, mask in masks.items()
            }
            windows_valid = all(
                (
                    in_window(summaries["bottom_left_red"], (0.0, 0.40), (0.60, 1.0)),
                    in_window(summaries["top_left_green"], (0.0, 0.40), (0.0, 0.40)),
                    in_window(summaries["bottom_right_blue"], (0.60, 1.0), (0.60, 1.0)),
                    in_window(summaries["top_right_yellow"], (0.60, 1.0), (0.0, 0.40)),
                    in_window(summaries["center_magenta"], (0.40, 0.60), (0.40, 0.60)),
                )
            )
            gpu = final_snapshot.get("gpu", {})
            resources = gpu.get("resources", {}) if isinstance(gpu, dict) else {}
            scatters = resources.get("scatters", {}) if isinstance(resources, dict) else {}
            metrics = scatters.get(plot.id, {}) if isinstance(scatters, dict) else {}
            representation = metrics.get("representation", {}) if isinstance(metrics, dict) else {}
            runtime = final_snapshot.get("runtime", {})
            capture_result.update(
                {
                    "width": width,
                    "height": height,
                    "channels": channels,
                    "rgba_bytes": int(first.nbytes),
                    "hashes": hashes,
                    "unique_hashes": len(set(hashes)),
                    "capture_ms": capture_ms,
                    "sentinels": summaries,
                    "sentinel_windows_valid": windows_valid,
                    "runtime_valid": (
                        runtime.get("startup_readiness") == "application_frame_presented"
                        and int(metrics.get("last_point_count", -1)) == 5
                        and metrics.get("payload_status") == "Ok"
                        and representation.get("policy_effective") == "exact"
                        and int(representation.get("render_rows", -1)) == 5
                        and int(metrics.get("updates", 0)) >= 1
                    ),
                    "scene_revision": metrics.get("scene_revision"),
                    "policy_requested": representation.get("policy_requested"),
                    "policy_effective": representation.get("policy_effective"),
                    "render_rows": representation.get("render_rows"),
                }
            )
        except Exception as exc:
            producer_error.append(f"{type(exc).__name__}: {exc}")

    worker = threading.Thread(target=capture_worker, name="scatter-correct-stable", daemon=True)
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
        producer_error.append("TimeoutError: capture worker did not finish")

    valid = (
        not producer_error
        and capture_result.get("unique_hashes") == 1
        and capture_result.get("sentinel_windows_valid") is True
        and capture_result.get("runtime_valid") is True
    )
    payload = {
        "status": "ok" if valid else "invalid",
        "contract": "five planted RGBA sentinels; fixed projection windows; ten consecutive byte-identical scatter captures",
        "captures_requested": args.captures,
        "capture": capture_result,
        "producer_errors": producer_error,
        "frame_ms": run_result.get("frame_ms"),
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload))
    if not valid:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
