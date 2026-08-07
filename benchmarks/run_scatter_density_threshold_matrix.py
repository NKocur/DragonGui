from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path


def first_mapping_value(value: object) -> dict[str, object]:
    if isinstance(value, dict):
        return next((item for item in value.values() if isinstance(item, dict)), {})
    return {}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--sizes", type=int, nargs="+", default=[100_000, 250_000, 500_000, 1_000_000])
    parser.add_argument("--render-frames", type=int, default=30)
    parser.add_argument("--distribution", default="uniform", choices=("structured", "uniform", "clustered", "skewed"))
    parser.add_argument("--seed", type=int, default=20260806)
    parser.add_argument("--package-root", type=Path, default=Path("python"))
    parser.add_argument("--out-dir", type=Path, default=Path("artifacts/xy-benchmark/scatter-density-thresholds"))
    args = parser.parse_args()

    case_script = Path(__file__).with_name("point_store_case.py")
    args.out_dir.mkdir(parents=True, exist_ok=True)
    cases: list[dict[str, object]] = []
    for n in args.sizes:
        results: dict[str, dict[str, object]] = {}
        for rendering in ("exact", "density"):
            out = args.out_dir / f"{args.distribution}-{rendering}-{n}.json"
            subprocess.run(
                [
                    sys.executable, str(case_script),
                    "--n", str(n), "--mode", "store", "--plots", "1",
                    "--dimensions", "2", "--render-frames", str(args.render_frames),
                    "--rendering", rendering, "--distribution", args.distribution,
                    "--seed", str(args.seed), "--package-root", str(args.package_root),
                    "--out", str(out),
                ],
                check=True,
                stdout=subprocess.DEVNULL,
            )
            results[rendering] = json.loads(out.read_text(encoding="utf-8"))

        exact_result = results["exact"]["render_result"]
        density_result = results["density"]["render_result"]
        exact_rep = first_mapping_value(exact_result["scatter_representations"])
        density_rep = first_mapping_value(density_result["scatter_representations"])
        exact_gpu = first_mapping_value(exact_result["scatter_gpu"])
        density_gpu = first_mapping_value(density_result["scatter_gpu"])
        density = density_rep.get("density") or {}
        exact_frame_ms = float(exact_result["frame_ms"])
        density_frame_ms = float(density_result["frame_ms"])
        cases.append({
            "source_rows": n,
            "representative_rows": int(density_rep["render_rows"]),
            "reduction_ratio": float(density_rep["reduction_ratio"]),
            "source_rows_conserved": bool(density.get("source_rows_conserved")),
            "density_build_ms": float(density.get("build_ms", 0.0)),
            "exact_frame_ms": exact_frame_ms,
            "density_frame_ms": density_frame_ms,
            "frame_change_percent": (density_frame_ms - exact_frame_ms) / exact_frame_ms * 100.0,
            "exact_gpu_allocated_bytes": int(exact_gpu["primary_allocated_bytes"]),
            "density_gpu_allocated_bytes": int(density_gpu["primary_allocated_bytes"]),
        })

    payload = {
        "status": "ok" if all(case["source_rows_conserved"] for case in cases) else "invalid",
        "distribution": args.distribution,
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
