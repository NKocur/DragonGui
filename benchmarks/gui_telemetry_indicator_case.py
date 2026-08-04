"""Profile one DragonGUI telemetry-indicator family at a fixed 30 Hz cadence."""

from __future__ import annotations

import argparse
from contextlib import nullcontext
from dataclasses import dataclass
import json
import os
from pathlib import Path
import platform
import sys
import threading
import time
from typing import Any

from gui_benchmark_validation import ValidationRecorder, find_tree_node, layout_issue_count
from gui_live_dashboard_case import RunMetrics, _pace
from gui_telemetry_viewer_case import TARGET_HZ, TARGET_S, TelemetryData


ROOT = Path(__file__).resolve().parents[1]
MODES = ("labels", "progress", "limits", "leds", "combined")
COUNTS = (24, 72, 100, 160, 320)


@dataclass(frozen=True)
class IndicatorConfig:
    count: int
    mode: str

    @property
    def properties_per_tick(self) -> int:
        per_indicator = {
            "labels": 1,
            "progress": 1,
            "limits": 1,
            "leds": 2,
            "combined": 4,
        }[self.mode]
        return self.count * per_indicator + 1  # status label


def run_case(args: argparse.Namespace, config: IndicatorConfig) -> dict[str, Any]:
    benchmark_python_path = os.environ.get("DRAGONGUI_BENCH_PYTHON_PATH")
    sys.path.insert(0, benchmark_python_path or str(ROOT / "python"))
    import dragongui as dg

    metrics = RunMetrics(
        round(args.warmup_seconds * TARGET_HZ),
        round(args.measure_seconds * TARGET_HZ),
    )
    app = dg.App(loading_screen=False)
    window = dg.Window(
        f"DragonGUI telemetry indicators: {config.mode} × {config.count}",
        width=900,
        height=860,
        style={"overflow": "hidden"},
    )
    labels: list[Any] = []
    bars: list[Any] = []
    limits_bars: list[Any] = []
    leds: list[Any] = []

    def build_indicator_rows() -> None:
        for index in range(config.count):
            value, text, healthy = TelemetryData.indicator(index, 0)
            with dg.HLayout(style={"height": 22, "gap": 5, "align_items": "center"}):
                if config.mode in {"leds", "combined"}:
                    leds.append(dg.LED(healthy, size=9, id=f"indicator-led-{index}"))
                if config.mode in {"labels", "combined"}:
                    labels.append(
                        dg.Label(
                            text,
                            wrap=False,
                            id=f"indicator-label-{index}",
                            style={"width": 150},
                        )
                    )
                if config.mode in {"progress", "combined"}:
                    bars.append(
                        dg.ProgressBar(
                            value / 100.0,
                            id=f"indicator-progress-{index}",
                            style={"width": 220, "height": 9},
                        )
                    )
                if config.mode == "limits":
                    limits_bars.append(
                        dg.LimitsBar(
                            value,
                            min=0,
                            red_low=10,
                            yellow_low=25,
                            yellow_high=75,
                            red_high=90,
                            id=f"indicator-limits-{index}",
                            style={"width": "100%", "height": 14},
                        )
                    )

    build_started = time.perf_counter()
    with dg.VLayout(style={"height": "100%", "padding": 6, "gap": 6}):
        with dg.HLayout(style={"height": 28, "gap": 10, "align_items": "center"}):
            status = dg.Label("tick 0", id="indicator-status", style={"width": 110})
            dg.Label(f"{config.mode} · {config.count} channels · {TARGET_HZ:.0f} Hz", wrap=False)
        with dg.ScrollArea(
            axis="y",
            style={"flex_grow": 1, "min_height": 0, "height": "100%", "gap": 2, "padding": 4},
        ):
            if config.mode == "limits":
                with dg.GridLayout(columns=args.limits_columns, gap=2, style={"width": "100%"}):
                    build_indicator_rows()
            else:
                build_indicator_rows()
    build_ms = (time.perf_counter() - build_started) * 1000.0

    producer_done = threading.Event()
    drain_recovery_ms = 0.0

    def producer() -> None:
        nonlocal drain_recovery_ms
        handle = None
        ready_deadline = time.perf_counter() + 30.0
        while time.perf_counter() < ready_deadline:
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
                update_context = app.update_batch() if args.update_mode == "batch" else nullcontext()
                with update_context:
                    if not args.static_status or tick == metrics.total_ticks - 1:
                        status.set_value(f"tick {tick}")
                    for index in range(config.count):
                        value, text, healthy = TelemetryData.indicator(index, tick)
                        if labels:
                            labels[index].set_value(text)
                        if bars:
                            bars[index].set_value(value / 100.0)
                        if limits_bars:
                            limits_bars[index].set_value(value)
                        if leds:
                            leds[index].set_on(healthy)
                # This is offered API work, not necessarily native traffic:
                # widgets may safely suppress unchanged properties.
                metrics.control_updates += config.properties_per_tick
                elapsed = (time.perf_counter() - started) * 1000.0
                if metrics.measuring(tick):
                    metrics.submit_ms.append(elapsed)
                metrics.completed_ticks += 1
                metrics.last_tick = tick
                metrics.applied_ticks.append(tick)
                if metrics.measuring(tick):
                    metrics.measurement_completed_ticks += 1

            app.call_soon_threadsafe(apply, coalesce_key="indicator-frame")
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

    threading.Thread(target=producer, name="indicator-producer", daemon=True).start()
    result = app.run(window)
    snapshot = result.get("debug_snapshot") or {}
    gpu = snapshot.get("gpu") or {}
    runtime = snapshot.get("runtime") or {}
    renderer = gpu.get("renderer") or {}
    framework = gpu.get("framework") or {}
    final_tick = metrics.total_ticks - 1
    _first_value, first_text, first_healthy = TelemetryData.indicator(0, final_tick)
    last_value, last_text, last_healthy = TelemetryData.indicator(config.count - 1, final_tick)

    validation = ValidationRecorder()
    validation.equal("producer finished", producer_done.is_set(), True, source="benchmark producer")
    validation.equal("all measurement ticks completed by deadline", metrics.measurement_completed_at_window_end, metrics.measure_ticks, source="producer deadline")
    validation.equal("status reached final tick", status.text, f"tick {final_tick}", source="Python widget state")
    native_status = find_tree_node(gpu.get("tree"), status.id)
    validation.equal("native status reached final tick", (native_status or {}).get("props", {}).get("text"), f"tick {final_tick}", source="native retained tree")
    if labels:
        validation.equal("first label final value", labels[0].text, first_text, source="Python widget state")
        validation.equal("last label final value", labels[-1].text, last_text, source="Python widget state")
    if bars:
        validation.equal("last progress final value", round(float(bars[-1].value), 4), round(last_value / 100.0, 4), source="Python widget state")
    if limits_bars:
        validation.equal(
            "first limits bar final value",
            round(float(limits_bars[0].value), 4),
            round(_first_value, 4),
            source="Python widget state",
        )
        validation.equal(
            "last limits bar final value",
            round(float(limits_bars[-1].value), 4),
            round(last_value, 4),
            source="Python widget state",
        )
        native_first_limits = find_tree_node(gpu.get("tree"), limits_bars[0].id)
        native_last_limits = find_tree_node(gpu.get("tree"), limits_bars[-1].id)
        validation.equal(
            "native first limits bar final value",
            round(float((native_first_limits or {}).get("props", {}).get("value", -1)), 4),
            round(_first_value, 4),
            source="native retained tree",
        )
        validation.equal(
            "native last limits bar final value",
            round(float((native_last_limits or {}).get("props", {}).get("value", -1)), 4),
            round(last_value, 4),
            source="native retained tree",
        )
    if leds:
        validation.equal("first LED final state", leds[0].on, first_healthy, source="Python widget state")
        validation.equal("last LED final state", leds[-1].on, last_healthy, source="Python widget state")
    validation.equal("command queue drained", runtime.get("command_queue_depth"), 0, source="native runtime snapshot")
    validation.equal("layout remained clean", layout_issue_count(snapshot), 0, source="native layout diagnostics")
    retained_rebuilds = ((renderer.get("primitives") or {}).get("retained_rebuilds") or {})
    if limits_bars and args.update_mode == "batch" and args.static_status:
        validation.require(
            "visual-only limits frames used targeted primitive rebuilds",
            int(retained_rebuilds.get("partial_base_completed") or 0),
            lambda observed: observed >= max(metrics.completed_ticks - 1, 0),
            f">={max(metrics.completed_ticks - 1, 0)}",
            source="native retained primitive diagnostics",
        )
        rejected = ((framework.get("command_text_rebuilds") or {}).get("rejected_batches") or {})
        validation.equal(
            "eligible limits batches were not rejected from targeted rebuilding",
            sum(int(value or 0) for value in rejected.values()),
            0,
            source="native deferred rebuild diagnostics",
        )
    native_sends = (runtime.get("python") or {}).get("native_sends") or {}
    native_batches = native_sends.get("batches") or {}
    if args.update_mode == "batch":
        validation.equal("one property packet per completed tick", native_batches.get("packets"), metrics.completed_ticks, source="Python/native batch diagnostics")
        submitted_updates = int(native_batches.get("updates_submitted") or 0)
        offered_updates = metrics.completed_ticks * config.properties_per_tick
        validation.require(
            "native property traffic did not exceed offered updates",
            submitted_updates,
            lambda observed: 0 < observed <= offered_updates,
            f"1..{offered_updates}",
            source="Python/native batch diagnostics",
        )

    metrics_report = metrics.report()
    return {
        "schema": 1,
        "benchmark": "telemetry_indicator_decomposition",
        "framework": "DragonGUI",
        "framework_version": "0.1.0",
        "python": platform.python_version(),
        "platform": platform.platform(),
        "mode": config.mode,
        "count": config.count,
        "target_hz": TARGET_HZ,
        "update_mode": args.update_mode,
        "static_status": args.static_status,
        "limits_columns": args.limits_columns if config.mode == "limits" else None,
        "properties_per_tick": config.properties_per_tick,
        "native_properties_per_completed_tick": (
            float(((native_batches.get("updates_submitted") or 0) / metrics.completed_ticks))
            if metrics.completed_ticks
            else 0.0
        ),
        "build_ms": build_ms,
        "drain_recovery_ms": drain_recovery_ms,
        "metrics": metrics_report,
        "validation": validation.report(),
        "native": {
            "layout_diagnostic_count": layout_issue_count(snapshot),
            "runtime": {
                "wall_fps": runtime.get("wall_fps"),
                "frames_rendered": runtime.get("frames_rendered"),
                "frame_timings": runtime.get("frame_timings"),
                "command_queue": runtime.get("command_queue"),
                "command_drain": runtime.get("command_drain"),
                "command_drain_yields": runtime.get("command_drain_yields"),
                "dirty_rebuilds": framework.get("dirty_rebuilds"),
                "command_text_rebuilds": framework.get("command_text_rebuilds"),
                "python": runtime.get("python"),
            },
            "renderer": {
                "widget_count": renderer.get("widget_count"),
                "primitives": renderer.get("primitives"),
                "text": renderer.get("text"),
                "layout_text_measurement": renderer.get("layout_text_measurement"),
            },
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=MODES, required=True)
    parser.add_argument("--count", choices=COUNTS, type=int, required=True)
    parser.add_argument("--warmup-seconds", type=float, default=2.0)
    parser.add_argument("--measure-seconds", type=float, default=8.0)
    parser.add_argument("--update-mode", choices=("individual", "batch"), default="batch")
    parser.add_argument(
        "--static-status",
        action="store_true",
        help="Update the status label only on the final tick to isolate indicator paint work.",
    )
    parser.add_argument(
        "--limits-columns",
        type=int,
        default=5,
        help="Grid columns for LimitsBar mode; five keeps 100 bars visible at once.",
    )
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.limits_columns < 1:
        parser.error("--limits-columns must be at least 1")
    report = run_case(args, IndicatorConfig(args.count, args.mode))
    payload = json.dumps(report, indent=2, sort_keys=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
    print(payload)
    return 0 if report["validation"]["passed"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
