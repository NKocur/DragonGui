"""Exercise retained leaf-primitive success and conservative fallback paths.

Run each case in a separate process because the native event loop is
process-scoped:

    py -3.12 tools/profile_retained_primitive_smoke.py
    py -3.12 tools/profile_retained_primitive_smoke.py --transformed
    py -3.12 tools/profile_retained_primitive_smoke.py --line-plot
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys
import threading
import time


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "python"))

import dragongui as dg  # noqa: E402


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--transformed",
        action="store_true",
        help="Place the updated leaf below a transformed parent to require fallback.",
    )
    mode.add_argument(
        "--line-plot",
        action="store_true",
        help="Update a line-plot axis label to require its dedicated renderer.",
    )
    parser.add_argument("--frames", type=int, default=24)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(max(1, args.frames))
    app = dg.App()
    window = dg.Window("Retained primitive smoke", width=520, height=260)
    panel_style = {"transform": "translateX(4px)"} if args.transformed else None
    with dg.Panel("Transformed parent" if args.transformed else "Stable parent", style=panel_style):
        if args.line_plot:
            widget = dg.LinePlot(
                {"sample": [0.0, 1.0, 2.0], "value": [0.0, 1.0, 0.5]},
                x="sample",
                y="value",
                x_label="before",
                style={"height": 150},
            )
        else:
            widget = dg.Label("before")

    def update() -> None:
        time.sleep(0.2)
        if args.line_plot:
            app.call_soon_threadsafe(lambda: widget.set_axis_labels(x="after"))
        else:
            app.call_soon_threadsafe(lambda: widget.set_value("after"))

    threading.Thread(target=update, name="retained-primitive-smoke", daemon=True).start()
    result = app.run(window)
    snapshot = result["debug_snapshot"]
    primitives = snapshot["gpu"]["renderer"]["primitives"]
    report = {
        "transformed": args.transformed,
        "line_plot": args.line_plot,
        "retained_rebuilds": primitives["retained_rebuilds"],
        "command_text_rebuilds": snapshot["gpu"]["framework"]["command_text_rebuilds"],
        "frame_work_ms_avg": snapshot["runtime"]["frame_work_ms_avg"],
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
