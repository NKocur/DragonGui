from __future__ import annotations

import argparse
import ast
import json
import os
import statistics
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
PYTHON_DIR = ROOT / "python"

PROBES: tuple[tuple[str, str], ...] = (
    ("selectable_list", "examples/css_feature_probes/selectable_list_probe.py"),
    ("radio_group", "examples/css_feature_probes/radio_group_probe.py"),
    ("tree_view", "examples/css_feature_probes/tree_view_probe.py"),
    ("drag_number", "examples/css_feature_probes/drag_number_probe.py"),
    ("range_slider", "examples/css_feature_probes/range_slider_probe.py"),
    ("splitter", "examples/css_feature_probes/splitter_probe.py"),
    ("tool_buttons", "examples/css_feature_probes/tool_buttons_probe.py"),
    ("property_grid", "examples/css_feature_probes/property_grid_probe.py"),
    ("command_palette", "examples/css_feature_probes/command_palette_probe.py"),
    ("data_table_upgrades", "examples/css_feature_probes/data_table_upgrades_probe.py"),
    ("drag_drop", "examples/css_feature_probes/drag_drop_probe.py"),
    ("toggle_switch", "examples/css_feature_probes/toggle_switch_probe.py"),
    ("date_time_inputs", "examples/css_feature_probes/date_time_inputs_probe.py"),
    ("code_editor", "examples/css_feature_probes/code_editor_probe.py"),
    ("log_view", "examples/css_feature_probes/log_view_probe.py"),
    ("breadcrumbs", "examples/css_feature_probes/breadcrumbs_probe.py"),
    ("toolbar", "examples/css_feature_probes/toolbar_probe.py"),
    ("loading_spinner", "examples/css_feature_probes/loading_spinner_probe.py"),
    ("scatter_plot_2d", "examples/css_feature_probes/scatter_plot_2d_probe.py"),
    ("scatter3d_dense", "examples/css_feature_probes/scatter3d_dense_probe.py"),
    ("heatmap", "examples/css_feature_probes/heatmap_probe.py"),
    ("bar_chart", "examples/css_feature_probes/bar_chart_probe.py"),
    ("layout_flex_stress", "examples/css_feature_probes/layout_flex_stress_probe.py"),
    ("layout_panel_bounds", "examples/css_feature_probes/layout_panel_bounds_probe.py"),
    ("layout_grid_masonry", "examples/css_feature_probes/layout_grid_masonry_probe.py"),
    ("layout_overlay_collision", "examples/css_feature_probes/layout_overlay_collision_probe.py"),
    (
        "layout_scrollable_composites",
        "examples/css_feature_probes/layout_scrollable_composites_probe.py",
    ),
    ("layout_plot_embedding", "examples/css_feature_probes/layout_plot_embedding_probe.py"),
)

QUICK_PROBES = {
    "tool_buttons",
    "property_grid",
    "data_table_upgrades",
    "drag_drop",
    "code_editor",
    "log_view",
    "heatmap",
    "bar_chart",
    "layout_grid_masonry",
    "layout_plot_embedding",
}


def get_path(data: Any, *keys: str) -> Any:
    value = data
    for key in keys:
        if not isinstance(value, dict):
            return None
        value = value.get(key)
    return value


def as_float(value: Any) -> float | None:
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def as_int(value: Any) -> int | None:
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def parse_run_result(stdout: str) -> dict[str, Any]:
    for line in reversed(stdout.splitlines()):
        line = line.strip()
        if not line:
            continue
        if line.startswith("{") and line.endswith("}"):
            parsed = ast.literal_eval(line)
            if isinstance(parsed, dict):
                return parsed
        if line.startswith("Primitive benchmark:"):
            parsed = json.loads(line.split(":", 1)[1].strip())
            if isinstance(parsed, dict):
                return {"status": "ok", "benchmark_summary": parsed}
    raise ValueError("could not find an app.run result in stdout")


def summarize_probe_result(name: str, path: str, result: dict[str, Any], elapsed_s: float) -> dict[str, Any]:
    snapshot = result.get("debug_snapshot")
    runtime = snapshot.get("runtime") if isinstance(snapshot, dict) else {}
    gpu = snapshot.get("gpu") if isinstance(snapshot, dict) else {}
    renderer = gpu.get("renderer") if isinstance(gpu, dict) else {}
    framework = gpu.get("framework") if isinstance(gpu, dict) else {}
    primitives = renderer.get("primitives") if isinstance(renderer, dict) else {}
    resources = gpu.get("resources") if isinstance(gpu, dict) else {}
    computed_styles = gpu.get("computed_styles") if isinstance(gpu, dict) else {}

    line_renderer = renderer.get("line_plot_renderer") if isinstance(renderer, dict) else {}
    heatmap_renderer = renderer.get("heatmap_renderer") if isinstance(renderer, dict) else {}
    scatter = resources.get("scatter") if isinstance(resources, dict) else {}
    line_plot = resources.get("line_plot") if isinstance(resources, dict) else {}

    primitive_batches = (as_int(primitives.get("base_batches")) or 0) + (
        as_int(primitives.get("overlay_batches")) or 0
    )

    return {
        "name": name,
        "path": path,
        "status": result.get("status", "unknown"),
        "elapsed_s": round(elapsed_s, 3),
        "frames_rendered": as_int(runtime.get("frames_rendered")),
        "frame_ms": as_float(runtime.get("frame_ms")),
        "frame_ms_avg": as_float(runtime.get("frame_ms_avg")),
        "last_frame_ms": as_float(runtime.get("last_frame_ms")),
        "cpu_frame_ms": as_float(runtime.get("cpu_frame_ms")),
        "frame_work_ms_avg": as_float(runtime.get("frame_work_ms_avg", runtime.get("frame_work_ms"))),
        "frame_prepare_ms_avg": as_float(
            runtime.get("frame_prepare_ms_avg", runtime.get("frame_prepare_ms"))
        ),
        "frame_encode_ms_avg": as_float(
            runtime.get("frame_encode_ms_avg", runtime.get("frame_encode_ms"))
        ),
        "frame_submit_ms_avg": as_float(
            runtime.get("frame_submit_ms_avg", runtime.get("frame_submit_ms"))
        ),
        "frame_present_ms_avg": as_float(
            runtime.get("frame_present_ms_avg", runtime.get("frame_present_ms"))
        ),
        "wall_fps": as_float(runtime.get("wall_fps")),
        "command_queue_depth": as_int(runtime.get("command_queue_depth")),
        "framework_layout_compute_ms": as_float(
            get_path(framework, "layout_compute", "avg_ms")
        ),
        "framework_text_rebuild_ms": as_float(get_path(framework, "text_rebuild", "avg_ms")),
        "framework_primitive_rebuild_ms": as_float(
            get_path(framework, "primitive_rebuild", "avg_ms")
        ),
        "framework_line_plot_rebuild_ms": as_float(
            get_path(framework, "line_plot_rebuild", "avg_ms")
        ),
        "framework_rebuild_text_ms": as_float(get_path(framework, "rebuild_text", "avg_ms")),
        "framework_rebuild_primitives_ms": as_float(
            get_path(framework, "rebuild_primitives", "avg_ms")
        ),
        "framework_rebuild_visuals_ms": as_float(
            get_path(framework, "rebuild_visuals", "avg_ms")
        ),
        "widget_count": as_int(renderer.get("widget_count")) if isinstance(renderer, dict) else None,
        "computed_style_count": len(computed_styles) if isinstance(computed_styles, dict) else None,
        "primitive_rects": as_int(primitives.get("rect_count")) if isinstance(primitives, dict) else None,
        "primitive_simple": as_int(primitives.get("simple_count")) if isinstance(primitives, dict) else None,
        "primitive_lines": as_int(primitives.get("line_count")) if isinstance(primitives, dict) else None,
        "primitive_complex": as_int(primitives.get("complex_count")) if isinstance(primitives, dict) else None,
        "primitive_batches": primitive_batches if isinstance(primitives, dict) else None,
        "primitive_buffer_kb": round((as_float(primitives.get("buffer_bytes")) or 0.0) / 1000.0, 3)
        if isinstance(primitives, dict)
        else None,
        "primitive_emit_ms": as_float(primitives.get("last_emit_ms")) if isinstance(primitives, dict) else None,
        "primitive_upload_ms": as_float(primitives.get("last_upload_ms"))
        if isinstance(primitives, dict)
        else None,
        "line_renderer_points": as_int(line_renderer.get("point_count"))
        if isinstance(line_renderer, dict)
        else None,
        "line_renderer_segments": as_int(line_renderer.get("segment_count"))
        if isinstance(line_renderer, dict)
        else None,
        "line_renderer_upload_ms": as_float(line_renderer.get("last_upload_ms"))
        if isinstance(line_renderer, dict)
        else None,
        "heatmap_cells": as_int(heatmap_renderer.get("cell_count"))
        if isinstance(heatmap_renderer, dict)
        else None,
        "scatter_points": as_int(scatter.get("last_point_count")) if isinstance(scatter, dict) else None,
        "scatter_updates": as_int(scatter.get("updates")) if isinstance(scatter, dict) else None,
        "scatter_grid_ms": as_float(scatter.get("last_grid_ms")) if isinstance(scatter, dict) else None,
        "scatter_overlay_ms": as_float(scatter.get("last_overlay_ms"))
        if isinstance(scatter, dict)
        else None,
        "scatter_total_native_ms": as_float(scatter.get("last_total_native_ms"))
        if isinstance(scatter, dict)
        else None,
        "scatter_render_encode_ms": as_float(scatter.get("last_render_encode_ms"))
        if isinstance(scatter, dict)
        else None,
        "scatter_render_redraw_ms": as_float(scatter.get("last_render_redraw_ms"))
        if isinstance(scatter, dict)
        else None,
        "scatter_render_composite_ms": as_float(scatter.get("last_render_composite_ms"))
        if isinstance(scatter, dict)
        else None,
        "scatter_render_cache_hit": scatter.get("last_render_cache_hit")
        if isinstance(scatter, dict)
        else None,
        "scatter_render_count": as_int(scatter.get("render_count"))
        if isinstance(scatter, dict)
        else None,
        "scatter_render_cache_hits": as_int(scatter.get("render_cache_hits"))
        if isinstance(scatter, dict)
        else None,
        "scatter_render_cache_hit_rate": as_float(scatter.get("render_cache_hit_rate"))
        if isinstance(scatter, dict)
        else None,
        "scatter_render_encode_avg_ms": as_float(scatter.get("render_encode_avg_ms"))
        if isinstance(scatter, dict)
        else None,
        "scatter_render_redraw_avg_ms": as_float(scatter.get("render_redraw_avg_ms"))
        if isinstance(scatter, dict)
        else None,
        "scatter_render_composite_avg_ms": as_float(scatter.get("render_composite_avg_ms"))
        if isinstance(scatter, dict)
        else None,
        "line_plot_points": as_int(line_plot.get("last_point_count"))
        if isinstance(line_plot, dict)
        else None,
    }


def run_probe(name: str, path: str, frames: int, timeout_s: float) -> dict[str, Any]:
    env = os.environ.copy()
    existing_pythonpath = env.get("PYTHONPATH")
    env["PYTHONPATH"] = (
        str(PYTHON_DIR)
        if not existing_pythonpath
        else os.pathsep.join([str(PYTHON_DIR), existing_pythonpath])
    )
    env["DRAGONGUI_SMOKE_FRAMES"] = str(frames)
    command = [sys.executable, path]
    start = time.perf_counter()
    proc = subprocess.run(
        command,
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
        timeout=timeout_s,
    )
    elapsed_s = time.perf_counter() - start
    if proc.returncode != 0:
        return {
            "name": name,
            "path": path,
            "status": "failed",
            "elapsed_s": round(elapsed_s, 3),
            "returncode": proc.returncode,
            "stderr_tail": proc.stderr[-4000:],
            "stdout_tail": proc.stdout[-4000:],
        }
    try:
        result = parse_run_result(proc.stdout)
    except Exception as exc:  # noqa: BLE001 - benchmark should report parse errors.
        return {
            "name": name,
            "path": path,
            "status": "parse_failed",
            "elapsed_s": round(elapsed_s, 3),
            "error": str(exc),
            "stdout_tail": proc.stdout[-4000:],
        }
    return summarize_probe_result(name, path, result, elapsed_s)


def median_float(values: list[float | None]) -> float | None:
    clean = [value for value in values if value is not None]
    if not clean:
        return None
    return statistics.median(clean)


def combine_repeats(samples: list[dict[str, Any]]) -> dict[str, Any]:
    if len(samples) == 1:
        return samples[0]
    combined = dict(samples[-1])
    combined["samples"] = samples
    combined["repeat_count"] = len(samples)
    for key in (
        "frame_ms",
        "frame_ms_avg",
        "last_frame_ms",
        "cpu_frame_ms",
        "frame_work_ms_avg",
        "frame_prepare_ms_avg",
        "frame_encode_ms_avg",
        "frame_submit_ms_avg",
        "frame_present_ms_avg",
        "wall_fps",
        "primitive_emit_ms",
        "primitive_upload_ms",
        "line_renderer_upload_ms",
        "scatter_grid_ms",
        "scatter_overlay_ms",
        "scatter_total_native_ms",
        "scatter_render_encode_ms",
        "scatter_render_redraw_ms",
        "scatter_render_composite_ms",
        "scatter_render_cache_hit_rate",
        "scatter_render_encode_avg_ms",
        "scatter_render_redraw_avg_ms",
        "scatter_render_composite_avg_ms",
        "framework_layout_compute_ms",
        "framework_text_rebuild_ms",
        "framework_primitive_rebuild_ms",
        "framework_line_plot_rebuild_ms",
        "framework_rebuild_text_ms",
        "framework_rebuild_primitives_ms",
        "framework_rebuild_visuals_ms",
    ):
        combined[key] = median_float([as_float(sample.get(key)) for sample in samples])
    return combined


def selected_probes(args: argparse.Namespace) -> list[tuple[str, str]]:
    probes = list(PROBES)
    if args.quick:
        probes = [probe for probe in probes if probe[0] in QUICK_PROBES]
    if args.probe:
        requested = set(args.probe)
        probes = [probe for probe in probes if probe[0] in requested]
        missing = requested.difference(name for name, _ in probes)
        if missing:
            raise SystemExit(f"unknown probe(s): {', '.join(sorted(missing))}")
    return probes


def format_ms(value: Any) -> str:
    number = as_float(value)
    return "n/a" if number is None else f"{number:7.3f}"


def format_int(value: Any) -> str:
    number = as_int(value)
    return "n/a" if number is None else f"{number:,}"


def print_table(results: list[dict[str, Any]]) -> None:
    print(
        "probe                         frame    work  prepare  encode  prims  complex  widgets  status"
    )
    print(
        "---------------------------- ------- ------- ------- ------- ------ -------- ------- --------"
    )
    for result in sorted(
        results,
        key=lambda item: as_float(item.get("frame_ms_avg", item.get("frame_ms"))) or -1.0,
        reverse=True,
    ):
        print(
            f"{result['name'][:28]:28} "
            f"{format_ms(result.get('frame_ms_avg', result.get('frame_ms')))} "
            f"{format_ms(result.get('frame_work_ms_avg'))} "
            f"{format_ms(result.get('frame_prepare_ms_avg'))} "
            f"{format_ms(result.get('frame_encode_ms_avg'))} "
            f"{format_int(result.get('primitive_rects')):>6} "
            f"{format_int(result.get('primitive_complex')):>8} "
            f"{format_int(result.get('widget_count')):>7} "
            f"{result.get('status')}"
        )


def main() -> int:
    parser = argparse.ArgumentParser(description="Run static v4 widget probe benchmarks.")
    parser.add_argument("--frames", type=int, default=20, help="smoke frames per probe")
    parser.add_argument("--repeat", type=int, default=1, help="repeats per probe, median reported")
    parser.add_argument("--timeout", type=float, default=90.0, help="timeout per probe run in seconds")
    parser.add_argument("--quick", action="store_true", help="run a representative subset")
    parser.add_argument("--probe", action="append", help="run only a named probe; can be repeated")
    parser.add_argument(
        "--json-out",
        default="benchmarking/v4_widget_baseline.json",
        help="path for benchmark JSON output",
    )
    args = parser.parse_args()

    probes = selected_probes(args)
    results: list[dict[str, Any]] = []
    for index, (name, path) in enumerate(probes, start=1):
        print(f"[{index}/{len(probes)}] {name}", flush=True)
        samples = [
            run_probe(name, path, frames=args.frames, timeout_s=args.timeout)
            for _ in range(max(1, args.repeat))
        ]
        results.append(combine_repeats(samples))

    payload = {
        "schema": 1,
        "created_at": datetime.now(timezone.utc).isoformat(),
        "frames": args.frames,
        "repeat": args.repeat,
        "quick": args.quick,
        "results": results,
    }
    out_path = ROOT / args.json_out
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    print_table(results)
    print(f"wrote {out_path.relative_to(ROOT)}")

    failed = [result for result in results if result.get("status") not in {"ok"}]
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
