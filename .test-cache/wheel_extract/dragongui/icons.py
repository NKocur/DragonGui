"""Semantic built-in icon identities.

This module describes the icon names understood by DragonGUI's native vector
renderer.  Widgets accept semantic names and aliases; :func:`resolve_icon`
reports the canonical built-in identity that will be painted.
"""

from __future__ import annotations

from dataclasses import dataclass
import math
from types import MappingProxyType
from collections.abc import Iterable, Mapping, Sequence
from typing import Final, TypeAlias


BUILTIN_ICONS: Final[frozenset[str]] = frozenset(
    {
        "add",
        "axes",
        "check",
        "close",
        "copy",
        "download",
        "edit",
        "eye",
        "eye-off",
        "file",
        "filter",
        "fit",
        "folder",
        "grid",
        "help",
        "home",
        "info",
        "list",
        "lock",
        "menu",
        "minus",
        "more",
        "pan",
        "pause",
        "play",
        "redo",
        "refresh",
        "save",
        "search",
        "settings",
        "sort",
        "stop",
        "undo",
        "unlock",
        "upload",
        "warning",
    }
)

ICON_ALIASES: Final = MappingProxyType(
    {
        "alert": "warning",
        "clear": "close",
        "delete": "close",
        "document": "file",
        "done": "check",
        "duplicate": "copy",
        "export": "download",
        "funnel": "filter",
        "gear": "settings",
        "hamburger": "menu",
        "hidden": "eye-off",
        "hide": "eye-off",
        "import": "upload",
        "new": "add",
        "ok": "check",
        "open": "folder",
        "folder-open": "folder",
        "pencil": "edit",
        "plus": "add",
        "question": "help",
        "reload": "refresh",
        "remove": "minus",
        "run": "play",
        "show": "eye",
        "square": "stop",
        "subtract": "minus",
        "sync": "refresh",
        "visible": "eye",
        "workflow": "list",
        "x": "close",
        "zoom": "search",
    }
)


@dataclass(frozen=True, slots=True)
class IconResolution:
    """Result of resolving a semantic icon name to native built-in geometry."""

    requested: str
    resolved: str
    recognized: bool
    alias: bool
    fallback: bool


def _finite_pair(point: Sequence[object]) -> tuple[float, float]:
    if isinstance(point, (str, bytes, bytearray)) or len(point) != 2:
        raise ValueError("icon stroke points must be coordinate pairs")
    pair = (float(point[0]), float(point[1]))
    if not all(math.isfinite(value) for value in pair):
        raise ValueError("icon stroke coordinates must be finite")
    return pair


@dataclass(frozen=True, slots=True)
class IconStroke:
    """One tintable polyline in an :class:`IconResource`."""

    points: tuple[tuple[float, float], ...]
    closed: bool = False

    def __init__(self, points: Iterable[Sequence[object]], closed: bool = False) -> None:
        normalized = tuple(_finite_pair(point) for point in points)
        if len(normalized) < 2:
            raise ValueError("IconStroke requires at least two points")
        object.__setattr__(self, "points", normalized)
        object.__setattr__(self, "closed", bool(closed))

    def to_dict(self) -> dict[str, object]:
        return {"points": [list(point) for point in self.points], "closed": self.closed}


@dataclass(frozen=True, slots=True)
class IconResource:
    """Bounded monochrome vector geometry for an application icon override."""

    strokes: tuple[IconStroke, ...]
    view_box: tuple[float, float, float, float] = (0.0, 0.0, 24.0, 24.0)
    stroke_width: float = 2.0

    def __init__(
        self,
        strokes: Iterable[IconStroke | Iterable[Sequence[object]]],
        *,
        view_box: Sequence[object] = (0.0, 0.0, 24.0, 24.0),
        stroke_width: float = 2.0,
    ) -> None:
        normalized_strokes = tuple(
            stroke if isinstance(stroke, IconStroke) else IconStroke(stroke)
            for stroke in strokes
        )
        if not normalized_strokes:
            raise ValueError("IconResource requires at least one stroke")
        if len(normalized_strokes) > 64:
            raise ValueError("IconResource cannot contain more than 64 strokes")
        if sum(len(stroke.points) for stroke in normalized_strokes) > 256:
            raise ValueError("IconResource cannot contain more than 256 points")
        if isinstance(view_box, (str, bytes, bytearray)) or len(view_box) != 4:
            raise ValueError("IconResource view_box must contain x, y, width, and height")
        normalized_box = tuple(float(value) for value in view_box)
        if not all(math.isfinite(value) for value in normalized_box):
            raise ValueError("IconResource view_box values must be finite")
        if normalized_box[2] <= 0 or normalized_box[3] <= 0:
            raise ValueError("IconResource view_box width and height must be positive")
        normalized_width = float(stroke_width)
        if not math.isfinite(normalized_width) or normalized_width <= 0:
            raise ValueError("IconResource stroke_width must be positive and finite")
        if normalized_width > min(normalized_box[2], normalized_box[3]):
            raise ValueError("IconResource stroke_width cannot exceed its view_box")
        object.__setattr__(self, "strokes", normalized_strokes)
        object.__setattr__(self, "view_box", normalized_box)
        object.__setattr__(self, "stroke_width", normalized_width)

    def to_dict(self) -> dict[str, object]:
        return {
            "type": "stroke",
            "view_box": list(self.view_box),
            "stroke_width": self.stroke_width,
            "strokes": [stroke.to_dict() for stroke in self.strokes],
        }


IconThemeValue: TypeAlias = IconResource | str


def serialize_icon_theme(theme: Mapping[str, IconThemeValue]) -> dict[str, object]:
    """Validate and serialize an application icon override mapping."""

    serialized: dict[str, object] = {}
    for raw_name, value in theme.items():
        name = normalize_icon_name(raw_name)
        if name in serialized:
            raise ValueError(f"duplicate normalized icon override {name!r}")
        if isinstance(value, IconResource):
            serialized[name] = value.to_dict()
        elif isinstance(value, str):
            serialized[name] = normalize_icon_name(value)
        else:
            raise TypeError("icon theme values must be IconResource objects or semantic names")
    return serialized


def normalize_icon_name(name: str) -> str:
    """Normalize an icon identifier without resolving aliases."""

    normalized = str(name).strip().lower().replace("_", "-")
    if not normalized:
        raise ValueError("icon name must be non-empty")
    return normalized


def resolve_icon(name: str) -> IconResolution:
    """Resolve *name* to native built-in geometry.

    Unknown names retain their requested identity for diagnostics and fall back
    to the built-in ``more`` glyph.  This mirrors native rendering behavior.
    """

    requested = normalize_icon_name(name)
    resolved = ICON_ALIASES.get(requested, requested)
    recognized = resolved in BUILTIN_ICONS
    return IconResolution(
        requested=requested,
        resolved=resolved if recognized else "more",
        recognized=recognized,
        alias=recognized and requested != resolved,
        fallback=not recognized,
    )
