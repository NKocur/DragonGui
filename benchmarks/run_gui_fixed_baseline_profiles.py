"""Compare fresh-process DragonGUI live-window memory profiles."""

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
PROFILES = (
    "minimal",
    "large-window",
    "numpy-large-window",
    "indicators-24",
    "indicators-320",
    "lines-4",
    "telemetry-stage1",
)


def _aggregate(samples: list[dict[str, Any]]) -> dict[str, Any]:
    rss = [sample["stage_memory"]["rss_bytes"] for sample in samples]
    private = [sample["stage_memory"]["private_bytes"] for sample in samples]
    return {
        "sample_count": len(samples),
        "all_ready": all(sample["details"].get("window_ready") for sample in samples),
        "rss_median_bytes": statistics.median(rss),
        "rss_range_bytes": [min(rss), max(rss)],
        "private_median_bytes": statistics.median(private),
        "private_range_bytes": [min(private), max(private)],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--profile", action="append", choices=PROFILES)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=20.0)
    parser.add_argument("--memory-hint", choices=("performance", "memory-usage"))
    args = parser.parse_args()
    profiles = tuple(args.profile or PROFILES)
    output_dir = args.output_dir.resolve()
    raw_dir = output_dir / "raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    samples: dict[str, list[dict[str, Any]]] = {profile: [] for profile in profiles}
    runs: list[dict[str, Any]] = []
    started = time.perf_counter()

    for repetition in range(max(1, args.repetitions)):
        offset = repetition % len(profiles)
        order = profiles[offset:] + profiles[:offset]
        for profile in order:
            output = raw_dir / f"{profile}-{repetition + 1}.json"
            command = [
                sys.executable,
                str(CASE),
                "--stage", "window",
                "--window-profile", profile,
                "--output", str(output),
                "--timeout", str(args.timeout),
            ]
            if args.memory_hint:
                command.extend(("--memory-hint", args.memory_hint))
            print(f"{profile}: repetition {repetition + 1}/{args.repetitions}", flush=True)
            completed = subprocess.run(
                command,
                cwd=ROOT,
                env={**os.environ, "PYTHONHASHSEED": "0"},
                capture_output=True,
                text=True,
                timeout=args.timeout + 25.0,
                check=False,
            )
            if completed.returncode != 0 or not output.exists():
                print(completed.stdout)
                print(completed.stderr, file=sys.stderr)
                return completed.returncode or 1
            sample = json.loads(output.read_text(encoding="utf-8"))
            samples[profile].append(sample)
            runs.append({
                "profile": profile,
                "repetition": repetition + 1,
                "raw": str(output.relative_to(output_dir)),
            })

    report = {
        "schema": 1,
        "benchmark": "dragongui_fixed_baseline_profiles",
        "method": {
            "fresh_process_per_sample": True,
            "rotated_profile_order": True,
            "repetitions": max(1, args.repetitions),
            "profiles": profiles,
            "memory_hint_requested": args.memory_hint,
        },
        "elapsed_ms": (time.perf_counter() - started) * 1000.0,
        "runs": runs,
        "results": {
            profile: {"aggregate": _aggregate(profile_samples), "samples": profile_samples}
            for profile, profile_samples in samples.items()
        },
    }
    summary = output_dir / "summary.json"
    summary.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Wrote {summary}")
    return 0 if all(item["aggregate"]["all_ready"] for item in report["results"].values()) else 2


if __name__ == "__main__":
    raise SystemExit(main())
