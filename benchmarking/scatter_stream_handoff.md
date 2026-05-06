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

## Current Machine Baseline

Date: 2026-05-06

Environment captured from this run:

- Python: 3.12.12
- NumPy: 2.4.0
- Platform: Windows-10-10.0.19045-SP0
- CPU/GPU WMI queries were blocked with access denied in this shell.

Important note: the first run on this machine was invalid. It accepted 180
updates but native applied only 1 update because `call_soon_threadsafe()`
queued one `DrainPythonTasks` native command per producer frame. Those drain
commands sat ahead of the coalescable scatter upload commands and hit the
native fairness limit. `python/dragongui/runtime.py` now coalesces outstanding
Python task drain wakeups so producer frames do not flood the native command
queue with duplicate drain commands.

Primary optimized baseline:

```powershell
python benchmarking\scatter_stream_compare.py --backend dragongui --points 1000000 --target-hz 60 --dragongui-update-mode live-frame --dragongui-workload prebuilt-payloads --dragongui-payload-format instances --json-out benchmarking\last_scatter_compare.json
```

Observed result:

```text
points              : 1,000,000
target_update_hz    : 60.0
accepted_update_hz  : 82.6
native_update_hz    : 13.5
completed_updates   : 180
present_fps         : 147.4
producer_build avg  : 0.04 ms
producer_gen avg    : 0.00 ms
producer_pack avg   : 0.00 ms
ui_update avg       : 9.37 ms
command_queue_depth : 2
native_updates      : 51
coalesced_updates   : 129
native pack/upload  : 0.00 / 3.16 ms
native decode       : 0.00 ms
native bounds       : 0.00 ms
native grid/overlay : 0.00 / 0.00 ms
native total        : 3.18 ms
```

Comparison runs:

| Workload | Payload | Accepted Hz | Native Hz | Native updates | UI avg | Gen avg | Pack avg | Native decode | Native upload | Native total | JSON |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| prebuilt-payloads | instances | 82.6 | 13.5 | 51 | 9.37 ms | 0.00 ms | 0.00 ms | 0.00 ms | 3.16 ms | 3.18 ms | `benchmarking/last_scatter_compare.json` |
| prebuilt-payloads | xyz | 85.7 | 26.4 | 100 | 3.12 ms | 0.00 ms | 0.00 ms | 8.58 ms | 2.68 ms | 11.26 ms | `benchmarking/scatter_baseline_1m_prebuilt_payloads_xyz.json` |
| prebuilt-frames | instances | 9.4 | 3.8 | 10 | 9.62 ms | 0.00 ms | 169.27 ms | 0.00 ms | 4.68 ms | 4.70 ms | `benchmarking/scatter_baseline_1m_prebuilt_frames_instances.json` |
| generate | instances | 6.8 | 2.2 | 6 | 10.50 ms | 78.40 ms | 167.01 ms | 0.00 ms | 4.52 ms | 4.54 ms | `benchmarking/scatter_baseline_1m_generate_instances.json` |

Interpretation for this machine:

- The direct `point_instance_v1` path is still the right native hot path:
  decode and bounds are zero, and GPU upload is about 3-5 ms for 1M points.
- Python/UI handoff remains expensive at roughly 9-10 ms per accepted update
  for the instance payload path.
- Preparing `point_instance_v1` from NumPy arrays in Python is very expensive
  here, around 167-169 ms for 1M points.
- Synthetic generation adds about 78 ms on top of packing in the full workload.
- Compact XYZ has lower Python handoff cost but pays about 8.6 ms native decode
  to expand and colorize the payload.

### Paced Producer Baseline

`benchmarking/scatter_stream_compare.py` now supports:

```powershell
--producer-mode flood|paced
```

- `flood` is the previous behavior: schedule producer callbacks at target rate
  and let native coalesce stale scatter uploads.
- `paced` schedules one frame, waits for the native scatter `updates` counter
  to advance through `app.debug_snapshot()`, then schedules the next frame.

To make benchmark duration reliable, `App.request_exit()` and the native
`RequestExit` command were added. The producer now asks the native event loop to
exit after the requested benchmark duration instead of relying only on
`DRAGONGUI_SMOKE_FRAMES`.

Focused smoke:

```powershell
python benchmarking\scatter_stream_compare.py --backend dragongui --points 100000 --duration 1 --target-hz 60 --producer-mode paced --dragongui-update-mode live-frame --dragongui-workload prebuilt-payloads --dragongui-payload-format instances --json-out benchmarking\scatter_paced_smoke_100k_instances.json
```

Result: 60 completed updates, 60 native updates, 0 coalesced updates, 0 paced
timeouts.

1M-point paced/flood comparison:

| Producer | Target Hz | Payload | Accepted Hz | Paced Ack Hz | Native updates | Coalesced | UI avg | Ack wait avg | Native decode | Native upload | Native total | JSON |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| flood | 60 | instances | 81.8 | n/a | 51 | 129 | 9.55 ms | n/a | 0.00 ms | 3.13 ms | 3.16 ms | `benchmarking/scatter_baseline_1m_flood_payloads_instances.json` |
| paced | 60 | instances | 57.3 | 57.3 | 172 | 0 | 6.96 ms | 16.06 ms | 0.00 ms | 2.46 ms | 2.47 ms | `benchmarking/scatter_baseline_1m_paced60_payloads_instances.json` |
| paced | 240 | instances | 55.0 | 55.0 | 165 | 0 | 7.75 ms | 18.16 ms | 0.00 ms | 3.09 ms | 3.11 ms | `benchmarking/scatter_baseline_1m_paced240_payloads_instances.json` |
| paced | 240 | xyz | 51.9 | 51.9 | 156 | 0 | 2.43 ms | 19.27 ms | 9.31 ms | 3.60 ms | 12.90 ms | `benchmarking/scatter_baseline_1m_paced240_payloads_xyz.json` |

Interpretation:

- With explicit pacing, 1M `point_instance_v1` payloads sustain about 55-57
  acknowledged native uploads/sec on this machine with zero coalescing.
- Raising target rate from 60 to 240 does not improve paced throughput; the
  acknowledgement loop saturates around 55 Hz.
- The native direct-upload hot path remains fast, around 2.5-3.1 ms for the
  last applied 1M instance upload.
- The paced acknowledgement wait is much larger, around 16-18 ms, which means
  the current Python callback + native command + debug-snapshot acknowledgement
  path is now the measured limiter for paced streaming.
- Compact XYZ remains slower end-to-end under pacing because native decode adds
  about 9.3 ms per 1M-point update.

### Direct Handoff Baseline

`benchmarking/scatter_stream_compare.py` now also supports:

```powershell
--dragongui-handoff callback|direct
```

- `callback` is the previous path: the producer schedules a Python UI callback
  with `app.call_soon_threadsafe(...)`, and that callback calls
  `ScatterLiveFrame.replace_prepared(...)`.
- `direct` calls `ScatterLiveFrame.enqueue_prepared(...)` directly from the
  producer thread. This bypasses Python UI callback scheduling while keeping a
  live-frame handle as the user-facing streaming object, and relies on the
  native command queue's existing coalesced scatter update behavior.

The native queue was already doing latest-frame coalescing for
`SetScatterPointsPacked` commands, so this benchmark mode uses the existing
native latest-frame queue semantics rather than adding a separate slot.

1M `point_instance_v1` direct-handoff results:

| Handoff | Producer | Target Hz | Accepted Hz | Paced Ack Hz | Native updates | Coalesced | UI avg | Ack wait avg | Native upload | Native total | JSON |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| callback | paced | 240 | 55.0 | 55.0 | 165 | 0 | 7.75 ms | 18.16 ms | 3.09 ms | 3.11 ms | `benchmarking/scatter_baseline_1m_paced240_payloads_instances.json` |
| direct | paced | 60 | 59.3 | 59.2 | 178 | 0 | 6.46 ms | 8.30 ms | 2.75 ms | 2.76 ms | `benchmarking/scatter_baseline_1m_direct_paced60_payloads_instances.json` |
| direct | paced | 240 | 64.9 | 65.0 | 195 | 0 | 6.62 ms | 8.73 ms | 2.47 ms | 2.47 ms | `benchmarking/scatter_baseline_1m_direct_paced240_payloads_instances.json` |
| direct | flood | 240 | 103.7 | n/a | 212 | 100 | 9.58 ms | n/a | 2.86 ms | 2.87 ms | `benchmarking/scatter_baseline_1m_direct_flood240_payloads_instances.json` |

Interpretation:

- Direct handoff improves paced high-target throughput from about 55 ack Hz to
  about 65 ack Hz for 1M GPU-ready payloads.
- Direct handoff cuts paced acknowledgement wait roughly in half, from about
  18 ms to about 8-9 ms.
- Direct flood mode applies about 69 native 1M-point uploads/sec with latest
  frame coalescing, while accepting about 104 producer submissions/sec.
- The remaining paced limiter is still the acknowledgement mechanism
  (`debug_snapshot()` round trip), not the native upload itself.
- If a production stream needs latest-frame semantics rather than every-frame
  acknowledgement, the direct enqueue path is already the better shape.

## Important Metrics

- `accepted_update_hz`: Python/UI callbacks accepted.
- `native_update_hz`: full point uploads actually applied by native.
- `coalesced_updates`: accepted frames that were superseded before native upload.
- `paced_ack_update_hz`: native-acknowledged producer rate in paced mode.
- `paced_ack_wait_ms`: time spent waiting for the native scatter update counter
  to advance after each scheduled frame.
- `handoff`: whether live-frame payloads were sent through a Python UI callback
  or directly to the native command queue.
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

Paced latest-frame mode:

```powershell
python benchmarking\scatter_stream_compare.py --backend dragongui --points 1000000 --target-hz 240 --producer-mode paced --dragongui-update-mode live-frame --dragongui-workload prebuilt-payloads --dragongui-payload-format instances
```

Direct native command handoff:

```powershell
python benchmarking\scatter_stream_compare.py --backend dragongui --points 1000000 --target-hz 240 --producer-mode paced --dragongui-handoff direct --dragongui-update-mode live-frame --dragongui-workload prebuilt-payloads --dragongui-payload-format instances
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
  - `AppHandle.call_soon_threadsafe(...)` now coalesces outstanding
    `DrainPythonTasks` wakeups so high-rate producers do not fill the native
    command queue with duplicate drain commands before coalescable scatter
    uploads.
  - `AppHandle.request_exit()` sends a native exit request for benchmark
    duration control.
- `python/dragongui/app.py`
  - `App.request_exit()` exposes the native exit request while an app is live.
- `python/dragongui/widgets.py`
  - `ScatterLiveFrame.enqueue_prepared(...)` exposes the thread-safe direct
    prepared-payload path for high-rate primary live frames.

Native:

- `native/src/commands.rs`
  - `ScatterTelemetry` now carries optional bounds.
  - `enqueue_set_scatter_points_packed(...)` accepts `bounds_min` and `bounds_max`.
  - Added `RequestExit` command.
- `native/src/runtime.rs`
  - `point_instance_v1` path can use supplied bounds instead of scanning the payload.
  - direct upload path reports `decode=0` and `bounds=0` when bounds are supplied.
  - `RequestExit` schedules the event loop to exit after one final redraw.
- `native/src/scatter/mod.rs`
  - Added `set_point_instances_raw(...)` for direct GPU upload of already GPU-shaped point records.

Benchmark:

- `benchmarking/scatter_stream_compare.py`
  - Added workload modes: `generate`, `prebuilt-frames`, `prebuilt-payloads`.
  - Added payload format modes: `xyz`, `instances`.
  - Added producer modes: `flood`, `paced`.
  - Added handoff modes: `callback`, `direct`.
  - Added accepted/native/coalesced rate reporting.
  - Added paced acknowledgement rate/wait/timeout reporting.
  - Added command queue depth and native sub-metrics.
  - Suppresses per-frame static metadata during benchmark streaming to avoid command queue flooding.
  - Uses `App.request_exit()` to finish benchmark runs at the requested
    duration.

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
   - Done: `--producer-mode paced` waits for native scatter `updates` to advance
     before sending the next payload.
   - Current 1M `point_instance_v1` result is about 55-57 acknowledged native
     uploads/sec with no coalescing.

2. Reduce Python/UI handoff overhead.
   - Partially done: `ScatterLiveFrame.enqueue_prepared(...)` and
     `--dragongui-handoff direct` bypass `app.call_soon_threadsafe(...)` for
     prepared payload streams.
   - Direct handoff improves paced high-target throughput from about 55 ack Hz
     to about 65 ack Hz, and direct flood mode applies about 69 native
     uploads/sec with latest-frame coalescing.
   - Remaining paced overhead is dominated by the debug-snapshot acknowledgement
     loop, not native upload.

3. Investigate direct native latest-frame slot.
   - The command queue already coalesces `SetScatterPointsPacked` commands by
     scatter id, so direct handoff now uses existing latest-frame queue
     semantics.
   - A future dedicated latest-frame slot could still reduce payload copies and
     provide cheaper acknowledgement than `debug_snapshot()`.

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

Additional focused verification after the drain wakeup fix:

```powershell
python -m pytest tests\test_python_api.py -q -k "app_handle_queues_and_drains_python_tasks or app_handle_coalesces_python_task_drain_wakeups or app_handle_request_redraw_and_exit_enqueue_native_commands or app_handle_bounds_python_task_drain or live_frame or scatter_live_frame"
```

Latest focused result:

```text
7 passed
```

