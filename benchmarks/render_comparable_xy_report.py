"""Build a conservative, directly-comparable DragonGUI/XY report."""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DG = ROOT / "artifacts" / "dragongui-xy-matrix.json"
XY_BUILD = ROOT / "artifacts" / "xy-python-build.json"
XY_LOAD = ROOT / "artifacts" / "xy-load-run2.json"
OUT = ROOT / "docs" / "dragongui-xy-comparable.html"


def median(values):
    values = sorted(values)
    return values[len(values) // 2] if values else None


def main():
    dg = json.loads(DG.read_text(encoding="utf-8"))
    xy_build = json.loads(XY_BUILD.read_text(encoding="utf-8"))
    xy_load = json.loads(XY_LOAD.read_text(encoding="utf-8"))
    dg_cells = {(c["n"], c["mode"]): c for c in dg["summaries"]}
    xb = {(c["n"], c["mode"]): c for c in xy_build["summaries"]}
    xy_rows = {(r["n"], r["arm"]): r for r in xy_load["results"] if r["status"] == "ok"}
    rows = []
    for n in sorted(set(dg["sizes"]) & set(xy_build["sizes"])):
        for mode, arm in (("adaptive", "xy"), ("exact", "xy-exact")):
            d = dg_cells.get((n, mode), {})
            x = xb.get((n, mode), {})
            xr = xy_rows.get((n, arm), {})
            ds = [s for s in d.get("render_wall_ms", {}).get("samples", []) if s.get("status") == "ok"]
            rows.append({
                "n": n, "mode": mode,
                "dg_build": median([s.get("public_build_ms") for s in ds if s.get("public_build_ms") is not None]),
                "xy_build": x.get("python_build_ms", {}).get("median"),
                "dg_memory": d.get("peak_rss_bytes", {}).get("median", 0) / 2**20,
                "xy_memory": xr.get("python_peak_rss_bytes", 0) / 2**20,
                "dg_ok": bool(d.get("render_wall_ms", {}).get("successful_runs")),
                "xy_ok": bool(xr),
            })
    table = "".join(f"<tr><td>{r['n']:,}</td><td>{r['mode']}</td><td>{r['dg_build']:.2f} ms</td><td>{r['xy_build']:.2f} ms</td><td>{r['dg_memory']:.1f} MiB</td><td>{r['xy_memory']:.1f} MiB</td><td>{'pass' if r['dg_ok'] else 'fail'}</td><td>{'pass' if r['xy_ok'] else 'fail'}</td></tr>" for r in rows)
    html = f'''<!doctype html><meta charset="utf-8"><title>Comparable DragonGUI vs XY Benchmark</title>
<style>body{{font:15px system-ui;background:#071116;color:#e4f0ed;max-width:1100px;margin:40px auto;padding:0 20px}}h1{{font-size:3.5rem;line-height:1}}.card{{background:#0d2028;border:1px solid #24424c;border-radius:14px;padding:22px;margin:18px 0}}table{{width:100%;border-collapse:collapse}}th,td{{padding:10px;border-bottom:1px solid #24424c;text-align:right}}th:first-child,td:first-child,th:nth-child(2),td:nth-child(2){{text-align:left}}.note{{color:#9bb4b1}}.pass{{color:#62d49b}}</style>
<h1>Comparable metrics only</h1><p class="note">DragonGUI vs XY · same point-count matrix · metrics below are intentionally limited to shared measurements.</p>
<div class="card"><h2>Direct comparison</h2><table><thead><tr><th>Rows</th><th>Mode</th><th>DG build</th><th>XY build</th><th>DG host memory</th><th>XY host memory</th><th>DG output</th><th>XY output</th></tr></thead><tbody>{table}</tbody></table></div>
<div class="card"><h2>What is excluded</h2><p class="note">Native-vs-browser frame timing, screenshot/readback latency, browser navigation readiness, and ten-stable completion are not scored here because their clocks and capture mechanisms differ.</p><p class="pass">This report answers: can both libraries ingest and produce a successful result for the same sizes, and what are their comparable Python build and host-memory costs?</p></div>'''
    OUT.write_text(html, encoding="utf-8")
    print(f"Wrote {OUT}")


if __name__ == "__main__":
    main()
