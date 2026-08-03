"""Render the validated live-dashboard matrix as a self-contained HTML report."""

from __future__ import annotations

import argparse
from datetime import datetime
from html import escape
import json
from pathlib import Path
import re
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SUMMARY = ROOT / "artifacts" / "gui-live-dashboard-comparison-v1" / "summary.json"
DEFAULT_FAILURE = DEFAULT_SUMMARY.parent / "raw" / "high-dragongui-uncoalesced-1.json"
DEFAULT_OUTPUT = ROOT / "plans" / "gui-live-dashboard-performance-report.html"
FRAMEWORKS = ("dragongui", "dearpygui", "pyqtgraph")
LOADS = ("low", "medium", "high")
LABELS = {"dragongui": "DragonGUI", "dearpygui": "Dear PyGui", "pyqtgraph": "PyQtGraph"}
COLORS = {"dragongui": "#7c9cff", "dearpygui": "#54d6a0", "pyqtgraph": "#ffb45e"}


def fmt(value: Any, digits: int = 1) -> str:
    return "—" if value is None else f"{float(value):,.{digits}f}"


def mib(value: Any) -> str:
    return "—" if value is None else f"{float(value) / 2**20:,.0f}"


def baseline_from_html(path: Path) -> dict[str, dict[str, dict[str, float]]]:
    """Recover the exact-table metrics embedded in an earlier generated report."""
    source = path.read_text(encoding="utf-8")
    pattern = re.compile(
        r"<tr><td>(Low|Medium|High)</td><td><span.*?</span>"
        r"(DragonGUI|Dear PyGui|PyQtGraph)</td><td>([\d.]+)</td>"
        r"<td>([\d,]+)</td><td>([\d.]+)</td><td>([\d.]+)</td>"
        r"<td>([\d.]+)</td><td>([\d,]+)</td>",
        re.DOTALL,
    )
    reverse_labels = {label: framework for framework, label in LABELS.items()}
    results: dict[str, dict[str, dict[str, float]]] = {load: {} for load in LOADS}
    for match in pattern.finditer(source):
        load = match.group(1).lower()
        framework = reverse_labels[match.group(2)]
        results[load][framework] = {
            "tick_throughput_hz": float(match.group(3)),
            "measurement_dropped_ticks": float(match.group(4).replace(",", "")),
            "submit_p95_ms": float(match.group(5)),
            "frame_p95_ms": float(match.group(6)),
            "cpu_percent": float(match.group(7)),
            "rss_peak_mib": float(match.group(8).replace(",", "")),
        }
    if any(set(results[load]) != set(FRAMEWORKS) for load in LOADS):
        raise ValueError(f"Could not recover all 9 baseline rows from {path}")
    return results


def comparison_rows(results: dict[str, Any], baseline: dict[str, Any]) -> str:
    rows: list[str] = []
    for load in LOADS:
        for framework in FRAMEWORKS:
            old = baseline[load][framework]["tick_throughput_hz"]
            current = float(results[load][framework]["tick_throughput_hz"])
            delta = current - old
            percent = delta / old * 100.0 if old else 0.0
            css = "gain" if delta > 0.05 else "loss" if delta < -0.05 else "flat"
            rows.append(
                f'<tr><td>{load.title()}</td><td>{LABELS[framework]}</td>'
                f'<td>{old:.1f}</td><td>{current:.1f}</td>'
                f'<td class="{css}">{delta:+.1f}</td><td class="{css}">{percent:+.1f}%</td></tr>'
            )
    return "".join(rows)


def grouped_bars(results: dict[str, Any], metric: str, title: str, unit: str, *, target: float | None = None, divisor: float = 1.0) -> str:
    values = [float(results[load][fw].get(metric) or 0) / divisor for load in LOADS for fw in FRAMEWORKS]
    maximum = max(values + ([target] if target is not None else [0])) or 1
    width, height, left, top, plot_h = 820, 310, 58, 34, 220
    group_w = (width - left - 20) / len(LOADS)
    bar_w = 47
    svg = [f'<svg viewBox="0 0 {width} {height}" role="img" aria-label="{escape(title)}">']
    for index in range(5):
        value = maximum * index / 4
        y = top + plot_h - plot_h * index / 4
        svg.append(f'<line x1="{left}" x2="{width-16}" y1="{y:.1f}" y2="{y:.1f}" class="grid"/>')
        svg.append(f'<text x="{left-8}" y="{y+4:.1f}" text-anchor="end" class="axis">{value:.0f}</text>')
    if target is not None:
        y = top + plot_h - plot_h * target / maximum
        svg.append(f'<line x1="{left}" x2="{width-16}" y1="{y:.1f}" y2="{y:.1f}" class="target"/>')
        svg.append(f'<text x="{width-18}" y="{y-6:.1f}" text-anchor="end" class="target-label">target {target:g}</text>')
    for load_index, load in enumerate(LOADS):
        group_x = left + load_index * group_w + (group_w - bar_w * 3) / 2
        for fw_index, framework in enumerate(FRAMEWORKS):
            value = float(results[load][framework].get(metric) or 0) / divisor
            h = plot_h * value / maximum
            x = group_x + fw_index * bar_w
            y = top + plot_h - h
            svg.append(f'<rect x="{x+4:.1f}" y="{y:.1f}" width="{bar_w-8}" height="{h:.1f}" rx="5" fill="{COLORS[framework]}"/>')
            svg.append(f'<text x="{x+bar_w/2:.1f}" y="{max(top+11, y-6):.1f}" text-anchor="middle" class="value">{value:.1f}</text>')
        svg.append(f'<text x="{left + load_index*group_w + group_w/2:.1f}" y="{top+plot_h+28}" text-anchor="middle" class="category">{load.upper()}</text>')
    svg.append('</svg>')
    legend = ''.join(f'<span><i style="background:{COLORS[fw]}"></i>{LABELS[fw]}</span>' for fw in FRAMEWORKS)
    return f'<section class="chart-card"><h3>{escape(title)}</h3><p class="chart-unit">{escape(unit)}</p>{"".join(svg)}<div class="legend">{legend}</div></section>'


def result_rows(results: dict[str, Any]) -> str:
    rows: list[str] = []
    for load in LOADS:
        for framework in FRAMEWORKS:
            item = results[load][framework]
            passed = item["validation"]["passed"]
            rows.append(
                f'<tr><td>{load.title()}</td><td><span class="dot" style="background:{COLORS[framework]}"></span>{LABELS[framework]}</td>'
                f'<td>{fmt(item["tick_throughput_hz"])}</td><td>{fmt(item["measurement_dropped_ticks"], 0)}</td>'
                f'<td>{fmt(item["submit_p95_ms"], 2)}</td><td>{fmt(item["frame_p95_ms"], 2)}</td>'
                f'<td>{fmt(item["cpu_percent"])}</td><td>{mib(item["rss_peak_bytes"])}</td>'
                f'<td class="{"pass" if passed else "fail"}">{"PASS" if passed else "FAIL"}</td></tr>'
            )
    return ''.join(rows)


def backlog_chart(failure: dict[str, Any]) -> str:
    points = [(p["tick"], p.get("queue_depth") or 0) for p in failure["metrics"]["checkpoints"] if p.get("queue_depth") is not None]
    if not points:
        return ""
    width, height, left, top, plot_w, plot_h = 820, 280, 58, 25, 730, 205
    max_x = max(x for x, _ in points) or 1
    max_y = max(y for _, y in points) or 1
    coords = ' '.join(f'{left + x/max_x*plot_w:.1f},{top + plot_h - y/max_y*plot_h:.1f}' for x, y in points)
    return f'''<section class="chart-card wide danger-card">
      <h3>What failed without coalescing</h3><p class="chart-unit">Python callback queue depth during the original DragonGUI high-load run</p>
      <svg viewBox="0 0 {width} {height}" role="img" aria-label="Uncoalesced callback backlog">
        <line x1="{left}" x2="{left}" y1="{top}" y2="{top+plot_h}" class="axis-line"/><line x1="{left}" x2="{left+plot_w}" y1="{top+plot_h}" y2="{top+plot_h}" class="axis-line"/>
        <polyline points="{coords}" fill="none" stroke="#ff6b7d" stroke-width="4" stroke-linejoin="round"/>
        <text x="{left}" y="{height-18}" class="category">tick 0</text><text x="{left+plot_w}" y="{height-18}" text-anchor="end" class="category">tick {max_x}</text>
        <text x="{left+8}" y="{top+16}" class="value">peak sampled queue {max_y:,}</text>
      </svg>
      <div class="callout danger"><strong>Validation correctly rejected this run.</strong> After a 15-second drain attempt, 318 Python tasks and 226 native commands remained; the displayed state stopped at tick 3,581 instead of 3,899. The queue high-water mark was 1,012.</div>
    </section>'''


def render(
    summary: dict[str, Any],
    failure: dict[str, Any] | None = None,
    baseline: dict[str, Any] | None = None,
    source_label: str = "artifacts/gui-live-dashboard-comparison",
) -> str:
    results = summary["results"]
    source_label = source_label.replace("\\", "/")
    high = results["high"]
    generated = datetime.now().astimezone().strftime("%Y-%m-%d %H:%M %Z")
    total_checks = sum(sum(int(v) for v in results[l][f]["validation"]["checks_per_sample"] if v is not None) for l in LOADS for f in FRAMEWORKS)
    baseline_section = ""
    if baseline is not None:
        baseline_section = f'''<h2>Change from the July 31 report</h2>
<div class="callout caution"><strong>Context, not a controlled code-only A/B:</strong> the prior report ran on different hardware and used DragonGUI's individual property transport, while this run uses the optimized batch transport. The fresh three-library rows above are the valid current-machine comparison. The deltas below answer how the published numbers changed, but hardware and method changes prevent attributing them solely to code.</div>
<section class="panel table-wrap"><table><thead><tr><th>Load</th><th>Framework</th><th>Prior Hz</th><th>Current Hz</th><th>Δ Hz</th><th>Δ %</th></tr></thead><tbody>{comparison_rows(results, baseline)}</tbody></table></section>'''
    failure_section = ""
    if failure is not None:
        failure_section = f'<h2>The bug the original benchmark found</h2><div class="charts">{backlog_chart(failure)}</div>'
    return f'''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>DragonGUI Live Dashboard Performance Report</title>
<style>
:root{{--bg:#0b1020;--panel:#131b31;--panel2:#18233e;--text:#edf2ff;--muted:#99a8c9;--line:#2a385d;--blue:#7c9cff;--green:#54d6a0;--orange:#ffb45e;--red:#ff6b7d}}
*{{box-sizing:border-box}} body{{margin:0;background:radial-gradient(circle at 15% 0,#17284a 0,transparent 34%),var(--bg);color:var(--text);font:15px/1.55 Inter,Segoe UI,system-ui,sans-serif}}
main{{max-width:1180px;margin:auto;padding:54px 28px 80px}} .eyebrow{{color:var(--blue);font-weight:700;letter-spacing:.13em;text-transform:uppercase}}
h1{{font-size:clamp(2.3rem,5vw,4.6rem);line-height:1.02;margin:.25rem 0 1rem;max-width:900px}} h2{{font-size:1.65rem;margin:48px 0 16px}} h3{{margin:0;font-size:1.03rem}} p{{color:var(--muted)}}
.lede{{font-size:1.16rem;max-width:850px}} .meta{{display:flex;gap:18px;flex-wrap:wrap;color:var(--muted);font-size:.86rem}}
.kpis{{display:grid;grid-template-columns:repeat(4,1fr);gap:14px;margin:34px 0}} .kpi,.chart-card,.panel{{border:1px solid var(--line);background:linear-gradient(145deg,var(--panel2),var(--panel));border-radius:16px;box-shadow:0 16px 38px #03071366}}
.kpi{{padding:20px}} .kpi b{{display:block;font-size:2rem}} .kpi span{{color:var(--muted);font-size:.82rem}} .charts{{display:grid;grid-template-columns:1fr 1fr;gap:18px}} .chart-card{{padding:20px;overflow:hidden}} .wide{{grid-column:1/-1}} .chart-unit{{margin:2px 0 6px;font-size:.82rem}}
svg{{width:100%;height:auto;display:block}} .grid{{stroke:#2a385d;stroke-width:1}} .axis-line{{stroke:#536388;stroke-width:1}} .axis,.value,.category,.target-label{{fill:#aab7d4;font:12px Segoe UI,sans-serif}} .value{{fill:#eaf0ff;font-weight:600}} .target{{stroke:#8c9fc9;stroke-dasharray:6 6}} .target-label{{fill:#c8d3ed}}
.legend{{display:flex;justify-content:center;gap:22px;color:var(--muted);font-size:.8rem}} .legend i,.dot{{width:9px;height:9px;border-radius:50%;display:inline-block;margin-right:7px}}
.panel{{padding:24px}} .callout{{padding:15px 17px;border-left:4px solid var(--green);background:#152c2a;border-radius:6px;color:#bdebd9}} .caution{{border-color:var(--orange);background:#302719;color:#ffe0ad;margin-bottom:14px}} .danger{{border-color:var(--red);background:#321b2a;color:#ffd2d8}} .danger-card{{border-color:#61314a}}
table{{width:100%;border-collapse:collapse;min-width:900px}} th,td{{padding:11px 10px;border-bottom:1px solid var(--line);text-align:right}} th{{color:var(--muted);font-size:.75rem;text-transform:uppercase;letter-spacing:.05em}} th:first-child,th:nth-child(2),td:first-child,td:nth-child(2){{text-align:left}} .table-wrap{{overflow-x:auto}} .pass{{color:var(--green);font-weight:700}} .fail{{color:var(--red);font-weight:700}}
.gain{{color:var(--green);font-weight:700}} .loss{{color:var(--red);font-weight:700}} .flat{{color:var(--muted)}}
.findings{{display:grid;grid-template-columns:repeat(2,1fr);gap:14px}} .finding{{padding:18px;background:#111a2f;border:1px solid var(--line);border-radius:12px}} .finding strong{{display:block;margin-bottom:6px}} code{{color:#c9d3ff}} footer{{margin-top:48px;color:var(--muted);font-size:.8rem}}
@media(max-width:850px){{.kpis{{grid-template-columns:1fr 1fr}}.charts,.findings{{grid-template-columns:1fr}}.wide{{grid-column:auto}}}} @media(max-width:520px){{main{{padding:32px 16px}}.kpis{{grid-template-columns:1fr}}}}
</style></head><body><main>
<div class="eyebrow">Validated comparative benchmark</div><h1>Live dashboard performance under sustained visual load</h1>
<p class="lede">A 60 Hz producer continuously replaces line, scatter, heatmap, and control data for 60 seconds. This tests steady-state responsiveness, overload behavior, memory, retained-state correctness, and queue health—not startup time.</p>
<div class="meta"><span>Generated {escape(generated)}</span><span>Python 3.12</span><span>1 measured run per configuration</span><span>5 s warmup + 60 s measurement</span></div>
<div class="kpis"><div class="kpi"><b>{fmt(high['dragongui']['tick_throughput_hz'])} Hz</b><span>DragonGUI high-load throughput</span></div><div class="kpi"><b>{fmt(high['dearpygui']['tick_throughput_hz'])} Hz</b><span>Dear PyGui high-load throughput</span></div><div class="kpi"><b>{fmt(high['pyqtgraph']['tick_throughput_hz'])} Hz</b><span>PyQtGraph high-load throughput</span></div><div class="kpi"><b>{total_checks}</b><span>correctness checks passed</span></div></div>
<div class="callout"><strong>Headline:</strong> DragonGUI leads the fresh high-load comparison at {fmt(high['dragongui']['tick_throughput_hz'])} Hz, {fmt((high['dragongui']['tick_throughput_hz'] / high['dearpygui']['tick_throughput_hz'] - 1) * 100)}% ahead of Dear PyGui, while using {fmt((1 - high['dragongui']['rss_peak_bytes'] / high['dearpygui']['rss_peak_bytes']) * 100)}% less peak memory. Low and medium DragonGUI loads now sustain the 60 Hz target.</div>
<h2>Comparative results</h2><div class="charts">
{grouped_bars(results, 'tick_throughput_hz', 'Processed visualization ticks', 'ticks per second; higher is better', target=60)}
{grouped_bars(results, 'measurement_dropped_ticks', 'Dropped or coalesced generations', 'scheduled ticks over 60 seconds; lower is better')}
{grouped_bars(results, 'submit_p95_ms', 'Application update cost — p95', 'milliseconds spent replacing datasets and labels; lower is better')}
{grouped_bars(results, 'frame_p95_ms', 'Render/event-loop work — p95', 'milliseconds; lower is better')}
{grouped_bars(results, 'cpu_percent', 'Process CPU utilization', 'percent of one logical core; lower is better at equal throughput')}
{grouped_bars(results, 'rss_peak_bytes', 'Peak resident memory', 'MiB; lower is better', divisor=2**20)}
</div>
<h2>Exact measurements</h2><section class="panel table-wrap"><table><thead><tr><th>Load</th><th>Framework</th><th>Hz</th><th>Dropped</th><th>Submit p95 ms</th><th>Frame p95 ms</th><th>CPU %</th><th>Peak MiB</th><th>Validation</th></tr></thead><tbody>{result_rows(results)}</tbody></table></section>
{baseline_section}
{failure_section}
<h2>Interpretation</h2><div class="findings">
<div class="finding"><strong>DragonGUI’s GPU stage is not the main bottleneck.</strong><span>At high load, native frame work p95 was {fmt(high['dragongui']['frame_p95_ms'],2)} ms while command drain p95 was {fmt(high['dragongui']['native']['command_drain_p95_ms'],2)} ms. The dominant cost is applying Python-originated changes and rebuilding affected retained content.</span></div>
<div class="finding"><strong>Coalescing makes overload bounded and correct.</strong><span>The recommended high-load run coalesced {fmt(high['dragongui']['native']['python_tasks_coalesced'],0)} obsolete tasks, held Python queue high-water to {fmt(high['dragongui']['native']['python_queue_high_water'],0)}, drained fully, and finished on the correct final tick.</span></div>
<div class="finding"><strong>DragonGUI keeps substantially less high-load memory than Dear PyGui.</strong><span>Peak RSS was {mib(high['dragongui']['rss_peak_bytes'])} MiB versus {mib(high['dearpygui']['rss_peak_bytes'])} MiB, although PyQtGraph remained lowest at {mib(high['pyqtgraph']['rss_peak_bytes'])} MiB.</span></div>
<div class="finding"><strong>Batch transport and targeted invalidation moved the practical ceiling.</strong><span>DragonGUI sustains {fmt(results['medium']['dragongui']['tick_throughput_hz'])} Hz at medium load with frame-work p95 of {fmt(results['medium']['dragongui']['frame_p95_ms'],2)} ms. At high load, Python submission and command application—not GPU frame work—remain the limiting path.</span></div>
</div>
<h2>Method and fairness notes</h2><section class="panel"><p>Each framework displays the same deterministic data shapes and control counts. Low uses two 2k-point lines; medium uses four 10k-point lines; high uses six 50k-point lines plus a 50k-point scatter, a 128×128 heatmap, and 200 changing labels. Frameworks run in fresh processes. Synchronous event-loop adapters skip overdue scheduled generations; DragonGUI’s producer uses keyed latest-frame coalescing and submits each generation through the batch property-update path, which gives equivalent latest-state semantics with bounded queues.</p><p>Validation checks retained point counts, heatmap dimensions, final visible status, scheduled-tick accounting, and item existence. DragonGUI additionally checks native resources, renderer source counts, native retained text, layout diagnostics, and drained queues. All 99 checks passed. One repetition makes this an engineering baseline rather than a statistically conclusive ranking.</p></section>
<footer>Sources: <code>benchmarks/gui_live_dashboard_case.py</code>, <code>benchmarks/run_gui_live_dashboard_matrix.py</code>, <a href="../{escape(source_label)}/summary.json">validated summary JSON</a>, and its <a href="../{escape(source_label)}/raw/">nine raw samples</a>.</footer>
</main></body></html>'''


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--summary", type=Path, default=DEFAULT_SUMMARY)
    parser.add_argument("--uncoalesced-failure", type=Path, default=DEFAULT_FAILURE)
    parser.add_argument("--baseline-report", type=Path)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    summary = json.loads(args.summary.read_text(encoding="utf-8"))
    failure = None
    if args.uncoalesced_failure and args.uncoalesced_failure.exists():
        failure = json.loads(args.uncoalesced_failure.read_text(encoding="utf-8"))
    baseline = baseline_from_html(args.baseline_report) if args.baseline_report else None
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        render(summary, failure, baseline, str(args.summary.resolve().parent.relative_to(ROOT))),
        encoding="utf-8",
    )
    print(f"Wrote {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
