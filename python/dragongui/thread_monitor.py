"""ThreadMonitor widget — embeddable diagnostics panel for DragonGUI apps."""
from __future__ import annotations

import threading
import time
from collections.abc import Mapping
from typing import TYPE_CHECKING, Any

from .components import ComponentCtx, component
from .diagnostics import DiagnosticsSnapshot, _get_collector
from .widgets import (
    HLayout,
    Label,
    Panel,
    ProgressBar,
    ScrollArea,
    Separator,
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
) -> None:
    collector = _get_collector(history_seconds)
    interval = max(1.0 / 15.0, 1.0 / max(1.0, float(refresh_hz)))

    def _run() -> None:
        while not app.closed:
            try:
                with app._lock:
                    sender = app._native_sender
                depth = sender.queue_depth() if sender is not None else len(app._tasks)
                collector.record_queue_depth(depth)
                new_snap = collector.snapshot()
                try:
                    app.call_soon_threadsafe(lambda s=new_snap: snap_slot.set(s))
                except RuntimeError:
                    break
            except Exception:
                pass
            time.sleep(interval)

    t = threading.Thread(target=_run, name="dg-thread-monitor", daemon=True)
    t.start()


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


_S_MUTED   = {"color": "muted_text", "font_size": 12}
_S_HEADER  = {"color": "muted_text", "font_size": 11, "font_weight": 600}
_S_COL_HDR = {"color": "muted_text", "font_size": 10, "font_weight": 600}
_S_DANGER  = {"color": "danger",     "font_size": 12}
_S_ROW     = {"gap": 6, "width": "100%", "height": 20, "align_items": "center"}
_S_STATUS  = {"width": 42, "flex_shrink": 0, "font_size": 12}
_S_DAEMON  = {"width": 14, "flex_shrink": 0, "font_size": 10, "color": "muted_text"}
_S_STAT    = {"width": 84, "flex_shrink": 0, **_S_MUTED}
_S_NAME    = {"flex_grow": 1, "flex_shrink": 1, "font_size": 12}
_S_KV_KEY  = {"width": 44, "flex_shrink": 0, "font_size": 11, "color": "muted_text"}
_S_KV_VAL  = {"flex_grow": 1, "font_size": 12}
_S_KV_ROW  = {"gap": 6, "width": "100%", "height": 18, "align_items": "center"}


def _section(text: str) -> None:
    Label(text, style=_S_HEADER)


def _kv(key: str, val: str) -> None:
    with HLayout(style=_S_KV_ROW):
        Label(key, style=_S_KV_KEY)
        Label(val, style=_S_KV_VAL)


def _build_panel(
    snap: DiagnosticsSnapshot,
    show_threads: bool,
    show_queue: bool,
    show_failures: bool,
    id: str | None,
    class_: str | None,
    style: Mapping[str, Any] | None,
) -> Panel:
    panel_style: dict[str, Any] = {"padding": 0, **(style or {})}
    with Panel("Thread Monitor", id=id, class_=class_, style=panel_style) as panel:
        with ScrollArea(
            style={
                "flex_grow": 1,
                "min_width": 0,
                "overflow_x": "hidden",
                "overflow_y": "auto",
            },
        ):
            with VLayout(
                style={
                    "gap": 8,
                    "padding": 10,
                    "padding_right": 26,
                    "min_width": 0,
                    "width": "100%",
                },
            ):

                # ── Queue ────────────────────────────────────────────────
                if show_queue:
                    _section("QUEUE")
                    rate_str = f"  {_fmt_rate(snap.enqueue_rate)}" if snap.enqueue_rate >= 0.05 else "—"
                    _kv("depth", str(snap.queue_depth))
                    _kv("max",   str(snap.queue_max))
                    _kv("avg",   f"{snap.queue_avg:.1f}")
                    _kv("total", _fmt_count(snap.enqueued_total))
                    _kv("rate",  rate_str)
                    if snap.queue_max > 0:
                        ProgressBar(
                            snap.queue_depth / snap.queue_max,
                            style={"height": 5, "width": "100%"},
                        )
                    if show_threads or show_failures:
                        Separator()

                # ── Threads ──────────────────────────────────────────────
                if show_threads:
                    alive = sum(1 for t in snap.threads if t.alive)
                    dead  = len(snap.threads) - alive
                    dead_str = f"  {dead} dead" if dead else ""
                    _section(f"THREADS  {alive} alive{dead_str}")

                    with HLayout(style={**_S_ROW, "height": 16}):
                        Label("STATUS", style={**_S_STATUS, **_S_COL_HDR})
                        Label("",       style=_S_DAEMON)
                        Label("NAME / ROLE", style={**_S_NAME, **_S_COL_HDR})
                        Label("TASKS",  style={**_S_STAT, **_S_COL_HDR})

                    for t in snap.threads:
                        with HLayout(style=_S_ROW):
                            color = "text" if t.alive else "danger"
                            status = "alive" if t.alive else "dead"
                            Label(status, style={**_S_STATUS, "color": color})
                            Label("D" if t.daemon else "", style=_S_DAEMON)
                            Label(t.role or t.name, style=_S_NAME)
                            parts = []
                            if t.cmd_count > 0:
                                parts.append(_fmt_count(t.cmd_count))
                            if t.cmd_per_sec >= 0.05:
                                parts.append(_fmt_rate(t.cmd_per_sec))
                            Label("  ·  ".join(parts), style=_S_STAT)

                    if not snap.threads:
                        Label("no threads", style=_S_MUTED)

                    if show_failures:
                        Separator()

                # ── Task Failures ────────────────────────────────────────
                if show_failures:
                    fail_color = "danger" if snap.failure_count else "muted_text"
                    _section(f"TASK FAILURES  {snap.failure_count}")
                    if snap.recent_failures:
                        for f in snap.recent_failures:
                            Label(
                                f"{f.exc_type}: {f.exc_msg}",
                                style=_S_DANGER,
                            )
                            Label(
                                f"  {f.callable_repr}  ·  {f.thread_name}  ·  {_fmt_age(f.ts_ms)}",
                                style={**_S_MUTED, "font_size": 11},
                            )
                    else:
                        Label("no failures", style=_S_MUTED)

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
    """
    snap = ctx.state("snap", DiagnosticsSnapshot())
    # Use a mutable list to track one-shot startup without triggering re-render.
    once: list[bool] = ctx.state("_once", [False]).value
    if not once[0] and ctx.app is not None:
        once[0] = True
        _start_monitor(ctx.app, snap, history_seconds, refresh_hz)

    return _build_panel(snap.value, show_threads, show_queue, show_failures, id, class_, style)
