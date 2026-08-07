"""Run the Phase 5 first-application-present gates in fresh processes."""

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
CASE = ROOT / "benchmarks" / "startup_first_present_case.py"
TARGETS_MS = {"empty": 1_000.0, "exact-100k": 1_500.0, "adaptive-1m": 2_000.0}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--frames", type=int, default=3)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=120.0)
    args = parser.parse_args()

    workloads = tuple(TARGETS_MS)
    raw_dir = args.out.parent / f"{args.out.stem}-raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    samples: list[dict[str, Any]] = []
    for repetition in range(1, args.repetitions + 1):
        ordered = workloads if repetition % 2 else tuple(reversed(workloads))
        for workload in ordered:
            sample_path = raw_dir / f"{workload}-r{repetition}.json"
            command = [
                sys.executable,
                str(CASE),
                "--workload", workload,
                "--frames", str(args.frames),
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
                    "workload": workload,
                    "stdout": completed.stdout[-1000:],
                    "stderr": completed.stderr[-2000:],
                }
            sample["repetition"] = repetition
            samples.append(sample)
            print(json.dumps({
                "workload": workload,
                "repetition": repetition,
                "status": sample.get("status"),
                "first_present_ms": sample.get(
                    "source_ready_to_first_application_present_ms"
                ),
            }), flush=True)

    summaries: list[dict[str, Any]] = []
    all_pass = True
    for workload in workloads:
        selected = [row for row in samples if row.get("workload") == workload]
        valid = [
            row for row in selected
            if row.get("status") == "ok"
            and row.get("source_ready_to_first_application_present_ms") is not None
        ]
        values = [
            float(row["source_ready_to_first_application_present_ms"]) for row in valid
        ]
        median_ms = statistics.median(values) if values else None
        target_ms = TARGETS_MS[workload]
        passed = len(valid) == args.repetitions and median_ms is not None and median_ms < target_ms
        all_pass &= passed
        summaries.append({
            "workload": workload,
            "target_ms": target_ms,
            "median_ms": median_ms,
            "min_ms": min(values) if values else None,
            "max_ms": max(values) if values else None,
            "passed": passed,
            "samples": selected,
        })

    result = {
        "status": "ok" if all_pass else "invalid",
        "generated_at_utc": datetime.now(UTC).isoformat().replace("+00:00", "Z"),
        "contract": "fresh process; source ready through first application presentation; medians of three by default",
        "repetitions": args.repetitions,
        "summaries": summaries,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(args.out)
    if not all_pass:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
