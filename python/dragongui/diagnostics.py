"""Lightweight diagnostics collection for ThreadMonitor."""
from __future__ import annotations

import threading
import time
import traceback
from collections import deque
from contextlib import contextmanager
from typing import Any


_RATE_WINDOW_SEC = 5.0
_RATE_BUCKET_SEC = 0.25
_MAX_THREAD_RECORDS = 256
_DEFAULT_MAX_DEAD_THREADS = 20


class _RateCounter:
    __slots__ = ("_bucket_seconds", "_window_seconds", "_buckets", "_last_prune_bucket")

    def __init__(
        self,
        *,
        window_seconds: float = _RATE_WINDOW_SEC,
        bucket_seconds: float = _RATE_BUCKET_SEC,
    ) -> None:
        self._bucket_seconds = bucket_seconds
        self._window_seconds = window_seconds
        self._buckets: dict[int, int] = {}
        self._last_prune_bucket = -1

    def add(self, now: float) -> None:
        bucket = int(now / self._bucket_seconds)
        self._buckets[bucket] = self._buckets.get(bucket, 0) + 1
        if bucket != self._last_prune_bucket:
            self._last_prune_bucket = bucket
            self._prune(now)

    def rate(self, now: float) -> float:
        self._prune(now)
        return sum(self._buckets.values()) / self._window_seconds

    def _prune(self, now: float) -> None:
        cutoff = int((now - self._window_seconds) / self._bucket_seconds)
        for bucket in list(self._buckets):
            if bucket < cutoff:
                self._buckets.pop(bucket, None)


class ThreadOrigin:
    __slots__ = ("ident", "name", "native_id", "daemon", "role")

    def __init__(
        self,
        ident: int,
        name: str,
        native_id: int | None,
        daemon: bool,
        role: str | None,
    ) -> None:
        self.ident = ident
        self.name = name
        self.native_id = native_id
        self.daemon = daemon
        self.role = role


class _ThreadRecord:
    __slots__ = ("name", "ident", "native_id", "daemon", "role", "last_seen")

    def __init__(
        self,
        ident: int,
        name: str,
        native_id: int | None,
        daemon: bool,
        role: str | None,
        last_seen: float,
    ) -> None:
        self.name = name
        self.ident = ident
        self.native_id = native_id
        self.daemon = daemon
        self.role = role
        self.last_seen = last_seen


class ThreadInfo:
    __slots__ = ("name", "ident", "native_id", "alive", "daemon", "role", "cmd_count", "cmd_per_sec")

    def __init__(
        self,
        name: str,
        ident: int | None,
        native_id: int | None,
        alive: bool,
        daemon: bool,
        role: str | None,
        cmd_count: int,
        cmd_per_sec: float,
    ) -> None:
        self.name = name
        self.ident = ident
        self.native_id = native_id
        self.alive = alive
        self.daemon = daemon
        self.role = role
        self.cmd_count = cmd_count
        self.cmd_per_sec = cmd_per_sec


class TaskFailure:
    __slots__ = (
        "ts_ms",
        "callable_repr",
        "thread_name",
        "thread_role",
        "thread_ident",
        "native_id",
        "exc_type",
        "exc_msg",
        "tb_short",
        "first_ts_ms",
        "repeat_count",
    )

    def __init__(
        self,
        callable_repr: str,
        exc: BaseException,
        tb_text: str,
        origin: ThreadOrigin | None = None,
    ) -> None:
        t = threading.current_thread()
        self.ts_ms = int(time.time() * 1000)
        self.first_ts_ms = self.ts_ms
        self.repeat_count = 1
        self.callable_repr = callable_repr
        self.thread_name = origin.name if origin is not None else t.name
        self.thread_role = origin.role if origin is not None else None
        self.thread_ident = origin.ident if origin is not None else t.ident
        self.native_id = (
            origin.native_id if origin is not None else getattr(t, "native_id", None)
        )
        self.exc_type = type(exc).__name__
        self.exc_msg = str(exc)
        lines = tb_text.strip().splitlines()
        self.tb_short = "\n".join(lines[-6:]) if len(lines) > 6 else tb_text.strip()


class DiagnosticsSnapshot:
    __slots__ = (
        "ts_ms",
        "queue_depth",
        "queue_max",
        "queue_avg",
        "queue_samples",
        "threads",
        "thread_total",
        "thread_alive",
        "thread_dead",
        "enqueued_total",
        "enqueue_rate",
        "failure_count",
        "recent_failures",
    )

    def __init__(self) -> None:
        self.ts_ms: int = 0
        self.queue_depth: int = 0
        self.queue_max: int = 0
        self.queue_avg: float = 0.0
        self.queue_samples: list[int] = []
        self.threads: list[ThreadInfo] = []
        self.thread_total: int = 0
        self.thread_alive: int = 0
        self.thread_dead: int = 0
        self.enqueued_total: int = 0
        self.enqueue_rate: float = 0.0
        self.failure_count: int = 0
        self.recent_failures: list[TaskFailure] = []


class DiagnosticsCollector:
    def __init__(self, history_seconds: int = 30) -> None:
        self._lock = threading.Lock()
        self._history_seconds = history_seconds
        self._roles: dict[int, str] = {}
        self._threads: dict[int, _ThreadRecord] = {}
        self._thread_counts: dict[int, int] = {}
        self._thread_rates: dict[int, _RateCounter] = {}
        self._queue_samples: deque[tuple[float, int]] = deque()
        self._enqueued_total: int = 0
        self._enqueue_rate = _RateCounter()
        self._failures: deque[TaskFailure] = deque(maxlen=50)
        self._failure_count: int = 0
        self._last_thread_prune = 0.0

    def configure(self, history_seconds: int) -> None:
        with self._lock:
            self._history_seconds = history_seconds

    def capture_thread_origin(self) -> ThreadOrigin:
        t = threading.current_thread()
        ident = threading.get_ident()
        now = time.monotonic()
        with self._lock:
            role = self._roles.get(ident)
            self._remember_thread_locked(
                ident,
                t.name,
                getattr(t, "native_id", None),
                t.daemon,
                role,
                now,
            )
        return ThreadOrigin(ident, t.name, getattr(t, "native_id", None), t.daemon, role)

    def _remember_thread_locked(
        self,
        ident: int,
        name: str,
        native_id: int | None,
        daemon: bool,
        role: str | None,
        now: float,
    ) -> None:
        record = self._threads.get(ident)
        if record is None:
            self._threads[ident] = _ThreadRecord(
                ident,
                name,
                native_id,
                daemon,
                role,
                now,
            )
            return
        record.name = name
        record.native_id = native_id
        record.daemon = daemon
        if role is not None:
            record.role = role
        record.last_seen = now

    def _prune_thread_records_locked(
        self,
        now: float,
        *,
        live_idents: set[int] | None = None,
        max_dead_threads: int = _DEFAULT_MAX_DEAD_THREADS,
        current_ident: int | None = None,
    ) -> None:
        live_idents = live_idents or set()
        retention = max(float(self._history_seconds), _RATE_WINDOW_SEC)
        stale_cutoff = now - retention
        for ident, record in list(self._threads.items()):
            if ident not in live_idents and record.last_seen < stale_cutoff:
                self._drop_thread_locked(ident)

        dead = [
            record
            for ident, record in self._threads.items()
            if ident not in live_idents and ident != current_ident
        ]
        dead.sort(key=lambda record: record.last_seen, reverse=True)
        for record in dead[max(0, max_dead_threads):]:
            self._drop_thread_locked(record.ident)

        if len(self._threads) > _MAX_THREAD_RECORDS:
            records = sorted(self._threads.values(), key=lambda record: record.last_seen)
            for record in records[: len(self._threads) - _MAX_THREAD_RECORDS]:
                if record.ident != current_ident and record.ident not in live_idents:
                    self._drop_thread_locked(record.ident)

    def _drop_thread_locked(self, ident: int) -> None:
        self._threads.pop(ident, None)
        self._roles.pop(ident, None)
        self._thread_counts.pop(ident, None)
        self._thread_rates.pop(ident, None)

    def register_role(self, ident: int, role: str) -> None:
        t = threading.current_thread()
        now = time.monotonic()
        with self._lock:
            self._roles[ident] = role
            self._remember_thread_locked(
                ident,
                t.name,
                getattr(t, "native_id", None),
                t.daemon,
                role,
                now,
            )

    def clear_role(self, ident: int) -> None:
        with self._lock:
            self._roles.pop(ident, None)

    def record_enqueue(self, origin: ThreadOrigin | None = None) -> ThreadOrigin:
        if origin is None:
            origin = self.capture_thread_origin()
        now = time.monotonic()
        with self._lock:
            record = self._threads.get(origin.ident)
            if record is None:
                self._remember_thread_locked(
                    origin.ident,
                    origin.name,
                    origin.native_id,
                    origin.daemon,
                    origin.role,
                    now,
                )
            else:
                record.last_seen = now
                if origin.role is not None:
                    record.role = origin.role
            self._enqueued_total += 1
            self._enqueue_rate.add(now)
            self._thread_counts[origin.ident] = self._thread_counts.get(origin.ident, 0) + 1
            if origin.ident not in self._thread_rates:
                self._thread_rates[origin.ident] = _RateCounter()
            self._thread_rates[origin.ident].add(now)
            if (
                now - self._last_thread_prune >= 1.0
                or len(self._threads) > _MAX_THREAD_RECORDS
            ):
                self._last_thread_prune = now
                self._prune_thread_records_locked(now, current_ident=origin.ident)
        return origin

    def record_queue_depth(self, depth: int) -> None:
        now = time.monotonic()
        with self._lock:
            self._queue_samples.append((now, depth))
            cutoff = now - self._history_seconds
            while self._queue_samples and self._queue_samples[0][0] < cutoff:
                self._queue_samples.popleft()

    def record_failure(self, failure: TaskFailure) -> int:
        with self._lock:
            self._failure_count += 1
            if self._failures:
                previous = self._failures[-1]
                same_failure = (
                    previous.callable_repr == failure.callable_repr
                    and previous.thread_name == failure.thread_name
                    and previous.thread_role == failure.thread_role
                    and previous.exc_type == failure.exc_type
                    and previous.exc_msg == failure.exc_msg
                    and previous.tb_short == failure.tb_short
                )
                # Collapse a hot stream of the same failing UI task into one
                # row while retaining the total occurrence count. A later
                # recurrence remains a separate incident for chronology.
                if same_failure and failure.ts_ms - previous.ts_ms <= 5_000:
                    previous.ts_ms = failure.ts_ms
                    previous.repeat_count += 1
                    return previous.repeat_count
            self._failures.append(failure)
            return 1

    def snapshot(
        self,
        *,
        include_threads: bool = True,
        include_failures: bool = True,
        max_threads: int | None = None,
        max_dead_threads: int = _DEFAULT_MAX_DEAD_THREADS,
    ) -> DiagnosticsSnapshot:
        now = time.monotonic()
        snap = DiagnosticsSnapshot()
        snap.ts_ms = int(time.time() * 1000)
        live_threads = (
            [t for t in threading.enumerate() if t.ident is not None]
            if include_threads
            else []
        )
        live_idents = {int(t.ident) for t in live_threads if t.ident is not None}

        with self._lock:
            for t in live_threads:
                assert t.ident is not None
                self._remember_thread_locked(
                    int(t.ident),
                    t.name,
                    getattr(t, "native_id", None),
                    t.daemon,
                    self._roles.get(int(t.ident)),
                    now,
                )

            if include_threads:
                self._prune_thread_records_locked(
                    now,
                    live_idents=live_idents,
                    max_dead_threads=max_dead_threads,
                )

            snap.enqueued_total = self._enqueued_total
            snap.enqueue_rate = self._enqueue_rate.rate(now)

            samples = list(self._queue_samples)
            if samples:
                depths = [s[1] for s in samples]
                snap.queue_depth = depths[-1]
                snap.queue_max = max(depths)
                snap.queue_avg = sum(depths) / len(depths)
                step = max(1, len(depths) // 40)
                snap.queue_samples = depths[::step][-40:]

            if include_failures:
                snap.failure_count = self._failure_count
                snap.recent_failures = list(self._failures)[-5:]

            if include_threads:
                roles_snap = dict(self._roles)
                threads_snap = dict(self._threads)
                counts_snap = dict(self._thread_counts)
                rates_snap = {
                    ident: rate.rate(now)
                    for ident, rate in self._thread_rates.items()
                }
            else:
                roles_snap = {}
                threads_snap = {}
                counts_snap = {}
                rates_snap = {}

        if include_threads:
            for t in live_threads:
                assert t.ident is not None
                ident = int(t.ident)
                count = counts_snap.get(ident, 0)
                snap.threads.append(
                    ThreadInfo(
                        t.name,
                        ident,
                        getattr(t, "native_id", None),
                        True,
                        t.daemon,
                        roles_snap.get(ident),
                        count,
                        rates_snap.get(ident, 0.0),
                    )
                )

            dead_records = [
                record
                for ident, record in threads_snap.items()
                if ident not in live_idents
            ]
            dead_records.sort(key=lambda record: record.last_seen, reverse=True)
            for record in dead_records:
                count = counts_snap.get(record.ident, 0)
                snap.threads.append(
                    ThreadInfo(
                        record.name,
                        record.ident,
                        record.native_id,
                        False,
                        record.daemon,
                        roles_snap.get(record.ident) or record.role,
                        count,
                        rates_snap.get(record.ident, 0.0),
                    )
                )

            snap.thread_alive = sum(1 for thread in snap.threads if thread.alive)
            snap.thread_dead = sum(1 for thread in snap.threads if not thread.alive)
            snap.thread_total = len(snap.threads)

            if max_threads is not None and max_threads >= 0:
                snap.threads = _select_threads_for_display(snap.threads, max_threads)

        return snap


def _select_threads_for_display(threads: list[ThreadInfo], max_threads: int) -> list[ThreadInfo]:
    if len(threads) <= max_threads:
        return threads
    ranked = sorted(
        threads,
        key=lambda thread: (
            not thread.alive,
            -(thread.cmd_per_sec),
            -(thread.cmd_count),
            thread.role or thread.name,
        ),
    )
    return ranked[:max_threads]


# ---------------------------------------------------------------------------
# Module-level singleton and public helpers
# ---------------------------------------------------------------------------

_collector: DiagnosticsCollector | None = None
_collector_lock = threading.Lock()


def _get_collector(history_seconds: int | None = None) -> DiagnosticsCollector:
    global _collector
    with _collector_lock:
        if _collector is None:
            _collector = DiagnosticsCollector(history_seconds if history_seconds is not None else 30)
        elif history_seconds is not None:
            _collector.configure(history_seconds)
        return _collector


def _reset_collector(history_seconds: int = 30) -> DiagnosticsCollector:
    global _collector
    with _collector_lock:
        _collector = DiagnosticsCollector(history_seconds)
    return _collector


def register_thread_role(role: str) -> None:
    """Register a display role for the calling thread (visible in ThreadMonitor)."""
    _get_collector().register_role(threading.get_ident(), role)


@contextmanager
def thread_role(role: str):
    """Context manager: tag the current thread with a display role for ThreadMonitor."""
    register_thread_role(role)
    try:
        yield
    finally:
        _get_collector().clear_role(threading.get_ident())


def record_task_failure(
    task: Any,
    exc: BaseException,
    origin: ThreadOrigin | None = None,
) -> int:
    """Called by the runtime when a call_soon_threadsafe task raises an exception."""
    try:
        name = (
            getattr(task, "__qualname__", None)
            or getattr(task, "__name__", None)
            or repr(task)
        )
        failure = TaskFailure(str(name)[:80], exc, traceback.format_exc(), origin)
        return _get_collector().record_failure(failure)
    except Exception:
        return 1
