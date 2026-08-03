"""Run validated DragonGUI update-pipeline probes in fresh processes."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CASE = ROOT / "benchmarks" / "gui_update_pipeline_case.py"
DEFAULT_OUTPUT = ROOT / "artifacts" / "gui-update-pipeline"


def _optional_int_env(name: str) -> int | None:
    value = os.environ.get(name)
    if value is None:
        return None
    try:
        return int(value)
    except ValueError:
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
        "benchmark_python_path": os.environ.get("DRAGONGUI_BENCH_PYTHON_PATH"),
        "present_mode": os.environ.get("DRAGONGUI_PRESENT_MODE") or "runtime default",
        "requested_scale_factor": os.environ.get("DRAGONGUI_VISUAL_AUDIT_SCALE_FACTOR")
        or "monitor default",
    }

FULL_CASES = (
    ("labels-fixed", 20, 100),
    ("labels-fixed", 200, 100),
    ("labels-fixed", 1000, 100),
    ("intrinsic-text", 20, 100),
    ("intrinsic-text", 200, 100),
    ("composite-text-fixed", 10, 100),
    ("composite-text-intrinsic", 10, 100),
    ("state-controls", 8, 100),
    ("plot-chrome", 3, 100),
    ("html-fallback", 2, 100),
    ("html-webview", 2, 100),
    ("semantic-icons", 4, 100),
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
    ("state-controls", 8, 100),
    ("plot-chrome", 3, 100),
    ("html-fallback", 2, 100),
    ("html-webview", 2, 100),
    ("semantic-icons", 4, 100),
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
    ("state-controls", 8, 100),
    ("plot-chrome", 3, 100),
    ("html-fallback", 2, 100),
    ("html-webview", 2, 100),
    ("semantic-icons", 4, 100),
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
    parser.add_argument(
        "--text-invalidation-mode",
        choices=("optimized", "forced-layout", "both"),
        default="optimized",
        help="Compare the optimized retained-text path with a forced-layout control.",
    )
    parser.add_argument(
        "--targeted-rebuild-verification",
        choices=("off", "verify-full"),
        default="off",
        help="Optionally compare every successful targeted rebuild with a full reconstruction.",
    )
    parser.add_argument(
        "--typed-target-diagnostics",
        choices=("required", "optional"),
        default="required",
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
        "--locality-roots",
        type=int,
        default=0,
        help=(
            "For labels-fixed cases, update only this many deterministic random "
            "labels per callback."
        ),
    )
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
            invalidation_modes = (args.text_invalidation_mode,)
            if args.text_invalidation_mode == "both":
                invalidation_modes = ("optimized", "forced-layout")
                if (repetition + case_index) % 2:
                    invalidation_modes = tuple(reversed(invalidation_modes))
            sample_modes = tuple(
                (update_mode, invalidation_mode)
                for update_mode in modes
                for invalidation_mode in invalidation_modes
            )
            for update_mode, invalidation_mode in sample_modes:
                name = (
                    f"{update_mode}-{invalidation_mode}-{scenario}"
                    f"-w{widgets}-b{burst_repeats}"
                    f"-r{repetition + 1}"
                )
                if args.targeted_rebuild_verification != "off":
                    name += f"-{args.targeted_rebuild_verification}"
                if args.locality_roots:
                    name += f"-l{args.locality_roots}"
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
                                "text_invalidation_mode": invalidation_mode,
                                "targeted_rebuild_verification": (
                                    args.targeted_rebuild_verification
                                ),
                                "locality_roots": args.locality_roots,
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
                    "--text-invalidation-mode",
                    invalidation_mode,
                    "--targeted-rebuild-verification",
                    args.targeted_rebuild_verification,
                    "--typed-target-diagnostics",
                    args.typed_target_diagnostics,
                    "--locality-roots",
                    str(args.locality_roots),
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
                        "text_invalidation_mode": invalidation_mode,
                        "targeted_rebuild_verification": (
                            args.targeted_rebuild_verification
                        ),
                        "locality_roots": args.locality_roots,
                        "process_wall_ms": wall_ms,
                        "raw": str(output.relative_to(output_dir)),
                        "reused": False,
                    }
                )

    comparisons: list[dict[str, Any]] = []
    comparison_passed = True
    if args.update_mode == "both":
        grouped: dict[tuple[str, int, int, int, str], dict[str, dict[str, Any]]] = {}
        for report, run in zip(reports, manifest, strict=True):
            key = (
                run["scenario"],
                run["widgets"],
                run["burst_repeats"],
                run["repetition"],
                run["text_invalidation_mode"],
            )
            grouped.setdefault(key, {})[run["update_mode"]] = report
        for key, modes in grouped.items():
            individual = modes.get("individual") or {}
            batch = modes.get("batch") or {}
            individual_equivalence = _nested(individual, "equivalence", "sha256")
            batch_equivalence = _nested(batch, "equivalence", "sha256")
            geometry_equal = bool(individual_equivalence) and (
                individual_equivalence == batch_equivalence
            )
            screenshot_equal = True
            if args.capture_screenshot:
                individual_screenshot = _nested(individual, "screenshot", "sha256")
                batch_screenshot = _nested(batch, "screenshot", "sha256")
                screenshot_equal = bool(individual_screenshot) and (
                    individual_screenshot == batch_screenshot
                )
            passed = len(modes) == 2 and geometry_equal and screenshot_equal
            comparison_passed &= passed
            comparisons.append(
                {
                    "comparison": "update-mode",
                    "scenario": key[0],
                    "widgets": key[1],
                    "burst_repeats": key[2],
                    "repetition": key[3],
                    "text_invalidation_mode": key[4],
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

    if args.text_invalidation_mode == "both":
        grouped_controls: dict[
            tuple[str, int, int, int, str], dict[str, dict[str, Any]]
        ] = {}
        for report, run in zip(reports, manifest, strict=True):
            key = (
                run["scenario"],
                run["widgets"],
                run["burst_repeats"],
                run["repetition"],
                run["update_mode"],
            )
            grouped_controls.setdefault(key, {})[
                run["text_invalidation_mode"]
            ] = report
        for key, modes in grouped_controls.items():
            optimized = modes.get("optimized") or {}
            forced_layout = modes.get("forced-layout") or {}
            optimized_equivalence = _nested(optimized, "equivalence", "sha256")
            forced_equivalence = _nested(forced_layout, "equivalence", "sha256")
            geometry_equal = bool(optimized_equivalence) and (
                optimized_equivalence == forced_equivalence
            )
            screenshot_equal = True
            if args.capture_screenshot:
                optimized_screenshot = _nested(optimized, "screenshot", "sha256")
                forced_screenshot = _nested(forced_layout, "screenshot", "sha256")
                screenshot_equal = bool(optimized_screenshot) and (
                    optimized_screenshot == forced_screenshot
                )
            passed = len(modes) == 2 and geometry_equal and screenshot_equal
            comparison_passed &= passed
            comparisons.append(
                {
                    "comparison": "text-invalidation-mode",
                    "scenario": key[0],
                    "widgets": key[1],
                    "burst_repeats": key[2],
                    "repetition": key[3],
                    "update_mode": key[4],
                    "passed": passed,
                    "retained_geometry_equal": geometry_equal,
                    "screenshot_equal": screenshot_equal,
                    "optimized_equivalence_sha256": _nested(
                        optimized, "equivalence", "sha256"
                    ),
                    "forced_layout_equivalence_sha256": _nested(
                        forced_layout, "equivalence", "sha256"
                    ),
                    "optimized_screenshot_sha256": _nested(
                        optimized, "screenshot", "sha256"
                    ),
                    "forced_layout_screenshot_sha256": _nested(
                        forced_layout, "screenshot", "sha256"
                    ),
                }
            )

    summary = {
        "schema": 1,
        "benchmark": "dragongui_update_pipeline_matrix",
        "machine": _machine_metadata(),
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
            "text_invalidation_mode": args.text_invalidation_mode,
            "targeted_rebuild_verification": args.targeted_rebuild_verification,
            "typed_target_diagnostics": args.typed_target_diagnostics,
            "capture_screenshot": args.capture_screenshot,
            "interaction_probe_ms": args.interaction_probe_ms,
            "locality_roots": args.locality_roots,
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
                "text_invalidation_mode": report["config"]["text_invalidation_mode"],
                "targeted_rebuild_verification": report["config"][
                    "targeted_rebuild_verification"
                ],
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
                "retained_visual_target_requests": _nested(
                    report,
                    "native",
                    "framework",
                    "command_text_rebuilds",
                    "target_classes",
                    "retained_visual",
                ),
                "primitive_paint_target_requests": _nested(
                    report,
                    "native",
                    "framework",
                    "command_text_rebuilds",
                    "target_classes",
                    "primitive_paint",
                ),
                "table_text_target_requests": _nested(
                    report,
                    "native",
                    "framework",
                    "command_text_rebuilds",
                    "target_classes",
                    "table_text",
                ),
                "overlay_text_target_requests": _nested(
                    report,
                    "native",
                    "framework",
                    "command_text_rebuilds",
                    "target_classes",
                    "overlay_text",
                ),
                "targeted_fallbacks": _nested(
                    report,
                    "native",
                    "framework",
                    "command_text_rebuilds",
                    "fallback_batches",
                ),
                "targeted_stale_generation_fallbacks": _nested(
                    report,
                    "native",
                    "framework",
                    "command_text_rebuilds",
                    "stale_generation_fallback_batches",
                ),
                "targeted_verification_attempts": _nested(
                    report,
                    "native",
                    "framework",
                    "targeted_rebuild_verification",
                    "attempts",
                ),
                "targeted_verification_mismatches": _nested(
                    report,
                    "native",
                    "framework",
                    "targeted_rebuild_verification",
                    "mismatches",
                ),
                "structure_generation": _nested(
                    report, "native", "framework", "structure_generation"
                ),
                "partial_text_entries_removed": _nested(
                    report,
                    "native",
                    "framework",
                    "partial_text_rebuilds",
                    "entries_removed",
                ),
                "partial_text_entries_inserted": _nested(
                    report,
                    "native",
                    "framework",
                    "partial_text_rebuilds",
                    "entries_inserted",
                ),
                "partial_primitive_attempts": _nested(
                    report,
                    "native",
                    "renderer",
                    "retained_rebuilds",
                    "partial_base_attempts",
                ),
                "partial_primitive_completed": _nested(
                    report,
                    "native",
                    "renderer",
                    "retained_rebuilds",
                    "partial_base_completed",
                ),
                "partial_primitive_upload_bytes": _nested(
                    report,
                    "native",
                    "renderer",
                    "retained_rebuilds",
                    "partial_upload_bytes",
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
                "targeted_line_plot_checks": _nested(
                    report,
                    "native",
                    "renderer",
                    "retained_rebuilds",
                    "targeted_line_plot_checks",
                ),
                "targeted_line_plot_rebuilds": _nested(
                    report,
                    "native",
                    "renderer",
                    "retained_rebuilds",
                    "targeted_line_plot_rebuilds",
                ),
                "targeted_line_plot_skips": _nested(
                    report,
                    "native",
                    "renderer",
                    "retained_rebuilds",
                    "targeted_line_plot_skips",
                ),
                "line_plot_renderer": _nested(
                    report, "native", "renderer", "line_plot_renderer"
                ),
                "html_renderer": _nested(
                    report, "native", "renderer", "html_reports"
                ),
                "icon_theme": _nested(report, "native", "icon_theme"),
                "icon_identity": _nested(report, "equivalence", "icon_identity"),
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
