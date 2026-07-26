from __future__ import annotations

import math
import random
import sys
from pathlib import Path
from typing import Any

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


class ColumnFrame:
    def __init__(self, **columns: list[Any]) -> None:
        self.columns = tuple(columns)
        self.dtypes = tuple("float64" if all(isinstance(value, (int, float)) for value in values) else "str" for values in columns.values())
        self.shape = (len(next(iter(columns.values()), [])), len(columns))
        self._columns = columns

    def __getitem__(self, column: str) -> list[Any]:
        return self._columns[column]


minutes = list(range(180))
SERIES = ColumnFrame(
    minute=minutes,
    throughput=[
        72.0 + math.sin(index / 12.0) * 11.0 + math.sin(index / 4.7) * 3.4
        for index in minutes
    ],
    latency=[
        34.0 + math.sin(index / 17.0 + 0.8) * 8.0 + math.cos(index / 5.0) * 2.2
        for index in minutes
    ],
)

OWNERS = ColumnFrame(
    owner=["Ingest", "Models", "Quality", "Export", "Archive"],
    volume=[84.0, 72.0, 61.0, 48.0, 32.0],
    latency=[21.0, 38.0, 29.0, 46.0, 17.0],
)

HEATMAP = [
    [
        round(
            18
            + row * 5.5
            + math.sin(column / 2.2 + row * 0.7) * 8
            + math.cos(column / 4.0) * 3,
            2,
        )
        for column in range(12)
    ]
    for row in range(6)
]

INCIDENT_ROWS = [
    {
        "route": f"north.line-{(index % 4) + 1}.cell-{index + 2:02d}",
        "owner": ("ingest", "model", "quality", "export")[index % 4],
        "state": ("watch", "review", "healthy", "queued")[index % 4],
        "p95_ms": round(28.0 + (index * 7.3) % 54.0, 1),
        "volume": 1200 + index * 347,
        "updated": f"{index + 2:02d}m ago",
    }
    for index in range(18)
]


class DemoState:
    def __init__(self) -> None:
        self.pages: dg.Pages | None = None
        self.sidebar: dg.Sidebar | None = None
        self.status: dg.Label | None = None
        self.status_badge: dg.Badge | None = None
        self.modal: dg.Modal | None = None
        self.table: dg.DataFrameTable | None = None
        self.random = random.Random(20260724)


state = DemoState()


def set_status(message: str, level: str = "info") -> None:
    if state.status is not None:
        state.status.set_value(message)
    if state.status_badge is not None:
        state.status_badge.set_value(level)


def navigate(route: str) -> None:
    if state.pages is not None:
        state.pages.set_value(route)
    set_status(f"Opened {route}", "ready")


def show_modal() -> None:
    if state.modal is not None:
        state.modal.show()
    set_status("Review panel opened", "review")


def toggle_sidebar() -> None:
    if state.sidebar is not None:
        state.sidebar.toggle_collapsed()


def randomize_table() -> None:
    shuffled = list(INCIDENT_ROWS)
    state.random.shuffle(shuffled)
    if state.table is not None:
        state.table.set_frame(shuffled)
    set_status("Operational feed refreshed", "live")


def page_scroller(id: str | None = None) -> dg.ScrollArea:
    return dg.ScrollArea(axis="y", gap=14, class_="page-scroll", id=id)


def page_heading(eyebrow: str, title: str, description: str) -> None:
    with dg.VLayout(class_="page-heading"):
        dg.Label(eyebrow, class_="eyebrow", wrap=False)
        with dg.FlowLayout(gap=10, row_gap=6, style={"align_items": "center"}):
            dg.Label(title, class_="page-title", wrap=False)
            dg.Badge("LIVE", level="success")
        dg.Label(description, class_="page-description")


def metric_card(
    label: str,
    value: str,
    detail: str,
    *,
    level: str,
    progress: float,
) -> None:
    with dg.Panel(class_=f"metric-card metric-{level}"):
        with dg.FlowLayout(gap=8, row_gap=4, style={"align_items": "center"}):
            dg.LED(level == "success")
            dg.Label(label, class_="metric-label", wrap=False)
            dg.Spacer()
            dg.Tag(level, level=level)
        dg.Label(value, class_="metric-value", wrap=False)
        dg.Label(detail, class_="metric-detail")
        dg.ProgressBar(progress, show_value=False, class_="metric-progress")


def build_overview() -> None:
    page_heading(
        "NORTH PLANT / COMMAND LAYER",
        "Operational intelligence",
        "A live view of throughput, latency, service pressure, and review queues across the plant.",
    )

    with dg.Panel(class_="hero-card"):
        with dg.FlowLayout(gap=16, row_gap=12, style={"align_items": "center"}):
            with dg.VLayout(class_="hero-copy"):
                dg.Label("System posture", class_="eyebrow", wrap=False)
                dg.Label("All critical paths are inside operating limits", class_="hero-title")
                dg.Label(
                    "Three routes are being watched. No customer-facing pipeline is currently degraded.",
                    class_="page-description",
                )
            with dg.FlowLayout(gap=8, row_gap=8, class_="hero-actions"):
                dg.Button("Review", class_="primary-action", on_click=show_modal)
                dg.SmallButton("Refresh feed", on_click=randomize_table)
                dg.Tag("99.98% available", level="success")

    with dg.GridLayout(
        columns={"default": 4, 1100: 2, 700: 1},
        min_column_width=210,
        gap=12,
        class_="metric-grid",
    ):
        metric_card("Availability", "99.98%", "+0.12% over 24 hours", level="success", progress=0.94)
        metric_card("Median latency", "34 ms", "p95 remains below 71 ms", level="info", progress=0.71)
        metric_card("Review queue", "12", "three require owner approval", level="warning", progress=0.48)
        metric_card("Events / min", "8.4k", "stable across all collectors", level="neutral", progress=0.82)

    with dg.GridLayout(columns=2, min_column_width=420, gap=12, class_="overview-grid"):
        with dg.Panel("Throughput and latency", class_="surface-card chart-card"):
            with dg.FlowLayout(gap=8, row_gap=6, class_="panel-tools"):
                dg.Tag("180 minute window", level="neutral")
                dg.SmallButton("1H", on_click=lambda: set_status("Range: one hour"))
                dg.SmallButton("3H", on_click=lambda: set_status("Range: three hours"))
                dg.SmallButton("Fit", on_click=lambda: set_status("Chart fitted"))
            dg.LinePlot(
                SERIES,
                x="minute",
                y=["throughput", "latency"],
                labels=["throughput", "latency"],
                colors=["#74f0c7", "#79a9ff"],
                show_toolbar=False,
                show_legend=True,
                style={"height": 275},
            )

        with dg.Panel("Service pressure", class_="surface-card pressure-card"):
            dg.Label(
                "Capacity consumption by pipeline stage. Bars should remain readable when this card collapses to one column.",
                class_="muted",
            )
            for label, value, level in [
                ("Realtime ingest", 0.78, "info"),
                ("Inference fleet", 0.64, "success"),
                ("Quality review", 0.47, "warning"),
                ("Report export", 0.35, "neutral"),
            ]:
                with dg.VLayout(class_="pressure-row"):
                    with dg.HLayout(class_="pressure-heading"):
                        dg.Label(label, wrap=False)
                        dg.Spacer()
                        dg.Label(f"{value * 100:.0f}%", class_="mono", wrap=False)
                    dg.ProgressBar(value, show_value=False, class_=f"pressure-{level}")
            with dg.Panel(class_="signal-card"):
                with dg.FlowLayout(gap=8, row_gap=6, style={"align_items": "center"}):
                    dg.Badge("3", level="warning")
                    dg.Label("Routes need operator review", class_="signal-title")
                dg.Label("Longest wait: north.line-2.cell-07 · 18 minutes", class_="muted")

    with dg.GridLayout(columns=3, min_column_width=280, gap=12):
        with dg.Panel("Owner distribution", class_="surface-card compact-chart"):
            dg.PieChart(
                labels=["Ingest", "Models", "Quality", "Export"],
                values=[38, 31, 19, 12],
                donut=True,
                center_value="8.4k",
                center_label="events/min",
                show_legend=True,
                legend_position="bottom",
                style={"height": 245},
            )
        with dg.Panel("Latency field", class_="surface-card compact-chart"):
            dg.Heatmap(
                HEATMAP,
                x_labels=[f"{hour:02d}" for hour in range(0, 24, 2)],
                y_labels=[f"L{index}" for index in range(1, 7)],
                title="p95 by line / hour",
                colormap="viridis",
                style={"height": 245},
            )
        with dg.Panel("Volume by owner", class_="surface-card compact-chart"):
            dg.BarChart(
                OWNERS,
                category="owner",
                value="volume",
                aggregate="mean",
                show_toolbar=False,
                style={"height": 245},
            )

    with dg.Panel("Operational feed", class_="surface-card feed-card"):
        with dg.FlowLayout(gap=8, row_gap=8, class_="feed-actions"):
            dg.SearchBox("", placeholder="Filter route, owner, or state", style={"width": 280})
            dg.Dropdown(("All states", "Watch", "Review", "Healthy"), value="All states")
            dg.SmallButton("Refresh", on_click=randomize_table)
            dg.Spacer()
            dg.Tag("18 routes", level="neutral")
        state.table = dg.DataFrameTable(
            INCIDENT_ROWS,
            page_size=18,
            sample_rows=18,
            sortable=True,
            resizable_columns=True,
            style={"height": 300},
            on_select=lambda selection: set_status(
                f"Selected {selection.column}: {selection.value}",
                "selected",
            ),
        )


def build_analytics() -> None:
    page_heading(
        "SIGNAL LAB / MODEL HEALTH",
        "Analytics workspace",
        "Dense analytical surfaces exercise responsive grids, plot sizing, and bounded detail panels.",
    )
    with dg.GridLayout(columns=2, min_column_width=420, gap=12):
        with dg.Panel("Latency distribution", class_="surface-card"):
            dg.Histogram(SERIES["latency"], bins=32, mode="count", style={"height": 310})
        with dg.Panel("Throughput by owner", class_="surface-card"):
            dg.BarChart(
                OWNERS,
                category="owner",
                value=["volume", "latency"],
                series=["volume", "latency"],
                aggregate="mean",
                show_toolbar=True,
                style={"height": 310},
            )
        with dg.Panel("Temporal heat field", class_="surface-card wide-card"):
            dg.Heatmap(
                HEATMAP,
                x_labels=[f"{hour:02d}:00" for hour in range(0, 24, 2)],
                y_labels=[f"Line {index}" for index in range(1, 7)],
                title="Model response latency",
                colormap="magma",
                style={"height": 330},
            )
        with dg.Panel("Analysis controls", class_="surface-card"):
            dg.Dropdown(("All owners", "Ingest", "Models", "Quality", "Export"), value="All owners")
            dg.RangeSlider((18, 82), min=0, max=100, step=1)
            dg.ToggleSwitch("Include maintenance windows", checked=False)
            dg.Checkbox("Normalize per route", checked=True)
            dg.NumberInput(30, min=5, max=240, step=5)
            with dg.FlowLayout(gap=8, row_gap=8):
                dg.Button("Apply model", class_="primary-action")
                dg.SmallButton("Reset")


def build_automation() -> None:
    page_heading(
        "AUTOMATION / RELEASE CONTROL",
        "Workflow composer",
        "Forms, code, logs, tree navigation, and constrained panels share a responsive work surface.",
    )
    with dg.GridLayout(columns=2, min_column_width=380, gap=12):
        with dg.Panel("Deployment request", class_="surface-card workflow-card"):
            dg.TextInput("north.model.calibration.v24", placeholder="Deployment route")
            dg.Dropdown(("Canary", "Staged", "Full rollout"), value="Canary")
            dg.TextArea(
                "Validate north-plant drift metrics before promoting the calibration bundle.",
                rows=5,
            )
            with dg.FlowLayout(gap=8, row_gap=8):
                dg.DateInput("2026-07-24")
                dg.TimeInput("14:30")
                dg.Tag("approval required", level="warning")
            with dg.FlowLayout(gap=8, row_gap=8):
                dg.Button("Queue deployment", class_="primary-action")
                dg.SmallButton("Save draft")

        with dg.Panel("Pipeline graph", class_="surface-card workflow-card"):
            with dg.ScrollArea(axis="y", style={"height": 220}, gap=4):
                with dg.TreeView(selected="validate"):
                    with dg.TreeNode("Calibration release", node_id="root", expanded=True):
                        dg.TreeNode("Collect telemetry", node_id="collect", leaf=True)
                        dg.TreeNode("Validate model", node_id="validate", leaf=True)
                        dg.TreeNode("Canary rollout", node_id="canary", leaf=True)
                        dg.TreeNode("Promote bundle", node_id="promote", leaf=True)
            dg.Separator()
            with dg.FlowLayout(gap=7, row_gap=7):
                dg.Tag("collect · ready", level="success")
                dg.Tag("validate · active", level="info")
                dg.Tag("canary · queued", level="neutral")

        with dg.Panel("Policy script", class_="surface-card workflow-card"):
            dg.CodeEditor(
                "def approve(snapshot):\n"
                "    healthy = snapshot.p95_ms < 70\n"
                "    stable = snapshot.error_rate < 0.02\n"
                "    return healthy and stable\n",
                language="python",
                rows=11,
                style={"height": 260},
            )
        with dg.Panel("Execution stream", class_="surface-card workflow-card"):
            dg.LogView(
                [
                    "14:22:08  telemetry snapshot accepted",
                    "14:22:09  validation stage started",
                    "14:22:12  p95 latency within policy",
                    "14:22:15  awaiting owner approval",
                ],
                rows=12,
                follow=True,
                wrap=True,
                variant="activity",
                style={"height": 260},
            )


def build_settings() -> None:
    page_heading(
        "PLATFORM / PREFERENCES",
        "Control surface",
        "A compact settings page stresses labels, form controls, flow wrapping, and intrinsic widget widths.",
    )
    with dg.GridLayout(columns=3, min_column_width=290, gap=12):
        with dg.Panel("Appearance", class_="surface-card settings-card"):
            dg.ColorPicker((116, 240, 199, 255), title="Accent")
            dg.Dropdown(("Midnight", "Carbon", "High contrast"), value="Midnight")
            dg.ToggleSwitch("Reduced motion", checked=False)
            dg.ToggleSwitch("Compact density", checked=True)
        with dg.Panel("Notifications", class_="surface-card settings-card"):
            dg.Checkbox("Incident escalations", checked=True)
            dg.Checkbox("Deployment updates", checked=True)
            dg.Checkbox("Weekly operations digest", checked=False)
            dg.Dropdown(("Immediate", "Five minute batch", "Hourly digest"), value="Immediate")
        with dg.Panel("Runtime", class_="surface-card settings-card"):
            dg.Slider(0.82, min=0.25, max=1.0, step=0.05)
            dg.NumberInput(90, min=30, max=240, step=10)
            dg.ToggleSwitch("GPU telemetry", checked=True)
            with dg.FlowLayout(gap=7, row_gap=7):
                dg.Badge("wgpu", level="success")
                dg.Tag("1.5x ready", level="info")
                dg.Tag("strict layout", level="neutral")


def build_window() -> dg.Window:
    with dg.Window("Aurora Operations Command Center", width=1440, height=900) as window:
        with dg.AppShell(class_="aurora-shell"):
            state.sidebar = dg.Sidebar(
                title="AURORA",
                width=232,
                collapsed_width=72,
                class_="aurora-sidebar",
                id="aurora-sidebar",
            )
            with state.sidebar:
                dg.Label("Operations command", class_="sidebar-subtitle")
                dg.Label("NORTH PLANT", class_="sidebar-section", wrap=False)
                dg.NavItem("Overview", page="overview", badge="live")
                dg.NavItem("Analytics", page="analytics")
                dg.NavItem("Automation", page="automation", badge="3")
                dg.NavItem("Settings", page="settings")
                dg.Spacer(height=10)
                dg.Label("PLATFORM", class_="sidebar-section", wrap=False)
                with dg.Panel(class_="sidebar-health"):
                    with dg.FlowLayout(gap=7, row_gap=5, style={"align_items": "center"}):
                        dg.LED(True)
                        dg.Label("All services nominal", class_="sidebar-health-title")
                    dg.Label("Last sync · 14 seconds ago", class_="sidebar-subtitle")
                    dg.ProgressBar(0.88, show_value=False)

            with dg.WorkbenchLayout(gap=8, padding=10, class_="aurora-workbench"):
                with dg.MenuBar(height=30):
                    with dg.Menu("Workspace", id="workspace-menu"):
                        dg.MenuItem("Overview", on_click=lambda: navigate("overview"))
                        dg.MenuItem("Analytics", on_click=lambda: navigate("analytics"))
                        dg.MenuItem("Automation", on_click=lambda: navigate("automation"))
                    with dg.Menu("Actions", id="actions-menu"):
                        dg.MenuItem("Refresh operational feed", on_click=randomize_table)
                        dg.MenuItem("Open review panel", on_click=show_modal)
                    with dg.Menu("Help"):
                        dg.MenuItem("Layout diagnostics", on_click=lambda: set_status("Layout diagnostics requested"))

                with dg.Toolbar(class_="command-toolbar"):
                    dg.IconButton(
                        "menu",
                        tooltip="Toggle navigation",
                        on_click=toggle_sidebar,
                        id="sidebar-toggle",
                    )
                    dg.ToolbarSeparator()
                    dg.SearchBox(
                        "",
                        placeholder="Search routes, owners, or commands",
                        class_="command-search",
                        id="command-search",
                    )
                    dg.Spacer()
                    dg.SmallButton("Refresh", on_click=randomize_table)
                    dg.Button(
                        "New review",
                        class_="primary-action",
                        on_click=show_modal,
                        id="new-review",
                    )
                    dg.Badge("online", level="success")

                with dg.Body():
                    state.pages = dg.Pages(value="overview", id="aurora-pages")
                    with state.pages:
                        with dg.Page("overview", title="Overview"):
                            with page_scroller("overview-scroll"):
                                build_overview()
                        with dg.Page("analytics", title="Analytics"):
                            with page_scroller("analytics-scroll"):
                                build_analytics()
                        with dg.Page("automation", title="Automation"):
                            with page_scroller("automation-scroll"):
                                build_automation()
                        with dg.Page("settings", title="Settings"):
                            with page_scroller("settings-scroll"):
                                build_settings()

                with dg.StatusBar(height=26, class_="aurora-status"):
                    state.status_badge = dg.Badge("ready", level="success")
                    state.status = dg.Label(
                        "Command center ready",
                        wrap=False,
                        style={"flex": 1, "min_width": 0},
                        tooltip="Shows navigation, refresh, selection, and review activity.",
                    )
                    dg.Tag("layout audit target", level="neutral")

    state.modal = dg.Modal(
        "Create operational review",
        open=False,
        width=560,
        height=360,
        parent=window,
        id="review-modal",
    )
    with state.modal:
        dg.Label(
            "Capture a review task without allowing the dialog actions or fields to escape at narrow sizes.",
            class_="page-description",
        )
        dg.TextInput("north.line-2.cell-07", placeholder="Route")
        dg.Dropdown(("Model owner", "Ingest owner", "Quality owner"), value="Model owner")
        dg.TextArea("Investigate p95 drift and attach the current telemetry snapshot.", rows=4)
        with dg.FlowLayout(gap=8, row_gap=8, style={"justify_content": "flex_end"}):
            dg.SmallButton(
                "Cancel",
                on_click=lambda: state.modal.close() if state.modal else None,
                id="review-cancel",
            )
            dg.Button(
                "Create review",
                class_="primary-action",
                on_click=lambda: (
                    state.modal.close(),
                    set_status("Review task created", "success"),
                )
                if state.modal
                else None,
            )
    return window


def build_app() -> tuple[dg.App, dg.Window]:
    app = dg.App(theme=dg.Theme.dark(accent="#74f0c7", radius=9))
    app.stylesheet(
        """
        :root {
            --aurora-bg: #071017;
            --aurora-panel: rgba(18, 30, 40, 0.94);
            --aurora-panel-soft: rgba(24, 40, 52, 0.82);
            --aurora-line: rgba(157, 192, 210, 0.18);
            --aurora-muted: rgba(211, 227, 236, 0.66);
            --aurora-text: rgba(244, 251, 255, 0.96);
            --aurora-accent: #74f0c7;
            --aurora-blue: #79a9ff;
        }

        Window {
            background:
                radial-gradient(circle at 86% 8%, rgba(70, 130, 180, 0.20), transparent 36%),
                linear-gradient(145deg, #071017, #0a151e 58%, #08121a);
            color: var(--aurora-text);
            font-size: 13px;
        }

        AppShell.aurora-shell {
            background: transparent;
        }

        Sidebar.aurora-sidebar {
            background:
                linear-gradient(180deg, rgba(13, 27, 37, 0.98), rgba(8, 18, 26, 0.98));
            border-right: 1px solid var(--aurora-line);
            border-radius: 0;
            padding: 16px;
            gap: 7px;
        }

        .sidebar-subtitle,
        .muted,
        .metric-detail,
        .page-description {
            color: var(--aurora-muted);
        }

        .sidebar-subtitle {
            font-size: 11px;
        }

        .sidebar-section,
        .eyebrow {
            color: rgba(116, 240, 199, 0.78);
            font-size: 10px;
            font-weight: 850;
            letter-spacing: 1.2px;
        }

        Panel.sidebar-health {
            background: rgba(116, 240, 199, 0.055);
            border: 1px solid rgba(116, 240, 199, 0.18);
            box-shadow: none;
            padding: 10px;
            gap: 7px;
        }

        .sidebar-health-title {
            color: rgba(231, 255, 247, 0.96);
            font-weight: 760;
        }

        WorkbenchLayout.aurora-workbench {
            background: transparent;
        }

        Toolbar.command-toolbar,
        StatusBar.aurora-status {
            background: rgba(12, 24, 33, 0.88);
            border: 1px solid var(--aurora-line);
            box-shadow: 0 10px 26px rgba(0, 0, 0, 0.16);
        }

        Toolbar.command-toolbar {
            padding: 5px 7px;
            border-radius: 11px;
            gap: 7px;
        }

        ScrollArea.page-scroll {
            padding: 8px 14px 22px 4px;
            gap: 14px;
        }

        ScrollArea.page-scroll::scrollbar-track {
            width: 8px;
            padding: 1px;
            background: rgba(255, 255, 255, 0.055);
            border-radius: 999px;
        }

        ScrollArea.page-scroll::scrollbar-thumb {
            width: 6px;
            background: rgba(116, 240, 199, 0.54);
            border-radius: 999px;
        }

        .page-heading {
            gap: 4px;
        }

        .page-title {
            color: white;
            font-size: 26px;
            font-weight: 880;
        }

        .page-description {
            font-size: 12px;
            line-height: 1.22;
        }

        Panel.surface-card,
        Panel.metric-card,
        Panel.hero-card {
            background:
                linear-gradient(145deg, rgba(27, 45, 58, 0.90), rgba(14, 27, 37, 0.94));
            border: 1px solid var(--aurora-line);
            border-radius: 14px;
            box-shadow: 0 14px 34px rgba(0, 0, 0, 0.20);
            padding: 12px;
            gap: 9px;
        }

        Panel.hero-card {
            background:
                radial-gradient(circle at 85% 20%, rgba(116, 240, 199, 0.16), transparent 42%),
                linear-gradient(135deg, rgba(25, 52, 63, 0.96), rgba(12, 28, 38, 0.98));
            border-color: rgba(116, 240, 199, 0.24);
            padding: 16px;
        }

        .hero-copy {
            flex: 1;
            min-width: 240px;
            gap: 4px;
        }

        .hero-actions {
            align-items: center;
        }

        .hero-actions Button,
        .hero-actions Tag {
            flex-shrink: 0;
        }

        .hero-title {
            color: white;
            font-size: 19px;
            font-weight: 820;
        }

        Button.primary-action {
            background:
                linear-gradient(135deg, rgba(116, 240, 199, 0.98), rgba(66, 202, 180, 0.98));
            border-color: rgba(179, 255, 235, 0.72);
            color: #062019;
            font-weight: 850;
            box-shadow: 0 8px 20px rgba(47, 207, 169, 0.20);
        }

        Panel.metric-card {
            min-height: 142px;
        }

        .metric-label {
            color: rgba(224, 238, 246, 0.78);
            font-size: 11px;
            font-weight: 760;
            text-transform: uppercase;
        }

        .metric-value {
            color: white;
            font-size: 27px;
            font-weight: 900;
        }

        .metric-detail {
            font-size: 11px;
        }

        .metric-progress {
            height: 6px;
        }

        .panel-tools,
        .feed-actions {
            align-items: center;
        }

        Panel.chart-card,
        Panel.pressure-card {
            min-height: 350px;
        }

        .pressure-row {
            gap: 4px;
        }

        .pressure-heading {
            min-height: 24px;
            align-items: center;
        }

        .mono {
            color: rgba(227, 245, 252, 0.84);
            font-family: "Consolas";
        }

        Panel.signal-card {
            background: rgba(255, 200, 97, 0.07);
            border: 1px solid rgba(255, 200, 97, 0.20);
            box-shadow: none;
            padding: 10px;
        }

        .signal-title {
            color: rgba(255, 240, 203, 0.96);
            font-weight: 760;
        }

        Panel.compact-chart {
            min-height: 310px;
        }

        Panel.feed-card {
            min-height: 390px;
        }

        Panel.workflow-card {
            min-height: 360px;
        }

        Panel.settings-card {
            min-height: 250px;
        }

        DataFrameTable {
            min-height: 180px;
        }

        @media (max-width: 760px) {
            Toolbar.command-toolbar {
                padding: 4px;
            }

            SearchBox.command-search {
                width: 100%;
                flex-basis: 100%;
            }

            .page-heading Badge {
                display: none;
            }

            StatusBar.aurora-status Tag {
                display: none;
            }

            ScrollArea.page-scroll {
                padding: 6px 8px 18px 2px;
                gap: 10px;
            }

            .page-title {
                font-size: 18px;
            }

            .hero-title {
                font-size: 16px;
            }

            .hero-copy {
                min-width: 0;
            }

            Panel.metric-card,
            Panel.chart-card,
            Panel.pressure-card,
            Panel.compact-chart,
            Panel.feed-card,
            Panel.workflow-card,
            Panel.settings-card {
                min-height: auto;
            }
        }
        """
    )
    return app, build_window()


def main() -> int:
    app, window = build_app()
    print(app.run(window))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
