from __future__ import annotations

import struct
import sys
import tempfile
import zlib
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


def _png_chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)


def _probe_icon_path() -> str:
    path = Path(tempfile.gettempdir()) / "dragongui_tool_button_icon.png"
    if path.exists():
        return str(path)

    width = 20
    height = 20
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            edge = x in (0, width - 1) or y in (0, height - 1)
            diag = abs(x - y) <= 1 or abs((width - 1 - x) - y) <= 1
            if edge:
                rows.extend((255, 255, 255, 255))
            elif diag:
                rows.extend((255, 209, 102, 255))
            else:
                rows.extend((54, 126, 235, 255))

    data = (
        b"\x89PNG\r\n\x1a\n"
        + _png_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + _png_chunk(b"IDAT", zlib.compress(bytes(rows), 9))
        + _png_chunk(b"IEND", b"")
    )
    path.write_bytes(data)
    return str(path)


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=7, focus="#ffd166"))
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

    Panel.case {
        width: 100%;
        background: rgba(22, 31, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        padding: 14px;
        gap: 12px;
    }

    HLayout.toolbar {
        gap: 8px;
        height: 38px;
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
        background: rgba(90, 169, 255, 0.12);
        border: 1px solid rgba(90, 169, 255, 0.34);
        border-radius: 8px;
        color: rgba(232, 244, 255, 0.96);
        font-weight: 750;
        padding: 8px 10px;
        width: 100%;
    }

    IconButton,
    ImageButton,
    ArrowButton {
        width: 32px;
        height: 32px;
        border-radius: 7px;
        padding: 5px;
    }

    IconButton::icon,
    ArrowButton::icon {
        color: rgba(247, 250, 255, 0.92);
    }

    IconButton.warning::icon {
        color: #ffd166;
    }

    SmallButton {
        height: 28px;
        border-radius: 7px;
        padding-left: 10px;
        padding-right: 10px;
        font-size: 13px;
    }

    IconButton:disabled,
    ImageButton:disabled,
    ArrowButton:disabled,
    SmallButton:disabled {
        opacity: 0.48;
    }
    """
)

win = dg.Window("Tool buttons probe", width=820, height=390)


with dg.VLayout(class_="root"):
    dg.Label("Tool buttons", class_="title")
    status = dg.Label("Ready", class_="status")

    def mark(action: str) -> None:
        status.set_value(f"Clicked: {action}")

    with dg.Panel("IconButton", class_="case"):
        dg.Label("Compact square controls keep a stable size regardless of icon name.", class_="caption")
        with dg.HLayout(class_="toolbar"):
            dg.IconButton("play", tooltip="Run", on_click=lambda: mark("play"))
            dg.IconButton("pause", tooltip="Pause", on_click=lambda: mark("pause"))
            dg.IconButton("stop", tooltip="Stop", on_click=lambda: mark("stop"))
            dg.IconButton("save", tooltip="Save", on_click=lambda: mark("save"))
            dg.IconButton("search", tooltip="Search", on_click=lambda: mark("search"))
            dg.IconButton("plus", tooltip="Add", on_click=lambda: mark("add"))
            dg.IconButton("close", tooltip="Close", class_="warning", on_click=lambda: mark("close"))
            dg.IconButton("play", disabled=True)

    with dg.Panel("ImageButton, ArrowButton, SmallButton", class_="case"):
        with dg.HLayout(class_="toolbar"):
            dg.ImageButton(_probe_icon_path(), tooltip="Image action", on_click=lambda: mark("image"))
            dg.ArrowButton("left", tooltip="Previous", on_click=lambda: mark("previous"))
            dg.ArrowButton("right", tooltip="Next", on_click=lambda: mark("next"))
            dg.ArrowButton("up", tooltip="Move up", on_click=lambda: mark("up"))
            dg.ArrowButton("down", tooltip="Move down", on_click=lambda: mark("down"))
            dg.SmallButton("Reset", on_click=lambda: mark("reset"))
            dg.SmallButton("Disabled", disabled=True)

    dg.Label("PASS: tool buttons render compactly, focus/hover/active/disabled states work, and callbacks fire.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
