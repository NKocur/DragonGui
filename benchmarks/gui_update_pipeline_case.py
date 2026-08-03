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
import random
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
    "state-controls",
    "plot-chrome",
    "html-fallback",
    "html-webview",
    "semantic-icons",
    "mixed-state",
    "same-property-burst",
    "distinct-property-burst",
    "ordered-barrier",
)

STATE_DROPDOWN_ITEMS = (
    "",
    "Café e\u0301 / 東京",
    "مرحبا بالعالم",
    "A deliberately long dropdown value used to verify established chrome clipping",
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


def _state_control_text(family: str, tick: int) -> str:
    variant = tick % 5
    if variant == 0:
        return ""
    if variant == 1:
        return f"{family} {tick}: Café e\u0301 / 東京 / 🙂"
    if variant == 2:
        return f"{family} {tick}: مرحبا بالعالم"
    if variant == 3:
        return f"{family} {tick}: first line\nsecond combining line e\u0301\nسطر ثالث"
    return (
        f"{family} {tick}: a deliberately long value that exercises clipping, wrapping, "
        "and internal text scrolling without changing the authored outer control geometry\n"
        "secondary line with Unicode 東京 and RTL مرحبا\nthird line\nfourth line\nfifth line"
    )


def _plot_axis_text(family: str, axis: str, tick: int) -> str:
    variants = (
        "Café / e\u0301",
        "東京 / 温度",
        "مرحبا / القيمة",
        "long established plot chrome label",
    )
    return f"{family} {axis} {tick}: {variants[tick % len(variants)]}"


def _html_document(tick: int) -> str:
    return (
        "<!doctype html><meta charset='utf-8'>"
        f"<title>DragonGUI report {tick}</title>"
        f"<main data-generation='{tick}'><h1>Report {tick}: Café / 東京</h1>"
        f"<p>{'λ' * (3 + tick % 11)}</p></main>"
    )


def _html_fallback_text(tick: int) -> str:
    return (
        f"HTML fallback generation {tick}\n"
        f"Café e\u0301 / 東京 / مرحبا / {'x' * (8 + tick % 17)}"
    )


def _fnv1a64(text: str) -> str:
    value = 0xCBF29CE484222325
    for byte in text.encode("utf-8"):
        value ^= byte
        value = value * 0x100000001B3 & 0xFFFFFFFFFFFFFFFF
    return f"{value:016x}"


def _text_state_sha256(values: list[str | None]) -> str:
    payload = json.dumps(values, ensure_ascii=False, separators=(",", ":"))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def _locality_indices(widget_count: int, root_count: int, tick: int) -> list[int]:
    if widget_count <= 0 or root_count <= 0:
        return []
    return random.Random(0xD4A60F00 + tick).sample(
        range(widget_count), min(widget_count, root_count)
    )


def _set_live_text(widget: Any, attribute: str, prop: str, value: str) -> None:
    setattr(widget, attribute, value)
    if (handle := widget._live()) is not None:
        handle.enqueue_set_prop(prop, value)


def _set_live_number_prop(widget: Any, prop: str, value: float) -> None:
    if (handle := widget._live()) is not None:
        handle.enqueue_set_prop(prop, float(value))


def _set_live_bool_prop(widget: Any, attribute: str, prop: str, value: bool) -> None:
    setattr(widget, attribute, bool(value))
    if (handle := widget._live()) is not None:
        handle.enqueue_set_prop(prop, bool(value))


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
    state = gpu.get("state") or {}
    renderer = gpu.get("renderer") or {}
    state_maps = {
        name: {
            widget_id: (state.get(name) or {}).get(widget_id)
            for widget_id in widget_ids
            if widget_id in (state.get(name) or {})
        }
        for name in (
            "text_val",
            "text_cursor",
            "text_scroll_x",
            "text_scroll_y",
            "float_val",
            "dropdown_index",
        )
    }
    caret_positions = {
        widget_id: (renderer.get("caret_positions") or {}).get(widget_id)
        for widget_id in widget_ids
        if widget_id in (renderer.get("caret_positions") or {})
    }
    text_owner_geometry = {
        widget_id: (renderer.get("text_owner_geometry") or {}).get(widget_id)
        for widget_id in widget_ids
        if widget_id in (renderer.get("text_owner_geometry") or {})
    }
    plot_geometry = {
        widget_id: (renderer.get("plot_geometry") or {}).get(widget_id)
        for widget_id in widget_ids
        if widget_id in (renderer.get("plot_geometry") or {})
    }
    computed_styles = gpu.get("computed_styles") or {}
    icon_identity = {
        widget_id: (computed_styles.get(widget_id) or {}).get("icon")
        for widget_id in widget_ids
        if (computed_styles.get(widget_id) or {}).get("icon") is not None
    }
    html_report_snapshot = renderer.get("html_reports") or {}
    html_instances = html_report_snapshot.get("instances") or {}
    html_reports = {
        "platform": html_report_snapshot.get("platform"),
        "enabled": html_report_snapshot.get("enabled"),
        "reason": html_report_snapshot.get("reason"),
        "environment_ready": html_report_snapshot.get("environment_ready"),
        "instances": {
            widget_id: html_instances.get(widget_id)
            for widget_id in widget_ids
            if widget_id in html_instances
        },
    }
    canonical = json.dumps(
        {
            "widgets": widgets,
            "scroll": scroll,
            "state": state_maps,
            "focused": state.get("focused"),
            "caret_positions": caret_positions,
            "text_owner_geometry": text_owner_geometry,
            "plot_geometry": plot_geometry,
            "icon_identity": icon_identity,
            "html_reports": html_reports,
        },
        sort_keys=True,
        separators=(",", ":"),
    )
    return {
        "schema": 1,
        "sha256": hashlib.sha256(canonical.encode("utf-8")).hexdigest(),
        "widgets": widgets,
        "scroll": scroll,
        "state": state_maps,
        "focused": state.get("focused"),
        "caret_positions": caret_positions,
        "text_owner_geometry": text_owner_geometry,
        "plot_geometry": plot_geometry,
        "icon_identity": icon_identity,
        "html_reports": html_reports,
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
    parser.add_argument(
        "--text-invalidation-mode",
        choices=("optimized", "forced-layout"),
        default="optimized",
        help="Diagnostic retained-text invalidation mode configured once for this process.",
    )
    parser.add_argument(
        "--targeted-rebuild-verification",
        choices=("off", "verify-full"),
        default="off",
        help="Optionally compare each successful targeted rebuild with a full reconstruction.",
    )
    parser.add_argument(
        "--typed-target-diagnostics",
        choices=("required", "optional"),
        default="required",
        help="Allow pre-Phase-4d controls that do not expose typed target counters.",
    )
    parser.add_argument("--capture-screenshot", action="store_true")
    parser.add_argument(
        "--screenshot-rgba-output",
        type=Path,
        help="Optional raw RGBA dump for visual diagnosis; requires --capture-screenshot.",
    )
    parser.add_argument("--interaction-probe-ms", type=float, default=0.0)
    parser.add_argument(
        "--locality-roots",
        type=int,
        default=0,
        help="For labels-fixed, update this many deterministic random labels per callback.",
    )
    parser.add_argument("--output", type=Path)
    parser.add_argument("--quiet", action="store_true")
    args = parser.parse_args()
    args.widgets = max(1, args.widgets)
    args.burst_repeats = max(2, args.burst_repeats)
    args.warmup_seconds = max(0.1, args.warmup_seconds)
    args.measure_seconds = max(0.2, args.measure_seconds)
    args.target_hz = max(1.0, args.target_hz)
    args.interaction_probe_ms = max(0.0, args.interaction_probe_ms)
    args.locality_roots = max(0, min(args.widgets, args.locality_roots))
    if args.locality_roots and args.scenario != "labels-fixed":
        parser.error("--locality-roots is supported only with --scenario labels-fixed")

    os.environ["DRAGONGUI_DIAGNOSTIC_TEXT_INVALIDATION_MODE"] = (
        args.text_invalidation_mode
    )
    os.environ["DRAGONGUI_DIAGNOSTIC_TARGETED_REBUILD_MODE"] = (
        args.targeted_rebuild_verification
    )
    if args.scenario == "state-controls":
        state_control_ids = (
            "pipeline-text-input",
            "pipeline-text-area",
            "pipeline-code-editor",
            "pipeline-log-follow",
            "pipeline-log-static",
            "pipeline-dropdown",
            "pipeline-number-input",
            "pipeline-drag-number",
        )
        os.environ["DRAGONGUI_SYNTHETIC_FOCUS_ID"] = "pipeline-text-area"
        os.environ["DRAGONGUI_DIAGNOSTIC_TEXT_GEOMETRY_IDS"] = ",".join(
            state_control_ids
        )
        os.environ["DRAGONGUI_SYNTHETIC_HOVER_IDS"] = ",".join(
            state_control_ids[:3]
            + (
                "pipeline-dropdown",
                "pipeline-number-input",
                "pipeline-drag-number",
            )
        )
    elif args.scenario == "plot-chrome":
        plot_ids = (
            "pipeline-line-plot",
            "pipeline-histogram",
            "pipeline-bar-chart",
        )
        os.environ["DRAGONGUI_DIAGNOSTIC_TEXT_GEOMETRY_IDS"] = ",".join(plot_ids)
        os.environ["DRAGONGUI_SYNTHETIC_HOVER_IDS"] = ",".join(plot_ids)
    elif args.scenario in ("html-fallback", "html-webview"):
        html_ids = ("pipeline-html-fallback", "pipeline-html-source")
        os.environ["DRAGONGUI_HTMLREPORT_WEBVIEW2"] = (
            "0" if args.scenario == "html-fallback" else "1"
        )
        os.environ["DRAGONGUI_DIAGNOSTIC_TEXT_GEOMETRY_IDS"] = ",".join(html_ids)
    elif args.scenario == "semantic-icons":
        icon_ids = tuple(f"pipeline-icon-{index}" for index in range(4))
        os.environ["DRAGONGUI_SYNTHETIC_HOVER_IDS"] = ",".join(icon_ids)

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
    state_controls: dict[str, Any] = {}
    plot_controls: dict[str, Any] = {}
    html_controls: dict[str, Any] = {}
    icon_controls: list[Any] = []
    icon_theme_resources: tuple[Any, Any] | None = None
    if args.scenario == "semantic-icons":
        icon_theme_resources = (
            dg.IconResource(
                (
                    dg.IconStroke(((3, 3), (21, 21))),
                    dg.IconStroke(((21, 3), (3, 21))),
                ),
                view_box=(0, 0, 24, 24),
                stroke_width=2,
            ),
            dg.IconResource(
                (
                    dg.IconStroke(((3, 12), (21, 12))),
                    dg.IconStroke(((12, 3), (12, 21))),
                ),
                view_box=(0, 0, 24, 24),
                stroke_width=2.5,
            ),
        )

    build_t0 = time.perf_counter()
    with dg.VLayout(style={"height": "100%", "padding": 8, "gap": 6}):
        status_style = {"height": 24}
        if args.scenario in {"labels-fixed", "mixed-state"}:
            status_style["width"] = 240
        status = dg.Label("tick -1", id="pipeline-status", style=status_style)
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
                        badges.append(
                            dg.Badge(
                                "000000",
                                id=f"pipeline-badge-{index}",
                                style={"width": 72, "height": 22},
                            )
                        )
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
            elif args.scenario == "state-controls":
                state_controls = {
                    "text_input": dg.TextInput(
                        _state_control_text("input", 0),
                        id="pipeline-text-input",
                        style={"width": 720, "height": 36},
                    ),
                    "text_area": dg.TextArea(
                        _state_control_text("area", 0),
                        id="pipeline-text-area",
                        rows=3,
                        wrap=True,
                        style={"width": 720, "height": 84},
                    ),
                    "code_editor": dg.CodeEditor(
                        _state_control_text("code", 0),
                        id="pipeline-code-editor",
                        rows=3,
                        wrap=False,
                        style={"width": 720, "height": 92},
                    ),
                    "log_follow": dg.LogView(
                        _state_control_text("follow", 0),
                        id="pipeline-log-follow",
                        follow=True,
                        rows=3,
                        wrap=True,
                        style={"width": 720, "height": 76},
                    ),
                    "log_static": dg.LogView(
                        _state_control_text("static", 0),
                        id="pipeline-log-static",
                        follow=False,
                        rows=3,
                        wrap=False,
                        style={"width": 720, "height": 76},
                    ),
                    "dropdown": dg.Dropdown(
                        STATE_DROPDOWN_ITEMS,
                        value=STATE_DROPDOWN_ITEMS[0],
                        id="pipeline-dropdown",
                        style={"width": 360, "height": 36},
                    ),
                    "number_input": dg.NumberInput(
                        0,
                        min=-1_000_000,
                        max=1_000_000,
                        step=0.25,
                        id="pipeline-number-input",
                        style={"width": 240, "height": 36},
                    ),
                    "drag_number": dg.DragNumber(
                        0,
                        min=-1_000_000,
                        max=1_000_000,
                        step=0.25,
                        id="pipeline-drag-number",
                        style={"width": 240, "height": 36},
                    ),
                }
            elif args.scenario == "plot-chrome":
                plot_controls = {
                    "line": dg.LinePlot(
                        {
                            "x": list(range(24)),
                            "signal": [((index % 7) - 3) * 1.25 for index in range(24)],
                            "trend": [index * 0.35 - 4.0 for index in range(24)],
                        },
                        x="x",
                        y=("signal", "trend"),
                        labels=("signal", "trend"),
                        x_label="line x 0",
                        y_label="line y 0",
                        show_legend=True,
                        show_toolbar=False,
                        id="pipeline-line-plot",
                        style={"width": 900, "height": 210},
                    ),
                    "histogram": dg.Histogram(
                        [((index * 17) % 31) - 15 for index in range(120)],
                        bins=12,
                        x_label="histogram x 0",
                        y_label="histogram y 0",
                        show_toolbar=False,
                        id="pipeline-histogram",
                        style={"width": 900, "height": 210},
                    ),
                    "bar": dg.BarChart(
                        labels=("North", "South", "East", "West"),
                        values=((4, 7, 3, 8), (6, 2, 9, 5)),
                        series=("current", "prior"),
                        x_label="bar x 0",
                        y_label="bar y 0",
                        show_toolbar=False,
                        id="pipeline-bar-chart",
                        style={"width": 900, "height": 210},
                    ),
                }
            elif args.scenario in ("html-fallback", "html-webview"):
                html_controls = {
                    "fallback": dg.HtmlReport.from_html(
                        _html_document(0),
                        base_dir=ROOT,
                        allow_remote=False,
                        allow_scripts=False,
                        external_fallback=True,
                        id="pipeline-html-fallback",
                        style={"width": 900, "height": 290},
                    ),
                    "source": dg.HtmlReport(
                        ROOT / "benchmarks" / "gui_update_pipeline_case.py",
                        allow_remote=False,
                        allow_scripts=True,
                        external_fallback=False,
                        id="pipeline-html-source",
                        style={"width": 900, "height": 290},
                    ),
                }
                html_controls["fallback"].diagnostic_text = _html_fallback_text(0)
            elif args.scenario == "semantic-icons":
                with dg.HLayout(
                    style={"width": 900, "height": 72, "gap": 12, "align-items": "center"}
                ):
                    for index, icon in enumerate(
                        ("search", "folder-open", "settings", "not-registered")
                    ):
                        icon_controls.append(
                            dg.IconButton(
                                icon,
                                on_click=lambda: None,
                                size=48,
                                id=f"pipeline-icon-{index}",
                                style={"width": 48, "height": 48},
                            )
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
    html_reload_attempts = 0
    producer_error: str | None = None
    producer_done = threading.Event()
    state_lock = threading.Lock()
    screenshot_capture: dict[str, Any] | None = None
    interaction_ms: list[float] = []
    interaction_error: str | None = None
    interaction_ready = threading.Event()
    interaction_stop = threading.Event()
    label_expected_ticks = [0] * len(labels)
    locality_touched_labels: set[int] = set()

    def apply_tick(tick: int) -> None:
        nonlocal completed, measurement_completed
        callback_t0 = time.perf_counter()
        update_context = app.update_batch() if args.update_mode == "batch" else nullcontext()
        with update_context:
            if args.scenario == "labels-fixed":
                if args.locality_roots:
                    selected = _locality_indices(
                        len(labels), args.locality_roots, tick
                    )
                else:
                    selected = range(len(labels))
                for index in selected:
                    labels[index].set_value(_fixed_text(index, tick))
                    label_expected_ticks[index] = tick
                    locality_touched_labels.add(index)
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
            elif args.scenario == "state-controls":
                state_controls["text_input"].set_value(
                    _state_control_text("input", tick).replace("\n", " / ")
                )
                state_controls["text_area"].set_value(_state_control_text("area", tick))
                state_controls["code_editor"].set_value(_state_control_text("code", tick))
                state_controls["log_follow"].set_lines(
                    _state_control_text("follow", tick)
                )
                state_controls["log_static"].set_lines(
                    _state_control_text("static", tick)
                )
                state_controls["dropdown"].set_value(
                    STATE_DROPDOWN_ITEMS[tick % len(STATE_DROPDOWN_ITEMS)]
                )
                state_controls["number_input"].set_value(tick * 1.25 - 500.5)
                state_controls["drag_number"].set_value(500.5 - tick * 0.75)
            elif args.scenario == "plot-chrome":
                show_grid = tick % 2 == 0
                show_axes = tick % 3 != 0
                show_ticks = tick % 4 != 0
                show_toolbar = tick % 5 in (1, 2)
                tick_count = 2 + tick % 8
                for family, plot in plot_controls.items():
                    plot.set_grid_visible(show_grid)
                    plot.set_axes_visible(show_axes)
                    plot.set_ticks_visible(show_ticks)
                    plot.set_toolbar_visible(show_toolbar)
                    plot.set_tick_count(tick_count)
                    if family == "bar":
                        _set_live_text(
                            plot,
                            "x_label",
                            "x_label",
                            _plot_axis_text(family, "x", tick),
                        )
                        _set_live_text(
                            plot,
                            "y_label",
                            "y_label",
                            _plot_axis_text(family, "y", tick),
                        )
                    else:
                        plot.set_axis_labels(
                            x=_plot_axis_text(family, "x", tick),
                            y=_plot_axis_text(family, "y", tick),
                        )
                line_plot = plot_controls["line"]
                line_plot.set_legend_visible(
                    tick % 2 == 1,
                    position=(
                        "top-right",
                        "top-left",
                        "bottom-right",
                        "bottom-left",
                    )[tick % 4],
                )
                line_plot.set_window_size(6.0 + tick % 5)
                bounds_phase = tick % 8
                for prop, value in (
                    ("x_min", bounds_phase),
                    ("x_max", bounds_phase + 12),
                    ("y_min", -10 - tick % 3),
                    ("y_max", 12 + tick % 4),
                ):
                    _set_live_number_prop(line_plot, prop, value)
            elif args.scenario in ("html-fallback", "html-webview"):
                fallback_report = html_controls["fallback"]
                source_report = html_controls["source"]
                _set_live_text(
                    fallback_report,
                    "diagnostic_text",
                    "text",
                    _html_fallback_text(tick),
                )
                if tick == warmup_ticks:
                    source_report.set_html(_html_document(tick), base_dir=ROOT)
                elif tick == total_ticks - 1:
                    source_report.set_path(
                        ROOT / "benchmarks" / "gui_update_pipeline_case.py"
                    )
                for report_index, report in enumerate((fallback_report, source_report)):
                    _set_live_bool_prop(
                        report,
                        "allow_remote",
                        "allow_remote",
                        (tick + report_index) % 2 == 0,
                    )
                    _set_live_bool_prop(
                        report,
                        "allow_scripts",
                        "allow_scripts",
                        (tick + report_index) % 3 != 0,
                    )
                    _set_live_bool_prop(
                        report,
                        "external_fallback",
                        "external_fallback",
                        (tick + report_index) % 4 != 0,
                    )
            elif args.scenario == "semantic-icons":
                assert icon_theme_resources is not None
                if tick % 2 == 0:
                    app.set_icon_theme(
                        {
                            "search": icon_theme_resources[0],
                            "folder-open": "warning",
                        }
                    )
                else:
                    app.set_icon_theme(
                        {
                            "search": "help",
                            "folder-open": icon_theme_resources[1],
                        }
                    )
                icon_names = ("search", "folder-open", "settings", "not-registered")
                for index, icon in enumerate(icon_controls):
                    icon.set_icon(icon_names[(tick + index) % len(icon_names)])
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
        nonlocal ready_snapshot, recovery_ms, producer_error, html_reload_attempts
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
            if args.scenario == "html-webview" and handle is not None:
                expected_source = "url"
                for _ in range(5):
                    snapshot = app.debug_snapshot(timeout_ms=3000)
                    html_snapshot = (
                        ((snapshot.get("gpu") or {}).get("renderer") or {}).get(
                            "html_reports"
                        )
                        or {}
                    )
                    instances = html_snapshot.get("instances") or {}
                    fallback_instance = instances.get(html_controls["fallback"].id) or {}
                    source_instance = instances.get(html_controls["source"].id) or {}
                    if (
                        fallback_instance.get("visible") is True
                        and fallback_instance.get("source") == "html"
                        and source_instance.get("visible") is True
                        and source_instance.get("source") == expected_source
                    ):
                        break
                    html_reload_attempts += 1
                    app.call_soon_threadsafe(
                        lambda: [report.reload() for report in html_controls.values()]
                    )
                    retry_deadline = min(recovery_deadline, time.perf_counter() + 2.0)
                    while time.perf_counter() < retry_deadline:
                        if not handle._python_debug_snapshot().get("queued_tasks"):
                            retry_snapshot = app.debug_snapshot(timeout_ms=3000)
                            if not (retry_snapshot.get("runtime") or {}).get(
                                "command_queue_depth"
                            ):
                                break
                        time.sleep(0.01)
                    time.sleep(0.08)
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
    if state_controls:
        probe_ids.extend(control.id for control in state_controls.values())
    if plot_controls:
        probe_ids.extend(plot.id for plot in plot_controls.values())
    if html_controls:
        probe_ids.extend(report.id for report in html_controls.values())
    if icon_controls:
        probe_ids.extend(icon.id for icon in icon_controls)
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
    synthetic_focus = os.environ.get("DRAGONGUI_SYNTHETIC_FOCUS_ID", "").strip()
    if synthetic_focus:
        focus_probe = (runtime.get("synthetic_input") or {}).get("focus") or {}
        validation.equal(
            "synthetic focus requested expected target",
            focus_probe.get("requested_id"),
            synthetic_focus,
            source="native synthetic focus profiler",
        )
        validation.equal(
            "synthetic focus was applied",
            focus_probe.get("applied"),
            True,
            source="native synthetic focus profiler",
        )
        validation.equal(
            "synthetic focus resolved expected target",
            focus_probe.get("resolved_id"),
            synthetic_focus,
            source="native synthetic focus profiler",
        )
    command_text_rebuilds = framework.get("command_text_rebuilds") or {}
    target_classes = command_text_rebuilds.get("target_classes") or {}
    if args.typed_target_diagnostics == "required" or target_classes:
        validation.require(
            "typed deferred target diagnostics present",
            target_classes,
            lambda value: all(
                isinstance(value.get(key), int) and value[key] >= 0
                for key in (
                    "retained_visual",
                    "primitive_paint",
                    "table_text",
                    "overlay_text",
                )
            ),
            "non-negative retained-visual, primitive-paint, table-text, and overlay counters",
            source="native typed target diagnostics",
        )
    targeted_rebuild_verification = (
        framework.get("targeted_rebuild_verification") or {}
    )
    validation.require(
        "retained structure generation diagnostic present",
        framework.get("structure_generation"),
        lambda value: isinstance(value, int) and value >= 0,
        "non-negative retained-tree structure generation",
        source="native targeted rebuild diagnostics",
    )
    validation.equal(
        "targeted rebuild verification mode",
        targeted_rebuild_verification.get("mode"),
        args.targeted_rebuild_verification,
        source="native retained rebuild diagnostics",
    )
    if args.targeted_rebuild_verification == "verify-full":
        validation.require(
            "targeted rebuild verifier exercised",
            targeted_rebuild_verification.get("attempts"),
            lambda value: isinstance(value, int) and value > 0,
            "at least one targeted/full comparison",
            source="native retained rebuild diagnostics",
        )
        validation.equal(
            "targeted rebuild verifier found no mismatches",
            targeted_rebuild_verification.get("mismatches"),
            0,
            source="native retained rebuild diagnostics",
        )
        validation.equal(
            "all targeted rebuild verifier attempts matched",
            targeted_rebuild_verification.get("matches"),
            targeted_rebuild_verification.get("attempts"),
            source="native retained rebuild diagnostics",
        )
    text_invalidation = framework.get("live_text_invalidation") or {}
    text_invalidation_reasons = text_invalidation.get("reasons") or {}
    validation.require(
        "live text invalidation diagnostics present",
        text_invalidation,
        lambda value: all(
            key in value for key in ("mode", "candidates", "text_only", "layout", "reasons")
        ),
        "configured mode, candidate, text-only, layout, and reason counters",
        source="native text invalidation diagnostics",
    )
    validation.equal(
        "live text invalidation mode",
        text_invalidation.get("mode"),
        args.text_invalidation_mode,
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
        expected_reason = (
            "forced_safe_layout"
            if args.text_invalidation_mode == "forced-layout"
            else "fixed_single_line_label"
        )
        validation.require(
            "fixed labels exercise expected invalidation mode",
            text_invalidation_reasons.get(expected_reason),
            lambda value: isinstance(value, int) and value > 0,
            f"at least one {expected_reason.replace('_', '-')} decision",
            source="native text invalidation diagnostics",
        )
        if args.text_invalidation_mode == "forced-layout":
            validation.equal(
                "forced-safe labels reject text-only invalidation",
                text_invalidation.get("text_only"),
                0,
                source="native text invalidation diagnostics",
            )
        else:
            validation.equal(
                "fixed-label workload avoids layout invalidation",
                text_invalidation.get("layout"),
                0,
                source="native text invalidation diagnostics",
            )
            validation.equal(
                "all fixed-label candidates use text-only invalidation",
                text_invalidation.get("text_only"),
                text_invalidation.get("candidates"),
                source="native text invalidation diagnostics",
            )
    elif args.scenario in ("html-fallback", "html-webview", "semantic-icons"):
        optimized_reason = (
            "target_local_icon"
            if args.scenario == "semantic-icons"
            else "target_local_html_fallback"
        )
        expected_reason = (
            "forced_safe_layout"
            if args.text_invalidation_mode == "forced-layout"
            else optimized_reason
        )
        validation.require(
            f"{args.scenario} exercises expected invalidation mode",
            text_invalidation_reasons.get(expected_reason),
            lambda value: isinstance(value, int) and value > 0,
            f"at least one {expected_reason.replace('_', '-')} decision",
            source="native text invalidation diagnostics",
        )
        if args.text_invalidation_mode == "forced-layout":
            validation.equal(
                f"forced-safe {args.scenario} rejects text-only invalidation",
                text_invalidation.get("text_only"),
                0,
                source="native text invalidation diagnostics",
            )
            validation.require(
                "forced-safe labels report forced layout",
                text_invalidation_reasons.get("forced_safe_layout"),
                lambda value: isinstance(value, int) and value > 0,
                "at least one forced-safe layout decision",
                source="native text invalidation diagnostics",
            )
        else:
            validation.require(
                "fixed labels exercise text-only invalidation",
                text_invalidation,
                lambda value: int(value.get("text_only") or 0)
                > int(value.get("layout") or 0),
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
        expected_reason = (
            "forced_safe_layout"
            if args.text_invalidation_mode == "forced-layout"
            else "fixed_composite"
        )
        validation.require(
            "fixed composites exercise expected invalidation mode",
            text_invalidation_reasons.get(expected_reason),
            lambda value: isinstance(value, int) and value > 0,
            f"at least one {expected_reason.replace('_', '-')} decision",
            source="native text invalidation diagnostics",
        )
        if args.text_invalidation_mode == "forced-layout":
            validation.equal(
                "forced-safe composites reject text-only invalidation",
                text_invalidation.get("text_only"),
                0,
                source="native text invalidation diagnostics",
            )
    elif args.scenario == "mixed-state":
        if args.text_invalidation_mode == "forced-layout":
            validation.equal(
                "forced-safe mixed state rejects text-only invalidation",
                text_invalidation.get("text_only"),
                0,
                source="native text invalidation diagnostics",
            )
        else:
            validation.equal(
                "fixed mixed-state workload avoids layout invalidation",
                text_invalidation.get("layout"),
                0,
                source="native text invalidation diagnostics",
            )
            validation.equal(
                "all fixed mixed-state text candidates use text-only invalidation",
                text_invalidation.get("text_only"),
                text_invalidation.get("candidates"),
                source="native text invalidation diagnostics",
            )
            validation.require(
                "fixed mixed-state batches complete targeted rebuilds",
                command_text_rebuilds.get("completed_batches"),
                lambda value: isinstance(value, int) and value > 0,
                "at least one completed targeted rebuild",
                source="native targeted rebuild diagnostics",
            )
            validation.equal(
                "fixed mixed-state targeted rebuilds avoid fallback",
                command_text_rebuilds.get("fallback_batches"),
                0,
                source="native targeted rebuild diagnostics",
            )
            validation.equal(
                "fixed mixed-state has no stale-generation fallback",
                command_text_rebuilds.get("stale_generation_fallback_batches"),
                0,
                source="native targeted rebuild diagnostics",
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
    elif args.scenario == "state-controls":
        expected_reason = (
            "forced_safe_layout"
            if args.text_invalidation_mode == "forced-layout"
            else "target_local_state"
        )
        validation.require(
            "state controls exercise expected invalidation mode",
            text_invalidation_reasons.get(expected_reason),
            lambda value: isinstance(value, int) and value > 0,
            f"at least one {expected_reason.replace('_', '-')} decision",
            source="native text invalidation diagnostics",
        )
        if args.text_invalidation_mode == "forced-layout":
            validation.equal(
                "forced-safe state controls reject text-only invalidation",
                text_invalidation.get("text_only"),
                0,
                source="native text invalidation diagnostics",
            )
    elif args.scenario == "plot-chrome":
        expected_reason = (
            "forced_safe_layout"
            if args.text_invalidation_mode == "forced-layout"
            else "target_local_plot"
        )
        validation.require(
            "plot chrome exercises expected invalidation mode",
            text_invalidation_reasons.get(expected_reason),
            lambda value: isinstance(value, int) and value > 0,
            f"at least one {expected_reason.replace('_', '-')} decision",
            source="native text invalidation diagnostics",
        )
        if args.text_invalidation_mode == "forced-layout":
            validation.equal(
                "forced-safe plot chrome rejects text-only invalidation",
                text_invalidation.get("text_only"),
                0,
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
        if args.scenario == "labels-fixed" and args.locality_roots:
            expected_first = _fixed_text(0, label_expected_ticks[0])
            expected_last = _fixed_text(len(labels) - 1, label_expected_ticks[-1])
        elif args.scenario == "labels-fixed":
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

    if args.scenario == "labels-fixed" and args.locality_roots:
        expected_texts = [
            _fixed_text(index, label_expected_ticks[index])
            for index in range(len(labels))
        ]
        python_texts = [label.text for label in labels]
        native_texts = [
            ((find_tree_node(gpu.get("tree"), label.id) or {}).get("props") or {}).get(
                "text"
            )
            for label in labels
        ]
        validation.equal(
            "all locality Python labels match deterministic expected state",
            _text_state_sha256(python_texts),
            _text_state_sha256(expected_texts),
            source="Python locality state",
        )
        validation.equal(
            "all locality native labels match deterministic expected state",
            _text_state_sha256(native_texts),
            _text_state_sha256(expected_texts),
            source="native retained locality state",
        )
        validation.require(
            "locality schedule reaches multiple retained siblings",
            len(locality_touched_labels),
            lambda value: isinstance(value, int)
            and value >= min(len(labels), args.locality_roots * 4),
            "at least four batches worth of distinct label roots",
            source="deterministic locality schedule",
        )

        partial_text = framework.get("partial_text_rebuilds") or {}
        renderer = gpu.get("renderer") or {}
        retained_rebuilds = (
            ((renderer.get("primitives") or {}).get("retained_rebuilds") or {})
        )
        rebuilt_roots = int(command_text_rebuilds.get("rebuilt_roots") or 0)
        maximum_requested_roots = completed * (args.locality_roots + 1)
        validation.require(
            "locality batches complete targeted rebuilds",
            command_text_rebuilds.get("completed_batches"),
            lambda value: isinstance(value, int) and value > 0,
            "at least one completed targeted rebuild",
            source="native targeted rebuild diagnostics",
        )
        validation.equal(
            "locality targeted rebuilds avoid fallback",
            command_text_rebuilds.get("fallback_batches"),
            0,
            source="native targeted rebuild diagnostics",
        )
        validation.require(
            "locality rebuilt roots stay proportional to requested roots",
            rebuilt_roots,
            lambda value: isinstance(value, int)
            and value > 0
            and value <= maximum_requested_roots,
            f"1..{maximum_requested_roots} rebuilt roots",
            source="native targeted rebuild diagnostics",
        )
        validation.require(
            "locality text entries stay bounded by rebuilt roots",
            partial_text,
            lambda value: all(
                isinstance(value.get(key), int)
                and 0 <= value[key] <= rebuilt_roots
                for key in ("entries_removed", "entries_inserted")
            ),
            "removed/inserted text entries no greater than rebuilt roots",
            source="native retained text diagnostics",
        )
        validation.equal(
            "locality primitive rebuilds avoid fallback",
            retained_rebuilds.get("partial_base_fallbacks"),
            0,
            source="native retained primitive diagnostics",
        )
        validation.equal(
            "text-only locality uploads no primitive bytes",
            retained_rebuilds.get("partial_upload_bytes"),
            0,
            source="native retained primitive diagnostics",
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

    if state_controls:
        native_state = gpu.get("state") or {}
        text_values = native_state.get("text_val") or {}
        float_values = native_state.get("float_val") or {}
        dropdown_indices = native_state.get("dropdown_index") or {}
        text_scroll_y = native_state.get("text_scroll_y") or {}
        renderer = gpu.get("renderer") or {}
        caret_positions = renderer.get("caret_positions") or {}
        owner_geometry = renderer.get("text_owner_geometry") or {}
        diagnostics = layout.get("diagnostics") or {}

        expected_text = {
            "text_input": _state_control_text("input", final_tick).replace("\n", " / "),
            "text_area": _state_control_text("area", final_tick),
            "code_editor": _state_control_text("code", final_tick),
            "log_follow": _state_control_text("follow", final_tick),
            "log_static": _state_control_text("static", final_tick),
        }
        for name, expected in expected_text.items():
            control = state_controls[name]
            python_value = control.value
            validation.equal(
                f"final Python {name} value",
                python_value,
                expected,
                source="Python state-control value",
            )
            validation.equal(
                f"final native {name} value",
                text_values.get(control.id),
                expected,
                source="native state-control buffer",
            )

        expected_dropdown_index = final_tick % len(STATE_DROPDOWN_ITEMS)
        validation.equal(
            "final Python dropdown value",
            state_controls["dropdown"].value,
            STATE_DROPDOWN_ITEMS[expected_dropdown_index],
            source="Python state-control value",
        )
        validation.equal(
            "final native dropdown index",
            dropdown_indices.get(state_controls["dropdown"].id),
            expected_dropdown_index,
            source="native dropdown state",
        )
        expected_number = final_tick * 1.25 - 500.5
        expected_drag = 500.5 - final_tick * 0.75
        for name, expected in (
            ("number_input", expected_number),
            ("drag_number", expected_drag),
        ):
            control = state_controls[name]
            validation.equal(
                f"final Python {name} value",
                control.value,
                expected,
                source="Python numeric-control value",
            )
            validation.equal(
                f"final native {name} value",
                float_values.get(control.id),
                expected,
                source="native numeric-control state",
            )

        focused_id = state_controls["text_area"].id
        validation.equal(
            "focused state control retained focus",
            native_state.get("focused"),
            focused_id,
            source="native widget focus state",
        )
        caret = caret_positions.get(focused_id)
        focused_rect = (diagnostics.get(focused_id) or {}).get("resolved") or {}
        validation.require(
            "focused text-area caret remains inside its control",
            {"caret": caret, "rect": focused_rect},
            lambda value: isinstance(value["caret"], list)
            and len(value["caret"]) == 2
            and all(isinstance(item, (int, float)) for item in value["caret"])
            and isinstance(value["rect"].get("width"), (int, float))
            and isinstance(value["rect"].get("height"), (int, float))
            and -0.5 <= value["caret"][0] <= value["rect"]["width"] + 0.5
            and -0.5 <= value["caret"][1] <= value["rect"]["height"] + 0.5,
            "relative caret x/y inside resolved width/height",
            source="native shaped caret diagnostics",
        )

        for control in state_controls.values():
            entries = owner_geometry.get(control.id)
            rect = (diagnostics.get(control.id) or {}).get("resolved") or {}
            validation.require(
                f"{control.id} exposes bounded internal text geometry",
                {"entries": entries, "rect": rect},
                lambda value: isinstance(value["entries"], list)
                and bool(value["entries"])
                and isinstance(value["rect"].get("x"), (int, float))
                and isinstance(value["rect"].get("y"), (int, float))
                and isinstance(value["rect"].get("width"), (int, float))
                and isinstance(value["rect"].get("height"), (int, float))
                and all(
                    isinstance(entry.get("line_count"), int)
                    and entry["line_count"] > 0
                    and isinstance((entry.get("clip") or {}).get("left"), (int, float))
                    and isinstance((entry.get("clip") or {}).get("top"), (int, float))
                    and isinstance((entry.get("clip") or {}).get("right"), (int, float))
                    and isinstance((entry.get("clip") or {}).get("bottom"), (int, float))
                    and entry["clip"]["left"] + 1.0 >= value["rect"]["x"]
                    and entry["clip"]["top"] + 1.0 >= value["rect"]["y"]
                    and entry["clip"]["right"]
                    <= value["rect"]["x"] + value["rect"]["width"] + 1.0
                    and entry["clip"]["bottom"]
                    <= value["rect"]["y"] + value["rect"]["height"] + 1.0
                    for entry in value["entries"]
                ),
                "owned text entries with clips inside the resolved control",
                source="native text-owner geometry diagnostics",
            )

        for name in ("text_area",):
            entries = owner_geometry.get(state_controls[name].id) or []
            validation.require(
                f"{name} exercises wrapped visual lines",
                entries,
                lambda value: max((entry.get("line_count") or 0 for entry in value), default=0)
                > 5,
                "more visual lines than the five logical input lines",
                source="native shaped text diagnostics",
            )
        validation.require(
            "following log requests internal bottom scroll",
            text_scroll_y.get(state_controls["log_follow"].id),
            lambda value: isinstance(value, (int, float)) and value > 0,
            "positive internal text scroll",
            source="native LogView follow state",
        )
        validation.require(
            "non-following log preserves its internal scroll",
            text_scroll_y.get(state_controls["log_static"].id, 0),
            lambda value: isinstance(value, (int, float)) and abs(value) <= 0.001,
            "zero internal text scroll",
            source="native LogView follow state",
        )

    if args.scenario == "plot-chrome":
        diagnostics = layout.get("diagnostics") or {}
        renderer = gpu.get("renderer") or {}
        plot_geometry = renderer.get("plot_geometry") or {}
        owner_geometry = renderer.get("text_owner_geometry") or {}
        expected_grid = final_tick % 2 == 0
        expected_axes = final_tick % 3 != 0
        expected_ticks = final_tick % 4 != 0
        expected_toolbar = final_tick % 5 in (1, 2)
        expected_tick_count = 2 + final_tick % 8
        for family, plot in plot_controls.items():
            native_plot = find_tree_node(gpu.get("tree"), plot.id) or {}
            props = (native_plot.get("props") or {}).get(
                "line_plot" if family == "line" else "bar_chart" if family == "bar" else "histogram"
            ) or {}
            for name, actual, expected in (
                ("grid", props.get("show_grid"), expected_grid),
                ("axes", props.get("show_axes"), expected_axes),
                ("ticks", props.get("show_ticks"), expected_ticks),
                ("toolbar", props.get("show_toolbar"), expected_toolbar),
                ("tick count", props.get("tick_count"), expected_tick_count),
                ("x label", props.get("x_label"), _plot_axis_text(family, "x", final_tick)),
                ("y label", props.get("y_label"), _plot_axis_text(family, "y", final_tick)),
            ):
                validation.equal(
                    f"final native {family} {name}",
                    actual,
                    expected,
                    source="native retained plot chrome",
                )
            geometry = plot_geometry.get(plot.id) or {}
            validation.require(
                f"{family} exposes a bounded internal drawable viewport",
                geometry,
                lambda value: all(
                    isinstance((value.get(section) or {}).get(key), (int, float))
                    for section, key in (
                        ("outer", "left"),
                        ("outer", "top"),
                        ("outer", "width"),
                        ("outer", "height"),
                        ("plot", "left"),
                        ("plot", "top"),
                        ("plot", "width"),
                        ("plot", "height"),
                    )
                )
                and value["plot"]["width"] > 0
                and value["plot"]["height"] > 0
                and value["plot"]["left"] + 0.5 >= value["outer"]["left"]
                and value["plot"]["top"] + 0.5 >= value["outer"]["top"]
                and value["plot"]["left"] + value["plot"]["width"]
                <= value["outer"]["left"] + value["outer"]["width"] + 0.5
                and value["plot"]["top"] + value["plot"]["height"]
                <= value["outer"]["top"] + value["outer"]["height"] + 0.5,
                "positive plot rectangle contained by the fixed outer plot rectangle",
                source="native plot geometry diagnostics",
            )
            entries = owner_geometry.get(plot.id) or []
            outer = geometry.get("outer") or {}
            validation.require(
                f"{family} emits clipped plot chrome text",
                {"entries": entries, "outer": outer},
                lambda value: bool(value["entries"])
                and all(
                    isinstance((entry.get("clip") or {}).get("left"), (int, float))
                    and isinstance((entry.get("clip") or {}).get("top"), (int, float))
                    and isinstance((entry.get("clip") or {}).get("right"), (int, float))
                    and isinstance((entry.get("clip") or {}).get("bottom"), (int, float))
                    and entry["clip"]["left"] + 1.0 >= value["outer"]["left"]
                    and entry["clip"]["top"] + 1.0 >= value["outer"]["top"]
                    and entry["clip"]["right"]
                    <= value["outer"]["left"] + value["outer"]["width"] + 1.0
                    and entry["clip"]["bottom"]
                    <= value["outer"]["top"] + value["outer"]["height"] + 1.0
                    for entry in value["entries"]
                ),
                "owned tick/axis/toolbar/legend text clipped inside the plot",
                source="native text-owner geometry diagnostics",
            )

        line_props = (
            (find_tree_node(gpu.get("tree"), plot_controls["line"].id) or {})
            .get("props", {})
            .get("line_plot", {})
        )
        expected_position = (
            "top-right",
            "top-left",
            "bottom-right",
            "bottom-left",
        )[final_tick % 4]
        validation.equal(
            "final line-plot legend visibility",
            line_props.get("show_legend"),
            final_tick % 2 == 1,
            source="native retained line-plot chrome",
        )
        validation.equal(
            "final line-plot legend position",
            line_props.get("legend_position"),
            expected_position,
            source="native retained line-plot chrome",
        )
        validation.equal(
            "final line-plot window size",
            line_props.get("window_size"),
            float(6 + final_tick % 5),
            source="native retained line-plot bounds",
        )
        expected_bounds = {
            "x_min": float(final_tick % 8),
            "x_max": float(final_tick % 8 + 12),
            "y_min": float(-10 - final_tick % 3),
            "y_max": float(12 + final_tick % 4),
        }
        validation.equal(
            "final line-plot authored bounds",
            {key: line_props.get(key) for key in expected_bounds},
            expected_bounds,
            source="native retained line-plot bounds",
        )
        validation.equal(
            "final line-plot resolved bounds",
            (plot_geometry.get(plot_controls["line"].id) or {}).get("bounds"),
            expected_bounds,
            source="native plot geometry diagnostics",
        )
        retained_rebuilds = (
            ((renderer.get("primitives") or {}).get("retained_rebuilds") or {})
        )
        validation.require(
            "targeted line-plot resource rebuild executed",
            retained_rebuilds,
            lambda value: isinstance(value.get("targeted_line_plot_rebuilds"), int)
            and value["targeted_line_plot_rebuilds"] > 0
            and value.get("targeted_line_plot_checks")
            == value["targeted_line_plot_rebuilds"]
            + int(value.get("targeted_line_plot_skips") or 0),
            "positive line-plot rebuild count with balanced targeted checks",
            source="native retained primitive diagnostics",
        )
        validation.require(
            "line-plot GPU resource contains both series",
            renderer.get("line_plot_renderer") or {},
            lambda value: value.get("series_count") == 2
            and isinstance(value.get("point_count"), int)
            and value["point_count"] >= 4,
            "two retained GPU series with uploaded points",
            source="native line-plot renderer diagnostics",
        )

    if args.scenario in ("html-fallback", "html-webview"):
        renderer = gpu.get("renderer") or {}
        html_renderer = renderer.get("html_reports") or {}
        diagnostics = layout.get("diagnostics") or {}
        fallback_report = html_controls["fallback"]
        source_report = html_controls["source"]
        fallback_node = find_tree_node(gpu.get("tree"), fallback_report.id) or {}
        source_node = find_tree_node(gpu.get("tree"), source_report.id) or {}
        fallback_props = fallback_node.get("props") or {}
        source_props = source_node.get("props") or {}
        fallback_html = fallback_props.get("html_report") or {}
        source_html = source_props.get("html_report") or {}

        validation.equal(
            "final Python HTML fallback text",
            fallback_report.diagnostic_text,
            _html_fallback_text(final_tick),
            source="Python HTML fallback state",
        )
        validation.equal(
            "final native HTML fallback text",
            fallback_props.get("text"),
            _html_fallback_text(final_tick),
            source="native retained HTML fallback state",
        )
        validation.equal(
            "fallback report retained its inline source fingerprint",
            fallback_html.get("inline_fnv1a64"),
            _fnv1a64(_html_document(0)),
            source="native retained HTML source diagnostics",
        )

        source_is_inline = False
        expected_source_path = (
            None
            if source_is_inline
            else str(ROOT / "benchmarks" / "gui_update_pipeline_case.py")
        )
        expected_source_html = _html_document(final_tick) if source_is_inline else None
        expected_base_dir = str(ROOT) if source_is_inline else None
        validation.equal(
            "final Python HTML source path",
            source_report.path,
            expected_source_path,
            source="Python HTML source state",
        )
        validation.equal(
            "final Python inline HTML source",
            source_report.html,
            expected_source_html,
            source="Python HTML source state",
        )
        validation.equal(
            "final native HTML source path",
            source_html.get("path"),
            expected_source_path,
            source="native retained HTML source state",
        )
        validation.equal(
            "final native HTML base directory",
            source_html.get("base_dir"),
            expected_base_dir,
            source="native retained HTML source state",
        )
        validation.equal(
            "final native inline HTML fingerprint",
            source_html.get("inline_fnv1a64"),
            _fnv1a64(expected_source_html) if expected_source_html is not None else None,
            source="native retained HTML source diagnostics",
        )

        for report_index, report in enumerate((fallback_report, source_report)):
            node = find_tree_node(gpu.get("tree"), report.id) or {}
            report_props = (node.get("props") or {}).get("html_report") or {}
            for name, actual, expected in (
                ("allow_remote", report_props.get("allow_remote"), (final_tick + report_index) % 2 == 0),
                ("allow_scripts", report_props.get("allow_scripts"), (final_tick + report_index) % 3 != 0),
                (
                    "external_fallback",
                    report_props.get("external_fallback"),
                    (final_tick + report_index) % 4 != 0,
                ),
            ):
                validation.equal(
                    f"final {report.id} {name}",
                    actual,
                    expected,
                    source="native retained HTML security state",
                )
            resolved = (diagnostics.get(report.id) or {}).get("resolved") or {}
            validation.require(
                f"{report.id} retained fixed outer geometry",
                resolved,
                lambda value: isinstance(value.get("width"), (int, float))
                and isinstance(value.get("height"), (int, float))
                and value["width"] > 0
                and value["height"] > 0,
                "positive fixed HTML report rectangle",
                source="native layout diagnostics",
            )

        if args.scenario == "html-fallback":
            validation.equal(
                "embedded HTML renderer forced off",
                html_renderer.get("enabled"),
                False,
                source="native HTML renderer diagnostics",
            )
            validation.require(
                "HTML fallback disable reason is explicit",
                html_renderer.get("reason"),
                lambda value: isinstance(value, str)
                and "DRAGONGUI_HTMLREPORT_WEBVIEW2=0" in value,
                "environment-forced native fallback reason",
                source="native HTML renderer diagnostics",
            )
            validation.equal(
                "disabled HTML renderer has no child views",
                html_renderer.get("instances") or {},
                {},
                source="native HTML renderer diagnostics",
            )
            owner_geometry = renderer.get("text_owner_geometry") or {}
            validation.require(
                "fallback text exposes owned shaped geometry",
                owner_geometry.get(fallback_report.id),
                lambda value: isinstance(value, list) and bool(value),
                "at least one native fallback text entry",
                source="native text-owner geometry diagnostics",
            )
        else:
            validation.equal(
                "embedded HTML renderer remained enabled",
                html_renderer.get("enabled"),
                True,
                source="native HTML renderer diagnostics",
            )
            validation.equal(
                "embedded HTML environment initialized",
                html_renderer.get("environment_ready"),
                True,
                source="native HTML renderer diagnostics",
            )
            validation.require(
                "embedded HTML renderer has no unrecovered error",
                html_renderer,
                lambda value: value.get("last_error") is None
                or value.get("profile_recovered") is True,
                "no error or successful fresh-profile recovery",
                source="native HTML renderer diagnostics",
            )
            instances = html_renderer.get("instances") or {}
            for report_index, report in enumerate((fallback_report, source_report)):
                instance = instances.get(report.id) or {}
                validation.equal(
                    f"{report.id} WebView is visible",
                    instance.get("visible"),
                    True,
                    source="native HTML renderer diagnostics",
                )
                validation.equal(
                    f"{report.id} WebView script policy",
                    instance.get("allow_scripts"),
                    (final_tick + report_index) % 3 != 0,
                    source="native HTML renderer diagnostics",
                )
                validation.require(
                    f"{report.id} WebView source fingerprint present",
                    instance.get("source_fingerprint"),
                    lambda value: isinstance(value, str) and len(value) == 16,
                    "stable 64-bit source fingerprint",
                    source="native HTML renderer diagnostics",
                )
            validation.equal(
                "fallback WebView retained inline source",
                (instances.get(fallback_report.id) or {}).get("source"),
                "html",
                source="native HTML renderer diagnostics",
            )
            validation.equal(
                "changing WebView retained final source family",
                (instances.get(source_report.id) or {}).get("source"),
                "html" if source_is_inline else "url",
                source="native HTML renderer diagnostics",
            )

    if args.scenario == "semantic-icons":
        computed_styles = gpu.get("computed_styles") or {}
        diagnostics = layout.get("diagnostics") or {}
        ready_layout = ((ready_snapshot.get("gpu") or {}).get("layout") or {})
        ready_diagnostics = ready_layout.get("diagnostics") or {}
        icon_names = ("search", "folder-open", "settings", "not-registered")
        final_theme_even = final_tick % 2 == 0
        validation.equal(
            "live icon theme retained two overrides",
            (gpu.get("icon_theme") or {}).get("override_count"),
            2,
            source="native icon-theme diagnostics",
        )
        for index, icon in enumerate(icon_controls):
            requested = icon_names[(final_tick + index) % len(icon_names)]
            validation.equal(
                f"final Python icon {index}",
                icon.icon,
                requested,
                source="Python semantic icon state",
            )
            native_icon = find_tree_node(gpu.get("tree"), icon.id) or {}
            validation.equal(
                f"final native icon {index}",
                (native_icon.get("props") or {}).get("icon"),
                requested,
                source="native retained semantic icon state",
            )
            if requested == "search":
                expected_identity = (
                    {
                        "requested": requested,
                        "resolved": "search",
                        "recognized": True,
                        "alias": False,
                        "fallback": False,
                        "source": "application",
                        "resource_type": "stroke",
                    }
                    if final_theme_even
                    else {
                        "requested": requested,
                        "resolved": "help",
                        "recognized": True,
                        "alias": False,
                        "fallback": False,
                        "source": "application",
                        "resource_type": None,
                    }
                )
            elif requested == "folder-open":
                expected_identity = (
                    {
                        "requested": requested,
                        "resolved": "warning",
                        "recognized": True,
                        "alias": False,
                        "fallback": False,
                        "source": "application",
                        "resource_type": None,
                    }
                    if final_theme_even
                    else {
                        "requested": requested,
                        "resolved": "folder-open",
                        "recognized": True,
                        "alias": False,
                        "fallback": False,
                        "source": "application",
                        "resource_type": "stroke",
                    }
                )
            else:
                resolution = dg.resolve_icon(requested)
                expected_identity = {
                    "requested": resolution.requested,
                    "resolved": resolution.resolved,
                    "recognized": resolution.recognized,
                    "alias": resolution.alias,
                    "fallback": resolution.fallback,
                    "source": "builtin",
                    "resource_type": None,
                }
            validation.equal(
                f"final computed icon identity {index}",
                (computed_styles.get(icon.id) or {}).get("icon"),
                expected_identity,
                source="native computed semantic icon diagnostics",
            )
            resolved = (diagnostics.get(icon.id) or {}).get("resolved") or {}
            ready_resolved = (ready_diagnostics.get(icon.id) or {}).get("resolved") or {}
            validation.equal(
                f"icon {index} retained ready-frame outer width",
                resolved.get("width"),
                ready_resolved.get("width"),
                source="native layout diagnostics",
            )
            validation.equal(
                f"icon {index} retained ready-frame outer height",
                resolved.get("height"),
                ready_resolved.get("height"),
                source="native layout diagnostics",
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
            "text_invalidation_mode": args.text_invalidation_mode,
            "targeted_rebuild_verification": args.targeted_rebuild_verification,
            "typed_target_diagnostics": args.typed_target_diagnostics,
            "locality_roots": args.locality_roots,
            "capture_screenshot": args.capture_screenshot,
            "interaction_probe_ms": args.interaction_probe_ms,
            "html_reload_attempts": html_reload_attempts,
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
                "structure_generation": framework.get("structure_generation"),
                "dirty_rebuilds": framework.get("dirty_rebuilds"),
                "command_text_rebuilds": framework.get("command_text_rebuilds"),
                "targeted_rebuild_verification": framework.get(
                    "targeted_rebuild_verification"
                ),
                "live_text_invalidation": framework.get("live_text_invalidation"),
                "partial_text_rebuild": framework.get("partial_text_rebuild"),
                "partial_text_rebuilds": framework.get("partial_text_rebuilds"),
                "style_reapply": framework.get("style_reapply"),
                "layout_compute": framework.get("layout_compute"),
                "apply_layout": framework.get("apply_layout"),
            },
            "renderer": {
                "retained_rebuilds": (
                    ((gpu.get("renderer") or {}).get("primitives") or {}).get(
                        "retained_rebuilds"
                    )
                ),
                "line_plot_renderer": (gpu.get("renderer") or {}).get(
                    "line_plot_renderer"
                ),
                "plot_geometry": (gpu.get("renderer") or {}).get("plot_geometry"),
                "html_reports": (gpu.get("renderer") or {}).get("html_reports"),
                "icon_geometry_cache": (
                    ((gpu.get("renderer") or {}).get("primitives") or {}).get(
                        "icon_geometry_cache"
                    )
                ),
            },
            "icon_theme": gpu.get("icon_theme"),
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
