from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from ._backend import run_document
from .theme import Theme
from .widgets import (
    Button,
    Checkbox,
    Container,
    Dropdown,
    Pages,
    Slider,
    Tabs,
    TextInput,
    Widget,
    Window,
)


def _collect_callbacks(
    window: Window,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Walk the widget tree and extract on_click / on_change callbacks."""
    click_cbs: dict[str, Any] = {}
    change_cbs: dict[str, Any] = {}

    def walk(widget: Widget) -> None:
        if isinstance(widget, Button) and widget.on_click is not None:
            click_cbs[widget.id] = widget.on_click
        if isinstance(widget, Checkbox) and widget.on_change is not None:
            def checkbox_changed(value: bool, widget: Checkbox = widget) -> None:
                widget.checked = bool(value)
                widget.on_change(widget.checked)

            change_cbs[widget.id] = checkbox_changed
        if isinstance(widget, Slider) and widget.on_change is not None:
            def slider_changed(value: float, widget: Slider = widget) -> None:
                widget.value = float(value)
                widget.on_change(widget.value)

            change_cbs[widget.id] = slider_changed
        if isinstance(widget, Dropdown) and widget.on_change is not None:
            def dropdown_changed(value: str, widget: Dropdown = widget) -> None:
                widget.value = str(value)
                widget.on_change(widget.value)

            change_cbs[widget.id] = dropdown_changed
        if isinstance(widget, TextInput) and widget.on_change is not None:
            def text_changed(value: str, widget: TextInput = widget) -> None:
                widget.value = str(value)
                widget.on_change(widget.value)

            change_cbs[widget.id] = text_changed
        if isinstance(widget, Tabs) and widget.on_change is not None:
            def tabs_changed(value: str, widget: Tabs = widget) -> None:
                widget.value = str(value)
                widget.on_change(widget.value)

            change_cbs[widget.id] = tabs_changed
        if isinstance(widget, Pages) and widget.on_change is not None:
            def pages_changed(value: str, widget: Pages = widget) -> None:
                widget.value = str(value)
                widget.on_change(widget.value)

            change_cbs[widget.id] = pages_changed
        if isinstance(widget, Container):
            for child in widget.children:
                walk(child)

    walk(window)
    return click_cbs, change_cbs


@dataclass(slots=True)
class App:
    """Top-level application object."""

    title: str = "DragonGUI"
    theme: Theme | None = None
    metadata: dict[str, Any] = field(default_factory=dict)

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

    def run(self, window: Window) -> dict[str, Any]:
        """Start the native event loop for a window."""
        click_cbs, change_cbs = _collect_callbacks(window)
        return run_document(self.document(window), click_cbs, change_cbs)
