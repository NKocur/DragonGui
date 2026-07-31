# DragonGUI Cross-Framework Performance Comparison

**Benchmark date:** 2026-07-31
**Status:** Initial controls/update matrix complete
**Machine:** Windows 11 24H2, Intel Core Ultra 7 265H, Python 3.12.10

## Executive summary

DragonGUI's steady rendering and live-control update performance are already
competitive with efficient Python GUI frameworks. In this matrix it is much
closer to Dear PyGui than to PyQt6 or Tkinter during active rendering, and it
keeps all four workloads below a 2 ms render-work p95. The corrected update
test completed every requested post-startup batch and retained zero layout
diagnostics.

The strongest current peer is Dear PyGui:

- Dear PyGui has the lowest continuously rendered static-frame cost.
- DragonGUI's dense-control median is 1.09 ms versus Dear PyGui's 0.68 ms.
- DragonGUI's corrected update render p95 is 0.79 ms versus 1.09 ms for Dear
  PyGui, but DragonGUI also performs command draining outside that render
  timer. Its command-drain p95 is approximately 2.49 ms, so the complete
  update pipeline is conservatively around 2–3 ms rather than 0.79 ms.
- Dear PyGui applies the same 100 value mutations in 0.15 ms median;
  DragonGUI enqueues them in 0.22 ms median before the native drain.

DragonGUI performed substantially less active work than PyQt6 in these forced
control repaint/update cases. It also handled dynamic updates much better than
Tkinter. Those results do not mean DragonGUI is categorically faster than Qt:
Qt has mature invalidation, model/view, accessibility, platform integration,
and many workloads not represented by this first matrix.

The clear DragonGUI weaknesses are startup and memory:

- Importing DragonGUI takes approximately 135–151 ms, versus 15–19 ms for
  Dear PyGui/Tkinter and 33–53 ms for PyQt6.
- A fresh DragonGUI event-loop lifecycle spends roughly 3–3.6 seconds beyond
  the deliberately paced frame interval. The benchmark disables the optional
  loading screen, so this is native GPU/runtime initialization rather than an
  authored splash delay.
- Live process RSS is high and variable: approximately 199 MiB for an empty
  window, 279 MiB for 50 rows, and a median 435 MiB for 200 rows. One of the
  three dense runs ended near 207 MiB while the other two exceeded 430 MiB,
  indicating adapter/driver reservation or resource-lifetime variance that
  requires instrumentation before attributing every byte to retained widgets.
- Dear PyGui remained near 92–95 MiB, PyQt6 near 70–75 MiB, and Tkinter near
  48–52 MiB in the same process-RSS measurement.

The overall position is therefore:

1. **Steady GPU UI work:** competitive; generally second to Dear PyGui.
2. **Batched live updates:** competitive and ahead of Qt/Tk in this case, but
   command-drain time must remain part of the DragonGUI number.
3. **Python-side declarative construction:** very inexpensive, although
   DragonGUI defers native document parsing and initial layout until `run()`.
4. **Startup:** currently poor and insufficiently instrumented.
5. **Memory:** currently the largest comparative concern.

## Frameworks

| Framework | Version | Architecture represented here |
|---|---:|---|
| DragonGUI | 0.1.0 | Retained Python tree; Rust layout/text; wgpu rendering |
| Dear PyGui | 2.3.1 | Retained public API over an immediate-mode, GPU-rendered core |
| PyQt6 | 6.11.0 | Qt Widgets retained object tree and event loop |
| Tkinter | Tk 8.6 | Python binding to Tcl/Tk widgets |

Dear PyGui is the most relevant performance peer. Its official documentation
describes a retained public API over an immediate-mode GPU implementation and
exposes a manually controlled render loop. Qt Widgets uses retained `QWidget`
objects, layouts, size hints/policies, and an event system that compresses
posted paint and resize events. Tkinter is included as a lightweight desktop
baseline, not as a GPU-toolkit equivalent.

References:

- [Dear PyGui: What & Why](https://dearpygui.readthedocs.io/en/latest/about/what-why.html)
- [Dear PyGui render loop](https://dearpygui.readthedocs.io/en/latest/documentation/render-loop.html)
- [Qt Widgets overview](https://doc.qt.io/qt-6/qtwidgets-index.html)
- [Qt event system](https://doc.qt.io/qt-6/eventsandfilters.html)
- [Qt layout management](https://doc.qt.io/qt-6/layout.html)
- [Python Tkinter threading/event-loop model](https://docs.python.org/3/library/tkinter.html#threading-model)

## Workloads

Every non-empty row contains the same logical controls:

1. row container;
2. label;
3. single-line text input;
4. button;
5. progress bar.

All rows are placed in a vertically scrollable 1000 × 720 window.

| Case | Rows | Logical controls | Frames | Update frames |
|---|---:|---:|---:|---:|
| Empty startup | 0 | 0 | 30 | 0 |
| Moderate controls | 50 | 200 | 60 | 0 |
| Dense controls | 200 | 800 | 60 | 0 |
| Batched updates | 50 | 200 | 120 | 60 |

The update case changes all 50 labels and all 50 progress bars once per update
frame. DragonGUI waits until `startup_readiness` reports the first application
frame as presented before beginning its 60 batches. All frameworks completed
all 60 batches.

Each sample runs in a fresh process. There are three repetitions per
framework/case, and framework order rotates each repetition. Frames are paced
at 60 Hz where the toolkit permits manual control. Reported table values are
the median of the three repetition summaries.

## Metrics and timing boundaries

- **Import:** dynamic framework import only.
- **Build:** creation of the framework's public widget representation.
- **First event:** show plus the first explicit event/render pass for PyQt6,
  Dear PyGui, and Tkinter.
- **Active frame:** time actively spent updating/processing/rendering, excluding
  benchmark pacing sleep.
- **Update apply:** time spent issuing the 100 public widget mutations.
- **Live RSS:** last process working-set sample while the event loop is alive.
- **Peak RSS:** largest live working-set sample.

DragonGUI currently lacks a public first-application-frame timestamp, so its
first-event cell is intentionally blank. Its `build_ms` measures Python widget
declaration only; native parsing, cascade, layout, text, GPU setup, and first
presentation happen inside blocking `App.run()`. PyQt6 and Dear PyGui create
more native state during their build/setup stages. Build values must therefore
not be interpreted as equivalent time-to-interactive values.

DragonGUI's native command drain is also outside its `frame_timings.work`
timer. The report gives render work and command drain separately rather than
hiding either boundary.

## Results

All times are milliseconds. RSS is MiB.

### Empty window

| Framework | Import | Public build | First event | Active p50 | Active p95 | Live RSS | Peak RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| DragonGUI | 145.90 | 0.20 | — | 0.63 | 1.05 | 198.6 | 222.1 |
| Dear PyGui | 15.05 | 0.79 | 185.53 | 0.42 | 1.70 | 93.6 | 93.6 |
| PyQt6 | 32.94 | 47.02 | 272.65 | 2.37 | 4.47 | 70.2 | 70.2 |
| Tkinter | 15.91 | 99.87 | 18.70 | 0.09 | 0.29 | 47.6 | 47.6 |

### Moderate controls — 50 rows / 252 retained DragonGUI widgets

| Framework | Import | Public build | First event | Active p50 | Active p95 | Live RSS | Peak RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| DragonGUI | 134.57 | 1.30 | — | 1.03 | 1.60 | 278.8 | 278.8 |
| Dear PyGui | 15.32 | 3.08 | 181.40 | 0.49 | 1.38 | 91.7 | 93.9 |
| PyQt6 | 52.75 | 72.89 | 268.17 | 3.22 | 4.71 | 70.9 | 72.6 |
| Tkinter | 15.52 | 251.91 | 73.12 | 0.07 | 0.17 | 47.8 | 48.7 |

### Dense controls — 200 rows / 1,002 retained DragonGUI widgets

| Framework | Import | Public build | First event | Active p50 | Active p95 | Live RSS | Peak RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| DragonGUI | 135.12 | 4.07 | — | 1.09 | 1.70 | 434.5 | 482.7 |
| Dear PyGui | 19.19 | 10.15 | 183.56 | 0.68 | 1.64 | 92.4 | 94.6 |
| PyQt6 | 34.63 | 86.91 | 254.37 | 2.92 | 4.34 | 73.6 | 75.1 |
| Tkinter | 15.53 | 674.93 | 89.97 | 0.07 | 0.18 | 50.8 | 51.6 |

Tkinter's unchanged-frame values are event-service costs, not equivalent full
redraw costs: Tk does not repaint clean widgets merely because `update()` is
called. DragonGUI and Dear PyGui continuously render, while the Qt benchmark
explicitly requests repaint. Tkinter is therefore excluded from the static
active-render ranking.

### Corrected batched updates — 50 labels + 50 progress bars

| Framework | Update apply p50 | Update apply p95 | Active p50 | Active p95 | Live RSS |
|---|---:|---:|---:|---:|---:|
| DragonGUI | 0.22 | 0.43 | 0.52 | 0.79 | 272.9 |
| Dear PyGui | 0.15 | 0.27 | 0.44 | 1.09 | 93.2 |
| PyQt6 | 3.55 | 5.41 | 4.71 | 8.71 | 71.1 |
| Tkinter | 1.21 | 2.02 | 8.93 | 22.70 | 47.9 |

For DragonGUI, `update apply` is Python-side command enqueue time. The native
command-drain p95 was 2.21–2.49 ms across the three repetitions; render p95 was
0.77–0.81 ms. These phases need an end-to-end presentation-latency counter
before they can be collapsed into one exact number. A conservative sum places
the current p95 update pipeline near 3 ms, behind Dear PyGui but comfortably
ahead of the measured Qt/Tk update passes.

## DragonGUI scaling details

| Case | Widgets | Style reapply p50 | Layout p50 | Text rebuild p50 | Render work average | Layout issues |
|---|---:|---:|---:|---:|---:|---:|
| Empty | 2 | 0.11 | 0.06 | 0.02 | 0.62 | 0 |
| Moderate | 252 | 2.77 | 1.41 | 1.08 | 1.07 | 0 |
| Dense | 1,002 | 10.91 | 5.25 | 2.20 | 1.12 | 0 |
| Updates | 252 | 1.88–1.97 | — | — | 0.51–0.53 p50 | 0 |

The dense initial cascade still scales approximately with the full retained
tree. Once the frame is built, however, the visible scroll viewport keeps
steady render work near 1 ms. This supports the existing plan to make hidden
or inactive subtree cascading generation-aware, but it also shows that memory
and startup initialization now deserve at least equal priority.

## Interpretation by framework

### Dear PyGui

Dear PyGui is the performance target to beat for dense, dynamic GPU tool UIs.
It combines low import cost, low steady frame work, inexpensive direct updates,
and stable memory. DragonGUI is within the same broad performance class but is
not yet as efficient overall.

### PyQt6

DragonGUI uses less active frame/update time in this intentionally repainting,
control-heavy matrix. Qt uses much less memory and reaches its event loop
without DragonGUI's multi-second GPU initialization. Qt's invalidation and
model/view systems also make it likely to perform differently in ordinary
applications that do not request a full repaint every frame.

### Tkinter

Tkinter has the smallest footprint and fastest simple first event. Its widget
construction scales poorly here, and changing 100 controls causes much higher
event/render work. Its static active-frame numbers are not comparable because
clean widgets remain unpainted.

## Limitations

This is a directional engineering benchmark, not a universal framework
ranking.

1. It is one Windows machine and three repetitions.
2. GPU adapter name, backend, driver, and whether an integrated or discrete
   adapter was selected are not yet exposed in the DragonGUI snapshot.
3. Process RSS includes native libraries, driver mappings, shared GPU memory,
   caches, and allocator retention. It is not a direct retained-widget heap
   measurement.
4. DragonGUI first-presentation latency is missing because the runtime does not
   publish a timestamp for it.
5. Public-build timing boundaries differ because DragonGUI defers native work.
6. Qt's repaint was explicitly requested; Tk cannot be forced through an
   equivalent continuous GPU render path.
7. The test covers ordinary controls. It does not yet compare tables, plots,
   scatter upload/rendering, text editing, accessibility, input latency, or
   resize/layout latency.
8. Visual output is functionally similar but not pixel-identical. Different
   toolkits draw different fonts, themes, borders, and control internals.

## Recommended optimization priorities

### P0 — startup observability and initialization

1. Publish monotonic timestamps for native entry, adapter selection, device
   creation, surface configuration, document parse, cascade, layout, text
   setup, pipeline creation, first submit, and first successful present.
2. Expose adapter name, device type, backend, driver, and limits in
   `debug_snapshot()`.
3. Identify shader/pipeline creation and font discovery costs; cache or lazily
   create pipelines not used by the initial document.
4. Add a benchmark mode that exits immediately after the first application
   present so time-to-interactive is directly comparable.

### P0 — memory attribution and reduction

1. Report CPU-side retained tree/style/provenance/layout/text allocation counts
   and bytes.
2. Report GPU buffer/texture/atlas/pipeline/cache bytes separately.
3. Record adapter-selected shared/dedicated memory behavior.
4. Repeat empty, 252-widget, and 1,002-widget cases while sampling before GPU
   initialization, after first present, after steady state, and after resource
   release.
5. Investigate the bimodal dense result before claiming a per-widget byte
   slope.

### P1 — cascade and initial-layout scaling

1. Continue reducing required full-cascade attribute and property work.
2. Prototype generation-based lazy cascading for inactive page subtrees.
3. Add an explicit initial-document parse/cascade/layout benchmark independent
   of GPU initialization.
4. Compare default-style and provenance storage against compact shared forms.

### P1 — end-to-end update latency

1. Timestamp Python mutation, queue insertion, native apply, dirty rebuild,
   submit, and presentation under one update ID.
2. Report p50/p95/p99 mutation-to-present latency rather than separately timed
   phases only.
3. Add 1, 10, 100, and 1,000 updates per frame to locate the crossover with
   Dear PyGui.

### P2 — differentiated data workloads

The next comparison should focus on DragonGUI's intended strengths:

- 100k, 500k, and 1M point upload/replacement/steady scatter rendering;
- 100k and 1M row virtualized tables with 20 columns;
- line plots with append and replacement workloads;
- background-thread update-to-present latency;
- resize and scroll latency on deeply nested layouts.

Scatter and table results must compare equivalent data ownership, precision,
visibility, interaction, and decimation. A Qt custom painter or model/view
implementation should be disclosed as such rather than presented as a stock
widget equivalent.

## Reproduction

Install the comparison dependency into the ignored artifacts directory:

```powershell
py -3.12 -m pip install --target artifacts\benchmark-deps dearpygui
```

Run the full matrix:

```powershell
py -3.12 benchmarks\run_gui_framework_matrix.py `
  --repetitions 3 `
  --output-dir artifacts\gui-framework-comparison-v2
```

Run the startup-synchronized update matrix:

```powershell
py -3.12 benchmarks\run_gui_framework_matrix.py `
  --repetitions 3 `
  --case batched_updates `
  --output-dir artifacts\gui-framework-comparison-updates-v3
```

Tracked harness files:

- `benchmarks/gui_framework_case.py`
- `benchmarks/run_gui_framework_matrix.py`

Raw measurements and summaries are intentionally stored under `artifacts/`
and should not be committed.
