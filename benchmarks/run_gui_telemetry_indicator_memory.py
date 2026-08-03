"""Run repeated snapshot-free DragonGUI telemetry-indicator memory samples."""

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
CASE = ROOT / "benchmarks" / "gui_telemetry_indicator_case.py"
MODES = ("labels", "leds", "combined")


def _median(values: list[float]) -> float:
    return statistics.median(values) if values else 0.0


def _sample(report: dict[str, Any], raw: str) -> dict[str, Any]:
    memory = report["metrics"]["measurement_memory"]
    renderer = report["native"]["renderer"]
    text = renderer.get("text") or {}
    measurement = renderer.get("layout_text_measurement") or {}
    primitives = renderer.get("primitives") or {}
    validation = report["validation"]
    return {
        "raw": raw,
        "validation_passed": validation["passed"],
        "validation_failures": validation["failure_count"],
        "throughput_hz": report["metrics"]["tick_throughput_hz"],
        "rss_start_bytes": memory["rss_start_bytes"],
        "rss_end_bytes": memory["rss_end_bytes"],
        "rss_peak_bytes": memory["rss_peak_bytes"],
        "rss_growth_bytes": memory["rss_growth_bytes"],
        "rss_growth_bytes_per_minute": memory["rss_growth_bytes_per_minute"],
        "private_start_bytes": memory["private_start_bytes"],
        "private_end_bytes": memory["private_end_bytes"],
        "private_peak_bytes": memory["private_peak_bytes"],
        "private_growth_bytes": memory["private_growth_bytes"],
        "private_growth_bytes_per_minute": memory["private_growth_bytes_per_minute"],
        "text_entries": text.get("entries"),
        "text_widget_owners": text.get("widget_owners"),
        "atlas_trims": text.get("atlas_trims"),
        "text_measure_cache_entries": measurement.get("cache_entries"),
        "text_measure_cache_misses": measurement.get("cache_misses"),
        "text_measure_capacity_clears": measurement.get("capacity_clears"),
        "primitive_buffer_bytes": primitives.get("buffer_bytes"),
        "primitive_source_bytes": primitives.get("source_bytes"),
        "primitive_rect_count": primitives.get("rect_count"),
    }


def _aggregate(samples: list[dict[str, Any]]) -> dict[str, Any]:
    valid_samples = [sample for sample in samples if sample["validation_passed"]]
    numeric = (
        "throughput_hz",
        "rss_start_bytes",
        "rss_end_bytes",
        "rss_peak_bytes",
        "rss_growth_bytes",
        "rss_growth_bytes_per_minute",
        "private_start_bytes",
        "private_end_bytes",
        "private_peak_bytes",
        "private_growth_bytes",
        "private_growth_bytes_per_minute",
        "text_entries",
        "text_widget_owners",
        "atlas_trims",
        "text_measure_cache_entries",
        "text_measure_cache_misses",
        "text_measure_capacity_clears",
        "primitive_buffer_bytes",
        "primitive_source_bytes",
        "primitive_rect_count",
    )
    medians = {
        key: _median([float(sample[key]) for sample in valid_samples if isinstance(sample.get(key), (int, float))])
        for key in numeric
    }
    return {
        "sample_count": len(samples),
        "valid_sample_count": len(valid_samples),
        "all_valid": len(valid_samples) == len(samples),
        "medians": medians,
        "rss_start_range_bytes": [min(sample["rss_start_bytes"] for sample in valid_samples), max(sample["rss_start_bytes"] for sample in valid_samples)] if valid_samples else [],
        "private_start_range_bytes": [min(sample["private_start_bytes"] for sample in valid_samples), max(sample["private_start_bytes"] for sample in valid_samples)] if valid_samples else [],
        "rss_growth_range_bytes": [min(sample["rss_growth_bytes"] for sample in valid_samples), max(sample["rss_growth_bytes"] for sample in valid_samples)] if valid_samples else [],
        "private_growth_range_bytes": [min(sample["private_growth_bytes"] for sample in valid_samples), max(sample["private_growth_bytes"] for sample in valid_samples)] if valid_samples else [],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", action="append", choices=MODES)
    parser.add_argument("--count", type=int, default=320)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--warmup-seconds", type=float, default=5.0)
    parser.add_argument("--measure-seconds", type=float, default=20.0)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    modes = tuple(args.mode or MODES)
    output_dir = args.output_dir.resolve()
    raw_dir = output_dir / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    samples: dict[str, list[dict[str, Any]]] = {mode: [] for mode in modes}
    manifest: list[dict[str, Any]] = []
    started = time.perf_counter()

    for repetition in range(args.repetitions):
        offset = repetition % len(modes)
        for mode in modes[offset:] + modes[:offset]:
            output = raw_dir / f"{mode}-{repetition + 1}.json"
            command = [
                sys.executable,
                str(CASE),
                "--mode", mode,
                "--count", str(args.count),
                "--warmup-seconds", str(args.warmup_seconds),
                "--measure-seconds", str(args.measure_seconds),
                "--output", str(output),
            ]
            print(f"{mode}: repetition {repetition + 1}/{args.repetitions}", flush=True)
            case_started = time.perf_counter()
            completed = subprocess.run(
                command,
                cwd=ROOT,
                env={**os.environ, "PYTHONHASHSEED": "0"},
                capture_output=True,
                text=True,
                timeout=max(120, int((args.warmup_seconds + args.measure_seconds) * 5)),
                check=False,
            )
            elapsed_ms = (time.perf_counter() - case_started) * 1000.0
            if completed.returncode not in {0, 2} or not output.exists():
                print(completed.stdout)
                print(completed.stderr, file=sys.stderr)
                return completed.returncode or 1
            report = json.loads(output.read_text(encoding="utf-8"))
            sample = _sample(report, str(output.relative_to(output_dir)))
            samples[mode].append(sample)
            manifest.append({"mode": mode, "repetition": repetition + 1, "process_wall_ms": elapsed_ms, "raw": sample["raw"]})

    summary = {
        "schema": 1,
        "benchmark": "telemetry_indicator_memory",
        "method": {
            "fresh_process_per_sample": True,
            "snapshot_free_measurement_window": True,
            "mode_order_rotates_by_repetition": True,
            "count": args.count,
            "repetitions": args.repetitions,
            "warmup_seconds": args.warmup_seconds,
            "measure_seconds": args.measure_seconds,
            "target_hz": 30.0,
        },
        "matrix_wall_ms": (time.perf_counter() - started) * 1000.0,
        "runs": manifest,
        "results": {
            mode: {"samples": mode_samples, "aggregate": _aggregate(mode_samples)}
            for mode, mode_samples in samples.items()
        },
    }
    summary_path = output_dir / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {summary_path}")
    return 0 if all(result["aggregate"]["all_valid"] for result in summary["results"].values()) else 2


if __name__ == "__main__":
    raise SystemExit(main())
