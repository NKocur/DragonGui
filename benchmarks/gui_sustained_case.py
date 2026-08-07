"""Run one sustained-work benchmark in an isolated GUI process.

This complements ``gui_framework_case.py``.  Startup is intentionally outside
the principal result: operations begin only after the first usable frame and
the report separates Python/API submission from event-loop/frame work.
"""

from __future__ import annotations

import argparse
import importlib.metadata
import json
import math
import os
from pathlib import Path
import platform
import statistics
import sys
import threading
import time
from typing import Any, Callable

from gui_framework_case import _percentile, _rss_bytes, _timings
from gui_benchmark_validation import (
    ValidationRecorder,
    add_common_runtime_checks,
    find_tree_node,
)


ROOT = Path(__file__).resolve().parents[1]
TARGET_FRAME_S = 1.0 / 60.0
FRAMEWORK_NAMES = {
    "dragongui": "DragonGUI",
    "dearpygui": "Dear PyGui",
    "pyqt6": "PyQt6",
}


def _version(distribution: str) -> str:
    try:
        return importlib.metadata.version(distribution)
    except importlib.metadata.PackageNotFoundError:
        return "unknown"


def _pace(started: float) -> None:
    remaining = TARGET_FRAME_S - (time.perf_counter() - started)
    if remaining > 0:
        time.sleep(remaining)


def _series(size: int, phase: float = 0.0) -> tuple[Any, Any]:
    import numpy as np

    x = np.linspace(0.0, 80.0, size, dtype=np.float32)
    y = (np.sin(x + phase) + np.cos(x * 0.23 - phase) * 0.25).astype(np.float32)
    return x, y


def _table_data(rows: int, phase: int = 0) -> dict[str, Any]:
    import numpy as np

    index = np.arange(rows)
    return {
        "id": index,
        "group": (index + phase) % 17,
        "value_a": np.sin(index * 0.013 + phase).astype(np.float64),
        "value_b": np.cos(index * 0.007 - phase).astype(np.float64),
        "value_c": ((index * 37 + phase) % 1000).astype(np.int64),
        "active": ((index + phase) % 3 == 0),
    }


class _ArrayFrame:
    """Minimal column-oriented dataframe contract understood by DragonGUI."""

    def __init__(self, columns: dict[str, Any]) -> None:
        self._columns = columns
        self.columns = tuple(columns)
        self.dtypes = tuple(str(value.dtype) for value in columns.values())
        self.shape = (len(next(iter(columns.values()), ())), len(columns))

    def __getitem__(self, column: str) -> Any:
        return self._columns[column]


def _base(args: argparse.Namespace, framework: str, version: str) -> dict[str, Any]:
    return {
        "schema": 1,
        "framework": framework,
        "framework_version": version,
        "python": platform.python_version(),
        "platform": platform.platform(),
        "workload": args.workload,
        "scale": args.scale,
        "operations_requested": args.operations,
        "supported": True,
        "support_note": None,
    }


def _unsupported(args: argparse.Namespace, framework: str, version: str, note: str) -> dict[str, Any]:
    report = _base(args, framework, version)
    report.update({"supported": False, "support_note": note})
    return report


def _dragon_controls(dg: Any, rows: int) -> None:
    with dg.ScrollArea(axis="y", style={"height": "100%", "gap": 4}):
        for index in range(rows):
            with dg.HLayout(style={"height": 32, "gap": 6}):
                dg.Label(f"Row {index:04d}", style={"width": 130})
                dg.TextInput(f"Value {index}", style={"width": 280})
                dg.Button("Action", style={"width": 100})
                dg.ProgressBar((index % 100) / 100.0, style={"width": 220})


def run_dragongui(args: argparse.Namespace) -> dict[str, Any]:
    sys.path.insert(0, str(ROOT / "python"))
    import dragongui as dg

    os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(max(180, args.operations + 120))
    app = dg.App(loading_screen=False)
    window = dg.Window("Sustained GUI benchmark", width=1000, height=720)
    target: Any = None
    table_validation: dict[str, Any] | None = None
    data_prepare_ms = 0.0
    build_started = time.perf_counter()
    if args.workload in {"restyle", "resize"}:
        _dragon_controls(dg, args.scale)
    elif args.workload == "line_replace":
        prepared = time.perf_counter()
        x, y = _series(args.scale)
        frame = {"x": x, "y": y}
        data_prepare_ms = (time.perf_counter() - prepared) * 1000.0
        target = dg.LinePlot(frame, x="x", y="y", show_toolbar=False, style={"height": "100%"})
    elif args.workload == "table_model":
        prepared = time.perf_counter()
        frame = _ArrayFrame(_table_data(args.scale))
        data_prepare_ms = (time.perf_counter() - prepared) * 1000.0
        # Keep a visible first page in the document as a correctness sentinel;
        # packed column buffers still carry the full virtualized data set.
        target = dg.DataFrameTable(frame, sample_rows=100, page_size=100, style={"height": "100%"})
        table_validation = {
            "rows": target.frame_summary.rows,
            "columns": list(target.frame_summary.columns),
            "visible_sample_rows": len(target.cells),
            "buffer_columns": len(target.column_buffers),
            "first_visible_row": list(target.cells[0]) if target.cells else None,
        }
    else:
        raise ValueError(args.workload)
    build_ms = (time.perf_counter() - build_started) * 1000.0

    submit_samples: list[float] = []
    completed = 0
    operation_start_after_run_ms: float | None = None
    run_started = 0.0

    css_a = ":root { --bench-accent: #57d6ff; } Button { background: #17354a; color: #eefaff; }"
    css_b = ":root { --bench-accent: #ffb45c; } Button { background: #4a2b17; color: #fff8ee; }"

    def producer() -> None:
        nonlocal completed, operation_start_after_run_ms
        deadline = time.perf_counter() + 20.0
        while time.perf_counter() < deadline:
            try:
                readiness = app.debug_snapshot(timeout_ms=2000).get("runtime", {}).get("startup_readiness")
            except (RuntimeError, TimeoutError):
                readiness = None
            if readiness == "application_frame_presented":
                break
            time.sleep(0.01)
        operation_start_after_run_ms = (time.perf_counter() - run_started) * 1000.0
        for iteration in range(args.operations):
            def apply(iteration: int = iteration) -> None:
                nonlocal completed
                started = time.perf_counter()
                if args.workload == "restyle":
                    app.set_stylesheet("benchmark", css_a if iteration % 2 else css_b)
                elif args.workload == "resize":
                    handle = getattr(app, "_handle", None)
                    if handle is not None:
                        width, height = ((760, 520), (1180, 820))[iteration % 2]
                        handle.request_window_resize(width, height)
                elif args.workload == "line_replace":
                    x, y = _series(args.scale, iteration * 0.08)
                    target.set_data({"x": x, "y": y}, x="x", y="y", fit=False)
                elif args.workload == "table_model":
                    target.set_frame(_ArrayFrame(_table_data(args.scale, iteration)))
                submit_samples.append((time.perf_counter() - started) * 1000.0)
                completed += 1

            app.call_soon_threadsafe(apply)
            time.sleep(TARGET_FRAME_S)

    rss_before = _rss_bytes()
    run_started = time.perf_counter()
    threading.Thread(target=producer, name="sustained-benchmark", daemon=True).start()
    result = app.run(window)
    run_ms = (time.perf_counter() - run_started) * 1000.0
    snapshot = result.get("debug_snapshot") or {}
    runtime = snapshot.get("runtime") or {}
    gpu = snapshot.get("gpu") or {}
    framework = gpu.get("framework") or {}
    renderer = gpu.get("renderer") or {}
    resources = gpu.get("resources") or {}
    validation = ValidationRecorder()
    smoke_frames = max(180, args.operations + 120)
    expected_widgets = 2 + args.scale * 5 if args.workload in {"restyle", "resize"} else 2
    add_common_runtime_checks(
        validation,
        snapshot,
        expected_widgets=expected_widgets,
        minimum_frames=smoke_frames,
    )
    validation.equal(
        "all requested operations completed",
        completed,
        args.operations,
        source="Python benchmark producer",
    )
    if args.workload == "restyle":
        sheets = gpu.get("stylesheets") or {}
        selector_matches = sheets.get("user_selector_matches") or {}
        validation.equal(
            "button selector matched every benchmark button",
            selector_matches.get("Button"),
            args.scale,
            source="native stylesheet diagnostics",
        )
        validation.equal(
            "live stylesheet replacement remained error-free",
            sheets.get("last_error"),
            None,
            source="native stylesheet diagnostics",
        )
    elif args.workload == "resize":
        expected_size = ((760, 520), (1180, 820))[(args.operations - 1) % 2]
        native_window = gpu.get("window") or {}
        scale_factor = float(native_window.get("scale_factor") or 1.0)
        expected_physical = tuple(round(value * scale_factor) for value in expected_size)
        validation.equal(
            "native window reached scaled final requested width",
            native_window.get("width"),
            expected_physical[0],
            source="native GPU window snapshot",
        )
        validation.equal(
            "native window reached scaled final requested height",
            native_window.get("height"),
            expected_physical[1],
            source="native GPU window snapshot",
        )
    elif args.workload == "line_replace":
        native_node = find_tree_node(gpu.get("tree"), target.id)
        series = ((native_node or {}).get("props", {}).get("line_plot", {}).get("series") or [])
        line_metrics = (resources.get("line_plots") or {}).get(target.id) or {}
        line_renderer = renderer.get("line_plot_renderer") or {}
        validation.equal(
            "native retained line series point count",
            series[0].get("points") if series else None,
            args.scale,
            source="native retained tree",
        )
        validation.equal(
            "native line resource point count",
            line_metrics.get("last_point_count"),
            args.scale,
            source="native line resource metrics",
        )
        validation.equal(
            "line renderer source point count",
            line_renderer.get("source_point_count"),
            args.scale,
            source="native line renderer snapshot",
        )
        validation.equal(
            "line renderer retained one series",
            line_renderer.get("series_count"),
            1,
            source="native line renderer snapshot",
        )
    elif args.workload == "table_model":
        table_registry = (((resources.get("registry") or {}).get("tables") or {}).get("items") or {})
        native_table = table_registry.get(target.resource_id) or {}
        final_group = str((args.operations - 1) % 17)
        validation.equal(
            "Python table retains the requested row count",
            target.frame_summary.rows,
            args.scale,
            source="Python dataframe summary",
        )
        validation.equal(
            "Python table keeps a visible correctness sample",
            len(target.cells),
            min(100, args.scale),
            source="Python table payload",
        )
        validation.equal(
            "Python table first row reflects final replacement",
            target.cells[0][1] if target.cells else None,
            final_group,
            source="Python table payload",
        )
        validation.equal(
            "native table registry row count",
            native_table.get("rows"),
            args.scale,
            source="native resource registry",
        )
        validation.equal(
            "native table registry packed every column",
            native_table.get("buffer_columns"),
            6,
            source="native resource registry",
        )
        validation.equal(
            "native table registry retains visible sample rows",
            native_table.get("sample_rows"),
            min(100, args.scale),
            source="native resource registry",
        )
    validation_report = validation.report()
    report = _base(args, "DragonGUI", _version("dragongui"))
    report.update({
        "data_prepare_ms": data_prepare_ms,
        "build_ms": build_ms,
        "run_wall_ms": run_ms,
        "rss_before_run_bytes": rss_before,
        "rss_after_run_bytes": _rss_bytes(),
        "operation_start_after_run_ms": operation_start_after_run_ms,
        "operations_completed": completed,
        "submit_ms": _timings(submit_samples),
        "active_frame_ms": runtime.get("frame_timings", {}).get("work", {}),
        "native": {
            "widget_count": renderer.get("widget_count"),
            "command_drain": runtime.get("command_drain"),
            "style_reapply": framework.get("style_reapply"),
            "layout_compute": framework.get("layout_compute"),
            "text_rebuild": framework.get("text_rebuild"),
        },
        "table_validation": table_validation,
        "validation": validation_report,
    })
    return report


def _qt_controls(QtWidgets: Any, rows: int) -> Any:
    content = QtWidgets.QWidget()
    layout = QtWidgets.QVBoxLayout(content)
    layout.setContentsMargins(8, 8, 8, 8)
    layout.setSpacing(4)
    for index in range(rows):
        row = QtWidgets.QWidget()
        row_layout = QtWidgets.QHBoxLayout(row)
        row_layout.setContentsMargins(0, 0, 0, 0)
        row_layout.addWidget(QtWidgets.QLabel(f"Row {index:04d}"))
        row_layout.addWidget(QtWidgets.QLineEdit(f"Value {index}"))
        row_layout.addWidget(QtWidgets.QPushButton("Action"))
        row_layout.addWidget(QtWidgets.QProgressBar())
        layout.addWidget(row)
    scroll = QtWidgets.QScrollArea()
    scroll.setWidgetResizable(True)
    scroll.setWidget(content)
    return scroll


def run_pyqt6(args: argparse.Namespace) -> dict[str, Any]:
    from PyQt6 import QtCore, QtGui, QtWidgets

    app = QtWidgets.QApplication.instance() or QtWidgets.QApplication([])
    window = QtWidgets.QMainWindow()
    window.resize(1000, 720)
    target: Any = None
    data_prepare_ms = 0.0
    build_started = time.perf_counter()

    if args.workload in {"restyle", "resize"}:
        target = _qt_controls(QtWidgets, args.scale)
    elif args.workload == "line_replace":
        x, y = _series(args.scale)

        class PlotWidget(QtWidgets.QWidget):
            def __init__(self) -> None:
                super().__init__()
                self.points = QtGui.QPolygonF()
                self.set_data(x, y)

            def set_data(self, xs: Any, ys: Any) -> None:
                self.points = QtGui.QPolygonF([
                    QtCore.QPointF(float(px) * 10.0, 300.0 - float(py) * 100.0)
                    for px, py in zip(xs, ys)
                ])
                self.update()

            def paintEvent(self, event: Any) -> None:
                painter = QtGui.QPainter(self)
                painter.setRenderHint(QtGui.QPainter.RenderHint.Antialiasing, False)
                painter.setPen(QtGui.QPen(QtGui.QColor("#57d6ff"), 1.5))
                painter.drawPolyline(self.points)

        target = PlotWidget()
    elif args.workload == "table_model":
        prepared = time.perf_counter()
        values = _table_data(args.scale)
        data_prepare_ms = (time.perf_counter() - prepared) * 1000.0

        class ArrayModel(QtCore.QAbstractTableModel):
            def __init__(self, columns: dict[str, Any]) -> None:
                super().__init__()
                self.columns = columns
                self.names = list(columns)

            def rowCount(self, parent: Any = QtCore.QModelIndex()) -> int:
                return args.scale

            def columnCount(self, parent: Any = QtCore.QModelIndex()) -> int:
                return len(self.names)

            def data(self, index: Any, role: int = int(QtCore.Qt.ItemDataRole.DisplayRole)) -> Any:
                if role == QtCore.Qt.ItemDataRole.DisplayRole:
                    return str(self.columns[self.names[index.column()]][index.row()])
                return None

            def replace(self, columns: dict[str, Any]) -> None:
                self.beginResetModel()
                self.columns = columns
                self.endResetModel()

        model = ArrayModel(values)
        target = QtWidgets.QTableView()
        target.setModel(model)
        target.verticalHeader().setDefaultSectionSize(24)
    else:
        raise ValueError(args.workload)
    window.setCentralWidget(target)
    build_ms = (time.perf_counter() - build_started) * 1000.0
    window.show()
    app.processEvents()
    rss_before = _rss_bytes()
    submit: list[float] = []
    frames: list[float] = []
    run_started = time.perf_counter()
    for iteration in range(args.operations):
        frame_started = time.perf_counter()
        started = time.perf_counter()
        if args.workload == "restyle":
            window.setStyleSheet(
                ("QPushButton { background:#17354a; color:#eefaff; }" if iteration % 2 else
                 "QPushButton { background:#4a2b17; color:#fff8ee; }")
            )
        elif args.workload == "resize":
            window.resize(*(((760, 520), (1180, 820))[iteration % 2]))
        elif args.workload == "line_replace":
            x, y = _series(args.scale, iteration * 0.08)
            target.set_data(x, y)
        else:
            model.replace(_table_data(args.scale, iteration))
        submit.append((time.perf_counter() - started) * 1000.0)
        target.repaint()
        app.processEvents(QtCore.QEventLoop.ProcessEventsFlag.AllEvents)
        frames.append((time.perf_counter() - frame_started) * 1000.0)
        _pace(frame_started)
    report = _base(args, "PyQt6", _version("PyQt6"))
    report.update({
        "data_prepare_ms": data_prepare_ms,
        "build_ms": build_ms,
        "run_wall_ms": (time.perf_counter() - run_started) * 1000.0,
        "rss_before_run_bytes": rss_before,
        "rss_after_run_bytes": _rss_bytes(),
        "operations_completed": args.operations,
        "submit_ms": _timings(submit),
        "active_frame_ms": _timings(frames),
    })
    window.close()
    app.processEvents()
    return report


def run_dearpygui(args: argparse.Namespace) -> dict[str, Any]:
    # Prefer wheels installed in the active benchmark environment; retain the
    # bundle only as a fallback for optional dependencies.
    sys.path.append(str(ROOT / "artifacts" / "benchmark-deps"))
    import dearpygui.dearpygui as dpg

    if args.workload == "table_model" and args.scale > 5000:
        return _unsupported(
            args, "Dear PyGui", _version("dearpygui"),
            "Public table rows are eagerly authored; above 5,000 rows is omitted to avoid benchmarking allocation failure rather than viewport work.",
        )
    dpg.create_context()
    dpg.create_viewport(title="Sustained GUI benchmark", width=1000, height=720)
    data_prepare_ms = 0.0
    build_started = time.perf_counter()
    theme_a = theme_b = None
    with dpg.window(tag="primary"):
        if args.workload in {"restyle", "resize"}:
            for index in range(args.scale):
                with dpg.group(horizontal=True):
                    dpg.add_text(f"Row {index:04d}")
                    dpg.add_input_text(default_value=f"Value {index}", width=280)
                    dpg.add_button(label="Action", width=100)
                    dpg.add_progress_bar(default_value=(index % 100) / 100.0, width=220)
            with dpg.theme() as theme_a:
                with dpg.theme_component(dpg.mvButton):
                    dpg.add_theme_color(dpg.mvThemeCol_Button, (23, 53, 74))
            with dpg.theme() as theme_b:
                with dpg.theme_component(dpg.mvButton):
                    dpg.add_theme_color(dpg.mvThemeCol_Button, (74, 43, 23))
        elif args.workload == "line_replace":
            x, y = _series(args.scale)
            with dpg.plot(height=-1, width=-1):
                dpg.add_plot_axis(dpg.mvXAxis, tag="x_axis")
                with dpg.plot_axis(dpg.mvYAxis, tag="y_axis"):
                    dpg.add_line_series(x.tolist(), y.tolist(), tag="series")
        elif args.workload == "table_model":
            prepared = time.perf_counter()
            values = _table_data(args.scale)
            data_prepare_ms = (time.perf_counter() - prepared) * 1000.0
            with dpg.table(header_row=True, scrollY=True, height=-1):
                for name in values:
                    dpg.add_table_column(label=name)
                for row in range(args.scale):
                    with dpg.table_row():
                        for name in values:
                            dpg.add_text(str(values[name][row]))
        else:
            raise ValueError(args.workload)
    dpg.set_primary_window("primary", True)
    dpg.setup_dearpygui()
    dpg.set_viewport_vsync(False)
    build_ms = (time.perf_counter() - build_started) * 1000.0
    dpg.show_viewport()
    dpg.render_dearpygui_frame()
    rss_before = _rss_bytes()
    submit: list[float] = []
    frames: list[float] = []
    run_started = time.perf_counter()
    for iteration in range(args.operations):
        frame_started = time.perf_counter()
        started = time.perf_counter()
        if args.workload == "restyle":
            dpg.bind_theme(theme_a if iteration % 2 else theme_b)
        elif args.workload == "resize":
            width, height = ((760, 520), (1180, 820))[iteration % 2]
            dpg.configure_viewport(0, width=width, height=height)
        elif args.workload == "line_replace":
            x, y = _series(args.scale, iteration * 0.08)
            dpg.set_value("series", [x.tolist(), y.tolist()])
        else:
            # Dear PyGui has no model reset equivalent; scrolling an eagerly
            # authored table is the closest safe sustained viewport workload.
            dpg.set_y_scroll("primary", float((iteration * 80) % max(1, args.scale * 20)))
        submit.append((time.perf_counter() - started) * 1000.0)
        dpg.render_dearpygui_frame()
        frames.append((time.perf_counter() - frame_started) * 1000.0)
        _pace(frame_started)
    report = _base(args, "Dear PyGui", _version("dearpygui"))
    report.update({
        "data_prepare_ms": data_prepare_ms,
        "build_ms": build_ms,
        "run_wall_ms": (time.perf_counter() - run_started) * 1000.0,
        "rss_before_run_bytes": rss_before,
        "rss_after_run_bytes": _rss_bytes(),
        "operations_completed": args.operations,
        "submit_ms": _timings(submit),
        "active_frame_ms": _timings(frames),
    })
    dpg.destroy_context()
    return report


RUNNERS: dict[str, Callable[[argparse.Namespace], dict[str, Any]]] = {
    "dragongui": run_dragongui,
    "dearpygui": run_dearpygui,
    "pyqt6": run_pyqt6,
}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--framework", required=True, choices=sorted(RUNNERS))
    parser.add_argument("--workload", required=True, choices=("restyle", "resize", "line_replace", "table_model"))
    parser.add_argument("--scale", required=True, type=int)
    parser.add_argument("--operations", type=int, default=30)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    args.scale = max(1, args.scale)
    args.operations = max(3, args.operations)
    report = RUNNERS[args.framework](args)
    payload = json.dumps(report, indent=2, sort_keys=True)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload + "\n", encoding="utf-8")
    print(payload)
    validation = report.get("validation")
    if args.framework == "dragongui" and validation and not validation.get("passed", False):
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
