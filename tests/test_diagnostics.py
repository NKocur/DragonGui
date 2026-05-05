from __future__ import annotations

import threading

import dragongui as dg
from dragongui.diagnostics import _reset_collector
from dragongui.runtime import AppHandle


def test_task_failure_records_enqueue_origin_thread_and_role() -> None:
    collector = _reset_collector()
    handle = AppHandle()

    def bad_task() -> None:
        raise RuntimeError("diagnostic boom")

    def worker() -> None:
        with dg.thread_role("worker-role"):
            handle.call_soon_threadsafe(bad_task)

    thread = threading.Thread(target=worker, name="origin-worker")
    thread.start()
    thread.join(timeout=1.0)
    assert not thread.is_alive()

    handle._drain_python_tasks()
    snap = collector.snapshot()

    assert snap.failure_count == 1
    failure = snap.recent_failures[-1]
    assert failure.thread_name == "origin-worker"
    assert failure.thread_role == "worker-role"

    origin_rows = [row for row in snap.threads if row.name == "origin-worker"]
    assert origin_rows
    assert origin_rows[0].alive is False
    assert origin_rows[0].role == "worker-role"
    assert origin_rows[0].cmd_count == 1

    capped = collector.snapshot(max_threads=1)
    assert len(capped.threads) == 1

    hidden = collector.snapshot(include_threads=False)
    assert hidden.threads == []


def test_enqueue_rate_reports_above_previous_timestamp_cap() -> None:
    collector = _reset_collector()
    origin = collector.capture_thread_origin()

    for _ in range(250):
        collector.record_enqueue(origin)

    snap = collector.snapshot()
    current_ident = threading.get_ident()
    current_row = next(row for row in snap.threads if row.ident == current_ident)

    assert snap.enqueued_total == 250
    assert snap.enqueue_rate >= 50.0
    assert current_row.cmd_count == 250
    assert current_row.cmd_per_sec >= 50.0
