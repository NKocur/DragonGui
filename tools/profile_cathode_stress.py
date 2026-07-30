"""Run one compact, repeatable CATHODE-7 performance profile.

Examples:

    py -3.12 tools/profile_cathode_stress.py --frames 10 --no-live
    py -3.12 tools/profile_cathode_stress.py --frames 20 --tiles 96 --rows 480

Run one case per process because the native GUI event loop is process-scoped.
The report intentionally excludes the enormous tree/layout portions of the
debug snapshot and keeps the timing, renderer, and queue evidence.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys
import threading
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "python"))
sys.path.insert(0, str(ROOT))

from examples import cathode_ops_stress_demo as cathode  # noqa: E402


LIVE_GROUPS = (
    "plot",
    "scope",
    "bars",
    "labels",
    "leds",
    "log",
    "core-map",
    "clock",
)


def _timing_total(item: tuple[str, Any]) -> float:
    value = item[1]
    return float(value.get("total_ms", 0.0)) if isinstance(value, dict) else 0.0


def _top_timings(values: Any, *, limit: int = 12) -> dict[str, Any]:
    if not isinstance(values, dict):
        return {}
    timings = [
        item
        for item in values.items()
        if isinstance(item[1], dict) and "total_ms" in item[1]
    ]
    ordered = sorted(timings, key=_timing_total, reverse=True)
    return dict(ordered[:limit])


def _compact_report(
    result: dict[str, Any],
    *,
    args: argparse.Namespace,
    build_ms: float,
    serialize_ms: float,
    document_bytes: int,
    run_ms: float,
) -> dict[str, Any]:
    snapshot = result.get("debug_snapshot") or {}
    runtime = snapshot.get("runtime") or {}
    gpu = snapshot.get("gpu") or {}
    renderer = gpu.get("renderer") or {}
    stylesheets = gpu.get("stylesheets") or {}
    framework = gpu.get("framework") or {}

    runtime_keys = (
        "frames_rendered",
        "wall_fps",
        "frame_ms_avg",
        "last_frame_ms",
        "frame_work_ms_avg",
        "frame_prepare_ms_avg",
        "frame_acquire_ms_avg",
        "frame_encode_ms_avg",
        "frame_submit_ms_avg",
        "frame_present_ms_avg",
        "upload_ms",
        "command_queue_depth",
        "command_drain_yields",
        "suppressed_command_fairness_warnings",
        "startup_readiness",
    )
    return {
        "profile_schema": 1,
        "workload": {
            "style": args.style,
            "tiles": args.tiles,
            "rows": args.rows,
            "frames": args.frames,
            "live": not args.no_live,
            "disabled_live_groups": sorted(args.disable_live_group),
            "synthetic_hover_targets": args.synthetic_hover,
            "widget_count": renderer.get("widget_count"),
        },
        "python": {
            "build_app_ms": build_ms,
            "document_serialize_ms": serialize_ms,
            "document_json_bytes": document_bytes,
            "run_wall_ms": run_ms,
            "scheduler": runtime.get("python"),
        },
        "runtime": {key: runtime.get(key) for key in runtime_keys},
        "frame_timings": runtime.get("frame_timings"),
        "loading_screen": runtime.get("loading_screen"),
        "command_drain": runtime.get("command_drain"),
        "synthetic_input": runtime.get("synthetic_input"),
        "top_command_timings": _top_timings(runtime.get("command_timings")),
        "command_dirty_counts": runtime.get("command_dirty_counts"),
        "dirty_rebuilds": framework.get("dirty_rebuilds"),
        "partial_text_rebuilds": framework.get("partial_text_rebuilds"),
        "command_text_rebuilds": framework.get("command_text_rebuilds"),
        "interaction_text_rebuilds": framework.get("interaction_text_rebuilds"),
        "animation_activity": framework.get("animation_activity"),
        "top_framework_timings": _top_timings(framework),
        "stylesheets": {
            "framework_rules": stylesheets.get("framework_rules"),
            "theme_rules": stylesheets.get("theme_rules"),
            "user_rules": stylesheets.get("user_rules"),
            "warning_count": stylesheets.get("warning_count"),
            "last_cascade": stylesheets.get("last_cascade"),
            "unmatched_user_selector_count": len(
                stylesheets.get("unmatched_user_selectors") or []
            ),
        },
        "renderer": {
            "present_mode": renderer.get("present_mode"),
            "desired_maximum_frame_latency": renderer.get(
                "desired_maximum_frame_latency"
            ),
            "primitives": renderer.get("primitives"),
            "line_plot_renderer": renderer.get("line_plot_renderer"),
            "text": renderer.get("text"),
            "layout_text_measurement": renderer.get("layout_text_measurement"),
            "scatter_count": renderer.get("scatter_count"),
        },
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--style", choices=sorted(cathode.PALETTES), default="phosphor")
    parser.add_argument("--tiles", type=int, default=96)
    parser.add_argument("--rows", type=int, default=480)
    parser.add_argument("--frames", type=int, default=10)
    parser.add_argument("--no-live", action="store_true")
    parser.add_argument(
        "--synthetic-hover",
        action="append",
        default=[],
        metavar="WIDGET_ID",
        help=(
            "Move an instrumented synthetic pointer to the visible center of this "
            "widget after presentation; repeat for an interaction sequence."
        ),
    )
    parser.add_argument(
        "--disable-live-group",
        action="append",
        choices=LIVE_GROUPS,
        default=[],
        help="Disable one telemetry update group while retaining its widgets.",
    )
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def _disable_live_groups(groups: list[str]) -> None:
    disabled = set(groups)
    if "plot" in disabled:
        cathode.state.plot = None
    if "scope" in disabled:
        cathode.state.scope = None
    if "bars" in disabled:
        cathode.state.lane_bars.clear()
        cathode.state.metric_bars.clear()
    if "labels" in disabled:
        cathode.state.lane_values.clear()
        cathode.state.metric_values.clear()
    if "leds" in disabled:
        cathode.state.leds.clear()
    if "log" in disabled:
        cathode.state.console_log = None
    if "core-map" in disabled:
        cathode.state.core_map = None
    if "clock" in disabled:
        cathode.state.clock = None


def main() -> int:
    args = parse_args()
    args.tiles = max(1, args.tiles)
    args.rows = max(1, args.rows)
    args.frames = max(1, args.frames, len(args.synthetic_hover) + 2)
    os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(args.frames)
    if args.synthetic_hover:
        os.environ["DRAGONGUI_SYNTHETIC_HOVER_IDS"] = ",".join(
            args.synthetic_hover
        )
    cathode.state.stop.clear()

    started = time.perf_counter()
    app, window = cathode.build_app(args.style, tiles=args.tiles, rows=args.rows)
    build_ms = (time.perf_counter() - started) * 1000.0
    _disable_live_groups(args.disable_live_group)

    started = time.perf_counter()
    document = app.document(window, include_startup_resource_payloads=False)
    serialize_ms = (time.perf_counter() - started) * 1000.0
    document_bytes = len(json.dumps(document, separators=(",", ":")).encode("utf-8"))

    if not args.no_live:
        threading.Thread(
            target=cathode.live_worker,
            args=(app,),
            name="cathode-profile-telemetry",
            daemon=True,
        ).start()

    started = time.perf_counter()
    try:
        result = app.run(window)
    finally:
        cathode.state.stop.set()
    run_ms = (time.perf_counter() - started) * 1000.0

    report = _compact_report(
        result,
        args=args,
        build_ms=build_ms,
        serialize_ms=serialize_ms,
        document_bytes=document_bytes,
        run_ms=run_ms,
    )
    payload = json.dumps(report, indent=2, sort_keys=True)
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
    print(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
