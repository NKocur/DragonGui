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
    parser = argparse.ArgumentParser(description="DragonGUI attitude sphere demo")
    parser.add_argument("--manual", action="store_true", help="disable generated 60 Hz IMU stream")
    parser.add_argument("--fps", type=float, default=60.0, help="generated stream rate")
    parser.add_argument("--label-fps", type=float, default=5.0, help="numeric label update rate")
    parser.add_argument("--width", type=int, default=720)
    parser.add_argument("--height", type=int, default=480)
    parser.add_argument("--sphere-size", type=float, default=300.0)
    parser.add_argument("--grid-quality", choices=("fast", "high"), default="fast")
    parser.add_argument("--no-readout", action="store_true", help="hide the internal P/R/Y widget readout")
    return parser.parse_args()


args = parse_args()
stream_enabled = not args.manual
stream_stop = threading.Event()
stream_thread: threading.Thread | None = None

app = dg.App(theme=dg.Theme.dark(accent="#4fb7ff", focus="#ffd34d"))
win = dg.Window("DragonGUI Attitude Sphere", width=args.width, height=args.height)


def set_labels(pitch: float, roll: float, yaw: float) -> None:
    pitch_value.set_value(f"Pitch {pitch:0.1f}")
    roll_value.set_value(f"Roll {roll:0.1f}")
    yaw_value.set_value(f"Yaw {yaw:0.1f}")


def update_attitude() -> None:
    pitch = pitch_slider.value
    roll = roll_slider.value
    yaw = yaw_slider.value
    sphere.set_orientation(pitch=pitch, roll=roll, yaw=yaw)
    set_labels(pitch, roll, yaw)


def reset_attitude() -> None:
    pitch_slider.set_value(0.0)
    roll_slider.set_value(0.0)
    yaw_slider.set_value(0.0)
    update_attitude()


def imu_values(t: float) -> tuple[float, float, float]:
    pitch = math.sin(t * 0.85) * 38.0
    roll = math.sin(t * 0.55 + 0.8) * 58.0
    yaw = (t * 42.0) % 360.0
    return pitch, roll, yaw


def stream_worker() -> None:
    period = 1.0 / max(args.fps, 1.0)
    label_period = 1.0 / max(args.label_fps, 0.25)
    start = time.perf_counter()
    next_frame = start
    next_label = start

    while sphere._live() is None and not stream_stop.wait(0.01):
        pass

    while not stream_stop.is_set():
        now = time.perf_counter()
        if now < next_frame:
            stream_stop.wait(next_frame - now)
            continue

        pitch, roll, yaw = imu_values(now - start)
        try:
            sphere.set_orientation(pitch=pitch, roll=roll, yaw=yaw)
            if now >= next_label:
                set_labels(pitch, roll, yaw)
                next_label = now + label_period
        except RuntimeError:
            break

        next_frame += period
        if next_frame < now - period:
            next_frame = now + period


def start_stream() -> None:
    global stream_enabled, stream_thread
    if stream_enabled:
        return
    stream_enabled = True
    stream_stop.clear()
    stream_thread = threading.Thread(target=stream_worker, name="attitude-sphere-stream", daemon=True)
    stream_thread.start()
    stream_button.set_value("Pause Stream")
    mode_value.set_value(f"Streaming {args.fps:0.0f} Hz")


def stop_stream() -> None:
    global stream_enabled
    stream_enabled = False
    stream_stop.set()
    stream_button.set_value("Start Stream")
    mode_value.set_value("Manual")


def toggle_stream() -> None:
    if stream_enabled:
        stop_stream()
    else:
        start_stream()


with dg.HLayout(parent=win, style={"padding": 12, "gap": 12}):
    with dg.Panel("IMU", width=230, style={"padding": 10, "gap": 8}):
        mode_value = dg.Label(f"Streaming {args.fps:0.0f} Hz" if stream_enabled else "Manual")
        pitch_value = dg.Label("Pitch 0.0")
        pitch_slider = dg.Slider(0.0, min=-90.0, max=90.0, step=1.0, on_change=lambda _value: update_attitude())
        roll_value = dg.Label("Roll 0.0")
        roll_slider = dg.Slider(0.0, min=-180.0, max=180.0, step=1.0, on_change=lambda _value: update_attitude())
        yaw_value = dg.Label("Yaw 0.0")
        yaw_slider = dg.Slider(0.0, min=0.0, max=360.0, step=1.0, on_change=lambda _value: update_attitude())
        stream_button = dg.Button("Pause Stream" if stream_enabled else "Start Stream", on_click=toggle_stream)
        dg.Button("Reset", on_click=reset_attitude)

    with dg.Panel("Attitude", style={"padding": 10}):
        sphere = dg.AttitudeSphere(
            size=args.sphere_size,
            show_grid=True,
            show_readout=not args.no_readout,
            grid_quality=args.grid_quality,
        )

if stream_enabled:
    stream_thread = threading.Thread(target=stream_worker, name="attitude-sphere-stream", daemon=True)
    stream_thread.start()

try:
    print(app.run(win))
finally:
    stream_stop.set()
