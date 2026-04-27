from __future__ import annotations

from collections import deque
from collections.abc import Callable, Iterable, Mapping
import json
from threading import RLock
import traceback
from typing import Any


_MAX_PYTHON_TASKS_PER_DRAIN = 100


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

    def enqueue_set_scatter_points_packed(
        self,
        xyz: bytes,
        *,
        pack_ms: float | None = None,
        enqueue_epoch_ms: float | None = None,
        colormap: str = "viridis",
    ) -> None:
        self.ensure_open()
        self.app.enqueue_set_scatter_points_packed(
            self.id,
            xyz,
            pack_ms=pack_ms,
            enqueue_epoch_ms=enqueue_epoch_ms,
            colormap=colormap,
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
        self._tasks: deque[Callable[[], None]] = deque()
        self._pending_native: deque[tuple[str, tuple[object, ...]]] = deque()
        self._click_callbacks: dict[str, Callable[[], None]] = {}
        self._change_callbacks: dict[str, Callable[[object], None]] = {}
        self._native_sender: Any | None = None
        self._closed = False

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

    def call_soon_threadsafe(self, fn: Callable[[], None]) -> None:
        if not callable(fn):
            raise TypeError("call_soon_threadsafe expects a callable")
        with self._lock:
            if self._closed:
                raise RuntimeError("DragonGUI app handle is closed")
            self._tasks.append(fn)
            sender = self._native_sender
        if sender is not None:
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

    def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
        self._send_or_queue_native("enqueue_set_prop", widget_id, prop, value)

    def enqueue_invalidate(self, widget_id: str, dirty: str) -> None:
        self._send_or_queue_native("enqueue_invalidate", widget_id, dirty)

    def enqueue_set_style(self, widget_id: str, style: object) -> None:
        self._send_or_queue_native("enqueue_set_style", widget_id, _style_json(style))

    def enqueue_replace_children(self, widget_id: str, children: object) -> None:
        self._send_or_queue_native("enqueue_replace_children", widget_id, _children_json(children))

    def enqueue_replace_node(self, widget_id: str, node: object) -> None:
        self._send_or_queue_native("enqueue_replace_node", widget_id, _node_json(node))

    def enqueue_set_scatter_points_packed(
        self,
        widget_id: str,
        xyz: bytes,
        *,
        pack_ms: float | None = None,
        enqueue_epoch_ms: float | None = None,
        colormap: str = "viridis",
    ) -> None:
        self._send_or_queue_native(
            "enqueue_set_scatter_points_packed",
            widget_id,
            xyz,
            pack_ms,
            enqueue_epoch_ms,
            _scatter_colormap(colormap),
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

    def enqueue_clear_stylesheets(self) -> None:
        self._send_or_queue_native("enqueue_clear_stylesheets", "user")

    def debug_snapshot(self, timeout_ms: int = 1000) -> dict[str, Any]:
        """Return a JSON-safe snapshot of the live native runtime."""
        with self._lock:
            if self._closed:
                raise RuntimeError("DragonGUI app handle is closed")
            sender = self._native_sender
            queued_tasks = len(self._tasks)
            pending_native = len(self._pending_native)
        if sender is None:
            return {
                "schema": 1,
                "runtime": {
                    "native_bound": False,
                    "queued_python_tasks": queued_tasks,
                    "pending_native_commands": pending_native,
                    "closed": False,
                },
            }
        snapshot_json = sender.debug_snapshot(timeout_ms)
        snapshot = json.loads(snapshot_json)
        if not isinstance(snapshot, dict):
            raise RuntimeError("DragonGUI native debug snapshot was not a JSON object")
        return snapshot

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

    def _send_or_queue_native(self, method: str, *args: object) -> None:
        with self._lock:
            if self._closed:
                raise RuntimeError("DragonGUI app handle is closed")
            sender = self._native_sender
            if sender is None:
                self._pending_native.append((method, args))
                return
        try:
            getattr(sender, method)(*args)
        except RuntimeError as exc:
            with self._lock:
                closed = self._closed
            is_closed = getattr(sender, "is_closed", False)
            sender_closed = bool(is_closed() if callable(is_closed) else is_closed)
            if closed or sender_closed:
                raise RuntimeError("DragonGUI app handle is closed") from exc
            raise

    def _bind_native_sender(self, sender: Any) -> None:
        with self._lock:
            if self._closed:
                if hasattr(sender, "close"):
                    sender.close()
                raise RuntimeError("cannot bind native sender to a closed DragonGUI app handle")
            self._native_sender = sender
            has_tasks = bool(self._tasks)
            pending = list(self._pending_native)
            self._pending_native.clear()
        for method, args in pending:
            getattr(sender, method)(*args)
        if has_tasks:
            sender.enqueue_drain_python_tasks()

    def _drain_python_tasks(self) -> None:
        processed = 0
        while processed < _MAX_PYTHON_TASKS_PER_DRAIN:
            with self._lock:
                if not self._tasks:
                    return
                task = self._tasks.popleft()
            try:
                task()
            except Exception:  # pragma: no cover - diagnostic path
                traceback.print_exc()
            processed += 1

        with self._lock:
            sender = self._native_sender if self._tasks and not self._closed else None
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
            self._tasks.clear()
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
    from .widgets import Button, Checkbox, Container, Dropdown, MenuItem, NumberInput, Pages, Slider, Tabs, TextInput, Widget

    click_callbacks: dict[str, Callable[[], None]] = {}
    change_callbacks: dict[str, Callable[[object], None]] = {}

    def walk(node: object) -> None:
        if not isinstance(node, Widget):
            return
        if isinstance(node, Button) and node.on_click is not None:
            click_callbacks[node.id] = node.on_click
        if isinstance(node, MenuItem) and node.on_click is not None:
            click_callbacks[node.id] = node.on_click
        if isinstance(node, Checkbox) and node.on_change is not None:
            def checkbox_changed(value: object, widget: Checkbox = node) -> None:
                widget.checked = bool(value)
                widget.on_change(widget.checked)

            change_callbacks[node.id] = checkbox_changed
        if isinstance(node, Slider) and node.on_change is not None:
            def slider_changed(value: object, widget: Slider = node) -> None:
                widget.value = float(value)
                widget.on_change(widget.value)

            change_callbacks[node.id] = slider_changed
        if isinstance(node, NumberInput) and node.on_change is not None:
            def number_changed(value: object, widget: NumberInput = node) -> None:
                widget.value = float(value)
                widget.on_change(widget.value)

            change_callbacks[node.id] = number_changed
        if isinstance(node, Dropdown) and node.on_change is not None:
            def dropdown_changed(value: object, widget: Dropdown = node) -> None:
                widget.value = str(value)
                widget.on_change(widget.value)

            change_callbacks[node.id] = dropdown_changed
        if isinstance(node, TextInput) and node.on_change is not None:
            def text_changed(value: object, widget: TextInput = node) -> None:
                widget.value = str(value)
                widget.on_change(widget.value)

            change_callbacks[node.id] = text_changed
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
        if isinstance(node, Container):
            for child in node.children:
                walk(child)

    walk(widget)
    return click_callbacks, change_callbacks


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
