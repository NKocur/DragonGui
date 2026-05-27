from __future__ import annotations

import argparse
import math
import sys
import threading
import time
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Minimal AttitudeSphere readout demo")
    parser.add_argument("--manual", action="store_true")
    parser.add_argument("--fps", type=float, default=60.0)
    parser.add_argument("--width", type=int, default=360)
    parser.add_argument("--height", type=int, default=300)
    parser.add_argument("--sphere-size", type=float, default=210.0)
    parser.add_argument("--grid-quality", choices=("fast", "high"), default="fast")
    return parser.parse_args()


args = parse_args()
stop_event = threading.Event()

app = dg.App(theme=dg.Theme.dark(accent="#4fb7ff", focus="#ffd34d"))
win = dg.Window("Attitude Readout", width=args.width, height=args.height)


def imu_values(t: float) -> tuple[float, float, float]:
    pitch = math.sin(t * 0.85) * 38.0
    roll = math.sin(t * 0.55 + 0.8) * 58.0
    yaw = (t * 42.0) % 360.0
    return pitch, roll, yaw


def update_from_sliders() -> None:
    sphere.set_orientation(
        pitch=pitch_slider.value,
        roll=roll_slider.value,
        yaw=yaw_slider.value,
    )


def stream_worker() -> None:
    period = 1.0 / max(args.fps, 1.0)
    start = time.perf_counter()
    next_frame = start

    while sphere._live() is None and not stop_event.wait(0.01):
        pass

    while not stop_event.is_set():
        now = time.perf_counter()
        if now < next_frame:
            stop_event.wait(next_frame - now)
            continue
        pitch, roll, yaw = imu_values(now - start)
        try:
            sphere.set_orientation(pitch=pitch, roll=roll, yaw=yaw)
        except RuntimeError:
            break
        next_frame += period
        if next_frame < now - period:
            next_frame = now + period


with dg.VLayout(parent=win, style={"padding": 10, "gap": 8, "align_items": "center"}):
    sphere = dg.AttitudeSphere(
        size=args.sphere_size,
        height=args.sphere_size + 24,
        show_grid=True,
        show_heading=True,
        show_readout=True,
        grid_quality=args.grid_quality,
        style={"width": args.sphere_size, "height": args.sphere_size + 24, "flex_shrink": 0},
    )

    if args.manual:
        with dg.Panel("Manual", style={"width": 320, "padding": 8, "gap": 6, "flex_shrink": 0}):
            pitch_slider = dg.Slider(0.0, min=-90.0, max=90.0, step=1.0, on_change=lambda _value: update_from_sliders())
            roll_slider = dg.Slider(0.0, min=-180.0, max=180.0, step=1.0, on_change=lambda _value: update_from_sliders())
            yaw_slider = dg.Slider(0.0, min=0.0, max=360.0, step=1.0, on_change=lambda _value: update_from_sliders())


thread: threading.Thread | None = None
if not args.manual:
    thread = threading.Thread(target=stream_worker, name="attitude-readout-stream", daemon=True)
    thread.start()

try:
    print(app.run(win))
finally:
    stop_event.set()
