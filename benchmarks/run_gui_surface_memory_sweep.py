"""Measure NumPy-free DragonGUI window memory across exact surface sizes."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CASE = ROOT / "benchmarks" / "gui_fixed_baseline_memory_case.py"
DEFAULT_SIZES = ((320, 240), (640, 480), (1024, 768), (1500, 960), (1920, 1080))


def _parse_size(value: str) -> tuple[int, int]:
    try:
        width_text, height_text = value.lower().split("x", 1)
        width, height = int(width_text), int(height_text)
    except (ValueError, TypeError) as exc:
        raise argparse.ArgumentTypeError("size must be WIDTHxHEIGHT") from exc
    if width < 64 or height < 64:
        raise argparse.ArgumentTypeError("surface dimensions must be at least 64")
    return width, height


def _linear_fit(points: list[tuple[int, float]]) -> dict[str, float]:
    count = len(points)
    mean_x = sum(x for x, _ in points) / count
    mean_y = sum(y for _, y in points) / count
    denominator = sum((x - mean_x) ** 2 for x, _ in points)
    if denominator == 0:
        return {"fixed_bytes": mean_y, "bytes_per_pixel": 0.0}
    slope = sum((x - mean_x) * (y - mean_y) for x, y in points) / denominator
    intercept = mean_y - slope * mean_x
    return {"fixed_bytes": intercept, "bytes_per_pixel": slope}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--size", action="append", type=_parse_size)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=20.0)
    args = parser.parse_args()
    sizes = tuple(args.size or DEFAULT_SIZES)
    repetitions = max(1, args.repetitions)
    output_dir = args.output_dir.resolve()
    raw_dir = output_dir / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    samples: dict[str, list[dict[str, Any]]] = {
        f"{width}x{height}": [] for width, height in sizes
    }
    runs: list[dict[str, Any]] = []
    started = time.perf_counter()

    for repetition in range(repetitions):
        offset = repetition % len(sizes)
        order = sizes[offset:] + sizes[:offset]
        for width, height in order:
            key = f"{width}x{height}"
            output = raw_dir / f"{key}-{repetition + 1}.json"
            command = [
                sys.executable,
                str(CASE),
                "--stage", "window",
                "--window-profile", "minimal",
                "--window-width", str(width),
                "--window-height", str(height),
                "--output", str(output),
                "--timeout", str(args.timeout),
            ]
            print(f"{key}: repetition {repetition + 1}/{repetitions}", flush=True)
            completed = subprocess.run(
                command,
                cwd=ROOT,
                env={**os.environ, "PYTHONHASHSEED": "0"},
                capture_output=True,
                text=True,
                timeout=args.timeout + 25.0,
                check=False,
            )
            if completed.returncode != 0 or not output.exists():
                print(completed.stdout)
                print(completed.stderr, file=sys.stderr)
                return completed.returncode or 1
            sample = json.loads(output.read_text(encoding="utf-8"))
            samples[key].append(sample)
            runs.append({
                "size": key,
                "pixels": width * height,
                "repetition": repetition + 1,
                "raw": str(output.relative_to(output_dir)),
            })

    results: dict[str, Any] = {}
    rss_points: list[tuple[int, float]] = []
    private_points: list[tuple[int, float]] = []
    all_ready = True
    for width, height in sizes:
        key = f"{width}x{height}"
        size_samples = samples[key]
        rss = [sample["stage_memory"]["rss_bytes"] for sample in size_samples]
        private = [sample["stage_memory"]["private_bytes"] for sample in size_samples]
        ready = all(sample["details"].get("window_ready") for sample in size_samples)
        all_ready &= ready
        rss_median = statistics.median(rss)
        private_median = statistics.median(private)
        pixels = width * height
        rss_points.append((pixels, rss_median))
        private_points.append((pixels, private_median))
        results[key] = {
            "width": width,
            "height": height,
            "pixels": pixels,
            "all_ready": ready,
            "rss_median_bytes": rss_median,
            "rss_range_bytes": [min(rss), max(rss)],
            "private_median_bytes": private_median,
            "private_range_bytes": [min(private), max(private)],
            "samples": size_samples,
        }

    report = {
        "schema": 1,
        "benchmark": "dragongui_surface_memory_sweep",
        "method": {
            "fresh_process_per_sample": True,
            "numpy_imported": False,
            "rotated_size_order": True,
            "repetitions": repetitions,
            "sizes": [f"{width}x{height}" for width, height in sizes],
        },
        "elapsed_ms": (time.perf_counter() - started) * 1000.0,
        "fits": {
            "rss": _linear_fit(rss_points),
            "private": _linear_fit(private_points),
        },
        "runs": runs,
        "results": results,
    }
    summary = output_dir / "summary.json"
    summary.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {summary}")
    return 0 if all_ready else 2


if __name__ == "__main__":
    raise SystemExit(main())
