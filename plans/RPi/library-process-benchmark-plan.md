# DragonGUI Library Process Benchmark Tracker

Last updated: 2026-05-22

## Goal

Benchmark internal library processes directly, instead of only changing visual
options in full GUI probes. These benchmarks are intended to identify
library-wide bottlenecks that can be improved for the Raspberry Pi port and for
the library as a whole.

## Benchmark Harness Added

The native crate now has ignored Rust benchmark tests for these CPU paths:

- CSS cascade over a mixed 2,102-widget tree.
- Layout over a scroll-heavy 1,894-widget tree.
- Scatter3D packed `xyz_f32_v0` decode, coloring, and bounds.
- Scatter3D `point_instance_v1` bounds scanning.
- LinePlot packed XY limited decode.
- LinePlot bounded append/trim behavior.
- Runtime command queue coalescing for high-rate scatter, line, attitude, and
  translation updates.

The tests are ignored, so normal test runs are not affected.

## Commands

Run all internal process benchmarks:

```bash
PATH=/home/xymbu/Desktop/Projects/cargo/bin:$PATH \
CARGO_HOME=/home/xymbu/Desktop/Projects/cargo \
RUSTUP_HOME=/home/xymbu/Desktop/Projects/rustup \
CARGO_TARGET_DIR=/home/xymbu/Desktop/Projects/DragonGui-RPi/target-local \
cargo test --manifest-path native/Cargo.toml bench_ --lib -- --ignored --nocapture
```

Run focused benchmarks:

```bash
cargo test --manifest-path native/Cargo.toml bench_css_cascade_many_widgets --lib -- --ignored --nocapture
cargo test --manifest-path native/Cargo.toml bench_layout_many_widgets --lib -- --ignored --nocapture
cargo test --manifest-path native/Cargo.toml bench_scatter_decode_and_bounds_paths --lib -- --ignored --nocapture
cargo test --manifest-path native/Cargo.toml bench_line_plot_decode_and_window_paths --lib -- --ignored --nocapture
cargo test --manifest-path native/Cargo.toml bench_runtime_command_coalescing --lib -- --ignored --nocapture
```

Optional knobs:

```bash
DRAGONGUI_BENCH_ITERS=10
DRAGONGUI_BENCH_POINTS=150000
DRAGONGUI_BENCH_KEEP_POINTS=50000
DRAGONGUI_BENCH_COMMANDS=20000
```

Release-mode numbers are the useful numbers for final optimization decisions:

```bash
cargo test --release --manifest-path native/Cargo.toml bench_ --lib -- --ignored --nocapture
```

## Release Results

Release-profile results from 2026-05-22 on Raspberry Pi. The first release
compile/link took about 5 minutes; later focused benchmark runs reused the
release artifacts and completed quickly.

| Process | Result | Notes |
| --- | ---: | --- |
| CSS cascade many widgets | 99,576 ns/widget | Mixed type/class/child/attribute/`:has`/`:nth-child` selectors over 2,102 widgets. |
| Layout many widgets | 21,316 ns/widget | Scroll-heavy tree with nested flow panels and controls. |
| Scatter compact `xyz_f32_v0` bounds scan | 6 ns/point | 150k points, used by the compact GPU shader path when telemetry bounds are not supplied. |
| Scatter old `xyz_f32_v0` decode + color + bounds | 68 ns/point | 150k points, old CPU expansion path with colormap assignment. |
| Scatter `point_instance_v1` bounds scan | 10 ns/point | 150k GPU-shaped packed points, byte decode via `pod_read_unaligned`. |
| Scatter decoded `PointInstance` bounds scan | 15 ns/point | 150k already-decoded native points. |
| LinePlot packed limited decode | 17 ns/kept point | 200k source points, 50k kept. |
| LinePlot bounded append + trim | 1.85 ms/op | 50k retained + 2,048 appended points. |
| Runtime command coalescing | 1,517 ns/input command | 20k input commands coalesced to latest values. |

## First Debug Smoke Results

These are debug-profile results from 2026-05-22. They verify the harness and
show relative shape, but they should not be treated as final optimized timings.

| Process | Result | Notes |
| --- | ---: | --- |
| CSS cascade many widgets | 347,619 ns/widget | Mixed type/class/child/attribute/`:has`/`:nth-child` selectors over 2,102 widgets. |
| Layout many widgets | 215,103 ns/widget | Scroll-heavy tree with nested flow panels and controls. |
| Scatter `xyz_f32_v0` decode + color + bounds | 662 ns/point | 150k points, includes colormap assignment. |
| Scatter `point_instance_v1` bounds scan | 678 ns/point | 150k GPU-shaped packed points, byte decode via `pod_read_unaligned`. |
| Scatter decoded `PointInstance` bounds scan | 109 ns/point | 150k already-decoded native points. |
| LinePlot packed limited decode | 607 ns/kept point | 200k source points, 50k kept. |
| LinePlot bounded append + trim | 6.81 ms/op | 50k retained + 2,048 appended points. |
| Runtime command coalescing | 5,570 ns/input command | 20k input commands coalesced to latest values. |

## Initial Read

The release results change the priority order from the debug smoke run.

- CSS cascade is meaningfully heavier than layout for large widget trees. More
  selector matching and computed-style caching work is likely worthwhile.
- Layout is not free, but it is not the first CPU target compared with cascade.
- Scatter automatic bounds discovery is not expensive enough to justify
  requiring predefined bounds for live LiDAR streams. The compact `xyz_f32_v0`
  bounds scan is about 0.9 ms for 150k points. The old CPU decode/color/bounds
  path was about 10.2 ms for 150k points, so the expensive part was CPU
  expansion and color assignment, not bounds.
- Optional per-frame bounds supplied by a producer may still be useful for
  prepared payloads, but automatic bounds should remain the default for live
  sensor streams.
- LinePlot bounded append/trim is a real target if high-rate telemetry streams
  use large retained windows.
- Command coalescing is useful behaviorally, but the current implementation is
  probably not the top CPU bottleneck unless queues become very large.

## Current Optimization Priority

1. CSS cascade caching/filtering beyond the current target filter.
2. Primitive collection/rebuild benchmarks to see whether widget rendering prep
   is now comparable to cascade.
3. Text collection/shaping prep benchmarks, because dashboard labels and tables
   may amplify that cost.
4. LinePlot append/windowing improvements for high-rate telemetry.
5. Scatter draw/GPU cost, now that default `xyz_f32_v0` streams avoid CPU
   point-instance expansion and shader-side colormap sampling handles color.

## Implemented From This Pass

- Default `xyz_f32_v0` Scatter3D streams now have a compact GPU path.
- Native uses telemetry bounds when present, otherwise scans compact xyz bytes
  for automatic bounds.
- The compact 12-byte xyz payload is uploaded directly as the vertex instance
  buffer.
- A new compact point shader derives point color from z using the current
  colormap and z range.
- `point_instance_v1` remains the path for explicit per-point color, size, or
  opacity.

## Next Benchmarks To Add

- Primitive collection/rebuild without GPU upload.
- Primitive split classification and batching cost.
- Text label collection and text shaping input preparation.
- DataFrame table window/sampling and formatted-cell path.
- Histogram bin decode and render-data generation.
- TranslationTrace and AttitudeSphere primitive generation.
- Scatter3D grid/chrome primitive generation independent of point draw.
- Python-to-native packed payload handoff cost from the Python API side.

## Next Optimization Questions

- Can `point_instance_v1` bounds be skipped more often by requiring or
  encouraging prepared bounds for live streams?
- Can layout avoid full-tree recompute for high-rate data-only updates?
- Can CSS computed styles be cached by stable widget signature for repeated
  dashboard rows/buttons?
- Can command coalescing avoid cloning large payload commands during drain?
- Can line plot append avoid cloning retained history in hot paths and compact
  less aggressively?
