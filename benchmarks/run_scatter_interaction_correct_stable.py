"""Run the Phase 6 interaction correctness gate in fresh processes."""

from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CASE = ROOT / "benchmarks" / "scatter_interaction_correct_stable_case.py"


def nested(row: dict[str, Any], *keys: str) -> Any:
    value: Any = row
    for key in keys:
        if not isinstance(value, dict):
            return None
        value = value.get(key)
    return value


def summary(rows: list[dict[str, Any]], *keys: str) -> dict[str, float | int | None]:
    values = [float(value) for row in rows if (value := nested(row, *keys)) is not None]
    return {
        "count": len(values),
        "median": statistics.median(values) if values else None,
        "min": min(values) if values else None,
        "max": max(values) if values else None,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--n", type=int, default=1_000_000)
    parser.add_argument("--captures", type=int, default=10)
    parser.add_argument("--frames", type=int, default=3000)
    parser.add_argument("--step-ms", type=float, default=320.0)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=120.0)
    parser.add_argument("--max-schedule-lag-ms", type=float, default=10.0)
    parser.add_argument("--max-correct-recovery-ms", type=float, default=1_000.0)
    parser.add_argument("--max-stable-recovery-ms", type=float, default=1_500.0)
    parser.add_argument("--max-selection-callback-ms", type=float, default=250.0)
    args = parser.parse_args()
    if args.repetitions < 3:
        parser.error("--repetitions must be at least 3")

    raw_dir = args.out.parent / f"{args.out.stem}-raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    samples: list[dict[str, Any]] = []
    for repetition in range(1, args.repetitions + 1):
        sample_path = raw_dir / f"interaction-r{repetition}.json"
        command = [
            sys.executable,
            str(CASE),
            "--n", str(args.n),
            "--captures", str(args.captures),
            "--frames", str(args.frames),
            "--step-ms", str(args.step_ms),
            "--package-root", str(args.package_root.resolve()),
            "--out", str(sample_path),
        ]
        completed = subprocess.run(
            command, cwd=ROOT, capture_output=True, text=True, timeout=args.timeout
        )
        if completed.returncode == 0 and sample_path.exists():
            sample = json.loads(sample_path.read_text(encoding="utf-8"))
        else:
            sample = {
                "status": f"failed(exit={completed.returncode})",
                "stdout": completed.stdout[-1000:],
                "stderr": completed.stderr[-3000:],
            }
        sample["repetition"] = repetition
        samples.append(sample)
        print(
            json.dumps(
                {
                    "repetition": repetition,
                    "status": sample.get("status"),
                    "hash": (nested(sample, "interaction", "hashes") or [None])[0],
                    "selection_ms": nested(
                        sample, "interaction", "selection", "callback_latency_ms"
                    ),
                    "correct_ms": nested(
                        sample, "interaction", "last_input_to_first_correct_ms"
                    ),
                    "stable_ms": nested(
                        sample, "interaction", "last_input_to_ten_stable_ms"
                    ),
                }
            ),
            flush=True,
        )

    hashes = [
        (nested(row, "interaction", "hashes") or [None])[0]
        for row in samples
    ]
    validations = {
        "all_runs_passed": all(row.get("status") == "ok" for row in samples),
        "stable_hash_matches_across_runs": bool(hashes[0]) and len(set(hashes)) == 1,
        "exact_source_row_selection_matches": all(
            nested(row, "interaction", "selection", "selected_indices") == [1]
            and nested(row, "interaction", "selection", "exact_source_row_verified") is True
            for row in samples
        ),
        "home_restore_is_idempotent": all(
            nested(row, "interaction", "home_restore_unique_states") == 1
            for row in samples
        ),
        "full_current_source_is_conserved": all(
            nested(row, "interaction", "density_scope") == "full"
            and nested(row, "interaction", "source_revision") == 2
            and nested(row, "interaction", "represented_source_rows") == args.n
            for row in samples
        ),
        "schedule_lag_within_ceiling": all(
            float(nested(row, "interaction", "schedule_max_lag_ms") or float("inf"))
            <= args.max_schedule_lag_ms
            for row in samples
        ),
        "correct_recovery_within_ceiling": all(
            float(
                nested(row, "interaction", "last_input_to_first_correct_ms")
                or float("inf")
            )
            <= args.max_correct_recovery_ms
            for row in samples
        ),
        "stable_recovery_within_ceiling": all(
            float(
                nested(row, "interaction", "last_input_to_ten_stable_ms")
                or float("inf")
            )
            <= args.max_stable_recovery_ms
            for row in samples
        ),
        "selection_callback_within_ceiling": all(
            float(
                nested(row, "interaction", "selection", "callback_latency_ms")
                or float("inf")
            )
            <= args.max_selection_callback_ms
            for row in samples
        ),
    }
    metric_paths = {
        "public_build_ms": ("interaction", "public_build_ms"),
        "first_nonblank_ms": ("interaction", "first_nonblank_ms"),
        "initial_first_correct_ms": ("interaction", "initial_first_correct_ms"),
        "schedule_max_lag_ms": ("interaction", "schedule_max_lag_ms"),
        "selection_callback_ms": ("interaction", "selection", "callback_latency_ms"),
        "last_input_to_first_correct_ms": (
            "interaction", "last_input_to_first_correct_ms"
        ),
        "last_input_to_ten_stable_ms": (
            "interaction", "last_input_to_ten_stable_ms"
        ),
        "frame_total_p50_ms": ("interaction", "frame_timings", "total", "p50_ms"),
        "frame_total_p95_ms": ("interaction", "frame_timings", "total", "p95_ms"),
        "frame_total_max_ms": ("interaction", "frame_timings", "total", "max_ms"),
        "presentation_hz": ("interaction", "presentation", "achieved_hz"),
        "missed_presentation_estimate_percent": (
            "interaction", "presentation", "missed_presentation_estimate_percent"
        ),
        "peak_rss_bytes": ("peak_rss_bytes",),
        "scatter_gpu_allocated_bytes": (
            "interaction", "gpu_memory", "total_allocated_bytes"
        ),
        "source_rows": ("interaction", "source_rows"),
        "render_rows": ("interaction", "render_rows"),
    }
    passed = all(validations.values())
    result = {
        "status": "ok" if passed else "invalid",
        "generated_at_utc": datetime.now(UTC).isoformat().replace("+00:00", "Z"),
        "contract": "fresh-process fixed-schedule interaction; exact source selection; full-density recovery; cross-run stable pixels",
        "rows": args.n,
        "repetitions": args.repetitions,
        "thresholds": {
            "max_schedule_lag_ms": args.max_schedule_lag_ms,
            "max_correct_recovery_ms": args.max_correct_recovery_ms,
            "max_stable_recovery_ms": args.max_stable_recovery_ms,
            "max_selection_callback_ms": args.max_selection_callback_ms,
        },
        "validations": validations,
        "metrics": {
            name: summary(samples, *path) for name, path in metric_paths.items()
        },
        "samples": samples,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(args.out)
    if not passed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
