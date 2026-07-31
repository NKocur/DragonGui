"""Render the sustained GUI matrix as a self-contained HTML report."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("summary", type=Path)
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "plans" / "gui-framework-sustained-performance-report.html",
    )
    args = parser.parse_args()
    data = json.loads(args.summary.read_text(encoding="utf-8"))
    embedded = json.dumps(data, separators=(",", ":")).replace("</", "<\\/")
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
<title>DragonGUI sustained performance study</title>
<style>
:root{--bg:#081018;--panel:#101c28;--panel2:#142434;--ink:#e9f3fb;--muted:#94a9ba;--line:#294156;--dragon:#5ee6b5;--dpg:#6fb2ff;--qt:#ffbc66;--tk:#d98cff;--bad:#ff7e8b;--good:#5ee6b5}
*{box-sizing:border-box}html{background:var(--bg);color:var(--ink);font:15px/1.52 Inter,Segoe UI,Arial,sans-serif}body{margin:0;background:radial-gradient(circle at 78% 0,#16304a 0,transparent 34rem),var(--bg)}
main{width:min(1460px,calc(100% - 40px));margin:auto;padding:48px 0 80px}.eyebrow{color:var(--dragon);font-size:.78rem;font-weight:800;letter-spacing:.16em;text-transform:uppercase}h1{max-width:1000px;margin:.3rem 0 .6rem;font-size:clamp(2.2rem,5vw,4.8rem);line-height:.98;letter-spacing:-.055em}h2{margin:0 0 8px;font-size:1.55rem}h3{margin:0 0 7px;font-size:1.03rem}.lede{max-width:930px;color:#bbcad6;font-size:1.14rem}.meta{display:flex;gap:12px;flex-wrap:wrap;margin:24px 0 34px}.pill{border:1px solid var(--line);background:#0d1924cc;border-radius:999px;padding:6px 11px;color:#c1d2df;font-size:.83rem}
.grid{display:grid;grid-template-columns:repeat(12,1fr);gap:16px}.card{border:1px solid var(--line);background:linear-gradient(145deg,#122130e8,#0d1823f2);border-radius:16px;padding:20px;box-shadow:0 16px 50px #0004}.metric{grid-column:span 3;min-height:148px}.metric .big{font-size:2rem;font-weight:800;letter-spacing:-.04em}.metric p,.note,p{color:var(--muted);margin:.4rem 0 0}.good{color:var(--good)}.bad{color:var(--bad)}
.section{margin-top:38px}.chart-card{grid-column:span 6;min-height:410px}.wide{grid-column:1/-1}.chart{height:300px;margin-top:12px}.chart svg{width:100%;height:100%;overflow:visible}.axis{stroke:#496075;stroke-width:1}.gridline{stroke:#203548;stroke-width:1}.tick{fill:#8fa5b7;font-size:11px}.legend{display:flex;flex-wrap:wrap;gap:14px;margin-top:8px;color:#a9bac8;font-size:.8rem}.key{display:inline-block;width:9px;height:9px;border-radius:50%;margin-right:6px}.tooltip{position:fixed;pointer-events:none;display:none;background:#02070bdd;border:1px solid #45627a;border-radius:8px;padding:8px 10px;font-size:.78rem;z-index:10;box-shadow:0 8px 30px #0009}
table{width:100%;border-collapse:collapse;margin-top:12px;font-size:.88rem}th,td{padding:9px 10px;text-align:right;border-bottom:1px solid #24394b}th:first-child,td:first-child{text-align:left}th{color:#b8c9d5;font-size:.74rem;text-transform:uppercase;letter-spacing:.06em}td{color:#d6e2eb}.callouts{display:grid;grid-template-columns:repeat(3,1fr);gap:16px}.callout{padding:18px;border-left:3px solid var(--dragon);background:#102131;border-radius:0 12px 12px 0}.callout.warn{border-color:var(--qt)}.callout.risk{border-color:var(--bad)}ul{margin:.5rem 0 0;padding-left:1.2rem;color:#acbecb}li+li{margin-top:.45rem}a{color:#86c9ff}.small{font-size:.82rem;color:#879cab}.rank{font-size:.75rem;color:#8398aa;text-transform:uppercase;letter-spacing:.08em}.unsupported{color:#718696;font-style:italic}
@media(max-width:1000px){.metric,.chart-card{grid-column:span 6}.callouts{grid-template-columns:1fr}}@media(max-width:680px){main{width:min(100% - 24px,1460px);padding-top:28px}.metric,.chart-card{grid-column:1/-1}.grid{gap:12px}.card{padding:15px}.chart{height:260px}}
</style>
</head>
<body><main>
<div class="eyebrow">Extended framework benchmark · July 31, 2026</div>
<h1>The Rust/GPU payoff appears under load.</h1>
<p class="lede">A second, broader benchmark of DragonGUI, Dear PyGui, PyQt6, and Tkinter. This study de-emphasizes import and first-window time and measures scaling, sustained mutations, resizing, live CSS, 1k–100k point plots, and 1k–100k row data models.</p>
<div class="meta"><span class="pill">113 isolated processes</span><span class="pill">2 repetitions · tables corrected with 3</span><span class="pill">60 Hz pacing</span><span class="pill">Windows · Python 3.12</span><span class="pill">lower is better throughout</span></div>

<section class="grid">
 <article class="card metric"><div class="rank">100k-point replacement</div><div class="big good">3.00 ms</div><p>DragonGUI Python submission p50; Dear PyGui 3.44 ms, custom PyQt painter 72.14 ms.</p></article>
 <article class="card metric"><div class="rank">400 mutations / tick</div><div class="big good">0.84 ms</div><p>DragonGUI observed active-frame p95; Dear PyGui 1.43 ms, PyQt6 13.46 ms, Tk 84.21 ms.</p></article>
 <article class="card metric"><div class="rank">2,502-widget steady work</div><div class="big good">0.62 ms</div><p>DragonGUI p50 versus Dear PyGui 0.83 ms and PyQt6 2.26 ms.</p></article>
 <article class="card metric"><div class="rank">2,502-widget memory</div><div class="big bad">530 MiB</div><p>DragonGUI’s largest obvious deficit: Dear PyGui 94 MiB and PyQt6 78 MiB.</p></article>
</section>

<section class="section"><div class="eyebrow">Scaling curves</div><h2>Where architectures cross over</h2><p class="note">Static Tk frame timings are shown for completeness but are not a clean redraw comparison; Tk only paints invalidated regions. Hover chart points for exact values.</p>
<div class="grid">
 <article class="card chart-card"><h3>Steady active work vs control rows</h3><div id="controlChart" class="chart"></div><div class="legend" data-legend="all"></div></article>
 <article class="card chart-card"><h3>Active p95 vs mutations per tick</h3><div id="mutationChart" class="chart"></div><div class="legend" data-legend="all"></div></article>
 <article class="card chart-card"><h3>Line replacement API submission p50</h3><div id="lineSubmitChart" class="chart"></div><div class="legend" data-legend="special"></div></article>
 <article class="card chart-card"><h3>Line workload active-frame p95</h3><div id="lineFrameChart" class="chart"></div><div class="legend" data-legend="special"></div></article>
 <article class="card chart-card"><h3>Public widget-tree build vs control rows</h3><div id="buildChart" class="chart"></div><div class="legend" data-legend="all"></div></article>
 <article class="card chart-card"><h3>Resident memory vs control rows</h3><div id="memoryChart" class="chart"></div><div class="legend" data-legend="all"></div></article>
</div></section>

<section class="section"><div class="eyebrow">Dynamic systems</div><h2>CSS and resize pressure</h2>
<div class="grid">
 <article class="card chart-card"><h3>200-row restyle: observed p95</h3><div id="restyleChart" class="chart"></div><p class="small">DragonGUI’s API call is asynchronous. Its 0.12 ms submission is not plotted as completed work; native style reapply p50 was 7.40 ms and layout p50 2.10 ms.</p></article>
 <article class="card chart-card"><h3>200-row alternating resize: observed p95</h3><div id="resizeChart" class="chart"></div><p class="small">Resize semantics differ by toolkit and OS. DragonGUI’s active-work sampler excludes time spent blocked inside the platform resize request, so this is diagnostic—not an end-user latency ranking.</p></article>
</div></section>

<section class="section"><div class="eyebrow">Large data</div><h2>Virtualized table capacity</h2><p class="note">DragonGUI and Qt use data-backed/virtualized views. Dear PyGui’s public table rows are eagerly authored; only 1,000 rows is included and larger points are explicitly unsupported.</p>
<article class="card wide"><div class="grid"><div style="grid-column:span 7"><div id="tableChart" class="chart"></div></div><div style="grid-column:span 5;overflow:auto"><table id="tableResults"></table></div></div></article></section>

<section class="section"><div class="eyebrow">DragonGUI internals</div><h2>What the native pipeline says</h2>
<article class="card wide" style="overflow:auto"><table id="nativeTable"></table></article></section>

<section class="section"><div class="callouts">
 <article class="callout"><h3>Demonstrated strengths</h3><ul><li>Public construction scales unusually well: 8.48 ms for 2,502 widgets.</li><li>Packed line updates remain competitive through 100k points.</li><li>Sustained mutation frame work stays below Dear PyGui at the tested 20, 100, and 400 mutation batches.</li><li>The virtualized table handles 100k rows without frame-time growth proportional to row count.</li></ul></article>
 <article class="callout warn"><h3>Optimization priorities</h3><ul><li>Reduce native retained memory per widget; dense controls are 5.7× Dear PyGui at 500 rows.</li><li>Incrementalize first/full cascade work: style reapply reaches 17.99 ms at 2,502 widgets.</li><li>Investigate platform resize dispatch: raw command-drain samples include 150–650 ms blocking outliers.</li><li>Separate operation-window telemetry from startup/idle frames for stricter asynchronous latency accounting.</li></ul></article>
 <article class="callout risk"><h3>Still outside this report</h3><ul><li>GPU/CPU utilization and energy consumption.</li><li>Input-to-photon latency, scrolling, text editing, and accessibility.</li><li>Images, heatmaps/scatter, multi-window work, terminal throughput, and long-duration memory stability.</li><li>Cross-machine, Linux/macOS, integrated-GPU, and release-wheel validation.</li></ul></article>
</div></section>

<section class="section card"><div class="eyebrow">Method and interpretation</div><h2>Fair enough to guide work, not a universal leaderboard</h2>
<p>Every sample ran in a fresh process; framework order rotated by case and repetition. Dynamic cases used 30 operations (tables used 10), paced toward 60 Hz. General values are medians of two process samples; the corrected table family uses three. DragonGUI operations begin after <code>application_frame_presented</code>. Its queued Python submission, native command drain, style/layout telemetry, and frame-work sampler are reported separately.</p>
<p>Qt line rendering uses a small public <code>QWidget</code>/<code>QPainter.drawPolyline</code> implementation rather than a third-party plotting package. Qt tables use <code>QAbstractTableModel</code>/<code>QTableView</code>. These are appropriate public primitives, but the plot result is not a claim about every Qt plotting library. Dear PyGui uses its native line series. Tk participates only in the common control cases.</p>
<p>Architecture references: <a href="https://dearpygui.readthedocs.io/en/latest/about/what-why.html">Dear PyGui’s retained/GPU model</a>, <a href="https://doc.qt.io/qt-6/modelview.html">Qt model/view</a>, <a href="https://doc.qt.io/qt-6/qabstracttablemodel.html">QAbstractTableModel</a>, and <a href="https://doc.qt.io/qt-6/qpainter.html">QPainter polyline APIs</a>.</p>
<p class="small">Reproduce the core with <code>py -3.12 benchmarks/run_gui_sustained_matrix.py --repetitions 2</code>; rerun tables with three repetitions and <code>--family table_model --base-summary &lt;core-summary&gt;</code>. Then render with <code>benchmarks/render_gui_sustained_report.py</code>. Raw results remain under ignored <code>artifacts/</code>.</p>
</section>
</main><div id="tooltip" class="tooltip"></div>
<script>
const DATA=__BENCHMARK_DATA__;
const COLORS={dragongui:'#5ee6b5',dearpygui:'#6fb2ff',pyqt6:'#ffbc66',tkinter:'#d98cff'};
const LABELS={dragongui:'DragonGUI',dearpygui:'Dear PyGui',pyqt6:'PyQt6',tkinter:'Tkinter'};
const tooltip=document.getElementById('tooltip');
function cases(family){return Object.values(DATA.results).filter(x=>x.family===family).sort((a,b)=>a.scale-b.scale)}
function val(c,fw,key){const x=c.frameworks[fw];return x&&x.supported?x[key]:null}
function svgEl(name,attrs={}){const e=document.createElementNS('http://www.w3.org/2000/svg',name);for(const[k,v]of Object.entries(attrs))e.setAttribute(k,v);return e}
function lineChart(id, family, metric, opts={}){
 const host=document.getElementById(id), cs=cases(family), fws=opts.frameworks||Object.keys(COLORS), W=620,H=290,m={l:58,r:20,t:16,b:44}, transform=opts.transform||((x)=>x), unit=opts.unit||'ms';
 const xs=cs.map(c=>c.scale), values=fws.flatMap(f=>cs.map(c=>val(c,f,metric))).filter(v=>v!=null).map(transform), ymax=(Math.max(...values,1))*1.12;
 const logx=!!opts.logx, xmin=Math.min(...xs), xmax=Math.max(...xs), xp=x=>m.l+(logx?(Math.log10(x)-Math.log10(xmin))/(Math.log10(xmax)-Math.log10(xmin)):(x-xmin)/(xmax-xmin||1))*(W-m.l-m.r), yp=y=>H-m.b-y/ymax*(H-m.t-m.b);
 const svg=svgEl('svg',{viewBox:`0 0 ${W} ${H}`,role:'img'});for(let i=0;i<=4;i++){let y=H-m.b-i/4*(H-m.t-m.b),v=i/4*ymax;svg.append(svgEl('line',{x1:m.l,y1:y,x2:W-m.r,y2:y,class:'gridline'}));let t=svgEl('text',{x:m.l-8,y:y+4,'text-anchor':'end',class:'tick'});t.textContent=(v>=100?v.toFixed(0):v>=10?v.toFixed(1):v.toFixed(2));svg.append(t)}
 svg.append(svgEl('line',{x1:m.l,y1:H-m.b,x2:W-m.r,y2:H-m.b,class:'axis'}));for(const x of xs){let t=svgEl('text',{x:xp(x),y:H-17,'text-anchor':'middle',class:'tick'});t.textContent=x>=1000?(x/1000)+'k':x;svg.append(t)}
 for(const fw of fws){const pts=cs.map(c=>[xp(c.scale),yp(transform(val(c,fw,metric))),c]).filter(p=>val(p[2],fw,metric)!=null);if(!pts.length)continue;svg.append(svgEl('polyline',{points:pts.map(p=>p.slice(0,2).join(',')).join(' '),fill:'none',stroke:COLORS[fw],'stroke-width':2.5}));for(const[x,y,c]of pts){const dot=svgEl('circle',{cx:x,cy:y,r:4.5,fill:COLORS[fw],stroke:'#071018','stroke-width':2});dot.addEventListener('mousemove',e=>showTip(e,`${LABELS[fw]} · ${c.scale.toLocaleString()}<br><b>${Number(transform(val(c,fw,metric))).toFixed(3)} ${unit}</b>`));dot.addEventListener('mouseleave',hideTip);svg.append(dot)}}host.append(svg)
}
function bars(id,family,scale,metric){const host=document.getElementById(id),c=cases(family).find(x=>x.scale===scale),fws=Object.keys(c.frameworks),W=620,H=290,m={l:58,r:20,t:16,b:52};const values=fws.map(f=>val(c,f,metric)).filter(v=>v!=null),max=Math.max(...values,1)*1.12,slot=(W-m.l-m.r)/fws.length;const svg=svgEl('svg',{viewBox:`0 0 ${W} ${H}`});for(let i=0;i<=4;i++){let y=H-m.b-i/4*(H-m.t-m.b),v=i/4*max;svg.append(svgEl('line',{x1:m.l,y1:y,x2:W-m.r,y2:y,class:'gridline'}));let t=svgEl('text',{x:m.l-8,y:y+4,'text-anchor':'end',class:'tick'});t.textContent=v.toFixed(v>=10?0:1);svg.append(t)}fws.forEach((fw,i)=>{const v=val(c,fw,metric);if(v==null)return;const h=v/max*(H-m.t-m.b),x=m.l+i*slot+slot*.2,y=H-m.b-h;const rect=svgEl('rect',{x,y,width:slot*.6,height:h,rx:5,fill:COLORS[fw]});rect.addEventListener('mousemove',e=>showTip(e,`${LABELS[fw]}<br><b>${v.toFixed(3)} ms</b>`));rect.addEventListener('mouseleave',hideTip);svg.append(rect);let t=svgEl('text',{x:x+slot*.3,y:H-24,'text-anchor':'middle',class:'tick'});t.textContent=LABELS[fw];svg.append(t)});host.append(svg)}
function showTip(e,s){tooltip.innerHTML=s;tooltip.style.display='block';tooltip.style.left=(e.clientX+12)+'px';tooltip.style.top=(e.clientY+12)+'px'}function hideTip(){tooltip.style.display='none'}
function legends(){document.querySelectorAll('[data-legend]').forEach(el=>{const f=el.dataset.legend==='all'?Object.keys(COLORS):['dragongui','dearpygui','pyqt6'];el.innerHTML=f.map(x=>`<span><i class="key" style="background:${COLORS[x]}"></i>${LABELS[x]}</span>`).join('')})}
function tableResults(){const rows=cases('table_model'),fws=['dragongui','dearpygui','pyqt6'];let h='<thead><tr><th>Rows</th>'+fws.map(f=>`<th>${LABELS[f]} build</th>`).join('')+'</tr></thead><tbody>';for(const c of rows){h+=`<tr><td>${c.scale.toLocaleString()}</td>`+fws.map(f=>{const x=c.frameworks[f];return x.supported?`<td>${x.build_ms.toFixed(2)} ms</td>`:`<td class="unsupported">unsupported</td>`}).join('')+'</tr>'}document.getElementById('tableResults').innerHTML=h+'</tbody>'}
function nativeTable(){const wanted=['control_scale-50','control_scale-200','control_scale-500','mutation_scale-400','restyle-200','line_replace-100000','table_model-100000'];let h='<thead><tr><th>Case</th><th>widgets</th><th>command p95</th><th>style p50</th><th>layout p50</th><th>text p50</th></tr></thead><tbody>';for(const name of wanted){const x=DATA.results[name].frameworks.dragongui,n=x.native;h+=`<tr><td>${name.replaceAll('_',' ')}</td><td>${n.widget_count??'—'}</td><td>${n.command_drain_p95_ms?.toFixed(2)??'—'} ms</td><td>${n.style_reapply_p50_ms?.toFixed(2)??'—'} ms</td><td>${n.layout_compute_p50_ms?.toFixed(2)??'—'} ms</td><td>${n.text_rebuild_p50_ms?.toFixed(2)??'—'} ms</td></tr>`}document.getElementById('nativeTable').innerHTML=h+'</tbody>'}
legends();lineChart('controlChart','control_scale','active_frame_p50_ms');lineChart('mutationChart','mutation_scale','active_frame_p95_ms');lineChart('lineSubmitChart','line_replace','submit_p50_ms',{frameworks:['dragongui','dearpygui','pyqt6'],logx:true});lineChart('lineFrameChart','line_replace','active_frame_p95_ms',{frameworks:['dragongui','dearpygui','pyqt6'],logx:true});lineChart('buildChart','control_scale','build_ms');lineChart('memoryChart','control_scale','rss_runtime_last_bytes',{transform:x=>x/1048576,unit:'MiB'});bars('restyleChart','restyle',200,'active_frame_p95_ms');bars('resizeChart','resize',200,'active_frame_p95_ms');lineChart('tableChart','table_model','build_ms',{frameworks:['dragongui','dearpygui','pyqt6'],logx:true});tableResults();nativeTable();
</script></body></html>'''


if __name__ == "__main__":
    raise SystemExit(main())
