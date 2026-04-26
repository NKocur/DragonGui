from __future__ import annotations

from collections.abc import Callable, Iterable
from contextlib import AbstractContextManager
from itertools import count
from typing import Any, ClassVar, Self

from .dataframe import summarize_frame

Callback = Callable[[], None]

_ids = count(1)
_AUTO_PARENT = object()


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
        id: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.text = text
        self.on_click = on_click
        super().__init__(id=id, parent=parent)

    def click(self) -> None:
        if self.on_click is not None:
            self.on_click()

    def props(self) -> dict[str, Any]:
        return {"text": self.text, "events": ["click"] if self.on_click else []}


class TextInput(Widget):
    kind = "text_input"

    def __init__(
        self,
        value: str = "",
        *,
        placeholder: str = "",
        id: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = value
        self.placeholder = placeholder
        super().__init__(id=id, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "value": self.value,
            "placeholder": self.placeholder,
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
        id: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = value
        self.min = min
        self.max = max
        self.step = step
        super().__init__(id=id, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "value": self.value,
            "min": self.min,
            "max": self.max,
            "step": self.step,
        }


class Dropdown(Widget):
    kind = "dropdown"

    def __init__(
        self,
        items: Iterable[str],
        *,
        value: str | None = None,
        id: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.items = [str(item) for item in items]
        self.value = value if value is not None else (self.items[0] if self.items else None)
        super().__init__(id=id, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "items": self.items,
            "value": self.value,
        }


class Checkbox(Widget):
    kind = "checkbox"

    def __init__(
        self,
        label: str,
        *,
        checked: bool = False,
        id: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.label = label
        self.checked = checked
        super().__init__(id=id, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "label": self.label,
            "checked": self.checked,
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
        }


class DataFrameTable(Widget):
    kind = "dataframe_table"

    def __init__(
        self,
        frame: Any,
        *,
        page_size: int = 100,
        id: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.frame = frame
        self.page_size = page_size
        self.frame_summary = summarize_frame(frame)
        super().__init__(id=id, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "frame": self.frame_summary.to_dict(),
            "page_size": self.page_size,
            "virtualized": True,
        }
