"""Render the DragonGUI telemetry indicator decomposition as HTML."""

from __future__ import annotations

import argparse
from datetime import datetime
from html import escape
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SUMMARY = ROOT / "artifacts" / "gui-telemetry-indicator-decomposition" / "summary.json"
DEFAULT_OUTPUT = ROOT / "plans" / "gui-telemetry-indicator-decomposition-report.html"
COUNTS = (24, 72, 160, 320)
MODES = ("labels", "progress", "leds", "combined")
LABELS = {"labels": "Labels", "progress": "Progress", "leds": "LEDs", "combined": "Combined"}
COLORS = {"labels": "#8aa4ff", "progress": "#57d7a1", "leds": "#ffb45f", "combined": "#ff7185"}


def fmt(value: Any, digits: int = 2) -> str:
    return f"{float(value):,.{digits}f}" if isinstance(value, (int, float)) else "—"


def value_at(results: dict[str, Any], count: int, mode: str, metric: str, divisor: float) -> float:
    value = results[str(count)][mode].get(metric)
    return float(value) / divisor if isinstance(value, (int, float)) else 0.0


def line_chart(results: dict[str, Any], metric: str, title: str, unit: str, *, divisor: float = 1.0, target: float | None = None) -> str:
    width, height = 760, 330
    left, top, plot_w, plot_h = 64, 28, 655, 232
    series = {mode: [value_at(results, count, mode, metric, divisor) for count in COUNTS] for mode in MODES}
    ceiling = max([value for values in series.values() for value in values] + ([target] if target else [1.0]))
    ceiling = max(1.0, ceiling * 1.12)
    xs = [left + index * plot_w / (len(COUNTS) - 1) for index in range(len(COUNTS))]
    grid = []
    for index in range(5):
        value = ceiling * index / 4
        y = top + plot_h - value / ceiling * plot_h
        grid.append(f'<line x1="{left}" x2="{left + plot_w}" y1="{y:.1f}" y2="{y:.1f}" class="grid"/><text x="{left - 8}" y="{y + 4:.1f}" text-anchor="end" class="axis">{value:.1f}</text>')
    target_markup = ""
    if target is not None and target <= ceiling:
        y = top + plot_h - target / ceiling * plot_h
        target_markup = f'<line x1="{left}" x2="{left + plot_w}" y1="{y:.1f}" y2="{y:.1f}" class="target"/><text x="{left + plot_w}" y="{y - 5:.1f}" text-anchor="end" class="axis">{target:g} target</text>'
    paths = []
    for mode, values in series.items():
        points, dots = [], []
        for x, value in zip(xs, values):
            y = top + plot_h - value / ceiling * plot_h
            points.append(f"{x:.1f},{y:.1f}")
            dots.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="4.5" fill="{COLORS[mode]}"/><text x="{x:.1f}" y="{y - 9:.1f}" text-anchor="middle" class="point">{value:.1f}</text>')
        paths.append(f'<polyline points="{" ".join(points)}" fill="none" stroke="{COLORS[mode]}" stroke-width="3"/>{"".join(dots)}')
    categories = "".join(f'<text x="{x:.1f}" y="{top + plot_h + 28}" text-anchor="middle" class="category">{count}</text>' for x, count in zip(xs, COUNTS))
    legend = "".join(f'<span><i style="background:{COLORS[mode]}"></i>{LABELS[mode]}</span>' for mode in MODES)
    return f'<section class="chart"><h3>{escape(title)}</h3><p>{escape(unit)}</p><svg viewBox="0 0 {width} {height}">{"".join(grid)}{target_markup}{"".join(paths)}{categories}</svg><div class="legend">{legend}</div></section>'


def rows(results: dict[str, Any]) -> str:
    output = []
    for count in COUNTS:
        for mode in MODES:
            item = results[str(count)][mode]
            validation = item["validation"]
            output.append(
                f'<tr><td>{count}</td><td>{LABELS[mode]}</td><td>{item["properties_per_tick"]}</td>'
                f'<td>{fmt(item["tick_throughput_hz"])}</td><td>{fmt(item["measurement_dropped_ticks"], 0)}</td>'
                f'<td>{fmt(item["submit_p95_ms"])}</td><td>{fmt(item["command_apply_p95_ms"])}</td>'
                f'<td>{fmt(item["frame_work_p95_ms"])}</td><td>{fmt(item["cpu_percent"], 1)}</td>'
                f'<td>{fmt(item["measurement_rss_peak_bytes"] / 2**20, 1)}</td><td>{fmt(item["text_measure_cache_misses"], 0)}</td>'
                f'<td>{fmt(item["text_measure_capacity_clears"], 0)}</td><td class="{"pass" if validation["passed"] else "fail"}">{"PASS" if validation["passed"] else "CAPACITY"}</td></tr>'
            )
    return "".join(output)


def render(summary: dict[str, Any], source_label: str) -> str:
    results = summary["results"]
    method = summary["method"]
    combined = results["320"]["combined"]
    labels = results["320"]["labels"]
    progress = results["320"]["progress"]
    leds = results["320"]["leds"]
    generated = datetime.now().astimezone().strftime("%Y-%m-%d %H:%M %Z")
    source_label = source_label.replace("\\", "/")
    return f'''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>DragonGUI Telemetry Indicator Decomposition</title><style>
:root{{--bg:#09101e;--panel:#121c32;--panel2:#172641;--text:#edf3ff;--muted:#9baccc;--line:#2c3e63;--blue:#8aa4ff;--green:#57d7a1;--orange:#ffb45f;--red:#ff7185}}*{{box-sizing:border-box}}body{{margin:0;background:radial-gradient(circle at 18% 0,#18335a 0,transparent 32%),var(--bg);color:var(--text);font:15px/1.55 Inter,Segoe UI,system-ui,sans-serif}}main{{max-width:1240px;margin:auto;padding:52px 28px 80px}}h1{{font-size:clamp(2.4rem,5vw,4.5rem);line-height:1.03;margin:.25rem 0 1rem;max-width:980px}}h2{{margin:48px 0 16px}}h3{{margin:0}}p{{color:var(--muted)}}.eyebrow{{color:var(--blue);font-weight:800;letter-spacing:.13em;text-transform:uppercase}}.lede{{font-size:1.16rem;max-width:900px}}.meta{{display:flex;gap:18px;flex-wrap:wrap;color:var(--muted);font-size:.85rem}}.kpis{{display:grid;grid-template-columns:repeat(4,1fr);gap:14px;margin:34px 0}}.kpi,.chart,.panel{{background:linear-gradient(145deg,var(--panel2),var(--panel));border:1px solid var(--line);border-radius:16px;box-shadow:0 18px 44px #02061166}}.kpi{{padding:20px}}.kpi b{{font-size:2rem;display:block}}.kpi span{{color:var(--muted);font-size:.82rem}}.callout{{padding:16px 18px;border-left:4px solid var(--red);background:#321b2a;border-radius:7px;color:#ffd3da}}.charts{{display:grid;grid-template-columns:1fr 1fr;gap:18px}}.chart{{padding:19px;overflow:hidden}}.chart p{{font-size:.82rem;margin:2px 0}}svg{{display:block;width:100%;height:auto}}.grid{{stroke:#2c3e63}}.axis,.category,.point{{fill:#aab9d7;font:12px Segoe UI,sans-serif}}.point{{font-size:10px;fill:#e4ebfb}}.target{{stroke:#9badcf;stroke-dasharray:6 6}}.legend{{display:flex;justify-content:center;gap:18px;color:var(--muted);font-size:.8rem}}.legend i{{display:inline-block;width:9px;height:9px;border-radius:50%;margin-right:6px}}.table-wrap{{padding:20px;overflow-x:auto}}table{{width:100%;border-collapse:collapse;min-width:1200px}}th,td{{padding:10px 8px;border-bottom:1px solid var(--line);text-align:right}}th{{color:var(--muted);font-size:.72rem;text-transform:uppercase}}th:nth-child(2),td:nth-child(2){{text-align:left}}.pass{{color:var(--green);font-weight:800}}.fail{{color:var(--red);font-weight:800}}.findings{{display:grid;grid-template-columns:1fr 1fr;gap:14px}}.finding{{padding:18px;border:1px solid var(--line);background:#101a2f;border-radius:12px}}.finding strong{{display:block;margin-bottom:6px}}code{{color:#cbd6ff}}footer{{margin-top:48px;color:var(--muted);font-size:.82rem}}@media(max-width:900px){{.charts,.findings{{grid-template-columns:1fr}}.kpis{{grid-template-columns:1fr 1fr}}}}@media(max-width:520px){{main{{padding:30px 14px}}.kpis{{grid-template-columns:1fr}}}}
</style></head><body><main><div class="eyebrow">DragonGUI bottleneck attribution</div><h1>Telemetry indicators: where the CPU and memory go</h1><p class="lede">Labels, progress bars, LEDs, and their combined row were isolated at 24, 72, 160, and 320 channels. Every active value updates at 30 Hz through one batched packet, with exact property accounting and final-state validation.</p><div class="meta"><span>Generated {escape(generated)}</span><span>{method["warmup_seconds"]:g} s warmup + {method["measure_seconds"]:g} s measurement</span><span>fresh process per case</span><span>16 samples</span></div>
<div class="kpis"><div class="kpi"><b>{fmt(labels["text_measure_cache_misses"],0)}</b><span>320-label text-measure misses</span></div><div class="kpi"><b>{fmt(leds["properties_per_tick"],0)}</b><span>properties/tick for 320 LEDs</span></div><div class="kpi"><b>{fmt(combined["command_drain_p95_ms"])} ms</b><span>combined command-drain p95</span></div><div class="kpi"><b>{fmt(combined["cpu_percent"],1)}%</b><span>combined CPU, one-core scale</span></div></div>
<div class="callout"><strong>The combined 320-channel case crossed the real-time boundary.</strong> It delivered {fmt(combined["tick_throughput_hz"])} Hz, coalesced {fmt(combined["measurement_dropped_ticks"],0)} measured generations, and reached {fmt(combined["command_drain_p95_ms"])} ms command-drain p95 against a 33.3 ms tick budget. Final state and queue drainage remained correct.</div>
<h2>Scaling curves</h2><div class="charts">{line_chart(results,"submit_p95_ms","Python update submission — p95","milliseconds")}{line_chart(results,"command_apply_p95_ms","Native property application — p95","milliseconds")}{line_chart(results,"command_drain_p95_ms","Complete native command drain — p95","milliseconds",target=33.333)}{line_chart(results,"frame_work_p95_ms","Native render work — p95","milliseconds")}{line_chart(results,"cpu_percent","Process CPU utilization","percent of one logical core")}{line_chart(results,"measurement_rss_peak_bytes","Peak measurement RSS","MiB",divisor=2**20)}{line_chart(results,"text_measure_cache_misses","Text-measurement cache misses","misses over the complete run")}{line_chart(results,"measurement_dropped_ticks","Missed/coalesced generations","measured ticks; lower is better")}</div>
<h2>Exact measurements</h2><section class="panel table-wrap"><table><thead><tr><th>Count</th><th>Mode</th><th>Props/tick</th><th>Hz</th><th>Dropped</th><th>Update p95</th><th>Apply p95</th><th>Frame p95</th><th>CPU %</th><th>Peak MiB</th><th>Text misses</th><th>Cache clears</th><th>Deadline</th></tr></thead><tbody>{rows(results)}</tbody></table></section>
<h2>Engineering conclusions</h2><div class="findings"><div class="finding"><strong>Fixed-width live labels still trigger measurement churn.</strong><span>At 320 labels, {fmt(labels["text_measure_cache_misses"],0)} distinct measurements and {fmt(labels["text_measure_capacity_clears"],0)} capacity clears drive native apply p95 to {fmt(labels["command_apply_p95_ms"])} ms. These labels are explicitly fixed-width and non-wrapping, so layout should be able to avoid most of this work.</span></div><div class="finding"><strong>LED updates double the property fan-out.</strong><span>Calling <code>set_on()</code> for 320 LEDs creates {fmt(leds["properties_per_tick"],0)} properties per frame because both state and color are submitted. Deduplicating unchanged states and combining LED state/color application are high-confidence targets.</span></div><div class="finding"><strong>Progress paint is not free, but it is not the first target.</strong><span>At 320 channels, progress-only application p95 is {fmt(progress["command_apply_p95_ms"])} ms and CPU is {fmt(progress["cpu_percent"],1)}%. It matters after label invalidation and LED fan-out are fixed.</span></div><div class="finding"><strong>The GPU renderer is not the limiter.</strong><span>Combined stage frame-work p95 is only {fmt(combined["frame_work_p95_ms"])} ms. The overload occurs earlier in property application, text/layout invalidation, and command draining.</span></div></div>
<h2>Recommended optimization order</h2><section class="panel" style="padding:22px"><ol><li>Add a fixed-geometry text update path: changing non-wrapping text inside an explicitly sized label should rebuild glyph content without invalidating layout or populating the layout-measure cache.</li><li>Make <code>LED.set_state()</code>/<code>set_on()</code> a no-op when the effective state and color do not change; send one composite update when they do.</li><li>Profile native property-packet application by property family, then reduce repeated lookup/dirty propagation across hundreds of sibling targets.</li><li>Re-run this exact matrix, then the full three-framework telemetry report, after each optimization.</li></ol></section><footer>Sources: <code>benchmarks/gui_telemetry_indicator_case.py</code>, <code>benchmarks/run_gui_telemetry_indicator_matrix.py</code>, and <a href="../{escape(source_label)}/summary.json">validated summary JSON</a>.</footer></main></body></html>'''


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--summary", type=Path, default=DEFAULT_SUMMARY)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    summary = json.loads(args.summary.read_text(encoding="utf-8"))
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(render(summary, str(args.summary.resolve().parent.relative_to(ROOT))), encoding="utf-8")
    print(f"Wrote {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
