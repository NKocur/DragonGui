"""Render the validated telemetry-viewer comparison summary as standalone HTML."""

from __future__ import annotations

import argparse
from datetime import datetime
from html import escape
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SUMMARY = ROOT / "artifacts" / "gui-telemetry-viewer-comparison" / "summary.json"
DEFAULT_OUTPUT = ROOT / "plans" / "gui-telemetry-viewer-performance-report.html"
STAGES = ("stage1", "stage2", "stage3", "stage4")
FRAMEWORKS = ("dragongui", "dearpygui", "pyqtgraph")
LABELS = {"dragongui": "DragonGUI", "dearpygui": "Dear PyGui", "pyqtgraph": "PyQtGraph"}
COLORS = {"dragongui": "#8aa4ff", "dearpygui": "#57d7a1", "pyqtgraph": "#ffb45f"}


def fmt(value: Any, digits: int = 2) -> str:
    if not isinstance(value, (int, float)):
        return "—"
    return f"{value:,.{digits}f}"


def mib(value: Any) -> str:
    return fmt(float(value) / 2**20, 1) if isinstance(value, (int, float)) else "—"


def _metric(results: dict[str, Any], stage: str, framework: str, metric: str) -> float:
    value = results[stage][framework].get(metric)
    return float(value) if isinstance(value, (int, float)) else 0.0


def line_chart(
    results: dict[str, Any],
    metric: str,
    title: str,
    unit: str,
    *,
    divisor: float = 1.0,
    target: float | None = None,
) -> str:
    width, height = 760, 330
    left, top, plot_w, plot_h = 62, 28, 660, 235
    series = {
        framework: [_metric(results, stage, framework, metric) / divisor for stage in STAGES]
        for framework in FRAMEWORKS
    }
    ceiling = max([value for values in series.values() for value in values] + ([target] if target else [1.0]))
    ceiling = max(1.0, ceiling * 1.12)
    xs = [left + index * plot_w / (len(STAGES) - 1) for index in range(len(STAGES))]
    grid = []
    for index in range(5):
        value = ceiling * index / 4
        y = top + plot_h - value / ceiling * plot_h
        grid.append(f'<line x1="{left}" x2="{left + plot_w}" y1="{y:.1f}" y2="{y:.1f}" class="grid"/><text x="{left - 9}" y="{y + 4:.1f}" text-anchor="end" class="axis">{value:.1f}</text>')
    target_line = ""
    if target is not None and target <= ceiling:
        y = top + plot_h - target / ceiling * plot_h
        target_line = f'<line x1="{left}" x2="{left + plot_w}" y1="{y:.1f}" y2="{y:.1f}" class="target"/><text x="{left + plot_w - 3}" y="{y - 6:.1f}" text-anchor="end" class="target-label">target {target:g}</text>'
    paths = []
    for framework, values in series.items():
        coords = []
        dots = []
        for x, value in zip(xs, values):
            y = top + plot_h - value / ceiling * plot_h
            coords.append(f"{x:.1f},{y:.1f}")
            dots.append(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="4.5" fill="{COLORS[framework]}"/><text x="{x:.1f}" y="{y - 9:.1f}" text-anchor="middle" class="point-value">{value:.1f}</text>')
        paths.append(f'<polyline points="{" ".join(coords)}" fill="none" stroke="{COLORS[framework]}" stroke-width="3"/>{"".join(dots)}')
    stage_labels = []
    for x, stage in zip(xs, STAGES):
        config = results[stage]["dragongui"]["config"]
        stage_labels.append(f'<text x="{x:.1f}" y="{top + plot_h + 24}" text-anchor="middle" class="category">{config["line_plots"]} plots</text><text x="{x:.1f}" y="{top + plot_h + 40}" text-anchor="middle" class="category faint">{config["indicators"]} indicators</text>')
    legend = "".join(f'<span><i style="background:{COLORS[f]}"></i>{LABELS[f]}</span>' for f in FRAMEWORKS)
    return f'''<section class="chart-card"><h3>{escape(title)}</h3><p>{escape(unit)}</p><svg viewBox="0 0 {width} {height}" role="img" aria-label="{escape(title)}">{"".join(grid)}{target_line}{"".join(paths)}{"".join(stage_labels)}</svg><div class="legend">{legend}</div></section>'''


def result_rows(results: dict[str, Any]) -> str:
    rows: list[str] = []
    for stage in STAGES:
        for framework in FRAMEWORKS:
            item = results[stage][framework]
            config = item["config"]
            passed = item["validation"]["passed"]
            rows.append(
                f'<tr><td>{stage[-1]}</td><td>{config["line_plots"]}</td><td>{config["indicators"]}</td><td>{LABELS[framework]}</td>'
                f'<td>{fmt(item.get("tick_throughput_hz"))}</td><td>{fmt(item.get("measurement_dropped_ticks"), 0)}</td>'
                f'<td>{fmt(item.get("submit_p95_ms"))}</td><td>{fmt(item.get("frame_p95_ms"))}</td>'
                f'<td>{fmt(item.get("cpu_percent"), 1)}</td><td>{mib(item.get("measurement_rss_peak_bytes") or item.get("rss_peak_bytes"))}</td>'
                f'<td>{mib(item.get("measurement_private_peak_bytes"))}</td>'
                f'<td class="{"pass" if passed else "fail"}">{"PASS" if passed else "FAIL"}</td></tr>'
            )
    return "".join(rows)


def dragon_before_after_rows(results: dict[str, Any], previous: dict[str, Any]) -> str:
    rows: list[str] = []
    old_results = previous["results"]
    for stage in STAGES:
        old = old_results[stage]["dragongui"]
        new = results[stage]["dragongui"]
        private_delta = (
            float(new["measurement_private_peak_bytes"])
            - float(old["measurement_private_peak_bytes"])
        )
        rows.append(
            f'<tr><td>{stage[-1]}</td><td>{fmt(old.get("tick_throughput_hz"))} → {fmt(new.get("tick_throughput_hz"))}</td>'
            f'<td>{fmt(old.get("cpu_percent"), 1)} → {fmt(new.get("cpu_percent"), 1)}</td>'
            f'<td>{fmt(old.get("submit_p95_ms"))} → {fmt(new.get("submit_p95_ms"))}</td>'
            f'<td>{mib(old.get("measurement_rss_peak_bytes"))} → {mib(new.get("measurement_rss_peak_bytes"))}</td>'
            f'<td>{mib(old.get("measurement_private_peak_bytes"))} → {mib(new.get("measurement_private_peak_bytes"))}</td>'
            f'<td class="{"pass" if private_delta <= 0 else "fail"}">{private_delta / 2**20:+.1f} MiB</td></tr>'
        )
    return "".join(rows)


def render(
    summary: dict[str, Any],
    source_label: str,
    previous: dict[str, Any] | None = None,
) -> str:
    results = summary["results"]
    method = summary["method"]
    generated = datetime.now().astimezone().strftime("%Y-%m-%d %H:%M %Z")
    all_valid = all(results[s][f]["validation"]["passed"] for s in STAGES for f in FRAMEWORKS)
    total_checks = sum(sum(int(value or 0) for value in results[s][f]["validation"]["checks_per_sample"]) for s in STAGES for f in FRAMEWORKS)
    high = results["stage4"]
    best_submit = min(FRAMEWORKS, key=lambda f: high[f]["submit_p95_ms"])
    best_frame = min(FRAMEWORKS, key=lambda f: high[f]["frame_p95_ms"])
    dg_low = results["stage1"]["dragongui"]
    dg_high = results["stage4"]["dragongui"]
    dg_submit_scale = dg_high["submit_p95_ms"] / max(dg_low["submit_p95_ms"], 1e-9)
    dg_frame_scale = dg_high["frame_p95_ms"] / max(dg_low["frame_p95_ms"], 1e-9)
    before_after = ""
    private_kpi = mib(high["dragongui"].get("measurement_private_peak_bytes"))
    if previous is not None:
        old_high = previous["results"]["stage4"]["dragongui"]
        private_reduction = (
            float(old_high["measurement_private_peak_bytes"])
            - float(high["dragongui"]["measurement_private_peak_bytes"])
        ) / 2**20
        before_after = f'''<h2>DragonGUI progress since the previous report</h2><div class="callout"><strong>Stage 4 private commit:</strong> {mib(old_high.get("measurement_private_peak_bytes"))} → {private_kpi} MiB, a reduction of {private_reduction:.1f} MiB. The table keeps throughput, CPU, update latency, RSS, and private commit together so a memory win cannot conceal a responsiveness regression.</div><section class="panel table-wrap"><table><thead><tr><th>Stage</th><th>Hz before → now</th><th>CPU % before → now</th><th>Update p95 before → now</th><th>RSS MiB before → now</th><th>Private MiB before → now</th><th>Private delta</th></tr></thead><tbody>{dragon_before_after_rows(results, previous)}</tbody></table></section>'''
    source_label = source_label.replace("\\", "/")
    return f'''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Live Telemetry Viewer Framework Benchmark</title><style>
:root{{--bg:#09101e;--panel:#121c32;--panel2:#172641;--text:#edf3ff;--muted:#9baccc;--line:#2c3e63;--blue:#8aa4ff;--green:#57d7a1;--orange:#ffb45f;--red:#ff7185}}
*{{box-sizing:border-box}}body{{margin:0;background:radial-gradient(circle at 18% 0,#18335a 0,transparent 32%),var(--bg);color:var(--text);font:15px/1.55 Inter,Segoe UI,system-ui,sans-serif}}main{{max-width:1240px;margin:auto;padding:52px 28px 80px}}h1{{font-size:clamp(2.4rem,5vw,4.6rem);line-height:1.02;margin:.25rem 0 1rem;max-width:980px}}h2{{margin:48px 0 16px}}h3{{margin:0}}p{{color:var(--muted)}}.eyebrow{{color:var(--blue);font-weight:800;letter-spacing:.13em;text-transform:uppercase}}.lede{{font-size:1.16rem;max-width:920px}}.meta{{display:flex;gap:18px;flex-wrap:wrap;color:var(--muted);font-size:.85rem}}.kpis{{display:grid;grid-template-columns:repeat(4,1fr);gap:14px;margin:34px 0}}.kpi,.chart-card,.panel{{background:linear-gradient(145deg,var(--panel2),var(--panel));border:1px solid var(--line);border-radius:16px;box-shadow:0 18px 44px #02061166}}.kpi{{padding:20px}}.kpi b{{font-size:2rem;display:block}}.kpi span{{color:var(--muted);font-size:.82rem}}.callout{{padding:16px 18px;border-left:4px solid var(--green);background:#132d2b;border-radius:7px;color:#c4f1df}}.charts{{display:grid;grid-template-columns:1fr 1fr;gap:18px}}.chart-card{{padding:19px;overflow:hidden}}.chart-card p{{margin:2px 0;font-size:.82rem}}svg{{display:block;width:100%;height:auto}}.grid{{stroke:#2c3e63;stroke-width:1}}.axis,.category,.target-label,.point-value{{fill:#aab9d7;font:12px Segoe UI,sans-serif}}.faint{{fill:#7587aa}}.point-value{{font-size:10px;fill:#dae3f7}}.target{{stroke:#8da0c8;stroke-dasharray:6 6}}.legend{{display:flex;justify-content:center;gap:22px;color:var(--muted);font-size:.8rem}}.legend i{{display:inline-block;width:9px;height:9px;border-radius:50%;margin-right:7px}}.table-wrap{{overflow-x:auto;padding:20px}}table{{width:100%;border-collapse:collapse;min-width:1050px}}th,td{{padding:10px;border-bottom:1px solid var(--line);text-align:right}}th{{color:var(--muted);font-size:.74rem;text-transform:uppercase}}th:nth-child(4),td:nth-child(4){{text-align:left}}.pass{{color:var(--green);font-weight:800}}.fail{{color:var(--red);font-weight:800}}.findings{{display:grid;grid-template-columns:1fr 1fr;gap:14px}}.finding{{padding:18px;border:1px solid var(--line);background:#101a2f;border-radius:12px}}.finding strong{{display:block;margin-bottom:6px}}code{{color:#cbd6ff}}footer{{margin-top:48px;color:var(--muted);font-size:.82rem}}@media(max-width:900px){{.charts,.findings{{grid-template-columns:1fr}}.kpis{{grid-template-columns:1fr 1fr}}}}@media(max-width:520px){{main{{padding:30px 14px}}.kpis{{grid-template-columns:1fr}}}}
</style></head><body><main><div class="eyebrow">Validated staged comparison</div><h1>Can DragonGUI carry a mission-control telemetry wall?</h1><p class="lede">A COSMOS-style workload replaces every line trace, numeric value, progress channel, and status light at 30 Hz. Four stages increase retained UI complexity from 4 plots and 24 indicators to 16 plots and 320 indicators while holding each trace at 1,024 samples.</p><div class="meta"><span>Generated {escape(generated)}</span><span>Python 3.12</span><span>{method["warmup_seconds"]:g} s warmup + {method["measure_seconds"]:g} s measurement</span><span>fresh process per sample</span></div>
<div class="kpis"><div class="kpi"><b>{fmt(high["dragongui"]["tick_throughput_hz"])} Hz</b><span>DragonGUI stage 4 throughput</span></div><div class="kpi"><b>{fmt(high["dragongui"]["submit_p95_ms"])} ms</b><span>DragonGUI stage 4 update p95</span></div><div class="kpi"><b>{private_kpi} MiB</b><span>DragonGUI stage 4 private commit</span></div><div class="kpi"><b>{total_checks}</b><span>correctness checks · {"all passed" if all_valid else "failures present"}</span></div></div>
<div class="callout"><strong>Stage 4:</strong> {LABELS[best_submit]} has the lowest application update p95 ({fmt(high[best_submit]["submit_p95_ms"])} ms), while {LABELS[best_frame]} has the lowest measured frame/event-loop p95 ({fmt(high[best_frame]["frame_p95_ms"])} ms). DragonGUI's update cost scales {dg_submit_scale:.2f}× and native frame work {dg_frame_scale:.2f}× from stage 1 to stage 4.</div>
{before_after}
<h2>Scaling curves</h2><div class="charts">
{line_chart(results,"tick_throughput_hz","Delivered telemetry generations","ticks per second; 30 is the requested cadence",target=30)}
{line_chart(results,"measurement_dropped_ticks","Dropped scheduled generations","ticks during the measured interval; lower is better")}
{line_chart(results,"submit_p95_ms","Application update cost — p95","milliseconds replacing all traces and indicator properties")}
{line_chart(results,"frame_p95_ms","Frame/event-loop work — p95","milliseconds; lower is better, 33.3 ms is the 30 Hz budget",target=33.333)}
{line_chart(results,"cpu_percent","Process CPU utilization","percent of one logical core")}
{line_chart(results,"measurement_rss_peak_bytes","Peak resident memory during measurement","MiB",divisor=2**20)}
{line_chart(results,"measurement_private_peak_bytes","Peak private commit during measurement","MiB; Windows private allocation, lower is better",divisor=2**20)}
</div><h2>Exact measurements</h2><section class="panel table-wrap"><table><thead><tr><th>Stage</th><th>Plots</th><th>Indicators</th><th>Framework</th><th>Hz</th><th>Dropped</th><th>Update p95 ms</th><th>Frame p95 ms</th><th>CPU %</th><th>RSS MiB</th><th>Private MiB</th><th>Checks</th></tr></thead><tbody>{result_rows(results)}</tbody></table></section>
<h2>What this benchmark establishes</h2><div class="findings"><div class="finding"><strong>It measures fan-out, not growing trace size.</strong><span>Every trace remains 1,024 points. Stage-to-stage changes therefore expose the cost of more retained plots and more property targets.</span></div><div class="finding"><strong>Displayed state is validated.</strong><span>Each adapter verifies the final tick, first and last text/progress/LED values, plot existence, and point retention. DragonGUI additionally verifies native retained text, all line resources, source-point totals, queue drain, and zero layout diagnostics.</span></div><div class="finding"><strong>Frame metrics are framework-native, not identical internals.</strong><span>DragonGUI reports native render work; Dear PyGui and PyQtGraph report their explicit render/process-events calls. Update p95, delivered cadence, CPU, and memory are the stronger cross-framework comparisons.</span></div><div class="finding"><strong>Overload uses latest-state semantics.</strong><span>Synchronous adapters skip overdue scheduled ticks. DragonGUI coalesces pending Python frames by key and must still drain to the exact final state before validation succeeds.</span></div></div>
<h2>Method and fairness notes</h2><section class="panel" style="padding:22px"><p>All three windows use a four-column plot wall and a scrollable indicator rail. Each indicator consists of a status light, changing numeric label, and progress channel. All plots and all indicator properties change every scheduled tick. Framework order rotates by stage, and every sample runs in a fresh process. DragonGUI uses its public NumPy-backed line API and batched property updates; PyQtGraph uses NumPy curves; Dear PyGui's public line-series API requires Python lists, so list conversion is part of its native application update cost.</p><p>This report uses one measured sample per configuration unless the matrix was invoked with additional repetitions. It is an engineering comparison on this machine, not a universal framework ranking. Startup time is collected but intentionally excluded from the headline analysis.</p></section><footer>Sources: <code>benchmarks/gui_telemetry_viewer_case.py</code>, <code>benchmarks/run_gui_telemetry_viewer_matrix.py</code>, and <a href="../{escape(source_label)}/summary.json">validated summary JSON</a>.</footer></main></body></html>'''


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--summary", type=Path, default=DEFAULT_SUMMARY)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--previous-summary", type=Path)
    args = parser.parse_args()
    summary = json.loads(args.summary.read_text(encoding="utf-8"))
    previous = (
        json.loads(args.previous_summary.read_text(encoding="utf-8"))
        if args.previous_summary
        else None
    )
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        render(summary, str(args.summary.resolve().parent.relative_to(ROOT)), previous),
        encoding="utf-8",
    )
    print(f"Wrote {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
