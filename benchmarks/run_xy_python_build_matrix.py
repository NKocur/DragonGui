"""Measure XY's production live-payload build in fresh Python processes."""

from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import sys
import tempfile
import time
from datetime import UTC, datetime
from pathlib import Path
from typing import Any


SEED = 20260713
SENTINELS = [(4.6, 2.2), (4.9, 2.45), (4.75, 2.05), (5.0, 2.3), (4.55, 2.5)]


def child(n: int, mode: str, out: Path) -> None:
    import importlib.metadata

    import numpy as np
    import psutil
    import xy

    rng = np.random.default_rng(SEED)
    x = rng.standard_normal(n, dtype=np.float32)
    y = rng.standard_normal(n, dtype=np.float32)
    y *= np.float32(1.2)
    y += x
    y *= np.float32(0.5)
    for index, (sx, sy) in enumerate(SENTINELS[:n]):
        x[index] = np.float32(sx)
        y[index] = np.float32(sy)
    process = psutil.Process()
    source_rss = process.memory_info().rss
    t0 = time.perf_counter()
    mark = xy.scatter(x=x, y=y, density=False if mode == "exact" else None)
    figure = xy.scatter_chart(mark, width=900, height=420).figure()
    spec, buffers = figure.build_payload_split()
    build_ms = (time.perf_counter() - t0) * 1000.0
    result = {
        "status": "ok",
        "library": "xy",
        "version": importlib.metadata.version("xy"),
        "n": n,
        "mode": mode,
        "render_mode": "density" if figure.traces[0].use_density() else "direct",
        "source_bytes": int(x.nbytes + y.nbytes),
        "python_build_ms": build_ms,
        "payload_bytes": sum(memoryview(buffer).nbytes for buffer in buffers),
        "buffer_count": len(buffers),
        "spec_bytes": len(json.dumps(spec, separators=(",", ":"), default=str).encode()),
        "source_rss_bytes": source_rss,
        "final_rss_bytes": process.memory_info().rss,
    }
    out.write_text(json.dumps(result), encoding="utf-8")


def summarize(samples: list[dict[str, Any]], metric: str) -> dict[str, Any]:
    values = [float(sample[metric]) for sample in samples if sample.get("status") == "ok"]
    return {
        "median": statistics.median(values) if values else None,
        "mean": statistics.fmean(values) if values else None,
        "min": min(values) if values else None,
        "max": max(values) if values else None,
        "samples": samples,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--child", action="store_true")
    parser.add_argument("--n", type=int)
    parser.add_argument("--mode", choices=("adaptive", "exact"))
    parser.add_argument("--child-out", type=Path)
    parser.add_argument("--sizes", default="10000,100000,1000000,2500000,4000000,5000000")
    parser.add_argument("--modes", default="adaptive,exact")
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    if args.child:
        if args.n is None or args.mode is None or args.child_out is None:
            parser.error("child mode requires --n, --mode, and --child-out")
        child(args.n, args.mode, args.child_out)
        return
    if args.out is None:
        parser.error("--out is required")

    sizes = [int(value) for value in args.sizes.split(",")]
    modes = [value.strip() for value in args.modes.split(",") if value.strip()]
    cells: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory() as temp_dir:
        for repetition in range(args.repetitions):
            ordered_modes = modes if repetition % 2 == 0 else list(reversed(modes))
            for n in sizes:
                for mode in ordered_modes:
                    output = Path(temp_dir) / f"{n}-{mode}-{repetition}.json"
                    command = [
                        sys.executable,
                        str(Path(__file__).resolve()),
                        "--child",
                        "--n",
                        str(n),
                        "--mode",
                        mode,
                        "--child-out",
                        str(output),
                    ]
                    try:
                        completed = subprocess.run(
                            command, capture_output=True, text=True, timeout=args.timeout
                        )
                    except subprocess.TimeoutExpired:
                        sample = {"status": "timeout", "n": n, "mode": mode}
                    else:
                        if completed.returncode == 0 and output.exists():
                            sample = json.loads(output.read_text(encoding="utf-8"))
                        else:
                            sample = {
                                "status": f"failed(exit={completed.returncode})",
                                "n": n,
                                "mode": mode,
                                "stderr": completed.stderr[-2000:],
                            }
                    sample["repetition"] = repetition + 1
                    cells.append(sample)
                    print(json.dumps(sample), flush=True)

    summaries = []
    for n in sizes:
        for mode in modes:
            selected = [row for row in cells if row.get("n") == n and row.get("mode") == mode]
            summaries.append(
                {"n": n, "mode": mode, "python_build_ms": summarize(selected, "python_build_ms")}
            )
    result = {
        "generated_at_utc": datetime.now(UTC).isoformat().replace("+00:00", "Z"),
        "contract": "source arrays ready; chart.figure plus production split payload",
        "sizes": sizes,
        "modes": modes,
        "repetitions": args.repetitions,
        "summaries": summaries,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
