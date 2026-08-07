from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path


DISTRIBUTIONS = ("structured", "uniform", "clustered", "skewed")


def first_mapping_value(value: object) -> dict[str, object]:
    if not isinstance(value, dict):
        return {}
    for item in value.values():
        if isinstance(item, dict):
            return item
    return {}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", type=int, default=1_000_000)
    parser.add_argument("--render-frames", type=int, default=3)
    parser.add_argument("--seed", type=int, default=20260806)
    parser.add_argument("--package-root", type=Path, default=Path("python"))
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path("artifacts/xy-benchmark/scatter-density-distributions"),
    )
    parser.add_argument("--include-adaptive", action="store_true")
    args = parser.parse_args()

    case_script = Path(__file__).with_name("point_store_case.py")
    args.out_dir.mkdir(parents=True, exist_ok=True)
    modes = ["exact", "density"]
    if args.include_adaptive:
        modes.append("adaptive")

    cases: list[dict[str, object]] = []
    for distribution in DISTRIBUTIONS:
        by_mode: dict[str, dict[str, object]] = {}
        for rendering in modes:
            out = args.out_dir / f"{distribution}-{rendering}-{args.n}.json"
            subprocess.run(
                [
                    sys.executable,
                    str(case_script),
                    "--n",
                    str(args.n),
                    "--mode",
                    "store",
                    "--plots",
                    "1",
                    "--dimensions",
                    "2",
                    "--render-frames",
                    str(args.render_frames),
                    "--rendering",
                    rendering,
                    "--distribution",
                    distribution,
                    "--seed",
                    str(args.seed),
                    "--package-root",
                    str(args.package_root),
                    "--out",
                    str(out),
                ],
                check=True,
                stdout=subprocess.DEVNULL,
            )
            by_mode[rendering] = json.loads(out.read_text(encoding="utf-8"))

        exact_result = by_mode["exact"]["render_result"]
        density_result = by_mode["density"]["render_result"]
        exact_rep = first_mapping_value(exact_result["scatter_representations"])
        density_rep = first_mapping_value(density_result["scatter_representations"])
        exact_gpu = first_mapping_value(exact_result["scatter_gpu"])
        density_gpu = first_mapping_value(density_result["scatter_gpu"])
        exact_frame_ms = float(exact_result["frame_ms"])
        density_frame_ms = float(density_result["frame_ms"])
        exact_gpu_bytes = int(exact_gpu["primary_allocated_bytes"])
        density_gpu_bytes = int(density_gpu["primary_allocated_bytes"])
        density = density_rep.get("density") or {}
        row = {
            "distribution": distribution,
            "source_rows": int(density_rep["source_rows"]),
            "representative_rows": int(density_rep["render_rows"]),
            "reduction_ratio": float(density_rep["reduction_ratio"]),
            "source_rows_conserved": bool(density.get("source_rows_conserved")),
            "max_bin_count": int(density.get("max_bin_count", 0)),
            "density_build_ms": float(density.get("build_ms", 0.0)),
            "exact_frame_ms": exact_frame_ms,
            "density_frame_ms": density_frame_ms,
            "frame_change_percent": (
                (density_frame_ms - exact_frame_ms) / exact_frame_ms * 100.0
            ),
            "exact_gpu_allocated_bytes": exact_gpu_bytes,
            "density_gpu_allocated_bytes": density_gpu_bytes,
            "gpu_allocation_change_percent": (
                (density_gpu_bytes - exact_gpu_bytes) / exact_gpu_bytes * 100.0
            ),
        }
        if "adaptive" in by_mode:
            adaptive_result = by_mode["adaptive"]["render_result"]
            adaptive_rep = first_mapping_value(adaptive_result["scatter_representations"])
            row["adaptive_effective"] = adaptive_rep.get("policy_effective")
            row["adaptive_frame_ms"] = adaptive_result.get("frame_ms")
        cases.append(row)

    payload = {
        "status": "ok"
        if all(case["source_rows_conserved"] for case in cases)
        else "invalid",
        "n": args.n,
        "seed": args.seed,
        "render_frames": args.render_frames,
        "cases": cases,
    }
    summary = args.out_dir / "summary.json"
    summary.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload, indent=2))
    if payload["status"] != "ok":
        raise SystemExit(1)


if __name__ == "__main__":
    main()
