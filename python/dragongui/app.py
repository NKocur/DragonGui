from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from ._backend import native_event_loop_available, run_document
from .components import ComponentInstance, render_component_window
from .runtime import AppHandle, _collect_runtime_callbacks
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
        return doc

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
                handle._close()
                self._handle = None

    def call_soon_threadsafe(self, fn: Any) -> None:
        """Schedule a callable on the DragonGUI runtime when the app is live."""
        if self._handle is None:
            raise RuntimeError("DragonGUI app is not running")
        self._handle.call_soon_threadsafe(fn)

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
