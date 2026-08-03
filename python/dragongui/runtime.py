from __future__ import annotations

import base64
from collections import deque, OrderedDict
from collections.abc import Callable, Iterable, Mapping, Sequence
import inspect
import json
import math
from threading import local, RLock
import time
import traceback
from typing import Any
import warnings

from .diagnostics import _get_collector as _diagnostics_collector
from .diagnostics import record_task_failure as _record_task_failure


_MAX_PYTHON_TASKS_PER_DRAIN = 100
_PYTHON_TASK_DRAIN_BUDGET_MS = 6.0
_PYTHON_TASK_QUEUE_WARNING_THRESHOLD = 256
_TOAST_LEVELS = {"info", "success", "warning", "error"}
_TOAST_POSITIONS = {"top-right", "top-left", "bottom-right", "bottom-left"}
_active_app_handle: AppHandle | None = None
_active_app_lock = RLock()


def _callable_label(fn: Callable[[], None]) -> str:
    name = getattr(fn, "__qualname__", None) or getattr(fn, "__name__", None)
    if name:
        module = getattr(fn, "__module__", None)
        return f"{module}.{name}" if module and module != "__main__" else str(name)
    return type(fn).__name__


def _record_timing_stat(bucket: dict[str, Any], elapsed_ms: float) -> None:
    count = int(bucket.get("count", 0)) + 1
    total = float(bucket.get("total_ms", 0.0)) + elapsed_ms
    bucket["count"] = count
    bucket["last_ms"] = elapsed_ms
    bucket["total_ms"] = total
    bucket["avg_ms"] = total / count
    bucket["max_ms"] = max(float(bucket.get("max_ms", 0.0)), elapsed_ms)


class _ScheduledPythonTask:
    __slots__ = ("fn", "origin", "diagnostics", "coalesce_key", "sequence")

    def __init__(
        self,
        fn: Callable[[], None],
        origin: Any | None,
        diagnostics: bool,
        coalesce_key: object | None,
        sequence: int,
    ) -> None:
        self.fn = fn
        self.origin = origin
        self.diagnostics = diagnostics
        self.coalesce_key = coalesce_key
        self.sequence = sequence


class _UpdateBatchContext:
    """Nestable context that collects live SetProp calls for one app handle."""

    def __init__(self, handle: AppHandle) -> None:
        self._handle = handle

    def __enter__(self) -> _UpdateBatchContext:
        self._handle._begin_update_batch()
        return self

    def __exit__(self, exc_type: object, exc: object, traceback: object) -> bool:
        self._handle._end_update_batch()
        return False


class ToastHandle:
    """Handle for a native toast notification shown by a running app."""

    def __init__(self, app: AppHandle, toast_id: str) -> None:
        self._app = app
        self.id = toast_id

    def update(
        self,
        message: object,
        *,
        level: str = "info",
        duration: int | float | None = 3000,
        opacity: int | float | None = None,
        radius: int | float | None = None,
        padding: int | float | None = None,
        position: str | None = None,
    ) -> None:
        """Replace this toast's message, level, duration, and optional styling."""
        self._app.enqueue_show_toast(
            self.id,
            message,
            level=level,
            duration=duration,
            opacity=opacity,
            radius=radius,
            padding=padding,
            position=position,
        )

    def dismiss(self) -> None:
        """Dismiss this toast if it is still visible."""
        self._app.enqueue_dismiss_toast(self.id)


def current_app_handle() -> AppHandle:
    with _active_app_lock:
        handle = _active_app_handle
    if handle is None or handle.closed:
        raise RuntimeError("DragonGUI app is not running")
    return handle


def _set_active_app_handle(handle: AppHandle | None) -> None:
    global _active_app_handle
    with _active_app_lock:
        _active_app_handle = handle


class LiveWidgetHandle:
    """Internal handle connecting a Python widget object to a running app."""

    def __init__(self, app: AppHandle, widget_id: str) -> None:
        self.app = app
        self.id = widget_id

    @property
    def closed(self) -> bool:
        return self.app.closed

    def ensure_open(self) -> None:
        if self.closed:
            raise RuntimeError("DragonGUI widget handle is closed")

    def enqueue_set_prop(self, prop: str, value: object) -> None:
        self.ensure_open()
        self.app.enqueue_set_prop(self.id, prop, value)

    def enqueue_invalidate(self, dirty: str) -> None:
        self.ensure_open()
        self.app.enqueue_invalidate(self.id, dirty)

    def enqueue_set_style(self, style: object) -> None:
        self.ensure_open()
        self.app.enqueue_set_style(self.id, style)

    def enqueue_replace_children(self, children: object) -> None:
        self.ensure_open()
        self.app.enqueue_replace_children(self.id, children)

    def enqueue_replace_node(self, node: object) -> None:
        self.ensure_open()
        self.app.enqueue_replace_node(self.id, node)

    def enqueue_update_extension_display_list(self, display_list: object) -> bool:
        self.ensure_open()
        return self.app.enqueue_update_extension_display_list(self.id, display_list)

    def enqueue_set_scatter_points_packed(
        self,
        xyz: bytes,
        *,
        pack_ms: float | None = None,
        enqueue_epoch_ms: float | None = None,
        colormap: str = "viridis",
        payload_format: str = "xyz_f32_v0",
        coalesce: bool = True,
        fit: bool = False,
        bounds_min: tuple[float, float, float] | None = None,
        bounds_max: tuple[float, float, float] | None = None,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_points_packed(
            self.id,
            xyz,
            pack_ms=pack_ms,
            enqueue_epoch_ms=enqueue_epoch_ms,
            colormap=colormap,
            payload_format=payload_format,
            coalesce=coalesce,
            fit=fit,
            bounds_min=bounds_min,
            bounds_max=bounds_max,
        )

    def enqueue_set_line_plot_data_packed(
        self,
        series: str,
        xy: bytes,
        *,
        label: str | None = None,
        color: object | None = None,
        line_width: float | None = None,
        line_style: str | None = None,
        show_grid: bool | None = None,
        auto_fit: bool | None = None,
        max_points: int | None = None,
        fit: bool = True,
        coalesce: bool = True,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_set_line_plot_data_packed(
            self.id,
            series,
            xy,
            label=label,
            color=color,
            line_width=line_width,
            line_style=line_style,
            show_grid=show_grid,
            auto_fit=auto_fit,
            max_points=max_points,
            fit=fit,
            coalesce=coalesce,
        )

    def enqueue_append_line_plot_points_packed(
        self,
        series: str,
        xy: bytes,
        *,
        max_points: int | None = None,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_append_line_plot_points_packed(
            self.id,
            series,
            xy,
            max_points=max_points,
        )

    def enqueue_clear_line_plot_series(self, series: str | None = None) -> None:
        self.ensure_open()
        self.app.enqueue_clear_line_plot_series(self.id, series)

    def enqueue_set_histogram_data(
        self,
        edges: Sequence[object],
        counts: Sequence[object],
        *,
        input_count: int,
        finite_count: int,
        auto_fit: bool = True,
        coalesce: bool = True,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_set_histogram_data(
            self.id,
            edges,
            counts,
            input_count=input_count,
            finite_count=finite_count,
            auto_fit=auto_fit,
            coalesce=coalesce,
        )

    def enqueue_reset_scatter_camera(self) -> None:
        self.ensure_open()
        self.app.enqueue_reset_scatter_camera(self.id)

    def enqueue_set_scatter_view_direction(self, direction: str) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_view_direction(self.id, direction)

    def enqueue_set_scatter_point_style(self, style: str) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_point_style(self.id, style)

    def enqueue_set_scatter_point_size(self, size: float) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_point_size(self.id, size)

    def enqueue_set_scatter_grid_visible(self, visible: bool) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_grid_visible(self.id, visible)

    def enqueue_set_scatter_grid_planes(self, major: bool, minor: bool) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_grid_planes(self.id, major, minor)

    def enqueue_set_scatter_grid_options(self, sticky: bool, all_edges: bool) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_grid_options(self.id, sticky, all_edges)

    def enqueue_set_scatter_ticks(
        self,
        x: int | None = None,
        y: int | None = None,
        z: int | None = None,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_ticks(self.id, x, y, z)

    def enqueue_set_scatter_axes(self, x: str, y: str, z: str) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_axes(self.id, x, y, z)

    def enqueue_set_scatter_axis_visibility(self, x: bool, y: bool, z: bool) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_axis_visibility(self.id, x, y, z)

    def enqueue_set_scatter_background(self, r: float, g: float, b: float) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_background(self.id, r, g, b)

    def enqueue_set_scatter_legend(
        self,
        visible: bool,
        position: str,
        entries: list[tuple[str, float, float, float]],
        title: "str | None" = None,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_legend(self.id, visible, position, entries, title)

    def enqueue_set_scatter_scalar_bar(
        self,
        visible: bool,
        vmin: float,
        vmax: float,
        log_scale: bool,
        colormap: str,
        title: str | None,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_scalar_bar(
            self.id, visible, vmin, vmax, log_scale, colormap, title
        )

    def enqueue_set_scatter_orientation_axes(self, visible: bool) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_orientation_axes(self.id, visible)

    def enqueue_add_scatter_label(
        self,
        label_id: int,
        x: float,
        y: float,
        z: float,
        text: str,
        r: float,
        g: float,
        b: float,
        size: float,
        anchor: str,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_add_scatter_label(self.id, label_id, x, y, z, text, r, g, b, size, anchor)

    def enqueue_update_scatter_label(
        self,
        label_id: int,
        x: float | None,
        y: float | None,
        z: float | None,
        text: str | None,
        r: float | None,
        g: float | None,
        b: float | None,
        size: float | None,
        anchor: str | None,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_update_scatter_label(
            self.id, label_id, x, y, z, text, r, g, b, size, anchor
        )

    def enqueue_remove_scatter_label(self, label_id: int) -> None:
        self.ensure_open()
        self.app.enqueue_remove_scatter_label(self.id, label_id)

    def enqueue_set_scatter_label_visible(self, label_id: int, visible: bool) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_label_visible(self.id, label_id, visible)

    def enqueue_clear_scatter_labels(self) -> None:
        self.ensure_open()
        self.app.enqueue_clear_scatter_labels(self.id)

    def enqueue_add_scatter_lines(
        self,
        overlay_id: int,
        segments: list[list[float]],
        r: float,
        g: float,
        b: float,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_add_scatter_lines(self.id, overlay_id, segments, r, g, b)

    def enqueue_update_scatter_lines(
        self,
        overlay_id: int,
        segments: list[list[float]],
        r: float,
        g: float,
        b: float,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_update_scatter_lines(self.id, overlay_id, segments, r, g, b)

    def enqueue_add_scatter_box(
        self,
        overlay_id: int,
        xmin: float,
        xmax: float,
        ymin: float,
        ymax: float,
        zmin: float,
        zmax: float,
        r: float,
        g: float,
        b: float,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_add_scatter_box(
            self.id, overlay_id, xmin, xmax, ymin, ymax, zmin, zmax, r, g, b
        )

    def enqueue_remove_scatter_overlay(self, overlay_id: int) -> None:
        self.ensure_open()
        self.app.enqueue_remove_scatter_overlay(self.id, overlay_id)

    def enqueue_set_scatter_overlay_visible(self, overlay_id: int, visible: bool) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_overlay_visible(self.id, overlay_id, visible)

    def enqueue_clear_scatter_overlays(self) -> None:
        self.ensure_open()
        self.app.enqueue_clear_scatter_overlays(self.id)

    def enqueue_add_scatter_actor(
        self, actor_id: int, payload_b64: str, colormap: str, payload_format: str,
        hover_meta: str | None = None,
        tooltip_x: str | None = None, tooltip_y: str | None = None, tooltip_z: str | None = None,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_add_scatter_actor(
            self.id, actor_id, payload_b64, colormap, payload_format,
            hover_meta, tooltip_x, tooltip_y, tooltip_z,
        )

    def enqueue_add_scatter_actor_packed(
        self, actor_id: int, payload: bytes, colormap: str, payload_format: str,
        hover_meta: str | None = None,
        tooltip_x: str | None = None, tooltip_y: str | None = None, tooltip_z: str | None = None,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_add_scatter_actor_packed(
            self.id, actor_id, payload, colormap, payload_format,
            hover_meta, tooltip_x, tooltip_y, tooltip_z,
        )

    def enqueue_update_scatter_actor(
        self, actor_id: int, payload_b64: str, colormap: str, payload_format: str,
        tooltip_x: str | None = None, tooltip_y: str | None = None, tooltip_z: str | None = None,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_update_scatter_actor(
            self.id, actor_id, payload_b64, colormap, payload_format,
            tooltip_x, tooltip_y, tooltip_z,
        )

    def enqueue_update_scatter_actor_packed(
        self, actor_id: int, payload: bytes, colormap: str, payload_format: str,
        tooltip_x: str | None = None, tooltip_y: str | None = None, tooltip_z: str | None = None,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_update_scatter_actor_packed(
            self.id, actor_id, payload, colormap, payload_format,
            tooltip_x, tooltip_y, tooltip_z,
        )

    def enqueue_remove_scatter_actor(self, actor_id: int) -> None:
        self.ensure_open()
        self.app.enqueue_remove_scatter_actor(self.id, actor_id)

    def enqueue_set_scatter_actor_visible(self, actor_id: int, visible: bool) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_actor_visible(self.id, actor_id, visible)

    def enqueue_clear_scatter_actors(self) -> None:
        self.ensure_open()
        self.app.enqueue_clear_scatter_actors(self.id)

    def enqueue_clear_scatter_scene(self) -> None:
        self.ensure_open()
        self.app.enqueue_clear_scatter_scene(self.id)

    def enqueue_add_scatter_stream(self, actor_id: int, max_points: int, mode: str) -> None:
        self.ensure_open()
        self.app.enqueue_add_scatter_stream(self.id, actor_id, max_points, mode)

    def enqueue_stream_scatter_actor(
        self, actor_id: int, payload_b64: str, colormap: str, payload_format: str
    ) -> None:
        self.ensure_open()
        self.app.enqueue_stream_scatter_actor(self.id, actor_id, payload_b64, colormap, payload_format)

    def enqueue_stream_scatter_actor_packed(
        self, actor_id: int, payload: bytes, colormap: str, payload_format: str
    ) -> None:
        self.ensure_open()
        self.app.enqueue_stream_scatter_actor_packed(self.id, actor_id, payload, colormap, payload_format)

    def enqueue_clear_scatter_stream(self, actor_id: int) -> None:
        self.ensure_open()
        self.app.enqueue_clear_scatter_stream(self.id, actor_id)

    def enqueue_set_scatter_lod(self, enabled: bool, threshold: int, factor: int) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_lod(self.id, enabled, threshold, factor)

    def enqueue_set_scatter_auto_point_size(self, enabled: bool) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_auto_point_size(self.id, enabled)

    def enqueue_set_scatter_interactive_render_scale(self, scale: float) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_interactive_render_scale(self.id, scale)

    def enqueue_set_scatter_auto_quality(self, enabled: bool, target_fps: float) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_auto_quality(self.id, enabled, target_fps)

    def enqueue_set_scatter_picking_mode(self, mode: str) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_picking_mode(self.id, mode)

    def enqueue_set_scatter_hover_tooltip(self, enabled: bool) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_hover_tooltip(self.id, enabled)

    def enqueue_set_scatter_primary_hover_meta(self, meta: str) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_primary_hover_meta(self.id, meta)

    def enqueue_set_scatter_primary_hover_columns(
        self,
        columns_json: str,
        buffers: object,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_primary_hover_columns(self.id, columns_json, buffers)

    def enqueue_set_scatter_tooltip_axis_labels(self, x: str, y: str, z: str) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_tooltip_axis_labels(self.id, x, y, z)

    def enqueue_add_scatter_mesh(
        self, mesh_id: int, positions_b64: str, indices_b64: str,
        r: float, g: float, b: float, a: float, wireframe: bool,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_add_scatter_mesh(
            self.id, mesh_id, positions_b64, indices_b64, r, g, b, a, wireframe
        )

    def enqueue_update_scatter_mesh(
        self, mesh_id: int, positions_b64: str | None = None,
        indices_b64: str | None = None, r: float | None = None,
        g: float | None = None, b: float | None = None,
        a: float | None = None, wireframe: bool | None = None,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_update_scatter_mesh(
            self.id, mesh_id, positions_b64, indices_b64, r, g, b, a, wireframe
        )

    def enqueue_remove_scatter_mesh(self, mesh_id: int) -> None:
        self.ensure_open()
        self.app.enqueue_remove_scatter_mesh(self.id, mesh_id)

    def enqueue_set_scatter_mesh_visible(self, mesh_id: int, visible: bool) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_mesh_visible(self.id, mesh_id, visible)

    def enqueue_clear_scatter_meshes(self) -> None:
        self.ensure_open()
        self.app.enqueue_clear_scatter_meshes(self.id)

    def enqueue_fit_scatter_camera(self, bounds: list[float] | None = None) -> None:
        self.ensure_open()
        self.app.enqueue_fit_scatter_camera(self.id, bounds)

    def enqueue_set_scatter_parallel_projection(self, parallel: bool) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_parallel_projection(self.id, parallel)

    def enqueue_set_scatter_parallel_scale(self, half_w: float, half_h: float) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_parallel_scale(self.id, half_w, half_h)

    def enqueue_set_scatter_camera_state(
        self,
        target: list[float],
        distance: float,
        yaw: float,
        pitch: float,
        parallel: bool = False,
    ) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_camera_state(
            self.id, target, distance, yaw, pitch, parallel
        )

    def enqueue_set_table_data(self, table: object) -> None:
        self.ensure_open()
        self.app.enqueue_set_table_data(self.id, table)

    def enqueue_set_table_data_columns(self, table: object, columns: object) -> None:
        self.ensure_open()
        self.app.enqueue_set_table_data_columns(self.id, table, columns)

    def release_resource(self, resource_id: str) -> None:
        self.ensure_open()
        self.app.release_resource(resource_id)


class AppHandle:
    """Internal runtime handle used for live updates and background tasks."""

    def __init__(self) -> None:
        self._lock = RLock()
        self._tasks: OrderedDict[int, _ScheduledPythonTask] = OrderedDict()
        self._pending_task_keys: dict[object, int] = {}
        self._task_sequence = 0
        self._python_tasks_enqueued = 0
        self._python_tasks_executed = 0
        self._python_tasks_coalesced = 0
        self._python_task_queue_high_water = 0
        self._unkeyed_python_tasks_pending = 0
        self._unkeyed_python_task_queue_high_water = 0
        self._python_task_queue_growth_warnings = 0
        self._next_python_task_queue_warning = _PYTHON_TASK_QUEUE_WARNING_THRESHOLD
        self._pending_native: deque[tuple[str, tuple[object, ...]]] = deque()
        self._click_callbacks: dict[str, Callable[[], None]] = {}
        self._change_callbacks: dict[str, Callable[[object], None]] = {}
        self._native_sender: Any | None = None
        self._drain_requested = False
        self._toast_seq = 0
        self._closed = False
        self._python_task_timings: dict[str, dict[str, Any]] = {}
        self._python_task_drain_timing: dict[str, Any] = {}
        self._last_python_task: dict[str, Any] | None = None
        self._last_python_task_drain: dict[str, Any] | None = None
        self._startup_timings: dict[str, Any] = {}
        self._native_sends_requested = 0
        self._native_sends_direct = 0
        self._native_sends_queued_before_bind = 0
        self._native_sends_flushed_after_bind = 0
        self._native_send_errors = 0
        self._native_send_timing: dict[str, Any] = {}
        self._native_send_methods: dict[str, dict[str, Any]] = {}
        self._update_batch_local = local()
        self._update_batches_started = 0
        self._update_batches_completed = 0
        self._update_batches_nested = 0
        self._update_batch_packets = 0
        self._update_batch_barrier_flushes = 0
        self._update_batch_updates_collected = 0
        self._update_batch_updates_submitted = 0
        self._update_batch_duplicates_removed = 0
        self._update_batch_max_updates = 0
        self._update_batch_fallback_packets = 0

    @property
    def closed(self) -> bool:
        with self._lock:
            return self._closed

    def widget_handle(self, widget_id: str) -> LiveWidgetHandle:
        return LiveWidgetHandle(self, widget_id)

    def register_widget_callbacks(self, widget: object) -> None:
        click_callbacks, change_callbacks = _collect_runtime_callbacks(widget)
        with self._lock:
            self._click_callbacks.update(click_callbacks)
            self._change_callbacks.update(change_callbacks)

    def unregister_widget_callbacks(self, widget: object) -> None:
        widget_ids = _collect_widget_ids(widget)
        with self._lock:
            for widget_id in widget_ids:
                self._click_callbacks.pop(widget_id, None)
                self._change_callbacks.pop(widget_id, None)

    def call_soon_threadsafe(
        self,
        fn: Callable[[], None],
        *,
        coalesce_key: object | None = None,
        _diagnostics: bool = True,
    ) -> None:
        if not callable(fn):
            raise TypeError("call_soon_threadsafe expects a callable")
        if coalesce_key is not None:
            try:
                hash(coalesce_key)
            except TypeError:
                raise TypeError("coalesce_key must be hashable") from None
        collector = None
        try:
            if _diagnostics:
                collector = _diagnostics_collector()
        except Exception:
            collector = None
        queue_growth_warning: str | None = None
        with self._lock:
            if self._closed:
                raise RuntimeError("DragonGUI app handle is closed")
            self._task_sequence += 1
            scheduled = _ScheduledPythonTask(
                fn,
                None,
                _diagnostics,
                coalesce_key,
                self._task_sequence,
            )
            if coalesce_key is not None:
                replaced_sequence = self._pending_task_keys.get(coalesce_key)
                if replaced_sequence is not None and self._tasks.pop(replaced_sequence, None):
                    self._python_tasks_coalesced += 1
                self._pending_task_keys[coalesce_key] = scheduled.sequence
            else:
                self._unkeyed_python_tasks_pending += 1
                self._unkeyed_python_task_queue_high_water = max(
                    self._unkeyed_python_task_queue_high_water,
                    self._unkeyed_python_tasks_pending,
                )
                if (
                    self._unkeyed_python_tasks_pending
                    >= self._next_python_task_queue_warning
                ):
                    pending = self._unkeyed_python_tasks_pending
                    queue_growth_warning = (
                        "DragonGUI's unkeyed Python task backlog reached "
                        f"{pending} pending callbacks. Unkeyed "
                        "call_soon_threadsafe work is FIFO and lossless; use a stable "
                        "coalesce_key for replaceable latest-state snapshots, or reduce "
                        "the producer rate for events that must remain lossless."
                    )
                    self._python_task_queue_growth_warnings += 1
                    while self._next_python_task_queue_warning <= pending:
                        self._next_python_task_queue_warning *= 2
            self._tasks[scheduled.sequence] = scheduled
            self._python_tasks_enqueued += 1
            self._python_task_queue_high_water = max(
                self._python_task_queue_high_water,
                len(self._tasks),
            )
            sender = self._native_sender
            should_request_drain = sender is not None and not self._drain_requested
            if should_request_drain:
                self._drain_requested = True
        if collector is not None:
            try:
                scheduled.origin = collector.record_enqueue()
            except Exception:
                pass
        if should_request_drain:
            try:
                sender.enqueue_drain_python_tasks()
            except RuntimeError as exc:
                with self._lock:
                    closed = self._closed
                is_closed = getattr(sender, "is_closed", False)
                sender_closed = bool(is_closed() if callable(is_closed) else is_closed)
                if closed or sender_closed:
                    raise RuntimeError("DragonGUI app handle is closed") from None
                raise
        if queue_growth_warning is not None:
            warnings.warn(queue_growth_warning, RuntimeWarning, stacklevel=2)

    def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
        if self._collect_update_batch_prop(widget_id, prop, value):
            return
        self._send_or_queue_native("enqueue_set_prop", widget_id, prop, value)

    def enqueue_invalidate(self, widget_id: str, dirty: str) -> None:
        self._send_or_queue_native("enqueue_invalidate", widget_id, dirty)

    def enqueue_set_style(self, widget_id: str, style: object) -> None:
        self._send_or_queue_native("enqueue_set_style", widget_id, _style_json(style))

    def enqueue_replace_children(self, widget_id: str, children: object) -> None:
        self._send_or_queue_native("enqueue_replace_children", widget_id, _children_json(children))

    def enqueue_replace_node(self, widget_id: str, node: object) -> None:
        self._send_or_queue_native("enqueue_replace_node", widget_id, _node_json(node))

    def enqueue_update_extension_display_list(
        self, widget_id: str, display_list: object
    ) -> bool:
        method = "enqueue_update_extension_display_list"
        if not self._native_method_available(method):
            return False
        self._send_or_queue_native(method, widget_id, json.dumps(display_list))
        return True

    def enqueue_prewarm_scatter_widgets(self, count: int) -> None:
        if count <= 0 or not self._native_method_available("enqueue_prewarm_scatter_widgets"):
            return
        self._send_or_queue_native("enqueue_prewarm_scatter_widgets", int(count))

    def enqueue_set_scatter_points_packed(
        self,
        widget_id: str,
        xyz: bytes,
        *,
        pack_ms: float | None = None,
        enqueue_epoch_ms: float | None = None,
        colormap: str = "viridis",
        payload_format: str = "xyz_f32_v0",
        coalesce: bool = True,
        fit: bool = False,
        bounds_min: tuple[float, float, float] | None = None,
        bounds_max: tuple[float, float, float] | None = None,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_set_scatter_points_packed",
            widget_id,
            xyz,
            pack_ms,
            enqueue_epoch_ms,
            _scatter_colormap(colormap),
            payload_format,
            bool(coalesce),
            bool(fit),
            bounds_min,
            bounds_max,
        )

    def enqueue_set_line_plot_data_packed(
        self,
        widget_id: str,
        series: str,
        xy: bytes,
        *,
        label: str | None = None,
        color: object | None = None,
        line_width: float | None = None,
        line_style: str | None = None,
        show_grid: bool | None = None,
        auto_fit: bool | None = None,
        max_points: int | None = None,
        fit: bool = True,
        coalesce: bool = True,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_set_line_plot_data_packed",
            widget_id,
            series,
            xy,
            label,
            _line_plot_color_arg(color),
            None if line_width is None else float(line_width),
            line_style,
            show_grid,
            auto_fit,
            max_points,
            bool(fit),
            bool(coalesce),
        )

    def enqueue_append_line_plot_points_packed(
        self,
        widget_id: str,
        series: str,
        xy: bytes,
        *,
        max_points: int | None = None,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_append_line_plot_points_packed",
            widget_id,
            series,
            xy,
            max_points,
        )

    def enqueue_clear_line_plot_series(self, widget_id: str, series: str | None = None) -> None:
        self._send_or_queue_native("enqueue_clear_line_plot_series", widget_id, series)

    def enqueue_set_histogram_data(
        self,
        widget_id: str,
        edges: Sequence[object],
        counts: Sequence[object],
        *,
        input_count: int,
        finite_count: int,
        auto_fit: bool = True,
        coalesce: bool = True,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_set_histogram_data",
            widget_id,
            [float(value) for value in edges],
            [float(value) for value in counts],
            int(input_count),
            int(finite_count),
            bool(auto_fit),
            bool(coalesce),
        )

    def enqueue_reset_scatter_camera(self, widget_id: str) -> None:
        self._send_or_queue_native("enqueue_reset_scatter_camera", widget_id)

    def enqueue_set_scatter_view_direction(self, widget_id: str, direction: str) -> None:
        self._send_or_queue_native("enqueue_set_scatter_view_direction", widget_id, direction)

    def enqueue_set_scatter_point_style(self, widget_id: str, style: str) -> None:
        self._send_or_queue_native("enqueue_set_scatter_point_style", widget_id, style)

    def enqueue_set_scatter_point_size(self, widget_id: str, size: float) -> None:
        if not self._native_method_available("enqueue_set_scatter_point_size"):
            self.enqueue_set_style(widget_id, {"scatter_point_size": float(size)})
            return
        self._send_or_queue_native("enqueue_set_scatter_point_size", widget_id, float(size))

    def enqueue_set_scatter_grid_visible(self, widget_id: str, visible: bool) -> None:
        self._send_or_queue_native("enqueue_set_scatter_grid_visible", widget_id, visible)

    def enqueue_set_scatter_grid_planes(self, widget_id: str, major: bool, minor: bool) -> None:
        self._send_or_queue_native("enqueue_set_scatter_grid_planes", widget_id, major, minor)

    def enqueue_set_scatter_grid_options(
        self, widget_id: str, sticky: bool = True, all_edges: bool = False
    ) -> None:
        self._send_or_queue_native(
            "enqueue_set_scatter_grid_options", widget_id, sticky, all_edges
        )

    def enqueue_set_scatter_ticks(
        self,
        widget_id: str,
        x: int | None = None,
        y: int | None = None,
        z: int | None = None,
    ) -> None:
        self._send_or_queue_native("enqueue_set_scatter_ticks", widget_id, x, y, z)

    def enqueue_set_scatter_axes(self, widget_id: str, x: str, y: str, z: str) -> None:
        self._send_or_queue_native("enqueue_set_scatter_axes", widget_id, x, y, z)

    def enqueue_set_scatter_axis_visibility(
        self, widget_id: str, x: bool, y: bool, z: bool
    ) -> None:
        self._send_or_queue_native("enqueue_set_scatter_axis_visibility", widget_id, x, y, z)

    def enqueue_set_scatter_background(
        self, widget_id: str, r: float, g: float, b: float
    ) -> None:
        self._send_or_queue_native("enqueue_set_scatter_background", widget_id, r, g, b)

    def enqueue_set_scatter_legend(
        self,
        widget_id: str,
        visible: bool,
        position: str,
        entries: list[tuple[str, float, float, float]],
        title: "str | None" = None,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_set_scatter_legend", widget_id, visible, position, entries, title
        )

    def enqueue_set_scatter_scalar_bar(
        self,
        widget_id: str,
        visible: bool,
        vmin: float,
        vmax: float,
        log_scale: bool,
        colormap: str,
        title: str | None,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_set_scatter_scalar_bar",
            widget_id, visible, vmin, vmax, log_scale, colormap, title,
        )

    def enqueue_set_scatter_orientation_axes(self, widget_id: str, visible: bool) -> None:
        self._send_or_queue_native("enqueue_set_scatter_orientation_axes", widget_id, visible)

    def enqueue_add_scatter_label(
        self,
        widget_id: str,
        label_id: int,
        x: float,
        y: float,
        z: float,
        text: str,
        r: float,
        g: float,
        b: float,
        size: float,
        anchor: str,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_add_scatter_label", widget_id, label_id, x, y, z, text, r, g, b, size, anchor
        )

    def enqueue_update_scatter_label(
        self,
        widget_id: str,
        label_id: int,
        x: float | None,
        y: float | None,
        z: float | None,
        text: str | None,
        r: float | None,
        g: float | None,
        b: float | None,
        size: float | None,
        anchor: str | None,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_update_scatter_label",
            widget_id, label_id, x, y, z, text, r, g, b, size, anchor,
        )

    def enqueue_remove_scatter_label(self, widget_id: str, label_id: int) -> None:
        self._send_or_queue_native("enqueue_remove_scatter_label", widget_id, label_id)

    def enqueue_set_scatter_label_visible(
        self, widget_id: str, label_id: int, visible: bool
    ) -> None:
        self._send_or_queue_native(
            "enqueue_set_scatter_label_visible", widget_id, label_id, visible
        )

    def enqueue_clear_scatter_labels(self, widget_id: str) -> None:
        self._send_or_queue_native("enqueue_clear_scatter_labels", widget_id)

    def enqueue_add_scatter_lines(
        self,
        widget_id: str,
        overlay_id: int,
        segments: list[list[float]],
        r: float,
        g: float,
        b: float,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_add_scatter_lines", widget_id, overlay_id, segments, r, g, b
        )

    def enqueue_update_scatter_lines(
        self,
        widget_id: str,
        overlay_id: int,
        segments: list[list[float]],
        r: float,
        g: float,
        b: float,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_update_scatter_lines", widget_id, overlay_id, segments, r, g, b
        )

    def enqueue_add_scatter_box(
        self,
        widget_id: str,
        overlay_id: int,
        xmin: float,
        xmax: float,
        ymin: float,
        ymax: float,
        zmin: float,
        zmax: float,
        r: float,
        g: float,
        b: float,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_add_scatter_box",
            widget_id, overlay_id, xmin, xmax, ymin, ymax, zmin, zmax, r, g, b,
        )

    def enqueue_remove_scatter_overlay(self, widget_id: str, overlay_id: int) -> None:
        self._send_or_queue_native("enqueue_remove_scatter_overlay", widget_id, overlay_id)

    def enqueue_set_scatter_overlay_visible(
        self, widget_id: str, overlay_id: int, visible: bool
    ) -> None:
        self._send_or_queue_native(
            "enqueue_set_scatter_overlay_visible", widget_id, overlay_id, visible
        )

    def enqueue_clear_scatter_overlays(self, widget_id: str) -> None:
        self._send_or_queue_native("enqueue_clear_scatter_overlays", widget_id)

    def enqueue_add_scatter_actor(
        self, widget_id: str, actor_id: int, payload_b64: str, colormap: str, payload_format: str,
        hover_meta: str | None = None,
        tooltip_x: str | None = None, tooltip_y: str | None = None, tooltip_z: str | None = None,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_add_scatter_actor", widget_id, actor_id, payload_b64, colormap, payload_format,
            hover_meta, tooltip_x, tooltip_y, tooltip_z,
        )

    def enqueue_add_scatter_actor_packed(
        self, widget_id: str, actor_id: int, payload: bytes, colormap: str, payload_format: str,
        hover_meta: str | None = None,
        tooltip_x: str | None = None, tooltip_y: str | None = None, tooltip_z: str | None = None,
    ) -> None:
        if not self._native_method_available("enqueue_add_scatter_actor_packed"):
            self.enqueue_add_scatter_actor(
                widget_id, actor_id, base64.b64encode(payload).decode("ascii"),
                colormap, payload_format, hover_meta, tooltip_x, tooltip_y, tooltip_z,
            )
            return
        self._send_or_queue_native(
            "enqueue_add_scatter_actor_packed", widget_id, actor_id, payload, colormap, payload_format,
            hover_meta, tooltip_x, tooltip_y, tooltip_z,
        )

    def enqueue_update_scatter_actor(
        self, widget_id: str, actor_id: int, payload_b64: str, colormap: str, payload_format: str,
        tooltip_x: str | None = None, tooltip_y: str | None = None, tooltip_z: str | None = None,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_update_scatter_actor", widget_id, actor_id, payload_b64, colormap, payload_format,
            tooltip_x, tooltip_y, tooltip_z,
        )

    def enqueue_update_scatter_actor_packed(
        self, widget_id: str, actor_id: int, payload: bytes, colormap: str, payload_format: str,
        tooltip_x: str | None = None, tooltip_y: str | None = None, tooltip_z: str | None = None,
    ) -> None:
        if not self._native_method_available("enqueue_update_scatter_actor_packed"):
            self.enqueue_update_scatter_actor(
                widget_id, actor_id, base64.b64encode(payload).decode("ascii"),
                colormap, payload_format, tooltip_x, tooltip_y, tooltip_z,
            )
            return
        self._send_or_queue_native(
            "enqueue_update_scatter_actor_packed", widget_id, actor_id, payload, colormap, payload_format,
            tooltip_x, tooltip_y, tooltip_z,
        )

    def enqueue_remove_scatter_actor(self, widget_id: str, actor_id: int) -> None:
        self._send_or_queue_native("enqueue_remove_scatter_actor", widget_id, actor_id)

    def enqueue_set_scatter_actor_visible(
        self, widget_id: str, actor_id: int, visible: bool
    ) -> None:
        self._send_or_queue_native(
            "enqueue_set_scatter_actor_visible", widget_id, actor_id, visible
        )

    def enqueue_clear_scatter_actors(self, widget_id: str) -> None:
        self._send_or_queue_native("enqueue_clear_scatter_actors", widget_id)

    def enqueue_clear_scatter_scene(self, widget_id: str) -> None:
        self._send_or_queue_native("enqueue_clear_scatter_scene", widget_id)

    def enqueue_add_scatter_stream(
        self, widget_id: str, actor_id: int, max_points: int, mode: str
    ) -> None:
        self._send_or_queue_native(
            "enqueue_add_scatter_stream", widget_id, actor_id, max_points, mode
        )

    def enqueue_stream_scatter_actor(
        self, widget_id: str, actor_id: int, payload_b64: str, colormap: str, payload_format: str
    ) -> None:
        self._send_or_queue_native(
            "enqueue_stream_scatter_actor", widget_id, actor_id, payload_b64, colormap, payload_format
        )

    def enqueue_stream_scatter_actor_packed(
        self, widget_id: str, actor_id: int, payload: bytes, colormap: str, payload_format: str
    ) -> None:
        if not self._native_method_available("enqueue_stream_scatter_actor_packed"):
            self.enqueue_stream_scatter_actor(
                widget_id, actor_id, base64.b64encode(payload).decode("ascii"),
                colormap, payload_format,
            )
            return
        self._send_or_queue_native(
            "enqueue_stream_scatter_actor_packed", widget_id, actor_id, payload, colormap, payload_format
        )

    def enqueue_clear_scatter_stream(self, widget_id: str, actor_id: int) -> None:
        self._send_or_queue_native("enqueue_clear_scatter_stream", widget_id, actor_id)

    def enqueue_set_scatter_lod(
        self, widget_id: str, enabled: bool, threshold: int, factor: int
    ) -> None:
        self._send_or_queue_native("enqueue_set_scatter_lod", widget_id, enabled, threshold, factor)

    def enqueue_set_scatter_auto_point_size(self, widget_id: str, enabled: bool) -> None:
        if not self._native_method_available("enqueue_set_scatter_auto_point_size"):
            return
        self._send_or_queue_native("enqueue_set_scatter_auto_point_size", widget_id, enabled)

    def enqueue_set_scatter_interactive_render_scale(self, widget_id: str, scale: float) -> None:
        if not self._native_method_available("enqueue_set_scatter_interactive_render_scale"):
            return
        self._send_or_queue_native(
            "enqueue_set_scatter_interactive_render_scale", widget_id, float(scale)
        )

    def enqueue_set_scatter_auto_quality(
        self, widget_id: str, enabled: bool, target_fps: float
    ) -> None:
        if not self._native_method_available("enqueue_set_scatter_auto_quality"):
            return
        self._send_or_queue_native(
            "enqueue_set_scatter_auto_quality", widget_id, bool(enabled), float(target_fps)
        )

    def enqueue_set_scatter_picking_mode(self, widget_id: str, mode: str) -> None:
        self._send_or_queue_native("enqueue_set_scatter_picking_mode", widget_id, mode)

    def enqueue_set_scatter_hover_tooltip(self, widget_id: str, enabled: bool) -> None:
        self._send_or_queue_native("enqueue_set_scatter_hover_tooltip", widget_id, enabled)

    def enqueue_set_scatter_primary_hover_meta(self, widget_id: str, meta: str) -> None:
        self._send_or_queue_native("enqueue_set_scatter_primary_hover_meta", widget_id, meta)

    def enqueue_set_scatter_primary_hover_columns(
        self,
        widget_id: str,
        columns_json: str,
        buffers: object,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_set_scatter_primary_hover_columns",
            widget_id,
            columns_json,
            buffers,
        )

    def enqueue_set_scatter_tooltip_axis_labels(self, widget_id: str, x: str, y: str, z: str) -> None:
        self._send_or_queue_native("enqueue_set_scatter_tooltip_axis_labels", widget_id, x, y, z)

    def enqueue_add_scatter_mesh(
        self, widget_id: str, mesh_id: int, positions_b64: str, indices_b64: str,
        r: float, g: float, b: float, a: float, wireframe: bool,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_add_scatter_mesh", widget_id, mesh_id, positions_b64, indices_b64,
            r, g, b, a, wireframe,
        )

    def enqueue_update_scatter_mesh(
        self, widget_id: str, mesh_id: int, positions_b64=None, indices_b64=None,
        r=None, g=None, b=None, a=None, wireframe=None,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_update_scatter_mesh", widget_id, mesh_id, positions_b64,
            indices_b64, r, g, b, a, wireframe,
        )

    def enqueue_remove_scatter_mesh(self, widget_id: str, mesh_id: int) -> None:
        self._send_or_queue_native("enqueue_remove_scatter_mesh", widget_id, mesh_id)

    def enqueue_set_scatter_mesh_visible(
        self, widget_id: str, mesh_id: int, visible: bool
    ) -> None:
        self._send_or_queue_native("enqueue_set_scatter_mesh_visible", widget_id, mesh_id, visible)

    def enqueue_clear_scatter_meshes(self, widget_id: str) -> None:
        self._send_or_queue_native("enqueue_clear_scatter_meshes", widget_id)

    def enqueue_fit_scatter_camera(self, widget_id: str, bounds: list[float] | None = None) -> None:
        self._send_or_queue_native("enqueue_fit_scatter_camera", widget_id, bounds)

    def enqueue_set_scatter_parallel_projection(self, widget_id: str, parallel: bool) -> None:
        self._send_or_queue_native("enqueue_set_scatter_parallel_projection", widget_id, parallel)

    def enqueue_set_scatter_parallel_scale(
        self, widget_id: str, half_w: float, half_h: float
    ) -> None:
        self._send_or_queue_native(
            "enqueue_set_scatter_parallel_scale", widget_id, half_w, half_h
        )

    def scatter_screenshot(
        self, widget_id: str, timeout_ms: int = 10000
    ) -> "tuple[int, int, bytes] | None":
        with self._lock:
            sender = self._native_sender
        if sender is None:
            return None
        return sender.scatter_screenshot(widget_id, timeout_ms)

    def window_screenshot(self, timeout_ms: int = 10000) -> "tuple[int, int, bytes] | None":
        with self._lock:
            sender = self._native_sender
        if sender is None:
            return None
        if not hasattr(sender, "window_screenshot"):
            return None
        return sender.window_screenshot(timeout_ms)

    def enqueue_set_scatter_camera_state(
        self,
        widget_id: str,
        target: list[float],
        distance: float,
        yaw: float,
        pitch: float,
        parallel: bool = False,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_set_scatter_camera_state",
            widget_id, target, distance, yaw, pitch, parallel,
        )

    def enqueue_set_table_data(self, widget_id: str, table: object) -> None:
        self._send_or_queue_native("enqueue_set_table_data", widget_id, _table_json(table))

    def enqueue_set_table_data_columns(self, widget_id: str, table: object, columns: object) -> None:
        metadata, buffers = _table_column_payload(columns)
        self._send_or_queue_native(
            "enqueue_set_table_data_columns",
            widget_id,
            _table_json(table),
            json.dumps(metadata, separators=(",", ":"), sort_keys=True),
            buffers,
        )

    def enqueue_set_buffer_resource(
        self,
        resource_id: str,
        data: object,
        *,
        kind: str = "bytes",
        owner: object = None,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_set_buffer_resource",
            _resource_id(resource_id),
            _resource_kind(kind),
            _byte_view(data, "buffer resource data"),
            _resource_owner_id(owner),
        )

    def release_resource(self, resource_id: str) -> None:
        self._send_or_queue_native("enqueue_release_resource", _resource_id(resource_id))

    def enqueue_set_stylesheet(self, css: str) -> None:
        self._send_or_queue_native("enqueue_set_stylesheet", "user", _stylesheet_css(css))

    def enqueue_set_named_stylesheet(self, stylesheet_id: str, css: str) -> None:
        self._send_or_queue_native(
            "enqueue_set_named_stylesheet",
            "user",
            _stylesheet_id(stylesheet_id),
            _stylesheet_css(css),
        )

    def enqueue_remove_stylesheet(self, stylesheet_id: str) -> None:
        self._send_or_queue_native(
            "enqueue_remove_stylesheet", "user", _stylesheet_id(stylesheet_id)
        )

    def enqueue_set_theme(self, theme: Mapping[str, object]) -> None:
        if not isinstance(theme, Mapping):
            raise TypeError("theme must be a mapping")
        self._send_or_queue_native(
            "enqueue_set_theme",
            json.dumps(dict(theme), separators=(",", ":"), sort_keys=True),
        )

    def enqueue_set_icon_theme(self, theme: Mapping[str, object]) -> None:
        if not isinstance(theme, Mapping):
            raise TypeError("icon theme must be a mapping")
        self._send_or_queue_native(
            "enqueue_set_icon_theme",
            json.dumps(dict(theme), separators=(",", ":"), sort_keys=True),
        )

    def enqueue_clear_stylesheets(self) -> None:
        self._send_or_queue_native("enqueue_clear_stylesheets", "user")

    def toast(
        self,
        message: object,
        *,
        level: str = "info",
        duration: int | float | None = 3000,
        opacity: int | float | None = None,
        radius: int | float | None = None,
        padding: int | float | None = None,
        position: str | None = None,
    ) -> ToastHandle:
        with self._lock:
            if self._closed:
                raise RuntimeError("DragonGUI app handle is closed")
            self._toast_seq += 1
            toast_id = f"toast-{self._toast_seq}"
        self.enqueue_show_toast(
            toast_id,
            message,
            level=level,
            duration=duration,
            opacity=opacity,
            radius=radius,
            padding=padding,
            position=position,
        )
        return ToastHandle(self, toast_id)

    def enqueue_show_toast(
        self,
        toast_id: str,
        message: object,
        *,
        level: str = "info",
        duration: int | float | None = 3000,
        opacity: int | float | None = None,
        radius: int | float | None = None,
        padding: int | float | None = None,
        position: str | None = None,
    ) -> None:
        self._send_or_queue_native(
            "enqueue_show_toast",
            _toast_id(toast_id),
            _toast_message(message),
            _toast_level(level),
            _toast_duration_ms(duration),
            _toast_opacity(opacity),
            _toast_non_negative("radius", radius),
            _toast_non_negative("padding", padding),
            _toast_position(position),
        )

    def enqueue_dismiss_toast(self, toast_id: str) -> None:
        self._send_or_queue_native("enqueue_dismiss_toast", _toast_id(toast_id))

    def request_redraw(self) -> None:
        """Request one native redraw without changing widget state."""
        self._send_or_queue_native("enqueue_request_redraw")

    def request_window_resize(self, width: int, height: int) -> None:
        """Request a logical window size on the native event-loop thread."""
        width = int(width)
        height = int(height)
        if width <= 0 or height <= 0:
            raise ValueError("window resize dimensions must be positive")
        self._send_or_queue_native("request_window_resize", width, height)

    def request_exit(self) -> None:
        """Request the native event loop to exit."""
        self._send_or_queue_native("enqueue_request_exit")

    def _set_startup_timings(self, timings: Mapping[str, Any]) -> None:
        with self._lock:
            self._startup_timings = {str(key): value for key, value in timings.items()}

    def _python_debug_snapshot(
        self,
        queued_tasks: int | None = None,
        pending_native: int | None = None,
    ) -> dict[str, Any]:
        with self._lock:
            if queued_tasks is None:
                queued_tasks = len(self._tasks)
            if pending_native is None:
                pending_native = len(self._pending_native)
            return {
                "queued_tasks": queued_tasks,
                "pending_native_commands": pending_native,
                "drain_requested": self._drain_requested,
                "tasks_enqueued": self._python_tasks_enqueued,
                "tasks_executed": self._python_tasks_executed,
                "tasks_coalesced": self._python_tasks_coalesced,
                "task_queue_high_water": self._python_task_queue_high_water,
                "unkeyed_tasks_pending": self._unkeyed_python_tasks_pending,
                "unkeyed_task_queue_high_water": (
                    self._unkeyed_python_task_queue_high_water
                ),
                "task_queue_growth_warnings": self._python_task_queue_growth_warnings,
                "next_task_queue_warning_at": self._next_python_task_queue_warning,
                "task_drain_budget_ms": _PYTHON_TASK_DRAIN_BUDGET_MS,
                "task_drain_timing": dict(self._python_task_drain_timing),
                "last_task_drain": (
                    dict(self._last_python_task_drain)
                    if self._last_python_task_drain is not None
                    else None
                ),
                "task_timings": {
                    name: dict(stats) for name, stats in self._python_task_timings.items()
                },
                "last_task": (
                    dict(self._last_python_task) if self._last_python_task is not None else None
                ),
                "native_sends": {
                    "scope": "live commands sent through AppHandle._send_or_queue_native",
                    "requested": self._native_sends_requested,
                    "direct": self._native_sends_direct,
                    "queued_before_bind": self._native_sends_queued_before_bind,
                    "flushed_after_bind": self._native_sends_flushed_after_bind,
                    "errors": self._native_send_errors,
                    "timing": dict(self._native_send_timing),
                    "methods": {
                        name: {
                            key: (dict(value) if isinstance(value, dict) else value)
                            for key, value in stats.items()
                        }
                        for name, stats in self._native_send_methods.items()
                    },
                    "batches": {
                        "started": self._update_batches_started,
                        "completed": self._update_batches_completed,
                        "nested": self._update_batches_nested,
                        "packets": self._update_batch_packets,
                        "barrier_flushes": self._update_batch_barrier_flushes,
                        "updates_collected": self._update_batch_updates_collected,
                        "updates_submitted": self._update_batch_updates_submitted,
                        "duplicates_removed": self._update_batch_duplicates_removed,
                        "max_updates": self._update_batch_max_updates,
                        "fallback_packets": self._update_batch_fallback_packets,
                    },
                },
                "startup": dict(self._startup_timings),
            }

    def debug_snapshot(self, timeout_ms: int = 1000) -> dict[str, Any]:
        """Return a JSON-safe snapshot of the live native runtime."""
        with self._lock:
            if self._closed:
                raise RuntimeError("DragonGUI app handle is closed")
            sender = self._native_sender
            queued_tasks = len(self._tasks)
            pending_native = len(self._pending_native)
        python_snapshot = self._python_debug_snapshot(queued_tasks, pending_native)
        if sender is None:
            return {
                "schema": 1,
                "runtime": {
                    "native_bound": False,
                    "queued_python_tasks": queued_tasks,
                    "pending_native_commands": pending_native,
                    "python": python_snapshot,
                    "closed": False,
                },
            }
        snapshot_json = sender.debug_snapshot(timeout_ms)
        snapshot = json.loads(snapshot_json)
        if not isinstance(snapshot, dict):
            raise RuntimeError("DragonGUI native debug snapshot was not a JSON object")
        runtime = snapshot.setdefault("runtime", {})
        if isinstance(runtime, dict):
            runtime["python"] = python_snapshot
        else:
            snapshot["python_runtime"] = python_snapshot
        return snapshot

    def latency_probe(self, timeout_ms: int = 1000) -> bool | None:
        """Round-trip a lightweight native ordering barrier for diagnostics."""
        with self._lock:
            if self._closed:
                raise RuntimeError("DragonGUI app handle is closed")
            sender = self._native_sender
        if sender is None or not hasattr(sender, "latency_probe"):
            return None
        sender.latency_probe(timeout_ms)
        return True

    def apply_patch(self, patch: object) -> None:
        from .vdom import Patch

        if not isinstance(patch, Patch):
            raise TypeError("apply_patch expects a DragonGUI VDOM Patch")
        if patch.kind == Patch.SET_PROP:
            if patch.node_id is None or patch.prop is None:
                raise ValueError("set_prop patches require node_id and prop")
            if patch.prop == "table" and isinstance(patch.value, Mapping):
                self.enqueue_set_table_data(patch.node_id, patch.value)
                return
            if patch.prop == "scatter" and isinstance(patch.value, Mapping):
                data_b64 = patch.value.get("data_b64")
                colormap = patch.value.get("colormap", "viridis")
                data_format = patch.value.get("data_format", "xyz_f32_v0")
                if isinstance(data_b64, str):
                    # data_b64 == "" means zero points — send an empty payload to clear native state.
                    payload = base64.b64decode(data_b64) if data_b64 else b""
                    self.enqueue_set_scatter_points_packed(
                        patch.node_id,
                        payload,
                        pack_ms=0.0,
                        enqueue_epoch_ms=time.time() * 1000.0,
                        colormap=colormap if isinstance(colormap, str) else "viridis",
                        payload_format=data_format if isinstance(data_format, str) else "xyz_f32_v0",
                    )
                # Sync grid chrome if included in the props diff.
                nid = patch.node_id
                v = patch.value
                if "grid_visible" in v:
                    self.enqueue_set_scatter_grid_visible(nid, bool(v["grid_visible"]))
                if "major_planes" in v or "minor_planes" in v:
                    self.enqueue_set_scatter_grid_planes(
                        nid,
                        bool(v.get("major_planes", False)),
                        bool(v.get("minor_planes", False)),
                    )
                if "grid_sticky" in v or "grid_all_edges" in v:
                    self.enqueue_set_scatter_grid_options(
                        nid,
                        bool(v.get("grid_sticky", True)),
                        bool(v.get("grid_all_edges", False)),
                    )
                if any(k in v for k in ("tick_x", "tick_y", "tick_z")):
                    self.enqueue_set_scatter_ticks(
                        nid,
                        v.get("tick_x"),
                        v.get("tick_y"),
                        v.get("tick_z"),
                    )
                if any(k in v for k in ("axis_x", "axis_y", "axis_z")):
                    self.enqueue_set_scatter_axes(
                        nid,
                        str(v.get("axis_x", "X")),
                        str(v.get("axis_y", "Y")),
                        str(v.get("axis_z", "Z")),
                    )
                if any(k in v for k in ("axis_vis_x", "axis_vis_y", "axis_vis_z")):
                    self.enqueue_set_scatter_axis_visibility(
                        nid,
                        bool(v.get("axis_vis_x", True)),
                        bool(v.get("axis_vis_y", True)),
                        bool(v.get("axis_vis_z", True)),
                    )
                if "background" in v:
                    bg = v["background"]
                    if isinstance(bg, (list, tuple)) and len(bg) >= 3:
                        self.enqueue_set_scatter_background(nid, float(bg[0]), float(bg[1]), float(bg[2]))
                if any(k in v for k in ("legend_visible", "legend_position", "legend_entries", "legend_title")):
                    entries_raw = v.get("legend_entries", [])
                    entries = [
                        (e["label"], float(e["color"][0]), float(e["color"][1]), float(e["color"][2]))
                        for e in entries_raw
                        if isinstance(e, dict)
                    ]
                    self.enqueue_set_scatter_legend(
                        nid,
                        bool(v.get("legend_visible", False)),
                        str(v.get("legend_position", "top-right")),
                        entries,
                        v.get("legend_title"),
                    )
                if any(
                    k in v
                    for k in (
                        "scalar_bar_visible", "scalar_bar_vmin", "scalar_bar_vmax",
                        "scalar_bar_log_scale", "scalar_bar_colormap", "scalar_bar_title",
                    )
                ):
                    self.enqueue_set_scatter_scalar_bar(
                        nid,
                        bool(v.get("scalar_bar_visible", False)),
                        float(v.get("scalar_bar_vmin", 0.0)),
                        float(v.get("scalar_bar_vmax", 1.0)),
                        bool(v.get("scalar_bar_log_scale", False)),
                        str(v.get("scalar_bar_colormap", "viridis")),
                        v.get("scalar_bar_title"),
                    )
                if "orientation_axes_visible" in v:
                    self.enqueue_set_scatter_orientation_axes(
                        nid, bool(v["orientation_axes_visible"])
                    )
                return
            if patch.prop == "line_plot" and isinstance(patch.value, Mapping):
                v = patch.value
                line_width = v.get("line_width")
                show_grid = v.get("show_grid")
                auto_fit = v.get("auto_fit")
                max_points = v.get("max_points")
                for prop_name in (
                    "x_label",
                    "y_label",
                    "show_grid",
                    "show_axes",
                    "show_ticks",
                    "show_toolbar",
                    "show_legend",
                    "legend_position",
                    "interaction",
                    "tick_count",
                    "auto_fit",
                    "line_width",
                    "window_size",
                ):
                    if prop_name in v:
                        self.enqueue_set_prop(patch.node_id, prop_name, v[prop_name])
                series_items = v.get("series", [])
                if isinstance(series_items, list):
                    for item in series_items:
                        if not isinstance(item, Mapping):
                            continue
                        data_b64 = item.get("data_b64")
                        if not isinstance(data_b64, str):
                            continue
                        label = str(item.get("label") or "series")
                        self.enqueue_set_line_plot_data_packed(
                            patch.node_id,
                            label,
                            base64.b64decode(data_b64) if data_b64 else b"",
                            label=label,
                            color=item.get("color"),
                            line_width=float(line_width) if line_width is not None else None,
                            line_style=(
                                str(item.get("line_style")) if item.get("line_style") is not None else None
                            ),
                            show_grid=bool(show_grid) if show_grid is not None else None,
                            auto_fit=bool(auto_fit) if auto_fit is not None else None,
                            max_points=int(max_points) if max_points is not None else None,
                            fit=True,
                        )
                return
            self.enqueue_set_prop(patch.node_id, patch.prop, patch.value)
            return
        if patch.kind == Patch.SET_STYLE:
            if patch.node_id is None or patch.style is None:
                raise ValueError("set_style patches require node_id and style")
            self.enqueue_set_style(patch.node_id, patch.style)
            return
        if patch.kind == Patch.REPLACE_CHILDREN:
            if patch.node_id is None:
                raise ValueError("replace_children patches require node_id")
            self.enqueue_replace_children(
                patch.node_id,
                [child.to_dict() for child in patch.children],
            )
            return
        if patch.kind == Patch.REPLACE_NODE:
            if patch.node_id is None or patch.node is None:
                raise ValueError("replace_node patches require node_id and node")
            self.enqueue_replace_node(patch.node_id, patch.node.to_dict())
            return
        raise ValueError(f"unknown VDOM patch kind: {patch.kind}")

    def apply_patches(self, patches: Iterable[object]) -> None:
        for patch in patches:
            self.apply_patch(patch)

    def update_batch(self) -> _UpdateBatchContext:
        """Return a thread-local, nestable live-property batching context."""
        return _UpdateBatchContext(self)

    def _update_batch_state(self) -> dict[str, Any] | None:
        return getattr(self._update_batch_local, "state", None)

    def _begin_update_batch(self) -> None:
        with self._lock:
            if self._closed:
                raise RuntimeError("DragonGUI app handle is closed")
        state = self._update_batch_state()
        if state is None:
            state = {"depth": 0, "updates": OrderedDict()}
            self._update_batch_local.state = state
        state["depth"] += 1
        with self._lock:
            self._update_batches_started += 1
            if state["depth"] > 1:
                self._update_batches_nested += 1

    def _end_update_batch(self) -> None:
        state = self._update_batch_state()
        if state is None or state["depth"] <= 0:
            raise RuntimeError("DragonGUI update batch is not active")
        state["depth"] -= 1
        if state["depth"] == 0:
            try:
                self._flush_update_batch(barrier=False)
            finally:
                del self._update_batch_local.state
                with self._lock:
                    self._update_batches_completed += 1
            return
        with self._lock:
            self._update_batches_completed += 1

    def _collect_update_batch_prop(self, widget_id: str, prop: str, value: object) -> bool:
        state = self._update_batch_state()
        if state is None or state["depth"] <= 0:
            return False
        updates: OrderedDict[tuple[str, str], tuple[str, str, object]] = state["updates"]
        key = (widget_id, prop)
        duplicate = key in updates
        if duplicate:
            updates.pop(key)
        updates[key] = (widget_id, prop, value)
        with self._lock:
            self._update_batch_updates_collected += 1
            if duplicate:
                self._update_batch_duplicates_removed += 1
            self._update_batch_max_updates = max(self._update_batch_max_updates, len(updates))
        return True

    def _flush_update_batch(self, *, barrier: bool) -> None:
        state = self._update_batch_state()
        if state is None:
            return
        updates_map: OrderedDict[tuple[str, str], tuple[str, str, object]] = state["updates"]
        if not updates_map:
            return
        updates = list(updates_map.values())
        updates_map.clear()
        with self._lock:
            sender = self._native_sender
        native_packet_available = sender is None or hasattr(sender, "enqueue_set_props")
        with self._lock:
            self._update_batch_packets += 1
            self._update_batch_updates_submitted += len(updates)
            if barrier:
                self._update_batch_barrier_flushes += 1
            if not native_packet_available:
                self._update_batch_fallback_packets += 1
        if native_packet_available:
            self._send_or_queue_native("enqueue_set_props", updates)
            return
        for widget_id, prop, value in updates:
            self._send_or_queue_native("enqueue_set_prop", widget_id, prop, value)

    def _send_or_queue_native(self, method: str, *args: object) -> None:
        if method != "enqueue_set_props":
            self._flush_update_batch(barrier=True)
        with self._lock:
            if self._closed:
                raise RuntimeError("DragonGUI app handle is closed")
            self._native_sends_requested += 1
            method_stats = self._native_send_methods.setdefault(method, {})
            method_stats["requested"] = int(method_stats.get("requested", 0)) + 1
            sender = self._native_sender
            if sender is None:
                self._pending_native.append((method, args))
                self._native_sends_queued_before_bind += 1
                method_stats["queued_before_bind"] = (
                    int(method_stats.get("queued_before_bind", 0)) + 1
                )
                return
        send_t0 = time.perf_counter()
        try:
            getattr(sender, method)(*args)
        except RuntimeError as exc:
            with self._lock:
                closed = self._closed
                self._native_send_errors += 1
                method_stats = self._native_send_methods.setdefault(method, {})
                method_stats["errors"] = int(method_stats.get("errors", 0)) + 1
            is_closed = getattr(sender, "is_closed", False)
            sender_closed = bool(is_closed() if callable(is_closed) else is_closed)
            if closed or sender_closed:
                raise RuntimeError("DragonGUI app handle is closed") from exc
            raise
        finally:
            send_ms = (time.perf_counter() - send_t0) * 1000.0
            with self._lock:
                _record_timing_stat(self._native_send_timing, send_ms)
                method_stats = self._native_send_methods.setdefault(method, {})
                timing = method_stats.setdefault("timing", {})
                _record_timing_stat(timing, send_ms)
        with self._lock:
            self._native_sends_direct += 1
            method_stats = self._native_send_methods.setdefault(method, {})
            method_stats["direct"] = int(method_stats.get("direct", 0)) + 1

    def _native_method_available(self, method: str) -> bool:
        with self._lock:
            sender = self._native_sender
            if sender is None:
                return True
            return hasattr(sender, method)

    def _bind_native_sender(self, sender: Any) -> None:
        with self._lock:
            if self._closed:
                if hasattr(sender, "close"):
                    sender.close()
                raise RuntimeError("cannot bind native sender to a closed DragonGUI app handle")
            self._native_sender = sender
            has_tasks = bool(self._tasks)
            should_request_drain = has_tasks and not self._drain_requested
            if should_request_drain:
                self._drain_requested = True
            pending = list(self._pending_native)
            self._pending_native.clear()
        for method, args in pending:
            send_t0 = time.perf_counter()
            try:
                getattr(sender, method)(*args)
            except RuntimeError:
                with self._lock:
                    self._native_send_errors += 1
                    method_stats = self._native_send_methods.setdefault(method, {})
                    method_stats["errors"] = int(method_stats.get("errors", 0)) + 1
                raise
            finally:
                send_ms = (time.perf_counter() - send_t0) * 1000.0
                with self._lock:
                    _record_timing_stat(self._native_send_timing, send_ms)
                    method_stats = self._native_send_methods.setdefault(method, {})
                    timing = method_stats.setdefault("timing", {})
                    _record_timing_stat(timing, send_ms)
            with self._lock:
                self._native_sends_flushed_after_bind += 1
                method_stats = self._native_send_methods.setdefault(method, {})
                method_stats["flushed_after_bind"] = (
                    int(method_stats.get("flushed_after_bind", 0)) + 1
                )
        if should_request_drain:
            sender.enqueue_drain_python_tasks()

    def _drain_python_tasks(self) -> None:
        processed = 0
        drain_t0 = time.perf_counter()
        try:
            while processed < _MAX_PYTHON_TASKS_PER_DRAIN:
                with self._lock:
                    if not self._tasks:
                        self._drain_requested = False
                        return
                    _, scheduled = self._tasks.popitem(last=False)
                    if scheduled.coalesce_key is not None:
                        pending_sequence = self._pending_task_keys.get(scheduled.coalesce_key)
                        if pending_sequence == scheduled.sequence:
                            self._pending_task_keys.pop(scheduled.coalesce_key, None)
                    else:
                        self._unkeyed_python_tasks_pending -= 1
                task = scheduled.fn
                task_label = _callable_label(task)
                task_t0 = time.perf_counter()
                outcome = "ok"
                try:
                    task()
                except Exception as _exc:  # pragma: no cover - diagnostic path
                    outcome = "error"
                    traceback.print_exc()
                    if scheduled.diagnostics:
                        try:
                            _record_task_failure(task, _exc, scheduled.origin)
                        except Exception:
                            pass
                finally:
                    task_ms = (time.perf_counter() - task_t0) * 1000.0
                    with self._lock:
                        stats = self._python_task_timings.setdefault(task_label, {})
                        _record_timing_stat(stats, task_ms)
                        self._last_python_task = {
                            "name": task_label,
                            "elapsed_ms": task_ms,
                            "outcome": outcome,
                        }
                        self._python_tasks_executed += 1
                processed += 1
                if (time.perf_counter() - drain_t0) * 1000.0 >= _PYTHON_TASK_DRAIN_BUDGET_MS:
                    break

            with self._lock:
                sender = self._native_sender if self._tasks and not self._closed else None
                self._drain_requested = sender is not None
            if sender is not None:
                try:
                    sender.enqueue_drain_python_tasks()
                except RuntimeError:  # pragma: no cover - close race diagnostic path
                    with self._lock:
                        closed = self._closed
                    is_closed = getattr(sender, "is_closed", False)
                    sender_closed = bool(is_closed() if callable(is_closed) else is_closed)
                    if not (closed or sender_closed):
                        traceback.print_exc()
        finally:
            drain_ms = (time.perf_counter() - drain_t0) * 1000.0
            with self._lock:
                _record_timing_stat(self._python_task_drain_timing, drain_ms)
                self._last_python_task_drain = {
                    "processed": processed,
                    "elapsed_ms": drain_ms,
                }

    def _invoke_click_callback(self, widget_id: str) -> bool:
        with self._lock:
            callback = self._click_callbacks.get(widget_id)
        if callback is None:
            return False
        try:
            callback()
        except Exception:  # pragma: no cover - diagnostic path
            traceback.print_exc()
        return True

    def _invoke_change_callback(self, widget_id: str, value: object) -> bool:
        with self._lock:
            callback = self._change_callbacks.get(widget_id)
        if callback is None:
            return False
        try:
            callback(value)
        except Exception:  # pragma: no cover - diagnostic path
            traceback.print_exc()
        return True

    def _close(self) -> None:
        with self._lock:
            if self._closed:
                return
            self._closed = True
            sender = self._native_sender
            self._native_sender = None
            self._drain_requested = False
            self._tasks.clear()
            self._pending_task_keys.clear()
            self._unkeyed_python_tasks_pending = 0
            self._pending_native.clear()
            self._click_callbacks.clear()
            self._change_callbacks.clear()
        if sender is not None and hasattr(sender, "close"):
            sender.close()


def _style_json(style: object) -> str:
    if not isinstance(style, Mapping):
        raise TypeError("style patch must be a mapping")
    return json.dumps(dict(style), separators=(",", ":"), sort_keys=True)


def _children_json(children: object) -> str:
    if not isinstance(children, list):
        raise TypeError("replacement children must be a list")
    return json.dumps(children, separators=(",", ":"), sort_keys=True)


def _node_json(node: object) -> str:
    if not isinstance(node, Mapping):
        raise TypeError("replacement node must be a mapping")
    return json.dumps(dict(node), separators=(",", ":"), sort_keys=True)


def _table_json(table: object) -> str:
    if not isinstance(table, Mapping):
        raise TypeError("table update must be a mapping")
    return json.dumps(dict(table), separators=(",", ":"), sort_keys=True)


def _stylesheet_css(css: object) -> str:
    if not isinstance(css, str):
        raise TypeError("css must be a string")
    if not css.strip():
        raise ValueError("css must be a non-empty string")
    return css


def _stylesheet_id(value: object) -> str:
    if not isinstance(value, str):
        raise TypeError("stylesheet_id must be a string")
    value = value.strip()
    if not value:
        raise ValueError("stylesheet_id must be a non-empty string")
    return value


def _toast_id(value: object) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValueError("toast id must be a non-empty string")
    return value


def _toast_message(value: object) -> str:
    text = str(value)
    if not text.strip():
        raise ValueError("toast message must be a non-empty string")
    return text


def _toast_level(value: object) -> str:
    if not isinstance(value, str):
        raise TypeError("toast level must be a string")
    level = value.strip().lower()
    if level not in _TOAST_LEVELS:
        allowed = ", ".join(sorted(_TOAST_LEVELS))
        raise ValueError(f"toast level must be one of: {allowed}")
    return level


def _toast_duration_ms(value: object) -> int | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise TypeError("toast duration must be a number of milliseconds or None")
    duration = float(value)
    if not math.isfinite(duration) or duration <= 0:
        raise ValueError("toast duration must be greater than zero milliseconds")
    return int(round(duration))


def _toast_opacity(value: object) -> float | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise TypeError("toast opacity must be a number between 0.0 and 1.0")
    opacity = float(value)
    if not math.isfinite(opacity) or opacity < 0.0 or opacity > 1.0:
        raise ValueError("toast opacity must be between 0.0 and 1.0")
    return opacity


def _toast_non_negative(name: str, value: object) -> float | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise TypeError(f"toast {name} must be a non-negative number")
    number = float(value)
    if not math.isfinite(number) or number < 0.0:
        raise ValueError(f"toast {name} must be a non-negative number")
    return number


def _toast_position(value: object) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str):
        raise TypeError("toast position must be a string")
    position = value.strip().lower().replace("_", "-")
    if position not in _TOAST_POSITIONS:
        allowed = ", ".join(sorted(_TOAST_POSITIONS))
        raise ValueError(f"toast position must be one of: {allowed}")
    return position


def _resource_id(value: object) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError("resource id must be a non-empty string")
    return value


def _resource_kind(value: object) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError("resource kind must be a non-empty string")
    return value


def _resource_owner_id(value: object) -> str | None:
    if value is None:
        return None
    if isinstance(value, str):
        if value:
            return value
        raise ValueError("resource owner id must be a non-empty string")
    owner_id = getattr(value, "id", None)
    if isinstance(owner_id, str) and owner_id:
        return owner_id
    raise ValueError("resource owner must be a widget, widget id string, or None")


def _scatter_colormap(value: object) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValueError("scatter colormap must be a non-empty string")
    return value.strip().lower()


def _line_plot_color_arg(value: object | None) -> str | None:
    if value is None:
        return None
    if isinstance(value, str):
        return value
    if isinstance(value, (list, tuple)) and len(value) in {3, 4}:
        return ",".join(str(float(channel)) for channel in value)
    return str(value)


def _byte_view(data: object, context: str) -> memoryview:
    if not isinstance(data, (bytes, bytearray, memoryview)):
        try:
            view = memoryview(data)  # type: ignore[arg-type]
        except TypeError as exc:
            raise TypeError(f"{context} must support the Python buffer protocol") from exc
    else:
        view = memoryview(data)
    try:
        return view.cast("B")
    except (TypeError, ValueError):
        return memoryview(bytes(view))


def _table_column_payload(columns: object) -> tuple[list[dict[str, object]], list[memoryview]]:
    if not isinstance(columns, list):
        raise TypeError("table column buffers must be a list")
    metadata: list[dict[str, object]] = []
    buffers: list[memoryview] = []
    for column in columns:
        if not isinstance(column, Mapping):
            raise TypeError("table column buffer entries must be mappings")
        name = column.get("name")
        dtype = column.get("dtype")
        data = column.get("data")
        if not isinstance(name, str) or not name:
            raise ValueError("table column buffer entries require a non-empty name")
        if not isinstance(dtype, str) or not dtype:
            raise ValueError("table column buffer entries require a non-empty dtype")
        if not isinstance(data, (bytes, bytearray, memoryview)):
            try:
                memoryview(data)  # type: ignore[arg-type]
            except TypeError as exc:
                raise TypeError("table column buffer data must support the Python buffer protocol") from exc
        metadata.append({"name": name, "dtype": dtype})
        buffers.append(_byte_view(data, "table column buffer data"))
    return metadata, buffers


def _collect_runtime_callbacks(
    widget: object,
) -> tuple[dict[str, Callable[[], None]], dict[str, Callable[[object], None]]]:
    from .node_graph import NodeGraph
    from .widgets import BarChart, BarChartBar, Button, Checkbox, CodeEditor, Collapsible, Container, DataFrameTable, DateInput, DateTimeInput, DragDropPayload, DragNumber, DropTarget, Dropdown, ExtensionWidget, Heatmap, HeatmapCell, MenuItem, NumberInput, Pages, PaintKeyEvent, PaintPointerEvent, RadioButton, RangeSlider, Scatter3D, ScatterHit, ScatterPick, Selectable, Slider, TableSelection, TableSort, Tabs, TextArea, TextInput, TimeInput, ToggleSwitch, TreeNode, TreeView, Widget

    click_callbacks: dict[str, Callable[[], None]] = {}
    change_callbacks: dict[str, Callable[[object], None]] = {}

    def walk(node: object) -> None:
        if not isinstance(node, Widget):
            return
        if isinstance(node, Button) and node.on_click is not None:
            click_callbacks[node.id] = node.on_click
        if isinstance(node, MenuItem) and node.on_click is not None:
            click_callbacks[node.id] = node.on_click
        if isinstance(node, ExtensionWidget) and node.on_click is not None and not node.disabled:
            click_callbacks[node.id] = node.on_click
        if isinstance(node, ExtensionWidget) and not node.disabled and (
            node.on_pointer_down is not None
            or node.on_pointer_move is not None
            or node.on_pointer_up is not None
            or node.on_wheel is not None
            or node.on_key_down is not None
        ):
            def extension_changed(value: object, widget: ExtensionWidget = node) -> None:
                payload = json.loads(value) if isinstance(value, str) else value
                if not isinstance(payload, Mapping):
                    raise TypeError("ExtensionWidget event payload must be a mapping")
                if payload.get("event") == "key_down":
                    if widget.on_key_down is not None:
                        widget.on_key_down(
                            PaintKeyEvent(
                                widget_id=str(payload.get("widget_id", widget.id)),
                                event="key_down",
                                key=str(payload.get("key", "")),
                                text=(
                                    str(payload.get("text"))
                                    if payload.get("text") is not None
                                    else None
                                ),
                                shift=bool(payload.get("shift", False)),
                                ctrl=bool(payload.get("ctrl", False)),
                                alt=bool(payload.get("alt", False)),
                                super=bool(payload.get("super", False)),
                                repeat=bool(payload.get("repeat", False)),
                            )
                        )
                    return
                event = PaintPointerEvent(
                    widget_id=str(payload.get("widget_id", widget.id)),
                    event=str(payload.get("event", "")),
                    x=float(payload.get("x", 0.0)),
                    y=float(payload.get("y", 0.0)),
                    local_x=float(payload.get("local_x", 0.0)),
                    local_y=float(payload.get("local_y", 0.0)),
                    dx=float(payload.get("dx", 0.0)),
                    dy=float(payload.get("dy", 0.0)),
                    button=(
                        str(payload.get("button"))
                        if payload.get("button") is not None
                        else None
                    ),
                )
                if event.event == "pointer_down" and widget.on_pointer_down is not None:
                    widget.on_pointer_down(event)
                elif event.event == "pointer_move" and widget.on_pointer_move is not None:
                    widget.on_pointer_move(event)
                elif event.event == "pointer_up" and widget.on_pointer_up is not None:
                    widget.on_pointer_up(event)
                elif event.event == "wheel" and widget.on_wheel is not None:
                    widget.on_wheel(event)

            change_callbacks[node.id] = extension_changed
        if isinstance(node, Checkbox) and node.on_change is not None:
            def checkbox_changed(value: object, widget: Checkbox = node) -> None:
                widget.checked = bool(value)
                widget.on_change(widget.checked)

            change_callbacks[node.id] = checkbox_changed
        if isinstance(node, ToggleSwitch) and node.on_change is not None:
            def toggle_switch_changed(value: object, widget: ToggleSwitch = node) -> None:
                widget.checked = bool(value)
                widget.on_change(widget.checked)

            change_callbacks[node.id] = toggle_switch_changed
        if isinstance(node, Selectable) and node.on_select is not None:
            def selectable_changed(value: object, widget: Selectable = node) -> None:
                widget.selected = bool(value)
                widget.on_select(widget.selected)

            change_callbacks[node.id] = selectable_changed
        if isinstance(node, RadioButton) and node.on_change is not None:
            def radio_changed(value: object, widget: RadioButton = node) -> None:
                widget.checked = bool(value)
                widget.on_change(widget.checked)

            change_callbacks[node.id] = radio_changed
        if isinstance(node, TreeView):
            node._wire_descendants()
        if isinstance(node, TreeNode) and (
            node.on_select is not None
            or node.on_expand is not None
            or node._tree_view is not None
        ):
            def tree_node_changed(value: object, widget: TreeNode = node) -> None:
                widget._handle_native_event(value)

            change_callbacks[node.id] = tree_node_changed
        if isinstance(node, Collapsible) and node.on_change is not None:
            def collapsible_changed(value: object, widget: Collapsible = node) -> None:
                widget.expanded = bool(value)
                widget.on_change(widget.expanded)

            change_callbacks[node.id] = collapsible_changed
        if isinstance(node, DropTarget) and node.on_drop is not None:
            def drop_target_changed(value: object, widget: DropTarget = node) -> None:
                payload = json.loads(value) if isinstance(value, str) else value
                if not isinstance(payload, Mapping):
                    raise TypeError("DropTarget change payload must be a mapping")
                drop = DragDropPayload(
                    source_id=str(payload.get("source_id", "")),
                    target_id=str(payload.get("target_id", widget.id)),
                    kind=payload.get("kind") if payload.get("kind") is not None else None,
                    payload=payload.get("payload"),
                    x=float(payload.get("x", 0.0)),
                    y=float(payload.get("y", 0.0)),
                )
                widget.on_drop(drop)

            change_callbacks[node.id] = drop_target_changed
        if isinstance(node, Slider) and node.on_change is not None:
            def slider_changed(value: object, widget: Slider = node) -> None:
                widget.value = float(value)
                widget.on_change(widget.value)

            change_callbacks[node.id] = slider_changed
        if isinstance(node, RangeSlider) and node.on_change is not None:
            def range_slider_changed(value: object, widget: RangeSlider = node) -> None:
                payload = json.loads(value) if isinstance(value, str) else value
                if isinstance(payload, Mapping):
                    low = payload.get("min")
                    high = payload.get("max")
                    if low is None or high is None:
                        pair = payload.get("value")
                        if isinstance(pair, Sequence) and len(pair) >= 2:
                            low, high = pair[0], pair[1]
                elif isinstance(payload, Sequence) and len(payload) >= 2:
                    low, high = payload[0], payload[1]
                else:
                    raise TypeError("RangeSlider change payload must be a pair or JSON object")
                widget.value = widget._normalize_value((float(low), float(high)))
                widget.on_change(widget.value)

            change_callbacks[node.id] = range_slider_changed
        if isinstance(node, NumberInput) and node.on_change is not None:
            def number_changed(value: object, widget: NumberInput = node) -> None:
                widget.value = float(value)
                widget.on_change(widget.value)

            change_callbacks[node.id] = number_changed
        if isinstance(node, DragNumber) and node.on_change is not None:
            def drag_number_changed(value: object, widget: DragNumber = node) -> None:
                widget.value = float(value)
                widget.on_change(widget.value)

            change_callbacks[node.id] = drag_number_changed
        if isinstance(node, Dropdown) and node.on_change is not None:
            def dropdown_changed(value: object, widget: Dropdown = node) -> None:
                widget.value = str(value)
                widget.on_change(widget.value)

            change_callbacks[node.id] = dropdown_changed
        if isinstance(node, (DateInput, TimeInput, DateTimeInput)):
            def temporal_changed(value: object, widget: DateInput | TimeInput | DateTimeInput = node) -> None:
                widget._handle_native_change(value)

            change_callbacks[node.id] = temporal_changed
        if isinstance(node, TextInput) and node.on_change is not None:
            def text_changed(value: object, widget: TextInput = node) -> None:
                widget.value = str(value)
                widget.on_change(widget.value)

            change_callbacks[node.id] = text_changed
        if isinstance(node, TextArea) and node.on_change is not None:
            def text_area_changed(value: object, widget: TextArea = node) -> None:
                widget.value = str(value)
                widget.on_change(widget.value)

            change_callbacks[node.id] = text_area_changed
        if isinstance(node, CodeEditor) and node.on_change is not None:
            def code_editor_changed(value: object, widget: CodeEditor = node) -> None:
                widget.value = str(value)
                widget.on_change(widget.value)

            change_callbacks[node.id] = code_editor_changed
        if isinstance(node, Tabs) and node.on_change is not None:
            def tabs_changed(value: object, widget: Tabs = node) -> None:
                widget.value = str(value)
                widget.on_change(widget.value)

            change_callbacks[node.id] = tabs_changed
        if isinstance(node, Pages) and node.on_change is not None:
            def pages_changed(value: object, widget: Pages = node) -> None:
                widget.value = str(value)
                widget.on_change(widget.value)

            change_callbacks[node.id] = pages_changed
        if isinstance(node, DataFrameTable) and (
            node.on_select is not None or node.on_sort is not None
        ):
            callback_arity = (
                _table_select_callback_arity(node.on_select)
                if node.on_select is not None
                else 1
            )

            def table_changed(
                value: object,
                widget: DataFrameTable = node,
                arity: int = callback_arity,
            ) -> None:
                payload = json.loads(value) if isinstance(value, str) else value
                if not isinstance(payload, Mapping):
                    raise TypeError("DataFrameTable change payload must be a mapping")
                event = str(payload.get("event", "select"))
                if event == "sort":
                    is_index = (
                        str(payload.get("target", "")).lower() == "index"
                        or bool(payload.get("is_index", False))
                        or int(payload.get("column_index", 0)) < 0
                    )
                    sort = TableSort(
                        column_index=-1 if is_index else int(payload["column_index"]),
                        column="#" if is_index else str(payload["column"]),
                        descending=bool(payload.get("descending", False)),
                        is_index=is_index,
                    )
                    widget.sort = sort
                    if widget.on_sort is not None:
                        widget.on_sort(sort)
                    return
                selection = TableSelection(
                    row_index=int(payload["row_index"]),
                    column_index=int(payload["column_index"]),
                    column=str(payload["column"]),
                    value=payload.get("value"),
                )
                widget.selection = selection
                if widget.on_select is None:
                    return
                if arity >= 4:
                    widget.on_select(
                        selection.row_index,
                        selection.column_index,
                        selection.column,
                        selection.value,
                    )
                elif arity == 3:
                    widget.on_select(selection.row_index, selection.column, selection.value)
                else:
                    widget.on_select(selection)

            change_callbacks[node.id] = table_changed
        if isinstance(node, NodeGraph):
            change_callbacks[node.id] = node._handle_graph_event
        if isinstance(node, Heatmap) and node.on_hover is not None:
            def heatmap_hover_changed(value: object, widget: Heatmap = node) -> None:
                payload = json.loads(value) if isinstance(value, str) else value
                if not isinstance(payload, Mapping) or payload.get("event") != "hover_changed":
                    return
                if "row" not in payload or "col" not in payload or "value" not in payload:
                    widget.hover_cell = None
                    widget.on_hover(None)
                    return
                cell = HeatmapCell(
                    row=int(payload["row"]),
                    col=int(payload["col"]),
                    value=float(payload["value"]),
                    x_label=payload.get("x_label") if payload.get("x_label") is not None else None,
                    y_label=payload.get("y_label") if payload.get("y_label") is not None else None,
                )
                widget.hover_cell = cell
                widget.on_hover(cell)

            change_callbacks[node.id] = heatmap_hover_changed
        if isinstance(node, BarChart) and node.on_hover is not None:
            def bar_chart_hover_changed(value: object, widget: BarChart = node) -> None:
                payload = json.loads(value) if isinstance(value, str) else value
                if not isinstance(payload, Mapping) or payload.get("event") != "hover_changed":
                    return
                if "index" not in payload or "series_index" not in payload or "value" not in payload:
                    widget.hover_bar = None
                    widget.on_hover(None)
                    return
                bar = BarChartBar(
                    index=int(payload["index"]),
                    category=str(payload.get("category") or ""),
                    series_index=int(payload["series_index"]),
                    series=str(payload.get("series") or ""),
                    value=float(payload["value"]),
                )
                widget.hover_bar = bar
                widget.on_hover(bar)

            change_callbacks[node.id] = bar_chart_hover_changed
        if isinstance(node, Scatter3D):
            def scatter_picked(
                value: object,
                widget: Scatter3D = node,
            ) -> None:
                payload = json.loads(value) if isinstance(value, str) else value
                if not isinstance(payload, Mapping):
                    return
                # Dispatch on explicit event tag first; fall through to legacy point-pick.
                if payload.get("event") == "hover_changed":
                    if "x" in payload:
                        widget.hover_point = (float(payload["x"]), float(payload["y"]), float(payload["z"]))
                        widget.hover_index = int(payload.get("index", 0))
                        widget.hover_actor = int(payload.get("actor", 0))
                        widget.hover_text = payload.get("hover_text")
                        on_hover = widget.on_hover
                        if on_hover is not None:
                            pick = ScatterPick(
                                index=widget.hover_index,
                                x=widget.hover_point[0],
                                y=widget.hover_point[1],
                                z=widget.hover_point[2],
                                actor=widget.hover_actor,
                            )
                            if _scatter_pick_callback_arity(on_hover) >= 4:
                                on_hover(pick.index, pick.x, pick.y, pick.z)
                            else:
                                on_hover(pick)
                    else:
                        widget.hover_point = None
                        widget.hover_index = None
                        widget.hover_actor = None
                        widget.hover_text = None
                        on_hover = widget.on_hover
                        if on_hover is not None:
                            if _scatter_pick_callback_arity(on_hover) >= 4:
                                on_hover(None, None, None, None)
                            else:
                                on_hover(None)
                    return
                # Point-pick payload: has index/x/y/z keys (no event tag).
                if "index" in payload and "x" in payload and "y" in payload and "z" in payload:
                    actor_id = int(payload.get("actor", 0))
                    pick = ScatterPick(
                        index=int(payload["index"]),
                        x=float(payload["x"]),
                        y=float(payload["y"]),
                        z=float(payload["z"]),
                        actor=actor_id,
                    )
                    widget.pick = pick
                    widget.picked_point = (pick.x, pick.y, pick.z)
                    widget.picked_index = pick.index
                    widget.picked_actor = pick.actor
                    on_pick = widget.on_pick
                    if on_pick is not None:
                        if _scatter_pick_callback_arity(on_pick) >= 4:
                            on_pick(pick.index, pick.x, pick.y, pick.z)
                        else:
                            on_pick(pick)
                elif payload.get("event") == "camera_changed":
                    # Native orbit/pan/zoom emits camera state so linked cameras can follow.
                    cam = payload.get("camera", {})
                    if cam:
                        for other in list(getattr(widget, "_camera_links", set())):
                            try:
                                other._receive_camera(cam)
                            except Exception:
                                pass
                else:
                    # Selection payload (rectangle/lasso) — route to _on_select.
                    actors: dict = payload.get("actors", {})
                    raw_index_values = {int(k): list(v) for k, v in actors.items()}
                    primary_labels = getattr(widget, "_primary_row_labels", None)
                    actor_labels_map = getattr(widget, "_actor_row_labels", {})

                    def _translate(actor_id: int, indices: list) -> list:
                        labels = primary_labels if actor_id == 0 else actor_labels_map.get(actor_id)
                        if labels is None:
                            return indices
                        return [labels[i] if i < len(labels) else i for i in indices]

                    # Build flat hit list, flat positional indices, and flat label values.
                    hits: list[ScatterHit] = []
                    all_positional: list[int] = []
                    all_label_values: list = []
                    has_labels = False
                    for actor_id, indices in sorted(raw_index_values.items()):
                        labels = primary_labels if actor_id == 0 else actor_labels_map.get(actor_id)
                        for idx in indices:
                            hits.append(ScatterHit(actor=actor_id, index=idx))
                            all_positional.append(idx)
                            if labels is not None and idx < len(labels):
                                has_labels = True
                                all_label_values.append(labels[idx])
                            else:
                                all_label_values.append(None)
                    widget.selected = hits
                    widget.selected_indices = all_positional
                    widget.selected_index_values = all_label_values if has_labels else None
                    on_sel = getattr(widget, "_on_select", None)
                    if on_sel is not None:
                        on_sel(payload)

            change_callbacks[node.id] = scatter_picked
        if isinstance(node, Container):
            for child in node.children:
                walk(child)

    walk(widget)
    return click_callbacks, change_callbacks


def _table_select_callback_arity(callback: Callable[..., object]) -> int:
    try:
        signature = inspect.signature(callback)
    except (TypeError, ValueError):
        return 1
    positional = 0
    for parameter in signature.parameters.values():
        if parameter.kind is inspect.Parameter.VAR_POSITIONAL:
            return 4
        if parameter.kind in {
            inspect.Parameter.POSITIONAL_ONLY,
            inspect.Parameter.POSITIONAL_OR_KEYWORD,
        }:
            positional += 1
    if positional >= 4:
        return 4
    if positional >= 3:
        return 3
    return 1


def _scatter_pick_callback_arity(callback: Callable[..., object]) -> int:
    try:
        signature = inspect.signature(callback)
    except (TypeError, ValueError):
        return 1
    positional = 0
    for parameter in signature.parameters.values():
        if parameter.kind is inspect.Parameter.VAR_POSITIONAL:
            return 4
        if parameter.kind in {
            inspect.Parameter.POSITIONAL_ONLY,
            inspect.Parameter.POSITIONAL_OR_KEYWORD,
        }:
            positional += 1
    return 4 if positional >= 4 else 1


def _collect_widget_ids(widget: object) -> set[str]:
    from .widgets import Container, Widget

    ids: set[str] = set()

    def walk(node: object) -> None:
        if not isinstance(node, Widget):
            return
        ids.add(node.id)
        if isinstance(node, Container):
            for child in node.children:
                walk(child)

    walk(widget)
    return ids
