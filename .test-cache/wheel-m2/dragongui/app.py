from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from ._backend import run_document
from .widgets import Window


@dataclass(slots=True)
class App:
    """Top-level application object."""

    title: str = "DragonGUI"
    metadata: dict[str, Any] = field(default_factory=dict)

    def document(self, window: Window) -> dict[str, Any]:
        return {
            "schema": 1,
            "type": "app",
            "title": self.title,
            "metadata": self.metadata,
            "window": window.to_dict(),
        }

    def run(self, window: Window) -> dict[str, Any]:
        """Start the native event loop for a window."""

        return run_document(self.document(window))
