"""Run validated DragonGUI update-pipeline probes in fresh processes."""

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
CASE = ROOT / "benchmarks" / "gui_update_pipeline_case.py"
DEFAULT_OUTPUT = ROOT / "artifacts" / "gui-update-pipeline"

FULL_CASES = (
    ("labels-fixed", 20, 100),
    ("labels-fixed", 200, 100),
    ("labels-fixed", 1000, 100),
    ("intrinsic-text", 20, 100),
    ("intrinsic-text", 200, 100),
    ("composite-text-fixed", 10, 100),
    ("composite-text-intrinsic", 10, 100),
    ("mixed-state", 20, 100),
    ("mixed-state", 200, 100),
    ("same-property-burst", 1, 100),
    ("same-property-burst", 1, 1000),
    ("same-property-burst", 1, 10000),
    ("distinct-property-burst", 20, 2),
    ("distinct-property-burst", 200, 2),
    ("distinct-property-burst", 1000, 2),
    ("ordered-barrier", 20, 2),
)

SMOKE_CASES = (
    ("labels-fixed", 20, 100),
    ("intrinsic-text", 20, 100),
    ("composite-text-fixed", 5, 100),
    ("composite-text-intrinsic", 5, 100),
    ("mixed-state", 10, 100),
    ("same-property-burst", 1, 100),
    ("distinct-property-burst", 20, 2),
    ("ordered-barrier", 10, 2),
)

ACCEPTANCE_CASES = (
    ("labels-fixed", 200, 100),
    ("labels-fixed", 1000, 100),
    ("intrinsic-text", 20, 100),
    ("intrinsic-text", 200, 100),
    ("composite-text-fixed", 10, 100),
    ("composite-text-intrinsic", 10, 100),
    ("mixed-state", 200, 100),
    ("same-property-burst", 1, 10000),
    ("distinct-property-burst", 1000, 2),
    ("ordered-barrier", 20, 2),
)


def _nested(value: dict[str, Any], *path: str) -> Any:
    current: Any = value
    for key in path:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def _filter_cases(
    cases: tuple[tuple[str, int, int], ...], selectors: list[str]
) -> tuple[tuple[str, int, int], ...]:
    if not selectors:
        return cases
    selected: list[tuple[str, int, int]] = []
    matched: set[str] = set()
    for case in cases:
        scenario, widgets, burst_repeats = case
        matches = {
            selector
            for selector in selectors
            if selector
            in {
                scenario,
                f"{scenario}:{widgets}",
                f"{scenario}:{widgets}:{burst_repeats}",
            }
        }
        if matches:
            selected.append(case)
            matched.update(matches)
    unmatched = set(selectors).difference(matched)
    if unmatched:
        available = ", ".join(
            f"{scenario}:{widgets}:{burst_repeats}"
            for scenario, widgets, burst_repeats in cases
        )
        raise ValueError(
            f"unknown --case selector(s): {', '.join(sorted(unmatched))}; available: {available}"
        )
    return tuple(selected)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--warmup-seconds", type=float, default=1.0)
    parser.add_argument("--measure-seconds", type=float, default=10.0)
    parser.add_argument("--target-hz", type=float, default=60.0)
    parser.add_argument("--repetitions", type=int, default=1)
    parser.add_argument(
        "--update-mode",
        choices=("individual", "batch", "both"),
        default="individual",
    )
    parser.add_argument("--smoke", action="store_true")
    parser.add_argument("--acceptance", action="store_true")
    parser.add_argument("--capture-screenshot", action="store_true")
    parser.add_argument(
        "--resume",
        action="store_true",
        help="Reuse existing raw samples that passed validation and capture checks.",
    )
    parser.add_argument("--interaction-probe-ms", type=float, default=0.0)
    parser.add_argument(
        "--case",
        action="append",
        default=[],
        help=(
            "Run scenario, scenario:widget-count, or "
            "scenario:widget-count:burst-repeats; repeatable."
        ),
    )
    args = parser.parse_args()

    output_dir = args.output_dir.resolve()
    raw_dir = output_dir / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    if args.smoke and args.acceptance:
        parser.error("--smoke and --acceptance are mutually exclusive")
    cases = SMOKE_CASES if args.smoke else ACCEPTANCE_CASES if args.acceptance else FULL_CASES
    try:
        cases = _filter_cases(cases, args.case)
    except ValueError as exc:
        parser.error(str(exc))
    reports: list[dict[str, Any]] = []
    manifest: list[dict[str, Any]] = []
    started = time.perf_counter()

    for repetition in range(max(1, args.repetitions)):
        for case_index, (scenario, widgets, burst_repeats) in enumerate(cases):
            modes = (args.update_mode,)
            if args.update_mode == "both":
                modes = ("individual", "batch")
                if (repetition + case_index) % 2:
                    modes = tuple(reversed(modes))
            for update_mode in modes:
                name = (
                    f"{update_mode}-{scenario}-w{widgets}-b{burst_repeats}"
                    f"-r{repetition + 1}"
                )
                output = raw_dir / f"{name}.json"
                if args.resume and output.exists():
                    prior = json.loads(output.read_text(encoding="utf-8"))
                    capture_ready = not args.capture_screenshot or bool(
                        _nested(prior, "screenshot", "sha256")
                    )
                    if _nested(prior, "validation", "passed") is True and capture_ready:
                        print(
                            f"[{repetition + 1}/{max(1, args.repetitions)}] {name} (reused)",
                            flush=True,
                        )
                        reports.append(prior)
                        manifest.append(
                            {
                                "name": name,
                                "scenario": scenario,
                                "widgets": widgets,
                                "burst_repeats": burst_repeats,
                                "repetition": repetition + 1,
                                "update_mode": update_mode,
                                "process_wall_ms": None,
                                "raw": str(output.relative_to(output_dir)),
                                "reused": True,
                            }
                        )
                        continue
                command = [
                    sys.executable,
                    str(CASE),
                    "--scenario",
                    scenario,
                    "--widgets",
                    str(widgets),
                    "--burst-repeats",
                    str(burst_repeats),
                    "--warmup-seconds",
                    str(args.warmup_seconds),
                    "--measure-seconds",
                    str(args.measure_seconds),
                    "--target-hz",
                    str(args.target_hz),
                    "--update-mode",
                    update_mode,
                    "--output",
                    str(output),
                    "--quiet",
                ]
                if args.interaction_probe_ms > 0:
                    command.extend(
                        ["--interaction-probe-ms", str(args.interaction_probe_ms)]
                    )
                if args.capture_screenshot:
                    command.append("--capture-screenshot")
                print(f"[{repetition + 1}/{max(1, args.repetitions)}] {name}", flush=True)
                case_t0 = time.perf_counter()
                completed = subprocess.run(
                    command,
                    cwd=ROOT,
                    env={**os.environ, "PYTHONHASHSEED": "0"},
                    capture_output=True,
                    text=True,
                    timeout=max(120, round(args.measure_seconds * 5)),
                    check=False,
                )
                wall_ms = (time.perf_counter() - case_t0) * 1000.0
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
                reports.append(report)
                manifest.append(
                    {
                        "name": name,
                        "scenario": scenario,
                        "widgets": widgets,
                        "burst_repeats": burst_repeats,
                        "repetition": repetition + 1,
                        "update_mode": update_mode,
                        "process_wall_ms": wall_ms,
                        "raw": str(output.relative_to(output_dir)),
                        "reused": False,
                    }
                )

    comparisons: list[dict[str, Any]] = []
    comparison_passed = True
    if args.update_mode == "both":
        grouped: dict[tuple[str, int, int, int], dict[str, dict[str, Any]]] = {}
        for report, run in zip(reports, manifest, strict=True):
            key = (
                run["scenario"],
                run["widgets"],
                run["burst_repeats"],
                run["repetition"],
            )
            grouped.setdefault(key, {})[run["update_mode"]] = report
        for key, modes in grouped.items():
            individual = modes.get("individual") or {}
            batch = modes.get("batch") or {}
            geometry_equal = _nested(individual, "equivalence", "sha256") == _nested(
                batch, "equivalence", "sha256"
            )
            screenshot_equal = True
            if args.capture_screenshot:
                screenshot_equal = _nested(individual, "screenshot", "sha256") == _nested(
                    batch, "screenshot", "sha256"
                )
            passed = len(modes) == 2 and geometry_equal and screenshot_equal
            comparison_passed &= passed
            comparisons.append(
                {
                    "scenario": key[0],
                    "widgets": key[1],
                    "burst_repeats": key[2],
                    "repetition": key[3],
                    "passed": passed,
                    "retained_geometry_equal": geometry_equal,
                    "screenshot_equal": screenshot_equal,
                    "individual_equivalence_sha256": _nested(
                        individual, "equivalence", "sha256"
                    ),
                    "batch_equivalence_sha256": _nested(batch, "equivalence", "sha256"),
                    "individual_screenshot_sha256": _nested(
                        individual, "screenshot", "sha256"
                    ),
                    "batch_screenshot_sha256": _nested(batch, "screenshot", "sha256"),
                }
            )

    summary = {
        "schema": 1,
        "benchmark": "dragongui_update_pipeline_matrix",
        "method": {
            "fresh_process_per_sample": True,
            "validation_required": True,
            "warmup_seconds": args.warmup_seconds,
            "measure_seconds": args.measure_seconds,
            "target_hz": args.target_hz,
            "smoke": args.smoke,
            "acceptance": args.acceptance,
            "repetitions": max(1, args.repetitions),
            "update_mode": args.update_mode,
            "capture_screenshot": args.capture_screenshot,
            "interaction_probe_ms": args.interaction_probe_ms,
            "case_selectors": args.case,
        },
        "matrix_wall_ms": (time.perf_counter() - started) * 1000.0,
        "runs": manifest,
        "comparisons": {
            "passed": comparison_passed,
            "count": len(comparisons),
            "items": comparisons,
        },
        "results": [
            {
                "scenario": report["scenario"],
                "update_mode": report["config"]["update_mode"],
                "config": report["config"],
                "validation": report["validation"],
                "equivalence_sha256": _nested(report, "equivalence", "sha256"),
                "screenshot_sha256": _nested(report, "screenshot", "sha256"),
                "throughput_hz": _nested(report, "metrics", "tick_throughput_hz"),
                "dropped_ticks": _nested(
                    report, "metrics", "measurement_dropped_or_coalesced_ticks"
                ),
                "callback_p50_ms": _nested(report, "metrics", "callback_ms", "median_ms"),
                "callback_p95_ms": _nested(report, "metrics", "callback_ms", "p95_ms"),
                "interaction_p50_ms": _nested(
                    report, "metrics", "interaction_roundtrip_ms", "median_ms"
                ),
                "interaction_p95_ms": _nested(
                    report, "metrics", "interaction_roundtrip_ms", "p95_ms"
                ),
                "command_drain_p95_ms": _nested(
                    report, "native", "runtime", "command_drain", "timing", "p95_ms"
                ),
                "command_apply_p95_ms": _nested(
                    report, "native", "runtime", "command_drain", "apply", "p95_ms"
                ),
                "rebuild_flush_p95_ms": _nested(
                    report,
                    "native",
                    "runtime",
                    "command_drain",
                    "flush_rebuilds",
                    "p95_ms",
                ),
                "native_sends": _nested(
                    report, "native", "runtime", "python", "native_sends", "requested"
                ),
                "set_prop_sends": _nested(
                    report,
                    "native",
                    "runtime",
                    "python",
                    "native_sends",
                    "methods",
                    "enqueue_set_prop",
                    "requested",
                ),
                "set_props_packets": _nested(
                    report,
                    "native",
                    "runtime",
                    "python",
                    "native_sends",
                    "methods",
                    "enqueue_set_props",
                    "requested",
                ),
                "batch_updates_submitted": _nested(
                    report,
                    "native",
                    "runtime",
                    "python",
                    "native_sends",
                    "batches",
                    "updates_submitted",
                ),
                "batch_duplicates_removed": _nested(
                    report,
                    "native",
                    "runtime",
                    "python",
                    "native_sends",
                    "batches",
                    "duplicates_removed",
                ),
                "queue_pushes": _nested(
                    report, "native", "runtime", "command_queue", "pushes"
                ),
                "queue_replacements": _nested(
                    report, "native", "runtime", "command_queue", "replacements"
                ),
                "queue_push_p95_ms": _nested(
                    report,
                    "native",
                    "runtime",
                    "command_queue",
                    "push_timing",
                    "p95_ms",
                ),
                "queue_high_water": _nested(
                    report, "native", "runtime", "command_queue", "high_water"
                ),
                "targeted_batches": _nested(
                    report,
                    "native",
                    "framework",
                    "command_text_rebuilds",
                    "completed_batches",
                ),
                "targeted_roots": _nested(
                    report,
                    "native",
                    "framework",
                    "command_text_rebuilds",
                    "rebuilt_roots",
                ),
                "targeted_fallbacks": _nested(
                    report,
                    "native",
                    "framework",
                    "command_text_rebuilds",
                    "fallback_batches",
                ),
                "text_invalidation_candidates": _nested(
                    report,
                    "native",
                    "framework",
                    "live_text_invalidation",
                    "candidates",
                ),
                "text_invalidation_fast_path": _nested(
                    report,
                    "native",
                    "framework",
                    "live_text_invalidation",
                    "text_only",
                ),
                "text_invalidation_layout": _nested(
                    report,
                    "native",
                    "framework",
                    "live_text_invalidation",
                    "layout",
                ),
                "text_invalidation_reasons": _nested(
                    report,
                    "native",
                    "framework",
                    "live_text_invalidation",
                    "reasons",
                ),
            }
            for report in reports
        ],
    }
    summary_path = output_dir / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {summary_path}")
    return 0 if comparison_passed else 3


if __name__ == "__main__":
    raise SystemExit(main())
