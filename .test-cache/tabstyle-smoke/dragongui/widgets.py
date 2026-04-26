from __future__ import annotations

import base64
from collections.abc import Callable, Iterable
from contextlib import AbstractContextManager
from itertools import count
import re
from typing import Any, ClassVar, Self

from .dataframe import DEFAULT_TABLE_SAMPLE_ROWS, extract_table_sample, summarize_frame


def _try_pack_xyz(frame: Any, x_col: str, y_col: str, z_col: str) -> str | None:
    """Serialize xyz columns as base64-encoded packed float32 (little-endian xyz triples).

    Returns a base64 string on success, or None if the frame has no accessible
    array attributes (e.g. mock frames used in tests).  NumPy is required for
    efficient serialization; without it the function returns None.
    """
    try:
        import numpy as np

        xs = np.asarray(getattr(frame, x_col), dtype=np.float32)
        ys = np.asarray(getattr(frame, y_col), dtype=np.float32)
        zs = np.asarray(getattr(frame, z_col), dtype=np.float32)
        buf = np.column_stack([xs, ys, zs]).astype(np.float32, copy=False).tobytes()
        return base64.b64encode(buf).decode("ascii")
    except (ImportError, AttributeError, TypeError, ValueError):
        return None

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
        parent: "Container | None | object" = _AUTO_PARENT,
    ) -> None:
        self.id = id or f"dg-{next(_ids)}"
        self.parent: Container | None = None
        if parent is _AUTO_PARENT:
            parent = _BuildContext.parent()
        if parent is not None:
            if not isinstance(parent, Container):
                raise TypeError("parent must be a DragonGUI container or None")
            parent.add(self)

    def props(self) -> dict[str, Any]:
        return {}

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "type": self.kind,
            "props": self.props(),
        }


class Container(Widget, AbstractContextManager["Container"]):
    def __init__(
        self,
        *,
        id: str | None = None,
        parent: "Container | None | object" = _AUTO_PARENT,
    ) -> None:
        self.children: list[Widget] = []
        super().__init__(id=id, parent=parent)

    def add(self, child: Widget) -> Widget:
        if child.parent is self:
            return child
        if child.parent is not None:
            child.parent.children.remove(child)
        child.parent = self
        self.children.append(child)
        return child

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
    ) -> None:
        if _BuildContext.stack:
            raise RuntimeError("cannot create a Window while a layout context is active")
        self.title = title
        self.width = width
        self.height = height
        _BuildContext.stack = []
        super().__init__(id=id, parent=None)
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


class Tabs(Container):
    kind = "tabs"

    def __init__(
        self,
        *,
        value: str | None = None,
        on_change: StringCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = str(value) if value is not None else None
        self.on_change = on_change
        self.disabled = disabled
        super().__init__(id=id, parent=parent)

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
        super().__init__(id=id, parent=actual_parent)

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
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = str(value) if value is not None else None
        self.on_change = on_change
        super().__init__(id=id, parent=parent)

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
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = str(value)
        if not self.value:
            raise ValueError("Page value cannot be empty")
        self.title = title
        actual_parent = _BuildContext.parent() if parent is _AUTO_PARENT else parent
        if actual_parent is not None and not isinstance(actual_parent, Pages):
            raise RuntimeError("Page must be created directly inside a Pages context")
        super().__init__(id=id, parent=actual_parent)

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
        width: int = 220,
        id: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if width <= 0:
            raise ValueError("Sidebar width must be greater than zero")
        self.width = int(width)
        super().__init__(id=id, parent=parent)

    def props(self) -> dict[str, Any]:
        return {"width": self.width}


class NavItem(Widget):
    kind = "nav_item"

    def __init__(
        self,
        label: str,
        *,
        page: str,
        disabled: bool = False,
        id: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.label = str(label)
        self.page = str(page)
        if not self.page:
            raise ValueError("NavItem page cannot be empty")
        self.disabled = disabled
        super().__init__(id=id, parent=parent)

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
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.title = title
        self.width = width
        super().__init__(id=id, parent=parent)

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
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.text = text
        super().__init__(id=id, parent=parent)

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
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.text = text
        self.on_click = on_click
        self.disabled = disabled
        super().__init__(id=id, parent=parent)

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
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = value
        self.placeholder = placeholder
        self.on_change = on_change
        self.disabled = disabled
        super().__init__(id=id, parent=parent)

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
        super().__init__(id=id, parent=parent)

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
        super().__init__(id=id, parent=parent)

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
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.label = label
        self.checked = checked
        self.on_change = on_change
        self.disabled = disabled
        super().__init__(id=id, parent=parent)

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
        id: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.frame = frame
        self.x = x
        self.y = y
        self.z = z
        self.frame_summary = summarize_frame(frame)
        super().__init__(id=id, parent=parent)

    def set_points(self, frame: Any, *, x: str, y: str | None = None, z: str | None = None) -> None:
        self.frame = frame
        self.x = x
        self.y = y if y is not None else self.y
        self.z = z if z is not None else self.z
        self.frame_summary = summarize_frame(frame)

    def props(self) -> dict[str, Any]:
        return {
            "frame": self.frame_summary.to_dict(),
            "x": self.x,
            "y": self.y,
            "z": self.z,
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
        super().__init__(id=id, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "frame": self.frame_summary.to_dict(),
            "page_size": self.page_size,
            "virtualized": True,
            "sample_rows": self.sample_rows,
            "cells": self.cells,
        }
