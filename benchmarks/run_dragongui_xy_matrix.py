"""Run fresh-process DragonGUI scatter samples for comparison with Reflex XY."""

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
CASE = ROOT / "benchmarks" / "dragongui_xy_case.py"


def summary(samples: list[dict[str, Any]], metric: str) -> dict[str, Any]:
    values = [float(row[metric]) for row in samples if row.get("status") == "ok"]
    return {
        "attempted_runs": len(samples),
        "successful_runs": len(values),
        "statuses": [row.get("status") for row in samples],
        "median": statistics.median(values) if values else None,
        "mean": statistics.fmean(values) if values else None,
        "min": min(values) if values else None,
        "max": max(values) if values else None,
        "samples": samples,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sizes", default="10000,100000,1000000,10000000")
    parser.add_argument("--modes", default="adaptive,exact")
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--frames", type=int, default=20)
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    sizes = [int(value) for value in args.sizes.split(",")]
    modes = [value.strip() for value in args.modes.split(",") if value.strip()]
    raw_dir = args.out.parent / f"{args.out.stem}-raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    cells: list[dict[str, Any]] = []
    for repetition in range(args.repetitions):
        ordered_modes = modes if repetition % 2 == 0 else list(reversed(modes))
        for n in sizes:
            for mode in ordered_modes:
                sample_path = raw_dir / f"{mode}-{n}-r{repetition + 1}.json"
                command = [
                    sys.executable,
                    str(CASE),
                    "--n",
                    str(n),
                    "--mode",
                    mode,
                    "--frames",
                    str(args.frames),
                    "--out",
                    str(sample_path),
                ]
                try:
                    completed = subprocess.run(
                        command,
                        cwd=ROOT,
                        capture_output=True,
                        text=True,
                        timeout=args.timeout,
                    )
                except subprocess.TimeoutExpired as exc:
                    sample = {
                        "status": "timeout",
                        "n": n,
                        "mode": mode,
                        "stderr": (exc.stderr or "")[-1000:],
                    }
                else:
                    if completed.returncode == 0 and sample_path.exists():
                        sample = json.loads(sample_path.read_text(encoding="utf-8"))
                    else:
                        sample = {
                            "status": f"failed(exit={completed.returncode})",
                            "n": n,
                            "mode": mode,
                            "stdout": completed.stdout[-1000:],
                            "stderr": completed.stderr[-2000:],
                        }
                sample["repetition"] = repetition + 1
                cells.append(sample)
                print(
                    json.dumps(
                        {
                            "n": n,
                            "mode": mode,
                            "repetition": repetition + 1,
                            "status": sample.get("status"),
                            "render_wall_ms": sample.get("render_wall_ms"),
                            "frame_ms": sample.get("frame_ms"),
                            "peak_mib": (
                                float(sample.get("peak_rss_bytes", 0)) / 2**20
                            ),
                        }
                    ),
                    flush=True,
                )

    summaries = []
    for n in sizes:
        for mode in modes:
            selected = [row for row in cells if row.get("n") == n and row.get("mode") == mode]
            summaries.append(
                {
                    "n": n,
                    "mode": mode,
                    "render_wall_ms": summary(selected, "render_wall_ms"),
                    "frame_ms": summary(selected, "frame_ms"),
                    "peak_rss_bytes": summary(selected, "peak_rss_bytes"),
                }
            )
    result = {
        "generated_at_utc": datetime.now(UTC).isoformat().replace("+00:00", "Z"),
        "contract": "installed wheel; data generation excluded; public build then 20 native frames",
        "sizes": sizes,
        "modes": modes,
        "repetitions": args.repetitions,
        "summaries": summaries,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(args.out)


if __name__ == "__main__":
    main()
