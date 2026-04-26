from __future__ import annotations

import json
import sys
import threading
import time
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33"))
win = dg.Window("DragonGUI Debug Snapshot", width=820, height=460)


def print_result_snapshot(result: dict[str, object]) -> None:
    snapshot = result.get("debug_snapshot")
    if not isinstance(snapshot, dict):
        return
    runtime = snapshot.get("runtime", {})
    gpu = snapshot.get("gpu", {})
    commands = runtime.get("commands") if isinstance(runtime, dict) else {}
    dirty = runtime.get("dirty") if isinstance(runtime, dict) else {}
    print(
        "Final snapshot:",
        json.dumps(
            {
                "frames_rendered": runtime.get("frames_rendered"),
                "frame_ms": runtime.get("frame_ms"),
                "command_history_count": (commands or {}).get("count"),
                "dirty_history_count": len((dirty or {}).get("recent", [])),
                "widget_count": (gpu.get("renderer") or {}).get("widget_count")
                if isinstance(gpu, dict)
                else None,
            },
            indent=2,
        ),
    )


def request_live_snapshot() -> None:
    time.sleep(0.35)
    try:
        amount.set_value(0.7)
        enabled.set_checked(False)
        time.sleep(0.05)
        snapshot = app.debug_snapshot(timeout_ms=1000)
    except RuntimeError:
        return

    runtime = snapshot["runtime"]
    gpu = snapshot["gpu"]
    commands = runtime["commands"]
    recent_commands = commands["recent"]
    last_command = recent_commands[-1] if recent_commands else None
    print(
        "Live snapshot:",
        json.dumps(
            {
                "frames_rendered": runtime["frames_rendered"],
                "queue_depth": runtime["command_queue_depth"],
                "command_history_count": commands["count"],
                "last_command": last_command,
                "dirty_history_count": len(runtime["dirty"]["recent"]),
                "focused": gpu["state"]["focused"],
                "widgets": gpu["renderer"]["widget_count"],
                "window": gpu["window"],
            },
            indent=2,
        ),
    )


with dg.HLayout(style={"padding": 14, "gap": 16}):
    with dg.Panel("Snapshot controls", width=300, style={"padding": 14, "gap": 10}):
        name = dg.TextInput("snapshot-demo", placeholder="Snapshot label", key="name")
        amount = dg.Slider(0.35, min=0.0, max=1.0, step=0.05, key="amount")
        enabled = dg.Checkbox("Include runtime details", checked=True, key="enabled")
        mode = dg.Dropdown(["summary", "layout", "state"], value="summary", key="mode")
        dg.Label("Close for the final snapshot.")

    with dg.Panel("What this validates", style={"padding": 14, "gap": 10}):
        dg.Label("Run result includes debug_snapshot.")
        dg.Label("Background snapshots work while live.")
        dg.Label("Includes layout, state, timings, theme.")
        dg.Label("Avoid snapshot calls from UI callbacks.")


if __name__ == "__main__":
    threading.Thread(target=request_live_snapshot, daemon=True).start()
    print_result_snapshot(app.run(win))
