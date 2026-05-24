from __future__ import annotations

import itertools
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


INITIAL_LINES = [
    "INFO  10:14:03 runtime initialized",
    "DEBUG 10:14:03 loaded stylesheet cache entries=42",
    "INFO  10:14:04 connected to telemetry stream",
    "WARN  10:14:06 sensor latency above target: 23.6 ms",
    "INFO  10:14:07 frame batch submitted rows=16384",
    "ERROR 10:14:08 retry queue exceeded soft limit",
    "INFO  10:14:09 recovered retry queue depth=3",
]


STREAM_MESSAGES = [
    "INFO  frame batch submitted rows=8192",
    "DEBUG present pass glyph_uploads=0 primitive_batches=11",
    "WARN  backpressure detected queue_depth=7",
    "INFO  pipeline idle time recovered",
    "ERROR packet decode failed source=sensor-b",
    "TRACE cache hit line_buffer visible_rows=15",
]


app = dg.App(theme=dg.Theme.dark(accent="#74ddb0", radius=7, focus="#ffd166"))
app.stylesheet(
    """
    Window {
        background: #10141b;
        color: rgba(246, 249, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
    }

    HLayout.content {
        width: 100%;
        flex-grow: 1;
        min-height: 0;
        gap: 12px;
    }

    Panel.case {
        width: calc(50% - 6px);
        min-width: 380px;
        height: 100%;
        min-height: 0;
        background: rgba(22, 31, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        padding: 14px;
        gap: 12px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(246, 249, 255, 0.70);
        line-height: 1.12;
    }

    Label.status {
        width: 100%;
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 8px;
        color: rgba(229, 255, 244, 0.96);
        font-weight: 750;
        padding: 8px 10px;
    }

    HLayout.actions {
        width: 100%;
        height: 36px;
        gap: 8px;
    }

    Button {
        min-width: 104px;
        border-radius: 8px;
        font-weight: 800;
    }

    LogView {
        width: 100%;
        flex-grow: 1;
        min-height: 0;
        background: rgba(5, 9, 14, 0.76);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 9px;
        color: rgba(230, 238, 248, 0.90);
        font-family: "Consolas";
        font-size: 13px;
        line-height: 19px;
        padding-left: 10px;
        padding-right: 10px;
        padding-top: 8px;
        padding-bottom: 8px;
    }

    LogView:focus {
        outline: 2px solid rgba(255, 209, 102, 0.58);
        outline-offset: 2px;
    }

    LogView::line {
        color: rgba(230, 238, 248, 0.88);
    }

    LogView::debug {
        color: rgba(146, 173, 204, 0.82);
    }

    LogView::info {
        color: #74ddb0;
        font-weight: 700;
    }

    LogView::warning {
        color: #ffd166;
        font-weight: 800;
    }

    LogView::error {
        color: #ff5c7a;
        font-weight: 850;
    }

    LogView.snapshot {
        opacity: 0.78;
    }
    """
)

win = dg.Window("LogView probe", width=960, height=560)

counter = itertools.count(10)
stream = itertools.cycle(STREAM_MESSAGES)
state = {"lines": len(INITIAL_LINES)}


def status_text() -> str:
    return f"Buffered lines: {state['lines']}"


with dg.VLayout(class_="root"):
    dg.Label("LogView", class_="title")
    status = dg.Label(status_text(), class_="status")

    with dg.HLayout(class_="content"):
        with dg.Panel("Live stream", class_="case"):
            dg.Label("Append-heavy logs should stay fixed-width, color severity rows, and follow the latest line.", class_="caption")
            live_log = dg.LogView(INITIAL_LINES, follow=True, max_lines=200, rows=12)

            def append_batch() -> None:
                lines = []
                for _ in range(4):
                    tick = next(counter)
                    lines.append(f"{next(stream)} tick={tick:04d}")
                live_log.append_lines(lines)
                state["lines"] = len(live_log.lines)
                status.set_value(status_text())

            def append_error() -> None:
                tick = next(counter)
                live_log.append_line(f"ERROR manual fault injected tick={tick:04d}")
                state["lines"] = len(live_log.lines)
                status.set_value(status_text())

            def clear_log() -> None:
                live_log.clear()
                state["lines"] = 0
                status.set_value(status_text())

            with dg.HLayout(class_="actions"):
                dg.Button("Append batch", on_click=append_batch)
                dg.Button("Add error", on_click=append_error)
                dg.Button("Clear", on_click=clear_log)

        with dg.Panel("Snapshot", class_="case"):
            dg.Label("Follow can be disabled for static diagnostic captures while keeping the same severity styling.", class_="caption")
            dg.LogView(
                [
                    "INFO  capture opened session=v4",
                    "DEBUG renderer=wgpu backend=dx12",
                    "WARN  dropped stale telemetry frame id=2401",
                    "ERROR shader reload failed path=debug_overlay.wgsl",
                    "INFO  capture closed duration=18.4s",
                ],
                follow=False,
                rows=12,
                class_="snapshot",
            )


if __name__ == "__main__":
    print(app.run(win))
