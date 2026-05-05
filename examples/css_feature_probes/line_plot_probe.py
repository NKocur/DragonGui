from __future__ import annotations

import math
import sys
import threading
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual visual probe requirement
    raise SystemExit("line_plot_probe.py requires NumPy") from exc


class SignalFrame:
    columns = ("time", "temperature", "pressure", "vibration", "events")
    dtypes = ("float32", "float32", "float32", "float32", "float32")

    def __init__(self, rows: int = 720) -> None:
        self.shape = (rows, len(self.columns))
        t = np.linspace(0.0, 60.0, rows, dtype=np.float32)
        phase = t / np.float32(60.0)
        ripple = np.sin(t * np.float32(1.9)) * np.float32(0.18)
        self.time = t
        self.temperature = (
            np.float32(68.0)
            + np.sin(phase * np.float32(math.tau * 2.0)) * np.float32(4.6)
            + ripple
        ).astype(np.float32)
        self.pressure = (
            np.float32(31.0)
            + np.cos(phase * np.float32(math.tau * 3.0)) * np.float32(2.2)
            + np.sin(t * np.float32(0.72)) * np.float32(0.45)
        ).astype(np.float32)
        self.vibration = (
            np.sin(t * np.float32(3.8)) * np.float32(0.7)
            + np.sin(t * np.float32(12.2)) * np.float32(0.18)
        ).astype(np.float32)
        self.events = np.zeros(rows, dtype=np.float32)
        for idx, height in ((112, 1.0), (276, 0.74), (431, 1.22), (606, 0.66)):
            width = np.arange(rows, dtype=np.float32) - np.float32(idx)
            self.events += np.exp(-(width * width) / np.float32(28.0)) * np.float32(height)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


SERIES = {
    "temperature": ("Temperature", "#5aa9ff"),
    "pressure": ("Pressure", "#74ddb0"),
    "vibration": ("Vibration", "#ffb45c"),
    "events": ("Events", "#f36b7f"),
}

frame = SignalFrame()
probe_state = {"active": "temperature", "stream_t": 60.0, "batch": 0}
stream_stop = threading.Event()
stream_thread: threading.Thread | None = None
main_plot: dg.LinePlot | None = None
status_label: dg.Label | None = None
width_label: dg.Label | None = None


def _active_label() -> str:
    return SERIES[str(probe_state["active"])][0]


def _active_color() -> str:
    return SERIES[str(probe_state["active"])][1]


def _set_status(message: str) -> None:
    if status_label is not None:
        status_label.set_value(message)


def _show_series(column: str) -> None:
    probe_state["active"] = column
    probe_state["stream_t"] = 60.0
    label, color = SERIES[column]
    if main_plot is not None:
        main_plot.set_data(frame, x="time", y=column, label=label, color=color)
    _set_status(f"showing {label.lower()}")


def _set_width(value: float) -> None:
    width = max(0.5, float(value))
    if width_label is not None:
        width_label.set_value(f"Line width: {width:.1f}px")
    if main_plot is not None:
        main_plot.set_line_width(width)


def _set_grid(visible: bool) -> None:
    if main_plot is not None:
        main_plot.set_grid_visible(visible)
    _set_status("grid on" if visible else "grid off")


def _set_axes(visible: bool) -> None:
    if main_plot is not None:
        main_plot.set_axes_visible(visible)
    _set_status("axes on" if visible else "axes off")


def _set_ticks(visible: bool) -> None:
    if main_plot is not None:
        main_plot.set_ticks_visible(visible)
    _set_status("ticks on" if visible else "ticks off")


def _set_tick_count(value: float) -> None:
    count = int(round(value))
    if main_plot is not None:
        main_plot.set_tick_count(count)
    _set_status(f"{count} ticks")


def _use_sensor_labels() -> None:
    if main_plot is not None:
        main_plot.set_axis_labels(x="Elapsed time (s)", y=f"{_active_label()} reading")
    _set_status("sensor axis labels")


def _use_compact_labels() -> None:
    if main_plot is not None:
        main_plot.set_axis_labels(x="t", y=_active_label())
    _set_status("compact axis labels")


def _generated_batch(samples: int = 32) -> tuple[np.ndarray, np.ndarray]:
    start = float(probe_state["stream_t"])
    end = start + 1.4
    t = np.linspace(start, end, samples, dtype=np.float32)
    probe_state["stream_t"] = end
    phase = t / np.float32(60.0)
    active = str(probe_state["active"])
    if active == "temperature":
        y = np.float32(68.0) + np.sin(phase * np.float32(math.tau * 2.0)) * np.float32(4.6)
        y += np.sin(t * np.float32(2.7)) * np.float32(0.35)
    elif active == "pressure":
        y = np.float32(31.0) + np.cos(phase * np.float32(math.tau * 3.0)) * np.float32(2.2)
        y += np.sin(t * np.float32(1.1)) * np.float32(0.55)
    elif active == "vibration":
        y = np.sin(t * np.float32(3.8)) * np.float32(0.7)
        y += np.sin(t * np.float32(12.2)) * np.float32(0.22)
    else:
        center = np.float32(start + 0.72)
        y = np.exp(-((t - center) ** 2) / np.float32(0.08)) * np.float32(1.15)
    return t.astype(np.float32), y.astype(np.float32)


def _append_batch() -> None:
    if main_plot is None:
        return
    x, y = _generated_batch()
    main_plot.append_points(x, y, series=_active_label(), max_points=720)
    probe_state["batch"] = int(probe_state["batch"]) + 1
    _set_status(f"appended batch {probe_state['batch']} to {_active_label().lower()}")


def _clear_plot() -> None:
    if main_plot is not None:
        main_plot.clear()
    _set_status("cleared visible series")


def _reset_plot() -> None:
    _show_series(str(probe_state["active"]))


def _start_stream() -> None:
    global stream_thread
    if stream_thread is not None and stream_thread.is_alive():
        _set_status("stream already running")
        return
    stream_stop.clear()

    def _run() -> None:
        while not stream_stop.wait(0.09):
            try:
                app.call_soon_threadsafe(_append_batch)
            except RuntimeError:
                break

    stream_thread = threading.Thread(target=_run, name="line-plot-stream", daemon=True)
    stream_thread.start()
    _set_status("stream running")


def _stop_stream() -> None:
    stream_stop.set()
    _set_status("stream stopped")


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #0c111d;
        color: rgba(246, 249, 255, 0.94);
        padding: 16px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
    }

    HLayout.grid {
        width: 100%;
        flex-grow: 1;
        flex-shrink: 1;
        min-width: 0;
        min-height: 0;
        gap: 12px;
    }

    VLayout.stack {
        width: 286px;
        flex-grow: 0;
        flex-shrink: 0;
        min-width: 0;
        min-height: 0;
        gap: 12px;
    }

    Panel {
        background: rgba(18, 25, 40, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 10px;
        padding: 12px;
        gap: 8px;
        min-width: 0;
        min-height: 0;
    }

    Panel.controls {
        width: 230px;
        flex-shrink: 0;
        padding: 10px;
        gap: 7px;
    }

    ScrollArea.controls-scroll {
        flex-grow: 1;
        min-height: 0;
        gap: 7px;
    }

    Panel.main {
        flex-grow: 1;
        flex-shrink: 1;
        min-width: 0;
        min-height: 0;
    }

    Panel.side {
        width: 286px;
        flex-grow: 1;
        flex-shrink: 1;
        min-height: 0;
        padding: 10px;
        gap: 6px;
    }

    Panel.side Label.pass {
        font-size: 11px;
        line-height: 1.04;
        height: 28px;
        flex-shrink: 0;
    }

    Button {
        width: 100%;
    }

    Button.danger {
        border-color: rgba(243, 107, 127, 0.68);
        color: #f36b7f;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(246, 249, 255, 0.68);
        font-size: 12px;
    }

    Label.section {
        color: rgba(246, 249, 255, 0.58);
        font-size: 11px;
        font-weight: 760;
    }

    Label.status {
        background: rgba(90, 169, 255, 0.11);
        border: 1px solid rgba(90, 169, 255, 0.26);
        border-radius: 8px;
        color: rgba(232, 244, 255, 0.95);
        font-size: 12px;
        padding: 7px 8px;
        height: 34px;
    }

    Label.case-title {
        color: rgba(246, 249, 255, 0.92);
        font-size: 13px;
        font-weight: 780;
    }

    Label.pass {
        color: #74ddb0;
        font-size: 12px;
    }

    LinePlot {
        width: 100%;
        min-height: 190px;
        background: rgba(3, 8, 18, 0.58);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 8px;
        padding: 10px;
    }

    LinePlot.primary {
        flex-grow: 1;
        min-height: 340px;
    }

    LinePlot.compact {
        min-height: 92px;
        padding: 8px;
    }
    """
)

win = dg.Window(
    "LinePlot Probe",
    width=1120,
    height=720,
    style={"display": "flex", "flex_direction": "column", "min_width": 0, "min_height": 0},
)

with dg.VLayout(parent=win, class_="root"):
    dg.Label("LinePlot", class_="title")
    dg.Label(
        "Controls exercise packed live replacement, append, clear, line width, and grid updates.",
        class_="caption",
    )

    with dg.HLayout(class_="grid"):
        with dg.Panel("Controls", class_="controls"):
            with dg.ScrollArea(axis="y", class_="controls-scroll"):
                status_label = dg.Label("ready", class_="status")
                dg.Label("SERIES", class_="section")
                dg.Button("Temperature", on_click=lambda: _show_series("temperature"))
                dg.Button("Pressure", on_click=lambda: _show_series("pressure"))
                dg.Button("Vibration", on_click=lambda: _show_series("vibration"))
                dg.Button("Events", on_click=lambda: _show_series("events"))

                dg.Label("LIVE DATA", class_="section")
                dg.Button("Append Batch", on_click=_append_batch)
                dg.Button("Start Stream", on_click=_start_stream)
                dg.Button("Stop Stream", on_click=_stop_stream)
                dg.Button("Clear", class_="danger", on_click=_clear_plot)
                dg.Button("Reset", on_click=_reset_plot)

                dg.Label("STYLE", class_="section")
                width_label = dg.Label("Line width: 2.2px")
                dg.Slider(2.2, min=0.5, max=6.0, step=0.1, on_change=_set_width)
                dg.Checkbox("Grid", checked=True, on_change=_set_grid)
                dg.Checkbox("Axes", checked=True, on_change=_set_axes)
                dg.Checkbox("Ticks", checked=True, on_change=_set_ticks)
                dg.Label("Tick count")
                dg.Slider(5, min=2, max=9, step=1, on_change=_set_tick_count)
                dg.Button("Sensor Labels", on_click=_use_sensor_labels)
                dg.Button("Compact Labels", on_click=_use_compact_labels)

        with dg.Panel("Live plot", class_="main"):
            dg.Label("Primary trace", class_="case-title")
            main_plot = dg.LinePlot(
                frame,
                x="time",
                y="temperature",
                label="Temperature",
                color="#5aa9ff",
                x_label="Elapsed time (s)",
                y_label="Temperature reading",
                line_width=2.2,
                tick_count=5,
                max_points=720,
                class_="primary",
            )
            dg.Label("PASS: controls update this plot without rebuilding the widget tree.", class_="pass")

        with dg.VLayout(class_="stack"):
            with dg.Panel("Multi-series static", class_="side"):
                dg.LinePlot(
                    frame,
                    x="time",
                    y=("temperature", "pressure"),
                    labels=("Temp", "Pressure"),
                    colors=("#5aa9ff", "#74ddb0"),
                    line_width=1.6,
                    show_toolbar=False,
                    tick_count=4,
                    class_="compact",
                )
                dg.Label("PASS: static multi-series renders both traces.", class_="pass")

            with dg.Panel("Sample-index plot", class_="side"):
                dg.LinePlot(
                    frame,
                    y="vibration",
                    label="Vibration",
                    color="#ffb45c",
                    line_width=1.6,
                    show_toolbar=False,
                    tick_count=4,
                    class_="compact",
                )
                dg.Label("PASS: y-only data uses sample index for x.", class_="pass")

            with dg.Panel("Event spikes", class_="side"):
                dg.LinePlot(
                    frame,
                    x="time",
                    y="events",
                    label="Events",
                    color="#f36b7f",
                    line_width=2.4,
                    show_toolbar=False,
                    tick_count=4,
                    class_="compact",
                )
                dg.Label("PASS: narrow peaks remain visible.", class_="pass")


if __name__ == "__main__":
    try:
        print(app.run(win))
    finally:
        stream_stop.set()
