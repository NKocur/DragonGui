"""Verify line/image renderers initialize after live structural insertion."""

from __future__ import annotations

import argparse
import json
import os
import sys
import threading
from pathlib import Path


class Frame:
    columns = ("x", "y")
    dtypes = ("float32", "float32")

    def __init__(self) -> None:
        self.x = [0.0, 1.0, 2.0]
        self.y = [0.0, 1.0, 0.5]
        self.shape = (3, 2)

    def __getitem__(self, name: str) -> object:
        return getattr(self, name)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--frames", type=int, default=180)
    args = parser.parse_args()

    sys.path.insert(0, str(args.package_root.resolve()))
    import dragongui as dg

    app = dg.App(loading_screen=False)
    window = dg.Window("Lazy optional renderer gate", width=700, height=420)
    with window:
        host = dg.VLayout()
        with host:
            dg.Label("Initial document has no line plot or image")

    def insert_optional_widgets() -> None:
        host.replace_children(
            [
                dg.LinePlot(Frame(), x="x", y="y", parent=None),
                dg.Image("__dragongui_missing_image_probe__.png", parent=None),
            ]
        )

    timer = threading.Timer(0.05, lambda: app.call_soon_threadsafe(insert_optional_widgets))
    timer.daemon = True
    timer.start()
    previous_smoke = os.environ.get("DRAGONGUI_SMOKE_FRAMES")
    os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(max(1, args.frames))
    try:
        result = app.run(window)
    finally:
        timer.cancel()
        if previous_smoke is None:
            os.environ.pop("DRAGONGUI_SMOKE_FRAMES", None)
        else:
            os.environ["DRAGONGUI_SMOKE_FRAMES"] = previous_smoke

    snapshot = result.get("debug_snapshot", {})
    gpu = snapshot.get("gpu", {}) if isinstance(snapshot, dict) else {}
    renderer = gpu.get("renderer", {}) if isinstance(gpu, dict) else {}
    line = renderer.get("line_plot_renderer")
    validation = {
        "line_plot_renderer_ready": isinstance(line, dict),
        "line_plot_series_count": line.get("series_count") if isinstance(line, dict) else None,
        "image_renderer_ready": renderer.get("image_renderer_ready") is True,
    }
    payload = {
        "status": "ok" if all((
            validation["line_plot_renderer_ready"],
            int(validation["line_plot_series_count"] or 0) == 1,
            validation["image_renderer_ready"],
        )) else "invalid",
        "validation": validation,
        "frame_ms": result.get("frame_ms"),
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload))
    if payload["status"] != "ok":
        raise SystemExit(1)


if __name__ == "__main__":
    main()
