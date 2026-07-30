"""Run fresh-process CATHODE performance cases and build a compact comparison.

Examples:

    py -3.12 tools/run_cathode_performance_matrix.py --preset quick
    py -3.12 tools/run_cathode_performance_matrix.py --preset full
    py -3.12 tools/run_cathode_performance_matrix.py --case live-default

Each child process owns one native event loop. Reports are written under
``artifacts/performance/matrix`` by default, along with JSON and Markdown
summaries suitable for comparing future library changes.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
from pathlib import Path
import subprocess
import sys
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
PROFILE_SCRIPT = ROOT / "tools" / "profile_cathode_stress.py"


@dataclass(frozen=True)
class MatrixCase:
    name: str
    tiles: int = 96
    rows: int = 480
    frames: int = 30
    live: bool = True
    disabled_groups: tuple[str, ...] = ()


QUICK_CASES = (
    MatrixCase("static-compact", tiles=16, rows=64, frames=10, live=False),
    MatrixCase("static-default", frames=10, live=False),
    MatrixCase("live-default", frames=30),
    MatrixCase("live-no-scope", frames=30, disabled_groups=("scope",)),
    MatrixCase(
        "live-no-dynamic-text",
        frames=30,
        disabled_groups=("scope", "labels", "log", "core-map", "clock"),
    ),
    MatrixCase("static-large", tiles=768, rows=960, frames=10, live=False),
)

FULL_CASES = QUICK_CASES + (
    MatrixCase("static-xlarge", tiles=2_000, rows=1_500, frames=10, live=False),
    MatrixCase("live-large", tiles=768, rows=960, frames=30),
    MatrixCase("live-no-plot", frames=30, disabled_groups=("plot",)),
    MatrixCase("live-no-bars", frames=30, disabled_groups=("bars",)),
    MatrixCase("live-no-labels", frames=30, disabled_groups=("labels", "clock")),
    MatrixCase("live-no-leds", frames=30, disabled_groups=("leds",)),
)

NIGHTLY_CASES = FULL_CASES + (
    MatrixCase("static-5k-target", tiles=3_600, rows=2_000, frames=15, live=False),
    MatrixCase("live-xlarge", tiles=2_000, rows=1_500, frames=60),
    MatrixCase("live-default-sustained", frames=600),
)

PRESETS = {
    "quick": QUICK_CASES,
    "full": FULL_CASES,
    "nightly": NIGHTLY_CASES,
}


def _nested(report: dict[str, Any], *keys: str, default: Any = None) -> Any:
    value: Any = report
    for key in keys:
        if not isinstance(value, dict):
            return default
        value = value.get(key)
    return default if value is None else value


def _number(report: dict[str, Any], *keys: str) -> float:
    value = _nested(report, *keys, default=0.0)
    return float(value) if isinstance(value, (int, float)) else 0.0


def _case_summary(case: MatrixCase, report: dict[str, Any]) -> dict[str, Any]:
    return {
        "case": case.name,
        "widgets": int(_number(report, "workload", "widget_count")),
        "live": case.live,
        "frames": int(_number(report, "runtime", "frames_rendered")),
        "wall_fps": _number(report, "runtime", "wall_fps"),
        "frame_work_avg_ms": _number(report, "runtime", "frame_work_ms_avg"),
        "frame_work_p95_ms": _number(report, "frame_timings", "work", "p95_ms"),
        "frame_acquire_p95_ms": _number(report, "frame_timings", "acquire", "p95_ms"),
        "style_total_ms": _number(report, "top_framework_timings", "style_reapply", "total_ms"),
        "text_total_ms": _number(report, "top_framework_timings", "rebuild_text", "total_ms"),
        "text_p95_ms": _number(report, "top_framework_timings", "rebuild_text", "p95_ms"),
        "primitive_total_ms": _number(
            report, "top_framework_timings", "rebuild_primitives", "total_ms"
        ),
        "primitive_p95_ms": _number(
            report, "top_framework_timings", "rebuild_primitives", "p95_ms"
        ),
        "drain_total_ms": _number(report, "command_drain", "timing", "total_ms"),
        "queue_end": int(_number(report, "runtime", "command_queue_depth")),
        "dirty_requested": _nested(report, "dirty_rebuilds", "requested", default={}),
        "dirty_executed": _nested(report, "dirty_rebuilds", "executed", default={}),
        "deferred_merges": int(_number(report, "dirty_rebuilds", "deferred_merges")),
        "command_dirty_counts": report.get("command_dirty_counts") or {},
        "partial_text_rebuilds": report.get("partial_text_rebuilds") or {},
        "animation_activity": report.get("animation_activity") or {},
    }


def _format_float(value: Any) -> str:
    return f"{float(value):.2f}"


def _markdown_summary(rows: list[dict[str, Any]]) -> str:
    lines = [
        "# CATHODE Performance Matrix",
        "",
        "| Case | Widgets | Live | FPS | Work avg | Work p95 | Style total | Text total | Primitive total | Drain | Queue |",
        "|---|---:|:---:|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for row in rows:
        lines.append(
            "| {case} | {widgets} | {live} | {fps} | {work_avg} ms | {work_p95} ms | "
            "{style} ms | {text} ms | {primitive} ms | {drain} ms | {queue} |".format(
                case=row["case"],
                widgets=row["widgets"],
                live="yes" if row["live"] else "no",
                fps=_format_float(row["wall_fps"]),
                work_avg=_format_float(row["frame_work_avg_ms"]),
                work_p95=_format_float(row["frame_work_p95_ms"]),
                style=_format_float(row["style_total_ms"]),
                text=_format_float(row["text_total_ms"]),
                primitive=_format_float(row["primitive_total_ms"]),
                drain=_format_float(row["drain_total_ms"]),
                queue=row["queue_end"],
            )
        )
    lines.extend(
        [
            "",
            "Times are fresh-process release measurements on the local machine. "
            "Surface-acquire time is reported in JSON separately because it is usually "
            "presentation pacing rather than CPU work.",
            "",
        ]
    )
    return "\n".join(lines)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preset", choices=sorted(PRESETS), default="quick")
    parser.add_argument("--case", action="append", default=[], help="Run only named cases.")
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=ROOT / "artifacts" / "performance" / "matrix",
    )
    parser.add_argument("--keep-going", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    available = {case.name: case for case in PRESETS[args.preset]}
    if args.case:
        missing = sorted(set(args.case) - set(available))
        if missing:
            raise SystemExit(f"Unknown case(s) for {args.preset}: {', '.join(missing)}")
        cases = [available[name] for name in args.case]
    else:
        cases = list(PRESETS[args.preset])

    args.output_dir.mkdir(parents=True, exist_ok=True)
    summaries: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []
    for index, case in enumerate(cases, 1):
        report_path = args.output_dir / f"{case.name}.json"
        command = [
            sys.executable,
            str(PROFILE_SCRIPT),
            "--tiles",
            str(case.tiles),
            "--rows",
            str(case.rows),
            "--frames",
            str(case.frames),
            "--output",
            str(report_path),
        ]
        if not case.live:
            command.append("--no-live")
        for group in case.disabled_groups:
            command.extend(("--disable-live-group", group))

        print(f"[{index}/{len(cases)}] {case.name}", flush=True)
        started = time.perf_counter()
        completed = subprocess.run(
            command,
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        elapsed_s = time.perf_counter() - started
        if completed.returncode != 0:
            failure = {
                "case": case.name,
                "returncode": completed.returncode,
                "elapsed_s": elapsed_s,
                "stderr": completed.stderr[-4_000:],
            }
            failures.append(failure)
            print(f"  failed ({completed.returncode}) after {elapsed_s:.1f}s", flush=True)
            if not args.keep_going:
                break
            continue

        report = json.loads(report_path.read_text(encoding="utf-8"))
        summary = _case_summary(case, report)
        summaries.append(summary)
        print(
            f"  {summary['widgets']} widgets, {summary['wall_fps']:.1f} FPS, "
            f"{summary['frame_work_p95_ms']:.2f} ms work p95",
            flush=True,
        )

    payload = {
        "schema": 1,
        "preset": args.preset,
        "python": sys.version,
        "cases": summaries,
        "failures": failures,
    }
    (args.output_dir / "matrix-summary.json").write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    (args.output_dir / "matrix-summary.md").write_text(
        _markdown_summary(summaries),
        encoding="utf-8",
    )
    print(f"Wrote {args.output_dir / 'matrix-summary.md'}")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
