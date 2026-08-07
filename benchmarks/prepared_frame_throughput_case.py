"""Benchmark direct producer-thread ScatterLiveFrame prepared submissions."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import statistics
import sys
import threading
import time
from typing import Any


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    if not ordered:
        return 0.0
    return ordered[round((len(ordered) - 1) * fraction)]


def nested(value: object, *keys: str) -> Any:
    current = value
    for key in keys:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--rows", type=int, default=20_000)
    parser.add_argument("--variants", type=int, default=8)
    parser.add_argument("--submissions", type=int, default=2_000)
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--warmup", type=int, default=200)
    parser.add_argument("--max-regression-percent", type=float, default=5.0)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if min(args.rows, args.variants, args.submissions, args.rounds) <= 0:
        parser.error("rows, variants, submissions, and rounds must be positive")
    if args.warmup < 0:
        parser.error("warmup must be non-negative")

    sys.path.insert(0, str(args.package_root.resolve()))
    import numpy as np
    import dragongui as dg

    frames = []
    payloads = []
    for variant in range(args.variants):
        count = args.rows + variant
        x = np.linspace(-1.0, 1.0, count, dtype=np.float32)
        frame = {
            "x": x,
            "y": np.sin(x * np.float32(7.0) + np.float32(variant * 0.01)),
            "z": np.cos(x * np.float32(5.0) - np.float32(variant * 0.01)),
        }
        frames.append(frame)
        payloads.append(dg.Scatter3D.prepare_points(frame, x="x", y="y", z="z"))

    app = dg.App(loading_screen=False)
    window = dg.Window("Prepared frame throughput", width=720, height=480)
    with window:
        scatter = dg.Scatter3D(frames[0], x="x", y="y", z="z", colormap="turbo")
    live = scatter.create_live_frame(mode="primary")

    result: dict[str, Any] = {}
    producer_error: list[str] = []
    producer_done = threading.Event()

    def submit_range(start: int, count: int) -> tuple[float, int]:
        started = time.perf_counter()
        payload_bytes = 0
        for sequence in range(start, start + count):
            payload = payloads[sequence % len(payloads)]
            live.enqueue_prepared(
                payload,
                fit=False,
                update_metadata=False,
                coalesce=True,
            )
            payload_bytes += len(payload.data)
        return time.perf_counter() - started, payload_bytes

    def producer() -> None:
        try:
            sequence = 0
            if args.warmup:
                submit_range(sequence, args.warmup)
                sequence += args.warmup
                app.debug_snapshot(timeout_ms=10_000)

            round_seconds: list[float] = []
            round_payload_bytes: list[int] = []
            acknowledged_counts: list[int] = []
            final_snapshot: dict[str, Any] = {}
            for _ in range(args.rounds):
                elapsed, payload_bytes = submit_range(sequence, args.submissions)
                sequence += args.submissions
                round_seconds.append(elapsed)
                round_payload_bytes.append(payload_bytes)
                final_snapshot = app.debug_snapshot(timeout_ms=10_000)
                acknowledged_counts.append(
                    int(
                        nested(
                            final_snapshot,
                            "gpu",
                            "resources",
                            "scatters",
                            scatter.id,
                            "metrics",
                            "last_point_count",
                        )
                        or nested(
                            final_snapshot,
                            "gpu",
                            "resources",
                            "scatters",
                            scatter.id,
                            "last_point_count",
                        )
                        or -1
                    )
                )

            rates = [args.submissions / seconds for seconds in round_seconds]
            byte_rates = [
                payload_bytes / seconds
                for payload_bytes, seconds in zip(round_payload_bytes, round_seconds)
            ]
            expected_counts = [
                args.rows + ((args.warmup + (index + 1) * args.submissions - 1) % args.variants)
                for index in range(args.rounds)
            ]
            command_queue = nested(final_snapshot, "runtime", "command_queue") or {}
            result.update(
                round_seconds=round_seconds,
                submission_rates=rates,
                payload_byte_rates=byte_rates,
                acknowledged_counts=acknowledged_counts,
                expected_counts=expected_counts,
                final_snapshot=final_snapshot,
                command_queue=command_queue,
            )
        except Exception as exc:  # pragma: no cover - reported in artifact
            producer_error.append(f"{type(exc).__name__}: {exc}")
        finally:
            producer_done.set()
            try:
                app.request_exit()
            except RuntimeError:
                pass

    timer = threading.Timer(0.15, lambda: threading.Thread(target=producer, daemon=True).start())
    timer.daemon = True
    timer.start()
    previous_smoke = os.environ.pop("DRAGONGUI_SMOKE_FRAMES", None)
    try:
        run_result = app.run(window)
    finally:
        timer.cancel()
        if previous_smoke is not None:
            os.environ["DRAGONGUI_SMOKE_FRAMES"] = previous_smoke
    producer_done.wait(15.0)

    rates = result.get("submission_rates", [])
    byte_rates = result.get("payload_byte_rates", [])
    median_rate = statistics.median(rates) if rates else 0.0
    median_byte_rate = statistics.median(byte_rates) if byte_rates else 0.0
    metrics = {
        "median_submissions_per_s": median_rate,
        "p05_submissions_per_s": percentile(rates, 0.05),
        "median_payload_bytes_per_s": median_byte_rate,
        "median_payload_gib_per_s": median_byte_rate / (1024**3),
        "round_submissions_per_s": rates,
        "round_payload_bytes_per_s": byte_rates,
    }

    baseline_metrics = None
    throughput_regression_percent = None
    baseline_valid = True
    if args.baseline is not None:
        baseline_report = json.loads(args.baseline.read_text(encoding="utf-8"))
        baseline_metrics = baseline_report.get("metrics") or {}
        baseline_rate = float(baseline_metrics.get("median_payload_bytes_per_s", 0.0))
        throughput_regression_percent = (
            (baseline_rate - median_byte_rate) / baseline_rate * 100.0
            if baseline_rate > 0.0
            else 100.0
        )
        baseline_valid = throughput_regression_percent <= args.max_regression_percent

    command_queue = result.get("command_queue") or {}
    checks = {
        "producer_completed": producer_done.is_set(),
        "producer_error_free": not producer_error,
        "all_rounds_acknowledged_latest_payload": (
            result.get("acknowledged_counts") == result.get("expected_counts")
        ),
        "native_queue_drained": int(command_queue.get("depth", -1)) == 0,
        "prepared_updates_were_coalesced": int(
            nested(command_queue, "replacements_by_family", "scatter_points") or 0
        )
        >= args.rounds * (args.submissions - 1),
        "throughput_within_baseline_budget": baseline_valid,
    }
    report = {
        "schema": 1,
        "benchmark": "prepared_frame_direct_producer_throughput",
        "status": "ok" if all(checks.values()) else "invalid",
        "package_root": str(args.package_root),
        "workload": {
            "rows": args.rows,
            "variants": args.variants,
            "submissions_per_round": args.submissions,
            "rounds": args.rounds,
            "warmup_submissions": args.warmup,
            "payload_format": payloads[0].payload_format,
        },
        "metrics": metrics,
        "baseline_metrics": baseline_metrics,
        "max_regression_percent": args.max_regression_percent,
        "throughput_regression_percent": throughput_regression_percent,
        "observed": {
            "acknowledged_counts": result.get("acknowledged_counts"),
            "expected_counts": result.get("expected_counts"),
            "command_queue": command_queue,
            "producer_errors": producer_error,
            "run_result_type": type(run_result).__name__,
        },
        "checks": checks,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report), flush=True)
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
