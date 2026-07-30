from __future__ import annotations

import struct
import sys
import zlib
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


def _png_chunk(kind: bytes, payload: bytes) -> bytes:
    body = kind + payload
    return struct.pack(">I", len(payload)) + body + struct.pack(">I", zlib.crc32(body))


def _probe_texture(width: int = 24, height: int = 16, *, warm: bool = False) -> bytes:
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            stripe = ((x // 4) + (y // 4)) % 2
            glow = max(0, 70 - abs(x - width // 2) * 6)
            rows.extend(
                (
                    (68 + glow + stripe * 22) if warm else (20 + glow // 4),
                    46 + stripe * 24,
                    (20 + glow // 4) if warm else (68 + glow + stripe * 22),
                    255,
                )
            )
    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + _png_chunk(b"IHDR", header)
        + _png_chunk(b"IDAT", zlib.compress(bytes(rows), 9))
        + _png_chunk(b"IEND", b"")
    )


app = dg.App(theme=dg.Theme.dark(accent="#7dc8ff", radius=12))
app.set_image_resource("probe-texture", _probe_texture())
app.stylesheet(
    """
    Window {
        background: #0b111b;
        color: #f4f8ff;
        padding: 20px;
        gap: 14px;
        font-size: 14px;
    }

    Label.title {
        color: #7dc8ff;
        font-size: 21px;
        font-weight: 800;
    }

    Label.caption, Label.card-copy { color: rgba(238, 246, 255, 0.82); }

    Panel.image-card {
        width: 340px;
        height: 154px;
        padding: 16px;
        gap: 8px;
        background-color: #152235;
        border: 2px solid rgba(180, 220, 255, 0.30);
        border-radius: 17px;
        box-shadow: 0 14px 30px rgba(0, 0, 0, 0.30);
    }

    Panel.contain { background-image: app-resource("probe-texture", contain); }
    Panel.cover { background-image: app-resource("probe-texture", cover); }
    Panel.stretch { background-image: app-resource("probe-texture", stretch); opacity: 0.92; }
    Panel.repeat { background-image: app-resource("probe-texture", repeat); }

    Label.card-title {
        color: white;
        font-size: 17px;
        font-weight: 800;
    }

    Button {
        width: 112px;
        height: 32px;
        background: rgba(8, 17, 28, 0.82);
        border: 1px solid rgba(255, 255, 255, 0.32);
    }
    """
)

win = dg.Window("Managed Background Image Probe", width=760, height=470)


def replace_texture() -> None:
    app.set_image_resource("probe-texture", _probe_texture(warm=True))


def release_texture() -> None:
    app.release_image_resource("probe-texture")

with dg.VLayout(style={"gap": 14}):
    dg.Label("Managed application backgrounds", class_="title")
    dg.Label(
        "Packaged PNG bytes, safe resource IDs, four fit modes, clipped borders, "
        "and child controls painted above their panel texture.",
        class_="caption",
    )

    for left_fit, right_fit in (("contain", "cover"), ("stretch", "repeat")):
        with dg.HLayout(style={"gap": 14}):
            for fit in (left_fit, right_fit):
                with dg.Panel(class_=f"image-card {fit}"):
                    dg.Label(fit, class_="card-title")
                    dg.Label(
                        "Text and controls remain visible above the managed image.",
                        class_="card-copy",
                    )
                    if fit == "contain":
                        dg.Button(
                            "Replace live",
                            id="replace-resource",
                            on_click=replace_texture,
                        )
                    elif fit == "cover":
                        dg.Button(
                            "Release live",
                            id="release-resource",
                            on_click=release_texture,
                        )
                    else:
                        dg.Button("Child control")


if __name__ == "__main__":
    print(app.run(win))
