"""Run the staged DragonGUI/Dear PyGui/PyQtGraph telemetry matrix."""

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
CASE_RUNNER = ROOT / "benchmarks" / "gui_telemetry_viewer_case.py"
DEFAULT_OUTPUT = ROOT / "artifacts" / "gui-telemetry-viewer-comparison"
FRAMEWORKS = ("dragongui", "dearpygui", "pyqtgraph")
STAGES = ("stage1", "stage2", "stage3", "stage4")


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
        "tick_throughput_hz": ("metrics", "tick_throughput_hz"),
        "measurement_completed_ticks": ("metrics", "measurement_completed_ticks"),
        "measurement_dropped_ticks": ("metrics", "measurement_dropped_ticks"),
        "submit_p50_ms": ("metrics", "submit_ms", "median_ms"),
        "submit_p95_ms": ("metrics", "submit_ms", "p95_ms"),
        "submit_p99_ms": ("metrics", "submit_ms", "p99_ms"),
        "frame_p50_ms": ("metrics", "frame_ms", "median_ms"),
        "frame_p95_ms": ("metrics", "frame_ms", "p95_ms"),
        "frame_p99_ms": ("metrics", "frame_ms", "p99_ms"),
        "frame_deadline_misses": ("metrics", "frame_deadline_misses"),
        "cpu_percent": ("metrics", "process_cpu_percent_one_core"),
        "rss_peak_bytes": ("metrics", "rss_peak_bytes"),
        "measurement_rss_peak_bytes": ("metrics", "measurement_memory", "rss_peak_bytes"),
        "measurement_rss_growth_bytes": ("metrics", "measurement_memory", "rss_growth_bytes"),
        "measurement_private_start_bytes": ("metrics", "measurement_memory", "private_start_bytes"),
        "measurement_private_peak_bytes": ("metrics", "measurement_memory", "private_peak_bytes"),
        "measurement_private_growth_bytes": ("metrics", "measurement_memory", "private_growth_bytes"),
        "line_updates": ("metrics", "line_updates"),
        "indicator_property_updates": ("metrics", "control_updates"),
        "drain_recovery_ms": ("drain_recovery_ms",),
    }
    result: dict[str, Any] = {
        "framework": first["framework"],
        "framework_version": first["framework_version"],
        "stage": first["stage"],
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
        result["frame_p50_ms"] = _median([_nested(r, "native", "runtime", "frame_timings", "work", "p50_ms") for r in reports])
        result["frame_p95_ms"] = _median([_nested(r, "native", "runtime", "frame_timings", "work", "p95_ms") for r in reports])
        result["frame_p99_ms"] = _median([_nested(r, "native", "runtime", "frame_timings", "work", "p99_ms") for r in reports])
        result["native"] = {
            "wall_fps": _median([_nested(r, "native", "runtime", "wall_fps") for r in reports]),
            "command_drain_p95_ms": _median([_nested(r, "native", "runtime", "command_drain", "timing", "p95_ms") for r in reports]),
            "queue_push_p95_ms": _median([_nested(r, "native", "runtime", "command_queue", "push_timing", "p95_ms") for r in reports]),
            "python_task_p95_ms": _median([_nested(r, "native", "runtime", "python", "task_drain_timing", "max_ms") for r in reports]),
            "python_queue_high_water": _median([_nested(r, "native", "runtime", "python", "task_queue_high_water") for r in reports]),
            "tasks_coalesced": _median([_nested(r, "native", "runtime", "python", "tasks_coalesced") for r in reports]),
            "line_source_points": _median([_nested(r, "native", "renderer", "line_plot_renderer", "source_point_count") for r in reports]),
            "line_render_points": _median([_nested(r, "native", "renderer", "line_plot_renderer", "point_count") for r in reports]),
            "layout_diagnostic_count": _median([_nested(r, "native", "layout_diagnostic_count") for r in reports]),
        }
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repetitions", type=int, default=1)
    parser.add_argument("--warmup-seconds", type=float, default=3.0)
    parser.add_argument("--measure-seconds", type=float, default=15.0)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--stage", action="append", choices=STAGES)
    parser.add_argument("--framework", action="append", choices=FRAMEWORKS)
    parser.add_argument("--resume", action="store_true")
    args = parser.parse_args()
    stages = tuple(args.stage or STAGES)
    frameworks = tuple(args.framework or FRAMEWORKS)
    repetitions = max(1, args.repetitions)
    output_dir = args.output_dir.resolve()
    raw_dir = output_dir / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    manifest: list[dict[str, Any]] = []
    matrix_started = time.perf_counter()

    for repetition in range(repetitions):
        for stage_index, stage in enumerate(stages):
            offset = (repetition + stage_index) % len(frameworks)
            ordered = frameworks[offset:] + frameworks[:offset]
            for framework in ordered:
                output = raw_dir / f"{stage}-{framework}-{repetition + 1}.json"
                if args.resume and output.exists():
                    prior = json.loads(output.read_text(encoding="utf-8"))
                    if _nested(prior, "validation", "passed") is True:
                        print(f"[{repetition + 1}/{repetitions}] {stage}: {framework} (reused)", flush=True)
                        grouped[(stage, framework)].append(prior)
                        manifest.append({"stage": stage, "framework": framework, "repetition": repetition + 1, "raw": str(output.relative_to(output_dir)), "reused": True})
                        continue
                command = [
                    sys.executable,
                    str(CASE_RUNNER),
                    "--framework",
                    framework,
                    "--stage",
                    stage,
                    "--warmup-seconds",
                    str(args.warmup_seconds),
                    "--measure-seconds",
                    str(args.measure_seconds),
                    "--output",
                    str(output),
                ]
                print(f"[{repetition + 1}/{repetitions}] {stage}: {framework}", flush=True)
                started = time.perf_counter()
                completed = subprocess.run(
                    command,
                    cwd=ROOT,
                    env={**os.environ, "PYTHONHASHSEED": "0"},
                    capture_output=True,
                    text=True,
                    timeout=max(180, int((args.warmup_seconds + args.measure_seconds) * 5)),
                    check=False,
                )
                elapsed = (time.perf_counter() - started) * 1000.0
                if completed.returncode != 0:
                    print(completed.stdout)
                    print(completed.stderr, file=sys.stderr)
                    return completed.returncode
                report = json.loads(output.read_text(encoding="utf-8"))
                grouped[(stage, framework)].append(report)
                manifest.append({"stage": stage, "framework": framework, "repetition": repetition + 1, "process_wall_ms": elapsed, "raw": str(output.relative_to(output_dir)), "reused": False})

    summary = {
        "schema": 1,
        "benchmark": "live_telemetry_viewer_scaling",
        "method": {
            "fresh_process_per_sample": True,
            "framework_order_rotates": True,
            "target_hz": 30.0,
            "warmup_seconds": args.warmup_seconds,
            "measure_seconds": args.measure_seconds,
            "line_points_per_plot": 1_024,
            "all_plots_and_indicators_update_each_tick": True,
            "dragon_update_mode": "batch",
            "validation_required": True,
        },
        "matrix_wall_ms": (time.perf_counter() - matrix_started) * 1000.0,
        "runs": manifest,
        "results": {
            stage: {framework: _aggregate(grouped[(stage, framework)]) for framework in frameworks}
            for stage in stages
        },
    }
    path = output_dir / "summary.json"
    path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
