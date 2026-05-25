# V4 Widget Optimization Baseline

Created from `benchmarking/v4_widget_benchmark.py` after the v4 widget/functionality pass.

## Commands

```powershell
python benchmarking\v4_widget_benchmark.py --frames 20 --json-out benchmarking\v4_widget_baseline.json
python benchmarking\v4_widget_benchmark.py --frames 30 --repeat 3 --probe layout_plot_embedding --probe scatter3d_dense --probe scatter_plot_2d --probe heatmap --probe bar_chart --json-out benchmarking\v4_widget_plot_focus_baseline.json
```

## Read The Numbers

`frame_ms` includes presentation timing and is noisy for short static smoke runs.
For optimization, use:

- `frame_work_ms_avg`
- `frame_prepare_ms_avg`
- `frame_encode_ms_avg`
- framework stage timings (`layout`, `text`, `primitive_rebuild`)
- primitive counts and complex primitive counts

## Full Baseline Hot Spots

| Probe | Work ms | Prepare ms | Encode ms | Primitives | Complex | Widgets |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `scatter3d_dense` | 4.464 | 0.885 | 3.051 | 14 | 7 | 13 |
| `layout_plot_embedding` | 5.615 | 1.635 | 3.440 | 113 | 18 | 37 |
| `scatter_plot_2d` | 4.216 | 1.178 | 2.489 | 14 | 7 | 14 |
| `heatmap` | 2.174 | 1.658 | 0.053 | 14,166 | 10 | 15 |
| `bar_chart` | 2.107 | 1.623 | 0.053 | 213 | 28 | 23 |
| `layout_scrollable_composites` | 1.639 | 0.998 | 0.054 | 93 | 21 | 342 |

## Focus Repeat

Median of 3 runs, 30 frames each:

| Probe | Work ms | Prepare ms | Encode ms | Main Signal |
| --- | ---: | ---: | ---: | --- |
| `layout_plot_embedding` | 3.648 | 1.287 | 1.827 | text rebuild ~1.77 ms plus plot encode |
| `scatter_plot_2d` | 3.400 | 1.159 | 1.574 | scatter encode plus text rebuild |
| `scatter3d_dense` | 3.299 | 0.841 | 2.039 | scatter encode dominates |
| `bar_chart` | 1.996 | 1.503 | 0.058 | text rebuild ~1.11 ms |
| `heatmap` | 1.969 | 1.524 | 0.048 | primitive rebuild ~1.04 ms from 14k cells |

## First Targets

1. Heatmap dense path: avoid one primitive per cell for large heatmaps. A dedicated heatmap texture/instance path should remove most of the 14k primitive rebuild cost.
2. Plot text labels: bar chart and plot embedding spend significant time rebuilding text. Cache stable tick/category labels or avoid regenerating unchanged text entries.
3. Scatter encode path: both 2D and 3D dense scatter have low primitive cost but high encode time, so the next investigation should inspect scatter pass encoding and buffer binding work.
4. Tool buttons: not a frame-time top offender yet, but it still emits 182 complex primitives. Icon drawing simplification can be a secondary target after plot-heavy widgets.

## Optimization Pass 1

Commands:

```powershell
python benchmarking\v4_widget_benchmark.py --frames 30 --repeat 3 --probe layout_plot_embedding --probe scatter3d_dense --probe scatter_plot_2d --probe heatmap --probe bar_chart --json-out benchmarking\v4_widget_plot_focus_after_text_cache.json
```

Changes:

- Dense heatmaps now use a resolution-aware cell stride when cells are smaller than a few screen pixels.
- Small heatmaps remain exact, so labeled correlation/confusion grids keep their existing appearance.
- Per-frame plot overlay labels now reuse shaped text buffers instead of discarding them every frame.

Before/after from the focused plot benchmark:

| Probe | Work ms | Prepare ms | Encode ms | Primitives | Buffer KB |
| --- | ---: | ---: | ---: | ---: | ---: |
| `heatmap` | 1.969 -> 0.957 | 1.524 -> 0.425 | 0.048 -> 0.059 | 14,166 -> 3,798 | 908.4 -> 244.8 |
| `bar_chart` | 1.996 -> 1.206 | 1.503 -> 0.687 | 0.058 -> 0.058 | 213 -> 213 | 18.6 -> 18.6 |
| `layout_plot_embedding` | 3.647 -> 3.263 | 1.287 -> 0.721 | 1.827 -> 1.929 | 113 -> 113 | 10.4 -> 10.4 |
| `scatter_plot_2d` | 3.400 -> 2.435 | 1.159 -> 0.314 | 1.574 -> 1.606 | 14 -> 14 | 2.1 -> 2.1 |
| `scatter3d_dense` | 3.299 -> 2.520 | 0.841 -> 0.267 | 2.039 -> 1.819 | 14 -> 14 | 2.1 -> 2.1 |

## Optimization Pass 2

Commands:

```powershell
python benchmarking\v4_widget_benchmark.py --frames 30 --repeat 3 --probe layout_plot_embedding --probe scatter3d_dense --probe scatter_plot_2d --probe heatmap --probe bar_chart --json-out benchmarking\v4_widget_plot_focus_after_scatter_cache.json
python benchmarking\v4_widget_benchmark.py --frames 120 --repeat 3 --probe scatter_plot_2d --probe scatter3d_dense --json-out benchmarking\v4_widget_scatter_cache_120f.json
```

Changes:

- Scatter widgets now track a scene revision for data/layout/camera/overlay changes.
- Static full-scale scatter views render into a cached target, then composite the target on unchanged frames.
- Scatter runtime snapshots now report redraw/composite/cache-hit timings so cached and uncached frames can be separated.
- Camera uniform uploads are skipped when the computed uniforms are unchanged.

The 30-frame focused run is too short to show the steady-state benefit clearly because it includes cache warmup and first redraw cost. The 120-frame scatter-only run shows the intended steady state:

| Probe | Work ms | Prepare ms | Encode ms | Scatter Encode ms | Cache Hit |
| --- | ---: | ---: | ---: | ---: | --- |
| `scatter_plot_2d` | 1.141 | 0.184 | 0.466 | 0.040 | yes |
| `scatter3d_dense` | 1.246 | 0.179 | 0.599 | 0.040 | yes |

Next likely target: benchmark dynamic scatter interactions separately, because camera motion intentionally invalidates the cached target and still pays the direct redraw path.

## Optimization Pass 3

Commands:

```powershell
python benchmarking\scatter_dynamic_camera_benchmark.py --points 65000 --camera-updates 90 --interval-ms 16 --repeat 3 --json-out benchmarking\scatter_dynamic_camera_paced_instrumented.json
python benchmarking\scatter_dynamic_camera_benchmark.py --points 65000 --camera-updates 90 --interval-ms 16 --repeat 3 --json-out benchmarking\scatter_dynamic_camera_paced_after_direct_motion.json
python benchmarking\v4_widget_benchmark.py --frames 120 --repeat 3 --probe scatter_plot_2d --probe scatter3d_dense --json-out benchmarking\v4_widget_scatter_after_direct_motion_120f.json
```

Changes:

- Added `scatter_dynamic_camera_benchmark_probe.py` and `scatter_dynamic_camera_benchmark.py`.
- Scatter snapshots now include aggregate render timing and cache hit counters, not only the last rendered frame.
- Full-scale scatter now renders directly while the scene revision is changing, instead of drawing into an offscreen target and compositing it every moving frame.
- Once the scene revision is stable, the static render target is repopulated and subsequent frames use the cache.

Paced dynamic camera benchmark, 65k points, 90 updates at 16 ms:

| Metric | Before | After |
| --- | ---: | ---: |
| Work ms | 1.948 | 1.569 |
| Prepare ms | 0.496 | 0.377 |
| Encode ms | 0.976 | 0.746 |
| Scatter encode avg ms | 0.924 | 0.695 |
| Cache hit rate | 1.5% | 0.0% |

Static scatter cache remained active after the change:

| Probe | Work ms | Encode ms | Scatter Encode Avg ms | Cache Hit Rate |
| --- | ---: | ---: | ---: | ---: |
| `scatter_plot_2d` | 1.167 | 0.506 | 0.418 | 98.3% |
| `scatter3d_dense` | 1.224 | 0.598 | 0.538 | 98.3% |

Next likely target: camera-motion prepare time. Grid label projection and overlay refresh are now the largest non-render costs during paced camera changes.
