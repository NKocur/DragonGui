"""Verify that exact XYZ crosses one device buffer through chunked rendering."""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

class Frame3D:
    columns = ("x", "y", "z")
    dtypes = ("float32", "float32", "float32")

    def __init__(self, x: object, y: object, z: object) -> None:
        self.x = x
        self.y = y
        self.z = z
        self.shape = (len(x), 3)  # type: ignore[arg-type]

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", type=int, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--expected-required-bytes", type=int, required=True)
    parser.add_argument("--expected-limit-bytes", type=int, required=True)
    args = parser.parse_args()

    sys.path.insert(0, str(args.package_root.resolve()))
    import numpy as np
    import dragongui as dg

    x = np.linspace(-1.0, 1.0, args.n, dtype=np.float32)
    y = np.sin(x * np.float32(20.0), dtype=np.float32)
    z = np.zeros(args.n, dtype=np.float32)
    app = dg.App(loading_screen=False)
    window = dg.Window("DragonGUI capacity probe", width=900, height=420)
    with window:
        dg.Scatter3D(
            Frame3D(x, y, z),
            x="x",
            y="y",
            z="z",
            lod=False,
            auto_point_size=False,
            id="capacity-probe-scatter",
        )

    previous_smoke = os.environ.get("DRAGONGUI_SMOKE_FRAMES")
    os.environ["DRAGONGUI_SMOKE_FRAMES"] = "1"
    try:
        try:
            result = app.run(window)
        except dg.ScatterCapacityError as error:
            payload = {
                "status": "invalid",
                "n": args.n,
                "message": f"unexpected ScatterCapacityError: {error}",
                "checks": {},
            }
        else:
            snapshot = result.get("debug_snapshot", {})
            scatter = (
                snapshot.get("gpu", {})
                .get("resources", {})
                .get("scatters", {})
                .get("capacity-probe-scatter", {})
            )
            representation = scatter.get("representation", {})
            checks = {
                "crosses_single_buffer": (
                    args.expected_required_bytes > args.expected_limit_bytes
                ),
                "payload_size": args.n * 12 == args.expected_required_bytes,
                "representation": representation.get("selected")
                == "chunked_xyz_f32_v0",
                "multiple_chunks": int(representation.get("chunk_count", 0)) >= 2,
                "payload_ok": scatter.get("payload_status") == "Ok",
                "all_rows_rendered": scatter.get("effective_draw_point_count") == args.n,
            }
            payload = {
                "status": "ok" if all(checks.values()) else "invalid",
                "n": args.n,
                "required_bytes": args.expected_required_bytes,
                "device_limit_bytes": args.expected_limit_bytes,
                "representation": representation,
                "checks": checks,
            }
    finally:
        if previous_smoke is None:
            os.environ.pop("DRAGONGUI_SMOKE_FRAMES", None)
        else:
            os.environ["DRAGONGUI_SMOKE_FRAMES"] = previous_smoke

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload))
    if payload["status"] != "ok":
        raise SystemExit(1)


if __name__ == "__main__":
    main()
