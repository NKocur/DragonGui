"""Run the DragonGUI telemetry indicator-family decomposition matrix."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CASE_RUNNER = ROOT / "benchmarks" / "gui_telemetry_indicator_case.py"
DEFAULT_OUTPUT = ROOT / "artifacts" / "gui-telemetry-indicator-decomposition"
MODES = ("labels", "progress", "leds", "combined")
COUNTS = (24, 72, 160, 320)


def _nested(value: dict[str, Any], *path: str) -> Any:
    current: Any = value
    for key in path:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def _summarize(report: dict[str, Any]) -> dict[str, Any]:
    frame_work = _nested(report, "native", "runtime", "frame_timings", "work") or {}
    command_drain = _nested(report, "native", "runtime", "command_drain") or {}
    python_runtime = _nested(report, "native", "runtime", "python") or {}
    batches = _nested(report, "native", "runtime", "python", "native_sends", "batches") or {}
    primitives = _nested(report, "native", "renderer", "primitives") or {}
    rebuilds = primitives.get("retained_rebuilds") or {}
    text = _nested(report, "native", "renderer", "text") or {}
    text_measure = _nested(report, "native", "renderer", "layout_text_measurement") or {}
    metrics = report["metrics"]
    return {
        "mode": report["mode"],
        "count": report["count"],
        "properties_per_tick": report["properties_per_tick"],
        "native_properties_per_completed_tick": report.get("native_properties_per_completed_tick"),
        "widget_count": _nested(report, "native", "renderer", "widget_count"),
        "build_ms": report["build_ms"],
        "tick_throughput_hz": metrics["tick_throughput_hz"],
        "measurement_completed_at_window_end": metrics["measurement_completed_at_window_end"],
        "measurement_dropped_ticks": metrics["measurement_dropped_ticks"],
        "submit_p50_ms": metrics["submit_ms"]["median_ms"],
        "submit_p95_ms": metrics["submit_ms"]["p95_ms"],
        "submit_p99_ms": metrics["submit_ms"]["p99_ms"],
        "cpu_percent": metrics["process_cpu_percent_one_core"],
        "rss_peak_bytes": metrics["rss_peak_bytes"],
        "measurement_rss_peak_bytes": metrics["measurement_memory"]["rss_peak_bytes"],
        "measurement_rss_growth_bytes": metrics["measurement_memory"]["rss_growth_bytes"],
        "frame_work_p50_ms": frame_work.get("p50_ms"),
        "frame_work_p95_ms": frame_work.get("p95_ms"),
        "frame_work_p99_ms": frame_work.get("p99_ms"),
        "command_drain_p50_ms": (command_drain.get("timing") or {}).get("p50_ms"),
        "command_drain_p95_ms": (command_drain.get("timing") or {}).get("p95_ms"),
        "command_apply_p95_ms": (command_drain.get("apply") or {}).get("p95_ms"),
        "rebuild_flush_p95_ms": (command_drain.get("flush_rebuilds") or {}).get("p95_ms"),
        "python_task_drain_max_ms": (python_runtime.get("task_drain_timing") or {}).get("max_ms"),
        "python_queue_high_water": python_runtime.get("task_queue_high_water"),
        "tasks_coalesced": python_runtime.get("tasks_coalesced"),
        "batch_max_updates": batches.get("max_updates"),
        "batch_updates_submitted": batches.get("updates_submitted"),
        "primitive_rect_count": primitives.get("rect_count"),
        "primitive_upload_ms": primitives.get("last_upload_ms"),
        "primitive_encode_ms": primitives.get("last_base_encode_ms"),
        "partial_upload_bytes": rebuilds.get("partial_upload_bytes"),
        "text_entries": text.get("entries"),
        "text_atlas_trims": text.get("atlas_trims"),
        "text_measure_cache_entries": text_measure.get("cache_entries"),
        "text_measure_cache_misses": text_measure.get("cache_misses"),
        "text_measure_capacity_clears": text_measure.get("capacity_clears"),
        "validation": report["validation"],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--warmup-seconds", type=float, default=2.0)
    parser.add_argument("--measure-seconds", type=float, default=8.0)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--mode", action="append", choices=MODES)
    parser.add_argument("--count", action="append", type=int, choices=COUNTS)
    parser.add_argument("--resume", action="store_true")
    args = parser.parse_args()
    modes = tuple(args.mode or MODES)
    counts = tuple(args.count or COUNTS)
    output_dir = args.output_dir.resolve()
    raw_dir = output_dir / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    results: dict[str, dict[str, Any]] = {str(count): {} for count in counts}
    manifest: list[dict[str, Any]] = []
    started_matrix = time.perf_counter()

    for count_index, count in enumerate(counts):
        offset = count_index % len(modes)
        ordered_modes = modes[offset:] + modes[:offset]
        for mode in ordered_modes:
            output = raw_dir / f"{count}-{mode}.json"
            if args.resume and output.exists():
                prior = json.loads(output.read_text(encoding="utf-8"))
                if prior.get("benchmark") == "telemetry_indicator_decomposition" and prior.get("validation"):
                    print(f"{count}: {mode} (reused)", flush=True)
                    results[str(count)][mode] = _summarize(prior)
                    manifest.append({"count": count, "mode": mode, "raw": str(output.relative_to(output_dir)), "reused": True})
                    continue
            command = [
                sys.executable,
                str(CASE_RUNNER),
                "--mode",
                mode,
                "--count",
                str(count),
                "--warmup-seconds",
                str(args.warmup_seconds),
                "--measure-seconds",
                str(args.measure_seconds),
                "--output",
                str(output),
            ]
            print(f"{count}: {mode}", flush=True)
            started = time.perf_counter()
            completed = subprocess.run(
                command,
                cwd=ROOT,
                env={**os.environ, "PYTHONHASHSEED": "0"},
                capture_output=True,
                text=True,
                timeout=max(120, int((args.warmup_seconds + args.measure_seconds) * 5)),
                check=False,
            )
            elapsed = (time.perf_counter() - started) * 1000.0
            # Exit 2 means the case remained correct but missed its real-time
            # deadline validation. That is a capacity result, not a harness
            # failure, so retain it and continue the matrix.
            if completed.returncode not in {0, 2} or not output.exists():
                print(completed.stdout)
                print(completed.stderr, file=sys.stderr)
                return completed.returncode
            report = json.loads(output.read_text(encoding="utf-8"))
            results[str(count)][mode] = _summarize(report)
            manifest.append({"count": count, "mode": mode, "process_wall_ms": elapsed, "raw": str(output.relative_to(output_dir)), "reused": False})

    summary = {
        "schema": 1,
        "benchmark": "telemetry_indicator_decomposition",
        "method": {
            "fresh_process_per_sample": True,
            "case_order_rotates_by_count": True,
            "target_hz": 30.0,
            "warmup_seconds": args.warmup_seconds,
            "measure_seconds": args.measure_seconds,
            "update_mode": "batch",
            "validation_required": True,
        },
        "matrix_wall_ms": (time.perf_counter() - started_matrix) * 1000.0,
        "runs": manifest,
        "results": results,
    }
    path = output_dir / "summary.json"
    path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
