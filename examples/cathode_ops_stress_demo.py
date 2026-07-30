"""CATHODE-7 Operations Monitor — vintage-terminal DragonGUI stress demo.

A dense, six-workspace application styled as a late-1970s phosphor CRT
operations console. It is built to stress the same surfaces the other flagship
demos target — responsive grids, scroll ownership, overlays, charts, tables,
widget parts, and live updates — while pushing the CSS layer hard with
scanline gradients, generated content, keyframe animations, container queries,
and a fully monochrome widget-part restyle.

Run:
    python examples/cathode_ops_stress_demo.py
    python examples/cathode_ops_stress_demo.py --style amber
    python examples/cathode_ops_stress_demo.py --style ice --rows 1200
    python examples/cathode_ops_stress_demo.py --no-live
"""

from __future__ import annotations

import argparse
import math
import random
import sys
import threading
import time
from pathlib import Path
from typing import Any

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:  # ScatterPlot2D wants array columns; the demo degrades gracefully without it.
    import numpy as np
except ImportError:  # pragma: no cover - optional dependency
    np = None


# ---------------------------------------------------------------------------
# Palettes
# ---------------------------------------------------------------------------
# Every visual token lives here. The stylesheet below is written once against
# `var(--crt-*)` names, so a style switch is a pure cascade change.

PALETTES: dict[str, dict[str, str]] = {
    "phosphor": {
        "label": "P1 PHOSPHOR / GREEN",
        "deep": "#010603",
        "bg": "#03130a",
        "panel": "rgba(6, 32, 18, 0.90)",
        "panel_alt": "rgba(4, 22, 13, 0.94)",
        "field": "rgba(2, 14, 8, 0.96)",
        "line": "rgba(61, 255, 135, 0.30)",
        "line_soft": "rgba(61, 255, 135, 0.14)",
        "fg": "#3dff87",
        "dim": "rgba(61, 255, 135, 0.62)",
        "bright": "#d2ffe4",
        "glow": "rgba(61, 255, 135, 0.34)",
        "halo": "rgba(61, 255, 135, 0.10)",
        "select": "rgba(61, 255, 135, 0.22)",
        "scan": "rgba(0, 0, 0, 0.46)",
        "scan_soft": "rgba(0, 0, 0, 0.24)",
        "ok": "#4dff9e",
        "warn": "#ffd24a",
        "alarm": "#ff5f4d",
        "info": "#7ef0ff",
        # Paint / chart colors (opaque hex only — the paint API takes literals).
        "trace": "#4dff9e",
        "trace_glow": "#0f5c33",
        "trace_alt": "#c9ff6b",
        "scope_face": "#031309",
        "scope_grid": "#0d4a29",
        "map_cold": "#0a3a20",
        "map_warm": "#2f9c5c",
        "map_hot": "#8dffbb",
        "map_bad": "#ff5f4d",
        "chart_a": "#4dff9e",
        "chart_b": "#c9ff6b",
        "chart_c": "#1f9e63",
        "chart_d": "#7ef0ff",
        "chart_e": "#ffd24a",
        "colormap": "greens",
    },
    "amber": {
        "label": "P3 PHOSPHOR / AMBER",
        "deep": "#080300",
        "bg": "#160c02",
        "panel": "rgba(46, 24, 4, 0.90)",
        "panel_alt": "rgba(32, 16, 2, 0.94)",
        "field": "rgba(20, 10, 1, 0.96)",
        "line": "rgba(255, 178, 64, 0.30)",
        "line_soft": "rgba(255, 178, 64, 0.14)",
        "fg": "#ffb642",
        "dim": "rgba(255, 182, 66, 0.62)",
        "bright": "#ffe8bd",
        "glow": "rgba(255, 178, 64, 0.34)",
        "halo": "rgba(255, 156, 32, 0.12)",
        "select": "rgba(255, 178, 64, 0.22)",
        "scan": "rgba(0, 0, 0, 0.48)",
        "scan_soft": "rgba(0, 0, 0, 0.26)",
        "ok": "#ffcf6b",
        "warn": "#ffe36a",
        "alarm": "#ff6a3d",
        "info": "#ffd9a0",
        "trace": "#ffb642",
        "trace_glow": "#6b4104",
        "trace_alt": "#ffe36a",
        "scope_face": "#140a01",
        "scope_grid": "#5a3607",
        "map_cold": "#402301",
        "map_warm": "#b2751a",
        "map_hot": "#ffdca1",
        "map_bad": "#ff6a3d",
        "chart_a": "#ffb642",
        "chart_b": "#ffe36a",
        "chart_c": "#b2751a",
        "chart_d": "#ff8c2b",
        "chart_e": "#ffdca1",
        "colormap": "hot",
    },
    "ice": {
        "label": "DEC VR / COOL WHITE",
        "deep": "#010508",
        "bg": "#040e16",
        "panel": "rgba(9, 27, 40, 0.90)",
        "panel_alt": "rgba(5, 19, 29, 0.94)",
        "field": "rgba(3, 12, 19, 0.96)",
        "line": "rgba(143, 216, 255, 0.28)",
        "line_soft": "rgba(143, 216, 255, 0.13)",
        "fg": "#8fd8ff",
        "dim": "rgba(143, 216, 255, 0.62)",
        "bright": "#e9f8ff",
        "glow": "rgba(143, 216, 255, 0.32)",
        "halo": "rgba(80, 180, 255, 0.12)",
        "select": "rgba(143, 216, 255, 0.22)",
        "scan": "rgba(0, 0, 0, 0.44)",
        "scan_soft": "rgba(0, 0, 0, 0.22)",
        "ok": "#7ef0d0",
        "warn": "#ffd24a",
        "alarm": "#ff6b7a",
        "info": "#8fd8ff",
        "trace": "#8fd8ff",
        "trace_glow": "#134a68",
        "trace_alt": "#7ef0d0",
        "scope_face": "#03101a",
        "scope_grid": "#0f3f5c",
        "map_cold": "#0b3247",
        "map_warm": "#3a8fbe",
        "map_hot": "#d6f2ff",
        "map_bad": "#ff6b7a",
        "chart_a": "#8fd8ff",
        "chart_b": "#7ef0d0",
        "chart_c": "#3a8fbe",
        "chart_d": "#c9b6ff",
        "chart_e": "#ffd24a",
        "colormap": "blues",
    },
}


def theme_for(palette: dict[str, str]) -> dg.Theme:
    return dg.Theme.dark(
        background=palette["deep"],
        surface=palette["bg"],
        surface_alt=palette["panel_alt"],
        text=palette["fg"],
        muted_text=palette["dim"],
        accent=palette["fg"],
        border=palette["line"],
        danger=palette["alarm"],
        warning=palette["warn"],
        success=palette["ok"],
        focus=palette["bright"],
        disabled=palette["line"],
        radius=0,
        spacing=6,
        font_size=13,
    )


# ---------------------------------------------------------------------------
# Deterministic demo data
# ---------------------------------------------------------------------------


class ColumnFrame:
    """Minimal column-oriented frame accepted by the plot widgets."""

    def __init__(self, **columns: list[Any]) -> None:
        self.columns = tuple(columns)
        self.dtypes = tuple(
            "float64"
            if all(isinstance(value, (int, float)) for value in values)
            else "str"
            for values in columns.values()
        )
        self.shape = (len(next(iter(columns.values()), [])), len(columns))
        self._columns = columns

    def __getitem__(self, column: str) -> list[Any]:
        return self._columns[column]


SAMPLES = list(range(240))
TELEMETRY = ColumnFrame(
    tick=[float(index) for index in SAMPLES],
    core=[
        58.0 + math.sin(index / 13.0) * 17.0 + math.sin(index / 3.1) * 4.5
        for index in SAMPLES
    ],
    channel=[
        41.0 + math.cos(index / 9.4 + 0.7) * 12.0 + math.sin(index / 2.3) * 3.1
        for index in SAMPLES
    ],
)

DEVICES = ColumnFrame(
    device=["TAPE-0", "TAPE-1", "DRUM-0", "DISC-0", "PUNCH", "PRINTER"],
    throughput=[88.0, 71.0, 64.0, 96.0, 22.0, 39.0],
    errors=[3.0, 11.0, 6.0, 2.0, 18.0, 7.0],
)

CORE_MATRIX = [
    [
        round(
            24
            + row * 4.5
            + math.sin(column / 2.6 + row * 0.8) * 9
            + math.cos(column / 3.7) * 4,
            2,
        )
        for column in range(16)
    ]
    for row in range(8)
]

QUEUES = ("SYSGEN", "BATCH-A", "BATCH-B", "TELETYPE", "SPOOL")
OPERATORS = ("R.HOPPER", "K.THOMPSON", "M.WILKES", "A.PERLIS", "J.BACKUS")
STATES = ("RUNNING", "HOLD", "WAIT-IO", "READY", "ABEND")
DEVICE_NAMES = ("TAPE-0", "DRUM-0", "DISC-0", "PUNCH", "PRINTER", "TAPE-1")


def make_jobs(count: int, *, seed: int = 19770614) -> list[dict[str, Any]]:
    rng = random.Random(seed)
    return [
        {
            "job": f"JOB{1000 + index:05d}",
            "queue": QUEUES[index % len(QUEUES)],
            "operator": OPERATORS[index % len(OPERATORS)],
            "state": STATES[(index * 3) % len(STATES)],
            "device": DEVICE_NAMES[(index * 5) % len(DEVICE_NAMES)],
            "cpu_ms": round(18.0 + (index * 11.7) % 940.0, 1),
            "core_kb": 4 * (2 + ((index * 7) % 30)),
            "cards": 120 + (index * 37) % 2400,
            "age": f"{(index * 4) % 59:02d}:{(index * 17) % 59:02d}",
            "parity": "OK" if rng.random() > 0.08 else "CHK",
        }
        for index in range(count)
    ]


BOOT_LINES = (
    "CATHODE-7 SYSTEM MONITOR  REV 3.11",
    "CORE 64K .................. OK",
    "DRUM STORE 512K ........... OK",
    "TAPE CONTROLLER 0/1 ....... OK",
    "TELETYPE ASR-33 ........... READY",
    "LOADING SUPERVISOR ........ DONE",
)

CONSOLE_LOG = (
    "07:14:02  SUPERVISOR RESIDENT",
    "07:14:02  CLOCK SYNC 60HZ LOCKED",
    "07:14:03  JOB01004 ATTACHED TAPE-0",
    "07:14:05  PARITY SCAN CYLINDER 000-127 CLEAN",
    "07:14:08  SPOOL QUEUE DEPTH 6",
    "07:14:11  OPERATOR REQUEST: MOUNT VOL 0042",
    "07:14:16  JOB01007 ABEND S0C7 - DUMP TAKEN",
    "07:14:19  RETRY SCHEDULED FOR JOB01007",
)

JCL_TEXT = (
    "//CORESCAN JOB (SYS7),'CATHODE OPS',CLASS=A,MSGLEVEL=(1,1)\n"
    "//STEP01   EXEC PGM=COREMON,PARM='SWEEP,PARITY'\n"
    "//SYSPRINT DD  SYSOUT=*\n"
    "//DRUM     DD  UNIT=DRUM,VOL=SER=000042,DISP=OLD\n"
    "//TAPEIN   DD  UNIT=TAPE,LABEL=(1,SL),DISP=(OLD,KEEP)\n"
    "//SYSIN    DD  *\n"
    "  SWEEP CYLINDER 000 THRU 127\n"
    "  ON PARITY CHECK  HOLD QUEUE 'BATCH-A'\n"
    "/*\n"
)


# ---------------------------------------------------------------------------
# Custom paint widgets
# ---------------------------------------------------------------------------


class CrtScope(dg.PaintWidget):
    """Graticule + afterglow trace, drawn from Python into a native display list."""

    def __init__(
        self,
        values: list[float],
        palette: dict[str, str],
        *,
        caption: str = "CH-A",
        height: int = 190,
        **kwargs: object,
    ) -> None:
        self.values = list(values)
        self.caption = caption
        self.face = palette["scope_face"]
        self.grid = palette["scope_grid"]
        self.trace = palette["trace"]
        self.after = palette["trace_glow"]
        self.marker = palette["map_hot"]
        self._height = height
        super().__init__(extension_type="crt-scope", **kwargs)

    def set_values(self, values: list[float]) -> None:
        self.values = list(values)
        self.repaint()

    def measure(self, constraints: dg.MeasureConstraints) -> dg.Size:
        return constraints.clamp(dg.Size(340, self._height))

    def paint(self, ctx: dg.PaintContext) -> None:
        ctx.rect(0, 0, ctx.width, ctx.height, fill=self.face)

        for step in range(1, 12):
            x = ctx.width * step / 12.0
            ctx.line(x, 0, x, ctx.height, stroke=self.grid, width=1.0)
        for step in range(1, 8):
            y = ctx.height * step / 8.0
            ctx.line(0, y, ctx.width, y, stroke=self.grid, width=1.0)
        ctx.line(0, ctx.height / 2.0, ctx.width, ctx.height / 2.0, stroke=self.grid, width=2.0)

        if len(self.values) < 2:
            return

        pad = 6.0
        low = min(self.values)
        high = max(self.values)
        span = (high - low) or 1.0
        plot_w = max(1.0, ctx.width - pad * 2.0)
        plot_h = max(1.0, ctx.height - pad * 2.0)
        step_x = plot_w / (len(self.values) - 1)
        points = [
            [
                pad + index * step_x,
                pad + plot_h - ((value - low) / span) * plot_h,
            ]
            for index, value in enumerate(self.values)
        ]

        # Two passes: a wide dim pass for phosphor afterglow, then the beam.
        ctx.polyline(points, stroke=self.after, width=5.0)
        ctx.polyline(points, stroke=self.trace, width=1.6)
        ctx.circle(points[-1][0], points[-1][1], 3.2, fill=self.marker)
        ctx.text(8, 6, self.caption, fill=self.trace, font_size=11, font_weight=800)
        ctx.text(
            ctx.width - 8,
            6,
            f"{self.values[-1]:6.2f}V",
            fill=self.trace,
            font_size=11,
            font_weight=800,
            align="right",
        )


class CoreMap(dg.PaintWidget):
    """Core-memory occupancy map: one cell per 1K word block."""

    def __init__(
        self,
        palette: dict[str, str],
        *,
        columns: int = 48,
        rows: int = 12,
        seed: int = 4242,
        height: int = 210,
        **kwargs: object,
    ) -> None:
        rng = random.Random(seed)
        self.columns = columns
        self.rows = rows
        self.cells = [rng.random() for _ in range(columns * rows)]
        self.cold = palette["map_cold"]
        self.warm = palette["map_warm"]
        self.hot = palette["map_hot"]
        self.bad = palette["map_bad"]
        self.face = palette["scope_face"]
        self.label_color = palette["trace"]
        self._height = height
        super().__init__(extension_type="core-map", **kwargs)

    def cycle(self, offset: int) -> None:
        self.cells = self.cells[offset:] + self.cells[:offset]
        self.repaint()

    def measure(self, constraints: dg.MeasureConstraints) -> dg.Size:
        return constraints.clamp(dg.Size(420, self._height))

    def paint(self, ctx: dg.PaintContext) -> None:
        ctx.rect(0, 0, ctx.width, ctx.height, fill=self.face)
        gutter = 46.0
        top = 18.0
        bottom = 6.0
        cell_w = max(1.0, (ctx.width - gutter - 6.0) / self.columns)
        cell_h = max(1.0, (ctx.height - top - bottom) / self.rows)

        ctx.text(6, 4, "CORE MAP  1K/BLOCK", fill=self.label_color, font_size=10, font_weight=800)

        for row in range(self.rows):
            y = top + row * cell_h
            ctx.text(
                gutter - 8,
                y + cell_h / 2.0 - 5.0,
                f"{row * self.columns:04X}",
                fill=self.label_color,
                font_size=9,
                font_weight=700,
                align="right",
            )
            for column in range(self.columns):
                value = self.cells[row * self.columns + column]
                if value > 0.965:
                    fill = self.bad
                elif value > 0.74:
                    fill = self.hot
                elif value > 0.36:
                    fill = self.warm
                else:
                    fill = self.cold
                ctx.rect(
                    gutter + column * cell_w,
                    y,
                    max(1.0, cell_w - 1.2),
                    max(1.0, cell_h - 1.2),
                    fill=fill,
                )


# ---------------------------------------------------------------------------
# Stylesheet
# ---------------------------------------------------------------------------


def root_block(palette: dict[str, str]) -> str:
    keys = (
        "deep",
        "bg",
        "panel",
        "panel_alt",
        "field",
        "line",
        "line_soft",
        "fg",
        "dim",
        "bright",
        "glow",
        "halo",
        "select",
        "scan",
        "scan_soft",
        "ok",
        "warn",
        "alarm",
        "info",
    )
    declarations = "\n".join(
        f"        --crt-{key.replace('_', '-')}: {palette[key]};" for key in keys
    )
    return ":root {\n" + declarations + "\n    }\n"


CHROME_CSS = """
    @keyframes crt-cursor {
        from { opacity: 1; }
        to   { opacity: 0; }
    }

    @keyframes crt-alarm {
        from {
            background: transparent;
            border-color: var(--crt-line);
            outline-color: rgba(0, 0, 0, 0);
            outline-offset: 1px;
        }
        to {
            background: var(--crt-select);
            border-color: var(--crt-alarm);
            outline-color: var(--crt-alarm);
            outline-offset: 3px;
        }
    }

    @keyframes crt-warmup {
        from { opacity: 0.55; scale: 0.998; }
        to   { opacity: 1; scale: 1; }
    }

    @keyframes crt-sweep {
        from { translate: 0 -6px; opacity: 0.10; }
        to   { translate: 0 6px; opacity: 0.42; }
    }

    Window {
        background:
            repeating-linear-gradient(
                0deg,
                var(--crt-scan) 0%,
                var(--crt-scan) 0.20%,
                rgba(0, 0, 0, 0) 0.20%,
                rgba(0, 0, 0, 0) 0.52%),
            radial-gradient(circle at 50% 40%, var(--crt-halo), rgba(0, 0, 0, 0) 66%),
            linear-gradient(180deg, var(--crt-bg), var(--crt-deep));
        color: var(--crt-fg);
        font-family: "Consolas";
        font-size: 13px;
    }

    AppShell.crt-shell,
    WorkbenchLayout.crt-workbench,
    Body,
    Pages,
    Page {
        background: transparent;
        color: var(--crt-fg);
    }

    Sidebar.crt-sidebar {
        background:
            repeating-linear-gradient(
                0deg,
                var(--crt-scan-soft) 0%,
                var(--crt-scan-soft) 0.30%,
                rgba(0, 0, 0, 0) 0.30%,
                rgba(0, 0, 0, 0) 0.80%),
            linear-gradient(180deg, var(--crt-panel), var(--crt-panel-alt));
        border: 1px solid var(--crt-line);
        border-radius: 0;
        padding: 12px 10px;
        gap: 5px;
        box-shadow: inset 0 0 22px rgba(0, 0, 0, 0.55);
    }

    .sidebar-title {
        color: var(--crt-bright);
        font-size: 15px;
        font-weight: 900;
        letter-spacing: 2.4px;
    }

    .sidebar-note,
    .muted,
    .page-description,
    .metric-detail {
        color: var(--crt-dim);
    }

    .sidebar-note { font-size: 11px; }

    .rail-label,
    .kicker {
        color: var(--crt-fg);
        font-size: 10px;
        font-weight: 800;
        letter-spacing: 1.6px;
        text-transform: uppercase;
        opacity: 0.82;
    }

    .rail-label::before { content: "-- "; }
    .rail-label::after  { content: " --"; }

    NavItem::item {
        background: transparent;
        border: 1px solid rgba(0, 0, 0, 0);
        border-radius: 0;
        color: var(--crt-dim);
        transition: background 110ms ease, border-color 110ms ease, color 110ms ease;
    }

    NavItem:hover::item {
        background: var(--crt-line-soft);
        border-color: var(--crt-line);
        color: var(--crt-fg);
    }

    NavItem:selected::item {
        background: var(--crt-select);
        border-color: var(--crt-fg);
        color: var(--crt-bright);
    }

    NavItem::accent {
        width: 3px;
        background: var(--crt-fg);
    }

    NavItem::badge,
    Tab::badge,
    Button::badge,
    SmallButton::badge {
        background: transparent;
        border: 1px solid var(--crt-line);
        border-radius: 0;
        color: var(--crt-fg);
        font-weight: 800;
    }

    MenuBar {
        background: var(--crt-panel-alt);
        border: 1px solid var(--crt-line);
        border-radius: 0;
        color: var(--crt-fg);
        padding: 2px 4px;
    }

    Menu,
    MenuItem {
        background: transparent;
        border-radius: 0;
        color: var(--crt-fg);
        font-weight: 700;
        letter-spacing: 0.6px;
    }

    Menu::menu,
    ContextMenu::menu,
    Dropdown::menu {
        background: var(--crt-panel-alt);
        border: 1px solid var(--crt-fg);
        border-radius: 0;
        box-shadow: 0 0 24px rgba(0, 0, 0, 0.75);
    }

    Menu::item-hover,
    ContextMenu::item-hover,
    MenuItem:hover,
    Dropdown::item-hover,
    Dropdown::item-selected {
        background: var(--crt-select);
        color: var(--crt-bright);
    }

    Menu::item-disabled,
    ContextMenu::item-disabled {
        color: var(--crt-line);
    }

    Toolbar.crt-toolbar,
    StatusBar.crt-status {
        background: var(--crt-panel-alt);
        border: 1px solid var(--crt-line);
        border-radius: 0;
        color: var(--crt-fg);
    }

    Toolbar.crt-toolbar {
        padding: 4px 6px;
        gap: 6px;
    }

    ToolbarSeparator {
        background: var(--crt-line);
    }

    Tabs {
        background: transparent;
        border-radius: 0;
    }

    Tabs::header {
        background: var(--crt-line-soft);
    }

    Tab::tab {
        background: transparent;
        border: 1px solid var(--crt-line-soft);
        border-radius: 0;
        color: var(--crt-dim);
        font-weight: 800;
        letter-spacing: 1.1px;
        text-transform: uppercase;
        transition: color 120ms ease, border-color 120ms ease, background 120ms ease;
    }

    Tab:hover::tab {
        border-color: var(--crt-line);
        color: var(--crt-fg);
    }

    Tab:selected::tab {
        background: var(--crt-select);
        border-color: var(--crt-fg);
        color: var(--crt-bright);
    }

    Tab::accent {
        height: 2px;
        background: var(--crt-fg);
    }

    ScrollArea.crt-page {
        background: transparent;
        padding: 8px 12px 24px 4px;
        gap: 13px;
    }

    /* ScrollArea (and Body, which is a ScrollArea kind) is intentionally absent
       from this group: the checked-in native binary predates the scrollbar-part
       registration in widget_css_capabilities.json and warns on those rules.
       Re-add them after the next `maturin develop`. */
    Panel::scrollbar-track,
    Sidebar::scrollbar-track,
    Modal::scrollbar-track,
    DataFrameTable::scrollbar-track {
        width: 10px;
        padding: 1px;
        background: var(--crt-field);
        border: 1px solid var(--crt-line-soft);
        border-radius: 0;
    }

    Panel::scrollbar-thumb,
    Sidebar::scrollbar-thumb,
    Modal::scrollbar-thumb,
    DataFrameTable::scrollbar-thumb {
        width: 8px;
        background: var(--crt-select);
        border: 1px solid var(--crt-fg);
        border-radius: 0;
    }

    Splitter::gutter {
        background: var(--crt-line-soft);
        border: 1px solid var(--crt-line);
    }
"""


SURFACE_CSS = """
    Panel {
        background:
            repeating-linear-gradient(
                0deg,
                var(--crt-scan-soft) 0%,
                var(--crt-scan-soft) 0.55%,
                rgba(0, 0, 0, 0) 0.55%,
                rgba(0, 0, 0, 0) 1.45%),
            var(--crt-panel);
        border: 1px solid var(--crt-line);
        border-radius: 0;
        color: var(--crt-fg);
        padding: 11px;
        gap: 8px;
        box-shadow: inset 0 0 18px rgba(0, 0, 0, 0.45);
    }

    Panel Panel {
        background: var(--crt-panel-alt);
        box-shadow: none;
    }

    Panel::accent {
        background: var(--crt-fg);
        width: 2px;
    }

    .page-heading { gap: 3px; }

    .page-title {
        color: var(--crt-bright);
        font-size: 25px;
        font-weight: 900;
        letter-spacing: 3px;
        text-transform: uppercase;
    }

    .page-description {
        font-size: 12px;
        line-height: 1.20;
    }

    Breadcrumbs {
        color: var(--crt-dim);
        font-size: 11px;
        letter-spacing: 1px;
    }

    Panel.banner-card {
        background:
            repeating-linear-gradient(
                0deg,
                var(--crt-scan-soft) 0%,
                var(--crt-scan-soft) 0.40%,
                rgba(0, 0, 0, 0) 0.40%,
                rgba(0, 0, 0, 0) 1.10%),
            linear-gradient(135deg, var(--crt-panel), var(--crt-panel-alt));
        border-color: var(--crt-fg);
        padding: 15px;
        animation: crt-warmup 900ms ease-out 1 normal both running;
    }

    .banner-title {
        color: var(--crt-bright);
        font-size: 19px;
        font-weight: 900;
        letter-spacing: 2.6px;
    }

    .banner-copy { min-width: 260px; flex: 1; gap: 3px; }

    .boot-line {
        color: var(--crt-dim);
        font-size: 11.5px;
        letter-spacing: 0.6px;
    }

    .boot-line::before { content: "  "; }

    .prompt {
        color: var(--crt-fg);
        font-weight: 800;
        letter-spacing: 1px;
    }

    .prompt::before { content: "> "; }

    .cursor {
        color: var(--crt-bright);
        font-weight: 900;
        animation: crt-cursor 640ms step-end infinite alternate both running;
    }

    Panel.metric-card {
        min-height: 132px;
        gap: 6px;
    }

    .metric-label {
        color: var(--crt-dim);
        font-size: 10.5px;
        font-weight: 800;
        letter-spacing: 1.4px;
        text-transform: uppercase;
    }

    .metric-value {
        color: var(--crt-bright);
        font-size: 26px;
        font-weight: 900;
        letter-spacing: 1.4px;
    }

    .metric-detail { font-size: 11px; }

    Panel.metric-alarm {
        border-color: var(--crt-alarm);
        animation: crt-alarm 900ms ease-in-out infinite alternate both running;
    }

    Panel.metric-alarm .metric-value { color: var(--crt-alarm); }

    Panel.chart-card,
    Panel.scope-card { min-height: 330px; }

    Panel.small-chart-card { min-height: 300px; }

    Panel.table-card { min-height: 380px; }

    Panel.work-card { min-height: 330px; }

    Panel.control-card { min-height: 250px; }

    Panel.wide-card { min-height: 280px; }

    .lane-row { gap: 3px; }

    .lane-heading { min-height: 22px; }

    .mono {
        color: var(--crt-bright);
        font-family: "Consolas";
        letter-spacing: 0.8px;
    }

    .ledger {
        color: var(--crt-dim);
        font-family: "Courier New";
        font-size: 12px;
        letter-spacing: 0.4px;
    }

    Panel.notice-card {
        border-color: var(--crt-warn);
        background: var(--crt-panel-alt);
    }

    .notice-title {
        color: var(--crt-warn);
        font-weight: 850;
        letter-spacing: 1.2px;
    }

    Separator { background: var(--crt-line); }

    Panel.sweep-strip {
        height: 8px;
        min-height: 8px;
        padding: 0;
        background: var(--crt-fg);
        border: 0;
        box-shadow: none;
        opacity: 0.22;
        animation: crt-sweep 2600ms ease-in-out infinite alternate both running;
    }
"""


CONTROL_CSS = """
    Button,
    SmallButton,
    IconButton,
    ArrowButton {
        background: transparent;
        border: 1px solid var(--crt-line);
        border-radius: 0;
        color: var(--crt-fg);
        font-weight: 800;
        letter-spacing: 1.1px;
        text-transform: uppercase;
        transition: background 110ms ease, border-color 110ms ease, color 110ms ease;
    }

    Button:hover,
    SmallButton:hover,
    IconButton:hover,
    ArrowButton:hover {
        background: var(--crt-line-soft);
        border-color: var(--crt-fg);
        color: var(--crt-bright);
    }

    Button:active,
    SmallButton:active,
    IconButton:active {
        background: var(--crt-select);
        color: var(--crt-bright);
    }

    Button:focus,
    SmallButton:focus,
    IconButton:focus,
    Dropdown:focus,
    TextInput:focus,
    TextArea:focus,
    NumberInput:focus,
    SearchBox:focus,
    CodeEditor:focus {
        outline: 1px solid var(--crt-bright);
        outline-offset: 2px;
    }

    Button.primary {
        background: var(--crt-select);
        border-color: var(--crt-fg);
        color: var(--crt-bright);
        box-shadow: 0 0 14px var(--crt-glow);
    }

    Button.primary::before { content: "* "; }

    Button:not(.primary):disabled,
    SmallButton:disabled {
        color: var(--crt-line);
        border-color: var(--crt-line-soft);
    }

    IconButton::icon,
    ArrowButton::icon { color: var(--crt-fg); }

    TextInput,
    TextArea,
    SearchBox,
    NumberInput,
    DateInput,
    TimeInput,
    DateTimeInput,
    CodeEditor,
    LogView,
    Dropdown::field {
        background: var(--crt-field);
        border: 1px solid var(--crt-line);
        border-radius: 0;
        color: var(--crt-fg);
    }

    Dropdown {
        background: transparent;
        border: 1px solid var(--crt-line);
        border-radius: 0;
        color: var(--crt-fg);
        letter-spacing: 0.8px;
    }

    Dropdown::chevron { color: var(--crt-fg); }

    Dropdown:open { border-color: var(--crt-fg); }

    NumberInput::stepper {
        background: var(--crt-panel-alt);
        border-radius: 0;
        color: var(--crt-fg);
    }

    NumberInput::stepper-divider,
    NumberInput::divider { background: var(--crt-line); }

    NumberInput::caret,
    CodeEditor::caret { background: var(--crt-bright); }

    CodeEditor::gutter {
        background: var(--crt-panel-alt);
        color: var(--crt-line);
    }

    CodeEditor::line-number { color: var(--crt-dim); }

    Checkbox::box {
        background: var(--crt-field);
        border: 1px solid var(--crt-line);
        border-radius: 0;
    }

    Checkbox:checked::box { border-color: var(--crt-fg); }

    Checkbox::indicator { background: var(--crt-fg); }

    Checkbox::label,
    ToggleSwitch::label { color: var(--crt-fg); letter-spacing: 0.6px; }

    ToggleSwitch::track {
        background: var(--crt-field);
        border: 1px solid var(--crt-line);
        border-radius: 0;
    }

    ToggleSwitch:checked::track { border-color: var(--crt-fg); background: var(--crt-select); }

    ToggleSwitch::thumb {
        background: var(--crt-fg);
        border-radius: 0;
    }

    RadioButton { color: var(--crt-fg); }

    Slider::track,
    RangeSlider::track,
    ProgressBar::track {
        background: var(--crt-field);
        border: 1px solid var(--crt-line-soft);
        border-radius: 0;
    }

    Slider::fill,
    ProgressBar::fill {
        background: var(--crt-fg);
        border-radius: 0;
    }

    RangeSlider::range { background: var(--crt-fg); }

    Slider::thumb,
    RangeSlider::thumb-min,
    RangeSlider::thumb-max {
        background: var(--crt-bright);
        border-radius: 0;
        width: 9px;
    }

    ProgressBar::label,
    RangeSlider::label { color: var(--crt-dim); font-size: 11px; }

    ProgressBar.lane-warn::fill { background: var(--crt-warn); }
    ProgressBar.lane-alarm::fill { background: var(--crt-alarm); }

    LoadingSpinner::track { color: var(--crt-line); }
    LoadingSpinner::arc { color: var(--crt-fg); }

    LED::dot { border-radius: 0; }
    LED::glow { opacity: 0.55; }

    Badge,
    Tag {
        background: transparent;
        border: 1px solid var(--crt-line);
        border-radius: 0;
        color: var(--crt-fg);
        font-weight: 800;
        letter-spacing: 1px;
        text-transform: uppercase;
    }

    Badge[level="warning"], Tag[level="warning"] { border-color: var(--crt-warn); color: var(--crt-warn); }
    Badge[level="danger"],  Tag[level="danger"]  { border-color: var(--crt-alarm); color: var(--crt-alarm); }
    Badge[level="success"], Tag[level="success"] { border-color: var(--crt-ok); color: var(--crt-ok); }
    Badge[level="info"],    Tag[level="info"]    { border-color: var(--crt-info); color: var(--crt-info); }

    Collapsible::header {
        background: var(--crt-panel-alt);
        border: 1px solid var(--crt-line);
        border-radius: 0;
        color: var(--crt-fg);
        font-weight: 800;
        letter-spacing: 1.1px;
        text-transform: uppercase;
    }

    Collapsible::indicator { color: var(--crt-fg); }

    Collapsible::body {
        border: 1px solid var(--crt-line-soft);
        border-radius: 0;
        padding: 8px;
    }

    Collapsible:expanded::header { border-color: var(--crt-fg); }

    TreeNode::row {
        border-radius: 0;
        color: var(--crt-fg);
    }

    TreeNode:selected::row {
        background: var(--crt-select);
        color: var(--crt-bright);
    }

    TreeNode::guide { background: var(--crt-line); }
    TreeNode::indicator { color: var(--crt-fg); }

    Selectable,
    SelectableList {
        border-radius: 0;
        color: var(--crt-fg);
    }

    Selectable:selected {
        background: var(--crt-select);
        color: var(--crt-bright);
    }

    DropZone {
        background: var(--crt-field);
        border: 1px solid var(--crt-line);
        border-radius: 0;
        color: var(--crt-dim);
        letter-spacing: 1px;
        text-transform: uppercase;
    }

    DropZone:hover { border-color: var(--crt-fg); color: var(--crt-fg); }

    DragSource {
        background: var(--crt-panel-alt);
        border: 1px solid var(--crt-line);
        border-radius: 0;
    }

    Modal {
        background: var(--crt-panel-alt);
        border: 1px solid var(--crt-fg);
        border-radius: 0;
        box-shadow: 0 0 36px rgba(0, 0, 0, 0.80);
    }

    Modal::scrim { background: rgba(0, 0, 0, 0.62); }

    CommandPalette {
        background: var(--crt-panel-alt);
        border: 1px solid var(--crt-fg);
        border-radius: 0;
    }

    PropertyGrid { color: var(--crt-fg); }

    ColorPicker {
        border: 1px solid var(--crt-line);
        border-radius: 0;
    }
"""


DATA_CSS = """
    LinePlot,
    Histogram,
    BarChart,
    Heatmap,
    PieChart,
    ScatterPlot2D {
        background: var(--crt-field);
        border: 1px solid var(--crt-line);
        border-radius: 0;
        color: var(--crt-fg);
    }

    Heatmap::grid { color: var(--crt-line-soft); }
    Heatmap::label,
    BarChart::label,
    BarChart::value-label,
    PieChart::label { color: var(--crt-dim); font-size: 11px; }

    DataFrameTable {
        background: var(--crt-field);
        border: 1px solid var(--crt-line);
        border-radius: 0;
        color: var(--crt-fg);
        min-height: 180px;
    }

    DataFrameTable::header {
        background: var(--crt-select);
        color: var(--crt-bright);
        font-weight: 850;
        letter-spacing: 1.1px;
        text-transform: uppercase;
    }

    DataFrameTable::row { color: var(--crt-fg); }
    DataFrameTable::row-selected { background: var(--crt-select); color: var(--crt-bright); }
    DataFrameTable::grid-line { background: var(--crt-line-soft); }

    LogView {
        font-family: "Consolas";
        font-size: 12px;
        letter-spacing: 0.4px;
    }

    LogView::line { color: var(--crt-dim); }
    LogView::debug { color: var(--crt-line); }
    LogView::info { color: var(--crt-fg); }
    LogView::warning { color: var(--crt-warn); }
    LogView::error { color: var(--crt-alarm); }

    ExtensionWidget.scope,
    ExtensionWidget.core-map {
        width: 100%;
        border: 1px solid var(--crt-line);
        border-radius: 0;
        background: var(--crt-field);
    }

    ExtensionWidget.scope { height: 190px; }
    ExtensionWidget.core-map { height: 210px; }

    Panel.channel-tile {
        min-height: 84px;
        padding: 8px;
        gap: 5px;
    }

    .channel-name {
        color: var(--crt-fg);
        font-size: 11px;
        font-weight: 800;
        letter-spacing: 1px;
    }

    .channel-value {
        color: var(--crt-bright);
        font-size: 15px;
        font-weight: 900;
    }

    Panel.load-card { min-height: 300px; }

    .check-row { gap: 8px; }
    .check-copy { gap: 1px; }
"""


RESPONSIVE_CSS = """
    @supports (box-shadow: inset 0 0 8px #000000) {
        Panel.banner-card { box-shadow: inset 0 0 30px rgba(0, 0, 0, 0.60); }
    }

    @supports selector(Panel > Button.primary) {
        Toolbar.crt-toolbar { border-color: var(--crt-line); }
    }

    Panel.query-card {
        container-name: querybox;
        container-type: inline-size;
    }

    @container querybox (max-width: 520px) {
        Panel.query-card SearchBox { width: 100%; flex-basis: 100%; }
        Panel.query-card Tag { display: none; }
    }

    @media (max-width: 1180px) {
        .page-title { font-size: 21px; letter-spacing: 2px; }
        ScrollArea.crt-page { padding: 6px 8px 20px 3px; gap: 10px; }
    }

    @media (max-width: 780px) {
        Toolbar.crt-toolbar { padding: 3px; }
        SearchBox.crt-search { width: 100%; flex-basis: 100%; }
        StatusBar.crt-status Tag { display: none; }
        .page-heading Breadcrumbs { display: none; }
        .page-title { font-size: 17px; letter-spacing: 1.2px; }
        .banner-title { font-size: 15px; }
        .banner-copy { min-width: 0; }
        Panel.metric-card,
        Panel.chart-card,
        Panel.scope-card,
        Panel.small-chart-card,
        Panel.table-card,
        Panel.work-card,
        Panel.control-card,
        Panel.wide-card,
        Panel.load-card,
        Panel.channel-tile {
            min-height: auto;
        }
    }
"""


def build_stylesheet(palette: dict[str, str]) -> str:
    return (
        root_block(palette)
        + CHROME_CSS
        + SURFACE_CSS
        + CONTROL_CSS
        + DATA_CSS
        + RESPONSIVE_CSS
    )


# ---------------------------------------------------------------------------
# Runtime state and behaviour
# ---------------------------------------------------------------------------


class DemoState:
    def __init__(self) -> None:
        self.app: dg.App | None = None
        self.palette: dict[str, str] = PALETTES["phosphor"]
        self.pages: dg.Pages | None = None
        self.tabs: dg.Tabs | None = None
        self.sidebar: dg.Sidebar | None = None
        self.status: dg.Label | None = None
        self.status_badge: dg.Badge | None = None
        self.clock: dg.Label | None = None
        self.modal: dg.Modal | None = None
        self.palette_overlay: dg.CommandPalette | None = None
        self.table: dg.DataFrameTable | None = None
        self.jobs: list[dict[str, Any]] = []
        self.plot: dg.LinePlot | None = None
        self.scope: CrtScope | None = None
        self.core_map: CoreMap | None = None
        self.console_log: dg.LogView | None = None
        self.lane_bars: list[dg.ProgressBar] = []
        self.lane_values: list[dg.Label] = []
        self.metric_values: list[dg.Label] = []
        self.metric_bars: list[dg.ProgressBar] = []
        self.leds: list[dg.LED] = []
        self.tick = len(SAMPLES)
        self.random = random.Random(19770614)
        self.stop = threading.Event()


state = DemoState()

ROUTES = (
    ("console", "Console", "live"),
    ("signals", "Signals", None),
    ("batch", "Batch", "6"),
    ("panel", "Panel", None),
    ("load", "Load", "480"),
    ("core", "Core", None),
)


def set_status(message: str, level: str = "info") -> None:
    if state.status is not None:
        state.status.set_value(message.upper())
    if state.status_badge is not None:
        state.status_badge.set_value(level.upper())


def navigate(route: str) -> None:
    if state.pages is not None:
        state.pages.set_value(route)
    if state.tabs is not None:
        state.tabs.set_value(route)
    set_status(f"opened {route}", "ready")


def toggle_sidebar() -> None:
    if state.sidebar is not None:
        state.sidebar.toggle_collapsed()


def show_modal() -> None:
    if state.modal is not None:
        state.modal.show()
    set_status("operator request pending", "hold")


def show_palette() -> None:
    if state.palette_overlay is not None:
        state.palette_overlay.show()
    set_status("command index opened", "index")


def refresh_jobs() -> None:
    rows = list(state.jobs)
    state.random.shuffle(rows)
    state.jobs = rows
    if state.table is not None:
        state.table.set_frame(rows)
    set_status("job queue re-read from drum", "live")


def dump_core() -> None:
    if state.core_map is not None:
        state.core_map.cycle(state.random.randint(1, 97))
    set_status("core image dumped to tape-1", "dump")


# ---------------------------------------------------------------------------
# Page fragments
# ---------------------------------------------------------------------------


def page_scroll(route: str) -> dg.ScrollArea:
    return dg.ScrollArea(axis="y", gap=13, class_="crt-page", id=f"{route}-scroll")


def page_heading(kicker: str, title: str, description: str) -> None:
    with dg.VLayout(class_="page-heading"):
        dg.Breadcrumbs(
            [("CATHODE-7", "console"), ("MONITOR", "console"), (title.upper(), title.lower())],
            on_select=lambda item: set_status(f"index {item.label}"),
        )
        dg.Label(kicker, class_="kicker", wrap=False)
        with dg.FlowLayout(gap=10, row_gap=6, style={"align_items": "center"}):
            dg.Label(title, class_="page-title", wrap=False)
            dg.Tag("stress target", level="info")
        dg.Label(description, class_="page-description")


def metric_card(
    label: str,
    value: str,
    detail: str,
    *,
    progress: float,
    level: str,
    alarm: bool = False,
) -> None:
    classes = "metric-card" + (" metric-alarm" if alarm else "")
    with dg.Panel(class_=classes):
        with dg.HLayout(style={"align_items": "center"}):
            dg.Label(label, class_="metric-label", wrap=False)
            dg.Spacer()
            state.leds.append(dg.LED(level in {"success", "info"}))
        state.metric_values.append(dg.Label(value, class_="metric-value", wrap=False))
        dg.Label(detail, class_="metric-detail")
        state.metric_bars.append(
            dg.ProgressBar(progress, show_value=False, style={"height": 6})
        )


def build_console_page() -> None:
    page_heading(
        "SUPERVISOR / RESIDENT MONITOR",
        "Main console",
        "Banner, gauges, live trace, scope, distribution charts, and the resident job queue "
        "share one scroll owner. Everything below is native-rendered.",
    )

    with dg.Panel(class_="banner-card"):
        with dg.FlowLayout(gap=18, row_gap=12, style={"align_items": "center"}):
            with dg.VLayout(class_="banner-copy"):
                dg.Label("CATHODE-7 SYSTEM MONITOR", class_="banner-title", wrap=False)
                for line in BOOT_LINES[1:]:
                    dg.Label(line, class_="boot-line", wrap=False)
                with dg.HLayout(style={"gap": 4, "align_items": "center"}):
                    dg.Label("READY", class_="prompt", wrap=False)
                    dg.Label("_", class_="cursor", wrap=False)
            with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
                dg.Button("Attach job", class_="primary", on_click=show_modal, id="crt-attach")
                dg.SmallButton("Re-read queue", on_click=refresh_jobs)
                dg.SmallButton("Dump core", on_click=dump_core)
                dg.Tag("60 hz lock", level="success")

    with dg.GridLayout(
        columns={"default": 4, 1180: 2, 760: 1},
        min_column_width=205,
        gap=12,
        balance_last_row=True,
    ):
        metric_card("Core in use", "47.5K", "of 64K words resident", progress=0.74, level="success")
        metric_card("Channel load", "62%", "selector channel 0 busy", progress=0.62, level="info")
        metric_card("Spool depth", "18", "cards awaiting punch", progress=0.44, level="warning")
        metric_card(
            "Parity checks",
            "3",
            "cylinder 041 flagged for re-scan",
            progress=0.18,
            level="danger",
            alarm=True,
        )

    with dg.GridLayout(columns=2, min_column_width=430, gap=12):
        with dg.Panel("Channel trace", class_="chart-card"):
            with dg.FlowLayout(gap=6, row_gap=6, style={"align_items": "center"}):
                dg.Tag("240 samples", level="neutral")
                dg.SmallButton("1 min", on_click=lambda: set_status("window 1 min"))
                dg.SmallButton("5 min", on_click=lambda: set_status("window 5 min"))
                dg.SmallButton("Fit", on_click=lambda: set_status("trace fitted"))
                dg.Spacer()
                dg.LoadingSpinner(size=15)
            state.plot = dg.LinePlot(
                TELEMETRY,
                x="tick",
                y=["core", "channel"],
                labels=["core", "channel"],
                colors=[state.palette["chart_a"], state.palette["chart_b"]],
                show_toolbar=False,
                show_legend=True,
                max_points=240,
                style={"height": 258},
                id="crt-trace",
            )

        with dg.Panel("Beam scope", class_="scope-card"):
            state.scope = CrtScope(
                [value / 40.0 for value in TELEMETRY["core"][-96:]],
                state.palette,
                caption="CH-A / CORE",
                class_="scope",
            )
            dg.Panel(class_="sweep-strip")
            for label, value, tone in (
                ("Selector channel", 0.86, ""),
                ("Multiplexor", 0.63, ""),
                ("Drum access", 0.48, " lane-warn"),
                ("Tape rewind", 0.21, " lane-alarm"),
            ):
                with dg.VLayout(class_="lane-row"):
                    with dg.HLayout(class_="lane-heading", style={"align_items": "center"}):
                        dg.Label(label, wrap=False)
                        dg.Spacer()
                        state.lane_values.append(
                            dg.Label(f"{value:.0%}", class_="mono", wrap=False)
                        )
                    state.lane_bars.append(
                        dg.ProgressBar(
                            value,
                            show_value=False,
                            class_=f"lane-bar{tone}".strip(),
                            style={"height": 7},
                        )
                    )

    with dg.GridLayout(columns=3, min_column_width=280, gap=12):
        with dg.Panel("Device share", class_="small-chart-card"):
            dg.PieChart(
                labels=["Tape", "Drum", "Disc", "Punch", "Print"],
                values=[31, 24, 21, 11, 13],
                donut=True,
                center_value="6",
                center_label="units",
                show_legend=True,
                legend_position="bottom",
                colors=[
                    state.palette["chart_a"],
                    state.palette["chart_b"],
                    state.palette["chart_c"],
                    state.palette["chart_d"],
                    state.palette["chart_e"],
                ],
                style={"height": 236},
            )
        with dg.Panel("Cylinder heat", class_="small-chart-card"):
            dg.Heatmap(
                CORE_MATRIX,
                x_labels=[f"{index:02d}" for index in range(16)],
                y_labels=[f"H{index}" for index in range(8)],
                title="access / head",
                colormap=state.palette["colormap"],
                style={"height": 236},
            )
        with dg.Panel("Device throughput", class_="small-chart-card"):
            dg.BarChart(
                DEVICES,
                category="device",
                value="throughput",
                aggregate="mean",
                show_toolbar=False,
                colors=[state.palette["chart_a"]],
                style={"height": 236},
            )

    with dg.GridLayout(columns=2, min_column_width=430, gap=12):
        with dg.Panel("Console log", class_="wide-card"):
            state.console_log = dg.LogView(
                CONSOLE_LOG,
                rows=12,
                follow=True,
                wrap=True,
                variant="activity",
                style={"height": 232},
                id="crt-console-log",
            )
        with dg.Panel("Operator notices", class_="wide-card"):
            with dg.Panel(class_="notice-card"):
                with dg.FlowLayout(gap=8, row_gap=6, style={"align_items": "center"}):
                    dg.Badge("2", level="warning")
                    dg.Label("Mount requests outstanding", class_="notice-title")
                dg.Label("VOL 0042 requested by JOB01004 at 07:14:11", class_="muted")
            with dg.Collapsible("Standing instructions", expanded=True):
                for line in (
                    "Hold BATCH-A on any parity check.",
                    "Rewind TAPE-0 before shift change.",
                    "Punch spool must not exceed 24 cards.",
                ):
                    dg.Label(line, class_="ledger")
            with dg.Collapsible("Shift handover (collapsed)", expanded=False):
                dg.Label("Collapsed content must not consume layout space.", class_="muted")

    with dg.Panel("Resident job queue", class_="table-card", id="crt-queue-panel"):
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            dg.SearchBox("", placeholder="FILTER JOB / OPERATOR / STATE", width=290)
            dg.Dropdown(("ALL STATES", *STATES), value="ALL STATES")
            dg.SmallButton("Re-read", on_click=refresh_jobs)
            dg.Spacer()
            dg.Badge(f"{len(state.jobs)} jobs", level="info")
        state.table = dg.DataFrameTable(
            state.jobs[:64],
            page_size=18,
            sample_rows=64,
            sortable=True,
            resizable_columns=True,
            style={"height": 300},
            on_select=lambda selection: set_status(
                f"selected {selection.column}: {selection.value}", "select"
            ),
            id="crt-queue-table",
        )


def build_signals_page() -> None:
    page_heading(
        "ANALYSIS / SIGNAL LABORATORY",
        "Signal lab",
        "Plot grids, a GPU scatter field, and an inspector column stress minimum sizing, "
        "responsive column counts, and vertical reachability.",
    )

    with dg.Panel("Query", class_="query-card"):
        with dg.FlowLayout(gap=9, row_gap=9, style={"align_items": "center"}):
            dg.SearchBox("", placeholder="SEARCH DIMENSION", grow=True)
            dg.Dropdown(("LAST HOUR", "LAST SHIFT", "LAST 24 HOURS"), value="LAST SHIFT")
            dg.Dropdown(("ALL DEVICES", *DEVICE_NAMES), value="ALL DEVICES")
            dg.ToggleSwitch("Anomalies only", checked=False)
            dg.Tag("drum resident", level="neutral")
            dg.Button("Run", class_="primary", on_click=lambda: set_status("query dispatched"))

    with dg.GridLayout(columns=2, min_column_width=400, gap=12):
        with dg.Panel("Channel distribution", class_="wide-card"):
            dg.Histogram(
                TELEMETRY["channel"],
                bins=32,
                mode="count",
                color=state.palette["chart_a"],
                show_toolbar=False,
                style={"height": 292},
            )
        with dg.Panel("Throughput vs errors", class_="wide-card"):
            dg.BarChart(
                DEVICES,
                category="device",
                value=["throughput", "errors"],
                series=["throughput", "errors"],
                aggregate="mean",
                show_toolbar=True,
                colors=[state.palette["chart_a"], state.palette["chart_e"]],
                style={"height": 292},
            )
        with dg.Panel("Access field", class_="wide-card"):
            dg.Heatmap(
                CORE_MATRIX,
                x_labels=[f"C{index:02d}" for index in range(16)],
                y_labels=[f"HEAD {index}" for index in range(8)],
                title="cylinder access density",
                colormap=state.palette["colormap"],
                style={"height": 300},
            )
        with dg.Panel("Inspector", class_="wide-card"):
            dg.Label("Sample window", class_="metric-label", wrap=False)
            dg.RangeSlider((24, 81), min=0, max=100, step=1)
            dg.Label("Beam gain", class_="metric-label", wrap=False)
            dg.Slider(0.72, min=0, max=1, step=0.01)
            dg.NumberInput(120, min=20, max=500, step=10)
            dg.RadioGroup(
                ["AUTOMATIC", "CONSERVATIVE", "AGGRESSIVE"],
                value="AUTOMATIC",
                orientation="horizontal",
            )
            dg.Checkbox("Normalise by channel time", checked=True)
            dg.Checkbox("Include maintenance windows", checked=False)
            with dg.FlowLayout(gap=8, row_gap=8):
                dg.Button("Apply", class_="primary")
                dg.SmallButton("Reset")

    with dg.Panel("Point field", class_="wide-card"):
        if np is None:
            dg.Label(
                "NumPy is not installed — the GPU scatter field is unavailable in this run.",
                class_="muted",
            )
            dg.LinePlot(
                TELEMETRY,
                x="tick",
                y=["core"],
                labels=["core"],
                colors=[state.palette["chart_a"]],
                show_toolbar=False,
                style={"height": 300},
            )
        else:
            dg.Label(
                "60,000 GPU points with a scalar bar, drawn beside CSS-styled chrome.",
                class_="muted",
            )
            dg.ScatterPlot2D(
                make_scatter_frame(),
                x="x",
                y="y",
                scalars="score",
                colormap=state.palette["colormap"],
                point_size=2.4,
                grid=True,
                axis_x="channel offset",
                axis_y="drum angle",
                scalar_bar=True,
                scalar_bar_title="LOAD",
                style={"height": 340},
            )


def make_scatter_frame() -> Any:
    """Build a spiral point field with numpy array columns."""

    rows = 60_000

    class ScatterFrame:
        columns = ("x", "y", "score")
        dtypes = ("float32", "float32", "float32")

        def __init__(self) -> None:
            index = np.arange(rows, dtype=np.float32)
            t = (index + 0.5) / rows
            golden = np.float32(math.pi * (3.0 - math.sqrt(5.0)))
            theta = index * golden
            radius = np.sqrt(t) * np.float32(3.2)
            ripple = np.sin(theta * np.float32(6.0)) * np.float32(0.06)
            self.x = (np.cos(theta) * (radius + ripple)).astype(np.float32)
            self.y = (np.sin(theta) * (radius + ripple)).astype(np.float32)
            self.score = (t * 0.6 + (np.sin(theta * 0.2) + 1.0) * 0.2).astype(np.float32)
            self.shape = (rows, 3)

        def __getitem__(self, column: str) -> Any:
            return getattr(self, column)

    return ScatterFrame()


def build_batch_page() -> None:
    page_heading(
        "OPERATIONS / BATCH CONTROL",
        "Batch deck",
        "Tree navigation, selection lists, forms, drag-and-drop staging, a JCL editor, and a "
        "spool log share a responsive two-column surface.",
    )

    with dg.GridLayout(columns=2, min_column_width=380, gap=12):
        with dg.Panel("Job deck", class_="work-card"):
            with dg.ScrollArea(axis="y", height=196, gap=3):
                with dg.TreeView(selected="scan"):
                    with dg.TreeNode("SYSGEN 03.11", node_id="sysgen", expanded=True):
                        dg.TreeNode("READ CARD DECK", node_id="read", leaf=True)
                        dg.TreeNode("CORE SWEEP", node_id="scan", leaf=True)
                        with dg.TreeNode("TAPE STAGE", node_id="tape", expanded=True):
                            dg.TreeNode("MOUNT VOL 0042", node_id="mount", leaf=True)
                            dg.TreeNode("VERIFY LABEL", node_id="verify", leaf=True)
                        dg.TreeNode("PUNCH RESULTS", node_id="punch", leaf=True)
            dg.Separator()
            dg.SelectableList(
                [
                    {"label": "VERIFY PARITY WINDOW", "value": "parity"},
                    {"label": "ATTACH CORE DUMP", "value": "dump"},
                    {"label": "NOTIFY SHIFT OPERATOR", "value": "notify"},
                    {"label": "SPOOL TO PRINTER 03", "value": "print"},
                ],
                selected=["parity", "dump"],
                selection_mode="multiple",
            )

        with dg.Panel("Submission", class_="work-card"):
            dg.TextInput("CORESCAN", placeholder="JOB NAME")
            dg.Dropdown(("CLASS A", "CLASS B", "CLASS X"), value="CLASS A")
            dg.TextArea(
                "Sweep cylinders 000-127 and hold BATCH-A on any parity check.",
                rows=4,
            )
            with dg.FlowLayout(gap=8, row_gap=8):
                dg.DateInput("1977-06-14")
                dg.TimeInput("07:30")
                dg.DateTimeInput("1977-06-14T07:30:00")
            dg.ToggleSwitch("Require operator approval", checked=True)
            dg.ProgressBar(0.68, label="DECK READY", show_value=True)
            with dg.FlowLayout(gap=8, row_gap=8):
                dg.Button("Submit deck", class_="primary", on_click=show_modal)
                dg.SmallButton("Save draft")

        with dg.Panel("Card staging", class_="work-card"):
            dg.Label("Drag a deck onto the reader.", class_="muted")
            with dg.FlowLayout(gap=8, row_gap=8):
                for payload in ("CORESCAN.JCL", "PARITY.DAT", "VOL0042.LBL", "PUNCH.CTL"):
                    with dg.DragSource(
                        {"deck": payload},
                        drag_kind="card-deck",
                        style={"padding": 7},
                    ):
                        dg.Tag(payload, level="info")
            dg.DropZone(
                "INSERT DECK INTO READER",
                accept="card-deck",
                style={"height": 116},
                on_drop=lambda payload: set_status(f"read {payload}", "read"),
            )
            with dg.FlowLayout(gap=7, row_gap=7):
                dg.Tag("reader ready", level="success")
                dg.Tag("hopper 240", level="neutral")
                dg.Tag("stacker 12", level="neutral")

        with dg.Panel("Control language", class_="work-card"):
            dg.CodeEditor(JCL_TEXT, language="", rows=10, style={"height": 214})
            dg.LogView(
                [
                    "07:30:01 DECK CORESCAN ACCEPTED",
                    "07:30:01 CLASS A QUEUE POSITION 3",
                    "07:30:04 ALLOCATE DRUM VOL 000042",
                    "07:30:06 WAITING OPERATOR APPROVAL",
                ],
                rows=6,
                follow=True,
                wrap=True,
                variant="activity",
                style={"height": 138},
            )


def build_panel_page() -> None:
    page_heading(
        "MAINTENANCE / FRONT PANEL",
        "Control panel",
        "The widget gallery. Every control is restyled through CSS parts and pseudo-states only "
        "— no per-widget inline colors.",
    )

    with dg.GridLayout(
        columns={"default": 3, 1120: 2, 720: 1},
        min_column_width=290,
        gap=12,
    ):
        with dg.Panel("Text and numeric", class_="control-card"):
            dg.TextInput("CATHODE-7", placeholder="SYSTEM NAME")
            dg.TextArea("Multi-line entry must stay inside its card.", rows=3)
            dg.NumberInput(42, min=0, max=127, step=1)
            dg.DragNumber(0.625, min=0, max=1, step=0.005)
            dg.DragVector((0.25, 0.5, 0.75), labels=("X", "Y", "Z"), min=0, max=1, step=0.05)

        with dg.Panel("Selection", class_="control-card"):
            dg.Dropdown(("AUTOMATIC", "BALANCED", "MANUAL"), value="BALANCED")
            dg.Checkbox("Enable teletype echo", checked=True)
            dg.Checkbox("Include archived decks", checked=False)
            dg.ToggleSwitch("Live update", checked=True)
            dg.RadioGroup(["COMPACT", "NORMAL", "WIDE"], value="NORMAL", orientation="horizontal")
            dg.SelectableList(["CONSOLE", "SIGNALS", "BATCH", "CORE"], selected=["CONSOLE"])

        with dg.Panel("Ranges and status", class_="control-card"):
            dg.Slider(0.64, min=0, max=1, step=0.01)
            dg.RangeSlider((18, 82), min=0, max=100, step=1)
            dg.ProgressBar(0.74, label="SWEEP", show_value=True)
            dg.ProgressBar(0.38, show_value=False, class_="lane-warn")
            dg.ProgressBar(0.12, show_value=False, class_="lane-alarm")
            with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
                dg.LED(True)
                dg.LED(False)
                dg.Badge("ONLINE", level="success")
                dg.Badge("12", level="warning")
                dg.Tag("BETA", level="info")
                dg.LoadingSpinner(size=17)

        with dg.Panel("Date and time", class_="control-card"):
            dg.DateInput("1977-06-14")
            dg.TimeInput("07:30")
            dg.DateTimeInput("1977-06-14T07:30:00")
            dg.Label("Temporal controls share one baseline and never escape the card.", class_="muted")

        with dg.Panel("Beam and appearance", class_="control-card"):
            dg.ColorPicker((61, 255, 135, 255), title="PHOSPHOR")
            dg.Dropdown(("P1 GREEN", "P3 AMBER", "COOL WHITE"), value="P1 GREEN")
            dg.ToggleSwitch("Reduced motion", checked=False)
            dg.ToggleSwitch("Scanline overlay", checked=True)
            dg.Slider(0.82, min=0.2, max=1.0, step=0.02, tooltip="Beam intensity")

        with dg.Panel("Buttons and disclosure", class_="control-card"):
            with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
                dg.Button("Execute", class_="primary")
                dg.Button("Halt")
                dg.SmallButton("Step")
                dg.SmallButton("Disabled", disabled=True)
                dg.IconButton("play", tooltip="Run")
                dg.IconButton("pause", tooltip="Hold")
                dg.IconButton("stop", tooltip="Halt")
                dg.ArrowButton("right")
            with dg.Collapsible("Advanced switches", expanded=True):
                dg.Checkbox("Retain diagnostics", checked=True)
                dg.Checkbox("Single-cycle mode", checked=False)
            with dg.Collapsible("Locked panel", expanded=False):
                dg.Label("Collapsed content must not reserve space.", class_="muted")

        with dg.Panel("Register file", class_="control-card"):
            dg.PropertyGrid(
                {
                    "ACC": "0o017742",
                    "IX": "0o000031",
                    "PC": "0o004120",
                    "SR": "0o000004",
                    "OVF": False,
                    "CARRY": True,
                },
                label_width=92,
            )

        with dg.Panel("Console typewriter", class_="control-card"):
            dg.LogView(
                [
                    "DEBUG  loader relocated to 0x1200",
                    "INFO   supervisor resident",
                    "WARN   spool depth above 16",
                    "ERROR  parity check cylinder 041",
                ],
                rows=8,
                wrap=True,
                variant="debug",
                style={"height": 150},
            )

        with dg.Panel("Tag rack", class_="control-card"):
            with dg.FlowLayout(gap=6, row_gap=6):
                for index, name in enumerate(
                    (
                        "TAPE-0", "TAPE-1", "DRUM-0", "DISC-0", "PUNCH", "PRINTER",
                        "READER", "CONSOLE", "CLOCK", "CHANNEL-0", "CHANNEL-1", "MPX",
                    )
                ):
                    level = ("neutral", "success", "info", "warning")[index % 4]
                    dg.Tag(name, level=level)


def build_load_page(tiles: int, rows: int) -> None:
    page_heading(
        "LOAD TEST / WIDGET PRESSURE",
        "Load bank",
        f"{tiles} channel tiles, a {rows}-row virtualized table, and a long tag rack in one "
        "scroll owner. This page exists to make the layout and paint path work.",
    )

    with dg.Panel("Channel bank", class_="load-card"):
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            dg.Tag(f"{tiles} tiles", level="info")
            dg.Tag("responsive 6/4/2/1", level="neutral")
            dg.Spacer()
            dg.SmallButton("Cycle map", on_click=dump_core)
        with dg.GridLayout(
            columns={"default": 6, 1250: 4, 900: 3, 640: 2},
            min_column_width=132,
            gap=8,
        ):
            for index in range(tiles):
                value = (index * 37 % 100) / 100.0
                level = "success" if value > 0.6 else "warning" if value > 0.3 else "neutral"
                with dg.Panel(class_="channel-tile"):
                    with dg.HLayout(style={"align_items": "center"}):
                        dg.LED(value > 0.45)
                        dg.Label(f"CH{index:03d}", class_="channel-name", wrap=False)
                        dg.Spacer()
                        dg.Tag(level[:3].upper(), level=level)
                    dg.Label(f"{value * 100:.0f}%", class_="channel-value", wrap=False)
                    dg.ProgressBar(value, show_value=False, style={"height": 5})

    with dg.Panel("Full job register", class_="table-card"):
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            dg.SearchBox("", placeholder="FILTER REGISTER", width=260)
            dg.Dropdown(("ALL QUEUES", *QUEUES), value="ALL QUEUES")
            dg.Spacer()
            dg.Badge(f"{rows} rows", level="info")
        dg.DataFrameTable(
            state.jobs,
            page_size=30,
            sample_rows=min(rows, 2048),
            sortable=True,
            resizable_columns=True,
            style={"height": 420},
        )

    with dg.Panel("Address rack", class_="wide-card"):
        dg.Label("240 inline tags exercise flow wrapping and text measurement.", class_="muted")
        with dg.FlowLayout(gap=5, row_gap=5):
            for index in range(240):
                level = ("neutral", "info", "success", "warning", "danger")[index % 5]
                dg.Tag(f"{index * 64:04X}", level=level)

    with dg.Panel("Spool listing", class_="wide-card"):
        dg.LogView(
            [
                f"{index:05d}  {QUEUES[index % len(QUEUES)]:<9} "
                f"{OPERATORS[index % len(OPERATORS)]:<12} "
                f"{STATES[index % len(STATES)]:<8} {(index * 13) % 999:4d}MS"
                for index in range(400)
            ],
            rows=18,
            follow=False,
            wrap=False,
            variant="default",
            style={"height": 300},
        )


def build_core_page() -> None:
    page_heading(
        "DIAGNOSTIC / CORE DUMP",
        "Core dump",
        "Split panes, a painted core map, a property grid, and the runtime contract table "
        "exercise nested viewport ownership.",
    )

    with dg.Panel("Runtime", class_="wide-card"):
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            dg.Badge("WGPU", level="success")
            dg.Tag("abi3 wheel", level="info")
            dg.Tag("strict layout", level="neutral")
            dg.Tag(state.palette["label"].lower(), level="neutral")
            dg.Spacer()
            dg.SmallButton("Capture snapshot", on_click=lambda: set_status("snapshot captured"))
        state.core_map = CoreMap(state.palette, class_="core-map")

    with dg.Splitter(
        orientation="horizontal",
        gutter_size=6,
        style={"height": 470, "min_height": 340},
    ):
        with dg.Pane(flex=44, min_size=290):
            with dg.Panel("Machine state", style={"flex": 1, "min_height": 0}):
                dg.PropertyGrid(
                    {
                        "Viewport": "1500 x 940",
                        "Scale factor": "1.0",
                        "Renderer": "WGPU",
                        "Core size": "64K words",
                        "Cycle time": "1.8 us",
                        "Structural errors": 0,
                        "Stylesheet warnings": 0,
                        "Snapshot schema": 1,
                    },
                    label_width=134,
                )
        with dg.Pane(flex=56, min_size=320):
            with dg.Panel("Supervisor trace", style={"flex": 1, "min_height": 0}):
                dg.LogView(
                    [
                        "07:30:00 DEBUG  renderer initialised",
                        "07:30:00 INFO   stylesheet cascade resolved",
                        "07:30:01 INFO   responsive grid selected 4 tracks",
                        "07:30:01 INFO   scroll ownership validated",
                        "07:30:02 WARN   spool depth above threshold",
                        "07:30:04 ERROR  parity check cylinder 041",
                        "07:30:05 INFO   retry scheduled",
                    ],
                    rows=16,
                    follow=True,
                    wrap=True,
                    variant="debug",
                    style={"height": "100%"},
                )

    with dg.GridLayout(columns=2, min_column_width=360, gap=12):
        with dg.Panel("Sizing contract", class_="wide-card"):
            dg.DataFrameTable(
                [
                    {"widget": "Panel", "grow": "0", "shrink": "1", "overflow": "visible"},
                    {"widget": "Body", "grow": "1", "shrink": "1", "overflow": "auto"},
                    {"widget": "Sidebar", "grow": "0", "shrink": "1", "overflow": "hidden"},
                    {"widget": "GridLayout", "grow": "1", "shrink": "1", "overflow": "visible"},
                    {"widget": "PaintWidget", "grow": "0", "shrink": "1", "overflow": "clip"},
                ],
                page_size=8,
                sample_rows=8,
                style={"height": 240},
            )
        with dg.Panel("Audit checklist", class_="wide-card"):
            for label, detail in (
                ("Public selectors", "Every rule targets stable widget identity"),
                ("Scroll reachability", "Each page owns explicit vertical scrolling"),
                ("Responsive grids", "4/2/1, 3/2/1, and 6/4/3/2 contracts are active"),
                ("Overlay bounds", "Modal, palette, and context menu stay in the viewport"),
                ("Live updates", "Trace, scope, gauges, and log patch without a rebuild"),
                ("Paint widgets", "Scope and core map repaint from the worker thread"),
            ):
                with dg.HLayout(class_="check-row", style={"align_items": "center"}):
                    dg.LED(True)
                    with dg.VLayout(class_="check-copy"):
                        dg.Label(label, wrap=False)
                        dg.Label(detail, class_="muted")


# ---------------------------------------------------------------------------
# Window
# ---------------------------------------------------------------------------


def build_window(*, tiles: int, rows: int) -> dg.Window:
    with dg.Window(
        "CATHODE-7 Operations Monitor - DragonGUI stress demo",
        width=1500,
        height=940,
    ) as window:
        with dg.AppShell(class_="crt-shell"):
            state.sidebar = dg.Sidebar(
                title="CATHODE-7",
                width=236,
                collapsed_width=70,
                class_="crt-sidebar",
                id="crt-sidebar",
            )
            with state.sidebar:
                dg.Label("SYSTEM MONITOR 3.11", class_="sidebar-note", wrap=False)
                dg.Label("WORKSPACES", class_="rail-label", wrap=False)
                for route, label, badge in ROUTES:
                    dg.NavItem(label.upper(), page=route, badge=badge)
                dg.Spacer(height=8)
                dg.Label("MACHINE", class_="rail-label", wrap=False)
                with dg.Panel():
                    with dg.FlowLayout(gap=7, row_gap=5, style={"align_items": "center"}):
                        dg.LED(True)
                        dg.Label("SUPERVISOR UP", class_="mono", wrap=False)
                    state.clock = dg.Label("07:30:00 SHIFT A", class_="sidebar-note", wrap=False)
                    dg.ProgressBar(0.74, show_value=False, style={"height": 5})
                dg.Spacer(height=6)
                dg.Label("BEAM", class_="rail-label", wrap=False)
                dg.Label(state.palette["label"], class_="sidebar-note")

            with dg.WorkbenchLayout(gap=7, padding=9, class_="crt-workbench"):
                with dg.MenuBar(height=30):
                    with dg.Menu("System", id="crt-system-menu"):
                        for route, label, _badge in ROUTES:
                            dg.MenuItem(
                                label.upper(),
                                on_click=lambda selected=route: navigate(selected),
                            )
                    with dg.Menu("Operator"):
                        dg.MenuItem("ATTACH JOB", on_click=show_modal)
                        dg.MenuItem("RE-READ QUEUE", on_click=refresh_jobs)
                        dg.MenuItem("DUMP CORE", on_click=dump_core)
                        dg.MenuItem("COMMAND INDEX", on_click=show_palette)
                    with dg.Menu("View"):
                        dg.MenuItem("TOGGLE RAIL", on_click=toggle_sidebar)
                        dg.MenuItem("DIAGNOSTICS", on_click=lambda: navigate("core"))
                    with dg.Menu("Help"):
                        dg.MenuItem("MAINTENANCE", disabled=True)

                with dg.Toolbar(class_="crt-toolbar"):
                    dg.IconButton(
                        "menu",
                        tooltip="Toggle rail",
                        on_click=toggle_sidebar,
                        id="crt-rail-toggle",
                    )
                    dg.ToolbarSeparator()
                    dg.IconButton("search", tooltip="Command index", on_click=show_palette)
                    dg.IconButton("refresh", tooltip="Re-read queue", on_click=refresh_jobs)
                    dg.IconButton("save", tooltip="Dump core", on_click=dump_core)
                    dg.ToolbarSeparator()
                    dg.SearchBox(
                        "",
                        placeholder="SEARCH JOBS, DEVICES, COMMANDS",
                        class_="crt-search",
                        id="crt-search",
                    )
                    dg.Spacer()
                    dg.SmallButton("Halt", on_click=lambda: set_status("halt requested", "halt"))
                    dg.Button("Attach", class_="primary", on_click=show_modal, id="crt-attach-bar")
                    dg.Badge("ONLINE", level="success")

                state.tabs = dg.Tabs(value="console", on_change=navigate, id="crt-tabs")
                with state.tabs:
                    for route, label, badge in ROUTES:
                        with dg.Tab(label.upper(), value=route, badge=badge):
                            pass

                with dg.Body():
                    state.pages = dg.Pages(value="console", id="crt-pages")
                    with state.pages:
                        with dg.Page("console", title="Console"):
                            with page_scroll("console"):
                                build_console_page()
                        with dg.Page("signals", title="Signals"):
                            with page_scroll("signals"):
                                build_signals_page()
                        with dg.Page("batch", title="Batch"):
                            with page_scroll("batch"):
                                build_batch_page()
                        with dg.Page("panel", title="Panel"):
                            with page_scroll("panel"):
                                build_panel_page()
                        with dg.Page("load", title="Load"):
                            with page_scroll("load"):
                                build_load_page(tiles, rows)
                        with dg.Page("core", title="Core"):
                            with page_scroll("core"):
                                build_core_page()

                with dg.StatusBar(height=27, class_="crt-status"):
                    state.status_badge = dg.Badge("READY", level="success")
                    state.status = dg.Label(
                        "CATHODE-7 MONITOR READY",
                        wrap=False,
                        style={"flex": 1, "min_width": 0},
                    )
                    dg.Tag("layout stress", level="neutral")
                    dg.Tag(state.palette["label"], level="neutral")

    state.modal = dg.Modal(
        "ATTACH JOB",
        open=False,
        width=560,
        height=330,
        parent=window,
        id="crt-modal",
    )
    with state.modal:
        dg.Label(
            "Attach a deck while verifying that dialog controls stay bounded at compact sizes.",
            class_="page-description",
        )
        dg.TextInput("CORESCAN", placeholder="JOB NAME")
        dg.Dropdown(("CLASS A", "CLASS B", "CLASS X"), value="CLASS A")
        dg.TextArea("Sweep cylinders 000-127 before the shift change.", rows=3)
        dg.ToggleSwitch("Require operator approval", checked=True)
        with dg.FlowLayout(gap=8, row_gap=8, style={"justify_content": "flex_end"}):
            dg.SmallButton(
                "Cancel",
                id="crt-modal-cancel",
                on_click=lambda: state.modal.close() if state.modal else None,
            )
            dg.Button(
                "Attach",
                class_="primary",
                on_click=lambda: (
                    state.modal.close(),
                    set_status("job attached to class a", "run"),
                )
                if state.modal
                else None,
            )

    state.palette_overlay = dg.CommandPalette(
        [
            dg.Command(f"route.{route}", f"Open {label}", on_run=lambda r=route: navigate(r))
            for route, label, _badge in ROUTES
        ]
        + [
            dg.Command("job.attach", "Attach job", on_run=show_modal),
            dg.Command("queue.reread", "Re-read job queue", on_run=refresh_jobs),
            dg.Command("core.dump", "Dump core image", on_run=dump_core),
        ],
        open=False,
        title="COMMAND INDEX",
        placeholder="TYPE A COMMAND",
        max_results=8,
        parent=window,
        id="crt-palette",
    )

    with dg.ContextMenu(target="crt-queue-panel", width=230, parent=window):
        dg.MenuItem("HOLD JOB", on_click=lambda: set_status("job held", "hold"))
        dg.MenuItem("RELEASE JOB", on_click=lambda: set_status("job released", "run"))
        dg.MenuItem("PURGE OUTPUT", on_click=lambda: set_status("output purged", "purge"))
        dg.MenuItem("CANCEL (DISABLED)", disabled=True)

    return window


# ---------------------------------------------------------------------------
# Live worker
# ---------------------------------------------------------------------------


def live_worker(app: dg.App) -> None:
    """Push telemetry into the trace, scope, gauges, LEDs, and console log."""

    lane_bases = (0.86, 0.63, 0.48, 0.21)
    metric_bases = (0.74, 0.62, 0.44, 0.18)
    metric_texts = ("{:.1f}K", "{:.0f}%", "{:.0f}", "{:.0f}")
    metric_scales = (64.0, 100.0, 40.0, 12.0)
    scope_window: list[float] = [value / 40.0 for value in TELEMETRY["core"][-96:]]

    while not state.stop.is_set():
        time.sleep(0.28)
        tick = state.tick
        state.tick += 1
        core = 58.0 + math.sin(tick / 13.0) * 17.0 + math.sin(tick / 3.1) * 4.5
        channel = 41.0 + math.cos(tick / 9.4 + 0.7) * 12.0 + math.sin(tick / 2.3) * 3.1
        scope_window.append(core / 40.0)
        del scope_window[:-96]
        frame = list(scope_window)

        def apply_stream(tick: int = tick, core: float = core, channel: float = channel) -> None:
            if state.plot is not None:
                state.plot.append_points([float(tick)], [core], series="core", max_points=240)
                state.plot.append_points([float(tick)], [channel], series="channel", max_points=240)
            if tick % 8 == 0 and state.console_log is not None:
                stamp = time.strftime("%H:%M:%S")
                state.console_log.append_line(
                    f"{stamp}  CHANNEL SCAN {tick:05d} CORE {core:5.1f} CH {channel:5.1f}"
                )
            if tick % 60 == 0 and state.core_map is not None:
                state.core_map.cycle(17)

        def apply_snapshot(
            tick: int = tick,
            frame: list[float] = frame,
        ) -> None:
            if state.scope is not None:
                state.scope.set_values(frame)

            phase = tick * 0.11
            for index, bar in enumerate(state.lane_bars):
                base = lane_bases[index % len(lane_bases)]
                value = min(0.99, max(0.03, base + math.sin(phase + index) * 0.09))
                bar.set_value(value)
                if index < len(state.lane_values):
                    state.lane_values[index].set_value(f"{value:.0%}")

            for index, bar in enumerate(state.metric_bars):
                base = metric_bases[index % len(metric_bases)]
                value = min(0.99, max(0.03, base + math.sin(phase * 0.7 + index * 1.7) * 0.07))
                bar.set_value(value)
                if index < len(state.metric_values):
                    scale = metric_scales[index % len(metric_scales)]
                    text = metric_texts[index % len(metric_texts)]
                    state.metric_values[index].set_value(text.format(value * scale))

            for index, led in enumerate(state.leds):
                led.set_on((tick + index) % 6 < 4)

            if state.clock is not None:
                state.clock.set_value(f"{time.strftime('%H:%M:%S')} SHIFT A")

        try:
            app.call_soon_threadsafe(apply_stream)
            app.call_soon_threadsafe(
                apply_snapshot,
                coalesce_key="cathode.telemetry.snapshot",
            )
        except Exception:  # pragma: no cover - app shutting down
            break


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def build_app(
    style: str = "phosphor",
    *,
    tiles: int = 96,
    rows: int = 480,
) -> tuple[dg.App, dg.Window]:
    if style not in PALETTES:
        raise ValueError(f"style must be one of {sorted(PALETTES)}")

    state.palette = PALETTES[style]
    state.jobs = make_jobs(rows)
    state.lane_bars.clear()
    state.lane_values.clear()
    state.metric_bars.clear()
    state.metric_values.clear()
    state.leds.clear()
    state.tick = len(SAMPLES)

    app = dg.App(theme=theme_for(state.palette))
    state.app = app
    app.stylesheet(build_stylesheet(state.palette))
    return app, build_window(tiles=tiles, rows=rows)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run the CATHODE-7 vintage-terminal DragonGUI stress demo."
    )
    parser.add_argument(
        "--style",
        choices=sorted(PALETTES),
        default="phosphor",
        help="CRT phosphor palette to apply (default: phosphor).",
    )
    parser.add_argument(
        "--tiles",
        type=int,
        default=96,
        help="Channel tiles rendered on the load page (default: 96).",
    )
    parser.add_argument(
        "--rows",
        type=int,
        default=480,
        help="Rows generated for the job register table (default: 480).",
    )
    parser.add_argument(
        "--no-live",
        action="store_true",
        help="Disable the background telemetry worker.",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    app, window = build_app(args.style, tiles=max(1, args.tiles), rows=max(1, args.rows))

    if not args.no_live:
        threading.Thread(
            target=live_worker,
            args=(app,),
            name="cathode-telemetry",
            daemon=True,
        ).start()

    try:
        print(app.run(window))
    finally:
        state.stop.set()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
