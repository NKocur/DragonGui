"""Benchmark source-order 3D chunk bounds and conservative frustum rejection."""

from __future__ import annotations

import argparse
import json
import statistics
import sys
import time
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", type=int, required=True)
    parser.add_argument("--order", choices=("coherent", "random"), required=True)
    parser.add_argument("--chunk-rows", type=int, default=65_536)
    parser.add_argument("--queries", type=int, default=7)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    sys.path.insert(0, str(args.package_root.resolve()))
    import numpy as np
    import dragongui as dg

    rng = np.random.default_rng(20260806)
    if args.order == "coherent":
        x = np.linspace(0.0, 1.0, args.n, dtype=np.float32)
        y = x.copy()
        z = x.copy()
    else:
        x = rng.random(args.n, dtype=np.float32)
        y = rng.random(args.n, dtype=np.float32)
        z = rng.random(args.n, dtype=np.float32)
    store = dg.PointStore(x, y, z=z, ownership="borrowed")
    bounds = (0.45, 0.55, 0.45, 0.55, 0.45, 0.55)
    planes = [
        (1.0, 0.0, 0.0, -0.45),
        (-1.0, 0.0, 0.0, 0.55),
        (0.0, 1.0, 0.0, -0.45),
        (0.0, -1.0, 0.0, 0.55),
        (0.0, 0.0, 1.0, -0.45),
        (0.0, 0.0, -1.0, 0.55),
    ]

    build = store.build_chunk_bounds(chunk_rows=args.chunk_rows)
    ranges = store.query_box_chunks(*bounds, chunk_rows=args.chunk_rows)
    frustum_ranges = store.query_frustum_chunks(planes, chunk_rows=args.chunk_rows)
    candidate_rows = int(np.sum(ranges[:, 1] - ranges[:, 0])) if len(ranges) else 0

    def candidate_query() -> int:
        count = 0
        for start, stop in ranges:
            chunk_x = x[start:stop]
            chunk_y = y[start:stop]
            chunk_z = z[start:stop]
            count += int(np.count_nonzero(
                (chunk_x >= bounds[0]) & (chunk_x <= bounds[1])
                & (chunk_y >= bounds[2]) & (chunk_y <= bounds[3])
                & (chunk_z >= bounds[4]) & (chunk_z <= bounds[5])
            ))
        return count

    def full_scan() -> int:
        return int(np.count_nonzero(
            (x >= bounds[0]) & (x <= bounds[1])
            & (y >= bounds[2]) & (y <= bounds[3])
            & (z >= bounds[4]) & (z <= bounds[5])
        ))

    candidate_ms: list[float] = []
    scan_ms: list[float] = []
    candidate_count = 0
    scan_count = 0
    for _ in range(args.queries):
        started = time.perf_counter()
        candidate_count = candidate_query()
        candidate_ms.append((time.perf_counter() - started) * 1000.0)
        started = time.perf_counter()
        scan_count = full_scan()
        scan_ms.append((time.perf_counter() - started) * 1000.0)

    candidate_median = statistics.median(candidate_ms)
    scan_median = statistics.median(scan_ms)
    payload = {
        "status": "ok"
        if candidate_count == scan_count and ranges.tolist() == frustum_ranges.tolist()
        else "invalid",
        "n": args.n,
        "order": args.order,
        "chunk_rows": args.chunk_rows,
        "chunk_count": build["chunk_count"],
        "visible_chunk_count": len(ranges),
        "candidate_rows": candidate_rows,
        "candidate_ratio": candidate_rows / args.n,
        "exact_result_rows": candidate_count,
        "build_ms": build["build_ms"],
        "index_bytes": build["owned_bytes"],
        "candidate_query_median_ms": candidate_median,
        "full_scan_median_ms": scan_median,
        "speedup": scan_median / candidate_median if candidate_median > 0 else None,
        "box_matches_frustum": ranges.tolist() == frustum_ranges.tolist(),
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload))
    if payload["status"] != "ok":
        raise SystemExit(1)


if __name__ == "__main__":
    main()
