from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from collections.abc import Callable, Mapping, Sequence
import re
import time
from typing import Any

from ._backend import native_event_loop_available, run_document
from .components import ComponentInstance, render_component_window
from .icons import IconThemeValue, serialize_icon_theme
from .runtime import AppHandle, ToastHandle, _collect_runtime_callbacks, _set_active_app_handle
from .theme import Theme
from .widgets import (
    Label,
    VLayout,
    Window,
    _startup_resource_payload_scope,
    _walk_widget_tree,
)

_IMAGE_RESOURCE_ID_RE = re.compile(r"^[A-Za-z0-9._-]{1,128}$")
_IMAGE_RESOURCE_MAX_ENCODED_BYTES = 16 * 1024 * 1024


def _image_resource_id(value: object) -> str:
    if not isinstance(value, str):
        raise TypeError("image resource id must be a string")
    resource_id = value.strip()
    if not _IMAGE_RESOURCE_ID_RE.fullmatch(resource_id):
        raise ValueError(
            "image resource id must contain 1..128 ASCII letters, digits, '.', '_', or '-'"
        )
    return resource_id


def _image_resource_data(value: object) -> bytes:
    if isinstance(value, (str, Path)):
        path = Path(value)
        if not path.is_file():
            raise ValueError(f"image resource path is not a file: {path}")
        data = path.read_bytes()
    elif isinstance(value, bytes):
        data = value
    elif isinstance(value, (bytearray, memoryview)):
        data = bytes(value)
    else:
        raise TypeError("image resource must be PNG/JPEG bytes or a filesystem path")
    if not data:
        raise ValueError("image resource data cannot be empty")
    if len(data) > _IMAGE_RESOURCE_MAX_ENCODED_BYTES:
        raise ValueError("encoded image resource cannot exceed 16 MiB")
    is_png = data.startswith(b"\x89PNG\r\n\x1a\n")
    is_jpeg = data.startswith(b"\xff\xd8\xff")
    if not (is_png or is_jpeg):
        raise ValueError("image resource must contain encoded PNG or JPEG data")
    return data


@dataclass(slots=True)
class LoadingScreen:
    """Native startup loading screen configuration."""

    enabled: bool = True
    title: str = "Loading"
    message: str | None = None
    background: str | tuple[float, float, float, float] | None = None
    text: str | tuple[float, float, float, float] | None = None
    accent: str | tuple[float, float, float, float] | None = None
    show_spinner: bool = True
    show_progress: bool = False
    min_duration_ms: int = 120

    def __post_init__(self) -> None:
        self.title = str(self.title)
        self.message = None if self.message is None else str(self.message)
        self.background = _loading_color_value(self.background, name="background")
        self.text = _loading_color_value(self.text, name="text")
        self.accent = _loading_color_value(self.accent, name="accent")
        self.min_duration_ms = max(0, int(self.min_duration_ms))

    def to_dict(self) -> dict[str, Any]:
        return {
            "enabled": bool(self.enabled),
            "title": self.title,
            "message": self.message,
            "background": self.background,
            "text": self.text,
            "accent": self.accent,
            "show_spinner": bool(self.show_spinner),
            "show_progress": bool(self.show_progress),
            "min_duration_ms": self.min_duration_ms,
        }


def _loading_color_value(
    value: str | Sequence[object] | None,
    *,
    name: str,
) -> str | tuple[float, float, float, float] | None:
    if value is None or isinstance(value, str):
        return value
    if isinstance(value, (bytes, bytearray)) or not isinstance(value, Sequence):
        raise TypeError(f"LoadingScreen {name} must be a color string or RGB/RGBA tuple")
    if len(value) not in {3, 4}:
        raise ValueError(f"LoadingScreen {name} tuple must contain 3 or 4 values")
    channels = tuple(float(channel) for channel in value)
    alpha = channels[3] if len(channels) == 4 else 1.0
    return (channels[0], channels[1], channels[2], alpha)


def _stylesheet_identifier(value: object) -> str:
    if not isinstance(value, str):
        raise TypeError("stylesheet_id must be a string")
    value = value.strip()
    if not value:
        raise ValueError("stylesheet_id must be a non-empty string")
    return value


@dataclass(slots=True)
class App:
    """Top-level application object."""

    title: str = "DragonGUI"
    theme: Theme | None = None
    metadata: dict[str, Any] = field(default_factory=dict)
    loading_screen: bool | LoadingScreen | None = None
    _handle: AppHandle | None = field(default=None, init=False, repr=False)
    _stylesheets: list[tuple[str | None, str]] = field(
        default_factory=list, init=False, repr=False
    )
    _icon_theme: dict[str, object] = field(default_factory=dict, init=False, repr=False)
    _image_resources: dict[str, bytes] = field(default_factory=dict, init=False, repr=False)

    def document(
        self,
        window: Window,
        *,
        include_startup_resource_payloads: bool = True,
    ) -> dict[str, Any]:
        with _startup_resource_payload_scope(include_startup_resource_payloads):
            window_doc = window.to_dict()
        doc: dict[str, Any] = {
            "schema": 1,
            "type": "app",
            "title": self.title,
            "metadata": self.metadata,
            "window": window_doc,
        }
        if self.theme is not None:
            doc["theme"] = self.theme.to_dict()
        doc["loading_screen"] = _loading_screen_to_dict(self.loading_screen)
        if self._stylesheets:
            doc["stylesheets"] = [
                {
                    "origin": "user",
                    "source": css,
                    **({"id": stylesheet_id} if stylesheet_id is not None else {}),
                }
                for stylesheet_id, css in self._stylesheets
            ]
        if self._icon_theme:
            doc["icon_theme"] = dict(self._icon_theme)
        return doc

    def set_icon_theme(self, overrides: Mapping[str, IconThemeValue]) -> None:
        """Replace semantic icon overrides used when the application starts.

        Live replacement is atomic; existing icon widgets are reconciled and
        their text/paint data is refreshed without changing layout.
        """

        if not isinstance(overrides, Mapping):
            raise TypeError("icon theme overrides must be a mapping")
        self._icon_theme = serialize_icon_theme(overrides)
        if self._handle is not None:
            self._handle.enqueue_set_icon_theme(self._icon_theme)

    def set_image_resource(self, resource_id: str, source: object) -> None:
        """Register or replace an application-owned PNG/JPEG image resource.

        ``source`` may be encoded bytes or an explicit filesystem path. CSS
        refers to the retained identifier; CSS never opens the path itself.
        """

        resource_id = _image_resource_id(resource_id)
        data = _image_resource_data(source)
        self._image_resources[resource_id] = data
        if self._handle is not None:
            self._handle.enqueue_set_buffer_resource(
                resource_id,
                data,
                kind="image_encoded",
            )

    def release_image_resource(self, resource_id: str) -> None:
        """Release a managed image from startup and live native retention."""

        resource_id = _image_resource_id(resource_id)
        self._image_resources.pop(resource_id, None)
        if self._handle is not None:
            self._handle.release_resource(resource_id)

    def _queue_image_resources(self, handle: AppHandle) -> None:
        for resource_id, data in self._image_resources.items():
            handle.enqueue_set_buffer_resource(
                resource_id,
                data,
                kind="image_encoded",
            )

    def stylesheet(self, css: str) -> None:
        if not isinstance(css, str):
            raise TypeError("css must be a string")
        if not css.strip():
            raise ValueError("css must be a non-empty string")
        if self._handle is not None:
            self._handle.enqueue_set_stylesheet(css)
            return
        self._stylesheets.append((None, css))

    def set_stylesheet(self, stylesheet_id: str, css: str) -> None:
        """Add or atomically replace a named user stylesheet.

        Replacing a name preserves its position in the user cascade.
        """
        stylesheet_id = _stylesheet_identifier(stylesheet_id)
        if not isinstance(css, str):
            raise TypeError("css must be a string")
        if not css.strip():
            raise ValueError("css must be a non-empty string")
        if self._handle is not None:
            self._handle.enqueue_set_named_stylesheet(stylesheet_id, css)
            return
        for index, (existing_id, _) in enumerate(self._stylesheets):
            if existing_id == stylesheet_id:
                self._stylesheets[index] = (stylesheet_id, css)
                break
        else:
            self._stylesheets.append((stylesheet_id, css))

    def remove_stylesheet(self, stylesheet_id: str) -> bool | None:
        """Remove a named user stylesheet.

        Before startup this returns whether the name existed. During runtime
        removal is asynchronous and returns ``None``.
        """
        stylesheet_id = _stylesheet_identifier(stylesheet_id)
        if self._handle is not None:
            self._handle.enqueue_remove_stylesheet(stylesheet_id)
            return None
        for index, (existing_id, _) in enumerate(self._stylesheets):
            if existing_id == stylesheet_id:
                self._stylesheets.pop(index)
                return True
        return False

    def set_theme(self, theme: Theme) -> None:
        """Atomically replace the active design-token theme."""
        if not isinstance(theme, Theme):
            raise TypeError("theme must be a DragonGUI Theme")
        self.theme = theme
        if self._handle is not None:
            self._handle.enqueue_set_theme(theme.to_dict())

    def load_stylesheet(self, path: str | Path) -> None:
        """Load a user stylesheet from disk."""
        css = Path(path).read_text(encoding="utf-8")
        self.stylesheet(css)

    def clear_stylesheets(self) -> None:
        """Clear user stylesheets queued for startup or applied to a live app."""
        if self._handle is not None:
            self._handle.enqueue_clear_stylesheets()
            return
        self._stylesheets.clear()

    def run(self, window: Window | ComponentInstance) -> dict[str, Any]:
        """Start the native event loop for a window."""
        startup_t0 = time.perf_counter()
        startup_timings: dict[str, Any] = {}

        def record_startup_phase(name: str, started: float) -> None:
            startup_timings[name] = (time.perf_counter() - started) * 1000.0

        component_runtime = window._runtime if isinstance(window, ComponentInstance) else None
        bind_live = native_event_loop_available()
        if bind_live:
            from .diagnostics import _reset_collector
            _reset_collector()
        handle = AppHandle()
        if component_runtime is not None and bind_live:
            component_runtime.attach(handle)
        if component_runtime is not None:
            phase_t0 = time.perf_counter()
            try:
                window = render_component_window(window)
            except Exception:
                if bind_live:
                    component_runtime.detach()
                    handle._close()
                raise
            record_startup_phase("render_component_window_ms", phase_t0)
        phase_t0 = time.perf_counter()
        click_cbs, change_cbs = _collect_runtime_callbacks(window)
        record_startup_phase("collect_callbacks_ms", phase_t0)
        phase_t0 = time.perf_counter()
        widgets = _walk_widget_tree(window)
        record_startup_phase("walk_widgets_ms", phase_t0)
        if bind_live:
            phase_t0 = time.perf_counter()
            self._handle = handle
            self._queue_image_resources(handle)
            if component_runtime is not None and component_runtime.app_handle is not handle:
                component_runtime.attach(handle)
            handle.register_widget_callbacks(window)
            for widget in widgets:
                widget._bind_live(handle.widget_handle(widget.id))
            for widget in widgets:
                widget._queue_startup_resources()
            _set_active_app_handle(handle)
            record_startup_phase("bind_and_queue_resources_ms", phase_t0)
        try:
            native_click_cbs = {} if component_runtime is not None and bind_live else click_cbs
            native_change_cbs = {} if component_runtime is not None and bind_live else change_cbs
            phase_t0 = time.perf_counter()
            startup_document = self.document(
                window, include_startup_resource_payloads=not bind_live
            )
            record_startup_phase("document_ms", phase_t0)
            startup_timings["pre_native_total_ms"] = (
                time.perf_counter() - startup_t0
            ) * 1000.0
            handle._set_startup_timings(startup_timings)
            result = run_document(
                startup_document,
                native_click_cbs,
                native_change_cbs,
                app_handle=handle if bind_live else None,
            )
            if bind_live and isinstance(result, dict):
                snapshot = result.get("debug_snapshot")
                if isinstance(snapshot, dict):
                    runtime = snapshot.setdefault("runtime", {})
                    if isinstance(runtime, dict):
                        runtime["python"] = handle._python_debug_snapshot()
            return result
        finally:
            if bind_live:
                if component_runtime is not None:
                    component_runtime.detach()
                for widget in widgets:
                    widget._unbind_live()
                _set_active_app_handle(None)
                handle._close()
                self._handle = None

    def run_with_loading(
        self,
        build_window: Callable[[], Window | ComponentInstance],
        *,
        title: str | None = None,
        width: int = 1024,
        height: int = 768,
    ) -> dict[str, Any]:
        """Start with the native loading screen, then build and swap in the real window.

        ``build_window`` is called after the native window and loading frame are
        visible. This covers expensive Python-side document construction that
        would otherwise happen before ``App.run()`` can show anything.
        """
        if not callable(build_window):
            raise TypeError("run_with_loading expects a callable that returns a Window")

        if not native_event_loop_available():
            return self.run(_require_startup_root(build_window()))

        from .diagnostics import _reset_collector

        _reset_collector()
        handle = AppHandle()
        placeholder = Window(
            title or self.title,
            width=width,
            height=height,
            id="__dg_startup_root",
            key="__dg_startup_root",
        )

        live_widgets: list[Any] = []
        component_runtime = None

        def build_and_replace() -> None:
            nonlocal component_runtime
            startup_t0 = time.perf_counter()
            timings: dict[str, Any] = {}

            def record_phase(name: str, start: float) -> None:
                timings[name] = (time.perf_counter() - start) * 1000.0

            try:
                phase_t0 = time.perf_counter()
                startup_root = build_window()
                record_phase("build_window_ms", phase_t0)

                phase_t0 = time.perf_counter()
                real_window, component_runtime = _render_startup_root(startup_root, handle)
                record_phase("render_startup_root_ms", phase_t0)

                phase_t0 = time.perf_counter()
                real_widgets = _walk_widget_tree(real_window)
                record_phase("walk_widgets_ms", phase_t0)
                scatter_widget_count = sum(
                    1 for widget in real_widgets if getattr(widget, "kind", None) == "scatter_3d"
                )

                phase_t0 = time.perf_counter()
                for widget in real_widgets:
                    widget._bind_live(handle.widget_handle(widget.id))
                record_phase("bind_live_ms", phase_t0)

                phase_t0 = time.perf_counter()
                handle.register_widget_callbacks(real_window)
                record_phase("register_callbacks_ms", phase_t0)
                live_widgets[:] = list(real_widgets)

                phase_t0 = time.perf_counter()
                with _startup_resource_payload_scope(False):
                    real_window_node = real_window.to_dict()
                record_phase("to_dict_ms", phase_t0)

                phase_t0 = time.perf_counter()
                handle.enqueue_prewarm_scatter_widgets(scatter_widget_count)
                record_phase("enqueue_prewarm_scatter_ms", phase_t0)

                phase_t0 = time.perf_counter()
                handle.enqueue_replace_node(placeholder.id, real_window_node)
                record_phase("enqueue_replace_node_ms", phase_t0)

                phase_t0 = time.perf_counter()
                resource_by_kind: dict[str, dict[str, float | int]] = {}
                resource_slowest: list[dict[str, Any]] = []
                for widget in real_widgets:
                    widget_t0 = time.perf_counter()
                    widget._queue_startup_resources()
                    widget_ms = (time.perf_counter() - widget_t0) * 1000.0
                    if widget_ms > 0.01:
                        kind = str(getattr(widget, "kind", type(widget).__name__))
                        bucket = resource_by_kind.setdefault(
                            kind,
                            {"count": 0, "total_ms": 0.0, "max_ms": 0.0},
                        )
                        bucket["count"] = int(bucket["count"]) + 1
                        bucket["total_ms"] = float(bucket["total_ms"]) + widget_ms
                        bucket["max_ms"] = max(float(bucket["max_ms"]), widget_ms)
                        item = {
                            "kind": kind,
                            "id": str(getattr(widget, "id", "")),
                            "elapsed_ms": widget_ms,
                        }
                        detail = getattr(widget, "_last_startup_resource_timings", None)
                        if isinstance(detail, dict):
                            item["detail"] = dict(detail)
                        resource_slowest.append(item)
                record_phase("queue_startup_resources_ms", phase_t0)
                timings["startup_resources_by_kind"] = resource_by_kind
                timings["startup_resources_slowest"] = sorted(
                    resource_slowest,
                    key=lambda item: float(item["elapsed_ms"]),
                    reverse=True,
                )[:12]
                timings["total_ms"] = (time.perf_counter() - startup_t0) * 1000.0
                handle._set_startup_timings(timings)
            except Exception as exc:
                timings["total_ms"] = (time.perf_counter() - startup_t0) * 1000.0
                handle._set_startup_timings(timings)
                error_window = Window(
                    title or self.title,
                    width=width,
                    height=height,
                    id=placeholder.id,
                    key=placeholder.key,
                )
                with error_window:
                    with VLayout(style={"padding": 18, "gap": 10}):
                        Label("Startup failed")
                        Label(str(exc), style={"text_wrap": "wrap"})
                handle.enqueue_replace_node(placeholder.id, error_window.to_dict())
                raise

        handle.call_soon_threadsafe(build_and_replace)
        self._handle = handle
        self._queue_image_resources(handle)
        _set_active_app_handle(handle)
        try:
            result = run_document(
                self.document(placeholder, include_startup_resource_payloads=False),
                {},
                {},
                app_handle=handle,
            )
            if isinstance(result, dict):
                snapshot = result.get("debug_snapshot")
                if isinstance(snapshot, dict):
                    runtime = snapshot.setdefault("runtime", {})
                    if isinstance(runtime, dict):
                        runtime["python"] = handle._python_debug_snapshot()
            return result
        finally:
            if component_runtime is not None:
                component_runtime.detach()
            for widget in live_widgets:
                widget._unbind_live()
            _set_active_app_handle(None)
            handle._close()
            self._handle = None

    def call_soon_threadsafe(self, fn: Any, *, coalesce_key: object | None = None) -> None:
        """Schedule a callable, optionally replacing pending work with the same key."""
        if self._handle is None:
            raise RuntimeError("DragonGUI app is not running")
        if coalesce_key is None:
            self._handle.call_soon_threadsafe(fn)
        else:
            self._handle.call_soon_threadsafe(fn, coalesce_key=coalesce_key)

    def update_batch(self) -> Any:
        """Return a nestable context that batches live property setters.

        The app must be running. Collected setters update Python widget state
        immediately and are submitted in one ordered native packet when the
        outer context exits. Duplicate widget/property writes keep the last
        value. Non-property commands act as ordering barriers, and exceptions
        flush pending setters before propagating; batches are not transactions.
        """
        if self._handle is None:
            raise RuntimeError("DragonGUI app is not running")
        return self._handle.update_batch()

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
        """Show a non-blocking native toast while the app is live."""
        if self._handle is None:
            raise RuntimeError("DragonGUI app is not running")
        return self._handle.toast(
            message,
            level=level,
            duration=duration,
            opacity=opacity,
            radius=radius,
            padding=padding,
            position=position,
        )

    def debug_snapshot(self, timeout_ms: int = 1000) -> dict[str, Any]:
        """Return a JSON-safe snapshot of the live native runtime."""
        if self._handle is None:
            raise RuntimeError("DragonGUI app is not running")
        return self._handle.debug_snapshot(timeout_ms)

    def _latency_probe(self, timeout_ms: int = 1000) -> bool | None:
        """Round-trip a private lightweight native ordering barrier."""
        if self._handle is None:
            raise RuntimeError("DragonGUI app is not running")
        return self._handle.latency_probe(timeout_ms)

    def _window_screenshot(self, timeout_ms: int = 10000) -> tuple[int, int, bytes] | None:
        """Return a private live whole-window RGBA screenshot tuple."""
        if self._handle is None:
            raise RuntimeError("DragonGUI app is not running")
        return self._handle.window_screenshot(timeout_ms)

    def request_redraw(self) -> None:
        """Request one native redraw without changing widget state."""
        if self._handle is None:
            raise RuntimeError("DragonGUI app is not running")
        self._handle.request_redraw()

    def request_exit(self) -> None:
        """Request the native event loop to exit."""
        if self._handle is None:
            raise RuntimeError("DragonGUI app is not running")
        self._handle.request_exit()

    def set_buffer_resource(
        self,
        resource_id: str,
        data: object,
        *,
        kind: str = "bytes",
        owner: object = None,
    ) -> None:
        """Upload a retained buffer resource while the app is running.

        By default generic buffers are app-owned and must be released with
        ``release_resource(resource_id)``. Pass ``owner=widget`` or
        ``owner=widget_id`` to tie the buffer to a live widget; widget-owned
        buffers are purged automatically when that widget leaves the retained
        tree.
        """
        if self._handle is None:
            raise RuntimeError("DragonGUI app is not running")
        self._handle.enqueue_set_buffer_resource(resource_id, data, kind=kind, owner=owner)

    def release_resource(self, resource_id: str) -> None:
        """Release a retained native resource while the app is running."""
        if self._handle is None:
            raise RuntimeError("DragonGUI app is not running")
        self._handle.release_resource(resource_id)


def _loading_screen_to_dict(value: bool | LoadingScreen | None) -> dict[str, Any]:
    if value is False:
        return LoadingScreen(enabled=False).to_dict()
    if value is True or value is None:
        return LoadingScreen().to_dict()
    if isinstance(value, LoadingScreen):
        return value.to_dict()
    raise TypeError("loading_screen must be a bool, LoadingScreen, or None")


def _require_startup_root(value: object) -> Window | ComponentInstance:
    if isinstance(value, (Window, ComponentInstance)):
        return value
    raise TypeError("run_with_loading builder must return a DragonGUI Window or component")


def _render_startup_root(
    value: object,
    handle: AppHandle,
) -> tuple[Window, Any | None]:
    if isinstance(value, Window):
        return value, None
    if isinstance(value, ComponentInstance):
        runtime = value._runtime
        runtime.attach(handle)
        try:
            return render_component_window(value), runtime
        except Exception:
            runtime.detach()
            raise
    raise TypeError("run_with_loading builder must return a DragonGUI Window or component")


def _require_window(value: object) -> Window:
    if isinstance(value, Window):
        return value
    raise TypeError("run_with_loading builder must return a DragonGUI Window")


def run_with_loading(
    build_window: Callable[[], Window],
    *,
    app: App | None = None,
    title: str = "DragonGUI",
    theme: Theme | None = None,
    metadata: dict[str, Any] | None = None,
    loading_screen: bool | LoadingScreen | None = None,
    width: int = 1024,
    height: int = 768,
) -> dict[str, Any]:
    """Run an app whose real window is built after the loading screen appears."""
    dragon_app = app or App(
        title=title,
        theme=theme,
        metadata={} if metadata is None else metadata,
        loading_screen=loading_screen,
    )
    return dragon_app.run_with_loading(
        build_window,
        title=title,
        width=width,
        height=height,
    )
