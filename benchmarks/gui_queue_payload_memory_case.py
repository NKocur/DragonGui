"""Measure queue memory while repeatedly replacing one large packed payload."""

from __future__ import annotations

import argparse
import gc
import json
import os
from pathlib import Path
import sys
import time
from typing import Any

from gui_benchmark_validation import ValidationRecorder
from gui_framework_case import _rss_bytes, _timings


ROOT = Path(__file__).resolve().parents[1]


def _mib(value: int | None) -> float | None:
    return None if value is None else value / (1024 * 1024)


def run(args: argparse.Namespace) -> dict[str, Any]:
    benchmark_python_path = os.environ.get("DRAGONGUI_BENCH_PYTHON_PATH")
    sys.path.insert(0, benchmark_python_path or str(ROOT / "python"))
    from dragongui import _dragongui

    payload_bytes = max(12, int(args.payload_mib * 1024 * 1024))
    payload_bytes -= payload_bytes % 12
    payload = bytearray(payload_bytes)
    for offset in range(0, payload_bytes, 4096):
        payload[offset] = (offset // 4096) % 251
    payload_view = memoryview(payload)

    sender = _dragongui._NativeCommandSender()
    gc.collect()
    rss_before = _rss_bytes()
    rss_samples: list[int] = []
    enqueue_ms: list[float] = []
    started = time.perf_counter()
    for replacement in range(args.replacements):
        t0 = time.perf_counter()
        sender.enqueue_set_scatter_points_packed(
            "memory-probe",
            payload_view,
            colormap="viridis",
            payload_format="xyz_f32_v0",
            coalesce=True,
            fit=replacement % 17 == 0,
        )
        enqueue_ms.append((time.perf_counter() - t0) * 1000.0)
        rss = _rss_bytes()
        if rss is not None:
            rss_samples.append(rss)
    elapsed_s = time.perf_counter() - started

    queued_snapshot = json.loads(sender._queue_debug_snapshot())
    rss_queued = _rss_bytes()
    drained_commands = sender._drain_for_test()
    gc.collect()
    rss_after_drain = _rss_bytes()
    drained_snapshot = json.loads(sender._queue_debug_snapshot())

    peak_rss = max(rss_samples, default=rss_queued or rss_before or 0)
    baseline = rss_before or 0
    peak_growth = max(0, peak_rss - baseline)
    queued_growth = max(0, (rss_queued or baseline) - baseline)
    logical_bytes_submitted = payload_bytes * args.replacements
    allowed_growth = payload_bytes * args.max_payload_multiples + 64 * 1024 * 1024

    validation = ValidationRecorder()
    validation.equal(
        "one live coalesced payload remains",
        queued_snapshot.get("depth"),
        1,
        source="native queue diagnostics",
    )
    validation.equal(
        "one physical queue slot remains",
        queued_snapshot.get("physical_slots"),
        1,
        source="native queue diagnostics",
    )
    validation.equal(
        "all older payloads were replaced",
        (queued_snapshot.get("replacements_by_family") or {}).get("scatter_points"),
        args.replacements - 1,
        source="native queue diagnostics",
    )
    validation.equal(
        "test drain returns only the newest payload",
        drained_commands,
        1,
        source="private benchmark drain",
    )
    validation.equal(
        "queue is empty after drain",
        drained_snapshot.get("depth"),
        0,
        source="native queue diagnostics",
    )
    validation.require(
        "peak RSS remains bounded relative to one payload",
        peak_growth,
        lambda value: value <= allowed_growth,
        f"at most {_mib(allowed_growth):.1f} MiB growth",
        source="process working set",
    )
    validation.require(
        "submitted bytes substantially exceed retained RSS growth",
        logical_bytes_submitted,
        lambda value: value >= max(1, peak_growth) * 10,
        "at least 10x peak RSS growth",
        source="payload accounting",
    )

    return {
        "schema": 1,
        "benchmark": "dragongui_queue_payload_memory",
        "config": {
            "payload_bytes": payload_bytes,
            "payload_mib": payload_bytes / (1024 * 1024),
            "replacements": args.replacements,
            "max_payload_multiples": args.max_payload_multiples,
        },
        "elapsed_s": elapsed_s,
        "enqueue_ms": _timings(enqueue_ms),
        "memory": {
            "rss_before_bytes": rss_before,
            "rss_peak_bytes": peak_rss,
            "rss_queued_bytes": rss_queued,
            "rss_after_drain_bytes": rss_after_drain,
            "peak_growth_bytes": peak_growth,
            "queued_growth_bytes": queued_growth,
            "logical_bytes_submitted": logical_bytes_submitted,
            "logical_to_peak_growth_ratio": logical_bytes_submitted / max(1, peak_growth),
        },
        "queue_before_drain": queued_snapshot,
        "queue_after_drain": drained_snapshot,
        "validation": validation.report(),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--payload-mib", type=float, default=8.0)
    parser.add_argument("--replacements", type=int, default=200)
    parser.add_argument("--max-payload-multiples", type=int, default=6)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    args.replacements = max(2, args.replacements)
    report = run(args)
    payload = json.dumps(report, indent=2, sort_keys=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
    print(payload)
    return 0 if report["validation"]["passed"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
