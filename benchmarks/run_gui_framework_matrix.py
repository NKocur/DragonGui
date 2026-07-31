"""Run the reproducible DragonGUI/PyQt6/Dear-PyGui/Tkinter matrix."""

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
CASE_RUNNER = ROOT / "benchmarks" / "gui_framework_case.py"
DEFAULT_OUTPUT = ROOT / "artifacts" / "gui-framework-comparison"
FRAMEWORKS = ("dragongui", "pyqt6", "dearpygui", "tkinter")
CASES = {
    "empty_startup": {"rows": 0, "frames": 30, "updates": 0},
    "moderate_controls": {"rows": 50, "frames": 60, "updates": 0},
    "dense_controls": {"rows": 200, "frames": 60, "updates": 0},
    "batched_updates": {"rows": 50, "frames": 120, "updates": 60},
}


def _nested(report: dict[str, Any], *path: str) -> Any:
    value: Any = report
    for key in path:
        if not isinstance(value, dict):
            return None
        value = value.get(key)
    return value


def _median(values: list[Any]) -> float | None:
    numeric = [float(value) for value in values if isinstance(value, (int, float))]
    return statistics.median(numeric) if numeric else None


def _aggregate(reports: list[dict[str, Any]]) -> dict[str, Any]:
    first = reports[0]
    scalar_paths = {
        "import_ms": ("import_ms",),
        "build_ms": ("build_ms",),
        "first_event_ms": ("first_event_ms",),
        "run_wall_ms": ("run_wall_ms",),
        "rss_before_run_bytes": ("rss_before_run_bytes",),
        "rss_after_run_bytes": ("rss_after_run_bytes",),
        "rss_runtime_median_bytes": ("rss_runtime_median_bytes",),
        "rss_runtime_peak_bytes": ("rss_runtime_peak_bytes",),
        "rss_runtime_last_bytes": ("rss_runtime_last_bytes",),
        "active_frame_median_ms": ("active_frame_ms", "median_ms"),
        "active_frame_p95_ms": ("active_frame_ms", "p95_ms"),
        "active_frame_max_ms": ("active_frame_ms", "max_ms"),
        "update_apply_median_ms": ("update_apply_ms", "median_ms"),
        "update_apply_p95_ms": ("update_apply_ms", "p95_ms"),
        "completed_update_frames": ("completed_update_frames",),
    }
    summary: dict[str, Any] = {
        "framework": first["framework"],
        "framework_version": first["framework_version"],
        "python": first["python"],
        "platform": first["platform"],
        "rows": first["rows"],
        "logical_controls": first["logical_controls"],
        "frames_requested": first["frames_requested"],
        "update_frames_requested": first["update_frames_requested"],
        "repetitions": len(reports),
    }
    for name, path in scalar_paths.items():
        summary[name] = _median([_nested(report, *path) for report in reports])
    if first["framework"] == "DragonGUI":
        summary["active_frame_median_ms"] = _median(
            [_nested(report, "active_frame_ms", "p50_ms") for report in reports]
        )
        summary["native"] = {
            "widget_count": _median(
                [_nested(report, "native", "widget_count") for report in reports]
            ),
            "layout_issue_count": _median(
                [_nested(report, "native", "layout_issue_count") for report in reports]
            ),
            "frame_work_ms_avg": _median(
                [_nested(report, "native", "frame_work_ms_avg") for report in reports]
            ),
            "wall_fps": _median(
                [_nested(report, "native", "wall_fps") for report in reports]
            ),
            "style_reapply_p50_ms": _median(
                [_nested(report, "native", "style_reapply", "p50_ms") for report in reports]
            ),
            "layout_compute_p50_ms": _median(
                [_nested(report, "native", "layout_compute", "p50_ms") for report in reports]
            ),
            "text_rebuild_p50_ms": _median(
                [_nested(report, "native", "text_rebuild", "p50_ms") for report in reports]
            ),
            "command_drain_p50_ms": _median(
                [
                    _nested(report, "native", "command_drain", "timing", "p50_ms")
                    for report in reports
                ]
            ),
            "command_drain_p95_ms": _median(
                [
                    _nested(report, "native", "command_drain", "timing", "p95_ms")
                    for report in reports
                ]
            ),
        }
        validations = [report.get("validation") or {} for report in reports]
        summary["validation"] = {
            "passed": all(item.get("passed") is True for item in validations),
            "samples": len(validations),
            "checks_per_sample": [item.get("check_count", 0) for item in validations],
            "total_failures": sum(int(item.get("failure_count", 0)) for item in validations),
        }
    return summary


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--framework", action="append", choices=FRAMEWORKS)
    parser.add_argument("--case", action="append", choices=sorted(CASES))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repetitions = max(1, args.repetitions)
    frameworks = tuple(args.framework or FRAMEWORKS)
    cases = tuple(args.case or CASES)
    output_dir = args.output_dir.resolve()
    raw_dir = output_dir / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    run_manifest: list[dict[str, Any]] = []
    matrix_started = time.perf_counter()

    for repetition in range(repetitions):
        # Rotate framework order to reduce systematic warm-machine bias while
        # keeping the matrix deterministic and reproducible.
        offset = repetition % len(frameworks)
        ordered_frameworks = frameworks[offset:] + frameworks[:offset]
        for case_name in cases:
            case = CASES[case_name]
            for framework in ordered_frameworks:
                output_path = raw_dir / f"{case_name}-{framework}-{repetition + 1}.json"
                command = [
                    sys.executable,
                    str(CASE_RUNNER),
                    "--framework",
                    framework,
                    "--rows",
                    str(case["rows"]),
                    "--frames",
                    str(case["frames"]),
                    "--updates",
                    str(case["updates"]),
                    "--output",
                    str(output_path),
                ]
                print(
                    f"[{repetition + 1}/{repetitions}] {case_name}: {framework}",
                    flush=True,
                )
                started = time.perf_counter()
                completed = subprocess.run(
                    command,
                    cwd=ROOT,
                    env={**os.environ, "PYTHONHASHSEED": "0"},
                    capture_output=True,
                    text=True,
                    timeout=180,
                    check=False,
                )
                elapsed_ms = (time.perf_counter() - started) * 1000.0
                if completed.returncode != 0:
                    print(completed.stdout)
                    print(completed.stderr, file=sys.stderr)
                    return completed.returncode
                report = json.loads(output_path.read_text(encoding="utf-8"))
                grouped[(case_name, framework)].append(report)
                run_manifest.append(
                    {
                        "case": case_name,
                        "framework": framework,
                        "repetition": repetition + 1,
                        "process_wall_ms": elapsed_ms,
                        "raw": str(output_path.relative_to(output_dir)),
                    }
                )

    summary = {
        "schema": 1,
        "method": {
            "framework_order_rotates_per_repetition": True,
            "fresh_process_per_sample": True,
            "target_frame_rate": 60,
            "cases": CASES,
        },
        "matrix_wall_ms": (time.perf_counter() - matrix_started) * 1000.0,
        "runs": run_manifest,
        "results": {
            case_name: {
                framework: _aggregate(grouped[(case_name, framework)])
                for framework in frameworks
            }
            for case_name in cases
        },
    }
    summary_path = output_dir / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {summary_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
