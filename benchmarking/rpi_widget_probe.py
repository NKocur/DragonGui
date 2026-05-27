from __future__ import annotations

import argparse
import json
import math
import os
import sys
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


@dataclass(slots=True)
class ProbeFrame:
    columns: tuple[str, ...]
    data: dict[str, list[object]]

    @property
    def dtypes(self) -> tuple[str, ...]:
        return tuple(type(self.data[column][0]).__name__ for column in self.columns)

    @property
    def shape(self) -> tuple[int, int]:
        if not self.columns:
            return (0, 0)
        return (len(self.data[self.columns[0]]), len(self.columns))

    def __len__(self) -> int:
        return self.shape[0]

    def __getitem__(self, column: str) -> list[object]:
        return self.data[column]


def build_frame(rows: int) -> ProbeFrame:
    xs: list[float] = []
    sine: list[float] = []
    cosine: list[float] = []
    values: list[float] = []
    labels: list[str] = []
    for index in range(rows):
        x = index / 20.0
        xs.append(x)
        sine.append(math.sin(x))
        cosine.append(math.cos(x * 0.7))
        values.append(math.sin(x * 0.35) * 40.0 + (index % 17))
        labels.append(f"row-{index:04d}")
    return ProbeFrame(
        columns=("x", "sine", "cosine", "value", "label"),
        data={
            "x": xs,
            "sine": sine,
            "cosine": cosine,
            "value": values,
            "label": labels,
        },
    )


def build_window(args: argparse.Namespace) -> tuple[dg.App, dg.Window, dg.LinePlot]:
    frame = build_frame(args.rows)
    app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33"))
    window = dg.Window(
        "DragonGUI RPi Widget Probe",
        width=args.width,
        height=args.height,
    )
    with window:
        with dg.HLayout(style={"padding": 8, "gap": 8, "height": "100%", "min_height": 0}):
            with dg.Panel(
                "Navigation",
                width=170,
                style={"padding": 8, "gap": 8, "min_height": 0},
            ):
                with dg.ScrollArea(axis="y", gap=6, style={"min_height": 0}):
                    for label in (
                        "Grid",
                        "Scatter3D",
                        "LinePlot",
                        "Histogram",
                        "HTML",
                        "Table",
                        "Stats",
                        "Settings",
                        "Logs",
                        "Export",
                    ):
                        dg.Badge(label)
                    dg.Separator()
                    for index in range(16):
                        dg.Button(f"Action {index + 1}")

            with dg.VLayout(style={"gap": 8, "flex_grow": 1, "min_width": 0, "min_height": 0}):
                with dg.HLayout(style={"gap": 8, "min_height": 0, "flex_grow": 1}):
                    line = dg.LinePlot(
                        frame,
                        x="x",
                        y=["sine", "cosine"],
                        labels=["sine", "cosine"],
                        show_grid=True,
                        show_ticks=True,
                        show_toolbar=True,
                        show_legend=True,
                        max_points=args.rows,
                        style={"flex_grow": 1, "min_width": 0, "min_height": 0},
                    )
                    dg.Histogram(
                        frame,
                        value="value",
                        bins=32,
                        show_grid=True,
                        show_ticks=True,
                        show_toolbar=True,
                        style={"flex_grow": 1, "min_width": 0, "min_height": 0},
                    )
                with dg.HLayout(style={"gap": 8, "height": 170, "min_height": 0}):
                    with dg.Panel("Frame", style={"padding": 8, "gap": 6, "flex_grow": 1}):
                        dg.DataFrameTable(frame, page_size=64, sample_rows=256)
                    with dg.Panel("Quick actions", width=230, style={"padding": 8, "gap": 6}):
                        dg.Checkbox("Grid", checked=True)
                        dg.Checkbox("Auto fit", checked=True)
                        dg.Button("Refresh")
                        dg.Button("Snapshot")
    return app, window, line


def snapshot_value(snapshot: dict[str, Any], path: tuple[str, ...], default: Any = None) -> Any:
    current: Any = snapshot
    for part in path:
        if not isinstance(current, dict) or part not in current:
            return default
        current = current[part]
    return current


def compact_summary(name: str, snapshot: dict[str, Any]) -> dict[str, Any]:
    runtime_perf = snapshot_value(snapshot, ("runtime", "performance"), {})
    gpu_perf = snapshot_value(runtime_perf, ("gpu",), {})
    direct_gpu_perf = snapshot_value(snapshot, ("gpu", "performance"), {})
    platform = snapshot_value(snapshot, ("runtime", "platform"), {})
    if not isinstance(gpu_perf, dict):
        gpu_perf = {}
    if not gpu_perf and isinstance(direct_gpu_perf, dict):
        gpu_perf = direct_gpu_perf
    if not isinstance(runtime_perf, dict):
        runtime_perf = {}
    if not isinstance(platform, dict):
        platform = {}
    primitive_stats = gpu_perf.get("primitive_stats")
    if not isinstance(primitive_stats, dict):
        primitive_stats = {}
    return {
        "scenario": name,
        "profile": platform.get("profile"),
        "profile_source": platform.get("profile_source"),
        "wgpu_backend_override": platform.get("wgpu_backend_override"),
        "window_backend_override": platform.get("window_backend_override"),
        "adapter_backend": snapshot_value(snapshot, ("gpu", "renderer", "adapter", "backend")),
        "window": snapshot_value(snapshot, ("gpu", "window")),
        "frames_rendered": snapshot_value(snapshot, ("runtime", "frames_rendered")),
        "wall_fps": snapshot_value(snapshot, ("runtime", "wall_fps")),
        "frame_ms": snapshot_value(snapshot, ("runtime", "frame_ms")),
        "last_frame_ms": snapshot_value(snapshot, ("runtime", "last_frame_ms")),
        "native_performance_counters": bool(gpu_perf),
        "last_dirty": gpu_perf.get("last_dirty"),
        "primitive_count": gpu_perf.get("last_primitive_instance_count"),
        "text_count": gpu_perf.get("last_text_entry_count"),
        "primitive_rebuild_ms": gpu_perf.get("last_primitive_rebuild_ms"),
        "text_rebuild_ms": gpu_perf.get("last_text_rebuild_ms"),
        "primitive_upload_ms": primitive_stats.get("last_upload_ms"),
        "primitive_upload_bytes": primitive_stats.get("last_upload_bytes"),
        "primitive_vertex_cap_bytes": primitive_stats.get("vertex_cap_bytes"),
        "primitive_total_batches": primitive_stats.get("total_batches"),
        "primitive_base_batches": primitive_stats.get("base_batches"),
        "primitive_overlay_batches": primitive_stats.get("overlay_batches"),
        "primitive_base_general": snapshot_value(
            primitive_stats, ("base_general", "count")
        ),
        "primitive_base_general_batches": snapshot_value(
            primitive_stats, ("base_general", "batches")
        ),
        "primitive_base_simple": snapshot_value(
            primitive_stats, ("base_simple_square", "count")
        ),
        "primitive_base_simple_batches": snapshot_value(
            primitive_stats, ("base_simple_square", "batches")
        ),
        "primitive_base_rounded": snapshot_value(
            primitive_stats, ("base_rounded_solid", "count")
        ),
        "primitive_base_rounded_batches": snapshot_value(
            primitive_stats, ("base_rounded_solid", "batches")
        ),
        "primitive_base_line_segment": snapshot_value(
            primitive_stats, ("base_line_segment", "count")
        ),
        "primitive_base_line_segment_batches": snapshot_value(
            primitive_stats, ("base_line_segment", "batches")
        ),
        "primitive_base_line_plot": snapshot_value(
            primitive_stats, ("base_line_plot", "count")
        ),
        "primitive_base_line_plot_batches": snapshot_value(
            primitive_stats, ("base_line_plot", "batches")
        ),
        "queue_depth": runtime_perf.get(
            "command_queue_depth",
            snapshot_value(snapshot, ("runtime", "command_queue_depth")),
        ),
        "queue_oldest_age_ms": runtime_perf.get("command_queue_oldest_age_ms"),
        "queue_max_depth_observed": runtime_perf.get("command_queue_max_depth_observed"),
        "last_drained_command_count": runtime_perf.get("last_drained_command_count"),
        "last_raw_drained_command_count": runtime_perf.get("last_raw_drained_command_count"),
        "last_coalesced_command_count": runtime_perf.get("last_coalesced_command_count"),
        "total_queue_coalesced_command_count": runtime_perf.get(
            "total_queue_coalesced_command_count"
        ),
        "notes": [
            "LinePlot startup/render surface included.",
            "LinePlot live append command queued before live snapshot when available.",
            "Histogram render surface included.",
            "DataFrameTable and page/sidebar scroll surfaces included.",
        ],
    }


def request_live_snapshot(
    app: dg.App,
    line: dg.LinePlot,
    args: argparse.Namespace,
    output: list[dict[str, Any]],
) -> None:
    time.sleep(args.snapshot_delay)
    try:
        start = args.rows
        xs = [float(start + index) / 20.0 for index in range(args.append_points)]
        ys = [math.sin(x) for x in xs]
        line.append_points(xs, ys, max_points=args.rows)
        app.request_redraw()
        time.sleep(args.snapshot_delay)
        output.append(app.debug_snapshot(timeout_ms=args.timeout_ms))
    except RuntimeError as exc:
        output.append({"error": str(exc)})


def configure_environment(args: argparse.Namespace) -> None:
    os.environ.setdefault("DRAGONGUI_PROFILE", args.profile)
    os.environ.setdefault("DRAGONGUI_SMOKE_FRAMES", str(args.frames))
    if args.backend:
        os.environ.setdefault("DRAGONGUI_WGPU_BACKEND", args.backend)
    if args.window_backend:
        os.environ.setdefault("DRAGONGUI_WINDOW_BACKEND", args.window_backend)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Capture a compact DragonGUI Raspberry Pi widget baseline."
    )
    parser.add_argument("--width", type=int, default=800)
    parser.add_argument("--height", type=int, default=480)
    parser.add_argument("--rows", type=int, default=2048)
    parser.add_argument("--append-points", type=int, default=256)
    parser.add_argument("--frames", type=int, default=10)
    parser.add_argument("--profile", default="pi")
    parser.add_argument("--backend", default="gl")
    parser.add_argument("--window-backend", default="x11")
    parser.add_argument("--timeout-ms", type=int, default=3000)
    parser.add_argument("--snapshot-delay", type=float, default=0.25)
    parser.add_argument("--json", action="store_true", help="Print raw JSON.")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    configure_environment(args)
    app, window, line = build_window(args)
    live_snapshots: list[dict[str, Any]] = []
    threading.Thread(
        target=request_live_snapshot,
        args=(app, line, args, live_snapshots),
        daemon=True,
    ).start()
    result = app.run(window)
    final_snapshot = result.get("debug_snapshot")
    if not isinstance(final_snapshot, dict):
        raise RuntimeError("DragonGUI run result did not include a debug snapshot")

    summaries = [compact_summary("final", final_snapshot)]
    if live_snapshots and "error" not in live_snapshots[-1]:
        summaries.insert(0, compact_summary("live_after_append", live_snapshots[-1]))
    elif live_snapshots:
        summaries.insert(0, {"scenario": "live_after_append", **live_snapshots[-1]})

    if args.json:
        print(json.dumps(summaries, indent=2, sort_keys=True))
    else:
        for summary in summaries:
            print(f"Scenario: {summary['scenario']}")
            for key, value in summary.items():
                if key in {"scenario", "notes"}:
                    continue
                print(f"  {key}: {value}")
            print("  notes:")
            for note in summary.get("notes", []):
                print(f"    - {note}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
