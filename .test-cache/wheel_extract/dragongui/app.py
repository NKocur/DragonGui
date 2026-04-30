from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from ._backend import native_event_loop_available, run_document
from .components import ComponentInstance, render_component_window
from .runtime import AppHandle, ToastHandle, _collect_runtime_callbacks, _set_active_app_handle
from .theme import Theme
from .widgets import (
    Window,
    _walk_widget_tree,
)


@dataclass(slots=True)
class App:
    """Top-level application object."""

    title: str = "DragonGUI"
    theme: Theme | None = None
    metadata: dict[str, Any] = field(default_factory=dict)
    _handle: AppHandle | None = field(default=None, init=False, repr=False)
    _stylesheets: list[str] = field(default_factory=list, init=False, repr=False)

    def document(self, window: Window) -> dict[str, Any]:
        doc: dict[str, Any] = {
            "schema": 1,
            "type": "app",
            "title": self.title,
            "metadata": self.metadata,
            "window": window.to_dict(),
        }
        if self.theme is not None:
            doc["theme"] = self.theme.to_dict()
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
        if component_runtime is not None:
            window = render_component_window(window)
        click_cbs, change_cbs = _collect_runtime_callbacks(window)
        handle = AppHandle()
        widgets = _walk_widget_tree(window)
        bind_live = native_event_loop_available()
        if bind_live:
            self._handle = handle
            if component_runtime is not None:
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
                self.document(window),
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
