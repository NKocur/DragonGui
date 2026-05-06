# Scatter Stream Optimization Handoff

Date: 2026-05-06

## Goal

We are optimizing `Scatter3D` for high-rate full-frame point cloud streaming, especially LiDAR-style workloads where each incoming frame replaces the entire point set.

The target use case is roughly:

- 1M points per frame
- complete frame replacement, not incremental append
- x/y/z plus a color-mapped variable
- latest-frame behavior is acceptable: if frames arrive faster than the renderer can upload them, stale frames should be coalesced/dropped

## Current Benchmark

Main script:

```powershell
C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe benchmarking/scatter_stream_compare.py --backend dragongui --points 1000000 --target-hz 60 --dragongui-update-mode live-frame --dragongui-workload prebuilt-payloads --dragongui-payload-format instances
```

Current best observed result:

```text
points              : 1,000,000
target_update_hz    : 60.0
accepted_update_hz  : 78.5
native_update_hz    : 7.2
completed_updates   : 166
present_fps         : 60.0
producer_build avg  : 0.03 ms
producer_gen avg    : 0.00 ms
producer_pack avg   : 0.00 ms
ui_update avg       : 10.05 ms
command_queue_depth : 1
native_updates      : 46
coalesced_updates   : 120
native pack/upload  : 0.00 / 4.80 ms
native decode       : 0.00 ms
native bounds       : 0.00 ms
native grid/overlay : 0.00 / 0.00 ms
native total        : 4.81 ms
```

Interpretation:

- The native hot path is now mostly GPU upload cost.
- Native decode has been eliminated for `point_instance_v1`.
- Native bounds scanning has been eliminated when prepared payload bounds are supplied.
- The benchmark still accepts more frames than native applies. That is expected with coalescing and latest-frame semantics.

## Important Metrics

- `accepted_update_hz`: Python/UI callbacks accepted.
- `native_update_hz`: full point uploads actually applied by native.
- `coalesced_updates`: accepted frames that were superseded before native upload.
- `producer_gen`: synthetic frame generation cost.
- `producer_pack`: Python payload packing cost.
- `ui_update`: Python-side handoff/callback cost.
- `native decode`: native conversion/decode cost.
- `native bounds`: native min/max scan cost.
- `native pack/upload`: telemetry pack time / GPU upload time.
- `native total`: total native cost for the last applied upload.

## Benchmark Modes

Full synthetic workload:

```powershell
C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe benchmarking/scatter_stream_compare.py --backend dragongui --points 1000000 --target-hz 60 --dragongui-update-mode live-frame --dragongui-workload generate
```

Remove frame generation, keep Python packing:

```powershell
C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe benchmarking/scatter_stream_compare.py --backend dragongui --points 1000000 --target-hz 60 --dragongui-update-mode live-frame --dragongui-workload prebuilt-frames
```

Remove generation and packing:

```powershell
C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe benchmarking/scatter_stream_compare.py --backend dragongui --points 1000000 --target-hz 60 --dragongui-update-mode live-frame --dragongui-workload prebuilt-payloads
```

Compare compact XYZ payload vs GPU-shaped instance payload:

```powershell
C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe benchmarking/scatter_stream_compare.py --backend dragongui --points 1000000 --target-hz 60 --dragongui-update-mode live-frame --dragongui-workload prebuilt-payloads --dragongui-payload-format xyz
```

```powershell
C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe benchmarking/scatter_stream_compare.py --backend dragongui --points 1000000 --target-hz 60 --dragongui-update-mode live-frame --dragongui-workload prebuilt-payloads --dragongui-payload-format instances
```

## Library Changes Made

Python:

- `python/dragongui/widgets.py`
  - `ScatterPayload` now carries optional bounds.
  - `Scatter3D.prepare_points(...)` computes payload bounds.
  - `ScatterLiveFrame.replace_prepared(...)` can deliver prepared payloads through the primary packed path.
- `python/dragongui/runtime.py`
  - `enqueue_set_scatter_points_packed(...)` forwards `bounds_min` and `bounds_max`.

Native:

- `native/src/commands.rs`
  - `ScatterTelemetry` now carries optional bounds.
  - `enqueue_set_scatter_points_packed(...)` accepts `bounds_min` and `bounds_max`.
- `native/src/runtime.rs`
  - `point_instance_v1` path can use supplied bounds instead of scanning the payload.
  - direct upload path reports `decode=0` and `bounds=0` when bounds are supplied.
- `native/src/scatter/mod.rs`
  - Added `set_point_instances_raw(...)` for direct GPU upload of already GPU-shaped point records.

Benchmark:

- `benchmarking/scatter_stream_compare.py`
  - Added workload modes: `generate`, `prebuilt-frames`, `prebuilt-payloads`.
  - Added payload format modes: `xyz`, `instances`.
  - Added accepted/native/coalesced rate reporting.
  - Added command queue depth and native sub-metrics.
  - Suppresses per-frame static metadata during benchmark streaming to avoid command queue flooding.

## Key Findings

1. Synthetic frame generation dominated the original benchmark.
   - 1M-point frame generation was often 55-70 ms by itself.

2. Compact `xyz_f32_v0` is smaller on the wire but expensive in native.
   - Native must expand 12-byte XYZ records into 32-byte GPU instances and assign colors.
   - Observed native decode was about 15-23 ms for 1M points.

3. `point_instance_v1` is larger but faster at upload time.
   - Python prepares GPU-shaped records.
   - Native can direct-upload those bytes.
   - Native decode dropped to 0 ms.

4. Precomputed bounds matter.
   - Native bounds scan was about 6 ms for 1M instance records.
   - Supplying bounds from `ScatterPayload` dropped native bounds to 0 ms.

5. Current optimized native upload cost is about 4.8 ms for 1M `point_instance_v1` points.
   - At this point the native hot path is mostly GPU upload/bandwidth.

## Current Open Questions

- Should `Scatter3D.prepare_points(...)` expose a clearer public option for GPU-ready payloads, or is the current `scalars=...`/`point_instance_v1` inference good enough?
- Should LiDAR users be encouraged to produce `point_instance_v1` directly if they already have color/size values?
- Should there be a lower-copy path from NumPy arrays into native without converting to Python `bytes` first?
- Should latest-frame streaming have backpressure so Python does not enqueue frames faster than native can consume?

## Next Optimization Targets

1. Add a paced/latest-frame benchmark mode.
   - Current benchmark can accept 70-80 UI updates/sec while native applies far fewer.
   - A paced mode should wait for the previous native update count to advance before sending the next payload.
   - This will measure sustainable applied upload rate more cleanly.

2. Reduce Python/UI handoff overhead.
   - Even with prebuilt payloads, `ui_update avg` is still around 8-10 ms in some runs.
   - Investigate whether `app.call_soon_threadsafe(...)` and callback execution are the limiting layer.

3. Investigate direct native latest-frame slot.
   - Instead of queueing Python callbacks that enqueue native commands, maintain a single latest payload slot and have native consume the newest payload on redraw.
   - This would make coalescing explicit and avoid flooding command queues.

4. Explore zero-copy or lower-copy payload transfer.
   - Current `point_instance_v1` still copies bytes into native command storage and then into the GPU buffer.
   - A NumPy buffer protocol path or persistent staging buffer might reduce CPU overhead.

5. Measure with real LiDAR frames.
   - Synthetic generation is not representative once using prebuilt payloads.
   - Real frame parsing/packing should be timed separately from DragonGUI upload.

## Rebuild Notes

After native changes, rebuild and copy with:

```powershell
.\rebuild_and_copy.bat
```

The script verifies the native import path after copying.

Focused verification used during this work:

```powershell
C:\Users\nkocur\AppData\Local\Programs\Python\Python311\python.exe -m pytest tests/test_python_api.py -k "live_frame or scatter_live_frame"
```

Latest focused result:

```text
3 passed
```

