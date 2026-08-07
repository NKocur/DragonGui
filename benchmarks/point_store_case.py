"""Measure two ScatterPlot2D widgets with legacy frames or a shared PointStore."""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path

try:
    import psutil
except ImportError:  # Keep the standalone benchmark usable in minimal installs.
    psutil = None


def current_rss_bytes() -> int:
    if psutil is not None:
        return int(psutil.Process().memory_info().rss)
    if os.name == "nt":
        import ctypes
        from ctypes import wintypes

        size_t = ctypes.c_size_t

        class ProcessMemoryCounters(ctypes.Structure):
            _fields_ = [
                ("cb", wintypes.DWORD),
                ("page_fault_count", wintypes.DWORD),
                ("peak_working_set_size", size_t),
                ("working_set_size", size_t),
                ("quota_peak_paged_pool_usage", size_t),
                ("quota_paged_pool_usage", size_t),
                ("quota_peak_non_paged_pool_usage", size_t),
                ("quota_non_paged_pool_usage", size_t),
                ("pagefile_usage", size_t),
                ("peak_pagefile_usage", size_t),
            ]

        counters = ProcessMemoryCounters()
        counters.cb = ctypes.sizeof(counters)
        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        psapi = ctypes.WinDLL("psapi", use_last_error=True)
        kernel32.GetCurrentProcess.restype = wintypes.HANDLE
        psapi.GetProcessMemoryInfo.argtypes = [
            wintypes.HANDLE,
            ctypes.POINTER(ProcessMemoryCounters),
            wintypes.DWORD,
        ]
        psapi.GetProcessMemoryInfo.restype = wintypes.BOOL
        if not psapi.GetProcessMemoryInfo(
            kernel32.GetCurrentProcess(), ctypes.byref(counters), counters.cb
        ):
            raise ctypes.WinError(ctypes.get_last_error())
        return int(counters.working_set_size)
    raise RuntimeError("RSS measurement requires psutil outside Windows")


class Frame:
    def __init__(self, x: object, y: object, z: object | None = None) -> None:
        self.x = x
        self.y = y
        self.columns = ("x", "y")
        self.dtypes = ("float32", "float32")
        if z is not None:
            self.z = z
            self.columns = ("x", "y", "z")
            self.dtypes = ("float32", "float32", "float32")
        self.shape = (len(x), len(self.columns))  # type: ignore[arg-type]

    def __getitem__(self, name: str) -> object:
        return getattr(self, name)


def generate_xy(np: object, n: int, distribution: str, seed: int) -> tuple[object, object]:
    if distribution == "structured":
        x = np.linspace(-1.0, 1.0, n, dtype=np.float32)
        y = np.sin(x * np.float32(20.0), dtype=np.float32)
        return x, y

    rng = np.random.default_rng(seed)
    if distribution == "uniform":
        x = rng.uniform(-1.0, 1.0, n).astype(np.float32)
        y = rng.uniform(-1.0, 1.0, n).astype(np.float32)
    elif distribution == "clustered":
        centers = np.asarray(
            [
                (-0.75, -0.65),
                (-0.45, 0.55),
                (-0.10, -0.15),
                (0.15, 0.72),
                (0.42, -0.58),
                (0.72, 0.30),
            ],
            dtype=np.float32,
        )
        membership = rng.integers(0, len(centers), size=n)
        noise = rng.normal(0.0, 0.055, size=(n, 2)).astype(np.float32)
        values = np.clip(centers[membership] + noise, -1.0, 1.0)
        x, y = values[:, 0], values[:, 1]
    elif distribution == "skewed":
        values = rng.lognormal(mean=-2.4, sigma=1.05, size=(n, 2))
        values = np.clip(values, 0.0, 1.0).astype(np.float32)
        values = values * np.float32(2.0) - np.float32(1.0)
        x, y = values[:, 0], values[:, 1]
    else:  # argparse guards this; keep direct callers honest.
        raise ValueError(f"unknown distribution: {distribution}")

    x = np.ascontiguousarray(x, dtype=np.float32)
    y = np.ascontiguousarray(y, dtype=np.float32)
    # Stable extrema make grid occupancy comparable across repeated runs.
    if n >= 4:
        x[:4] = (-1.0, 1.0, 0.0, 0.0)
        y[:4] = (0.0, 0.0, -1.0, 1.0)
    return x, y


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--n", type=int, required=True)
    parser.add_argument("--mode", choices=("legacy", "store"), required=True)
    parser.add_argument("--plots", type=int, default=2)
    parser.add_argument("--dimensions", type=int, choices=(2, 3), default=2)
    parser.add_argument("--render-frames", type=int, default=0)
    parser.add_argument("--window-width", type=int, default=900)
    parser.add_argument("--window-height", type=int, default=600)
    parser.add_argument(
        "--rendering",
        choices=("exact", "decimated", "density", "adaptive"),
        default="exact",
    )
    parser.add_argument(
        "--source-retention", choices=("current", "none"), default="current"
    )
    parser.add_argument(
        "--distribution",
        choices=("structured", "uniform", "clustered", "skewed"),
        default="structured",
    )
    parser.add_argument("--seed", type=int, default=20260806)
    parser.add_argument("--reverse-source", action="store_true")
    parser.add_argument("--toggle-density-cache", action="store_true")
    parser.add_argument("--toggle-colormap-cache", action="store_true")
    parser.add_argument("--stream-updates", type=int, default=0)
    parser.add_argument("--viewport-half-scale", type=float)
    parser.add_argument("--viewport-supersede-half-scale", type=float)
    parser.add_argument("--viewport-supersede-delay", type=float, default=0.16)
    parser.add_argument("--viewport-all-plots", action="store_true")
    parser.add_argument("--viewport-lru-probe", action="store_true")
    parser.add_argument("--viewport-lru-delay", type=float, default=0.28)
    parser.add_argument("--expect-deferred-density-index", action="store_true")
    parser.add_argument("--expect-superseded-viewport", action="store_true")
    parser.add_argument("--expect-viewport-exact", action="store_true")
    parser.add_argument("--expect-viewport-roundtrip", action="store_true")
    parser.add_argument("--expect-derived-cache-hit", action="store_true")
    parser.add_argument("--expect-shared-decimation-cache", action="store_true")
    parser.add_argument("--expect-shared-decimation-gpu", action="store_true")
    parser.add_argument("--expect-shared-density-gpu", action="store_true")
    parser.add_argument("--expect-stream-latest-revision", action="store_true")
    parser.add_argument("--expect-derived-cache-lru", action="store_true")
    parser.add_argument("--expect-density-colormap-cache", action="store_true")
    parser.add_argument("--expect-source-retention-none", action="store_true")
    parser.add_argument("--source-viewport-race", action="store_true")
    parser.add_argument("--source-viewport-job-delay-ms", type=int, default=250)
    parser.add_argument("--expected-representative-fingerprint", type=int)
    parser.add_argument("--expected-density-grid", type=int, nargs=2, metavar=("WIDTH", "HEIGHT"))
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if args.viewport_supersede_half_scale is not None and args.viewport_half_scale is None:
        parser.error("--viewport-supersede-half-scale requires --viewport-half-scale")
    if (args.expect_superseded_viewport or args.expect_viewport_roundtrip) and (
        args.viewport_supersede_half_scale is None
    ):
        parser.error("supersession/roundtrip expectations require a superseding viewport")
    if args.expect_derived_cache_hit and args.viewport_supersede_half_scale is None:
        parser.error("--expect-derived-cache-hit requires a superseding viewport")
    if args.expect_shared_decimation_cache and (
        args.rendering != "decimated" or args.mode != "store" or args.plots < 2
    ):
        parser.error(
            "--expect-shared-decimation-cache requires store mode, decimated rendering, and at least two plots"
        )
    if args.expect_shared_decimation_gpu and not args.expect_shared_decimation_cache:
        parser.error(
            "--expect-shared-decimation-gpu requires --expect-shared-decimation-cache"
        )
    if args.expect_shared_density_gpu and (
        args.rendering != "density"
        or args.mode != "store"
        or args.plots < 2
        or not args.toggle_colormap_cache
    ):
        parser.error(
            "--expect-shared-density-gpu requires store mode, density rendering, at least two plots, and --toggle-colormap-cache"
        )
    if args.expect_stream_latest_revision and (
        args.mode != "store"
        or args.dimensions != 2
        or args.rendering not in {"density", "adaptive"}
        or args.stream_updates < 2
        or args.render_frames <= 0
    ):
        parser.error(
            "--expect-stream-latest-revision requires rendered 2D store density/adaptive mode and at least two stream updates"
        )
    if args.viewport_lru_probe and (
        args.viewport_half_scale is None or args.viewport_all_plots or args.plots != 1
    ):
        parser.error(
            "--viewport-lru-probe requires one plot, --viewport-half-scale, and no --viewport-all-plots"
        )
    if args.expect_derived_cache_lru and not args.viewport_lru_probe:
        parser.error("--expect-derived-cache-lru requires --viewport-lru-probe")
    if args.expect_density_colormap_cache and (
        not args.toggle_colormap_cache
        or args.rendering != "density"
        or args.mode != "store"
    ):
        parser.error(
            "--expect-density-colormap-cache requires --toggle-colormap-cache, store mode, and density rendering"
        )
    if args.expect_viewport_exact and args.rendering != "adaptive":
        parser.error("--expect-viewport-exact requires --rendering adaptive")
    if args.expect_source_retention_none and (
        args.source_retention != "none"
        or args.dimensions != 2
        or args.rendering not in {"density", "decimated", "adaptive"}
        or args.render_frames <= 0
    ):
        parser.error(
            "--expect-source-retention-none requires rendered 2D density, decimated, or adaptive mode with --source-retention none"
        )
    if args.source_viewport_race and (
        args.mode != "store"
        or args.plots != 1
        or args.dimensions != 2
        or args.rendering not in {"density", "adaptive"}
        or args.source_retention != "current"
        or args.stream_updates < 2
        or args.viewport_half_scale is None
        or args.render_frames <= 0
    ):
        parser.error(
            "--source-viewport-race requires one rendered 2D PointStore density/adaptive plot, current retention, at least two stream updates, and --viewport-half-scale"
        )

    sys.path.insert(0, str(args.package_root.resolve()))
    import numpy as np
    import dragongui as dg

    initial_rss = current_rss_bytes()
    x, y = generate_xy(np, args.n, args.distribution, args.seed)
    if args.reverse_source:
        x = np.ascontiguousarray(x[::-1])
        y = np.ascontiguousarray(y[::-1])
    z = (
        np.cos(x * np.float32(13.0), dtype=np.float32)
        if args.dimensions == 3
        else None
    )
    source_rss = current_rss_bytes()

    source = (
        dg.PointStore(x, y, z=z, ownership="borrowed")
        if args.mode == "store"
        else Frame(x, y, z)
    )
    app = dg.App(loading_screen=False) if args.render_frames > 0 else None
    window = (
        dg.Window(
            "DragonGUI PointStore benchmark",
            width=args.window_width,
            height=args.window_height,
        )
        if app is not None
        else None
    )
    build_t0 = time.perf_counter()
    def make_plot(*, detached: bool) -> object:
        if args.dimensions == 3:
            if detached:
                return dg.Scatter3D(
                    source, x="x", y="y", z="z", rendering=args.rendering,
                    source_retention=args.source_retention, parent=None
                )
            return dg.Scatter3D(
                source, x="x", y="y", z="z", rendering=args.rendering,
                source_retention=args.source_retention
            )
        if detached:
            return dg.ScatterPlot2D(
                source, rendering=args.rendering,
                source_retention=args.source_retention, parent=None
            )
        return dg.ScatterPlot2D(
            source, rendering=args.rendering,
            source_retention=args.source_retention
        )

    if window is None:
        plots = [make_plot(detached=True) for _ in range(args.plots)]
    else:
        with window:
            plots = [make_plot(detached=False) for _ in range(args.plots)]
    build_ms = (time.perf_counter() - build_t0) * 1000.0
    final_rss = current_rss_bytes()

    stale_stream_probe = None
    if args.expect_stream_latest_revision:
        x_name = plots[0]._source_x
        y_name = plots[0]._source_y
        stale_stream_probe = (
            source._dragongui_pack_xy(x_name, y_name),
            *source._dragongui_xy_source(x_name, y_name),
        )

    render_result = None
    if app is not None and window is not None:
        interaction_timers = []
        if (
            args.toggle_density_cache
            or args.toggle_colormap_cache
            or args.stream_updates > 0
            or args.viewport_half_scale is not None
        ):
            import threading

            def apply_benchmark_interaction() -> None:
                if args.toggle_density_cache:
                    plots[0].set_rendering("exact")
                    plots[0].set_rendering(args.rendering)
                if args.toggle_colormap_cache:
                    plots[0].set_colormap("plasma")
                if args.stream_updates > 0 and not args.source_viewport_race:
                    for update_index in range(1, args.stream_updates + 1):
                        updated_y = np.ascontiguousarray(
                            y + np.float32(update_index * 0.001), dtype=np.float32
                        )
                        source.replace_column("y", updated_y, ownership="moved")
                        for plot in plots:
                            plot.set_points(source)
                    if stale_stream_probe is not None:
                        stale_payload, stale_store_id, stale_revision = stale_stream_probe

                        def enqueue_stale_stream_revision() -> None:
                            handle = plots[0]._live()
                            if handle is not None:
                                handle.enqueue_set_scatter_store_points_packed(
                                    stale_payload,
                                    store_id=stale_store_id,
                                    revision=stale_revision,
                                    colormap=plots[0].colormap,
                                    payload_format="xy_f32_v0",
                                    coalesce=True,
                                    fit=False,
                                )

                        # First enqueue in the same batch, after the newest frame,
                        # to exercise revision-aware batch coalescing.
                        enqueue_stale_stream_revision()
                        # Then enqueue once more in a later batch to exercise the
                        # runtime's cross-batch revision watermark.
                        stale_timer = threading.Timer(
                            0.15,
                            lambda: app.call_soon_threadsafe(
                                enqueue_stale_stream_revision
                            ),
                        )
                        stale_timer.daemon = True
                        interaction_timers.append(stale_timer)
                        stale_timer.start()
                if args.viewport_half_scale is not None:
                    viewport_plots = plots if args.viewport_all_plots else plots[:1]
                    for plot in viewport_plots:
                        plot.set_parallel_scale(
                            args.viewport_half_scale, args.viewport_half_scale
                        )
                    if args.viewport_supersede_half_scale is not None:
                        def apply_superseding_viewport() -> None:
                            for plot in viewport_plots:
                                plot.set_parallel_scale(
                                    args.viewport_supersede_half_scale,
                                    args.viewport_supersede_half_scale,
                                )

                        supersede_timer = threading.Timer(
                            args.viewport_supersede_delay,
                            lambda: app.call_soon_threadsafe(
                                apply_superseding_viewport
                            ),
                        )
                        supersede_timer.daemon = True
                        interaction_timers.append(supersede_timer)
                        supersede_timer.start()
                if args.source_viewport_race:
                    for update_index in range(1, args.stream_updates + 1):
                        def apply_source_viewport_update(index: int = update_index) -> None:
                            updated_y = np.ascontiguousarray(
                                y + np.float32(index * 0.001), dtype=np.float32
                            )
                            source.replace_column("y", updated_y, ownership="moved")
                            plots[0].set_points(source)
                            scale = max(
                                0.05,
                                float(args.viewport_half_scale) - 0.04 * index,
                            )
                            plots[0].set_parallel_scale(scale, scale)

                        race_timer = threading.Timer(
                            0.22 + 0.28 * (update_index - 1),
                            lambda callback=apply_source_viewport_update: app.call_soon_threadsafe(
                                callback
                            ),
                        )
                        race_timer.daemon = True
                        interaction_timers.append(race_timer)
                        race_timer.start()
                    if args.viewport_lru_probe:
                        base_scale = args.viewport_half_scale
                        lru_scales = [
                            base_scale + 0.003 * index for index in range(1, 15)
                        ]
                        # Refresh the oldest viewport just before inserting two
                        # more products that force byte-budget eviction, then
                        # revisit it once more to require a surviving LRU hit.
                        lru_scales.extend(
                            [
                                base_scale,
                                base_scale + 0.045,
                                base_scale + 0.048,
                                base_scale,
                            ]
                        )
                        for index, scale in enumerate(lru_scales, start=1):
                            lru_timer = threading.Timer(
                                args.viewport_lru_delay * index,
                                lambda selected_scale=scale: app.call_soon_threadsafe(
                                    lambda: plots[0].set_parallel_scale(
                                        selected_scale, selected_scale
                                    )
                                ),
                            )
                            lru_timer.daemon = True
                            interaction_timers.append(lru_timer)
                            lru_timer.start()

            toggle_timer = threading.Timer(
                0.05, lambda: app.call_soon_threadsafe(apply_benchmark_interaction)
            )
            toggle_timer.daemon = True
            interaction_timers.append(toggle_timer)
            toggle_timer.start()
        previous_smoke = os.environ.get("DRAGONGUI_SMOKE_FRAMES")
        previous_viewport_delay = os.environ.get(
            "DRAGONGUI_BENCH_SCATTER_VIEWPORT_JOB_DELAY_MS"
        )
        os.environ["DRAGONGUI_SMOKE_FRAMES"] = str(args.render_frames)
        if args.source_viewport_race:
            os.environ["DRAGONGUI_BENCH_SCATTER_VIEWPORT_JOB_DELAY_MS"] = str(
                max(0, args.source_viewport_job_delay_ms)
            )
        try:
            raw_result = app.run(window)
            snapshot = raw_result.get("debug_snapshot", {})
            gpu = snapshot.get("gpu", {}) if isinstance(snapshot, dict) else {}
            renderer = gpu.get("renderer", {}) if isinstance(gpu, dict) else {}
            resources = gpu.get("resources", {}) if isinstance(gpu, dict) else {}
            scatters = resources.get("scatters", {}) if isinstance(resources, dict) else {}
            point_stores = resources.get("point_stores", {}) if isinstance(resources, dict) else {}
            runtime = snapshot.get("runtime", {}) if isinstance(snapshot, dict) else {}
            runtime_command_drain = (
                runtime.get("command_drain", {}) if isinstance(runtime, dict) else {}
            )
            runtime_coalescing = (
                runtime_command_drain.get("coalescing", {})
                if isinstance(runtime_command_drain, dict)
                else {}
            )
            runtime_command_queue = (
                runtime.get("command_queue", {}) if isinstance(runtime, dict) else {}
            )
            python_runtime = runtime.get("python", {}) if isinstance(runtime, dict) else {}
            native_sends = python_runtime.get("native_sends", {}) if isinstance(python_runtime, dict) else {}
            send_methods = native_sends.get("methods", {}) if isinstance(native_sends, dict) else {}
            render_result = {
                "status": raw_result.get("status"),
                "frame_ms": raw_result.get("frame_ms"),
                "upload_ms": raw_result.get("upload_ms"),
                "scatter_count": renderer.get("scatter_count") if isinstance(renderer, dict) else None,
                "scatter_points": {
                    widget_id: metrics.get("last_point_count")
                    for widget_id, metrics in scatters.items()
                    if isinstance(metrics, dict)
                } if isinstance(scatters, dict) else {},
                "scatter_sources": {
                    widget_id: metrics.get("point_store_source")
                    for widget_id, metrics in scatters.items()
                    if isinstance(metrics, dict)
                } if isinstance(scatters, dict) else {},
                "scatter_gpu": {
                    widget_id: metrics.get("gpu_memory")
                    for widget_id, metrics in scatters.items()
                    if isinstance(metrics, dict)
                } if isinstance(scatters, dict) else {},
                "scatter_representations": {
                    widget_id: metrics.get("representation")
                    for widget_id, metrics in scatters.items()
                    if isinstance(metrics, dict)
                } if isinstance(scatters, dict) else {},
                "point_stores": point_stores,
                "coalescing": runtime_coalescing,
                "command_queue": runtime_command_queue,
                "point_store_commands": {
                    name: stats.get("direct", 0) + stats.get("flushed_after_bind", 0)
                    for name, stats in send_methods.items()
                    if name in {
                        "enqueue_set_scatter_store_points_packed",
                        "enqueue_set_scatter_point_store_reference",
                    } and isinstance(stats, dict)
                } if isinstance(send_methods, dict) else {},
            }
        finally:
            for interaction_timer in interaction_timers:
                interaction_timer.cancel()
            if previous_smoke is None:
                os.environ.pop("DRAGONGUI_SMOKE_FRAMES", None)
            else:
                os.environ["DRAGONGUI_SMOKE_FRAMES"] = previous_smoke
            if previous_viewport_delay is None:
                os.environ.pop(
                    "DRAGONGUI_BENCH_SCATTER_VIEWPORT_JOB_DELAY_MS", None
                )
            else:
                os.environ[
                    "DRAGONGUI_BENCH_SCATTER_VIEWPORT_JOB_DELAY_MS"
                ] = previous_viewport_delay

    payloads = [plot._cached_payload for plot in plots]
    unique_payloads = {id(payload): payload for payload in payloads if payload is not None}
    referenced_payload_bytes = sum(len(payload) for payload in payloads if payload is not None)
    unique_payload_bytes = sum(len(payload) for payload in unique_payloads.values())
    shared_payload = len(unique_payloads) == 1 if plots else True
    expected_shared = args.mode == "store" or args.plots <= 1
    policy_valid = True
    density_cache_valid = True
    viewport_valid = True
    viewport_index_valid = True
    deferred_density_index_valid = True
    superseded_viewport_valid = True
    viewport_exact_valid = True
    viewport_roundtrip_valid = True
    decimated_valid = True
    representative_fingerprint_valid = True
    derived_cache_valid = True
    shared_decimation_cache_valid = True
    shared_decimation_gpu_valid = True
    shared_density_gpu_valid = True
    stream_latest_revision_valid = True
    derived_cache_lru_valid = True
    density_colormap_cache_valid = True
    density_grid_valid = True
    source_retention_valid = True
    source_viewport_race_valid = True
    if render_result is not None:
        representations = render_result.get("scatter_representations", {})
        density_initial_expected = args.dimensions == 2 and (
            args.rendering == "density"
            or (args.rendering == "adaptive" and args.n >= 300_000)
        )
        expected_effective = (
            "exact"
            if args.expect_viewport_exact
            else (
                "decimated"
                if args.rendering == "decimated" and args.dimensions == 2
                else ("density" if density_initial_expected else "exact")
            )
        )
        policy_valid = bool(representations) and all(
            isinstance(representation, dict)
            and representation.get("policy_requested") == args.rendering
            and representation.get("policy_effective") == expected_effective
            for representation in representations.values()
        )
        if density_initial_expected and args.mode == "store":
            point_store_stats = render_result.get("point_stores", {})
            density_cache_valid = (
                isinstance(point_store_stats, dict)
                and point_store_stats.get("density_cache_entries") == 1
                and int(point_store_stats.get("density_cache_misses", -1))
                == 1
                + (
                    args.stream_updates
                    if args.source_viewport_race
                    else int(args.stream_updates > 0)
                )
                and int(point_store_stats.get("density_cache_hits", 0))
                >= max(0, args.plots - 1)
                * (
                    1
                    + (
                        args.stream_updates
                        if args.source_viewport_race
                        else int(args.stream_updates > 0)
                    )
                )
                + int(args.toggle_density_cache)
            )
        if args.viewport_half_scale is not None:
            viewport_valid = bool(representations) and all(
                isinstance(representation, dict)
                and isinstance(representation.get("density"), dict)
                and (
                    representation["density"].get("scope") == "viewport"
                    or (
                        args.expect_viewport_roundtrip
                        and representation["density"].get("scope") == "full"
                    )
                )
                and representation["density"].get("source_rows_conserved") is True
                and int(representation["density"].get("viewport_rebuilds", 0)) >= 1
                for representation in representations.values()
            )
            if args.mode == "store" and (
                density_initial_expected or args.rendering == "decimated"
            ):
                viewport_index_valid = bool(representations) and all(
                    isinstance(representation, dict)
                    and isinstance(representation.get("density"), dict)
                    and (
                        representation["density"].get("spatial_index_used") is True
                        or (
                            args.expect_viewport_roundtrip
                            and representation["density"].get("spatial_index_status")
                            == "ready"
                        )
                    )
                    and (
                        args.expect_viewport_roundtrip
                        or int(
                            representation["density"].get(
                                "scanned_source_rows", args.n
                            )
                        )
                        < args.n
                    )
                    for representation in representations.values()
                )
        if args.expect_deferred_density_index:
            point_store_stats = render_result.get("point_stores", {})
            deferred_density_index_valid = (
                bool(representations)
                and all(
                    isinstance(representation, dict)
                    and isinstance(representation.get("density"), dict)
                    and representation["density"].get("scope") == "full"
                    and representation["density"].get("spatial_index_status")
                    == "deferred"
                    and representation["density"].get("spatial_index_used") is False
                    for representation in representations.values()
                )
                and isinstance(point_store_stats, dict)
                and int(point_store_stats.get("density_spatial_index_unique_bytes", -1))
                == 0
                and float(point_store_stats.get("density_spatial_index_build_ms", -1.0))
                == 0.0
            )
        if args.expect_superseded_viewport:
            superseded_viewport_valid = bool(representations) and all(
                isinstance(representation, dict)
                and isinstance(representation.get("density"), dict)
                and int(representation["density"].get("viewport_jobs_started", 0))
                >= 2
                and int(representation["density"].get("viewport_jobs_completed", 0))
                >= 1
                and int(representation["density"].get("viewport_jobs_stale", 0))
                >= 1
                and int(representation["density"].get("viewport_job_errors", 0)) == 0
                and representation["density"].get("viewport_job_pending") is False
                for representation in representations.values()
            )
        if args.expect_viewport_exact:
            viewport_exact_valid = bool(representations) and all(
                isinstance(representation, dict)
                and representation.get("selected")
                == "exact_viewport_point_instance_v1"
                and representation.get("policy_effective") == "exact"
                and isinstance(representation.get("density"), dict)
                and representation["density"].get("viewport_representation")
                == "exact"
                and representation["density"].get("source_rows_conserved") is True
                and int(representation.get("render_rows", -1))
                == int(representation["density"].get("finite_source_rows", -2))
                for representation in representations.values()
            )
        if args.expect_viewport_roundtrip:
            viewport_roundtrip_valid = bool(representations) and all(
                isinstance(representation, dict)
                and representation.get("selected") == "density_grid_point_instance_v1"
                and representation.get("policy_effective") == "density"
                and isinstance(representation.get("density"), dict)
                and representation["density"].get("viewport_representation")
                == "density"
                and representation["density"].get("scope") == "full"
                and int(representation["density"].get("viewport_jobs_completed", 0))
                >= 2
                and int(representation["density"].get("viewport_jobs_stale", 0)) == 0
                for representation in representations.values()
            )
        if args.rendering == "decimated" and args.dimensions == 2:
            decimated_valid = bool(representations) and all(
                isinstance(representation, dict)
                and representation.get("selected")
                == "decimated_grid_source_points_v1"
                and representation.get("policy_effective") == "decimated"
                and isinstance(representation.get("density"), dict)
                and representation["density"].get("viewport_representation")
                == "decimated"
                and int(representation.get("render_rows", 0)) > 0
                and int(representation.get("render_rows", 65_541)) <= 65_540
                and int(
                    representation["density"].get(
                        "representative_fingerprint", 0
                    )
                )
                != 0
                for representation in representations.values()
            )
        if args.expected_representative_fingerprint is not None:
            representative_fingerprint_valid = bool(representations) and all(
                isinstance(representation, dict)
                and isinstance(representation.get("density"), dict)
                and int(
                    representation["density"].get(
                        "representative_fingerprint", -1
                    )
                )
                == args.expected_representative_fingerprint
                for representation in representations.values()
            )
        if args.expect_derived_cache_hit:
            point_store_stats = render_result.get("point_stores", {})
            derived_cache_valid = (
                bool(representations)
                and all(
                    isinstance(representation, dict)
                    and representation.get("derived_cache_hit") is True
                    and isinstance(representation.get("density"), dict)
                    and int(representation["density"].get("viewport_jobs_completed", 0))
                    >= 2
                    for representation in representations.values()
                )
                and isinstance(point_store_stats, dict)
                and int(point_store_stats.get("derived_cache_entries", 0)) >= 1
                and int(point_store_stats.get("derived_cache_misses", 0)) >= 1
                and int(point_store_stats.get("derived_cache_hits", 0)) >= 1
            )
        if args.expect_shared_decimation_cache:
            point_store_stats = render_result.get("point_stores", {})
            cache_hit_count = sum(
                1
                for representation in representations.values()
                if isinstance(representation, dict)
                and representation.get("derived_cache_hit") is True
            )
            fingerprints = {
                int(representation["density"].get("representative_fingerprint", -1))
                for representation in representations.values()
                if isinstance(representation, dict)
                and isinstance(representation.get("density"), dict)
            }
            shared_decimation_cache_valid = (
                len(representations) == args.plots
                and cache_hit_count >= args.plots - 1
                and len(fingerprints) == 1
                and -1 not in fingerprints
                and isinstance(point_store_stats, dict)
                and int(point_store_stats.get("derived_cache_entries", -1)) == 1
                and int(point_store_stats.get("derived_cache_misses", -1)) == 1
                and int(point_store_stats.get("derived_cache_hits", 0))
                >= args.plots - 1
            )
        if args.expect_shared_decimation_gpu:
            point_store_stats = render_result.get("point_stores", {})
            scatter_gpu = render_result.get("scatter_gpu", {})
            gpu_hit_count = sum(
                1
                for representation in representations.values()
                if isinstance(representation, dict)
                and representation.get("derived_gpu_cache_hit") is True
            )
            shared_decimation_gpu_valid = (
                len(scatter_gpu) == args.plots
                and all(
                    isinstance(memory, dict)
                    and memory.get("primary_shared") is True
                    for memory in scatter_gpu.values()
                )
                and gpu_hit_count >= args.plots - 1
                and isinstance(point_store_stats, dict)
                and point_store_stats.get("derived_gpu_scope")
                == "density_and_decimated_point_instance_v1"
                and int(point_store_stats.get("derived_gpu_entries", -1)) == 1
                and int(point_store_stats.get("derived_gpu_scatter_references", -1))
                == args.plots
                and int(point_store_stats.get("derived_gpu_unique_used_bytes", 0)) > 0
                and int(point_store_stats.get("derived_gpu_unique_allocated_bytes", 0))
                >= int(point_store_stats.get("derived_gpu_unique_used_bytes", 0))
            )
        if args.expect_shared_density_gpu:
            point_store_stats = render_result.get("point_stores", {})
            scatter_gpu = render_result.get("scatter_gpu", {})
            presentation_colormaps = {
                representation.get("density", {}).get("presentation_colormap")
                for representation in representations.values()
                if isinstance(representation, dict)
                and isinstance(representation.get("density"), dict)
            }
            shared_density_gpu_valid = (
                len(scatter_gpu) == args.plots
                and all(
                    isinstance(memory, dict)
                    and memory.get("primary_shared") is True
                    for memory in scatter_gpu.values()
                )
                and presentation_colormaps == {"viridis", "plasma"}
                and isinstance(point_store_stats, dict)
                and point_store_stats.get("derived_gpu_scope")
                == "density_and_decimated_point_instance_v1"
                and int(point_store_stats.get("derived_gpu_entries", -1)) == 1
                and int(point_store_stats.get("derived_gpu_scatter_references", -1))
                == args.plots
                and int(point_store_stats.get("derived_gpu_unique_used_bytes", 0)) > 0
                and int(point_store_stats.get("derived_gpu_unique_allocated_bytes", 0))
                >= int(point_store_stats.get("derived_gpu_unique_used_bytes", 0))
            )
        if args.expect_stream_latest_revision:
            point_store_stats = render_result.get("point_stores", {})
            scatter_gpu = render_result.get("scatter_gpu", {})
            scatter_sources = render_result.get("scatter_sources", {})
            command_queue = render_result.get("command_queue", {})
            replacement_families = (
                command_queue.get("replacements_by_family", {})
                if isinstance(command_queue, dict)
                else {}
            )
            expected_revision = int(source.stats()["data_revision"])
            stream_latest_revision_valid = (
                len(scatter_sources) == args.plots
                and all(
                    isinstance(store_source, dict)
                    and int(store_source.get("revision", -1)) == expected_revision
                    for store_source in scatter_sources.values()
                )
                and all(
                    isinstance(memory, dict)
                    and 0 < int(memory.get("primary_used_bytes", 0))
                    <= 65_536 * 32
                    for memory in scatter_gpu.values()
                )
                and isinstance(point_store_stats, dict)
                and int(point_store_stats.get("revision_count", -1)) == 1
                and int(point_store_stats.get("density_cache_entries", -1)) == 1
                and int(point_store_stats.get("stream_revision_advances", 0)) >= 1
                and int(point_store_stats.get("stream_stale_updates_dropped", 0)) >= 1
                and int(
                    point_store_stats.get(
                        "stream_point_store_revisions_invalidated", 0
                    )
                )
                >= 1
                and int(
                    point_store_stats.get("stream_density_products_invalidated", 0)
                )
                >= 1
                and int(
                    point_store_stats.get(
                        "stream_derived_gpu_products_invalidated", 0
                    )
                )
                >= 1
                and isinstance(replacement_families, dict)
                and int(replacement_families.get("scatter_points", 0))
                >= args.plots * (args.stream_updates - 1) + 1
            )
        if args.expect_derived_cache_lru:
            point_store_stats = render_result.get("point_stores", {})
            derived_cache_lru_valid = (
                bool(representations)
                and all(
                    isinstance(representation, dict)
                    and representation.get("derived_cache_hit") is True
                    and isinstance(representation.get("density"), dict)
                    and int(representation["density"].get("viewport_jobs_completed", 0))
                    >= 19
                    for representation in representations.values()
                )
                and isinstance(point_store_stats, dict)
                and point_store_stats.get("derived_cache_eviction_policy") == "lru"
                and int(point_store_stats.get("derived_cache_evictions", 0)) >= 2
                and int(point_store_stats.get("derived_cache_recency_updates", 0)) >= 2
                and int(point_store_stats.get("derived_cache_hits", 0)) >= 2
                and int(point_store_stats.get("derived_cache_misses", 0)) >= 18
                and int(point_store_stats.get("derived_cache_entries", 0))
                <= int(point_store_stats.get("derived_cache_max_entries", 0))
                and int(point_store_stats.get("derived_cache_retained_bytes", 0))
                <= int(point_store_stats.get("derived_cache_max_bytes", 0))
            )
        if args.expect_density_colormap_cache:
            point_store_stats = render_result.get("point_stores", {})
            density_colormap_cache_valid = (
                bool(representations)
                and all(
                    isinstance(representation, dict)
                    and representation.get("density_cache_hit") is True
                    and isinstance(representation.get("density"), dict)
                    and representation["density"].get("presentation_colormap")
                    == "plasma"
                    for representation in representations.values()
                )
                and isinstance(point_store_stats, dict)
                and int(point_store_stats.get("density_cache_entries", -1)) == 1
                and int(point_store_stats.get("density_cache_misses", -1)) == 1
                and int(point_store_stats.get("density_cache_hits", 0)) >= 1
            )
        if args.expected_density_grid is not None:
            expected_grid_width, expected_grid_height = args.expected_density_grid
            density_grid_valid = bool(representations) and all(
                isinstance(representation, dict)
                and isinstance(representation.get("density"), dict)
                and int(representation["density"].get("grid_width", -1))
                == expected_grid_width
                and int(representation["density"].get("grid_height", -1))
                == expected_grid_height
                and representation["density"].get("grid_resolution_policy")
                == "one_cell_per_two_physical_pixels_clamped_32_256"
                for representation in representations.values()
            )
        if args.expect_source_retention_none:
            point_store_stats = render_result.get("point_stores", {})
            scatter_sources = render_result.get("scatter_sources", {})
            source_retention_valid = (
                len(representations) == args.plots
                and all(
                    isinstance(representation, dict)
                    and representation.get("source_retention_requested") == "none"
                    and representation.get("source_retention_effective") == "none"
                    and int(representation.get("source_bytes_retained", -1)) == 0
                    and int(representation.get("source_bytes_released_total", 0)) > 0
                    for representation in representations.values()
                )
                and all(
                    isinstance(store_source, dict)
                    and int(store_source.get("payload_bytes", -1)) == 0
                    for store_source in scatter_sources.values()
                )
                and isinstance(point_store_stats, dict)
                and int(point_store_stats.get("revision_count", -1)) == 0
                and int(point_store_stats.get("unique_payload_bytes", -1)) == 0
            )
        if args.source_viewport_race:
            point_store_stats = render_result.get("point_stores", {})
            scatter_sources = render_result.get("scatter_sources", {})
            expected_revision = int(source.stats()["data_revision"])
            source_viewport_race_valid = (
                len(representations) == 1
                and all(
                    isinstance(representation, dict)
                    and isinstance(representation.get("density"), dict)
                    and int(representation["density"].get("source_revision", -1))
                    == expected_revision
                    and int(representation["density"].get("viewport_jobs_stale", 0))
                    >= args.stream_updates
                    and int(representation["density"].get("viewport_jobs_completed", 0))
                    >= 1
                    and representation["density"].get("viewport_job_pending") is False
                    and int(representation["density"].get("viewport_job_errors", -1)) == 0
                    for representation in representations.values()
                )
                and all(
                    isinstance(store_source, dict)
                    and int(store_source.get("revision", -1)) == expected_revision
                    for store_source in scatter_sources.values()
                )
                and isinstance(point_store_stats, dict)
                and int(point_store_stats.get("revision_count", -1)) == 1
                and int(point_store_stats.get("density_cache_entries", -1)) == 1
                and int(point_store_stats.get("stream_revision_advances", 0))
                >= args.stream_updates
            )
    payload = {
        "status": (
            "ok"
            if shared_payload == expected_shared
            and policy_valid
            and density_cache_valid
            and viewport_valid
            and viewport_index_valid
            and deferred_density_index_valid
            and superseded_viewport_valid
            and viewport_exact_valid
            and viewport_roundtrip_valid
            and decimated_valid
            and representative_fingerprint_valid
            and derived_cache_valid
            and shared_decimation_cache_valid
            and shared_decimation_gpu_valid
            and shared_density_gpu_valid
            and stream_latest_revision_valid
            and derived_cache_lru_valid
            and density_colormap_cache_valid
            and density_grid_valid
            and source_retention_valid
            and source_viewport_race_valid
            else "invalid"
        ),
        "mode": args.mode,
        "n": args.n,
        "plots": args.plots,
        "dimensions": args.dimensions,
        "rendering": args.rendering,
        "source_retention": args.source_retention,
        "source_viewport_race": args.source_viewport_race,
        "source_viewport_job_delay_ms": args.source_viewport_job_delay_ms,
        "window_width": args.window_width,
        "window_height": args.window_height,
        "distribution": args.distribution,
        "seed": args.seed,
        "reverse_source": args.reverse_source,
        "toggle_density_cache": args.toggle_density_cache,
        "toggle_colormap_cache": args.toggle_colormap_cache,
        "stream_updates": args.stream_updates,
        "viewport_half_scale": args.viewport_half_scale,
        "viewport_supersede_half_scale": args.viewport_supersede_half_scale,
        "viewport_supersede_delay": args.viewport_supersede_delay,
        "viewport_all_plots": args.viewport_all_plots,
        "viewport_lru_probe": args.viewport_lru_probe,
        "viewport_lru_delay": args.viewport_lru_delay,
        "build_ms": build_ms,
        "initial_rss_bytes": initial_rss,
        "source_rss_bytes": source_rss,
        "final_rss_bytes": final_rss,
        "widget_rss_delta_bytes": final_rss - source_rss,
        "referenced_payload_bytes": referenced_payload_bytes,
        "unique_payload_bytes": unique_payload_bytes,
        "shared_payload": shared_payload,
        "policy_valid": policy_valid,
        "density_cache_valid": density_cache_valid,
        "viewport_valid": viewport_valid,
        "viewport_index_valid": viewport_index_valid,
        "deferred_density_index_valid": deferred_density_index_valid,
        "superseded_viewport_valid": superseded_viewport_valid,
        "viewport_exact_valid": viewport_exact_valid,
        "viewport_roundtrip_valid": viewport_roundtrip_valid,
        "decimated_valid": decimated_valid,
        "representative_fingerprint_valid": representative_fingerprint_valid,
        "derived_cache_valid": derived_cache_valid,
        "shared_decimation_cache_valid": shared_decimation_cache_valid,
        "shared_decimation_gpu_valid": shared_decimation_gpu_valid,
        "shared_density_gpu_valid": shared_density_gpu_valid,
        "stream_latest_revision_valid": stream_latest_revision_valid,
        "derived_cache_lru_valid": derived_cache_lru_valid,
        "density_colormap_cache_valid": density_colormap_cache_valid,
        "density_grid_valid": density_grid_valid,
        "source_retention_valid": source_retention_valid,
        "source_viewport_race_valid": source_viewport_race_valid,
        "store": source.stats() if args.mode == "store" else None,
        "render_result": render_result,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload))
    if payload["status"] != "ok":
        raise SystemExit(1)


if __name__ == "__main__":
    main()
