"""Run the extended sustained/scaling GUI benchmark matrix."""

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
CONTROL_RUNNER = ROOT / "benchmarks" / "gui_framework_case.py"
SPECIAL_RUNNER = ROOT / "benchmarks" / "gui_sustained_case.py"
DEFAULT_OUTPUT = ROOT / "artifacts" / "gui-sustained-comparison"
FRAMEWORKS = ("dragongui", "dearpygui", "pyqt6", "tkinter")
SPECIAL_FRAMEWORKS = ("dragongui", "dearpygui", "pyqt6")

CASES: tuple[dict[str, Any], ...] = tuple(
    {"family": "control_scale", "scale": rows, "rows": rows, "frames": 60, "updates": 0, "runner": "control"}
    for rows in (10, 50, 200, 500)
) + tuple(
    {"family": "mutation_scale", "scale": rows * 2, "rows": rows, "frames": 120, "updates": 60, "runner": "control"}
    for rows in (10, 50, 200)
) + tuple(
    {"family": workload, "scale": scale, "operations": operations, "runner": "special"}
    for workload, scales, operations in (
        ("restyle", (50, 200), 30),
        ("resize", (50, 200), 30),
        ("line_replace", (1_000, 10_000, 100_000), 30),
        ("table_model", (1_000, 10_000, 100_000), 10),
    )
    for scale in scales
)


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
    if not first.get("supported", True):
        return {
            "framework": first["framework"],
            "framework_version": first["framework_version"],
            "supported": False,
            "support_note": first.get("support_note"),
            "repetitions": len(reports),
        }
    paths = {
        "build_ms": ("build_ms",),
        "data_prepare_ms": ("data_prepare_ms",),
        "rss_before_run_bytes": ("rss_before_run_bytes",),
        "rss_after_run_bytes": ("rss_after_run_bytes",),
        "rss_runtime_last_bytes": ("rss_runtime_last_bytes",),
        "active_frame_p50_ms": ("active_frame_ms", "median_ms"),
        "active_frame_p95_ms": ("active_frame_ms", "p95_ms"),
        "submit_p50_ms": ("submit_ms", "median_ms"),
        "submit_p95_ms": ("submit_ms", "p95_ms"),
        "update_apply_p50_ms": ("update_apply_ms", "median_ms"),
        "update_apply_p95_ms": ("update_apply_ms", "p95_ms"),
        "operations_completed": ("operations_completed",),
        "completed_update_frames": ("completed_update_frames",),
    }
    result: dict[str, Any] = {
        "framework": first["framework"],
        "framework_version": first["framework_version"],
        "supported": True,
        "repetitions": len(reports),
    }
    for name, path in paths.items():
        result[name] = _median([_nested(report, *path) for report in reports])
    # DragonGUI names its native p50 key explicitly rather than median_ms.
    if first["framework"] == "DragonGUI":
        result["active_frame_p50_ms"] = _median([
            _nested(report, "active_frame_ms", "p50_ms") for report in reports
        ])
        result["native"] = {
            "widget_count": _median([_nested(r, "native", "widget_count") for r in reports]),
            "command_drain_p50_ms": _median([
                _nested(r, "native", "command_drain", "timing", "p50_ms") for r in reports
            ]),
            "command_drain_p95_ms": _median([
                _nested(r, "native", "command_drain", "timing", "p95_ms") for r in reports
            ]),
            "style_reapply_p50_ms": _median([
                _nested(r, "native", "style_reapply", "p50_ms") for r in reports
            ]),
            "style_reapply_p95_ms": _median([
                _nested(r, "native", "style_reapply", "p95_ms") for r in reports
            ]),
            "layout_compute_p50_ms": _median([
                _nested(r, "native", "layout_compute", "p50_ms") for r in reports
            ]),
            "layout_compute_p95_ms": _median([
                _nested(r, "native", "layout_compute", "p95_ms") for r in reports
            ]),
            "text_rebuild_p50_ms": _median([
                _nested(r, "native", "text_rebuild", "p50_ms") for r in reports
            ]),
        }
        if first.get("table_validation") is not None:
            result["table_validation"] = first["table_validation"]
        validations = [report.get("validation") or {} for report in reports]
        result["validation"] = {
            "passed": all(item.get("passed") is True for item in validations),
            "samples": len(validations),
            "checks_per_sample": [item.get("check_count", 0) for item in validations],
            "total_failures": sum(int(item.get("failure_count", 0)) for item in validations),
        }
    return result


def _case_id(case: dict[str, Any]) -> str:
    return f"{case['family']}-{case['scale']}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repetitions", type=int, default=2)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--family", action="append", choices=sorted({c["family"] for c in CASES}))
    parser.add_argument(
        "--base-summary",
        type=Path,
        help="Preserve unselected families from an earlier summary while replacing rerun families.",
    )
    args = parser.parse_args()
    repetitions = max(1, args.repetitions)
    selected = tuple(c for c in CASES if not args.family or c["family"] in args.family)
    output_dir = args.output_dir.resolve()
    raw_dir = output_dir / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    grouped: dict[tuple[str, str], list[dict[str, Any]]] = defaultdict(list)
    manifest: list[dict[str, Any]] = []
    matrix_started = time.perf_counter()

    for repetition in range(repetitions):
        for case_index, case in enumerate(selected):
            frameworks = SPECIAL_FRAMEWORKS if case["runner"] == "special" else FRAMEWORKS
            offset = (repetition + case_index) % len(frameworks)
            ordered = frameworks[offset:] + frameworks[:offset]
            for framework in ordered:
                case_id = _case_id(case)
                output = raw_dir / f"{case_id}-{framework}-{repetition + 1}.json"
                if case["runner"] == "control":
                    command = [
                        sys.executable, str(CONTROL_RUNNER), "--framework", framework,
                        "--rows", str(case["rows"]), "--frames", str(case["frames"]),
                        "--updates", str(case["updates"]), "--output", str(output),
                    ]
                else:
                    command = [
                        sys.executable, str(SPECIAL_RUNNER), "--framework", framework,
                        "--workload", case["family"], "--scale", str(case["scale"]),
                        "--operations", str(case["operations"]), "--output", str(output),
                    ]
                print(f"[{repetition + 1}/{repetitions}] {case_id}: {framework}", flush=True)
                started = time.perf_counter()
                completed = subprocess.run(
                    command, cwd=ROOT, env={**os.environ, "PYTHONHASHSEED": "0"},
                    capture_output=True, text=True, timeout=300, check=False,
                )
                elapsed_ms = (time.perf_counter() - started) * 1000.0
                if completed.returncode != 0:
                    print(completed.stdout)
                    print(completed.stderr, file=sys.stderr)
                    return completed.returncode
                report = json.loads(output.read_text(encoding="utf-8"))
                grouped[(case_id, framework)].append(report)
                manifest.append({
                    "case": case_id, "framework": framework, "repetition": repetition + 1,
                    "process_wall_ms": elapsed_ms, "raw": str(output.relative_to(output_dir)),
                })

    results: dict[str, Any] = {}
    for case in selected:
        case_id = _case_id(case)
        frameworks = SPECIAL_FRAMEWORKS if case["runner"] == "special" else FRAMEWORKS
        results[case_id] = {
            "family": case["family"],
            "scale": case["scale"],
            "configuration": case,
            "frameworks": {
                framework: _aggregate(grouped[(case_id, framework)]) for framework in frameworks
            },
        }
    base: dict[str, Any] = {}
    if args.base_summary:
        base = json.loads(args.base_summary.read_text(encoding="utf-8"))
    replaced_families = {case["family"] for case in selected}
    preserved_results = {
        key: value for key, value in (base.get("results") or {}).items()
        if value.get("family") not in replaced_families
    }
    preserved_runs = [
        run for run in (base.get("runs") or [])
        if not any(str(run.get("case", "")).startswith(f"{family}-") for family in replaced_families)
    ]
    summary = {
        "schema": 1,
        "method": {
            "fresh_process_per_sample": True,
            "framework_order_rotates_per_case_and_repetition": True,
            "target_frame_rate": 60,
            "repetitions": repetitions,
            "cases": list(CASES) if args.base_summary else list(selected),
            "principal_focus": "post-readiness sustained work; startup is retained only as context",
        },
        "matrix_wall_ms": (time.perf_counter() - matrix_started) * 1000.0,
        "runs": preserved_runs + manifest,
        "results": {**preserved_results, **results},
    }
    path = output_dir / "summary.json"
    path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
