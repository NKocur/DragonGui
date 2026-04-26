from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass
class Theme:
    """Design-token set for DragonGUI's native renderer.

    Color values are CSS-style hex strings (``"#rrggbb"``).
    Numeric values are logical (device-independent) pixels.
    """

    background: str = "#12121a"
    surface: str = "#1e1e2e"
    surface_alt: str = "#2a2a40"
    text: str = "#f0f0f7"
    muted_text: str = "#a8a8ba"
    accent: str = "#746cff"
    border: str = "#383850"
    danger: str = "#ff5c7a"
    warning: str = "#ffbf47"
    success: str = "#43d48f"
    focus: str = "#6bdcff"
    disabled: str = "#66667a"
    radius: float = 5.0
    spacing: float = 8.0
    font_size: float = 13.0

    @classmethod
    def dark(cls, **overrides: object) -> "Theme":
        """Return the built-in dark theme, optionally overriding specific tokens.

        Example::

            theme = dg.Theme.dark(accent="#4ea1ff", radius=8.0)
        """
        return cls(**overrides)

    @classmethod
    def light(cls, **overrides: object) -> "Theme":
        """Return the built-in light theme, optionally overriding specific tokens."""
        values = {
            "background": "#f6f7fb",
            "surface": "#ffffff",
            "surface_alt": "#eef1f7",
            "text": "#171821",
            "muted_text": "#646879",
            "accent": "#245cff",
            "border": "#d7dbe7",
            "danger": "#cf2445",
            "warning": "#a86f00",
            "success": "#137a4a",
            "focus": "#0077c8",
            "disabled": "#aeb4c2",
            "radius": 5.0,
            "spacing": 8.0,
            "font_size": 13.0,
        }
        values.update(overrides)
        return cls(**values)

    def to_dict(self) -> dict[str, Any]:
        return {
            "background": self.background,
            "surface": self.surface,
            "surface_alt": self.surface_alt,
            "text": self.text,
            "muted_text": self.muted_text,
            "accent": self.accent,
            "border": self.border,
            "danger": self.danger,
            "warning": self.warning,
            "success": self.success,
            "focus": self.focus,
            "disabled": self.disabled,
            "radius": self.radius,
            "spacing": self.spacing,
            "font_size": self.font_size,
        }
