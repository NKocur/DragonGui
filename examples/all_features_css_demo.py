from __future__ import annotations

import math
from pathlib import Path
from pprint import pprint
import struct
import sys
import threading
import tempfile
import zlib

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual demo guard
    raise SystemExit("all_features_demo.py requires NumPy for scatter/table data") from exc


ROWS = 80_000


def make_demo_image() -> str:
    width, height = 180, 108
    rows = []
    for y in range(height):
        scanline = bytearray([0])
        for x in range(width):
            r = int(35 + 160 * x / max(1, width - 1))
            g = int(55 + 150 * y / max(1, height - 1))
            b = int(230 - 120 * x / max(1, width - 1))
            a = 255
            scanline.extend((r, g, b, a))
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
        + chunk("IHDR".encode("ascii"), struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk("IDAT".encode("ascii"), zlib.compress(b"".join(rows), 9))
        + chunk("IEND".encode("ascii"), b"")
    )
    path = Path(tempfile.gettempdir()) / "dragongui_all_features_demo.png"
    path.write_bytes(png)
    return str(path)


class DemoFrame:
    columns = ("x", "y", "z", "signal", "score", "row_id", "group", "selected")
    dtypes = ("float32", "float32", "float32", "float32", "float32", "int64", "str", "bool")

    def __init__(self, phase: float = 0.0, mode: str = "helix", rows: int = ROWS) -> None:
        self.shape = (rows, len(self.columns))
        t = np.linspace(0.0, 1.0, rows, dtype=np.float32)
        theta = t * np.float32(math.tau * 14.0 + phase)
        if mode == "wave":
            self.x = (t - np.float32(0.5)) * np.float32(10.0)
            self.y = np.sin(theta) * np.float32(3.0)
            self.z = np.cos(theta * np.float32(0.43)) * np.float32(3.0)
        elif mode == "cloud":
            rng = np.random.default_rng(int(phase * 1000.0) + 19)
            cloud = rng.standard_normal((rows, 3)).astype(np.float32)
            self.x = cloud[:, 0] * np.float32(2.8)
            self.y = cloud[:, 1] * np.float32(2.8)
            self.z = cloud[:, 2] * np.float32(2.8)
        else:
            radius = np.float32(0.8) + np.float32(3.2) * t
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
                out[key] = "<packed float32 xyz data>"
            elif key == "cells" and value:
                out[key] = "<sampled table cells>"
            else:
                out[key] = redacted_document(value)
        return out
    if isinstance(doc, list):
        return [redacted_document(item) for item in doc]
    return doc


CSS_PAPER_LAB = """
:root {
    --panel-radius: 2px;
    --control-radius: 2px;
}

Window {
    background: #f4efe4;
}

Sidebar {
    background: #17202a;
    border-color: #d06b2c;
}

StatusBar,
MenuBar {
    background: #fff9ed;
    border-color: #c9b99f;
}

Panel,
Modal {
    background: #fffaf0;
    border: 1px solid #c9b99f;
    border-radius: var(--panel-radius);
    accent: #d06b2c;
    color: #28313d;
}

Panel.token-panel {
    background: #fff2d8;
    border-color: #d06b2c;
}

Panel.color-picker {
    border-color: #2c8c99;
}

Panel.horizontal-strip {
    width: 250px;
    height: 92px;
    overflow-x: auto;
    overflow-y: hidden;
    padding: 12px;
    gap: 0;
}

HLayout.strip-row {
    width: 392px;
    height: 34px;
    gap: 8px;
    flex-shrink: 0;
}

Label {
    color: #28313d;
}

Label.brand-title {
    color: #ffffff;
    font-size: 22px;
    font-weight: 800;
}

Label.brand-subtitle {
    color: #ffbd75;
    font-weight: 700;
}

Label.snapshot-label {
    color: #b14d11;
    font-weight: 700;
}

Button,
Dropdown,
TextInput,
NumberInput {
    background: #ffffff;
    border: 1px solid #9f8970;
    border-radius: var(--control-radius);
    color: #18202a;
    accent: #d06b2c;
}

Button {
    font-weight: 700;
}

Button.strip-item {
    width: 120px;
    flex-shrink: 0;
}

Button:hover,
Dropdown:hover {
    background: #ffe0b8;
    border-color: #d06b2c;
}

Button:active,
Dropdown:active {
    background: #d06b2c;
    border-color: #8f3f0c;
    color: #ffffff;
}

TextInput:focus,
NumberInput:focus {
    border-color: #2c8c99;
}

Checkbox {
    accent: #2c8c99;
    color: #28313d;
}

Slider {
    accent: #d06b2c;
    track-color: #cfbea6;
    thumb-color: #ffffff;
}

ProgressBar {
    background: #ffffff;
    border-color: #9f8970;
    accent: #2c8c99;
    border-radius: var(--control-radius);
}

NavItem,
Menu {
    accent: #d06b2c;
    border-radius: var(--control-radius);
    color: #ffffff;
}

Tab {
    accent: #d06b2c;
    border-radius: var(--control-radius);
    color: #28313d;
}

Image,
Scatter3D,
DataFrameTable {
    border-color: #9f8970;
    border-width: 1px;
}
"""


CSS_NEON_CONSOLE = """
:root {
    --panel-radius: 14px;
    --control-radius: 10px;
}

Window {
    background: #040617;
}

Sidebar {
    background: #0a1028;
    border-color: #ff36d6;
}

StatusBar,
MenuBar {
    background: #090f26;
    border-color: #00e5ff;
}

Panel,
Modal {
    background: #0d1433;
    border: 1px solid #00e5ff;
    border-radius: var(--panel-radius);
    accent: #ff36d6;
    color: #d7fbff;
}

Panel.token-panel {
    background: #1b0935;
    border-color: #ff36d6;
}

Panel.color-picker {
    border-color: #39ff88;
}

Panel.horizontal-strip {
    width: 250px;
    height: 92px;
    overflow-x: auto;
    overflow-y: hidden;
    padding: 12px;
    gap: 0;
}

HLayout.strip-row {
    width: 392px;
    height: 34px;
    gap: 8px;
    flex-shrink: 0;
}

Label {
    color: #d7fbff;
}

Label.brand-title {
    color: #39ff88;
    font-size: 22px;
    font-weight: 800;
}

Label.brand-subtitle {
    color: #00e5ff;
    font-weight: 700;
}

Label.snapshot-label {
    color: #00e5ff;
    font-weight: 700;
}

Button,
Dropdown,
TextInput,
NumberInput {
    background: #101947;
    border: 1px solid #ff36d6;
    border-radius: var(--control-radius);
    color: #ffffff;
    accent: #ff36d6;
}

Button {
    font-weight: 700;
}

Button.strip-item {
    width: 120px;
    flex-shrink: 0;
}

Button:hover,
Dropdown:hover {
    background: #2c0f58;
    border-color: #00e5ff;
}

Button:active,
Dropdown:active {
    background: #ff36d6;
    border-color: #ff36d6;
    color: #050617;
}

TextInput:focus,
NumberInput:focus {
    border-color: #39ff88;
}

Checkbox {
    accent: #39ff88;
    color: #ffffff;
}

Slider {
    accent: #ff36d6;
    track-color: #22306a;
    thumb-color: #39ff88;
}

ProgressBar {
    background: #101947;
    border-color: #00e5ff;
    accent: #39ff88;
    border-radius: var(--control-radius);
}

Tab,
NavItem,
Menu {
    accent: #ff36d6;
    border-radius: var(--control-radius);
    color: #ffffff;
}

Image,
Scatter3D,
DataFrameTable {
    border-color: #00e5ff;
    border-width: 1px;
}
"""


CSS_THEMES = {
    "paper": CSS_PAPER_LAB,
    "neon": CSS_NEON_CONSOLE,
}


app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33"))
app.stylesheet(CSS_PAPER_LAB)
win = dg.Window("DragonGUI CSS All Features Demo", width=1360, height=860)

stream_stop = threading.Event()
stream_thread: threading.Thread | None = None
demo_state = {"mode": "helix", "phase": 0.0, "style": 0, "children": 0, "css_theme": "paper"}
initial_frame = DemoFrame(mode="helix")
demo_image_path = make_demo_image()


def set_status(message: str) -> None:
    status.set_value(message)


def apply_css_theme(name: str) -> None:
    app.stylesheet(CSS_THEMES[name])
    demo_state["css_theme"] = name
    set_status(f"CSS theme: {name}")


def next_frame(mode: str | None = None) -> DemoFrame:
    if mode is not None:
        demo_state["mode"] = mode
    demo_state["phase"] += 0.35
    return DemoFrame(phase=demo_state["phase"], mode=str(demo_state["mode"]))


def push_scatter(mode: str | None = None) -> None:
    frame = next_frame(mode)
    scatter.set_points(frame, x="x", y="y", z="z")
    progress.set_value((demo_state["phase"] % 6.0) / 6.0)
    set_status(f"Scatter: {demo_state['mode']} phase {demo_state['phase']:.2f}")


def set_scatter_colormap(name: str) -> None:
    scatter.set_colormap(name)
    set_status(f"Colormap: {name}")


def pick_scatter_point(point: dg.ScatterPick) -> None:
    set_status(
        f"Scatter point {point.index}: ({point.x:.2f}, {point.y:.2f}, {point.z:.2f})"
    )


def start_stream() -> None:
    global stream_thread
    if stream_thread is not None and stream_thread.is_alive():
        return
    stream_stop.clear()

    def worker() -> None:
        while not stream_stop.wait(0.75):
            try:
                app.call_soon_threadsafe(push_scatter)
            except RuntimeError:
                break

    stream_thread = threading.Thread(target=worker, daemon=True)
    stream_thread.start()
    set_status("Background stream started")


def stop_stream() -> None:
    stream_stop.set()
    set_status("Background stream stopped")


def update_table(mode: str) -> None:
    frame = DemoFrame(phase=demo_state["phase"] + 0.8, mode=mode, rows=40_000)
    table.set_frame(frame)
    set_status(f"Table frame swapped to {mode}")


def select_table_cell(selection: dg.TableSelection) -> None:
    set_status(
        f"Table selected row {selection.row_index}, {selection.column}: {selection.value}"
    )


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
    color_demo_button.set_style(
        {
            "height": 40,
            "background": selected,
            "border_color": selected,
            "text_align": "center",
        }
    )
    set_status(f"ColorPicker: {selected}")


def show_demo_toast() -> None:
    dg.toast(
        "CSS demo toast",
        level="success",
        duration=2400,
        opacity=0.94,
        radius=12,
        padding=14,
        position="bottom-right",
    )
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
            app.call_soon_threadsafe(lambda: set_status(f"Snapshot printed: {summary['widgets']} widgets"))
        except RuntimeError:
            pass

    threading.Thread(target=worker, daemon=True).start()


def make_summary_children() -> list[object]:
    return [
        dg.Label("Dynamic Summary", parent=None, style={"font_size": 18, "font_weight": "bold"}),
        dg.Separator(parent=None),
        dg.Label(f"Rows: {ROWS:,}", parent=None),
        dg.Label("Columns: x, y, z, signal, score", parent=None),
        dg.Spacer(height=8, parent=None),
        dg.Button("Dynamic Action", parent=None, on_click=lambda: set_status("Dynamic action clicked")),
    ]


def make_pipeline_children() -> list[object]:
    return [
        dg.Label("Pipeline Status", parent=None, style={"font_size": 18, "font_weight": "bold"}),
        dg.Separator(parent=None),
        dg.Label("Load: complete", parent=None, style={"color": "success"}),
        dg.Label("Normalize: complete", parent=None, style={"color": "success"}),
        dg.Label("Cluster: waiting", parent=None, style={"color": "warning"}),
        dg.Spacer(height=8, parent=None),
        dg.Button("Dynamic Callback", parent=None, on_click=lambda: set_status("Inserted callback fired")),
    ]


def swap_children() -> None:
    demo_state["children"] = 1 - int(demo_state["children"])
    if demo_state["children"] == 0:
        dynamic_panel.replace_children(make_summary_children())
        set_status("Replaced children with summary")
    else:
        dynamic_panel.replace_children(make_pipeline_children())
        set_status("Replaced children with pipeline")


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
    style_panel.set_style({"padding": 16, "gap": 12, **current["panel"]})
    style_label.set_style({"font_size": 16, **current["label"]})
    style_button.set_style({"height": 46, "width": 190, **current["button"]})
    set_status("Applied live style patch")


with dg.MenuBar(height=34, tooltip="MenuBar opens native overlay menus."):
    with dg.Menu("File", tooltip="Top-level menus are keyboard focusable."):
        dg.MenuItem("Open CSV...", on_click=choose_csv)
        dg.MenuItem("Print Snapshot", on_click=print_snapshot)
        dg.MenuItem("Upload Buffer", on_click=upload_buffer)
        dg.MenuItem("Release Buffer", on_click=release_buffer)
    with dg.Menu("Stream"):
        dg.MenuItem("Start Scatter Stream", on_click=start_stream)
        dg.MenuItem("Stop Scatter Stream", on_click=stop_stream)
    with dg.Menu("Help"):
        dg.MenuItem("About DragonGUI", on_click=lambda: about_modal.show())


with dg.HLayout(style={"gap": 0}):
    with dg.Sidebar(width=230, style={"padding": 14, "gap": 10}):
        dg.Label("DragonGUI", class_="brand-title")
        dg.Label("CSS edition", class_="brand-subtitle")
        with dg.HLayout(style={"gap": 6, "height": 24}):
            dg.Badge("CSS", level="success")
            dg.Tag("styled", level="info")
        dg.Separator()
        dg.NavItem("Overview", page="overview")
        dg.NavItem("Controls", page="controls")
        dg.NavItem("Data", page="data")
        dg.NavItem("Live Runtime", page="live")
        dg.NavItem("Styling", page="styling")
        dg.NavItem("Disabled", page="disabled", disabled=True)
        dg.Spacer()
        dg.Separator()
        dg.Label("Navigation: Pages + NavItem")

    with dg.Pages(value="overview", on_change=lambda value: set_status(f"Page: {value}")):
        with dg.Page("overview", title="Overview"):
            with dg.HLayout(style={"gap": 16, "padding": 14}):
                with dg.Panel("Scatter controls", width=310, style={"padding": 14, "gap": 10}):
                    mode = dg.Dropdown(
                        ("helix", "wave", "cloud"),
                        value="helix",
                        on_change=push_scatter,
                        key="scatter-mode",
                        tooltip="Choose the generated point-cloud shape.",
                    )
                    dg.Dropdown(
                        ("Viridis", "Magma", "Plasma", "Cividis"),
                        value="Viridis",
                        on_change=set_scatter_colormap,
                        key="scatter-colormap",
                        tooltip="Change the Scatter3D GPU colormap.",
                    )
                    dg.Button("Push Scatter", on_click=lambda: push_scatter(mode.value), tooltip="Upload a new point buffer now.")
                    dg.Button("Start Stream", on_click=start_stream, tooltip="Start a background thread that posts live scatter updates.")
                    dg.Button("Stop Stream", on_click=stop_stream, tooltip="Stop the background scatter update thread.")
                    progress = dg.ProgressBar(0.0, min=0, max=1, show_value=True, tooltip="ProgressBar supports live set_value updates.")
                    dg.Checkbox(
                        "Use GPU point sprites",
                        checked=True,
                        on_change=lambda v: set_status(f"GPU sprites: {v}"),
                        tooltip="Checkbox hover and clicks include the full label row.",
                    )
                    dg.Separator()
                    dg.Label("Mouse drag/wheel orbits plot.", tooltip="Labels can show passive contextual help.")
                scatter = dg.Scatter3D(
                    initial_frame,
                    x="x",
                    y="y",
                    z="z",
                    colormap="viridis",
                    on_pick=pick_scatter_point,
                    key="main-scatter",
                    tooltip="Native GPU scatter widget with live buffer updates.",
                )

        with dg.Page("controls", title="Controls"):
            with dg.HLayout(style={"gap": 16, "padding": 14}):
                with dg.Panel("Form controls", width=330, style={"padding": 14, "gap": 10}):
                    with dg.HLayout(style={"gap": 8, "height": 28}):
                        dg.Badge("live", level="success")
                        dg.Badge("queued", level="warning")
                        dg.Tag("review", level="neutral")
                    dg.TextInput(
                        "editable text",
                        placeholder="Type here",
                        on_change=lambda v: set_status(f"Text: {v}"),
                        tooltip="TextInput supports caret movement, selection focus, and live callbacks.",
                    )
                    dg.Dropdown(
                        ("Low", "Medium", "High"),
                        value="Medium",
                        on_change=lambda v: set_status(f"Dropdown: {v}"),
                        tooltip="Dropdown menus render as native overlays.",
                    )
                    dg.Slider(
                        0.42,
                        min=0,
                        max=1,
                        step=0.02,
                        on_change=lambda v: set_status(f"Slider: {v:.2f}"),
                        tooltip="Drag or click the track to change the value.",
                    )
                    dg.NumberInput(
                        42,
                        min=0,
                        max=100,
                        step=0.5,
                        on_change=lambda v: set_status(f"Number: {v:g}"),
                        tooltip="NumberInput supports text entry plus stepper buttons.",
                    )
                    color_demo_button = dg.Button(
                        "Color target",
                        style={
                            "height": 40,
                            "background": "#3fc7ff",
                            "border_color": "#3fc7ff",
                            "text_align": "center",
                        },
                    )
                    dg.ColorPicker(
                        (63, 199, 255),
                        alpha=False,
                        class_="color-picker",
                        on_change=apply_demo_color,
                        tooltip="ColorPicker is a composite widget built from sliders and a swatch.",
                    )
                    dg.Checkbox("Enable analysis", checked=True, on_change=lambda v: set_status(f"Analysis: {v}"))
                    dg.Button("Regular Button", on_click=lambda: set_status("Button clicked"))
                    dg.Button("Show Toast", badge="new", on_click=show_demo_toast)
                    dg.Button("Disabled Button", disabled=True)
                    dg.TextInput("disabled input", disabled=True)
                    with dg.Collapsible(
                        "Advanced notes",
                        expanded=False,
                        on_change=lambda expanded: set_status(f"Advanced notes: {expanded}"),
                    ):
                        dg.TextArea(
                            "Line one\nLine two\nLine three\nLine four\nLine five",
                            rows=3,
                            wrap=True,
                            on_change=lambda v: set_status(f"Notes length: {len(v)}"),
                        )
                with dg.Panel("Tabs", style={"padding": 14, "gap": 10}):
                    with dg.Tabs(value="one", on_change=lambda v: set_status(f"Tab: {v}")):
                        with dg.Tab("One", value="one"):
                            dg.Label("Tab content one")
                            dg.Separator()
                            dg.Label("Separator above, spacer below.")
                            dg.Spacer(height=10)
                            tab_button = dg.Button("Tab Button", on_click=lambda: set_status("Tab button clicked"))
                            with dg.Tooltip(target=tab_button):
                                dg.Label("Rich tooltip")
                                dg.ProgressBar(0.66, show_value=True)
                        with dg.Tab("Two", value="two"):
                            dg.Label("Tab content two")
                            dg.Checkbox("A checkbox in a tab", checked=False)
                        with dg.Tab("Three", value="three"):
                            dg.Label("Tab content three")
                            dg.Slider(0.7, min=0, max=1, step=0.05)

        with dg.Page("data", title="Data"):
            with dg.HLayout(style={"gap": 16, "padding": 14}):
                with dg.Panel("Data controls", width=310, style={"padding": 14, "gap": 10}):
                    dg.Button("Load Helix Table", on_click=lambda: update_table("helix"))
                    dg.Button("Load Wave Table", on_click=lambda: update_table("wave"))
                    dg.Button("Load Cloud Table", on_click=lambda: update_table("cloud"))
                    dg.Separator()
                    dg.Button("Upload Buffer", on_click=upload_buffer, tooltip="Create a named native buffer resource.")
                    dg.Button("Release Buffer", on_click=release_buffer, tooltip="Release the named native buffer resource.")
                    dg.Button("Confirm Reset", on_click=lambda: confirm_modal.show(), tooltip="Open a modal confirmation overlay.")
                    dg.Button("Print Snapshot", on_click=print_snapshot, tooltip="Print runtime, layout, style, and resource diagnostics.")
                    dg.Label("Table is virtualized native UI.")
                table = dg.DataFrameTable(
                    DemoFrame(rows=40_000),
                    page_size=90,
                    on_select=select_table_cell,
                    key="main-table",
                    tooltip="DataFrameTable virtualizes rows and columns in native code.",
                )

        with dg.Page("live", title="Live Runtime"):
            with dg.HLayout(style={"gap": 16, "padding": 14}):
                with dg.Panel("Live commands", width=310, style={"padding": 14, "gap": 10}):
                    dg.Button("Replace Children", on_click=swap_children, tooltip="Send ReplaceChildren to the native retained tree.")
                    dg.Button("Cycle Style", on_click=cycle_style, tooltip="Send live style patches to existing widgets.")
                    dg.Button("Print Snapshot", on_click=print_snapshot, tooltip="Inspect current retained runtime state.")
                    dg.Label("Commands wake the event loop.")
                    dg.Separator()
                    dg.Label("Includes ReplaceChildren.")
                with dg.Panel(
                    "ReplaceChildren target",
                    style={"padding": 18, "gap": 10, "border_color": "accent", "border_radius": 12},
                ) as dynamic_panel:
                    for child in make_summary_children():
                        dynamic_panel.add(child)

        with dg.Page("styling", title="CSS Styling"):
            with dg.HLayout(style={"gap": 16, "padding": 14}):
                with dg.Panel("CSS theme controls", width=330, class_="token-panel", style={"padding": 14, "gap": 10}):
                    dg.Button("Paper Lab CSS", on_click=lambda: apply_css_theme("paper"))
                    dg.Button("Neon Console CSS", on_click=lambda: apply_css_theme("neon"))
                    dg.Separator()
                    with dg.Panel("Horizontal overflow", class_="horizontal-strip"):
                        with dg.HLayout(class_="strip-row"):
                            dg.Button("First", class_="strip-item")
                            dg.Button("Second", class_="strip-item")
                            dg.Button("Third", class_="strip-item")
                    dg.Separator()
                    dg.Button("Danger token", style={"background": "danger", "border_color": "danger"})
                    dg.Button("Warning token", style={"background": "warning", "border_color": "warning"})
                    dg.Button("Success token", style={"background": "success", "border_color": "success"})
                    dg.Button(
                        "Hover / Press",
                        style={
                            "background": "surface_alt",
                            "border_color": "border",
                            "hover": {"background": "accent_mix_20", "border_color": "accent"},
                            "active": {"background": "accent_dark"},
                            "text_align": "center",
                        },
                    )
                    dg.Label("class_ is retained for snapshots.", class_="snapshot-label")
                with dg.Panel("Live style preview", style={"padding": 16, "gap": 12, **styles[0]["panel"]}) as style_panel:
                    style_label = dg.Label("Styled label", style={"font_size": 16, **styles[0]["label"]})
                    style_button = dg.Button("Styled button", on_click=cycle_style, style={"height": 46, "width": 190, **styles[0]["button"]})
                    dg.Image(
                        demo_image_path,
                        fit="cover",
                        height=170,
                        style={"border_color": "accent", "border_radius": 10},
                        tooltip="Image renders PNG/JPEG files as native textured quads.",
                    )
                    dg.Label("Button cycles this panel style.")

with dg.StatusBar(height=40):
    status = dg.TextInput("Ready", placeholder="status", style={"width": 300})
    dg.Separator(orientation="vertical")
    dg.Label(f"{ROWS:,} rows")
    dg.Spacer()
    dg.Label("All current widgets + live runtime")

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
    "This demo uses native widgets, live commands, modals, menus, tables, and Scatter3D.",
    open=False,
    parent=win,
)

with dg.ContextMenu(target=table, width=230, parent=win):
    dg.MenuItem("Print Snapshot", on_click=print_snapshot)
    dg.MenuItem("Load Wave Table", on_click=lambda: update_table("wave"))
    dg.MenuItem("Load Cloud Table", on_click=lambda: update_table("cloud"))

try:
    result = app.run(win)
except dg.BackendUnavailableError:
    print("DragonGUI source import works.")
    print("Native backend is not built, so this run prints the UI document.")
    pprint(redacted_document(app.document(win)))
else:
    print(result)
finally:
    stream_stop.set()
