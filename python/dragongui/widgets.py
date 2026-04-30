from __future__ import annotations

import base64
from collections.abc import Callable, Iterable, Mapping, Sequence
from contextlib import AbstractContextManager
from dataclasses import dataclass
from itertools import count
import math
import numbers
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


def _format_number(value: float) -> str:
    text = f"{float(value):.12g}"
    return "0" if text == "-0" else text


def _normalize_color_tuple(value: Sequence[object], *, alpha: bool) -> tuple[int, ...]:
    if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
        raise TypeError("ColorPicker value must be a sequence of RGB or RGBA values")
    if len(value) not in {3, 4}:
        raise ValueError("ColorPicker value must contain 3 RGB or 4 RGBA channels")

    raw_channels = list(value)
    channels = [float(channel) for channel in raw_channels]
    if not all(math.isfinite(channel) for channel in channels):
        raise ValueError("ColorPicker channels must be finite numbers")

    normalized_input = all(0.0 <= channel <= 1.0 for channel in channels) and any(
        not isinstance(channel, numbers.Integral) for channel in raw_channels
    )
    if normalized_input:
        channels = [channel * 255.0 for channel in channels]
    if len(channels) == 3 and alpha:
        channels.append(255.0)
    elif len(channels) == 4 and not alpha:
        channels = channels[:3]
    return tuple(max(0, min(255, int(round(channel)))) for channel in channels)


def _color_hex(value: Sequence[int]) -> str:
    r, g, b = (max(0, min(255, int(channel))) for channel in value[:3])
    return f"#{r:02x}{g:02x}{b:02x}"


_SUPPORTED_PARTS_BY_KIND: dict[str, set[str]] = {
    "h_layout": {"scrollbar-track", "scrollbar-thumb"},
    "v_layout": {"scrollbar-track", "scrollbar-thumb"},
    "pages": {"scrollbar-track", "scrollbar-thumb"},
    "page": {"scrollbar-track", "scrollbar-thumb"},
    "sidebar": {"scrollbar-track", "scrollbar-thumb"},
    "panel": {"accent", "scrollbar-track", "scrollbar-thumb"},
    "collapsible": {
        "header",
        "indicator",
        "body",
        "scrollbar-track",
        "scrollbar-thumb",
    },
    "modal": {"scrim", "scrollbar-track", "scrollbar-thumb"},
    "button": {"badge"},
    "number_input": {
        "field",
        "stepper",
        "stepper-up",
        "stepper-down",
        "stepper-divider",
        "divider",
        "caret",
    },
    "dropdown": {"field", "chevron", "menu", "item", "item-selected", "item-hover"},
    "checkbox": {"row", "box", "indicator", "label"},
    "slider": {"track", "fill", "thumb"},
    "progress_bar": {"track", "fill", "label"},
    "tabs": {"header"},
    "tab": {"tab", "accent", "badge"},
    "nav_item": {"item", "accent", "badge"},
    "dataframe_table": {"header", "row", "row-selected", "grid-line"},
}

_BADGE_LEVELS = {"neutral", "info", "success", "warning", "danger", "error"}


def _normalize_part_name(name: object) -> str:
    if not isinstance(name, str):
        raise TypeError("inline style part names must be strings")
    normalized = name.strip().replace("_", "-").lower()
    if not normalized:
        raise ValueError("inline style part names must be non-empty")
    return normalized


def _validate_style_parts(style: Mapping[str, object], widget_kind: str) -> None:
    if "parts" not in style:
        return
    parts = style["parts"]
    if not isinstance(parts, Mapping):
        raise TypeError("style['parts'] must be a mapping")
    supported = _SUPPORTED_PARTS_BY_KIND.get(widget_kind, set())
    for raw_name, part_style in parts.items():
        name = _normalize_part_name(raw_name)
        if name not in supported:
            widget = widget_kind.replace("_", " ").title().replace(" ", "")
            allowed = ", ".join(sorted(supported)) or "none"
            raise ValueError(
                f"{widget} has no CSS part {name!r}; supported parts: {allowed}"
            )
        if not isinstance(part_style, Mapping):
            raise TypeError(f"style['parts'][{raw_name!r}] must be a mapping")


def _copy_style(
    style: Mapping[str, object] | None,
    *,
    widget_kind: str,
) -> dict[str, object] | None:
    if style is None:
        return None
    if not isinstance(style, Mapping):
        raise TypeError("widget style must be a mapping")
    _validate_style_parts(style, widget_kind)
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
ColorCallback = Callable[[tuple[int, ...]], None]
BadgeValue = str | int | None


@dataclass(frozen=True)
class TableSelection:
    row_index: int
    column_index: int
    column: str
    value: object


TableSelectCallback = Callable[[TableSelection], None]


@dataclass(frozen=True)
class ScatterPick:
    index: int
    x: float
    y: float
    z: float


ScatterPickCallback = Callable[[ScatterPick], None]

_ids = count(1)
_AUTO_PARENT = object()


def _route_value(label: str) -> str:
    value = re.sub(r"[^a-z0-9]+", "_", label.lower()).strip("_")
    return value or "page"


def _badge_value(value: BadgeValue) -> str | None:
    if value is None:
        return None
    if isinstance(value, bool):
        raise TypeError("badge must be a str, int, or None")
    if isinstance(value, int):
        return str(value)
    if isinstance(value, str):
        return value
    raise TypeError("badge must be a str, int, or None")


def _badge_level(value: str) -> str:
    if not isinstance(value, str):
        raise TypeError("badge level must be a string")
    level = value.strip().lower()
    if level not in _BADGE_LEVELS:
        allowed = ", ".join(sorted(_BADGE_LEVELS))
        raise ValueError(f"unknown badge level {value!r}; expected one of: {allowed}")
    return level


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
        tooltip: str | None = None,
        parent: "Container | None | object" = _AUTO_PARENT,
    ) -> None:
        if key is not None and (not isinstance(key, str) or not key):
            raise ValueError("widget key must be a non-empty string")
        if class_ is not None and (not isinstance(class_, str) or not class_):
            raise ValueError("widget class_ must be a non-empty string")
        self.id = id or f"dg-{next(_ids)}"
        self.key = key
        self.class_ = class_
        self.style = _copy_style(style, widget_kind=self.kind)
        self.tooltip = None if tooltip is None else str(tooltip)
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
        new_style = _copy_style(style, widget_kind=self.kind)
        patch = _style_patch(self.style, new_style)
        self.style = new_style
        handle = self._live()
        if handle is not None and patch:
            handle.enqueue_set_style(patch)

    def to_dict(self) -> dict[str, Any]:
        props = self.props()
        if self.tooltip:
            props = dict(props)
            props["tooltip"] = self.tooltip
        data = {
            "id": self.id,
            "type": self.kind,
            "props": props,
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
        tooltip: str | None = None,
        parent: "Container | None | object" = _AUTO_PARENT,
    ) -> None:
        self.children: list[Widget] = []
        super().__init__(
            id=id,
            key=key,
            class_=class_,
            style=style,
            tooltip=tooltip,
            parent=parent,
        )

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
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if orientation not in {"auto", "horizontal", "vertical"}:
            raise ValueError("Separator orientation must be 'auto', 'horizontal', or 'vertical'")
        self.orientation = orientation
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

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
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if width is not None and float(width) < 0:
            raise ValueError("Spacer width cannot be negative")
        if height is not None and float(height) < 0:
            raise ValueError("Spacer height cannot be negative")
        self.width = None if width is None else float(width)
        self.height = None if height is None else float(height)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

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
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if float(height) <= 0:
            raise ValueError("StatusBar height must be greater than zero")
        self.height = float(height)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def props(self) -> dict[str, Any]:
        return {"height": self.height}


class MenuBar(Container):
    kind = "menu_bar"

    def __init__(
        self,
        *,
        height: int | float = 34,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if float(height) <= 0:
            raise ValueError("MenuBar height must be greater than zero")
        self.height = float(height)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def add(self, child: Widget) -> Widget:
        if not isinstance(child, Menu):
            raise TypeError("MenuBar can only contain Menu children")
        return super().add(child)

    def props(self) -> dict[str, Any]:
        return {"height": self.height}


class Menu(Container):
    kind = "menu"

    def __init__(
        self,
        label: str,
        *,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.label = str(label)
        if not self.label:
            raise ValueError("Menu label cannot be empty")
        self.disabled = bool(disabled)
        actual_parent = _BuildContext.parent() if parent is _AUTO_PARENT else parent
        if actual_parent is not None and not isinstance(actual_parent, MenuBar):
            raise RuntimeError("Menu must be created directly inside a MenuBar context")
        super().__init__(
            id=id,
            key=key,
            class_=class_,
            style=style,
            tooltip=tooltip,
            parent=actual_parent,
        )

    def add(self, child: Widget) -> Widget:
        if not isinstance(child, MenuItem):
            raise TypeError("Menu can only contain MenuItem children")
        return super().add(child)

    def props(self) -> dict[str, Any]:
        return {"label": self.label, "disabled": self.disabled}


class MenuItem(Widget):
    kind = "menu_item"

    def __init__(
        self,
        label: str,
        *,
        on_click: Callback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.label = str(label)
        if not self.label:
            raise ValueError("MenuItem label cannot be empty")
        self.on_click = on_click
        self.disabled = bool(disabled)
        actual_parent = _BuildContext.parent() if parent is _AUTO_PARENT else parent
        if actual_parent is not None and not isinstance(actual_parent, (Menu, ContextMenu)):
            raise RuntimeError("MenuItem must be created inside a Menu or ContextMenu context")
        super().__init__(
            id=id,
            key=key,
            class_=class_,
            style=style,
            tooltip=tooltip,
            parent=actual_parent,
        )

    def props(self) -> dict[str, Any]:
        return {
            "label": self.label,
            "disabled": self.disabled,
            "events": ["click"] if self.on_click and not self.disabled else [],
        }


class ContextMenu(Container):
    kind = "context_menu"

    def __init__(
        self,
        *,
        target: Widget | str | None = None,
        width: int | float = 220,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if float(width) <= 0:
            raise ValueError("ContextMenu width must be greater than zero")
        if isinstance(target, Widget):
            self.target = target.id
        elif target is None:
            self.target = None
        else:
            self.target = str(target)
            if not self.target:
                raise ValueError("ContextMenu target cannot be empty")
        self.width = float(width)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def add(self, child: Widget) -> Widget:
        if not isinstance(child, MenuItem):
            raise TypeError("ContextMenu can only contain MenuItem children")
        return super().add(child)

    def props(self) -> dict[str, Any]:
        return {"target": self.target, "width": self.width}


class Tooltip(Container):
    kind = "tooltip"

    def __init__(
        self,
        *,
        target: Widget | str,
        width: int | float = 280,
        height: int | float | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if isinstance(target, Widget):
            self.target = target.id
        else:
            self.target = str(target)
        if not self.target:
            raise ValueError("Tooltip target cannot be empty")
        self.width = float(width)
        if self.width <= 0:
            raise ValueError("Tooltip width must be greater than zero")
        self.height = None if height is None else float(height)
        if self.height is not None and self.height <= 0:
            raise ValueError("Tooltip height must be greater than zero")
        super().__init__(id=id, key=key, class_=class_, style=style, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "target": self.target,
            "width": self.width,
            "height": self.height,
        }


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
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = str(value) if value is not None else None
        self.on_change = on_change
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

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

    def set_value(self, value: str, *, notify: bool = False) -> None:
        selected = str(value)
        if not selected:
            raise ValueError("Tabs value cannot be empty")
        values = {child.value for child in self.children if isinstance(child, Tab)}
        if values and selected not in values:
            raise ValueError("Tabs value must match one of its Tab children")
        self.value = selected
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("value", self.value)
        if notify and self.on_change is not None:
            self.on_change(self.value)


class Tab(Container):
    kind = "tab"

    def __init__(
        self,
        label: str,
        *,
        value: str | None = None,
        badge: BadgeValue = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.label = str(label)
        self.value = str(value) if value is not None else _route_value(self.label)
        if not self.value:
            raise ValueError("Tab value cannot be empty")
        self.badge = _badge_value(badge)
        self.disabled = disabled
        actual_parent = _BuildContext.parent() if parent is _AUTO_PARENT else parent
        if actual_parent is not None and not isinstance(actual_parent, Tabs):
            raise RuntimeError("Tab must be created directly inside a Tabs context")
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=actual_parent)

    def props(self) -> dict[str, Any]:
        return {
            "label": self.label,
            "value": self.value,
            "badge": self.badge,
            "disabled": self.disabled,
        }

    def set_badge(self, value: BadgeValue) -> None:
        self.badge = _badge_value(value)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("badge", self.badge)


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
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = str(value) if value is not None else None
        self.on_change = on_change
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

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

    def set_value(self, value: str, *, notify: bool = False) -> None:
        selected = str(value)
        if not selected:
            raise ValueError("Pages value cannot be empty")
        values = {child.value for child in self.children if isinstance(child, Page)}
        if values and selected not in values:
            raise ValueError("Pages value must match one of its Page children")
        self.value = selected
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("value", self.value)
        if notify and self.on_change is not None:
            self.on_change(self.value)


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
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = str(value)
        if not self.value:
            raise ValueError("Page value cannot be empty")
        self.title = title
        actual_parent = _BuildContext.parent() if parent is _AUTO_PARENT else parent
        if actual_parent is not None and not isinstance(actual_parent, Pages):
            raise RuntimeError("Page must be created directly inside a Pages context")
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=actual_parent)

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
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if width <= 0:
            raise ValueError("Sidebar width must be greater than zero")
        self.title = title
        self.width = int(width)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

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
        badge: BadgeValue = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.label = str(label)
        self.page = str(page)
        if not self.page:
            raise ValueError("NavItem page cannot be empty")
        self.badge = _badge_value(badge)
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "label": self.label,
            "page": self.page,
            "badge": self.badge,
            "disabled": self.disabled,
        }

    def set_badge(self, value: BadgeValue) -> None:
        self.badge = _badge_value(value)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("badge", self.badge)


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
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.title = title
        self.width = width
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "title": self.title,
            "width": self.width,
        }


class Collapsible(Container):
    kind = "collapsible"

    def __init__(
        self,
        title: str,
        *,
        expanded: bool = True,
        on_change: BoolCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.title = str(title)
        if not self.title:
            raise ValueError("Collapsible title cannot be empty")
        self.expanded = bool(expanded)
        self.on_change = on_change
        self.disabled = bool(disabled)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def set_expanded(self, expanded: bool) -> None:
        self.expanded = bool(expanded)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("expanded", self.expanded)

    def expand(self) -> None:
        self.set_expanded(True)

    def collapse(self) -> None:
        self.set_expanded(False)

    def toggle(self) -> None:
        self.set_expanded(not self.expanded)

    def props(self) -> dict[str, Any]:
        return {
            "title": self.title,
            "expanded": self.expanded,
            "disabled": self.disabled,
            "events": ["change"] if self.on_change and not self.disabled else [],
        }


class Modal(Container):
    kind = "modal"

    def __init__(
        self,
        title: str = "",
        *,
        open: bool = False,
        width: int | float = 420,
        height: int | float = 220,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if float(width) <= 0 or float(height) <= 0:
            raise ValueError("Modal width and height must be greater than zero")
        self.title = title
        self.open = bool(open)
        self.width = float(width)
        self.height = float(height)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def set_open(self, open: bool) -> None:
        self.open = bool(open)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("open", self.open)

    def show(self) -> None:
        self.set_open(True)

    def close(self) -> None:
        self.set_open(False)

    def props(self) -> dict[str, Any]:
        return {
            "title": self.title,
            "open": self.open,
            "width": self.width,
            "height": self.height,
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
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.text = text
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def props(self) -> dict[str, Any]:
        return {"text": self.text}

    def set_value(self, value: object) -> None:
        self.text = str(value)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("text", self.text)


class Badge(Widget):
    kind = "badge"

    def __init__(
        self,
        text: BadgeValue,
        *,
        level: str = "info",
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        value = _badge_value(text)
        if value is None:
            raise ValueError("Badge text cannot be None")
        self.text = value
        self.level = _badge_level(level)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def props(self) -> dict[str, Any]:
        return {"text": self.text, "level": self.level}

    def set_value(self, value: BadgeValue) -> None:
        text = _badge_value(value)
        if text is None:
            raise ValueError("Badge text cannot be None")
        self.text = text
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("text", self.text)

    def set_level(self, level: str) -> None:
        self.level = _badge_level(level)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("level", self.level)


class Tag(Badge):
    kind = "tag"

    def __init__(
        self,
        text: BadgeValue,
        *,
        level: str = "neutral",
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        super().__init__(
            text,
            level=level,
            id=id,
            key=key,
            class_=class_,
            style=style,
            tooltip=tooltip,
            parent=parent,
        )


class Button(Widget):
    kind = "button"

    def __init__(
        self,
        text: str,
        *,
        on_click: Callback | None = None,
        badge: BadgeValue = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.text = text
        self.on_click = on_click
        self.badge = _badge_value(badge)
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def click(self) -> None:
        if not self.disabled and self.on_click is not None:
            self.on_click()

    def props(self) -> dict[str, Any]:
        return {
            "text": self.text,
            "badge": self.badge,
            "disabled": self.disabled,
            "events": ["click"] if self.on_click and not self.disabled else [],
        }

    def set_badge(self, value: BadgeValue) -> None:
        self.badge = _badge_value(value)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("badge", self.badge)


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
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = value
        self.placeholder = placeholder
        self.on_change = on_change
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

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


class TextArea(Widget):
    kind = "text_area"

    def __init__(
        self,
        value: str = "",
        *,
        placeholder: str = "",
        rows: int = 4,
        wrap: bool = True,
        on_change: StringCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        rows_i = int(rows)
        if rows_i < 1:
            raise ValueError("TextArea rows must be at least 1")
        self.value = str(value)
        self.placeholder = str(placeholder)
        self.rows = rows_i
        self.wrap = bool(wrap)
        self.on_change = on_change
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def set_value(self, value: str) -> None:
        self.value = str(value)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("value", self.value)

    def props(self) -> dict[str, Any]:
        return {
            "value": self.value,
            "placeholder": self.placeholder,
            "rows": self.rows,
            "wrap": self.wrap,
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
        tooltip: str | None = None,
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
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

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


class ProgressBar(Widget):
    kind = "progress_bar"

    def __init__(
        self,
        value: float = 0,
        *,
        min: float = 0,
        max: float = 1,
        label: str | None = None,
        show_value: bool = False,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        min_value = float(min)
        max_value = float(max)
        if max_value < min_value:
            raise ValueError("ProgressBar max must be greater than or equal to min")
        self.min = min_value
        self.max = max_value
        self.value = self._clamp_value(float(value))
        self.label = None if label is None else str(label)
        self.show_value = bool(show_value)
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def _clamp_value(self, value: float) -> float:
        if value < self.min:
            return self.min
        if value > self.max:
            return self.max
        return value

    def _display_label(self) -> str | None:
        if self.label is not None:
            return self.label
        if not self.show_value:
            return None
        span = self.max - self.min
        t = 0.0 if span <= 0 else (self.value - self.min) / span
        return f"{round(t * 100):.0f}%"

    def set_value(self, value: float) -> None:
        old_label = self._display_label()
        self.value = self._clamp_value(float(value))
        new_label = self._display_label()
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("value", self.value)
            if old_label != new_label:
                handle.enqueue_set_prop("label", new_label)

    def props(self) -> dict[str, Any]:
        return {
            "value": self.value,
            "min": self.min,
            "max": self.max,
            "label": self._display_label(),
            "disabled": self.disabled,
        }


class NumberInput(Widget):
    kind = "number_input"

    def __init__(
        self,
        value: float = 0,
        *,
        min: float | None = None,
        max: float | None = None,
        step: float = 1,
        on_change: FloatCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        min_value = None if min is None else float(min)
        max_value = None if max is None else float(max)
        if min_value is not None and not math.isfinite(min_value):
            raise ValueError("NumberInput min must be finite")
        if max_value is not None and not math.isfinite(max_value):
            raise ValueError("NumberInput max must be finite")
        if min_value is not None and max_value is not None and max_value < min_value:
            raise ValueError("NumberInput max must be greater than or equal to min")
        step_value = float(step)
        if step_value <= 0 or not math.isfinite(step_value):
            raise ValueError("NumberInput step must be greater than zero")
        self.min = min_value
        self.max = max_value
        self.step = step_value
        value_f = float(value)
        if not math.isfinite(value_f):
            raise ValueError("NumberInput value must be finite")
        self.value = self._clamp_value(value_f)
        self.on_change = on_change
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def _clamp_value(self, value: float) -> float:
        if self.min is not None and value < self.min:
            return self.min
        if self.max is not None and value > self.max:
            return self.max
        return value

    def set_value(self, value: float) -> None:
        value_f = float(value)
        if not math.isfinite(value_f):
            raise ValueError("NumberInput value must be finite")
        self.value = self._clamp_value(value_f)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("value", self.value)

    def props(self) -> dict[str, Any]:
        return {
            "value": self.value,
            "min": self.min,
            "max": self.max,
            "step": self.step,
            "text": _format_number(self.value),
            "disabled": self.disabled,
            "events": ["change"] if self.on_change and not self.disabled else [],
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
        tooltip: str | None = None,
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
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

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
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.label = label
        self.checked = checked
        self.on_change = on_change
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

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


class ColorPicker(Panel):
    """Composite RGB/RGBA color picker built from DragonGUI controls.

    Integer channels are treated as 0..255 values. Floating-point channels in
    the 0.0..1.0 range are treated as normalized colors. The ``width`` argument
    is treated as a preferred maximum width so the picker can shrink inside
    narrow parent panels instead of overflowing them.
    """

    _CHANNEL_INDEX = {"r": 0, "g": 1, "b": 2, "a": 3}

    def __init__(
        self,
        value: Sequence[object] = (255, 100, 0),
        *,
        alpha: bool = True,
        on_change: ColorCallback | None = None,
        title: str | None = "Color",
        width: int | None = 320,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.alpha = bool(alpha)
        self.value = _normalize_color_tuple(value, alpha=self.alpha)
        self.on_change = on_change
        self._sliders: dict[str, Slider] = {}
        self._value_labels: dict[str, Label] = {}
        base_style: dict[str, object] = {
            "padding": 14,
            "gap": 6,
            "flex_grow": 0,
            "flex_shrink": 1,
        }
        if width is not None:
            base_style["max_width"] = int(width)
        if style is not None:
            base_style.update(_copy_style(style, widget_kind=self.kind) or {})
        super().__init__(
            title,
            width=None,
            id=id,
            key=key,
            class_=class_,
            style=base_style,
            tooltip=tooltip,
            parent=parent,
        )

        with self:
            self._swatch = Button(
                " ",
                disabled=True,
                style={
                    "height": 36,
                    "background": _color_hex(self.value),
                    "border_color": "border",
                    "border_width": 1,
                    "disabled": {"opacity": 1.0},
                },
                tooltip="Current color preview",
            )
            self._add_channel("r", "R")
            self._add_channel("g", "G")
            self._add_channel("b", "B")
            if self.alpha:
                self._add_channel("a", "A")

    def _add_channel(self, channel: str, label: str) -> None:
        value = self.value[self._CHANNEL_INDEX[channel]]
        with HLayout(style={"height": 32, "gap": 4}):
            Label(
                label,
                style={
                    "width": 26,
                    "height": 32,
                    "color": "text",
                    "font_weight": 700,
                    "text_align": "center",
                },
            )
            slider = Slider(
                value,
                min=0,
                max=255,
                step=1,
                on_change=lambda new_value, ch=channel: self._set_channel(ch, new_value),
                style={"flex": 1},
            )
            value_label = Label(str(value), style={"width": 38, "text_align": "right"})
        self._sliders[channel] = slider
        self._value_labels[channel] = value_label

    def _set_swatch_color(self) -> None:
        style = dict(self._swatch.style or {})
        style["background"] = _color_hex(self.value)
        self._swatch.set_style(style)

    def _set_channel(self, channel: str, value: float) -> None:
        channels = list(self.value)
        channels[self._CHANNEL_INDEX[channel]] = max(0, min(255, int(round(float(value)))))
        self.value = tuple(channels)
        self._value_labels[channel].set_value(str(self.value[self._CHANNEL_INDEX[channel]]))
        self._set_swatch_color()
        if self.on_change is not None:
            self.on_change(self.value)

    def set_value(self, value: Sequence[object], *, notify: bool = False) -> None:
        """Update the displayed color.

        By default this preserves the historical programmatic behavior and does
        not invoke ``on_change``. Pass ``notify=True`` to call ``on_change``
        after the internal sliders, labels, and swatch have been updated.
        """
        self.value = _normalize_color_tuple(value, alpha=self.alpha)
        for channel, slider in self._sliders.items():
            channel_value = self.value[self._CHANNEL_INDEX[channel]]
            slider.set_value(channel_value)
            self._value_labels[channel].set_value(str(channel_value))
        self._set_swatch_color()
        if notify and self.on_change is not None:
            self.on_change(self.value)


class Image(Widget):
    kind = "image"

    def __init__(
        self,
        path: object,
        *,
        fit: str = "contain",
        width: int | float | None = None,
        height: int | float | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.path = self._normalize_path(path)
        self.fit = self._normalize_fit(fit)
        self.width = None if width is None else float(width)
        self.height = None if height is None else float(height)
        if self.width is not None and self.width <= 0:
            raise ValueError("Image width must be greater than zero")
        if self.height is not None and self.height <= 0:
            raise ValueError("Image height must be greater than zero")
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    @staticmethod
    def _normalize_path(path: object) -> str:
        text = str(path)
        if not text:
            raise ValueError("Image path must be a non-empty path")
        return text

    @staticmethod
    def _normalize_fit(fit: str) -> str:
        value = str(fit).strip().lower()
        if value not in {"contain", "cover", "stretch"}:
            raise ValueError("Image fit must be 'contain', 'cover', or 'stretch'")
        return value

    def set_path(self, path: object) -> None:
        self.path = self._normalize_path(path)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("path", self.path)

    def reload(self) -> None:
        self.set_path(self.path)

    def set_fit(self, fit: str) -> None:
        self.fit = self._normalize_fit(fit)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("fit", self.fit)

    def props(self) -> dict[str, Any]:
        return {
            "path": self.path,
            "fit": self.fit,
            "width": self.width,
            "height": self.height,
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
        on_pick: ScatterPickCallback | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.frame = frame
        self.x = x
        self.y = y
        self.z = z
        self.colormap = _scatter_colormap(colormap)
        self.frame_summary = summarize_frame(frame)
        self.on_pick = on_pick
        self.pick: ScatterPick | None = None
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

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
            "events": ["change"] if self.on_pick is not None else [],
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
        on_select: TableSelectCallback | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
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
        self.on_select = on_select
        self.selection: TableSelection | None = None
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)
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
        props = self._table_payload()
        props["events"] = ["change"] if self.on_select is not None else []
        return props


def alert(
    title: str,
    message: str,
    *,
    open: bool = True,
    width: int | float = 420,
    height: int | float = 200,
    on_close: Callback | None = None,
    parent: Container | None | object = _AUTO_PARENT,
) -> Modal:
    modal = Modal(title, open=open, width=width, height=height, parent=parent)
    Label(message, parent=modal)
    Spacer(parent=modal)

    def close() -> None:
        modal.close()
        if on_close is not None:
            on_close()

    Button("OK", on_click=close, parent=modal, style={"width": 96, "text_align": "center"})
    return modal


def confirm(
    title: str,
    message: str,
    *,
    open: bool = True,
    width: int | float = 460,
    height: int | float = 220,
    on_confirm: Callback | None = None,
    on_cancel: Callback | None = None,
    parent: Container | None | object = _AUTO_PARENT,
) -> Modal:
    modal = Modal(title, open=open, width=width, height=height, parent=parent)
    Label(message, parent=modal)
    Spacer(parent=modal)
    with HLayout(parent=modal, style={"gap": 8, "height": 38}):

        def cancel() -> None:
            modal.close()
            if on_cancel is not None:
                on_cancel()

        def accept() -> None:
            modal.close()
            if on_confirm is not None:
                on_confirm()

        Spacer()
        Button("Cancel", on_click=cancel, style={"width": 104, "text_align": "center"})
        Button(
            "Confirm",
            on_click=accept,
            style={
                "width": 112,
                "text_align": "center",
                "background": "danger",
                "border_color": "danger",
            },
        )
    return modal
