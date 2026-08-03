"""Run one validated, staged live-telemetry viewer comparison sample.

The workload models a COSMOS-like operations screen: many continuously
replaced line traces plus dense value/progress/status indicator rows.  Plot
point count and update cadence stay fixed while each stage increases retained
widget count, making the result a scaling test rather than a data-size test.
"""

from __future__ import annotations

import argparse
from contextlib import nullcontext
from dataclasses import asdict, dataclass
import importlib.metadata
import json
import os
from pathlib import Path
import platform
import sys
import threading
import time
from typing import Any, Callable

from gui_benchmark_validation import ValidationRecorder, find_tree_node, layout_issue_count
from gui_live_dashboard_case import RunMetrics, _pace


ROOT = Path(__file__).resolve().parents[1]
TARGET_HZ = 30.0
TARGET_S = 1.0 / TARGET_HZ
PLOT_COLUMNS = 4


@dataclass(frozen=True)
class StageConfig:
    line_plots: int
    indicators: int
    line_points: int = 1_024


STAGES = {
    "stage1": StageConfig(4, 24),
    "stage2": StageConfig(8, 72),
    "stage3": StageConfig(12, 160),
    "stage4": StageConfig(16, 320),
}


def _version(distribution: str) -> str:
    try:
        return importlib.metadata.version(distribution)
    except importlib.metadata.PackageNotFoundError:
        return "unknown"


class TelemetryData:
    def __init__(self, config: StageConfig) -> None:
        import numpy as np

        self.np = np
        self.config = config
        self.x = np.linspace(0.0, 20.0, config.line_points, dtype=np.float32)

    def line(self, channel: int, tick: int) -> tuple[Any, Any]:
        phase = tick * 0.075 + channel * 0.37
        carrier = self.np.sin(self.x * (0.55 + channel * 0.012) + phase)
        modulation = 0.22 * self.np.cos(self.x * 0.16 - phase * 0.65)
        trend = 0.015 * channel + 0.04 * self.np.sin(phase * 0.3)
        return self.x, (carrier + modulation + trend).astype(self.np.float32)

    @staticmethod
    def indicator(index: int, tick: int) -> tuple[float, str, bool]:
        value = ((tick * 3 + index * 11) % 1_000) / 10.0
        text = f"TM-{index:03d}  {value:05.1f}"
        healthy = ((tick // 5) + index) % 7 != 0
        return value, text, healthy


def _base(framework: str, version: str, args: argparse.Namespace, config: StageConfig) -> dict[str, Any]:
    return {
        "schema": 1,
        "benchmark": "live_telemetry_viewer_scaling",
        "framework": framework,
        "framework_version": version,
        "python": platform.python_version(),
        "platform": platform.platform(),
        "stage": args.stage,
        "config": asdict(config),
        "target_hz": TARGET_HZ,
        "warmup_seconds_requested": args.warmup_seconds,
        "measure_seconds_requested": args.measure_seconds,
        "update_mode": args.update_mode if framework == "DragonGUI" else "framework-native",
    }


def _final_values(config: StageConfig, tick: int) -> tuple[tuple[float, str, bool], tuple[float, str, bool]]:
    return TelemetryData.indicator(0, tick), TelemetryData.indicator(config.indicators - 1, tick)


def _run_telemetry_loop(
    metrics: RunMetrics,
    apply: Callable[[int], None],
    render: Callable[[], None],
) -> None:
    """Drive synchronous adapters at the same 30 Hz deadlines as DragonGUI."""

    next_deadline = time.perf_counter()
    tick = 0
    while tick < metrics.total_ticks:
        if tick >= metrics.warmup_ticks and metrics.measure_wall_start is None:
            metrics.begin_measurement()
        frame_started = time.perf_counter()
        apply_started = time.perf_counter()
        apply(tick)
        apply_ms = (time.perf_counter() - apply_started) * 1000.0
        render()
        frame_ms = (time.perf_counter() - frame_started) * 1000.0
        if metrics.measuring(tick):
            metrics.submit_ms.append(apply_ms)
            metrics.frame_ms.append(frame_ms)
        metrics.completed_ticks += 1
        metrics.last_tick = tick
        metrics.applied_ticks.append(tick)
        if metrics.measuring(tick):
            metrics.measurement_completed_ticks += 1
        if tick % round(TARGET_HZ) == 0:
            metrics.sample(tick)
        next_deadline += TARGET_S
        _, lag = _pace(next_deadline)
        if metrics.measuring(tick):
            metrics.schedule_lag_ms.append(lag)
        if lag > TARGET_S * 1000.0 and tick < metrics.total_ticks - 1:
            skipped = min(
                metrics.total_ticks - tick - 2,
                max(0, int(lag / (TARGET_S * 1000.0))),
            )
            if skipped:
                first_skipped = tick + 1
                last_skipped = tick + skipped
                metrics.dropped_ticks += skipped
                metrics.measurement_dropped_ticks += sum(
                    skipped_tick >= metrics.warmup_ticks
                    for skipped_tick in range(first_skipped, last_skipped + 1)
                )
                tick += skipped
                next_deadline += skipped * TARGET_S
        tick += 1
    metrics.finish_measurement()
    metrics.sample(metrics.total_ticks)


def run_dragongui(args: argparse.Namespace, config: StageConfig) -> dict[str, Any]:
    benchmark_python_path = os.environ.get("DRAGONGUI_BENCH_PYTHON_PATH")
    sys.path.insert(0, benchmark_python_path or str(ROOT / "python"))
    import dragongui as dg

    data = TelemetryData(config)
    metrics = RunMetrics(
        round(args.warmup_seconds * TARGET_HZ),
        round(args.measure_seconds * TARGET_HZ),
    )
    app = dg.App(loading_screen=False)
    window = dg.Window(
        "DragonGUI telemetry viewer benchmark",
        width=1500,
        height=960,
        style={"overflow": "hidden"},
    )
    plots: list[Any] = []
    labels: list[Any] = []
    bars: list[Any] = []
    leds: list[Any] = []
    plot_rows = (config.line_plots + PLOT_COLUMNS - 1) // PLOT_COLUMNS
    plot_height = max(145, int(850 / max(1, plot_rows)))
    build_started = time.perf_counter()
    with dg.VLayout(style={"height": "100%", "padding": 6, "gap": 6}):
        with dg.HLayout(style={"height": 28, "gap": 12, "align_items": "center"}):
            status = dg.Label("tick 0", id="telemetry-status", style={"width": 110})
            dg.Label(
                f"{config.line_plots} traces · {config.indicators} indicators · {TARGET_HZ:.0f} Hz",
                wrap=False,
            )
        with dg.HLayout(style={"flex_grow": 1, "min_height": 0, "gap": 6}):
            with dg.ScrollArea(
                axis="y",
                style={"flex_grow": 1, "min_width": 0, "height": "100%", "gap": 6},
            ):
                for row_start in range(0, config.line_plots, PLOT_COLUMNS):
                    with dg.HLayout(style={"height": plot_height, "gap": 6}):
                        for channel in range(
                            row_start,
                            min(row_start + PLOT_COLUMNS, config.line_plots),
                        ):
                            x, y = data.line(channel, 0)
                            plots.append(
                                dg.LinePlot(
                                    {"x": x, "y": y},
                                    x="x",
                                    y="y",
                                    label=f"TM trace {channel:02d}",
                                    max_points=config.line_points,
                                    show_toolbar=False,
                                    style={"width": "25%", "height": "100%"},
                                )
                            )
            with dg.ScrollArea(
                axis="y",
                style={"width": 300, "height": "100%", "gap": 2, "padding": 3},
            ):
                for index in range(config.indicators):
                    value, text, healthy = data.indicator(index, 0)
                    with dg.HLayout(style={"height": 22, "gap": 4, "align_items": "center"}):
                        leds.append(dg.LED(healthy, size=9))
                        labels.append(dg.Label(text, wrap=False, style={"width": 118}))
                        bars.append(dg.ProgressBar(value / 100.0, style={"width": 142, "height": 8}))
    build_ms = (time.perf_counter() - build_started) * 1000.0

    producer_done = threading.Event()
    drain_recovery_ms = 0.0

    def producer() -> None:
        nonlocal drain_recovery_ms
        handle = None
        deadline = time.perf_counter() + 30.0
        while time.perf_counter() < deadline:
            handle = getattr(app, "_handle", None)
            try:
                if handle is not None and handle.latency_probe(timeout_ms=3000):
                    time.sleep(0.25)
                    break
            except (RuntimeError, TimeoutError):
                pass
            time.sleep(0.01)

        next_deadline = time.perf_counter()
        for tick in range(metrics.total_ticks):
            if tick == metrics.warmup_ticks:
                metrics.begin_measurement()

            def apply(tick: int = tick) -> None:
                started = time.perf_counter()
                for channel, plot in enumerate(plots):
                    x, y = data.line(channel, tick)
                    plot.set_data({"x": x, "y": y}, x="x", y="y", fit=False)
                    metrics.line_updates += 1
                update_context = app.update_batch() if args.update_mode == "batch" else nullcontext()
                with update_context:
                    status.set_value(f"tick {tick}")
                    for index, (label, bar, led) in enumerate(zip(labels, bars, leds)):
                        value, text, healthy = data.indicator(index, tick)
                        label.set_value(text)
                        bar.set_value(value / 100.0)
                        led.set_on(healthy)
                metrics.control_updates += config.indicators * 3
                elapsed = (time.perf_counter() - started) * 1000.0
                if metrics.measuring(tick):
                    metrics.submit_ms.append(elapsed)
                metrics.completed_ticks += 1
                metrics.last_tick = tick
                metrics.applied_ticks.append(tick)
                if metrics.measuring(tick):
                    metrics.measurement_completed_ticks += 1

            app.call_soon_threadsafe(apply, coalesce_key="telemetry-frame")
            if tick % round(TARGET_HZ) == 0:
                queue_depth = None
                if handle is not None:
                    queue_depth = handle._python_debug_snapshot().get("queued_tasks")
                metrics.sample(tick, queue_depth=queue_depth)
            next_deadline += TARGET_S
            _, lag = _pace(next_deadline)
            if metrics.measuring(tick):
                metrics.schedule_lag_ms.append(lag)

        metrics.finish_measurement()
        recovery_started = time.perf_counter()
        settle_deadline = recovery_started + 20.0
        while handle is not None and time.perf_counter() < settle_deadline:
            if not handle._python_debug_snapshot().get("queued_tasks"):
                try:
                    if handle.latency_probe(timeout_ms=3000):
                        break
                except (RuntimeError, TimeoutError):
                    pass
            time.sleep(0.01)
        drain_recovery_ms = (time.perf_counter() - recovery_started) * 1000.0
        metrics.dropped_ticks = metrics.total_ticks - metrics.completed_ticks
        metrics.measurement_dropped_ticks = metrics.measure_ticks - metrics.measurement_completed_ticks
        metrics.sample(metrics.total_ticks)
        time.sleep(0.25)
        if handle is not None:
            handle.request_exit()
        producer_done.set()

    threading.Thread(target=producer, name="telemetry-producer", daemon=True).start()
    result = app.run(window)
    final_snapshot = result.get("debug_snapshot") or {}
    gpu = final_snapshot.get("gpu") or {}
    runtime = final_snapshot.get("runtime") or {}
    renderer = gpu.get("renderer") or {}
    framework_metrics = gpu.get("framework") or {}
    line_renderer = renderer.get("line_plot_renderer") or {}
    line_resources = (gpu.get("resources") or {}).get("line_plots") or {}
    final_tick = metrics.total_ticks - 1
    first_expected, last_expected = _final_values(config, final_tick)

    validation = ValidationRecorder()
    validation.equal("producer finished", producer_done.is_set(), True, source="benchmark producer")
    validation.equal("scheduled ticks accounted for", metrics.completed_ticks + metrics.dropped_ticks, metrics.total_ticks, source="Python coalescing accounting")
    validation.equal("status reached final tick", status.text, f"tick {final_tick}", source="Python widget state")
    native_status = find_tree_node(gpu.get("tree"), status.id)
    validation.equal("native status reached final tick", (native_status or {}).get("props", {}).get("text"), f"tick {final_tick}", source="native retained tree")
    validation.equal("first indicator text reached final value", labels[0].text, first_expected[1], source="Python widget state")
    validation.equal("last indicator text reached final value", labels[-1].text, last_expected[1], source="Python widget state")
    validation.equal("first progress reached final value", round(float(bars[0].value), 4), round(first_expected[0] / 100.0, 4), source="Python widget state")
    validation.equal("last LED reached final state", leds[-1].on, last_expected[2], source="Python widget state")
    validation.equal("line renderer series count", line_renderer.get("series_count"), config.line_plots, source="native line renderer")
    validation.equal("line renderer source points", line_renderer.get("source_point_count"), config.line_plots * config.line_points, source="native line renderer")
    validation.equal("line resource count", len(line_resources), config.line_plots, source="native line resources")
    validation.require("all line resources retained points", [entry.get("last_point_count") for entry in line_resources.values()], lambda values: len(values) == config.line_plots and all(value == config.line_points for value in values), f"{config.line_plots} × {config.line_points}", source="native line resources")
    validation.equal("command queue drained", runtime.get("command_queue_depth"), 0, source="native runtime snapshot")
    validation.equal("layout remained clean", layout_issue_count(final_snapshot), 0, source="native layout diagnostics")
    validation.equal("line update accounting", metrics.line_updates, metrics.completed_ticks * config.line_plots, source="applied generation counters")

    report = _base("DragonGUI", _version("dragongui"), args, config)
    report.update({
        "build_ms": build_ms,
        "drain_recovery_ms": drain_recovery_ms,
        "metrics": metrics.report(),
        "validation": validation.report(),
        "native": {
            "runtime": {
                "wall_fps": runtime.get("wall_fps"),
                "frames_rendered": runtime.get("frames_rendered"),
                "frame_timings": runtime.get("frame_timings"),
                "command_queue": runtime.get("command_queue"),
                "command_drain": runtime.get("command_drain"),
                "command_drain_yields": runtime.get("command_drain_yields"),
                "python": runtime.get("python"),
            },
            "renderer": renderer,
            "framework": framework_metrics,
            "layout_diagnostic_count": layout_issue_count(final_snapshot),
            "line_resources": line_resources,
        },
    })
    return report


def run_dearpygui(args: argparse.Namespace, config: StageConfig) -> dict[str, Any]:
    dependency_root = os.environ.get("DRAGONGUI_BENCHMARK_DEPS_PATH")
    sys.path.insert(0, dependency_root or str(ROOT / "artifacts" / "benchmark-deps"))
    import dearpygui.dearpygui as dpg

    data = TelemetryData(config)
    metrics = RunMetrics(round(args.warmup_seconds * TARGET_HZ), round(args.measure_seconds * TARGET_HZ))
    build_started = time.perf_counter()
    dpg.create_context()
    dpg.create_viewport(title="Dear PyGui telemetry viewer benchmark", width=1500, height=960)
    plot_tags: list[str] = []
    label_tags: list[str] = []
    bar_tags: list[str] = []
    led_tags: list[str] = []
    plot_rows = (config.line_plots + PLOT_COLUMNS - 1) // PLOT_COLUMNS
    plot_height = max(145, int(850 / max(1, plot_rows)))
    with dpg.window(tag="primary"):
        dpg.add_text("tick 0", tag="telemetry-status")
        with dpg.group(horizontal=True):
            with dpg.child_window(width=1160, height=880):
                for row_start in range(0, config.line_plots, PLOT_COLUMNS):
                    with dpg.group(horizontal=True):
                        for channel in range(row_start, min(row_start + PLOT_COLUMNS, config.line_plots)):
                            with dpg.plot(width=275, height=plot_height, label=f"TM trace {channel:02d}"):
                                dpg.add_plot_axis(dpg.mvXAxis)
                                with dpg.plot_axis(dpg.mvYAxis):
                                    x, y = data.line(channel, 0)
                                    tag = f"line-{channel}"
                                    dpg.add_line_series(x.tolist(), y.tolist(), tag=tag)
                                    plot_tags.append(tag)
            with dpg.child_window(width=300, height=880):
                for index in range(config.indicators):
                    value, text, healthy = data.indicator(index, 0)
                    with dpg.group(horizontal=True):
                        led_tag = f"led-{index}"
                        label_tag = f"indicator-{index}"
                        bar_tag = f"bar-{index}"
                        dpg.add_text("●" if healthy else "○", tag=led_tag)
                        dpg.add_text(text, tag=label_tag)
                        dpg.add_progress_bar(default_value=value / 100.0, width=110, tag=bar_tag)
                        led_tags.append(led_tag)
                        label_tags.append(label_tag)
                        bar_tags.append(bar_tag)
    dpg.set_primary_window("primary", True)
    dpg.setup_dearpygui()
    dpg.set_viewport_vsync(False)
    build_ms = (time.perf_counter() - build_started) * 1000.0
    dpg.show_viewport()
    dpg.render_dearpygui_frame()

    def apply(tick: int) -> None:
        for channel, tag in enumerate(plot_tags):
            x, y = data.line(channel, tick)
            dpg.set_value(tag, [x.tolist(), y.tolist()])
            metrics.line_updates += 1
        dpg.set_value("telemetry-status", f"tick {tick}")
        for index, (label_tag, bar_tag, led_tag) in enumerate(zip(label_tags, bar_tags, led_tags)):
            value, text, healthy = data.indicator(index, tick)
            dpg.set_value(label_tag, text)
            dpg.set_value(bar_tag, value / 100.0)
            dpg.set_value(led_tag, "●" if healthy else "○")
        metrics.control_updates += config.indicators * 3

    _run_telemetry_loop(metrics, apply, dpg.render_dearpygui_frame)
    final_tick = metrics.total_ticks - 1
    first_expected, last_expected = _final_values(config, final_tick)
    validation = ValidationRecorder()
    validation.equal("scheduled ticks accounted for", metrics.completed_ticks + metrics.dropped_ticks, metrics.total_ticks, source="benchmark loop")
    validation.equal("status reached final tick", dpg.get_value("telemetry-status"), f"tick {final_tick}", source="Dear PyGui state")
    validation.equal("all line items exist", sum(dpg.does_item_exist(tag) for tag in plot_tags), config.line_plots, source="Dear PyGui registry")
    validation.equal("last line retains points", len(dpg.get_value(plot_tags[-1])[0]), config.line_points, source="Dear PyGui state")
    validation.equal("first indicator final text", dpg.get_value(label_tags[0]), first_expected[1], source="Dear PyGui state")
    validation.equal("last indicator final text", dpg.get_value(label_tags[-1]), last_expected[1], source="Dear PyGui state")
    validation.equal("first progress final value", round(float(dpg.get_value(bar_tags[0])), 4), round(first_expected[0] / 100.0, 4), source="Dear PyGui state")
    validation.equal("last LED final state", dpg.get_value(led_tags[-1]), "●" if last_expected[2] else "○", source="Dear PyGui state")
    report = _base("Dear PyGui", _version("dearpygui"), args, config)
    report.update({"build_ms": build_ms, "metrics": metrics.report(), "validation": validation.report()})
    dpg.destroy_context()
    return report


def run_pyqtgraph(args: argparse.Namespace, config: StageConfig) -> dict[str, Any]:
    os.environ.setdefault("QT_ENABLE_HIGHDPI_SCALING", "1")
    from PyQt6 import QtCore, QtWidgets
    import pyqtgraph as pg

    data = TelemetryData(config)
    metrics = RunMetrics(round(args.warmup_seconds * TARGET_HZ), round(args.measure_seconds * TARGET_HZ))
    app = QtWidgets.QApplication.instance() or QtWidgets.QApplication([])
    window = QtWidgets.QMainWindow()
    window.resize(1500, 960)
    root = QtWidgets.QWidget()
    outer = QtWidgets.QVBoxLayout(root)
    status = QtWidgets.QLabel("tick 0")
    outer.addWidget(status)
    split = QtWidgets.QSplitter()
    outer.addWidget(split, 1)
    plots_host = QtWidgets.QWidget()
    plot_grid = QtWidgets.QGridLayout(plots_host)
    indicator_scroll = QtWidgets.QScrollArea()
    indicator_scroll.setWidgetResizable(True)
    indicators_host = QtWidgets.QWidget()
    indicator_grid = QtWidgets.QGridLayout(indicators_host)
    indicator_grid.setSpacing(2)
    curves: list[Any] = []
    labels: list[Any] = []
    bars: list[Any] = []
    leds: list[Any] = []
    build_started = time.perf_counter()
    for channel in range(config.line_plots):
        plot = pg.PlotWidget(title=f"TM trace {channel:02d}")
        x, y = data.line(channel, 0)
        curves.append(plot.plot(x, y, pen=pg.mkPen(width=1.4)))
        plot_grid.addWidget(plot, channel // PLOT_COLUMNS, channel % PLOT_COLUMNS)
    for index in range(config.indicators):
        value, text, healthy = data.indicator(index, 0)
        led = QtWidgets.QLabel("●" if healthy else "○")
        label = QtWidgets.QLabel(text)
        bar = QtWidgets.QProgressBar()
        bar.setRange(0, 1_000)
        bar.setValue(round(value * 10.0))
        bar.setTextVisible(False)
        indicator_grid.addWidget(led, index, 0)
        indicator_grid.addWidget(label, index, 1)
        indicator_grid.addWidget(bar, index, 2)
        leds.append(led)
        labels.append(label)
        bars.append(bar)
    indicator_grid.setRowStretch(config.indicators, 1)
    indicator_scroll.setWidget(indicators_host)
    split.addWidget(plots_host)
    split.addWidget(indicator_scroll)
    split.setSizes([1160, 300])
    window.setCentralWidget(root)
    build_ms = (time.perf_counter() - build_started) * 1000.0
    window.show()
    app.processEvents()

    def apply(tick: int) -> None:
        for channel, curve in enumerate(curves):
            x, y = data.line(channel, tick)
            curve.setData(x, y)
            metrics.line_updates += 1
        status.setText(f"tick {tick}")
        for index, (label, bar, led) in enumerate(zip(labels, bars, leds)):
            value, text, healthy = data.indicator(index, tick)
            label.setText(text)
            bar.setValue(round(value * 10.0))
            led.setText("●" if healthy else "○")
        metrics.control_updates += config.indicators * 3

    def render() -> None:
        window.repaint()
        app.processEvents(QtCore.QEventLoop.ProcessEventsFlag.AllEvents)

    _run_telemetry_loop(metrics, apply, render)
    final_tick = metrics.total_ticks - 1
    first_expected, last_expected = _final_values(config, final_tick)
    validation = ValidationRecorder()
    validation.equal("scheduled ticks accounted for", metrics.completed_ticks + metrics.dropped_ticks, metrics.total_ticks, source="benchmark loop")
    validation.equal("status reached final tick", status.text(), f"tick {final_tick}", source="Qt widget state")
    validation.equal("all curves exist", len(curves), config.line_plots, source="PyQtGraph object state")
    validation.equal("last line retains points", len(curves[-1].xData), config.line_points, source="PyQtGraph curve state")
    validation.equal("first indicator final text", labels[0].text(), first_expected[1], source="Qt widget state")
    validation.equal("last indicator final text", labels[-1].text(), last_expected[1], source="Qt widget state")
    validation.equal("first progress final value", bars[0].value(), round(first_expected[0] * 10.0), source="Qt widget state")
    validation.equal("last LED final state", leds[-1].text(), "●" if last_expected[2] else "○", source="Qt widget state")
    report = _base("PyQtGraph", _version("pyqtgraph"), args, config)
    report.update({"build_ms": build_ms, "metrics": metrics.report(), "validation": validation.report()})
    window.close()
    app.processEvents()
    return report


RUNNERS: dict[str, Callable[[argparse.Namespace, StageConfig], dict[str, Any]]] = {
    "dragongui": run_dragongui,
    "dearpygui": run_dearpygui,
    "pyqtgraph": run_pyqtgraph,
}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--framework", required=True, choices=sorted(RUNNERS))
    parser.add_argument("--stage", required=True, choices=sorted(STAGES))
    parser.add_argument("--warmup-seconds", type=float, default=3.0)
    parser.add_argument("--measure-seconds", type=float, default=15.0)
    parser.add_argument("--update-mode", choices=("individual", "batch"), default="batch")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    args.warmup_seconds = max(0.1, args.warmup_seconds)
    args.measure_seconds = max(0.2, args.measure_seconds)
    report = RUNNERS[args.framework](args, STAGES[args.stage])
    payload = json.dumps(report, indent=2, sort_keys=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
    print(payload)
    return 0 if (report.get("validation") or {}).get("passed") is True else 2


if __name__ == "__main__":
    raise SystemExit(main())
