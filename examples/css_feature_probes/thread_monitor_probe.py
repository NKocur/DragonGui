"""ThreadMonitor probe.

Verifies:
- Widget renders and refreshes at the configured rate.
- Thread inventory shows live / dead threads.
- Task queue depth samples are tracked and displayed.
- call_soon_threadsafe task failures appear in the failure log.
- register_thread_role / thread_role tag threads correctly.
- Burst tasks spike queue depth and drain cleanly.
"""
from __future__ import annotations

import sys
import threading
import time
from pathlib import Path

_REPO_PYTHON = str(Path(__file__).resolve().parents[2] / "python")
if _REPO_PYTHON not in sys.path:
    sys.path.insert(0, _REPO_PYTHON)

import dragongui as dg


_stop_event = threading.Event()
_producer: threading.Thread | None = None


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #0b1020;
        color: rgba(247, 250, 255, 0.94);
        font-size: 14px;
        padding: 12px;
        overflow-x: hidden;
        overflow-y: hidden;
    }

    HLayout.probe-root {
        width: 100%;
        flex-grow: 1;
        flex-shrink: 1;
        min-width: 0;
        min-height: 0;
        gap: 12px;
    }

    Label.title {
        color: white;
        font-size: 18px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(247, 250, 255, 0.68);
        font-size: 12px;
    }

    Panel.controls {
        width: 244px;
        max-width: 244px;
        height: 100%;
        min-height: 0;
        flex-shrink: 0;
        overflow-x: hidden;
        overflow-y: auto;
        gap: 8px;
        padding: 12px;
        padding-right: 28px;
    }

    Panel.controls::scrollbar-track,
    Panel.monitor-panel::scrollbar-track,
    ScrollArea::scrollbar-track {
        width: 8px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    Panel.controls::scrollbar-thumb,
    Panel.monitor-panel::scrollbar-thumb,
    ScrollArea::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.68);
        border-radius: 999px;
    }

    Panel.controls Button {
        width: 100%;
        flex-shrink: 0;
    }

    Panel.monitor-panel {
        flex-grow: 1;
        flex-shrink: 1;
        min-width: 0;
        height: 100%;
        min-height: 0;
        padding: 0;
        overflow: hidden;
    }

    Label.section {
        color: rgba(247, 250, 255, 0.56);
        font-size: 11px;
        font-weight: 700;
    }

    Label.ok   { color: #39ff88; font-size: 12px; }
    Label.warn { color: #ffcc33; font-size: 12px; }
    Label.fail { color: #ff4d6a; font-size: 12px; }

    Button.danger  { border-color: #ff4d6a; color: #ff4d6a; }
    Button.warning { border-color: #ffcc33; color: #ffcc33; }
"""
)


@dg.component
def ThreadMonitorProbe(_ctx: dg.ComponentCtx) -> dg.Window:
    win = dg.Window(
        "ThreadMonitor probe",
        width=960,
        height=640,
        style={"display": "flex", "flex_direction": "column", "min_width": 0, "min_height": 0},
    )

    with dg.HLayout(parent=win, class_="probe-root"):
        with dg.Panel("Controls", class_="controls"):
            dg.Label("BACKGROUND THREAD", class_="section")

            def _start() -> None:
                global _producer
                if _producer and _producer.is_alive():
                    _thread_status.set_value("already running")
                    return
                _stop_event.clear()

                @dg.thread_role("test-producer")
                def _run() -> None:
                    count = 0
                    while not _stop_event.wait(0.08):
                        count += 1
                        try:
                            app.call_soon_threadsafe(
                                lambda c=count: _counter.set_value(f"tasks sent: {c}")
                            )
                        except RuntimeError:
                            break

                _producer = threading.Thread(target=_run, name="test-producer", daemon=True)
                _producer.start()
                _thread_status.set_value("running")

            def _stop() -> None:
                _stop_event.set()
                _thread_status.set_value("stopped")

            dg.Button("Start producer", on_click=_start)
            dg.Button("Stop producer", on_click=_stop)
            _thread_status = dg.Label("idle", class_="caption")
            _counter = dg.Label("tasks sent: 0", class_="caption")

            dg.Separator()
            dg.Label("FAILURE CAPTURE", class_="section")

            def _trigger_fail() -> None:
                def _bad() -> None:
                    raise RuntimeError("probe deliberate failure")

                app.call_soon_threadsafe(_bad)
                _fail_status.set_value("failure scheduled")

            dg.Button("Schedule failure", on_click=_trigger_fail, class_="danger")
            _fail_status = dg.Label("none yet", class_="caption")

            dg.Separator()
            dg.Label("QUEUE BURST", class_="section")

            def _burst() -> None:
                for _ in range(60):
                    app.call_soon_threadsafe(lambda: None)
                _burst_status.set_value("60 tasks queued")

            dg.Button("Burst 60 tasks", on_click=_burst, class_="warning")
            _burst_status = dg.Label("idle", class_="caption")

            dg.Separator()
            dg.Label("THREAD ROLES", class_="section")

            def _spawn_named() -> None:
                def _named_worker() -> None:
                    with dg.thread_role("named-worker"):
                        time.sleep(4.0)

                thread = threading.Thread(target=_named_worker, name="named-worker", daemon=True)
                thread.start()
                _named_status.set_value("spawned (4 s lifetime)")

            dg.Button("Spawn named thread", on_click=_spawn_named)
            _named_status = dg.Label("none", class_="caption")

        dg.ThreadMonitor(
            key="monitor",
            show_threads=True,
            show_queue=True,
            show_failures=True,
            history_seconds=30,
            refresh_hz=4.0,
            class_="monitor-panel",
        )

    return win


if __name__ == "__main__":
    app.run(ThreadMonitorProbe())
