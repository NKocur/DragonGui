"""Render the DragonGUI versus Reflex XY benchmark as self-contained HTML."""

from __future__ import annotations

import argparse
import json
import statistics
from datetime import UTC, datetime
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SIZES = (10_000, 100_000, 1_000_000, 2_500_000, 4_000_000, 5_000_000)


def _median(values: list[float]) -> float | None:
    return statistics.median(values) if values else None


def _dg_cell(documents: list[dict[str, Any]], n: int, mode: str) -> dict[str, Any]:
    for document in documents:
        for cell in document.get("summaries", []):
            if cell.get("n") == n and cell.get("mode") == mode:
                samples = cell.get("render_wall_ms", {}).get("samples", [])
                failures = [sample for sample in samples if sample.get("status") != "ok"]
                return {
                    "ready_ms": cell.get("render_wall_ms", {}).get("median"),
                    "frame_ms": cell.get("frame_ms", {}).get("median"),
                    "memory_mib": (
                        float(cell["peak_rss_bytes"]["median"]) / 2**20
                        if cell.get("peak_rss_bytes", {}).get("median") is not None
                        else None
                    ),
                    "successes": cell.get("render_wall_ms", {}).get("successful_runs", 0),
                    "attempts": cell.get("render_wall_ms", {}).get("attempted_runs", 0),
                    "failure": failures[0].get("stderr", "") if failures else None,
                }
    return {
        "ready_ms": None,
        "frame_ms": None,
        "memory_mib": None,
        "successes": 0,
        "attempts": 0,
        "failure": "No DragonGUI result",
    }


def _xy_build(build: dict[str, Any], n: int, mode: str) -> float | None:
    for cell in build.get("summaries", []):
        if cell.get("n") == n and cell.get("mode") == mode:
            return cell.get("python_build_ms", {}).get("median")
    return None


def _xy_cell(
    runs: list[dict[str, Any]], build: dict[str, Any], n: int, mode: str
) -> dict[str, Any]:
    arm = "xy" if mode == "adaptive" else "xy-exact"
    samples = [
        row
        for run in runs
        for row in run.get("results", [])
        if row.get("n") == n and row.get("arm") == arm and row.get("status") == "ok"
    ]
    visible = [float(row["visible_complete_ms"]) for row in samples]
    build_ms = _xy_build(build, n, mode)
    stable_ms = _median(visible)
    return {
        "build_ms": build_ms,
        "stable_ms": stable_ms,
        "ready_ms": build_ms + stable_ms if build_ms is not None and stable_ms is not None else None,
        "host_memory_mib": _median(
            [float(row["python_peak_rss_bytes"]) / 2**20 for row in samples]
        ),
        "browser_memory_mib": _median(
            [float(row["browser_peak_rss_bytes"]) / 2**20 for row in samples]
        ),
        "samples": len(samples),
        "render_mode": samples[0].get("mode") if samples else None,
    }


def build_report_data(
    dg_documents: list[dict[str, Any]], xy_runs: list[dict[str, Any]], xy_build: dict[str, Any]
) -> dict[str, Any]:
    rows = []
    for n in DEFAULT_SIZES:
        entry: dict[str, Any] = {"n": n}
        for mode in ("adaptive", "exact"):
            dragon = _dg_cell(dg_documents, n, mode)
            xy = _xy_cell(xy_runs, xy_build, n, mode)
            speedup = (
                dragon["ready_ms"] / xy["ready_ms"]
                if dragon["ready_ms"] is not None and xy["ready_ms"]
                else None
            )
            entry[mode] = {"dragongui": dragon, "xy": xy, "xy_speedup": speedup}
        rows.append(entry)
    return {
        "generated_at_utc": datetime.now(UTC).isoformat().replace("+00:00", "Z"),
        "benchmark_date": "2026-08-05",
        "environment": {
            "os": "Windows 10.0.19045",
            "python": "CPython 3.13.14",
            "logical_cpus": 16,
            "dragongui": "1.0.0",
            "xy": "0.0.5",
            "viewport": "900 × 420",
            "source": "seeded correlated float32 x/y; 8 bytes per row",
        },
        "rows": rows,
        "interaction": {
            "xy_frame_p50_ms": 4.10,
            "xy_frame_p95_ms": 5.30,
            "note": "1M trusted wheel workload; browser and native frame metrics are directional",
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--dragongui",
        type=Path,
        action="append",
        default=[],
        help="DragonGUI matrix summary; repeat for multiple size groups",
    )
    parser.add_argument("--xy-run", type=Path, action="append", default=[])
    parser.add_argument(
        "--xy-build", type=Path, default=ROOT / "artifacts" / "xy-python-build.json"
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "docs" / "dragongui-vs-xy-benchmark.html",
    )
    args = parser.parse_args()
    dg_paths = args.dragongui or [
        ROOT / "artifacts" / "dragongui-xy-matrix.json",
        ROOT / "artifacts" / "dragongui-xy-ceiling.json",
    ]
    xy_paths = args.xy_run or [
        ROOT / "artifacts" / "xy-load-run2.json",
        ROOT / "artifacts" / "xy-load-run3.json",
        ROOT / "artifacts" / "xy-load-run4.json",
    ]
    documents = [json.loads(path.read_text(encoding="utf-8")) for path in dg_paths]
    xy_runs = [json.loads(path.read_text(encoding="utf-8")) for path in xy_paths]
    xy_build = json.loads(args.xy_build.read_text(encoding="utf-8"))
    report = build_report_data(documents, xy_runs, xy_build)
    embedded = json.dumps(report, separators=(",", ":")).replace("</", "<\\/")
    html = TEMPLATE.replace("__BENCHMARK_DATA__", embedded)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(html, encoding="utf-8")
    print(f"Wrote {args.output.resolve()}")
    return 0


TEMPLATE = r'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="description" content="Measured comparison of DragonGUI 1.0.0 and Reflex XY 0.0.5 scatter rendering.">
<title>DragonGUI vs XY — Scatter Benchmark</title>
<style>
:root{color-scheme:dark;--bg:#071016;--surface:#0c1922;--surface-2:#10232f;--ink:#edf7f5;--muted:#94aaa9;--line:#233d46;--dragon:#ff9b54;--xy:#4de2b1;--exact:#72a7ff;--danger:#ff6f7d;--warning:#ffc857;--shadow:#0008}
*{box-sizing:border-box}html{scroll-behavior:smooth;background:var(--bg);color:var(--ink);font:15px/1.55 Inter,ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif}body{margin:0;background:radial-gradient(circle at 80% -10%,#174a4a 0,transparent 31rem),radial-gradient(circle at 0 15%,#3b2418 0,transparent 27rem),var(--bg)}
main{width:min(1420px,calc(100% - 40px));margin:auto;padding:50px 0 80px}.eyebrow{color:var(--xy);font-size:.76rem;font-weight:700;letter-spacing:.15em;text-transform:uppercase}.hero{display:grid;grid-template-columns:minmax(0,1.45fr) minmax(280px,.55fr);gap:30px;align-items:end}.hero h1{font-size:clamp(2.5rem,6vw,5.8rem);line-height:.93;letter-spacing:-.065em;margin:.35rem 0 .8rem;max-width:980px}.lede{color:#b9cdcb;font-size:1.12rem;max-width:880px}.hero-note{border-left:3px solid var(--danger);padding:8px 0 8px 17px;color:#d6e4e1}.meta{display:flex;gap:9px;flex-wrap:wrap;margin:25px 0 37px}.pill{border:1px solid var(--line);background:#0a1820bf;border-radius:999px;padding:6px 11px;color:#bfd1cf;font-size:.8rem}
.grid{display:grid;grid-template-columns:repeat(12,1fr);gap:16px}.card{border:1px solid var(--line);background:linear-gradient(145deg,#10232fe8,#09151df2);border-radius:16px;padding:20px;box-shadow:0 18px 55px var(--shadow)}.metric{grid-column:span 3;min-height:150px}.rank{font-size:.72rem;letter-spacing:.09em;text-transform:uppercase;color:var(--muted)}.big{font-size:2.15rem;line-height:1.1;font-weight:750;letter-spacing:-.045em;margin:.42rem 0}.metric p,.note{color:var(--muted);margin:.35rem 0 0}.xy{color:var(--xy)}.dragon{color:var(--dragon)}.danger{color:var(--danger)}
.section{margin-top:42px}.section-head{display:flex;justify-content:space-between;gap:20px;align-items:end;margin-bottom:15px}.section h2{font-size:1.65rem;letter-spacing:-.025em;margin:.2rem 0}.section h3{font-size:1rem;margin:0}.chart-card{grid-column:span 6;min-height:440px}.chart{height:330px;margin-top:10px}.chart svg{width:100%;height:100%;overflow:visible}.axis{stroke:#51676d;stroke-width:1}.gridline{stroke:#1e3540;stroke-width:1}.tick{fill:#91a7a7;font-size:11px}.label{fill:#c8d8d6;font-size:11px}.legend{display:flex;flex-wrap:wrap;gap:15px;margin-top:7px;color:#a8bcba;font-size:.8rem}.key{display:inline-block;width:10px;height:10px;border-radius:50%;margin-right:6px}.tooltip{position:fixed;pointer-events:none;display:none;background:#02080cdd;border:1px solid #46616a;border-radius:9px;padding:8px 10px;font-size:.78rem;z-index:20;box-shadow:0 10px 35px #000b}
.table-wrap{overflow-x:auto}.table-card{grid-column:1/-1}table{width:100%;border-collapse:collapse;margin-top:8px;font-size:.87rem;min-width:840px}th,td{padding:10px 11px;text-align:right;border-bottom:1px solid #203741;white-space:nowrap}th:first-child,td:first-child{text-align:left}th{font-size:.7rem;letter-spacing:.075em;text-transform:uppercase;color:#a7bdbb}td{color:#d9e7e4}.fail{color:var(--danger);font-weight:700}.faster{color:var(--xy)}
.callouts{display:grid;grid-template-columns:repeat(3,1fr);gap:16px}.callout{border-left:3px solid var(--xy);padding:17px;background:#0d2029;border-radius:0 12px 12px 0}.callout.warn{border-color:var(--warning)}.callout.risk{border-color:var(--danger)}.callout p,.callout li{color:#a9bdbb}.callout ul{margin:.6rem 0 0;padding-left:1.2rem}.callout li+li{margin-top:.45rem}
.mode{display:inline-flex;align-items:center;gap:7px;color:var(--muted);font-size:.79rem}.mode:before{content:"";width:8px;height:8px;border-radius:50%;background:var(--xy)}.mode.exact:before{background:var(--exact)}.followup-card{grid-column:span 4}.method{grid-column:span 7}.limits{grid-column:span 5}.method p,.limits p,.method li,.limits li{color:#a9bdba}.small{font-size:.8rem;color:#829997}.foot{margin-top:40px;padding-top:20px;border-top:1px solid var(--line);display:flex;justify-content:space-between;gap:20px;flex-wrap:wrap;color:#819794;font-size:.78rem}a{color:#7fc8ff}code{color:#dbe9e6;background:#0a151b;border:1px solid #1d333c;border-radius:5px;padding:.1rem .3rem}
@media(max-width:1000px){.hero{grid-template-columns:1fr}.metric,.chart-card,.followup-card{grid-column:span 6}.callouts{grid-template-columns:1fr}.method,.limits{grid-column:1/-1}}
@media(max-width:650px){main{width:min(100% - 24px,1420px);padding-top:28px}.metric,.chart-card,.followup-card{grid-column:1/-1}.card{padding:15px}.chart{height:290px}.section-head{display:block}.hero h1{font-size:2.8rem}.big{font-size:1.85rem}}
@media print{body{background:#fff;color:#111}.card{box-shadow:none;break-inside:avoid}.tooltip{display:none!important}}
</style>
</head>
<body><main>
<header class="hero">
 <div><div class="eyebrow">Measured scatter study · August 5, 2026</div><h1>XY wins the load race. DragonGUI wins back frame time.</h1><p class="lede">A fresh-process comparison of the released <strong>dragongui 1.0.0</strong> and <strong>xy 0.0.5</strong> wheels, covering exact markers, adaptive rendering, memory, cold readiness, and the point-count ceiling.</p></div>
 <aside class="hero-note"><strong>Critical ceiling</strong><br>DragonGUI renders 4 million points, then aborts at 5 million because a 320 MB scatter vertex buffer exceeds the adapter's 256 MiB limit. Adaptive LOD does not prevent the allocation.</aside>
</header>
<div class="meta"><span class="pill">3 fresh processes per cell</span><span class="pill">10 correct + stable browser frames</span><span class="pill">20 validated native frames</span><span class="pill">900 × 420 viewport</span><span class="pill">Python 3.13.14 · Windows</span><span class="pill">lower is better</span></div>

<section class="grid" aria-label="Headline results">
 <article class="card metric"><div class="rank">1M adaptive · ready</div><div class="big xy" id="metricAdaptive">14.0×</div><p>XY reaches a correct stable image in about 392 ms; DragonGUI needs about 5.50 s.</p></article>
 <article class="card metric"><div class="rank">4M exact · ready</div><div class="big xy" id="metricExact">2.2×</div><p>XY's exact path narrows the gap as every marker reaches the renderer.</p></article>
 <article class="card metric"><div class="rank">DragonGUI retained frames</div><div class="big dragon">3.3–6.2 ms</div><p>Measured 20-frame averages remain strong from 10k through 4M source rows.</p></article>
 <article class="card metric"><div class="rank">Maximum successful source</div><div class="big danger">4M</div><p>Both DragonGUI exact and adaptive modes fail at the next tested size, 5M.</p></article>
</section>

<section class="section"><div class="section-head"><div><div class="eyebrow">Time to ready</div><h2>Cold-start cost dominates DragonGUI</h2></div><p class="note">Python input arrays are ready before the clock starts. Hover points for exact medians.</p></div>
 <div class="grid">
  <article class="card chart-card"><h3>Adaptive rendering</h3><div class="mode">XY density above 200k · DragonGUI stride LOD</div><div id="adaptiveTime" class="chart"></div><div class="legend"><span><i class="key" style="background:var(--xy)"></i>XY correct + stable</span><span><i class="key" style="background:var(--dragon)"></i>DragonGUI 20 frames</span></div></article>
  <article class="card chart-card"><h3>Exact markers</h3><div class="mode exact">Every retained row becomes a point</div><div id="exactTime" class="chart"></div><div class="legend"><span><i class="key" style="background:var(--xy)"></i>XY correct + stable</span><span><i class="key" style="background:var(--dragon)"></i>DragonGUI 20 frames</span></div></article>
 </div>
</section>

<section class="section"><div class="section-head"><div><div class="eyebrow">Scaling and memory</div><h2>XY bounds adaptive output; DragonGUI retains excellent steady frames</h2></div><p class="note">XY host and browser memory remain separate because Chrome has a roughly 500 MiB baseline.</p></div>
 <div class="grid">
  <article class="card chart-card"><h3>DragonGUI average native frame time</h3><div id="frameTime" class="chart"></div><div class="legend"><span><i class="key" style="background:var(--xy)"></i>Adaptive</span><span><i class="key" style="background:var(--exact)"></i>Exact</span></div></article>
  <article class="card chart-card"><h3>Resident memory by process pool</h3><div id="memoryChart" class="chart"></div><div class="legend"><span><i class="key" style="background:var(--dragon)"></i>DragonGUI process</span><span><i class="key" style="background:var(--xy)"></i>XY adaptive host</span><span><i class="key" style="background:var(--exact)"></i>XY exact host</span></div></article>
 </div>
</section>

<section class="section"><div class="section-head"><div><div class="eyebrow">Fair interaction latency · 1M points</div><h2>Correct pixels arrive quickly; validation is a separate cost</h2></div><p class="note">The bars intentionally separate render/application timing from ten-frame stability validation.</p></div>
 <article class="card chart-card" style="grid-column:1/-1"><h3>Matched wheel workload</h3><div id="interactionLatency" class="chart"></div><div class="legend"><span><i class="key" style="background:var(--dragon)"></i>DragonGUI</span><span><i class="key" style="background:var(--xy)"></i>XY</span></div></article>
</section>

<section class="section"><div class="section-head"><div><div class="eyebrow">Measured cells</div><h2>Complete median results</h2></div></div>
 <article class="card table-card"><h3>Adaptive modes</h3><div class="table-wrap"><table id="adaptiveTable"></table></div><p class="small">* XY frame p50/p95 is from the separate 1M trusted-wheel workload; it is a browser gesture metric, not the same clock as DragonGUI’s native frame average.</p></article>
 <article class="card table-card" style="margin-top:16px"><h3>Exact-marker modes</h3><div class="table-wrap"><table id="exactTable"></table></div><p class="small">* XY frame p50/p95 is from the separate 1M trusted-wheel workload; it is a browser gesture metric, not the same clock as DragonGUI’s native frame average.</p></article>
</section>

<section class="section"><div class="section-head"><div><div class="eyebrow">Current branch follow-up · August 6</div><h2>Both trusted-input gesture gates now pass</h2></div><p class="note">The shared contract exposes a long DragonGUI correct-to-stable measurement tail; input, renderer, and capture stacks still differ.</p></div>
 <div class="grid">
  <article class="card followup-card"><h3>DragonGUI · matched XY wheel workload</h3><div class="big dragon">Pass · ~115 ms correct</div><p class="note">Split and batched runs showed correct pixels at 93–116 ms. Exact representation was applied independently; ten-stable completion remains a readback diagnostic, not renderer latency.</p></article>
  <article class="card followup-card"><h3>DragonGUI · mixed OS regression</h3><div class="big dragon">Pass · 260.16 ms stable</div><p class="note">Center wheel zoom, Shift-pan, resize, Home, and exact rectangle selection remained correct after adding cursor-anchored 2D zoom.</p></article>
  <article class="card followup-card"><h3>XY · trusted CDP wheel workload</h3><div class="big xy">Pass · 20.40 ms settle</div><p class="note">All 42 trusted events applied; final span was 0.0005 and the target stayed anchored. Total first-input-to-stable time was 1,383.20 ms with 5.30 ms gesture-frame p95.</p></article>
 </div>
</section>

<section class="section"><div class="callouts">
 <article class="callout"><h3>What XY demonstrates</h3><ul><li>Separate exact source columns from disposable render buffers.</li><li>Switch dense overviews to a screen-bounded density grid.</li><li>Keep adaptive payloads near 256 KiB from 1M through 5M rows.</li><li>Stop benchmarks only after pixels are correct and stable.</li></ul></article>
 <article class="callout warn"><h3>What DragonGUI already does well</h3><ul><li>Retained-frame averages stay under 6.3 ms through 4M points.</li><li>Exact rendering scales without XY's browser-process footprint.</li><li>Packed NumPy ingestion and native upload are not the cold-start bottleneck.</li><li>A native desktop runtime remains attractive for sustained tools.</li></ul></article>
 <article class="callout risk"><h3>Immediate engineering priorities</h3><ul><li>Guard every GPU allocation and eliminate native capacity panics.</li><li>Replace the 64-byte point expansion with compact attribute buffers.</li><li>Apply reduction before allocation and add density aggregation.</li><li>Instrument the roughly 5.2-second startup queue gap.</li></ul></article>
 </div></section>

<section class="section grid">
 <article class="card method"><div class="eyebrow">Method</div><h2>Same rows, different presentation stacks</h2><p>Both libraries receive seeded correlated Gaussian <code>float32</code> x/y arrays with five planted sentinels. Package import and data generation are excluded. XY total time combines production Python figure/payload construction with browser navigation to a correct image and ten byte-identical frames. DragonGUI time covers public widget construction plus <code>App.run()</code> through 20 native frames after confirming the source count, native update, and application presentation.</p><p>Adaptive modes target the same user outcome but are not algorithmically identical: XY uses density above 200k rows while DragonGUI uses stride LOD factor 8. The exact table is the stricter engine-to-engine comparison.</p></article>
 <article class="card limits"><div class="eyebrow">Interpretation limits</div><h2>Directional, not universal</h2><ul><li>One Windows machine and three samples per successful cell.</li><li>XY's headless Chrome used SwiftShader; DragonGUI used its native wgpu adapter. This biases browser timing against XY.</li><li>XY performs pixel readback validation; DragonGUI validates native state and completed frames.</li><li>Memory pools cannot be collapsed without hiding Chrome's process baseline.</li></ul></article>
</section>

<footer class="foot"><span>Sources: <a href="dragongui-vs-xy-benchmark.md">benchmark notes</a> · <a href="../plans/xy-inspired-scatter-improvement-plan.md">improvement plan</a> · <a href="https://github.com/reflex-dev/xy/blob/main/benchmarks/README.md">XY benchmark runbook</a></span><span>Generated from raw JSON in <code>artifacts/</code></span></footer>
</main><div id="tooltip" class="tooltip"></div>
<script>
const DATA=__BENCHMARK_DATA__;
const tooltip=document.getElementById('tooltip');
const css=name=>getComputedStyle(document.documentElement).getPropertyValue(name).trim();
const COLORS={xy:css('--xy'),dragon:css('--dragon'),adaptive:css('--xy'),exact:css('--exact')};
const fmtN=n=>n>=1e6?(n/1e6).toLocaleString(undefined,{maximumFractionDigits:1})+'M':(n/1e3).toLocaleString()+'k';
const fmtMs=v=>v==null?'failed':v>=1000?(v/1000).toFixed(2)+' s':v.toFixed(1)+' ms';
const svgEl=(name,attrs={})=>{const e=document.createElementNS('http://www.w3.org/2000/svg',name);for(const[k,v]of Object.entries(attrs))e.setAttribute(k,v);return e};
function showTip(event,html){tooltip.innerHTML=html;tooltip.style.display='block';tooltip.style.left=Math.min(innerWidth-190,event.clientX+12)+'px';tooltip.style.top=(event.clientY+12)+'px'}
function hideTip(){tooltip.style.display='none'}
function lineChart(id,series,{unit='ms',logY=false,maxY=null}={}){
 const host=document.getElementById(id),W=650,H=315,m={l:64,r:22,t:20,b:45},xs=DATA.rows.map(r=>r.n),values=series.flatMap(s=>s.values).filter(v=>v!=null),minPositive=Math.max(.01,Math.min(...values)),upper=maxY||Math.max(...values)*1.12;
 const xp=x=>m.l+(Math.log10(x)-Math.log10(xs[0]))/(Math.log10(xs.at(-1))-Math.log10(xs[0]))*(W-m.l-m.r);
 const yp=y=>{if(logY){const lo=Math.log10(minPositive*.8),hi=Math.log10(upper);return H-m.b-(Math.log10(y)-lo)/(hi-lo)*(H-m.t-m.b)}return H-m.b-y/upper*(H-m.t-m.b)};
 const svg=svgEl('svg',{viewBox:`0 0 ${W} ${H}`,role:'img','aria-label':host.dataset.label||'Benchmark line chart'});
 for(let i=0;i<=4;i++){const ratio=i/4,y=H-m.b-ratio*(H-m.t-m.b),value=logY?10**(Math.log10(minPositive*.8)+ratio*(Math.log10(upper)-Math.log10(minPositive*.8))):ratio*upper;svg.append(svgEl('line',{x1:m.l,y1:y,x2:W-m.r,y2:y,class:'gridline'}));const t=svgEl('text',{x:m.l-9,y:y+4,'text-anchor':'end',class:'tick'});t.textContent=unit==='MiB'?Math.round(value):value>=1000?(value/1000).toFixed(1)+'s':value>=10?Math.round(value):value.toFixed(1);svg.append(t)}
 svg.append(svgEl('line',{x1:m.l,y1:H-m.b,x2:W-m.r,y2:H-m.b,class:'axis'}));for(const x of xs){const t=svgEl('text',{x:xp(x),y:H-17,'text-anchor':'middle',class:'tick'});t.textContent=fmtN(x);svg.append(t)}
 for(const s of series){const points=s.values.map((v,i)=>v==null?null:[xp(xs[i]),yp(v),v,xs[i]]);let segment=[];const flush=()=>{if(segment.length>1)svg.append(svgEl('polyline',{points:segment.map(p=>`${p[0]},${p[1]}`).join(' '),fill:'none',stroke:s.color,'stroke-width':2.6}));segment=[]};for(const point of points){if(point)segment.push(point);else flush()}flush();for(const point of points){if(!point)continue;const[x,y,v,n]=point,dot=svgEl('circle',{cx:x,cy:y,r:4.8,fill:s.color,stroke:css('--bg'),'stroke-width':2});dot.addEventListener('mousemove',event=>showTip(event,`${s.label} · ${fmtN(n)}<br><b>${unit==='MiB'?v.toFixed(1)+' MiB':fmtMs(v)}</b>`));dot.addEventListener('mouseleave',hideTip);svg.append(dot)}}host.append(svg)
}
function barChart(id,items){const host=document.getElementById(id),W=980,H=315,m={l:190,r:28,t:22,b:42},max=Math.max(...items.map(x=>x.value))*1.12,svg=svgEl('svg',{viewBox:`0 0 ${W} ${H}`,role:'img','aria-label':'Fair interaction latency comparison'});for(let i=0;i<=4;i++){const x=m.l+i/4*(W-m.l-m.r),v=i/4*max;svg.append(svgEl('line',{x1:x,y1:m.t,x2:x,y2:H-m.b,class:'gridline'}));const t=svgEl('text',{x,y:H-16,'text-anchor':'middle',class:'tick'});t.textContent=fmtMs(v);svg.append(t)}const gap=(H-m.t-m.b)/items.length;items.forEach((item,i)=>{const y=m.t+i*gap+gap*.22,h=gap*.56,w=item.value/max*(W-m.l-m.r);const label=svgEl('text',{x:m.l-12,y:y+h*.68,'text-anchor':'end',class:'label'});label.textContent=item.label;svg.append(label);const rect=svgEl('rect',{x:m.l,y,width:w,height:h,rx:6,fill:item.color});rect.addEventListener('mousemove',e=>showTip(e,`${item.label}<br><b>${fmtMs(item.value)}</b>`));rect.addEventListener('mouseleave',hideTip);svg.append(rect)});host.append(svg)}
function table(id,mode){let html='<thead><tr><th>Source rows</th><th>XY build</th><th>XY stable render</th><th>XY total</th><th>DragonGUI ready</th><th>DG frame avg</th><th>XY frame p50 / p95*</th><th>XY ready advantage</th></tr></thead><tbody>';for(const row of DATA.rows){const cell=row[mode],x=cell.xy,d=cell.dragongui,interaction=row.n===1000000?`${DATA.interaction.xy_frame_p50_ms.toFixed(2)} / ${DATA.interaction.xy_frame_p95_ms.toFixed(2)} ms`:'—';html+=`<tr><td>${fmtN(row.n)}</td><td>${fmtMs(x.build_ms)}</td><td>${fmtMs(x.stable_ms)}</td><td>${fmtMs(x.ready_ms)}</td><td class="${d.ready_ms==null?'fail':''}">${fmtMs(d.ready_ms)}</td><td>${d.frame_ms==null?'—':d.frame_ms.toFixed(2)+' ms'}</td><td>${interaction}</td><td class="${cell.xy_speedup?'faster':'fail'}">${cell.xy_speedup?cell.xy_speedup.toFixed(1)+'×':'capacity failure'}</td></tr>`}document.getElementById(id).innerHTML=html+'</tbody>'}
const adaptiveXY=DATA.rows.map(r=>r.adaptive.xy.ready_ms),adaptiveDG=DATA.rows.map(r=>r.adaptive.dragongui.ready_ms),exactXY=DATA.rows.map(r=>r.exact.xy.ready_ms),exactDG=DATA.rows.map(r=>r.exact.dragongui.ready_ms);
lineChart('adaptiveTime',[{label:'XY adaptive',color:COLORS.xy,values:adaptiveXY},{label:'DragonGUI adaptive',color:COLORS.dragon,values:adaptiveDG}],{logY:true});
lineChart('exactTime',[{label:'XY exact',color:COLORS.xy,values:exactXY},{label:'DragonGUI exact',color:COLORS.dragon,values:exactDG}],{logY:true});
lineChart('frameTime',[{label:'DragonGUI adaptive',color:COLORS.adaptive,values:DATA.rows.map(r=>r.adaptive.dragongui.frame_ms)},{label:'DragonGUI exact',color:COLORS.exact,values:DATA.rows.map(r=>r.exact.dragongui.frame_ms)}],{maxY:7});
lineChart('memoryChart',[{label:'DragonGUI process',color:COLORS.dragon,values:DATA.rows.map(r=>r.adaptive.dragongui.memory_mib)},{label:'XY adaptive host',color:COLORS.adaptive,values:DATA.rows.map(r=>r.adaptive.xy.host_memory_mib)},{label:'XY exact host',color:COLORS.exact,values:DATA.rows.map(r=>r.exact.xy.host_memory_mib)}],{unit:'MiB'});
barChart('interactionLatency',[{label:'DragonGUI first correct pixels',value:115.83,color:COLORS.dragon},{label:'DragonGUI exact representation sample',value:263.33,color:COLORS.dragon},{label:'XY trusted density settle',value:20.40,color:COLORS.xy},{label:'DragonGUI ten-stable validation',value:1409.34,color:COLORS.exact}]);
table('adaptiveTable','adaptive');table('exactTable','exact');
</script></body></html>'''


if __name__ == "__main__":
    raise SystemExit(main())
