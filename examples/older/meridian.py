"""Meridian -- GPU-native data exploration tool.

A 500k-point 3D scatter explorer built with DragonGUI's CSS styling system,
reactive components, live updates, and native GPU rendering.

Run: python examples/meridian.py
"""

from __future__ import annotations

import math
import sys
import threading
import time
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:
    raise SystemExit("meridian.py requires NumPy") from exc


# ---------------------------------------------------------------------------
# Data generation -- 3 gaussian clusters + noise
# ---------------------------------------------------------------------------

def generate_dataset(n: int = 500_000) -> object:
    rng = np.random.default_rng(42)
    n1 = int(n * 0.45)
    n2 = int(n * 0.30)
    n3 = int(n * 0.20)
    n_noise = n - n1 - n2 - n3

    c1 = rng.normal(loc=[0.0, 0.0, 0.0], scale=1.0, size=(n1, 3)).astype(np.float32)
    c2 = rng.normal(loc=[3.0, 2.0, 1.0], scale=0.7, size=(n2, 3)).astype(np.float32)
    c3 = rng.normal(loc=[-2.0, 3.0, -1.0], scale=0.5, size=(n3, 3)).astype(np.float32)
    noise = rng.uniform(-6.0, 6.0, size=(n_noise, 3)).astype(np.float32)

    pts = np.vstack([c1, c2, c3, noise])
    rng.shuffle(pts)

    class Frame:
        columns = ("x", "y", "z")
        shape = (pts.shape[0], 3)
        x = pts[:, 0]
        y = pts[:, 1]
        z = pts[:, 2]

    return Frame()


POINTS = 500_000
frame = generate_dataset(POINTS)


# ---------------------------------------------------------------------------
# CSS design system
# ---------------------------------------------------------------------------

CSS = """\
:root {
    --bg: #0a0a12;
    --surface: #13131f;
    --surface-2: #1c1c2e;
    --surface-3: #26263d;
    --border: #1a1a2a;
    --border-strong: #2a2a44;
    --accent: #6c63ff;
    --accent-2: #ff6584;
    --text: #f0f0f8;
    --text-muted: #6b6b8a;
    --text-dim: #3d3d58;
    --success: #43d48f;
    --warning: #ffbf47;
    --danger: #ff5c7a;
    --radius-sm: 4px;
    --radius-md: 8px;
    --radius-lg: 14px;
}

Window {
    background: var(--bg);
    color: var(--text);
    font-size: 13px;
}

Sidebar {
    background: var(--surface);
    border-color: var(--border);
}

NavItem {
    border-radius: var(--radius-md);
    color: var(--text-muted);
    font-weight: 500;
}
NavItem:hover { background: var(--surface-3); color: var(--text); }

Panel {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--text);
    font-size: 13px;
    padding: 12px;
    gap: 10px;
}
Panel.controls {
    background: var(--surface);
    border-color: var(--border);
    border-radius: 0px;
    padding: 0px;
    gap: 0px;
    width: 320px;
}
Panel.gpu-stats {
    background: var(--surface-2);
    border-color: var(--border);
    padding: 10px;
    gap: 6px;
}
Panel.stat-card {
    flex-grow: 1;
    background: var(--surface-2);
    border-color: var(--border);
    padding: 10px;
    gap: 4px;
}

Collapsible {
    border-color: var(--border);
    color: var(--text);
}

Label { color: var(--text); }
Label.app-title {
    color: var(--text);
    font-size: 16px;
    font-weight: 700;
}
Label.section-title {
    color: var(--text-dim);
    font-size: 10px;
    font-weight: 700;
}
Label.value-display {
    color: var(--text);
    font-size: 22px;
    font-weight: 700;
}
Label.value-unit {
    color: var(--text-muted);
    font-size: 11px;
}

Button {
    background: var(--surface-3);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-md);
    color: var(--text);
    font-weight: 500;
}
Button:hover { border-color: var(--accent); }
Button:active { background: var(--accent); border-color: var(--accent); }
Button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #ffffff;
    font-weight: 600;
}
Button.primary:hover { background: #7b73ff; border-color: #7b73ff; }
Button.ghost {
    background: transparent;
    border-color: var(--border);
    color: var(--text-muted);
}
Button.ghost:hover { border-color: var(--border-strong); color: var(--text); }
Button.danger {
    background: transparent;
    border-color: var(--danger);
    color: var(--danger);
}
Button.danger:hover { background: var(--danger); color: #ffffff; }

Dropdown {
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-md);
    color: var(--text);
}
Dropdown:hover { border-color: var(--accent); }

Slider {
    accent: var(--accent);
    track-color: var(--surface-3);
    thumb-color: #ffffff;
}

Checkbox {
    accent: var(--accent);
    color: var(--text-muted);
}

TextInput {
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--text);
}
TextInput:focus { border-color: var(--accent); }

NumberInput {
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--text);
}
NumberInput:focus { border-color: var(--accent); }

ProgressBar {
    accent: var(--accent);
    background: var(--surface-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
}

Scatter3D {
    border-color: var(--border);
    border-width: 1px;
}

DataFrameTable {
    border-color: var(--border);
    border-width: 1px;
    table-row-height: 24px;
    table-header-height: 28px;
}

StatusBar {
    background: var(--surface);
    border-color: var(--border);
    font-size: 12px;
}

Toast {
    border-radius: var(--radius-lg);
    border: 1px solid var(--border-strong);
}
Toast.success { background: #0f1f18; border-color: #1a3d2c; color: var(--success); }
Toast.error   { background: #1f0f14; border-color: #3d1a24; color: var(--danger); }

Separator { background: var(--border); }

Tab {
    border-radius: var(--radius-md);
    accent: var(--accent);
    color: var(--text-muted);
}
"""


# ---------------------------------------------------------------------------
# App setup
# ---------------------------------------------------------------------------

app = dg.App(
    title="Meridian",
    theme=dg.Theme.dark(
        accent="#6c63ff",
        focus="#6c63ff",
        radius=8,
        spacing=8,
        font_size=13,
    ),
)
app.stylesheet(CSS)
win = dg.Window("Meridian Data Explorer", width=1400, height=900)

rotate_stop = threading.Event()
rotate_thread: threading.Thread | None = None


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def set_status(msg: str) -> None:
    status_input.set_value(msg)


def on_colormap(name: str) -> None:
    scatter.set_colormap(name.lower())
    set_status(f"Colormap: {name}")


def on_pick(pick: dg.ScatterPick) -> None:
    dg.toast(
        f"Point {pick.index}: ({pick.x:.3f}, {pick.y:.3f}, {pick.z:.3f})",
        level="info",
        duration=3000,
        app=app,
    )


def camera_view(name: str) -> None:
    dg.toast(f"Camera: {name} view", level="info", duration=1200, app=app)
    set_status(f"Camera: {name}")


def apply_filters() -> None:
    dg.toast("Filters applied", level="success", duration=1600, app=app)
    set_status("Filters applied")


def reset_filters() -> None:
    def on_confirm() -> None:
        set_status("Filters reset to defaults")
        dg.toast("Reset complete", level="success", duration=1400, app=app)

    confirm_modal.show()


def do_export() -> None:
    handle = dg.toast("Exporting...", level="info", duration=None, app=app)
    set_status("Exporting...")

    def worker() -> None:
        time.sleep(1.5)
        try:
            app.call_soon_threadsafe(lambda: (
                handle.update("Export complete", level="success", duration=2000),
                set_status("Export complete"),
            ))
        except RuntimeError:
            pass

    threading.Thread(target=worker, daemon=True).start()


def toggle_rotate(checked: bool) -> None:
    global rotate_thread
    if checked:
        rotate_stop.clear()

        def worker() -> None:
            tick = 0
            while not rotate_stop.wait(0.8):
                tick += 1
                try:
                    app.call_soon_threadsafe(
                        lambda t=tick: set_status(f"Auto-rotate tick {t}"),
                        coalesce_key="meridian.auto-rotate-status.latest",
                    )
                except RuntimeError:
                    break

        rotate_thread = threading.Thread(target=worker, daemon=True)
        rotate_thread.start()
        dg.toast("Auto-rotate started", level="info", duration=1200, app=app)
    else:
        rotate_stop.set()
        set_status("Auto-rotate stopped")


def show_snapshot() -> None:
    def worker() -> None:
        try:
            snap = app.debug_snapshot()
            gpu = snap.get("gpu", {})
            count = gpu.get("renderer", {}).get("widget_count", "?")
            frames = snap.get("runtime", {}).get("frames_rendered", "?")
            app.call_soon_threadsafe(
                lambda: set_status(f"Widgets: {count}, Frames: {frames}"),
                coalesce_key="meridian.snapshot-status.latest",
            )
        except RuntimeError:
            pass

    threading.Thread(target=worker, daemon=True).start()


# ---------------------------------------------------------------------------
# Widget tree
# ---------------------------------------------------------------------------

with dg.MenuBar(height=30):
    with dg.Menu("File"):
        dg.MenuItem("Export...", on_click=do_export)
        dg.MenuItem("Debug Snapshot", on_click=show_snapshot)
    with dg.Menu("View"):
        dg.MenuItem("XY Plane", on_click=lambda: camera_view("XY"))
        dg.MenuItem("XZ Plane", on_click=lambda: camera_view("XZ"))
        dg.MenuItem("YZ Plane", on_click=lambda: camera_view("YZ"))

with dg.HLayout(style={"gap": 0}):

    # -- Left sidebar -------------------------------------------------------
    with dg.Sidebar(width=280, style={"padding": 16, "gap": 8}):
        with dg.HLayout(style={"gap": 10, "height": 36}):
            dg.Badge("M", level="info")
            dg.Label("Meridian", class_="app-title")
            dg.Tag("v0.1", level="info")
        dg.Separator()
        dg.NavItem("Explore", page="explore", badge=f"{POINTS // 1000}k")
        dg.NavItem("Cluster", page="cluster", badge="3")
        dg.NavItem("Statistics", page="stats", badge="12")
        dg.NavItem("Export", page="export")
        dg.Spacer()
        dg.Separator()
        with dg.Panel(class_="gpu-stats"):
            dg.Label("RUNTIME", class_="section-title")
            with dg.HLayout(style={"gap": 8, "height": 22}):
                dg.Label(f"{POINTS // 1000}k pts")
                dg.Badge("GPU", level="success")

    # -- Center scatter + pages ---------------------------------------------
    with dg.Pages(value="explore", on_change=lambda v: set_status(f"Page: {v}")):

        with dg.Page("explore"):
            scatter = dg.Scatter3D(
                frame,
                x="x", y="y", z="z",
                colormap="plasma",
                on_pick=on_pick,
                tooltip="Left-drag to orbit. Wheel to zoom. Click a point to pick.",
            )

        with dg.Page("cluster"):
            with dg.Panel(style={"padding": 20, "gap": 14}):
                dg.Label("Cluster Analysis", class_="app-title")
                dg.Label("3 clusters detected via density estimation.", class_="value-unit")
                dg.Separator()
                with dg.HLayout(style={"gap": 12}):
                    with dg.Panel(class_="stat-card"):
                        dg.Label("CLUSTER 1", class_="section-title")
                        dg.Label(f"{int(POINTS * 0.45):,}", class_="value-display")
                        dg.Label("points", class_="value-unit")
                    with dg.Panel(class_="stat-card"):
                        dg.Label("CLUSTER 2", class_="section-title")
                        dg.Label(f"{int(POINTS * 0.30):,}", class_="value-display")
                        dg.Label("points", class_="value-unit")
                    with dg.Panel(class_="stat-card"):
                        dg.Label("CLUSTER 3", class_="section-title")
                        dg.Label(f"{int(POINTS * 0.20):,}", class_="value-display")
                        dg.Label("points", class_="value-unit")

        with dg.Page("stats"):
            with dg.Panel(style={"padding": 20, "gap": 14}):
                dg.Label("Distribution Statistics", class_="app-title")
                dg.Separator()
                with dg.HLayout(style={"gap": 12}):
                    with dg.Panel(class_="stat-card"):
                        dg.Label("MEAN X", class_="section-title")
                        dg.Label(f"{float(frame.x.mean()):.3f}", class_="value-display")
                        dg.Label("units", class_="value-unit")
                    with dg.Panel(class_="stat-card"):
                        dg.Label("STD DEV", class_="section-title")
                        dg.Label(f"{float(frame.x.std()):.3f}", class_="value-display")
                        dg.Label("sigma", class_="value-unit")
                with dg.HLayout(style={"gap": 12}):
                    with dg.Panel(class_="stat-card"):
                        dg.Label("DENSITY", class_="section-title")
                        dg.Label(f"{POINTS / 216:.0f}", class_="value-display")
                        dg.Label("pts / unit^3", class_="value-unit")
                    with dg.Panel(class_="stat-card"):
                        dg.Label("CLUSTERS", class_="section-title")
                        dg.Label("3", class_="value-display")
                        dg.Label("detected", class_="value-unit")
                dg.Label("DATA QUALITY", class_="section-title")
                dg.ProgressBar(0.87, show_value=True)

        with dg.Page("export"):
            with dg.Panel(style={"padding": 20, "gap": 14}):
                dg.Label("Export Pipeline", class_="app-title")
                dg.Separator()
                dg.Label("FILENAME", class_="section-title")
                dg.TextInput("meridian_export", placeholder="output filename")
                dg.Label("FORMAT", class_="section-title")
                dg.Dropdown(("PNG", "SVG", "CSV", "JSON"), value="CSV")
                dg.Separator()
                dg.Button("Export", class_="primary", on_click=do_export)
                dg.Button("Copy to Clipboard", class_="ghost",
                          on_click=lambda: dg.toast("Copied", level="success", duration=1200, app=app))

    # -- Right controls panel -----------------------------------------------
    with dg.Panel(class_="controls"):
        with dg.Collapsible("Visualization", expanded=True):
            dg.Label("COLORMAP", class_="section-title")
            dg.Dropdown(
                ("Viridis", "Plasma", "Inferno", "Magma", "Coolwarm", "Turbo"),
                value="Plasma",
                on_change=on_colormap,
            )
            dg.Label("POINT SIZE", class_="section-title")
            dg.Slider(1.5, min=0.5, max=5.0, step=0.1)
            dg.Label("OPACITY", class_="section-title")
            dg.Slider(0.85, min=0.0, max=1.0, step=0.01)
            with dg.HLayout(style={"gap": 6}):
                dg.Button("XY", class_="ghost", on_click=lambda: camera_view("XY"))
                dg.Button("XZ", class_="ghost", on_click=lambda: camera_view("XZ"))
                dg.Button("YZ", class_="ghost", on_click=lambda: camera_view("YZ"))
            dg.Button("Reset Camera", class_="ghost",
                      on_click=lambda: dg.toast("Camera reset", level="info", duration=1000, app=app))

        with dg.Collapsible("Filters", expanded=True):
            dg.Label("X RANGE", class_="section-title")
            with dg.HLayout(style={"gap": 6}):
                dg.NumberInput(-6.0, min=-10, max=10, step=0.5)
                dg.NumberInput(6.0, min=-10, max=10, step=0.5)
            dg.Label("Y RANGE", class_="section-title")
            with dg.HLayout(style={"gap": 6}):
                dg.NumberInput(-6.0, min=-10, max=10, step=0.5)
                dg.NumberInput(6.0, min=-10, max=10, step=0.5)
            dg.Checkbox("Show Outliers", checked=True)
            dg.Checkbox("Auto-rotate", checked=False, on_change=toggle_rotate)
            dg.Button("Apply Filters", class_="primary", on_click=apply_filters)
            dg.Button("Reset", class_="ghost", on_click=reset_filters)

        with dg.Collapsible("Statistics", expanded=False):
            with dg.HLayout(style={"gap": 10}):
                with dg.Panel(class_="stat-card"):
                    dg.Label("MEAN X", class_="section-title")
                    dg.Label(f"{float(frame.x.mean()):.2f}", class_="value-display")
                    dg.Label("units", class_="value-unit")
                with dg.Panel(class_="stat-card"):
                    dg.Label("STD DEV", class_="section-title")
                    dg.Label(f"{float(frame.x.std()):.2f}", class_="value-display")
                    dg.Label("sigma", class_="value-unit")
            with dg.HLayout(style={"gap": 10}):
                with dg.Panel(class_="stat-card"):
                    dg.Label("DENSITY", class_="section-title")
                    dg.Label(f"{POINTS / 216:.0f}", class_="value-display")
                    dg.Label("pts/u^3", class_="value-unit")
                with dg.Panel(class_="stat-card"):
                    dg.Label("CLUSTERS", class_="section-title")
                    dg.Label("3", class_="value-display")
                    dg.Label("detected", class_="value-unit")
            dg.Label("DATA QUALITY", class_="section-title")
            dg.ProgressBar(0.87, show_value=True)

        with dg.Collapsible("Export", expanded=False):
            dg.TextInput("meridian_export", placeholder="filename")
            dg.Dropdown(("PNG", "SVG", "CSV", "JSON"), value="CSV")
            dg.Button("Export", class_="primary", on_click=do_export)
            dg.Button("Copy to Clipboard", class_="ghost",
                      on_click=lambda: dg.toast("Copied", level="success", duration=1200, app=app))


# -- Status bar -------------------------------------------------------------
with dg.StatusBar(height=28):
    dg.Label(f"{POINTS:,} points  |  3 clusters detected")
    dg.Spacer()
    status_input = dg.TextInput("Ready", placeholder="status", style={"width": 260})
    dg.Spacer()
    dg.Badge("GPU ACTIVE", level="success")
    dg.Label("  |  ", style={"color": "#3d3d58"})
    dg.Badge("wgpu", level="info")


# -- Modals -----------------------------------------------------------------
confirm_modal = dg.confirm(
    "Reset Filters",
    "Reset all filter values to defaults?",
    open=False,
    on_confirm=lambda: (
        set_status("Filters reset to defaults"),
        dg.toast("Reset complete", level="success", duration=1400, app=app),
    ),
    on_cancel=lambda: set_status("Reset cancelled"),
    parent=win,
)


# ---------------------------------------------------------------------------
# Run
# ---------------------------------------------------------------------------

try:
    result = app.run(win)
except dg.BackendUnavailableError:
    print("Native backend not built. Run: maturin develop --manifest-path native/Cargo.toml")
else:
    print(result)
finally:
    rotate_stop.set()
