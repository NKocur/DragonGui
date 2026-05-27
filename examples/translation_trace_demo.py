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
    parser = argparse.ArgumentParser(description="DragonGUI translation trace demo")
    parser.add_argument("--manual", action="store_true", help="disable generated movement stream")
    parser.add_argument("--fps", type=float, default=60.0, help="generated stream rate")
    parser.add_argument("--label-fps", type=float, default=8.0, help="numeric label update rate")
    parser.add_argument("--width", type=int, default=620)
    parser.add_argument("--height", type=int, default=480)
    parser.add_argument("--trace-size", type=float, default=330.0)
    parser.add_argument("--range-m", type=float, default=12.0)
    parser.add_argument("--ring-step-m", type=float, default=2.0)
    parser.add_argument("--trail-points", type=int, default=90)
    return parser.parse_args()


args = parse_args()
stream_enabled = not args.manual
stream_stop = threading.Event()
stream_thread: threading.Thread | None = None

app = dg.App(theme=dg.Theme.dark(accent="#42d7ff", focus="#ffd04a"))
app.stylesheet(
    """
Window {
    background: #0b111b;
    color: rgba(244, 248, 255, 0.94);
    font-family: "Piboto";
}

Panel,
Label,
Button {
    font-family: "Piboto";
    font-weight: 400;
}

TranslationTrace {
    background: rgba(3, 8, 16, 0.78);
    border-radius: 0px;
}
"""
)
win = dg.Window("DragonGUI Translation Trace", width=args.width, height=args.height)


def set_labels(x: float, y: float) -> None:
    x_value.set_value(f"X {x:0.2f} m")
    y_value.set_value(f"Y {y:0.2f} m")
    distance_value.set_value(f"R {math.hypot(x, y):0.2f} m")


def update_manual() -> None:
    x = x_slider.value
    y = y_slider.value
    trace.set_position(x, y)
    set_labels(x, y)


def reset_trace() -> None:
    x_slider.set_value(0.0)
    y_slider.set_value(0.0)
    trace.set_position(0.0, 0.0)
    trace.clear_trail()
    set_labels(0.0, 0.0)


def generated_position(t: float) -> tuple[float, float]:
    x = math.sin(t * 0.42) * 5.8 + math.sin(t * 1.15) * 1.4
    y = math.cos(t * 0.36 + 0.6) * 4.6 + math.sin(t * 0.85) * 1.1
    return x, y


def stream_worker() -> None:
    period = 1.0 / max(args.fps, 1.0)
    label_period = 1.0 / max(args.label_fps, 0.25)
    start = time.perf_counter()
    next_frame = start
    next_label = start

    while trace._live() is None and not stream_stop.wait(0.01):
        pass

    while not stream_stop.is_set():
        now = time.perf_counter()
        if now < next_frame:
            stream_stop.wait(next_frame - now)
            continue

        x, y = generated_position(now - start)
        try:
            trace.set_position(x, y)
            if now >= next_label:
                set_labels(x, y)
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
    stream_thread = threading.Thread(target=stream_worker, name="translation-trace-stream", daemon=True)
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


with dg.HLayout(parent=win, style={"padding": 12, "gap": 12, "height": "100%"}):
    with dg.Panel("Translation", width=180, style={"padding": 8, "gap": 7}):
        mode_value = dg.Label(f"Streaming {args.fps:0.0f} Hz" if stream_enabled else "Manual")
        x_value = dg.Label("X 0.00 m")
        x_slider = dg.Slider(0.0, min=-args.range_m, max=args.range_m, step=0.1, on_change=lambda _value: update_manual())
        y_value = dg.Label("Y 0.00 m")
        y_slider = dg.Slider(0.0, min=-args.range_m, max=args.range_m, step=0.1, on_change=lambda _value: update_manual())
        distance_value = dg.Label("R 0.00 m")
        stream_button = dg.Button("Pause Stream" if stream_enabled else "Start Stream", on_click=toggle_stream)
        dg.Button("Clear Trail", on_click=reset_trace)

    with dg.Panel("Position", style={"padding": 10, "min_height": 0, "flex_grow": 1}):
        trace = dg.TranslationTrace(
            size=args.trace_size,
            range_m=args.range_m,
            ring_step_m=args.ring_step_m,
            trail_points=args.trail_points,
        )

if stream_enabled:
    stream_thread = threading.Thread(target=stream_worker, name="translation-trace-stream", daemon=True)
    stream_thread.start()

try:
    print(app.run(win))
finally:
    stream_stop.set()
