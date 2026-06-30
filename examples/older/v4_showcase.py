from __future__ import annotations

import math
import os
import struct
import sys
import tempfile
import time
import zlib
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


PAGE = os.environ.get("DRAGONGUI_V4_SHOWCASE_PAGE", "dashboard")
CHART_STYLE = {"height": 260, "min_height": 220, "width": "100%", "min_width": 0}
TALL_CHART_STYLE = {"height": 330, "min_height": 260, "width": "100%", "min_width": 0}
TABLE_STYLE = {"height": 300, "min_height": 0, "width": "100%", "min_width": 0}
CARD_STYLE = {
    "min_width": 0,
    "min_height": 0,
    "flex_grow": 0,
    "flex_shrink": 1,
}


class ShowcaseFrame:
    columns = (
        "sample",
        "throughput",
        "latency",
        "errors",
        "score",
        "x",
        "y",
        "region",
        "segment",
        "stage",
        "accounts",
        "revenue",
    )
    dtypes = (
        "int32",
        "float32",
        "float32",
        "float32",
        "float32",
        "float32",
        "float32",
        "object",
        "object",
        "object",
        "int32",
        "float32",
    )

    def __init__(self, rows: int = 720) -> None:
        self.shape = (rows, len(self.columns))
        regions = ("Americas", "EMEA", "APAC", "LATAM")
        segments = ("Enterprise", "Team", "Partner", "Free", "Trial")
        stages = ("Ingest", "Transform", "Validate", "Export")
        self.sample: list[int] = []
        self.throughput: list[float] = []
        self.latency: list[float] = []
        self.errors: list[float] = []
        self.score: list[float] = []
        self.x: list[float] = []
        self.y: list[float] = []
        self.region: list[str] = []
        self.segment: list[str] = []
        self.stage: list[str] = []
        self.accounts: list[int] = []
        self.revenue: list[float] = []
        for index in range(rows):
            t = index / max(1, rows - 1)
            wave = math.sin(t * math.tau * 4.0)
            pulse = math.cos(t * math.tau * 9.0)
            self.sample.append(index)
            self.throughput.append(420.0 + 80.0 * wave + 28.0 * pulse)
            self.latency.append(42.0 + 13.0 * math.sin(t * math.tau * 3.0 + 0.6) + 4.0 * pulse)
            self.errors.append(max(0.0, 2.5 + 2.2 * math.sin(t * math.tau * 5.0 - 0.4)))
            self.score.append(0.5 + 0.42 * math.sin(t * math.tau * 2.0 + 1.2))
            radius = 0.8 + 3.8 * t
            theta = t * math.tau * 7.0
            self.x.append(math.cos(theta) * radius)
            self.y.append(math.sin(theta) * radius + 0.4 * math.sin(theta * 0.35))
            self.region.append(regions[index % len(regions)])
            self.segment.append(segments[(index // 9) % len(segments)])
            self.stage.append(stages[(index // 17) % len(stages)])
            self.accounts.append(3 + (index % 11))
            self.revenue.append(1.8 + (index % 13) * 0.42 + 1.1 * max(0.0, wave))

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


FRAME = ShowcaseFrame()
HEATMAP = [
    [
        math.sin(row * 0.62) * 0.55
        + math.cos(col * 0.47) * 0.45
        + (row - col) * 0.025
        for col in range(12)
    ]
    for row in range(9)
]


def make_asset_image() -> str:
    width, height = 260, 154
    rows: list[bytes] = []
    for y in range(height):
        scanline = bytearray([0])
        for x in range(width):
            nx = x / max(1, width - 1)
            ny = y / max(1, height - 1)
            stripe = 0.5 + 0.5 * math.sin((nx * 5.0 + ny * 2.0) * math.tau)
            r = int(34 + 72 * nx + 42 * stripe)
            g = int(56 + 118 * (1.0 - ny) + 20 * stripe)
            b = int(72 + 62 * ny + 54 * (1.0 - stripe))
            scanline.extend((max(0, min(255, r)), max(0, min(255, g)), max(0, min(255, b)), 255))
        rows.append(bytes(scanline))

    def chunk(name: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(name + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + name + data + struct.pack(">I", crc)

    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(b"".join(rows), 9))
        + chunk(b"IEND", b"")
    )
    path = Path(tempfile.gettempdir()) / "dragongui_v4_showcase_asset.png"
    path.write_bytes(png)
    return str(path)


ASSET_IMAGE = make_asset_image()


app = dg.App(
    title="DragonGUI V4 Showcase",
    theme=dg.Theme.dark(accent="#3aa99f", radius=8, focus="#f0c35b"),
    loading_screen=dg.LoadingScreen(
        title="DragonGUI V4 Showcase",
        message="Building native widget surfaces",
        background="#111313",
        text="#f4f1e6",
        accent="#3aa99f",
        show_progress=True,
        min_duration_ms=200,
    ),
)

app.stylesheet(
    """
    Window {
        background: #111313;
        color: #f4f1e6;
        padding: 0;
        gap: 0;
        overflow-x: hidden;
        overflow-y: hidden;
        font-size: 14px;
    }

    MenuBar {
        background: #151716;
        border-bottom: 1px solid rgba(244, 241, 230, 0.12);
    }

    HLayout.shell {
        width: 100%;
        height: 0;
        min-width: 0;
        min-height: 0;
        flex: 1;
        gap: 0;
    }

    Sidebar.sidebar {
        width: 248px;
        height: 100%;
        padding: 14px;
        gap: 8px;
        background: #171917;
        border-right: 1px solid rgba(244, 241, 230, 0.12);
        flex-shrink: 0;
    }

    VLayout.main {
        width: 0;
        height: 100%;
        min-width: 0;
        min-height: 0;
        flex: 1;
        gap: 0;
    }

    Toolbar.topbar {
        width: 100%;
        height: 48px;
        padding: 8px 12px;
        background: #181a1f;
        border-bottom: 1px solid rgba(244, 241, 230, 0.12);
    }

    Pages.main-pages {
        width: 100%;
        height: 0;
        min-width: 0;
        min-height: 0;
        flex: 1;
    }

    Page {
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 0;
    }

    ScrollArea.page-scroll {
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 0;
        padding: 14px;
        padding-right: 26px;
        overflow-y: auto;
    }

    ScrollArea.page-scroll::scrollbar-track,
    DataFrameTable::scrollbar-track,
    CodeEditor::scrollbar-track,
    LogView::scrollbar-track {
        width: 8px;
        background: rgba(244, 241, 230, 0.08);
        border-radius: 999px;
    }

    ScrollArea.page-scroll::scrollbar-thumb,
    DataFrameTable::scrollbar-thumb,
    CodeEditor::scrollbar-thumb,
    LogView::scrollbar-thumb {
        width: 6px;
        background: rgba(58, 169, 159, 0.7);
        border-radius: 999px;
    }

    GridLayout.card-grid {
        width: 100%;
        min-width: 0;
        align-items: stretch;
        gap: 12px;
        row-gap: 12px;
    }

    Panel.card,
    Panel.metric {
        min-width: 0;
        min-height: 0;
        padding: 12px;
        gap: 10px;
        background: #1b1d20;
        border: 1px solid rgba(244, 241, 230, 0.13);
        border-radius: 8px;
        box-shadow: 0 10px 26px rgba(0, 0, 0, 0.22);
    }

    Panel.metric {
        min-height: 130px;
    }

    Label.brand {
        color: #f7f2dd;
        font-size: 18px;
        font-weight: 850;
    }

    Label.page-title {
        color: #f7f2dd;
        font-size: 20px;
        font-weight: 850;
        height: 28px;
    }

    Label.caption,
    Label.subtle {
        color: rgba(244, 241, 230, 0.68);
        line-height: 1.2;
    }

    Label.kpi {
        color: #f7f2dd;
        font-size: 28px;
        font-weight: 850;
        height: 34px;
    }

    Label.kpi-label {
        color: rgba(244, 241, 230, 0.66);
        font-size: 12px;
        font-weight: 700;
        height: 18px;
    }

    Label.status-chip {
        background: rgba(58, 169, 159, 0.12);
        border: 1px solid rgba(58, 169, 159, 0.35);
        border-radius: 8px;
        color: #ddfff9;
        padding: 7px 9px;
        width: 100%;
    }

    Button.primary,
    SmallButton.primary {
        background: #2d827a;
        border-color: #3aa99f;
        color: #f6fffc;
    }

    Button.danger,
    SmallButton.danger {
        border-color: #ad4058;
        color: #ffccd6;
    }

    DataFrameTable,
    CodeEditor,
    LogView,
    HtmlReport {
        width: 100%;
        min-width: 0;
    }

    CodeEditor::field,
    LogView::line {
        font-family: Consolas;
        font-size: 12px;
    }

    Heatmap,
    BarChart,
    Histogram,
    PieChart {
        width: 100%;
        min-width: 0;
        background: #15181b;
        border: 1px solid rgba(244, 241, 230, 0.12);
        border-radius: 8px;
    }

    Heatmap::label,
    BarChart::label,
    BarChart::value-label,
    PieChart::label {
        color: #f4f1e6;
        font-size: 12px;
        font-weight: 650;
    }

    TreeView.navigator {
        width: 100%;
        min-height: 250px;
    }

    DropTarget.drop-zone {
        background: rgba(58, 169, 159, 0.08);
        border: 1px dashed rgba(58, 169, 159, 0.5);
        border-radius: 8px;
    }
    """
)


status: dg.TextInput | None = None
activity_log: dg.LogView | None = None
palette: dg.CommandPalette | None = None
confirm_modal: dg.Modal | None = None
about_modal: dg.Modal | None = None
pages: dg.Pages | None = None
data_table: dg.DataFrameTable | None = None


def mark(message: str, *, level: str = "info") -> None:
    stamp = time.strftime("%H:%M:%S")
    if status is not None:
        status.set_value(message)
    if activity_log is not None:
        activity_log.append_line(f"{stamp}  {level.upper()}  {message}")


def show_toast(message: str, *, level: str = "info") -> None:
    mark(message, level=level)
    try:
        dg.toast(message, level=level, duration=1800, app=app)
    except Exception:
        pass


def set_page(value: str) -> None:
    mark(f"Page: {value}")


def open_page(value: str) -> None:
    if pages is not None:
        pages.set_value(value, notify=True)


def open_palette() -> None:
    if palette is not None:
        palette.show()


def open_confirm() -> None:
    if confirm_modal is not None:
        confirm_modal.show()


def open_about() -> None:
    if about_modal is not None:
        about_modal.show()


def on_property(change: dg.PropertyChange) -> None:
    mark(f"{change.key}: {change.old_value!r} -> {change.value!r}")


def on_drop(payload: dg.DragDropPayload) -> None:
    mark(f"Dropped {payload.kind}: {payload.payload}", level="success")


def on_heatmap(cell: dg.HeatmapCell | None) -> None:
    if cell is not None:
        mark(f"Heatmap {cell.y_label or cell.row}/{cell.x_label or cell.col}: {cell.value:.3f}")


def on_bar(bar: dg.BarChartBar | None) -> None:
    if bar is not None:
        mark(f"Bar {bar.category}/{bar.series}: {bar.value:.3g}")


def on_pick(point: dg.ScatterPick) -> None:
    mark(f"Scatter point {point.index}")


def page_shell(title: str) -> dg.ScrollArea:
    area = dg.ScrollArea(axis="y", class_="page-scroll")
    with area:
        dg.Label(title, class_="page-title")
    return area


def metric_card(title: str, value: str, meta: str, level: str = "info") -> None:
    with dg.Panel(title, class_="metric", style=CARD_STYLE):
        dg.Label(value, class_="kpi")
        dg.Label(meta, class_="kpi-label")
        with dg.FlowLayout(gap=6, row_gap=6):
            dg.Tag(level, level=level)
            dg.LED(level, states={level: level})


win = dg.Window("DragonGUI V4 Showcase", width=1440, height=900, key="v4-showcase")

with win:
    with dg.MenuBar(height=34):
        with dg.Menu("File"):
            dg.MenuItem("Command palette", on_click=open_palette)
            dg.MenuItem("Confirm action", on_click=open_confirm)
            dg.MenuItem("About", on_click=open_about)
        with dg.Menu("View"):
            dg.MenuItem("Dashboard", on_click=lambda: open_page("dashboard"))
            dg.MenuItem("Workbench", on_click=lambda: open_page("workbench"))
            dg.MenuItem("Data", on_click=lambda: open_page("data"))
            dg.MenuItem("Media", on_click=lambda: open_page("media"))
        with dg.Menu("Runtime"):
            dg.MenuItem("Toast", on_click=lambda: show_toast("Runtime check queued"))
            dg.MenuItem("Append log", on_click=lambda: mark("Manual log event"))

    with dg.HLayout(class_="shell"):
        with dg.Sidebar(class_="sidebar"):
            dg.Label("DragonGUI V4", class_="brand")
            dg.Label("Operations console", class_="subtle")
            with dg.FlowLayout(gap=6, row_gap=5):
                dg.Badge("native", level="success")
                dg.Tag("v4", level="info")
                dg.Tag("gpu", level="warning")
            dg.Separator()
            dg.NavItem("Dashboard", page="dashboard", badge="live")
            dg.NavItem("Workbench", page="workbench")
            dg.NavItem("Data", page="data")
            dg.NavItem("Controls", page="controls")
            dg.NavItem("Media", page="media")
            dg.NavItem("Diagnostics", page="diagnostics")
            dg.Spacer()
            dg.Separator()
            dg.ProgressBar(0.73, show_value=True, style={"height": 12, "width": "100%"})
            dg.Label("Release readiness", class_="subtle")

        with dg.VLayout(class_="main"):
            with dg.Toolbar(class_="topbar", compact=False):
                dg.IconButton("search", tooltip="Command palette", on_click=open_palette)
                dg.IconButton("play", tooltip="Run acquisition", on_click=lambda: mark("Run started"))
                dg.IconButton("pause", tooltip="Pause acquisition", on_click=lambda: mark("Run paused"))
                dg.IconButton("stop", tooltip="Stop acquisition", on_click=lambda: mark("Run stopped"))
                dg.ToolbarSeparator()
                dg.SmallButton("Deploy", class_="primary", on_click=lambda: show_toast("Deploy queued"))
                dg.SmallButton("Reset", on_click=open_confirm)
                dg.ToolbarSeparator()
                dg.SearchBox(
                    "",
                    placeholder="Filter events",
                    on_change=lambda value: mark(f"Filter: {value or 'none'}"),
                    style={"width": 220},
                )
                dg.Spacer()
                dg.LoadingSpinner(size=22, label="Syncing", speed=1.15)

            with dg.Pages(value=PAGE, on_change=set_page, key="main-pages", class_="main-pages") as pages:
                with dg.Page("dashboard", title="Dashboard"):
                    with page_shell("Dashboard"):
                        with dg.GridLayout(
                            columns=4,
                            min_column_width=220,
                            masonry=True,
                            class_="card-grid",
                        ):
                            metric_card("Throughput", "486/s", "rolling 15 minute mean", "success")
                            metric_card("Latency", "43.8 ms", "p95 service time", "warning")
                            metric_card("Errors", "0.42%", "last 8,000 requests", "danger")
                            metric_card("Coverage", "97.6%", "validated samples", "info")

                            with dg.Panel("Acquisition trend", class_="card", style=CARD_STYLE):
                                dg.LinePlot(
                                    FRAME,
                                    x="sample",
                                    y=("throughput", "latency"),
                                    labels=("Throughput", "Latency"),
                                    colors=("#3aa99f", "#d99b2b"),
                                    line_styles=("solid", "dashed"),
                                    show_toolbar=True,
                                    show_legend=True,
                                    legend_position="top-right",
                                    style=TALL_CHART_STYLE,
                                )

                            with dg.Panel("Segment mix", class_="card", style=CARD_STYLE):
                                dg.PieChart(
                                    FRAME,
                                    category="segment",
                                    value="accounts",
                                    aggregate="sum",
                                    top_n=4,
                                    donut=True,
                                    inner_radius=0.58,
                                    label_mode="legend",
                                    center_value="4,984",
                                    center_label="accounts",
                                    show_toolbar=True,
                                    style=TALL_CHART_STYLE,
                                )

                            with dg.Panel("Routing", class_="card", style=CARD_STYLE):
                                dg.Breadcrumbs(
                                    [
                                        ("Workspace", "workspace"),
                                        ("Telemetry", "telemetry"),
                                        ("Release", "release"),
                                        ("Run 042", "run-042"),
                                    ],
                                    max_items=4,
                                    on_select=lambda item: mark(f"Breadcrumb: {item.label}"),
                                )
                                with dg.Tabs(value="summary", on_change=lambda value: mark(f"Tab: {value}")):
                                    with dg.Tab("Summary", value="summary", badge=4):
                                        dg.Label("Pipeline health is nominal.", class_="status-chip")
                                        dg.ProgressBar(0.82, show_value=True)
                                    with dg.Tab("Queue", value="queue", badge=9):
                                        dg.Label("Queue age: 18 s", class_="status-chip")
                                        dg.ProgressBar(0.38, show_value=True)
                                    with dg.Tab("Risk", value="risk"):
                                        dg.Label("Two alert rules are muted.", class_="status-chip")
                                        dg.ProgressBar(0.26, show_value=True)

                            with dg.Panel("Action states", class_="card", style=CARD_STYLE):
                                with dg.FlowLayout(gap=8, row_gap=8):
                                    dg.Badge("ready", level="success")
                                    dg.Badge("queued", level="warning")
                                    dg.Badge("blocked", level="danger")
                                    dg.Tag("batch-17", level="info")
                                    dg.LED(True, tooltip="Online")
                                    dg.LED("warm", states={"warm": "warning"})
                                dg.LoadingSpinner(size=30, label="Streaming", speed=1.4)
                                dg.ProgressBar(0.61, show_value=True)

                with dg.Page("workbench", title="Workbench"):
                    with page_shell("Workbench"):
                        with dg.Splitter(
                            orientation="horizontal",
                            sizes=(260, "1fr"),
                            min_sizes=(220, 520),
                            gutter_size=10,
                            style={"height": 650, "width": "100%", "min_width": 0},
                        ):
                            with dg.Pane(min_size=220, style={"min_width": 0, "gap": 8}):
                                dg.SearchBox(
                                    "",
                                    placeholder="Search assets",
                                    on_change=lambda value: mark(f"Asset search: {value}"),
                                )
                                with dg.TreeView(
                                    class_="navigator",
                                    on_select=lambda node_id: mark(f"Tree: {node_id}"),
                                ):
                                    with dg.TreeNode("Plant", node_id="plant", expanded=True):
                                        dg.TreeNode("North gate", node_id="north", leaf=True)
                                        dg.TreeNode("Compressor", node_id="compressor", leaf=True)
                                        with dg.TreeNode("Line A", node_id="line-a", expanded=True):
                                            dg.TreeNode("Station 01", node_id="station-01", leaf=True)
                                            dg.TreeNode("Station 02", node_id="station-02", leaf=True)
                                    with dg.TreeNode("Reports", node_id="reports"):
                                        dg.TreeNode("Current run", node_id="report-current", leaf=True)
                                with dg.FlowLayout(gap=8, row_gap=8):
                                    with dg.DragSource({"metric": "latency", "window": "p95"}, drag_kind="metric"):
                                        dg.Badge("latency p95", level="warning")
                                    with dg.DragSource({"metric": "errors", "window": "1h"}, drag_kind="metric"):
                                        dg.Badge("error rate", level="danger")
                                dg.DropZone(
                                    "Drop metric",
                                    accept="metric",
                                    on_drop=on_drop,
                                    style={"height": 86, "width": "100%"},
                                )

                            with dg.Pane(min_size=520, style={"min_width": 0, "gap": 12}):
                                with dg.GridLayout(
                                    columns=2,
                                    min_column_width=330,
                                    masonry=True,
                                    class_="card-grid",
                                ):
                                    with dg.Panel("Property inspector", class_="card", style=CARD_STYLE):
                                        dg.PropertyGrid(
                                            {
                                                "Name": "Compressor A",
                                                "Enabled": True,
                                                "Mode": "Auto",
                                                "Gain": 0.42,
                                                "Band": (18, 84),
                                                "Tint": "#3aa99f",
                                            },
                                            schema={
                                                "Mode": {
                                                    "type": "select",
                                                    "options": ["Auto", "Manual", "Disabled"],
                                                },
                                                "Gain": {"type": "float", "min": 0, "max": 1, "step": 0.01},
                                                "Band": {"type": "range", "min": 0, "max": 100, "step": 2},
                                                "Tint": {"type": "color"},
                                            },
                                            sections={
                                                "Device": ["Name", "Enabled", "Mode"],
                                                "Tuning": ["Gain", "Band", "Tint"],
                                            },
                                            on_change=on_property,
                                            label_width=92,
                                        )

                                    with dg.Panel("Manual properties", class_="card", style=CARD_STYLE):
                                        with dg.PropertyGrid(label_width=96):
                                            dg.Property(
                                                "Threshold",
                                                dg.RangeSlider(
                                                    (0.22, 0.82),
                                                    min=0,
                                                    max=1,
                                                    step=0.02,
                                                    on_change=lambda value: mark(
                                                        f"Threshold {value[0]:.2f}-{value[1]:.2f}"
                                                    ),
                                                ),
                                            )
                                            dg.Property(
                                                "Vector",
                                                dg.DragVector(
                                                    (0.0, 1.5, -2.0),
                                                    labels=("X", "Y", "Z"),
                                                    min=-5,
                                                    max=5,
                                                    step=0.25,
                                                    component_width=78,
                                                    on_change=lambda value: mark(
                                                        f"Vector {tuple(round(v, 2) for v in value)}"
                                                    ),
                                                ),
                                            )
                                            with dg.Property("Notes"):
                                                dg.TextArea(
                                                    "Active route\nBatch 17\nReview after export",
                                                    rows=3,
                                                )

                                    with dg.Panel("Command surface", class_="card", style=CARD_STYLE):
                                        with dg.Toolbar():
                                            dg.IconButton("play", tooltip="Run", on_click=lambda: mark("Toolbar run"))
                                            dg.IconButton("pause", tooltip="Pause", on_click=lambda: mark("Toolbar pause"))
                                            dg.IconButton("save", tooltip="Save", on_click=lambda: mark("Toolbar save"))
                                            dg.ToolbarSeparator()
                                            dg.ImageButton(
                                                ASSET_IMAGE,
                                                size=34,
                                                fit="cover",
                                                tooltip="Asset action",
                                                on_click=lambda: mark("Image action"),
                                            )
                                        with dg.FlowLayout(gap=8, row_gap=8):
                                            dg.ArrowButton("left", tooltip="Previous", on_click=lambda: mark("Left"))
                                            dg.ArrowButton("right", tooltip="Next", on_click=lambda: mark("Right"))
                                            dg.ArrowButton("up", tooltip="Promote", on_click=lambda: mark("Up"))
                                            dg.ArrowButton("down", tooltip="Demote", on_click=lambda: mark("Down"))
                                            dg.SmallButton("Palette", class_="primary", on_click=open_palette)
                                            dg.SmallButton("Confirm", on_click=open_confirm)

                                    with dg.Panel("Selections", class_="card", style=CARD_STYLE):
                                        dg.SelectableList(
                                            [
                                                ("North line", "north"),
                                                ("Compressor", "compressor"),
                                                ("Validation", "validation"),
                                                {"label": "Archive", "value": "archive", "disabled": True},
                                            ],
                                            value="compressor",
                                            on_change=lambda value: mark(f"Selection: {value}"),
                                            max_height=160,
                                        )
                                        dg.RadioGroup(
                                            ["Low", "Medium", "High"],
                                            value="Medium",
                                            orientation="horizontal",
                                            gap=12,
                                            on_change=lambda value: mark(f"Priority: {value}"),
                                        )

                with dg.Page("data", title="Data"):
                    with page_shell("Data"):
                        with dg.GridLayout(
                            columns=2,
                            min_column_width=470,
                            masonry=True,
                            class_="card-grid",
                        ):
                            with dg.Panel("Data table", class_="card", style=CARD_STYLE):
                                data_table = dg.DataFrameTable(
                                    FRAME,
                                    page_size=60,
                                    sample_rows=80,
                                    sortable=True,
                                    resizable_columns=True,
                                    on_select=lambda selection: mark(
                                        f"Cell {selection.row_index}:{selection.column}"
                                    ),
                                    on_sort=lambda sort: mark(
                                        f"Sort {sort.column} {sort.direction}"
                                    ),
                                    style=TABLE_STYLE,
                                )

                            with dg.Panel("Scatter plot", class_="card", style=CARD_STYLE):
                                dg.ScatterPlot2D(
                                    FRAME,
                                    x="x",
                                    y="y",
                                    scalars="score",
                                    colormap="turbo",
                                    point_size=3.2,
                                    scalar_bar=True,
                                    scalar_bar_title="score",
                                    hover=["sample", "segment"],
                                    on_pick=on_pick,
                                    style=TALL_CHART_STYLE,
                                )

                            with dg.Panel("Heatmap", class_="card", style=CARD_STYLE):
                                dg.Heatmap(
                                    HEATMAP,
                                    x_labels=[f"T{i}" for i in range(12)],
                                    y_labels=[f"L{i}" for i in range(9)],
                                    colormap="magma",
                                    title="Load matrix",
                                    on_hover=on_heatmap,
                                    style=CHART_STYLE,
                                )

                            with dg.Panel("Bar chart", class_="card", style=CARD_STYLE):
                                dg.BarChart(
                                    FRAME,
                                    category="stage",
                                    value=["revenue", "accounts"],
                                    aggregate="sum",
                                    x_label="stage",
                                    y_label="total",
                                    colors=["#3aa99f", "#d99b2b"],
                                    show_toolbar=True,
                                    on_hover=on_bar,
                                    style=CHART_STYLE,
                                )

                            with dg.Panel("Histogram", class_="card", style=CARD_STYLE):
                                dg.Histogram(
                                    FRAME,
                                    value="latency",
                                    bins=28,
                                    range=(20, 70),
                                    mode="percent",
                                    x_label="latency",
                                    y_label="share",
                                    color="#3aa99f",
                                    show_toolbar=True,
                                    style=CHART_STYLE,
                                )

                            with dg.Panel("Composition", class_="card", style=CARD_STYLE):
                                dg.PieChart(
                                    FRAME,
                                    category="region",
                                    value="revenue",
                                    aggregate="sum",
                                    donut=True,
                                    inner_radius=0.58,
                                    label_mode="legend",
                                    value_mode="both",
                                    center_value="$3.1M",
                                    center_label="revenue",
                                    show_toolbar=True,
                                    style=CHART_STYLE,
                                )

                with dg.Page("controls", title="Controls"):
                    with page_shell("Controls"):
                        with dg.GridLayout(
                            columns=3,
                            min_column_width=320,
                            masonry=True,
                            class_="card-grid",
                        ):
                            with dg.Panel("Primitive inputs", class_="card", style=CARD_STYLE):
                                dg.TextInput(
                                    "Compressor A",
                                    placeholder="Asset name",
                                    on_change=lambda value: mark(f"Name: {value}"),
                                )
                                dg.TextArea(
                                    "Line A\nshift 2\nQA owner: Mira",
                                    rows=4,
                                    on_change=lambda value: mark(f"Notes: {len(value)} chars"),
                                )
                                dg.Dropdown(
                                    ("Auto", "Manual", "Disabled"),
                                    value="Auto",
                                    on_change=lambda value: mark(f"Mode: {value}"),
                                )
                                dg.NumberInput(
                                    42,
                                    min=0,
                                    max=100,
                                    step=1,
                                    on_change=lambda value: mark(f"Number: {value:g}"),
                                )

                            with dg.Panel("Toggles and ranges", class_="card", style=CARD_STYLE):
                                dg.Checkbox("Enable alerts", checked=True, on_change=lambda v: mark(f"Alerts: {v}"))
                                dg.ToggleSwitch("Live acquisition", checked=True, on_change=lambda v: mark(f"Live: {v}"))
                                dg.Slider(0.64, min=0, max=1, step=0.02, on_change=lambda v: mark(f"Slider: {v:.2f}"))
                                dg.RangeSlider(
                                    (18, 84),
                                    min=0,
                                    max=100,
                                    step=2,
                                    on_change=lambda v: mark(f"Range: {v[0]:.0f}-{v[1]:.0f}"),
                                )
                                dg.DragNumber(
                                    12.5,
                                    min=0,
                                    max=30,
                                    step=0.5,
                                    on_change=lambda value: mark(f"Delay: {value:.1f} ms"),
                                )

                            with dg.Panel("Date and time", class_="card", style=CARD_STYLE):
                                dg.DateInput("2026-06-19", on_change=lambda value: mark(f"Date: {value}"))
                                dg.TimeInput("09:30", on_change=lambda value: mark(f"Time: {value}"))
                                dg.DateTimeInput(
                                    "2026-06-19T09:30:00",
                                    on_change=lambda value: mark(f"DateTime: {value}"),
                                )
                                dg.RadioButton(
                                    "Manual override",
                                    checked=False,
                                    on_change=lambda value: mark(f"Radio: {value}"),
                                )
                                dg.RadioGroup(
                                    [("Fast", "fast"), ("Balanced", "balanced"), ("Quality", "quality")],
                                    value="balanced",
                                    orientation="horizontal",
                                    on_change=lambda value: mark(f"Quality: {value}"),
                                )

                            dg.ColorPicker(
                                (58, 169, 159),
                                alpha=False,
                                title="Color picker",
                                width=None,
                                on_change=lambda value: mark(f"Color: {value}"),
                                class_="card",
                                style=CARD_STYLE,
                            )

                            with dg.Panel("Disclosure", class_="card", style=CARD_STYLE):
                                with dg.Collapsible("Acquisition", expanded=True):
                                    dg.Label("Window: 15 min", class_="subtle")
                                    dg.ProgressBar(0.73, show_value=True)
                                with dg.Collapsible("Validation", expanded=False):
                                    dg.Label("Ruleset: release-4", class_="subtle")
                                    dg.ProgressBar(0.48, show_value=True)

                            with dg.Panel("Buttons", class_="card", style=CARD_STYLE):
                                with dg.FlowLayout(gap=8, row_gap=8):
                                    dg.Button("Primary", class_="primary", on_click=lambda: mark("Primary"))
                                    dg.Button("Danger", class_="danger", on_click=open_confirm)
                                    dg.SmallButton("Small", on_click=lambda: mark("Small"))
                                    dg.IconButton("save", tooltip="Save", on_click=lambda: mark("Save"))
                                target = dg.Button("Tooltip target", on_click=lambda: mark("Tooltip target"))
                                with dg.Tooltip(target=target):
                                    dg.Label("Owner: platform operations")
                                    dg.ProgressBar(0.68, show_value=True)

                with dg.Page("media", title="Media"):
                    with page_shell("Media"):
                        with dg.GridLayout(
                            columns=2,
                            min_column_width=460,
                            masonry=True,
                            class_="card-grid",
                        ):
                            with dg.Panel("Image asset", class_="card", style=CARD_STYLE):
                                dg.Image(
                                    ASSET_IMAGE,
                                    fit="cover",
                                    height=220,
                                    style={"width": "100%", "border_radius": 8},
                                )
                                with dg.FlowLayout(gap=8, row_gap=8):
                                    dg.ImageButton(
                                        ASSET_IMAGE,
                                        size=42,
                                        fit="cover",
                                        tooltip="Preview",
                                        on_click=lambda: mark("Preview clicked"),
                                    )
                                    dg.Badge("asset", level="info")
                                    dg.Tag("generated", level="neutral")

                            with dg.Panel("HTML report", class_="card", style=CARD_STYLE):
                                dg.HtmlReport.from_html(
                                    """
                                    <html>
                                      <body style="font-family: system-ui; background:#111313; color:#f4f1e6">
                                        <h1>Run 042</h1>
                                        <p>Revenue mix, latency and validation checkpoints.</p>
                                      </body>
                                    </html>
                                    """,
                                    height=260,
                                    style={"width": "100%"},
                                )

                            with dg.Panel("Code editor", class_="card", style=CARD_STYLE):
                                dg.CodeEditor(
                                    "def classify(sample):\n"
                                    "    if sample.score > 0.82:\n"
                                    "        return 'review'\n"
                                    "    if sample.errors > 4:\n"
                                    "        return 'watch'\n"
                                    "    return 'ok'\n",
                                    language="python",
                                    rows=10,
                                    on_change=lambda value: mark(f"Code lines: {len(value.splitlines())}"),
                                    style={"width": "100%"},
                                )

                            with dg.Panel("Activity log", class_="card", style=CARD_STYLE):
                                activity_log = dg.LogView(
                                    [
                                        "09:30:00  INFO  V4 showcase mounted",
                                        "09:30:03  INFO  Loaded frame: 720 rows",
                                        "09:30:08  WARN  Two muted validation rules",
                                    ],
                                    follow=True,
                                    max_lines=300,
                                    rows=12,
                                    style={"width": "100%"},
                                )
                                with dg.FlowLayout(gap=8, row_gap=8):
                                    dg.SmallButton("Append", on_click=lambda: mark("Manual append"))
                                    dg.SmallButton(
                                        "Clear",
                                        on_click=lambda: activity_log.clear() if activity_log is not None else None,
                                    )

                with dg.Page("diagnostics", title="Diagnostics"):
                    with page_shell("Diagnostics"):
                        with dg.GridLayout(
                            columns=2,
                            min_column_width=460,
                            masonry=True,
                            class_="card-grid",
                        ):
                            dg.ThreadMonitor(
                                key="v4-thread-monitor",
                                show_threads=True,
                                show_queue=True,
                                show_failures=True,
                                history_seconds=30,
                                refresh_hz=4.0,
                                max_threads=60,
                                max_dead_threads=12,
                                style={"height": 520, "min_height": 360, "width": "100%"},
                            )
                            with dg.Panel("Runtime tools", class_="card", style=CARD_STYLE):
                                dg.Label("Queue health", class_="status-chip")
                                with dg.FlowLayout(gap=8, row_gap=8):
                                    dg.Button("Schedule task", on_click=lambda: app.call_soon_threadsafe(lambda: mark("Task ran")))
                                    dg.Button(
                                        "Schedule failure",
                                        class_="danger",
                                        on_click=lambda: app.call_soon_threadsafe(
                                            lambda: (_ for _ in ()).throw(RuntimeError("showcase failure"))
                                        ),
                                    )
                                    dg.Button("Toast", on_click=lambda: show_toast("Diagnostic toast"))
                                dg.Separator()
                                dg.LogView(
                                    [
                                        "collector: ready",
                                        "refresh: 4 hz",
                                        "failure capture: enabled",
                                    ],
                                    rows=9,
                                    style={"width": "100%"},
                                )

    with dg.StatusBar(height=38):
        status = dg.TextInput("Ready", placeholder="status", style={"width": 420})
        dg.Separator(orientation="vertical")
        dg.Label("720 rows")
        dg.Label("6 pages")
        dg.Spacer()
        dg.LED(True)
        dg.Label("Renderer online")

    confirm_modal = dg.confirm(
        "Reset Showcase State",
        "Clear transient status and append a reset event?",
        open=False,
        on_confirm=lambda: mark("State reset", level="warning"),
        parent=win,
    )
    about_modal = dg.alert(
        "DragonGUI V4 Showcase",
        "Standalone review surface for V4 widgets and app composition.",
        open=False,
        parent=win,
    )

    if data_table is not None:
        with dg.ContextMenu(target=data_table, width=220, parent=win):
            dg.MenuItem("Copy cell", on_click=lambda: mark("Copy cell"))
            dg.MenuItem("Export CSV", on_click=lambda: mark("Export CSV"))
            dg.MenuItem("Open command palette", on_click=open_palette)

    palette = dg.CommandPalette(
        [
            dg.Command("dashboard", "Open Dashboard", lambda: open_page("dashboard"), "Primary overview"),
            dg.Command("workbench", "Open Workbench", lambda: open_page("workbench"), "Inspector workspace"),
            dg.Command("data", "Open Data", lambda: open_page("data"), "Tables and charts"),
            dg.Command("media", "Open Media", lambda: open_page("media"), "Reports and logs"),
            dg.Command("diagnostics", "Open Diagnostics", lambda: open_page("diagnostics"), "Runtime monitor"),
            dg.Command("toast", "Show Toast", lambda: show_toast("Command toast")),
            dg.Command("disabled", "Disabled Command", disabled=True),
        ],
        title="Command Palette",
        open=False,
        on_run=lambda command: mark(f"Command: {command.id}"),
        parent=win,
    )


if __name__ == "__main__":
    print(app.run(win))
