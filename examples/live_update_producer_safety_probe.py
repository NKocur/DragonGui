"""Validate keyed latest-state updates alongside lossless event callbacks."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import threading
import time
from typing import Any

import dragongui as dg


def _find_node(value: object, widget_id: str) -> dict[str, Any] | None:
    if isinstance(value, dict):
        if value.get("id") == widget_id:
            return value
        for child in value.values():
            found = _find_node(child, widget_id)
            if found is not None:
                return found
    elif isinstance(value, list):
        for child in value:
            found = _find_node(child, widget_id)
            if found is not None:
                return found
    return None


def run_probe(
    *,
    bursts: int = 100,
    snapshots_per_burst: int = 100,
    timeout_seconds: float = 30.0,
) -> dict[str, Any]:
    if bursts <= 0:
        raise ValueError("bursts must be positive")
    if snapshots_per_burst <= 0:
        raise ValueError("snapshots_per_burst must be positive")
    if timeout_seconds <= 0:
        raise ValueError("timeout_seconds must be positive")

    app = dg.App(theme=dg.Theme.dark(), loading_screen=False)
    window = dg.Window("Producer safety probe", width=640, height=320)
    with dg.VLayout(style={"height": "100%", "padding": 20, "gap": 12}):
        dg.Label("Latest-state snapshots and lossless events")
        latest_label = dg.Label("snapshot -1", id="producer-safety-latest")
        event_label = dg.Label("events 0", id="producer-safety-events")
        progress = dg.ProgressBar(0.0)

    expected_final = bursts * snapshots_per_burst - 1
    applied_final = -1
    retained_events: list[int] = []
    worker_error: list[str] = []
    captured_snapshot: list[dict[str, Any]] = []
    worker_done = threading.Event()

    def producer() -> None:
        nonlocal applied_final
        runtime_ready = False
        readiness_deadline = time.monotonic() + timeout_seconds
        try:
            for burst in range(bursts):
                acknowledged = threading.Event()
                first_sequence = burst * snapshots_per_burst
                for offset in range(snapshots_per_burst):
                    sequence = first_sequence + offset

                    def apply_snapshot(sequence: int = sequence) -> None:
                        nonlocal applied_final
                        applied_final = sequence
                        with app.update_batch():
                            latest_label.set_value(f"snapshot {sequence}")
                            progress.set_value((sequence + 1) / (expected_final + 1))

                    while True:
                        try:
                            app.call_soon_threadsafe(
                                apply_snapshot,
                                coalesce_key="producer-safety.latest",
                            )
                            runtime_ready = True
                            break
                        except RuntimeError:
                            if runtime_ready or time.monotonic() >= readiness_deadline:
                                raise
                            time.sleep(0.005)

                def record_event(burst: int = burst) -> None:
                    retained_events.append(burst)
                    event_label.set_value(f"events {len(retained_events)}")
                    acknowledged.set()

                app.call_soon_threadsafe(record_event)
                if not acknowledged.wait(timeout_seconds):
                    raise TimeoutError(f"lossless event {burst} was not acknowledged")

            captured_snapshot.append(app.debug_snapshot(timeout_ms=5000))
        except Exception as exc:  # pragma: no cover - reported by live probe
            worker_error.append(f"{type(exc).__name__}: {exc}")
        finally:
            try:
                app.request_exit()
            except RuntimeError:
                pass
            worker_done.set()

    worker = threading.Thread(
        target=producer,
        name="producer-safety-overload",
        daemon=True,
    )
    worker.start()
    run_result = app.run(window)
    worker_done.wait(timeout_seconds)
    worker.join(timeout=1.0)

    snapshot = captured_snapshot[0] if captured_snapshot else {}
    native_runtime = snapshot.get("runtime") or {}
    python_runtime = native_runtime.get("python") or {}
    native_command_queue = native_runtime.get("command_queue") or {}
    latest_node = _find_node(snapshot, latest_label.id)
    event_node = _find_node(snapshot, event_label.id)
    latest_props = (latest_node or {}).get("props") or {}
    event_props = (event_node or {}).get("props") or {}
    expected_events = list(range(bursts))
    checks = {
        "worker_completed": worker_done.is_set() and not worker.is_alive(),
        "worker_error_free": not worker_error,
        "final_python_state": applied_final == expected_final,
        "all_lossless_events_retained": retained_events == expected_events,
        "native_latest_state": latest_props.get("text") == f"snapshot {expected_final}",
        "native_event_state": event_props.get("text") == f"events {bursts}",
        "python_queue_high_water_at_most_two": (
            int(python_runtime.get("task_queue_high_water", -1)) <= 2
        ),
        "unkeyed_queue_high_water_at_most_one": (
            int(python_runtime.get("unkeyed_task_queue_high_water", -1)) <= 1
        ),
        "no_queue_growth_warnings": (
            int(python_runtime.get("task_queue_growth_warnings", -1)) == 0
        ),
        "python_queue_drained": int(python_runtime.get("queued_tasks", -1)) == 0,
        "native_queue_drained": (
            int(native_runtime.get("command_queue_depth", -1)) == 0
            and int(native_command_queue.get("depth", -1)) == 0
        ),
        "snapshots_were_coalesced": int(python_runtime.get("tasks_coalesced", 0)) > 0,
    }
    return {
        "schema": 1,
        "probe": "live_update_producer_safety",
        "passed": all(checks.values()),
        "workload": {
            "bursts": bursts,
            "snapshots_per_burst": snapshots_per_burst,
            "snapshot_submissions": bursts * snapshots_per_burst,
            "lossless_event_submissions": bursts,
        },
        "expected": {
            "final_snapshot": expected_final,
            "lossless_events": bursts,
            "maximum_python_queue_high_water": 2,
        },
        "observed": {
            "final_snapshot": applied_final,
            "lossless_events": len(retained_events),
            "latest_native_text": latest_props.get("text"),
            "event_native_text": event_props.get("text"),
            "python_runtime": python_runtime,
            "native_command_queue": native_command_queue,
            "worker_errors": worker_error,
            "run_result_type": type(run_result).__name__,
        },
        "checks": checks,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bursts", type=int, default=100)
    parser.add_argument("--snapshots-per-burst", type=int, default=100)
    parser.add_argument("--timeout-seconds", type=float, default=30.0)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    report = run_probe(
        bursts=args.bursts,
        snapshots_per_burst=args.snapshots_per_burst,
        timeout_seconds=args.timeout_seconds,
    )
    payload = json.dumps(report, indent=2, sort_keys=True)
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
    print(payload, flush=True)
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
