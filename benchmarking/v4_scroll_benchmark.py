from __future__ import annotations

import argparse
import json
import os
import statistics
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from v4_widget_benchmark import as_float, parse_run_result, summarize_probe_result


ROOT = Path(__file__).resolve().parents[1]
PYTHON_DIR = ROOT / "python"
PROBE_NAME = "v4_scroll"
PROBE_PATH = "examples/css_feature_probes/v4_scroll_benchmark_probe.py"


def run_once(args: argparse.Namespace) -> dict[str, Any]:
    env = os.environ.copy()
    existing_pythonpath = env.get("PYTHONPATH")
    env["PYTHONPATH"] = (
        str(PYTHON_DIR)
        if not existing_pythonpath
        else os.pathsep.join([str(PYTHON_DIR), existing_pythonpath])
    )
    env.pop("DRAGONGUI_SMOKE_FRAMES", None)
    env["DRAGONGUI_BENCH_AUTOSTART"] = "1"
    env["DRAGONGUI_BENCH_EXIT_ON_DONE"] = "1"
    env["DRAGONGUI_BENCH_SCROLL_STEPS"] = str(args.steps)
    env["DRAGONGUI_BENCH_SCROLL_INTERVAL_MS"] = str(args.interval_ms)

    start = time.perf_counter()
    proc = subprocess.run(
        [sys.executable, PROBE_PATH],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
        timeout=args.timeout,
    )
    elapsed_s = time.perf_counter() - start
    if proc.returncode != 0:
        return {
            "name": PROBE_NAME,
            "path": PROBE_PATH,
            "status": "failed",
            "elapsed_s": round(elapsed_s, 3),
            "returncode": proc.returncode,
            "stderr_tail": proc.stderr[-4000:],
            "stdout_tail": proc.stdout[-4000:],
        }
    try:
        result = parse_run_result(proc.stdout)
    except Exception as exc:  # noqa: BLE001
        return {
            "name": PROBE_NAME,
            "path": PROBE_PATH,
            "status": "parse_failed",
            "elapsed_s": round(elapsed_s, 3),
            "error": str(exc),
            "stdout_tail": proc.stdout[-4000:],
        }
    return summarize_probe_result(PROBE_NAME, PROBE_PATH, result, elapsed_s)


def median_float(values: list[Any]) -> float | None:
    clean = [float(value) for value in values if as_float(value) is not None]
    if not clean:
        return None
    return statistics.median(clean)


def median_int(values: list[Any]) -> int | None:
    clean = [int(value) for value in values if value is not None]
    if not clean:
        return None
    return int(statistics.median(clean))


def combine(samples: list[dict[str, Any]]) -> dict[str, Any]:
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
        "framework_layout_compute_ms",
        "framework_text_rebuild_ms",
        "framework_primitive_rebuild_ms",
        "framework_line_plot_rebuild_ms",
        "framework_rebuild_visuals_ms",
        "scatter_render_encode_avg_ms",
        "scatter_render_redraw_avg_ms",
        "scatter_render_composite_avg_ms",
        "scatter_render_cache_hit_rate",
    ):
        combined[key] = median_float([sample.get(key) for sample in samples])
    for key in (
        "frames_rendered",
        "widget_count",
        "primitive_rects",
        "primitive_simple",
        "primitive_lines",
        "scatter_render_count",
        "scatter_render_cache_hits",
    ):
        combined[key] = median_int([sample.get(key) for sample in samples])
    return combined


def fmt(value: Any) -> str:
    number = as_float(value)
    return "n/a" if number is None else f"{number:7.3f}"


def main() -> int:
    parser = argparse.ArgumentParser(description="Benchmark V4-like scroll-area redraw cost.")
    parser.add_argument("--steps", type=int, default=120)
    parser.add_argument("--interval-ms", type=float, default=8.0)
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=120.0)
    parser.add_argument(
        "--json-out",
        default="benchmarking/v4_scroll_baseline.json",
        help="path for benchmark JSON output",
    )
    args = parser.parse_args()

    samples = [run_once(args) for _ in range(max(1, args.repeat))]
    result = combine(samples)
    payload = {
        "schema": 1,
        "created_at": datetime.now(timezone.utc).isoformat(),
        "steps": args.steps,
        "interval_ms": args.interval_ms,
        "repeat": args.repeat,
        "result": result,
    }
    out_path = ROOT / args.json_out
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")

    print(
        "probe        work  prepare  encode  layout  text  prims  sc_avg  cache%  frames status"
    )
    print(
        "----------- ------- ------- ------- ------- ------ ------ ------- ------- ------ ------"
    )
    print(
        f"{PROBE_NAME[:11]:11} "
        f"{fmt(result.get('frame_work_ms_avg'))} "
        f"{fmt(result.get('frame_prepare_ms_avg'))} "
        f"{fmt(result.get('frame_encode_ms_avg'))} "
        f"{fmt(result.get('framework_layout_compute_ms'))} "
        f"{fmt(result.get('framework_text_rebuild_ms'))} "
        f"{fmt(result.get('framework_primitive_rebuild_ms'))} "
        f"{fmt(result.get('scatter_render_encode_avg_ms'))} "
        f"{fmt((result.get('scatter_render_cache_hit_rate') or 0.0) * 100.0)} "
        f"{result.get('frames_rendered')!s:>6} "
        f"{result.get('status')}"
    )
    print(f"wrote {out_path.relative_to(ROOT)}")
    return 0 if result.get("status") == "ok" else 1


if __name__ == "__main__":
    raise SystemExit(main())
