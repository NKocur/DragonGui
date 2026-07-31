"""Run one realistic, sustained live-visualization dashboard benchmark.

Unlike the synthetic control-row suites, this benchmark maintains several
plots at different cadences for a warmup and measurement interval.  Each
framework receives the same deterministic NumPy data.  Correctness checks are
part of the result; a DragonGUI failure exits non-zero so invalid timings cannot
enter a matrix report.
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

from gui_benchmark_validation import (
    ValidationRecorder,
    find_tree_node,
    layout_issue_count,
)
from gui_framework_case import _percentile, _rss_bytes, _timings


ROOT = Path(__file__).resolve().parents[1]
TARGET_HZ = 60.0
TARGET_S = 1.0 / TARGET_HZ


@dataclass(frozen=True)
class LoadConfig:
    line_plots: int
    line_points: int
    line_every: int
    scatter_points: int
    scatter_every: int
    heat_size: int
    heat_every: int
    controls: int


LOADS = {
    "low": LoadConfig(2, 2_000, 4, 2_000, 12, 32, 30, 20),
    "medium": LoadConfig(4, 10_000, 2, 10_000, 6, 64, 15, 80),
    "high": LoadConfig(6, 50_000, 1, 50_000, 3, 128, 6, 200),
}


def _version(distribution: str) -> str:
    try:
        return importlib.metadata.version(distribution)
    except importlib.metadata.PackageNotFoundError:
        return "unknown"


def _pace(deadline: float) -> tuple[float, float]:
    now = time.perf_counter()
    if now < deadline:
        time.sleep(deadline - now)
    finished = time.perf_counter()
    return finished, max(0.0, (finished - deadline) * 1000.0)


class DashboardData:
    def __init__(self, config: LoadConfig) -> None:
        import numpy as np

        self.np = np
        self.config = config
        self.line_x = np.linspace(0.0, 40.0, config.line_points, dtype=np.float32)
        self.scatter_t = np.linspace(0.0, np.pi * 16.0, config.scatter_points, dtype=np.float32)
        axis = np.linspace(-3.0, 3.0, config.heat_size, dtype=np.float32)
        self.heat_x, self.heat_y = np.meshgrid(axis, axis)

    def line(self, channel: int, tick: int) -> tuple[Any, Any]:
        phase = tick * 0.035 + channel * 0.47
        y = (
            self.np.sin(self.line_x * (0.42 + channel * 0.018) + phase)
            + self.np.cos(self.line_x * 0.11 - phase * 0.7) * 0.22
        ).astype(self.np.float32)
        return self.line_x, y

    def scatter(self, tick: int) -> tuple[Any, Any]:
        phase = tick * 0.025
        radius = 1.0 + 0.18 * self.np.sin(self.scatter_t * 0.37 + phase)
        x = (self.np.cos(self.scatter_t + phase) * radius).astype(self.np.float32)
        y = (self.np.sin(self.scatter_t * 1.07 - phase) * radius).astype(self.np.float32)
        return x, y

    def heat(self, tick: int) -> Any:
        phase = tick * 0.04
        return (
            self.np.sin(self.heat_x * 1.4 + phase)
            * self.np.cos(self.heat_y * 1.1 - phase)
        ).astype(self.np.float32)


class RunMetrics:
    def __init__(self, warmup_ticks: int, measure_ticks: int) -> None:
        self.warmup_ticks = warmup_ticks
        self.measure_ticks = measure_ticks
        self.total_ticks = warmup_ticks + measure_ticks
        self.completed_ticks = 0
        self.measurement_completed_ticks = 0
        self.dropped_ticks = 0
        self.measurement_dropped_ticks = 0
        self.last_tick = -1
        self.applied_ticks: list[int] = []
        self.measurement_completed_at_window_end = 0
        self.line_updates = 0
        self.scatter_updates = 0
        self.heat_updates = 0
        self.control_updates = 0
        self.submit_ms: list[float] = []
        self.frame_ms: list[float] = []
        self.schedule_lag_ms: list[float] = []
        self.rss_samples: list[int] = []
        self.checkpoints: list[dict[str, Any]] = []
        self.measure_wall_start: float | None = None
        self.measure_wall_end: float | None = None
        self.measure_cpu_start: float | None = None
        self.measure_cpu_end: float | None = None

    def measuring(self, tick: int) -> bool:
        return tick >= self.warmup_ticks

    def begin_measurement(self) -> None:
        if self.measure_wall_start is None:
            self.measure_wall_start = time.perf_counter()
            self.measure_cpu_start = time.process_time()

    def finish_measurement(self) -> None:
        self.measure_wall_end = time.perf_counter()
        self.measure_cpu_end = time.process_time()
        self.measurement_completed_at_window_end = self.measurement_completed_ticks

    def sample(self, tick: int, *, queue_depth: int | None = None) -> None:
        rss = _rss_bytes()
        if rss is not None:
            self.rss_samples.append(rss)
        self.checkpoints.append({
            "tick": tick,
            "rss_bytes": rss,
            "queue_depth": queue_depth,
            "completed_ticks": self.completed_ticks,
        })

    def report(self) -> dict[str, Any]:
        wall = max(1e-9, (self.measure_wall_end or 0.0) - (self.measure_wall_start or 0.0))
        cpu = max(0.0, (self.measure_cpu_end or 0.0) - (self.measure_cpu_start or 0.0))
        start_rss = self.rss_samples[0] if self.rss_samples else 0
        end_rss = self.rss_samples[-1] if self.rss_samples else 0
        return {
            "warmup_ticks": self.warmup_ticks,
            "measure_ticks": self.measure_ticks,
            "completed_ticks": self.completed_ticks,
            "measurement_completed_ticks": self.measurement_completed_ticks,
            "measurement_completed_at_window_end": self.measurement_completed_at_window_end,
            "dropped_ticks": self.dropped_ticks,
            "measurement_dropped_ticks": self.measurement_dropped_ticks,
            "last_tick": self.last_tick,
            "measurement_wall_s": wall,
            "tick_throughput_hz": self.measurement_completed_at_window_end / wall,
            "process_cpu_percent_one_core": cpu / wall * 100.0,
            "submit_ms": _timings(self.submit_ms),
            "frame_ms": _timings(self.frame_ms),
            "schedule_lag_ms": _timings(self.schedule_lag_ms),
            "submit_deadline_misses": sum(value > TARGET_S * 1000.0 for value in self.submit_ms),
            "frame_deadline_misses": sum(value > TARGET_S * 1000.0 for value in self.frame_ms),
            "schedule_late_2ms": sum(value > 2.0 for value in self.schedule_lag_ms),
            "line_updates": self.line_updates,
            "scatter_updates": self.scatter_updates,
            "heat_updates": self.heat_updates,
            "control_updates": self.control_updates,
            "rss_start_bytes": start_rss,
            "rss_end_bytes": end_rss,
            "rss_peak_bytes": max(self.rss_samples, default=0),
            "rss_growth_bytes": end_rss - start_rss,
            "rss_growth_bytes_per_minute": (end_rss - start_rss) / wall * 60.0,
            "checkpoints": self.checkpoints,
        }


def _base(framework: str, version: str, args: argparse.Namespace, config: LoadConfig) -> dict[str, Any]:
    return {
        "schema": 1,
        "benchmark": "live_visualization_dashboard",
        "framework": framework,
        "framework_version": version,
        "python": platform.python_version(),
        "platform": platform.platform(),
        "load": args.load,
        "config": asdict(config),
        "warmup_seconds_requested": args.warmup_seconds,
        "measure_seconds_requested": args.measure_seconds,
        "target_hz": TARGET_HZ,
        "update_mode": args.update_mode if framework == "DragonGUI" else "framework-native",
    }


def _expected_updates(total: int, every: int) -> int:
    return ((total - 1) // every) + 1 if total else 0


def run_dragongui(args: argparse.Namespace, config: LoadConfig) -> dict[str, Any]:
    benchmark_python_path = os.environ.get("DRAGONGUI_BENCH_PYTHON_PATH")
    sys.path.insert(0, benchmark_python_path or str(ROOT / "python"))
    import dragongui as dg

    data = DashboardData(config)
    warmup_ticks = round(args.warmup_seconds * TARGET_HZ)
    measure_ticks = round(args.measure_seconds * TARGET_HZ)
    metrics = RunMetrics(warmup_ticks, measure_ticks)
    app = dg.App(loading_screen=False)
    window = dg.Window("DragonGUI live dashboard benchmark", width=1400, height=900)
    line_plots: list[Any] = []
    controls: list[Any] = []
    build_started = time.perf_counter()
    with dg.VLayout(style={"height": "100%", "gap": 6, "padding": 6}):
        status = dg.Label("tick 0", id="dashboard-status", style={"height": 24})
        with dg.HLayout(style={"height": 650, "gap": 6}):
            with dg.VLayout(style={"width": "calc(100% - 250px)", "gap": 6}):
                for row_start in range(0, config.line_plots, 3):
                    with dg.HLayout(style={"height": 210, "gap": 6}):
                        for channel in range(row_start, min(row_start + 3, config.line_plots)):
                            x, y = data.line(channel, 0)
                            line_plots.append(dg.LinePlot(
                                {"x": x, "y": y}, x="x", y="y", label=f"channel-{channel}",
                                max_points=config.line_points, show_toolbar=False,
                                style={"width": "33%", "height": "100%"},
                            ))
                with dg.HLayout(style={"height": 210, "gap": 6}):
                    sx, sy = data.scatter(0)
                    scatter = dg.ScatterPlot2D(
                        {"x": sx, "y": sy}, x="x", y="y", point_size=2.0,
                        auto_point_size=False, style={"width": "50%", "height": "100%"},
                    )
                    heat = dg.Heatmap(
                        data.heat(0), show_labels=False, scalar_bar=False,
                        style={"width": "50%", "height": "100%"},
                    )
            with dg.ScrollArea(axis="y", style={"width": 244, "height": "100%", "gap": 3}):
                for index in range(config.controls):
                    with dg.HLayout(style={"height": 24, "gap": 4}):
                        controls.append(dg.Label(f"sensor {index:03d}: 0", style={"width": 130}))
                        dg.ProgressBar(0.0, style={"width": 90})
    build_ms = (time.perf_counter() - build_started) * 1000.0

    ready_snapshot: dict[str, Any] = {}
    producer_done = threading.Event()
    drain_recovery_ms = 0.0

    def producer() -> None:
        nonlocal ready_snapshot, drain_recovery_ms
        deadline = time.perf_counter() + 30.0
        while time.perf_counter() < deadline:
            try:
                ready_snapshot = app.debug_snapshot(timeout_ms=3000)
            except (RuntimeError, TimeoutError):
                ready_snapshot = {}
            if (ready_snapshot.get("runtime") or {}).get("startup_readiness") == "application_frame_presented":
                break
            time.sleep(0.01)
        next_deadline = time.perf_counter()
        for tick in range(metrics.total_ticks):
            if tick == metrics.warmup_ticks:
                metrics.begin_measurement()

            def apply(tick: int = tick) -> None:
                started = time.perf_counter()
                if tick % config.line_every == 0:
                    for channel, plot in enumerate(line_plots):
                        x, y = data.line(channel, tick)
                        plot.set_data({"x": x, "y": y}, x="x", y="y", fit=False)
                        metrics.line_updates += 1
                if tick % config.scatter_every == 0:
                    sx, sy = data.scatter(tick)
                    scatter.set_points({"x": sx, "y": sy}, x="x", y="y", fit=False)
                    metrics.scatter_updates += 1
                if tick % config.heat_every == 0:
                    heat.set_data(data.heat(tick))
                    metrics.heat_updates += 1
                update_context = (
                    app.update_batch() if args.update_mode == "batch" else nullcontext()
                )
                with update_context:
                    status.set_value(f"tick {tick}")
                    for index, label in enumerate(controls):
                        label.set_value(f"sensor {index:03d}: {(tick + index) % 100}")
                metrics.control_updates += len(controls)
                elapsed = (time.perf_counter() - started) * 1000.0
                if metrics.measuring(tick):
                    metrics.submit_ms.append(elapsed)
                metrics.completed_ticks += 1
                metrics.last_tick = tick
                metrics.applied_ticks.append(tick)
                if metrics.measuring(tick):
                    metrics.measurement_completed_ticks += 1

            app.call_soon_threadsafe(apply, coalesce_key="live-dashboard-frame")
            if tick % 60 == 0:
                handle = getattr(app, "_handle", None)
                queue = None
                if handle is not None:
                    queue = handle._python_debug_snapshot().get("queued_tasks")
                metrics.sample(tick, queue_depth=queue)
            next_deadline += TARGET_S
            _, lag = _pace(next_deadline)
            if metrics.measuring(tick):
                metrics.schedule_lag_ms.append(lag)
        metrics.finish_measurement()
        recovery_started = time.perf_counter()
        settle_deadline = recovery_started + 15.0
        handle = getattr(app, "_handle", None)
        while handle is not None and time.perf_counter() < settle_deadline:
            if not handle._python_debug_snapshot().get("queued_tasks"):
                break
            time.sleep(0.01)
        while handle is not None and time.perf_counter() < settle_deadline:
            try:
                native_state = app.debug_snapshot(timeout_ms=3000).get("runtime") or {}
            except (RuntimeError, TimeoutError):
                native_state = {"command_queue_depth": 1}
            if not native_state.get("command_queue_depth"):
                break
            time.sleep(0.01)
        drain_recovery_ms = (time.perf_counter() - recovery_started) * 1000.0
        metrics.dropped_ticks = metrics.total_ticks - metrics.completed_ticks
        metrics.measurement_dropped_ticks = metrics.measure_ticks - metrics.measurement_completed_ticks
        metrics.sample(metrics.total_ticks)
        time.sleep(0.5)
        if handle is not None:
            handle.request_exit()
        producer_done.set()

    threading.Thread(target=producer, name="live-dashboard-producer", daemon=True).start()
    run_started = time.perf_counter()
    result = app.run(window)
    run_wall_ms = (time.perf_counter() - run_started) * 1000.0
    final_snapshot = result.get("debug_snapshot") or {}
    gpu = final_snapshot.get("gpu") or {}
    runtime = final_snapshot.get("runtime") or {}
    resources = gpu.get("resources") or {}
    renderer = gpu.get("renderer") or {}
    line_renderer = renderer.get("line_plot_renderer") or {}
    scatter_resources = resources.get("scatters") or {}
    line_resources = resources.get("line_plots") or {}

    validation = ValidationRecorder()
    validation.equal("producer finished", producer_done.is_set(), True, source="benchmark producer")
    validation.equal("scheduled ticks accounted for", metrics.completed_ticks + metrics.dropped_ticks, metrics.total_ticks, source="Python coalescing accounting")
    validation.equal("command queue drained", runtime.get("command_queue_depth"), 0, source="native runtime snapshot")
    validation.equal("layout remained clean", layout_issue_count(final_snapshot), 0, source="native layout diagnostics")
    validation.equal("status reached final tick", status.text, f"tick {metrics.total_ticks - 1}", source="Python widget state")
    native_status = find_tree_node(gpu.get("tree"), status.id)
    validation.equal("native status reached final tick", (native_status or {}).get("props", {}).get("text"), f"tick {metrics.total_ticks - 1}", source="native retained tree")
    validation.equal("line renderer series count", line_renderer.get("series_count"), config.line_plots, source="native line renderer")
    validation.equal("line renderer source points", line_renderer.get("source_point_count"), config.line_plots * config.line_points, source="native line renderer")
    validation.equal("line resource count", len(line_resources), config.line_plots, source="native line resources")
    validation.require("every line resource has expected points", [entry.get("last_point_count") for entry in line_resources.values()], lambda values: len(values) == config.line_plots and all(value == config.line_points for value in values), f"{config.line_plots} × {config.line_points}", source="native line resources")
    validation.equal("scatter resource count", len(scatter_resources), 1, source="native scatter resources")
    scatter_metrics = next(iter(scatter_resources.values()), {})
    validation.equal("scatter resource point count", scatter_metrics.get("last_point_count"), config.scatter_points, source="native scatter resources")
    validation.equal("heatmap Python dimensions", [heat.rows, heat.cols], [config.heat_size, config.heat_size], source="Python heatmap state")
    validation.equal("line update generation count", metrics.line_updates, sum(tick % config.line_every == 0 for tick in metrics.applied_ticks) * config.line_plots, source="applied generation counters")
    validation.equal("scatter update generation count", metrics.scatter_updates, sum(tick % config.scatter_every == 0 for tick in metrics.applied_ticks), source="applied generation counters")
    validation.equal("heat update generation count", metrics.heat_updates, sum(tick % config.heat_every == 0 for tick in metrics.applied_ticks), source="applied generation counters")
    native_sends = (runtime.get("python") or {}).get("native_sends") or {}
    native_send_methods = native_sends.get("methods") or {}
    native_batches = native_sends.get("batches") or {}
    validation.equal(
        "native send method accounting",
        sum(int(entry.get("requested") or 0) for entry in native_send_methods.values()),
        native_sends.get("requested"),
        source="Python/native boundary diagnostics",
    )
    command_queue = runtime.get("command_queue") or {}
    validation.require(
        "native queue diagnostics present",
        command_queue,
        lambda value: all(key in value for key in ("depth", "pushes", "push_timing")),
        "command queue depth, push count, and timing",
        source="native command queue diagnostics",
    )
    validation.equal(
        "native queue timing accounting",
        (command_queue.get("push_timing") or {}).get("count"),
        command_queue.get("pushes"),
        source="native command queue diagnostics",
    )
    validation.equal(
        "native queue snapshot drained",
        command_queue.get("depth"),
        0,
        source="native command queue diagnostics",
    )
    if args.update_mode == "batch":
        validation.equal(
            "one property packet per applied dashboard frame",
            native_batches.get("packets"),
            metrics.completed_ticks,
            source="Python update batch diagnostics",
        )
        validation.equal(
            "batch packet sender accounting",
            (native_send_methods.get("enqueue_set_props") or {}).get("requested"),
            native_batches.get("packets"),
            source="Python/native boundary diagnostics",
        )
        validation.equal(
            "all dashboard property updates entered batch packets",
            native_batches.get("updates_submitted"),
            metrics.completed_ticks * (config.controls + 1),
            source="Python update batch diagnostics",
        )

    report = _base("DragonGUI", _version("dragongui"), args, config)
    report.update({
        "build_ms": build_ms,
        "run_wall_ms": run_wall_ms,
        "drain_recovery_ms": drain_recovery_ms,
        "metrics": metrics.report(),
        "validation": validation.report(),
        "native": {
            "runtime": {
                "frames_rendered": runtime.get("frames_rendered"),
                "wall_fps": runtime.get("wall_fps"),
                "frame_timings": runtime.get("frame_timings"),
                "command_queue_depth": runtime.get("command_queue_depth"),
                "command_queue": runtime.get("command_queue"),
                "command_drain": runtime.get("command_drain"),
                "command_timings": runtime.get("command_timings"),
                "command_dirty_counts": runtime.get("command_dirty_counts"),
                "commands": runtime.get("commands"),
                "command_drain_yields": runtime.get("command_drain_yields"),
                "python": runtime.get("python"),
            },
            "framework": gpu.get("framework"),
            "renderer": renderer,
            "line_resources": line_resources,
            "scatter_resources": scatter_resources,
        },
    })
    return report


def _run_loop(
    metrics: RunMetrics,
    apply: Callable[[int], None],
    render: Callable[[], None],
    queue_depth: Callable[[], int | None] | None = None,
) -> None:
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
        if tick % 60 == 0:
            metrics.sample(tick, queue_depth=queue_depth() if queue_depth else None)
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


def run_dearpygui(args: argparse.Namespace, config: LoadConfig) -> dict[str, Any]:
    sys.path.insert(0, str(ROOT / "artifacts" / "benchmark-deps"))
    import dearpygui.dearpygui as dpg

    data = DashboardData(config)
    metrics = RunMetrics(round(args.warmup_seconds * TARGET_HZ), round(args.measure_seconds * TARGET_HZ))
    build_started = time.perf_counter()
    dpg.create_context()
    dpg.create_viewport(title="Dear PyGui live dashboard benchmark", width=1400, height=900)
    line_tags: list[str] = []
    control_tags: list[str] = []
    with dpg.window(tag="primary"):
        dpg.add_text("tick 0", tag="dashboard-status")
        for row_start in range(0, config.line_plots, 3):
            with dpg.group(horizontal=True):
                for channel in range(row_start, min(row_start + 3, config.line_plots)):
                    with dpg.plot(width=400, height=210):
                        dpg.add_plot_axis(dpg.mvXAxis)
                        with dpg.plot_axis(dpg.mvYAxis):
                            x, y = data.line(channel, 0)
                            tag = f"line-{channel}"
                            dpg.add_line_series(x.tolist(), y.tolist(), tag=tag)
                            line_tags.append(tag)
        with dpg.group(horizontal=True):
            with dpg.plot(width=400, height=210):
                dpg.add_plot_axis(dpg.mvXAxis)
                with dpg.plot_axis(dpg.mvYAxis):
                    sx, sy = data.scatter(0)
                    dpg.add_scatter_series(sx.tolist(), sy.tolist(), tag="scatter")
            with dpg.plot(width=400, height=210):
                dpg.add_plot_axis(dpg.mvXAxis)
                with dpg.plot_axis(dpg.mvYAxis):
                    initial_heat = data.heat(0)
                    dpg.add_heat_series(initial_heat.ravel().tolist(), config.heat_size, config.heat_size, tag="heat")
        with dpg.child_window(height=160, autosize_x=True):
            for index in range(config.controls):
                tag = f"sensor-{index}"
                dpg.add_text(f"sensor {index:03d}: 0", tag=tag)
                control_tags.append(tag)
    dpg.set_primary_window("primary", True)
    dpg.setup_dearpygui()
    dpg.set_viewport_vsync(False)
    build_ms = (time.perf_counter() - build_started) * 1000.0
    dpg.show_viewport()
    dpg.render_dearpygui_frame()

    def apply(tick: int) -> None:
        if tick % config.line_every == 0:
            for channel, tag in enumerate(line_tags):
                x, y = data.line(channel, tick)
                dpg.set_value(tag, [x.tolist(), y.tolist()])
                metrics.line_updates += 1
        if tick % config.scatter_every == 0:
            sx, sy = data.scatter(tick)
            dpg.set_value("scatter", [sx.tolist(), sy.tolist()])
            metrics.scatter_updates += 1
        if tick % config.heat_every == 0:
            heat_value = dpg.get_value("heat")
            heat_value[0] = data.heat(tick).ravel().tolist()
            dpg.set_value("heat", heat_value)
            metrics.heat_updates += 1
        dpg.set_value("dashboard-status", f"tick {tick}")
        for index, tag in enumerate(control_tags):
            dpg.set_value(tag, f"sensor {index:03d}: {(tick + index) % 100}")
        metrics.control_updates += len(control_tags)

    _run_loop(metrics, apply, dpg.render_dearpygui_frame)
    validation = ValidationRecorder()
    validation.equal("scheduled ticks accounted for", metrics.completed_ticks + metrics.dropped_ticks, metrics.total_ticks, source="benchmark loop")
    validation.equal("status reached final scheduled tick", dpg.get_value("dashboard-status"), f"tick {metrics.total_ticks - 1}", source="Dear PyGui item state")
    line_value = dpg.get_value(line_tags[-1])
    validation.equal("last line retains expected points", len(line_value[0]), config.line_points, source="Dear PyGui item state")
    scatter_value = dpg.get_value("scatter")
    validation.equal("scatter retains expected points", len(scatter_value[0]), config.scatter_points, source="Dear PyGui item state")
    validation.equal("all line items exist", sum(dpg.does_item_exist(tag) for tag in line_tags), config.line_plots, source="Dear PyGui registry")
    report = _base("Dear PyGui", _version("dearpygui"), args, config)
    report.update({"build_ms": build_ms, "metrics": metrics.report(), "validation": validation.report()})
    dpg.destroy_context()
    return report


def run_pyqtgraph(args: argparse.Namespace, config: LoadConfig) -> dict[str, Any]:
    os.environ.setdefault("QT_ENABLE_HIGHDPI_SCALING", "1")
    from PyQt6 import QtCore, QtWidgets
    import pyqtgraph as pg

    data = DashboardData(config)
    metrics = RunMetrics(round(args.warmup_seconds * TARGET_HZ), round(args.measure_seconds * TARGET_HZ))
    app = QtWidgets.QApplication.instance() or QtWidgets.QApplication([])
    window = QtWidgets.QMainWindow()
    window.resize(1400, 900)
    root = QtWidgets.QWidget()
    outer = QtWidgets.QVBoxLayout(root)
    status = QtWidgets.QLabel("tick 0")
    outer.addWidget(status)
    grid = QtWidgets.QGridLayout()
    outer.addLayout(grid, 1)
    curves: list[Any] = []
    build_started = time.perf_counter()
    for channel in range(config.line_plots):
        plot = pg.PlotWidget()
        x, y = data.line(channel, 0)
        curves.append(plot.plot(x, y, pen=pg.mkPen(width=1.5)))
        grid.addWidget(plot, channel // 3, channel % 3)
    scatter_plot = pg.PlotWidget()
    sx, sy = data.scatter(0)
    scatter = pg.ScatterPlotItem(x=sx, y=sy, size=3, pen=None, brush=pg.mkBrush(90, 210, 180, 180))
    scatter_plot.addItem(scatter)
    grid.addWidget(scatter_plot, 2, 0)
    heat_plot = pg.PlotWidget()
    heat = pg.ImageItem(data.heat(0))
    heat_plot.addItem(heat)
    grid.addWidget(heat_plot, 2, 1)
    controls = QtWidgets.QListWidget()
    for index in range(config.controls):
        controls.addItem(f"sensor {index:03d}: 0")
    grid.addWidget(controls, 2, 2)
    window.setCentralWidget(root)
    build_ms = (time.perf_counter() - build_started) * 1000.0
    window.show()
    app.processEvents()

    def apply(tick: int) -> None:
        if tick % config.line_every == 0:
            for channel, curve in enumerate(curves):
                x, y = data.line(channel, tick)
                curve.setData(x, y)
                metrics.line_updates += 1
        if tick % config.scatter_every == 0:
            sx, sy = data.scatter(tick)
            scatter.setData(x=sx, y=sy)
            metrics.scatter_updates += 1
        if tick % config.heat_every == 0:
            heat.setImage(data.heat(tick), autoLevels=False)
            metrics.heat_updates += 1
        status.setText(f"tick {tick}")
        for index in range(config.controls):
            controls.item(index).setText(f"sensor {index:03d}: {(tick + index) % 100}")
        metrics.control_updates += config.controls

    def render() -> None:
        window.repaint()
        app.processEvents(QtCore.QEventLoop.ProcessEventsFlag.AllEvents)

    _run_loop(metrics, apply, render)
    validation = ValidationRecorder()
    validation.equal("scheduled ticks accounted for", metrics.completed_ticks + metrics.dropped_ticks, metrics.total_ticks, source="benchmark loop")
    validation.equal("status reached final scheduled tick", status.text(), f"tick {metrics.total_ticks - 1}", source="Qt widget state")
    validation.equal("last line retains expected points", len(curves[-1].xData), config.line_points, source="PyQtGraph curve state")
    scatter_data = scatter.getData()
    validation.equal("scatter retains expected points", len(scatter_data[0]), config.scatter_points, source="PyQtGraph scatter state")
    validation.equal("heatmap retains expected dimensions", list(heat.image.shape), [config.heat_size, config.heat_size], source="PyQtGraph image state")
    report = _base("PyQtGraph", _version("pyqtgraph"), args, config)
    report.update({"build_ms": build_ms, "metrics": metrics.report(), "validation": validation.report()})
    window.close()
    app.processEvents()
    return report


RUNNERS: dict[str, Callable[[argparse.Namespace, LoadConfig], dict[str, Any]]] = {
    "dragongui": run_dragongui,
    "dearpygui": run_dearpygui,
    "pyqtgraph": run_pyqtgraph,
}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--framework", required=True, choices=sorted(RUNNERS))
    parser.add_argument("--load", required=True, choices=sorted(LOADS))
    parser.add_argument("--warmup-seconds", type=float, default=5.0)
    parser.add_argument("--measure-seconds", type=float, default=60.0)
    parser.add_argument(
        "--update-mode",
        choices=("individual", "batch"),
        default="individual",
        help="DragonGUI ordinary property transport; other adapters use native behavior",
    )
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    args.warmup_seconds = max(0.1, args.warmup_seconds)
    args.measure_seconds = max(0.2, args.measure_seconds)
    report = RUNNERS[args.framework](args, LOADS[args.load])
    payload = json.dumps(report, indent=2, sort_keys=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
    print(payload)
    validation = report.get("validation") or {}
    return 0 if validation.get("passed") is True else 2


if __name__ == "__main__":
    raise SystemExit(main())
