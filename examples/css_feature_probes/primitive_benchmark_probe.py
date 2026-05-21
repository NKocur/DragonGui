from __future__ import annotations

import json
import os
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


CELL_COUNT = max(1, int(os.environ.get("DRAGONGUI_PRIMITIVE_BENCH_CELLS", "3600")))
CELL_SIZE = max(2, int(os.environ.get("DRAGONGUI_PRIMITIVE_BENCH_CELL_SIZE", "14")))
CELL_GAP = max(0, int(os.environ.get("DRAGONGUI_PRIMITIVE_BENCH_GAP", "2")))
MODE = os.environ.get("DRAGONGUI_PRIMITIVE_BENCH_MODE", "rounded").strip().lower()
if MODE not in {"solid", "rounded", "outline", "complex"}:
    raise SystemExit(
        "DRAGONGUI_PRIMITIVE_BENCH_MODE must be solid, rounded, outline, or complex"
    )

app = dg.App(theme=dg.Theme.dark(accent="#6ea8fe", radius=6))
win = dg.Window("DragonGUI Primitive Benchmark", width=1280, height=900)

app.stylesheet(
    f"""
    Window {{
        background: #101318;
        padding: 12px;
    }}

    FlowLayout.bench {{
        width: 100%;
        height: 100%;
        gap: {CELL_GAP}px;
        row-gap: {CELL_GAP}px;
        align-items: flex-start;
        overflow: hidden;
    }}

    Panel.cell {{
        width: {CELL_SIZE}px;
        height: {CELL_SIZE}px;
        min-width: {CELL_SIZE}px;
        min-height: {CELL_SIZE}px;
        max-width: {CELL_SIZE}px;
        max-height: {CELL_SIZE}px;
        padding: 0;
        gap: 0;
    }}

    Panel.solid {{
        background: rgba(98, 164, 255, 0.92);
        border: 0 solid transparent;
        border-radius: 0;
    }}

    Panel.rounded {{
        background: rgba(98, 164, 255, 0.88);
        border: 0 solid transparent;
        border-radius: 5px;
    }}

    Panel.outline {{
        background: rgba(98, 164, 255, 0.72);
        border: 1px solid rgba(214, 231, 255, 0.62);
        border-radius: 5px;
    }}

    Panel.complex {{
        background: linear-gradient(135deg, #62a4ff 0%, #78ddad 48%, #ffd166 100%);
        border: 1px solid rgba(255, 255, 255, 0.42);
        border-radius: 6px;
        box-shadow: 0 2px 7px rgba(0, 0, 0, 0.28);
    }}
    """
)

with dg.FlowLayout(class_="bench", gap=CELL_GAP, row_gap=CELL_GAP):
    for index in range(CELL_COUNT):
        klass = MODE
        if MODE == "outline" and index % 5 == 0:
            klass = "rounded"
        elif MODE == "complex" and index % 4 == 0:
            klass = "rounded"
        dg.Panel(None, class_=f"cell {klass}")


def benchmark_summary(result: dict[str, object]) -> dict[str, object]:
    snapshot = result.get("debug_snapshot")
    if not isinstance(snapshot, dict):
        return {"error": "missing debug snapshot"}
    runtime = snapshot.get("runtime")
    gpu = snapshot.get("gpu")
    renderer = gpu.get("renderer") if isinstance(gpu, dict) else None
    primitives = renderer.get("primitives") if isinstance(renderer, dict) else None
    return {
        "mode": MODE,
        "cells": CELL_COUNT,
        "cell_size": CELL_SIZE,
        "gap": CELL_GAP,
        "split_env": os.environ.get("DRAGONGUI_PRIMITIVE_SPLIT", "default"),
        "frames_rendered": runtime.get("frames_rendered") if isinstance(runtime, dict) else None,
        "frame_ms_avg": round(float(runtime.get("frame_ms") or 0.0), 4)
        if isinstance(runtime, dict)
        else None,
        "last_frame_ms": round(float(runtime.get("last_frame_ms") or 0.0), 4)
        if isinstance(runtime, dict)
        else None,
        "frame_encode_ms": round(float(runtime.get("frame_encode_ms") or 0.0), 4)
        if isinstance(runtime, dict)
        else None,
        "primitive": primitives if isinstance(primitives, dict) else None,
        "widget_count": renderer.get("widget_count") if isinstance(renderer, dict) else None,
    }


if __name__ == "__main__":
    result = app.run(win)
    print(
        "Primitive benchmark:",
        json.dumps(benchmark_summary(result), sort_keys=True),
        flush=True,
    )
