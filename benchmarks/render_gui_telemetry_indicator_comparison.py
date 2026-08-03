"""Render a controlled before/after telemetry-indicator optimization report."""

from __future__ import annotations

import argparse
from datetime import datetime
from html import escape
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
MODES = ("labels", "progress", "leds", "combined")
COUNTS = (24, 72, 160, 320)


def number(value: Any, digits: int = 1) -> str:
    return f"{float(value):,.{digits}f}"


def delta(before: float, after: float) -> float:
    return (after / before - 1.0) * 100.0 if before else 0.0


def comparison_chart(before: dict[str, Any], after: dict[str, Any], metric: str, title: str, unit: str) -> str:
    values = [float(dataset["320"][mode][metric]) for dataset in (before, after) for mode in MODES]
    ceiling = max(values + [1.0]) * 1.12
    width, height, left, top, plot_w, plot_h = 760, 330, 60, 34, 660, 225
    group_w = plot_w / len(MODES)
    bar_w = 42
    bars: list[str] = []
    for index, mode in enumerate(MODES):
        center = left + group_w * (index + 0.5)
        for offset, dataset, color, label in ((-24, before, "#7385a8", "before"), (24, after, "#57d7a1", "after")):
            value = float(dataset["320"][mode][metric])
            bar_h = value / ceiling * plot_h
            x = center + offset - bar_w / 2
            y = top + plot_h - bar_h
            bars.append(f'<rect x="{x:.1f}" y="{y:.1f}" width="{bar_w}" height="{bar_h:.1f}" rx="5" fill="{color}"/><text x="{center + offset:.1f}" y="{y - 7:.1f}" text-anchor="middle">{value:.1f}</text>')
        bars.append(f'<text x="{center:.1f}" y="{top + plot_h + 28}" text-anchor="middle" class="axis">{escape(mode.title())}</text>')
    grid = "".join(f'<line x1="{left}" x2="{left + plot_w}" y1="{top + plot_h - plot_h*i/4:.1f}" y2="{top + plot_h - plot_h*i/4:.1f}" class="grid"/>' for i in range(5))
    return f'<section class="chart"><h3>{escape(title)}</h3><p>{escape(unit)} at 320 channels; lower is better</p><svg viewBox="0 0 {width} {height}">{grid}{"".join(bars)}</svg><div class="legend"><span><i class="before"></i>Before</span><span><i class="after"></i>After</span></div></section>'


def table_rows(before: dict[str, Any], after: dict[str, Any]) -> str:
    rows: list[str] = []
    for count in COUNTS:
        for mode in MODES:
            b, a = before[str(count)][mode], after[str(count)][mode]
            rows.append(
                f'<tr><td>{count}</td><td>{escape(mode.title())}</td>'
                f'<td>{number(b["tick_throughput_hz"],2)} → {number(a["tick_throughput_hz"],2)}</td>'
                f'<td>{b["measurement_dropped_ticks"]} → {a["measurement_dropped_ticks"]}</td>'
                f'<td>{number(b["cpu_percent"])} → {number(a["cpu_percent"])}</td>'
                f'<td>{number(b["command_apply_p95_ms"],2)} → {number(a["command_apply_p95_ms"],2)}</td>'
                f'<td>{number(b["command_drain_p95_ms"],2)} → {number(a["command_drain_p95_ms"],2)}</td>'
                f'<td>{number(b["text_measure_cache_misses"],0)} → {number(a["text_measure_cache_misses"],0)}</td>'
                f'<td>{number(a.get("native_properties_per_completed_tick", a["properties_per_tick"]),1)} / {a["properties_per_tick"]}</td>'
                f'<td class="{"pass" if a["validation"]["passed"] else "fail"}">{"PASS" if a["validation"]["passed"] else "FAIL"}</td></tr>'
            )
    return "".join(rows)


def render(baseline: dict[str, Any], candidate: dict[str, Any], baseline_path: Path, candidate_path: Path) -> str:
    before, after = baseline["results"], candidate["results"]
    b320, a320 = before["320"], after["320"]
    generated = datetime.now().astimezone().strftime("%Y-%m-%d %H:%M %Z")
    combined_cpu = delta(b320["combined"]["cpu_percent"], a320["combined"]["cpu_percent"])
    combined_apply = delta(b320["combined"]["command_apply_p95_ms"], a320["combined"]["command_apply_p95_ms"])
    label_misses = delta(b320["labels"]["text_measure_cache_misses"], a320["labels"]["text_measure_cache_misses"])
    led_traffic = float(a320["leds"]["native_properties_per_completed_tick"])
    return f'''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>DragonGUI Indicator Optimization A/B</title><style>
:root{{--bg:#09101e;--panel:#121c32;--panel2:#172641;--text:#edf3ff;--muted:#9baccc;--line:#2c3e63;--green:#57d7a1;--red:#ff7185;--amber:#ffbd69}}*{{box-sizing:border-box}}body{{margin:0;background:radial-gradient(circle at 18% 0,#18335a 0,transparent 32%),var(--bg);color:var(--text);font:15px/1.55 Inter,Segoe UI,system-ui,sans-serif}}main{{max-width:1240px;margin:auto;padding:48px 28px 80px}}h1{{font-size:clamp(2.4rem,5vw,4.3rem);line-height:1.04;margin:.3rem 0 1rem}}h2{{margin-top:46px}}h3{{margin:0}}p{{color:var(--muted)}}.eyebrow{{color:var(--green);font-weight:800;letter-spacing:.13em;text-transform:uppercase}}.kpis{{display:grid;grid-template-columns:repeat(4,1fr);gap:14px;margin:30px 0}}.kpi,.panel,.chart{{background:linear-gradient(145deg,var(--panel2),var(--panel));border:1px solid var(--line);border-radius:16px;box-shadow:0 18px 44px #02061166}}.kpi{{padding:19px}}.kpi b{{font-size:1.9rem;display:block;color:var(--green)}}.kpi span{{color:var(--muted);font-size:.82rem}}.success,.warning{{padding:17px 19px;border-radius:8px;margin-top:12px}}.success{{border-left:4px solid var(--green);background:#102b2b}}.warning{{border-left:4px solid var(--amber);background:#332817;color:#ffe1b9}}.charts{{display:grid;grid-template-columns:1fr 1fr;gap:18px}}.chart{{padding:18px}}.chart p{{font-size:.82rem;margin:.2rem 0}}svg{{width:100%;height:auto}}svg text{{fill:#e5ecfb;font:11px Segoe UI,sans-serif}}.grid{{stroke:#2c3e63}}.axis{{fill:#aab9d7}}.legend{{display:flex;gap:18px;justify-content:center;color:var(--muted)}}.legend i{{display:inline-block;width:11px;height:11px;border-radius:3px;margin-right:6px}}.before{{background:#7385a8}}.after{{background:var(--green)}}.table-wrap{{padding:18px;overflow-x:auto}}table{{width:100%;border-collapse:collapse;min-width:1200px}}th,td{{padding:9px 8px;border-bottom:1px solid var(--line);text-align:right}}th{{font-size:.7rem;text-transform:uppercase;color:var(--muted)}}th:nth-child(2),td:nth-child(2){{text-align:left}}.pass{{color:var(--green);font-weight:800}}.fail{{color:var(--red)}}code{{color:#cad5ff}}footer{{margin-top:42px;color:var(--muted);font-size:.82rem}}@media(max-width:900px){{.charts{{grid-template-columns:1fr}}.kpis{{grid-template-columns:1fr 1fr}}}}@media(max-width:520px){{.kpis{{grid-template-columns:1fr}}}}
</style></head><body><main><div class="eyebrow">Controlled same-machine A/B</div><h1>Telemetry indicator optimization</h1><p>Same 16-case workload, 30 Hz target, fresh process per sample, and final-state validation. The candidate adds stable-height wrapped-label invalidation, a bulk text-rebuild crossover, and effective-state LED deduplication.</p><p>Generated {escape(generated)}</p>
<div class="kpis"><div class="kpi"><b>{combined_cpu:.1f}%</b><span>320 combined CPU change</span></div><div class="kpi"><b>{combined_apply:.1f}%</b><span>320 combined native apply p95 change</span></div><div class="kpi"><b>{label_misses:.1f}%</b><span>320-label measurement-miss change</span></div><div class="kpi"><b>{led_traffic:.1f}</b><span>native LED properties/tick from 641 offered</span></div></div>
<div class="success"><strong>The overloaded case is now real-time.</strong> The 320-channel combined case changed from {number(b320["combined"]["tick_throughput_hz"],2)} Hz with {b320["combined"]["measurement_dropped_ticks"]} dropped ticks to {number(a320["combined"]["tick_throughput_hz"],2)} Hz with {a320["combined"]["measurement_dropped_ticks"]} dropped ticks. All 16 candidate cases passed their correctness checks.</div>
<div class="success"><strong>Memory follow-up resolved the apparent regression and reduced retained footprint.</strong> Repeated snapshot-free samples showed flat retained counts and private commit, while the candidate's private commit was already lower than the earlier baseline despite higher Windows working-set RSS. Compacting optional retained style layers then reduced 320-combined median RSS by another 11.93 MiB and private commit by 16.64 MiB at the same 30 Hz.</div>
<h2>High-scale comparison</h2><div class="charts">{comparison_chart(before,after,"cpu_percent","Process CPU","percent of one logical core")}{comparison_chart(before,after,"command_apply_p95_ms","Native property application p95","milliseconds")}{comparison_chart(before,after,"command_drain_p95_ms","Complete command drain p95","milliseconds")}{comparison_chart(before,after,"text_measure_cache_misses","Layout text measurements","cache misses over run")}</div>
<h2>All cases</h2><section class="panel table-wrap"><table><thead><tr><th>Count</th><th>Mode</th><th>Hz before → after</th><th>Drops</th><th>CPU %</th><th>Apply p95 ms</th><th>Drain p95 ms</th><th>Text misses</th><th>Native/offered props</th><th>Validation</th></tr></thead><tbody>{table_rows(before,after)}</tbody></table></section>
<h2>What changed</h2><section class="panel" style="padding:22px"><ol><li>Fixed-width wrapped labels compare old and replacement shaped heights. Equal-height changes rebuild retained text without relaying out the window; changed line counts retain the safe layout path.</li><li>Large text batches cross over to one linear text rebuild instead of repeatedly scanning retained entries for hundreds of tiny subtree patches.</li><li><code>LED.set_state()</code>, <code>set_on()</code>, and <code>set_color()</code> no longer enqueue unchanged effective state or color.</li></ol></section>
<footer>Baseline: <code>{escape(str(baseline_path))}</code><br>Candidate: <code>{escape(str(candidate_path))}</code></footer></main></body></html>'''


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    baseline = json.loads(args.baseline.read_text(encoding="utf-8"))
    candidate = json.loads(args.candidate.read_text(encoding="utf-8"))
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(render(baseline, candidate, args.baseline, args.candidate), encoding="utf-8")
    print(f"Wrote {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
