"""Run repeated fresh-process DragonGUI fixed-baseline memory stages."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CASE = ROOT / "benchmarks" / "gui_fixed_baseline_memory_case.py"
STAGES = ("stdlib", "native", "package", "document", "window")


def _median(samples: list[dict[str, Any]], section: str, key: str) -> float | None:
    values = [sample[section][key] for sample in samples if sample[section][key] is not None]
    return statistics.median(values) if values else None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=20.0)
    parser.add_argument("--memory-hint", choices=("performance", "memory-usage"))
    args = parser.parse_args()
    output_dir = args.output_dir.resolve()
    raw_dir = output_dir / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    samples: dict[str, list[dict[str, Any]]] = {stage: [] for stage in STAGES}
    runs: list[dict[str, Any]] = []
    started = time.perf_counter()

    for repetition in range(args.repetitions):
        offset = repetition % len(STAGES)
        order = STAGES[offset:] + STAGES[:offset]
        for stage in order:
            output = raw_dir / f"{stage}-{repetition + 1}.json"
            command = [
                sys.executable,
                str(CASE),
                "--stage", stage,
                "--output", str(output),
                "--timeout", str(args.timeout),
            ]
            if args.memory_hint:
                command.extend(("--memory-hint", args.memory_hint))
            print(f"{stage}: repetition {repetition + 1}/{args.repetitions}", flush=True)
            completed = subprocess.run(
                command,
                cwd=ROOT,
                env={**os.environ, "PYTHONHASHSEED": "0"},
                capture_output=True,
                text=True,
                timeout=args.timeout + 20.0,
                check=False,
            )
            if completed.returncode not in {0, 2} or not output.exists():
                print(completed.stdout)
                print(completed.stderr, file=sys.stderr)
                return completed.returncode or 1
            sample = json.loads(output.read_text(encoding="utf-8"))
            samples[stage].append(sample)
            runs.append({
                "stage": stage,
                "repetition": repetition + 1,
                "returncode": completed.returncode,
                "raw": str(output.relative_to(output_dir)),
            })

    aggregates: dict[str, Any] = {}
    previous_rss: float | None = None
    previous_private: float | None = None
    for stage in STAGES:
        rss = _median(samples[stage], "stage_memory", "rss_bytes")
        private = _median(samples[stage], "stage_memory", "private_bytes")
        aggregates[stage] = {
            "sample_count": len(samples[stage]),
            "rss_median_bytes": rss,
            "private_median_bytes": private,
            "rss_delta_from_previous_bytes": None if rss is None or previous_rss is None else rss - previous_rss,
            "private_delta_from_previous_bytes": None if private is None or previous_private is None else private - previous_private,
            "all_window_ready": all(sample["details"].get("window_ready", True) for sample in samples[stage]),
        }
        previous_rss, previous_private = rss, private

    report = {
        "schema": 1,
        "benchmark": "dragongui_fixed_baseline_memory",
        "method": {
            "fresh_process_per_stage": True,
            "rotated_stage_order": True,
            "repetitions": args.repetitions,
            "stages": STAGES,
            "memory_hint_requested": args.memory_hint,
        },
        "elapsed_ms": (time.perf_counter() - started) * 1000.0,
        "runs": runs,
        "aggregates": aggregates,
        "samples": samples,
    }
    summary = output_dir / "summary.json"
    summary.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {summary}")
    return 0 if aggregates["window"]["all_window_ready"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
