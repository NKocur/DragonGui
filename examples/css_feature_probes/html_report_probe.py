"""HtmlReport probe.

Verifies:
- HtmlReport serializes and lays out like a report surface.
- CSS selectors can style the widget.
- Live path/html swaps update the native placeholder.
- The embedded WebView2 path can fall back to external-browser viewing.
"""
from __future__ import annotations

import json
import math
import sys
from html import escape
from pathlib import Path

_REPO_PYTHON = str(Path(__file__).resolve().parents[2] / "python")
if _REPO_PYTHON not in sys.path:
    sys.path.insert(0, _REPO_PYTHON)

import dragongui as dg


REPORT_DIR = Path(__file__).resolve().parent / "generated"
REPORT_A = REPORT_DIR / "html_report_probe_overview.html"
REPORT_B = REPORT_DIR / "html_report_probe_detail.html"


def _plotly_report_html(title: str, color: str, phase: str, offset: float) -> str:
    points = [
        [idx, 42.0 + 12.0 * math.sin(idx * 0.18 + offset) + 4.0 * math.cos(idx * 0.47)]
        for idx in range(72)
    ]
    data = json.dumps(points, separators=(",", ":"))
    safe_title = escape(title)
    safe_phase = escape(phase)
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{safe_title}</title>
  <style>
    body {{
      margin: 0;
      font-family: Segoe UI, Arial, sans-serif;
      background: #101827;
      color: #edf3ff;
    }}
    main {{ padding: 22px; }}
    h1 {{ margin: 0 0 6px; font-size: 23px; }}
    p {{ margin: 0 0 18px; color: rgba(237, 243, 255, 0.72); }}
    .plotly-graph-div {{
      min-height: 342px;
      border: 1px solid rgba(255, 255, 255, 0.16);
      border-radius: 8px;
      background: #0d1424;
      position: relative;
      overflow: hidden;
      user-select: none;
    }}
    .modebar {{
      position: absolute;
      top: 10px;
      right: 10px;
      display: flex;
      gap: 6px;
      z-index: 2;
    }}
    .modebar-btn {{
      min-width: 34px;
      height: 30px;
      border-radius: 5px;
      border: 1px solid rgba(255, 255, 255, 0.20);
      background: rgba(24, 34, 53, 0.92);
      color: #edf3ff;
      cursor: pointer;
    }}
    .modebar-btn.active {{ border-color: {color}; color: white; }}
    svg {{ width: 100%; height: 342px; display: block; }}
    .hoverlabel {{
      position: absolute;
      left: 18px;
      bottom: 14px;
      padding: 6px 8px;
      border-radius: 5px;
      background: rgba(3, 7, 18, 0.86);
      border: 1px solid rgba(255, 255, 255, 0.16);
      color: rgba(237, 243, 255, 0.82);
      font-size: 12px;
    }}
    .selection {{
      display: none;
      position: absolute;
      border: 1px solid {color};
      background: rgba(90, 169, 255, 0.12);
      pointer-events: none;
    }}
  </style>
</head>
<body>
  <main>
    <h1>{safe_title}</h1>
    <p>Self-contained Plotly-style report fixture with modebar controls, hover readout, wheel zoom, and drag selection.</p>
    <div id="graph" class="plotly-graph-div">
      <div class="modebar" role="toolbar" aria-label="plot tools">
        <button class="modebar-btn active" data-mode="pan" title="Pan">Pan</button>
        <button class="modebar-btn" data-mode="zoom" title="Zoom">Zoom</button>
        <button class="modebar-btn" data-mode="select" title="Box select">Box</button>
        <button class="modebar-btn" data-mode="reset" title="Reset axes">Fit</button>
      </div>
      <svg id="plot" viewBox="0 0 760 342" aria-label="demo chart">
        <g stroke="rgba(255,255,255,0.14)" stroke-width="1">
          <path d="M58 42V282M186 42V282M314 42V282M442 42V282M570 42V282M698 42V282"/>
          <path d="M58 282H712M58 222H712M58 162H712M58 102H712M58 42H712"/>
        </g>
        <path id="trace" fill="none" stroke="{color}" stroke-width="4" stroke-linecap="round"/>
        <g id="markers"></g>
        <g fill="rgba(237,243,255,0.74)" font-size="14">
          <text x="58" y="312">0</text>
          <text x="342" y="312">{safe_phase}</text>
          <text x="668" y="312">72s</text>
          <text x="16" y="48">high</text>
          <text x="20" y="286">low</text>
        </g>
      </svg>
      <div id="hover" class="hoverlabel">mode: pan, point: --</div>
      <div id="selection" class="selection"></div>
    </div>
  </main>
  <script>
    const data = {data};
    const graph = document.getElementById("graph");
    const plot = document.getElementById("plot");
    const trace = document.getElementById("trace");
    const markers = document.getElementById("markers");
    const hover = document.getElementById("hover");
    const selection = document.getElementById("selection");
    let mode = "pan";
    let scale = 1.0;
    let dragStart = null;

    function sx(x) {{ return 58 + x * (654 / 71); }}
    function sy(y) {{ return 282 - (y - 20) * (240 / 44) * scale; }}
    function draw() {{
      const path = data.map((p, i) => (i === 0 ? "M" : "L") + sx(p[0]).toFixed(1) + " " + sy(p[1]).toFixed(1)).join(" ");
      trace.setAttribute("d", path);
      markers.innerHTML = data.filter((_, i) => i % 6 === 0).map(p =>
        '<circle cx="' + sx(p[0]).toFixed(1) + '" cy="' + sy(p[1]).toFixed(1) + '" r="3.5" fill="{color}"/>'
      ).join("");
    }}
    draw();

    document.querySelectorAll(".modebar-btn").forEach(btn => {{
      btn.addEventListener("click", () => {{
        document.querySelectorAll(".modebar-btn").forEach(item => item.classList.remove("active"));
        btn.classList.add("active");
        if (btn.dataset.mode === "reset") {{
          scale = 1.0;
          mode = "pan";
          draw();
        }} else {{
          mode = btn.dataset.mode;
        }}
        hover.textContent = "mode: " + mode + ", point: --";
      }});
    }});

    plot.addEventListener("wheel", event => {{
      event.preventDefault();
      scale = Math.max(0.72, Math.min(1.55, scale + (event.deltaY < 0 ? 0.08 : -0.08)));
      draw();
      hover.textContent = "mode: wheel zoom, scale: " + scale.toFixed(2);
    }}, {{ passive: false }});

    plot.addEventListener("pointermove", event => {{
      const rect = plot.getBoundingClientRect();
      const x = Math.max(0, Math.min(71, Math.round((event.clientX - rect.left) / rect.width * 71)));
      const point = data[x];
      if (point) hover.textContent = "mode: " + mode + ", point: " + point[0] + ", " + point[1].toFixed(2);
      if (dragStart && mode === "select") {{
        const left = Math.min(dragStart.x, event.clientX - rect.left);
        const top = Math.min(dragStart.y, event.clientY - rect.top);
        const width = Math.abs(event.clientX - rect.left - dragStart.x);
        const height = Math.abs(event.clientY - rect.top - dragStart.y);
        selection.style.display = "block";
        selection.style.left = left + "px";
        selection.style.top = top + "px";
        selection.style.width = width + "px";
        selection.style.height = height + "px";
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
      if (mode !== "select") selection.style.display = "none";
    }});
    </script>
</body>
</html>
"""


INLINE_REPORT = """<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>Inline report</title></head>
<body style="margin:0;background:#101827;color:#edf3ff;font-family:Segoe UI,Arial,sans-serif">
  <main style="padding:28px">
    <h1 style="margin:0 0 8px;font-size:24px">Inline report</h1>
    <p style="color:rgba(237,243,255,.72)">This document was provided directly as an HTML string.</p>
  </main>
</body>
</html>
"""


def _write_reports() -> None:
    REPORT_DIR.mkdir(parents=True, exist_ok=True)
    REPORT_A.write_text(
        _plotly_report_html("Sensor overview", "#5aa9ff", "overview", 0.0),
        encoding="utf-8",
    )
    REPORT_B.write_text(
        _plotly_report_html("Failure detail", "#f36b7f", "detail", 1.7),
        encoding="utf-8",
    )


_write_reports()

app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #0b1020;
        color: rgba(247, 250, 255, 0.94);
        font-size: 14px;
        padding: 12px;
        overflow: hidden;
    }

    HLayout.root {
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 0;
        gap: 12px;
    }

    Panel.controls {
        width: 260px;
        max-width: 260px;
        height: 100%;
        flex-shrink: 0;
        padding: 12px;
        gap: 8px;
    }

    Panel.controls Button {
        width: 100%;
        flex-shrink: 0;
    }

    Panel.viewer-panel {
        flex-grow: 1;
        min-width: 0;
        min-height: 0;
        height: 100%;
        padding: 12px;
        gap: 10px;
    }

    HtmlReport.viewer {
        width: 100%;
        min-height: 320px;
        background: #111827;
        border-color: rgba(90, 169, 255, 0.45);
        border-radius: 8px;
        color: rgba(237, 243, 255, 0.74);
        font-size: 14px;
        padding: 18px;
    }

    Label.title {
        font-size: 18px;
        font-weight: 800;
        color: white;
    }

    Label.caption {
        color: rgba(247, 250, 255, 0.64);
        font-size: 12px;
    }

    Label.status {
        color: #74ddb0;
        font-size: 12px;
    }
    """
)

report_view: dg.HtmlReport | None = None
status_label: dg.Label | None = None


def _set_status(text: str) -> None:
    if status_label is not None:
        status_label.set_value(text)


def _show_overview() -> None:
    if report_view is not None:
        report_view.set_path(REPORT_A)
    _set_status(f"path: {REPORT_A.name}")


def _show_detail() -> None:
    if report_view is not None:
        report_view.set_path(REPORT_B)
    _set_status(f"path: {REPORT_B.name}")


def _show_inline() -> None:
    if report_view is not None:
        report_view.set_html(INLINE_REPORT, base_dir=REPORT_DIR)
    _set_status("inline HTML document")


def _reload() -> None:
    if report_view is not None:
        report_view.reload()
    _set_status("reloaded current report source")


def _open_external() -> None:
    if report_view is not None and report_view.open_external():
        _set_status("opened in external browser")
    else:
        _set_status("external fallback disabled or unavailable")


win = dg.Window(
    "HtmlReport probe",
    width=980,
    height=620,
    style={"display": "flex", "flex_direction": "column", "min_width": 0, "min_height": 0},
)

with dg.HLayout(parent=win, class_="root"):
    with dg.Panel("Report controls", class_="controls"):
        dg.Label("HTML REPORT", class_="title")
        dg.Label("Swap file-backed and inline documents, then open the active report externally.", class_="caption")
        dg.Button("Overview Report", on_click=_show_overview)
        dg.Button("Detail Report", on_click=_show_detail)
        dg.Button("Inline HTML", on_click=_show_inline)
        dg.Button("Reload Source", on_click=_reload)
        dg.Button("Open External", on_click=_open_external)
        status_label = dg.Label(f"path: {REPORT_A.name}", class_="status")

    with dg.Panel("Viewer surface", class_="viewer-panel"):
        report_view = dg.HtmlReport(REPORT_A, class_="viewer")
        dg.Label(
            "PASS: the report surface is reserved and styled. If embedded WebView2 is unavailable, external open still shows the interactive HTML.",
            class_="caption",
        )


if __name__ == "__main__":
    print(app.run(win))
