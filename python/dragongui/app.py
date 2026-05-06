from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from collections.abc import Callable, Sequence
from typing import Any

from ._backend import native_event_loop_available, run_document
from .components import ComponentInstance, render_component_window
from .runtime import AppHandle, ToastHandle, _collect_runtime_callbacks, _set_active_app_handle
from .theme import Theme
from .widgets import (
    Label,
    VLayout,
    Window,
    _startup_resource_payload_scope,
    _walk_widget_tree,
)


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


@dataclass(slots=True)
class App:
    """Top-level application object."""

    title: str = "DragonGUI"
    theme: Theme | None = None
    metadata: dict[str, Any] = field(default_factory=dict)
    loading_screen: bool | LoadingScreen | None = None
    _handle: AppHandle | None = field(default=None, init=False, repr=False)
    _stylesheets: list[str] = field(default_factory=list, init=False, repr=False)

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
                {"origin": "user", "source": css} for css in self._stylesheets
            ]
        return doc

    def stylesheet(self, css: str) -> None:
        if not isinstance(css, str):
            raise TypeError("css must be a string")
        if not css.strip():
            raise ValueError("css must be a non-empty string")
        if self._handle is not None:
            self._handle.enqueue_set_stylesheet(css)
            return
        self._stylesheets.append(css)

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
        component_runtime = window._runtime if isinstance(window, ComponentInstance) else None
        bind_live = native_event_loop_available()
        if bind_live:
            from .diagnostics import _reset_collector
            _reset_collector()
        handle = AppHandle()
        if component_runtime is not None and bind_live:
            component_runtime.attach(handle)
        if component_runtime is not None:
            try:
                window = render_component_window(window)
            except Exception:
                if bind_live:
                    component_runtime.detach()
                    handle._close()
                raise
        click_cbs, change_cbs = _collect_runtime_callbacks(window)
        widgets = _walk_widget_tree(window)
        if bind_live:
            self._handle = handle
            if component_runtime is not None and component_runtime.app_handle is not handle:
                component_runtime.attach(handle)
            handle.register_widget_callbacks(window)
            for widget in widgets:
                widget._bind_live(handle.widget_handle(widget.id))
            for widget in widgets:
                widget._queue_startup_resources()
            _set_active_app_handle(handle)
        try:
            native_click_cbs = {} if component_runtime is not None and bind_live else click_cbs
            native_change_cbs = {} if component_runtime is not None and bind_live else change_cbs
            return run_document(
                self.document(window, include_startup_resource_payloads=not bind_live),
                native_click_cbs,
                native_change_cbs,
                app_handle=handle if bind_live else None,
            )
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
            try:
                real_window, component_runtime = _render_startup_root(build_window(), handle)
                real_widgets = _walk_widget_tree(real_window)
                for widget in real_widgets:
                    widget._bind_live(handle.widget_handle(widget.id))
                handle.register_widget_callbacks(real_window)
                live_widgets[:] = list(real_widgets)
                handle.enqueue_replace_node(placeholder.id, real_window.to_dict())
                for widget in real_widgets:
                    widget._queue_startup_resources()
            except Exception as exc:
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
        _set_active_app_handle(handle)
        try:
            return run_document(
                self.document(placeholder, include_startup_resource_payloads=False),
                {},
                {},
                app_handle=handle,
            )
        finally:
            if component_runtime is not None:
                component_runtime.detach()
            for widget in live_widgets:
                widget._unbind_live()
            _set_active_app_handle(None)
            handle._close()
            self._handle = None

    def call_soon_threadsafe(self, fn: Any) -> None:
        """Schedule a callable on the DragonGUI runtime when the app is live."""
        if self._handle is None:
            raise RuntimeError("DragonGUI app is not running")
        self._handle.call_soon_threadsafe(fn)

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

    def request_redraw(self) -> None:
        """Request one native redraw without changing widget state."""
        if self._handle is None:
            raise RuntimeError("DragonGUI app is not running")
        self._handle.request_redraw()

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
