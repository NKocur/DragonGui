"""Thread Monitor demo.

Shows live threading diagnostics:
- Python task queue depth and rate.
- Active thread inventory with per-thread command rates.
- Task failures captured from call_soon_threadsafe.

Buttons on the right let you:
- Start / stop a background scatter producer.
- Trigger a stale-handle command drop.
- Schedule a task that deliberately raises an exception.
"""
from __future__ import annotations

import sys
import threading
import time
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import numpy as np
import pandas as pd

import dragongui as dg

# ---------------------------------------------------------------------------
# Shared state
# ---------------------------------------------------------------------------

_producer_stop = threading.Event()
_producer_thread: threading.Thread | None = None


def _make_scatter_data(n: int = 5000) -> pd.DataFrame:
    rng = np.random.default_rng()
    t = rng.uniform(0, 2 * np.pi, n)
    r = rng.uniform(0.5, 1.0, n)
    return pd.DataFrame(
        {
            "x": r * np.cos(t) + rng.normal(0, 0.05, n),
            "y": r * np.sin(t) + rng.normal(0, 0.05, n),
            "z": rng.uniform(-0.5, 0.5, n),
            "v": rng.uniform(0, 1, n),
        }
    )


# ---------------------------------------------------------------------------
# App
# ---------------------------------------------------------------------------

app = dg.App(theme=dg.Theme.dark())
app.stylesheet("""
    Panel.controls { gap: 8px; padding: 12px; width: 260px; flex-shrink: 0; }
    Panel.monitor  { flex-grow: 1; min-width: 240px; padding: 0; }
    Label.heading  { font-weight: 700; font-size: 13px; }
    Button.danger  { border-color: danger; color: danger; }
""")

win = dg.Window("Thread Monitor Demo", width=1100, height=680)

with dg.HLayout():

    # ── Controls ────────────────────────────────────────────────────────────
    with dg.Panel("Controls", class_="controls"):

        dg.Label("Producer thread", class_="heading")

        def start_producer() -> None:
            global _producer_thread
            if _producer_thread and _producer_thread.is_alive():
                return
            _producer_stop.clear()

            @dg.thread_role("scatter-producer")
            def _run() -> None:
                while not _producer_stop.wait(0.25):
                    df = _make_scatter_data()
                    try:
                        app.call_soon_threadsafe(
                            lambda d=df: scatter.set_points(d, x="x", y="y", z="z", scalars=d["v"])
                        )
                    except RuntimeError:
                        break

            _producer_thread = threading.Thread(target=_run, name="scatter-producer", daemon=True)
            _producer_thread.start()
            status_label.set_value(text="Producer running")

        def stop_producer() -> None:
            _producer_stop.set()
            status_label.set_value(text="Producer stopped")

        dg.Button("Start producer", on_click=start_producer)
        dg.Button("Stop producer", on_click=stop_producer)

        dg.Separator()
        dg.Label("Diagnostics triggers", class_="heading")

        def trigger_failure() -> None:
            def _bad_task() -> None:
                raise ValueError("deliberate test failure from thread_monitor_demo")

            app.call_soon_threadsafe(_bad_task)

        def trigger_burst() -> None:
            for _ in range(40):
                app.call_soon_threadsafe(lambda: None)

        dg.Button("Schedule failing task", on_click=trigger_failure, class_="danger")
        dg.Button("Burst 40 tasks", on_click=trigger_burst)

        dg.Separator()
        status_label = dg.Label("Idle", style={"color": "muted_text", "font_size": 12})

    # ── Scatter + monitor ────────────────────────────────────────────────────
    with dg.VLayout(style={"flex_grow": 1, "gap": 8}):

        scatter = dg.Scatter3D(
            _make_scatter_data(),
            x="x", y="y", z="z",
            scalars="v",
            colormap="viridis",
            style={"flex_grow": 1, "min_height": 300},
        )

        dg.ThreadMonitor(
            key="monitor",
            show_threads=True,
            show_queue=True,
            show_failures=True,
            refresh_hz=4,
            history_seconds=30,
            class_="monitor",
            style={"height": 260, "flex_shrink": 0},
        )

app.run(win)
