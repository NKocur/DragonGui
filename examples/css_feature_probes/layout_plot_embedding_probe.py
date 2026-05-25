from __future__ import annotations

import math
import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

from probe_helpers import probe_app, probe_grid, probe_header

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual visual probe requirement
    raise SystemExit("layout_plot_embedding_probe.py requires NumPy") from exc


class PlotFrame:
    columns = ("time", "temperature", "pressure", "x", "y", "z", "score", "segment")
    dtypes = ("float32", "float32", "float32", "float32", "float32", "float32", "float32", "str")

    def __init__(self, rows: int = 900) -> None:
        self.shape = (rows, len(self.columns))
        i = np.arange(rows, dtype=np.float32)
        t = np.linspace(0.0, 90.0, rows, dtype=np.float32)
        phase = t / np.float32(90.0)
        theta = i * np.float32(math.pi * (3.0 - math.sqrt(5.0)))
        radius = np.sqrt((i + np.float32(1.0)) / np.float32(rows)) * np.float32(3.2)
        self.time = t
        self.temperature = (
            np.float32(68.0)
            + np.sin(phase * np.float32(math.tau * 2.1)) * np.float32(4.8)
            + np.sin(t * np.float32(0.9)) * np.float32(0.32)
        ).astype(np.float32)
        self.pressure = (
            np.float32(31.0)
            + np.cos(phase * np.float32(math.tau * 3.4)) * np.float32(2.1)
            + np.sin(t * np.float32(0.54)) * np.float32(0.44)
        ).astype(np.float32)
        self.x = np.cos(theta) * radius
        self.y = np.sin(theta) * radius
        self.z = (np.sin(theta * np.float32(0.19)) * np.float32(1.25) + phase * np.float32(1.1)).astype(np.float32)
        self.score = ((self.temperature - np.float32(62.0)) * np.float32(0.08) + phase).astype(np.float32)
        self.segment = np.where(phase > 0.68, "outer", np.where(phase > 0.34, "middle", "inner")).tolist()

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


class MetricsFrame:
    columns = ("stage", "owner", "latency_ms", "quality", "cost", "jobs")
    dtypes = ("str", "str", "float32", "float32", "float32", "float32")

    def __init__(self, rows: int = 420) -> None:
        self.shape = (rows, len(self.columns))
        rng = np.random.default_rng(24)
        stages = np.array(["Ingest", "Transform", "Validate", "Export", "Archive"], dtype=object)
        owners = np.array(["North", "South", "East", "West"], dtype=object)
        stage_index = rng.integers(0, len(stages), rows)
        self.stage = stages[stage_index].tolist()
        self.owner = owners[rng.integers(0, len(owners), rows)].tolist()
        base_latency = np.array([22, 48, 36, 30, 18], dtype=np.float32)
        self.latency_ms = (
            base_latency[stage_index] + rng.normal(0.0, 7.0, rows)
        ).clip(2.0, None).astype(np.float32)
        self.quality = rng.beta(5.0, 1.8, rows).astype(np.float32)
        self.cost = (self.latency_ms * rng.uniform(0.32, 0.74, rows)).astype(np.float32)
        self.jobs = rng.poisson(lam=np.maximum(self.latency_ms / 7.5, 1.0), size=rows).astype(np.float32)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


def heat_matrix(rows: int = 28, cols: int = 36) -> np.ndarray:
    y = np.linspace(-2.4, 2.4, rows, dtype=np.float32)
    x = np.linspace(-3.0, 3.0, cols, dtype=np.float32)
    xx, yy = np.meshgrid(x, y)
    ridge = np.sin(xx * 2.5) * np.cos(yy * 1.7)
    hot = np.exp(-((xx - 0.9) ** 2 + (yy + 0.55) ** 2) * 2.0)
    cool = np.exp(-((xx + 1.4) ** 2 + (yy - 0.7) ** 2) * 2.9)
    return (ridge * 0.52 + hot * 1.15 - cool * 0.82).astype(np.float32)


plot_frame = PlotFrame()
metrics_frame = MetricsFrame()
matrix = heat_matrix()
phase = {"value": 0.0}

app, win = probe_app("Layout Plot Embedding Probe", width=1120, height=780)
app.stylesheet(
    """
    Window {
        background: #0f141c;
        color: rgba(245, 248, 255, 0.94);
        padding: 16px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        overflow-y: auto;
        overflow-x: hidden;
        padding-right: 24px;
        padding-bottom: 34px;
        gap: 12px;
    }

    VLayout.root::scrollbar-track,
    DataFrameTable::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb,
    DataFrameTable::scrollbar-thumb {
        width: 6px;
        background: rgba(105, 183, 255, 0.72);
        border-radius: 999px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(245, 248, 255, 0.70);
        line-height: 1.14;
    }

    Label.status {
        width: 100%;
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 8px;
        color: rgba(229, 255, 244, 0.96);
        font-weight: 800;
        padding: 8px 10px;
    }

    FlowLayout.controls {
        width: 100%;
        gap: 8px;
        row-gap: 8px;
    }

    GridLayout.grid {
        width: 100%;
        gap: 12px;
        row-gap: 12px;
    }

    Panel.case {
        width: 100%;
        min-width: 0;
        min-height: 0;
        height: 318px;
        background: rgba(22, 30, 41, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 10px;
        padding: 12px;
        gap: 9px;
        overflow: hidden;
    }

    Panel.wide {
        height: 342px;
    }

    Panel.compact {
        height: 248px;
    }

    Panel.tiny {
        height: 190px;
        padding: 10px;
        gap: 7px;
    }

    HLayout.split {
        width: 100%;
        flex: 1;
        min-width: 0;
        min-height: 0;
        gap: 10px;
    }

    VLayout.plot-column,
    VLayout.table-column {
        flex: 1;
        flex-basis: 0;
        flex-shrink: 1;
        min-width: 0;
        min-height: 0;
        gap: 7px;
    }

    VLayout.table-column {
        max-width: 420px;
    }

    LinePlot,
    Histogram,
    BarChart,
    Heatmap,
    Scatter3D {
        width: 100%;
        flex: 1;
        min-height: 0;
        background: rgba(5, 9, 14, 0.72);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 8px;
        padding: 8px;
    }

    LinePlot.compact-plot,
    Histogram.compact-plot,
    BarChart.compact-plot,
    Heatmap.compact-plot,
    Scatter3D.compact-plot {
        padding: 7px;
    }

    Scatter3D.scatter-plot-2d {
        scatter-grid-visible: true;
        scatter-point-style: gaussian;
        scatter-point-size: 3px;
    }

    Scatter3D.volume {
        scatter-grid-visible: true;
        scatter-grid-planes: all;
        scatter-orientation-axes: true;
        scatter-point-style: gaussian;
        scatter-point-size: 3px;
    }

    Heatmap.compact-plot {
        padding: 6px;
    }

    DataFrameTable.embedded-table {
        width: 100%;
        flex: 1;
        min-height: 0;
        background: rgba(5, 9, 14, 0.72);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 8px;
        color: rgba(230, 238, 248, 0.90);
        font-size: 12px;
        table-row-height: 25px;
        table-header-height: 32px;
        table-column-width: 116px;
        table-index-width: 48px;
    }

    DataFrameTable.embedded-table::header {
        background: rgba(105, 183, 255, 0.17);
        color: white;
        font-weight: 850;
    }

    Button {
        width: auto;
        min-width: 74px;
        height: 32px;
        padding-left: 10px;
        padding-right: 10px;
        font-weight: 760;
    }

    Button.primary {
        background: rgba(116, 221, 176, 0.18);
        border-color: rgba(116, 221, 176, 0.44);
        color: white;
    }
    """
)

status: dg.Label | None = None
line_plot: dg.LinePlot | None = None
scatter2d: dg.ScatterPlot2D | None = None
scatter3d: dg.Scatter3D | None = None
histogram: dg.Histogram | None = None
bar_chart: dg.BarChart | None = None
heatmap: dg.Heatmap | None = None


def set_status(text: str) -> None:
    if status is not None:
        status.set_value(text)


def hover_heatmap(cell: dg.HeatmapCell | None) -> None:
    if cell is None:
        set_status("Hover plots and use Fit All to check overlays and controls in constrained cards.")
        return
    set_status(f"Heatmap cell r{cell.row} c{cell.col}: {cell.value:.4g}")


def hover_bar(bar: dg.BarChartBar | None) -> None:
    if bar is None:
        set_status("Hover plots and use Fit All to check overlays and controls in constrained cards.")
        return
    set_status(f"Bar {bar.category} / {bar.series}: {bar.value:.4g}")


def fit_all() -> None:
    for plot in (line_plot, scatter2d, scatter3d, histogram, bar_chart):
        if plot is not None:
            plot.fit()
    set_status("Fit All sent to line, scatter, histogram, and bar chart widgets.")


def refresh_data() -> None:
    phase["value"] += 0.38
    rows = plot_frame.shape[0]
    shift = int((phase["value"] * 67) % rows)
    shifted = PlotFrame(rows)
    shifted.temperature = np.roll(shifted.temperature, shift)
    shifted.pressure = np.roll(shifted.pressure, shift)
    if line_plot is not None:
        line_plot.set_data(
            shifted,
            x="time",
            y=("temperature", "pressure"),
            labels=("Temp", "Pressure"),
            colors=("#69b7ff", "#76e0b1"),
            fit=True,
        )
    if scatter2d is not None:
        scatter2d.set_points(shifted, x="x", y="y", scalars="score", fit=True)
    if scatter3d is not None:
        scatter3d.set_points(shifted, x="x", y="y", z="z", scalars="score", fit=True)
    if heatmap is not None:
        heatmap.set_data(heat_matrix(28, 36) + np.float32(math.sin(phase["value"]) * 0.08))
    set_status(f"Refreshed embedded plots at phase {phase['value']:.2f}.")


def view_3d_iso() -> None:
    if scatter3d is not None:
        scatter3d.view_isometric()
    set_status("3D view snapped to isometric inside the compact card.")


with win:
    with dg.VLayout(class_="root"):
        probe_header(
            "Plot embedding stress",
            "Plots live in fixed cards, split rows, compact grid cells, and narrow cards. They should start with valid bounds, keep controls usable, and draw hover overlays above surrounding content.",
        )
        status = dg.Label(
            "Ready: hover plots and use Fit All to check constrained plot behavior.",
            class_="status",
        )
        with dg.FlowLayout(class_="controls"):
            dg.Button("Fit All", class_="primary", on_click=fit_all)
            dg.Button("Refresh Data", on_click=refresh_data)
            dg.Button("3D Iso", on_click=view_3d_iso)

        with dg.Panel("Wide split: line plot beside virtualized table", class_="case wide"):
            with dg.HLayout(class_="split"):
                with dg.VLayout(class_="plot-column"):
                    dg.Label("Line chart should use the remaining split width without pushing the table out.", class_="caption")
                    line_plot = dg.LinePlot(
                        plot_frame,
                        x="time",
                        y=("temperature", "pressure"),
                        labels=("Temp", "Pressure"),
                        colors=("#69b7ff", "#76e0b1"),
                        line_styles=("solid", "dashed"),
                        x_label="time",
                        y_label="reading",
                        show_toolbar=True,
                        show_legend=True,
                        interaction="pan",
                        tick_count=5,
                    )
                with dg.VLayout(class_="table-column"):
                    dg.Label("Table scrollbars should stay local to this split pane.", class_="caption")
                    dg.DataFrameTable(
                        metrics_frame,
                        page_size=42,
                        sample_rows=42,
                        sortable=True,
                        resizable_columns=True,
                        on_select=lambda selection: set_status(
                            f"Table row {selection.row_index}: {selection.column} = {selection.value}"
                        ),
                        on_sort=lambda sort: set_status(
                            f"Sorted {'index' if sort.is_index else sort.column} {sort.direction}"
                        ),
                        class_="embedded-table",
                    )

        with probe_grid(min_column_width=330, gap=12, row_gap=12):
            with dg.Panel("Compact 2D scatter", class_="case"):
                dg.Label("2D scatter should not collapse to a dot in a card.", class_="caption")
                scatter2d = dg.ScatterPlot2D(
                    plot_frame,
                    x="x",
                    y="y",
                    scalars="score",
                    colormap="turbo",
                    scalar_bar=True,
                    scalar_bar_title="score",
                    point_size=3.0,
                    auto_quality=True,
                    quality_target_fps=30.0,
                    hover=["segment", "score"],
                    class_="compact-plot",
                )

            with dg.Panel("Compact 3D scatter", class_="case"):
                dg.Label("3D scatter should keep orbit controls, scalar bar, and orientation axes inside the panel.", class_="caption")
                scatter3d = dg.Scatter3D(
                    plot_frame,
                    x="x",
                    y="y",
                    z="z",
                    scalars="score",
                    colormap="turbo",
                    scalar_bar=True,
                    scalar_bar_title="score",
                    grid=True,
                    major_planes=True,
                    orientation_axes=True,
                    point_size=3.0,
                    auto_quality=True,
                    quality_target_fps=30.0,
                    hover=["segment", "score"],
                    class_="volume compact-plot",
                )

            with dg.Panel("Heatmap in a card", class_="case compact"):
                dg.Label("Cell labels and hover readout should not cover unrelated text.", class_="caption")
                heatmap = dg.Heatmap(
                    matrix,
                    colormap="turbo",
                    show_labels=False,
                    scalar_bar=True,
                    title="28 x 36 embedded grid",
                    on_hover=hover_heatmap,
                    class_="compact-plot",
                )

            with dg.Panel("Histogram in a card", class_="case compact"):
                dg.Label("Toolbar and axes should remain usable in a short panel.", class_="caption")
                histogram = dg.Histogram(
                    metrics_frame,
                    value="latency_ms",
                    bins=28,
                    range=(0.0, 92.0),
                    x_label="latency",
                    y_label="jobs",
                    color="#69b7ff",
                    show_toolbar=True,
                    interaction="pan",
                    class_="compact-plot",
                )

            with dg.Panel("Bar chart in a card", class_="case compact"):
                dg.Label("Horizontal labels should reserve enough room without shifting on scroll.", class_="caption")
                bar_chart = dg.BarChart(
                    metrics_frame,
                    category="stage",
                    value=["latency_ms", "cost"],
                    aggregate="mean",
                    orientation="horizontal",
                    x_label="mean",
                    y_label="stage",
                    colors=["#ffbf69", "#ed7d9a"],
                    show_toolbar=True,
                    on_hover=hover_bar,
                    class_="compact-plot",
                )

            with dg.Panel("Tiny mixed plot", class_="case tiny"):
                dg.Label("Very short plots should still show data instead of an empty frame.", class_="caption")
                dg.LinePlot(
                    plot_frame,
                    x="time",
                    y="temperature",
                    label="Temp",
                    color="#76e0b1",
                    x_label="t",
                    y_label="temp",
                    show_toolbar=False,
                    show_legend=False,
                    tick_count=3,
                    class_="compact-plot",
                )

        dg.Label(
            "PASS TARGET: plot data is visible on first frame, plot controls stay inside cards, fit buttons work after embedding, and hover readouts draw above nearby text without deleting it.",
            class_="caption",
        )


if __name__ == "__main__":
    print(app.run(win))
