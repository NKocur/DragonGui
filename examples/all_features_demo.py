from __future__ import annotations

import math
from pathlib import Path
from pprint import pprint
import sys
import threading

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual demo guard
    raise SystemExit("all_features_demo.py requires NumPy for scatter/table data") from exc


ROWS = 80_000


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


app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff", focus="#ffcc33"))
win = dg.Window("DragonGUI All Features Demo", width=1360, height=860)

stream_stop = threading.Event()
stream_thread: threading.Thread | None = None
demo_state = {"mode": "helix", "phase": 0.0, "style": 0, "children": 0}
initial_frame = DemoFrame(mode="helix")


def set_status(message: str) -> None:
    status.set_value(message)


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


def upload_buffer() -> None:
    payload = np.linspace(0.0, 1.0, 4096, dtype=np.float32)
    app.set_buffer_resource("demo:f32-buffer", payload, kind="f32")
    set_status("Uploaded demo:f32-buffer")


def release_buffer() -> None:
    app.release_resource("demo:f32-buffer")
    set_status("Released demo:f32-buffer")


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


with dg.HLayout(style={"gap": 0}):
    with dg.Sidebar(width=230, style={"padding": 14, "gap": 10, "background": "surface"}):
        dg.Label("DragonGUI", style={"font_size": 20, "font_weight": "bold", "color": "accent"})
        dg.Label("All features", style={"color": "muted_text"})
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
                    )
                    dg.Dropdown(
                        ("Viridis", "Magma", "Plasma", "Cividis"),
                        value="Viridis",
                        on_change=set_scatter_colormap,
                        key="scatter-colormap",
                    )
                    dg.Button("Push Scatter", on_click=lambda: push_scatter(mode.value))
                    dg.Button("Start Stream", on_click=start_stream)
                    dg.Button("Stop Stream", on_click=stop_stream)
                    progress = dg.Slider(0.0, min=0, max=1, step=0.01, on_change=lambda v: set_status(f"Progress {v:.2f}"))
                    dg.Checkbox("Use GPU point sprites", checked=True, on_change=lambda v: set_status(f"GPU sprites: {v}"))
                    dg.Separator()
                    dg.Label("Mouse drag/wheel orbits plot.")
                scatter = dg.Scatter3D(initial_frame, x="x", y="y", z="z", colormap="viridis", key="main-scatter")

        with dg.Page("controls", title="Controls"):
            with dg.HLayout(style={"gap": 16, "padding": 14}):
                with dg.Panel("Form controls", width=330, style={"padding": 14, "gap": 10}):
                    dg.TextInput("editable text", placeholder="Type here", on_change=lambda v: set_status(f"Text: {v}"))
                    dg.Dropdown(("Low", "Medium", "High"), value="Medium", on_change=lambda v: set_status(f"Dropdown: {v}"))
                    dg.Slider(0.42, min=0, max=1, step=0.02, on_change=lambda v: set_status(f"Slider: {v:.2f}"))
                    dg.Checkbox("Enable analysis", checked=True, on_change=lambda v: set_status(f"Analysis: {v}"))
                    dg.Button("Regular Button", on_click=lambda: set_status("Button clicked"))
                    dg.Button("Disabled Button", disabled=True)
                    dg.TextInput("disabled input", disabled=True)
                with dg.Panel("Tabs", style={"padding": 14, "gap": 10}):
                    with dg.Tabs(value="one", on_change=lambda v: set_status(f"Tab: {v}")):
                        with dg.Tab("One", value="one"):
                            dg.Label("Tab content one")
                            dg.Separator()
                            dg.Label("Separator above, spacer below.")
                            dg.Spacer(height=10)
                            dg.Button("Tab Button", on_click=lambda: set_status("Tab button clicked"))
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
                    dg.Button("Upload Buffer", on_click=upload_buffer)
                    dg.Button("Release Buffer", on_click=release_buffer)
                    dg.Button("Print Snapshot", on_click=print_snapshot)
                    dg.Label("Table is virtualized native UI.")
                table = dg.DataFrameTable(DemoFrame(rows=40_000), page_size=90, key="main-table")

        with dg.Page("live", title="Live Runtime"):
            with dg.HLayout(style={"gap": 16, "padding": 14}):
                with dg.Panel("Live commands", width=310, style={"padding": 14, "gap": 10}):
                    dg.Button("Replace Children", on_click=swap_children)
                    dg.Button("Cycle Style", on_click=cycle_style)
                    dg.Button("Print Snapshot", on_click=print_snapshot)
                    dg.Label("Commands wake the event loop.")
                    dg.Separator()
                    dg.Label("Includes ReplaceChildren.")
                with dg.Panel(
                    "ReplaceChildren target",
                    style={"padding": 18, "gap": 10, "border_color": "accent", "border_radius": 12},
                ) as dynamic_panel:
                    for child in make_summary_children():
                        dynamic_panel.add(child)

        with dg.Page("styling", title="Styling"):
            with dg.HLayout(style={"gap": 16, "padding": 14}):
                with dg.Panel("Token styles", width=330, class_="token-panel", style={"padding": 14, "gap": 10}):
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
                    dg.Label("Button cycles this panel style.")

with dg.StatusBar(height=40):
    status = dg.TextInput("Ready", placeholder="status", style={"width": 300})
    dg.Separator(orientation="vertical")
    dg.Label(f"{ROWS:,} rows")
    dg.Spacer()
    dg.Label("All current widgets + live runtime")

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
