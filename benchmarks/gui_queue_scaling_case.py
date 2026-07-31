"""Validate production command-queue scaling without starting a GUI window."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys
import time
from typing import Any

from gui_benchmark_validation import ValidationRecorder


ROOT = Path(__file__).resolve().parents[1]


def run(args: argparse.Namespace) -> dict[str, Any]:
    benchmark_python_path = os.environ.get("DRAGONGUI_BENCH_PYTHON_PATH")
    sys.path.insert(0, benchmark_python_path or str(ROOT / "python"))
    from dragongui import _dragongui

    results: list[dict[str, Any]] = []
    validation = ValidationRecorder()
    for pending in args.pending:
        sender = _dragongui._NativeCommandSender()
        started = time.perf_counter()
        for index in range(pending):
            sender.enqueue_set_prop(f"widget-{index}", "text", index)
        distinct_elapsed_ms = (time.perf_counter() - started) * 1000.0
        distinct = json.loads(sender._queue_debug_snapshot())

        started = time.perf_counter()
        for value in range(args.hot_replacements):
            sender.enqueue_set_prop("widget-0", "text", value)
        hot_elapsed_ms = (time.perf_counter() - started) * 1000.0
        hot = json.loads(sender._queue_debug_snapshot())

        validation.equal(
            f"{pending} keys retain exact logical depth",
            hot.get("depth"),
            pending,
            source="native queue diagnostics",
        )
        validation.equal(
            f"{pending} keys retain exact physical slots",
            hot.get("physical_slots"),
            pending,
            source="native queue diagnostics",
        )
        validation.equal(
            f"{pending} keys record all hot replacements",
            (hot.get("replacements_by_family") or {}).get("set_prop"),
            args.hot_replacements,
            source="native queue diagnostics",
        )
        validation.equal(
            f"{pending} keys have no stale entries",
            hot.get("stale_entries"),
            0,
            source="native queue diagnostics",
        )
        validation.equal(
            f"{pending} keys drain exactly once each",
            sender._drain_for_test(),
            pending,
            source="private benchmark drain",
        )
        results.append(
            {
                "pending": pending,
                "distinct_elapsed_ms": distinct_elapsed_ms,
                "distinct_push_p95_ms": (distinct.get("push_timing") or {}).get("p95_ms"),
                "hot_replacements": args.hot_replacements,
                "hot_elapsed_ms": hot_elapsed_ms,
                "hot_push_p95_ms": (hot.get("push_timing") or {}).get("p95_ms"),
                "queue": hot,
            }
        )

    p95_values = [float(item["hot_push_p95_ms"] or 0.0) for item in results]
    smallest = max(0.0001, min(p95_values, default=0.0))
    largest = max(p95_values, default=0.0)
    validation.require(
        "hot-key push p95 remains approximately flat through 100,000 pending keys",
        largest / smallest,
        lambda value: value <= 10.0,
        "at most 10x smallest-scale p95",
        source="native queue push timing",
    )
    validation.require(
        "100,000-key hot replacement remains sub-50-microsecond p95",
        results[-1]["hot_push_p95_ms"],
        lambda value: isinstance(value, (int, float)) and value <= 0.05,
        "at most 0.05 ms",
        source="native queue push timing",
    )

    return {
        "schema": 1,
        "benchmark": "dragongui_production_queue_scaling",
        "config": {
            "pending": args.pending,
            "hot_replacements": args.hot_replacements,
        },
        "results": results,
        "validation": validation.report(),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pending", type=int, action="append")
    parser.add_argument("--hot-replacements", type=int, default=10_000)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    args.pending = sorted(set(args.pending or [32, 1_000, 100_000]))
    args.hot_replacements = max(1, args.hot_replacements)
    report = run(args)
    payload = json.dumps(report, indent=2, sort_keys=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
    print(payload)
    return 0 if report["validation"]["passed"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
