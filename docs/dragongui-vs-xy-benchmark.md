# DragonGUI 1.0.0 vs Reflex XY 0.0.5

**Benchmark date:** 2026-08-05  
**Machine:** Windows 10.0.19045, 16 logical CPUs, CPython 3.13.14  
**Packages:** public PyPI wheels installed into one clean virtual environment  
**Workload:** seeded correlated Gaussian `float32` x/y columns, 8 source bytes per row,
900 x 420 viewport, five planted sentinel rows

## Result

XY is decisively faster to a correct, stable first chart and has a much higher
point ceiling. DragonGUI's advantage is after startup: once its scene exists,
the measured 20-frame averages remain about 3.3-6.2 ms through four million
source points. That is useful for a continuously running native application,
but it does not offset the current five-second cold-start path.

The most important defect is the point ceiling. DragonGUI succeeds at four
million rows but aborts at five million in both exact and LOD modes. It requests
a 320 MB `scatter-vb` buffer against the selected adapter's 256 MiB maximum.
LOD is applied too late to avoid that allocation, so enabling it does not raise
the ingest ceiling.

## Timing results

All cells are medians of three fresh-process runs. Input generation and package
import are excluded. "XY total" is production Python figure/payload build plus
browser navigation-to-correct-and-stable render. The browser gate verifies all
visible sentinels and requires ten byte-identical full-frame readbacks.
"DragonGUI ready" is public widget build plus `App.run()` through 20 validated
native frames; its sub-millisecond public build is omitted at this precision.

### Adaptive modes

These modes are not algorithmically equivalent. XY switches above 200k rows to
a screen-bounded density grid while retaining exact source columns. DragonGUI
uses stride LOD factor 8 and still allocates from the full source count.

| Source rows | XY Python build | XY stable browser | XY total | DragonGUI ready | DragonGUI 20-frame avg | Outcome |
|---:|---:|---:|---:|---:|---:|---|
| 10k | 40.9 ms | 365.5 ms | 406.4 ms | 5,265.8 ms | 3.56 ms | XY 13.0x faster to ready |
| 100k | 43.8 ms | 387.7 ms | 431.5 ms | 5,457.8 ms | 3.64 ms | XY 12.6x faster |
| 1M | 54.5 ms | 337.3 ms | 391.8 ms | 5,499.3 ms | 4.00 ms | XY 14.0x faster |
| 2.5M | 65.9 ms | 335.5 ms | 401.4 ms | 5,526.3 ms | 5.33 ms | XY 13.8x faster |
| 4M | 77.7 ms | 345.9 ms | 423.6 ms | 5,524.1 ms | 6.07 ms | XY 13.0x faster |
| 5M | 92.2 ms | 348.6 ms | 440.8 ms | **failed** | — | DragonGUI buffer-limit abort |

### Exact-marker modes

Both libraries receive every source row and render one point per retained row;
XY uses `density=False`, and DragonGUI uses `lod=False`.

| Source rows | XY Python build | XY stable browser | XY total | DragonGUI ready | DragonGUI 20-frame avg | Outcome |
|---:|---:|---:|---:|---:|---:|---|
| 10k | 39.6 ms | 361.9 ms | 401.5 ms | 5,336.5 ms | 4.26 ms | XY 13.3x faster to ready |
| 100k | 45.1 ms | 403.0 ms | 448.1 ms | 5,415.6 ms | 3.31 ms | XY 12.1x faster |
| 1M | 50.6 ms | 893.7 ms | 944.3 ms | 5,430.0 ms | 3.99 ms | XY 5.8x faster |
| 2.5M | 59.3 ms | 1,702.2 ms | 1,761.5 ms | 5,596.8 ms | 5.07 ms | XY 3.2x faster |
| 4M | 72.3 ms | 2,574.7 ms | 2,647.0 ms | 5,717.8 ms | 6.23 ms | XY 2.2x faster |
| 5M | 79.8 ms | 3,108.8 ms | 3,188.6 ms | **failed** | — | DragonGUI buffer-limit abort |

## Memory

Memory pools are deliberately not collapsed into one ranking. DragonGUI is a
single native process; XY has a Python host and a separate Chrome process tree.
Headless Chrome's roughly 500 MiB floor dominates the adaptive browser rows.

| Source rows | DragonGUI process | XY adaptive host | XY adaptive browser | XY exact host | XY exact browser |
|---:|---:|---:|---:|---:|---:|
| 10k | 169.6 MiB | 52.3 MiB | 522.6 MiB | 52.0 MiB | 522.3 MiB |
| 100k | 172.6 MiB | 54.7 MiB | 555.2 MiB | 54.9 MiB | 558.5 MiB |
| 1M | 258.4 MiB | 75.8 MiB | 519.1 MiB | 82.8 MiB | 704.3 MiB |
| 2.5M | 402.5 MiB | 110.6 MiB | 520.2 MiB | 129.1 MiB | 744.6 MiB |
| 4M | 557.2 MiB | 144.9 MiB | 519.5 MiB | 205.3 MiB | 821.7 MiB |
| 5M | failed | 168.0 MiB | 522.9 MiB | 243.3 MiB | 856.6 MiB |

XY's adaptive payload stays approximately 256 KiB from one to five million
rows. DragonGUI's packed startup input is 12 bytes per point, then its renderer
attempts a 64-byte-per-point vertex allocation. This explains both its steeper
process-memory curve and the five-million-point failure.

## Fairness and limits

- XY's browser ran through SwiftShader in this headless Windows environment;
  DragonGUI used its native wgpu adapter. This biases the browser timing against
  XY, not in its favor.
- XY's stable-render gate is stronger: it checks planted pixels and ten stable
  readbacks. DragonGUI validates source count, native update receipt, application
  presentation, and 20 completed frames, but does not read pixels back.
- The adaptive modes answer the same user goal but use different algorithms.
  The exact table is the stricter engine-to-engine comparison.
- DragonGUI's frame average includes its startup outliers among 20 frames. Its
  retained render-cache steady frames are often below 1 ms; use the raw JSON for
  p50/p95 and native stage details.
- This is one Windows machine and three repetitions, not a universal ranking.

## Current-branch interaction follow-up (2026-08-06)

DragonGUI now has a separate 1M adaptive gate using real Windows wheel, pan,
resize, Home, and rectangle-selection input. It proves each gesture changed the
intended semantic state, verifies the selected exact source row, checks planted
pixels, drains the command queue, and requires ten identical readbacks. That
run passed: the last input reached correct pixels in 244.96 ms and ten stable
captures in 340.56 ms; frame p50 / p95 / maximum were 4.74 / 12.01 / 20.21 ms.

XY's upstream browser gesture phase was also re-enabled. A benchmark-only MIME
shim was required because its live host served `index.js` as `text/plain` on
this Windows configuration. JavaScript-created `WheelEvent` objects did not
zoom, so the wrapper gained a trusted CDP `mouseWheel` path. On this Windows
headless setup it must use SwiftShader: after hardware-path WebGL recovery, XY
0.0.5's installed wheel closure retains a zero-size stale canvas even though
the current canvas receives the event. The software path preserves valid
geometry and is the one reported here.

A stricter DragonGUI wheel-only gate now uses XY's exact seed, correlated
Gaussian source, five planted sentinels, 900×420 viewport, target, 42 inputs,
and 33 ms cadence. This exposed and fixed center-only 2D zoom: DragonGUI now
translates the orthographic camera target so the data point beneath the pointer
remains fixed. With real Windows wheel input, the 1M adaptive run passed. Its
final span was 0.004659 of home, the target sentinel remained lit, correct
pixels arrived 121.94 ms after the last input, and ten consecutive identical
correct frames completed at 1,429.47 ms. The final adaptive product was one
exact visible source row; maximum schedule lag was 1.06 ms.

The matched XY 1M density run also passed. CDP delivered all 42 trusted wheel
events; XY queued and applied all 42 view changes, kept the fixed data target
under the cursor, reduced the final x-span to 0.0005 of home, and reached its
correct ten-frame-stable stop 20.40 ms after input delivery. Total time from
first input through the final stable view was 1,383.20 ms. Gesture-frame p50 /
p95 / maximum were 4.10 / 5.30 / 21.20 ms, and maximum driver schedule lag was
14.13 ms.

The split-timing rerun shows why that single number was misleading. DragonGUI
applied the exact visible representation at 263.33 ms after the last input and
showed correct pixels at 93.06 ms. The 12 validation captures totaled 161.20 ms
(13.43 ms average, 21.49 ms p95), while 11 debug-snapshot queries totaled
97.23 ms. Ten-stable completion was 1,409.34 ms. The remaining interval is
validation-loop pacing/orchestration, not 1.4 seconds of scatter rendering.
XY's 20.40 ms settle uses browser frame capture and is not an isolated measure
of the same work. The next optimization should batch or asynchronously sample
stability rather than tune the renderer against the combined wall-clock value.

Split-timing artifact:

- `artifacts/xy-benchmark/scatter-xy-wheel-comparison-adaptive-1m-split-timing.json`

A follow-up timeline run recorded every stable sample. Exact representation was
already applied at 15.66 ms and first correct pixels arrived at 113.94 ms;
capture and snapshot work totaled only 115.49 ms and 67.55 ms respectively.
The nine stability samples nevertheless arrived at 287.0, 444.9, 607.3, 739.7,
858.6, 980.6, 1,105.5, 1,222.4, and 1,342.0 ms after the last input—roughly
117–162 ms between samples. That confirms the long tail is validation-loop
wakeup/orchestration behavior, not scatter rendering.

Timeline artifact:

- `artifacts/xy-benchmark/scatter-xy-wheel-comparison-adaptive-1m-validation-timeline.json`

The optional `--batched-stability` probe then removed all per-sample semantic
snapshots and performed one final runtime check. It still completed in
1,484.48 ms, with 136.70 ms total screenshot work and zero per-sample snapshot
time; sample spacing remained roughly 110–120 ms. The dominant pacing is
therefore the synchronous screenshot/readback request path itself. This is the
next native improvement target: expose an asynchronous or batched screenshot
readback that can validate multiple frame hashes without blocking each render
request behind a full command round trip.

Batched-probe artifact:

- `artifacts/xy-benchmark/scatter-xy-wheel-comparison-adaptive-1m-batched-stability.json`

The XY-inspired frame-generation probe is the first meaningful validation-tail
improvement. It waited for nine additional completed native frame generations,
then performed one final screenshot and semantic snapshot. The 1M run passed in
340.16 ms after the last input, with first correct pixels at 121.68 ms and only
17.07 ms of final capture work. This preserves the correctness gate while
removing repeated screenshot transport from the stability clock.

Frame-generation artifact:

- `artifacts/xy-benchmark/scatter-xy-wheel-comparison-adaptive-1m-frame-generation.json`

### Fair latency view

Use first-correct pixels and exact-representation application as the primary
interaction metrics. The latest DragonGUI split run showed correct pixels at
113.94 ms and exact representation at 263.33 ms in that sample; the batched
run showed correct pixels at 115.83 ms. XY's trusted browser run reached its
final density settle in 20.40 ms after the gesture window, but that is not
directly equivalent to DragonGUI's native exact handoff. DragonGUI's 1.3–1.5 s
ten-stable values are validation diagnostics, not renderer latency, because
screenshot/readback requests arrive roughly every 110–120 ms.

This remains a directional interaction comparison, not a pure renderer league
table. DragonGUI used real Windows input and native wgpu hardware; XY used
trusted CDP input and headless SwiftShader. Their wheel sensitivities also
produce different final spans (0.004659 versus 0.0005), although both runs use
the same source, target, viewport, 42-input/33 ms schedule, zoom proof, and
correct-and-ten-stable stop rule.

## Priorities exposed by the benchmark

1. Chunk or cap scatter vertex buffers by `max_buffer_size`; never allow a wgpu
   validation panic to cross the Python API.
2. Apply LOD before allocating the full 64-byte-per-point render buffer, or use
   a compact GPU point format for the primary scene.
3. Instrument the roughly 5.2-second payload queue/startup gap and publish an
   exact first-application-present timestamp.
4. Add pixel correctness/readback to DragonGUI's benchmark mode so the same
   correct-and-stable stop rule can be used on both libraries.
5. Add scripted 2D pan/zoom input and mutation-to-present latency. This run only
   establishes load, ceiling, memory, and native frame cost.

## Reproduction

Create an isolated environment and install released packages:

```powershell
python -m venv artifacts\xy-benchmark-venv
artifacts\xy-benchmark-venv\Scripts\python.exe -m pip install `
  dragongui==1.0.0 xy==0.0.5 numpy psutil tornado websocket-client
git clone --depth 1 https://github.com/reflex-dev/xy.git artifacts\xy-source
```

Run DragonGUI's fresh-process matrix:

```powershell
artifacts\xy-benchmark-venv\Scripts\python.exe `
  benchmarks\run_dragongui_xy_matrix.py `
  --sizes 10000,100000,1000000,2500000,4000000,5000000 `
  --repetitions 3 --out artifacts\dragongui-xy.json
```

Run XY's upstream correct-and-stable load engine three times, alternating arm
order between runs:

```powershell
artifacts\xy-benchmark-venv\Scripts\python.exe `
  benchmarks\run_xy_load_benchmark.py `
  --xy-source artifacts\xy-source `
  --chrome "C:\Program Files\Google\Chrome\Application\chrome.exe" `
  --arms xy,xy-exact --out artifacts\xy-load-1.json
```

Enable XY's upstream fixed-cadence gesture and final zoom-correctness phase:

```powershell
artifacts\xy-benchmark-venv\Scripts\python.exe `
  benchmarks\run_xy_load_benchmark.py `
  --xy-source artifacts\xy-source --sizes 1000000 --arms xy `
  --chrome "C:\Program Files\Google\Chrome\Application\chrome.exe" `
  --software --trusted-wheel `
  --out artifacts\xy-benchmark\xy-browser-trusted-wheel-adaptive-1m.json
```

The wrapper applies a JavaScript MIME compatibility header to the live host and
adapts XY's upstream driver in memory to deliver trusted CDP wheel input. It
does not modify the XY checkout or installed package. The upstream dataset,
gesture schedule, correctness proof, and stability rule remain in force.

Run DragonGUI's matched-data trusted-wheel gate:

```powershell
artifacts\xy-benchmark-venv\Scripts\python.exe `
  benchmarks\scatter_xy_wheel_comparison_case.py `
  --n 1000000 --package-root python `
  --out artifacts\xy-benchmark\scatter-xy-wheel-comparison-adaptive-1m.json
```

Measure XY's Python-side production payload build separately:

```powershell
artifacts\xy-benchmark-venv\Scripts\python.exe `
  benchmarks\run_xy_python_build_matrix.py `
  --repetitions 3 --out artifacts\xy-python-build.json
```

Upstream method references: [XY repository](https://github.com/reflex-dev/xy),
[benchmark runbook](https://github.com/reflex-dev/xy/blob/main/benchmarks/README.md),
and [XY architecture](https://reflex.dev/docs/xy/advanced).
