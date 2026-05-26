from __future__ import annotations

import math
import os
import sys
import threading
import time
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

from probe_helpers import probe_app, probe_header

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - benchmark dependency
    raise SystemExit("v4_scroll_benchmark_probe.py requires NumPy") from exc


SCROLL_STEPS = max(1, int(os.environ.get("DRAGONGUI_BENCH_SCROLL_STEPS", "120")))
SCROLL_INTERVAL_MS = max(0.0, float(os.environ.get("DRAGONGUI_BENCH_SCROLL_INTERVAL_MS", "8")))
AUTOSTART = os.environ.get("DRAGONGUI_BENCH_AUTOSTART") == "1"
EXIT_ON_DONE = os.environ.get("DRAGONGUI_BENCH_EXIT_ON_DONE", "1") != "0"
SCROLL_ID = "v4-scroll-bench-root"


class DemoFrame:
    columns = ("x", "y", "z", "score", "group", "row_id")
    dtypes = ("float32", "float32", "float32", "float32", "str", "str")

    def __init__(self, rows: int = 14_000) -> None:
        self.shape = (rows, len(self.columns))
        t = np.linspace(0.0, 1.0, rows, dtype=np.float32)
        theta = t * np.float32(math.tau * 9.0)
        self.x = np.cos(theta) * (0.35 + t * 2.4)
        self.y = np.sin(theta * np.float32(1.17)) * (0.6 + t * 2.0)
        self.z = np.sin(theta * np.float32(0.31)) * np.float32(1.2)
        self.score = ((np.sin(theta * np.float32(0.77)) + 1.0) * 0.5).astype(np.float32)
        groups = np.array(["A", "B", "C", "D"], dtype=object)
        self.group = groups[(np.arange(rows) // 251) % len(groups)]
        self.row_id = np.array([f"row-{i:05d}" for i in range(rows)], dtype=object)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


class TableFrame:
    columns = ("metric", "value", "status", "owner")
    dtypes = ("str", "float32", "str", "str")

    def __init__(self, rows: int = 360) -> None:
        self.shape = (rows, len(self.columns))
        i = np.arange(rows, dtype=np.float32)
        self.metric = np.array([f"metric.channel.{idx:03d}" for idx in range(rows)], dtype=object)
        self.value = (np.sin(i * 0.09) * 50.0 + 50.0).astype(np.float32)
        self.status = np.array(["ok", "warn", "ok", "busy", "ok", "error"] * 60, dtype=object)[:rows]
        self.owner = np.array([f"team-{idx % 7}" for idx in range(rows)], dtype=object)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


def heatmap_data() -> np.ndarray:
    x = np.linspace(-2.5, 2.5, 22, dtype=np.float32)
    y = np.linspace(-2.0, 2.0, 16, dtype=np.float32)
    xx, yy = np.meshgrid(x, y)
    return np.exp(-(xx * xx + yy * yy) * 0.45) + np.sin(xx * 2.2) * 0.18


app, win = probe_app("V4 Scroll Benchmark", width=1160, height=780)
app.stylesheet(
    """
    Window {
        background: #0f1725;
        color: rgba(246, 249, 255, 0.94);
        padding: 16px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
    }

    Label.title {
        font-size: 20px;
        font-weight: 850;
        color: #ffffff;
    }

    Label.caption,
    Label.subtle {
        color: rgba(235, 241, 255, 0.70);
        line-height: 1.2;
    }

    Label.metric {
        width: 100%;
        padding: 8px 10px;
        background: rgba(90, 169, 255, 0.12);
        border: 1px solid rgba(90, 169, 255, 0.34);
        border-radius: 8px;
        color: rgba(232, 244, 255, 0.96);
        font-family: "Consolas";
        line-height: 1.22;
    }

    ScrollArea.bench-scroll {
        width: 100%;
        height: 100%;
        min-height: 0;
        padding: 0;
        gap: 14px;
    }

    ScrollArea.bench-scroll::scrollbar-track {
        width: 7px;
        background: rgba(255, 255, 255, 0.10);
        border-radius: 999px;
    }

    ScrollArea.bench-scroll::scrollbar-thumb {
        width: 7px;
        background: rgba(116, 221, 176, 0.72);
        border-radius: 999px;
    }

    GridLayout.cards {
        width: 100%;
        gap: 14px;
        masonry: true;
    }

    Panel.card {
        width: 100%;
        min-width: 0;
        padding: 12px;
        gap: 10px;
        background: rgba(18, 27, 43, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 8px;
    }

    Panel.tall {
        min-height: 420px;
    }

    Toolbar,
    FlowLayout.row {
        width: 100%;
        gap: 8px;
    }

    Scatter3D.chart,
    Heatmap.chart,
    BarChart.chart {
        width: 100%;
        height: 260px;
        min-height: 0;
        background: rgba(4, 8, 18, 0.72);
        border: 1px solid rgba(90, 169, 255, 0.44);
        border-radius: 8px;
    }

    DataFrameTable.table {
        width: 100%;
        height: 230px;
        min-height: 0;
    }
    """
)

frame = DemoFrame()
table = TableFrame()
matrix = heatmap_data()
status_label: dg.Label | None = None
runner_thread: threading.Thread | None = None
stop_event = threading.Event()


def set_status(text: str) -> None:
    if status_label is not None:
        status_label.set_value(text)


def scroll_target_for_step(step: int, max_scroll: float) -> float:
    if SCROLL_STEPS <= 1 or max_scroll <= 0.0:
        return 0.0
    phase = step / (SCROLL_STEPS - 1)
    triangle = phase * 2.0 if phase <= 0.5 else (1.0 - phase) * 2.0
    return max_scroll * triangle


def run_scroll_benchmark() -> None:
    started = time.perf_counter()
    try:
        initial = app.debug_snapshot(timeout_ms=5000)
    except RuntimeError:
        return
    layout = (initial.get("gpu") or {}).get("layout") or {}
    scroll_max_y = (layout.get("scroll_max_y") or {}).get(SCROLL_ID, 0.0)
    try:
        max_scroll = float(scroll_max_y)
    except (TypeError, ValueError):
        max_scroll = 0.0
    set_status(
        f"scroll range: {max_scroll:.1f}px\nsteps: 0/{SCROLL_STEPS}\ninterval: {SCROLL_INTERVAL_MS:.1f} ms"
    )
    for step in range(SCROLL_STEPS):
        if stop_event.is_set():
            break
        target = scroll_target_for_step(step, max_scroll)
        try:
            app.call_soon_threadsafe(lambda y=target: scroll_area.scroll_to(y=y))
        except RuntimeError:
            break
        if step % 20 == 0 or step == SCROLL_STEPS - 1:
            elapsed_ms = (time.perf_counter() - started) * 1000.0
            set_status(
                f"scroll range: {max_scroll:.1f}px\nsteps: {step + 1}/{SCROLL_STEPS}\nelapsed: {elapsed_ms:.1f} ms"
            )
        if SCROLL_INTERVAL_MS > 0.0 and stop_event.wait(SCROLL_INTERVAL_MS / 1000.0):
            break
    try:
        app.debug_snapshot(timeout_ms=5000)
    except RuntimeError:
        pass
    if EXIT_ON_DONE:
        try:
            app.request_exit()
        except RuntimeError:
            pass


def start_scroll_benchmark() -> None:
    global runner_thread
    if runner_thread is not None and runner_thread.is_alive():
        return
    stop_event.clear()
    runner_thread = threading.Thread(target=run_scroll_benchmark, daemon=True)
    runner_thread.start()


def stop_scroll_benchmark() -> None:
    stop_event.set()


with win:
    with dg.VLayout(class_="root"):
        probe_header(
            "V4 Scroll Benchmark",
            "Programmatically scrolls a V4-like page to isolate scrollbar/scroll-area frame cost.",
        )
        status_label = dg.Label("Ready.", class_="metric")
        with dg.HLayout(style={"height": 34, "gap": 8}):
            dg.Button("Run scroll benchmark", on_click=start_scroll_benchmark)
            dg.Button("Stop", on_click=stop_scroll_benchmark)
            dg.Button("Top", on_click=lambda: scroll_area.scroll_to(y=0))

        with dg.ScrollArea(axis="y", id=SCROLL_ID, class_="bench-scroll"):
            with dg.GridLayout(columns=2, min_column_width=430, masonry=True, class_="cards"):
                with dg.Panel("Controls", class_="card"):
                    with dg.Toolbar():
                        for icon in ("play", "pause", "stop", "save", "refresh", "settings"):
                            dg.IconButton(icon, tooltip=icon.title())
                    with dg.FlowLayout(class_="row", row_gap=8):
                        dg.ToggleSwitch("Live acquisition", checked=True)
                        dg.Badge("latency p95", level="warning")
                        dg.Badge("error rate", level="danger")
                        dg.SmallButton("Deploy")
                    dg.RangeSlider((20, 84), min=0, max=100, step=2)
                    dg.DragVector((0.0, 1.5, -2.0), labels=("X", "Y", "Z"), component_width=86)

                with dg.Panel("Property inspector", class_="card"):
                    with dg.Splitter(orientation="horizontal", sizes=(120, "1fr"), min_sizes=(110, 240)):
                        with dg.Pane(min_size=110):
                            dg.Selectable("Inspector", selected=True)
                            dg.Selectable("Metrics")
                            dg.Selectable("Exports")
                        with dg.Pane(min_size=240):
                            dg.PropertyGrid(
                                {
                                    "Name": "Sensor A",
                                    "Enabled": True,
                                    "Mode": "Auto",
                                    "Gain": 0.42,
                                    "Color": "#69b7ff",
                                },
                                schema={
                                    "Mode": {"type": "select", "options": ["Auto", "Manual", "Disabled"]},
                                    "Gain": {"type": "float", "min": 0.0, "max": 1.0, "step": 0.01},
                                    "Color": {"type": "color"},
                                },
                                label_width=84,
                            )

                with dg.Panel("Scatter and heatmap", class_="card tall"):
                    dg.ScatterPlot2D(
                        frame,
                        x="x",
                        y="y",
                        scalars="score",
                        colormap="turbo",
                        scalar_bar=True,
                        scalar_bar_title="score",
                        point_size=3.0,
                        hover=["row_id", "group"],
                        class_="chart",
                    )
                    dg.Heatmap(
                        matrix,
                        x_labels=[f"T{i}" for i in range(matrix.shape[1])],
                        y_labels=[f"L{i}" for i in range(matrix.shape[0])],
                        title="Classifier bins",
                        colormap="magma",
                        class_="chart",
                    )

                with dg.Panel("Table and bars", class_="card tall"):
                    dg.DataFrameTable(
                        table,
                        page_size=36,
                        sample_rows=36,
                        sortable=True,
                        resizable_columns=True,
                        class_="table",
                    )
                    dg.BarChart(
                        labels=["Ingest", "Transform", "Validate", "Export"],
                        values=[78, 64, 52, 38],
                        orientation="horizontal",
                        x_label="jobs",
                        y_label="stage",
                        colors=["#ffbf69"],
                        show_toolbar=True,
                        class_="chart",
                    )

                for section in range(8):
                    with dg.Panel(f"Filler card {section + 1}", class_="card"):
                        dg.Label("These cards make the root scroll range comparable to the V4 tab.", class_="subtle")
                        for row in range(5):
                            with dg.FlowLayout(class_="row", row_gap=6):
                                dg.Badge(f"metric {section}.{row}", level="info")
                                dg.TextInput(f"value-{section}-{row}", style={"flex_grow": 1, "min_width": 0})
                                dg.SmallButton("Apply")


if AUTOSTART:
    def _autostart() -> None:
        try:
            app.call_soon_threadsafe(start_scroll_benchmark)
        except RuntimeError:
            pass

    timer = threading.Timer(0.05, _autostart)
    timer.daemon = True
    timer.start()


try:
    print(app.run(win))
finally:
    stop_event.set()
