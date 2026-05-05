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


def _fmt_age(ts_ms: int, now_ms: int) -> str:
    age_s = max(0, now_ms - ts_ms) // 1000
    if age_s < 60:
        return f"{age_s}s ago"
    if age_s < 3600:
        return f"{age_s // 60}m ago"
    return f"{age_s // 3600}h ago"


_S_MUTED    = {"color": "muted_text", "font_size": 11}
_S_HEADER   = {"color": "muted_text", "font_size": 10, "font_weight": 700}
_S_DANGER   = {"color": "danger",     "font_size": 11}
_S_FAIL_DET = {"color": "muted_text", "font_size": 10}
_S_BLOCK    = {"font_size": 11, "font_family": "monospace"}
_S_DOT_LIVE = {"color": "text",   "font_size": 11, "font_family": "monospace", "width": 12, "flex_shrink": 0}
_S_DOT_DEAD = {"color": "danger", "font_size": 11, "font_family": "monospace", "width": 12, "flex_shrink": 0}
_S_THR_TEXT = {"font_size": 11, "font_family": "monospace", "flex_grow": 1}
_S_THR_ROW  = {"gap": 0, "width": "100%", "align_items": "center", "flex_grow": 0, "height": 15}


def _section(text: str) -> None:
    Label(text, style=_S_HEADER)


def _format_queue_block(snap: DiagnosticsSnapshot) -> str:
    rate = _fmt_rate(snap.enqueue_rate) if snap.enqueue_rate >= 0.05 else "—"
    depth = (
        f"{snap.queue_depth} / {snap.queue_max}"
        if snap.queue_max > 0
        else str(snap.queue_depth)
    )
    return (
        f"depth / max  {depth}\n"
        f"avg          {snap.queue_avg:.1f}\n"
        f"total        {_fmt_count(snap.enqueued_total)}\n"
        f"rate         {rate}"
    )


def _format_thread_stat(t: Any) -> str:
    parts = []
    if t.cmd_count > 0:
        parts.append(_fmt_count(t.cmd_count))
    if t.cmd_per_sec >= 0.05:
        parts.append(_fmt_rate(t.cmd_per_sec))
    return "  ".join(parts)




def _build_panel(
    snap: DiagnosticsSnapshot,
    show_threads: bool,
    show_queue: bool,
    show_failures: bool,
    title: str | None,
    padding: int | float,
    id: str | None,
    class_: str | None,
    style: Mapping[str, Any] | None,
) -> Panel:
    now_ms = int(time.time() * 1000)
    panel_style: dict[str, Any] = {"padding": float(padding), **(style or {})}
    with Panel(title, id=id, key="__tm_root", class_=class_, style=panel_style) as panel:
        with ScrollArea(
            key="__tm_scroll",
            style={
                "flex_grow": 1,
                "min_width": 0,
                "overflow_x": "hidden",
                "overflow_y": "auto",
            },
        ):
            with VLayout(
                style={
                    "gap": 2,
                    "min_width": 0,
                    "width": "100%",
                },
            ):

                # ── Queue ────────────────────────────────────────────────
                if show_queue:
                    _section("QUEUE")
                    Label(_format_queue_block(snap), style=_S_BLOCK)
                    if snap.queue_max > 0:
                        ProgressBar(
                            snap.queue_depth / snap.queue_max,
                            style={
                                "height": 4,
                                "margin_top": 4,
                                "margin_bottom": 6,
                                "margin_left": 8,
                                "margin_right": 8,
                            },
                        )
                    if show_threads or show_failures:
                        Separator()

                # ── Threads ──────────────────────────────────────────────
                if show_threads:
                    alive = sum(1 for t in snap.threads if t.alive)
                    dead  = len(snap.threads) - alive
                    header = f"THREADS  {alive} alive" + (f"  {dead} dead" if dead else "")
                    _section(header)

                    if snap.threads:
                        # Tight inner VLayout (gap=0) so thread rows pack
                        # closer than the outer section gap. margin_bottom
                        # adds breathing room before the next Separator so
                        # the last row's text doesn't visually butt against it.
                        with VLayout(style={"gap": 0, "width": "100%", "flex_grow": 0, "margin_bottom": 14}):
                            for t in snap.threads:
                                name = t.role or t.name
                                if t.daemon:
                                    name = f"{name} D"
                                stat = _format_thread_stat(t)
                                with HLayout(style=_S_THR_ROW):
                                    Label("●" if t.alive else "○",
                                          style=_S_DOT_LIVE if t.alive else _S_DOT_DEAD)
                                    Label(f"{name:<28}{stat:>14}", style=_S_THR_TEXT)
                    else:
                        Label("no threads", style=_S_MUTED)

                    if show_failures:
                        Separator()

                # ── Task Failures ────────────────────────────────────────
                if show_failures:
                    _section(f"TASK FAILURES  {snap.failure_count}")
                    if snap.recent_failures:
                        for f in snap.recent_failures:
                            Label(
                                f"{f.exc_type}: {f.exc_msg}",
                                style=_S_DANGER,
                            )
                            Label(
                                f"  {f.callable_repr}  ·  {f.thread_name}  ·  {_fmt_age(f.ts_ms, now_ms)}",
                                style=_S_FAIL_DET,
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
    title: str | None = "Thread Monitor",
    padding: int | float = 6,
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
    title:
        Optional panel title. Pass ``None`` or ``""`` for a compact untitled
        monitor with no reserved title band.
    padding:
        Inner panel padding in logical pixels. Inline ``style`` can still
        override this by setting ``padding`` explicitly.
    """
    if float(padding) < 0:
        raise ValueError("ThreadMonitor padding must be non-negative")
    snap = ctx.state("snap", DiagnosticsSnapshot())
    # Use a mutable list to track one-shot startup without triggering re-render.
    once: list[bool] = ctx.state("_once", [False]).value
    if not once[0] and ctx.app is not None:
        once[0] = True
        _start_monitor(ctx.app, snap, history_seconds, refresh_hz)

    return _build_panel(
        snap.value,
        show_threads,
        show_queue,
        show_failures,
        title or None,
        padding,
        id,
        class_,
        style,
    )
