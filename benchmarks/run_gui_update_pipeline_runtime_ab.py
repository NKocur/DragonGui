"""Alternate two DragonGUI runtimes for a validated update-pipeline A/B gate."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CASE = ROOT / "benchmarks" / "gui_update_pipeline_case.py"


def _optional_int_env(name: str) -> int | None:
    try:
        return int(os.environ[name])
    except (KeyError, ValueError):
        return None


def _machine_metadata() -> dict[str, Any]:
    return {
        "label": os.environ.get("DRAGONGUI_BENCH_MACHINE_LABEL"),
        "platform": platform.platform(),
        "machine": platform.machine(),
        "processor": platform.processor() or os.environ.get("PROCESSOR_IDENTIFIER"),
        "logical_cpu_count": os.cpu_count(),
        "gpu": os.environ.get("DRAGONGUI_BENCH_GPU"),
        "physical_memory_bytes": _optional_int_env("DRAGONGUI_BENCH_MEMORY_BYTES"),
        "python_version": platform.python_version(),
        "python_executable": sys.executable,
        "present_mode": os.environ.get("DRAGONGUI_PRESENT_MODE") or "runtime default",
        "requested_scale_factor": os.environ.get("DRAGONGUI_VISUAL_AUDIT_SCALE_FACTOR")
        or "monitor default",
    }


def _nested(value: dict[str, Any], *path: str) -> Any:
    current: Any = value
    for key in path:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def _median(values: list[float]) -> float:
    return statistics.median(values)


def _lower_is_better_overhead(control: float, candidate: float) -> float:
    return (candidate / control - 1.0) * 100.0 if control > 0 else float("inf")


def _higher_is_better_regression(control: float, candidate: float) -> float:
    return (1.0 - candidate / control) * 100.0 if control > 0 else float("inf")


def _metrics(report: dict[str, Any]) -> dict[str, float]:
    return {
        "throughput_hz": float(_nested(report, "metrics", "tick_throughput_hz") or 0.0),
        "command_drain_p95_ms": float(
            _nested(report, "native", "runtime", "command_drain", "timing", "p95_ms") or 0.0
        ),
        "command_apply_p95_ms": float(
            _nested(report, "native", "runtime", "command_drain", "apply", "p95_ms") or 0.0
        ),
        "rebuild_flush_p95_ms": float(
            _nested(
                report,
                "native",
                "runtime",
                "command_drain",
                "flush_rebuilds",
                "p95_ms",
            )
            or 0.0
        ),
        "callback_p95_ms": float(_nested(report, "metrics", "callback_ms", "p95_ms") or 0.0),
        "process_cpu_percent_one_core": float(
            _nested(report, "metrics", "process_cpu_percent_one_core") or 0.0
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--control-path", type=Path, required=True)
    parser.add_argument("--candidate-path", type=Path, required=True)
    parser.add_argument("--control-label", default="control")
    parser.add_argument("--candidate-label", default="candidate")
    parser.add_argument("--scenario", default="intrinsic-text")
    parser.add_argument("--widgets", type=int, default=200)
    parser.add_argument("--warmup-seconds", type=float, default=3.0)
    parser.add_argument("--measure-seconds", type=float, default=30.0)
    parser.add_argument("--target-hz", type=float, default=60.0)
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--max-overhead-percent", type=float, default=5.0)
    parser.add_argument("--capture-screenshot", action="store_true")
    args = parser.parse_args()

    output_dir = args.output_dir.resolve()
    raw_dir = output_dir / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    runtimes = {
        "control": args.control_path.resolve(),
        "candidate": args.candidate_path.resolve(),
    }
    labels = {
        "control": args.control_label,
        "candidate": args.candidate_label,
    }
    for variant, runtime in runtimes.items():
        if not (runtime / "dragongui" / "_dragongui.pyd").is_file():
            parser.error(f"{variant} runtime is missing dragongui/_dragongui.pyd: {runtime}")

    started = time.perf_counter()
    runs: list[dict[str, Any]] = []
    reports: dict[tuple[int, str], dict[str, Any]] = {}
    repetitions = max(1, args.repetitions)
    for repetition in range(repetitions):
        order = ("control", "candidate")
        if repetition % 2:
            order = tuple(reversed(order))
        for variant in order:
            name = f"r{repetition + 1}-{variant}-{args.scenario}-w{args.widgets}"
            output = raw_dir / f"{name}.json"
            command = [
                sys.executable,
                str(CASE),
                "--scenario",
                args.scenario,
                "--widgets",
                str(max(1, args.widgets)),
                "--warmup-seconds",
                str(max(0.1, args.warmup_seconds)),
                "--measure-seconds",
                str(max(0.2, args.measure_seconds)),
                "--target-hz",
                str(max(1.0, args.target_hz)),
                "--update-mode",
                "batch",
                "--text-invalidation-mode",
                "optimized",
                "--targeted-rebuild-verification",
                "off",
                "--typed-target-diagnostics",
                "optional" if variant == "control" else "required",
                "--output",
                str(output),
                "--quiet",
            ]
            if args.capture_screenshot:
                command.append("--capture-screenshot")
            env = {
                **os.environ,
                "PYTHONHASHSEED": "0",
                "DRAGONGUI_BENCH_PYTHON_PATH": str(runtimes[variant]),
            }
            print(f"[{repetition + 1}/{repetitions}] {labels[variant]} ({variant})", flush=True)
            run_t0 = time.perf_counter()
            completed = subprocess.run(
                command,
                cwd=ROOT,
                env=env,
                capture_output=True,
                text=True,
                timeout=max(120, round(args.measure_seconds * 5)),
                check=False,
            )
            wall_ms = (time.perf_counter() - run_t0) * 1000.0
            if completed.stdout:
                print(completed.stdout.strip())
            if completed.returncode != 0:
                if completed.stderr:
                    print(completed.stderr, file=sys.stderr)
                return completed.returncode
            report = json.loads(output.read_text(encoding="utf-8"))
            if _nested(report, "validation", "passed") is not True:
                print(f"Validation unexpectedly failed: {output}", file=sys.stderr)
                return 2
            reports[(repetition, variant)] = report
            runs.append(
                {
                    "repetition": repetition + 1,
                    "variant": variant,
                    "label": labels[variant],
                    "runtime_path": str(runtimes[variant]),
                    "raw": str(output.relative_to(output_dir)),
                    "process_wall_ms": wall_ms,
                    "metrics": _metrics(report),
                    "equivalence_sha256": _nested(report, "equivalence", "sha256"),
                    "screenshot_sha256": _nested(report, "screenshot", "sha256"),
                }
            )

    metric_names = tuple(_metrics(next(iter(reports.values()))))
    medians: dict[str, dict[str, float]] = {}
    for variant in ("control", "candidate"):
        medians[variant] = {
            metric: _median(
                [
                    _metrics(reports[(index, variant)])[metric]
                    for index in range(repetitions)
                ]
            )
            for metric in metric_names
        }

    comparisons = {
        "throughput_regression_percent": _higher_is_better_regression(
            medians["control"]["throughput_hz"], medians["candidate"]["throughput_hz"]
        ),
        "command_drain_overhead_percent": _lower_is_better_overhead(
            medians["control"]["command_drain_p95_ms"],
            medians["candidate"]["command_drain_p95_ms"],
        ),
        "rebuild_flush_overhead_percent": _lower_is_better_overhead(
            medians["control"]["rebuild_flush_p95_ms"],
            medians["candidate"]["rebuild_flush_p95_ms"],
        ),
    }
    hashes_match = all(
        _nested(reports[(index, "control")], "equivalence", "sha256")
        == _nested(reports[(index, "candidate")], "equivalence", "sha256")
        for index in range(repetitions)
    )
    screenshots_match = not args.capture_screenshot or all(
        _nested(reports[(index, "control")], "screenshot", "sha256")
        == _nested(reports[(index, "candidate")], "screenshot", "sha256")
        for index in range(repetitions)
    )
    performance_passed = all(
        value <= args.max_overhead_percent for value in comparisons.values()
    )
    passed = performance_passed and hashes_match and screenshots_match
    summary = {
        "schema": 1,
        "benchmark": "dragongui_update_pipeline_runtime_ab",
        "machine": _machine_metadata(),
        "method": {
            "fresh_process_per_sample": True,
            "alternated_order": True,
            "scenario": args.scenario,
            "widgets": max(1, args.widgets),
            "warmup_seconds": max(0.1, args.warmup_seconds),
            "measure_seconds": max(0.2, args.measure_seconds),
            "target_hz": max(1.0, args.target_hz),
            "repetitions": repetitions,
            "max_overhead_percent": args.max_overhead_percent,
            "capture_screenshot": args.capture_screenshot,
        },
        "runtimes": {
            variant: {"label": labels[variant], "path": str(runtimes[variant])}
            for variant in ("control", "candidate")
        },
        "medians": medians,
        "comparisons": comparisons,
        "correctness": {
            "equivalence_hashes_match": hashes_match,
            "screenshot_hashes_match": screenshots_match,
        },
        "performance_passed": performance_passed,
        "passed": passed,
        "matrix_wall_ms": (time.perf_counter() - started) * 1000.0,
        "runs": runs,
    }
    summary_path = output_dir / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {summary_path}")
    print(f"gate passed={passed} comparisons={comparisons}")
    return 0 if passed else 3


if __name__ == "__main__":
    raise SystemExit(main())
