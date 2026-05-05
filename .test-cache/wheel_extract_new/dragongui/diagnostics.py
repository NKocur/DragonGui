"""Lightweight diagnostics collection for ThreadMonitor."""
from __future__ import annotations

import threading
import time
import traceback
from collections import deque
from contextlib import contextmanager
from typing import Any


_RATE_WINDOW_SEC = 5.0
_MAX_RATE_TIMESTAMPS = 200


class ThreadInfo:
    __slots__ = ("name", "ident", "native_id", "alive", "daemon", "role", "cmd_count", "cmd_per_sec")

    def __init__(
        self,
        thread: threading.Thread,
        role: str | None,
        cmd_count: int,
        cmd_per_sec: float,
    ) -> None:
        self.name = thread.name
        self.ident = thread.ident
        self.native_id = getattr(thread, "native_id", None)
        self.alive = thread.is_alive()
        self.daemon = thread.daemon
        self.role = role
        self.cmd_count = cmd_count
        self.cmd_per_sec = cmd_per_sec


class TaskFailure:
    __slots__ = ("ts_ms", "callable_repr", "thread_name", "exc_type", "exc_msg", "tb_short")

    def __init__(self, callable_repr: str, exc: BaseException, tb_text: str) -> None:
        t = threading.current_thread()
        self.ts_ms = int(time.time() * 1000)
        self.callable_repr = callable_repr
        self.thread_name = t.name
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
        self.enqueued_total: int = 0
        self.enqueue_rate: float = 0.0
        self.failure_count: int = 0
        self.recent_failures: list[TaskFailure] = []

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, DiagnosticsSnapshot):
            return NotImplemented
        if (
            self.queue_depth != other.queue_depth
            or self.queue_max != other.queue_max
            or self.enqueued_total != other.enqueued_total
            or self.failure_count != other.failure_count
            or len(self.threads) != len(other.threads)
        ):
            return False
        alive_self  = sum(1 for t in self.threads if t.alive)
        alive_other = sum(1 for t in other.threads if t.alive)
        return alive_self == alive_other


class DiagnosticsCollector:
    def __init__(self, history_seconds: int = 30) -> None:
        self._lock = threading.Lock()
        self._history_seconds = history_seconds
        self._roles: dict[int, str] = {}
        self._thread_counts: dict[int, int] = {}
        self._thread_times: dict[int, deque[float]] = {}
        self._queue_samples: deque[tuple[float, int]] = deque()
        self._enqueued_total: int = 0
        self._enqueue_times: deque[float] = deque(maxlen=_MAX_RATE_TIMESTAMPS)
        self._failures: deque[TaskFailure] = deque(maxlen=50)

    def register_role(self, ident: int, role: str) -> None:
        with self._lock:
            self._roles[ident] = role

    def clear_role(self, ident: int) -> None:
        with self._lock:
            self._roles.pop(ident, None)

    def record_enqueue(self) -> None:
        ident = threading.get_ident()
        now = time.monotonic()
        with self._lock:
            self._enqueued_total += 1
            self._enqueue_times.append(now)
            self._thread_counts[ident] = self._thread_counts.get(ident, 0) + 1
            if ident not in self._thread_times:
                self._thread_times[ident] = deque(maxlen=_MAX_RATE_TIMESTAMPS)
            self._thread_times[ident].append(now)

    def record_queue_depth(self, depth: int) -> None:
        now = time.monotonic()
        with self._lock:
            self._queue_samples.append((now, depth))
            cutoff = now - self._history_seconds
            while self._queue_samples and self._queue_samples[0][0] < cutoff:
                self._queue_samples.popleft()

    def record_failure(self, failure: TaskFailure) -> None:
        with self._lock:
            self._failures.append(failure)

    def snapshot(self) -> DiagnosticsSnapshot:
        now = time.monotonic()
        snap = DiagnosticsSnapshot()
        snap.ts_ms = int(time.time() * 1000)

        with self._lock:
            snap.enqueued_total = self._enqueued_total

            cutoff_rate = now - _RATE_WINDOW_SEC
            snap.enqueue_rate = (
                sum(1 for ts in self._enqueue_times if ts >= cutoff_rate) / _RATE_WINDOW_SEC
            )

            samples = list(self._queue_samples)
            if samples:
                depths = [s[1] for s in samples]
                snap.queue_depth = depths[-1]
                snap.queue_max = max(depths)
                snap.queue_avg = sum(depths) / len(depths)
                step = max(1, len(depths) // 40)
                snap.queue_samples = depths[::step][-40:]

            snap.failure_count = len(self._failures)
            snap.recent_failures = list(self._failures)[-5:]

            roles_snap = dict(self._roles)
            counts_snap = dict(self._thread_counts)
            times_snap = {k: list(v) for k, v in self._thread_times.items()}

        for t in threading.enumerate():
            if t.ident is None:
                continue
            ident = t.ident
            count = counts_snap.get(ident, 0)
            times = times_snap.get(ident, [])
            rate = sum(1 for ts in times if ts >= now - _RATE_WINDOW_SEC) / _RATE_WINDOW_SEC
            snap.threads.append(ThreadInfo(t, roles_snap.get(ident), count, rate))

        return snap


# ---------------------------------------------------------------------------
# Module-level singleton and public helpers
# ---------------------------------------------------------------------------

_collector: DiagnosticsCollector | None = None
_collector_lock = threading.Lock()


def _get_collector(history_seconds: int = 30) -> DiagnosticsCollector:
    global _collector
    with _collector_lock:
        if _collector is None:
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


def record_task_failure(task: Any, exc: BaseException) -> None:
    """Called by the runtime when a call_soon_threadsafe task raises an exception."""
    try:
        name = (
            getattr(task, "__qualname__", None)
            or getattr(task, "__name__", None)
            or repr(task)
        )
        failure = TaskFailure(str(name)[:80], exc, traceback.format_exc())
        _get_collector().record_failure(failure)
    except Exception:
        pass
