"""Run a compact, repeatable THEME FORGE performance profile.

Examples:

    py -3.12 tools/profile_theme_forge_stress.py --frames 60 --no-live
    py -3.12 tools/profile_theme_forge_stress.py --frames 120
    py -3.12 tools/profile_theme_forge_stress.py --no-live --theme-cycles 3
    py -3.12 tools/profile_theme_forge_stress.py --rows 2000 --output report.json

Run one case per process because the native GUI event loop is process-scoped.
The report excludes the large tree/layout maps and retains the timing, cascade,
renderer, and command-queue evidence needed for library-level comparisons.
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

from examples import theme_forge_stress_demo as forge  # noqa: E402


def _top_timings(values: Any, *, limit: int = 16) -> dict[str, Any]:
    if not isinstance(values, dict):
        return {}
    timings = [
        (name, value)
        for name, value in values.items()
        if isinstance(value, dict) and "total_ms" in value
    ]
    timings.sort(key=lambda item: float(item[1].get("total_ms", 0.0)), reverse=True)
    return dict(timings[:limit])


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
    layout = gpu.get("layout") or {}
    diagnostics = layout.get("diagnostics") or {}
    issue_count = sum(bool(entry.get("issues")) for entry in diagnostics.values())

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
        "command_queue_depth",
        "command_drain_yields",
        "suppressed_command_fairness_warnings",
    )
    return {
        "profile_schema": 1,
        "profile_kind": "theme-forge",
        "workload": {
            "theme": args.theme,
            "rows": args.rows,
            "frames": args.frames,
            "live": not args.no_live,
            "theme_cycles": args.theme_cycles,
            "widget_count": renderer.get("widget_count"),
            "layout_rect_count": len(layout.get("rects") or {}),
            "layout_issue_count": issue_count,
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
        "command_drain": runtime.get("command_drain"),
        "top_command_timings": _top_timings(runtime.get("command_timings")),
        "command_dirty_counts": runtime.get("command_dirty_counts"),
        "dirty_rebuilds": framework.get("dirty_rebuilds"),
        "partial_text_rebuilds": framework.get("partial_text_rebuilds"),
        "command_text_rebuilds": framework.get("command_text_rebuilds"),
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
            "primitives": renderer.get("primitives"),
            "text": renderer.get("text"),
            "layout_text_measurement": renderer.get("layout_text_measurement"),
            "line_plot_renderer": renderer.get("line_plot_renderer"),
        },
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--theme", choices=sorted(forge.TOKENS), default="modern-dark")
    parser.add_argument("--rows", type=int, default=600)
    parser.add_argument("--frames", type=int, default=90)
    parser.add_argument("--no-live", action="store_true")
    parser.add_argument(
        "--theme-cycles",
        type=int,
        default=0,
        help="rapid full passes through every theme after startup",
    )
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    args.rows = max(1, args.rows)
    args.frames = max(3, args.frames)
    args.theme_cycles = max(0, args.theme_cycles)
    os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(args.frames)
    forge.state.stop.clear()

    started = time.perf_counter()
    app, window = forge.build_app(
        args.theme,
        rows=args.rows,
        decorations="client",
    )
    build_ms = (time.perf_counter() - started) * 1000.0

    started = time.perf_counter()
    document = app.document(window, include_startup_resource_payloads=False)
    serialize_ms = (time.perf_counter() - started) * 1000.0
    document_bytes = len(json.dumps(document, separators=(",", ":")).encode("utf-8"))

    if not args.no_live:
        threading.Thread(
            target=forge.live_worker,
            args=(app,),
            name="theme-forge-profile-live",
            daemon=True,
        ).start()
    if args.theme_cycles:
        def cycle_themes() -> None:
            # Let the initial document settle so startup and replacement
            # cascades remain distinguishable in the timing report.
            time.sleep(0.3)
            forge.rapid_cycle(args.theme_cycles)

        threading.Thread(
            target=cycle_themes,
            name="theme-forge-profile-theme-cycles",
            daemon=True,
        ).start()

    started = time.perf_counter()
    try:
        result = app.run(window)
    finally:
        forge.state.stop.set()
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
