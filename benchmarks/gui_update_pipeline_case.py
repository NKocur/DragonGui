"""Validated DragonGUI-only probes for the live widget update pipeline.

These cases separate fixed text, intrinsic text, mixed widget state, and native
queue replacement behavior. Performance output is rejected when final retained
state, queue accounting, or layout diagnostics are incorrect.
"""

from __future__ import annotations

import argparse
from contextlib import nullcontext
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import sys
import threading
import time
from typing import Any, Callable


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "benchmarks"))

from gui_benchmark_validation import (  # noqa: E402
    ValidationRecorder,
    find_tree_node,
    layout_issue_count,
)


SCENARIOS = (
    "labels-fixed",
    "intrinsic-text",
    "composite-text-fixed",
    "composite-text-intrinsic",
    "mixed-state",
    "same-property-burst",
    "distinct-property-burst",
    "ordered-barrier",
)


def _timings(values: list[float]) -> dict[str, float | int]:
    if not values:
        return {
            "count": 0,
            "mean_ms": 0.0,
            "median_ms": 0.0,
            "p95_ms": 0.0,
            "p99_ms": 0.0,
            "max_ms": 0.0,
        }
    ordered = sorted(values)

    def percentile(fraction: float) -> float:
        index = round((len(ordered) - 1) * fraction)
        return ordered[index]

    return {
        "count": len(values),
        "mean_ms": statistics.fmean(values),
        "median_ms": statistics.median(values),
        "p95_ms": percentile(0.95),
        "p99_ms": percentile(0.99),
        "max_ms": max(values),
    }


def _pace(deadline: float) -> float:
    remaining = deadline - time.perf_counter()
    if remaining > 0:
        time.sleep(remaining)
    return max(0.0, (time.perf_counter() - deadline) * 1000.0)


def _fixed_text(index: int, tick: int) -> str:
    return f"sensor {index:04d}: {tick:06d}"


def _intrinsic_text(index: int, tick: int) -> str:
    if tick % 3 == 0:
        return f"record {index}: {tick}"
    if tick % 3 == 1:
        return (
            f"record {index}: generation {tick} carries a deliberately long status message "
            "that may wrap and change intrinsic height"
        )
    return f"record {index}: generation {tick}\nsecondary diagnostic line"


def _composite_text(family: str, index: int, tick: int) -> str:
    if tick % 3 == 0:
        return f"{family} {index}: {tick}"
    if tick % 3 == 1:
        return f"{family} {index}: deliberately long generation {tick} content"
    return f"{family} {index}: {tick} / alternate"


def _set_live_text(widget: Any, attribute: str, prop: str, value: str) -> None:
    setattr(widget, attribute, value)
    if (handle := widget._live()) is not None:
        handle.enqueue_set_prop(prop, value)


def _wait_for_ready(app: Any, timeout_s: float = 30.0) -> dict[str, Any]:
    deadline = time.perf_counter() + timeout_s
    snapshot: dict[str, Any] = {}
    while time.perf_counter() < deadline:
        try:
            snapshot = app.debug_snapshot(timeout_ms=3000)
        except (RuntimeError, TimeoutError):
            snapshot = {}
        runtime = snapshot.get("runtime") or {}
        if runtime.get("startup_readiness") == "application_frame_presented":
            return snapshot
        time.sleep(0.01)
    raise TimeoutError("DragonGUI application frame was not presented before timeout")


def _equivalence_probe(gpu: dict[str, Any], widget_ids: list[str]) -> dict[str, Any]:
    tree = gpu.get("tree")
    diagnostics = ((gpu.get("layout") or {}).get("diagnostics") or {})
    widgets: dict[str, Any] = {}
    for widget_id in widget_ids:
        node = find_tree_node(tree, widget_id)
        entry = diagnostics.get(widget_id) or {}
        widgets[widget_id] = {
            "node": node,
            "resolved": entry.get("resolved"),
            "available": entry.get("available"),
            "overflow": entry.get("overflow"),
            "paint_clip": (entry.get("inspection") or {}).get("final_paint_clip"),
            "dynamic_geometry": entry.get("dynamic_geometry"),
        }
    layout = gpu.get("layout") or {}
    scroll = {
        widget_id: {
            "x": (layout.get("scroll_x") or {}).get(widget_id),
            "y": (layout.get("scroll_y") or {}).get(widget_id),
            "max_x": (layout.get("scroll_max_x") or {}).get(widget_id),
            "max_y": (layout.get("scroll_max_y") or {}).get(widget_id),
        }
        for widget_id in widget_ids
    }
    canonical = json.dumps(
        {"widgets": widgets, "scroll": scroll}, sort_keys=True, separators=(",", ":")
    )
    return {
        "schema": 1,
        "sha256": hashlib.sha256(canonical.encode("utf-8")).hexdigest(),
        "widgets": widgets,
        "scroll": scroll,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scenario", choices=SCENARIOS, required=True)
    parser.add_argument("--widgets", type=int, default=200)
    parser.add_argument("--burst-repeats", type=int, default=100)
    parser.add_argument("--warmup-seconds", type=float, default=1.0)
    parser.add_argument("--measure-seconds", type=float, default=10.0)
    parser.add_argument("--target-hz", type=float, default=60.0)
    parser.add_argument("--update-mode", choices=("individual", "batch"), default="individual")
    parser.add_argument("--capture-screenshot", action="store_true")
    parser.add_argument(
        "--screenshot-rgba-output",
        type=Path,
        help="Optional raw RGBA dump for visual diagnosis; requires --capture-screenshot.",
    )
    parser.add_argument("--interaction-probe-ms", type=float, default=0.0)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--quiet", action="store_true")
    args = parser.parse_args()
    args.widgets = max(1, args.widgets)
    args.burst_repeats = max(2, args.burst_repeats)
    args.warmup_seconds = max(0.1, args.warmup_seconds)
    args.measure_seconds = max(0.2, args.measure_seconds)
    args.target_hz = max(1.0, args.target_hz)
    args.interaction_probe_ms = max(0.0, args.interaction_probe_ms)

    benchmark_python_path = os.environ.get("DRAGONGUI_BENCH_PYTHON_PATH")
    sys.path.insert(0, benchmark_python_path or str(ROOT / "python"))
    import dragongui as dg

    app = dg.App(loading_screen=False)
    window = dg.Window(f"DragonGUI update pipeline: {args.scenario}", width=1000, height=760)
    labels: list[Any] = []
    badges: list[Any] = []
    leds: list[Any] = []
    progress_bars: list[Any] = []
    composite_targets: list[tuple[Any, str, str, str, int]] = []
    composite_rows: list[Any] = []

    build_t0 = time.perf_counter()
    with dg.VLayout(style={"height": "100%", "padding": 8, "gap": 6}):
        status = dg.Label("tick -1", id="pipeline-status", style={"height": 24})
        scroll_height: object = (
            180
            if args.scenario in ("composite-text-fixed", "composite-text-intrinsic")
            else "calc(100% - 30px)"
        )
        with dg.ScrollArea(
            axis="y",
            id="pipeline-scroll",
            style={
                "height": scroll_height,
                "gap": 2,
                **(
                    {"flex-grow": 0, "flex-shrink": 0}
                    if args.scenario in ("composite-text-fixed", "composite-text-intrinsic")
                    else {}
                ),
            },
        ) as scroller:
            if args.scenario == "labels-fixed":
                for index in range(args.widgets):
                    labels.append(
                        dg.Label(
                            _fixed_text(index, 0),
                            id=f"pipeline-label-{index}",
                            wrap=False,
                            style={"height": 22, "width": 240},
                        )
                    )
            elif args.scenario == "intrinsic-text":
                for index in range(args.widgets):
                    labels.append(
                        dg.Label(
                            _intrinsic_text(index, 0),
                            id=f"pipeline-label-{index}",
                            wrap=True,
                            style={"width": "100%", "min-height": 22},
                        )
                    )
            elif args.scenario == "mixed-state":
                for index in range(args.widgets):
                    with dg.HLayout(style={"height": 28, "gap": 6, "align-items": "center"}):
                        leds.append(dg.LED(False, id=f"pipeline-led-{index}"))
                        labels.append(
                            dg.Label(
                                _fixed_text(index, 0),
                                id=f"pipeline-label-{index}",
                                wrap=False,
                                style={"width": 210},
                            )
                        )
                        badges.append(dg.Badge("000000", id=f"pipeline-badge-{index}"))
                        progress_bars.append(
                            dg.ProgressBar(
                                0.0,
                                id=f"pipeline-progress-{index}",
                                style={"width": 180},
                            )
                        )
            elif args.scenario in ("composite-text-fixed", "composite-text-intrinsic"):
                intrinsic = args.scenario.endswith("intrinsic")
                component_styles = (
                    {"width": 150, "height": 32}
                    if not intrinsic
                    else {"width": 150, "height": "auto"}
                )
                for index in range(args.widgets):
                    with dg.HLayout(
                        id=f"pipeline-composite-row-{index}",
                        style={"width": "100%", "gap": 6, "align-items": "start"},
                    ) as row:
                        composite_rows.append(row)
                        button = dg.Button(
                            _composite_text("button", index, 0),
                            id=f"pipeline-button-{index}",
                            badge="0",
                            style=component_styles,
                        )
                        badge = dg.Badge(
                            _composite_text("badge", index, 0),
                            id=f"pipeline-composite-badge-{index}",
                            style={**component_styles, "width": 110},
                        )
                        spinner = dg.LoadingSpinner(
                            label=_composite_text("spinner", index, 0),
                            spinning=False,
                            id=f"pipeline-spinner-{index}",
                            style={**component_styles, "width": 170},
                        )
                        nav = dg.NavItem(
                            _composite_text("nav", index, 0),
                            page=f"page-{index}",
                            badge="0",
                            id=f"pipeline-nav-{index}",
                            style={**component_styles, "width": 150},
                        )
                        panel = dg.Panel(
                            _composite_text("panel", index, 0),
                            id=f"pipeline-composite-panel-{index}",
                            style={**component_styles, "width": 300},
                        )
                    composite_targets.extend(
                        [
                            (button, "text", "text", "button", index),
                            (badge, "text", "text", "badge", index),
                            (spinner, "label", "label", "spinner", index),
                            (nav, "label", "label", "nav", index),
                            (panel, "title", "title", "panel", index),
                        ]
                    )
            elif args.scenario == "same-property-burst":
                labels.append(
                    dg.Label(
                        "burst 000000",
                        id="pipeline-label-0",
                        wrap=False,
                        style={"height": 22, "width": 240},
                    )
                )
            elif args.scenario == "ordered-barrier":
                for index in range(args.widgets):
                    labels.append(
                        dg.Label(
                            f"burst {index:04d}: 000000-B",
                            id=f"pipeline-label-{index}",
                            wrap=False,
                            style={"height": 22, "width": 260},
                        )
                    )
            else:
                for index in range(args.widgets):
                    labels.append(
                        dg.Label(
                            f"burst {index:04d}: 000000-A",
                            id=f"pipeline-label-{index}",
                            wrap=False,
                            style={"height": 22, "width": 260},
                        )
                    )
    build_ms = (time.perf_counter() - build_t0) * 1000.0

    warmup_ticks = round(args.warmup_seconds * args.target_hz)
    measure_ticks = round(args.measure_seconds * args.target_hz)
    total_ticks = warmup_ticks + measure_ticks
    target_s = 1.0 / args.target_hz
    callback_ms: list[float] = []
    schedule_lag_ms: list[float] = []
    applied_ticks: list[int] = []
    completed = 0
    measurement_completed = 0
    measurement_completed_at_window_end = 0
    measurement_wall_start = 0.0
    measurement_wall_end = 0.0
    measurement_cpu_start = 0.0
    measurement_cpu_end = 0.0
    ready_snapshot: dict[str, Any] = {}
    recovery_ms = 0.0
    producer_error: str | None = None
    producer_done = threading.Event()
    state_lock = threading.Lock()
    screenshot_capture: dict[str, Any] | None = None
    interaction_ms: list[float] = []
    interaction_error: str | None = None
    interaction_ready = threading.Event()
    interaction_stop = threading.Event()

    def apply_tick(tick: int) -> None:
        nonlocal completed, measurement_completed
        callback_t0 = time.perf_counter()
        update_context = app.update_batch() if args.update_mode == "batch" else nullcontext()
        with update_context:
            if args.scenario == "labels-fixed":
                for index, label in enumerate(labels):
                    label.set_value(_fixed_text(index, tick))
            elif args.scenario == "intrinsic-text":
                for index, label in enumerate(labels):
                    label.set_value(_intrinsic_text(index, tick))
            elif args.scenario == "mixed-state":
                for index, label in enumerate(labels):
                    label.set_value(_fixed_text(index, tick))
                    badges[index].set_value(f"{(tick + index) % 1_000_000:06d}")
                    leds[index].set_on((tick + index) % 2 == 0)
                    progress_bars[index].set_value(((tick + index) % 101) / 100.0)
            elif args.scenario in ("composite-text-fixed", "composite-text-intrinsic"):
                for widget, attribute, prop, family, index in composite_targets:
                    _set_live_text(
                        widget,
                        attribute,
                        prop,
                        _composite_text(family, index, tick),
                    )
            elif args.scenario == "same-property-burst":
                for repeat in range(args.burst_repeats):
                    labels[0].set_value(f"burst {tick:06d}-{repeat:06d}")
            elif args.scenario == "ordered-barrier":
                for index, label in enumerate(labels):
                    label.set_value(f"burst {index:04d}: {tick:06d}-A")
                labels[0].set_style(
                    {
                        "height": 22,
                        "width": 260,
                        "background": "#192330" if tick % 2 == 0 else "#202b38",
                    }
                )
                for index, label in enumerate(labels):
                    label.set_value(f"burst {index:04d}: {tick:06d}-B")
            else:
                for index, label in enumerate(labels):
                    label.set_value(f"burst {index:04d}: {tick:06d}-A")
                for index, label in enumerate(labels):
                    label.set_value(f"burst {index:04d}: {tick:06d}-B")
            status.set_value(f"tick {tick}")
        elapsed_ms = (time.perf_counter() - callback_t0) * 1000.0
        with state_lock:
            completed += 1
            applied_ticks.append(tick)
            if tick >= warmup_ticks:
                measurement_completed += 1
                callback_ms.append(elapsed_ms)

    def producer() -> None:
        nonlocal ready_snapshot, recovery_ms, producer_error
        nonlocal measurement_wall_start, measurement_wall_end
        nonlocal measurement_cpu_start, measurement_cpu_end
        nonlocal measurement_completed_at_window_end
        nonlocal screenshot_capture
        try:
            ready_snapshot = _wait_for_ready(app)
            next_deadline = time.perf_counter()
            for tick in range(total_ticks):
                if tick == warmup_ticks:
                    measurement_wall_start = time.perf_counter()
                    measurement_cpu_start = time.process_time()
                    interaction_ready.set()
                app.call_soon_threadsafe(
                    lambda value=tick: apply_tick(value),
                    coalesce_key="update-pipeline-generation",
                )
                next_deadline += target_s
                lag = _pace(next_deadline)
                if tick >= warmup_ticks:
                    schedule_lag_ms.append(lag)
            measurement_wall_end = time.perf_counter()
            measurement_cpu_end = time.process_time()
            interaction_stop.set()
            interaction_thread.join(timeout=15.0)
            if interaction_thread.is_alive():
                raise TimeoutError("interaction probe did not stop before recovery")
            with state_lock:
                measurement_completed_at_window_end = measurement_completed

            recovery_t0 = time.perf_counter()
            recovery_deadline = recovery_t0 + 15.0
            handle = getattr(app, "_handle", None)
            while handle is not None and time.perf_counter() < recovery_deadline:
                if not handle._python_debug_snapshot().get("queued_tasks"):
                    break
                time.sleep(0.005)
            while handle is not None and time.perf_counter() < recovery_deadline:
                snapshot = app.debug_snapshot(timeout_ms=3000)
                if not (snapshot.get("runtime") or {}).get("command_queue_depth"):
                    break
                time.sleep(0.005)
            if args.scenario in ("composite-text-fixed", "composite-text-intrinsic"):
                scroller.scroll_to(y=1_000_000)
                while handle is not None and time.perf_counter() < recovery_deadline:
                    snapshot = app.debug_snapshot(timeout_ms=3000)
                    if not (snapshot.get("runtime") or {}).get("command_queue_depth"):
                        break
                    time.sleep(0.005)
            recovery_ms = (time.perf_counter() - recovery_t0) * 1000.0
            time.sleep(0.25)
            if args.capture_screenshot:
                screenshot = app._window_screenshot(timeout_ms=10000)
                if screenshot is None:
                    raise RuntimeError("window screenshot API was unavailable")
                width, height, rgba = screenshot
                screenshot_capture = {
                    "width": width,
                    "height": height,
                    "rgba_bytes": len(rgba),
                    "sha256": hashlib.sha256(rgba).hexdigest(),
                }
                if args.screenshot_rgba_output is not None:
                    args.screenshot_rgba_output.parent.mkdir(parents=True, exist_ok=True)
                    args.screenshot_rgba_output.write_bytes(rgba)
            if handle is not None:
                handle.request_exit()
        except Exception as exc:  # pragma: no cover - surfaced by validation/report
            producer_error = f"{type(exc).__name__}: {exc}"
            handle = getattr(app, "_handle", None)
            if handle is not None:
                handle.request_exit()
        finally:
            interaction_stop.set()
            producer_done.set()

    def interaction_probe() -> None:
        nonlocal interaction_error
        if args.interaction_probe_ms <= 0:
            return
        try:
            if not interaction_ready.wait(timeout=30.0):
                raise TimeoutError("interaction probe did not reach measurement window")
            interval_s = args.interaction_probe_ms / 1000.0
            while not interaction_stop.is_set():
                probe_t0 = time.perf_counter()
                completed = app._latency_probe(timeout_ms=10000)
                elapsed_ms = (time.perf_counter() - probe_t0) * 1000.0
                if completed is not True:
                    raise RuntimeError("native latency probe API was unavailable")
                interaction_ms.append(elapsed_ms)
                interaction_stop.wait(interval_s)
        except Exception as exc:  # pragma: no cover - surfaced by validation/report
            interaction_error = f"{type(exc).__name__}: {exc}"
            interaction_stop.set()

    interaction_thread = threading.Thread(
        target=interaction_probe,
        name="update-pipeline-interaction-probe",
        daemon=True,
    )
    interaction_thread.start()
    threading.Thread(target=producer, name="update-pipeline-producer", daemon=True).start()
    run_t0 = time.perf_counter()
    result = app.run(window)
    run_wall_ms = (time.perf_counter() - run_t0) * 1000.0

    final_snapshot = result.get("debug_snapshot") or {}
    runtime = final_snapshot.get("runtime") or {}
    gpu = final_snapshot.get("gpu") or {}
    layout = gpu.get("layout") or {}
    framework = gpu.get("framework") or {}
    command_queue = runtime.get("command_queue") or {}
    python_runtime = runtime.get("python") or {}
    native_sends = python_runtime.get("native_sends") or {}
    method_stats = native_sends.get("methods") or {}
    final_tick = total_ticks - 1
    probe_ids = [status.id, scroller.id]
    if labels:
        probe_ids.extend([labels[0].id, labels[-1].id])
    if args.scenario == "mixed-state":
        for group in (badges, leds, progress_bars):
            probe_ids.extend([group[0].id, group[-1].id])
    if composite_targets:
        for family in ("button", "badge", "spinner", "nav", "panel"):
            matches = [target for target in composite_targets if target[3] == family]
            probe_ids.extend([matches[0][0].id, matches[-1][0].id])
    probe_ids = list(dict.fromkeys(probe_ids))
    equivalence = _equivalence_probe(gpu, probe_ids)

    validation = ValidationRecorder()
    validation.equal("producer finished", producer_done.is_set(), True, source="benchmark producer")
    validation.equal("producer error", producer_error, None, source="benchmark producer")
    if args.interaction_probe_ms > 0:
        validation.equal(
            "interaction probe error",
            interaction_error,
            None,
            source="concurrent request/response probe",
        )
        validation.require(
            "interaction probe collected samples",
            len(interaction_ms),
            lambda value: value >= 3,
            "at least three round-trip samples",
            source="concurrent request/response probe",
        )
    validation.require(
        "completed tick count is bounded",
        completed,
        lambda value: 0 < value <= total_ticks,
        f"between 1 and {total_ticks}",
        source="Python latest-generation accounting",
    )
    validation.equal(
        "final scheduled tick executed",
        applied_ticks[-1] if applied_ticks else None,
        final_tick,
        source="Python latest-generation accounting",
    )
    validation.equal(
        "final Python status",
        status.text,
        f"tick {final_tick}",
        source="Python widget state",
    )
    native_status = find_tree_node(gpu.get("tree"), status.id)
    validation.equal(
        "final native status",
        (native_status or {}).get("props", {}).get("text"),
        f"tick {final_tick}",
        source="native retained tree",
    )
    validation.equal(
        "layout remained clean",
        layout_issue_count(final_snapshot),
        0,
        source="native layout diagnostics",
    )
    validation.equal(
        "native command queue drained",
        runtime.get("command_queue_depth"),
        0,
        source="native runtime snapshot",
    )
    validation.require(
        "native queue diagnostics present",
        command_queue,
        lambda value: all(key in value for key in ("depth", "pushes", "push_timing")),
        "command queue depth, push count, and timing",
        source="native command queue diagnostics",
    )
    validation.equal(
        "native queue snapshot drained",
        command_queue.get("depth"),
        0,
        source="native command queue diagnostics",
    )
    validation.equal(
        "native queue timing accounting",
        (command_queue.get("push_timing") or {}).get("count"),
        command_queue.get("pushes"),
        source="native command queue diagnostics",
    )
    validation.equal(
        "native send method accounting",
        sum(int(entry.get("requested") or 0) for entry in method_stats.values()),
        native_sends.get("requested"),
        source="Python/native boundary diagnostics",
    )
    synthetic_targets = [
        target.strip()
        for target in os.environ.get("DRAGONGUI_SYNTHETIC_HOVER_IDS", "").split(",")
        if target.strip()
    ]
    if synthetic_targets:
        synthetic_input = runtime.get("synthetic_input") or {}
        validation.equal(
            "synthetic hover dispatched every target",
            synthetic_input.get("dispatched"),
            len(synthetic_targets),
            source="native synthetic hit-test profiler",
        )
        validation.equal(
            "synthetic hover resolved every target",
            synthetic_input.get("resolved"),
            len(synthetic_targets),
            source="native synthetic hit-test profiler",
        )
        validation.equal(
            "synthetic hover has no missing targets",
            synthetic_input.get("missing"),
            0,
            source="native synthetic hit-test profiler",
        )
        validation.equal(
            "synthetic hover has no mismatched targets",
            synthetic_input.get("mismatched"),
            0,
            source="native synthetic hit-test profiler",
        )
    text_invalidation = framework.get("live_text_invalidation") or {}
    text_invalidation_reasons = text_invalidation.get("reasons") or {}
    validation.require(
        "live text invalidation diagnostics present",
        text_invalidation,
        lambda value: all(key in value for key in ("candidates", "text_only", "layout", "reasons")),
        "candidate, text-only, layout, and reason counters",
        source="native text invalidation diagnostics",
    )
    validation.equal(
        "live text invalidation decision accounting",
        int(text_invalidation.get("text_only") or 0)
        + int(text_invalidation.get("layout") or 0),
        text_invalidation.get("candidates"),
        source="native text invalidation diagnostics",
    )
    validation.equal(
        "live text invalidation reason accounting",
        sum(int(value or 0) for value in text_invalidation_reasons.values()),
        text_invalidation.get("candidates"),
        source="native text invalidation diagnostics",
    )
    if args.scenario == "labels-fixed":
        validation.require(
            "fixed labels exercise text-only invalidation",
            text_invalidation,
            lambda value: int(value.get("text_only") or 0) > int(value.get("layout") or 0),
            "more text-only than layout decisions",
            source="native text invalidation diagnostics",
        )
    elif args.scenario == "intrinsic-text":
        validation.equal(
            "intrinsic labels reject text-only invalidation",
            text_invalidation.get("text_only"),
            0,
            source="native text invalidation diagnostics",
        )
    elif args.scenario == "composite-text-fixed":
        validation.require(
            "fixed composites exercise text-only invalidation",
            text_invalidation_reasons.get("fixed_composite"),
            lambda value: isinstance(value, int) and value > 0,
            "at least one fixed-composite decision",
            source="native text invalidation diagnostics",
        )
    elif args.scenario == "composite-text-intrinsic":
        validation.equal(
            "relative composites reject text-only invalidation",
            text_invalidation.get("text_only"),
            0,
            source="native text invalidation diagnostics",
        )
        validation.require(
            "relative composites report intrinsic height",
            text_invalidation_reasons.get("intrinsic_height"),
            lambda value: isinstance(value, int) and value > 0,
            "at least one intrinsic-height decision",
            source="native text invalidation diagnostics",
        )
        validation.require(
            "intrinsic labels exercise layout invalidation",
            text_invalidation.get("layout"),
            lambda value: isinstance(value, int) and value > 0,
            "at least one layout decision",
            source="native text invalidation diagnostics",
        )
    if args.capture_screenshot:
        validation.require(
            "window screenshot captured",
            screenshot_capture,
            lambda value: isinstance(value, dict)
            and isinstance(value.get("width"), int)
            and value["width"] > 0
            and isinstance(value.get("height"), int)
            and value["height"] > 0
            and value.get("rgba_bytes") == value["width"] * value["height"] * 4
            and isinstance(value.get("sha256"), str)
            and len(value["sha256"]) == 64,
            "non-empty RGBA screenshot with consistent dimensions and SHA-256",
            source="native whole-window screenshot",
        )

    if labels:
        if args.scenario == "labels-fixed":
            expected_first = _fixed_text(0, final_tick)
            expected_last = _fixed_text(len(labels) - 1, final_tick)
        elif args.scenario == "intrinsic-text":
            expected_first = _intrinsic_text(0, final_tick)
            expected_last = _intrinsic_text(len(labels) - 1, final_tick)
        elif args.scenario == "same-property-burst":
            expected_first = f"burst {final_tick:06d}-{args.burst_repeats - 1:06d}"
            expected_last = expected_first
        else:
            expected_first = f"burst {0:04d}: {final_tick:06d}-B"
            expected_last = f"burst {len(labels) - 1:04d}: {final_tick:06d}-B"
        if args.scenario == "mixed-state":
            expected_first = _fixed_text(0, final_tick)
            expected_last = _fixed_text(len(labels) - 1, final_tick)
        validation.equal(
            "first Python label reached final state",
            labels[0].text,
            expected_first,
            source="Python widget state",
        )
        validation.equal(
            "last Python label reached final state",
            labels[-1].text,
            expected_last,
            source="Python widget state",
        )
        first_native = find_tree_node(gpu.get("tree"), labels[0].id)
        last_native = find_tree_node(gpu.get("tree"), labels[-1].id)
        validation.equal(
            "first native label reached final state",
            (first_native or {}).get("props", {}).get("text"),
            expected_first,
            source="native retained tree",
        )
        validation.equal(
            "last native label reached final state",
            (last_native or {}).get("props", {}).get("text"),
            expected_last,
            source="native retained tree",
        )

    if args.scenario == "intrinsic-text" and len(labels) >= 2:
        diagnostics = layout.get("diagnostics") or {}
        first_geometry = (diagnostics.get(labels[0].id) or {}).get("dynamic_geometry") or {}
        first_rect = (diagnostics.get(labels[0].id) or {}).get("resolved") or {}
        second_rect = (diagnostics.get(labels[1].id) or {}).get("resolved") or {}
        measured_height = first_geometry.get("measured_content_height")
        resolved_height = first_rect.get("height")
        validation.require(
            "wrapped label reserves its measured content height",
            {"measured": measured_height, "resolved": resolved_height},
            lambda value: isinstance(value["measured"], (int, float))
            and isinstance(value["resolved"], (int, float))
            and value["resolved"] + 0.5 >= value["measured"],
            "resolved height >= measured wrapped content height",
            source="native dynamic geometry diagnostics",
        )
        validation.require(
            "adjacent intrinsic-text rows do not overlap",
            {"first": first_rect, "second": second_rect},
            lambda value: all(
                isinstance(item, (int, float))
                for item in (
                    value["first"].get("y"),
                    value["first"].get("height"),
                    value["second"].get("y"),
                )
            )
            and value["second"]["y"] + 0.5
            >= value["first"]["y"] + value["first"]["height"],
            "second row y >= first row bottom",
            source="native layout diagnostics",
        )

    if composite_targets:
        tree = gpu.get("tree")
        diagnostics = layout.get("diagnostics") or {}
        for widget, attribute, prop, family, index in composite_targets:
            expected = _composite_text(family, index, final_tick)
            validation.equal(
                f"final Python {family} {index}",
                getattr(widget, attribute),
                expected,
                source="Python composite state",
            )
            node = find_tree_node(tree, widget.id)
            validation.equal(
                f"final native {family} {index}",
                (node or {}).get("props", {}).get("text"),
                expected,
                source="native retained composite tree",
            )
            diagnostic = diagnostics.get(widget.id) or {}
            available = diagnostic.get("available") or {}
            if isinstance(available.get("height"), (int, float)) and available["height"] > 0.5:
                overflow = diagnostic.get("overflow") or {}
                validation.require(
                    f"visible {family} {index} has no horizontal container clipping",
                    overflow,
                    lambda value: isinstance(value.get("left"), (int, float))
                    and isinstance(value.get("right"), (int, float))
                    and value["left"] <= 0.5
                    and value["right"] <= 0.5,
                    "left/right overflow <= 0.5 physical pixels",
                    source="native composite clip diagnostics",
                )
        row_rects = [
            (diagnostics.get(row.id) or {}).get("resolved") or {}
            for row in composite_rows
        ]
        for index, (first, second) in enumerate(zip(row_rects, row_rects[1:])):
            validation.require(
                f"composite rows {index}/{index + 1} do not overlap",
                {"first": first, "second": second},
                lambda value: all(
                    isinstance(item, (int, float))
                    for item in (
                        value["first"].get("y"),
                        value["first"].get("height"),
                        value["second"].get("y"),
                    )
                )
                and value["second"]["y"] + 0.5
                >= value["first"]["y"] + value["first"]["height"],
                "next row y >= prior row bottom",
                source="native composite layout diagnostics",
            )
        scroll_y = (layout.get("scroll_y") or {}).get(scroller.id)
        scroll_max_y = (layout.get("scroll_max_y") or {}).get(scroller.id)
        validation.require(
            "composite viewport has vertical overflow",
            scroll_max_y,
            lambda value: isinstance(value, (int, float)) and value > 0,
            "positive vertical scroll maximum",
            source="native composite scroll diagnostics",
        )
        validation.require(
            "composite viewport clamps at bottom",
            {"scroll_y": scroll_y, "scroll_max_y": scroll_max_y},
            lambda value: isinstance(value["scroll_y"], (int, float))
            and isinstance(value["scroll_max_y"], (int, float))
            and abs(value["scroll_y"] - value["scroll_max_y"]) <= 0.5,
            "scroll_y equals scroll_max_y within 0.5 logical pixels",
            source="native composite scroll diagnostics",
        )

    if args.scenario == "mixed-state":
        validation.equal(
            "first badge reached final state",
            badges[0].text,
            f"{final_tick % 1_000_000:06d}",
            source="Python widget state",
        )
        validation.equal(
            "last badge reached final state",
            badges[-1].text,
            f"{(final_tick + len(badges) - 1) % 1_000_000:06d}",
            source="Python widget state",
        )
        validation.equal(
            "first LED reached final state",
            leds[0].state,
            "on" if final_tick % 2 == 0 else "off",
            source="Python widget state",
        )
        validation.equal(
            "last progress reached final state",
            progress_bars[-1].value,
            ((final_tick + len(progress_bars) - 1) % 101) / 100.0,
            source="Python widget state",
        )

    replacements = int(command_queue.get("replacements") or 0)
    batches = native_sends.get("batches") or {}
    if args.update_mode == "individual" and args.scenario in {
        "same-property-burst",
        "distinct-property-burst",
    }:
        expected_minimum_replacements = completed * (
            args.burst_repeats - 1
            if args.scenario == "same-property-burst"
            else len(labels)
        )
        validation.require(
            "burst exercised native queue replacement",
            replacements,
            lambda value: value >= expected_minimum_replacements,
            f"at least {expected_minimum_replacements} replacements",
            source="native command queue diagnostics",
        )
    if args.update_mode == "batch":
        validation.require(
            "batch packets were submitted",
            batches.get("packets"),
            lambda value: isinstance(value, int) and value > 0,
            "at least one typed update packet",
            source="Python/native boundary diagnostics",
        )
        style_sends = int((method_stats.get("enqueue_set_style") or {}).get("requested") or 0)
        expected_packets = completed + (style_sends if args.scenario == "ordered-barrier" else 0)
        validation.equal(
            "expected batch packets per completed callback",
            batches.get("packets"),
            expected_packets,
            source="Python/native boundary diagnostics",
        )
        if args.scenario == "ordered-barrier":
            validation.equal(
                "each emitted style command flushed a property ordering barrier",
                batches.get("barrier_flushes"),
                style_sends,
                source="Python update batch diagnostics",
            )
            validation.require(
                "ordered-barrier case emitted style commands",
                style_sends,
                lambda value: value > 0,
                "at least one style ordering barrier",
                source="Python/native boundary diagnostics",
            )
        validation.equal(
            "batch sender method accounting",
            (method_stats.get("enqueue_set_props") or {}).get("requested"),
            batches.get("packets"),
            source="Python/native boundary diagnostics",
        )
        if args.scenario in {"same-property-burst", "distinct-property-burst"}:
            expected_duplicates = completed * (
                args.burst_repeats - 1
                if args.scenario == "same-property-burst"
                else len(labels)
            )
            validation.equal(
                "batch removed expected duplicate properties",
                batches.get("duplicates_removed"),
                expected_duplicates,
                source="Python update batch diagnostics",
            )

    wall_s = max(1e-9, measurement_wall_end - measurement_wall_start)
    cpu_s = max(0.0, measurement_cpu_end - measurement_cpu_start)
    report = {
        "schema": 1,
        "benchmark": "dragongui_update_pipeline",
        "framework": "DragonGUI",
        "framework_version": getattr(dg, "__version__", "unknown"),
        "python": platform.python_version(),
        "platform": platform.platform(),
        "scenario": args.scenario,
        "config": {
            "widgets": args.widgets,
            "burst_repeats": args.burst_repeats,
            "target_hz": args.target_hz,
            "warmup_seconds": args.warmup_seconds,
            "measure_seconds": args.measure_seconds,
            "update_mode": args.update_mode,
            "capture_screenshot": args.capture_screenshot,
            "interaction_probe_ms": args.interaction_probe_ms,
        },
        "build_ms": build_ms,
        "run_wall_ms": run_wall_ms,
        "recovery_ms": recovery_ms,
        "metrics": {
            "total_ticks": total_ticks,
            "completed_ticks": completed,
            "dropped_or_coalesced_ticks": total_ticks - completed,
            "measurement_completed_ticks": measurement_completed,
            "measurement_completed_at_window_end": measurement_completed_at_window_end,
            "measurement_dropped_or_coalesced_ticks": measure_ticks - measurement_completed,
            "measurement_wall_s": wall_s,
            "tick_throughput_hz": measurement_completed_at_window_end / wall_s,
            "process_cpu_percent_one_core": cpu_s / wall_s * 100.0,
            "callback_ms": _timings(callback_ms),
            "schedule_lag_ms": _timings(schedule_lag_ms),
            "interaction_roundtrip_ms": _timings(interaction_ms),
            "applied_first_tick": applied_ticks[0] if applied_ticks else None,
            "applied_last_tick": applied_ticks[-1] if applied_ticks else None,
        },
        "native": {
            "runtime": {
                "command_queue_depth": runtime.get("command_queue_depth"),
                "command_queue": command_queue,
                "command_drain": runtime.get("command_drain"),
                "command_drain_yields": runtime.get("command_drain_yields"),
                "command_timings": runtime.get("command_timings"),
                "command_dirty_counts": runtime.get("command_dirty_counts"),
                "python": python_runtime,
            },
            "framework": {
                "dirty_rebuilds": framework.get("dirty_rebuilds"),
                "command_text_rebuilds": framework.get("command_text_rebuilds"),
                "live_text_invalidation": framework.get("live_text_invalidation"),
                "partial_text_rebuild": framework.get("partial_text_rebuild"),
                "partial_text_rebuilds": framework.get("partial_text_rebuilds"),
                "style_reapply": framework.get("style_reapply"),
                "layout_compute": framework.get("layout_compute"),
                "apply_layout": framework.get("apply_layout"),
            },
        },
        "equivalence": equivalence,
        "screenshot": screenshot_capture,
        "validation": validation.report(),
        "ready_snapshot_present": bool(ready_snapshot),
    }

    payload = json.dumps(report, indent=2, sort_keys=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
    if args.quiet:
        print(
            f"{args.scenario}: validation={report['validation']['passed']} "
            f"throughput={report['metrics']['tick_throughput_hz']:.2f} Hz"
        )
    else:
        print(payload)
    return 0 if report["validation"]["passed"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
