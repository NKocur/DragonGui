"""ThreadMonitor widget — embeddable diagnostics panel for DragonGUI apps."""
from __future__ import annotations

import builtins
import threading
import time
from collections.abc import Callable, Mapping
from typing import TYPE_CHECKING, Any

from .components import ComponentCtx, component
from .diagnostics import DiagnosticsSnapshot, _get_collector
from .widgets import (
    Button,
    GridLayout,
    LED,
    Label,
    Panel,
    ProgressBar,
    ScrollArea,
    Separator,
    Tag,
    VLayout,
)

if TYPE_CHECKING:
    from .runtime import AppHandle
    from .components import StateSlot


# ---------------------------------------------------------------------------
# Background poll thread
# ---------------------------------------------------------------------------


def _start_monitor(
    app: AppHandle,
    snap_slot: StateSlot,
    history_seconds: int,
    refresh_hz: float,
    *,
    show_threads: bool = True,
    show_queue: bool = True,
    show_failures: bool = True,
    max_threads: int = 80,
    max_dead_threads: int = 20,
    enabled: bool | Callable[[], bool] = True,
) -> threading.Thread:
    collector = _get_collector(history_seconds)
    interval = max(1.0 / 15.0, 1.0 / max(1.0, float(refresh_hz)))
    last_fingerprint: tuple[Any, ...] | None = None

    def _is_attached() -> bool:
        runtime = getattr(snap_slot, "_runtime", None)
        return runtime is None or getattr(runtime, "app_handle", None) is app

    def _apply_snapshot(snap: DiagnosticsSnapshot) -> None:
        if _is_attached():
            snap_slot.set(snap)

    def _enabled() -> bool:
        if callable(enabled):
            try:
                return bool(enabled())
            except Exception:
                return False
        return bool(enabled)

    def _run() -> None:
        nonlocal last_fingerprint
        while not app.closed and _is_attached():
            try:
                if not _enabled():
                    last_fingerprint = None
                    time.sleep(interval)
                    continue
                with app._lock:
                    sender = app._native_sender
                if show_queue:
                    depth = sender.queue_depth() if sender is not None else len(app._tasks)
                    collector.record_queue_depth(depth)
                new_snap = collector.snapshot(
                    include_threads=show_threads,
                    include_failures=show_failures,
                    max_threads=max_threads,
                    max_dead_threads=max_dead_threads,
                )
                fingerprint = _snapshot_fingerprint(
                    new_snap,
                    show_threads=show_threads,
                    show_queue=show_queue,
                    show_failures=show_failures,
                )
                if fingerprint == last_fingerprint:
                    time.sleep(interval)
                    continue
                last_fingerprint = fingerprint
                try:
                    app.call_soon_threadsafe(
                        lambda s=new_snap: _apply_snapshot(s),
                        _diagnostics=False,
                    )
                except RuntimeError:
                    break
            except Exception:
                pass
            time.sleep(interval)

    t = threading.Thread(target=_run, name="dg-thread-monitor", daemon=True)
    t.start()
    return t


def _snapshot_fingerprint(
    snap: DiagnosticsSnapshot,
    *,
    show_threads: bool,
    show_queue: bool,
    show_failures: bool,
) -> tuple[Any, ...]:
    parts: list[Any] = []
    now_ms = snap.ts_ms or int(time.time() * 1000)
    if show_queue:
        parts.extend(
            (
                snap.queue_depth,
                snap.queue_max,
                round(snap.queue_avg, 1),
                snap.enqueued_total,
                round(snap.enqueue_rate, 1),
            )
        )
    if show_threads:
        parts.extend((snap.thread_total, snap.thread_alive, snap.thread_dead))
        parts.extend(
            (
                thread.ident,
                thread.name,
                thread.role,
                thread.alive,
                thread.daemon,
                thread.cmd_count,
                round(thread.cmd_per_sec, 1),
            )
            for thread in snap.threads
        )
    if show_failures:
        parts.append(snap.failure_count)
        parts.extend(
            (
                failure.ts_ms,
                _failure_age_bucket(now_ms, failure.ts_ms),
                failure.callable_repr,
                failure.thread_name,
                failure.thread_role,
                failure.exc_type,
                failure.exc_msg,
            )
            for failure in snap.recent_failures
        )
    return tuple(parts)


def _failure_age_bucket(now_ms: int, ts_ms: int) -> tuple[str, int]:
    age_s = max(0, now_ms - ts_ms) // 1000
    if age_s < 60:
        return ("s", age_s)
    if age_s < 3600:
        return ("m", age_s // 60)
    return ("h", age_s // 3600)


# ---------------------------------------------------------------------------
# Rendering helpers
# ---------------------------------------------------------------------------


def _fmt_rate(r: float) -> str:
    if r < 0.1:
        return "0/s"
    if r < 10.0:
        return f"{r:.1f}/s"
    return f"{r:.0f}/s"


def _fmt_count(n: int) -> str:
    if n >= 1_000_000:
        return f"{n / 1_000_000:.1f}M"
    if n >= 1_000:
        return f"{n / 1_000:.1f}k"
    return str(n)


def _fmt_age(ts_ms: int) -> str:
    age_s = max(0, int(time.time() * 1000) - ts_ms) // 1000
    if age_s < 60:
        return f"{age_s}s ago"
    if age_s < 3600:
        return f"{age_s // 60}m ago"
    return f"{age_s // 3600}h ago"


_S_PANEL = {
    "padding": 0,
    "gap": 0,
    "min_width": 0,
    "min_height": 0,
    "overflow": "hidden",
}
_S_HEADER_BAR = {
    "padding": 8,
    "padding_bottom": 7,
    "gap": 6,
    "width": "100%",
    "flex_grow": 0,
    "flex_shrink": 0,
    "align_items": "center",
    "background": "surface_alt",
}
_S_HEADER_TEXT = {
    "gap": 1,
    "min_width": 0,
    "width": "100%",
    "height": 38,
    "flex_grow": 0,
    "flex_shrink": 0,
}
_S_TITLE = {
    "color": "text",
    "font_size": 14,
    "font_weight": 750,
    "height": 20,
    "line_height": "18px",
}
_S_SUBTITLE = {
    "color": "muted_text",
    "font_size": 11,
    "height": 17,
    "line_height": "16px",
    "min_width": 0,
}
_S_BODY = {
    "gap": 8,
    "padding": 8,
    "padding_right": 18,
    "flex_grow": 0,
    "flex_shrink": 0,
    "min_width": 0,
    "width": "100%",
}
_S_SECTION = {
    "gap": 6,
    "width": "100%",
    "height": 18,
    "flex_grow": 0,
    "flex_shrink": 0,
    "align_items": "center",
}
_S_SECTION_TITLE = {
    "color": "muted_text",
    "font_size": 11,
    "font_weight": 700,
    "height": 18,
    "line_height": "17px",
}
_S_MUTED = {"color": "muted_text", "font_size": 12, "height": 18, "line_height": "17px"}
_S_COL_HDR = {
    "color": "muted_text",
    "font_size": 10,
    "font_weight": 700,
    "height": 16,
    "line_height": "15px",
}
_S_DANGER = {"color": "danger", "font_size": 12, "height": 18, "line_height": "17px"}
_S_METRIC_KEY = {
    "color": "muted_text",
    "font_size": 11,
    "height": 18,
    "line_height": "17px",
}
_S_METRIC_VAL = {
    "color": "text",
    "font_size": 12,
    "font_weight": 600,
    "height": 18,
    "line_height": "17px",
}
_S_STATUS = {"font_size": 12, "font_weight": 600, "height": 18, "line_height": "17px"}
_S_DAEMON = {"font_size": 10, "color": "muted_text", "height": 16, "line_height": "15px"}
_S_STAT = {"font_size": 12, "color": "muted_text", "height": 18, "line_height": "17px"}
_S_NAME = {"font_size": 12, "height": 18, "line_height": "17px", "min_width": 0}
_S_FAILURE_META = {
    "color": "muted_text",
    "font_size": 11,
    "height": 17,
    "line_height": "16px",
    "min_width": 0,
}
_S_TAG = {
    "font_size": 10,
    "height": 18,
    "line_height": "14px",
    "flex_grow": 0,
    "flex_shrink": 0,
}
_S_HEADER_BUTTON = {
    "height": 24,
    "min_width": 66,
    "padding": 4,
    "padding_left": 8,
    "padding_right": 8,
    "font_size": 11,
    "line_height": "14px",
    "flex_grow": 0,
    "flex_shrink": 0,
}
_S_HEADER_LED = {
    "align_self": "center",
    "flex_grow": 0,
    "flex_shrink": 0,
    "transform": {"translate_y": 15},
}
_S_ROW_LED = {
    "align_self": "center",
    "flex_grow": 0,
    "flex_shrink": 0,
    "transform": {"translate_y": 5},
}
_S_FAILURE_LED = {
    "align_self": "center",
    "flex_grow": 0,
    "flex_shrink": 0,
    "transform": {"translate_y": 11},
}
_HEADER_COLUMNS = (10, "1fr", "auto", "auto")
_SECTION_COLUMNS = ("1fr", "auto")
_METRIC_COLUMNS = (52, "1fr", 52, "1fr")
_THREAD_COLUMNS = (10, 44, 16, "1fr", 84)
_FAILURE_COLUMNS = (10, "1fr", "auto")


def _overall_status(snap: DiagnosticsSnapshot, monitoring_enabled: bool) -> tuple[str, str, str]:
    if not monitoring_enabled:
        return ("paused", "neutral", "warning")
    if snap.failure_count:
        return ("issues", "danger", "danger")
    if snap.queue_depth > 0:
        return ("busy", "warning", "warning")
    return ("ok", "success", "success")


def _summary_line(
    snap: DiagnosticsSnapshot,
    show_queue: bool,
    show_threads: bool,
    monitoring_enabled: bool,
) -> str:
    if not monitoring_enabled:
        return "updates paused"
    parts: list[str] = []
    if show_queue:
        parts.append(f"queue {snap.queue_depth}")
        if snap.enqueue_rate >= 0.05:
            parts.append(_fmt_rate(snap.enqueue_rate))
    if show_threads:
        parts.append(f"{snap.thread_alive} alive")
        if snap.thread_dead:
            parts.append(f"{snap.thread_dead} dead")
    if not parts:
        return "diagnostics"
    return " | ".join(parts)


def _header(
    snap: DiagnosticsSnapshot,
    show_queue: bool,
    show_threads: bool,
    monitoring_enabled: bool,
    on_toggle: Callable[[], None] | None,
) -> None:
    status, level, led_state = _overall_status(snap, monitoring_enabled)
    with GridLayout(template_columns=_HEADER_COLUMNS, gap=8, key="header", style=_S_HEADER_BAR):
        LED(
            led_state,
            states={"success": "success", "warning": "warning", "danger": "danger"},
            size=9,
            key="header-led",
            style=_S_HEADER_LED,
        )
        with VLayout(key="header-copy", style=_S_HEADER_TEXT):
            Label("Thread Monitor", key="header-title", style=_S_TITLE, wrap=False)
            Label(
                _summary_line(snap, show_queue, show_threads, monitoring_enabled),
                key="header-subtitle",
                style=_S_SUBTITLE,
                wrap=False,
            )
        Tag(status.upper(), level=level, key="header-status", style=_S_TAG)
        Button(
            "Disable" if monitoring_enabled else "Enable",
            on_click=on_toggle,
            key="header-toggle",
            style=_S_HEADER_BUTTON,
            tooltip="Pause or resume ThreadMonitor background refreshes.",
        )


def _section(
    title: str,
    meta: str | None = None,
    *,
    level: str = "neutral",
    key: str,
) -> None:
    with GridLayout(
        template_columns=_SECTION_COLUMNS,
        gap=8,
        key=f"{key}-section",
        style=_S_SECTION,
    ):
        Label(title, key=f"{key}-section-title", style=_S_SECTION_TITLE, wrap=False)
        if meta:
            Tag(meta, level=level, key=f"{key}-section-meta", style=_S_TAG)
        else:
            Label("", key=f"{key}-section-meta-empty", style=_S_MUTED, wrap=False)


def _metrics(items: list[tuple[str, str]], *, key: str) -> None:
    with GridLayout(
        template_columns=_METRIC_COLUMNS,
        gap=6,
        row_gap=2,
        key=f"{key}-metrics",
        style={"width": "100%", "min_width": 0, "flex_grow": 0, "flex_shrink": 0},
    ):
        for index, (metric_key, val) in enumerate(items):
            Label(
                metric_key,
                key=f"{key}-metric-{index}-key",
                style=_S_METRIC_KEY,
                wrap=False,
            )
            Label(val, key=f"{key}-metric-{index}-value", style=_S_METRIC_VAL, wrap=False)
        if len(items) % 2:
            Label("", key=f"{key}-metric-pad-key", style=_S_METRIC_KEY, wrap=False)
            Label("", key=f"{key}-metric-pad-value", style=_S_METRIC_VAL, wrap=False)


def _thread_tasks_text(count: int, rate: float) -> str:
    parts = [_fmt_count(count)]
    if rate >= 0.05:
        parts.append(_fmt_rate(rate))
    return "  ".join(parts)


def _thread_table(snap: DiagnosticsSnapshot) -> None:
    with GridLayout(
        template_columns=_THREAD_COLUMNS,
        gap=5,
        row_gap=2,
        key="threads-table",
        style={
            "width": "100%",
            "min_width": 0,
            "flex_grow": 0,
            "flex_shrink": 0,
            "align_items": "center",
        },
    ):
        Label("", key="threads-head-led", style=_S_COL_HDR, wrap=False)
        Label("STATE", key="threads-head-state", style=_S_COL_HDR, wrap=False)
        Label("D", key="threads-head-daemon", style=_S_COL_HDR, wrap=False)
        Label("THREAD / ROLE", key="threads-head-name", style=_S_COL_HDR, wrap=False)
        Label("TASKS", key="threads-head-tasks", style=_S_COL_HDR, wrap=False)
        for index, thread in enumerate(snap.threads):
            row_key = f"thread-{thread.ident if thread.ident is not None else index}"
            state = "alive" if thread.alive else "dead"
            color = "text" if thread.alive else "danger"
            LED(
                state,
                states={"alive": "success", "dead": "danger"},
                size=8,
                key=f"{row_key}-led",
                style=_S_ROW_LED,
            )
            Label(state, key=f"{row_key}-state", style={**_S_STATUS, "color": color}, wrap=False)
            Label(
                "D" if thread.daemon else "",
                key=f"{row_key}-daemon",
                style=_S_DAEMON,
                wrap=False,
            )
            Label(thread.role or thread.name, key=f"{row_key}-name", style=_S_NAME, wrap=False)
            Label(
                _thread_tasks_text(thread.cmd_count, thread.cmd_per_sec),
                key=f"{row_key}-tasks",
                style=_S_STAT,
                wrap=False,
            )


def _failure_table(snap: DiagnosticsSnapshot) -> None:
    with GridLayout(
        template_columns=_FAILURE_COLUMNS,
        gap=6,
        row_gap=4,
        key="failures-table",
        style={
            "width": "100%",
            "min_width": 0,
            "flex_grow": 0,
            "flex_shrink": 0,
            "align_items": "center",
        },
    ):
        for index, failure in enumerate(snap.recent_failures):
            row_key = f"failure-{failure.ts_ms}-{index}"
            LED(
                "failure",
                states={"failure": "danger"},
                size=8,
                key=f"{row_key}-led",
                style=_S_FAILURE_LED,
            )
            with VLayout(
                key=f"{row_key}-copy",
                style={"gap": 1, "min_width": 0, "width": "100%", "flex_grow": 0},
            ):
                Label(
                    f"{failure.exc_type}: {failure.exc_msg}",
                    key=f"{row_key}-message",
                    style=_S_DANGER,
                    wrap=False,
                )
                Label(
                    f"{failure.callable_repr} | {failure.thread_role or failure.thread_name}",
                    key=f"{row_key}-meta",
                    style=_S_FAILURE_META,
                    wrap=False,
                )
            Label(
                _fmt_age(failure.ts_ms),
                key=f"{row_key}-age",
                style=_S_FAILURE_META,
                wrap=False,
            )


def _build_panel(
    snap: DiagnosticsSnapshot,
    show_threads: bool,
    show_queue: bool,
    show_failures: bool,
    id: str | None,
    class_: str | None,
    style: Mapping[str, Any] | None,
    root_key: str = "thread-monitor-root",
    monitoring_enabled: bool = True,
    on_toggle: Callable[[], None] | None = None,
) -> Panel:
    panel_style: dict[str, Any] = {**_S_PANEL, **(style or {})}
    with Panel("", id=id, key=root_key, class_=class_, style=panel_style) as panel:
        _header(snap, show_queue, show_threads, monitoring_enabled, on_toggle)
        Separator(key="header-separator")
        with ScrollArea(
            key="body-scroll",
            style={
                "flex_grow": 1,
                "flex_shrink": 1,
                "min_width": 0,
                "min_height": 0,
                "overflow_x": "hidden",
                "overflow_y": "auto",
            },
        ):
            with VLayout(key="body", style=_S_BODY):

                # Queue
                if show_queue:
                    rate_str = (
                        _fmt_rate(snap.enqueue_rate)
                        if snap.enqueue_rate >= 0.05
                        else "idle"
                    )
                    _section("Queue", rate_str, level="info", key="queue")
                    _metrics(
                        [
                            ("depth", str(snap.queue_depth)),
                            ("max", str(snap.queue_max)),
                            ("avg", f"{snap.queue_avg:.1f}"),
                            ("total", _fmt_count(snap.enqueued_total)),
                            ("rate", rate_str),
                        ],
                        key="queue",
                    )
                    if snap.queue_max > 0:
                        ProgressBar(
                            snap.queue_depth / snap.queue_max,
                            key="queue-progress",
                            style={"height": 6, "width": "100%"},
                        )
                    if show_threads or show_failures:
                        Separator(key="queue-separator")

                # Threads
                if show_threads:
                    alive = snap.thread_alive
                    dead = snap.thread_dead
                    thread_level = "warning" if dead else "success"
                    thread_meta = f"{alive} alive"
                    if dead:
                        thread_meta = f"{thread_meta} / {dead} dead"
                    shown_str = (
                        f"Showing {len(snap.threads)} of {snap.thread_total}."
                        if snap.thread_total > len(snap.threads)
                        else ""
                    )
                    _section("Threads", thread_meta, level=thread_level, key="threads")

                    if not snap.threads:
                        Label(
                            "No threads recorded.",
                            key="threads-empty",
                            style=_S_MUTED,
                            wrap=False,
                        )
                    else:
                        _thread_table(snap)
                    if shown_str:
                        Label(shown_str, key="threads-shown", style=_S_MUTED, wrap=False)

                    if show_failures:
                        Separator(key="threads-separator")

                # Task failures
                if show_failures:
                    failure_level = "danger" if snap.failure_count else "success"
                    _section(
                        "Task Failures",
                        f"{snap.failure_count} total",
                        level=failure_level,
                        key="failures",
                    )
                    if snap.recent_failures:
                        _failure_table(snap)
                    else:
                        Label(
                            "No task failures recorded.",
                            key="failures-empty",
                            style=_S_MUTED,
                            wrap=False,
                        )

    return panel


# ---------------------------------------------------------------------------
# Component
# ---------------------------------------------------------------------------


@component
def ThreadMonitor(
    ctx: ComponentCtx,
    *,
    show_threads: bool = True,
    show_queue: bool = True,
    show_failures: bool = True,
    history_seconds: int = 30,
    refresh_hz: float = 4.0,
    max_threads: int = 80,
    max_dead_threads: int = 20,
    enabled: bool | Callable[[], bool] = True,
    id: str | None = None,
    class_: str | None = None,
    style: Mapping[str, Any] | None = None,
) -> Panel:
    """Embeddable diagnostics panel showing live thread and task queue health.

    Usage::

        with dg.Sidebar("Debug"):
            dg.ThreadMonitor(key="monitor")

    Parameters
    ----------
    show_threads:
        Show the Python thread inventory table.
    show_queue:
        Show the task queue depth and history.
    show_failures:
        Show recent ``call_soon_threadsafe`` task failures.
    history_seconds:
        Rolling window in seconds for queue depth history.
    refresh_hz:
        Refresh rate in Hz (1–15). Default 4.
    max_threads:
        Maximum number of thread rows to render. Highest-rate live producers are kept first.
    max_dead_threads:
        Maximum number of recently dead thread rows retained in diagnostics.
    enabled:
        Boolean or predicate that controls background refreshes. When false, the
        monitor stays mounted but does not poll diagnostics or enqueue UI patches.
    """
    snap = ctx.state("snap", DiagnosticsSnapshot())
    root_key = ctx.state("_root_key", f"thread-monitor-{builtins.id(ctx._runtime)}").value
    manual_enabled = ctx.state("_manual_enabled", True)
    enabled_ref = ctx.state("_enabled_ref", {"enabled": enabled}).value
    if isinstance(enabled_ref, dict):
        enabled_ref["enabled"] = enabled

    def _external_enabled() -> bool:
        current = enabled_ref.get("enabled", True) if isinstance(enabled_ref, dict) else enabled
        if callable(current):
            try:
                return bool(current())
            except Exception:
                return False
        return bool(current)

    def _monitor_enabled() -> bool:
        return bool(manual_enabled.value) and _external_enabled()

    def _toggle_monitor() -> None:
        manual_enabled.set(not bool(manual_enabled.value))

    # Use a mutable list to track one-shot startup without triggering re-render.
    once: list[bool] = ctx.state("_once", [False]).value
    if not once[0] and ctx.app is not None:
        once[0] = True
        _start_monitor(
            ctx.app,
            snap,
            history_seconds,
            refresh_hz,
            show_threads=show_threads,
            show_queue=show_queue,
            show_failures=show_failures,
            max_threads=max_threads,
            max_dead_threads=max_dead_threads,
            enabled=_monitor_enabled,
        )

    return _build_panel(
        snap.value,
        show_threads,
        show_queue,
        show_failures,
        id,
        class_,
        style,
        root_key=str(root_key),
        monitoring_enabled=bool(manual_enabled.value),
        on_toggle=_toggle_monitor,
    )
