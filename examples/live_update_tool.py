from __future__ import annotations

import math
import sys
import threading
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


app = dg.App()
win = dg.Window("DragonGUI Live Update Demo", width=760, height=430)

counter = {"value": 0}
stream_stop = threading.Event()
stream_thread: threading.Thread | None = None


def apply_next_state(prefix: str = "Click") -> None:
    counter["value"] += 1
    n = counter["value"]
    t = (math.sin(n * 0.45) + 1.0) * 0.5

    status.set_value(f"{prefix} update #{n}")
    slider.set_value(t)
    checkbox.set_checked(n % 2 == 0)
    dropdown.set_value(["x", "y", "z"][n % 3])


def start_background_updates() -> None:
    global stream_thread
    if stream_thread is not None and stream_thread.is_alive():
        return

    stream_stop.clear()

    def worker() -> None:
        while not stream_stop.wait(0.5):
            try:
                app.call_soon_threadsafe(lambda: apply_next_state("Thread"))
            except RuntimeError:
                break

    stream_thread = threading.Thread(target=worker, daemon=True)
    stream_thread.start()
    status.set_value("Background updates started")


def stop_background_updates() -> None:
    stream_stop.set()
    status.set_value("Background updates stopped")


with dg.HLayout():
    with dg.Panel("Live controls", width=300):
        status = dg.TextInput("Ready", placeholder="Status text", key="status")
        slider = dg.Slider(0.25, min=0.0, max=1.0, step=0.01, key="amount")
        dropdown = dg.Dropdown(["x", "y", "z"], value="x", key="axis")
        checkbox = dg.Checkbox("Flag enabled", checked=False, key="flag")
        dg.Button("Apply Update", on_click=apply_next_state)
        dg.Button("Start Updates", on_click=start_background_updates)
        dg.Button("Stop Updates", on_click=stop_background_updates)

    with dg.Panel("What to test"):
        dg.Label("Click Apply Python Update")
        dg.Label("Input, slider, dropdown, and checkbox update.")
        dg.Label("Background updates run every 500ms.")

try:
    print(app.run(win))
finally:
    stream_stop.set()
