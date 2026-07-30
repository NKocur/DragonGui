from __future__ import annotations

import json
import math
import os
from pathlib import Path
from pprint import pprint
import struct
import sys
import threading
import tempfile
import time
import zlib
from collections.abc import Mapping
from html import escape

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual demo guard
    raise SystemExit("all_features_v3_demo.py requires NumPy") from exc

try:
    import plotly.graph_objects as go
except ImportError:  # pragma: no cover - optional demo dependency
    go = None


POINT_ROWS = 125_000
TABLE_ROWS = 50_000
LINE_ROWS = 720
HISTOGRAM_ROWS = 8_000
STREAM_FRAME_COUNT = 24
GRID_GAP = 12
REPORT_DIR = Path(tempfile.gettempdir()) / "dragongui_all_features_v3_reports"
REPORT_OVERVIEW = REPORT_DIR / "plotly_style_sensor_report.html"
REPORT_DETAIL = REPORT_DIR / "plotly_style_failure_report.html"
REPORT_INLINE = REPORT_DIR / "plotly_style_inline_report.html"
REPORT_BACKEND_LABEL = "Real Plotly" if go is not None else "Self-contained fallback"
NODE_BINDING_TEMPLATES = dg.multi_agent_node_templates()
NODE_BINDING_NO_GRAPH = os.environ.get("DRAGONGUI_BINDING_PROBE_NO_GRAPH", "").strip().lower() in {"1", "true", "yes", "on"}
NODE_BINDING_ACTION_BUTTON_TARGETS = (
    ("v3-run-section-action-button", "Run Section Action Button"),
    ("v3-run-selection-button", "Run Selection Button"),
    ("v3-send-prompt-button", "Send Prompt Button"),
    ("v3-clear-logs-button", "Clear Logs Button"),
    ("v3-snapshot-button", "Snapshot Button"),
)
NODE_BINDING_ACTION_SLOT_TARGETS = (
    ("v3-action-slot-a", "Action Slot A"),
    ("v3-action-slot-b", "Action Slot B"),
    ("v3-action-slot-c", "Action Slot C"),
    ("v3-action-slot-d", "Action Slot D"),
)
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
    "gap": 8,
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
LINE_LAYOUT_STYLE = {
    "padding": 10,
    "gap": GRID_GAP,
    "align_items": "stretch",
    "flex_grow": 1,
    "flex_shrink": 1,
    "min_width": 0,
    "min_height": 0,
    "overflow_y": "hidden",
}
LINE_CONTROLS_PANEL_STYLE = {
    "width": 280,
    "padding": 10,
    "gap": 8,
    "font_size": 14,
    "line_height": "18px",
    "flex_grow": 0,
    "flex_shrink": 0,
    "align_self": "stretch",
    "min_height": 0,
    "overflow_y": "hidden",
}
LINE_CONTROLS_SCROLL_STYLE = {
    "padding_bottom": 26,
    "gap": 8,
    "font_size": 14,
    "line_height": "18px",
    "flex_grow": 1,
    "flex_shrink": 1,
    "min_height": 0,
}
LINE_STACK_STYLE = {
    "gap": 10,
    "width": "100%",
    "min_width": 0,
    "min_height": 0,
    "flex_grow": 1,
    "flex_shrink": 1,
    "align_self": "stretch",
}
HISTOGRAM_LAYOUT_STYLE = {
    "padding": 10,
    "gap": GRID_GAP,
    "align_items": "stretch",
    "flex_grow": 1,
    "flex_shrink": 1,
    "min_width": 0,
    "min_height": 0,
    "overflow_y": "hidden",
}
HISTOGRAM_CONTROLS_PANEL_STYLE = {
    "width": 280,
    "padding": 10,
    "gap": 8,
    "font_size": 14,
    "line_height": "18px",
    "flex_grow": 0,
    "flex_shrink": 0,
    "align_self": "stretch",
    "min_height": 0,
    "overflow_y": "hidden",
}
HISTOGRAM_CONTROLS_SCROLL_STYLE = {
    "padding_bottom": 26,
    "gap": 8,
    "font_size": 14,
    "line_height": "18px",
    "flex_grow": 1,
    "flex_shrink": 1,
    "min_height": 0,
}
HISTOGRAM_SCROLL_STYLE = {
    "flex_grow": 1,
    "flex_shrink": 1,
    "min_width": 0,
    "min_height": 0,
    "padding_right": 18,
    "padding_bottom": 22,
    "overflow_y": "auto",
}
DEBUG_MONITOR_STYLE = {
    "height": 540,
    "min_height": 360,
    "min_width": 0,
    "width": "100%",
    "flex_grow": 1,
    "flex_shrink": 1,
    "align_self": "stretch",
}
DEBUG_GRID_STYLE = {
    **GRID_STYLE,
    "align_items": "stretch",
    "min_width": 0,
    "min_height": 0,
}
V4_SCROLL_STYLE = {
    "width": "100%",
    "height": "100%",
    "min_height": 0,
    "flex_grow": 1,
    "flex_shrink": 1,
    "padding": 10,
    "padding_right": 18,
    "padding_bottom": 24,
    "overflow_y": "auto",
}
V4_CARD_STYLE = {
    **CARD_STYLE,
    "min_width": 0,
    "min_height": 0,
}
V4_CHART_STYLE = {
    "height": 220,
    "min_height": 180,
    "width": "100%",
}
V4_INSPECTOR_SPLITTER_STYLE = {
    "height": 330,
    "width": "100%",
}
V4_INSPECTOR_NAV_PANE_STYLE = {
    "gap": 6,
    "padding_right": 4,
    "min_width": 0,
}
V4_INSPECTOR_BODY_PANE_STYLE = {
    "gap": 10,
    "min_width": 0,
}
V4_INSPECTOR_SEPARATOR_STYLE = {
    "margin_top": 4,
    "margin_bottom": 4,
    "opacity": 0.55,
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


def make_plotly_style_report(title: str, accent: str, label: str, offset: float) -> str:
    points = [
        [idx, 48.0 + 10.0 * math.sin(idx * 0.17 + offset) + 3.5 * math.cos(idx * 0.43 - offset)]
        for idx in range(96)
    ]
    bars = [34 + 15 * math.sin(idx * 0.7 + offset) + 10 * math.cos(idx * 0.31) for idx in range(18)]
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{escape(title)}</title>
  <style>
    :root {{ color-scheme: dark; }}
    body {{
      margin: 0;
      background: #0b1020;
      color: #eef4ff;
      font-family: Segoe UI, Arial, sans-serif;
    }}
    main {{ padding: 18px; }}
    header {{
      display: flex;
      justify-content: space-between;
      align-items: start;
      gap: 18px;
      margin-bottom: 14px;
    }}
    h1 {{ margin: 0 0 5px; font-size: 22px; }}
    p {{ margin: 0; color: rgba(238, 244, 255, 0.66); }}
    .summary {{
      display: grid;
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 10px;
      margin-bottom: 14px;
    }}
    .metric {{
      border: 1px solid rgba(255,255,255,.13);
      border-radius: 7px;
      background: #111a2c;
      padding: 10px;
    }}
    .metric strong {{ display: block; font-size: 18px; color: white; }}
    .metric span {{ color: rgba(238,244,255,.62); font-size: 12px; }}
    .plotly-graph-div {{
      position: relative;
      height: 390px;
      border: 1px solid rgba(255,255,255,.14);
      border-radius: 8px;
      background: #0e1728;
      overflow: hidden;
      user-select: none;
    }}
    .modebar {{
      position: absolute;
      top: 10px;
      right: 10px;
      display: flex;
      gap: 6px;
      z-index: 3;
    }}
    .modebar button {{
      height: 30px;
      min-width: 36px;
      border: 1px solid rgba(255,255,255,.20);
      border-radius: 5px;
      background: rgba(17, 26, 44, .94);
      color: #edf3ff;
      cursor: pointer;
      font: inherit;
      font-size: 12px;
    }}
    .modebar button.active {{ border-color: {accent}; color: white; }}
    svg {{ width: 100%; height: 100%; display: block; }}
    .hoverlabel {{
      position: absolute;
      left: 14px;
      bottom: 12px;
      border: 1px solid rgba(255,255,255,.16);
      border-radius: 5px;
      background: rgba(3,7,18,.86);
      color: rgba(238,244,255,.82);
      padding: 6px 8px;
      font-size: 12px;
    }}
    .select-box {{
      display: none;
      position: absolute;
      border: 1px solid {accent};
      background: rgba(90,169,255,.12);
      pointer-events: none;
    }}
  </style>
</head>
<body>
  <main>
    <header>
      <div>
        <h1>{escape(title)}</h1>
        <p>Self-contained report fixture using Plotly-style DOM, modebar controls, hover, wheel zoom, and drag selection.</p>
      </div>
    </header>
    <section class="summary">
      <div class="metric"><strong>{label}</strong><span>report variant</span></div>
      <div class="metric"><strong>96</strong><span>trace samples</span></div>
      <div class="metric"><strong>18</strong><span>distribution bins</span></div>
    </section>
    <section id="graph" class="plotly-graph-div">
      <div class="modebar">
        <button class="active" data-mode="pan">Pan</button>
        <button data-mode="zoom">Zoom</button>
        <button data-mode="select">Box</button>
        <button data-mode="reset">Fit</button>
      </div>
      <svg id="plot" viewBox="0 0 840 390" aria-label="interactive report plot">
        <g stroke="rgba(255,255,255,.12)" stroke-width="1">
          <path d="M70 42V306M220 42V306M370 42V306M520 42V306M670 42V306M790 42V306"/>
          <path d="M70 306H790M70 240H790M70 174H790M70 108H790M70 42H790"/>
        </g>
        <g id="bars" fill="rgba(116,221,176,.34)"></g>
        <path id="trace" fill="none" stroke="{accent}" stroke-width="4" stroke-linecap="round"/>
        <g fill="rgba(238,244,255,.72)" font-size="13">
          <text x="70" y="342">0s</text>
          <text x="396" y="342">elapsed</text>
          <text x="752" y="342">96s</text>
          <text x="18" y="50">high</text>
          <text x="24" y="310">low</text>
        </g>
      </svg>
      <div id="hover" class="hoverlabel">mode: pan, point: --</div>
      <div id="selectBox" class="select-box"></div>
    </section>
  </main>
  <script>
    const traceData = {json.dumps(points, separators=(",", ":"))};
    const barData = {json.dumps(bars, separators=(",", ":"))};
    const graph = document.getElementById("graph");
    const plot = document.getElementById("plot");
    const trace = document.getElementById("trace");
    const bars = document.getElementById("bars");
    const hover = document.getElementById("hover");
    const selectBox = document.getElementById("selectBox");
    let mode = "pan";
    let scale = 1.0;
    let dragStart = null;
    function sx(x) {{ return 70 + x * (720 / 95); }}
    function sy(y) {{ return 306 - (y - 28) * (264 / 38) * scale; }}
    function draw() {{
      trace.setAttribute("d", traceData.map((p, i) =>
        (i === 0 ? "M" : "L") + sx(p[0]).toFixed(1) + " " + sy(p[1]).toFixed(1)
      ).join(" "));
      bars.innerHTML = barData.map((value, i) => {{
        const x = 80 + i * 38;
        const h = Math.max(8, value * 3.0);
        return '<rect x="' + x.toFixed(1) + '" y="' + (306 - h).toFixed(1) + '" width="23" height="' + h.toFixed(1) + '" rx="2"/>';
      }}).join("");
    }}
    draw();
    document.querySelectorAll(".modebar button").forEach(button => {{
      button.addEventListener("click", () => {{
        document.querySelectorAll(".modebar button").forEach(item => item.classList.remove("active"));
        button.classList.add("active");
        if (button.dataset.mode === "reset") {{
          mode = "pan";
          scale = 1.0;
          selectBox.style.display = "none";
          draw();
        }} else {{
          mode = button.dataset.mode;
        }}
        hover.textContent = "mode: " + mode + ", point: --";
      }});
    }});
    plot.addEventListener("wheel", event => {{
      event.preventDefault();
      scale = Math.max(.72, Math.min(1.55, scale + (event.deltaY < 0 ? .08 : -.08)));
      draw();
      hover.textContent = "mode: wheel zoom, scale: " + scale.toFixed(2);
    }}, {{ passive: false }});
    plot.addEventListener("pointermove", event => {{
      const rect = plot.getBoundingClientRect();
      const x = Math.max(0, Math.min(95, Math.round((event.clientX - rect.left) / rect.width * 95)));
      const point = traceData[x];
      if (point) hover.textContent = "mode: " + mode + ", point: " + point[0] + ", " + point[1].toFixed(2);
      if (dragStart && mode === "select") {{
        const px = event.clientX - rect.left;
        const py = event.clientY - rect.top;
        selectBox.style.display = "block";
        selectBox.style.left = Math.min(dragStart.x, px) + "px";
        selectBox.style.top = Math.min(dragStart.y, py) + "px";
        selectBox.style.width = Math.abs(px - dragStart.x) + "px";
        selectBox.style.height = Math.abs(py - dragStart.y) + "px";
      }}
    }});
    plot.addEventListener("pointerdown", event => {{
      const rect = plot.getBoundingClientRect();
      dragStart = {{ x: event.clientX - rect.left, y: event.clientY - rect.top }};
      plot.setPointerCapture(event.pointerId);
    }});
    plot.addEventListener("pointerup", event => {{
      dragStart = null;
      plot.releasePointerCapture(event.pointerId);
      if (mode !== "select") selectBox.style.display = "none";
    }});
  </script>
</body>
</html>
"""


def make_real_plotly_report(title: str, accent: str, label: str, offset: float) -> str:
    if go is None:
        return make_plotly_style_report(title, accent, label, offset)

    x_values = list(range(96))
    y_values = [
        48.0 + 10.0 * math.sin(idx * 0.17 + offset) + 3.5 * math.cos(idx * 0.43 - offset)
        for idx in x_values
    ]
    bar_x = list(range(18))
    bar_y = [34 + 15 * math.sin(idx * 0.7 + offset) + 10 * math.cos(idx * 0.31) for idx in bar_x]
    table_rows = [
        ("Mean", f"{sum(y_values) / len(y_values):.2f}", "stable"),
        ("Peak", f"{max(y_values):.2f}", "watch"),
        ("Low", f"{min(y_values):.2f}", "ok"),
        ("Last", f"{y_values[-1]:.2f}", "live"),
        ("Bins", str(len(bar_y)), "complete"),
    ]
    config = {
        "displayModeBar": True,
        "displaylogo": False,
        "responsive": True,
        "scrollZoom": True,
        "modeBarButtonsToRemove": ["lasso2d"],
    }

    trace_fig = go.Figure()
    trace_fig.add_trace(
        go.Scatter(
            x=x_values,
            y=y_values,
            mode="lines+markers",
            name=f"{label} signal",
            line={"color": accent, "width": 3},
            marker={"size": 5, "color": accent},
            hovertemplate="elapsed=%{x}s<br>value=%{y:.2f}<extra></extra>",
        )
    )
    trace_fig.update_layout(
        title="Sensor trace",
        template="plotly_dark",
        paper_bgcolor="#0b1020",
        plot_bgcolor="#0e1728",
        font={"family": "Segoe UI, Arial, sans-serif", "color": "#eef4ff"},
        margin={"l": 54, "r": 22, "t": 48, "b": 46},
        height=330,
        legend={"orientation": "h", "y": 1.12, "x": 0.72},
    )
    trace_fig.update_xaxes(title_text="elapsed time (s)", gridcolor="rgba(255,255,255,0.12)", zeroline=False)
    trace_fig.update_yaxes(title_text="sensor value", gridcolor="rgba(255,255,255,0.12)", zeroline=False)

    bar_fig = go.Figure()
    bar_fig.add_trace(
        go.Bar(
            x=bar_x,
            y=bar_y,
            name="bins",
            marker={"color": "rgba(116, 221, 176, 0.70)"},
            hovertemplate="bin=%{x}<br>count=%{y:.1f}<extra></extra>",
        )
    )
    bar_fig.update_layout(
        title="Distribution bins",
        template="plotly_dark",
        paper_bgcolor="#0b1020",
        plot_bgcolor="#0e1728",
        font={"family": "Segoe UI, Arial, sans-serif", "color": "#eef4ff"},
        margin={"l": 48, "r": 18, "t": 48, "b": 46},
        height=300,
        showlegend=False,
    )
    bar_fig.update_xaxes(title_text="bin", gridcolor="rgba(255,255,255,0.12)", zeroline=False)
    bar_fig.update_yaxes(title_text="count", gridcolor="rgba(255,255,255,0.12)", zeroline=False)

    table_fig = go.Figure()
    table_fig.add_trace(
        go.Table(
            header={
                "values": ["Metric", "Value", "Status"],
                "fill_color": "#18243a",
                "font": {"color": "#eef4ff", "size": 13},
                "align": "left",
                "height": 30,
            },
            cells={
                "values": [
                    [row[0] for row in table_rows],
                    [row[1] for row in table_rows],
                    [row[2] for row in table_rows],
                ],
                "fill_color": "#101827",
                "font": {"color": "#dce7f7", "size": 12},
                "align": "left",
                "height": 28,
            },
        )
    )
    table_fig.update_layout(
        title="Report table",
        template="plotly_dark",
        paper_bgcolor="#0b1020",
        margin={"l": 8, "r": 8, "t": 48, "b": 8},
        height=300,
    )

    trace_html = trace_fig.to_html(
        full_html=False,
        include_plotlyjs=True,
        config=config,
    )
    bar_html = bar_fig.to_html(
        full_html=False,
        include_plotlyjs=False,
        config=config,
    )
    table_html = table_fig.to_html(
        full_html=False,
        include_plotlyjs=False,
        config=config,
    )

    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{escape(title)}</title>
  <style>
    :root {{ color-scheme: dark; }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      background: #0b1020;
      color: #eef4ff;
      font-family: Segoe UI, Arial, sans-serif;
    }}
    main {{ padding: 18px; }}
    header {{
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: 18px;
      margin-bottom: 14px;
    }}
    h1 {{ margin: 0 0 5px; font-size: 22px; }}
    p {{ margin: 0; color: rgba(238, 244, 255, 0.66); }}
    .badge {{
      flex-shrink: 0;
      border: 1px solid {accent};
      border-radius: 999px;
      padding: 6px 10px;
      color: white;
      background: rgba(90, 169, 255, 0.14);
      font-size: 12px;
      font-weight: 700;
    }}
    .summary {{
      display: grid;
      grid-template-columns: repeat(4, minmax(0, 1fr));
      gap: 10px;
      margin-bottom: 14px;
    }}
    .metric {{
      border: 1px solid rgba(255,255,255,.13);
      border-radius: 7px;
      background: #111a2c;
      padding: 10px;
    }}
    .metric strong {{ display: block; font-size: 18px; color: white; }}
    .metric span {{ color: rgba(238,244,255,.62); font-size: 12px; }}
    .plot-grid {{
      display: grid;
      grid-template-columns: minmax(0, 1.15fr) minmax(320px, .85fr);
      gap: 14px;
      align-items: stretch;
    }}
    .plot-card {{
      min-width: 0;
      border: 1px solid rgba(255,255,255,.14);
      border-radius: 8px;
      background: #0e1728;
      overflow: hidden;
      padding: 8px;
    }}
    .plot-card.full {{ grid-column: 1 / -1; }}
    @media (max-width: 980px) {{
      .summary {{ grid-template-columns: repeat(2, minmax(0, 1fr)); }}
      .plot-grid {{ grid-template-columns: minmax(0, 1fr); }}
    }}
  </style>
</head>
<body>
  <main>
    <header>
      <div>
        <h1>{escape(title)}</h1>
        <p>Real Plotly HTML export with KPI cards, interactive charts, modebars, hover, wheel zoom, and a Plotly table.</p>
      </div>
      <div class="badge">Real Plotly</div>
    </header>
    <section class="summary">
      <div class="metric"><strong>{label}</strong><span>report variant</span></div>
      <div class="metric"><strong>{len(x_values)}</strong><span>trace samples</span></div>
      <div class="metric"><strong>{max(y_values):.1f}</strong><span>peak value</span></div>
      <div class="metric"><strong>{len(table_rows)}</strong><span>table rows</span></div>
    </section>
    <section class="plot-grid">
      <div class="plot-card full">{trace_html}</div>
      <div class="plot-card">{bar_html}</div>
      <div class="plot-card">{table_html}</div>
    </section>
  </main>
</body>
</html>
"""


def make_report_html(title: str, accent: str, label: str, offset: float) -> str:
    if go is not None:
        return make_real_plotly_report(title, accent, label, offset)
    return make_plotly_style_report(title, accent, label, offset)


def write_demo_reports() -> tuple[str, str, str]:
    REPORT_DIR.mkdir(parents=True, exist_ok=True)
    REPORT_OVERVIEW.write_text(
        make_report_html("Operations report", "#5aa9ff", "overview", 0.0),
        encoding="utf-8",
    )
    REPORT_DETAIL.write_text(
        make_report_html("Failure analysis report", "#f36b7f", "detail", 1.65),
        encoding="utf-8",
    )
    REPORT_INLINE.write_text(
        make_report_html("Inline report", "#74ddb0", "inline", 3.2),
        encoding="utf-8",
    )
    return str(REPORT_OVERVIEW), str(REPORT_DETAIL), str(REPORT_INLINE)


def inline_report_html() -> str:
    return make_report_html("Inline report", "#74ddb0", "inline", 3.2)


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


class LineFrame:
    columns = ("time", "temperature", "pressure", "vibration", "events")
    dtypes = ("float32", "float32", "float32", "float32", "float32")

    def __init__(self, rows: int = LINE_ROWS, offset: float = 0.0) -> None:
        self.shape = (rows, len(self.columns))
        t = np.linspace(offset, offset + 60.0, rows, dtype=np.float32)
        phase = t / np.float32(60.0)
        self.time = t
        self.temperature = (
            np.float32(68.0)
            + np.sin(phase * np.float32(math.tau * 2.0)) * np.float32(4.6)
            + np.sin(t * np.float32(1.9)) * np.float32(0.18)
        ).astype(np.float32)
        self.pressure = (
            np.float32(31.0)
            + np.cos(phase * np.float32(math.tau * 3.0)) * np.float32(2.2)
            + np.sin(t * np.float32(0.72)) * np.float32(0.45)
        ).astype(np.float32)
        self.vibration = (
            np.sin(t * np.float32(3.8)) * np.float32(0.7)
            + np.sin(t * np.float32(12.2)) * np.float32(0.18)
        ).astype(np.float32)
        self.events = np.zeros(rows, dtype=np.float32)
        for idx, height in ((112, 1.0), (276, 0.74), (431, 1.22), (606, 0.66)):
            center = np.float32(offset + idx / max(1, rows - 1) * 60.0)
            width = t - center
            self.events += np.exp(-(width * width) / np.float32(0.20)) * np.float32(height)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


class HistogramFrame:
    columns = ("latency_ms", "score", "revenue", "residual")
    dtypes = ("float32", "float32", "float32", "float32")

    def __init__(self, rows: int = HISTOGRAM_ROWS) -> None:
        self.shape = (rows, len(self.columns))
        rng = np.random.default_rng(303)
        fast = rng.normal(43.0, 8.0, int(rows * 0.70))
        slow = rng.normal(91.0, 20.0, rows - len(fast))
        self.latency_ms = np.clip(np.concatenate([fast, slow]), 0.0, 170.0).astype(np.float32)
        self.score = rng.beta(4.0, 1.7, rows).astype(np.float32)
        self.revenue = rng.lognormal(mean=3.3, sigma=0.6, size=rows).astype(np.float32)
        x = np.linspace(-math.tau, math.tau, rows, dtype=np.float32)
        self.residual = (
            np.sin(x * np.float32(1.65)) * np.float32(0.38)
            + rng.normal(0.0, 0.15, rows)
        ).astype(np.float32)

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
    --v4-surface: rgba(18, 27, 43, 0.96);
    --v4-surface-alt: rgba(255, 255, 255, 0.07);
    --v4-control-bg: rgba(255, 255, 255, 0.08);
    --v4-control-hover: rgba(90, 169, 255, 0.18);
    --v4-border: rgba(255, 255, 255, 0.16);
    --v4-border-strong: rgba(90, 169, 255, 0.52);
    --v4-text: rgba(245, 248, 255, 0.94);
    --v4-muted: rgba(245, 248, 255, 0.66);
    --v4-icon: rgba(245, 248, 255, 0.94);
    --v4-accent: #5aa9ff;
    --v4-accent-2: #74ddb0;
    --v4-danger: #f36b7f;
    --v4-plot-bg: rgba(4, 8, 18, 0.72);
    --v4-plot-grid: rgba(255, 255, 255, 0.14);
    --v4-menu-bg: #101827;
    --v4-menu-row: #172235;
    --v4-spinner-track: rgba(255, 255, 255, 0.16);
    --v4-spinner-arc: #74ddb0;
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

/* Refined sidebar nav buttons: slightly less-round corners, and an accent bar
   that floats with an even gap on both sides (bar inset 4px + item padding
   11px, so the 3px bar sits with ~4px of breathing room on each side). */
NavItem {
    border-radius: 5px;
}

NavItem::accent {
    padding: 4px;
}

NavItem::item {
    padding: 11px;
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
    border-radius: 999px;
    accent: #74ddb0;
}

MenuBar Menu {
    background: transparent;
    border-color: transparent;
    color: var(--v4-text);
}

MenuBar Menu:hover,
MenuBar Menu:open {
    background: var(--v4-control-hover);
    border-color: var(--v4-border-strong);
    color: var(--v4-text);
}

Menu::menu,
ContextMenu::menu {
    background: var(--v4-menu-bg);
    border: 1px solid var(--v4-border-strong);
    border-radius: var(--control-radius);
    box-shadow: 0 14px 34px rgba(0, 0, 0, 0.34);
}

Menu::item,
ContextMenu::item {
    background: var(--v4-menu-row);
    color: var(--v4-text);
}

Menu::item-hover,
ContextMenu::item-hover {
    background: var(--v4-control-hover);
    color: var(--v4-text);
}

Menu::item-disabled,
ContextMenu::item-disabled {
    color: var(--v4-muted);
}

SmallButton,
IconButton,
ArrowButton,
TextArea,
CodeEditor,
LogView,
DragNumber {
    background: var(--v4-control-bg);
    border: 1px solid var(--v4-border);
    border-radius: var(--control-radius);
    color: var(--v4-text);
    accent: var(--v4-accent);
}

IconButton::icon,
ArrowButton::icon {
    color: var(--v4-icon);
}

SmallButton:hover,
IconButton:hover,
ArrowButton:hover,
TextArea:hover,
CodeEditor:hover,
LogView:hover,
DragNumber:hover {
    background: var(--v4-control-hover);
    border-color: var(--v4-border-strong);
}

Selectable,
RadioButton,
TreeNode,
ToggleSwitch {
    color: var(--v4-text);
    accent: var(--v4-accent);
    border-radius: var(--control-radius);
}

Selectable:hover,
Selectable:selected,
RadioButton:hover,
TreeNode:hover,
TreeNode:selected {
    background: var(--v4-control-hover);
    border-color: var(--v4-border-strong);
}

TextInput.temporal-input.invalid {
    border-color: var(--v4-danger);
}

DragNumber::field,
NumberInput::field,
CodeEditor::field {
    background: var(--v4-control-bg);
    border-color: var(--v4-border);
}

DragNumber::grip,
CodeEditor::gutter,
CodeEditor::line-number {
    color: var(--v4-muted);
}

RangeSlider {
    accent: var(--v4-accent);
    track-color: var(--v4-border);
    thumb-color: var(--v4-text);
}

RangeSlider::track {
    height: 8px;
    background: var(--v4-border);
    border-radius: 999px;
}

RangeSlider::range {
    height: 8px;
    background: var(--v4-accent);
    border-radius: 999px;
}

RangeSlider::thumb-min,
RangeSlider::thumb-max {
    width: 18px;
    height: 18px;
    background: var(--v4-text);
    border: 2px solid var(--v4-accent);
    border-radius: 999px;
}

RangeSlider::label,
LoadingSpinner::label {
    color: var(--v4-muted);
}

ToggleSwitch::track {
    background: var(--v4-border);
    border-radius: 999px;
}

ToggleSwitch:checked::track {
    background: var(--v4-accent);
}

ToggleSwitch::thumb {
    background: var(--v4-text);
    border-radius: 999px;
}

Splitter::gutter {
    width: 4px;
    background: var(--v4-border-strong);
    border-radius: 999px;
}

DropTarget.drop-zone {
    background: var(--v4-surface-alt);
    border: 1px solid var(--v4-border);
    border-radius: var(--control-radius);
    color: var(--v4-muted);
}

DropTarget.drop-zone:selected {
    background: var(--v4-control-hover);
    border-color: var(--v4-border-strong);
    color: var(--v4-text);
}

Label.drop-zone-label {
    color: var(--v4-muted);
}

VLayout.selectable-list,
VLayout.radio-group,
VLayout.property-grid {
    color: var(--v4-text);
}

HLayout.toolbar {
    background: var(--v4-surface-alt);
    border: 1px solid var(--v4-border);
    border-radius: var(--control-radius);
    padding: 5px;
}

Separator.toolbar-separator {
    background: var(--v4-border);
    border-color: var(--v4-border);
}

HLayout.breadcrumbs {
    color: var(--v4-muted);
}

SmallButton.breadcrumb-item {
    background: transparent;
    border-color: transparent;
    color: var(--v4-accent);
}

Label.breadcrumb-current {
    color: var(--v4-text);
    font-weight: 750;
}

Label.breadcrumb-separator,
Label.breadcrumb-overflow {
    color: var(--v4-muted);
}

HLayout.search-box {
    background: var(--v4-control-bg);
    border: 1px solid var(--v4-border);
    border-radius: var(--control-radius);
    padding: 4px;
}

HLayout.search-box TextInput {
    background: transparent;
    border-color: transparent;
}

IconButton.search-box-icon,
IconButton.search-box-clear {
    background: transparent;
    border-color: transparent;
}

Modal.command-palette {
    background: var(--v4-surface);
    border-color: var(--v4-border-strong);
}

VLayout.command-palette-results {
    background: transparent;
}

Selectable.command-palette-row {
    background: transparent;
    border-color: transparent;
}

Selectable.command-palette-row:hover,
Selectable.command-palette-row:selected {
    background: var(--v4-control-hover);
    border-color: var(--v4-border-strong);
}

Label.command-palette-empty,
Label.property-label,
Label.property-value {
    color: var(--v4-muted);
}

HLayout.property-row {
    color: var(--v4-text);
}

Collapsible.property-section::header {
    background: var(--v4-surface-alt);
    border-color: var(--v4-border);
}

LoadingSpinner::track {
    background: var(--v4-spinner-track);
}

LoadingSpinner::arc {
    background: var(--v4-spinner-arc);
}

BarChart,
Heatmap,
PieChart {
    width: 100%;
    min-height: 180px;
    background: var(--v4-plot-bg);
    border: 1px solid var(--v4-border-strong);
    border-radius: var(--control-radius);
    color: var(--v4-accent);
}

BarChart::label,
BarChart::value-label,
Heatmap::label,
PieChart::label {
    color: var(--v4-text);
    font-size: 13px;
    font-weight: 650;
}

PieChart.dashboard {
    min-height: 300px;
    background:
        linear-gradient(145deg, rgba(255, 255, 255, 0.055), rgba(255, 255, 255, 0.018)),
        var(--v4-plot-bg);
    border-color: var(--v4-border-strong);
}

DataFrameTable {
    background: var(--v4-plot-bg);
    border-color: var(--v4-border-strong);
}

DataFrameTable::header {
    background: var(--v4-surface-alt);
    color: var(--v4-text);
}

DataFrameTable::row {
    color: var(--v4-text);
}

DataFrameTable::row-selected {
    background: var(--v4-control-hover);
}

DataFrameTable::grid-line,
DataFrameTable::scrollbar-track {
    background: var(--v4-border);
}

DataFrameTable::scrollbar-thumb {
    background: var(--v4-border-strong);
}

DataFrameTable,
Histogram,
LinePlot,
Scatter3D,
HtmlReport,
Image {
    border-color: rgba(90, 169, 255, 0.28);
    border-width: 1px;
}

LinePlot {
    width: 100%;
    min-height: 175px;
    flex-grow: 1;
    flex-shrink: 1;
    background: rgba(4, 8, 18, 0.72);
    border-radius: 12px;
    padding: 10px;
}

LinePlot.stream-plot {
    min-height: 205px;
}

Histogram {
    width: 100%;
    min-height: 220px;
    flex-grow: 1;
    flex-shrink: 1;
    background: rgba(4, 8, 18, 0.72);
    border-radius: 12px;
    padding: 10px;
}

Histogram.latency {
    color: #5aa9ff;
}

Histogram.density {
    color: #74ddb0;
}

Histogram.percent {
    color: #ffcc66;
}

Histogram.cumulative {
    color: #f36b7f;
}

Scatter3D {
    width: 100%;
    min-height: 500px;
    flex-grow: 1;
    background: rgba(4, 8, 18, 0.72);
    border-radius: 12px;
    scatter-point-style: circle;
}

HtmlReport {
    width: 100%;
    min-height: 420px;
    flex-grow: 1;
    flex-shrink: 1;
    background: rgba(4, 8, 18, 0.72);
    border-radius: 12px;
    padding: 10px;
}

HtmlReport.report-viewer {
    height: 540px;
    min-height: 420px;
    align-self: stretch;
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

HLayout.line-layout {
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    align-items: stretch;
    overflow-y: hidden;
}

Panel.line-controls {
    height: calc(100% - 8px);
    max-height: calc(100% - 8px);
    align-self: stretch;
    overflow-y: hidden;
    flex-grow: 0;
    flex-shrink: 0;
}

ScrollArea.line-control-scroll {
    flex-grow: 1;
    flex-shrink: 1;
    min-height: 0;
    overflow-y: auto;
}

VLayout.line-stack {
    height: calc(100% - 8px);
    min-height: 0;
    align-self: stretch;
}

HLayout.histogram-layout {
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    align-items: stretch;
    overflow-y: hidden;
}

Panel.histogram-controls {
    height: calc(100% - 8px);
    max-height: calc(100% - 8px);
    align-self: stretch;
    overflow-y: hidden;
    flex-grow: 0;
    flex-shrink: 0;
}

ScrollArea.histogram-control-scroll,
ScrollArea.histogram-scroll {
    flex-grow: 1;
    flex-shrink: 1;
    min-height: 0;
    overflow-y: auto;
}

GridLayout.histogram-grid {
    width: 100%;
    min-width: 0;
    align-items: stretch;
}
"""


CSS_PAPER = CSS_MIDNIGHT + """
:root {
    --v4-surface: #fffaf0;
    --v4-surface-alt: #fff2dc;
    --v4-control-bg: #ffffff;
    --v4-control-hover: #ffe0b8;
    --v4-border: #c9b99f;
    --v4-border-strong: #d06b2c;
    --v4-text: #18202a;
    --v4-muted: #735f4a;
    --v4-icon: #18202a;
    --v4-accent: #d06b2c;
    --v4-accent-2: #2c8c99;
    --v4-danger: #b8322b;
    --v4-plot-bg: #fffdf7;
    --v4-plot-grid: #dbc6a6;
    --v4-menu-bg: #fffaf0;
    --v4-menu-row: #ffffff;
    --v4-spinner-track: rgba(115, 95, 74, 0.24);
    --v4-spinner-arc: #2c8c99;
}

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
    background: transparent;
    border-color: transparent;
    color: #d06b2c;
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
Histogram,
LinePlot,
Scatter3D,
HtmlReport,
Image {
    border-color: #9f8970;
}

LinePlot {
    background: #fffdf7;
    border-color: #9f8970;
}

Histogram {
    background: #fffdf7;
    border-color: #9f8970;
}

HtmlReport {
    background: #fffdf7;
    border-color: #9f8970;
}
"""


CSS_NEON = CSS_MIDNIGHT + """
:root {
    --v4-surface: #0d1433;
    --v4-surface-alt: #101947;
    --v4-control-bg: #101947;
    --v4-control-hover: rgba(255, 54, 214, 0.22);
    --v4-border: #ff36d6;
    --v4-border-strong: #00e5ff;
    --v4-text: #ffffff;
    --v4-muted: #9eefff;
    --v4-icon: #ffffff;
    --v4-accent: #ff36d6;
    --v4-accent-2: #39ff88;
    --v4-danger: #ff5f9f;
    --v4-plot-bg: rgba(4, 8, 28, 0.86);
    --v4-plot-grid: #22306a;
    --v4-menu-bg: #090f26;
    --v4-menu-row: #101947;
    --v4-spinner-track: rgba(0, 229, 255, 0.22);
    --v4-spinner-arc: #39ff88;
}

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

LinePlot {
    background: rgba(4, 8, 28, 0.86);
    border-color: #00e5ff;
}

Histogram {
    background: rgba(4, 8, 28, 0.86);
    border-color: #00e5ff;
}

HtmlReport {
    background: rgba(4, 8, 28, 0.86);
    border-color: #00e5ff;
}
"""

CSS_TERMINAL = CSS_MIDNIGHT + """
:root {
    --v4-surface: #030712;
    --v4-surface-alt: rgba(16, 185, 129, 0.10);
    --v4-control-bg: #020617;
    --v4-control-hover: rgba(16, 185, 129, 0.18);
    --v4-border: #10b981;
    --v4-border-strong: #bef264;
    --v4-text: #d1fae5;
    --v4-muted: rgba(209, 250, 229, 0.68);
    --v4-icon: #bef264;
    --v4-accent: #34d399;
    --v4-accent-2: #bef264;
    --v4-danger: #f87171;
    --v4-plot-bg: #020617;
    --v4-plot-grid: rgba(16, 185, 129, 0.22);
    --v4-menu-bg: #020617;
    --v4-menu-row: #052e2b;
    --v4-spinner-track: rgba(16, 185, 129, 0.22);
    --v4-spinner-arc: #bef264;
}

Window {
    background: #030712;
    color: #d1fae5;
    font-family: "Consolas";
}

MenuBar,
StatusBar {
    background: #020617;
    border-color: #10b981;
    color: #d1fae5;
}

MenuBar Menu {
    background: transparent;
    border-color: transparent;
    color: #a7f3d0;
}

MenuBar Menu:hover,
MenuBar Menu:open {
    background: transparent;
    border-color: transparent;
    color: #bef264;
}

Sidebar {
    background: #020617;
    border-color: #10b981;
}

Panel,
Modal {
    background:
        linear-gradient(180deg, rgba(6, 78, 59, 0.22), rgba(3, 7, 18, 0.98)),
        #030712;
    border: 1px solid #10b981;
    border-radius: 2px;
    box-shadow: 0 0 0 1px rgba(16, 185, 129, 0.16), 0 18px 36px rgba(0, 0, 0, 0.38);
    color: #d1fae5;
    accent: #34d399;
}

Panel.highlight {
    border-color: #bef264;
}

Label.brand,
Label.stat-value {
    color: #6ee7b7;
}

Label.subtle,
Label.stat-label {
    color: rgba(209, 250, 229, 0.68);
}

Button,
Dropdown,
TextInput,
TextArea,
NumberInput {
    background: #020617;
    border-color: #10b981;
    border-radius: 2px;
    color: #d1fae5;
    accent: #34d399;
    font-family: "Consolas";
}

Button {
    background: #052e2b;
    color: #a7f3d0;
}

Button.primary {
    background: #10b981;
    border-color: #bef264;
    color: #022c22;
}

Button:hover,
Dropdown:hover,
TextInput:hover,
TextArea:hover,
NumberInput:hover {
    background: rgba(16, 185, 129, 0.12);
    border-color: #bef264;
}

Button:focus,
Dropdown:focus,
TextInput:focus,
TextArea:focus,
NumberInput:focus {
    outline: 2px solid rgba(190, 242, 100, 0.70);
    outline-offset: 2px;
}

NavItem,
Menu {
    accent: #bef264;
    color: #a7f3d0;
    border-radius: 2px;
}

Checkbox {
    accent: #bef264;
    color: #d1fae5;
}

Checkbox::row,
Dropdown::field,
NumberInput::field,
NumberInput::stepper {
    background: rgba(16, 185, 129, 0.10);
    border-radius: 2px;
}

Checkbox::box {
    border-color: #10b981;
    border-radius: 2px;
}

Checkbox::indicator {
    background: #bef264;
    border-radius: 1px;
}

Dropdown::chevron {
    color: #bef264;
}

Dropdown::menu {
    background: #020617;
    border-color: #10b981;
    border-radius: 2px;
}

Dropdown::item-hover {
    background: rgba(16, 185, 129, 0.18);
}

Dropdown::item-selected {
    background: rgba(190, 242, 100, 0.18);
    color: #ecfccb;
}

Slider {
    accent: #34d399;
    track-color: rgba(16, 185, 129, 0.22);
    thumb-color: #bef264;
}

Slider::track,
ProgressBar::track {
    background: rgba(16, 185, 129, 0.14);
    border: 1px solid rgba(16, 185, 129, 0.46);
    border-radius: 2px;
}

Slider::fill,
ProgressBar::fill {
    background: linear-gradient(90deg, #10b981, #bef264);
    border-radius: 2px;
}

Slider::thumb {
    width: 18px;
    height: 18px;
    background: #bef264;
    border: 2px solid #10b981;
    border-radius: 2px;
}

LED {
    background: #10b981;
    border-color: #bef264;
    border-radius: 2px;
}

LED.off {
    background: #064e3b;
    border-color: rgba(16, 185, 129, 0.45);
}

LED.stream,
LED.busy {
    background: #bef264;
    border-color: #34d399;
}

LED::dot {
    background: #34d399;
    border-color: #bef264;
    border-radius: 1px;
}

LED.stream::dot,
LED.busy::dot {
    background: #bef264;
}

LED::glow {
    opacity: 0.18;
    background: #34d399;
    border-radius: 2px;
}

LED::highlight {
    opacity: 0;
}

ProgressBar {
    background: #020617;
    border-color: #10b981;
    accent: #bef264;
}

Badge,
Tag {
    background: rgba(16, 185, 129, 0.16);
    border-color: rgba(190, 242, 100, 0.45);
    color: #d1fae5;
    border-radius: 2px;
}

DataFrameTable,
Histogram,
LinePlot,
Scatter3D,
HtmlReport,
Image {
    background: rgba(2, 6, 23, 0.92);
    border-color: #10b981;
    border-width: 1px;
    border-radius: 2px;
    color: #d1fae5;
}

Histogram.latency,
LinePlot {
    color: #34d399;
}

Histogram.density {
    color: #bef264;
}

Histogram.percent {
    color: #facc15;
}

Histogram.cumulative {
    color: #6ee7b7;
}

VLayout::scrollbar-track,
ScrollArea::scrollbar-track,
Panel::scrollbar-track {
    background: rgba(16, 185, 129, 0.10);
    border: 1px solid rgba(16, 185, 129, 0.46);
    border-radius: 2px;
}

VLayout::scrollbar-thumb,
ScrollArea::scrollbar-thumb,
Panel::scrollbar-thumb {
    background: linear-gradient(180deg, #34d399, #bef264);
    border: 1px solid rgba(190, 242, 100, 0.50);
    border-radius: 2px;
}
"""


CSS_THEMES = {
    "midnight": CSS_MIDNIGHT,
    "paper": CSS_PAPER,
    "neon": CSS_NEON,
    "terminal": CSS_TERMINAL,
}


app = dg.App(
    theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"),
    loading_screen=dg.LoadingScreen(
        title="Loading DragonGUI V3",
        message="Preparing plots, tables, reports, and controls...",
        background="#07111f",
        text="#f8fafc",
        accent="#5aa9ff",
        show_progress=True,
        min_duration_ms=350,
    ),
)
app.stylesheet(CSS_THEMES["midnight"])
stream_controller: dg.ScatterFrameStream | None = None
stream_build_thread: threading.Thread | None = None
line_stream_thread: threading.Thread | None = None
stats_thread: threading.Thread | None = None
stream_cancel = threading.Event()
line_stream_stop = threading.Event()
stats_stop = threading.Event()
state_lock = threading.Lock()
demo_state = {
    "mode": "lidar",
    "phase": 0.0,
    "theme": "midnight",
    "stream_interval_ms": 40.0,
    "stream_handoff": "direct",
    "stream_generation": 0,
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
    "page": os.environ.get("DRAGONGUI_DEMO_PAGE", "overview"),
    "line_stream_t": 60.0,
    "line_width": 2.0,
    "line_ticks": 5,
    "line_window": None,
    "histogram_ticks": 5,
}
initial_frame = DemoFrame(mode="lidar")
line_frame = LineFrame()
histogram_frame = HistogramFrame()
pie_segment_frame = type(
    "PieSegmentFrame",
    (),
    {
        "columns": ("segment", "revenue", "accounts"),
        "dtypes": ("object", "float32", "int32"),
        "shape": (9, 3),
        "segment": ["Enterprise", "Team", "Free", "Team", "Partner", "Enterprise", "Trial", "Free", "Team"],
        "revenue": [42.0, 18.0, 3.0, 22.0, 14.0, 48.0, 6.0, 4.0, 21.0],
        "accounts": [11, 7, 24, 8, 5, 10, 3, 18, 6],
        "__getitem__": lambda self, column: getattr(self, column),
    },
)()
PIE_DASHBOARD_COLORS = ["#2f6fb3", "#247a59", "#a97413", "#ad4058", "#6d4fb0"]
v4_scatter_frame = DemoFrame(mode="cloud", rows=12_000)
v4_table_frame = DemoFrame(mode="wave", rows=420)
v4_heatmap_matrix = (
    np.sin(np.linspace(0.0, math.tau * 1.8, 9, dtype=np.float32))[:, None] * 0.55
    + np.cos(np.linspace(0.0, math.tau * 1.4, 12, dtype=np.float32))[None, :] * 0.45
    + np.linspace(-0.25, 0.35, 12, dtype=np.float32)[None, :]
).astype(np.float32)
demo_image_path = make_demo_image()
report_overview_path, report_detail_path, report_inline_path = write_demo_reports()
stream_payload_cache: dict[str, list[tuple[float, dg.ScatterPayload]]] = {}
line_plot_top: dg.LinePlot | None = None
line_plot_mid: dg.LinePlot | None = None
line_plot_bottom: dg.LinePlot | None = None
html_report_view: dg.HtmlReport | None = None
html_report_status: dg.Label | None = None
histogram_latency: dg.Histogram | None = None
histogram_score: dg.Histogram | None = None
histogram_revenue: dg.Histogram | None = None
histogram_residual: dg.Histogram | None = None
line_width_label: dg.Label | None = None
line_tick_label: dg.Label | None = None
line_window_label: dg.Label | None = None
histogram_tick_label: dg.Label | None = None
binding_playground: NodeGraphBindingPlayground | None = None


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
    frame_work_ms_value = runtime.get("frame_work_ms_avg")
    frame_present_ms_value = runtime.get("frame_present_ms_avg")
    try:
        frame_ms = float(frame_ms_value)
    except (TypeError, ValueError):
        frame_ms = 0.0
    frame_fps = 1000.0 / frame_ms if frame_ms > 0.0 else 0.0
    observed_text = "--" if observed_fps is None else f"{observed_fps:.1f} fps"
    lod = scatter_metrics.get("lod", {})
    lod_active = bool(lod.get("active")) if isinstance(lod, dict) else False
    scalar_bar = scatter_metrics.get("scalar_bar", {})
    scalar_bar_colormap = (
        scalar_bar.get("colormap", "--") if isinstance(scalar_bar, dict) else "--"
    )
    commands = runtime.get("commands", {})
    recent_scatter_commands: list[str] = []
    if isinstance(commands, dict):
        recent = commands.get("recent", [])
        if isinstance(recent, list):
            for record in reversed(recent):
                if not isinstance(record, dict):
                    continue
                command = str(record.get("command", ""))
                target = str(record.get("target", ""))
                if target != scatter.id or command not in {
                    "SetScatterPointsPacked",
                    "SetScatterScalarBar",
                }:
                    continue
                detail = str(record.get("detail", ""))
                if "colormap" not in detail:
                    continue
                recent_scatter_commands.append(f"{command}: {detail}")
                if len(recent_scatter_commands) >= 3:
                    break
    with state_lock:
        handoff = str(demo_state.get("stream_handoff", "direct"))
    scatter_stats_summary.set_value(
        "\n".join(
            (
                f"Frame avg: {fmt_ms(frame_ms_value)} ({frame_fps:.1f} fps)",
                f"CPU work / present: {fmt_ms(frame_work_ms_value)} / {fmt_ms(frame_present_ms_value)}",
                f"Observed redraws: {observed_text} / {fmt_count(runtime.get('frames_rendered'))} frames",
                f"Stream handoff: {stream_handoff_label(handoff)}",
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
                "Colormaps: "
                f"widget {scatter.colormap}, "
                f"native bar {scalar_bar_colormap}, "
                f"native compact {scatter_metrics.get('primary_compact_colormap', '--')}",
                "Recent cmap cmds: "
                + (" | ".join(recent_scatter_commands) if recent_scatter_commands else "--"),
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


def set_html_report_status(message: str) -> None:
    if html_report_status is not None:
        html_report_status.set_value(message)
    set_status(message)


def show_report_overview() -> None:
    if html_report_view is not None:
        html_report_view.set_path(report_overview_path)
    set_html_report_status(f"{REPORT_BACKEND_LABEL}: {Path(report_overview_path).name}")


def show_report_detail() -> None:
    if html_report_view is not None:
        html_report_view.set_path(report_detail_path)
    set_html_report_status(f"{REPORT_BACKEND_LABEL}: {Path(report_detail_path).name}")


def show_report_inline() -> None:
    if html_report_view is not None:
        if go is not None:
            html_report_view.set_path(report_inline_path)
            set_html_report_status(f"{REPORT_BACKEND_LABEL}: {Path(report_inline_path).name}")
            return
        html_report_view.set_html(inline_report_html(), base_dir=REPORT_DIR)
    set_html_report_status(f"{REPORT_BACKEND_LABEL}: inline HTML")


def reload_html_report() -> None:
    if html_report_view is not None:
        html_report_view.reload()
    set_html_report_status("Report reloaded")


def open_html_report_external() -> None:
    if html_report_view is not None and html_report_view.open_external():
        set_html_report_status("Report opened externally")
    else:
        set_html_report_status("External report open unavailable")


def refresh_html_report_snapshot() -> None:
    def worker() -> None:
        try:
            snapshot = app.debug_snapshot(timeout_ms=500)
            gpu = snapshot.get("gpu", {})
            renderer = gpu.get("renderer", {}) if isinstance(gpu, dict) else {}
            reports = renderer.get("html_reports", {}) if isinstance(renderer, dict) else {}
            enabled = reports.get("enabled") if isinstance(reports, dict) else None
            instances = reports.get("instances") if isinstance(reports, dict) else {}
            count = len(instances) if isinstance(instances, dict) else 0
            recovered = reports.get("profile_recovered") if isinstance(reports, dict) else None
            detail = f"HtmlReport: enabled={enabled}, instances={count}, recovered={recovered}"
            app.call_soon_threadsafe(lambda text=detail: set_html_report_status(text))
        except RuntimeError:
            pass

    threading.Thread(target=worker, daemon=True).start()


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
    normalized_name = name.strip().lower()
    if stream_controller is not None:
        stream_controller.set_colormap(normalized_name)
    scatter.set_colormap(name)
    scatter.show_scalar_bar(True, colormap=scatter.colormap, title="z")
    with state_lock:
        stream_payload_cache.clear()
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
    normalized_colormap = str(colormap).strip().lower()
    cache_key = mode
    with state_lock:
        cached = stream_payload_cache.get(cache_key)
    if cached is not None:
        return [
            (
                phase,
                dg.ScatterPayload(
                    data=payload.data,
                    payload_format=payload.payload_format,
                    colormap=normalized_colormap,
                    point_count=payload.point_count,
                    pack_ms=payload.pack_ms,
                    axis_labels=payload.axis_labels,
                    bounds=payload.bounds,
                    hover_meta=payload.hover_meta,
                    frame_summary=payload.frame_summary,
                )
                if payload.payload_format == "xyz_f32_v0"
                else payload
            )
            for phase, payload in cached
        ]

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
                colormap=normalized_colormap,
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
    with state_lock:
        handoff = str(demo_state.get("stream_handoff", "direct"))
    label = stream_handoff_label(handoff)
    if metrics is None:
        set_status(f"Streaming {mode} ({label}): phase {phase:.2f}")
    else:
        set_status(
            f"Streaming {mode} ({label}): phase {phase:.2f}, submitted {metrics.submitted:,}"
        )


def current_stream_interval_ms() -> float:
    with state_lock:
        return float(demo_state["stream_interval_ms"])


def normalize_stream_handoff(value: str) -> str:
    text = str(value).strip().lower().replace("-", "_")
    if text in ("ui_callback", "callback"):
        return "callback"
    return "direct"


def stream_handoff_label(value: str) -> str:
    return "UI callback" if normalize_stream_handoff(value) == "callback" else "Direct"


def set_stream_handoff(value: str) -> None:
    handoff = normalize_stream_handoff(value)
    was_running = stream_controller is not None and stream_controller.running
    with state_lock:
        previous = str(demo_state.get("stream_handoff", "direct"))
        demo_state["stream_handoff"] = handoff
    if handoff == normalize_stream_handoff(previous):
        set_status(f"Scatter stream handoff: {stream_handoff_label(handoff)}")
        return
    if was_running:
        stop_stream()
        start_stream()
    set_status(f"Scatter stream handoff: {stream_handoff_label(handoff)}")


def start_stream() -> None:
    global stream_controller, stream_build_thread
    if stream_controller is not None and stream_controller.running:
        return
    if stream_build_thread is not None and stream_build_thread.is_alive():
        return
    stream_cancel.clear()
    with state_lock:
        generation = int(demo_state["stream_generation"])

    def worker() -> None:
        global stream_build_thread
        with state_lock:
            mode = str(demo_state["mode"])
            handoff = str(demo_state.get("stream_handoff", "direct"))
            colormap = scatter.colormap
        try:
            payloads = stream_payloads_for_mode(mode, colormap)
        except RuntimeError:
            return
        with state_lock:
            current_generation = int(demo_state["stream_generation"])
        if stream_cancel.is_set() or current_generation != generation:
            return

        def launch() -> None:
            global stream_controller
            with state_lock:
                current_generation = int(demo_state["stream_generation"])
            if stream_cancel.is_set() or current_generation != generation:
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
                handoff=handoff,
            )
            stream_controller.set_colormap(scatter.colormap)
            stream_controller.start()
            set_status(f"Scatter stream started ({stream_handoff_label(handoff)})")

        try:
            app.call_soon_threadsafe(launch)
        except RuntimeError:
            return
        finally:
            if threading.current_thread() is stream_build_thread:
                stream_build_thread = None

    stream_build_thread = threading.Thread(target=worker, daemon=True)
    stream_build_thread.start()
    set_status("Preparing scatter stream")


def stop_stream() -> None:
    global stream_controller, stream_build_thread
    with state_lock:
        demo_state["stream_generation"] = int(demo_state["stream_generation"]) + 1
    stream_cancel.set()
    if stream_controller is not None:
        stream_controller.stop(timeout=0.25)
        stream_controller = None
    if stream_build_thread is not None and stream_build_thread is not threading.current_thread():
        stream_build_thread.join(timeout=0.25)
        if not stream_build_thread.is_alive():
            stream_build_thread = None
    set_status("Scatter stream stopped")


def update_all_line_plots(callback) -> None:
    for plot in (line_plot_top, line_plot_mid, line_plot_bottom):
        if plot is not None:
            callback(plot)


def update_all_histograms(callback) -> None:
    for hist in (histogram_latency, histogram_score, histogram_revenue, histogram_residual):
        if hist is not None:
            callback(hist)


def set_line_width(value: float) -> None:
    width = max(0.5, float(value))
    demo_state["line_width"] = width
    if line_width_label is not None:
        line_width_label.set_value(f"Line width: {width:.1f}px")
    update_all_line_plots(lambda plot: plot.set_line_width(width))
    set_status(f"Line width: {width:.1f}px")


def set_line_tick_count(value: float) -> None:
    count = max(2, min(9, int(round(float(value)))))
    demo_state["line_ticks"] = count
    if line_tick_label is not None:
        line_tick_label.set_value(f"Ticks: {count}")
    update_all_line_plots(lambda plot: plot.set_tick_count(count))
    set_status(f"Line ticks: {count}")


def set_line_window(seconds: float | None) -> None:
    demo_state["line_window"] = seconds
    if line_window_label is not None:
        line_window_label.set_value(
            "Window: full history" if seconds is None else f"Window: latest {seconds:g}s"
        )
    update_all_line_plots(lambda plot: plot.set_window_size(seconds))
    set_status("Line window: full history" if seconds is None else f"Line window: {seconds:g}s")


def set_line_grid(enabled: bool) -> None:
    update_all_line_plots(lambda plot: plot.set_grid_visible(bool(enabled)))
    set_status(f"Line grid: {enabled}")


def set_line_axes(enabled: bool) -> None:
    update_all_line_plots(lambda plot: plot.set_axes_visible(bool(enabled)))
    set_status(f"Line axes: {enabled}")


def set_line_ticks(enabled: bool) -> None:
    update_all_line_plots(lambda plot: plot.set_ticks_visible(bool(enabled)))
    set_status(f"Line tick labels: {enabled}")


def set_line_toolbar(enabled: bool) -> None:
    update_all_line_plots(lambda plot: plot.set_toolbar_visible(bool(enabled)))
    set_status(f"Line toolbar: {enabled}")


def set_line_legend(enabled: bool) -> None:
    update_all_line_plots(lambda plot: plot.set_legend_visible(bool(enabled)))
    set_status(f"Line legend: {enabled}")


def fit_line_plots() -> None:
    update_all_line_plots(lambda plot: plot.fit())
    if line_window_label is not None:
        line_window_label.set_value("Window: full history")
    demo_state["line_window"] = None
    set_status("Line plots fit to data")


def reset_line_plots() -> None:
    demo_state["line_stream_t"] = 60.0
    if line_plot_top is None or line_plot_mid is None or line_plot_bottom is None:
        return
    line_plot_top.set_data(
        line_frame,
        x="time",
        y=("temperature", "pressure"),
        labels=("Temperature", "Pressure"),
        colors=("#5aa9ff", "#74ddb0"),
        line_styles=("solid", "dashed"),
    )
    line_plot_mid.set_data(
        line_frame,
        x="time",
        y=("vibration", "events"),
        labels=("Vibration", "Events"),
        colors=("#ffb45c", "#f36b7f"),
        line_styles=("dotted", "dashdot"),
    )
    line_plot_bottom.set_data(
        line_frame,
        x="time",
        y="temperature",
        label="Live Temperature",
        color="#5aa9ff",
        line_style="solid",
    )
    update_all_line_plots(lambda plot: plot.set_line_width(float(demo_state["line_width"])))
    update_all_line_plots(lambda plot: plot.set_tick_count(int(demo_state["line_ticks"])))
    if demo_state["line_window"] is not None:
        update_all_line_plots(lambda plot: plot.set_window_size(float(demo_state["line_window"])))
    set_status("Line plots reset")


def generated_line_batch(samples: int = 28) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    start = float(demo_state["line_stream_t"])
    end = start + 1.35
    t = np.linspace(start, end, samples, dtype=np.float32)
    demo_state["line_stream_t"] = end
    phase = t / np.float32(60.0)
    temperature = (
        np.float32(68.0)
        + np.sin(phase * np.float32(math.tau * 2.0)) * np.float32(4.6)
        + np.sin(t * np.float32(2.7)) * np.float32(0.35)
    ).astype(np.float32)
    pressure = (
        np.float32(31.0)
        + np.cos(phase * np.float32(math.tau * 3.0)) * np.float32(2.2)
        + np.sin(t * np.float32(1.1)) * np.float32(0.55)
    ).astype(np.float32)
    vibration = (
        np.sin(t * np.float32(3.8)) * np.float32(0.7)
        + np.sin(t * np.float32(12.2)) * np.float32(0.22)
    ).astype(np.float32)
    center = np.float32(start + 0.72)
    events = np.exp(-((t - center) ** 2) / np.float32(0.08)) * np.float32(1.15)
    return t, temperature, pressure, vibration, events.astype(np.float32)


def append_line_batch() -> None:
    if line_plot_top is None or line_plot_mid is None or line_plot_bottom is None:
        return
    t, temperature, pressure, vibration, events = generated_line_batch()
    line_plot_top.append_points(t, temperature, series="Temperature")
    line_plot_top.append_points(t, pressure, series="Pressure")
    line_plot_mid.append_points(t, vibration, series="Vibration")
    line_plot_mid.append_points(t, events, series="Events")
    line_plot_bottom.append_points(t, temperature, series="Live Temperature")
    set_status(f"Line stream appended through t={float(t[-1]):.1f}s")


def start_line_stream() -> None:
    global line_stream_thread
    if line_stream_thread is not None and line_stream_thread.is_alive():
        set_status("Line stream already running")
        return
    line_stream_stop.clear()

    def worker() -> None:
        while not line_stream_stop.wait(0.12):
            try:
                app.call_soon_threadsafe(append_line_batch)
            except RuntimeError:
                break

    line_stream_thread = threading.Thread(target=worker, daemon=True)
    line_stream_thread.start()
    set_status("Line stream started")


def stop_line_stream() -> None:
    line_stream_stop.set()
    set_status("Line stream stopped")


def set_histogram_tick_count(value: float) -> None:
    count = max(2, min(9, int(round(float(value)))))
    demo_state["histogram_ticks"] = count
    if histogram_tick_label is not None:
        histogram_tick_label.set_value(f"Ticks: {count}")
    update_all_histograms(lambda hist: hist.set_tick_count(count))
    set_status(f"Histogram ticks: {count}")


def set_histogram_grid(enabled: bool) -> None:
    update_all_histograms(lambda hist: hist.set_grid_visible(bool(enabled)))
    set_status(f"Histogram grid: {enabled}")


def set_histogram_axes(enabled: bool) -> None:
    update_all_histograms(lambda hist: hist.set_axes_visible(bool(enabled)))
    set_status(f"Histogram axes: {enabled}")


def set_histogram_ticks(enabled: bool) -> None:
    update_all_histograms(lambda hist: hist.set_ticks_visible(bool(enabled)))
    set_status(f"Histogram tick labels: {enabled}")


def set_histogram_toolbar(enabled: bool) -> None:
    update_all_histograms(lambda hist: hist.set_toolbar_visible(bool(enabled)))
    set_status(f"Histogram toolbar: {enabled}")


def fit_histograms() -> None:
    update_all_histograms(lambda hist: hist.fit())
    set_status("Histograms fit to data")


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



class NodeGraphBindingPlayground:
    def __init__(self, host_window: dg.Window) -> None:
        self.host_window = host_window
        self.graph: dg.NodeGraph | None = None
        self.runtime_status_label: dg.Label | None = None
        self.target_status_label: dg.Label | None = None
        self.selected_section_id: str | None = None
        self.prompt_input: dg.TextInput | None = None
        self.review_input: dg.TextInput | None = None
        self.scratch_input: dg.TextInput | None = None
        self.main_output_log: dg.LogView | None = None
        self.review_output_log: dg.LogView | None = None
        self.message_log: dg.LogView | None = None
        self.event_log: dg.LogView | None = None
        self.playground_terminal: dg.Terminal | None = None
        self.action_slot_buttons: dict[str, dg.Button] = {}

    def log(self, line: object = "") -> None:
        if self.event_log is not None:
            self.event_log.append_line(str(line))

    def mark(self, text: str) -> None:
        self.log(text)
        try:
            set_status(f"Node binding: {text}")
        except Exception:
            pass

    def on_tab_change(self, value: str) -> None:
        self.log(f"tab changed: {value}")
        self.refresh_target_status()

    def refresh_counts(self) -> None:
        return

    def refresh_selection(self) -> None:
        return

    def refresh_runtime_status(self) -> None:
        if self.graph is None or self.runtime_status_label is None:
            return
        self.runtime_status_label.set_value(self.graph.managed_runtime_status_text())

    def refresh_target_status(self) -> None:
        if self.graph is None or self.target_status_label is None:
            return
        self.target_status_label.set_value(
            f"Discovered Actions: {len(self.graph.action_target_ids())} | "
            f"Widgets: {len(self.graph.widget_target_ids())}"
        )

    def force_refresh_targets(self) -> None:
        if self.graph is None:
            return
        self.graph.refresh_binding_targets_from_host(self.host_window)
        self.refresh_target_status()
        self.log(f"refreshed action targets: {', '.join(self.graph.action_target_ids())}")
        self.mark("binding targets refreshed")

    def on_select(self, node_id: str) -> None:
        self.selected_section_id = None
        self.refresh_selection()
        self.mark(f"selected node {node_id}")

    def on_move(self, node_id: str, x: float, y: float) -> None:
        self.log(f"Moved {node_id}: {x:.0f}, {y:.0f}")

    def on_graph_event(self, event: dict[str, object]) -> None:
        event_name = str(event.get("event", "event"))
        if event_name in {
            "node_created",
            "node_duplicated",
            "node_deleted",
            "edge_created",
            "edge_deleted",
            "section_created",
            "section_deleted",
            "property_editor_saved",
            "graph_changed",
        }:
            self.refresh_counts()
        if event_name == "section_selected":
            self.selected_section_id = str(event.get("section") or "") or None
        elif event_name in {"node_selected", "selection_cleared"}:
            self.selected_section_id = None
        if event_name in {"node_selected", "section_selected", "selection_cleared"}:
            self.refresh_selection()
        if event_name == "editor_action":
            action = str(event.get("action", ""))
            if action == "run_node":
                self.run_selected_node_runtime()
            elif action == "run_section":
                self.run_selected_section_runtime()
            elif action == "cleanup_runtime":
                self.cleanup_runtime()
            elif action == "refresh_targets":
                self.force_refresh_targets()
            elif action == "snapshot":
                self.snapshot_graph()
            elif action == "reset_graph":
                self.reset_blank_graph()
            else:
                self.mark(f"editor action ignored: {action}")
        if event_name not in {"viewport_changed", "node_moved"}:
            self.log(json.dumps(event, sort_keys=True))

    def binding_action(self, action_id: str, command: str) -> object:
        message = f"action target fired: {action_id} command={command}"
        self.mark(message)
        if self.message_log is not None:
            self.message_log.append_line(message)
        return {"action_id": action_id, "command": command}

    def run_selected_node_runtime(self) -> None:
        if self.graph is None:
            return
        if not self.graph.selected_node:
            self.mark("select a node before running it")
            return
        try:
            event = self.graph.run_node_runtime(self.graph.selected_node)
        except Exception as exc:
            self.mark(f"node runtime failed: {exc}")
            return
        self.log(f"node {self.graph.selected_node} runtime event: {event.event}")
        self.refresh_runtime_status()
        self.mark(f"node {self.graph.selected_node} ran with managed runtime")

    def run_selected_section_runtime(self) -> None:
        if self.graph is None:
            return
        if not self.selected_section_id:
            self.mark("select a section before running its action")
            return
        try:
            result = self.graph.run_section_runtime(self.selected_section_id)
        except Exception as exc:
            self.mark(f"section action failed: {exc}")
            return
        self.log(f"section {self.selected_section_id} runtime event: {result.event}")
        self.refresh_runtime_status()
        self.mark(f"section {self.selected_section_id} ran with managed runtime")

    def section_action_config(self, section: dg.NodeGraphSection) -> tuple[str, str]:
        data = section.data or {}
        if not isinstance(data, Mapping):
            return "", "run"
        config = data.get("config")
        source = config if isinstance(config, Mapping) else data
        action_id = str(source.get("action_id", "") or "").strip()
        command = str(source.get("section_command", "") or "run").strip() or "run"
        return action_id, command

    def sections_for_action_target(self, action_id: str) -> tuple[dg.NodeGraphSection, ...]:
        if self.graph is None:
            return ()
        target_id = str(action_id or "").strip()
        return tuple(
            section
            for section in self.graph.sections
            if self.section_action_config(section)[0] == target_id
        )

    def log_section_runtime_diagnostic(self, section: dg.NodeGraphSection) -> None:
        if self.graph is None:
            return
        try:
            node_ids = self.graph.section_nodes(section.id)
        except Exception as exc:
            node_ids = ()
            self.log(f"section {section.id} node lookup failed: {exc}")
        binding = self.graph.runtime_binding()
        section_binding = binding.section_binding(section.id)
        terminal_nodes: list[dict[str, object]] = []
        for node in self.graph.nodes:
            data = node.data or {}
            if not isinstance(data, Mapping):
                continue
            if str(data.get("node_type", data.get("template_id", ""))) != "terminal":
                continue
            config = data.get("config") if isinstance(data.get("config"), Mapping) else {}
            terminal_nodes.append(
                {
                    "id": node.id,
                    "in_section": node.id in node_ids,
                    "inputs": [port.id for port in node.inputs],
                    "session_id": str(config.get("session_id", "")),
                    "terminal_widget_id": str(config.get("terminal_widget_id", "")),
                }
            )
        self.log(
            "section diagnostic "
            + json.dumps(
                {
                    "section": section.id,
                    "section_nodes": list(node_ids),
                    "binding_nodes": list(section_binding.node_ids) if section_binding else [],
                    "terminal_nodes": terminal_nodes,
                    "widget_targets": list(self.graph.widget_target_ids()),
                },
                sort_keys=True,
            )
        )

    def run_action_slot(self, action_id: str, command: str | None = None) -> object:
        if self.graph is None:
            return {"action_id": action_id, "ran": 0}
        sections = self.sections_for_action_target(action_id)
        if not sections:
            self.mark(f"no sections assigned to {action_id}")
            return {"action_id": action_id, "ran": 0}
        events: list[str] = []
        for section in sections:
            _, section_command = self.section_action_config(section)
            command_s = str(command or section_command or "run").strip() or "run"
            self.log_section_runtime_diagnostic(section)
            try:
                event = self.graph.run_section_runtime(section.id, command_s)
            except Exception as exc:
                self.log(f"action slot {action_id} section {section.id} failed: {exc}")
                continue
            events.append(event.event)
            self.log(
                "section result "
                + json.dumps(
                    {
                        "action_id": action_id,
                        "section": section.id,
                        "command": command_s,
                        "event": event.event,
                        "data": event.data or {},
                    },
                    sort_keys=True,
                )
            )
            self.log_runtime_tail()
        self.refresh_runtime_status()
        self.mark(f"{action_id} ran {len(events)} assigned section(s)")
        return {"action_id": action_id, "ran": len(events), "events": events}

    def action_slot_callback(self, action_id: str, command: str) -> object:
        return self.run_action_slot(action_id, command)

    def cleanup_runtime(self) -> None:
        if self.graph is None:
            return
        result = self.graph.cleanup_managed_runtime()
        self.log(f"managed runtime cleanup: {json.dumps(result, sort_keys=True)}")
        self.refresh_runtime_status()
        self.mark("managed runtime cleaned up")

    def append_sample_outputs(self) -> None:
        if self.main_output_log is not None:
            self.main_output_log.append_line("Main output sample: hello from the playground.")
        if self.review_output_log is not None:
            self.review_output_log.append_line("Reviewer output sample: this would be agent/reviewer text.")
        if self.message_log is not None:
            self.message_log.append_line("Parsed message sample: @type report")
        self.mark("sample output appended")

    def send_direct_terminal_test(self) -> None:
        if self.playground_terminal is None:
            self.mark("direct terminal test skipped: no terminal widget")
            return
        delivered = self.playground_terminal.send_line("echo direct terminal widget test")
        self.log(f"direct terminal widget send delivered={delivered}")
        self.mark(f"direct terminal widget send delivered={delivered}")

    def clear_logs(self) -> None:
        for target in (self.main_output_log, self.review_output_log, self.message_log, self.event_log):
            if target is not None:
                target.clear()
        self.mark("logs cleared")

    def dump_bindings(self, *, announce: bool = True) -> None:
        if self.graph is None:
            return
        self.graph.refresh_binding_targets_from_host(self.host_window)
        payload = {
            "binding_targets": self.graph.binding_target_ids(),
            "widget_targets": self.graph.widget_target_ids(),
            "action_targets": self.graph.action_target_ids(),
        }
        self.log(json.dumps(payload, sort_keys=True))
        if announce:
            self.mark("binding registry dumped to event log")

    def log_runtime_tail(self) -> None:
        if self.graph is None or self.graph.managed_runtime is None:
            return
        events = self.graph.managed_runtime.snapshot().get("events", [])
        tail = events[-6:] if isinstance(events, list) else []
        for event in tail:
            self.log(f"runtime event {json.dumps(event, sort_keys=True)}")

    def snapshot_graph(self) -> None:
        if self.graph is None:
            return
        data = self.graph.to_graph_data()
        self.log(
            json.dumps(
                {
                    "nodes": len(data.get("nodes", [])),
                    "edges": len(data.get("edges", [])),
                    "sections": len(data.get("sections", [])),
                    "viewport": data.get("viewport"),
                },
                sort_keys=True,
            )
        )
        self.mark("graph snapshot written to event log")

    def reset_blank_graph(self) -> None:
        if self.graph is None:
            return
        self.graph.set_nodes([])
        self.graph.set_edges([])
        self.graph.set_sections([])
        self.selected_section_id = None
        self.refresh_counts()
        self.refresh_selection()
        self.refresh_runtime_status()
        self.mark("graph cleared")

    def register_binding_targets(self) -> None:
        if self.graph is None:
            return
        formats = ("text", "json", "repr", "message_body", "raw")
        if self.prompt_input is not None:
            self.graph.register_binding_target(
                "v3-prompt-input",
                label="Prompt Input",
                target_type="text_input",
                widget_type="text_input",
                widget=self.prompt_input,
                supported_update_modes=("set",),
                default_update_mode="set",
                supported_port_profiles=("text", "terminal_input"),
                default_port_profile="text",
                supported_formats=formats,
            )
        if self.review_input is not None:
            self.graph.register_binding_target(
                "v3-review-input",
                label="Reviewer Prompt Input",
                target_type="text_input",
                widget_type="text_input",
                widget=self.review_input,
                supported_update_modes=("set",),
                default_update_mode="set",
                supported_port_profiles=("text", "message"),
                default_port_profile="text",
                supported_formats=formats,
            )
        if self.scratch_input is not None:
            self.graph.register_binding_target(
                "v3-scratch-input",
                label="Scratch Text Input",
                target_type="text_input",
                widget_type="text_input",
                widget=self.scratch_input,
                supported_update_modes=("set",),
                default_update_mode="set",
                supported_port_profiles=("text", "json"),
                default_port_profile="text",
                supported_formats=formats,
            )
        for target_id, label, widget, profiles in (
            ("v3-main-output-log", "Main Output Log", self.main_output_log, ("terminal_output", "text", "json", "message")),
            ("v3-review-output-log", "Reviewer Output Log", self.review_output_log, ("terminal_output", "text", "json", "message")),
            ("v3-message-log", "Parsed Message Log", self.message_log, ("message", "json", "text", "event")),
            ("v3-event-log", "Event Log", self.event_log, ("event", "json", "text", "message", "terminal_output")),
        ):
            if widget is not None:
                self.graph.register_binding_target(
                    target_id,
                    label=label,
                    target_type="log",
                    widget_type="log_view",
                    widget=widget,
                    supported_update_modes=("append", "set"),
                    default_update_mode="append",
                    supported_port_profiles=profiles,
                    default_port_profile=profiles[0],
                    supported_formats=formats,
                )
        if self.playground_terminal is not None:
            self.graph.register_binding_target(
                "v3-playground-terminal",
                label="Playground Terminal",
                target_type="terminal",
                widget_type="terminal",
                widget=self.playground_terminal,
                supported_update_modes=("append", "set"),
                default_update_mode="append",
                supported_port_profiles=("terminal_output", "text"),
                default_port_profile="terminal_output",
                supported_formats=("terminal_text", "text", "repr"),
                data={"runtime_object_id": "v3-playground-terminal"},
            )
        for action_id, label in NODE_BINDING_ACTION_BUTTON_TARGETS:
            self.graph.register_binding_target(
                action_id,
                label=label,
                target_type="button",
                action_type="button",
                callback=self.binding_action,
                supported_commands=("run", "stop", "reset", "replay"),
                default_command="run",
            )
        for action_id, fallback_label in NODE_BINDING_ACTION_SLOT_TARGETS:
            button = self.action_slot_buttons.get(action_id)
            self.graph.register_binding_target(
                action_id,
                label=button.text if button is not None else fallback_label,
                target_type="button",
                action_type="button",
                callback=self.action_slot_callback,
                supported_commands=("run", "stop", "reset", "replay"),
                default_command="run",
            )
        self.dump_bindings(announce=False)
        self.refresh_runtime_status()

    def build(self) -> None:
        page_style = {"width": "100%", "height": "100%", "min_width": 0, "min_height": 0}
        panel_style = {
            **CARD_STYLE,
            "width": "100%",
            "height": "100%",
            "min_width": 0,
            "min_height": 0,
            "align_self": "stretch",
            "overflow_y": "auto",
        }
        log_style = {
            "width": "100%",
            "flex_shrink": 0,
            "font_family": "Consolas",
            "font_size": 12,
            "line_height": "17px",
            "padding_bottom": 12,
            "background": "#070a0f",
            "border_color": "#263347",
            "border_radius": 6,
        }
        with dg.Tabs(
            value="objects",
            id="v3-binding-playground-tabs",
            on_change=self.on_tab_change,
            style=page_style,
        ):
            with dg.Tab("Node Editor", value="editor", style=page_style):
                with dg.Panel("Blank Node Editor", style={**panel_style, "overflow_y": "hidden"}):
                    if NODE_BINDING_NO_GRAPH:
                        dg.Label("NodeGraph disabled for layout isolation", class_="subtle")
                        dg.Button("Native placeholder button")
                        dg.LogView(
                            ["No HtmlReport/WebView was created in this run."],
                            rows=6,
                            follow=True,
                            wrap=True,
                            style=log_style,
                        )
                    else:
                        self.graph = dg.NodeGraph(
                            [],
                            [],
                            sections=[],
                            templates=NODE_BINDING_TEMPLATES,
                            on_graph_event=self.on_graph_event,
                            on_node_select=self.on_select,
                            on_node_move=self.on_move,
                            enable_zoom=True,
                            show_port_labels=True,
                            show_subtitles=True,
                            show_editor_chrome=True,
                            editor_title="Binding Playground",
                            editor_actions=(
                                {"id": "run_node", "label": "Run node", "icon": "Node", "wide": True, "separator_before": True},
                                {"id": "run_section", "label": "Run section", "icon": "Section", "wide": True},
                                {"id": "cleanup_runtime", "label": "Cleanup runtime", "icon": "X", "separator_before": True},
                                {"id": "refresh_targets", "label": "Refresh targets", "icon": "Refresh", "wide": True},
                                {"id": "snapshot", "label": "Snapshot", "icon": "S", "separator_before": True},
                                {"id": "reset_graph", "label": "Reset graph", "icon": "Reset", "wide": True},
                            ),
                            width=1020,
                            height=680,
                            style={"width": "100%", "flex_grow": 1, "min_height": 640, "border_color": "#263347", "border_radius": 8},
                        )

            with dg.Tab("GUI Objects", value="objects", style=page_style):
                with dg.Panel("Bindable GUI Controls", style=panel_style):
                    dg.Label("Add Widget Source, Widget Sink, Text Input, Parser, Terminal, or Section nodes in the editor tab, then assign them to these real GUI targets.", class_="subtle")
                    self.runtime_status_label = dg.Label("Runtime: idle", class_="subtle")
                    self.target_status_label = dg.Label("Discovered Actions: loading", class_="subtle")
                    dg.Label("Text Sources", style={"font_weight": "bold"})
                    self.prompt_input = dg.TextInput(
                        "echo hello from prompt input",
                        id="v3-prompt-input",
                        placeholder="Prompt text source...",
                        on_change=lambda value: self.mark(f"Prompt Input changed: {value}"),
                        style={"width": "100%", "height": 34},
                    )
                    self.review_input = dg.TextInput(
                        "Please review the implementation.",
                        id="v3-review-input",
                        placeholder="Reviewer prompt source...",
                        on_change=lambda value: self.mark(f"Reviewer Prompt changed: {value}"),
                        style={"width": "100%", "height": 34},
                    )
                    self.scratch_input = dg.TextInput(
                        '{"kind": "scratch", "ok": true}',
                        id="v3-scratch-input",
                        placeholder="Scratch JSON/text source...",
                        on_change=lambda value: self.mark(f"Scratch changed: {value}"),
                        style={"width": "100%", "height": 34},
                    )
                    dg.Label("Action Buttons", style={"font_weight": "bold"})
                    with dg.FlowLayout(gap=6, row_gap=6):
                        dg.Button("Run Section Target", id="v3-run-section-action-button", on_click=lambda: self.binding_action("v3-run-section-action-button", "run"))
                        self.action_slot_buttons["v3-action-slot-a"] = dg.Button("Action Slot A", id="v3-action-slot-a", on_click=lambda: self.run_action_slot("v3-action-slot-a"))
                        self.action_slot_buttons["v3-action-slot-b"] = dg.Button("Action Slot B", id="v3-action-slot-b", on_click=lambda: self.run_action_slot("v3-action-slot-b"))
                        self.action_slot_buttons["v3-action-slot-c"] = dg.Button("Action Slot C", id="v3-action-slot-c", on_click=lambda: self.run_action_slot("v3-action-slot-c"))
                        self.action_slot_buttons["v3-action-slot-d"] = dg.Button("Action Slot D", id="v3-action-slot-d", on_click=lambda: self.run_action_slot("v3-action-slot-d"))
                        dg.Button("Run Selection", id="v3-run-selection-button", on_click=lambda: self.binding_action("v3-run-selection-button", "run"))
                        dg.Button("Send Prompt", id="v3-send-prompt-button", on_click=lambda: self.binding_action("v3-send-prompt-button", "run"))
                        dg.Button("Direct Terminal Test", id="v3-direct-terminal-test-button", on_click=self.send_direct_terminal_test)
                        dg.Button("Append Samples", id="v3-append-samples-button", on_click=self.append_sample_outputs)
                        dg.Button("Snapshot Target", id="v3-snapshot-button", on_click=lambda: self.binding_action("v3-snapshot-button", "run"))
                        dg.Button("Dump Bindings", id="v3-dump-bindings-button", on_click=self.dump_bindings)
                        dg.Button("Clear Logs", id="v3-clear-logs-button", on_click=self.clear_logs)
                    dg.Label("Main Output Log", style={"font_weight": "bold"})
                    self.main_output_log = dg.LogView(["Main output log ready."], id="v3-main-output-log", rows=8, follow=True, wrap=True, style=log_style)
                    dg.Label("Reviewer Output Log", style={"font_weight": "bold"})
                    self.review_output_log = dg.LogView(["Reviewer output log ready."], id="v3-review-output-log", rows=6, follow=True, wrap=True, style=log_style)
                    dg.Label("Parsed Message Log", style={"font_weight": "bold"})
                    self.message_log = dg.LogView(["Parsed message log ready."], id="v3-message-log", rows=6, follow=True, wrap=True, style=log_style)
                    dg.Label("Event Log", style={"font_weight": "bold"})
                    self.event_log = dg.LogView(["Binding playground ready."], id="v3-event-log", rows=6, follow=True, wrap=True, style=log_style)

            with dg.Tab("Terminal", value="terminal", style=page_style):
                with dg.Panel("Terminal Target", style=panel_style):
                    dg.Label("This WebView-backed terminal is a real bindable widget target for Terminal and Widget Sink nodes.", class_="subtle")
                    self.playground_terminal = dg.Terminal(
                        "cmd.exe",
                        id="v3-playground-terminal",
                        title="Playground Terminal",
                        prefer_pty=True,
                        height=420,
                        style={"width": "100%", "height": 420, "min_height": 220, "border_color": "#263347", "border_radius": 6},
                    )

        self.register_binding_targets()
        self.refresh_counts()
        self.refresh_selection()
        self.refresh_runtime_status()
        if self.graph is not None:
            self.graph.refresh_binding_targets_from_host(self.host_window)
            self.log(f"startup action targets: {', '.join(self.graph.action_target_ids())}")
        self.refresh_target_status()

@dg.component
def AllFeaturesV3(_ctx: dg.ComponentCtx) -> dg.Window:
    global win, status, progress, scatter_stats_summary, scatter, table
    global color_target, x_tick_label, y_tick_label, z_tick_label
    global stream_interval_label, dynamic_panel, style_panel, style_label, style_button
    global line_plot_top, line_plot_mid, line_plot_bottom
    global line_width_label, line_tick_label, line_window_label
    global html_report_view, html_report_status
    global histogram_latency, histogram_score, histogram_revenue, histogram_residual
    global histogram_tick_label
    global confirm_modal, about_modal, binding_playground

    v4_log: dg.LogView | None = None
    v4_palette: dg.CommandPalette | None = None
    v4_drop_status: dg.Label | None = None

    def v4_mark(message: str) -> None:
        set_status(f"V4: {message}")
        if v4_log is not None:
            v4_log.append_line(f"{time.strftime('%H:%M:%S')}  {message}")

    def open_v4_palette() -> None:
        if v4_palette is not None:
            v4_palette.show()

    def on_v4_property(change: dg.PropertyChange) -> None:
        v4_mark(f"{change.key}: {change.old_value!r} -> {change.value!r}")

    def on_v4_drop(payload: dg.DragDropPayload) -> None:
        text = f"Dropped {payload.kind or 'payload'}: {payload.payload}"
        if v4_drop_status is not None:
            v4_drop_status.set_value(text)
        v4_mark(text)

    def on_v4_heatmap(cell: dg.HeatmapCell | None) -> None:
        if cell is not None:
            v4_mark(f"Heatmap {cell.y_label or cell.row}/{cell.x_label or cell.col}: {cell.value:.3f}")

    def on_v4_bar(bar: dg.BarChartBar | None) -> None:
        if bar is not None:
            v4_mark(f"Bar {bar.category}/{bar.series}: {bar.value:.3g}")

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
        with dg.Menu("LinePlot"):
            dg.MenuItem("Append Batch", on_click=append_line_batch)
            dg.MenuItem("Start Stream", on_click=start_line_stream)
            dg.MenuItem("Stop Stream", on_click=stop_line_stream)
            dg.MenuItem("Fit Plots", on_click=fit_line_plots)
        with dg.Menu("Histogram"):
            dg.MenuItem("Fit Histograms", on_click=fit_histograms)
        with dg.Menu("Help"):
            dg.MenuItem("About", on_click=lambda: about_modal.show())


    with dg.HLayout(style={"gap": 0}):
        with dg.Sidebar(width=238, style={"padding": 12, "gap": 8}):
            dg.Label("DragonGUI", class_="brand")
            dg.Label("All features V3 + V4", class_="subtle")
            with dg.FlowLayout(gap=6, row_gap=4):
                dg.LED(True, tooltip="Renderer online")
                dg.LED("stream", states={"stream": "warning"}, tooltip="Custom stream state")
                dg.Badge("Grid", level="success")
                dg.Tag("Scatter3D", level="info")
                dg.Tag("LinePlot", level="info")
                dg.Tag("Histogram", level="warning")
                dg.Tag("V4", level="success")
                dg.Tag("HtmlReport", level="success")
            dg.Separator()
            dg.NavItem("Overview", page="overview")
            dg.NavItem("Scatter", page="scatter")
            dg.NavItem("Line plots", page="lineplots")
            dg.NavItem("Histograms", page="histograms")
            dg.NavItem("Pie charts", page="piecharts")
            dg.NavItem("V4 widgets", page="v4widgets")
            dg.NavItem("Controls", page="controls")
            dg.NavItem("Data", page="data")
            dg.NavItem("Runtime", page="runtime")
            dg.NavItem("Debug", page="debug")
            dg.NavItem("Styling", page="styling")
            dg.NavItem("Node Binding", page="layout")
            dg.Spacer()
            dg.Separator()
            dg.Label("Responsive grids, CSS, overlays, tables, and live native commands.", class_="subtle")

        with dg.Pages(value=str(demo_state["page"]), on_change=set_page, key="main-pages"):
            with dg.Page("overview", title="Overview"):
                with dg.GridLayout(columns=3, min_column_width=270, gap=GRID_GAP, style=GRID_STYLE):
                    with dg.Panel("Frame", class_="highlight", style=CARD_STYLE):
                        dg.Label(f"{POINT_ROWS:,}", class_="stat-value")
                        dg.Label("Scatter points per generated frame", class_="stat-label")
                    with dg.Panel("Data", class_="highlight", style=CARD_STYLE):
                        dg.Label(f"{TABLE_ROWS:,}", class_="stat-value")
                        dg.Label("Virtualized table rows", class_="stat-label")
                    with dg.Panel("Histograms", class_="highlight", style=CARD_STYLE):
                        dg.Label(f"{HISTOGRAM_ROWS:,}", class_="stat-value")
                        dg.Label("Distribution samples", class_="stat-label")
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
                        dg.Button("Terminal", on_click=lambda: apply_theme("terminal"))

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
                                        "Frame avg: --",
                                        "CPU work / present: --",
                                        "Observed redraws: --",
                                        "Stream handoff: Direct",
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
                            dg.Dropdown(("Direct", "UI callback"), value="Direct", on_change=set_stream_handoff)
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

            with dg.Page("lineplots", title="Line plots"):
                with dg.HLayout(class_="line-layout", style=LINE_LAYOUT_STYLE):
                    with dg.Panel("Line plot controls", class_="line-controls", style=LINE_CONTROLS_PANEL_STYLE):
                        with dg.ScrollArea(
                            axis="y",
                            gap=8,
                            class_="line-control-scroll",
                            style=LINE_CONTROLS_SCROLL_STYLE,
                        ):
                            dg.Label("Data")
                            with dg.FlowLayout(gap=8, row_gap=8):
                                dg.Button("Append batch", class_="primary", on_click=append_line_batch)
                                dg.Button("Start stream", on_click=start_line_stream)
                                dg.Button("Stop stream", on_click=stop_line_stream)
                                dg.Button("Reset plots", on_click=reset_line_plots)
                            dg.Button("Fit all plots", on_click=fit_line_plots)
                            dg.Separator()
                            dg.Label("Streaming window")
                            line_window_label = dg.Label("Window: full history")
                            with dg.FlowLayout(gap=8, row_gap=8):
                                dg.Button("Follow 10s", on_click=lambda: set_line_window(10.0))
                                dg.Button("Follow 30s", on_click=lambda: set_line_window(30.0))
                                dg.Button("Full history", on_click=lambda: set_line_window(None))
                            dg.Label(
                                "A moving window keeps all appended samples but follows the newest x values.",
                                class_="subtle",
                            )
                            dg.Separator()
                            dg.Label("Style and visibility")
                            line_width_label = dg.Label("Line width: 2.0px")
                            dg.Slider(2.0, min=0.5, max=6.0, step=0.1, on_change=set_line_width)
                            line_tick_label = dg.Label("Ticks: 5")
                            dg.Slider(5, min=2, max=9, step=1, on_change=set_line_tick_count)
                            with dg.FlowLayout(gap=8, row_gap=8):
                                dg.Checkbox("Grid", checked=True, on_change=set_line_grid)
                                dg.Checkbox("Axes", checked=True, on_change=set_line_axes)
                                dg.Checkbox("Ticks", checked=True, on_change=set_line_ticks)
                                dg.Checkbox("Legend", checked=True, on_change=set_line_legend)
                                dg.Checkbox("Toolbar", checked=True, on_change=set_line_toolbar)
                            dg.Label(
                                "Toolbar buttons cover fit, pan, zoom, box zoom, grid, and axes. Hover over each plot to inspect the nearest point.",
                                class_="subtle",
                            )

                    with dg.VLayout(class_="line-stack", style=LINE_STACK_STYLE):
                        with dg.Panel(
                            "Sensors: temperature and pressure",
                            style={
                                **CARD_STYLE,
                                "flex_grow": 1,
                                "flex_shrink": 1,
                                "min_height": 0,
                                "align_self": "stretch",
                            },
                        ):
                            line_plot_top = dg.LinePlot(
                                line_frame,
                                x="time",
                                y=("temperature", "pressure"),
                                labels=("Temperature", "Pressure"),
                                colors=("#5aa9ff", "#74ddb0"),
                                line_styles=("solid", "dashed"),
                                x_label="Elapsed time (s)",
                                y_label="Sensor reading",
                                show_legend=True,
                                show_toolbar=True,
                                tick_count=5,
                                line_width=2.0,
                                class_="stream-plot",
                                key="v3-line-top",
                            )

                        with dg.Panel(
                            "Line-style stress: vibration and events",
                            style={
                                **CARD_STYLE,
                                "flex_grow": 1,
                                "flex_shrink": 1,
                                "min_height": 0,
                                "align_self": "stretch",
                            },
                        ):
                            line_plot_mid = dg.LinePlot(
                                line_frame,
                                x="time",
                                y=("vibration", "events"),
                                labels=("Vibration", "Events"),
                                colors=("#ffb45c", "#f36b7f"),
                                line_styles=("dotted", "dashdot"),
                                x_label="Elapsed time (s)",
                                y_label="Normalized signal",
                                show_legend=True,
                                show_toolbar=True,
                                tick_count=5,
                                line_width=2.0,
                                class_="stream-plot",
                                key="v3-line-mid",
                            )

                        with dg.Panel(
                            "Streaming viewport",
                            style={
                                **CARD_STYLE,
                                "flex_grow": 1,
                                "flex_shrink": 1,
                                "min_height": 0,
                                "align_self": "stretch",
                            },
                        ):
                            line_plot_bottom = dg.LinePlot(
                                line_frame,
                                x="time",
                                y="temperature",
                                label="Live Temperature",
                                color="#5aa9ff",
                                line_style="solid",
                                x_label="Elapsed time (s)",
                                y_label="Temperature",
                                show_legend=True,
                                show_toolbar=True,
                                tick_count=5,
                                line_width=2.0,
                                window_size=None,
                                class_="stream-plot",
                                key="v3-line-bottom",
                            )

            with dg.Page("histograms", title="Histograms"):
                with dg.HLayout(class_="histogram-layout", style=HISTOGRAM_LAYOUT_STYLE):
                    with dg.Panel(
                        "Histogram controls",
                        class_="histogram-controls",
                        style=HISTOGRAM_CONTROLS_PANEL_STYLE,
                    ):
                        with dg.ScrollArea(
                            axis="y",
                            gap=8,
                            class_="histogram-control-scroll",
                            style=HISTOGRAM_CONTROLS_SCROLL_STYLE,
                        ):
                            dg.Label("Viewport tools")
                            dg.Button("Fit all histograms", class_="primary", on_click=fit_histograms)
                            dg.Label(
                                "Each histogram toolbar mirrors the line plot tools: fit, pan, wheel zoom, box zoom, grid, and axes.",
                                class_="subtle",
                            )
                            dg.Separator()
                            dg.Label("Visibility")
                            histogram_tick_label = dg.Label("Ticks: 5")
                            dg.Slider(5, min=2, max=9, step=1, on_change=set_histogram_tick_count)
                            with dg.FlowLayout(gap=8, row_gap=8):
                                dg.Checkbox("Grid", checked=True, on_change=set_histogram_grid)
                                dg.Checkbox("Axes", checked=True, on_change=set_histogram_axes)
                                dg.Checkbox("Ticks", checked=True, on_change=set_histogram_ticks)
                                dg.Checkbox("Toolbar", checked=True, on_change=set_histogram_toolbar)
                            dg.Separator()
                            dg.Label("Coverage")
                            with dg.FlowLayout(gap=6, row_gap=6):
                                dg.Badge("count", level="info")
                                dg.Badge("density", level="success")
                                dg.Badge("percent", level="warning")
                                dg.Badge("cumulative", level="danger")
                            dg.Label(
                                "This section stresses normalized modes, explicit bins, cumulative bars, and toolbar interaction in styled panels.",
                                class_="subtle",
                            )

                    with dg.ScrollArea(
                        axis="y",
                        gap=12,
                        class_="histogram-scroll",
                        style=HISTOGRAM_SCROLL_STYLE,
                    ):
                        with dg.GridLayout(
                            columns=2,
                            min_column_width=420,
                            gap=GRID_GAP,
                            row_gap=GRID_GAP,
                            class_="histogram-grid",
                            style={"width": "100%", "align_items": "stretch", "min_width": 0},
                        ):
                            with dg.Panel("Latency distribution", style=CARD_STYLE):
                                dg.Label("Bimodal response-time data over a fixed 0-170 ms range.", class_="subtle")
                                histogram_latency = dg.Histogram(
                                    histogram_frame,
                                    value="latency_ms",
                                    bins=34,
                                    range=(0.0, 170.0),
                                    label="Latency",
                                    x_label="latency (ms)",
                                    y_label="requests",
                                    color="#5aa9ff",
                                    show_toolbar=True,
                                    tick_count=5,
                                    class_="latency",
                                    key="v3-hist-latency",
                                )

                            with dg.Panel("Density normalization", style=CARD_STYLE):
                                dg.Label("Score values normalized so total bin area is one.", class_="subtle")
                                histogram_score = dg.Histogram(
                                    histogram_frame,
                                    value="score",
                                    bins=26,
                                    range=(0.0, 1.0),
                                    mode="density",
                                    x_label="score",
                                    y_label="density",
                                    color="#74ddb0",
                                    show_toolbar=True,
                                    tick_count=5,
                                    class_="density",
                                    key="v3-hist-score",
                                )

                            with dg.Panel("Explicit revenue bins", style=CARD_STYLE):
                                dg.Label("Log-normal revenue with manually supplied threshold bins.", class_="subtle")
                                histogram_revenue = dg.Histogram(
                                    histogram_frame,
                                    value="revenue",
                                    bin_edges=(0, 12, 20, 32, 50, 80, 125, 200, 320),
                                    mode="percent",
                                    x_label="revenue",
                                    y_label="share (%)",
                                    color="#ffcc66",
                                    show_toolbar=True,
                                    tick_count=5,
                                    class_="percent",
                                    bar_gap=2.0,
                                    key="v3-hist-revenue",
                                )

                            with dg.Panel("Cumulative residuals", style=CARD_STYLE):
                                dg.Label("Residual values accumulated left to right as probability mass.", class_="subtle")
                                histogram_residual = dg.Histogram(
                                    histogram_frame,
                                    value="residual",
                                    bins=30,
                                    range=(-1.2, 1.2),
                                    mode="probability",
                                    cumulative=True,
                                    x_label="residual",
                                    y_label="cumulative probability",
                                    color="#f36b7f",
                                    show_toolbar=True,
                                    tick_count=5,
                                    class_="cumulative",
                                    bar_gap=1.5,
                                    key="v3-hist-residual",
                                )

            with dg.Page("piecharts", title="Pie charts"):
                with dg.GridLayout(columns=2, min_column_width=520, gap=GRID_GAP, style=GRID_STYLE):
                    with dg.Panel("Cloud spend by category", style=CARD_STYLE):
                        dg.Label("Share of total cloud expenditure by category.", class_="subtle")
                        dg.PieChart(
                            labels=["Compute", "Storage", "Network", "Support"],
                            values=[42, 27, 18, 13],
                            colors=PIE_DASHBOARD_COLORS,
                            donut=True,
                            inner_radius=0.58,
                            label_mode="legend",
                            center_value="$58.4k",
                            center_label="Total spend",
                            show_toolbar=True,
                            class_="dashboard",
                            key="v3-pie-spend",
                        )
                    with dg.Panel("Regional mix", style=CARD_STYLE):
                        dg.Label("Distribution of accounts by region.", class_="subtle")
                        dg.PieChart(
                            labels=["Americas", "EMEA", "APAC", "Other"],
                            values=[38, 29, 21, 12],
                            colors=PIE_DASHBOARD_COLORS,
                            donut=True,
                            inner_radius=0.58,
                            label_mode="legend",
                            center_value="1,842",
                            center_label="Total accounts",
                            show_toolbar=True,
                            class_="dashboard",
                            key="v3-pie-regions",
                        )
                    with dg.Panel("Accounts by segment", style=CARD_STYLE):
                        dg.Label("Frame-backed account totals grouped by segment.", class_="subtle")
                        dg.PieChart(
                            pie_segment_frame,
                            category="segment",
                            value="accounts",
                            aggregate="sum",
                            top_n=4,
                            other_label="Long tail",
                            colors=PIE_DASHBOARD_COLORS,
                            donut=True,
                            inner_radius=0.58,
                            label_mode="legend",
                            center_value="92",
                            center_label="Total accounts",
                            show_toolbar=True,
                            class_="dashboard",
                            key="v3-pie-accounts",
                        )
                    with dg.Panel("Revenue by segment", style=CARD_STYLE):
                        dg.Label("Frame-backed revenue totals with top-N grouping.", class_="subtle")
                        dg.PieChart(
                            pie_segment_frame,
                            category="segment",
                            value="revenue",
                            aggregate="sum",
                            top_n=3,
                            other_label="Other",
                            colors=PIE_DASHBOARD_COLORS,
                            donut=True,
                            inner_radius=0.58,
                            label_mode="legend",
                            center_value="$178k",
                            center_label="Total revenue",
                            show_toolbar=True,
                            class_="dashboard",
                            key="v3-pie-revenue",
                        )

            with dg.Page("v4widgets", title="V4 widgets"):
                with dg.ScrollArea(axis="y", gap=GRID_GAP, style=V4_SCROLL_STYLE):
                    with dg.GridLayout(columns=2, min_column_width=430, masonry=True, gap=GRID_GAP, style=GRID_STYLE):
                        with dg.Panel("Selection and navigation", style=V4_CARD_STYLE):
                            dg.Breadcrumbs(
                                [
                                    ("Workspace", "workspace"),
                                    ("Telemetry", "telemetry"),
                                    ("Sensor cluster", "cluster"),
                                    ("Run 042", "run"),
                                ],
                                max_items=4,
                                on_select=lambda item: v4_mark(f"Breadcrumb: {item.label}"),
                            )
                            dg.SelectableList(
                                [
                                    ("Overview", "overview"),
                                    ("Sensors", "sensors"),
                                    ("Streaming plots", "streams"),
                                    {"label": "Archived batch", "value": "archive", "disabled": True},
                                ],
                                value="sensors",
                                on_change=lambda value: v4_mark(f"SelectableList: {value}"),
                                max_height=150,
                            )
                            with dg.FlowLayout(gap=8, row_gap=6):
                                dg.Selectable(
                                    "Standalone selectable",
                                    selected=True,
                                    on_select=lambda selected: v4_mark(f"Selectable: {selected}"),
                                )
                                dg.RadioButton(
                                    "Manual radio",
                                    checked=True,
                                    on_change=lambda checked: v4_mark(f"RadioButton: {checked}"),
                                )
                            dg.RadioGroup(
                                ["Low", "Medium", "High"],
                                value="Medium",
                                orientation="horizontal",
                                gap=12,
                                on_change=lambda value: v4_mark(f"RadioGroup: {value}"),
                            )
                            with dg.TreeView(on_select=lambda node_id: v4_mark(f"TreeView: {node_id}")):
                                with dg.TreeNode("Scene", node_id="scene", expanded=True):
                                    dg.TreeNode("Camera", node_id="camera", leaf=True)
                                    with dg.TreeNode("Lights", node_id="lights", expanded=True):
                                        dg.TreeNode("Key", node_id="key-light", leaf=True, selected=True)
                                        dg.TreeNode("Fill", node_id="fill-light", leaf=True)
                                with dg.TreeNode("Materials", node_id="materials"):
                                    dg.TreeNode("Glass", node_id="glass", leaf=True)
                                    dg.TreeNode("Metal", node_id="metal", leaf=True)

                        with dg.Panel("Numeric and temporal inputs", style=V4_CARD_STYLE):
                            dg.ToggleSwitch(
                                "Live acquisition",
                                checked=True,
                                on_change=lambda checked: v4_mark(f"ToggleSwitch: {checked}"),
                            )
                            dg.DragNumber(
                                12.5,
                                min=0,
                                max=30,
                                step=0.5,
                                on_change=lambda value: v4_mark(f"DragNumber: {value:.1f} ms"),
                            )
                            dg.DragVector(
                                (0.0, 1.5, -2.0),
                                labels=("X", "Y", "Z"),
                                min=-5,
                                max=5,
                                step=0.25,
                                component_width=86,
                                on_change=lambda value: v4_mark(f"DragVector: {tuple(round(v, 2) for v in value)}"),
                            )
                            dg.RangeSlider(
                                (20, 84),
                                min=0,
                                max=100,
                                step=2,
                                on_change=lambda value: v4_mark(f"RangeSlider: {value[0]:.0f}-{value[1]:.0f}"),
                            )
                            with dg.GridLayout(columns=3, min_column_width=120, gap=8, style={"width": "100%"}):
                                dg.DateInput("2026-05-24", on_change=lambda value: v4_mark(f"DateInput: {value}"))
                                dg.TimeInput("09:30", on_change=lambda value: v4_mark(f"TimeInput: {value}"))
                                dg.DateTimeInput(
                                    "2026-05-24T09:30:00",
                                    on_change=lambda value: v4_mark(f"DateTimeInput: {value}"),
                                )

                        with dg.Panel("Toolbar and command palette", style=V4_CARD_STYLE):
                            with dg.Toolbar():
                                dg.IconButton("play", tooltip="Run", on_click=lambda: v4_mark("Toolbar run"))
                                dg.IconButton("pause", tooltip="Pause", on_click=lambda: v4_mark("Toolbar pause"))
                                dg.IconButton("stop", tooltip="Stop", on_click=lambda: v4_mark("Toolbar stop"))
                                dg.ToolbarSeparator()
                                dg.IconButton("save", tooltip="Save", on_click=lambda: v4_mark("Toolbar save"))
                                dg.ImageButton(
                                    demo_image_path,
                                    size=34,
                                    fit="cover",
                                    tooltip="ImageButton",
                                    on_click=lambda: v4_mark("ImageButton"),
                                )
                                dg.ToolbarSeparator()
                                dg.SearchBox(
                                    "",
                                    placeholder="Search status...",
                                    on_change=lambda value: v4_mark(f"SearchBox: {value}"),
                                    style={"width": 210},
                                )
                                dg.SmallButton("Deploy", class_="primary", on_click=lambda: v4_mark("SmallButton deploy"))
                            with dg.FlowLayout(gap=8, row_gap=8):
                                dg.ArrowButton("left", tooltip="Previous", on_click=lambda: v4_mark("Arrow left"))
                                dg.ArrowButton("right", tooltip="Next", on_click=lambda: v4_mark("Arrow right"))
                                dg.ArrowButton("up", tooltip="Move up", on_click=lambda: v4_mark("Arrow up"))
                                dg.ArrowButton("down", tooltip="Move down", on_click=lambda: v4_mark("Arrow down"))
                                dg.Button("Command palette", on_click=open_v4_palette)
                            with dg.HLayout(style={"gap": 10, "align_items": "center"}):
                                dg.LoadingSpinner(size=26, label="Syncing", speed=1.25)
                                dg.LoadingSpinner(size=22, spinning=False, label="Paused")

                        with dg.Panel("Property inspector and split panes", style=V4_CARD_STYLE):
                            with dg.Splitter(
                                orientation="horizontal",
                                sizes=(120, "1fr"),
                                min_sizes=(110, 240),
                                gutter_size=10,
                                style=V4_INSPECTOR_SPLITTER_STYLE,
                            ):
                                with dg.Pane(min_size=110, style=V4_INSPECTOR_NAV_PANE_STYLE):
                                    dg.Selectable("Inspector", selected=True)
                                    dg.Selectable("Metrics")
                                    dg.Selectable("Exports")
                                    dg.Separator(style=V4_INSPECTOR_SEPARATOR_STYLE)
                                    dg.Label("Drag center divider", class_="subtle")
                                with dg.Pane(min_size=240, style=V4_INSPECTOR_BODY_PANE_STYLE):
                                    with dg.PropertyGrid(label_width=84):
                                        dg.Property(
                                            "Threshold",
                                            dg.RangeSlider(
                                                (0.2, 0.82),
                                                min=0,
                                                max=1,
                                                step=0.02,
                                                on_change=lambda value: v4_mark(
                                                    f"Threshold: {value[0]:.2f}-{value[1]:.2f}"
                                                ),
                                            ),
                                        )
                                        dg.Property(
                                            "Exposure",
                                            dg.DragNumber(
                                                8.5,
                                                min=0,
                                                max=24,
                                                step=0.5,
                                                on_change=lambda value: v4_mark(f"Exposure: {value:.1f}"),
                                            ),
                                        )
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
                                        on_change=on_v4_property,
                                        label_width=84,
                                    )

                        with dg.Panel("Drag, drop, and table upgrades", style=V4_CARD_STYLE):
                            with dg.FlowLayout(gap=8, row_gap=8):
                                with dg.DragSource(
                                    {"metric": "latency", "window": "p95"},
                                    drag_kind="metric",
                                ):
                                    dg.Badge("latency p95", level="warning")
                                with dg.DragSource(
                                    {"metric": "error_rate", "window": "1h"},
                                    drag_kind="metric",
                                ):
                                    dg.Badge("error rate", level="danger")
                            v4_drop_status = dg.Label("Drag a metric badge onto the drop zone.", class_="subtle")
                            dg.DropZone("Drop metric here", accept="metric", on_drop=on_v4_drop, style={"height": 96})
                            dg.DataFrameTable(
                                v4_table_frame,
                                page_size=36,
                                sample_rows=36,
                                sortable=True,
                                resizable_columns=True,
                                on_select=lambda selection: v4_mark(
                                    f"Table {selection.row_index}:{selection.column}={selection.value}"
                                ),
                                on_sort=lambda sort: v4_mark(f"Sort {sort.target} {sort.column} {sort.direction}"),
                                style={"height": 230, "min_height": 0},
                            )

                        with dg.Panel("Code editor and log view", style=V4_CARD_STYLE):
                            dg.CodeEditor(
                                "def classify(sample):\n"
                                "    if sample.score > 0.8:\n"
                                "        return 'watch'\n"
                                "    return 'ok'\n",
                                language="python",
                                rows=7,
                                on_change=lambda value: v4_mark(f"CodeEditor lines: {len(value.splitlines())}"),
                                style={"width": "100%"},
                            )
                            v4_log = dg.LogView(
                                [
                                    "09:30:00  V4 page mounted",
                                    "09:30:01  Waiting for widget interaction",
                                    "09:30:02  Native log rows are virtualized",
                                ],
                                rows=7,
                                follow=True,
                                style={"width": "100%"},
                            )
                            with dg.FlowLayout(gap=8, row_gap=8):
                                dg.SmallButton("Append log", on_click=lambda: v4_mark("Manual log append"))
                                dg.SmallButton("Clear log", on_click=lambda: v4_log.clear() if v4_log is not None else None)

                        with dg.Panel("2D scatter and heatmap", style=V4_CARD_STYLE):
                            dg.ScatterPlot2D(
                                v4_scatter_frame,
                                x="x",
                                y="y",
                                scalars="score",
                                colormap="turbo",
                                point_size=3.0,
                                scalar_bar=True,
                                scalar_bar_title="score",
                                hover=["row_id", "group"],
                                on_pick=lambda point: v4_mark(f"Scatter2D point {point.index}"),
                                style=V4_CHART_STYLE,
                            )
                            dg.Heatmap(
                                v4_heatmap_matrix,
                                x_labels=[f"T{i}" for i in range(v4_heatmap_matrix.shape[1])],
                                y_labels=[f"L{i}" for i in range(v4_heatmap_matrix.shape[0])],
                                title="Classifier bins",
                                colormap="magma",
                                on_hover=on_v4_heatmap,
                                style=V4_CHART_STYLE,
                            )

                        with dg.Panel("Bar chart", style=V4_CARD_STYLE):
                            dg.BarChart(
                                pie_segment_frame,
                                category="segment",
                                value=["revenue", "accounts"],
                                aggregate="sum",
                                x_label="segment",
                                y_label="total",
                                colors=["#69b7ff", "#76e0b1"],
                                show_toolbar=True,
                                on_hover=on_v4_bar,
                                style=V4_CHART_STYLE,
                            )
                            dg.BarChart(
                                labels=["Ingest", "Transform", "Validate", "Export"],
                                values=[78, 64, 52, 38],
                                orientation="horizontal",
                                x_label="jobs",
                                y_label="stage",
                                colors=["#ffbf69"],
                                show_toolbar=True,
                                on_hover=on_v4_bar,
                                style=V4_CHART_STYLE,
                            )

                    v4_palette = dg.CommandPalette(
                        [
                            dg.Command("push-scatter", "Push scatter frame", lambda: push_scatter("cloud"), "Cloud demo data"),
                            dg.Command("fit-lines", "Fit line plots", fit_line_plots, "Reset line plot bounds"),
                            dg.Command("snapshot", "Print snapshot", print_snapshot, "Dump redacted document"),
                            dg.Command("theme-midnight", "Theme: midnight", lambda: apply_theme("midnight")),
                            dg.Command("disabled", "Disabled command", disabled=True),
                        ],
                        title="V4 Command Palette",
                        on_run=lambda command: v4_mark(f"Command: {command.id}"),
                        parent=win,
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
                with dg.GridLayout(
                    columns=2,
                    min_column_width=420,
                    masonry=True,
                    gap=GRID_GAP,
                    style=DEBUG_GRID_STYLE,
                ):
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
                    with dg.Panel(
                        "HTML report viewer",
                        style={
                            **CARD_STYLE,
                            "min_height": 0,
                            "align_self": "stretch",
                            "flex_grow": 1,
                            "flex_shrink": 1,
                        },
                    ):
                        dg.Label(
                            f"{REPORT_BACKEND_LABEL} report. File-backed and inline HTML reports are embedded with WebView2 on Windows.",
                            class_="subtle",
                        )
                        with dg.FlowLayout(gap=8, row_gap=8):
                            dg.Button("Overview", class_="primary", on_click=show_report_overview)
                            dg.Button("Detail", on_click=show_report_detail)
                            dg.Button("Inline", on_click=show_report_inline)
                            dg.Button("Reload", on_click=reload_html_report)
                            dg.Button("External", on_click=open_html_report_external)
                            dg.Button("Snapshot", on_click=refresh_html_report_snapshot)
                        html_report_view = dg.HtmlReport(
                            report_overview_path,
                            class_="report-viewer",
                            key="debug-html-report",
                            height=540,
                        )
                        html_report_status = dg.Label(
                            f"{REPORT_BACKEND_LABEL}: {Path(report_overview_path).name}",
                            class_="subtle",
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
                        dg.Button("Terminal CSS", on_click=lambda: apply_theme("terminal"))
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

            with dg.Page("layout", title="Node Binding"):
                binding_playground = NodeGraphBindingPlayground(win)
                binding_playground.build()

    with dg.StatusBar(height=40):
        status = dg.TextInput("Ready", placeholder="status", style={"width": 360})
        dg.Separator(orientation="vertical")
        dg.Label(f"{POINT_ROWS:,} points")
        dg.Label(f"{TABLE_ROWS:,} table rows")
        dg.Spacer()
        dg.Label("All features V3 + V4")


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
        "This V3 demo now includes the V4 widget set: inspectors, command palette, toolbars, split panes, drag/drop, temporal inputs, code/log views, 2D scatter, heatmap, and bar chart.",
        open=False,
        parent=win,
    )

    with dg.ContextMenu(target=table, width=230, parent=win):
        dg.MenuItem("Print Snapshot", on_click=print_snapshot)
        dg.MenuItem("Load Wave Table", on_click=lambda: update_table("wave"))
        dg.MenuItem("Load Cloud Table", on_click=lambda: update_table("cloud"))

    return win


def layout_summary_for_page(result: dict, route: str) -> dict:
    gpu = result.get("debug_snapshot", {}).get("gpu", {})
    tree = gpu.get("tree") or {}
    layout = gpu.get("layout") or {}
    rects = layout.get("rects") or {}
    clips = layout.get("clips") or {}
    scroll_max_y = layout.get("scroll_max_y") or {}

    page: dict | None = None
    descendants: list[dict] = []

    def walk(node: dict, under_page: bool = False) -> None:
        nonlocal page
        props = node.get("props") or {}
        is_page = node.get("type") == "page" and props.get("route_value") == route
        if is_page:
            page = node
        if under_page or is_page:
            descendants.append(node)
        for child in node.get("children") or []:
            if isinstance(child, dict):
                walk(child, under_page or is_page)

    if isinstance(tree, dict):
        walk(tree)

    def rect_for(node: dict) -> dict | None:
        rect = rects.get(node.get("id"))
        if not isinstance(rect, dict):
            return None
        return {
            "x": round(float(rect.get("x", 0.0)), 1),
            "y": round(float(rect.get("y", 0.0)), 1),
            "w": round(float(rect.get("w", 0.0)), 1),
            "h": round(float(rect.get("h", 0.0)), 1),
        }

    def label_for(node: dict) -> str:
        props = node.get("props") or {}
        return str(props.get("text") or props.get("route_value") or node.get("type") or node.get("id"))

    focus_types = {"page", "scroll_area", "grid_layout"}
    focus = []
    panels = []
    for node in descendants:
        node_type = node.get("type")
        rect = rect_for(node)
        if rect is None:
            continue
        item = {
            "id": node.get("id"),
            "type": node_type,
            "label": label_for(node),
            "rect": rect,
            "clip": clips.get(node.get("id")),
            "scroll_max_y": round(float(scroll_max_y.get(node.get("id"), 0.0)), 1),
        }
        if node_type in focus_types:
            focus.append(item)
        if node_type == "panel":
            panels.append(item)

    overlaps = []
    for index, left in enumerate(panels):
        a = left["rect"]
        for right in panels[index + 1 :]:
            b = right["rect"]
            overlap_w = min(a["x"] + a["w"], b["x"] + b["w"]) - max(a["x"], b["x"])
            overlap_h = min(a["y"] + a["h"], b["y"] + b["h"]) - max(a["y"], b["y"])
            if overlap_w > 1 and overlap_h > 1:
                overlaps.append(
                    {
                        "a": left["label"],
                        "b": right["label"],
                        "overlap": [round(overlap_w, 1), round(overlap_h, 1)],
                    }
                )

    return {
        "route": route,
        "page_found": page is not None,
        "focus": focus,
        "panels": panels,
        "panel_overlaps": overlaps,
    }



stats_thread = threading.Thread(target=scatter_stats_worker, daemon=True)
stats_thread.start()

try:
    result = app.run_with_loading(
        AllFeaturesV3,
        title="DragonGUI All Features V3 Demo",
        width=1440,
        height=900,
    )
except dg.BackendUnavailableError:
    print("DragonGUI source import works.")
    print("Native backend is not built, so this run prints the UI document.")
    pprint(redacted_document(app.document(win)))
else:
    if os.environ.get("DRAGONGUI_DEMO_LAYOUT_SUMMARY"):
        print(json.dumps(layout_summary_for_page(result, str(demo_state["page"])), indent=2))
    else:
        print(result)
finally:
    stats_stop.set()
    stream_cancel.set()
    if stats_thread is not None:
        stats_thread.join(timeout=0.25)
    if stream_controller is not None:
        stream_controller.stop(timeout=0.25)


