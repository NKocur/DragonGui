from __future__ import annotations

import base64
from collections.abc import Callable, Iterable, Mapping
from contextlib import AbstractContextManager
from itertools import count
import re
import time
from typing import Any, ClassVar, Self

from .dataframe import (
    DEFAULT_TABLE_SAMPLE_ROWS,
    extract_table_column_buffers,
    extract_table_sample,
    summarize_frame,
)


def _pack_xyz_bytes(frame: Any, x_col: str, y_col: str, z_col: str) -> bytes | None:
    """Serialize xyz columns as packed float32 little-endian xyz triples.

    Returns raw bytes on success, or None if the frame has no accessible array
    attributes (e.g. mock frames used in tests).  NumPy is required for
    efficient serialization; without it this returns None.
    """
    try:
        import numpy as np

        xs = np.asarray(getattr(frame, x_col), dtype=np.float32)
        ys = np.asarray(getattr(frame, y_col), dtype=np.float32)
        zs = np.asarray(getattr(frame, z_col), dtype=np.float32)
        return np.column_stack([xs, ys, zs]).astype(np.float32, copy=False).tobytes()
    except (ImportError, AttributeError, TypeError, ValueError):
        return None


def _try_pack_xyz(frame: Any, x_col: str, y_col: str, z_col: str) -> str | None:
    """Serialize xyz columns as base64 for startup document compatibility."""
    buf = _pack_xyz_bytes(frame, x_col, y_col, z_col)
    if buf is None:
        return None
    return base64.b64encode(buf).decode("ascii")


_SCATTER_COLORMAPS = {
    "viridis",
    "plasma",
    "inferno",
    "magma",
    "coolwarm",
    "hot",
    "gray",
    "grey",
    "turbo",
    "cividis",
    "blues",
    "greens",
    "reds",
}


def _scatter_colormap(value: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValueError("Scatter3D colormap must be a non-empty string")
    colormap = value.strip().lower()
    if colormap not in _SCATTER_COLORMAPS:
        allowed = ", ".join(sorted(_SCATTER_COLORMAPS))
        raise ValueError(f"unknown Scatter3D colormap {value!r}; expected one of: {allowed}")
    return colormap


def _copy_style(style: Mapping[str, object] | None) -> dict[str, object] | None:
    if style is None:
        return None
    if not isinstance(style, Mapping):
        raise TypeError("widget style must be a mapping")
    return dict(style)


def _style_patch(
    old: Mapping[str, object] | None,
    new: Mapping[str, object] | None,
) -> dict[str, object | None]:
    old_map = dict(old or {})
    new_map = dict(new or {})
    patch: dict[str, object | None] = {}
    for key in sorted(set(old_map) | set(new_map)):
        if key not in new_map:
            patch[key] = None
        elif key not in old_map or old_map[key] != new_map[key]:
            patch[key] = new_map[key]
    return patch


def _walk_widget_tree(widget: "Widget") -> list["Widget"]:
    widgets = [widget]
    if isinstance(widget, Container):
        for child in widget.children:
            widgets.extend(_walk_widget_tree(child))
    return widgets


Callback = Callable[[], None]
BoolCallback = Callable[[bool], None]
FloatCallback = Callable[[float], None]
StringCallback = Callable[[str], None]

_ids = count(1)
_AUTO_PARENT = object()


def _route_value(label: str) -> str:
    value = re.sub(r"[^a-z0-9]+", "_", label.lower()).strip("_")
    return value or "page"


class _BuildContext:
    stack: ClassVar[list[Container]] = []
    root: ClassVar[Window | None] = None

    @classmethod
    def parent(cls) -> Container | None:
        if cls.stack:
            return cls.stack[-1]
        return cls.root

    @classmethod
    def push(cls, widget: Container) -> None:
        cls.stack.append(widget)

    @classmethod
    def pop(cls, widget: Container) -> None:
        if not cls.stack or cls.stack[-1] is not widget:
            raise RuntimeError("DragonGUI layout contexts exited out of order")
        cls.stack.pop()


class Widget:
    kind = "widget"

    def __init__(
        self,
        *,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: "Container | None | object" = _AUTO_PARENT,
    ) -> None:
        if key is not None and (not isinstance(key, str) or not key):
            raise ValueError("widget key must be a non-empty string")
        if class_ is not None and (not isinstance(class_, str) or not class_):
            raise ValueError("widget class_ must be a non-empty string")
        self.id = id or f"dg-{next(_ids)}"
        self.key = key
        self.class_ = class_
        self.style = _copy_style(style)
        self._live_handle: Any | None = None
        self.parent: Container | None = None
        if parent is _AUTO_PARENT:
            parent = _BuildContext.parent()
        if parent is not None:
            if not isinstance(parent, Container):
                raise TypeError("parent must be a DragonGUI container or None")
            parent.add(self)

    def props(self) -> dict[str, Any]:
        return {}

    @property
    def is_live(self) -> bool:
        return self._live() is not None

    def _live(self) -> Any | None:
        handle = self._live_handle
        if handle is None or handle.closed:
            return None
        return handle

    def _bind_live(self, handle: Any) -> None:
        self._live_handle = handle

    def _unbind_live(self) -> None:
        self._live_handle = None

    def _sync_after_id_change(self, old_id: str) -> None:
        pass

    def _queue_startup_resources(self) -> None:
        pass

    def set_style(self, style: Mapping[str, object] | None) -> None:
        new_style = _copy_style(style)
        patch = _style_patch(self.style, new_style)
        self.style = new_style
        handle = self._live()
        if handle is not None and patch:
            handle.enqueue_set_style(patch)

    def to_dict(self) -> dict[str, Any]:
        data = {
            "id": self.id,
            "type": self.kind,
            "props": self.props(),
        }
        if self.key is not None:
            data["key"] = self.key
        if self.class_ is not None:
            data["class"] = self.class_
        if self.style is not None:
            data["style"] = self.style
        return data

    def to_vnode(self) -> object:
        from .vdom import widget_to_vnode

        return widget_to_vnode(self)


class Container(Widget, AbstractContextManager["Container"]):
    def __init__(
        self,
        *,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: "Container | None | object" = _AUTO_PARENT,
    ) -> None:
        self.children: list[Widget] = []
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def add(self, child: Widget) -> Widget:
        if child.parent is self:
            return child
        if child.parent is not None:
            child.parent.children.remove(child)
        child.parent = self
        self.children.append(child)
        return child

    def replace_children(self, children: Iterable[Widget]) -> None:
        new_children = list(children)
        if not all(isinstance(child, Widget) for child in new_children):
            raise TypeError("replace_children expects DragonGUI widget children")

        old_children = list(self.children)
        live_handle = self._live()
        app_handle = live_handle.app if live_handle is not None else None
        for child in old_children:
            if app_handle is not None:
                app_handle.unregister_widget_callbacks(child)
            child.parent = None
            if live_handle is not None:
                for widget in _walk_widget_tree(child):
                    widget._unbind_live()
        self.children = []
        for child in new_children:
            self.add(child)

        live_handle = self._live()
        if live_handle is not None:
            app_handle = live_handle.app
            for child in new_children:
                for widget in _walk_widget_tree(child):
                    widget._bind_live(app_handle.widget_handle(widget.id))
                app_handle.register_widget_callbacks(child)
            live_handle.enqueue_replace_children([child.to_dict() for child in self.children])
            for child in new_children:
                for widget in _walk_widget_tree(child):
                    widget._queue_startup_resources()

    def __enter__(self) -> Self:
        _BuildContext.push(self)
        return self

    def __exit__(self, exc_type: object, exc: object, tb: object) -> None:
        _BuildContext.pop(self)

    def to_dict(self) -> dict[str, Any]:
        data = super().to_dict()
        data["children"] = [child.to_dict() for child in self.children]
        return data


class Window(Container):
    kind = "window"

    def __init__(
        self,
        title: str,
        *,
        width: int = 1024,
        height: int = 768,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
    ) -> None:
        if _BuildContext.stack:
            raise RuntimeError("cannot create a Window while a layout context is active")
        self.title = title
        self.width = width
        self.height = height
        _BuildContext.stack = []
        super().__init__(id=id, key=key, class_=class_, style=style, parent=None)
        _BuildContext.root = self

    def props(self) -> dict[str, Any]:
        return {
            "title": self.title,
            "width": self.width,
            "height": self.height,
        }


class HLayout(Container):
    kind = "h_layout"


class VLayout(Container):
    kind = "v_layout"


class Separator(Widget):
    kind = "separator"

    def __init__(
        self,
        *,
        orientation: str = "auto",
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if orientation not in {"auto", "horizontal", "vertical"}:
            raise ValueError("Separator orientation must be 'auto', 'horizontal', or 'vertical'")
        self.orientation = orientation
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def props(self) -> dict[str, Any]:
        return {"orientation": self.orientation}


class Spacer(Widget):
    kind = "spacer"

    def __init__(
        self,
        *,
        width: int | float | None = None,
        height: int | float | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if width is not None and float(width) < 0:
            raise ValueError("Spacer width cannot be negative")
        if height is not None and float(height) < 0:
            raise ValueError("Spacer height cannot be negative")
        self.width = None if width is None else float(width)
        self.height = None if height is None else float(height)
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "width": self.width,
            "height": self.height,
        }


class StatusBar(Container):
    kind = "status_bar"

    def __init__(
        self,
        *,
        height: int | float = 28,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if float(height) <= 0:
            raise ValueError("StatusBar height must be greater than zero")
        self.height = float(height)
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def props(self) -> dict[str, Any]:
        return {"height": self.height}


class Tabs(Container):
    kind = "tabs"

    def __init__(
        self,
        *,
        value: str | None = None,
        on_change: StringCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = str(value) if value is not None else None
        self.on_change = on_change
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def add(self, child: Widget) -> Widget:
        if not isinstance(child, Tab):
            raise TypeError("Tabs can only contain Tab children")
        if any(isinstance(existing, Tab) and existing.value == child.value for existing in self.children):
            raise ValueError(f"duplicate Tab value: {child.value!r}")
        if self.value is None:
            self.value = child.value
        return super().add(child)

    def props(self) -> dict[str, Any]:
        return {
            "value": self.value,
            "disabled": self.disabled,
            "events": ["change"] if self.on_change and not self.disabled else [],
        }


class Tab(Container):
    kind = "tab"

    def __init__(
        self,
        label: str,
        *,
        value: str | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.label = str(label)
        self.value = str(value) if value is not None else _route_value(self.label)
        if not self.value:
            raise ValueError("Tab value cannot be empty")
        self.disabled = disabled
        actual_parent = _BuildContext.parent() if parent is _AUTO_PARENT else parent
        if actual_parent is not None and not isinstance(actual_parent, Tabs):
            raise RuntimeError("Tab must be created directly inside a Tabs context")
        super().__init__(id=id, key=key, class_=class_, style=style, parent=actual_parent)

    def props(self) -> dict[str, Any]:
        return {
            "label": self.label,
            "value": self.value,
            "disabled": self.disabled,
        }


class Pages(Container):
    kind = "pages"

    def __init__(
        self,
        *,
        value: str | None = None,
        on_change: StringCallback | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = str(value) if value is not None else None
        self.on_change = on_change
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def add(self, child: Widget) -> Widget:
        if not isinstance(child, Page):
            raise TypeError("Pages can only contain Page children")
        if any(isinstance(existing, Page) and existing.value == child.value for existing in self.children):
            raise ValueError(f"duplicate Page value: {child.value!r}")
        if self.value is None:
            self.value = child.value
        return super().add(child)

    def props(self) -> dict[str, Any]:
        return {
            "value": self.value,
            "events": ["change"] if self.on_change else [],
        }


class Page(Container):
    kind = "page"

    def __init__(
        self,
        value: str,
        *,
        title: str | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = str(value)
        if not self.value:
            raise ValueError("Page value cannot be empty")
        self.title = title
        actual_parent = _BuildContext.parent() if parent is _AUTO_PARENT else parent
        if actual_parent is not None and not isinstance(actual_parent, Pages):
            raise RuntimeError("Page must be created directly inside a Pages context")
        super().__init__(id=id, key=key, class_=class_, style=style, parent=actual_parent)

    def props(self) -> dict[str, Any]:
        return {
            "value": self.value,
            "title": self.title,
        }


class Sidebar(Container):
    kind = "sidebar"

    def __init__(
        self,
        *,
        title: str | None = None,
        width: int = 220,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if width <= 0:
            raise ValueError("Sidebar width must be greater than zero")
        self.title = title
        self.width = int(width)
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "title": self.title,
            "width": self.width,
        }


class NavItem(Widget):
    kind = "nav_item"

    def __init__(
        self,
        label: str,
        *,
        page: str,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.label = str(label)
        self.page = str(page)
        if not self.page:
            raise ValueError("NavItem page cannot be empty")
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "label": self.label,
            "page": self.page,
            "disabled": self.disabled,
        }


class Panel(Container):
    kind = "panel"

    def __init__(
        self,
        title: str | None = None,
        *,
        width: int | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.title = title
        self.width = width
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "title": self.title,
            "width": self.width,
        }


class Label(Widget):
    kind = "label"

    def __init__(
        self,
        text: str,
        *,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.text = text
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def props(self) -> dict[str, Any]:
        return {"text": self.text}


class Button(Widget):
    kind = "button"

    def __init__(
        self,
        text: str,
        *,
        on_click: Callback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.text = text
        self.on_click = on_click
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def click(self) -> None:
        if not self.disabled and self.on_click is not None:
            self.on_click()

    def props(self) -> dict[str, Any]:
        return {
            "text": self.text,
            "disabled": self.disabled,
            "events": ["click"] if self.on_click and not self.disabled else [],
        }


class TextInput(Widget):
    kind = "text_input"

    def __init__(
        self,
        value: str = "",
        *,
        placeholder: str = "",
        on_change: StringCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = value
        self.placeholder = placeholder
        self.on_change = on_change
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def set_value(self, value: str) -> None:
        self.value = str(value)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("value", self.value)

    def props(self) -> dict[str, Any]:
        return {
            "value": self.value,
            "placeholder": self.placeholder,
            "disabled": self.disabled,
        }


class Slider(Widget):
    kind = "slider"

    def __init__(
        self,
        value: float = 0,
        *,
        min: float = 0,
        max: float = 1,
        step: float = 0.01,
        on_change: "FloatCallback | None" = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        min_value = float(min)
        max_value = float(max)
        if max_value < min_value:
            raise ValueError("Slider max must be greater than or equal to min")
        step_value = float(step)
        if step_value <= 0:
            raise ValueError("Slider step must be greater than zero")
        value_f = float(value)
        self.value = value_f
        if self.value < min_value:
            self.value = min_value
        elif self.value > max_value:
            self.value = max_value
        self.min = min_value
        self.max = max_value
        self.step = step_value
        self.on_change = on_change
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def _clamp_value(self, value: float) -> float:
        value_f = float(value)
        if value_f < self.min:
            return self.min
        if value_f > self.max:
            return self.max
        return value_f

    def set_value(self, value: float) -> None:
        self.value = self._clamp_value(float(value))
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("value", self.value)

    def props(self) -> dict[str, Any]:
        return {
            "value": self.value,
            "min": self.min,
            "max": self.max,
            "step": self.step,
            "disabled": self.disabled,
        }


class Dropdown(Widget):
    kind = "dropdown"

    def __init__(
        self,
        items: Iterable[str],
        *,
        value: str | None = None,
        on_change: StringCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.items = [str(item) for item in items]
        if not self.items:
            raise ValueError("Dropdown items cannot be empty")
        selected = self.items[0] if value is None else str(value)
        if selected not in self.items:
            raise ValueError("Dropdown value must be one of its items")
        self.value = selected
        self.on_change = on_change
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def set_value(self, value: str) -> None:
        selected = str(value)
        if selected not in self.items:
            raise ValueError("Dropdown value must be one of its items")
        self.value = selected
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("value", self.value)

    def props(self) -> dict[str, Any]:
        return {
            "items": self.items,
            "value": self.value,
            "disabled": self.disabled,
        }


class Checkbox(Widget):
    kind = "checkbox"

    def __init__(
        self,
        label: str,
        *,
        checked: bool = False,
        on_change: "BoolCallback | None" = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.label = label
        self.checked = checked
        self.on_change = on_change
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def set_checked(self, checked: bool) -> None:
        self.checked = bool(checked)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("checked", self.checked)

    def props(self) -> dict[str, Any]:
        return {
            "label": self.label,
            "checked": self.checked,
            "disabled": self.disabled,
        }


class Scatter3D(Widget):
    kind = "scatter_3d"

    def __init__(
        self,
        frame: Any = None,
        *,
        x: str,
        y: str,
        z: str,
        colormap: str = "viridis",
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.frame = frame
        self.x = x
        self.y = y
        self.z = z
        self.colormap = _scatter_colormap(colormap)
        self.frame_summary = summarize_frame(frame)
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def set_points(self, frame: Any, *, x: str, y: str | None = None, z: str | None = None) -> None:
        self.frame = frame
        self.x = x
        self.y = y if y is not None else self.y
        self.z = z if z is not None else self.z
        self.frame_summary = summarize_frame(frame)
        if (handle := self._live()) is not None:
            t0 = time.perf_counter()
            payload = _pack_xyz_bytes(self.frame, self.x, self.y, self.z)
            pack_ms = (time.perf_counter() - t0) * 1000.0
            if payload is None:
                raise RuntimeError(
                    "live Scatter3D.set_points requires NumPy and addressable numeric x/y/z columns"
                )
            handle.enqueue_set_scatter_points_packed(
                payload,
                pack_ms=pack_ms,
                enqueue_epoch_ms=time.time() * 1000.0,
                colormap=self.colormap,
            )

    def set_colormap(self, colormap: str) -> None:
        self.colormap = _scatter_colormap(colormap)
        if (handle := self._live()) is not None:
            t0 = time.perf_counter()
            payload = _pack_xyz_bytes(self.frame, self.x, self.y, self.z)
            pack_ms = (time.perf_counter() - t0) * 1000.0
            if payload is None:
                raise RuntimeError(
                    "live Scatter3D.set_colormap requires NumPy and addressable numeric x/y/z columns"
                )
            handle.enqueue_set_scatter_points_packed(
                payload,
                pack_ms=pack_ms,
                enqueue_epoch_ms=time.time() * 1000.0,
                colormap=self.colormap,
            )

    def props(self) -> dict[str, Any]:
        return {
            "frame": self.frame_summary.to_dict(),
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "colormap": self.colormap,
            # Packed float32 xyz triples (little-endian), base64-encoded.
            # None when frame has no addressable array attributes (mock frames,
            # dev-fallback mode, or numpy unavailable).
            "data_b64": _try_pack_xyz(self.frame, self.x, self.y, self.z),
        }


class DataFrameTable(Widget):
    kind = "dataframe_table"

    def __init__(
        self,
        frame: Any,
        *,
        page_size: int = 100,
        sample_rows: int = DEFAULT_TABLE_SAMPLE_ROWS,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        page_size_i = int(page_size)
        sample_rows_i = int(sample_rows)
        if page_size_i <= 0:
            raise ValueError("DataFrameTable page_size must be greater than zero")
        if sample_rows_i < 0:
            raise ValueError("DataFrameTable sample_rows cannot be negative")
        self.frame = frame
        self.page_size = page_size_i
        self.sample_rows = sample_rows_i
        self.frame_summary = summarize_frame(frame)
        self.cells = extract_table_sample(frame, self.frame_summary, self.sample_rows)
        self.column_buffers = extract_table_column_buffers(frame, self.frame_summary)
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)
        self.resource_id = f"{self.id}:table"

    def set_frame(self, frame: Any, *, sample_rows: int | None = None) -> None:
        sample_rows_i = self.sample_rows if sample_rows is None else int(sample_rows)
        if sample_rows_i < 0:
            raise ValueError("DataFrameTable sample_rows cannot be negative")
        self.frame = frame
        self.sample_rows = sample_rows_i
        self.frame_summary = summarize_frame(frame)
        self.cells = extract_table_sample(frame, self.frame_summary, self.sample_rows)
        self.column_buffers = extract_table_column_buffers(frame, self.frame_summary)
        if (handle := self._live()) is not None:
            if self.column_buffers:
                handle.enqueue_set_table_data_columns(
                    self._table_payload(),
                    self.column_buffers,
                )
            else:
                handle.enqueue_set_table_data(self._table_payload())

    def _queue_startup_resources(self) -> None:
        if (handle := self._live()) is not None and self.column_buffers:
            handle.enqueue_set_table_data_columns(
                self._table_payload(),
                self.column_buffers,
            )

    def _sync_after_id_change(self, old_id: str) -> None:
        if self.resource_id == f"{old_id}:table":
            self.resource_id = f"{self.id}:table"

    def _table_payload(self) -> dict[str, Any]:
        return {
            "frame": self.frame_summary.to_dict(),
            "resource_id": self.resource_id,
            "resource_ref": id(self.frame),
            "page_size": self.page_size,
            "virtualized": True,
            "sample_rows": self.sample_rows,
            "buffer_columns": len(self.column_buffers),
            "cells": self.cells,
        }

    def props(self) -> dict[str, Any]:
        return self._table_payload()
