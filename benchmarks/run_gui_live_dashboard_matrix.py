"""Run the validated low/medium/high live-dashboard benchmark matrix."""

from __future__ import annotations

import argparse
from collections import defaultdict
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CASE_RUNNER = ROOT / "benchmarks" / "gui_live_dashboard_case.py"
DEFAULT_OUTPUT = ROOT / "artifacts" / "gui-live-dashboard-comparison"
FRAMEWORKS = ("dragongui", "dearpygui", "pyqtgraph")
LOADS = ("low", "medium", "high")


def _nested(value: dict[str, Any], *path: str) -> Any:
    current: Any = value
    for key in path:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def _median(values: list[Any]) -> float | None:
    numeric = [float(value) for value in values if isinstance(value, (int, float))]
    return statistics.median(numeric) if numeric else None


def _aggregate(reports: list[dict[str, Any]]) -> dict[str, Any]:
    first = reports[0]
    paths = {
        "build_ms": ("build_ms",),
        "measurement_wall_s": ("metrics", "measurement_wall_s"),
        "tick_throughput_hz": ("metrics", "tick_throughput_hz"),
        "measurement_completed_ticks": ("metrics", "measurement_completed_ticks"),
        "measurement_completed_at_window_end": ("metrics", "measurement_completed_at_window_end"),
        "measurement_dropped_ticks": ("metrics", "measurement_dropped_ticks"),
        "drain_recovery_ms": ("drain_recovery_ms",),
        "submit_p50_ms": ("metrics", "submit_ms", "median_ms"),
        "submit_p95_ms": ("metrics", "submit_ms", "p95_ms"),
        "submit_p99_ms": ("metrics", "submit_ms", "p99_ms"),
        "frame_p50_ms": ("metrics", "frame_ms", "median_ms"),
        "frame_p95_ms": ("metrics", "frame_ms", "p95_ms"),
        "frame_p99_ms": ("metrics", "frame_ms", "p99_ms"),
        "frame_max_ms": ("metrics", "frame_ms", "max_ms"),
        "frame_deadline_misses": ("metrics", "frame_deadline_misses"),
        "schedule_late_2ms": ("metrics", "schedule_late_2ms"),
        "cpu_percent": ("metrics", "process_cpu_percent_one_core"),
        "rss_start_bytes": ("metrics", "rss_start_bytes"),
        "rss_end_bytes": ("metrics", "rss_end_bytes"),
        "rss_peak_bytes": ("metrics", "rss_peak_bytes"),
        "rss_growth_bytes": ("metrics", "rss_growth_bytes"),
        "rss_growth_bytes_per_minute": ("metrics", "rss_growth_bytes_per_minute"),
        "measurement_rss_start_bytes": ("metrics", "measurement_memory", "rss_start_bytes"),
        "measurement_rss_end_bytes": ("metrics", "measurement_memory", "rss_end_bytes"),
        "measurement_rss_peak_bytes": ("metrics", "measurement_memory", "rss_peak_bytes"),
        "measurement_rss_growth_bytes": ("metrics", "measurement_memory", "rss_growth_bytes"),
        "measurement_rss_growth_bytes_per_minute": (
            "metrics", "measurement_memory", "rss_growth_bytes_per_minute"
        ),
        "measurement_private_start_bytes": (
            "metrics", "measurement_memory", "private_start_bytes"
        ),
        "measurement_private_end_bytes": (
            "metrics", "measurement_memory", "private_end_bytes"
        ),
        "measurement_private_peak_bytes": (
            "metrics", "measurement_memory", "private_peak_bytes"
        ),
        "measurement_private_growth_bytes": (
            "metrics", "measurement_memory", "private_growth_bytes"
        ),
        "measurement_private_growth_bytes_per_minute": (
            "metrics", "measurement_memory", "private_growth_bytes_per_minute"
        ),
        "validation_snapshot_rss_delta_bytes": (
            "final_validation_snapshot_memory", "rss_delta_bytes"
        ),
        "line_updates": ("metrics", "line_updates"),
        "scatter_updates": ("metrics", "scatter_updates"),
        "heat_updates": ("metrics", "heat_updates"),
    }
    result: dict[str, Any] = {
        "framework": first["framework"],
        "framework_version": first["framework_version"],
        "load": first["load"],
        "config": first["config"],
        "repetitions": len(reports),
        "validation": {
            "passed": all(_nested(report, "validation", "passed") is True for report in reports),
            "checks_per_sample": [_nested(report, "validation", "check_count") for report in reports],
            "total_failures": sum(int(_nested(report, "validation", "failure_count") or 0) for report in reports),
        },
    }
    for name, path in paths.items():
        result[name] = _median([_nested(report, *path) for report in reports])
    if first["framework"] == "DragonGUI":
        result["frame_p50_ms"] = _median([
            _nested(report, "native", "runtime", "frame_timings", "work", "p50_ms") for report in reports
        ])
        result["frame_p95_ms"] = _median([
            _nested(report, "native", "runtime", "frame_timings", "work", "p95_ms") for report in reports
        ])
        result["frame_p99_ms"] = _median([
            _nested(report, "native", "runtime", "frame_timings", "work", "p99_ms") for report in reports
        ])
        result["frame_max_ms"] = _median([
            _nested(report, "native", "runtime", "frame_timings", "work", "max_ms") for report in reports
        ])
        result["native"] = {
            "wall_fps": _median([_nested(r, "native", "runtime", "wall_fps") for r in reports]),
            "command_drain_yields": _median([_nested(r, "native", "runtime", "command_drain_yields") for r in reports]),
            "command_drain_p95_ms": _median([_nested(r, "native", "runtime", "command_drain", "timing", "p95_ms") for r in reports]),
            "command_queue_push_p95_ms": _median([_nested(r, "native", "runtime", "command_queue", "push_timing", "p95_ms") for r in reports]),
            "command_queue_pushes": _median([_nested(r, "native", "runtime", "command_queue", "pushes") for r in reports]),
            "command_queue_replacements": _median([_nested(r, "native", "runtime", "command_queue", "replacements") for r in reports]),
            "set_prop_replacements": _median([_nested(r, "native", "runtime", "command_queue", "replacements_by_family", "set_prop") for r in reports]),
            "python_queue_high_water": _median([_nested(r, "native", "runtime", "python", "task_queue_high_water") for r in reports]),
            "python_tasks_coalesced": _median([_nested(r, "native", "runtime", "python", "tasks_coalesced") for r in reports]),
            "python_native_sends": _median([_nested(r, "native", "runtime", "python", "native_sends", "requested") for r in reports]),
            "python_native_send_avg_ms": _median([_nested(r, "native", "runtime", "python", "native_sends", "timing", "avg_ms") for r in reports]),
            "line_source_points": _median([_nested(r, "native", "renderer", "line_plot_renderer", "source_point_count") for r in reports]),
            "line_decimated_points": _median([_nested(r, "native", "renderer", "line_plot_renderer", "point_count") for r in reports]),
            "line_upload_ms": _median([_nested(r, "native", "renderer", "line_plot_renderer", "last_upload_ms") for r in reports]),
        }
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repetitions", type=int, default=1)
    parser.add_argument("--warmup-seconds", type=float, default=5.0)
    parser.add_argument("--measure-seconds", type=float, default=60.0)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--load", action="append", choices=LOADS)
    parser.add_argument("--framework", action="append", choices=FRAMEWORKS)
    parser.add_argument(
        "--update-mode",
        choices=("individual", "batch"),
        default="individual",
        help="DragonGUI property transport mode (default: individual).",
    )
    parser.add_argument("--resume", action="store_true", help="Reuse existing raw samples that passed validation")
    args = parser.parse_args()
    loads = tuple(args.load or LOADS)
    frameworks = tuple(args.framework or FRAMEWORKS)
    repetitions = max(1, args.repetitions)
    output_dir = args.output_dir.resolve()
    raw_dir = output_dir / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    manifest: list[dict[str, Any]] = []
    matrix_started = time.perf_counter()

    for repetition in range(repetitions):
        for load_index, load in enumerate(loads):
            offset = (repetition + load_index) % len(frameworks)
            ordered = frameworks[offset:] + frameworks[:offset]
            for framework in ordered:
                output = raw_dir / f"{load}-{framework}-{repetition + 1}.json"
                if args.resume and output.exists():
                    prior = json.loads(output.read_text(encoding="utf-8"))
                    if _nested(prior, "validation", "passed") is True:
                        print(f"[{repetition + 1}/{repetitions}] {load}: {framework} (reused)", flush=True)
                        grouped[(load, framework)].append(prior)
                        manifest.append({
                            "load": load, "framework": framework, "repetition": repetition + 1,
                            "process_wall_ms": None, "raw": str(output.relative_to(output_dir)),
                            "reused": True,
                        })
                        continue
                command = [
                    sys.executable, str(CASE_RUNNER), "--framework", framework,
                    "--load", load, "--warmup-seconds", str(args.warmup_seconds),
                    "--measure-seconds", str(args.measure_seconds), "--output", str(output),
                ]
                if framework == "dragongui":
                    command.extend(["--update-mode", args.update_mode])
                print(f"[{repetition + 1}/{repetitions}] {load}: {framework}", flush=True)
                started = time.perf_counter()
                completed = subprocess.run(
                    command, cwd=ROOT, env={**os.environ, "PYTHONHASHSEED": "0"},
                    capture_output=True, text=True, timeout=max(300, int(args.measure_seconds * 4)),
                    check=False,
                )
                elapsed = (time.perf_counter() - started) * 1000.0
                if completed.returncode != 0:
                    print(completed.stdout)
                    print(completed.stderr, file=sys.stderr)
                    return completed.returncode
                report = json.loads(output.read_text(encoding="utf-8"))
                grouped[(load, framework)].append(report)
                manifest.append({
                    "load": load, "framework": framework, "repetition": repetition + 1,
                    "process_wall_ms": elapsed, "raw": str(output.relative_to(output_dir)),
                    "reused": False,
                })

    summary = {
        "schema": 1,
        "benchmark": "live_visualization_dashboard",
        "method": {
            "fresh_process_per_sample": True,
            "framework_order_rotates": True,
            "target_hz": 60.0,
            "warmup_seconds": args.warmup_seconds,
            "measure_seconds": args.measure_seconds,
            "dragon_update_mode": args.update_mode,
            "synchronous_adapters_drop_overdue_scheduled_ticks": True,
            "dragon_uses_latest_frame_task_coalescing": True,
            "dragon_exits_after_producer_completion_and_queue_settle": True,
            "validation_required": True,
        },
        "matrix_wall_ms": (time.perf_counter() - matrix_started) * 1000.0,
        "runs": manifest,
        "results": {
            load: {framework: _aggregate(grouped[(load, framework)]) for framework in frameworks}
            for load in loads
        },
    }
    path = output_dir / "summary.json"
    path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
