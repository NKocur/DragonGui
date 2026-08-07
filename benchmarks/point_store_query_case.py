"""Benchmark PointStore rectangular scans against its lazy sorted-X index."""

from __future__ import annotations

import argparse
import json
import statistics
import sys
import time
from pathlib import Path

import psutil


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", type=int, required=True)
    parser.add_argument("--distribution", choices=("monotonic", "random"), required=True)
    parser.add_argument("--dimensions", type=int, choices=(2, 3), default=2)
    parser.add_argument("--queries", type=int, default=10)
    parser.add_argument("--width-fraction", type=float, default=0.01)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    sys.path.insert(0, str(args.package_root.resolve()))
    import numpy as np
    import dragongui as dg

    rng = np.random.default_rng(20260806)
    if args.distribution == "monotonic":
        x = np.linspace(0.0, 1.0, args.n, dtype=np.float32)
    else:
        x = rng.random(args.n, dtype=np.float32)
    y = rng.random(args.n, dtype=np.float32)
    z = rng.random(args.n, dtype=np.float32) if args.dimensions == 3 else None
    store = dg.PointStore(x, y, z=z, ownership="borrowed")
    process = psutil.Process()
    before_index_rss = process.memory_info().rss

    width = float(args.width_fraction)
    centers = np.linspace(width, 1.0 - width, args.queries, dtype=np.float64)
    rectangles = [
        (
            float(center - width / 2.0),
            float(center + width / 2.0),
            0.25,
            0.75,
            *((0.25, 0.75) if args.dimensions == 3 else ()),
        )
        for center in centers
    ]
    query = store.query_box if args.dimensions == 3 else store.query_rect

    first_t0 = time.perf_counter()
    first_rows = query(*rectangles[0], strategy="sorted_x")
    first_indexed_ms = (time.perf_counter() - first_t0) * 1000.0
    after_index_rss = process.memory_info().rss
    index_stats = store.stats()

    indexed_ms: list[float] = []
    indexed_counts: list[int] = []
    candidates_before = int(store.stats()["spatial_candidate_rows"])
    for rect in rectangles:
        started = time.perf_counter()
        rows = query(*rect, strategy="sorted_x")
        indexed_ms.append((time.perf_counter() - started) * 1000.0)
        indexed_counts.append(len(rows))
    candidates_after = int(store.stats()["spatial_candidate_rows"])

    scan_ms: list[float] = []
    scan_counts: list[int] = []
    for rect in rectangles:
        started = time.perf_counter()
        rows = query(*rect, strategy="scan")
        scan_ms.append((time.perf_counter() - started) * 1000.0)
        scan_counts.append(len(rows))

    indexed_median = statistics.median(indexed_ms)
    scan_median = statistics.median(scan_ms)
    saved_per_query = scan_median - indexed_median
    index_build_ms = float(index_stats["spatial_index_entries"] and (
        first_indexed_ms - indexed_ms[0]
    ))
    payload = {
        "status": "ok" if indexed_counts == scan_counts and len(first_rows) == indexed_counts[0] else "invalid",
        "n": args.n,
        "distribution": args.distribution,
        "dimensions": args.dimensions,
        "queries": args.queries,
        "width_fraction": width,
        "first_indexed_ms": first_indexed_ms,
        "estimated_index_build_ms": max(0.0, index_build_ms),
        "indexed_median_ms": indexed_median,
        "scan_median_ms": scan_median,
        "speedup": scan_median / indexed_median if indexed_median > 0 else None,
        "break_even_queries": max(0.0, index_build_ms) / saved_per_query if saved_per_query > 0 else None,
        "average_candidate_rows": (candidates_after - candidates_before) / args.queries,
        "average_result_rows": statistics.mean(indexed_counts),
        "index_bytes": index_stats["spatial_index_bytes"],
        "index_rss_delta_bytes": after_index_rss - before_index_rss,
        "monotonic_zero_copy": bool(
            args.distribution == "monotonic" and index_stats["spatial_index_bytes"] == 0
        ),
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload))
    if payload["status"] != "ok":
        raise SystemExit(1)


if __name__ == "__main__":
    main()
