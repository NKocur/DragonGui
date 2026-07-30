"""Profile isolated DragonGUI subsystems with comparable fresh-process reports.

Examples:

    py -3.12 tools/profile_subsystem_stress.py --workload text --frames 20
    py -3.12 tools/profile_subsystem_stress.py --matrix --frames 20

Each workload intentionally emphasizes one library subsystem while retaining a
small amount of normal window, layout, and theme overhead. Matrix mode launches
one child process per workload because the native event loop is process-scoped.
"""

from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import time
from typing import Any, Callable


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "python"))

import dragongui as dg  # noqa: E402

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - profiling dependency
    raise SystemExit("profile_subsystem_stress.py requires NumPy") from exc


WORKLOADS = ("css", "text", "primitives", "table", "scatter")
DEFAULT_SIZES = {
    "css": 1_600,
    "text": 1_600,
    "primitives": 3_600,
    "table": 20_000,
    "scatter": 125_000,
}


class ColumnFrame:
    def __init__(self, dtypes: tuple[str, ...], **columns: Any) -> None:
        self.columns = tuple(columns)
        self.dtypes = dtypes
        self.shape = (len(next(iter(columns.values()), ())), len(columns))
        self._columns = columns

    def __getitem__(self, column: str) -> Any:
        return self._columns[column]


def _base_app(title: str) -> tuple[dg.App, dg.Window]:
    app = dg.App(theme=dg.Theme.dark(accent="#6ea8fe", radius=6))
    window = dg.Window(title, width=1280, height=900)
    app.stylesheet(
        """
        Window {
            background: #0f141d;
            color: #edf3ff;
            padding: 10px;
            gap: 6px;
            font-size: 13px;
        }
        FlowLayout.bench {
            width: 100%;
            height: 100%;
            align-items: flex-start;
            align-content: flex-start;
            overflow: hidden;
        }
        """
    )
    return app, window


def _build_css(size: int) -> tuple[dg.App, dg.Window, dict[str, Any]]:
    app, window = _base_app("DragonGUI CSS subsystem profile")
    buckets = 200
    rules = [
        f"""
        Label.css-item.bucket-{index} {{
            color: rgba({80 + index % 120}, {120 + index % 100}, 220, 0.92);
            background: rgba(110, 168, 254, {0.03 + (index % 5) * 0.01:.2f});
            border: 1px solid rgba(110, 168, 254, 0.12);
            border-radius: {index % 7}px;
            padding: 2px 4px;
        }}
        """
        for index in range(buckets)
    ]
    rules.extend(
        f"""
        FlowLayout.bench > Label.css-item:nth-child({stride}n + {offset}) {{
            font-weight: {400 + (offset % 5) * 100};
            opacity: {0.74 + (offset % 4) * 0.06:.2f};
        }}
        """
        for stride in range(3, 13)
        for offset in range(1, min(stride, 5) + 1)
    )
    app.stylesheet("\n".join(rules))
    with window:
        with dg.FlowLayout(class_="bench", gap=2, row_gap=2):
            for index in range(size):
                dg.Label(
                    f"css {index:04d}",
                    class_=f"css-item bucket-{index % buckets}",
                    wrap=False,
                    style={"width": 74, "height": 18},
                )
    return app, window, {"rules_generated": len(rules), "buckets": buckets}


def _build_text(size: int) -> tuple[dg.App, dg.Window, dict[str, Any]]:
    app, window = _base_app("DragonGUI text subsystem profile")
    app.stylesheet(
        """
        Label.text-item {
            width: 148px;
            height: 38px;
            padding: 2px 4px;
            color: rgba(238, 244, 255, 0.94);
            font-size: 11px;
            line-height: 1.08;
        }
        Label.mono { font-family: "Consolas"; }
        Label.emphasis { font-weight: 750; color: #8ce4bb; }
        """
    )
    samples = (
        "Telemetry αβγ channel pressure",
        "日本語 shaping • café • naïve",
        "AVATAR Toffee ffi office 012345",
        "Wrapped diagnostic message with proportional glyphs",
    )
    with window:
        with dg.FlowLayout(class_="bench", gap=2, row_gap=2):
            for index in range(size):
                classes = ["text-item"]
                if index % 3 == 0:
                    classes.append("mono")
                if index % 7 == 0:
                    classes.append("emphasis")
                dg.Label(
                    f"{index:04d} {samples[index % len(samples)]}",
                    class_=" ".join(classes),
                    wrap=True,
                )
    return app, window, {"sample_variants": len(samples), "wrapped": True}


def _build_primitives(size: int) -> tuple[dg.App, dg.Window, dict[str, Any]]:
    app, window = _base_app("DragonGUI primitive subsystem profile")
    app.stylesheet(
        """
        Panel.primitive-cell {
            width: 14px;
            height: 14px;
            min-width: 14px;
            min-height: 14px;
            max-width: 14px;
            max-height: 14px;
            padding: 0;
            gap: 0;
            border: 0 solid transparent;
        }
        Panel.simple {
            background: rgba(98, 164, 255, 0.88);
            border-radius: 4px;
        }
        Panel.outline {
            background: rgba(116, 221, 176, 0.58);
            border: 1px solid rgba(220, 255, 240, 0.52);
            border-radius: 5px;
        }
        Panel.complex {
            background: linear-gradient(135deg, #62a4ff, #78ddad, #ffd166);
            border: 1px solid rgba(255, 255, 255, 0.38);
            border-radius: 6px;
            box-shadow: 0 2px 7px rgba(0, 0, 0, 0.28);
        }
        """
    )
    kinds = ("simple", "simple", "outline", "complex")
    with window:
        with dg.FlowLayout(class_="bench", gap=2, row_gap=2):
            for index in range(size):
                dg.Panel(None, class_=f"primitive-cell {kinds[index % len(kinds)]}")
    return app, window, {"mix": dict(simple=2, outline=1, complex=1)}


def _build_table(size: int) -> tuple[dg.App, dg.Window, dict[str, Any]]:
    app, window = _base_app("DragonGUI table subsystem profile")
    app.stylesheet(
        """
        DataFrameTable.table-bench {
            width: 100%;
            height: 100%;
            min-height: 0;
        }
        """
    )
    row = np.arange(size, dtype=np.int32)
    frame = ColumnFrame(
        ("int32", "str", "float32", "float32", "str", "str"),
        row=row,
        metric=np.array([f"metric.channel.{i:06d}" for i in range(size)], dtype=object),
        value=(np.sin(row * 0.017) * 50.0 + 50.0).astype(np.float32),
        delta=(np.cos(row * 0.031) * 8.0).astype(np.float32),
        state=np.array(("ok", "warn", "busy", "error"), dtype=object)[row % 4],
        owner=np.array([f"team-{i % 17:02d}" for i in range(size)], dtype=object),
    )
    with window:
        dg.DataFrameTable(
            frame,
            page_size=48,
            sample_rows=min(size, 4_096),
            sortable=True,
            resizable_columns=True,
            class_="table-bench",
        )
    return app, window, {"columns": len(frame.columns), "sample_rows": min(size, 4_096)}


def _build_scatter(size: int) -> tuple[dg.App, dg.Window, dict[str, Any]]:
    app, window = _base_app("DragonGUI scatter subsystem profile")
    app.stylesheet(
        """
        Scatter3D.scatter-bench {
            width: 100%;
            height: 100%;
            min-width: 0;
            min-height: 0;
            background: #07101a;
            border: 1px solid rgba(110, 168, 254, 0.32);
            border-radius: 8px;
        }
        """
    )
    t = np.linspace(0.0, 1.0, size, dtype=np.float32)
    theta = t * np.float32(math.tau * 23.0)
    radius = np.float32(0.2) + t * np.float32(3.8)
    frame = ColumnFrame(
        ("float32", "float32", "float32", "float32"),
        x=np.cos(theta) * radius,
        y=np.sin(theta * np.float32(1.07)) * radius,
        z=np.sin(theta * np.float32(0.31)) * np.float32(2.5),
        scalar=(np.sin(theta * np.float32(0.17)) + 1.0) * np.float32(0.5),
    )
    with window:
        dg.Scatter3D(
            frame,
            x="x",
            y="y",
            z="z",
            scalars="scalar",
            colormap="turbo",
            scalar_bar=True,
            grid=True,
            major_planes=True,
            point_size=2.4,
            class_="scatter-bench",
        )
    return app, window, {"columns": len(frame.columns), "scalar_bar": True}


BUILDERS: dict[
    str, Callable[[int], tuple[dg.App, dg.Window, dict[str, Any]]]
] = {
    "css": _build_css,
    "text": _build_text,
    "primitives": _build_primitives,
    "table": _build_table,
    "scatter": _build_scatter,
}


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
    return dict(sorted(timings, key=_timing_total, reverse=True)[:limit])


def _compact_report(
    result: dict[str, Any],
    *,
    workload: str,
    size: int,
    frames: int,
    metadata: dict[str, Any],
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
    resources = gpu.get("resources") or {}
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
        "startup_readiness",
    )
    return {
        "profile_schema": 1,
        "profile_kind": "isolated-subsystem",
        "workload": {
            "name": workload,
            "size": size,
            "frames": frames,
            "widget_count": renderer.get("widget_count"),
            **metadata,
        },
        "python": {
            "build_app_ms": build_ms,
            "document_serialize_ms": serialize_ms,
            "document_json_bytes": document_bytes,
            "run_wall_ms": run_ms,
        },
        "runtime": {key: runtime.get(key) for key in runtime_keys},
        "frame_timings": runtime.get("frame_timings"),
        "loading_screen": runtime.get("loading_screen"),
        "command_drain": runtime.get("command_drain"),
        "top_command_timings": _top_timings(runtime.get("command_timings")),
        "top_framework_timings": _top_timings(framework),
        "stylesheets": {
            "framework_rules": stylesheets.get("framework_rules"),
            "theme_rules": stylesheets.get("theme_rules"),
            "user_rules": stylesheets.get("user_rules"),
            "warning_count": stylesheets.get("warning_count"),
            "last_cascade": stylesheets.get("last_cascade"),
        },
        "renderer": {
            "primitives": renderer.get("primitives"),
            "line_plot_renderer": renderer.get("line_plot_renderer"),
            "text": renderer.get("text"),
            "layout_text_measurement": renderer.get("layout_text_measurement"),
            "scatter_count": renderer.get("scatter_count"),
        },
        "resources": {
            "scatter": resources.get("scatter"),
            "scatters": resources.get("scatters"),
            "line_plot": resources.get("line_plot"),
            "line_plots": resources.get("line_plots"),
            "tables": resources.get("tables"),
            "buffers": resources.get("buffers"),
        },
    }


def _value(report: dict[str, Any], *path: str, default: Any = 0) -> Any:
    value: Any = report
    for key in path:
        if not isinstance(value, dict):
            return default
        value = value.get(key)
    return default if value is None else value


def _matrix_markdown(reports: list[dict[str, Any]]) -> str:
    lines = [
        "# DragonGUI isolated subsystem profile",
        "",
        "| Workload | Size | Widgets | Build ms | Serialize ms | Style ms | Layout ms | Text ms | Primitive ms | CPU work ms |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for report in reports:
        workload = report["workload"]
        framework = report.get("top_framework_timings") or {}
        lines.append(
            "| {name} | {size:,} | {widgets:,} | {build:.2f} | {serialize:.2f} | "
            "{style:.2f} | {layout:.2f} | {text:.2f} | {primitive:.2f} | {work:.2f} |".format(
                name=workload["name"],
                size=int(workload["size"]),
                widgets=int(workload.get("widget_count") or 0),
                build=float(_value(report, "python", "build_app_ms")),
                serialize=float(_value(report, "python", "document_serialize_ms")),
                style=float(_value(framework, "style_reapply", "total_ms")),
                layout=float(_value(framework, "layout_compute", "total_ms")),
                text=float(_value(framework, "text_rebuild", "total_ms")),
                primitive=float(_value(framework, "primitive_rebuild", "total_ms")),
                work=float(_value(report, "runtime", "frame_work_ms_avg")),
            )
        )
    lines.append("")
    return "\n".join(lines)


def _run_matrix(args: argparse.Namespace) -> int:
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    reports = []
    for workload in WORKLOADS:
        output = output_dir / f"{workload}.json"
        command = [
            sys.executable,
            str(Path(__file__).resolve()),
            "--workload",
            workload,
            "--frames",
            str(args.frames),
            "--output",
            str(output),
            "--quiet",
        ]
        subprocess.run(command, cwd=ROOT, check=True)
        reports.append(json.loads(output.read_text(encoding="utf-8")))
    summary = {
        "profile_schema": 1,
        "profile_kind": "isolated-subsystem-matrix",
        "frames": args.frames,
        "reports": reports,
    }
    (output_dir / "matrix-summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    markdown = _matrix_markdown(reports)
    (output_dir / "matrix-summary.md").write_text(markdown, encoding="utf-8")
    print(markdown)
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workload", choices=WORKLOADS, default="css")
    parser.add_argument("--size", type=int)
    parser.add_argument("--frames", type=int, default=20)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--quiet", action="store_true")
    parser.add_argument("--matrix", action="store_true")
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=ROOT / "artifacts" / "performance" / "subsystem-matrix",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    args.frames = max(1, args.frames)
    if args.matrix:
        return _run_matrix(args)

    size = max(1, args.size or DEFAULT_SIZES[args.workload])
    os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(args.frames)

    started = time.perf_counter()
    app, window, metadata = BUILDERS[args.workload](size)
    build_ms = (time.perf_counter() - started) * 1000.0

    started = time.perf_counter()
    document = app.document(window, include_startup_resource_payloads=False)
    serialize_ms = (time.perf_counter() - started) * 1000.0
    document_bytes = len(json.dumps(document, separators=(",", ":")).encode("utf-8"))

    started = time.perf_counter()
    result = app.run(window)
    run_ms = (time.perf_counter() - started) * 1000.0
    report = _compact_report(
        result,
        workload=args.workload,
        size=size,
        frames=args.frames,
        metadata=metadata,
        build_ms=build_ms,
        serialize_ms=serialize_ms,
        document_bytes=document_bytes,
        run_ms=run_ms,
    )
    payload = json.dumps(report, indent=2, sort_keys=True)
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
    if not args.quiet:
        print(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
