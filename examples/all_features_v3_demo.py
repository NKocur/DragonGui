from __future__ import annotations

import math
from pathlib import Path
from pprint import pprint
import struct
import sys
import threading
import tempfile
import time
import zlib

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual demo guard
    raise SystemExit("all_features_v3_demo.py requires NumPy") from exc


POINT_ROWS = 125_000
TABLE_ROWS = 50_000
STREAM_FRAME_COUNT = 24
GRID_GAP = 12
GRID_STYLE = {"padding": 10, "align_items": "start", "flex_grow": 0, "flex_shrink": 1}
CARD_STYLE = {
    "padding": 10,
    "gap": 8,
    "font_size": 14,
    "line_height": "18px",
    "flex_grow": 0,
    "flex_shrink": 1,
    "align_self": "start",
}
SCATTER_GRID_STYLE = {
    "padding": 10,
    "gap": GRID_GAP,
    "align_items": "stretch",
    "flex_grow": 1,
    "flex_shrink": 1,
    "min_height": 0,
    "overflow_y": "hidden",
}
SCATTER_CONTROLS_PANEL_STYLE = {
    "padding": 10,
    "gap": 0,
    "font_size": 14,
    "line_height": "18px",
    "flex_grow": 1,
    "flex_shrink": 1,
    "align_self": "stretch",
    "min_height": 0,
    "overflow_y": "hidden",
}
SCATTER_CONTROLS_SCROLL_STYLE = {
    "padding_bottom": 26,
    "gap": 8,
    "font_size": 14,
    "line_height": "18px",
    "flex_grow": 1,
    "flex_shrink": 1,
    "min_height": 0,
}
DEBUG_MONITOR_STYLE = {
    "height": 540,
    "min_height": 0,
    "flex_grow": 1,
    "flex_shrink": 1,
    "align_self": "stretch",
}


def make_demo_image() -> str:
    width, height = 220, 132
    rows = []
    for y in range(height):
        scanline = bytearray([0])
        for x in range(width):
            wave = math.sin((x / width) * math.tau * 3.0 + (y / height) * math.tau)
            r = int(32 + 130 * x / max(1, width - 1) + 36 * max(0.0, wave))
            g = int(64 + 120 * y / max(1, height - 1))
            b = int(210 - 90 * x / max(1, width - 1) + 30 * max(0.0, -wave))
            scanline.extend((max(0, min(255, r)), max(0, min(255, g)), max(0, min(255, b)), 255))
        rows.append(bytes(scanline))

    def chunk(kind: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(b"".join(rows), 9))
        + chunk(b"IEND", b"")
    )
    path = Path(tempfile.gettempdir()) / "dragongui_all_features_v3_demo.png"
    path.write_bytes(png)
    return str(path)


class DemoFrame:
    columns = ("x", "y", "z", "signal", "score", "row_id", "group", "selected")
    dtypes = ("float32", "float32", "float32", "float32", "float32", "int64", "str", "bool")

    def __init__(self, phase: float = 0.0, mode: str = "lidar", rows: int = POINT_ROWS) -> None:
        self.shape = (rows, len(self.columns))
        t = np.linspace(0.0, 1.0, rows, dtype=np.float32)
        theta = t * np.float32(math.tau * 13.0 + phase)
        if mode == "lidar":
            scan_rows = max(64, int(math.sqrt(rows / 2.0)))
            scan_cols = math.ceil(rows / scan_rows)
            u, v = np.meshgrid(
                np.linspace(-1.0, 1.0, scan_cols, dtype=np.float32),
                np.linspace(-1.0, 1.0, scan_rows, dtype=np.float32),
            )
            u = u.ravel()[:rows]
            v = v.ravel()[:rows]
            sweep = np.exp(-((u - np.sin(phase) * 0.72) ** 2) * np.float32(36.0))
            self.x = u * np.float32(14.0) + np.sin(v * np.float32(math.tau) + phase) * np.float32(0.32)
            self.y = v * np.float32(5.5) + np.cos(u * np.float32(math.tau) - phase) * np.float32(0.22)
            self.z = (
                np.float32(18.0)
                + np.sin(u * np.float32(math.tau * 2.0) + phase) * np.float32(0.45)
                + np.cos(v * np.float32(math.tau * 3.0) - phase) * np.float32(0.32)
                + sweep * np.float32(4.2)
            ).astype(np.float32)
        elif mode == "cloud":
            rng = np.random.default_rng(int(phase * 1000.0) + 29)
            cloud = rng.standard_normal((rows, 3)).astype(np.float32)
            self.x = cloud[:, 0] * np.float32(3.0)
            self.y = cloud[:, 1] * np.float32(3.0)
            self.z = cloud[:, 2] * np.float32(3.0)
        elif mode == "wave":
            self.x = (t - np.float32(0.5)) * np.float32(12.0)
            self.y = np.sin(theta) * np.float32(3.0)
            self.z = np.cos(theta * np.float32(0.43) + phase) * np.float32(3.2)
        else:
            radius = np.float32(0.8) + np.float32(3.4) * t
            self.x = np.cos(theta) * radius
            self.y = np.sin(theta) * radius
            self.z = (t - np.float32(0.5)) * np.float32(8.0)
        self.signal = np.sin(theta).astype(np.float32)
        self.score = np.cos(theta * np.float32(0.31)).astype(np.float32)
        self.row_id = np.arange(rows, dtype=np.int64)
        self.group = np.where(self.signal > 0.45, "high", np.where(self.signal < -0.45, "low", "mid"))
        self.selected = self.score > 0.75

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


def redacted_document(doc: object) -> object:
    if isinstance(doc, dict):
        out: dict[str, object] = {}
        for key, value in doc.items():
            if key == "data_b64" and value is not None:
                out[key] = "<packed scatter data>"
            elif key == "cells" and value:
                out[key] = "<sampled table cells>"
            else:
                out[key] = redacted_document(value)
        return out
    if isinstance(doc, list):
        return [redacted_document(item) for item in doc]
    return doc


CSS_MIDNIGHT = """
:root {
    --panel-radius: 10px;
    --control-radius: 8px;
}

Window {
    background: #0c111d;
    color: rgba(245, 248, 255, 0.94);
    font-size: 14px;
}

MenuBar,
StatusBar {
    background: #101827;
    border-color: rgba(255, 255, 255, 0.12);
}

Sidebar {
    background: #121b2b;
    border-color: rgba(90, 169, 255, 0.28);
}

Panel,
Modal {
    background: rgba(18, 27, 43, 0.96);
    border: 1px solid rgba(255, 255, 255, 0.13);
    border-radius: var(--panel-radius);
    color: rgba(245, 248, 255, 0.94);
    accent: #5aa9ff;
}

Panel.highlight {
    border-color: rgba(90, 169, 255, 0.45);
}

Panel.scroll-card {
    height: 260px;
    overflow-y: auto;
    padding-bottom: 26px;
}

Label.brand {
    color: #ffffff;
    font-size: 21px;
    font-weight: 850;
}

Label.subtle,
Label.stat-label {
    color: rgba(245, 248, 255, 0.68);
}

Label.stat-value {
    color: #ffffff;
    font-size: 19px;
    font-weight: 850;
}

Button,
Dropdown,
TextInput,
NumberInput {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: var(--control-radius);
    color: rgba(245, 248, 255, 0.94);
    accent: #5aa9ff;
}

Button {
    font-weight: 750;
}

Button.primary {
    background: rgba(90, 169, 255, 0.24);
    border-color: rgba(90, 169, 255, 0.62);
}

Button:hover,
Dropdown:hover {
    background: rgba(90, 169, 255, 0.18);
    border-color: rgba(90, 169, 255, 0.70);
}

NavItem,
Menu {
    accent: #5aa9ff;
    border-radius: var(--control-radius);
    color: rgba(245, 248, 255, 0.94);
}

Checkbox {
    accent: #74ddb0;
    color: rgba(245, 248, 255, 0.92);
}

Slider {
    accent: #5aa9ff;
    track-color: rgba(255, 255, 255, 0.18);
    thumb-color: #e3f0ff;
}

Slider::track {
    height: 8px;
    background: rgba(255, 255, 255, 0.16);
    border-radius: 999px;
}

Slider::fill {
    height: 8px;
    background: #5aa9ff;
    border-radius: 999px;
}

Slider::thumb {
    width: 18px;
    height: 18px;
    background: #e3f0ff;
    border: 2px solid #5aa9ff;
    border-radius: 999px;
}

LED {
    width: 16px;
    height: 16px;
    background: success;
    border-color: rgba(0, 0, 0, 0.58);
}

LED.off {
    background: disabled;
}

LED.stream,
LED.busy {
    background: #ffcc33;
}

LED::dot {
    width: 12px;
    height: 12px;
    border: 1px solid rgba(0, 0, 0, 0.64);
    border-radius: 999px;
}

LED::glow {
    width: 24px;
    height: 24px;
    opacity: 0.14;
    border-radius: 999px;
}

LED.off::glow {
    opacity: 0;
}

LED::highlight {
    width: 4px;
    height: 3px;
    background: rgba(255, 255, 255, 0.74);
    border-radius: 999px;
}

LED.css-demo {
    width: 22px;
    height: 22px;
    background: #ffcc33;
    border-color: rgba(255, 204, 51, 0.64);
    border-radius: 5px;
}

LED.css-demo::dot {
    width: 15px;
    height: 15px;
    background: radial-gradient(circle at 28% 24%, #fff7c7 0%, #ffcc33 42%, #9a5c00 100%);
    border: 1px solid rgba(94, 58, 0, 0.82);
    border-radius: 5px;
}

LED.css-demo::glow {
    width: 27px;
    height: 27px;
    background: #ffcc33;
    opacity: 0.18;
    box-shadow: none;
    border-radius: 999px;
}

LED.css-demo::highlight {
    width: 5px;
    height: 3px;
    background: rgba(255, 255, 255, 0.72);
    border-radius: 999px;
}

ProgressBar {
    background: rgba(255, 255, 255, 0.10);
    border-color: rgba(255, 255, 255, 0.14);
    accent: #74ddb0;
}

DataFrameTable,
Scatter3D,
Image {
    border-color: rgba(90, 169, 255, 0.28);
    border-width: 1px;
}

Scatter3D {
    width: 100%;
    min-height: 500px;
    flex-grow: 1;
    background: rgba(4, 8, 18, 0.72);
    border-radius: 12px;
    scatter-point-style: circle;
}

GridLayout.scatter-grid {
    height: 100%;
    min-height: 0;
    align-items: stretch;
    grid-template-columns: minmax(340px, 1fr) minmax(0, 2fr);
    overflow-y: hidden;
}

Panel.scatter-controls {
    height: calc(100% - 8px);
    max-height: calc(100% - 8px);
    align-self: stretch;
    overflow-y: hidden;
}

ScrollArea.scatter-control-scroll {
    flex-grow: 1;
    flex-shrink: 1;
    min-height: 0;
    overflow-y: auto;
}

Scatter3D.main-scatter {
    height: calc(100% - 8px);
    min-height: 420px;
    align-self: stretch;
}
"""


CSS_PAPER = CSS_MIDNIGHT + """
Window {
    background: #f4efe4;
    color: #28313d;
}

MenuBar,
StatusBar {
    background: #fff9ed;
    border-color: #c9b99f;
}

MenuBar Menu {
    background: transparent;
    border-color: transparent;
    color: #28313d;
}

MenuBar Menu:hover {
    background: #ffe0b8;
    border-color: #d06b2c;
    color: #18202a;
}

Sidebar {
    background: #17202a;
    border-color: #d06b2c;
}

Panel,
Modal {
    background: #fffaf0;
    border-color: #c9b99f;
    color: #28313d;
    accent: #d06b2c;
}

Panel.highlight {
    border-color: #d06b2c;
}

Label.subtle,
Label.stat-label {
    color: #735f4a;
}

Label.stat-value {
    color: #18202a;
}

Button,
Dropdown,
TextInput,
NumberInput {
    background: #ffffff;
    border-color: #9f8970;
    color: #18202a;
    accent: #d06b2c;
}

Button.primary {
    background: #ffe0b8;
    border-color: #d06b2c;
}

Button:hover,
Dropdown:hover {
    background: #ffe0b8;
    border-color: #d06b2c;
}

NavItem,
Menu {
    accent: #d06b2c;
}

Checkbox {
    accent: #2c8c99;
    color: #28313d;
}

Slider {
    accent: #d06b2c;
    track-color: #b99d78;
    thumb-color: #fffaf0;
}

Slider::track {
    background: #dbc6a6;
    border: 1px solid #9f8970;
}

Slider::fill {
    background: #d06b2c;
}

Slider::thumb {
    background: #fffaf0;
    border: 2px solid #b14d11;
    box-shadow: 0 2px 5px rgba(82, 42, 12, 0.28);
}

LED {
    width: 21px;
    height: 15px;
    background: #2c8c99;
    border-color: #735f4a;
    border-radius: 4px;
}

LED.off {
    background: #c9b99f;
}

LED.stream,
LED.busy {
    background: #d06b2c;
}

LED::dot {
    width: 11px;
    height: 11px;
    background: #2c8c99;
    border: 1px solid #735f4a;
    border-radius: 2px;
    transform: rotate(45deg);
}

LED.off::dot {
    background: #c9b99f;
}

LED.stream::dot,
LED.busy::dot {
    background: #d06b2c;
}

LED::glow {
    opacity: 0;
    box-shadow: none;
}

LED::highlight {
    opacity: 0;
}

LED.css-demo {
    width: 34px;
    height: 18px;
    background: #fffaf0;
    border-color: #9f8970;
    border-radius: 3px;
}

LED.css-demo::dot {
    width: 25px;
    height: 9px;
    background: linear-gradient(90deg, #7b3d12 0%, #d06b2c 48%, #ffe0b8 100%);
    border: 1px solid #7b3d12;
    border-radius: 2px;
    transform: none;
}

LED.css-demo::glow {
    opacity: 0;
    box-shadow: none;
}

LED.css-demo::highlight {
    width: 10px;
    height: 2px;
    background: rgba(255, 250, 240, 0.82);
    opacity: 0.65;
}

ProgressBar {
    background: #ffffff;
    border-color: #9f8970;
    accent: #2c8c99;
}

DataFrameTable,
Scatter3D,
Image {
    border-color: #9f8970;
}
"""


CSS_NEON = CSS_MIDNIGHT + """
Window {
    background: #040617;
}

MenuBar,
StatusBar {
    background: #090f26;
    border-color: #00e5ff;
}

Sidebar {
    background: #0a1028;
    border-color: #ff36d6;
}

Panel,
Modal {
    background: #0d1433;
    border-color: #00e5ff;
    color: #d7fbff;
    accent: #ff36d6;
}

Panel.highlight {
    border-color: #39ff88;
}

Label.brand {
    color: #39ff88;
}

Button,
Dropdown,
TextInput,
NumberInput {
    background: #101947;
    border-color: #ff36d6;
    color: #ffffff;
    accent: #ff36d6;
}

Button.primary {
    background: #2c0f58;
    border-color: #00e5ff;
}

Slider {
    accent: #ff36d6;
    track-color: #22306a;
    thumb-color: #39ff88;
}

Slider::fill {
    background: #ff36d6;
}

Slider::thumb {
    background: #39ff88;
    border-color: #00e5ff;
}

LED {
    width: 20px;
    height: 20px;
    background: #39ff88;
    border-color: #00e5ff;
    border-radius: 999px;
}

LED.off {
    background: #22306a;
    border-color: #56628f;
}

LED.stream,
LED.busy {
    background: #ff36d6;
    border-color: #00e5ff;
}

LED::dot {
    width: 14px;
    height: 14px;
    background: radial-gradient(circle at 25% 20%, #ffffff 0%, #39ff88 34%, #008dff 100%);
    border: 1px solid #d7fbff;
    border-radius: 999px;
}

LED.stream::dot,
LED.busy::dot {
    background: radial-gradient(circle at 24% 18%, #ffffff 0%, #ff36d6 36%, #5b1cff 100%);
}

LED::glow {
    width: 20px;
    height: 20px;
    background: rgba(57, 255, 136, 0.02);
    opacity: 0.6;
    border-radius: 999px;
    box-shadow: 0 0 2px 0px rgba(57, 255, 136, 0.04);
}

LED.stream::glow,
LED.busy::glow {
    background: rgba(255, 54, 214, 0.02);
    box-shadow: 0 0 2px 0px rgba(255, 54, 214, 0.04);
}

LED.off::glow {
    opacity: 0.05;
    box-shadow: none;
}

LED::highlight {
    width: 6px;
    height: 4px;
    background: rgba(255, 255, 255, 0.92);
    border-radius: 999px;
    transform: rotate(-22deg);
}

LED.css-demo {
    width: 30px;
    height: 30px;
    background: #ff36d6;
    border-color: #00e5ff;
    border-radius: 999px;
    box-shadow: 0 0 3px 0px rgba(0, 229, 255, 0.18);
}

LED.css-demo::dot {
    width: 19px;
    height: 19px;
    background: radial-gradient(circle at 28% 20%, #ffffff 0%, #00e5ff 24%, #ff36d6 58%, #3910a8 100%);
    border: 2px solid #39ff88;
    border-radius: 999px;
}

LED.css-demo::glow {
    width: 34px;
    height: 34px;
    background: rgba(0, 229, 255, 0.03);
    opacity: 0.6;
    box-shadow: 0 0 3px 0px rgba(0, 229, 255, 0.06);
}

LED.css-demo::highlight {
    width: 8px;
    height: 5px;
    background: rgba(255, 255, 255, 0.96);
    transform: rotate(-30deg);
}
"""


CSS_THEMES = {
    "midnight": CSS_MIDNIGHT,
    "paper": CSS_PAPER,
    "neon": CSS_NEON,
}


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))
app.stylesheet(CSS_THEMES["midnight"])
stream_controller: dg.ScatterFrameStream | None = None
stream_build_thread: threading.Thread | None = None
stats_thread: threading.Thread | None = None
stream_cancel = threading.Event()
stats_stop = threading.Event()
state_lock = threading.Lock()
demo_state = {
    "mode": "lidar",
    "phase": 0.0,
    "theme": "midnight",
    "stream_interval_ms": 40.0,
    "style": 0,
    "grid": True,
    "planes": True,
    "grid_sticky": True,
    "grid_all_edges": False,
    "orientation": True,
    "axis_x": "x",
    "axis_y": "y",
    "axis_z": "z",
    "axis_visible_x": True,
    "axis_visible_y": True,
    "axis_visible_z": True,
    "ticks_x": 5,
    "ticks_y": 5,
    "ticks_z": 5,
    "stats_auto": False,
    "page": "overview",
}
initial_frame = DemoFrame(mode="lidar")
demo_image_path = make_demo_image()
stream_payload_cache: dict[tuple[str, str], list[tuple[float, dg.ScatterPayload]]] = {}


def set_status(message: str) -> None:
    status.set_value(message)


def fmt_ms(value: object) -> str:
    try:
        ms = float(value)
    except (TypeError, ValueError):
        return "--"
    return f"{ms:.2f} ms"


def fmt_count(value: object) -> str:
    try:
        return f"{int(value):,}"
    except (TypeError, ValueError):
        return "--"


def fmt_payload(value: object) -> str:
    try:
        bytes_value = float(value)
    except (TypeError, ValueError):
        return "--"
    if bytes_value >= 1024 * 1024:
        return f"{bytes_value / (1024 * 1024):.2f} MiB"
    if bytes_value >= 1024:
        return f"{bytes_value / 1024:.1f} KiB"
    return f"{bytes_value:.0f} B"


def metric_ms(metrics: dict[str, object], key: str) -> str:
    return fmt_ms(metrics.get(key))


def update_scatter_stats(snapshot: dict[str, object], observed_fps: float | None = None) -> None:
    runtime = snapshot.get("runtime", {})
    gpu = snapshot.get("gpu", {})
    if not isinstance(runtime, dict) or not isinstance(gpu, dict):
        return
    resources = gpu.get("resources", {})
    if not isinstance(resources, dict):
        return
    scatters = resources.get("scatters", {})
    scatter_metrics: dict[str, object] = {}
    if isinstance(scatters, dict):
        selected = scatters.get(scatter.id)
        if isinstance(selected, dict):
            scatter_metrics = selected
    if not scatter_metrics:
        selected = resources.get("scatter", {})
        if isinstance(selected, dict):
            scatter_metrics = selected

    frame_ms_value = runtime.get("frame_ms")
    try:
        frame_ms = float(frame_ms_value)
    except (TypeError, ValueError):
        frame_ms = 0.0
    frame_fps = 1000.0 / frame_ms if frame_ms > 0.0 else 0.0
    observed_text = "--" if observed_fps is None else f"{observed_fps:.1f} fps"
    lod = scatter_metrics.get("lod", {})
    lod_active = bool(lod.get("active")) if isinstance(lod, dict) else False
    scatter_stats_summary.set_value(
        "\n".join(
            (
                f"Frame CPU avg: {fmt_ms(frame_ms_value)} ({frame_fps:.1f} fps)",
                f"Observed redraws: {observed_text} / {fmt_count(runtime.get('frames_rendered'))} frames",
                f"Scatter encode: {metric_ms(scatter_metrics, 'last_render_encode_ms')}",
                "Payload: "
                f"{fmt_count(scatter_metrics.get('last_point_count'))} pts / "
                f"{fmt_payload(scatter_metrics.get('last_payload_bytes'))}",
                "Native update: "
                f"{metric_ms(scatter_metrics, 'last_total_native_ms')} total, "
                f"{metric_ms(scatter_metrics, 'last_upload_ms')} upload",
                "Decode/grid/overlay: "
                f"{metric_ms(scatter_metrics, 'last_decode_ms')} / "
                f"{metric_ms(scatter_metrics, 'last_grid_ms')} / "
                f"{metric_ms(scatter_metrics, 'last_overlay_ms')}",
                "Updates: "
                f"{fmt_count(scatter_metrics.get('updates'))}, "
                f"LOD {'active' if lod_active else 'idle'}, "
                f"{scatter_metrics.get('payload_status', '--')}",
            )
        )
    )


def refresh_scatter_stats() -> None:
    def worker() -> None:
        try:
            snapshot = app.debug_snapshot(timeout_ms=500)
            app.call_soon_threadsafe(lambda s=snapshot: update_scatter_stats(s))
        except RuntimeError:
            pass

    threading.Thread(target=worker, daemon=True).start()


def toggle_scatter_stats(enabled: bool) -> None:
    with state_lock:
        demo_state["stats_auto"] = bool(enabled)
    set_status(f"Scatter stats: {'auto' if enabled else 'paused'}")
    if enabled:
        refresh_scatter_stats()


def set_page(value: str) -> None:
    with state_lock:
        demo_state["page"] = value
    set_status(f"Page: {value}")


def debug_page_active() -> bool:
    with state_lock:
        return demo_state.get("page") == "debug"


def scatter_stats_worker() -> None:
    last_time: float | None = None
    last_frames: int | None = None
    if stats_stop.wait(2.0):
        return
    while not stats_stop.is_set():
        with state_lock:
            enabled = bool(demo_state["stats_auto"])
        if not enabled:
            stats_stop.wait(0.25)
            continue
        try:
            snapshot = app.debug_snapshot(timeout_ms=500)
        except RuntimeError:
            stats_stop.wait(0.25)
            continue
        runtime = snapshot.get("runtime", {})
        frames = 0
        if isinstance(runtime, dict):
            try:
                frames = int(runtime.get("frames_rendered", 0))
            except (TypeError, ValueError):
                frames = 0
        now = time.perf_counter()
        observed_fps = None
        if last_time is not None and last_frames is not None:
            elapsed = now - last_time
            if elapsed > 0.0:
                observed_fps = max(0.0, (frames - last_frames) / elapsed)
        last_time = now
        last_frames = frames
        try:
            app.call_soon_threadsafe(
                lambda s=snapshot, fps=observed_fps: update_scatter_stats(s, fps)
            )
        except RuntimeError:
            break
        stats_stop.wait(1.0)


def apply_theme(name: str) -> None:
    app.stylesheet(CSS_THEMES[name])
    demo_state["theme"] = name
    set_status(f"Theme: {name}")


def next_frame(mode: str | None = None) -> DemoFrame:
    if mode is not None:
        demo_state["mode"] = mode
    demo_state["phase"] += 0.38
    return DemoFrame(phase=demo_state["phase"], mode=str(demo_state["mode"]))


def push_scatter(mode: str | None = None) -> None:
    previous_mode = str(demo_state["mode"])
    frame = next_frame(mode)
    refit_camera = mode is not None and str(demo_state["mode"]) != previous_mode
    scatter.set_points(
        frame,
        x="x",
        y="y",
        z="z",
        scalars="z",
        hover=["row_id", "group", "signal"],
        fit=refit_camera,
    )
    progress.set_value((demo_state["phase"] % 6.0) / 6.0)
    set_status(f"Scatter: {demo_state['mode']} phase {demo_state['phase']:.2f}")


def set_colormap(name: str) -> None:
    scatter.set_colormap(name)
    scatter.show_scalar_bar(True, colormap=name.lower(), title="z")
    if stream_controller is not None and stream_controller.running:
        stop_stream()
        start_stream()
    set_status(f"Colormap: {name}")


def set_stream_interval(value: float) -> None:
    interval_ms = max(5.0, float(value))
    with state_lock:
        demo_state["stream_interval_ms"] = interval_ms
    stream_interval_label.set_value(f"Stream interval: {interval_ms:.0f} ms")


def set_point_size(value: float) -> None:
    scatter.set_point_size(float(value))
    set_status(f"Point size: {value:.1f}")


def toggle_grid(enabled: bool) -> None:
    demo_state["grid"] = bool(enabled)
    scatter.show_grid(bool(enabled))
    set_status(f"Grid: {enabled}")


def toggle_planes(enabled: bool) -> None:
    demo_state["planes"] = bool(enabled)
    scatter.show_grid_planes(bool(enabled), bool(enabled))
    set_status(f"Grid planes: {enabled}")


def apply_grid_options() -> None:
    scatter.set_grid_options(
        sticky=bool(demo_state["grid_sticky"]),
        all_edges=bool(demo_state["grid_all_edges"]),
    )


def toggle_grid_sticky(enabled: bool) -> None:
    demo_state["grid_sticky"] = bool(enabled)
    apply_grid_options()
    set_status(f"Sticky grid: {enabled}")


def toggle_grid_all_edges(enabled: bool) -> None:
    demo_state["grid_all_edges"] = bool(enabled)
    apply_grid_options()
    set_status(f"Grid all edges: {enabled}")


def toggle_orientation(enabled: bool) -> None:
    demo_state["orientation"] = bool(enabled)
    scatter.show_orientation_axes(bool(enabled))
    set_status(f"Orientation axes: {enabled}")


def update_axis_label(axis: str, value: str) -> None:
    label = value.strip() or axis
    demo_state[f"axis_{axis}"] = label
    scatter.set_axes(str(demo_state["axis_x"]), str(demo_state["axis_y"]), str(demo_state["axis_z"]))
    set_status(
        f"Axes: {demo_state['axis_x']} / {demo_state['axis_y']} / {demo_state['axis_z']}"
    )


def set_tick_count(axis: str, value: float) -> None:
    count = max(2, int(round(float(value))))
    demo_state[f"ticks_{axis}"] = count
    tick_label = {
        "x": x_tick_label,
        "y": y_tick_label,
        "z": z_tick_label,
    }[axis]
    tick_label.set_value(f"{axis.upper()} ticks: {count}")
    tick_x = demo_state["ticks_x"]
    tick_y = demo_state["ticks_y"]
    tick_z = demo_state["ticks_z"]
    scatter.set_ticks(
        x=None if tick_x is None else int(tick_x),
        y=None if tick_y is None else int(tick_y),
        z=None if tick_z is None else int(tick_z),
    )
    tick_status = {
        axis_name: "auto" if demo_state[f"ticks_{axis_name}"] is None else demo_state[f"ticks_{axis_name}"]
        for axis_name in ("x", "y", "z")
    }
    set_status(
        f"Ticks: x={tick_status['x']} y={tick_status['y']} z={tick_status['z']}"
    )


def reset_tick_counts() -> None:
    demo_state["ticks_x"] = None
    demo_state["ticks_y"] = None
    demo_state["ticks_z"] = None
    x_tick_label.set_value("X ticks: auto")
    y_tick_label.set_value("Y ticks: auto")
    z_tick_label.set_value("Z ticks: auto")
    scatter.set_ticks()
    set_status("Ticks: auto")


def toggle_axis_visibility(axis: str, enabled: bool) -> None:
    demo_state[f"axis_visible_{axis}"] = bool(enabled)
    scatter.set_axis_visibility(
        x=bool(demo_state["axis_visible_x"]),
        y=bool(demo_state["axis_visible_y"]),
        z=bool(demo_state["axis_visible_z"]),
    )
    set_status(
        "Axis visibility: "
        f"x={demo_state['axis_visible_x']} "
        f"y={demo_state['axis_visible_y']} "
        f"z={demo_state['axis_visible_z']}"
    )


def set_scatter_point_style(value: str) -> None:
    scatter.set_point_style(value.lower())
    set_status(f"Point style: {value}")


def set_scatter_view(value: str) -> None:
    view = value.lower()
    if view == "xy":
        scatter.view_xy()
    elif view == "xz":
        scatter.view_xz()
    elif view == "yz":
        scatter.view_yz()
    else:
        scatter.view_isometric()
    set_status(f"View: {value}")


def pick_scatter_point(point: dg.ScatterPick) -> None:
    set_status(f"Point {point.index}: x={point.x:.2f}, y={point.y:.2f}, z={point.z:.2f}")


def stream_payloads_for_mode(mode: str, colormap: str) -> list[tuple[float, dg.ScatterPayload]]:
    cache_key = (mode, colormap)
    with state_lock:
        cached = stream_payload_cache.get(cache_key)
    if cached is not None:
        return cached

    try:
        app.call_soon_threadsafe(lambda m=mode: set_status(f"Prebuilding {m} stream frames"))
    except RuntimeError:
        pass
    phase_step = math.tau / STREAM_FRAME_COUNT
    built = [
        (
            index * phase_step,
            dg.Scatter3D.prepare_points(
                DemoFrame(phase=index * phase_step, mode=mode),
                x="x",
                y="y",
                z="z",
                colormap=colormap,
            ),
        )
        for index in range(STREAM_FRAME_COUNT)
    ]
    with state_lock:
        stream_payload_cache[cache_key] = built
    return built


def record_stream_frame(
    phase: float,
    mode: str,
    metrics: dg.ScatterStreamMetrics | None = None,
) -> None:
    demo_state["phase"] = phase
    progress.set_value((phase % math.tau) / math.tau)
    if metrics is None:
        set_status(f"Streaming {mode}: phase {phase:.2f}")
    else:
        set_status(f"Streaming {mode}: phase {phase:.2f}, submitted {metrics.submitted:,}")


def current_stream_interval_ms() -> float:
    with state_lock:
        return float(demo_state["stream_interval_ms"])


def start_stream() -> None:
    global stream_controller, stream_build_thread
    if stream_controller is not None and stream_controller.running:
        return
    if stream_build_thread is not None and stream_build_thread.is_alive():
        return
    stream_cancel.clear()

    def worker() -> None:
        with state_lock:
            mode = str(demo_state["mode"])
            colormap = scatter.colormap
        try:
            payloads = stream_payloads_for_mode(mode, colormap)
        except RuntimeError:
            return
        if stream_cancel.is_set():
            return

        def launch() -> None:
            global stream_controller
            if stream_cancel.is_set():
                return
            if stream_controller is not None and stream_controller.running:
                return
            frames = [payload for _, payload in payloads]

            def on_frame(
                _payload: dg.ScatterPayload,
                index: int,
                metrics: dg.ScatterStreamMetrics,
            ) -> None:
                phase = payloads[index % len(payloads)][0]
                record_stream_frame(phase, mode, metrics)

            stream_controller = scatter.stream_prepared_frames(
                frames,
                interval_ms=current_stream_interval_ms,
                loop=True,
                on_frame=on_frame,
                ui_interval_ms=500,
            )
            stream_controller.start()
            set_status("Scatter stream started")

        try:
            app.call_soon_threadsafe(launch)
        except RuntimeError:
            return

    stream_build_thread = threading.Thread(target=worker, daemon=True)
    stream_build_thread.start()
    set_status("Preparing scatter stream")


def stop_stream() -> None:
    stream_cancel.set()
    if stream_controller is not None:
        stream_controller.stop(timeout=0.25)
    set_status("Scatter stream stopped")


def update_table(mode: str) -> None:
    table.set_frame(DemoFrame(phase=demo_state["phase"] + 0.6, mode=mode, rows=TABLE_ROWS))
    set_status(f"Table frame: {mode}")


def select_table_cell(selection: dg.TableSelection) -> None:
    set_status(f"Table row {selection.row_index}, {selection.column}: {selection.value}")


def upload_buffer() -> None:
    payload = np.linspace(0.0, 1.0, 4096, dtype=np.float32)
    app.set_buffer_resource("demo:f32-buffer", payload, kind="f32")
    set_status("Uploaded demo:f32-buffer")


def release_buffer() -> None:
    app.release_resource("demo:f32-buffer")
    set_status("Released demo:f32-buffer")


def choose_csv() -> None:
    dg.open_file_dialog(
        title="Open CSV",
        filters=[("CSV files", ["csv"])],
        on_select=lambda path: set_status(f"Selected: {path}" if path else "Open CSV cancelled"),
        app=app,
    )


def color_hex(color: tuple[int, ...]) -> str:
    r, g, b = color[:3]
    return f"#{r:02x}{g:02x}{b:02x}"


def apply_demo_color(color: tuple[int, ...]) -> None:
    selected = color_hex(color)
    color_target.set_style(
        {"height": 38, "background": selected, "border_color": selected, "text_align": "center"}
    )
    set_status(f"ColorPicker: {selected}")


def show_demo_toast() -> None:
    dg.toast("All features V3 toast", level="success", duration=2400, position="bottom-right")
    set_status("Toast queued")


def print_snapshot() -> None:
    def worker() -> None:
        try:
            snapshot = app.debug_snapshot()
            summary = {
                "frames": snapshot["runtime"]["frames_rendered"],
                "widgets": snapshot["gpu"]["renderer"]["widget_count"],
                "commands": snapshot["runtime"]["commands"]["count"],
                "buffers": snapshot["gpu"]["resources"]["registry"]["buffers"]["count"],
                "tables": snapshot["gpu"]["resources"]["registry"]["tables"]["count"],
            }
            print("Debug snapshot:", summary, flush=True)
            app.call_soon_threadsafe(lambda: set_status(f"Snapshot: {summary['widgets']} widgets"))
        except RuntimeError:
            pass

    threading.Thread(target=worker, daemon=True).start()


def make_summary_children() -> list[object]:
    return [
        dg.Label("Dynamic Summary", parent=None, style={"font_size": 18, "font_weight": "bold"}),
        dg.Separator(parent=None),
        dg.Label(f"Scatter rows: {POINT_ROWS:,}", parent=None),
        dg.Label("Layout: GridLayout + FlowLayout", parent=None),
        dg.Button("Dynamic Action", parent=None, on_click=lambda: set_status("Dynamic action clicked")),
    ]


def make_pipeline_children() -> list[object]:
    return [
        dg.Label("Pipeline Status", parent=None, style={"font_size": 18, "font_weight": "bold"}),
        dg.Separator(parent=None),
        dg.Label("Load: complete", parent=None, style={"color": "success"}),
        dg.Label("Transform: queued", parent=None, style={"color": "warning"}),
        dg.Label("Render: live", parent=None, style={"color": "accent"}),
    ]


def swap_children() -> None:
    demo_state["style"] = 1 - int(demo_state["style"])
    dynamic_panel.replace_children(
        make_pipeline_children() if demo_state["style"] else make_summary_children()
    )
    set_status("Replaced runtime panel children")


styles = [
    {
        "panel": {"background": "#172235", "border_color": "accent", "border_radius": 12},
        "label": {"color": "#b9f6ff", "font_weight": "bold"},
        "button": {"background": "#24314a", "border_color": "accent", "text_align": "center"},
    },
    {
        "panel": {"background": "#211a27", "border_color": "warning", "border_radius": 14},
        "label": {"color": "warning", "font_weight": "bold"},
        "button": {"background": "#3a2932", "border_color": "warning", "text_align": "center"},
    },
]


def cycle_style() -> None:
    demo_state["style"] = 1 - int(demo_state["style"])
    current = styles[int(demo_state["style"])]
    style_panel.set_style({**CARD_STYLE, **current["panel"]})
    style_label.set_style({"font_size": 16, **current["label"]})
    style_button.set_style({"height": 44, "width": 190, **current["button"]})
    set_status("Applied live style patch")


@dg.component
def AllFeaturesV3(_ctx: dg.ComponentCtx) -> dg.Window:
    global win, status, progress, scatter_stats_summary, scatter, table
    global color_target, x_tick_label, y_tick_label, z_tick_label
    global stream_interval_label, dynamic_panel, style_panel, style_label, style_button
    global confirm_modal, about_modal

    win = dg.Window("DragonGUI All Features V3 Demo", width=1440, height=900)

    with dg.MenuBar(height=34, tooltip="Menus render as native overlays."):
        with dg.Menu("File"):
            dg.MenuItem("Open CSV...", on_click=choose_csv)
            dg.MenuItem("Print Snapshot", on_click=print_snapshot)
            dg.MenuItem("Upload Buffer", on_click=upload_buffer)
            dg.MenuItem("Release Buffer", on_click=release_buffer)
        with dg.Menu("Scatter"):
            dg.MenuItem("Push LiDAR Frame", on_click=lambda: push_scatter("lidar"))
            dg.MenuItem("Start Stream", on_click=start_stream)
            dg.MenuItem("Stop Stream", on_click=stop_stream)
        with dg.Menu("Help"):
            dg.MenuItem("About", on_click=lambda: about_modal.show())


    with dg.HLayout(style={"gap": 0}):
        with dg.Sidebar(width=238, style={"padding": 12, "gap": 8}):
            dg.Label("DragonGUI", class_="brand")
            dg.Label("All features V3", class_="subtle")
            with dg.FlowLayout(gap=6, row_gap=4):
                dg.LED(True, tooltip="Renderer online")
                dg.LED("stream", states={"stream": "warning"}, tooltip="Custom stream state")
                dg.Badge("Grid", level="success")
                dg.Tag("Scatter3D", level="info")
            dg.Separator()
            dg.NavItem("Overview", page="overview")
            dg.NavItem("Scatter", page="scatter")
            dg.NavItem("Controls", page="controls")
            dg.NavItem("Data", page="data")
            dg.NavItem("Runtime", page="runtime")
            dg.NavItem("Debug", page="debug")
            dg.NavItem("Styling", page="styling")
            dg.NavItem("Layout", page="layout")
            dg.Spacer()
            dg.Separator()
            dg.Label("Responsive grids, CSS, overlays, tables, and live native commands.", class_="subtle")

        with dg.Pages(value="overview", on_change=set_page, key="main-pages"):
            with dg.Page("overview", title="Overview"):
                with dg.GridLayout(columns=3, min_column_width=270, gap=GRID_GAP, style=GRID_STYLE):
                    with dg.Panel("Frame", class_="highlight", style=CARD_STYLE):
                        dg.Label(f"{POINT_ROWS:,}", class_="stat-value")
                        dg.Label("Scatter points per generated frame", class_="stat-label")
                    with dg.Panel("Data", class_="highlight", style=CARD_STYLE):
                        dg.Label(f"{TABLE_ROWS:,}", class_="stat-value")
                        dg.Label("Virtualized table rows", class_="stat-label")
                    with dg.Panel("Layout", class_="highlight", style=CARD_STYLE):
                        dg.Label("Grid + Flow", class_="stat-value")
                        dg.Label("Responsive page composition", class_="stat-label")
                    with dg.Panel("Quick actions", style=CARD_STYLE):
                        with dg.FlowLayout(gap=8, row_gap=8):
                            dg.Button("Push LiDAR", class_="primary", on_click=lambda: push_scatter("lidar"))
                            dg.Button("Start Stream", on_click=start_stream)
                            dg.Button("Stop Stream", on_click=stop_stream)
                            dg.Button("Snapshot", on_click=print_snapshot)
                        progress = dg.ProgressBar(0.0, min=0, max=1, show_value=True)
                    with dg.Panel("Visual asset", style=CARD_STYLE):
                        dg.Image(demo_image_path, fit="cover", height=180, style={"border_radius": 10})
                        dg.Label("Generated PNG image, styled as a native textured quad.", class_="subtle")
                    with dg.Panel("Theme", style=CARD_STYLE):
                        dg.Button("Midnight", on_click=lambda: apply_theme("midnight"))
                        dg.Button("Paper", on_click=lambda: apply_theme("paper"))
                        dg.Button("Neon", on_click=lambda: apply_theme("neon"))

            with dg.Page("scatter", title="Scatter3D"):
                with dg.GridLayout(
                    columns=2,
                    min_column_width=380,
                    gap=GRID_GAP,
                    class_="scatter-grid",
                    style=SCATTER_GRID_STYLE,
                ):
                    with dg.Panel("Scatter controls", class_="scatter-controls", style=SCATTER_CONTROLS_PANEL_STYLE):
                        with dg.ScrollArea(
                            axis="y",
                            gap=8,
                            class_="scatter-control-scroll",
                            style=SCATTER_CONTROLS_SCROLL_STYLE,
                        ):
                            dg.Label("Performance")
                            dg.Button(
                                "Refresh stats",
                                class_="primary",
                                on_click=refresh_scatter_stats,
                                style={"height": 34, "width": 170},
                            )
                            dg.Checkbox(
                                "Auto stats",
                                checked=False,
                                on_change=toggle_scatter_stats,
                                style={"height": 34},
                            )
                            scatter_stats_summary = dg.Label(
                                "\n".join(
                                    (
                                        "Frame CPU avg: --",
                                        "Observed redraws: --",
                                        "Scatter encode: --",
                                        "Payload: --",
                                        "Native update: --",
                                        "Decode/grid/overlay: --",
                                        "Updates: --",
                                    )
                                )
                            )
                            dg.Separator()
                            dg.Label("Data")
                            mode = dg.Dropdown(("lidar", "helix", "wave", "cloud"), value="lidar", on_change=push_scatter)
                            dg.Dropdown(("Viridis", "Magma", "Plasma", "Turbo", "Cividis"), value="Turbo", on_change=set_colormap)
                            dg.Dropdown(("Circle", "Square", "Gaussian"), value="Circle", on_change=set_scatter_point_style)
                            with dg.FlowLayout(gap=8, row_gap=8):
                                dg.Button("Push frame", class_="primary", on_click=lambda: push_scatter(mode.value))
                                dg.Button("Fit camera", on_click=lambda: scatter.fit())
                            dg.Label("View")
                            dg.Dropdown(("Isometric", "XY", "XZ", "YZ"), value="Isometric", on_change=set_scatter_view)
                            with dg.FlowLayout(gap=8, row_gap=6):
                                dg.Checkbox("Grid", checked=True, on_change=toggle_grid)
                                dg.Checkbox("Grid planes", checked=True, on_change=toggle_planes)
                                dg.Checkbox("Orientation", checked=True, on_change=toggle_orientation)
                            with dg.FlowLayout(gap=8, row_gap=6):
                                dg.Checkbox("Sticky grid", checked=True, on_change=toggle_grid_sticky)
                                dg.Checkbox("All edges", checked=False, on_change=toggle_grid_all_edges)
                            dg.Label("Axis labels")
                            dg.TextInput("x", placeholder="X label", on_change=lambda value: update_axis_label("x", value))
                            dg.TextInput("y", placeholder="Y label", on_change=lambda value: update_axis_label("y", value))
                            dg.TextInput("z", placeholder="Z label", on_change=lambda value: update_axis_label("z", value))
                            with dg.FlowLayout(gap=8, row_gap=6):
                                dg.Checkbox("X axis", checked=True, on_change=lambda value: toggle_axis_visibility("x", value))
                                dg.Checkbox("Y axis", checked=True, on_change=lambda value: toggle_axis_visibility("y", value))
                                dg.Checkbox("Z axis", checked=True, on_change=lambda value: toggle_axis_visibility("z", value))
                            x_tick_label = dg.Label("X ticks: 5")
                            dg.Slider(5, min=2, max=12, step=1, on_change=lambda value: set_tick_count("x", value))
                            y_tick_label = dg.Label("Y ticks: 5")
                            dg.Slider(5, min=2, max=12, step=1, on_change=lambda value: set_tick_count("y", value))
                            z_tick_label = dg.Label("Z ticks: 5")
                            dg.Slider(5, min=2, max=12, step=1, on_change=lambda value: set_tick_count("z", value))
                            dg.Button("Auto ticks", on_click=reset_tick_counts)
                            dg.Label("Point size")
                            dg.Slider(3.2, min=1.0, max=8.0, step=0.2, on_change=set_point_size)
                            dg.Label("Stream")
                            dg.Button("Start stream", on_click=start_stream)
                            dg.Button("Stop stream", on_click=stop_stream)
                            stream_interval_label = dg.Label("Stream interval: 40 ms")
                            dg.Slider(40, min=5, max=250, step=5, on_change=set_stream_interval)
                    scatter = dg.Scatter3D(
                        initial_frame,
                        x="x",
                        y="y",
                        z="z",
                        scalars="z",
                        colormap="turbo",
                        point_size=3.2,
                        opacity=1.0,
                        grid=True,
                        major_planes=True,
                        minor_planes=True,
                        grid_sticky=True,
                        grid_all_edges=False,
                        orientation_axes=True,
                        scalar_bar=True,
                        scalar_bar_title="z",
                        axis_x="x",
                        axis_y="y",
                        axis_z="z",
                        background=(0.02, 0.02, 0.03),
                        hover=["row_id", "group", "signal"],
                        on_pick=pick_scatter_point,
                        class_="main-scatter",
                        key="main-scatter",
                    )

            with dg.Page("controls", title="Controls"):
                with dg.GridLayout(columns=2, min_column_width=360, gap=GRID_GAP, style=GRID_STYLE):
                    with dg.Panel("Form controls", style=CARD_STYLE):
                        with dg.FlowLayout(gap=8, row_gap=6, cross_align="center"):
                            dg.LED(True, tooltip="Boolean on state")
                            dg.LED(False, tooltip="Boolean off state")
                            dg.LED("busy", states={"busy": "#ffcc33", "ready": "success"}, tooltip="Named custom state")
                            dg.LED("busy", states={"busy": "#ffcc33"}, class_="css-demo", tooltip="CSS styled LED parts")
                            dg.Badge("live", level="success")
                            dg.Badge("queued", level="warning")
                            dg.Tag("styled", level="info")
                        dg.TextInput("editable text", placeholder="Type here", on_change=lambda v: set_status(f"Text: {v}"))
                        dg.Dropdown(("Low", "Medium", "High"), value="Medium", on_change=lambda v: set_status(f"Dropdown: {v}"))
                        dg.Slider(0.42, min=0, max=1, step=0.02, on_change=lambda v: set_status(f"Slider: {v:.2f}"))
                        dg.NumberInput(42, min=0, max=100, step=0.5, on_change=lambda v: set_status(f"Number: {v:g}"))
                        color_target = dg.Button(
                            "Color target",
                            style={"height": 38, "background": "#5aa9ff", "border_color": "#5aa9ff"},
                        )
                        dg.ColorPicker((90, 169, 255), alpha=False, on_change=apply_demo_color)
                        dg.Checkbox("Enable analysis", checked=True, on_change=lambda v: set_status(f"Analysis: {v}"))
                        dg.Button("Regular button", on_click=lambda: set_status("Button clicked"))
                        dg.Button("Show toast", badge="new", on_click=show_demo_toast)
                        dg.Button("Disabled button", disabled=True)
                        dg.TextInput("disabled input", disabled=True)
                    with dg.Panel("Tabs and disclosure", style=CARD_STYLE):
                        with dg.Tabs(value="one", on_change=lambda v: set_status(f"Tab: {v}")):
                            with dg.Tab("One", value="one"):
                                dg.Label("Tab content one")
                                tab_button = dg.Button("Tooltip target", on_click=lambda: set_status("Tab button"))
                                with dg.Tooltip(target=tab_button):
                                    dg.Label("Rich tooltip")
                                    dg.ProgressBar(0.66, show_value=True)
                            with dg.Tab("Two", value="two"):
                                dg.Checkbox("A checkbox in a tab", checked=False)
                                dg.TextArea("Line one\nLine two\nLine three", rows=3)
                            with dg.Tab("Three", value="three"):
                                dg.Slider(0.7, min=0, max=1, step=0.05)
                        with dg.Collapsible("Advanced notes", expanded=False):
                            dg.TextArea("Extra notes\nstay scrollable\ninside the panel", rows=3)

            with dg.Page("data", title="Data"):
                with dg.GridLayout(columns=2, min_column_width=390, gap=GRID_GAP, style=GRID_STYLE):
                    with dg.Panel("Data controls", style=CARD_STYLE):
                        dg.Button("Load LiDAR Table", on_click=lambda: update_table("lidar"))
                        dg.Button("Load Helix Table", on_click=lambda: update_table("helix"))
                        dg.Button("Load Wave Table", on_click=lambda: update_table("wave"))
                        dg.Button("Load Cloud Table", on_click=lambda: update_table("cloud"))
                        dg.Separator()
                        dg.Button("Upload Buffer", on_click=upload_buffer)
                        dg.Button("Release Buffer", on_click=release_buffer)
                        dg.Button("Confirm Reset", on_click=lambda: confirm_modal.show())
                        dg.Button("Print Snapshot", on_click=print_snapshot)
                        dg.Label("DataFrameTable virtualizes rows and columns.", class_="subtle")
                    table = dg.DataFrameTable(
                        DemoFrame(rows=TABLE_ROWS),
                        page_size=90,
                        on_select=select_table_cell,
                        key="main-table",
                    )

            with dg.Page("runtime", title="Runtime"):
                with dg.GridLayout(columns=2, min_column_width=360, gap=GRID_GAP, style=GRID_STYLE):
                    with dg.Panel("Live commands", style=CARD_STYLE):
                        dg.Button("Replace children", on_click=swap_children)
                        dg.Button("Cycle style", on_click=cycle_style)
                        dg.Button("Print snapshot", on_click=print_snapshot)
                        dg.Button("Upload buffer", on_click=upload_buffer)
                        dg.Button("Release buffer", on_click=release_buffer)
                        dg.Label("Commands are retained-tree updates sent to the native runtime.", class_="subtle")
                    with dg.Panel("ReplaceChildren target", class_="highlight", style=CARD_STYLE):
                        with dg.VLayout(style={"gap": 4}) as dynamic_panel:
                            for child in make_summary_children():
                                dynamic_panel.add(child)

            with dg.Page("debug", title="Debug"):
                with dg.GridLayout(columns=2, min_column_width=420, gap=GRID_GAP, style=GRID_STYLE):
                    dg.ThreadMonitor(
                        key="debug-thread-monitor",
                        show_threads=True,
                        show_queue=True,
                        show_failures=True,
                        history_seconds=30,
                        refresh_hz=4.0,
                        max_threads=60,
                        max_dead_threads=12,
                        enabled=debug_page_active,
                        class_="debug-monitor",
                        style=DEBUG_MONITOR_STYLE,
                    )
                    with dg.Panel("Snapshot tools", style=CARD_STYLE):
                        dg.Button("Print snapshot", class_="primary", on_click=print_snapshot)
                        dg.Button("Refresh scatter stats", on_click=refresh_scatter_stats)
                        dg.Checkbox("Auto scatter stats", checked=False, on_change=toggle_scatter_stats)
                        dg.Separator()
                        dg.Label("ThreadMonitor shows Python task queue, producer threads, and task failures.", class_="subtle")
                        dg.Label("Use the scatter controls or background stream to create live task traffic.", class_="subtle")

            with dg.Page("styling", title="Styling"):
                with dg.GridLayout(columns=2, min_column_width=360, gap=GRID_GAP, style=GRID_STYLE):
                    with dg.Panel("CSS themes", style=CARD_STYLE):
                        dg.Button("Midnight CSS", on_click=lambda: apply_theme("midnight"))
                        dg.Button("Paper CSS", on_click=lambda: apply_theme("paper"))
                        dg.Button("Neon CSS", on_click=lambda: apply_theme("neon"))
                        dg.Separator()
                        with dg.Panel("Horizontal overflow", style={"height": 94, "overflow_x": "auto", "overflow_y": "hidden", "padding": 12}):
                            with dg.HLayout(style={"width": 430, "height": 34, "gap": 8, "flex_shrink": 0}):
                                dg.Button("First", style={"width": 126, "flex_shrink": 0})
                                dg.Button("Second", style={"width": 126, "flex_shrink": 0})
                                dg.Button("Third", style={"width": 126, "flex_shrink": 0})
                        dg.Button("Danger token", style={"background": "danger", "border_color": "danger"})
                        dg.Button("Warning token", style={"background": "warning", "border_color": "warning"})
                        dg.Button("Success token", style={"background": "success", "border_color": "success"})
                    with dg.Panel("Live style preview", style={**CARD_STYLE, **styles[0]["panel"]}) as style_panel:
                        style_label = dg.Label("Styled label", style={"font_size": 16, **styles[0]["label"]})
                        style_button = dg.Button("Cycle this panel", on_click=cycle_style, style={"height": 44, "width": 190, **styles[0]["button"]})
                        dg.Image(demo_image_path, fit="cover", height=170, style={"border_color": "accent", "border_radius": 10})
                        dg.Label("CSS, inline style, pseudo states, and live style patches.", class_="subtle")

            with dg.Page("layout", title="Layout"):
                with dg.GridLayout(columns=3, min_column_width=300, gap=GRID_GAP, style=GRID_STYLE):
                    with dg.Panel("Flow wrap", style=CARD_STYLE):
                        with dg.FlowLayout(gap=8, row_gap=8):
                            for label in ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta"]:
                                dg.Button(label)
                        dg.Label("FlowLayout wraps intrinsic controls without clipping.", class_="subtle")
                    with dg.Panel("Vertical scroll", class_="scroll-card", style=CARD_STYLE):
                        for index in range(1, 18):
                            dg.Label(f"Scrollable row {index:02d}")
                        dg.Button("Last action")
                    with dg.Panel("Composition", style=CARD_STYLE):
                        dg.Label("GridLayout chooses columns from available width.", class_="subtle")
                        dg.Label("Panels use bounded padding and overflow rules.", class_="subtle")
                        dg.Label("Rounded controls get paint breathing room in titled panels.", class_="subtle")
                        dg.ProgressBar(0.72, show_value=True)


    with dg.StatusBar(height=40):
        status = dg.TextInput("Ready", placeholder="status", style={"width": 360})
        dg.Separator(orientation="vertical")
        dg.Label(f"{POINT_ROWS:,} points")
        dg.Label(f"{TABLE_ROWS:,} table rows")
        dg.Spacer()
        dg.Label("All features V3")


    confirm_modal = dg.confirm(
        "Reset Demo State",
        "This modal blocks background input until it is closed.",
        open=False,
        on_confirm=lambda: set_status("Confirmed reset action"),
        on_cancel=lambda: set_status("Cancelled reset action"),
        parent=win,
    )

    about_modal = dg.alert(
        "About DragonGUI",
        "This V3 demo uses responsive grids, CSS themes, Scatter3D, DataFrameTable, modals, menus, context menus, toasts, resources, and live runtime updates.",
        open=False,
        parent=win,
    )

    with dg.ContextMenu(target=table, width=230, parent=win):
        dg.MenuItem("Print Snapshot", on_click=print_snapshot)
        dg.MenuItem("Load Wave Table", on_click=lambda: update_table("wave"))
        dg.MenuItem("Load Cloud Table", on_click=lambda: update_table("cloud"))

    return win



stats_thread = threading.Thread(target=scatter_stats_worker, daemon=True)
stats_thread.start()

try:
    result = app.run(AllFeaturesV3())
except dg.BackendUnavailableError:
    print("DragonGUI source import works.")
    print("Native backend is not built, so this run prints the UI document.")
    pprint(redacted_document(app.document(win)))
else:
    print(result)
finally:
    stats_stop.set()
    stream_cancel.set()
    if stats_thread is not None:
        stats_thread.join(timeout=0.25)
    if stream_controller is not None:
        stream_controller.stop(timeout=0.25)
