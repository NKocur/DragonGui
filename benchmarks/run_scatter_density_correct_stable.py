"""Run and compare explicit-density and adaptive correct/stable gates."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CASE = ROOT / "benchmarks" / "scatter_density_correct_stable_case.py"


def nested(row: dict[str, Any], *keys: str) -> Any:
    value: Any = row
    for key in keys:
        if not isinstance(value, dict):
            return None
        value = value.get(key)
    return value


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", type=int, default=1_000_000)
    parser.add_argument("--captures", type=int, default=10)
    parser.add_argument("--frames", type=int, default=260)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=120.0)
    args = parser.parse_args()

    samples: dict[str, dict[str, Any]] = {}
    raw_dir = args.out.parent / f"{args.out.stem}-raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    for mode in ("density", "adaptive"):
        sample_path = raw_dir / f"{mode}.json"
        command = [
            sys.executable,
            str(CASE),
            "--mode", mode,
            "--n", str(args.n),
            "--captures", str(args.captures),
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
                "stdout": completed.stdout[-1000:],
                "stderr": completed.stderr[-2000:],
            }
        samples[mode] = sample
        print(json.dumps({
            "mode": mode,
            "status": sample.get("status"),
            "hash": (nested(sample, "capture", "hashes") or [None])[0],
            "render_rows": nested(sample, "capture", "render_rows"),
        }), flush=True)

    density = samples["density"]
    adaptive = samples["adaptive"]
    density_hash = (nested(density, "capture", "hashes") or [None])[0]
    adaptive_hash = (nested(adaptive, "capture", "hashes") or [None])[0]
    validations = {
        "both_passed": all(sample.get("status") == "ok" for sample in samples.values()),
        "capture_hashes_match": bool(density_hash) and density_hash == adaptive_hash,
        "representative_fingerprints_match": (
            nested(density, "capture", "representative_fingerprint")
            == nested(adaptive, "capture", "representative_fingerprint")
        ),
        "render_rows_match": (
            nested(density, "capture", "render_rows")
            == nested(adaptive, "capture", "render_rows")
        ),
        "represented_source_rows_match": (
            nested(density, "capture", "represented_source_rows")
            == nested(adaptive, "capture", "represented_source_rows")
            == args.n
        ),
    }
    passed = all(validations.values())
    result = {
        "status": "ok" if passed else "invalid",
        "generated_at_utc": datetime.now(UTC).isoformat().replace("+00:00", "Z"),
        "contract": "density and adaptive must independently pass ten-frame correctness and produce identical bounded output",
        "rows": args.n,
        "captures_per_mode": args.captures,
        "validations": validations,
        "samples": samples,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(args.out)
    if not passed:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
