"""Runnable example: a background thread pushes live scatter updates."""

from __future__ import annotations

import math
import sys
import threading
import time
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual example guard
    raise SystemExit("This example requires NumPy for scatter point packing") from exc


ROWS = 160_000


class StreamFrame:
    columns = ("x", "y", "z")
    dtypes = ("float32", "float32", "float32")
    shape = (ROWS, 3)

    def __init__(self, phase: float) -> None:
        t = np.linspace(0.0, 1.0, ROWS, dtype=np.float32)
        theta = t * np.float32(math.tau * 16.0 + phase)
        self.x = np.cos(theta) * (np.float32(1.0) + np.float32(3.0) * t)
        self.y = np.sin(theta) * (np.float32(1.0) + np.float32(3.0) * t)
        self.z = (t - np.float32(0.5)) * np.float32(9.0)


@dg.component
def StreamingScatter(ctx: dg.ComponentCtx, initial: StreamFrame) -> dg.Window:
    running = ctx.state("running", False)
    status = ctx.state("status", "idle")
    generation = ctx.state("generation", 0)
    stop_event = ctx.state("stop_event", None)

    def start_stream() -> None:
        if running.value:
            return
        event = threading.Event()
        stop_event.set(event)
        running.set(True)
        status.set("streaming")

        def worker() -> None:
            next_generation = int(generation.value)
            while not event.wait(0.5):
                next_generation += 1
                frame = StreamFrame(next_generation * 0.35)
                try:
                    app = ctx.app
                    if app is None:
                        break
                    app.call_soon_threadsafe(
                        lambda frame=frame, n=next_generation: (
                            scatter.set_points(frame, x="x", y="y", z="z"),
                            generation.set(n),
                            status.set(f"updated {n}"),
                        )
                    )
                except RuntimeError:
                    break

        threading.Thread(target=worker, name="dragongui-scatter-stream", daemon=True).start()

    def stop_stream() -> None:
        event = stop_event.value
        if isinstance(event, threading.Event):
            event.set()
        running.set(False)
        status.set("stopped")

    win = dg.Window("Background Scatter Component", width=1100, height=720, id="stream-window", key="stream-window")
    with dg.HLayout(parent=win, style={"padding": 14, "gap": 16}):
        with dg.Panel("Stream controls", width=310, style={"padding": 14, "gap": 10}):
            dg.Label(str(status.value), id="status-label", key="status-label")
            dg.Label(f"Generation {generation.value}", id="generation-label", key="generation-label")
            dg.Button("Start", id="start", key="start", on_click=start_stream, disabled=bool(running.value))
            dg.Button("Stop", id="stop", key="stop", on_click=stop_stream, disabled=not bool(running.value))
            dg.Label("The worker pushes through app.call_soon_threadsafe(...).")
        scatter = dg.Scatter3D(
            initial,
            x="x",
            y="y",
            z="z",
            id="stream-scatter-component",
            key="stream-scatter-component",
            style={"flex": 1, "border_color": "border", "border_width": 1},
        )
    return win


def main() -> None:
    app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33"))
    try:
        print(app.run(StreamingScatter(StreamFrame(0.0))))
    finally:
        # Active stream threads are daemons, but let their stop event trip if
        # the current component instance still exists.
        pass


if __name__ == "__main__":
    main()
