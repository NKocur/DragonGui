from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass
class Theme:
    """Design-token set for DragonGUI's native renderer.

    Color values are CSS-style strings accepted by the native color parser,
    including hex, ``transparent``, common named colors, ``rgb()/rgba()``,
    ``hsl()/hsla()``, ``hwb()``, ``lab()``, ``lch()``, ``oklab()``, ``oklch()``,
    ``color(srgb ...)``, and ``color(srgb-linear ...)``.
    Numeric values are logical (device-independent) pixels.
    """

    background: str = "#0a0f14"
    surface: str = "#121922"
    surface_alt: str = "#1d2833"
    text: str = "#f2f6f8"
    muted_text: str = "#91a0ad"
    accent: str = "#37c6d0"
    border: str = "#263543"
    danger: str = "#ff5f72"
    warning: str = "#f4b84a"
    success: str = "#45c48a"
    focus: str = "#7bdcff"
    disabled: str = "#5d6a75"
    radius: float = 3.0
    spacing: float = 5.0
    font_size: float = 13.0

    @property
    def space_xs(self) -> float:
        """Extra-small layout spacing derived from :attr:`spacing`."""
        return self.spacing * 0.5

    @property
    def space_sm(self) -> float:
        """Small layout spacing; equivalent to the base :attr:`spacing`."""
        return self.spacing

    @property
    def space_md(self) -> float:
        """Medium layout spacing derived from :attr:`spacing`."""
        return self.spacing * 2.0

    @property
    def space_lg(self) -> float:
        """Large layout spacing derived from :attr:`spacing`."""
        return self.spacing * 3.0

    @property
    def space_xl(self) -> float:
        """Extra-large layout spacing derived from :attr:`spacing`."""
        return self.spacing * 4.0

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
            "surface_alt": "#e8eef5",
            "text": "#171821",
            "muted_text": "#5f6b7a",
            "accent": "#087ea4",
            "border": "#cfd8e3",
            "danger": "#cf2445",
            "warning": "#a86f00",
            "success": "#137a4a",
            "focus": "#0a86c8",
            "disabled": "#aeb4c2",
            "radius": 3.0,
            "spacing": 5.0,
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
