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


@dataclass
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


def build_table_frame(rows: int) -> ProbeFrame:
    xs: list[float] = []
    sine: list[float] = []
    cosine: list[float] = []
    values: list[float] = []
    labels: list[str] = []
    for index in range(max(1, rows)):
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


def build_scatter_frame(points: int) -> ProbeFrame:
    count = max(1, points)
    side = max(2, int(math.sqrt(count)))
    xs: list[float] = []
    ys: list[float] = []
    zs: list[float] = []
    for index in range(count):
        gx = index % side
        gy = index // side
        u = (gx / max(1, side - 1)) * 2.0 - 1.0
        v = (gy / max(1, side - 1)) * 2.0 - 1.0
        phase = math.sin(u * math.tau * 3.0) + math.cos(v * math.tau * 2.0)
        xs.append(u * 18.0)
        ys.append(v * 10.0)
        zs.append(phase * 4.0)
    return ProbeFrame(columns=("x", "y", "z"), data={"x": xs, "y": ys, "z": zs})


def add_navigation() -> None:
    with dg.Panel(
        "Navigation",
        width=168,
        style={"padding": 6, "gap": 5, "height": "100%", "min_height": 0, "overflow_y": "auto"},
    ):
        with dg.FlowLayout(gap=4, style={"padding_right": 12}):
            for label in ("Grid", "Scatter3D", "LinePlot", "Histogram", "HtmlReport"):
                dg.Badge(label)
        dg.Separator()
        for label in (
            "Start",
            "Stop",
            "Snapshot",
            "Fit",
            "Reset",
            "Grid",
            "Planes",
            "Colorbar",
            "Debug",
            "Export",
            "Settings",
            "Help",
        ):
            dg.Button(label, style={"height": 24, "min_height": 24, "padding": "2px 6px"})


def add_controls_panel() -> None:
    with dg.Panel("Controls", style={"padding": 6, "gap": 5, "height": "100%", "min_height": 0}):
        with dg.FlowLayout(gap=4):
            dg.Badge("Camera")
            dg.Badge("Live")
            dg.Badge("Pi")
        dg.Checkbox("Grid", checked=True)
        dg.Checkbox("Auto fit", checked=True)
        dg.Checkbox("Colorbar", checked=True)
        dg.Checkbox("Debug", checked=False)
        with dg.FlowLayout(gap=4):
            dg.Button("Wave", style={"height": 24, "min_height": 24})
            dg.Button("Rings", style={"height": 24, "min_height": 24})
            dg.Button("Clear", style={"height": 24, "min_height": 24})
        dg.Slider(0.35, min=0.0, max=1.0, step=0.05)
        dg.Dropdown(["phase", "intensity", "depth"], value="phase")


def add_widget_stack(args: argparse.Namespace, frame: ProbeFrame) -> None:
    with dg.VLayout(style={"gap": 6, "min_height": 0, "flex_grow": 1}):
        with dg.HLayout(style={"gap": 6, "min_height": 0, "flex_grow": 1}):
            dg.LinePlot(
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
        with dg.HLayout(style={"gap": 6, "height": 160, "min_height": 0}):
            with dg.Panel("Frame", style={"padding": 6, "gap": 4, "flex_grow": 1, "min_width": 0}):
                dg.DataFrameTable(frame, page_size=64, sample_rows=256)
            with dg.Panel("Quick actions", width=220, style={"padding": 6, "gap": 4}):
                dg.Button("Refresh", style={"height": 24, "min_height": 24})
                dg.Button("Snapshot", style={"height": 24, "min_height": 24})
                dg.Button("Export", style={"height": 24, "min_height": 24})


def add_scatter(args: argparse.Namespace, with_points: bool) -> None:
    frame = build_scatter_frame(args.points) if with_points else None
    dg.Scatter3D(
        frame,
        x="x",
        y="y",
        z="z",
        grid=args.scatter_grid,
        major_planes=args.scatter_planes,
        minor_planes=False,
        scalar_bar=with_points and args.scatter_scalar_bar,
        orientation_axes=args.scatter_orientation_axes,
        point_size=args.point_size,
        auto_point_size=args.auto_point_size,
        lod=args.lod,
        lod_threshold=args.lod_threshold,
        lod_factor=args.lod_factor,
        interactive_render_scale=args.render_scale,
        class_="probe-scatter",
        style={
            "scatter-point-style": args.point_style,
            "flex_grow": 1,
            "flex_shrink": 1,
            "min_width": 0,
            "min_height": 0,
            "height": "100%",
        },
    )


def build_window(args: argparse.Namespace) -> tuple[dg.App, dg.Window]:
    app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33", radius=args.radius))
    window = dg.Window("DragonGUI RPi Fullscreen Probe", width=args.width, height=args.height)
    table_frame = build_table_frame(args.rows)

    with window:
        if args.scenario == "bare":
            dg.Label("DragonGUI fullscreen probe", style={"padding": 10})
        elif args.scenario == "labels":
            with dg.VLayout(style={"padding": 8, "gap": 4, "height": "100%", "min_height": 0}):
                for index in range(args.label_count):
                    dg.Label(f"Label row {index + 1:02d}")
        else:
            with dg.HLayout(
                style={
                    "padding": args.padding,
                    "gap": args.gap,
                    "height": "100%",
                    "min_height": 0,
                    "overflow_y": "hidden",
                }
            ):
                if args.scenario in {"nav", "controls", "widgets", "scatter-empty", "scatter-points"}:
                    add_navigation()
                if args.scenario == "nav":
                    with dg.Panel("Main", style={"padding": 8, "flex_grow": 1, "min_height": 0}):
                        dg.Label("Navigation-only baseline.")
                elif args.scenario == "controls":
                    with dg.HLayout(style={"gap": 6, "flex_grow": 1, "min_height": 0}):
                        add_controls_panel()
                        with dg.Panel("Main", style={"padding": 8, "flex_grow": 1, "min_height": 0}):
                            dg.Label("Controls baseline.")
                elif args.scenario == "widgets":
                    add_widget_stack(args, table_frame)
                elif args.scenario == "scatter-empty":
                    with dg.VLayout(style={"gap": 6, "width": 180, "min_height": 0}):
                        add_controls_panel()
                    add_scatter(args, with_points=False)
                elif args.scenario == "scatter-points":
                    with dg.VLayout(style={"gap": 6, "width": 180, "min_height": 0}):
                        add_controls_panel()
                    add_scatter(args, with_points=True)
                else:
                    raise ValueError(f"unknown scenario: {args.scenario}")
    return app, window


def value_at(snapshot: dict[str, Any], path: tuple[str, ...], default: Any = None) -> Any:
    current: Any = snapshot
    for key in path:
        if not isinstance(current, dict):
            return default
        current = current.get(key)
    return default if current is None else current


def scatter_snapshot(snapshot: dict[str, Any]) -> dict[str, Any]:
    scatter = value_at(snapshot, ("gpu", "resources", "scatter"), {})
    return scatter if isinstance(scatter, dict) else {}


def summarize(snapshot: dict[str, Any], args: argparse.Namespace, label: str) -> dict[str, Any]:
    runtime_perf = value_at(snapshot, ("runtime", "performance"), {})
    gpu_perf = value_at(runtime_perf, ("gpu",), {})
    if not isinstance(runtime_perf, dict):
        runtime_perf = {}
    if not isinstance(gpu_perf, dict):
        gpu_perf = value_at(snapshot, ("gpu", "performance"), {})
    if not isinstance(gpu_perf, dict):
        gpu_perf = {}
    scatter = scatter_snapshot(snapshot)
    return {
        "label": label,
        "scenario": args.scenario,
        "requested_size": [args.width, args.height],
        "window": value_at(snapshot, ("gpu", "window")),
        "profile": value_at(snapshot, ("runtime", "platform", "profile")),
        "wgpu_backend": value_at(snapshot, ("runtime", "platform", "wgpu_backend")),
        "wgpu_backend_override": value_at(snapshot, ("runtime", "platform", "wgpu_backend_override")),
        "present_mode": value_at(snapshot, ("runtime", "platform", "present_mode")),
        "max_frame_latency": value_at(
            snapshot, ("runtime", "platform", "desired_maximum_frame_latency")
        ),
        "frames_rendered": value_at(snapshot, ("runtime", "frames_rendered")),
        "wall_fps": value_at(snapshot, ("runtime", "wall_fps"), 0.0),
        "frame_ms_avg": value_at(snapshot, ("runtime", "frame_ms_avg"), 0.0),
        "last_frame_ms": value_at(snapshot, ("runtime", "last_frame_ms"), 0.0),
        "frame_work_ms": value_at(snapshot, ("runtime", "frame_work_ms"), 0.0),
        "frame_prepare_ms": value_at(snapshot, ("runtime", "frame_prepare_ms"), 0.0),
        "frame_acquire_ms": value_at(snapshot, ("runtime", "frame_acquire_ms"), 0.0),
        "frame_encode_ms": value_at(snapshot, ("runtime", "frame_encode_ms"), 0.0),
        "frame_base_pass_encode_ms": value_at(snapshot, ("runtime", "frame_base_pass_encode_ms"), 0.0),
        "frame_scatter_pass_encode_ms": value_at(snapshot, ("runtime", "frame_scatter_pass_encode_ms"), 0.0),
        "frame_overlay_pass_encode_ms": value_at(snapshot, ("runtime", "frame_overlay_pass_encode_ms"), 0.0),
        "frame_encoder_finish_ms": value_at(snapshot, ("runtime", "frame_encoder_finish_ms"), 0.0),
        "frame_submit_ms": value_at(snapshot, ("runtime", "frame_submit_ms"), 0.0),
        "frame_present_ms": value_at(snapshot, ("runtime", "frame_present_ms"), 0.0),
        "command_queue_depth": value_at(snapshot, ("runtime", "command_queue_depth"), 0),
        "last_dirty": gpu_perf.get("last_dirty"),
        "primitive_rebuild_ms": gpu_perf.get("last_primitive_rebuild_ms"),
        "text_rebuild_ms": gpu_perf.get("last_text_rebuild_ms"),
        "primitive_count": gpu_perf.get("last_primitive_instance_count"),
        "text_count": gpu_perf.get("last_text_entry_count"),
        "scatter_point_count": scatter.get("point_count"),
        "scatter_effective_drawn": scatter.get("effective_draw_point_count"),
        "scatter_point_scale": scatter.get("point_size_scale"),
        "scatter_static_render_scale": scatter.get("static_render_scale"),
        "scatter_active_render_scale": scatter.get("active_render_scale"),
        "scatter_render_encode_ms": scatter.get("last_render_encode_ms"),
        "scatter_upload_ms": scatter.get("last_upload_ms"),
        "scatter_grid_ms": scatter.get("last_grid_ms"),
        "scatter_overlay_ms": scatter.get("last_overlay_ms"),
    }


def print_summary(summary: dict[str, Any]) -> None:
    print(f"Scenario: {summary['scenario']} ({summary['label']})")
    for key, value in summary.items():
        if key in {"scenario", "label"}:
            continue
        print(f"  {key}: {value}")


def request_midrun_snapshot(
    app: dg.App,
    args: argparse.Namespace,
    snapshots: list[dict[str, Any]],
) -> None:
    time.sleep(args.snapshot_delay)
    try:
        snapshots.append(app.debug_snapshot(timeout_ms=args.timeout_ms))
    except RuntimeError as exc:
        snapshots.append({"error": str(exc)})


def configure_environment(args: argparse.Namespace) -> None:
    os.environ.setdefault("DRAGONGUI_PROFILE", args.profile)
    os.environ.setdefault("DRAGONGUI_SMOKE_FRAMES", str(args.frames))
    if args.static_render_scale is not None:
        os.environ["DRAGONGUI_SCATTER_STATIC_RENDER_SCALE"] = str(args.static_render_scale)
    if args.present_mode:
        os.environ["DRAGONGUI_PRESENT_MODE"] = args.present_mode
    if args.max_frame_latency is not None:
        os.environ["DRAGONGUI_MAX_FRAME_LATENCY"] = str(args.max_frame_latency)
    if args.backend:
        os.environ.setdefault("DRAGONGUI_WGPU_BACKEND", args.backend)
    if args.window_backend:
        os.environ.setdefault("DRAGONGUI_WINDOW_BACKEND", args.window_backend)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Measure DragonGUI fullscreen GUI cost on Raspberry Pi.")
    parser.add_argument(
        "--scenario",
        choices=("bare", "labels", "nav", "controls", "widgets", "scatter-empty", "scatter-points"),
        default="scatter-empty",
    )
    parser.add_argument("--width", type=int, default=800)
    parser.add_argument("--height", type=int, default=480)
    parser.add_argument("--frames", type=int, default=90)
    parser.add_argument("--rows", type=int, default=2048)
    parser.add_argument("--label-count", type=int, default=32)
    parser.add_argument("--points", type=int, default=150_000)
    parser.add_argument("--point-size", type=float, default=2.0)
    parser.add_argument("--point-style", choices=("circle", "square", "gaussian"), default="circle")
    parser.add_argument("--auto-point-size", action=argparse.BooleanOptionalAction, default=True)
    parser.add_argument("--lod", action=argparse.BooleanOptionalAction, default=True)
    parser.add_argument("--lod-threshold", type=int, default=50_000)
    parser.add_argument("--lod-factor", type=int, default=8)
    parser.add_argument("--render-scale", type=float, default=0.75)
    parser.add_argument("--static-render-scale", type=float, default=None)
    parser.add_argument("--scatter-grid", action=argparse.BooleanOptionalAction, default=True)
    parser.add_argument("--scatter-planes", action=argparse.BooleanOptionalAction, default=True)
    parser.add_argument("--scatter-scalar-bar", action=argparse.BooleanOptionalAction, default=True)
    parser.add_argument("--scatter-orientation-axes", action=argparse.BooleanOptionalAction, default=True)
    parser.add_argument("--padding", type=int, default=4)
    parser.add_argument("--gap", type=int, default=4)
    parser.add_argument("--radius", type=float, default=8.0)
    parser.add_argument("--profile", default="pi")
    parser.add_argument("--backend", default="gl")
    parser.add_argument("--window-backend", default="x11")
    parser.add_argument("--present-mode", default=None)
    parser.add_argument("--max-frame-latency", type=int, default=None)
    parser.add_argument("--snapshot-delay", type=float, default=0.35)
    parser.add_argument("--timeout-ms", type=int, default=3000)
    parser.add_argument("--json", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    configure_environment(args)
    app, window = build_window(args)
    midrun_snapshots: list[dict[str, Any]] = []
    threading.Thread(
        target=request_midrun_snapshot,
        args=(app, args, midrun_snapshots),
        daemon=True,
    ).start()
    result = app.run(window)
    final_snapshot = result.get("debug_snapshot")
    if not isinstance(final_snapshot, dict):
        raise RuntimeError("DragonGUI run result did not include a debug snapshot")

    summaries: list[dict[str, Any]] = []
    if midrun_snapshots and "error" not in midrun_snapshots[-1]:
        summaries.append(summarize(midrun_snapshots[-1], args, "midrun"))
    summaries.append(summarize(final_snapshot, args, "final"))

    if args.json:
        print(json.dumps(summaries, indent=2, sort_keys=True))
    else:
        for index, summary in enumerate(summaries):
            if index:
                print()
            print_summary(summary)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
