from __future__ import annotations

import base64
import json
import struct
from collections.abc import Callable, Iterable, Mapping, Sequence
import zlib
from contextlib import AbstractContextManager
from contextvars import ContextVar
from dataclasses import dataclass, replace
from datetime import date as _Date, datetime as _DateTime, time as _Time
from itertools import count
import math
import numbers
from pathlib import Path
import re
import tempfile
import threading
import time
from typing import Any, ClassVar, Self
import webbrowser

from .dataframe import (
    DEFAULT_TABLE_SAMPLE_ROWS,
    extract_table_column_buffers,
    extract_table_sample,
    summarize_frame,
)

_INCLUDE_STARTUP_RESOURCE_PAYLOADS: ContextVar[bool] = ContextVar(
    "dragongui_include_startup_resource_payloads",
    default=True,
)


class _StartupResourcePayloadScope(AbstractContextManager[None]):
    def __init__(self, include: bool) -> None:
        self._include = bool(include)
        self._token: object | None = None

    def __enter__(self) -> None:
        self._token = _INCLUDE_STARTUP_RESOURCE_PAYLOADS.set(self._include)
        return None

    def __exit__(self, exc_type: object, exc: object, tb: object) -> None:
        if self._token is not None:
            _INCLUDE_STARTUP_RESOURCE_PAYLOADS.reset(self._token)  # type: ignore[arg-type]
            self._token = None


def _startup_resource_payload_scope(include: bool) -> AbstractContextManager[None]:
    return _StartupResourcePayloadScope(include)


def _include_startup_resource_payloads() -> bool:
    return bool(_INCLUDE_STARTUP_RESOURCE_PAYLOADS.get())


def _get_frame_col(frame: Any, col: str) -> Any:
    """Extract a column from a frame, trying subscript access before attribute access."""
    try:
        return frame[col]
    except (KeyError, TypeError, IndexError):
        pass
    return getattr(frame, col)


def _pack_xyz_bytes(frame: Any, x_col: str, y_col: str, z_col: str) -> object | None:
    """Serialize xyz columns as packed float32 little-endian xyz triples.

    Returns raw bytes on success, or None if the frame has no accessible array
    attributes (e.g. mock frames used in tests).  NumPy is required for
    efficient serialization; without it this returns None.
    """
    try:
        import numpy as np

        xs = np.asarray(_get_frame_col(frame, x_col), dtype=np.float32)
        ys = np.asarray(_get_frame_col(frame, y_col), dtype=np.float32)
        zs = np.asarray(_get_frame_col(frame, z_col), dtype=np.float32)
        if len(xs) == 0:
            return b""
        out = np.empty((len(xs), 3), dtype="<f4")
        out[:, 0] = xs
        out[:, 1] = ys
        out[:, 2] = zs
        return out.view(np.uint8).reshape(-1)
    except (ImportError, AttributeError, TypeError, ValueError):
        return None


def _try_pack_xyz(frame: Any, x_col: str, y_col: str, z_col: str) -> str | None:
    """Serialize xyz columns as base64 for startup document compatibility."""
    buf = _pack_xyz_bytes(frame, x_col, y_col, z_col)
    if buf is None:
        return None
    return base64.b64encode(buf).decode("ascii")


class _Scatter2DFrame:
    """Frame adapter that supplies a synthetic zero-z column for Scatter3D."""

    z_col = "_scatter2d_z"

    def __init__(self, frame: Any, x_col: str, y_col: str) -> None:
        self._frame = frame
        self._x_col = str(x_col)
        self._y_col = str(y_col)
        source_columns = getattr(frame, "columns", None)
        if source_columns is not None:
            columns = tuple(str(column) for column in source_columns)
        else:
            columns = (self._x_col, self._y_col)
        if self.z_col not in columns:
            columns = (*columns, self.z_col)
        self.columns = columns

        source_dtypes = getattr(frame, "dtypes", None)
        try:
            dtypes = tuple(str(dtype) for dtype in source_dtypes) if source_dtypes is not None else ()
        except TypeError:
            dtypes = ()
        if len(dtypes) == len(columns) - 1:
            dtypes = (*dtypes, "float32")
        elif len(dtypes) != len(columns):
            dtypes = tuple("" for _ in columns)
        self.dtypes = dtypes

        source_shape = getattr(frame, "shape", None)
        if isinstance(source_shape, tuple) and source_shape:
            self.shape = (source_shape[0], len(columns))
        else:
            try:
                self.shape = (len(frame), len(columns))
            except TypeError:
                self.shape = (None, len(columns))

    @property
    def index(self) -> Any:
        return getattr(self._frame, "index")

    def _zero_z(self) -> Any:
        import numpy as np

        y = np.asarray(_get_frame_col(self._frame, self._y_col), dtype=np.float32).reshape(-1)
        return np.zeros(len(y), dtype=np.float32)

    def __getitem__(self, column: str) -> Any:
        if str(column) == self.z_col:
            return self._zero_z()
        try:
            return self._frame[column]
        except (KeyError, TypeError, IndexError):
            pass
        try:
            return getattr(self._frame, str(column))
        except AttributeError as exc:
            raise KeyError(column) from exc

    def __getattr__(self, name: str) -> Any:
        if name == self.z_col:
            return self._zero_z()
        return getattr(self._frame, name)


def _pack_xy_bytes(frame: Any, x_col: str | None, y_col: str) -> object | None:
    """Serialize xy columns as packed float32 little-endian pairs."""
    try:
        import numpy as np

        ys = np.asarray(_get_frame_col(frame, y_col), dtype=np.float32).reshape(-1)
        if x_col is None:
            xs = np.arange(len(ys), dtype=np.float32)
        else:
            xs = np.asarray(_get_frame_col(frame, x_col), dtype=np.float32).reshape(-1)
        if len(xs) != len(ys):
            raise ValueError("LinePlot x and y columns must have the same length")
        if len(ys) == 0:
            return b""
        out = np.empty((len(ys), 2), dtype="<f4")
        out[:, 0] = xs
        out[:, 1] = ys
        return out.view(np.uint8).reshape(-1)
    except (ImportError, AttributeError, TypeError, ValueError):
        return None


def _try_pack_xy(frame: Any, x_col: str | None, y_col: str) -> str | None:
    """Serialize xy columns as base64 for startup document compatibility."""
    buf = _pack_xy_bytes(frame, x_col, y_col)
    if buf is None:
        return None
    return base64.b64encode(buf).decode("ascii")


# Qualitative palette for categorical colors (tab10-style, [0,1] RGB).
_CATEGORICAL_PALETTE: list[tuple[float, float, float]] = [
    (0.122, 0.467, 0.706),  # blue
    (1.000, 0.498, 0.055),  # orange
    (0.173, 0.627, 0.173),  # green
    (0.839, 0.153, 0.157),  # red
    (0.580, 0.404, 0.741),  # purple
    (0.549, 0.337, 0.294),  # brown
    (0.890, 0.467, 0.761),  # pink
    (0.498, 0.498, 0.498),  # gray
    (0.737, 0.741, 0.133),  # olive
    (0.090, 0.745, 0.812),  # teal
]
_CATEGORICAL_MAX_UNIQUE = 20


def _is_categorical(arr: Any) -> bool:
    """Return True if the array looks like a categorical (string or low-cardinality int)."""
    import numpy as np
    a = np.asarray(arr)
    if a.dtype.kind in ("U", "S", "O"):
        return True
    if a.dtype.kind in ("i", "u") and len(np.unique(a)) <= _CATEGORICAL_MAX_UNIQUE:
        return True
    return False


def _categorical_to_rgb(arr: Any) -> Any:
    """Assign palette colors by stable category order; returns (N, 3) float32 RGB."""
    import numpy as np
    a = np.asarray(arr)
    unique_vals = list(dict.fromkeys(a.tolist()))  # stable insertion order
    cat_map = {v: i for i, v in enumerate(unique_vals)}
    n_colors = len(_CATEGORICAL_PALETTE)
    indices = np.array([cat_map[v] % n_colors for v in a.tolist()], dtype=np.intp)
    palette = np.array(_CATEGORICAL_PALETTE, dtype=np.float32)
    return palette[indices]


def _categorical_legend_entries(arr: Any) -> "list[tuple[str, float, float, float]]":
    """Return (label, r, g, b) legend entries for each unique category value, in insertion order."""
    a: Any
    try:
        import numpy as _np
        a = _np.asarray(arr)
    except Exception:
        return []
    unique_vals = list(dict.fromkeys(a.tolist()))
    n_colors = len(_CATEGORICAL_PALETTE)
    return [(str(v), float(_CATEGORICAL_PALETTE[i % n_colors][0]),
             float(_CATEGORICAL_PALETTE[i % n_colors][1]),
             float(_CATEGORICAL_PALETTE[i % n_colors][2])) for i, v in enumerate(unique_vals)]


def _resolve_nan_color(nan_color: Any) -> Any:
    """Normalise nan_color to a (3,) float32 RGB array in [0, 1], or None."""
    if nan_color is None:
        return None
    import numpy as np
    arr = np.asarray(nan_color, dtype=np.float32).ravel()[:3]
    if len(arr) < 3:
        return None
    return arr / 255.0 if arr.max() > 1.0 else arr


def _scalars_to_rgb(
    scalars: Any,
    colormap: str,
    clim: tuple[float, float] | None,
    log_scale: bool,
    nan_color: Any = None,
) -> Any:
    """Map a 1-D numeric array through a colormap; returns (N, 3) float32 RGB."""
    import numpy as np
    from .colormap import sample_colormap_numpy

    raw = np.asarray(scalars, dtype=np.float32)
    # NaN mask is based on the original raw values (matching DragonSci): only non-finite
    # values get nan_color; finite non-positive values are clamped, not treated as NaN.
    nan_mask = ~np.isfinite(raw)

    tiny = np.finfo(np.float32).tiny

    # Range is always computed from raw values (matching DragonSci public API).
    if clim is not None:
        raw_lo, raw_hi = float(clim[0]), float(clim[1])
    else:
        finite_raw = raw[~nan_mask]
        raw_lo = float(finite_raw.min()) if len(finite_raw) > 0 else 0.0
        raw_hi = float(finite_raw.max()) if len(finite_raw) > 0 else 1.0

    if log_scale:
        # Normalize in log-space; clip non-positive finite values to tiny (not NaN).
        lv = np.log10(np.maximum(raw, tiny))
        lv[nan_mask] = np.nan
        lo = float(np.log10(max(raw_lo, tiny)))
        hi = float(np.log10(max(raw_hi, tiny)))
        lspan = hi - lo
        if abs(lspan) < 1e-7:
            t = np.full(len(raw), 0.5, dtype=np.float32)
        else:
            t = np.clip((lv - lo) / lspan, 0.0, 1.0).astype(np.float32)
    else:
        span = raw_hi - raw_lo
        if span == 0.0:
            t = np.zeros(len(raw), dtype=np.float32)  # DragonSci parity: collapsed linear → t=0
        else:
            t = np.clip((raw - raw_lo) / span, 0.0, 1.0).astype(np.float32)
    t[nan_mask] = 0.0

    rgb = sample_colormap_numpy(colormap, t)

    nc = _resolve_nan_color(nan_color)
    if nc is not None and nan_mask.any():
        rgb[nan_mask] = nc

    return rgb


def _normalize_sizes(arr: Any, size_range: tuple[float, float]) -> Any:
    """Linearly map arr values into [size_range[0], size_range[1]]."""
    import numpy as np
    a = np.asarray(arr, dtype=np.float32)
    finite = a[np.isfinite(a)]
    lo_val = float(finite.min()) if len(finite) > 0 else 0.0
    hi_val = float(finite.max()) if len(finite) > 0 else 1.0
    lo_px, hi_px = float(size_range[0]), float(size_range[1])
    span = hi_val - lo_val
    if span == 0.0:
        return np.full(len(a), (lo_px + hi_px) * 0.5, dtype=np.float32)
    t = np.clip((a - lo_val) / span, 0.0, 1.0)
    return (lo_px + t * (hi_px - lo_px)).astype(np.float32)


def _pack_point_instances(
    frame: Any,
    x_col: str,
    y_col: str,
    z_col: str,
    *,
    color: str | Any | None = None,
    colors: Any | None = None,
    scalars: str | Any | None = None,
    point_size: float = 4.0,
    point_sizes: str | Any | None = None,
    size_range: tuple[float, float] | None = None,
    opacity: float = 1.0,
    colormap: str = "viridis",
    clim: tuple[float, float] | None = None,
    log_scale: bool = False,
    nan_color: Any = None,
) -> bytes | None:
    """Build a point_instance_v1 packet: N × [x, y, z, size, r, g, b, alpha] as little-endian f32.

    Color priority: explicit colors/color array > categorical/scalar color column > z-derived colormap.
    Returns None when NumPy is unavailable or columns are inaccessible.
    """
    try:
        import numpy as np

        xs = np.asarray(_get_frame_col(frame, x_col), dtype=np.float32)
        ys = np.asarray(_get_frame_col(frame, y_col), dtype=np.float32)
        zs = np.asarray(_get_frame_col(frame, z_col), dtype=np.float32)
        n = len(xs)

        # --- colors ---
        rgb: Any = None
        if colors is not None:
            arr = np.asarray(colors, dtype=np.float32)
            if arr.ndim == 2 and arr.shape == (n, 3):
                rgb = arr if arr.max() <= 1.0 else arr / 255.0
            elif arr.ndim == 2 and arr.shape == (n, 4):
                arr = arr if arr.max() <= 1.0 else arr / 255.0
                rgb = arr[:, :3]
        elif color is not None:
            if isinstance(color, str):
                col_data = _get_frame_col(frame, color)
                if _is_categorical(col_data):
                    rgb = _categorical_to_rgb(col_data)
                else:
                    rgb = _scalars_to_rgb(col_data, colormap, clim, log_scale, nan_color)
            else:
                arr = np.asarray(color, dtype=np.float32)
                if arr.ndim == 2 and arr.shape[0] == n and arr.shape[1] in (3, 4):
                    arr = arr if arr.max() <= 1.0 else arr / 255.0
                    rgb = arr[:, :3]
        elif scalars is not None:
            col_data = _get_frame_col(frame, scalars) if isinstance(scalars, str) else scalars
            rgb = _scalars_to_rgb(col_data, colormap, clim, log_scale, nan_color)

        if rgb is None or len(rgb) != n:
            rgb = _scalars_to_rgb(zs, colormap, clim, log_scale, nan_color)

        # --- sizes: normalize through size_range if given, then clamp ---
        if point_sizes is not None:
            raw_sizes = _get_frame_col(frame, point_sizes) if isinstance(point_sizes, str) else point_sizes
            raw = np.asarray(raw_sizes, dtype=np.float32)
            if len(raw) != n:
                raise ValueError(f"point_sizes length {len(raw)} != point count {n}")
            if size_range is not None:
                sizes = _normalize_sizes(raw, size_range)
            else:
                sizes = np.where(np.isfinite(raw), np.clip(raw, 0.0, None), float(point_size))
        else:
            sizes = np.full(n, float(point_size), dtype=np.float32)

        alpha_val = float(max(0.0, min(1.0, opacity)))

        out = np.empty((n, 8), dtype="<f4")
        out[:, 0] = xs
        out[:, 1] = ys
        out[:, 2] = zs
        out[:, 3] = sizes
        out[:, 4:7] = rgb
        out[:, 7] = alpha_val
        return out.tobytes()
    except (ImportError, AttributeError, TypeError, ValueError, KeyError):
        return None


def _xyz_bounds(
    frame: Any,
    x_col: str,
    y_col: str,
    z_col: str,
) -> tuple[tuple[float, float, float], tuple[float, float, float]] | None:
    try:
        import numpy as np

        xs = np.asarray(_get_frame_col(frame, x_col), dtype=np.float32)
        ys = np.asarray(_get_frame_col(frame, y_col), dtype=np.float32)
        zs = np.asarray(_get_frame_col(frame, z_col), dtype=np.float32)
        finite = np.isfinite(xs) & np.isfinite(ys) & np.isfinite(zs)
        if not bool(np.any(finite)):
            return None
        return (
            (float(np.min(xs[finite])), float(np.min(ys[finite])), float(np.min(zs[finite]))),
            (float(np.max(xs[finite])), float(np.max(ys[finite])), float(np.max(zs[finite]))),
        )
    except (ImportError, AttributeError, TypeError, ValueError, KeyError):
        return None


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
    if len(value) >= 4:
        a = max(0, min(255, int(value[3])))
        return f"#{r:02x}{g:02x}{b:02x}{a:02x}"
    return f"#{r:02x}{g:02x}{b:02x}"


_SUPPORTED_PARTS_BY_KIND: dict[str, set[str]] = {
    "h_layout": {"scrollbar-track", "scrollbar-thumb"},
    "v_layout": {"scrollbar-track", "scrollbar-thumb"},
    "pages": {"scrollbar-track", "scrollbar-thumb"},
    "page": {"scrollbar-track", "scrollbar-thumb"},
    "sidebar": {"scrollbar-track", "scrollbar-thumb"},
    "splitter": {"gutter"},
    "pane": {"pane"},
    "panel": {"accent", "scrollbar-track", "scrollbar-thumb"},
    "collapsible": {
        "header",
        "indicator",
        "body",
        "scrollbar-track",
        "scrollbar-thumb",
    },
    "modal": {"scrim", "scrollbar-track", "scrollbar-thumb"},
    "menu": {"menu", "item", "item-hover", "item-disabled"},
    "context_menu": {"menu", "item", "item-hover", "item-disabled"},
    "button": {"badge"},
    "small_button": {"badge"},
    "icon_button": {"icon"},
    "image_button": {"image"},
    "arrow_button": {"icon"},
    "number_input": {
        "field",
        "stepper",
        "stepper-up",
        "stepper-down",
        "stepper-divider",
        "divider",
        "caret",
    },
    "code_editor": {"field", "gutter", "line-number", "caret"},
    "log_view": {"line", "debug", "info", "warning", "error"},
    "drag_number": {"field", "value", "grip"},
    "dropdown": {"field", "chevron", "menu", "item", "item-selected", "item-hover"},
    "checkbox": {"row", "box", "indicator", "label"},
    "toggle_switch": {"row", "track", "thumb", "label"},
    "tree_node": {"row", "indicator", "label", "guide"},
    "led": {"dot", "glow", "highlight"},
    "slider": {"track", "fill", "thumb"},
    "range_slider": {"track", "range", "thumb-min", "thumb-max", "label"},
    "progress_bar": {"track", "fill", "label"},
    "loading_spinner": {"track", "arc", "label"},
    "heatmap": {"cell", "grid", "hover", "scalar-bar", "label"},
    "bar_chart": {"label", "value-label"},
    "tabs": {"header"},
    "tab": {"tab", "accent", "badge"},
    "nav_item": {"item", "accent", "badge"},
    "dataframe_table": {
        "header",
        "row",
        "row-selected",
        "grid-line",
        "scrollbar-track",
        "scrollbar-thumb",
    },
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
RangeCallback = Callable[[tuple[float, float]], None]
StringCallback = Callable[[str], None]
ColorCallback = Callable[[tuple[int, ...]], None]
BadgeValue = str | int | None
LedColorValue = str | Sequence[object]
DropAcceptValue = str | Sequence[str] | None


@dataclass(frozen=True)
class PropertyChange:
    key: str
    value: object
    old_value: object | None = None


@dataclass(frozen=True)
class BreadcrumbSelection:
    index: int
    label: str
    value: object


@dataclass(frozen=True)
class TableSelection:
    row_index: int
    column_index: int
    column: str
    value: object


@dataclass(frozen=True)
class TableSort:
    column_index: int
    column: str
    descending: bool = False
    is_index: bool = False

    @property
    def direction(self) -> str:
        return "desc" if self.descending else "asc"

    @property
    def target(self) -> str:
        return "index" if self.is_index else "column"


TableSelectCallback = Callable[[TableSelection], None]
TableSortCallback = Callable[[TableSort], None]
BreadcrumbCallback = Callable[[BreadcrumbSelection], None]


@dataclass(frozen=True)
class HeatmapCell:
    row: int
    col: int
    value: float
    x_label: str | None = None
    y_label: str | None = None


HeatmapHoverCallback = Callable[[HeatmapCell | None], None]


@dataclass(frozen=True)
class BarChartBar:
    index: int
    category: str
    series_index: int
    series: str
    value: float


BarChartHoverCallback = Callable[[BarChartBar | None], None]


@dataclass(frozen=True)
class DragDropPayload:
    source_id: str
    target_id: str
    payload: object
    kind: str | None = None
    x: float = 0.0
    y: float = 0.0


DropCallback = Callable[[DragDropPayload], None]


@dataclass(frozen=True)
class PaintPointerEvent:
    widget_id: str
    event: str
    x: float
    y: float
    local_x: float
    local_y: float
    dx: float = 0.0
    dy: float = 0.0
    button: str | None = None


PaintPointerCallback = Callable[[PaintPointerEvent], None]


@dataclass(frozen=True)
class PaintKeyEvent:
    widget_id: str
    event: str
    key: str
    text: str | None = None
    shift: bool = False
    ctrl: bool = False
    alt: bool = False
    super: bool = False
    repeat: bool = False


PaintKeyCallback = Callable[[PaintKeyEvent], None]


@dataclass(frozen=True)
class ScatterPick:
    index: int
    x: float
    y: float
    z: float
    actor: int = 0


@dataclass(frozen=True)
class ScatterHit:
    """One record in a lasso/rectangle selection result.

    Mirrors DragonSci's per-point hit record: which actor the point belongs to
    and its positional index within that actor's buffer.
    """
    actor: int
    index: int


@dataclass(frozen=True)
class ScatterPayload:
    """Immutable packed Scatter3D payload safe to prepare off the UI thread."""

    data: bytes
    payload_format: str
    colormap: str
    point_count: int
    pack_ms: float = 0.0
    axis_labels: tuple[str, str, str] = ("x", "y", "z")
    bounds: tuple[tuple[float, float, float], tuple[float, float, float]] | None = None
    hover_meta: str | None = None
    frame_summary: Any | None = None


@dataclass(frozen=True)
class LinePlotPayload:
    """Immutable packed LinePlot payload safe to prepare off the UI thread."""

    data: bytes
    payload_format: str
    point_count: int
    pack_ms: float = 0.0
    x_label: str = "sample"
    y_label: str = "value"
    frame_summary: Any | None = None


@dataclass(frozen=True)
class HistogramBins:
    """Immutable histogram bin payload used by the Histogram widget."""

    edges: tuple[float, ...]
    counts: tuple[float, ...]
    input_count: int
    finite_count: int


@dataclass(frozen=True)
class BarChartData:
    """Immutable categorical bar chart payload."""

    labels: tuple[str, ...]
    series_labels: tuple[str, ...]
    values: tuple[tuple[float, ...], ...]
    colors: tuple[object, ...]
    input_count: int
    finite_count: int


@dataclass(frozen=True)
class PieChartData:
    """Immutable normalized pie chart payload."""

    labels: tuple[str, ...]
    values: tuple[float, ...]
    colors: tuple[object, ...]
    total: float
    input_count: int
    finite_count: int


@dataclass
class ScatterStreamMetrics:
    produced: int = 0
    submitted: int = 0
    ui_callbacks: int = 0
    errors: int = 0
    last_error: str | None = None


class ScatterFrameStream:
    """Background latest-frame sender for prepared Scatter3D payloads."""

    def __init__(
        self,
        scatter: "Scatter3D",
        frames: Iterable[ScatterPayload],
        *,
        interval_ms: float | Callable[[], float] = 16.0,
        loop: bool = True,
        on_frame: Callable[[ScatterPayload, int, ScatterStreamMetrics], None] | None = None,
        ui_interval_ms: float = 250.0,
        handoff: str = "direct",
    ) -> None:
        self.scatter = scatter
        self.frames = tuple(frames)
        if not self.frames:
            raise ValueError("ScatterFrameStream requires at least one prepared frame")
        normalized_handoff = str(handoff).strip().lower().replace("-", "_")
        if normalized_handoff == "ui_callback":
            normalized_handoff = "callback"
        if normalized_handoff not in ("direct", "callback"):
            raise ValueError("ScatterFrameStream handoff must be 'direct' or 'callback'")
        self.interval_ms = interval_ms
        self.loop = bool(loop)
        self.on_frame = on_frame
        self.ui_interval_ms = max(0.0, float(ui_interval_ms))
        self.handoff = normalized_handoff
        self.metrics = ScatterStreamMetrics()
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self._metrics_lock = threading.Lock()
        self._state_lock = threading.Lock()
        self._colormap_override: str | None = None
        self._last_metadata_colormap: str | None = None

    @property
    def running(self) -> bool:
        thread = self._thread
        return thread is not None and thread.is_alive()

    def start(self) -> None:
        if self.running:
            return
        active_stream = getattr(self.scatter, "_active_frame_stream", None)
        if active_stream is None:
            setattr(self.scatter, "_active_frame_stream", self)
        elif active_stream is not self:
            self._stop.set()
            return
        self._stop.clear()
        self._thread = threading.Thread(
            target=self._run,
            name=f"DragonGUI-ScatterFrameStream-{self.scatter.id}",
            daemon=True,
        )
        self._thread.start()

    def stop(self, timeout: float | None = None) -> None:
        self._stop.set()
        if getattr(self.scatter, "_active_frame_stream", None) is self:
            setattr(self.scatter, "_active_frame_stream", None)
        thread = self._thread
        if thread is not None and thread is not threading.current_thread():
            thread.join(timeout=timeout)

    def _is_active_stream(self) -> bool:
        return getattr(self.scatter, "_active_frame_stream", self) is self

    def set_colormap(self, colormap: str | None) -> None:
        """Override compact stream rendering with the current widget colormap."""
        next_colormap = None if colormap is None else _scatter_colormap(colormap)
        with self._state_lock:
            self._colormap_override = next_colormap
            self._last_metadata_colormap = None
            if next_colormap is not None:
                self.frames = tuple(
                    replace(payload, colormap=next_colormap)
                    if payload.payload_format == "xyz_f32_v0"
                    else payload
                    for payload in self.frames
                )

    def _current_payload_colormap(self, payload: ScatterPayload) -> str:
        with self._state_lock:
            override = self._colormap_override
        if payload.payload_format == "xyz_f32_v0":
            current_colormap = (
                override
                if override is not None
                else getattr(self.scatter, "colormap", payload.colormap)
            )
        else:
            current_colormap = payload.colormap
        try:
            return _scatter_colormap(current_colormap)
        except (TypeError, ValueError):
            return _scatter_colormap(payload.colormap)

    def _current_interval_ms(self) -> float:
        value = self.interval_ms() if callable(self.interval_ms) else self.interval_ms
        try:
            return max(0.0, float(value))
        except (TypeError, ValueError):
            return 16.0

    def _run(self) -> None:
        index = 0
        last_ui_ms = 0.0
        while not self._stop.is_set():
            if not self._is_active_stream():
                break
            if index >= len(self.frames) and not self.loop:
                break
            frame_index = index % len(self.frames)
            payload = self.frames[frame_index]
            with self._metrics_lock:
                self.metrics.produced += 1
            try:
                current_colormap = self._current_payload_colormap(payload)
                include_metadata = index == 0 or current_colormap != self._last_metadata_colormap
                if include_metadata:
                    self._last_metadata_colormap = current_colormap
                now_ms = time.perf_counter() * 1000.0
                notify_frame = (
                    self.on_frame is not None
                    and now_ms - last_ui_ms >= self.ui_interval_ms
                )
                if notify_frame:
                    last_ui_ms = now_ms
                if self.handoff == "callback":
                    handle = self.scatter._live()
                    if handle is not None:
                        handle.app.call_soon_threadsafe(
                            lambda p=payload,
                            i=index,
                            include_metadata=include_metadata,
                            notify=notify_frame: self._apply_callback_frame(
                                p,
                                i,
                                include_metadata=include_metadata,
                                notify_frame=notify,
                            )
                        )
                else:
                    if not self._is_active_stream():
                        break
                    self.scatter.enqueue_prepared_points(
                        payload,
                        coalesce=True,
                        include_metadata=include_metadata,
                        colormap_override=current_colormap,
                    )
                    with self._metrics_lock:
                        self.metrics.submitted += 1
                    if notify_frame:
                        handle = self.scatter._live()
                        if handle is not None:
                            with self._metrics_lock:
                                self.metrics.ui_callbacks += 1
                                snapshot = ScatterStreamMetrics(**self.metrics.__dict__)
                            handle.app.call_soon_threadsafe(
                                lambda p=payload, i=index, m=snapshot: self.on_frame(p, i, m)
                            )
                index += 1
            except RuntimeError:
                break
            except Exception as exc:  # pragma: no cover - defensive stream diagnostics
                with self._metrics_lock:
                    self.metrics.errors += 1
                    self.metrics.last_error = str(exc)
                break

            interval_ms = self._current_interval_ms()
            if self._stop.wait(interval_ms / 1000.0):
                break

    def _apply_callback_frame(
        self,
        payload: ScatterPayload,
        index: int,
        *,
        include_metadata: bool,
        notify_frame: bool,
    ) -> None:
        if self._stop.is_set() or not self._is_active_stream():
            return
        try:
            current_colormap = self._current_payload_colormap(payload)
            self.scatter.enqueue_prepared_points(
                payload,
                coalesce=True,
                include_metadata=include_metadata,
                colormap_override=current_colormap,
            )
            with self._metrics_lock:
                self.metrics.submitted += 1
                if notify_frame and self.on_frame is not None:
                    self.metrics.ui_callbacks += 1
                    snapshot = ScatterStreamMetrics(**self.metrics.__dict__)
                else:
                    snapshot = None
            if snapshot is not None and self.on_frame is not None:
                self.on_frame(payload, index, snapshot)
        except Exception as exc:  # pragma: no cover - defensive stream diagnostics
            with self._metrics_lock:
                self.metrics.errors += 1
                self.metrics.last_error = str(exc)


ScatterPickCallback = Callable[[ScatterPick], None]

_ids = count(1)
_AUTO_PARENT = object()
_UNSET = object()


class ScatterLiveFrame:
    """Retained full-frame replacement handle for high-rate scatter sources.

    ``Scatter3D.set_points()`` is the simple full-scene replacement API. A
    ``ScatterLiveFrame`` keeps one actor stable and replaces only that actor's
    point payload, which better matches sensors that publish a complete current
    frame on every tick.
    """

    def __init__(
        self,
        scatter: "Scatter3D",
        *,
        capacity: int | None = None,
        x: str | None = None,
        y: str | None = None,
        z: str | None = None,
        color: Any | None = None,
        colors: Any | None = None,
        scalars: str | Any | None = None,
        point_size: float | None = None,
        point_sizes: str | Any | None = None,
        opacity: float | None = None,
        colormap: str | None = None,
        clim: tuple[float, float] | None = None,
        log_scale: bool | None = None,
        nan_color: tuple[float, float, float] | None = None,
        size_range: tuple[float, float] | None = None,
        mode: str = "primary",
    ) -> None:
        if mode not in ("primary", "actor"):
            raise ValueError("ScatterLiveFrame mode must be 'primary' or 'actor'")
        self.scatter = scatter
        self.capacity = None if capacity is None else max(0, int(capacity))
        self.mode = mode
        self.actor: int | None = None
        self.x = x
        self.y = y
        self.z = z
        self.color = color
        self.colors = colors
        self.scalars = scalars
        self.point_size = point_size
        self.point_sizes = point_sizes
        self.opacity = opacity
        self.colormap = colormap
        self.clim = clim
        self.log_scale = log_scale
        self.nan_color = nan_color
        self.size_range = size_range
        self.replaces = 0
        self._replace_lock = threading.Lock()

    @property
    def handle(self) -> int | None:
        """Native actor handle for actor mode; ``0`` for retained primary mode."""
        if self.mode == "primary":
            return 0
        return self.actor

    def replace(
        self,
        frame: Any,
        *,
        x: str | None = None,
        y: str | None = None,
        z: str | None = None,
        color: Any | None = _UNSET,
        colors: Any | None = _UNSET,
        scalars: str | Any | None = _UNSET,
        point_size: float | None = None,
        point_sizes: str | Any | None = _UNSET,
        opacity: float | None = None,
        colormap: str | None = None,
        clim: tuple[float, float] | None = _UNSET,
        log_scale: bool | None = None,
        nan_color: tuple[float, float, float] | None = _UNSET,
        size_range: tuple[float, float] | None = _UNSET,
        fit: bool = False,
    ) -> None:
        """Replace this live layer with a complete new frame."""
        xx = x if x is not None else (self.x or self.scatter.x)
        yy = y if y is not None else (self.y or self.scatter.y)
        zz = z if z is not None else (self.z or self.scatter.z)
        cc = self.color if color is _UNSET else color
        explicit_colors = self.colors if colors is _UNSET else colors
        scalar_values = self.scalars if scalars is _UNSET else scalars
        ps = self.point_size if point_size is None else float(point_size)
        pss = self.point_sizes if point_sizes is _UNSET else point_sizes
        alpha = self.opacity if opacity is None else float(opacity)
        cmap = self.colormap if colormap is None else colormap
        scalar_range = self.clim if clim is _UNSET else clim
        use_log = self.log_scale if log_scale is None else bool(log_scale)
        nan = self.nan_color if nan_color is _UNSET else nan_color
        size_rng = self.size_range if size_range is _UNSET else size_range

        point_size_value = self.scatter.point_size if ps is None else ps
        opacity_value = self.scatter.opacity if alpha is None else alpha
        colormap_value = self.scatter.colormap if cmap is None else cmap
        log_scale_value = self.scatter.log_scale if use_log is None else use_log

        if self.mode == "primary":
            payload = self.scatter.prepare_points(
                frame,
                x=xx,
                y=yy,
                z=zz,
                color=cc,
                colors=explicit_colors,
                scalars=scalar_values,
                point_size=point_size_value,
                point_sizes=pss,
                opacity=opacity_value,
                colormap=colormap_value,
                clim=scalar_range,
                log_scale=log_scale_value,
                nan_color=nan,
                size_range=size_rng,
            )
            self.scatter.set_prepared_points(
                payload,
                coalesce=True,
                update_metadata=True,
                fit=fit,
            )
            self.replaces += 1
            return

        kwargs = {
            "x": xx,
            "y": yy,
            "z": zz,
            "color": cc,
            "colors": explicit_colors,
            "scalars": scalar_values,
            "point_size": point_size_value,
            "point_sizes": pss,
            "opacity": opacity_value,
            "colormap": colormap_value,
            "clim": scalar_range,
            "log_scale": log_scale_value,
            "nan_color": nan,
            "size_range": size_rng,
        }
        if self.actor is None:
            self.actor = self.scatter.add_points(frame, **kwargs)
        else:
            self.scatter.update_actor(self.actor, frame, **kwargs)
        self.replaces += 1
        if fit:
            self.scatter.fit()

    def replace_prepared(
        self,
        payload: ScatterPayload,
        *,
        fit: bool = False,
        update_metadata: bool = True,
    ) -> None:
        """Replace this live frame with an already-packed scatter payload.

        This is the lowest-overhead UI-thread path for high-rate sources: parse
        and pack the sensor frame on a worker thread with
        ``Scatter3D.prepare_points(...)``, then enqueue the prepared payload here.
        """
        if self.mode == "primary":
            self.scatter.set_prepared_points(
                payload,
                coalesce=True,
                update_metadata=update_metadata,
                fit=fit,
            )
            self.replaces += 1
            return

        raise NotImplementedError(
            "ScatterLiveFrame.replace_prepared() is currently supported only for mode='primary'"
        )

    def enqueue_prepared(
        self,
        payload: ScatterPayload,
        *,
        fit: bool = False,
        update_metadata: bool = False,
        coalesce: bool = True,
    ) -> None:
        """Thread-safe enqueue for an already-packed primary scatter frame.

        Use this from producer threads that already have a
        ``ScatterPayload``. It bypasses Python UI callback scheduling and sends
        the payload straight to the native command queue, where stale pending
        frames for the same scatter are coalesced when ``coalesce=True``.
        """
        if self.mode != "primary":
            raise NotImplementedError(
                "ScatterLiveFrame.enqueue_prepared() is currently supported only for mode='primary'"
            )
        self.scatter.enqueue_prepared_points(
            payload,
            coalesce=coalesce,
            include_metadata=update_metadata,
            fit=fit,
        )
        with self._replace_lock:
            self.replaces += 1

    def remove(self) -> None:
        """Remove this live layer from the scatter."""
        if self.mode == "primary":
            self.scatter.clear()
            return
        if self.actor is not None:
            self.scatter.remove_actor(self.actor)
            self.actor = None

    def set_visible(self, visible: bool) -> None:
        """Show or hide this live layer."""
        if self.mode == "primary":
            return
        if self.actor is not None:
            self.scatter.set_actor_visibility(self.actor, bool(visible))


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


def _json_compatible_payload(value: object) -> object:
    try:
        encoded = json.dumps(value, separators=(",", ":"), sort_keys=True)
    except (TypeError, ValueError) as exc:
        raise TypeError("drag payload must be JSON serializable") from exc
    return json.loads(encoded)


def _payload_kind(payload: object, drag_kind: str | None) -> str | None:
    if drag_kind is not None:
        kind = str(drag_kind).strip()
        if not kind:
            raise ValueError("drag kind cannot be empty")
        return kind
    if isinstance(payload, Mapping):
        value = payload.get("kind")
        if value is not None:
            kind = str(value).strip()
            if kind:
                return kind
    return None


def _drop_accept_list(accept: DropAcceptValue) -> list[str]:
    if accept is None:
        return []
    if isinstance(accept, str):
        values = [accept]
    elif isinstance(accept, Sequence) and not isinstance(accept, (bytes, bytearray)):
        values = list(accept)
    else:
        raise TypeError("drop accept must be a string, a sequence of strings, or None")
    normalized: list[str] = []
    for value in values:
        if not isinstance(value, str):
            raise TypeError("drop accept entries must be strings")
        item = value.strip()
        if not item:
            raise ValueError("drop accept entries cannot be empty")
        normalized.append(item)
    return normalized


def _badge_level(value: str) -> str:
    if not isinstance(value, str):
        raise TypeError("badge level must be a string")
    level = value.strip().lower()
    if level not in _BADGE_LEVELS:
        allowed = ", ".join(sorted(_BADGE_LEVELS))
        raise ValueError(f"unknown badge level {value!r}; expected one of: {allowed}")
    return level


def _led_state_name(value: object) -> str:
    if isinstance(value, bool):
        return "on" if value else "off"
    state = str(value).strip()
    if not state:
        raise ValueError("LED state must be a non-empty string or bool")
    return state


def _led_color_value(value: LedColorValue) -> str:
    if isinstance(value, str):
        color = value.strip()
        if not color:
            raise ValueError("LED color must be a non-empty string or RGB/RGBA sequence")
        return color
    return _color_hex(_normalize_color_tuple(value, alpha=True))


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

    def set_class(self, class_: str | None) -> None:
        if class_ is not None and (not isinstance(class_, str) or not class_):
            raise ValueError("widget class_ must be a non-empty string")
        self.class_ = class_
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("class", self.class_)

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


def _non_negative_finite_value(value: int | float | None, name: str) -> float | None:
    if value is None:
        return None
    value_f = float(value)
    if not math.isfinite(value_f) or value_f < 0:
        raise ValueError(f"{name} must be a non-negative finite number")
    return value_f


def _positive_finite_value(value: int | float, name: str) -> float:
    value_f = float(value)
    if not math.isfinite(value_f) or value_f <= 0:
        raise ValueError(f"{name} must be a positive finite number")
    return value_f


@dataclass(frozen=True)
class Size:
    """Logical size returned by custom painted widgets."""

    width: float
    height: float

    def __post_init__(self) -> None:
        object.__setattr__(self, "width", _positive_finite_value(self.width, "width"))
        object.__setattr__(self, "height", _positive_finite_value(self.height, "height"))


@dataclass(frozen=True)
class MeasureConstraints:
    """Logical layout constraints passed to `PaintWidget.measure`."""

    min_width: float = 0.0
    min_height: float = 0.0
    max_width: float | None = None
    max_height: float | None = None

    def __post_init__(self) -> None:
        min_width = _non_negative_finite_value(self.min_width, "min_width")
        min_height = _non_negative_finite_value(self.min_height, "min_height")
        max_width = _non_negative_finite_value(self.max_width, "max_width")
        max_height = _non_negative_finite_value(self.max_height, "max_height")
        object.__setattr__(self, "min_width", min_width or 0.0)
        object.__setattr__(self, "min_height", min_height or 0.0)
        object.__setattr__(self, "max_width", max_width)
        object.__setattr__(self, "max_height", max_height)
        if max_width is not None and max_width < self.min_width:
            raise ValueError("max_width must be greater than or equal to min_width")
        if max_height is not None and max_height < self.min_height:
            raise ValueError("max_height must be greater than or equal to min_height")

    def clamp(self, size: Size) -> Size:
        width = max(size.width, self.min_width)
        height = max(size.height, self.min_height)
        if self.max_width is not None:
            width = min(width, self.max_width)
        if self.max_height is not None:
            height = min(height, self.max_height)
        return Size(width, height)


PaintColor = str | Sequence[object]


def _paint_color_value(value: PaintColor | None, name: str) -> object | None:
    if value is None:
        return None
    if isinstance(value, str):
        color = value.strip()
        if not color:
            raise ValueError(f"{name} must be a non-empty color string or RGB/RGBA sequence")
        return color
    if isinstance(value, Sequence) and not isinstance(value, (bytes, bytearray)):
        return list(_normalize_color_tuple(value, alpha=True))
    raise TypeError(f"{name} must be a color string or RGB/RGBA sequence")


def _paint_finite(value: int | float, name: str) -> float:
    value_f = float(value)
    if not math.isfinite(value_f):
        raise ValueError(f"{name} must be finite")
    return value_f


def _paint_non_negative(value: int | float, name: str) -> float:
    value_f = _paint_finite(value, name)
    if value_f < 0:
        raise ValueError(f"{name} must be non-negative")
    return value_f


class PaintContext:
    """Records display-list drawing commands for a `PaintWidget`."""

    def __init__(self, width: int | float, height: int | float) -> None:
        self.width = _positive_finite_value(width, "width")
        self.height = _positive_finite_value(height, "height")
        self._commands: list[dict[str, object]] = []

    def rect(
        self,
        x: int | float,
        y: int | float,
        width: int | float,
        height: int | float,
        *,
        fill: PaintColor | None = "surface_alt",
        stroke: PaintColor | None = None,
        stroke_width: int | float = 1.0,
        radius: int | float = 0.0,
    ) -> None:
        command: dict[str, object] = {
            "cmd": "rect",
            "x": _paint_finite(x, "x"),
            "y": _paint_finite(y, "y"),
            "w": _paint_non_negative(width, "width"),
            "h": _paint_non_negative(height, "height"),
            "radius": _paint_non_negative(radius, "radius"),
        }
        if fill is not None:
            command["fill"] = _paint_color_value(fill, "fill")
        if stroke is not None:
            command["stroke"] = _paint_color_value(stroke, "stroke")
            command["stroke_width"] = _paint_non_negative(stroke_width, "stroke_width")
        self._commands.append(command)

    def rounded_rect(
        self,
        x: int | float,
        y: int | float,
        width: int | float,
        height: int | float,
        *,
        radius: int | float = 6.0,
        fill: PaintColor | None = "surface_alt",
        stroke: PaintColor | None = None,
        stroke_width: int | float = 1.0,
    ) -> None:
        self.rect(
            x,
            y,
            width,
            height,
            fill=fill,
            stroke=stroke,
            stroke_width=stroke_width,
            radius=radius,
        )

    def line(
        self,
        x1: int | float,
        y1: int | float,
        x2: int | float,
        y2: int | float,
        *,
        stroke: PaintColor = "accent",
        width: int | float = 1.5,
    ) -> None:
        self._commands.append(
            {
                "cmd": "line",
                "x1": _paint_finite(x1, "x1"),
                "y1": _paint_finite(y1, "y1"),
                "x2": _paint_finite(x2, "x2"),
                "y2": _paint_finite(y2, "y2"),
                "stroke": _paint_color_value(stroke, "stroke"),
                "stroke_width": _positive_finite_value(width, "width"),
            }
        )

    def polyline(
        self,
        points: Sequence[Sequence[int | float]],
        *,
        stroke: PaintColor = "accent",
        width: int | float = 1.5,
    ) -> None:
        if isinstance(points, (str, bytes, bytearray)):
            raise TypeError("points must be a sequence of coordinate pairs")
        clean_points: list[list[float]] = []
        for index, point in enumerate(points):
            if not isinstance(point, Sequence) or isinstance(point, (str, bytes, bytearray)):
                raise TypeError("points must be a sequence of coordinate pairs")
            if len(point) != 2:
                raise ValueError(f"point {index} must have exactly two values")
            clean_points.append(
                [
                    _paint_finite(point[0], f"point {index} x"),
                    _paint_finite(point[1], f"point {index} y"),
                ]
            )
        self._commands.append(
            {
                "cmd": "polyline",
                "points": clean_points,
                "stroke": _paint_color_value(stroke, "stroke"),
                "stroke_width": _positive_finite_value(width, "width"),
            }
        )

    def circle(
        self,
        cx: int | float,
        cy: int | float,
        radius: int | float,
        *,
        fill: PaintColor | None = "accent",
        stroke: PaintColor | None = None,
        stroke_width: int | float = 1.0,
    ) -> None:
        command: dict[str, object] = {
            "cmd": "circle",
            "cx": _paint_finite(cx, "cx"),
            "cy": _paint_finite(cy, "cy"),
            "r": _paint_non_negative(radius, "radius"),
        }
        if fill is not None:
            command["fill"] = _paint_color_value(fill, "fill")
        if stroke is not None:
            command["stroke"] = _paint_color_value(stroke, "stroke")
            command["stroke_width"] = _paint_non_negative(stroke_width, "stroke_width")
        self._commands.append(command)

    def text(
        self,
        x: int | float,
        y: int | float,
        text: object,
        *,
        fill: PaintColor = "text",
        font_size: int | float | None = None,
        font_weight: int | None = None,
        align: str = "left",
    ) -> None:
        align_value = str(align).strip().lower()
        if align_value not in {"left", "center", "right"}:
            raise ValueError("align must be 'left', 'center', or 'right'")
        command: dict[str, object] = {
            "cmd": "text",
            "x": _paint_finite(x, "x"),
            "y": _paint_finite(y, "y"),
            "text": str(text),
            "fill": _paint_color_value(fill, "fill"),
            "align": align_value,
        }
        if font_size is not None:
            command["font_size"] = _positive_finite_value(font_size, "font_size")
        if font_weight is not None:
            weight = int(font_weight)
            if weight < 1 or weight > 1000:
                raise ValueError("font_weight must be between 1 and 1000")
            command["font_weight"] = weight
        self._commands.append(command)

    def image(
        self,
        path: object,
        x: int | float,
        y: int | float,
        width: int | float,
        height: int | float,
        *,
        fit: str = "contain",
        radius: int | float = 0.0,
    ) -> None:
        fit_value = str(fit).strip().lower()
        if fit_value not in {"contain", "cover", "stretch"}:
            raise ValueError("fit must be 'contain', 'cover', or 'stretch'")
        path_text = Image._normalize_path(path)
        self._commands.append(
            {
                "cmd": "image",
                "path": path_text,
                "x": _paint_finite(x, "x"),
                "y": _paint_finite(y, "y"),
                "w": _paint_non_negative(width, "width"),
                "h": _paint_non_negative(height, "height"),
                "fit": fit_value,
                "radius": _paint_non_negative(radius, "radius"),
            }
        )

    def to_list(self) -> list[dict[str, object]]:
        return [dict(command) for command in self._commands]


class ExtensionWidget(Widget):
    """Internal leaf foundation for future third-party custom widgets.

    This serializes as ``type: "extension"`` and keeps arbitrary JSON props
    under a stable ``extension_type`` name. It can receive simple click events;
    custom drawing and richer pointer events layer on top of this native widget
    kind.
    """

    kind = "extension"

    def __init__(
        self,
        extension_type: str,
        props: Mapping[str, object] | None = None,
        *,
        intrinsic_width: int | float | None = None,
        intrinsic_height: int | float | None = None,
        width: int | float | None = None,
        height: int | float | None = None,
        on_click: Callback | None = None,
        on_pointer_down: PaintPointerCallback | None = None,
        on_pointer_move: PaintPointerCallback | None = None,
        on_pointer_up: PaintPointerCallback | None = None,
        on_wheel: PaintPointerCallback | None = None,
        on_key_down: PaintKeyCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        name = str(extension_type).strip()
        if not name:
            raise ValueError("extension_type must be a non-empty string")
        if props is not None and not isinstance(props, Mapping):
            raise TypeError("ExtensionWidget props must be a mapping")
        clean_props = _json_compatible_payload(dict(props or {}))
        if not isinstance(clean_props, dict):
            raise TypeError("ExtensionWidget props must serialize to a JSON object")
        clean_props["extension_type"] = name
        for prop_name, prop_value in (
            ("intrinsic_width", _non_negative_finite_value(intrinsic_width, "intrinsic_width")),
            ("intrinsic_height", _non_negative_finite_value(intrinsic_height, "intrinsic_height")),
            ("width", _non_negative_finite_value(width, "width")),
            ("height", _non_negative_finite_value(height, "height")),
        ):
            if prop_value is not None:
                clean_props[prop_name] = prop_value
        self.extension_type = name
        self.on_click = on_click
        self.on_pointer_down = on_pointer_down
        self.on_pointer_move = on_pointer_move
        self.on_pointer_up = on_pointer_up
        self.on_wheel = on_wheel
        self.on_key_down = on_key_down
        self.disabled = bool(disabled)
        self.extension_props: dict[str, object] = self._runtime_extension_props(clean_props)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def _runtime_extension_props(self, props: Mapping[str, object]) -> dict[str, object]:
        clean_props = dict(props)
        clean_props["extension_type"] = self.extension_type
        if self.disabled:
            clean_props["disabled"] = True
        else:
            clean_props.pop("disabled", None)
        events: list[str] = []
        if self.on_click is not None and not self.disabled:
            events.append("click")
        if self.on_pointer_down is not None and not self.disabled:
            events.append("pointer_down")
        if self.on_pointer_move is not None and not self.disabled:
            events.append("pointer_move")
        if self.on_pointer_up is not None and not self.disabled:
            events.append("pointer_up")
        if self.on_wheel is not None and not self.disabled:
            events.append("wheel")
        if self.on_key_down is not None and not self.disabled:
            events.append("key_down")
        if events:
            clean_props["events"] = events
        else:
            clean_props.pop("events", None)
        return clean_props

    def props(self) -> dict[str, Any]:
        return dict(self.extension_props)

    def set_extension_props(self, props: Mapping[str, object]) -> None:
        """Replace extension props and queue a live node replacement when mounted."""
        if not isinstance(props, Mapping):
            raise TypeError("ExtensionWidget props must be a mapping")
        clean_props = _json_compatible_payload(dict(props))
        if not isinstance(clean_props, dict):
            raise TypeError("ExtensionWidget props must serialize to a JSON object")
        self.extension_props = self._runtime_extension_props(clean_props)
        if (handle := self._live()) is not None:
            handle.enqueue_replace_node(self.to_dict())


class PaintWidget(ExtensionWidget):
    """Base class for pure-Python custom painted widgets.

    Subclasses override `measure()` and `paint(ctx)`. The paint method records a
    small display list that the native renderer consumes through the extension
    widget path.
    """

    def __init__(
        self,
        *,
        extension_type: str = "paint",
        width: int | float | None = None,
        height: int | float | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        on_click: Callback | None = None,
        on_pointer_down: PaintPointerCallback | None = None,
        on_pointer_move: PaintPointerCallback | None = None,
        on_pointer_up: PaintPointerCallback | None = None,
        on_wheel: PaintPointerCallback | None = None,
        on_key_down: PaintKeyCallback | None = None,
        disabled: bool = False,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self._paint_fixed_width = _non_negative_finite_value(width, "width")
        self._paint_fixed_height = _non_negative_finite_value(height, "height")
        constraints = MeasureConstraints(
            max_width=self._paint_fixed_width,
            max_height=self._paint_fixed_height,
        )
        self.paint_size = constraints.clamp(self.measure(constraints))
        props = self._paint_extension_props()
        super().__init__(
            extension_type,
            props,
            intrinsic_width=self.paint_size.width,
            intrinsic_height=self.paint_size.height,
            width=self._paint_fixed_width,
            height=self._paint_fixed_height,
            on_click=on_click,
            on_pointer_down=on_pointer_down,
            on_pointer_move=on_pointer_move,
            on_pointer_up=on_pointer_up,
            on_wheel=on_wheel,
            on_key_down=on_key_down,
            disabled=disabled,
            id=id,
            key=key,
            class_=class_,
            style=style,
            tooltip=tooltip,
            parent=parent,
        )

    def measure(self, constraints: MeasureConstraints) -> Size:
        width = constraints.max_width if constraints.max_width is not None else 160.0
        height = constraints.max_height if constraints.max_height is not None else 80.0
        return Size(width, height)

    def paint(self, ctx: PaintContext) -> None:
        pass

    def repaint(self) -> None:
        """Rebuild the display list and replace live extension props if mounted."""
        self.set_extension_props(self._paint_extension_props())

    def _build_display_list(self) -> list[dict[str, object]]:
        ctx = PaintContext(self.paint_size.width, self.paint_size.height)
        self.paint(ctx)
        return ctx.to_list()

    def _paint_extension_props(self) -> dict[str, object]:
        props: dict[str, object] = {
            "paint_width": self.paint_size.width,
            "paint_height": self.paint_size.height,
            "intrinsic_width": self.paint_size.width,
            "intrinsic_height": self.paint_size.height,
            "display_list": self._build_display_list(),
        }
        if self._paint_fixed_width is not None:
            props["width"] = self._paint_fixed_width
        if self._paint_fixed_height is not None:
            props["height"] = self._paint_fixed_height
        return props


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


class ScrollArea(Container):
    """Bounded scroll viewport for content that may exceed available space.

    ``axis`` controls which overflow direction scrolls: ``"y"`` (default),
    ``"x"``, ``"both"``, or ``"none"``.  The widget behaves like a vertical
    layout by default and is intended for cases where the parent provides a
    bounded rectangle, such as a page, panel body, or grid cell.
    """

    kind = "scroll_area"
    _AXES = {"x", "y", "both", "none"}

    def __init__(
        self,
        *,
        axis: str = "y",
        gap: "int | None" = None,
        width: "int | float | None" = None,
        height: "int | float | None" = None,
        id: "str | None" = None,
        key: "str | None" = None,
        class_: "str | None" = None,
        style: "Mapping[str, object] | None" = None,
        tooltip: "str | None" = None,
        parent: "Container | None | object" = _AUTO_PARENT,
    ) -> None:
        axis_value = axis.strip().lower()
        if axis_value not in self._AXES:
            raise ValueError("ScrollArea axis must be 'x', 'y', 'both', or 'none'")
        if gap is not None and int(gap) < 0:
            raise ValueError("ScrollArea gap must be non-negative")
        if width is not None and float(width) < 0:
            raise ValueError("ScrollArea width cannot be negative")
        if height is not None and float(height) < 0:
            raise ValueError("ScrollArea height cannot be negative")

        extra: "dict[str, object]" = {
            "display": "flex",
            "flex_direction": "column",
            "flex_grow": 1,
            "flex_shrink": 1,
            "min_height": 0,
        }
        extra["overflow_x"] = "auto" if axis_value in {"x", "both"} else "hidden"
        extra["overflow_y"] = "auto" if axis_value in {"y", "both"} else "hidden"
        if gap is not None:
            extra["gap"] = int(gap)
        if width is not None:
            extra["width"] = float(width)
        if height is not None:
            extra["height"] = float(height)
        merged: "Mapping[str, object]" = {**extra, **(style or {})}
        super().__init__(id=id, key=key, class_=class_, style=merged, tooltip=tooltip, parent=parent)

    def scroll_to(
        self,
        *,
        x: "int | float | None" = None,
        y: "int | float | None" = None,
    ) -> None:
        """Set the live scroll offset for this scroll area, in logical pixels."""
        handle = self._live()
        if handle is None:
            return
        if x is not None:
            handle.enqueue_set_prop("scroll_x", float(x))
        if y is not None:
            handle.enqueue_set_prop("scroll_y", float(y))


GridTrackValue = int | float | str | Mapping[str, object]
GridTrackTemplate = Sequence[GridTrackValue] | str


def _grid_track_number(value: object, name: str, *, positive: bool = False) -> int | float:
    if isinstance(value, bool):
        raise ValueError(f"GridLayout {name} track sizes must be numeric")
    if isinstance(value, str):
        try:
            num = float(value)
        except ValueError as exc:
            raise ValueError(f"GridLayout {name} track sizes must be numeric") from exc
    elif isinstance(value, numbers.Real):
        num = float(value)
    else:
        raise ValueError(f"GridLayout {name} track sizes must be numeric")
    if not math.isfinite(num) or num < 0 or (positive and num <= 0):
        qualifier = "positive" if positive else "non-negative"
        raise ValueError(f"GridLayout {name} track sizes must be {qualifier}")
    return int(num) if num.is_integer() else num


def _grid_track_percent(value: object, name: str) -> dict[str, object]:
    return {"percent": _grid_track_number(value, name)}


def _normalize_grid_fit_content(value: object, name: str) -> object:
    track = _normalize_grid_track(value, name)
    if isinstance(track, (int, float)):
        return track
    if isinstance(track, Mapping) and set(track) == {"percent"}:
        return dict(track)
    raise ValueError(f"GridLayout {name} fit-content tracks require px or percent sizes")


def _normalize_grid_min_track(value: object, name: str) -> object:
    track = _normalize_grid_track(value, name)
    if isinstance(track, (int, float)) or track == "auto":
        return track
    if isinstance(track, Mapping) and set(track) == {"percent"}:
        return dict(track)
    raise ValueError(f"GridLayout {name} minmax min tracks require px, percent, or auto")


def _normalize_grid_max_track(value: object, name: str) -> object:
    track = _normalize_grid_track(value, name)
    if isinstance(track, (int, float)) or track == "auto":
        return track
    if isinstance(track, Mapping) and set(track) <= {"percent", "fr"}:
        return dict(track)
    raise ValueError(f"GridLayout {name} minmax max tracks require px, percent, fr, or auto")


def _normalize_grid_minmax(value: object, name: str) -> dict[str, object]:
    if isinstance(value, Mapping):
        if "min" not in value or "max" not in value:
            raise ValueError(f"GridLayout {name} minmax tracks require min and max")
        min_value = value["min"]
        max_value = value["max"]
    elif (
        isinstance(value, Sequence)
        and not isinstance(value, (str, bytes, bytearray))
        and len(value) == 2
    ):
        min_value, max_value = value
    else:
        raise ValueError(f"GridLayout {name} minmax tracks require a two-item sequence")
    return {
        "min": _normalize_grid_min_track(min_value, name),
        "max": _normalize_grid_max_track(max_value, name),
    }


def _normalize_grid_track_mapping(value: Mapping[str, object], name: str) -> object:
    if "fr" in value:
        return {"fr": _grid_track_number(value["fr"], name, positive=True)}
    if "percent" in value:
        return _grid_track_percent(value["percent"], name)
    if "fit_content" in value:
        return {"fit_content": _normalize_grid_fit_content(value["fit_content"], name)}
    if "fit" in value:
        return {"fit_content": _normalize_grid_fit_content(value["fit"], name)}
    if "minmax" in value:
        return {"minmax": _normalize_grid_minmax(value["minmax"], name)}
    if "min" in value or "max" in value:
        return {"minmax": _normalize_grid_minmax(value, name)}
    if "repeat" in value:
        repeat = value["repeat"]
        if not isinstance(repeat, Mapping):
            raise ValueError(f"GridLayout {name} repeat tracks require a mapping")
        kind = str(repeat.get("kind", "auto-fit")).strip().lower()
        if kind not in {"auto-fit", "auto-fill"}:
            raise ValueError("GridLayout repeat kind must be 'auto-fit' or 'auto-fill'")
        tracks = repeat.get("tracks")
        return {
            "repeat": {
                "kind": kind,
                "tracks": _normalize_grid_template_tracks(tracks, name),
            }
        }
    raise ValueError(f"GridLayout {name} track mapping is not recognized")


def _normalize_grid_track_text(value: str, name: str) -> object:
    token = value.strip().lower()
    if not token:
        raise ValueError(f"GridLayout {name} track values cannot be empty")
    if token == "auto":
        return "auto"
    if token.endswith("fr"):
        return {"fr": _grid_track_number(token[:-2].strip(), name, positive=True)}
    if token.endswith("px"):
        return _grid_track_number(token[:-2].strip(), name)
    if token.endswith("%"):
        return _grid_track_percent(token[:-1].strip(), name)
    if token.startswith("fit-content(") and token.endswith(")"):
        return {"fit_content": _normalize_grid_fit_content(token[12:-1], name)}
    if token.startswith("minmax(") and token.endswith(")"):
        parts = token[7:-1].split(",", 1)
        if len(parts) != 2:
            raise ValueError(f"GridLayout {name} minmax tracks require two values")
        return {
            "minmax": {
                "min": _normalize_grid_min_track(parts[0].strip(), name),
                "max": _normalize_grid_max_track(parts[1].strip(), name),
            }
        }
    raise ValueError(f"GridLayout {name} track value {value!r} is not supported")


def _normalize_grid_track(value: object, name: str) -> object:
    if isinstance(value, Mapping):
        return _normalize_grid_track_mapping(value, name)
    if isinstance(value, str):
        return _normalize_grid_track_text(value, name)
    return _grid_track_number(value, name)


def _normalize_grid_template_tracks(value: object, name: str) -> list[object]:
    if value is None:
        raise ValueError(f"GridLayout {name} cannot be None")
    if isinstance(value, str):
        raw_tracks: list[object] = value.split()
    elif isinstance(value, Sequence) and not isinstance(value, (bytes, bytearray)):
        raw_tracks = list(value)
    else:
        raise ValueError(f"GridLayout {name} must be a sequence or whitespace-separated string")
    if not raw_tracks:
        raise ValueError(f"GridLayout {name} must contain at least one track")
    return [_normalize_grid_track(track, name) for track in raw_tracks]


class GridLayout(Container):
    """Responsive CSS-grid container.

    ``columns`` may be a positive integer (maximum column count) or the string
    ``"auto"`` (auto-fill based on ``min_column_width``).  When both an integer
    column count and ``min_column_width`` are given, the layout uses up to that
    many columns and collapses to fewer columns when there is not enough space.

    When ``masonry=True``, children are packed into the shortest column after
    their responsive column widths are resolved.  This keeps card galleries
    dense without forcing every item in a visual row to share the tallest row
    height.

    ``template_columns`` and ``template_rows`` accept explicit grid track
    definitions such as ``(44, "1fr")`` for compact key/value layouts.  When
    ``template_columns`` is provided, it overrides ``columns`` and
    ``min_column_width``. ``gap`` sets both row and column gap; ``row_gap``
    overrides the row gap independently.  Both are in logical pixels.  All
    arguments may be overridden by CSS class rules applied to the widget.
    """

    kind = "grid_layout"

    def __init__(
        self,
        *,
        columns: "int | str" = 2,
        min_column_width: "int | None" = 320,
        template_columns: "GridTrackTemplate | None" = None,
        template_rows: "GridTrackTemplate | None" = None,
        masonry: bool = False,
        gap: "int | None" = None,
        row_gap: "int | None" = None,
        id: "str | None" = None,
        key: "str | None" = None,
        class_: "str | None" = None,
        style: "Mapping[str, object] | None" = None,
        tooltip: "str | None" = None,
        parent: "Container | None | object" = _AUTO_PARENT,
    ) -> None:
        if isinstance(columns, str):
            if columns != "auto":
                raise ValueError("GridLayout columns must be a positive integer or 'auto'")
        elif isinstance(columns, int):
            if columns < 1:
                raise ValueError("GridLayout columns must be a positive integer or 'auto'")
        else:
            raise ValueError("GridLayout columns must be a positive integer or 'auto'")
        if min_column_width is not None and int(min_column_width) <= 0:
            raise ValueError("GridLayout min_column_width must be positive")
        if gap is not None and int(gap) < 0:
            raise ValueError("GridLayout gap must be non-negative")
        if row_gap is not None and int(row_gap) < 0:
            raise ValueError("GridLayout row_gap must be non-negative")
        self._grid_columns = columns
        self._grid_min_column_width = min_column_width
        self._grid_template_columns = (
            _normalize_grid_template_tracks(template_columns, "template_columns")
            if template_columns is not None
            else None
        )
        self._grid_template_rows = (
            _normalize_grid_template_tracks(template_rows, "template_rows")
            if template_rows is not None
            else None
        )
        self._grid_masonry = bool(masonry)
        extra: "dict[str, object]" = {}
        if gap is not None:
            extra["gap"] = int(gap)
        if row_gap is not None:
            extra["row_gap"] = int(row_gap)
        merged: "Mapping[str, object] | None" = ({**extra, **(style or {})} if extra else style)
        super().__init__(id=id, key=key, class_=class_, style=merged, tooltip=tooltip, parent=parent)

    def props(self) -> "dict[str, Any]":
        p: "dict[str, Any]" = {}
        if self._grid_template_columns is not None:
            p["template_columns"] = self._grid_template_columns
        elif self._grid_columns != "auto" and isinstance(self._grid_columns, int):
            p["columns"] = self._grid_columns
        if self._grid_template_rows is not None:
            p["template_rows"] = self._grid_template_rows
        if self._grid_template_columns is None and self._grid_min_column_width is not None:
            p["min_column_width"] = int(self._grid_min_column_width)
        if self._grid_masonry:
            p["masonry"] = True
        return p


class FlowLayout(Container):
    """Wrapping flex-row container.

    Children keep their intrinsic widths and wrap onto new rows when they
    exceed the available width.  ``gap`` sets both row and column gap;
    ``row_gap`` overrides the row gap independently.  Both are in logical
    pixels and may be overridden by CSS class rules. ``align`` controls
    horizontal distribution across each row and accepts ``"start"``,
    ``"center"``, or ``"end"``.
    """

    kind = "flow_layout"

    def __init__(
        self,
        *,
        gap: "int | None" = None,
        row_gap: "int | None" = None,
        align: "str" = "start",
        cross_align: "str" = "start",
        id: "str | None" = None,
        key: "str | None" = None,
        class_: "str | None" = None,
        style: "Mapping[str, object] | None" = None,
        tooltip: "str | None" = None,
        parent: "Container | None | object" = _AUTO_PARENT,
    ) -> None:
        if align not in {"start", "center", "end"}:
            raise ValueError("FlowLayout align must be 'start', 'center', or 'end'")
        if cross_align not in {"start", "center", "end", "stretch"}:
            raise ValueError("FlowLayout cross_align must be 'start', 'center', 'end', or 'stretch'")
        if gap is not None and int(gap) < 0:
            raise ValueError("FlowLayout gap must be non-negative")
        if row_gap is not None and int(row_gap) < 0:
            raise ValueError("FlowLayout row_gap must be non-negative")
        self._flow_align = align
        self._flow_cross_align = cross_align
        extra: "dict[str, object]" = {}
        if gap is not None:
            extra["gap"] = int(gap)
        if row_gap is not None:
            extra["row_gap"] = int(row_gap)
        merged: "Mapping[str, object] | None" = ({**extra, **(style or {})} if extra else style)
        super().__init__(id=id, key=key, class_=class_, style=merged, tooltip=tooltip, parent=parent)

    def props(self) -> "dict[str, Any]":
        p: "dict[str, Any]" = {}
        if self._flow_align != "start":
            p["align"] = self._flow_align
        if self._flow_cross_align != "start":
            p["cross_align"] = self._flow_cross_align
        return p


SplitterSize = int | float | str | None


def _normalize_splitter_orientation(value: str) -> str:
    normalized = value.strip().lower()
    if normalized in {"h", "horizontal", "row", "x"}:
        return "horizontal"
    if normalized in {"v", "vertical", "column", "y"}:
        return "vertical"
    raise ValueError("Splitter orientation must be 'horizontal' or 'vertical'")


def _normalize_splitter_size(value: SplitterSize) -> float | str | None:
    if value is None:
        return None
    if isinstance(value, str):
        text = value.strip().lower()
        if text in {"", "auto"}:
            return None
        if text.endswith("fr"):
            number = text[:-2].strip() or "1"
            flex = float(number)
            if not math.isfinite(flex) or flex <= 0:
                raise ValueError("Splitter fr sizes must be positive")
            return f"{flex:g}fr"
        raise ValueError("Splitter sizes must be numbers, 'auto', or '<number>fr'")
    size = float(value)
    if not math.isfinite(size) or size < 0:
        raise ValueError("Splitter sizes must be non-negative finite values")
    return size


def _splitter_size_to_flex(value: float | str | None) -> float:
    if isinstance(value, str) and value.endswith("fr"):
        return float(value[:-2])
    return 1.0


class Splitter(Container):
    """Resizable multi-pane layout container."""

    kind = "splitter"

    def __init__(
        self,
        *,
        orientation: str = "horizontal",
        sizes: Sequence[SplitterSize] | None = None,
        min_sizes: Sequence[float | None] | None = None,
        max_sizes: Sequence[float | None] | None = None,
        gutter_size: int | float = 6,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.orientation = _normalize_splitter_orientation(orientation)
        self.sizes = tuple(_normalize_splitter_size(size) for size in (sizes or ()))
        self.min_sizes = self._normalize_optional_sizes(min_sizes, "min_sizes")
        self.max_sizes = self._normalize_optional_sizes(max_sizes, "max_sizes")
        gutter = float(gutter_size)
        if not math.isfinite(gutter) or gutter < 1:
            raise ValueError("Splitter gutter_size must be a positive finite value")
        self.gutter_size = gutter
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    @staticmethod
    def _normalize_optional_sizes(
        values: Sequence[float | None] | None,
        name: str,
    ) -> tuple[float | None, ...]:
        if values is None:
            return ()
        normalized: list[float | None] = []
        for value in values:
            if value is None:
                normalized.append(None)
                continue
            size = float(value)
            if not math.isfinite(size) or size < 0:
                raise ValueError(f"Splitter {name} entries must be non-negative finite values")
            normalized.append(size)
        return tuple(normalized)

    def add(self, child: Widget) -> Widget:
        child = super().add(child)
        self._apply_pane_defaults()
        return child

    def set_sizes(self, sizes: Sequence[SplitterSize]) -> None:
        self.sizes = tuple(_normalize_splitter_size(size) for size in sizes)
        self._apply_pane_defaults()
        for index, child in enumerate(self.children):
            if not isinstance(child, Pane) or index >= len(self.sizes):
                continue
            size = self.sizes[index]
            if isinstance(size, str):
                child.set_size(None)
                child.flex = _splitter_size_to_flex(size)
            else:
                child.set_size(size)

    def _apply_pane_defaults(self) -> None:
        for index, child in enumerate(self.children):
            if not isinstance(child, Pane):
                continue
            child._splitter_orientation = self.orientation
            child._splitter_index = index
            if index < len(self.sizes) and child._explicit_size is None:
                size = self.sizes[index]
                child._splitter_size = None if isinstance(size, str) else size
                child._splitter_flex = _splitter_size_to_flex(size)
            if index < len(self.min_sizes) and child._explicit_min_size is None:
                child._splitter_min_size = self.min_sizes[index]
            if index < len(self.max_sizes) and child._explicit_max_size is None:
                child._splitter_max_size = self.max_sizes[index]

    def props(self) -> dict[str, Any]:
        self._apply_pane_defaults()
        return {
            "orientation": self.orientation,
            "gutter_size": self.gutter_size,
            "sizes": list(self.sizes),
            "min_sizes": list(self.min_sizes),
            "max_sizes": list(self.max_sizes),
        }


class Pane(Container):
    """Pane child for Splitter layouts."""

    kind = "pane"

    def __init__(
        self,
        *,
        size: float | None = None,
        min_size: float | None | object = _UNSET,
        max_size: float | None = None,
        flex: float = 1,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self._explicit_size = self._normalize_optional_size(size, "size")
        if min_size is _UNSET:
            self._explicit_min_size = None
            self._default_min_size: float | None = 80.0
        else:
            self._explicit_min_size = self._normalize_optional_size(min_size, "min_size")
            self._default_min_size = None
        self._explicit_max_size = self._normalize_optional_size(max_size, "max_size")
        flex_value = float(flex)
        if not math.isfinite(flex_value) or flex_value <= 0:
            raise ValueError("Pane flex must be a positive finite value")
        self.flex = flex_value
        self._splitter_orientation = "horizontal"
        self._splitter_index: int | None = None
        self._splitter_size: float | None = None
        self._splitter_min_size: float | None = None
        self._splitter_max_size: float | None = None
        self._splitter_flex: float | None = None
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    @staticmethod
    def _normalize_optional_size(value: float | None, name: str) -> float | None:
        if value is None:
            return None
        size = float(value)
        if not math.isfinite(size) or size < 0:
            raise ValueError(f"Pane {name} must be a non-negative finite value")
        return size

    def _effective_size(self) -> float | None:
        return self._explicit_size if self._explicit_size is not None else self._splitter_size

    def _effective_min_size(self) -> float | None:
        if self._explicit_min_size is not None:
            return self._explicit_min_size
        if self._splitter_min_size is not None:
            return self._splitter_min_size
        return self._default_min_size

    def _effective_max_size(self) -> float | None:
        if self._explicit_max_size is not None:
            return self._explicit_max_size
        return self._splitter_max_size

    def _effective_flex(self) -> float:
        return self._splitter_flex if self._splitter_flex is not None else self.flex

    def set_size(self, size: float | None) -> None:
        self._explicit_size = self._normalize_optional_size(size, "size")
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("size", self._explicit_size)

    def props(self) -> dict[str, Any]:
        return {
            "orientation": self._splitter_orientation,
            "size": self._effective_size(),
            "min_size": self._effective_min_size(),
            "max_size": self._effective_max_size(),
            "flex": self._effective_flex(),
        }


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


class Toolbar(HLayout):
    """Compact command strip with stable spacing and orientation defaults."""

    _ORIENTATIONS = {"horizontal", "vertical"}

    def __init__(
        self,
        *,
        orientation: str = "horizontal",
        gap: int | float | None = 6,
        compact: bool = True,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        value = str(orientation).strip().lower()
        if value not in self._ORIENTATIONS:
            raise ValueError("Toolbar orientation must be 'horizontal' or 'vertical'")
        if gap is not None:
            gap_f = float(gap)
            if not math.isfinite(gap_f) or gap_f < 0:
                raise ValueError("Toolbar gap must be a non-negative finite number")
        else:
            gap_f = None
        self.orientation = value
        self.compact = bool(compact)
        direction = "row" if value == "horizontal" else "column"
        default_style: dict[str, object] = {
            "display": "flex",
            "flex_direction": direction,
            "align_items": "center",
            "min_width": 0,
            "min_height": 0,
        }
        if gap_f is not None:
            default_style["gap"] = gap_f
        if value == "horizontal":
            default_style["width"] = "100%"
            default_style["height"] = 38 if compact else 44
        else:
            default_style["width"] = 38 if compact else 44
            default_style["height"] = "100%"
        merged_class = _merge_widget_class(
            "toolbar toolbar-vertical" if value == "vertical" else "toolbar toolbar-horizontal",
            class_,
        )
        super().__init__(
            id=id,
            key=key,
            class_=merged_class,
            style={**default_style, **(style or {})},
            tooltip=tooltip,
            parent=parent,
        )

    def props(self) -> dict[str, Any]:
        return {
            "orientation": self.orientation,
            "compact": self.compact,
        }


class ToolbarSeparator(Separator):
    """Separator that chooses the correct axis when placed inside a Toolbar."""

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
        resolved_parent = _BuildContext.parent() if parent is _AUTO_PARENT else parent
        resolved_orientation = orientation
        if orientation == "auto" and isinstance(resolved_parent, Toolbar):
            resolved_orientation = (
                "vertical" if resolved_parent.orientation == "horizontal" else "horizontal"
            )
        default_style: dict[str, object] = {}
        if resolved_orientation == "vertical":
            default_style = {"width": 1, "height": 24}
        elif resolved_orientation == "horizontal":
            default_style = {"width": 24, "height": 1}
        super().__init__(
            orientation=resolved_orientation,
            id=id,
            key=key,
            class_=_merge_widget_class("toolbar-separator", class_),
            style={**default_style, **(style or {})} or None,
            tooltip=tooltip,
            parent=resolved_parent,
        )


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


class DragSource(Container):
    """Transparent container that starts an app-local drag with a JSON payload."""

    kind = "drag_source"

    def __init__(
        self,
        payload: object,
        *,
        drag_kind: str | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.payload = _json_compatible_payload(payload)
        self.drag_kind = _payload_kind(self.payload, drag_kind)
        self.disabled = bool(disabled)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "payload": self.payload,
            "drag_kind": self.drag_kind,
            "disabled": self.disabled,
        }


class DropTarget(Container):
    """Transparent container that accepts app-local drops from ``DragSource``."""

    kind = "drop_target"

    def __init__(
        self,
        *,
        accept: DropAcceptValue = None,
        on_drop: DropCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.accept = _drop_accept_list(accept)
        self.on_drop = on_drop
        self.disabled = bool(disabled)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def props(self) -> dict[str, Any]:
        return {
            "accept": self.accept,
            "disabled": self.disabled,
            "events": ["change"] if self.on_drop and not self.disabled else [],
        }


class DropZone(DropTarget):
    """Styled drop target convenience widget with a centered label."""

    _DEFAULT_STYLE: ClassVar[dict[str, object]] = {
        "height": 142,
        "padding": 14,
        "align_items": "center",
        "justify_content": "center",
        "gap": 6,
    }

    def __init__(
        self,
        label: str,
        *,
        accept: DropAcceptValue = None,
        on_drop: DropCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.label = str(label)
        merged_style = {**self._DEFAULT_STYLE, **(style or {})}
        super().__init__(
            accept=accept,
            on_drop=on_drop,
            disabled=disabled,
            id=id,
            key=key,
            class_=_merge_widget_class("drop-zone", class_),
            style=merged_style,
            tooltip=tooltip,
            parent=parent,
        )
        Label(self.label, parent=self, class_="drop-zone-label")

    def props(self) -> dict[str, Any]:
        data = super().props()
        data["text"] = self.label
        return data


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
        close_button: bool = False,
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
        self.close_button = bool(close_button)
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
            "close_button": self.close_button,
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
        wrap: bool = True,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.text = text
        self.wrap = bool(wrap)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def props(self) -> dict[str, Any]:
        return {"text": self.text, "wrap": self.wrap}

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


class LED(Widget):
    """Compact status light with boolean or named states.

    ``state`` may be a bool (``True`` = ``"on"``, ``False`` = ``"off"``) or a
    string.  Named states resolve through ``states``; ``on`` and ``off`` always
    have default colors unless overridden.
    """

    kind = "led"

    def __init__(
        self,
        state: bool | str = False,
        *,
        states: Mapping[str, LedColorValue] | None = None,
        on_color: LedColorValue = "success",
        off_color: LedColorValue = "disabled",
        size: int | float = 14,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        size_f = float(size)
        if not math.isfinite(size_f) or size_f <= 0:
            raise ValueError("LED size must be a positive finite number")
        self.states: dict[str, str] = {
            "on": _led_color_value(on_color),
            "off": _led_color_value(off_color),
        }
        if states is not None:
            if not isinstance(states, Mapping):
                raise TypeError("LED states must be a mapping of state name to color")
            for name, color in states.items():
                self.states[_led_state_name(name)] = _led_color_value(color)
        self.state = _led_state_name(state)
        self.color = self._color_for_state(self.state)
        self.size = size_f
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def _color_for_state(self, state: str) -> str:
        try:
            return self.states[state]
        except KeyError as exc:
            known = ", ".join(sorted(self.states))
            raise ValueError(f"unknown LED state {state!r}; expected one of: {known}") from exc

    @property
    def on(self) -> bool:
        return self.state == "on"

    def set_state(self, state: bool | str, *, color: LedColorValue | None = None) -> None:
        state_name = _led_state_name(state)
        if color is not None:
            self.states[state_name] = _led_color_value(color)
        self.state = state_name
        self.color = self._color_for_state(state_name)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("state", self.state)
            handle.enqueue_set_prop("color", self.color)

    def set_on(self, on: bool = True) -> None:
        self.set_state(bool(on))

    def set_color(self, color: LedColorValue) -> None:
        self.states[self.state] = _led_color_value(color)
        self.color = self.states[self.state]
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("color", self.color)

    def set_size(self, size: int | float) -> None:
        size_f = float(size)
        if not math.isfinite(size_f) or size_f <= 0:
            raise ValueError("LED size must be a positive finite number")
        self.size = size_f
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("size", self.size)

    def props(self) -> dict[str, Any]:
        return {
            "state": self.state,
            "color": self.color,
            "size": self.size,
        }


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


def _button_size_value(value: int | float | None, name: str) -> float | None:
    if value is None:
        return None
    size = float(value)
    if not math.isfinite(size) or size <= 0:
        raise ValueError(f"{name} must be a positive finite number")
    return size


class SmallButton(Button):
    kind = "small_button"


class IconButton(Button):
    kind = "icon_button"

    def __init__(
        self,
        icon: str,
        *,
        on_click: Callback | None = None,
        disabled: bool = False,
        size: int | float | None = None,
        width: int | float | None = None,
        height: int | float | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.icon = self._normalize_icon(icon)
        default_size = _button_size_value(size, "IconButton size")
        self.width = _button_size_value(width, "IconButton width")
        self.height = _button_size_value(height, "IconButton height")
        if default_size is not None:
            self.width = self.width or default_size
            self.height = self.height or default_size
        super().__init__(
            "",
            on_click=on_click,
            disabled=disabled,
            id=id,
            key=key,
            class_=class_,
            style=style,
            tooltip=tooltip,
            parent=parent,
        )

    @staticmethod
    def _normalize_icon(icon: str) -> str:
        value = str(icon).strip().lower().replace("_", "-")
        if not value:
            raise ValueError("IconButton icon must be non-empty")
        return value

    def props(self) -> dict[str, Any]:
        return {
            "icon": self.icon,
            "width": self.width,
            "height": self.height,
            "disabled": self.disabled,
            "events": ["click"] if self.on_click and not self.disabled else [],
        }

    def set_icon(self, icon: str) -> None:
        self.icon = self._normalize_icon(icon)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("icon", self.icon)


class ImageButton(Button):
    kind = "image_button"

    def __init__(
        self,
        path: object,
        *,
        fit: str = "contain",
        on_click: Callback | None = None,
        disabled: bool = False,
        size: int | float | None = None,
        width: int | float | None = None,
        height: int | float | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.path = Image._normalize_path(path)
        self.fit = Image._normalize_fit(fit)
        default_size = _button_size_value(size, "ImageButton size")
        self.width = _button_size_value(width, "ImageButton width")
        self.height = _button_size_value(height, "ImageButton height")
        if default_size is not None:
            self.width = self.width or default_size
            self.height = self.height or default_size
        super().__init__(
            "",
            on_click=on_click,
            disabled=disabled,
            id=id,
            key=key,
            class_=class_,
            style=style,
            tooltip=tooltip,
            parent=parent,
        )

    def set_path(self, path: object) -> None:
        self.path = Image._normalize_path(path)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("path", self.path)

    def reload(self) -> None:
        self.set_path(self.path)

    def set_fit(self, fit: str) -> None:
        self.fit = Image._normalize_fit(fit)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("fit", self.fit)

    def props(self) -> dict[str, Any]:
        return {
            "path": self.path,
            "fit": self.fit,
            "width": self.width,
            "height": self.height,
            "disabled": self.disabled,
            "events": ["click"] if self.on_click and not self.disabled else [],
        }


class ArrowButton(Button):
    kind = "arrow_button"
    _DIRECTIONS = {"left", "right", "up", "down"}

    def __init__(
        self,
        direction: str,
        *,
        on_click: Callback | None = None,
        disabled: bool = False,
        size: int | float | None = None,
        width: int | float | None = None,
        height: int | float | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.direction = self._normalize_direction(direction)
        default_size = _button_size_value(size, "ArrowButton size")
        self.width = _button_size_value(width, "ArrowButton width")
        self.height = _button_size_value(height, "ArrowButton height")
        if default_size is not None:
            self.width = self.width or default_size
            self.height = self.height or default_size
        super().__init__(
            "",
            on_click=on_click,
            disabled=disabled,
            id=id,
            key=key,
            class_=class_,
            style=style,
            tooltip=tooltip,
            parent=parent,
        )

    @classmethod
    def _normalize_direction(cls, direction: str) -> str:
        value = str(direction).strip().lower()
        if value not in cls._DIRECTIONS:
            allowed = ", ".join(sorted(cls._DIRECTIONS))
            raise ValueError(f"ArrowButton direction must be one of: {allowed}")
        return value

    def props(self) -> dict[str, Any]:
        return {
            "direction": self.direction,
            "width": self.width,
            "height": self.height,
            "disabled": self.disabled,
            "events": ["click"] if self.on_click and not self.disabled else [],
        }

    def set_direction(self, direction: str) -> None:
        self.direction = self._normalize_direction(direction)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("direction", self.direction)


class Selectable(Widget):
    kind = "selectable"

    def __init__(
        self,
        text: str,
        *,
        value: str | None = None,
        selected: bool = False,
        toggle: bool = True,
        on_select: BoolCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.text = str(text)
        self.value = self.text if value is None else str(value)
        self.selected = bool(selected)
        self.toggle = bool(toggle)
        self.on_select = on_select
        self.disabled = bool(disabled)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def set_selected(self, selected: bool, *, notify: bool = False) -> None:
        self.selected = bool(selected)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("checked", self.selected)
        if notify and not self.disabled and self.on_select is not None:
            self.on_select(self.selected)

    def select(self, *, notify: bool = False) -> None:
        self.set_selected(True, notify=notify)

    def clear_selection(self, *, notify: bool = False) -> None:
        self.set_selected(False, notify=notify)

    def props(self) -> dict[str, Any]:
        return {
            "text": self.text,
            "value": self.value,
            "checked": self.selected,
            "toggle": self.toggle,
            "disabled": self.disabled,
            "events": ["change"] if self.on_select and not self.disabled else [],
        }


class RadioButton(Widget):
    kind = "radio_button"

    def __init__(
        self,
        label: str,
        *,
        value: str | None = None,
        checked: bool = False,
        on_change: BoolCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.label = str(label)
        self.value = self.label if value is None else str(value)
        self.checked = bool(checked)
        self.on_change = on_change
        self.disabled = bool(disabled)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def set_checked(self, checked: bool, *, notify: bool = False) -> None:
        self.checked = bool(checked)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("checked", self.checked)
        if notify and not self.disabled and self.on_change is not None:
            self.on_change(self.checked)

    def select(self, *, notify: bool = False) -> None:
        self.set_checked(True, notify=notify)

    def props(self) -> dict[str, Any]:
        return {
            "label": self.label,
            "value": self.value,
            "checked": self.checked,
            "toggle": False,
            "disabled": self.disabled,
            "events": ["change"] if self.on_change and not self.disabled else [],
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


class CodeEditor(Widget):
    kind = "code_editor"

    def __init__(
        self,
        value: str = "",
        *,
        language: str = "",
        placeholder: str = "",
        rows: int = 10,
        wrap: bool = False,
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
            raise ValueError("CodeEditor rows must be at least 1")
        self.value = str(value)
        self.language = str(language)
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
            "language": self.language,
            "placeholder": self.placeholder,
            "rows": self.rows,
            "wrap": self.wrap,
            "disabled": self.disabled,
        }


class LogView(Widget):
    kind = "log_view"

    def __init__(
        self,
        lines: str | Iterable[object] = (),
        *,
        follow: bool = True,
        max_lines: int = 10_000,
        rows: int = 12,
        wrap: bool = False,
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
            raise ValueError("LogView rows must be at least 1")
        max_lines_i = int(max_lines)
        if max_lines_i < 1:
            raise ValueError("LogView max_lines must be at least 1")
        self.follow = bool(follow)
        self.max_lines = max_lines_i
        self.rows = rows_i
        self.wrap = bool(wrap)
        self.disabled = bool(disabled)
        self.lines = self._normalize_lines(lines)
        self._trim()
        self.value = self._joined()
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    @staticmethod
    def _normalize_lines(lines: str | Iterable[object]) -> list[str]:
        if isinstance(lines, str):
            return lines.splitlines()
        out: list[str] = []
        for line in lines:
            text = str(line)
            parts = text.splitlines()
            out.extend(parts if parts else [""])
        return out

    def _trim(self) -> None:
        if len(self.lines) > self.max_lines:
            self.lines = self.lines[-self.max_lines :]

    def _joined(self) -> str:
        return "\n".join(self.lines)

    def _sync_value(self) -> None:
        self._trim()
        self.value = self._joined()
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("value", self.value)

    def set_lines(self, lines: str | Iterable[object]) -> None:
        self.lines = self._normalize_lines(lines)
        self._sync_value()

    def append_line(self, line: object = "") -> None:
        self.lines.extend(self._normalize_lines([line]))
        self._sync_value()

    def append_lines(self, lines: Iterable[object]) -> None:
        self.lines.extend(self._normalize_lines(lines))
        self._sync_value()

    def clear(self) -> None:
        self.lines = []
        self._sync_value()

    def props(self) -> dict[str, Any]:
        return {
            "value": self.value,
            "follow": self.follow,
            "max_lines": self.max_lines,
            "rows": self.rows,
            "wrap": self.wrap,
            "disabled": self.disabled,
        }


TemporalValue = str | _Date | _Time | _DateTime


def _format_time_iso(value: _Time) -> str:
    timespec = "microseconds" if value.microsecond else "seconds" if value.second else "minutes"
    return value.isoformat(timespec=timespec)


def _format_datetime_iso(value: _DateTime) -> str:
    timespec = "microseconds" if value.microsecond else "seconds"
    return value.isoformat(timespec=timespec)


class _TemporalInput(Widget):
    kind = "text_input"
    _css_class = "temporal-input"
    _placeholder = ""
    _value_name = "TemporalInput"

    def __init__(
        self,
        value: object = "",
        *,
        placeholder: str | None = None,
        on_change: StringCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        normalized = self._normalize(value)
        self.value = normalized
        self.text = normalized
        self.placeholder = self._placeholder if placeholder is None else str(placeholder)
        self.on_change = on_change
        self.disabled = bool(disabled)
        self.invalid = False
        self._base_class = _merge_widget_class(self._css_class, class_)
        super().__init__(
            id=id,
            key=key,
            class_=self._class_name(),
            style=style,
            tooltip=tooltip,
            parent=parent,
        )

    def _normalize(self, value: object) -> str:
        raise NotImplementedError

    def _class_name(self) -> str:
        if self.invalid:
            return _merge_widget_class(self._base_class, "invalid")
        return self._base_class

    def _refresh_class(self) -> None:
        next_class = self._class_name()
        if next_class != self.class_:
            self.set_class(next_class)

    def _set_invalid(self, invalid: bool) -> None:
        self.invalid = bool(invalid)
        self._refresh_class()

    def _handle_native_change(self, value: object) -> None:
        text = str(value)
        self.text = text
        try:
            normalized = self._normalize(text)
        except (TypeError, ValueError):
            self._set_invalid(bool(text))
            return
        self.value = normalized
        self.text = text
        self._set_invalid(False)
        if self.on_change is not None and not self.disabled:
            self.on_change(self.value)

    def set_value(self, value: object, *, notify: bool = False) -> None:
        self.value = self._normalize(value)
        self.text = self.value
        self._set_invalid(False)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("value", self.text)
        if notify and self.on_change is not None and not self.disabled:
            self.on_change(self.value)

    def props(self) -> dict[str, Any]:
        return {
            "value": self.text,
            "placeholder": self.placeholder,
            "disabled": self.disabled,
            "events": ["change"],
        }


class DateInput(_TemporalInput):
    """Validated ISO date input. `value` and callbacks use `YYYY-MM-DD` strings."""

    _css_class = "temporal-input date-input"
    _placeholder = "YYYY-MM-DD"
    _value_name = "DateInput"

    def _normalize(self, value: object) -> str:
        if value is None:
            return ""
        if isinstance(value, _DateTime):
            return value.date().isoformat()
        if isinstance(value, _Date):
            return value.isoformat()
        text = str(value).strip()
        if not text:
            return ""
        try:
            return _Date.fromisoformat(text).isoformat()
        except ValueError as exc:
            raise ValueError("DateInput value must be an ISO date string (YYYY-MM-DD)") from exc


class TimeInput(_TemporalInput):
    """Validated ISO time input. `value` and callbacks use ISO time strings."""

    _css_class = "temporal-input time-input"
    _placeholder = "HH:MM"
    _value_name = "TimeInput"

    def _normalize(self, value: object) -> str:
        if value is None:
            return ""
        if isinstance(value, _DateTime):
            return _format_time_iso(value.time())
        if isinstance(value, _Time):
            return _format_time_iso(value)
        text = str(value).strip()
        if not text:
            return ""
        try:
            return _format_time_iso(_Time.fromisoformat(text))
        except ValueError as exc:
            raise ValueError("TimeInput value must be an ISO time string (HH:MM or HH:MM:SS)") from exc


class DateTimeInput(_TemporalInput):
    """Validated ISO datetime input. `value` and callbacks use ISO datetime strings."""

    _css_class = "temporal-input datetime-input"
    _placeholder = "YYYY-MM-DDTHH:MM:SS"
    _value_name = "DateTimeInput"

    def _normalize(self, value: object) -> str:
        if value is None:
            return ""
        if isinstance(value, _DateTime):
            return _format_datetime_iso(value)
        text = str(value).strip()
        if not text:
            return ""
        try:
            return _format_datetime_iso(_DateTime.fromisoformat(text))
        except ValueError as exc:
            raise ValueError(
                "DateTimeInput value must be an ISO datetime string (YYYY-MM-DDTHH:MM:SS)"
            ) from exc


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


class RangeSlider(Widget):
    """Two-thumb slider for selecting a bounded numeric interval."""

    kind = "range_slider"

    def __init__(
        self,
        value: Sequence[float] = (0, 1),
        *,
        min: float = 0,
        max: float = 1,
        step: float = 0.01,
        on_change: RangeCallback | None = None,
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
        if not math.isfinite(min_value):
            raise ValueError("RangeSlider min must be finite")
        if not math.isfinite(max_value):
            raise ValueError("RangeSlider max must be finite")
        if max_value < min_value:
            raise ValueError("RangeSlider max must be greater than or equal to min")
        step_value = float(step)
        if step_value <= 0 or not math.isfinite(step_value):
            raise ValueError("RangeSlider step must be greater than zero")
        self.min = min_value
        self.max = max_value
        self.step = step_value
        self.value = self._normalize_value(value)
        self.on_change = on_change
        self.disabled = bool(disabled)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def _normalize_value(self, value: Sequence[float]) -> tuple[float, float]:
        if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
            raise TypeError("RangeSlider value must be a two-item sequence")
        if len(value) != 2:
            raise ValueError("RangeSlider value must contain exactly two values")
        low = float(value[0])
        high = float(value[1])
        if not math.isfinite(low) or not math.isfinite(high):
            raise ValueError("RangeSlider values must be finite")
        low, high = sorted((low, high))
        return (self._clamp_value(low), self._clamp_value(high))

    def _clamp_value(self, value: float) -> float:
        if value < self.min:
            return self.min
        if value > self.max:
            return self.max
        return value

    def set_value(self, value: Sequence[float], *, notify: bool = False) -> None:
        self.value = self._normalize_value(value)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("value_min", self.value[0])
            handle.enqueue_set_prop("value_max", self.value[1])
        if notify and self.on_change is not None and not self.disabled:
            self.on_change(self.value)

    def props(self) -> dict[str, Any]:
        return {
            "value_min": self.value[0],
            "value_max": self.value[1],
            "min": self.min,
            "max": self.max,
            "step": self.step,
            "disabled": self.disabled,
            "events": ["change"] if self.on_change and not self.disabled else [],
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


class LoadingSpinner(Widget):
    kind = "loading_spinner"

    def __init__(
        self,
        *,
        size: float = 18,
        label: str | None = None,
        stroke_width: float | None = None,
        speed: float = 1.0,
        spinning: bool = True,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        size_value = float(size)
        if not math.isfinite(size_value) or size_value <= 0:
            raise ValueError("LoadingSpinner size must be positive")
        if stroke_width is None:
            stroke_value = None
        else:
            stroke_value = float(stroke_width)
            if not math.isfinite(stroke_value) or stroke_value <= 0:
                raise ValueError("LoadingSpinner stroke_width must be positive")
        speed_value = float(speed)
        if not math.isfinite(speed_value) or speed_value < 0:
            raise ValueError("LoadingSpinner speed must be non-negative")
        self.size = size_value
        self.label = None if label is None else str(label)
        self.stroke_width = stroke_value
        self.speed = speed_value
        self.spinning = bool(spinning)
        self.disabled = disabled
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def set_label(self, label: str | None) -> None:
        self.label = None if label is None else str(label)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("label", self.label)

    def set_spinning(self, spinning: bool) -> None:
        self.spinning = bool(spinning)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("spinning", self.spinning)

    def props(self) -> dict[str, Any]:
        return {
            "size": self.size,
            "label": self.label,
            "stroke_width": self.stroke_width,
            "speed": self.speed,
            "spinning": self.spinning,
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


class DragNumber(Widget):
    """Numeric field adjusted by horizontal pointer dragging."""

    kind = "drag_number"

    def __init__(
        self,
        value: float = 0,
        *,
        min: float | None = None,
        max: float | None = None,
        step: float = 1,
        speed: float | None = None,
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
            raise ValueError("DragNumber min must be finite")
        if max_value is not None and not math.isfinite(max_value):
            raise ValueError("DragNumber max must be finite")
        if min_value is not None and max_value is not None and max_value < min_value:
            raise ValueError("DragNumber max must be greater than or equal to min")
        step_value = float(step)
        if step_value <= 0 or not math.isfinite(step_value):
            raise ValueError("DragNumber step must be greater than zero")
        speed_value = step_value if speed is None else float(speed)
        if speed_value <= 0 or not math.isfinite(speed_value):
            raise ValueError("DragNumber speed must be greater than zero")
        value_f = float(value)
        if not math.isfinite(value_f):
            raise ValueError("DragNumber value must be finite")
        self.min = min_value
        self.max = max_value
        self.step = step_value
        self.speed = speed_value
        self.value = self._clamp_value(value_f)
        self.on_change = on_change
        self.disabled = bool(disabled)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def _clamp_value(self, value: float) -> float:
        if self.min is not None and value < self.min:
            return self.min
        if self.max is not None and value > self.max:
            return self.max
        return value

    def set_value(self, value: float, *, notify: bool = False) -> None:
        value_f = float(value)
        if not math.isfinite(value_f):
            raise ValueError("DragNumber value must be finite")
        self.value = self._clamp_value(value_f)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("value", self.value)
        if notify and not self.disabled and self.on_change is not None:
            self.on_change(self.value)

    def props(self) -> dict[str, Any]:
        return {
            "value": self.value,
            "min": self.min,
            "max": self.max,
            "step": self.step,
            "speed": self.speed,
            "text": _format_number(self.value),
            "disabled": self.disabled,
            "events": ["change"] if self.on_change and not self.disabled else [],
        }


class DragVector(FlowLayout):
    """Compact vector editor built from DragNumber components."""

    _DEFAULT_LABELS = ("X", "Y", "Z", "W")

    def __init__(
        self,
        value: Sequence[float],
        *,
        labels: Sequence[str] | None = None,
        min: float | Sequence[float] | None = None,
        max: float | Sequence[float] | None = None,
        step: float | Sequence[float] = 1,
        speed: float | Sequence[float] | None = None,
        on_change: Callable[[tuple[float, ...]], None] | None = None,
        disabled: bool = False,
        gap: int | float | None = 8,
        row_gap: int | float | None = 6,
        component_gap: int | float = 4,
        component_width: int | float | None = 88,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        values = tuple(float(v) for v in value)
        if not values:
            raise ValueError("DragVector value cannot be empty")
        if len(values) > len(self._DEFAULT_LABELS):
            raise ValueError("DragVector supports at most 4 components")
        if not all(math.isfinite(v) for v in values):
            raise ValueError("DragVector values must be finite")
        self.labels = tuple(str(label) for label in (labels or self._DEFAULT_LABELS[: len(values)]))
        if len(self.labels) != len(values):
            raise ValueError("DragVector labels must match value length")
        self.min_values = self._component_values(min, len(values), "min")
        self.max_values = self._component_values(max, len(values), "max")
        for min_value, max_value in zip(self.min_values, self.max_values, strict=True):
            if min_value is not None and max_value is not None and max_value < min_value:
                raise ValueError("DragVector max values must be greater than or equal to min values")
        self.value = tuple(
            self._clamp_component(component, min_value, max_value)
            for component, min_value, max_value in zip(values, self.min_values, self.max_values, strict=True)
        )
        self.step_values = self._component_values(step, len(values), "step", default=1.0)
        self.speed_values = self._component_values(
            speed,
            len(values),
            "speed",
            default=None,
            fallback=self.step_values,
        )
        if any(v is None or v <= 0 for v in self.step_values):
            raise ValueError("DragVector step values must be greater than zero")
        if any(v is None or v <= 0 for v in self.speed_values):
            raise ValueError("DragVector speed values must be greater than zero")
        self.on_change = on_change
        self.disabled = bool(disabled)
        self._number_widgets: list[DragNumber] = []
        self.component_gap = self._non_negative_finite(component_gap, "component_gap")
        self.component_width = (
            None if component_width is None else self._positive_finite(component_width, "component_width")
        )

        merged_style = dict(style or {})
        if gap is not None:
            merged_style.setdefault("gap", self._non_negative_finite(gap, "gap"))
        if row_gap is not None:
            merged_style.setdefault("row_gap", self._non_negative_finite(row_gap, "row_gap"))

        super().__init__(
            gap=None,
            row_gap=None,
            cross_align="center",
            id=id,
            key=key,
            class_=class_,
            style=merged_style or None,
            tooltip=tooltip,
            parent=parent,
        )
        self._sync_children(live=False)

    @staticmethod
    def _component_values(
        value: float | Sequence[float] | None,
        count: int,
        name: str,
        *,
        default: float | None = None,
        fallback: Sequence[float | None] | None = None,
    ) -> tuple[float | None, ...]:
        if value is None:
            if fallback is not None:
                return tuple(fallback)
            return tuple(default for _ in range(count))
        if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
            values: tuple[float | None, ...] = tuple(float(value) for _ in range(count))
        else:
            if len(value) != count:
                raise ValueError(f"DragVector {name} values must match value length")
            values = tuple(float(v) if v is not None else None for v in value)
        if any(v is not None and not math.isfinite(v) for v in values):
            raise ValueError(f"DragVector {name} values must be finite")
        return values

    @staticmethod
    def _non_negative_finite(value: int | float, name: str) -> float:
        value_f = float(value)
        if not math.isfinite(value_f) or value_f < 0:
            raise ValueError(f"DragVector {name} must be a non-negative finite number")
        return value_f

    @staticmethod
    def _positive_finite(value: int | float, name: str) -> float:
        value_f = float(value)
        if not math.isfinite(value_f) or value_f <= 0:
            raise ValueError(f"DragVector {name} must be a positive finite number")
        return value_f

    @staticmethod
    def _clamp_component(value: float, min_value: float | None, max_value: float | None) -> float:
        if min_value is not None and value < min_value:
            return min_value
        if max_value is not None and value > max_value:
            return max_value
        return value

    def _handle_component_change(self, index: int, component_value: float) -> None:
        values = list(self.value)
        values[index] = float(component_value)
        self.value = tuple(values)
        if self.on_change is not None and not self.disabled:
            self.on_change(self.value)

    def _make_children(self) -> list[Widget]:
        children: list[Widget] = []
        numbers: list[DragNumber] = []
        for index, label in enumerate(self.labels):
            group = HLayout(
                class_="drag-vector-component",
                style={
                    "gap": self.component_gap,
                    "align_items": "center",
                    "flex_grow": 0,
                    "flex_shrink": 0,
                },
                parent=None,
            )
            Label(label, class_="drag-vector-label", parent=group)

            def on_number_change(value: float, index: int = index) -> None:
                self._handle_component_change(index, value)

            number_style = {"width": self.component_width} if self.component_width is not None else None
            number = DragNumber(
                self.value[index],
                min=self.min_values[index],
                max=self.max_values[index],
                step=self.step_values[index] or 1,
                speed=self.speed_values[index] or self.step_values[index] or 1,
                on_change=on_number_change,
                disabled=self.disabled,
                class_="drag-vector-value",
                style=number_style,
                parent=group,
            )
            numbers.append(number)
            children.append(group)
        self._number_widgets = numbers
        return children

    def _sync_children(self, *, live: bool) -> None:
        children = self._make_children()
        if live:
            self.replace_children(children)
            return
        self.children = []
        for child in children:
            self.add(child)

    def set_value(self, value: Sequence[float], *, notify: bool = False) -> None:
        values = tuple(float(v) for v in value)
        if len(values) != len(self.value):
            raise ValueError("DragVector value length cannot change")
        if not all(math.isfinite(v) for v in values):
            raise ValueError("DragVector values must be finite")
        next_values = tuple(
            self._clamp_component(component, min_value, max_value)
            for component, min_value, max_value in zip(values, self.min_values, self.max_values, strict=True)
        )
        self.value = next_values
        for number, component_value in zip(self._number_widgets, self.value, strict=True):
            number.set_value(component_value)
        if notify and self.on_change is not None and not self.disabled:
            self.on_change(self.value)


def _merge_widget_class(base: str, extra: str | None) -> str:
    return base if extra is None else f"{base} {extra}"


def _property_fill_style(extra: Mapping[str, object] | None = None) -> dict[str, object]:
    return {"width": 0, "flex": 1, "min_width": 0, **(extra or {})}


class Property(HLayout):
    """Single label/editor row for PropertyGrid."""

    def __init__(
        self,
        label: str,
        editor: Widget | None = None,
        *,
        label_width: int | float | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.label = str(label)
        if not self.label:
            raise ValueError("Property label cannot be empty")
        actual_parent = _BuildContext.parent() if parent is _AUTO_PARENT else parent
        grid = actual_parent if isinstance(actual_parent, PropertyGrid) else None
        width = grid.label_width if label_width is None and grid is not None else label_width
        if width is None:
            width = 140.0
        width_f = float(width)
        if not math.isfinite(width_f) or width_f <= 0:
            raise ValueError("Property label_width must be a positive finite number")
        row_style = {
            "gap": 10,
            "align_items": "center",
            "width": "100%",
            **(style or {}),
        }
        super().__init__(
            id=id,
            key=key,
            class_=_merge_widget_class("property-row", class_),
            style=row_style,
            tooltip=tooltip,
            parent=actual_parent,
        )
        Label(
            self.label,
            class_="property-label",
            style={"width": width_f, "flex_shrink": 0, "text_overflow": "ellipsis"},
            parent=self,
        )
        self._editor_slot = HLayout(
            class_="property-editor",
            style={
                "gap": 8,
                "align_items": "center",
                "flex": 1,
                "min_width": 0,
            },
            parent=self,
        )
        if editor is not None:
            self.set_editor(editor)

    def set_editor(self, editor: Widget) -> None:
        if not isinstance(editor, Widget):
            raise TypeError("Property editor must be a DragonGUI widget")
        self._editor_slot.add(editor)

    def __enter__(self) -> Self:
        _BuildContext.push(self._editor_slot)
        return self

    def __exit__(self, exc_type: object, exc: object, tb: object) -> None:
        _BuildContext.pop(self._editor_slot)


class PropertyGrid(VLayout):
    """Structured property inspector built from existing controls."""

    def __init__(
        self,
        values: Mapping[str, object] | None = None,
        *,
        schema: Mapping[str, Mapping[str, object]] | None = None,
        sections: Mapping[str, Iterable[str] | Mapping[str, object]] | None = None,
        on_change: Callable[[PropertyChange], None] | None = None,
        label_width: int | float = 140,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        label_width_f = float(label_width)
        if not math.isfinite(label_width_f) or label_width_f <= 0:
            raise ValueError("PropertyGrid label_width must be a positive finite number")
        self.values: dict[str, object] = {str(k): v for k, v in (values or {}).items()}
        self.schema: dict[str, dict[str, object]] = {
            str(k): dict(v) for k, v in (schema or {}).items()
        }
        self.sections = sections
        self.on_change = on_change
        self.label_width = label_width_f
        self.disabled = bool(disabled)
        self._editors: dict[str, Widget] = {}
        merged_style = {"gap": 6, "width": "100%", **(style or {})}
        super().__init__(
            id=id,
            key=key,
            class_=_merge_widget_class("property-grid", class_),
            style=merged_style,
            tooltip=tooltip,
            parent=parent,
        )
        if values is not None:
            self._sync_children(live=False)

    def editor(self, key: str) -> Widget | None:
        return self._editors.get(str(key))

    def set_value(self, key: str, value: object, *, notify: bool = False) -> None:
        key_s = str(key)
        if key_s not in self.values:
            raise KeyError(key_s)
        schema = self.schema.get(key_s, {})
        old_value = self.values.get(key_s)
        next_value = self._coerce_value(key_s, value, schema)
        self.values[key_s] = next_value
        self._set_editor_value(key_s, next_value)
        if notify and self.on_change is not None and not self.disabled:
            self.on_change(PropertyChange(key_s, next_value, old_value))

    def set_values(self, values: Mapping[str, object], *, notify: bool = False) -> None:
        for key, value in values.items():
            self.set_value(str(key), value, notify=notify)

    def _sync_children(self, *, live: bool) -> None:
        self._editors = {}
        children = self._make_children()
        if live:
            self.replace_children(children)
            return
        self.children = []
        for child in children:
            self.add(child)

    def _make_children(self) -> list[Widget]:
        if self.sections:
            return self._make_sectioned_children()
        grouped: dict[str | None, list[str]] = {}
        for key in self.values:
            section = self.schema.get(key, {}).get("section")
            section_key = None if section is None else str(section)
            grouped.setdefault(section_key, []).append(key)
        children: list[Widget] = []
        for section, keys in grouped.items():
            rows = [self._make_property_row(key) for key in keys]
            if section is None:
                children.extend(rows)
            else:
                children.append(self._make_section(section, rows))
        return children

    def _make_sectioned_children(self) -> list[Widget]:
        children: list[Widget] = []
        seen: set[str] = set()
        assert self.sections is not None
        for section, spec in self.sections.items():
            if isinstance(spec, Mapping):
                keys = []
                for raw_key, value in spec.items():
                    key = str(raw_key)
                    keys.append(key)
                    self.values.setdefault(key, value)
            else:
                keys = [str(key) for key in spec]
            seen.update(keys)
            children.append(self._make_section(str(section), [self._make_property_row(key) for key in keys]))
        remaining = [key for key in self.values if key not in seen]
        children.extend(self._make_property_row(key) for key in remaining)
        return children

    def _make_section(self, title: str, rows: list[Widget]) -> Collapsible:
        section = Collapsible(title, expanded=True, class_="property-section", parent=None)
        for row in rows:
            section.add(row)
        return section

    def _make_property_row(self, key: str) -> Property:
        if key not in self.values:
            raise KeyError(key)
        editor = self._make_editor(key, self.values[key], self.schema.get(key, {}))
        row = Property(
            self.schema.get(key, {}).get("label", key),
            editor,
            label_width=self.label_width,
            parent=None,
        )
        return row

    def _make_editor(self, key: str, value: object, schema: Mapping[str, object]) -> Widget:
        disabled = self.disabled or bool(schema.get("disabled", False))
        editor_kind = self._editor_kind(value, schema)

        def changed(next_value: object) -> None:
            self._handle_editor_change(key, next_value)

        if editor_kind == "bool":
            editor = Checkbox("", checked=bool(value), on_change=changed, disabled=disabled, parent=None)
        elif editor_kind == "select":
            options = self._schema_options(schema)
            selected = str(value) if value is not None else options[0]
            editor = Dropdown(
                options,
                value=selected,
                on_change=changed,
                disabled=disabled,
                style=_property_fill_style(),
                parent=None,
            )
        elif editor_kind == "range":
            low, high = self._range_pair(value)
            editor = RangeSlider(
                (low, high),
                min=float(schema.get("min", min(low, high, 0.0))),
                max=float(schema.get("max", max(low, high, 1.0))),
                step=float(schema.get("step", 0.01)),
                on_change=changed,
                disabled=disabled,
                style=_property_fill_style(),
                parent=None,
            )
        elif editor_kind == "slider":
            editor = Slider(
                float(value),
                min=float(schema.get("min", 0.0)),
                max=float(schema.get("max", 1.0)),
                step=float(schema.get("step", 0.01)),
                on_change=changed,
                disabled=disabled,
                style=_property_fill_style(),
                parent=None,
            )
        elif editor_kind == "number":
            editor = NumberInput(
                float(value),
                min=self._optional_float(schema.get("min")),
                max=self._optional_float(schema.get("max")),
                step=float(schema.get("step", 1.0)),
                on_change=changed,
                disabled=disabled,
                style=_property_fill_style(),
                parent=None,
            )
        elif editor_kind == "float" or editor_kind == "int":
            editor = DragNumber(
                float(value),
                min=self._optional_float(schema.get("min")),
                max=self._optional_float(schema.get("max")),
                step=float(schema.get("step", 1.0 if editor_kind == "int" else 0.01)),
                speed=self._optional_float(schema.get("speed")),
                on_change=changed,
                disabled=disabled,
                style=_property_fill_style(),
                parent=None,
            )
        elif editor_kind == "color":
            editor = self._make_color_editor(str(value), changed, disabled)
        elif editor_kind == "multiline":
            editor = TextArea(
                str(value),
                rows=int(schema.get("rows", 4)),
                on_change=changed,
                disabled=disabled,
                style=_property_fill_style(),
                parent=None,
            )
        elif editor_kind == "readonly":
            editor = Label(str(value), class_="property-value", style=_property_fill_style(), parent=None)
        else:
            editor = TextInput(
                str(value),
                on_change=changed,
                disabled=disabled,
                style=_property_fill_style(),
                parent=None,
            )
        self._editors[key] = editor
        return editor

    def _make_color_editor(
        self,
        value: str,
        changed: Callable[[object], None],
        disabled: bool,
    ) -> HLayout:
        row = HLayout(
            class_="property-color-editor",
            style={"gap": 8, "align_items": "center", "width": "100%", "min_width": 0},
            parent=None,
        )
        swatch = LED(True, states={"on": value}, size=16, parent=row)

        def color_changed(next_value: str) -> None:
            swatch.set_color(next_value)
            changed(next_value)

        text = TextInput(
            value,
            on_change=color_changed,
            disabled=disabled,
            style={"width": 0, "flex": 1, "min_width": 0},
            parent=row,
        )
        return row

    @staticmethod
    def _optional_float(value: object) -> float | None:
        if value is None:
            return None
        return float(value)

    @staticmethod
    def _schema_options(schema: Mapping[str, object]) -> list[str]:
        raw = schema.get("options", schema.get("items"))
        if raw is None:
            raise ValueError("PropertyGrid select properties require options")
        options = [str(item[1] if isinstance(item, tuple) and len(item) == 2 else item) for item in raw]  # type: ignore[union-attr]
        if not options:
            raise ValueError("PropertyGrid select options cannot be empty")
        return options

    @staticmethod
    def _range_pair(value: object) -> tuple[float, float]:
        if isinstance(value, (str, bytes, bytearray)) or not isinstance(value, Sequence):
            raise TypeError("PropertyGrid range values must be a two-item sequence")
        if len(value) != 2:
            raise ValueError("PropertyGrid range values must contain exactly two values")
        return float(value[0]), float(value[1])

    @staticmethod
    def _editor_kind(value: object, schema: Mapping[str, object]) -> str:
        explicit = schema.get("editor", schema.get("type"))
        if explicit is not None:
            kind = str(explicit).strip().lower().replace("_", "-")
            aliases = {
                "boolean": "bool",
                "dropdown": "select",
                "choice": "select",
                "drag": "float",
                "drag-number": "float",
                "text": "string",
                "str": "string",
                "read-only": "readonly",
            }
            return aliases.get(kind, kind)
        if "options" in schema or "items" in schema:
            return "select"
        if isinstance(value, bool):
            return "bool"
        if isinstance(value, int) and not isinstance(value, bool):
            return "int"
        if isinstance(value, float):
            return "float"
        if (
            isinstance(value, Sequence)
            and not isinstance(value, (str, bytes, bytearray))
            and len(value) == 2
            and all(isinstance(item, numbers.Real) for item in value)
        ):
            return "range"
        return "string"

    def _coerce_value(self, key: str, value: object, schema: Mapping[str, object]) -> object:
        kind = self._editor_kind(self.values.get(key), schema)
        if kind == "bool":
            return bool(value)
        if kind == "int":
            return int(round(float(value)))
        if kind in {"float", "slider", "number"}:
            return float(value)
        if kind == "range":
            return self._range_pair(value)
        if kind == "select":
            return str(value)
        if kind in {"color", "string", "multiline"}:
            return str(value)
        return value

    def _handle_editor_change(self, key: str, value: object) -> None:
        if self.disabled:
            return
        old_value = self.values.get(key)
        next_value = self._coerce_value(key, value, self.schema.get(key, {}))
        self.values[key] = next_value
        if self.on_change is not None:
            self.on_change(PropertyChange(key, next_value, old_value))

    def _set_editor_value(self, key: str, value: object) -> None:
        editor = self._editors.get(key)
        if editor is None:
            return
        if isinstance(editor, Checkbox):
            editor.set_checked(bool(value))
        elif isinstance(editor, (TextInput, TextArea)):
            editor.set_value(str(value))
        elif isinstance(editor, Dropdown):
            editor.set_value(str(value))
        elif isinstance(editor, (Slider, NumberInput, DragNumber)):
            editor.set_value(float(value))
        elif isinstance(editor, RangeSlider):
            editor.set_value(self._range_pair(value))
        elif isinstance(editor, HLayout):
            for child in editor.children:
                if isinstance(child, LED):
                    child.set_color(str(value))
                elif isinstance(child, TextInput):
                    child.set_value(str(value))


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


class ToggleSwitch(Widget):
    kind = "toggle_switch"

    _LABEL_POSITIONS = {"left", "right"}

    def __init__(
        self,
        label: str,
        *,
        checked: bool = False,
        on_change: "BoolCallback | None" = None,
        disabled: bool = False,
        label_position: str = "right",
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        position = str(label_position).strip().lower()
        if position not in self._LABEL_POSITIONS:
            raise ValueError("ToggleSwitch label_position must be 'left' or 'right'")
        self.label = label
        self.checked = bool(checked)
        self.on_change = on_change
        self.disabled = bool(disabled)
        self.label_position = position
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
            "label_position": self.label_position,
            "events": ["change"] if self.on_change and not self.disabled else [],
        }


class SelectableList(VLayout):
    """Dense single- or multi-selection list built from Selectable rows."""

    _MODES = {"single", "multiple"}

    def __init__(
        self,
        items: Iterable[object],
        *,
        value: str | None = None,
        selected: Iterable[str] | None = None,
        selection_mode: str = "single",
        on_change: Callable[[object], None] | None = None,
        disabled: bool = False,
        max_height: int | float | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        mode = selection_mode.strip().lower()
        if mode not in self._MODES:
            raise ValueError("SelectableList selection_mode must be 'single' or 'multiple'")
        self.items = [self._normalize_item(item) for item in items]
        if not self.items:
            raise ValueError("SelectableList items cannot be empty")
        values = [item[1] for item in self.items]
        if len(set(values)) != len(values):
            raise ValueError("SelectableList item values must be unique")

        self.selection_mode = mode
        self.on_change = on_change
        self.disabled = bool(disabled)
        self._option_widgets: list[Selectable] = []

        if mode == "single":
            self.value: str | None = values[0] if value is None else str(value)
            self._selected_values: set[str] = set()
            if self.value is not None:
                self._validate_value(self.value)
                self._selected_values.add(self.value)
        else:
            if value is not None:
                raise ValueError("SelectableList value is only valid in single selection mode")
            selected_values = {str(item) for item in (selected or ())}
            self._validate_values(selected_values)
            self.value = None
            self._selected_values = selected_values

        merged_style = dict(style or {})
        if max_height is not None:
            height = float(max_height)
            if not math.isfinite(height) or height <= 0:
                raise ValueError("SelectableList max_height must be a positive finite number")
            merged_style.setdefault("max_height", height)
            merged_style.setdefault("overflow_y", "auto")
            merged_style.setdefault("min_height", 0)

        super().__init__(
            id=id,
            key=key,
            class_=_merge_widget_class(f"selectable-list selectable-list-{mode}", class_),
            style=merged_style or None,
            tooltip=tooltip,
            parent=parent,
        )
        self._sync_children(live=False)

    @property
    def selected(self) -> set[str]:
        return set(self._selected_values)

    @staticmethod
    def _normalize_item(item: object) -> tuple[str, str, bool]:
        if isinstance(item, Mapping):
            raw_label = item.get("label", item.get("text", item.get("value")))
            if raw_label is None:
                raise ValueError("SelectableList item mappings require label, text, or value")
            label = str(raw_label)
            value = str(item.get("value", label))
            disabled = bool(item.get("disabled", False))
            return label, value, disabled
        if isinstance(item, tuple) and len(item) == 2:
            label, value = item
            return str(label), str(value), False
        label = str(item)
        return label, label, False

    def _validate_value(self, value: str) -> None:
        if value not in {item[1] for item in self.items}:
            raise ValueError("SelectableList value must be one of its item values")

    def _validate_values(self, values: set[str]) -> None:
        valid = {item[1] for item in self.items}
        missing = values - valid
        if missing:
            raise ValueError("SelectableList selected values must be item values")

    def _ordered_selected(self) -> tuple[str, ...]:
        return tuple(value for _, value, _ in self.items if value in self._selected_values)

    def _notify_payload(self) -> object:
        if self.selection_mode == "single":
            return self.value
        return self._ordered_selected()

    def _handle_item_select(self, value: str, selected: bool) -> None:
        if self.disabled:
            return
        if self.selection_mode == "single":
            changed = value != self.value or not self._selected_values
            self.set_value(value, notify=changed)
            return

        next_selected = set(self._selected_values)
        if selected:
            next_selected.add(value)
        else:
            next_selected.discard(value)
        self.set_selected(next_selected, notify=next_selected != self._selected_values)

    def _make_widgets(self) -> list[Selectable]:
        toggle = self.selection_mode == "multiple"
        widgets: list[Selectable] = []
        for label, value, item_disabled in self.items:
            def on_select(selected: bool, value: str = value) -> None:
                self._handle_item_select(value, selected)

            widgets.append(
                Selectable(
                    label,
                    value=value,
                    selected=value in self._selected_values,
                    toggle=toggle,
                    on_select=on_select,
                    disabled=self.disabled or item_disabled,
                    parent=None,
                )
            )
        return widgets

    def _sync_children(self, *, live: bool) -> None:
        widgets = self._make_widgets()
        self._option_widgets = widgets
        if live:
            self.replace_children(widgets)
            return
        self.children = []
        for widget in widgets:
            self.add(widget)

    def set_value(self, value: str, *, notify: bool = False) -> None:
        if self.selection_mode != "single":
            raise RuntimeError("SelectableList.set_value is only valid in single selection mode")
        selected = str(value)
        self._validate_value(selected)
        self.value = selected
        self._selected_values = {selected}
        for widget in self._option_widgets:
            widget.set_selected(widget.value == selected)
        if notify and self.on_change is not None:
            self.on_change(self._notify_payload())

    def set_selected(self, values: Iterable[str], *, notify: bool = False) -> None:
        if self.selection_mode != "multiple":
            raise RuntimeError("SelectableList.set_selected is only valid in multiple selection mode")
        selected_values = {str(value) for value in values}
        self._validate_values(selected_values)
        self._selected_values = selected_values
        for widget in self._option_widgets:
            widget.set_selected(widget.value in self._selected_values)
        if notify and self.on_change is not None:
            self.on_change(self._notify_payload())

    def clear_selection(self, *, notify: bool = False) -> None:
        if self.selection_mode == "single":
            self.value = None
        self._selected_values = set()
        for widget in self._option_widgets:
            widget.set_selected(False)
        if notify and self.on_change is not None:
            self.on_change(self._notify_payload())


@dataclass(frozen=True)
class BreadcrumbItem:
    label: str
    value: object
    disabled: bool = False


class Breadcrumbs(HLayout):
    """Compact path navigation built from existing DragonGUI controls."""

    def __init__(
        self,
        items: Iterable[object],
        *,
        current: int | object | None = None,
        separator: str = ">",
        max_items: int | None = None,
        click_current: bool = False,
        on_select: BreadcrumbCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.items = self._normalize_items(items)
        self.separator = str(separator)
        if not self.separator:
            raise ValueError("Breadcrumbs separator must be non-empty")
        self.max_items = self._normalize_max_items(max_items)
        self.click_current = bool(click_current)
        self.on_select = on_select
        self.disabled = bool(disabled)
        self.current_index = self._normalize_current(current)
        merged_style = {
            "gap": 4,
            "align_items": "center",
            "width": "100%",
            "min_width": 0,
            **(style or {}),
        }
        super().__init__(
            id=id,
            key=key,
            class_=_merge_widget_class("breadcrumbs", class_),
            style=merged_style,
            tooltip=tooltip,
            parent=parent,
        )
        self._sync_children(live=False)

    @staticmethod
    def _normalize_items(items: Iterable[object]) -> list[BreadcrumbItem]:
        normalized: list[BreadcrumbItem] = []
        for item in items:
            if isinstance(item, BreadcrumbItem):
                label = item.label
                value = item.value
                disabled = item.disabled
            elif isinstance(item, Mapping):
                raw_label = item.get("label", item.get("text", item.get("value")))
                if raw_label is None:
                    raise ValueError("Breadcrumbs item mappings require label, text, or value")
                label = str(raw_label).strip()
                value = item.get("value", label)
                disabled = bool(item.get("disabled", False))
            elif isinstance(item, tuple) and len(item) == 2:
                raw_label, value = item
                label = str(raw_label).strip()
                disabled = False
            else:
                label = str(item).strip()
                value = label
                disabled = False
            if not label:
                raise ValueError("Breadcrumbs item labels must be non-empty")
            normalized.append(BreadcrumbItem(label, value, bool(disabled)))
        if not normalized:
            raise ValueError("Breadcrumbs items cannot be empty")
        return normalized

    @staticmethod
    def _normalize_max_items(max_items: int | None) -> int | None:
        if max_items is None:
            return None
        count = int(max_items)
        if count < 3:
            raise ValueError("Breadcrumbs max_items must be at least 3")
        return count

    def _normalize_current(self, current: int | object | None) -> int:
        if current is None:
            return len(self.items) - 1
        if isinstance(current, int):
            index = current + len(self.items) if current < 0 else current
            if index < 0 or index >= len(self.items):
                raise ValueError("Breadcrumbs current index is out of range")
            return index
        for index, item in enumerate(self.items):
            if item.value == current:
                return index
        raise ValueError("Breadcrumbs current value must match an item value")

    def _visible_entries(self) -> list[int | None]:
        if self.max_items is None or len(self.items) <= self.max_items:
            return list(range(len(self.items)))
        tail_count = self.max_items - 2
        tail_start = len(self.items) - tail_count
        return [0, None, *range(tail_start, len(self.items))]

    def _selection_payload(self, index: int) -> BreadcrumbSelection:
        item = self.items[index]
        return BreadcrumbSelection(index=index, label=item.label, value=item.value)

    def _segment_class(self, index: int) -> str:
        classes = ["breadcrumb-current" if index == self.current_index else "breadcrumb-item"]
        if self.items[index].disabled:
            classes.append("breadcrumb-disabled")
        return " ".join(classes)

    def _segment_clickable(self, index: int) -> bool:
        if self.disabled or self.items[index].disabled:
            return False
        return self.click_current or index != self.current_index

    def _sync_children(self, *, live: bool) -> None:
        children: list[Widget] = []
        for position, entry in enumerate(self._visible_entries()):
            if position > 0:
                children.append(Label(self.separator, class_="breadcrumb-separator", wrap=False, parent=None))
            if entry is None:
                children.append(Label("...", class_="breadcrumb-overflow", wrap=False, parent=None))
                continue
            item = self.items[entry]
            segment_class = self._segment_class(entry)
            if self._segment_clickable(entry):
                def on_click(index: int = entry) -> None:
                    self.select(index, notify=True)

                children.append(
                    SmallButton(
                        item.label,
                        on_click=on_click,
                        class_=segment_class,
                        tooltip=str(item.value) if item.value != item.label else None,
                        parent=None,
                    )
                )
            else:
                children.append(Label(item.label, class_=segment_class, wrap=False, parent=None))
        if live:
            self.replace_children(children)
            return
        self.children = []
        for child in children:
            self.add(child)

    def select(self, index: int, *, notify: bool = True) -> None:
        selected = self._normalize_current(index)
        if self.disabled or self.items[selected].disabled:
            return
        self.current_index = selected
        self._sync_children(live=self.is_live)
        if notify and self.on_select is not None:
            self.on_select(self._selection_payload(selected))

    def set_current(self, current: int | object, *, notify: bool = False) -> None:
        selected = self._normalize_current(current)
        self.current_index = selected
        self._sync_children(live=self.is_live)
        if notify and self.on_select is not None:
            self.on_select(self._selection_payload(selected))

    def set_items(
        self,
        items: Iterable[object],
        *,
        current: int | object | None = None,
    ) -> None:
        self.items = self._normalize_items(items)
        self.current_index = self._normalize_current(current)
        self._sync_children(live=self.is_live)


class SearchBox(HLayout):
    """Search input with leading search affordance and a clear button."""

    def __init__(
        self,
        value: str = "",
        *,
        placeholder: str = "Search...",
        on_change: StringCallback | None = None,
        disabled: bool = False,
        clearable: bool = True,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.value = str(value)
        self.placeholder = str(placeholder)
        self.on_change = on_change
        self.disabled = bool(disabled)
        self.clearable = bool(clearable)
        self.input: TextInput | None = None
        self.clear_button: IconButton | None = None
        merged_style = {
            "gap": 6,
            "align_items": "center",
            "height": 38,
            "flex_grow": 0,
            "flex_shrink": 0,
            "width": "100%",
            **(style or {}),
        }
        super().__init__(
            id=id,
            key=key,
            class_=_merge_widget_class("search-box", class_),
            style=merged_style,
            tooltip=tooltip,
            parent=parent,
        )
        IconButton(
            "search",
            disabled=True,
            size=28,
            class_="search-box-icon",
            parent=self,
        )
        self.input = TextInput(
            self.value,
            placeholder=self.placeholder,
            on_change=self._handle_input_change,
            disabled=self.disabled,
            class_="search-box-input",
            style={"flex": 1, "min_width": 0},
            parent=self,
        )
        if self.clearable:
            self.clear_button = IconButton(
                "close",
                on_click=self.clear,
                disabled=self.disabled,
                size=28,
                class_="search-box-clear",
                tooltip="Clear",
                parent=self,
            )

    def _handle_input_change(self, value: str) -> None:
        self.value = str(value)
        if self.on_change is not None and not self.disabled:
            self.on_change(self.value)

    def set_value(self, value: str, *, notify: bool = False) -> None:
        self.value = str(value)
        if self.input is not None:
            self.input.set_value(self.value)
        if notify and self.on_change is not None and not self.disabled:
            self.on_change(self.value)

    def clear(self, *, notify: bool = True) -> None:
        self.set_value("", notify=notify)


@dataclass
class Command:
    """CommandPalette action metadata."""

    id: str
    title: str
    on_run: Callback | None = None
    subtitle: str | None = None
    keywords: Iterable[str] = ()
    disabled: bool = False

    def __post_init__(self) -> None:
        command_id = str(self.id).strip()
        title = str(self.title).strip()
        if not command_id:
            raise ValueError("Command id must be a non-empty string")
        if not title:
            raise ValueError("Command title must be a non-empty string")
        self.id = command_id
        self.title = title
        self.subtitle = None if self.subtitle is None else str(self.subtitle)
        if self.keywords is None:
            keywords: Iterable[object] = ()
        elif isinstance(self.keywords, str):
            keywords = (self.keywords,)
        else:
            keywords = self.keywords
        self.keywords = tuple(str(keyword) for keyword in keywords)
        self.disabled = bool(self.disabled)

    def matches(self, query: str) -> bool:
        needle = query.strip().lower()
        if not needle:
            return True
        haystack = " ".join(
            (
                self.id,
                self.title,
                self.subtitle or "",
                *self.keywords,
            )
        ).lower()
        return needle in haystack


class CommandPalette(Modal):
    """Searchable command launcher built from SearchBox and Selectable rows."""

    def __init__(
        self,
        commands: Iterable[Command | Mapping[str, object]],
        *,
        open: bool = False,
        title: str = "Command Palette",
        value: str = "",
        placeholder: str = "Search commands...",
        width: int | float = 520,
        height: int | float = 360,
        max_results: int | None = None,
        close_on_run: bool = True,
        on_run: Callable[[Command], None] | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.commands = [self._normalize_command(command) for command in commands]
        command_ids = [command.id for command in self.commands]
        if len(set(command_ids)) != len(command_ids):
            raise ValueError("CommandPalette command ids must be unique")
        self.query = str(value)
        self.placeholder = str(placeholder)
        self.max_results = None if max_results is None else int(max_results)
        if self.max_results is not None and self.max_results <= 0:
            raise ValueError("CommandPalette max_results must be greater than zero")
        self.close_on_run = bool(close_on_run)
        self.on_run = on_run
        self.selected: str | None = None
        self.search_box: SearchBox | None = None
        self.results: VLayout | None = None
        merged_style = {
            "gap": 10,
            **(style or {}),
        }
        super().__init__(
            title,
            open=open,
            width=width,
            height=height,
            close_button=True,
            id=id,
            key=key,
            class_=_merge_widget_class("command-palette", class_),
            style=merged_style,
            tooltip=tooltip,
            parent=parent,
        )
        self.search_box = SearchBox(
            self.query,
            placeholder=self.placeholder,
            on_change=self._handle_query_change,
            class_="command-palette-search",
            style={"width": "100%", "height": 38},
            parent=self,
        )
        self.results = VLayout(
            class_="command-palette-results",
            style={"gap": 3, "width": "100%", "flex": 1, "min_height": 0, "overflow_y": "auto"},
            parent=self,
        )
        self._sync_results(live=False)

    @staticmethod
    def _normalize_command(command: Command | Mapping[str, object]) -> Command:
        if isinstance(command, Command):
            return command
        if not isinstance(command, Mapping):
            raise TypeError("CommandPalette commands must be Command objects or mappings")
        if "id" not in command or "title" not in command:
            raise ValueError("Command mappings require id and title")
        on_run = command.get("on_run")
        if on_run is not None and not callable(on_run):
            raise TypeError("Command on_run must be callable")
        return Command(
            str(command["id"]),
            str(command["title"]),
            on_run=on_run,  # type: ignore[arg-type]
            subtitle=None if command.get("subtitle") is None else str(command.get("subtitle")),
            keywords=command.get("keywords", ()),  # type: ignore[arg-type]
            disabled=bool(command.get("disabled", False)),
        )

    def filtered_commands(self) -> tuple[Command, ...]:
        matches = [command for command in self.commands if command.matches(self.query)]
        if self.max_results is not None:
            matches = matches[: self.max_results]
        return tuple(matches)

    def _ensure_selected(self, commands: Sequence[Command]) -> None:
        enabled_ids = [command.id for command in commands if not command.disabled]
        if self.selected not in enabled_ids:
            self.selected = enabled_ids[0] if enabled_ids else None

    def _row_text(self, command: Command) -> str:
        if command.subtitle:
            return f"{command.title} - {command.subtitle}"
        return command.title

    def _make_result_rows(self) -> list[Widget]:
        commands = list(self.filtered_commands())
        self._ensure_selected(commands)
        if not commands:
            return [
                Label(
                    "No commands",
                    class_="command-palette-empty",
                    style={"height": 34, "color": "muted", "align_self": "center"},
                    parent=None,
                )
            ]
        rows: list[Widget] = []
        for command in commands:
            def on_select(selected: bool, command_id: str = command.id) -> None:
                if selected:
                    self.run(command_id)

            rows.append(
                Selectable(
                    self._row_text(command),
                    value=command.id,
                    selected=command.id == self.selected,
                    toggle=False,
                    on_select=on_select,
                    disabled=command.disabled,
                    class_="command-palette-row",
                    parent=None,
                )
            )
        return rows

    def _sync_results(self, *, live: bool) -> None:
        if self.results is None:
            return
        rows = self._make_result_rows()
        if live:
            self.results.replace_children(rows)
            return
        self.results.children = []
        for row in rows:
            self.results.add(row)

    def _handle_query_change(self, query: str) -> None:
        self.query = str(query)
        self._sync_results(live=self.results is not None and self.results.is_live)

    def set_query(self, query: str) -> None:
        self.query = str(query)
        if self.search_box is not None:
            self.search_box.set_value(self.query)
        self._sync_results(live=self.results is not None and self.results.is_live)

    def set_commands(self, commands: Iterable[Command | Mapping[str, object]]) -> None:
        self.commands = [self._normalize_command(command) for command in commands]
        command_ids = [command.id for command in self.commands]
        if len(set(command_ids)) != len(command_ids):
            raise ValueError("CommandPalette command ids must be unique")
        self._sync_results(live=self.results is not None and self.results.is_live)

    def run_selected(self) -> None:
        if self.selected is not None:
            self.run(self.selected)

    def run(self, command_id: str) -> None:
        selected = str(command_id)
        command = next((command for command in self.commands if command.id == selected), None)
        if command is None:
            raise ValueError("CommandPalette command_id must match a command")
        if command.disabled:
            return
        self.selected = command.id
        if command.on_run is not None:
            command.on_run()
        if self.on_run is not None:
            self.on_run(command)
        if self.close_on_run:
            self.close()


class RadioGroup(VLayout):
    """Mutually exclusive choice group built from RadioButton rows."""

    _ORIENTATIONS = {"vertical", "horizontal"}

    def __init__(
        self,
        items: Iterable[object],
        *,
        value: str | None = None,
        orientation: str = "vertical",
        on_change: StringCallback | None = None,
        disabled: bool = False,
        gap: int | float | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        orientation_value = orientation.strip().lower()
        if orientation_value not in self._ORIENTATIONS:
            raise ValueError("RadioGroup orientation must be 'vertical' or 'horizontal'")
        self.items = [self._normalize_item(item) for item in items]
        if not self.items:
            raise ValueError("RadioGroup items cannot be empty")
        values = [item[1] for item in self.items]
        if len(set(values)) != len(values):
            raise ValueError("RadioGroup item values must be unique")

        selected = values[0] if value is None else str(value)
        self._validate_value(selected)
        self.value = selected
        self.orientation = orientation_value
        self.on_change = on_change
        self.disabled = bool(disabled)
        self._button_widgets: list[RadioButton] = []

        merged_style = dict(style or {})
        if orientation_value == "horizontal":
            merged_style.setdefault("display", "flex")
            merged_style.setdefault("flex_direction", "row")
            merged_style.setdefault("align_items", "center")
            merged_style.setdefault("height", "auto")
        if gap is not None:
            gap_value = float(gap)
            if not math.isfinite(gap_value) or gap_value < 0:
                raise ValueError("RadioGroup gap must be a non-negative finite number")
            merged_style.setdefault("gap", gap_value)

        super().__init__(
            id=id,
            key=key,
            class_=_merge_widget_class(
                f"radio-group radio-group-{orientation_value}",
                class_,
            ),
            style=merged_style or None,
            tooltip=tooltip,
            parent=parent,
        )
        self._sync_children(live=False)

    @staticmethod
    def _normalize_item(item: object) -> tuple[str, str, bool]:
        if isinstance(item, Mapping):
            raw_label = item.get("label", item.get("text", item.get("value")))
            if raw_label is None:
                raise ValueError("RadioGroup item mappings require label, text, or value")
            label = str(raw_label)
            value = str(item.get("value", label))
            disabled = bool(item.get("disabled", False))
            return label, value, disabled
        if isinstance(item, tuple) and len(item) == 2:
            label, value = item
            return str(label), str(value), False
        label = str(item)
        return label, label, False

    def _validate_value(self, value: str) -> None:
        if value not in {item[1] for item in self.items}:
            raise ValueError("RadioGroup value must be one of its item values")

    def _handle_button_change(self, value: str, checked: bool) -> None:
        if self.disabled or not checked:
            return
        self.set_value(value, notify=value != self.value)

    def _make_buttons(self) -> list[RadioButton]:
        buttons: list[RadioButton] = []
        for label, value, item_disabled in self.items:
            def on_change(checked: bool, value: str = value) -> None:
                self._handle_button_change(value, checked)

            buttons.append(
                RadioButton(
                    label,
                    value=value,
                    checked=value == self.value,
                    on_change=on_change,
                    disabled=self.disabled or item_disabled,
                    parent=None,
                )
            )
        return buttons

    def _sync_children(self, *, live: bool) -> None:
        buttons = self._make_buttons()
        self._button_widgets = buttons
        if live:
            self.replace_children(buttons)
            return
        self.children = []
        for button in buttons:
            self.add(button)

    def set_value(self, value: str, *, notify: bool = False) -> None:
        selected = str(value)
        self._validate_value(selected)
        self.value = selected
        for button in self._button_widgets:
            button.set_checked(button.value == selected)
        if notify and self.on_change is not None:
            self.on_change(self.value)


class TreeNode(Container):
    """Dense hierarchical row for TreeView."""

    kind = "tree_node"

    def __init__(
        self,
        label: str,
        *,
        node_id: str | None = None,
        expanded: bool = False,
        selected: bool = False,
        leaf: bool = False,
        on_select: BoolCallback | None = None,
        on_expand: BoolCallback | None = None,
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
            raise ValueError("TreeNode label cannot be empty")
        self.node_id = self.label if node_id is None else str(node_id)
        if not self.node_id:
            raise ValueError("TreeNode node_id cannot be empty")
        self.expanded = bool(expanded)
        self.selected = bool(selected)
        self.leaf = bool(leaf)
        self.on_select = on_select
        self.on_expand = on_expand
        self.disabled = bool(disabled)
        self._tree_view: TreeView | None = None
        super().__init__(
            id=id,
            key=key,
            class_=class_,
            style=style,
            tooltip=tooltip,
            parent=parent,
        )
        self._attach_tree_view_from_parent()

    def _attach_tree_view_from_parent(self) -> None:
        parent = self.parent
        while parent is not None:
            if isinstance(parent, TreeView):
                parent._wire_node(self)
                return
            parent = parent.parent

    def add(self, child: Widget) -> Widget:
        added = super().add(child)
        if isinstance(added, TreeNode):
            added._tree_view = self._tree_view
            if self._tree_view is not None:
                self._tree_view._wire_node(added)
        return added

    def set_expanded(self, expanded: bool, *, notify: bool = False) -> None:
        self.expanded = bool(expanded)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("expanded", self.expanded)
        if notify and not self.disabled and self.on_expand is not None:
            self.on_expand(self.expanded)

    def expand(self, *, notify: bool = False) -> None:
        self.set_expanded(True, notify=notify)

    def collapse(self, *, notify: bool = False) -> None:
        self.set_expanded(False, notify=notify)

    def toggle(self, *, notify: bool = False) -> None:
        self.set_expanded(not self.expanded, notify=notify)

    def set_selected(self, selected: bool, *, notify: bool = False) -> None:
        self.selected = bool(selected)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("checked", self.selected)
        if notify and not self.disabled and self.on_select is not None:
            self.on_select(self.selected)

    def select(self, *, notify: bool = False) -> None:
        if self._tree_view is not None:
            self._tree_view.set_selected(self.node_id, notify=notify)
            return
        self.set_selected(True, notify=notify)

    def _handle_native_event(self, value: object) -> None:
        payload = json.loads(value) if isinstance(value, str) else value
        if not isinstance(payload, Mapping):
            payload = {"event": "select", "selected": bool(value)}
        event = str(payload.get("event", "select"))
        if event == "expand":
            self.expanded = bool(payload.get("expanded", False))
            if self.on_expand is not None:
                self.on_expand(self.expanded)
            return
        if event == "select":
            self.selected = bool(payload.get("selected", True))
            if self._tree_view is not None and self.selected:
                self._tree_view._handle_node_selected(self, notify=True)
            elif self.on_select is not None:
                self.on_select(self.selected)

    def props(self) -> dict[str, Any]:
        return {
            "label": self.label,
            "value": self.node_id,
            "expanded": self.expanded,
            "checked": self.selected,
            "leaf": self.leaf,
            "disabled": self.disabled,
            "events": ["change"]
            if (self.on_select or self.on_expand or self._tree_view is not None)
            and not self.disabled
            else [],
        }


class TreeView(Container):
    """Hierarchical selectable tree container."""

    kind = "tree_view"

    def __init__(
        self,
        items: Iterable[object] | None = None,
        *,
        selected: str | None = None,
        on_select: StringCallback | None = None,
        disabled: bool = False,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.selected = None if selected is None else str(selected)
        self.on_select = on_select
        self.disabled = bool(disabled)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)
        if items is not None:
            for item in items:
                self.add(self._node_from_item(item))
        self._wire_descendants()
        if self.selected is not None and self._find_node(self.selected) is not None:
            self.set_selected(self.selected)

    def add(self, child: Widget) -> Widget:
        added = super().add(child)
        if isinstance(added, TreeNode):
            self._wire_node(added)
        return added

    def _wire_node(self, node: TreeNode) -> None:
        node._tree_view = self
        if self.disabled:
            node.disabled = True
        if node.selected:
            self.selected = node.node_id
        elif self.selected == node.node_id:
            node.selected = True
        for child in node.children:
            if isinstance(child, TreeNode):
                self._wire_node(child)

    def _wire_descendants(self) -> None:
        for child in self.children:
            if isinstance(child, TreeNode):
                self._wire_node(child)

    @staticmethod
    def _node_from_item(item: object) -> TreeNode:
        if isinstance(item, Mapping):
            raw_label = item.get("label", item.get("text", item.get("name", item.get("id"))))
            if raw_label is None:
                raise ValueError("TreeView item mappings require label, text, name, or id")
            label = str(raw_label)
            node_id = str(item.get("id", item.get("node_id", label)))
            children = item.get("children", ())
            node = TreeNode(
                label,
                node_id=node_id,
                expanded=bool(item.get("expanded", False)),
                selected=bool(item.get("selected", False)),
                leaf=bool(item.get("leaf", False)),
                disabled=bool(item.get("disabled", False)),
                parent=None,
            )
            if children is not None:
                for child in children:
                    node.add(TreeView._node_from_item(child))
            return node
        if isinstance(item, tuple) and len(item) == 2:
            label, node_id = item
            return TreeNode(str(label), node_id=str(node_id), leaf=True, parent=None)
        label = str(item)
        return TreeNode(label, node_id=label, leaf=True, parent=None)

    def _tree_nodes(self) -> list[TreeNode]:
        nodes: list[TreeNode] = []

        def walk(node: Widget) -> None:
            if isinstance(node, TreeNode):
                nodes.append(node)
            if isinstance(node, Container):
                for child in node.children:
                    walk(child)

        for child in self.children:
            walk(child)
        return nodes

    def _find_node(self, node_id: str) -> TreeNode | None:
        for node in self._tree_nodes():
            if node.node_id == node_id:
                return node
        return None

    def _handle_node_selected(self, node: TreeNode, *, notify: bool) -> None:
        self.selected = node.node_id
        for candidate in self._tree_nodes():
            candidate.set_selected(candidate is node)
        if notify and self.on_select is not None:
            self.on_select(node.node_id)

    def set_selected(self, node_id: str, *, notify: bool = False) -> None:
        selected = str(node_id)
        node = self._find_node(selected)
        if node is None:
            raise ValueError("TreeView selected node_id must exist in the tree")
        self._handle_node_selected(node, notify=notify)

    def clear_selection(self, *, notify: bool = False) -> None:
        self.selected = None
        for node in self._tree_nodes():
            node.set_selected(False)
        if notify and self.on_select is not None:
            self.on_select("")

    def props(self) -> dict[str, Any]:
        return {
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


class HtmlReport(Widget):
    """Display a local HTML report, with an external-browser fallback.

    The native renderer currently shows a styled placeholder. The API is shaped
    so the Windows WebView2 backend can consume the same serialized props later.
    """

    kind = "html_report"

    def __init__(
        self,
        path: object | None = None,
        *,
        html: str | None = None,
        base_dir: object | None = None,
        allow_remote: bool = False,
        allow_scripts: bool = True,
        external_fallback: bool = True,
        width: int | float | None = None,
        height: int | float | None = 420,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if path is None and html is None:
            raise ValueError("HtmlReport requires either path or html")
        if path is not None and html is not None:
            raise ValueError("HtmlReport accepts path or html, not both")
        self.path = None if path is None else self._normalize_path(path)
        self.html = None if html is None else self._normalize_html(html)
        self.base_dir = None if base_dir is None else self._normalize_path(base_dir)
        self.allow_remote = bool(allow_remote)
        self.allow_scripts = bool(allow_scripts)
        self.external_fallback = bool(external_fallback)
        self.width = None if width is None else float(width)
        self.height = None if height is None else float(height)
        if self.width is not None and self.width <= 0:
            raise ValueError("HtmlReport width must be greater than zero")
        if self.height is not None and self.height <= 0:
            raise ValueError("HtmlReport height must be greater than zero")
        self._external_temp_path: str | None = None
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    @classmethod
    def from_html(
        cls,
        html: str,
        *,
        base_dir: object | None = None,
        **kwargs: Any,
    ) -> Self:
        return cls(html=html, base_dir=base_dir, **kwargs)

    @staticmethod
    def _normalize_path(path: object) -> str:
        text = str(path)
        if not text:
            raise ValueError("HtmlReport path must be a non-empty path")
        return text

    @staticmethod
    def _normalize_html(html: str) -> str:
        text = str(html)
        if not text.strip():
            raise ValueError("HtmlReport html must be non-empty")
        return text

    def _placeholder_text(self) -> str:
        if self.path is not None:
            name = Path(self.path).name or self.path
            return f"HTML report: {name}\nOpen externally to view interactive content."
        return "HTML report: inline document\nOpen externally to view interactive content."

    def set_path(self, path: object) -> None:
        self.path = self._normalize_path(path)
        self.html = None
        self.base_dir = None
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("path", self.path)
            handle.enqueue_set_prop("html", None)
            handle.enqueue_set_prop("base_dir", None)
            handle.enqueue_set_prop("text", self._placeholder_text())

    def set_html(self, html: str, *, base_dir: object | None = None) -> None:
        self.html = self._normalize_html(html)
        self.path = None
        self.base_dir = None if base_dir is None else self._normalize_path(base_dir)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("html", self.html)
            handle.enqueue_set_prop("path", None)
            handle.enqueue_set_prop("base_dir", self.base_dir)
            handle.enqueue_set_prop("text", self._placeholder_text())

    def reload(self) -> None:
        if (handle := self._live()) is None:
            return
        if self.path is not None:
            handle.enqueue_set_prop("path", self.path)
        else:
            handle.enqueue_set_prop("html", self.html)

    def open_external(self) -> bool:
        if not self.external_fallback:
            return False
        if self.path is not None:
            target = self.path
            if not target.lower().startswith(("http://", "https://", "file://")):
                target = Path(target).expanduser().resolve().as_uri()
        else:
            if self._external_temp_path is None:
                with tempfile.NamedTemporaryFile(
                    "w",
                    suffix=".html",
                    prefix="dragongui-html-report-",
                    encoding="utf-8",
                    delete=False,
                ) as handle:
                    handle.write(self.html or "")
                    self._external_temp_path = handle.name
            target = Path(self._external_temp_path).resolve().as_uri()
        return bool(webbrowser.open(target))

    def props(self) -> dict[str, Any]:
        return {
            "path": self.path,
            "html": self.html,
            "base_dir": self.base_dir,
            "allow_remote": self.allow_remote,
            "allow_scripts": self.allow_scripts,
            "external_fallback": self.external_fallback,
            "width": self.width,
            "height": self.height,
            "text": self._placeholder_text(),
        }


def _scatter_needs_v1(
    color: Any,
    colors: Any,
    scalars: Any,
    point_sizes: Any,
    opacity: float,
    nan_color: Any = None,
    clim: Any = None,
    log_scale: bool = False,
) -> bool:
    """Return True when the options require v1 point-instance packing."""
    return (
        color is not None
        or colors is not None
        or scalars is not None
        or point_sizes is not None
        or opacity != 1.0
        or nan_color is not None
        or clim is not None
        or log_scale
    )


def _line_plot_columns(value: str | Sequence[str]) -> tuple[str, ...]:
    if isinstance(value, str):
        text = value.strip()
        if not text:
            raise ValueError("LinePlot y must be a non-empty column name")
        return (text,)
    if isinstance(value, (bytes, bytearray)) or not isinstance(value, Sequence):
        raise TypeError("LinePlot y must be a column name or sequence of column names")
    columns = tuple(str(item).strip() for item in value)
    if not columns or any(not item for item in columns):
        raise ValueError("LinePlot y columns must be non-empty")
    return columns


def _line_plot_optional_values(
    value: str | Sequence[object] | None,
    *,
    count: int,
    name: str,
) -> tuple[object | None, ...]:
    if value is None:
        return (None,) * count
    if isinstance(value, str):
        if count != 1:
            raise ValueError(f"LinePlot {name} length must match y series count")
        return (value,)
    if isinstance(value, (bytes, bytearray)) or not isinstance(value, Sequence):
        raise TypeError(f"LinePlot {name} must be a value or sequence")
    values = tuple(value)
    if len(values) != count:
        raise ValueError(f"LinePlot {name} length must match y series count")
    return values


def _line_plot_optional_colors(
    value: object | Sequence[object] | None,
    *,
    count: int,
    name: str,
) -> tuple[object | None, ...]:
    if value is None:
        return (None,) * count
    if isinstance(value, str):
        return (value,) * count
    if isinstance(value, (bytes, bytearray)) or not isinstance(value, Sequence):
        return (value,) * count
    values = tuple(value)
    if len(values) in {3, 4} and all(isinstance(item, (int, float)) for item in values):
        return (values,) * count
    if len(values) != count:
        raise ValueError(f"LinePlot {name} length must match y series count")
    return values


_LINE_PLOT_LINE_STYLES = {"solid", "dashed", "dotted", "dashdot"}
_LINE_PLOT_LEGEND_POSITIONS = {"top-right", "top-left", "bottom-right", "bottom-left"}
_HISTOGRAM_MODES = {"count", "density", "probability", "percent"}


def _normalize_line_plot_line_style(value: object) -> str:
    style = str(value).strip().lower().replace("_", "-")
    aliases = {
        "dash-dot": "dashdot",
        "dash_dot": "dashdot",
        "dot": "dotted",
        "dash": "dashed",
    }
    style = aliases.get(style, style)
    if style not in _LINE_PLOT_LINE_STYLES:
        raise ValueError(
            "LinePlot line_style must be 'solid', 'dashed', 'dotted', or 'dashdot'"
        )
    return style


def _line_plot_optional_line_styles(
    value: object | Sequence[object] | None,
    *,
    count: int,
    name: str,
) -> tuple[str, ...]:
    if value is None:
        return ("solid",) * count
    if isinstance(value, str):
        return (_normalize_line_plot_line_style(value),) * count
    if isinstance(value, (bytes, bytearray)) or not isinstance(value, Sequence):
        return (_normalize_line_plot_line_style(value),) * count
    values = tuple(value)
    if len(values) != count:
        raise ValueError(f"LinePlot {name} length must match y series count")
    return tuple(_normalize_line_plot_line_style(item) for item in values)


def _pack_xy_values(x_values: Any, y_values: Any | None = None) -> bytes | None:
    try:
        import numpy as np

        if y_values is None:
            ys = np.asarray(x_values, dtype=np.float32).reshape(-1)
            xs = np.arange(len(ys), dtype=np.float32)
        else:
            xs = np.asarray(x_values, dtype=np.float32).reshape(-1)
            ys = np.asarray(y_values, dtype=np.float32).reshape(-1)
        if len(xs) != len(ys):
            raise ValueError("LinePlot x and y values must have the same length")
        if len(ys) == 0:
            return b""
        out = np.empty((len(ys), 2), dtype="<f4")
        out[:, 0] = xs
        out[:, 1] = ys
        return memoryview(out.view(np.uint8).reshape(-1)).tobytes()
    except (ImportError, TypeError, ValueError):
        return None


_PIE_DEFAULT_COLORS: tuple[str, ...] = (
    "#5aa9ff",
    "#74ddb0",
    "#ffd36a",
    "#f36b7f",
    "#b388ff",
    "#ff9f43",
    "#4dd0e1",
    "#a3e635",
)


def _pie_color_list(
    labels: Sequence[str],
    colors: Sequence[object] | Mapping[str, object] | None,
) -> tuple[object, ...]:
    if isinstance(colors, Mapping):
        return tuple(colors.get(label, _PIE_DEFAULT_COLORS[i % len(_PIE_DEFAULT_COLORS)]) for i, label in enumerate(labels))
    if colors is None:
        return tuple(_PIE_DEFAULT_COLORS[i % len(_PIE_DEFAULT_COLORS)] for i in range(len(labels)))
    color_items = list(colors)
    if not color_items:
        return tuple(_PIE_DEFAULT_COLORS[i % len(_PIE_DEFAULT_COLORS)] for i in range(len(labels)))
    return tuple(color_items[i % len(color_items)] for i in range(len(labels)))


def _normalize_pie_data(
    labels: Sequence[object],
    values: Sequence[object],
    *,
    colors: Sequence[object] | Mapping[str, object] | None = None,
    top_n: int | None = None,
    other_label: str = "Other",
) -> PieChartData:
    label_items = [str(label) for label in labels]
    value_items = [float(value) for value in values]
    if len(label_items) != len(value_items):
        raise ValueError("PieChart labels and values must have the same length")
    if not label_items:
        raise ValueError("PieChart requires at least one slice")

    pairs: list[tuple[str, float]] = []
    finite_count = 0
    for label, value in zip(label_items, value_items, strict=True):
        if not math.isfinite(value):
            continue
        finite_count += 1
        if value < 0:
            raise ValueError("PieChart values must be non-negative")
        if value > 0:
            pairs.append((label, value))
    if not pairs:
        raise ValueError("PieChart requires at least one positive finite value")

    pairs.sort(key=lambda item: item[1], reverse=True)
    if top_n is not None:
        limit = max(1, int(top_n))
        if len(pairs) > limit:
            kept = pairs[:limit]
            other = sum(value for _, value in pairs[limit:])
            if other > 0:
                kept.append((str(other_label), other))
            pairs = kept

    out_labels = tuple(label for label, _ in pairs)
    out_values = tuple(value for _, value in pairs)
    total = float(sum(out_values))
    return PieChartData(
        labels=out_labels,
        values=out_values,
        colors=_pie_color_list(out_labels, colors),
        total=total,
        input_count=len(label_items),
        finite_count=finite_count,
    )


def _pie_from_frame(
    data: Any,
    *,
    category: str,
    value: str | None = None,
    aggregate: str = "count",
    colors: Sequence[object] | Mapping[str, object] | None = None,
    top_n: int | None = None,
    other_label: str = "Other",
) -> PieChartData:
    aggregate = str(aggregate).strip().lower()
    if aggregate not in {"count", "sum", "mean", "min", "max"}:
        raise ValueError("PieChart aggregate must be one of: count, sum, mean, min, max")
    categories = list(_get_frame_col(data, category))
    if value is None:
        values = [1.0] * len(categories)
    else:
        values = [float(item) for item in _get_frame_col(data, value)]
    if len(categories) != len(values):
        raise ValueError("PieChart category and value columns must have the same length")

    grouped: dict[str, list[float]] = {}
    for label, number in zip(categories, values, strict=True):
        if not math.isfinite(number):
            continue
        if number < 0:
            raise ValueError("PieChart values must be non-negative")
        grouped.setdefault(str(label), []).append(number)

    labels: list[str] = []
    totals: list[float] = []
    for label, items in grouped.items():
        if aggregate == "count":
            result = float(len(items))
        elif aggregate == "sum":
            result = float(sum(items))
        elif aggregate == "mean":
            result = float(sum(items) / len(items)) if items else 0.0
        elif aggregate == "min":
            result = float(min(items)) if items else 0.0
        else:
            result = float(max(items)) if items else 0.0
        labels.append(label)
        totals.append(result)
    return _normalize_pie_data(labels, totals, colors=colors, top_n=top_n, other_label=other_label)


def _histogram_column_values(data: Any, value: str | None) -> Any:
    if value is None:
        return data
    return _get_frame_col(data, value)


def _histogram_mode(value: str) -> str:
    mode = str(value).strip().lower().replace("_", "-")
    aliases = {"counts": "count", "prob": "probability", "percentage": "percent"}
    mode = aliases.get(mode, mode)
    if mode not in _HISTOGRAM_MODES:
        raise ValueError(
            "Histogram mode must be 'count', 'density', 'probability', or 'percent'"
        )
    return mode


def _finite_float_values(values: Any) -> tuple[list[float], int]:
    try:
        import numpy as np

        arr = np.asarray(values, dtype=np.float64).reshape(-1)
        input_count = int(arr.size)
        finite = arr[np.isfinite(arr)]
        return [float(item) for item in finite.tolist()], input_count
    except (ImportError, TypeError, ValueError):
        out: list[float] = []
        input_count = 0
        for item in values:
            input_count += 1
            try:
                number = float(item)
            except (TypeError, ValueError):
                continue
            if math.isfinite(number):
                out.append(number)
        return out, input_count


def _normalize_histogram_edges(edges: Sequence[object]) -> tuple[float, ...]:
    if isinstance(edges, (str, bytes, bytearray)) or not isinstance(edges, Sequence):
        raise TypeError("Histogram bin_edges must be a sequence of numbers")
    normalized = tuple(float(edge) for edge in edges)
    if len(normalized) < 2:
        raise ValueError("Histogram bin_edges must contain at least two values")
    if any(not math.isfinite(edge) for edge in normalized):
        raise ValueError("Histogram bin_edges must be finite")
    if any(right <= left for left, right in zip(normalized, normalized[1:])):
        raise ValueError("Histogram bin_edges must be strictly increasing")
    return normalized


def _histogram_edges(
    values: Sequence[float],
    *,
    bins: int | Sequence[object],
    range: tuple[float, float] | None,
    bin_edges: Sequence[object] | None,
) -> tuple[float, ...]:
    if bin_edges is not None:
        return _normalize_histogram_edges(bin_edges)
    if isinstance(bins, Sequence) and not isinstance(bins, (str, bytes, bytearray)):
        return _normalize_histogram_edges(bins)
    bin_count = int(bins)
    if bin_count <= 0:
        raise ValueError("Histogram bins must be greater than zero")
    if range is not None:
        lo, hi = float(range[0]), float(range[1])
    elif values:
        lo, hi = min(values), max(values)
    else:
        lo, hi = 0.0, 1.0
    if not math.isfinite(lo) or not math.isfinite(hi):
        raise ValueError("Histogram range must be finite")
    if hi < lo:
        raise ValueError("Histogram range max must be greater than or equal to min")
    if hi == lo:
        lo -= 0.5
        hi += 0.5
    step = (hi - lo) / bin_count
    return tuple(lo + step * index for index in range_fn(bin_count + 1))


def range_fn(count_value: int) -> range:
    return range(count_value)


def _compute_histogram_bins(
    data: Any,
    *,
    value: str | None,
    bins: int | Sequence[object],
    range: tuple[float, float] | None,
    bin_edges: Sequence[object] | None,
    mode: str,
    cumulative: bool,
) -> HistogramBins:
    values, input_count = _finite_float_values(_histogram_column_values(data, value))
    edges = _histogram_edges(values, bins=bins, range=range, bin_edges=bin_edges)
    counts = [0.0 for _ in range_fn(len(edges) - 1)]
    if counts:
        lo = edges[0]
        hi = edges[-1]
        width = hi - lo
        for number in values:
            if number < lo or number > hi:
                continue
            if number == hi:
                index = len(counts) - 1
            else:
                index = int((number - lo) / width * len(counts)) if width > 0 else 0
                index = max(0, min(len(counts) - 1, index))
            counts[index] += 1.0
    total = sum(counts)
    if mode in {"probability", "percent"} and total > 0.0:
        scale = 100.0 if mode == "percent" else 1.0
        counts = [count / total * scale for count in counts]
    elif mode == "density" and total > 0.0:
        counts = [
            count / (total * (right - left))
            if right > left else 0.0
            for count, left, right in zip(counts, edges, edges[1:])
        ]
    if cumulative:
        running = 0.0
        cumulative_counts: list[float] = []
        for count in counts:
            running += count
            cumulative_counts.append(running)
        counts = cumulative_counts
    return HistogramBins(
        edges=tuple(float(edge) for edge in edges),
        counts=tuple(float(count) for count in counts),
        input_count=input_count,
        finite_count=len(values),
    )


class Histogram(Widget):
    kind = "histogram"

    def __init__(
        self,
        data: Any,
        *,
        value: str | None = None,
        bins: int | Sequence[object] = 30,
        bin_edges: Sequence[object] | None = None,
        range: tuple[float, float] | None = None,
        mode: str = "count",
        cumulative: bool = False,
        label: str | None = None,
        x_label: str | None = None,
        y_label: str | None = None,
        color: str | Sequence[object] | None = None,
        show_grid: bool = True,
        show_axes: bool = True,
        show_ticks: bool = True,
        show_toolbar: bool = False,
        interaction: str = "inspect",
        tick_count: int = 5,
        auto_fit: bool = True,
        bar_gap: float = 1.0,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.data = data
        self.value = None if value is None else str(value)
        self.bins = bins
        self.bin_edges = None if bin_edges is None else tuple(bin_edges)
        self.range = range
        self.mode = _histogram_mode(mode)
        self.cumulative = bool(cumulative)
        self.label = None if label is None else str(label)
        self.x_label = str(x_label if x_label is not None else (self.value or "value"))
        default_y = {
            "count": "count",
            "density": "density",
            "probability": "probability",
            "percent": "percent",
        }[self.mode]
        self.y_label = str(y_label if y_label is not None else default_y)
        self.color = color
        self.show_grid = bool(show_grid)
        self.show_axes = bool(show_axes)
        self.show_ticks = bool(show_ticks)
        self.show_toolbar = bool(show_toolbar)
        if interaction not in {"inspect", "pan", "zoom", "box_zoom"}:
            raise ValueError(
                "Histogram interaction must be 'inspect', 'pan', 'zoom', or 'box_zoom'"
            )
        self.interaction = interaction
        self.tick_count = max(2, min(9, int(tick_count)))
        self.auto_fit = bool(auto_fit)
        self.bar_gap = max(0.0, float(bar_gap))
        self.frame_summary = summarize_frame(data)
        self._bins = _compute_histogram_bins(
            data,
            value=self.value,
            bins=self.bins,
            range=self.range,
            bin_edges=self.bin_edges,
            mode=self.mode,
            cumulative=self.cumulative,
        )
        super().__init__(
            id=id,
            key=key,
            class_=class_,
            style=style,
            tooltip=tooltip,
            parent=parent,
        )

    def set_data(
        self,
        data: Any,
        *,
        value: str | None = None,
        bins: int | Sequence[object] | None = None,
        bin_edges: Sequence[object] | None = None,
        range: tuple[float, float] | None = None,
    ) -> None:
        self.data = data
        if value is not None:
            self.value = str(value)
        if bins is not None:
            self.bins = bins
        if bin_edges is not None:
            self.bin_edges = tuple(bin_edges)
        if range is not None:
            self.range = range
        self.frame_summary = summarize_frame(data)
        self._bins = _compute_histogram_bins(
            data,
            value=self.value,
            bins=self.bins,
            range=self.range,
            bin_edges=self.bin_edges,
            mode=self.mode,
            cumulative=self.cumulative,
        )
        if self.is_live:
            raise RuntimeError("live Histogram.set_data is not implemented yet")

    def set_grid_visible(self, visible: bool) -> None:
        self.show_grid = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("show_grid", self.show_grid)

    def set_axes_visible(self, visible: bool) -> None:
        self.show_axes = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("show_axes", self.show_axes)

    def set_ticks_visible(self, visible: bool) -> None:
        self.show_ticks = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("show_ticks", self.show_ticks)

    def set_toolbar_visible(self, visible: bool) -> None:
        self.show_toolbar = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("show_toolbar", self.show_toolbar)

    def set_interaction(self, interaction: str) -> None:
        if interaction not in {"inspect", "pan", "zoom", "box_zoom"}:
            raise ValueError(
                "Histogram interaction must be 'inspect', 'pan', 'zoom', or 'box_zoom'"
            )
        self.interaction = interaction
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("interaction", self.interaction)

    def fit(self) -> None:
        self.auto_fit = True
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("auto_fit", True)

    def set_tick_count(self, count: int) -> None:
        self.tick_count = max(2, min(9, int(count)))
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("tick_count", self.tick_count)

    def set_axis_labels(self, *, x: str | None = None, y: str | None = None) -> None:
        if x is not None:
            self.x_label = str(x)
        if y is not None:
            self.y_label = str(y)
        if (handle := self._live()) is not None:
            if x is not None:
                handle.enqueue_set_prop("x_label", self.x_label)
            if y is not None:
                handle.enqueue_set_prop("y_label", self.y_label)

    def props(self) -> dict[str, Any]:
        return {
            "frame": self.frame_summary.to_dict(),
            "value": self.value or "",
            "label": self.label or "",
            "x_label": self.x_label,
            "y_label": self.y_label,
            "mode": self.mode,
            "cumulative": self.cumulative,
            "show_grid": self.show_grid,
            "show_axes": self.show_axes,
            "show_ticks": self.show_ticks,
            "show_toolbar": self.show_toolbar,
            "interaction": self.interaction,
            "tick_count": self.tick_count,
            "auto_fit": self.auto_fit,
            "bar_gap": self.bar_gap,
            "color": self.color,
            "input_count": self._bins.input_count,
            "finite_count": self._bins.finite_count,
            "edges": list(self._bins.edges),
            "counts": list(self._bins.counts),
        }


_BAR_CHART_DEFAULT_COLORS = (
    "#69b7ff",
    "#76e0b1",
    "#ffbf69",
    "#ed7d9a",
    "#b388ff",
    "#4dd0e1",
)


def _bar_chart_orientation(value: str) -> str:
    orientation = str(value).strip().lower()
    if orientation not in {"vertical", "horizontal"}:
        raise ValueError("BarChart orientation must be 'vertical' or 'horizontal'")
    return orientation


def _bar_chart_aggregate(value: str) -> str:
    aggregate = str(value).strip().lower()
    if aggregate not in {"count", "sum", "mean", "min", "max"}:
        raise ValueError("BarChart aggregate must be one of: count, sum, mean, min, max")
    return aggregate


def _bar_chart_colors(count: int, colors: Sequence[object] | Mapping[str, object] | None, labels: Sequence[str]) -> tuple[object, ...]:
    if isinstance(colors, Mapping):
        return tuple(colors.get(label, _BAR_CHART_DEFAULT_COLORS[i % len(_BAR_CHART_DEFAULT_COLORS)]) for i, label in enumerate(labels))
    if colors is None:
        return tuple(_BAR_CHART_DEFAULT_COLORS[i % len(_BAR_CHART_DEFAULT_COLORS)] for i in range(count))
    items = tuple(colors)
    if not items:
        return tuple(_BAR_CHART_DEFAULT_COLORS[i % len(_BAR_CHART_DEFAULT_COLORS)] for i in range(count))
    return tuple(items[i % len(items)] for i in range(count))


def _bar_chart_value_matrix(values: Any, label_count: int) -> tuple[tuple[float, ...], ...]:
    try:
        import numpy as np

        arr = np.asarray(values, dtype=np.float64)
        if arr.ndim == 1:
            if int(arr.shape[0]) != label_count:
                raise ValueError("BarChart values length must match labels")
            return (tuple(float(v) for v in arr.tolist()),)
        if arr.ndim == 2:
            if int(arr.shape[1]) != label_count:
                raise ValueError("BarChart grouped values must have shape (series, labels)")
            return tuple(tuple(float(v) for v in row.tolist()) for row in arr)
        raise ValueError("BarChart values must be a 1D or 2D numeric sequence")
    except ImportError:
        pass

    if isinstance(values, (str, bytes, bytearray)) or not isinstance(values, Sequence):
        raise TypeError("BarChart values must be a numeric sequence")
    raw = list(values)
    if not raw:
        raise ValueError("BarChart requires at least one value")
    first = raw[0]
    if isinstance(first, Sequence) and not isinstance(first, (str, bytes, bytearray)):
        series: list[tuple[float, ...]] = []
        for row in raw:
            if isinstance(row, (str, bytes, bytearray)) or not isinstance(row, Sequence):
                raise ValueError("BarChart grouped values must be a 2D numeric sequence")
            parsed = tuple(float(value) for value in row)
            if len(parsed) != label_count:
                raise ValueError("BarChart grouped values rows must match labels")
            series.append(parsed)
        return tuple(series)
    parsed = tuple(float(value) for value in raw)
    if len(parsed) != label_count:
        raise ValueError("BarChart values length must match labels")
    return (parsed,)


def _normalize_bar_chart_data(
    labels: Sequence[object],
    values: Any,
    *,
    series: Sequence[object] | None = None,
    colors: Sequence[object] | Mapping[str, object] | None = None,
) -> BarChartData:
    label_items = tuple(str(label) for label in labels)
    if not label_items:
        raise ValueError("BarChart requires at least one category label")
    matrix = _bar_chart_value_matrix(values, len(label_items))
    if not matrix:
        raise ValueError("BarChart requires at least one series")
    if series is None:
        series_labels = ("value",) if len(matrix) == 1 else tuple(f"series {i + 1}" for i in range(len(matrix)))
    else:
        series_labels = tuple(str(label) for label in series)
        if len(series_labels) != len(matrix):
            raise ValueError("BarChart series labels must match the number of value series")
    finite_count = sum(1 for row in matrix for value in row if math.isfinite(value))
    return BarChartData(
        labels=label_items,
        series_labels=series_labels,
        values=tuple(tuple(float(value) for value in row) for row in matrix),
        colors=_bar_chart_colors(len(matrix), colors, series_labels),
        input_count=len(label_items) * len(matrix),
        finite_count=finite_count,
    )


def _aggregate_bar_values(values: Sequence[float], aggregate: str) -> float:
    if aggregate == "count":
        return float(len(values))
    if not values:
        return 0.0
    if aggregate == "sum":
        return float(sum(values))
    if aggregate == "mean":
        return float(sum(values) / len(values))
    if aggregate == "min":
        return float(min(values))
    return float(max(values))


def _bar_chart_from_frame(
    data: Any,
    *,
    category: str,
    value: str | Sequence[str] | None = None,
    aggregate: str = "sum",
    colors: Sequence[object] | Mapping[str, object] | None = None,
) -> BarChartData:
    aggregate = _bar_chart_aggregate(aggregate)
    categories = [str(item) for item in _get_frame_col(data, category)]
    if value is None:
        value_columns: tuple[str | None, ...] = (None,)
        raw_series = [[1.0] * len(categories)]
        aggregate = "count"
    elif isinstance(value, str):
        value_columns = (value,)
        raw_series = [[float(item) for item in _get_frame_col(data, value)]]
    else:
        value_columns = tuple(str(column) for column in value)
        if not value_columns:
            raise ValueError("BarChart value sequence must not be empty")
        raw_series = [[float(item) for item in _get_frame_col(data, column)] for column in value_columns]
    if any(len(series_values) != len(categories) for series_values in raw_series):
        raise ValueError("BarChart category and value columns must have the same length")

    ordered_labels: list[str] = []
    buckets: dict[str, list[list[float]]] = {}
    for category_label in categories:
        if category_label not in buckets:
            ordered_labels.append(category_label)
            buckets[category_label] = [[] for _ in raw_series]
    for row_index, category_label in enumerate(categories):
        for series_index, series_values in enumerate(raw_series):
            number = series_values[row_index]
            if math.isfinite(number):
                buckets[category_label][series_index].append(number)

    matrix: list[tuple[float, ...]] = []
    for series_index in range(len(raw_series)):
        matrix.append(tuple(_aggregate_bar_values(buckets[label][series_index], aggregate) for label in ordered_labels))
    series_labels = ("count",) if value_columns == (None,) else tuple(str(column) for column in value_columns)
    finite_count = sum(len(bucket) for group in buckets.values() for bucket in group)
    return BarChartData(
        labels=tuple(ordered_labels),
        series_labels=series_labels,
        values=tuple(matrix),
        colors=_bar_chart_colors(len(matrix), colors, series_labels),
        input_count=len(categories) * len(raw_series),
        finite_count=finite_count,
    )


class BarChart(Widget):
    kind = "bar_chart"

    def __init__(
        self,
        data: Any | None = None,
        *,
        category: str | None = None,
        value: str | Sequence[str] | None = None,
        labels: Sequence[object] | None = None,
        values: Any | None = None,
        series: Sequence[object] | None = None,
        aggregate: str = "sum",
        orientation: str = "vertical",
        label: str | None = None,
        x_label: str | None = None,
        y_label: str | None = None,
        colors: Sequence[object] | Mapping[str, object] | None = None,
        show_grid: bool = True,
        show_axes: bool = True,
        show_ticks: bool = True,
        show_toolbar: bool = False,
        tick_count: int = 5,
        auto_fit: bool = True,
        bar_gap: float = 2.0,
        on_hover: BarChartHoverCallback | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.data = data
        self.category = None if category is None else str(category)
        self.value = value
        self.aggregate = _bar_chart_aggregate(aggregate)
        self.orientation = _bar_chart_orientation(orientation)
        self.label = None if label is None else str(label)
        self.x_label = str(x_label if x_label is not None else (self.category or "category"))
        default_value_label = "count" if value is None and category is not None else "value"
        self.y_label = str(y_label if y_label is not None else default_value_label)
        self.colors = colors
        self.show_grid = bool(show_grid)
        self.show_axes = bool(show_axes)
        self.show_ticks = bool(show_ticks)
        self.show_toolbar = bool(show_toolbar)
        self.tick_count = max(2, min(9, int(tick_count)))
        self.auto_fit = bool(auto_fit)
        self.bar_gap = max(0.0, float(bar_gap))
        self.on_hover = on_hover
        self.hover_bar: BarChartBar | None = None
        self.frame_summary = summarize_frame(data) if data is not None else summarize_frame(())
        if data is not None and self.category is not None:
            self._chart = _bar_chart_from_frame(
                data,
                category=self.category,
                value=value,
                aggregate=self.aggregate,
                colors=colors,
            )
        else:
            if labels is None or values is None:
                raise ValueError("BarChart requires frame data with category=, or labels= and values=")
            self._chart = _normalize_bar_chart_data(labels, values, series=series, colors=colors)
        super().__init__(
            id=id,
            key=key,
            class_=class_,
            style=style,
            tooltip=tooltip,
            parent=parent,
        )

    def set_grid_visible(self, visible: bool) -> None:
        self.show_grid = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("show_grid", self.show_grid)

    def set_axes_visible(self, visible: bool) -> None:
        self.show_axes = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("show_axes", self.show_axes)

    def set_ticks_visible(self, visible: bool) -> None:
        self.show_ticks = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("show_ticks", self.show_ticks)

    def set_toolbar_visible(self, visible: bool) -> None:
        self.show_toolbar = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("show_toolbar", self.show_toolbar)

    def fit(self) -> None:
        self.auto_fit = True
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("auto_fit", True)

    def set_tick_count(self, count: int) -> None:
        self.tick_count = max(2, min(9, int(count)))
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("tick_count", self.tick_count)

    def props(self) -> dict[str, Any]:
        return {
            "frame": self.frame_summary.to_dict(),
            "category": self.category or "",
            "value": list(self.value) if isinstance(self.value, Sequence) and not isinstance(self.value, str) else (self.value or ""),
            "label": self.label or "",
            "x_label": self.x_label,
            "y_label": self.y_label,
            "aggregate": self.aggregate,
            "orientation": self.orientation,
            "show_grid": self.show_grid,
            "show_axes": self.show_axes,
            "show_ticks": self.show_ticks,
            "show_toolbar": self.show_toolbar,
            "tick_count": self.tick_count,
            "auto_fit": self.auto_fit,
            "bar_gap": self.bar_gap,
            "input_count": self._chart.input_count,
            "finite_count": self._chart.finite_count,
            "labels": list(self._chart.labels),
            "series": [
                {
                    "label": label,
                    "values": list(values),
                    "color": color,
                }
                for label, values, color in zip(
                    self._chart.series_labels,
                    self._chart.values,
                    self._chart.colors,
                    strict=True,
                )
            ],
            "events": ["change"] if self.on_hover is not None else [],
        }


def _normalize_heatmap_labels(
    labels: Sequence[object] | None,
    *,
    count: int,
    name: str,
) -> tuple[str, ...]:
    if labels is None:
        return ()
    if isinstance(labels, (str, bytes, bytearray)) or not isinstance(labels, Sequence):
        raise TypeError(f"Heatmap {name} must be a sequence of labels")
    normalized = tuple(str(label) for label in labels)
    if len(normalized) != count:
        raise ValueError(f"Heatmap {name} length must match matrix {'columns' if name == 'x_labels' else 'rows'}")
    return normalized


def _normalize_heatmap_matrix(
    matrix: Any,
    clim: tuple[float, float] | None,
) -> tuple[int, int, bytes, int, float, float]:
    try:
        import numpy as np

        arr = np.asarray(matrix, dtype=np.float32)
        if arr.ndim != 2:
            raise ValueError("Heatmap matrix must be 2D")
        rows, cols = int(arr.shape[0]), int(arr.shape[1])
        if rows <= 0 or cols <= 0:
            raise ValueError("Heatmap matrix must not be empty")
        arr = np.ascontiguousarray(arr)
        flat = arr.reshape(-1)
        finite = flat[np.isfinite(flat)]
        finite_count = int(finite.size)
        payload = arr.tobytes(order="C")
        if clim is None:
            if finite_count:
                vmin = float(np.min(finite))
                vmax = float(np.max(finite))
            else:
                vmin, vmax = 0.0, 1.0
        else:
            vmin, vmax = float(clim[0]), float(clim[1])
    except ImportError:
        if isinstance(matrix, (str, bytes, bytearray)) or not isinstance(matrix, Sequence):
            raise TypeError("Heatmap matrix must be a 2D numeric sequence")
        raw_rows = list(matrix)
        if not raw_rows:
            raise ValueError("Heatmap matrix must not be empty")
        flat_values: list[float] = []
        cols: int | None = None
        finite_values: list[float] = []
        for row in raw_rows:
            if isinstance(row, (str, bytes, bytearray)) or not isinstance(row, Sequence):
                raise ValueError("Heatmap matrix must be 2D")
            values = [float(value) for value in row]
            if cols is None:
                cols = len(values)
                if cols <= 0:
                    raise ValueError("Heatmap matrix must not be empty")
            elif len(values) != cols:
                raise ValueError("Heatmap matrix rows must all have the same length")
            flat_values.extend(values)
            finite_values.extend(value for value in values if math.isfinite(value))
        rows = len(raw_rows)
        cols = int(cols or 0)
        finite_count = len(finite_values)
        payload = struct.pack(f"<{len(flat_values)}f", *flat_values)
        if clim is None:
            if finite_values:
                vmin, vmax = min(finite_values), max(finite_values)
            else:
                vmin, vmax = 0.0, 1.0
        else:
            vmin, vmax = float(clim[0]), float(clim[1])
    if not math.isfinite(vmin) or not math.isfinite(vmax):
        raise ValueError("Heatmap color range must be finite")
    if vmax < vmin:
        raise ValueError("Heatmap color range max must be greater than or equal to min")
    if vmax == vmin:
        vmin -= 0.5
        vmax += 0.5
    return rows, cols, payload, finite_count, vmin, vmax


class Heatmap(Widget):
    kind = "heatmap"

    def __init__(
        self,
        matrix: Any,
        *,
        x_labels: Sequence[object] | None = None,
        y_labels: Sequence[object] | None = None,
        colormap: str = "viridis",
        clim: tuple[float, float] | None = None,
        title: str | None = None,
        show_labels: bool = True,
        scalar_bar: bool = True,
        on_hover: HeatmapHoverCallback | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.colormap = _scatter_colormap(colormap)
        self.clim = None if clim is None else (float(clim[0]), float(clim[1]))
        self.title = None if title is None else str(title)
        self.show_labels = bool(show_labels)
        self.scalar_bar = bool(scalar_bar)
        self.on_hover = on_hover
        self.hover_cell: HeatmapCell | None = None
        self.rows = 0
        self.cols = 0
        self.finite_count = 0
        self.vmin = 0.0
        self.vmax = 1.0
        self._payload = b""
        self._payload_b64: str | None = None
        self._payload_token = 0
        self.set_data(matrix, x_labels=x_labels, y_labels=y_labels, clim=self.clim, _initial=True)
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def set_data(
        self,
        matrix: Any,
        *,
        x_labels: Sequence[object] | None = None,
        y_labels: Sequence[object] | None = None,
        clim: tuple[float, float] | None = None,
        _initial: bool = False,
    ) -> None:
        effective_clim = self.clim if clim is None else (float(clim[0]), float(clim[1]))
        rows, cols, payload, finite_count, vmin, vmax = _normalize_heatmap_matrix(matrix, effective_clim)
        self.rows = rows
        self.cols = cols
        self.finite_count = finite_count
        self.vmin = vmin
        self.vmax = vmax
        self.clim = effective_clim
        x_source = x_labels
        if x_source is None and hasattr(self, "x_labels") and len(self.x_labels) == cols:
            x_source = self.x_labels
        y_source = y_labels
        if y_source is None and hasattr(self, "y_labels") and len(self.y_labels) == rows:
            y_source = self.y_labels
        self.x_labels = _normalize_heatmap_labels(x_source, count=cols, name="x_labels")
        self.y_labels = _normalize_heatmap_labels(y_source, count=rows, name="y_labels")
        self._payload = payload
        self._payload_b64 = None
        self._payload_token = zlib.crc32(payload) if payload else 0
        if not _initial and (handle := self._live()) is not None:
            handle.enqueue_replace_node(self.to_dict())

    def set_colormap(self, colormap: str) -> None:
        self.colormap = _scatter_colormap(colormap)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("colormap", self.colormap)

    def set_scalar_bar_visible(self, visible: bool) -> None:
        self.scalar_bar = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("scalar_bar", self.scalar_bar)

    def set_labels_visible(self, visible: bool) -> None:
        self.show_labels = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("show_labels", self.show_labels)

    def _queue_startup_resources(self) -> None:
        if (handle := self._live()) is not None:
            handle.enqueue_replace_node(self.to_dict())

    def _payload_data_b64(self) -> str:
        if self._payload_b64 is None:
            self._payload_b64 = base64.b64encode(self._payload).decode("ascii")
        return self._payload_b64

    def props(self) -> dict[str, Any]:
        include_payload = _include_startup_resource_payloads()
        payload: dict[str, Any] = {
            "rows": self.rows,
            "cols": self.cols,
            "finite_count": self.finite_count,
            "data_format": "f32_matrix_v0",
            "_payload_token": self._payload_token,
            "vmin": self.vmin,
            "vmax": self.vmax,
            "colormap": self.colormap,
            "x_labels": list(self.x_labels),
            "y_labels": list(self.y_labels),
            "show_labels": self.show_labels,
            "scalar_bar": self.scalar_bar,
            "events": ["change"] if self.on_hover is not None else [],
        }
        if self.title is not None:
            payload["title"] = self.title
        if include_payload:
            payload["data_b64"] = self._payload_data_b64()
        return payload


class LinePlot(Widget):
    kind = "line_plot"

    def __init__(
        self,
        frame: Any = None,
        *,
        x: str | None = None,
        y: str | Sequence[str],
        label: str | None = None,
        labels: Sequence[str] | None = None,
        color: str | Sequence[object] | None = None,
        colors: Sequence[object] | None = None,
        x_label: str | None = None,
        y_label: str | None = None,
        show_grid: bool = True,
        show_axes: bool = True,
        show_ticks: bool = True,
        show_toolbar: bool = False,
        show_legend: bool = False,
        legend_position: str = "top-right",
        interaction: str = "inspect",
        tick_count: int = 5,
        auto_fit: bool = True,
        line_width: float = 2.0,
        line_style: str | Sequence[object] = "solid",
        line_styles: Sequence[object] | None = None,
        window_size: float | None = None,
        max_points: int | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self.frame = frame
        self.x = x
        self.y_columns = _line_plot_columns(y)
        if labels is not None and label is not None:
            raise ValueError("LinePlot accepts either label or labels, not both")
        if colors is not None and color is not None:
            raise ValueError("LinePlot accepts either color or colors, not both")
        if line_styles is not None and line_style != "solid":
            raise ValueError("LinePlot accepts either line_style or line_styles, not both")
        label_values = (
            _line_plot_optional_values(labels, count=len(self.y_columns), name="labels")
            if labels is not None
            else _line_plot_optional_values(label, count=len(self.y_columns), name="label")
        )
        self.labels = tuple(None if item is None else str(item) for item in label_values)
        self.colors = _line_plot_optional_colors(
            colors if colors is not None else color,
            count=len(self.y_columns),
            name="colors",
        )
        self.line_styles = _line_plot_optional_line_styles(
            line_styles if line_styles is not None else line_style,
            count=len(self.y_columns),
            name="line_styles",
        )
        self.x_label = str(x_label if x_label is not None else (x if x is not None else "sample"))
        self.y_label = str(y_label if y_label is not None else self.y_columns[0])
        self.show_grid = bool(show_grid)
        self.show_axes = bool(show_axes)
        self.show_ticks = bool(show_ticks)
        self.show_toolbar = bool(show_toolbar)
        self.show_legend = bool(show_legend)
        self.legend_position = str(legend_position).strip().lower()
        if self.legend_position not in _LINE_PLOT_LEGEND_POSITIONS:
            raise ValueError(
                f"LinePlot legend_position must be one of {sorted(_LINE_PLOT_LEGEND_POSITIONS)}"
            )
        if interaction not in {"inspect", "pan", "zoom", "box_zoom"}:
            raise ValueError(
                "LinePlot interaction must be 'inspect', 'pan', 'zoom', or 'box_zoom'"
            )
        self.interaction = interaction
        self.tick_count = max(2, min(9, int(tick_count)))
        self.auto_fit = bool(auto_fit)
        self.line_width = max(0.5, float(line_width))
        self.window_size = (
            None
            if window_size is None
            else max(float(window_size), 0.000001)
        )
        self.max_points = None if max_points is None else max(1, int(max_points))
        self.frame_summary = summarize_frame(frame)
        self.data_format = "xy_f32_v0"
        self._cached_payloads: dict[str, bytes | None] = {}
        self._cached_payload_b64: dict[str, str | None] = {}
        self._payload_tokens: dict[str, int] = {}
        self._refresh_cached_payload_b64()
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    @property
    def y(self) -> str | list[str]:
        return self.y_columns[0] if len(self.y_columns) == 1 else list(self.y_columns)

    @property
    def label(self) -> str | None:
        label = self.labels[0] if self.labels else None
        return label if label else None

    @property
    def color(self) -> object | None:
        return self.colors[0] if self.colors else None

    @staticmethod
    def _immutable_payload_bytes(payload: object) -> bytes:
        if isinstance(payload, bytes):
            return payload
        return memoryview(payload).tobytes()

    @staticmethod
    def _payload_point_count(payload: bytes, payload_format: str) -> int:
        bytes_per_point = 8 if payload_format == "xy_f32_v0" else 0
        return len(payload) // bytes_per_point if bytes_per_point else 0

    def _series_label(self, index: int) -> str:
        label = self.labels[index] if index < len(self.labels) else None
        return str(label) if label else self.y_columns[index]

    def _series_color(self, index: int) -> object | None:
        return self.colors[index] if index < len(self.colors) else None

    def _series_line_style(self, index: int) -> str:
        return self.line_styles[index] if index < len(self.line_styles) else "solid"

    def _build_payload(self, y_column: str) -> bytes | None:
        return _pack_xy_bytes(self.frame, self.x, y_column)

    def _refresh_cached_payload_b64(self) -> None:
        self._cached_payloads.clear()
        self._cached_payload_b64.clear()
        self._payload_tokens.clear()
        for y_column in self.y_columns:
            payload = self._build_payload(y_column)
            if payload is None:
                self._cached_payloads[y_column] = None
                self._cached_payload_b64[y_column] = None
                self._payload_tokens[y_column] = 0
                continue
            data = self._immutable_payload_bytes(payload)
            self._cached_payloads[y_column] = data
            self._cached_payload_b64[y_column] = base64.b64encode(data).decode("ascii")
            self._payload_tokens[y_column] = zlib.crc32(data) if data else 0

    @classmethod
    def prepare_points(
        cls,
        frame: Any,
        *,
        x: str | None = None,
        y: str,
        x_label: str | None = None,
        y_label: str | None = None,
    ) -> LinePlotPayload:
        """Pack one LinePlot series without mutating a live widget."""
        t0 = time.perf_counter()
        raw = _pack_xy_bytes(frame, x, y)
        pack_ms = (time.perf_counter() - t0) * 1000.0
        if raw is None:
            raise RuntimeError("LinePlot.prepare_points requires NumPy and a numeric y column")
        data = cls._immutable_payload_bytes(raw)
        return LinePlotPayload(
            data=data,
            payload_format="xy_f32_v0",
            point_count=cls._payload_point_count(data, "xy_f32_v0"),
            pack_ms=pack_ms,
            x_label=str(x_label if x_label is not None else (x if x is not None else "sample")),
            y_label=str(y_label if y_label is not None else y),
            frame_summary=summarize_frame(frame),
        )

    @classmethod
    def prepare_series(
        cls,
        frame: Any,
        *,
        x: str | None = None,
        y: str | Sequence[str],
    ) -> tuple[LinePlotPayload, ...]:
        """Pack one or more LinePlot series without mutating a live widget."""
        return tuple(cls.prepare_points(frame, x=x, y=column) for column in _line_plot_columns(y))

    def set_data(
        self,
        frame: Any,
        *,
        x: str | None = None,
        y: str | Sequence[str] | None = None,
        label: str | None = None,
        labels: Sequence[str] | None = None,
        color: str | Sequence[object] | None = None,
        colors: Sequence[object] | None = None,
        line_style: str | Sequence[object] | None = None,
        line_styles: Sequence[object] | None = None,
        fit: bool = True,
    ) -> None:
        self.frame = frame
        self.x = x
        if y is not None:
            self.y_columns = _line_plot_columns(y)
        if labels is not None or label is not None:
            if labels is not None and label is not None:
                raise ValueError("LinePlot.set_data accepts either label or labels, not both")
            label_values = (
                _line_plot_optional_values(labels, count=len(self.y_columns), name="labels")
                if labels is not None
                else _line_plot_optional_values(label, count=len(self.y_columns), name="label")
            )
            self.labels = tuple(None if item is None else str(item) for item in label_values)
        if colors is not None or color is not None:
            if colors is not None and color is not None:
                raise ValueError("LinePlot.set_data accepts either color or colors, not both")
            self.colors = _line_plot_optional_colors(
                colors if colors is not None else color,
                count=len(self.y_columns),
                name="colors",
            )
        if line_styles is not None or line_style is not None:
            if line_styles is not None and line_style is not None:
                raise ValueError("LinePlot.set_data accepts either line_style or line_styles, not both")
            self.line_styles = _line_plot_optional_line_styles(
                line_styles if line_styles is not None else line_style,
                count=len(self.y_columns),
                name="line_styles",
            )
        self.x_label = str(x if x is not None else "sample")
        self.y_label = self._series_label(0)
        self.frame_summary = summarize_frame(frame)
        self._refresh_cached_payload_b64()
        if (handle := self._live()) is not None:
            handle.enqueue_clear_line_plot_series()
            handle.enqueue_set_prop("x_label", self.x_label)
            handle.enqueue_set_prop("y_label", self.y_label)
            for index, y_column in enumerate(self.y_columns):
                payload = self._cached_payloads.get(y_column)
                if payload is None:
                    raise RuntimeError("live LinePlot.set_data requires numeric x/y columns")
                handle.enqueue_set_line_plot_data_packed(
                    self._series_label(index),
                    payload,
                    label=self._series_label(index),
                    color=self._series_color(index),
                    line_width=self.line_width,
                    line_style=self._series_line_style(index),
                    show_grid=self.show_grid,
                    auto_fit=self.auto_fit,
                    max_points=self.max_points,
                    fit=fit,
                )

    def append_points(
        self,
        x_values: Any,
        y_values: Any | None = None,
        *,
        series: str | None = None,
        max_points: int | None = None,
    ) -> None:
        payload = _pack_xy_values(x_values, y_values)
        if payload is None:
            raise RuntimeError("LinePlot.append_points requires numeric x/y values")
        target = series or self._series_label(0)
        effective_max = max_points if max_points is not None else self.max_points
        if (handle := self._live()) is not None:
            handle.enqueue_append_line_plot_points_packed(
                target,
                payload,
                max_points=None if effective_max is None else int(effective_max),
            )

    def clear(self, series: str | None = None) -> None:
        if (handle := self._live()) is not None:
            handle.enqueue_clear_line_plot_series(series)

    def _queue_startup_resources(self) -> None:
        handle = self._live()
        if handle is None:
            return
        for index, y_column in enumerate(self.y_columns):
            payload = self._cached_payloads.get(y_column)
            if payload is None:
                continue
            handle.enqueue_set_line_plot_data_packed(
                self._series_label(index),
                payload,
                label=self._series_label(index),
                color=self._series_color(index),
                line_width=self.line_width,
                line_style=self._series_line_style(index),
                show_grid=self.show_grid,
                auto_fit=self.auto_fit,
                max_points=self.max_points,
                fit=self.auto_fit,
                coalesce=True,
            )

    def set_line_width(self, width: float) -> None:
        self.line_width = max(0.5, float(width))
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("line_width", self.line_width)

    def set_grid_visible(self, visible: bool) -> None:
        self.show_grid = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("show_grid", self.show_grid)

    def set_axes_visible(self, visible: bool) -> None:
        self.show_axes = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("show_axes", self.show_axes)

    def set_ticks_visible(self, visible: bool) -> None:
        self.show_ticks = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("show_ticks", self.show_ticks)

    def set_toolbar_visible(self, visible: bool) -> None:
        self.show_toolbar = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("show_toolbar", self.show_toolbar)

    def set_legend_visible(self, visible: bool, *, position: str | None = None) -> None:
        self.show_legend = bool(visible)
        if position is not None:
            normalized = str(position).strip().lower()
            if normalized not in _LINE_PLOT_LEGEND_POSITIONS:
                raise ValueError(
                    f"LinePlot legend_position must be one of {sorted(_LINE_PLOT_LEGEND_POSITIONS)}"
                )
            self.legend_position = normalized
        if (handle := self._live()) is not None:
            if position is not None:
                handle.enqueue_set_prop("legend_position", self.legend_position)
            handle.enqueue_set_prop("show_legend", self.show_legend)

    def set_interaction(self, interaction: str) -> None:
        if interaction not in {"inspect", "pan", "zoom", "box_zoom"}:
            raise ValueError(
                "LinePlot interaction must be 'inspect', 'pan', 'zoom', or 'box_zoom'"
            )
        self.interaction = interaction
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("interaction", self.interaction)

    def fit(self) -> None:
        self.auto_fit = True
        self.window_size = None
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("window_size", None)
            handle.enqueue_set_prop("auto_fit", True)

    def set_window_size(self, size: float | None) -> None:
        """Set a moving x-axis window for streaming data.

        When set, appended points remain stored, but the visible x range follows
        the newest sample using this width. Pass None to return to auto-fit.
        """
        self.window_size = None if size is None else max(float(size), 0.000001)
        if self.window_size is None:
            self.auto_fit = True
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("window_size", self.window_size)

    def set_tick_count(self, count: int) -> None:
        self.tick_count = max(2, min(9, int(count)))
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("tick_count", self.tick_count)

    def set_axis_labels(self, *, x: str | None = None, y: str | None = None) -> None:
        if x is not None:
            self.x_label = str(x)
        if y is not None:
            self.y_label = str(y)
        if (handle := self._live()) is not None:
            if x is not None:
                handle.enqueue_set_prop("x_label", self.x_label)
            if y is not None:
                handle.enqueue_set_prop("y_label", self.y_label)

    def props(self) -> dict[str, Any]:
        include_payload = _include_startup_resource_payloads()
        if include_payload and any(
            self._cached_payload_b64.get(column) is None for column in self.y_columns
        ):
            self._refresh_cached_payload_b64()
        series_items: list[dict[str, Any]] = []
        for index, y_column in enumerate(self.y_columns):
            payload = self._cached_payloads.get(y_column) or b""
            item: dict[str, Any] = {
                "label": self._series_label(index),
                "data_format": self.data_format,
                "points": self._payload_point_count(payload, self.data_format),
                "_payload_token": self._payload_tokens.get(y_column, 0),
            }
            if (color := self._series_color(index)) is not None:
                item["color"] = color
            item["line_style"] = self._series_line_style(index)
            if include_payload:
                item["data_b64"] = self._cached_payload_b64.get(y_column) or ""
            series_items.append(item)
        return {
            "frame": self.frame_summary.to_dict(),
            "x": self.x or "",
            "y": self.y,
            "x_label": self.x_label,
            "y_label": self.y_label,
            "show_grid": self.show_grid,
            "show_axes": self.show_axes,
            "show_ticks": self.show_ticks,
            "show_toolbar": self.show_toolbar,
            "show_legend": self.show_legend,
            "legend_position": self.legend_position,
            "interaction": self.interaction,
            "tick_count": self.tick_count,
            "auto_fit": self.auto_fit,
            "line_width": self.line_width,
            "window_size": self.window_size,
            "max_points": self.max_points,
            "series": series_items,
        }


class PieChart(Widget):
    kind = "pie_chart"

    def __init__(
        self,
        data: Any = None,
        *,
        labels: Sequence[object] | None = None,
        values: Sequence[object] | None = None,
        category: str | None = None,
        value: str | None = None,
        aggregate: str = "count",
        top_n: int | None = None,
        other_label: str = "Other",
        donut: bool = False,
        inner_radius: float = 0.52,
        start_angle: float = -90.0,
        clockwise: bool = True,
        label_mode: str = "auto",
        value_mode: str = "percent",
        show_legend: bool = True,
        legend_position: str = "right",
        show_labels: bool = False,
        selected: str | int | None = None,
        colors: Sequence[object] | Mapping[str, object] | None = None,
        title: str | None = None,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        if data is not None:
            if category is None:
                raise ValueError("PieChart frame data requires category=")
            payload = _pie_from_frame(
                data,
                category=category,
                value=value,
                aggregate=aggregate,
                colors=colors,
                top_n=top_n,
                other_label=other_label,
            )
        else:
            if labels is None or values is None:
                raise ValueError("PieChart requires labels/values or frame data with category=")
            payload = _normalize_pie_data(
                labels,
                values,
                colors=colors,
                top_n=top_n,
                other_label=other_label,
            )
        self.data = data
        self.category = category
        self.value = value
        self.aggregate = aggregate
        self.top_n = top_n
        self.other_label = str(other_label)
        self.donut = bool(donut)
        self.inner_radius = max(0.18, min(0.82, float(inner_radius)))
        self.start_angle = float(start_angle)
        self.clockwise = bool(clockwise)
        self.label_mode = str(label_mode).strip().lower()
        if self.label_mode not in {"auto", "inside", "outside", "legend", "none"}:
            raise ValueError("PieChart label_mode must be auto, inside, outside, legend, or none")
        self.value_mode = str(value_mode).strip().lower()
        if self.value_mode not in {"percent", "value", "both", "none"}:
            raise ValueError("PieChart value_mode must be percent, value, both, or none")
        self.show_legend = bool(show_legend)
        self.legend_position = str(legend_position).strip().lower()
        if self.legend_position not in {"right", "left", "bottom", "top", "none"}:
            raise ValueError("PieChart legend_position must be right, left, bottom, top, or none")
        self.show_labels = bool(show_labels)
        self.selected = selected
        self.title = None if title is None else str(title)
        self._payload = payload
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def set_data(
        self,
        labels: Sequence[object],
        values: Sequence[object],
        *,
        colors: Sequence[object] | Mapping[str, object] | None = None,
    ) -> None:
        self._payload = _normalize_pie_data(labels, values, colors=colors)
        if (handle := self._live()) is not None:
            handle.enqueue_set_prop("pie_data_token", self._data_token())
            handle.enqueue_replace_node(self.to_dict())

    def _data_token(self) -> float:
        return float(zlib.crc32(repr((self._payload.labels, self._payload.values)).encode("utf-8")))

    def props(self) -> dict[str, Any]:
        return {
            "labels": list(self._payload.labels),
            "values": list(self._payload.values),
            "colors": list(self._payload.colors),
            "total": self._payload.total,
            "input_count": self._payload.input_count,
            "finite_count": self._payload.finite_count,
            "donut": self.donut,
            "inner_radius": self.inner_radius,
            "start_angle": self.start_angle,
            "clockwise": self.clockwise,
            "label_mode": self.label_mode,
            "value_mode": self.value_mode,
            "show_legend": self.show_legend,
            "legend_position": self.legend_position,
            "show_labels": self.show_labels,
            "selected": self.selected,
            "title": self.title,
            "_data_token": self._data_token(),
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
        color: str | Any | None = None,
        colors: Any | None = None,
        scalars: str | Any | None = None,
        point_size: float = 4.0,
        point_sizes: str | Any | None = None,
        auto_point_size: bool = True,
        opacity: float = 1.0,
        clim: tuple[float, float] | None = None,
        log_scale: bool = False,
        nan_color: tuple[float, float, float] | None = None,
        size_range: tuple[float, float] | None = None,
        on_pick: ScatterPickCallback | None = None,
        grid: bool = False,
        major_planes: bool = False,
        minor_planes: bool = False,
        grid_sticky: bool = True,
        grid_all_edges: bool = False,
        axis_x: str = "X",
        axis_y: str = "Y",
        axis_z: str = "Z",
        background: tuple[float, float, float] | None = None,
        legend: bool = False,
        legend_position: str = "top-right",
        legend_entries: list[tuple[str, float, float, float]] | None = None,
        scalar_bar: bool = False,
        scalar_bar_vmin: "float | None" = None,
        scalar_bar_vmax: "float | None" = None,
        scalar_bar_log_scale: bool = False,
        scalar_bar_colormap: str = "viridis",
        scalar_bar_title: str | None = None,
        orientation_axes: bool = False,
        hover: "str | list[str] | None" = None,
        on_hover: "ScatterPickCallback | None" = None,
        lod: bool = False,
        lod_threshold: int = 200_000,
        lod_factor: int = 8,
        interactive_render_scale: float = 1.0,
        auto_quality: bool = False,
        quality_target_fps: float = 10.0,
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
        self.color = color
        self.colors = colors
        self.scalars = scalars
        self.point_size = float(point_size)
        self.point_sizes = point_sizes
        self.auto_point_size = bool(auto_point_size)
        self.opacity = float(max(0.0, min(1.0, opacity)))
        self.clim = clim
        self.log_scale = bool(log_scale)
        self.nan_color = nan_color
        self.size_range = size_range
        self.frame_summary = summarize_frame(frame)
        self.on_pick = on_pick
        self.on_hover = on_hover
        self.pick: ScatterPick | None = None
        self.picked_point: tuple[float, float, float] | None = None
        self.picked_index: int | None = None
        self.picked_actor: int | None = None
        self.hover_point: tuple[float, float, float] | None = None
        self.hover_index: int | None = None
        self.hover_actor: int | None = None
        self.hover_text: str | None = None
        # selected: flat ordered ScatterHit list across all actors.
        # selected_indices: flat ordered positional indices across all actors.
        # selected_index_values: flat list of dataframe index labels when any actor has non-trivial
        # index; otherwise None. Matches DragonSci's contract.
        self.selected: list[ScatterHit] = []
        self.selected_indices: list[int] = []
        self.selected_index_values: list | None = None
        self._cached_payload: bytes | None = None
        self._cached_payload_b64: str | None = None
        self._parallel_projection: bool = False
        self._grid_visible: bool = bool(grid)
        self._major_planes: bool = bool(major_planes)
        self._minor_planes: bool = bool(minor_planes)
        self._grid_sticky: bool = bool(grid_sticky)
        self._grid_all_edges: bool = bool(grid_all_edges)
        self._tick_override: tuple[int | None, int | None, int | None] = (None, None, None)
        self._axis_labels: tuple[str, str, str] = (str(axis_x), str(axis_y), str(axis_z))
        self._axis_visible: tuple[bool, bool, bool] = (True, True, True)
        self._background: tuple[float, float, float] | None = background
        self._legend_visible: bool = bool(legend)
        self._legend_position: str = str(legend_position)
        self._legend_entries: list[tuple[str, float, float, float]] = list(legend_entries or [])
        self._scalar_bar_visible: bool = bool(scalar_bar)
        self._scalar_bar_vmin: float = float(scalar_bar_vmin) if scalar_bar_vmin is not None else 0.0
        self._scalar_bar_vmax: float = float(scalar_bar_vmax) if scalar_bar_vmax is not None else 1.0
        self._scalar_bar_log_scale: bool = bool(scalar_bar_log_scale)
        self._scalar_bar_colormap: str = str(scalar_bar_colormap)
        self._scalar_bar_title: str | None = scalar_bar_title
        self._orientation_axes_visible: bool = bool(orientation_axes)
        # Phase 5 — LOD and picking mode (not startup props; always set live)
        self._lod_enabled: bool = bool(lod)
        self._lod_threshold: int = max(0, int(lod_threshold))
        self._lod_factor: int = max(1, int(lod_factor))
        self._interactive_render_scale: float = max(0.25, min(1.0, float(interactive_render_scale)))
        self._auto_quality: bool = bool(auto_quality)
        self._quality_target_fps: float = max(1.0, float(quality_target_fps))
        self._picking_mode: str = "point"
        self._on_select: Any | None = None
        self._camera_links: set["Scatter3D"] = set()
        self._propagating: bool = False
        self._hover_tooltip: bool = True
        self._hover: str | list[str] | None = hover
        self._primary_row_labels: list | None = self._extract_row_labels(frame)
        self._actor_row_labels: dict[int, list | None] = {}
        # Stores per-handle ellipsoid params for partial updates (center, covariance).
        self._ellipsoid_params: dict[int, dict] = {}
        # Pre-live pending scene operations replayed in _queue_startup_resources.
        self._pending_scene_ops: list[tuple[str, tuple]] = []
        self._primary_cleared: bool = False
        # Auto-derived color metadata recomputed in _refresh_cached_payload_b64.
        self._auto_legend_entries: "list[tuple[str, float, float, float]] | None" = None
        self._auto_legend_title: "str | None" = None
        self._auto_scalar_vmin: float | None = None
        self._auto_scalar_vmax: float | None = None
        self._auto_scalar_colormap: "str | None" = None
        self._auto_scalar_log_scale: "bool | None" = None
        # Per-field explicit-override flags; each set only when user passes that arg.
        self._scalar_range_explicit: bool = False
        self._scalar_log_explicit: bool = False
        self._scalar_colormap_explicit: bool = False
        self._scalar_title_explicit: bool = False
        # Determine format upfront so props() and live setters are consistent.
        self.data_format = (
            "point_instance_v1"
            if _scatter_needs_v1(color, colors, scalars, point_sizes, opacity, nan_color, clim, log_scale)
            else "xyz_f32_v0"
        )
        self._refresh_cached_payload()
        if scalar_bar_vmin is not None or scalar_bar_vmax is not None:
            self._scalar_range_explicit = True
        if scalar_bar_log_scale:
            self._scalar_log_explicit = True
        if scalar_bar_colormap != "viridis":
            self._scalar_colormap_explicit = True
        if scalar_bar_title is not None:
            self._scalar_title_explicit = True
        super().__init__(id=id, key=key, class_=class_, style=style, tooltip=tooltip, parent=parent)

    def _build_payload(self) -> bytes | None:
        if self._primary_cleared:
            return b""
        if self.data_format == "point_instance_v1":
            return _pack_point_instances(
                self.frame, self.x, self.y, self.z,
                color=self.color,
                colors=self.colors,
                scalars=self.scalars,
                point_size=self.point_size,
                point_sizes=self.point_sizes,
                size_range=self.size_range,
                opacity=self.opacity,
                colormap=self.colormap,
                clim=self.clim,
                log_scale=self.log_scale,
                nan_color=self.nan_color,
            )
        return _pack_xyz_bytes(self.frame, self.x, self.y, self.z)

    def _refresh_cached_payload(self) -> None:
        buf = self._build_payload()
        self._cached_payload = buf
        self._cached_payload_b64 = None
        self._payload_token = (
            zlib.crc32(buf) if buf is not None and len(buf) > 0 else 0
        )
        self._compute_auto_color_meta()

    def _refresh_cached_payload_b64(self) -> None:
        buf = self._cached_payload
        if buf is None:
            self._refresh_cached_payload()
            buf = self._cached_payload
        self._cached_payload_b64 = base64.b64encode(buf).decode("ascii") if buf is not None else None

    def _compute_auto_color_meta(self) -> None:
        """Derive legend entries and scalar range from the current color/scalars settings."""
        self._auto_legend_entries = None
        self._auto_legend_title = None
        self._auto_scalar_vmin = None
        self._auto_scalar_vmax = None
        self._auto_scalar_colormap = None
        self._auto_scalar_log_scale = None
        try:
            import numpy as np

            def _scalar_range(arr: Any) -> "tuple[float, float] | None":
                # Scalar bar vmin/vmax are raw domain (matching DragonSci public API).
                # Log-space conversion happens only inside _scalars_to_rgb for color normalization.
                if self.clim is not None:
                    return float(self.clim[0]), float(self.clim[1])
                a = np.asarray(arr, dtype=np.float32)
                finite = a[np.isfinite(a)]
                if len(finite) == 0:
                    return None
                return float(finite.min()), float(finite.max())

            if isinstance(self.color, str):
                col_data = _get_frame_col(self.frame, self.color)
                if col_data is not None:
                    if _is_categorical(col_data):
                        self._auto_legend_entries = _categorical_legend_entries(col_data)
                        self._auto_legend_title = self.color
                    else:
                        rng = _scalar_range(col_data)
                        if rng is not None:
                            self._auto_scalar_vmin, self._auto_scalar_vmax = rng
                            self._auto_scalar_colormap = self.colormap
                            self._auto_scalar_log_scale = self.log_scale
                            self._auto_legend_title = self.color
            elif self.scalars is not None:
                col_data = (
                    _get_frame_col(self.frame, self.scalars)
                    if isinstance(self.scalars, str)
                    else self.scalars
                )
                rng = _scalar_range(col_data)
                if rng is not None:
                    self._auto_scalar_vmin, self._auto_scalar_vmax = rng
                    self._auto_scalar_colormap = self.colormap
                    self._auto_scalar_log_scale = self.log_scale
                    if isinstance(self.scalars, str):
                        self._auto_legend_title = self.scalars
            elif self.colors is None:
                # Default z-colormap path. When clim or log_scale is set, v1 packing is
                # forced and _scalars_to_rgb applies them; use _scalar_range to match.
                # When neither is set, native xyz_f32_v0 colors linearly from finite z range.
                z_data = _get_frame_col(self.frame, self.z) if isinstance(self.z, str) else self.z
                if z_data is not None:
                    rng = _scalar_range(z_data)
                    if rng is not None:
                        self._auto_scalar_vmin, self._auto_scalar_vmax = rng
                        self._auto_scalar_colormap = self.colormap
                        self._auto_scalar_log_scale = self.log_scale
                        self._auto_legend_title = self.z if isinstance(self.z, str) else None
        except Exception:
            pass

    def _effective_legend_entries(self) -> "list[tuple[str, float, float, float]]":
        """Return user-set entries if any, else auto-derived categorical entries."""
        if self._legend_entries:
            return list(self._legend_entries)
        if self._auto_legend_entries:
            return list(self._auto_legend_entries)
        return []

    def _effective_scalar_range(self) -> "tuple[float, float]":
        """Return scalar bar vmin/vmax: auto-derived unless user has explicitly set them."""
        if (
            not self._scalar_range_explicit
            and self._auto_scalar_vmin is not None
            and self._auto_scalar_vmax is not None
        ):
            return (self._auto_scalar_vmin, self._auto_scalar_vmax)
        return (self._scalar_bar_vmin, self._scalar_bar_vmax)

    def _effective_scalar_bar_state(self) -> "tuple[float, float, bool, str, str | None]":
        """Return (vmin, vmax, log_scale, colormap, title) for the scalar bar.

        Each field prefers the auto-derived value unless the user has explicitly overridden
        that specific field via show_scalar_bar() or the constructor.
        """
        vmin, vmax = self._effective_scalar_range()
        log_scale = (
            self._scalar_bar_log_scale
            if self._scalar_log_explicit or self._auto_scalar_log_scale is None
            else self._auto_scalar_log_scale
        )
        colormap = (
            self._scalar_bar_colormap
            if self._scalar_colormap_explicit or self._auto_scalar_colormap is None
            else self._auto_scalar_colormap
        )
        title = (
            self._scalar_bar_title
            if self._scalar_title_explicit
            else (self._scalar_bar_title or self._auto_legend_title)
        )
        return vmin, vmax, log_scale, colormap, title

    def _primary_hover_columns_payload(self, handle: Any) -> "tuple[str, list[object]] | None":
        if not handle.app._native_method_available("enqueue_set_scatter_primary_hover_columns"):
            return None
        return self._extract_hover_columns(self.frame, self._hover)

    def _enqueue_primary_hover_metadata(self, handle: Any) -> None:
        columns_payload = self._primary_hover_columns_payload(handle)
        if columns_payload is not None:
            columns_json, buffers = columns_payload
            handle.enqueue_set_scatter_primary_hover_columns(columns_json, buffers)
            return
        meta = self._extract_hover_meta(self.frame, self._hover)
        if meta is not None:
            handle.enqueue_set_scatter_primary_hover_meta(meta)

    @classmethod
    def colormap_names(cls) -> list[str]:
        return sorted(_SCATTER_COLORMAPS)

    @staticmethod
    def _immutable_payload_bytes(payload: object) -> bytes:
        if isinstance(payload, bytes):
            return payload
        return memoryview(payload).tobytes()

    @staticmethod
    def _payload_point_count(payload: bytes, payload_format: str) -> int:
        bytes_per_point = 32 if payload_format == "point_instance_v1" else 12
        return len(payload) // bytes_per_point if bytes_per_point else 0

    @classmethod
    def prepare_points(
        cls,
        frame: Any,
        *,
        x: str,
        y: str,
        z: str,
        colormap: str = "viridis",
        color: str | Any | None = None,
        colors: Any | None = None,
        scalars: str | Any | None = None,
        point_size: float = 4.0,
        point_sizes: str | Any | None = None,
        opacity: float = 1.0,
        clim: tuple[float, float] | None = None,
        log_scale: bool = False,
        nan_color: tuple[float, float, float] | None = None,
        size_range: tuple[float, float] | None = None,
        hover: str | list[str] | None = None,
    ) -> ScatterPayload:
        """Pack Scatter3D points without mutating a live widget.

        This method is intended for background workers and high-rate streams.
        The returned payload can be delivered with ``set_prepared_points`` or
        ``enqueue_prepared_points`` without repacking the frame.
        """
        cmap = _scatter_colormap(colormap)
        payload_format = (
            "point_instance_v1"
            if _scatter_needs_v1(color, colors, scalars, point_sizes, opacity, nan_color, clim, log_scale)
            else "xyz_f32_v0"
        )
        t0 = time.perf_counter()
        if payload_format == "point_instance_v1":
            raw = _pack_point_instances(
                frame,
                x,
                y,
                z,
                color=color,
                colors=colors,
                scalars=scalars,
                point_size=point_size,
                point_sizes=point_sizes,
                size_range=size_range,
                opacity=opacity,
                colormap=cmap,
                clim=clim,
                log_scale=log_scale,
                nan_color=nan_color,
            )
        else:
            raw = _pack_xyz_bytes(frame, x, y, z)
        bounds = _xyz_bounds(frame, x, y, z)
        pack_ms = (time.perf_counter() - t0) * 1000.0
        if raw is None:
            raise RuntimeError(
                "Scatter3D.prepare_points requires NumPy and addressable numeric x/y/z columns"
            )
        data = cls._immutable_payload_bytes(raw)
        return ScatterPayload(
            data=data,
            payload_format=payload_format,
            colormap=cmap,
            point_count=cls._payload_point_count(data, payload_format),
            pack_ms=pack_ms,
            axis_labels=(str(x), str(y), str(z)),
            bounds=bounds,
            hover_meta=cls._extract_hover_meta(frame, hover),
            frame_summary=summarize_frame(frame),
        )

    def set_prepared_points(
        self,
        payload: ScatterPayload,
        *,
        coalesce: bool = True,
        update_metadata: bool = True,
        fit: bool = False,
    ) -> None:
        """Apply a prepared payload on the UI thread without repacking data."""
        if update_metadata:
            self.colormap = payload.colormap
            self.data_format = payload.payload_format
            self.x, self.y, self.z = payload.axis_labels
            if payload.frame_summary is not None:
                self.frame_summary = payload.frame_summary
            self._cached_payload = payload.data
            self._cached_payload_b64 = None
            self._payload_token = zlib.crc32(payload.data) if payload.data else 0
        self.enqueue_prepared_points(payload, coalesce=coalesce, fit=fit)

    def enqueue_prepared_points(
        self,
        payload: ScatterPayload,
        *,
        coalesce: bool = True,
        include_metadata: bool = True,
        fit: bool = False,
        colormap_override: str | None = None,
    ) -> None:
        """Thread-safe native enqueue for an already-packed primary scatter frame."""
        handle = self._live()
        if handle is None:
            return
        payload_colormap = _scatter_colormap(payload.colormap)
        render_colormap = (
            _scatter_colormap(
                self.colormap if colormap_override is None else colormap_override
            )
            if payload.payload_format == "xyz_f32_v0"
            else payload_colormap
        )
        handle.enqueue_set_scatter_points_packed(
            payload.data,
            pack_ms=payload.pack_ms,
            enqueue_epoch_ms=time.time() * 1000.0,
            colormap=render_colormap,
            payload_format=payload.payload_format,
            coalesce=coalesce,
            fit=fit,
            bounds_min=payload.bounds[0] if payload.bounds is not None else None,
            bounds_max=payload.bounds[1] if payload.bounds is not None else None,
        )
        if include_metadata:
            previous_colormap = self.colormap
            scalar_bar_tracks_colormap = not self._scalar_colormap_explicit or (
                self._scalar_bar_colormap == previous_colormap
            )
            if payload.payload_format != "xyz_f32_v0":
                self.colormap = payload_colormap
            elif colormap_override is not None:
                self.colormap = render_colormap
                if self._scalar_bar_visible:
                    self._scalar_bar_colormap = render_colormap
                    self._auto_scalar_colormap = render_colormap
            if payload.bounds is not None and not self._scalar_range_explicit:
                self._auto_scalar_vmin = float(payload.bounds[0][2])
                self._auto_scalar_vmax = float(payload.bounds[1][2])
            if scalar_bar_tracks_colormap:
                self._scalar_bar_colormap = render_colormap
                self._auto_scalar_colormap = render_colormap
            if not self._scalar_title_explicit:
                self._auto_legend_title = payload.axis_labels[2]
            handle.enqueue_set_scatter_tooltip_axis_labels(*payload.axis_labels)
            if payload.hover_meta is not None:
                handle.enqueue_set_scatter_primary_hover_meta(payload.hover_meta)
            if self._scalar_bar_visible:
                eff_vmin, eff_vmax, eff_log, eff_cm, eff_title = self._effective_scalar_bar_state()
                handle.enqueue_set_scatter_scalar_bar(
                    True, eff_vmin, eff_vmax, eff_log, eff_cm, eff_title
                )

    def stream_prepared_frames(
        self,
        frames: Iterable[ScatterPayload],
        *,
        interval_ms: float | Callable[[], float] = 16.0,
        loop: bool = True,
        on_frame: Callable[[ScatterPayload, int, ScatterStreamMetrics], None] | None = None,
        ui_interval_ms: float = 250.0,
        handoff: str = "direct",
    ) -> ScatterFrameStream:
        """Create a latest-frame stream for already-prepared scatter payloads.

        ``handoff="direct"`` enqueues prepared payloads from the stream thread.
        ``handoff="callback"`` schedules a Python UI callback for each payload,
        which is useful for comparing the older callback handoff path.
        """
        stream = ScatterFrameStream(
            self,
            frames,
            interval_ms=interval_ms,
            loop=loop,
            on_frame=on_frame,
            ui_interval_ms=ui_interval_ms,
            handoff=handoff,
        )
        setattr(self, "_active_frame_stream", stream)
        return stream

    def set_points(
        self,
        frame: Any,
        *,
        x: str,
        y: str | None = None,
        z: str | None = None,
        color: str | Any | None = _UNSET,
        colors: Any | None = _UNSET,
        scalars: str | Any | None = _UNSET,
        point_sizes: str | Any | None = _UNSET,
        point_size: float | None = None,
        opacity: float | None = None,
        clim: tuple[float, float] | None = _UNSET,
        log_scale: bool | None = None,
        nan_color: tuple[float, float, float] | None = _UNSET,
        size_range: tuple[float, float] | None = _UNSET,
        hover: "str | list[str] | None" = _UNSET,
        fit: bool = False,
    ) -> None:
        self.frame = frame
        self.x = x
        self.y = y if y is not None else self.y
        self.z = z if z is not None else self.z
        if color is not _UNSET:
            self.color = color
        if colors is not _UNSET:
            self.colors = colors
        if scalars is not _UNSET:
            self.scalars = scalars
        if point_sizes is not _UNSET:
            self.point_sizes = point_sizes
        if point_size is not None:
            self.point_size = float(point_size)
        if opacity is not None:
            self.opacity = float(max(0.0, min(1.0, opacity)))
        if clim is not _UNSET:
            self.clim = clim
        if log_scale is not None:
            self.log_scale = bool(log_scale)
        if nan_color is not _UNSET:
            self.nan_color = nan_color
        if size_range is not _UNSET:
            self.size_range = size_range
        if hover is not _UNSET:
            self._hover = hover
        self._primary_row_labels = self._extract_row_labels(frame)
        self._primary_cleared = False
        self.data_format = (
            "point_instance_v1"
            if _scatter_needs_v1(self.color, self.colors, self.scalars, self.point_sizes, self.opacity, self.nan_color, self.clim, self.log_scale)
            else "xyz_f32_v0"
        )
        self.frame_summary = summarize_frame(frame)
        # Clear point-layer metadata (DragonSci parity: set_points replaces the point scene).
        self._actor_row_labels.clear()
        self._pending_scene_ops = [
            (op, args) for op, args in self._pending_scene_ops
            if op not in ("add_points", "add_stream", "set_actor_visibility")
        ]
        if hasattr(self, "_next_actor_id"):
            self._next_actor_id = 1
        self._cached_payload = None
        self._cached_payload_b64 = None
        if (handle := self._live()) is not None:
            # Clear extra actors before replacing primary data (DragonSci parity).
            handle.enqueue_clear_scatter_actors()
            t0 = time.perf_counter()
            payload = self._build_payload()
            pack_ms = (time.perf_counter() - t0) * 1000.0
            if payload is None:
                raise RuntimeError(
                    "live Scatter3D.set_points requires NumPy and addressable numeric x/y/z columns"
                )
            self._cached_payload = payload
            self._compute_auto_color_meta()
            handle.enqueue_set_scatter_points_packed(
                payload,
                pack_ms=pack_ms,
                enqueue_epoch_ms=time.time() * 1000.0,
                colormap=self.colormap,
                payload_format=self.data_format,
                fit=fit,
            )
            handle.enqueue_set_scatter_tooltip_axis_labels(self.x, self.y, self.z)
            self._enqueue_primary_hover_metadata(handle)
            if self._legend_visible:
                handle.enqueue_set_scatter_legend(
                    True, self._legend_position,
                    list(self._effective_legend_entries()),
                    self._auto_legend_title,
                )
            if self._scalar_bar_visible:
                eff_vmin, eff_vmax, eff_log, eff_cm, eff_title = self._effective_scalar_bar_state()
                handle.enqueue_set_scatter_scalar_bar(
                    True, eff_vmin, eff_vmax, eff_log, eff_cm, eff_title
                )

    def set_colormap(self, colormap: str) -> None:
        previous_colormap = self.colormap
        next_colormap = _scatter_colormap(colormap)
        active_stream = getattr(self, "_active_frame_stream", None)
        if active_stream is not None and hasattr(active_stream, "set_colormap"):
            active_stream.set_colormap(next_colormap)
        scalar_bar_tracks_colormap = (
            not self._scalar_colormap_explicit
            or self._scalar_bar_colormap == previous_colormap
        )
        self.colormap = next_colormap
        if scalar_bar_tracks_colormap:
            self._scalar_bar_colormap = next_colormap
        # v1 packets bake colors, so a colormap change requires a repack.
        if self.data_format == "point_instance_v1":
            self._cached_payload = None
            self._cached_payload_b64 = None
        if (handle := self._live()) is not None:
            if self._cached_payload is not None:
                payload = self._cached_payload
                pack_ms = 0.0
            else:
                t0 = time.perf_counter()
                payload = self._build_payload()
                pack_ms = (time.perf_counter() - t0) * 1000.0
            if payload is None:
                raise RuntimeError(
                    "live Scatter3D.set_colormap requires NumPy and addressable numeric x/y/z columns"
                )
            self._cached_payload = payload
            self._payload_token = zlib.crc32(payload)
            self._compute_auto_color_meta()
            handle.enqueue_set_scatter_points_packed(
                payload,
                pack_ms=pack_ms,
                enqueue_epoch_ms=time.time() * 1000.0,
                colormap=self.colormap,
                payload_format=self.data_format,
            )
            # Native clears primary_hover_meta on SetScatterPointsPacked; re-send it.
            self._enqueue_primary_hover_metadata(handle)
            if self._scalar_bar_visible:
                eff_vmin, eff_vmax, eff_log, eff_cm, eff_title = self._effective_scalar_bar_state()
                handle.enqueue_set_scatter_scalar_bar(
                    True, eff_vmin, eff_vmax, eff_log, eff_cm, eff_title
                )

    def props(self) -> dict[str, Any]:
        include_payload = _include_startup_resource_payloads()
        if include_payload:
            if self._cached_payload_b64 is None:
                self._refresh_cached_payload_b64()
            data_b64 = self._cached_payload_b64
        else:
            if self._cached_payload is None:
                payload = self._build_payload()
                self._cached_payload = payload
                self._payload_token = (
                    zlib.crc32(payload) if payload is not None and len(payload) > 0 else 0
                )
                self._compute_auto_color_meta()
            data_b64 = None
        p: dict[str, Any] = {
            "frame": self.frame_summary.to_dict(),
            "x": self.x,
            "y": self.y,
            "z": self.z,
            "colormap": self.colormap,
            "data_format": self.data_format,
            "events": ["change"] if self.on_pick is not None else [],
            # Compact identity for diff — never sent to native directly.
            "_payload_token": self._payload_token,
            "grid_visible": self._grid_visible,
            "major_planes": self._major_planes,
            "minor_planes": self._minor_planes,
            "grid_sticky": self._grid_sticky,
            "grid_all_edges": self._grid_all_edges,
            "axis_x": self._axis_labels[0],
            "axis_y": self._axis_labels[1],
            "axis_z": self._axis_labels[2],
            "axis_vis_x": self._axis_visible[0],
            "axis_vis_y": self._axis_visible[1],
            "axis_vis_z": self._axis_visible[2],
        }
        if include_payload:
            p["data_b64"] = data_b64 or ""
        tx, ty, tz = self._tick_override
        if tx is not None:
            p["tick_x"] = tx
        if ty is not None:
            p["tick_y"] = ty
        if tz is not None:
            p["tick_z"] = tz
        if self._background is not None:
            r, g, b = self._background
            p["background"] = [float(r), float(g), float(b), 1.0]
        p["legend_visible"] = self._legend_visible
        p["legend_position"] = self._legend_position
        p["legend_entries"] = [
            {"label": lbl, "color": [r, g, b]}
            for lbl, r, g, b in self._effective_legend_entries()
        ]
        if self._auto_legend_title is not None:
            p["legend_title"] = self._auto_legend_title
        eff_vmin, eff_vmax, eff_log, eff_cm, eff_title = self._effective_scalar_bar_state()
        p["scalar_bar_visible"] = self._scalar_bar_visible
        p["scalar_bar_vmin"] = eff_vmin
        p["scalar_bar_vmax"] = eff_vmax
        p["scalar_bar_log_scale"] = eff_log
        p["scalar_bar_colormap"] = eff_cm
        if eff_title is not None:
            p["scalar_bar_title"] = eff_title
        p["orientation_axes_visible"] = self._orientation_axes_visible
        return p

    def show_grid(self, visible: bool = True) -> None:
        """Show or hide the axis grid and tick marks."""
        self._grid_visible = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_grid_visible(self._grid_visible)

    def show_grid_planes(self, major: bool = True, minor: bool = False) -> None:
        """Enable or disable filled grid planes behind the scatter.

        major: draw the three major (back-facing) grid planes.
        minor: draw minor subdivision lines on each plane.
        """
        self._major_planes = bool(major)
        self._minor_planes = bool(minor)
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_grid_planes(self._major_planes, self._minor_planes)

    def set_grid_options(
        self,
        *,
        sticky: bool = True,
        all_edges: bool = False,
    ) -> None:
        """Set grid stability options.

        ``sticky`` keeps automatically generated nice bounds and tick steps
        stable while new data remains inside the current grid range. ``all_edges``
        draws an unlabeled boundary box so rotations keep a consistent frame of
        reference even when the active tick/label face changes.
        """
        self._grid_sticky = bool(sticky)
        self._grid_all_edges = bool(all_edges)
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_grid_options(self._grid_sticky, self._grid_all_edges)

    def set_ticks(
        self,
        x: int | None = None,
        y: int | None = None,
        z: int | None = None,
    ) -> None:
        """Override the number of tick marks on each axis (None = auto)."""
        self._tick_override = (x, y, z)
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_ticks(x, y, z)

    def set_axes(self, x: str, y: str, z: str) -> None:
        """Set the axis label text shown at the ends of each axis."""
        self._axis_labels = (str(x), str(y), str(z))
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_axes(str(x), str(y), str(z))

    def set_axis_visibility(
        self,
        x: bool = True,
        y: bool = True,
        z: bool = True,
    ) -> None:
        """Show or hide individual axes and their tick labels."""
        self._axis_visible = (bool(x), bool(y), bool(z))
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_axis_visibility(bool(x), bool(y), bool(z))

    def set_background(self, color, g: float | None = None, b: float | None = None) -> None:
        """Set the scatter background fill color.

        Accepts either three separate floats ``(r, g, b)``, a 3-tuple/list, or a
        ``"#rrggbb"`` hex string. Values are in the 0.0–1.0 range.
        """
        if g is not None and b is not None:
            r_f, g_f, b_f = float(color), float(g), float(b)
        elif isinstance(color, str):
            h = color.lstrip("#")
            if len(h) != 6:
                raise ValueError(f"hex color must be '#rrggbb', got {color!r}")
            r_f = int(h[0:2], 16) / 255.0
            g_f = int(h[2:4], 16) / 255.0
            b_f = int(h[4:6], 16) / 255.0
        else:
            r_f, g_f, b_f = float(color[0]), float(color[1]), float(color[2])
        self._background = (r_f, g_f, b_f)
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_background(r_f, g_f, b_f)

    _LEGEND_POSITIONS = frozenset({"top-right", "top-left", "bottom-right", "bottom-left"})

    @property
    def legend_position(self) -> str:
        return self._legend_position

    @legend_position.setter
    def legend_position(self, value: str) -> None:
        if value not in self._LEGEND_POSITIONS:
            raise ValueError(f"legend_position must be one of {sorted(self._LEGEND_POSITIONS)}, got {value!r}")
        self._legend_position = value
        if self._legend_visible and (handle := self._live()) is not None:
            handle.enqueue_set_scatter_legend(
                True, value, list(self._effective_legend_entries()), self._auto_legend_title
            )

    def show_legend(
        self,
        visible: bool = True,
        position: "str | None" = None,
        entries: list[tuple[str, float, float, float]] | None = None,
    ) -> None:
        """Show or hide the color legend overlay.

        entries: list of (label, r, g, b) tuples (0.0–1.0 per channel).
        position: 'top-right', 'top-left', 'bottom-right', 'bottom-left'. When None,
                  the current legend_position is kept.
        """
        self._legend_visible = bool(visible)
        if entries is not None:
            self._legend_entries = list(entries)
        if position is not None:
            if position not in self._LEGEND_POSITIONS:
                raise ValueError(
                    f"legend_position must be one of {sorted(self._LEGEND_POSITIONS)}, got {position!r}"
                )
            self._legend_position = position
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_legend(
                bool(visible), self._legend_position,
                list(self._effective_legend_entries()),
                self._auto_legend_title,
            )

    def show_scalar_bar(
        self,
        visible: bool = True,
        vmin: float | None = None,
        vmax: float | None = None,
        log_scale: bool | None = None,
        colormap: str | None = None,
        title: str | None = None,
    ) -> None:
        """Show or hide the scalar color bar overlay.

        When vmin/vmax are omitted the bar defaults to the range of the current
        color/scalars data.  Passing explicit values fixes the range and disables
        automatic scaling on subsequent data updates.
        """
        self._scalar_bar_visible = bool(visible)
        if vmin is not None:
            self._scalar_bar_vmin = float(vmin)
            self._scalar_range_explicit = True
        if vmax is not None:
            self._scalar_bar_vmax = float(vmax)
            self._scalar_range_explicit = True
        if log_scale is not None:
            self._scalar_bar_log_scale = bool(log_scale)
            self._scalar_log_explicit = True
        if colormap is not None:
            self._scalar_bar_colormap = _scatter_colormap(colormap)
            self._scalar_colormap_explicit = True
        if title is not None:
            self._scalar_bar_title = title
            self._scalar_title_explicit = True
        eff_vmin, eff_vmax, eff_log, eff_cm, eff_title = self._effective_scalar_bar_state()
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_scalar_bar(
                bool(visible), eff_vmin, eff_vmax, eff_log, eff_cm, eff_title
            )

    def scalar_bar(
        self,
        visible: bool = True,
        vmin: float | None = None,
        vmax: float | None = None,
        log_scale: bool | None = None,
        colormap: str | None = None,
        title: str | None = None,
    ) -> None:
        """Alias for show_scalar_bar() — DragonSci-compatible name."""
        self.show_scalar_bar(visible=visible, vmin=vmin, vmax=vmax,
                             log_scale=log_scale, colormap=colormap, title=title)

    def show_orientation_axes(self, visible: bool = True) -> None:
        """Show or hide the orientation axes indicator in the bottom-left corner."""
        self._orientation_axes_visible = bool(visible)
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_orientation_axes(bool(visible))

    # ── User labels ──────────────────────────────────────────────────────────

    def add_label(
        self,
        position: tuple[float, float, float],
        text: str,
        color: tuple[float, float, float] = (1.0, 1.0, 1.0),
        size: float = 14.0,
        anchor: str = "center",
    ) -> int:
        """Add a world-space text label at ``position``.

        Returns a handle that can be passed to ``update_label``, ``remove_label``,
        or ``set_label_visibility``.
        """
        _VALID_ANCHORS = frozenset({"left", "center", "right", "top", "bottom"})
        if anchor not in _VALID_ANCHORS:
            raise ValueError(f"anchor must be one of {sorted(_VALID_ANCHORS)}, got {anchor!r}")
        if not hasattr(self, "_next_label_id"):
            self._next_label_id: int = 0
        lid = self._next_label_id
        self._next_label_id += 1
        r, g, b = color
        if (handle := self._live()) is not None:
            handle.enqueue_add_scatter_label(lid, *position, text, float(r), float(g), float(b), float(size), anchor)
        else:
            px, py, pz = position
            self._pending_scene_ops.append(("add_label", (lid, float(px), float(py), float(pz), text, float(r), float(g), float(b), float(size), anchor)))
        return lid

    def update_label(
        self,
        handle: int,
        position: tuple[float, float, float] | None = None,
        text: str | None = None,
        color: tuple[float, float, float] | None = None,
        size: float | None = None,
        anchor: str | None = None,
    ) -> None:
        """Update a world-space label by its handle."""
        if (wh := self._live()) is not None:
            x, y, z = position if position is not None else (None, None, None)
            r, g, b = color if color is not None else (None, None, None)
            wh.enqueue_update_scatter_label(handle, x, y, z, text, r, g, b, size, anchor)
        else:
            px = position[0] if position is not None else None
            py = position[1] if position is not None else None
            pz = position[2] if position is not None else None
            r = color[0] if color is not None else None
            g = color[1] if color is not None else None
            b = color[2] if color is not None else None
            self._pending_scene_ops.append(("update_label", (handle, px, py, pz, text, r, g, b, size, anchor)))

    def remove_label(self, handle: int) -> None:
        """Remove a world-space label by its handle."""
        if (wh := self._live()) is not None:
            wh.enqueue_remove_scatter_label(handle)
        else:
            self._pending_scene_ops.append(("remove_label", (handle,)))

    def set_label_visibility(self, handle: int, visible: bool) -> None:
        """Show or hide a world-space label."""
        if (wh := self._live()) is not None:
            wh.enqueue_set_scatter_label_visible(handle, bool(visible))
        else:
            self._pending_scene_ops.append(("set_label_visibility", (handle, bool(visible))))

    def clear_labels(self) -> None:
        """Remove all user-added world-space labels."""
        if (wh := self._live()) is not None:
            wh.enqueue_clear_scatter_labels()
        else:
            self._pending_scene_ops.append(("clear_labels", ()))

    # ── Line and box overlays ─────────────────────────────────────────────────

    @staticmethod
    def _coerce_line_segments(segments: Any) -> "list[list[float]]":
        """Normalize DragonSci ``(N, 6)`` line segments.

        For compatibility with earlier DragonGUI builds, ``(N, 3)`` point lists
        are still accepted and converted into adjacent polyline segments.
        """
        import numpy as np

        arr = np.asarray(segments, dtype=np.float32)
        if arr.ndim != 2 or arr.shape[1] not in (3, 6):
            raise ValueError(f"line segments must be shape (N, 6) or polyline shape (N, 3), got {arr.shape}")
        if arr.shape[1] == 3:
            if arr.shape[0] < 2:
                return []
            out = np.empty((arr.shape[0] - 1, 6), dtype=np.float32)
            out[:, 0:3] = arr[:-1]
            out[:, 3:6] = arr[1:]
            arr = out
        return [[float(v) for v in row] for row in np.ascontiguousarray(arr).reshape((-1, 6))]

    def add_lines(
        self,
        segments: Any,
        color: tuple[float, float, float] = (1.0, 1.0, 1.0),
    ) -> int:
        """Add world-space line segment overlays.

        ``segments`` follows DragonSci's ``(N, 6)`` shape, where each row is
        ``[x0, y0, z0, x1, y1, z1]``. Existing ``(N, 3)`` point lists are treated
        as a polyline compatibility path.
        """
        if not hasattr(self, "_next_overlay_id"):
            self._next_overlay_id: int = 0
        oid = self._next_overlay_id
        self._next_overlay_id += 1
        r, g, b = color
        segs = self._coerce_line_segments(segments)
        if (wh := self._live()) is not None:
            wh.enqueue_add_scatter_lines(oid, segs, float(r), float(g), float(b))
        else:
            self._pending_scene_ops.append(("add_lines", (oid, segs, float(r), float(g), float(b))))
        return oid

    def update_lines(
        self,
        handle: int,
        segments: Any,
        color: tuple[float, float, float] = (1.0, 1.0, 1.0),
    ) -> None:
        """Replace the geometry and color of an existing line overlay."""
        r, g, b = color
        segs = self._coerce_line_segments(segments)
        payload = (handle, segs, float(r), float(g), float(b))
        if (wh := self._live()) is not None:
            wh.enqueue_update_scatter_lines(handle, segs, float(r), float(g), float(b))
            return

        for i, (op, args) in enumerate(self._pending_scene_ops):
            if op in ("add_lines", "update_lines") and args[0] == handle:
                self._pending_scene_ops[i] = (op, payload)
                return
        self._pending_scene_ops.append(("update_lines", payload))

    def add_box(
        self,
        bounds: tuple[float, float, float, float, float, float],
        color: tuple[float, float, float] = (1.0, 1.0, 1.0),
    ) -> int:
        """Add a world-space axis-aligned bounding box.

        ``bounds`` is ``(xmin, ymin, zmin, xmax, ymax, zmax)`` (DragonSci order).
        Returns an overlay handle.
        """
        if not hasattr(self, "_next_overlay_id"):
            self._next_overlay_id = 0
        oid = self._next_overlay_id
        self._next_overlay_id += 1
        r, g, b = color
        xmin, ymin, zmin, xmax, ymax, zmax = bounds
        # Native command order is (xmin, xmax, ymin, ymax, zmin, zmax).
        if (wh := self._live()) is not None:
            wh.enqueue_add_scatter_box(
                oid,
                float(xmin), float(xmax),
                float(ymin), float(ymax),
                float(zmin), float(zmax),
                float(r), float(g), float(b),
            )
        else:
            self._pending_scene_ops.append(("add_box", (oid, (float(xmin), float(xmax), float(ymin), float(ymax), float(zmin), float(zmax)), float(r), float(g), float(b))))
        return oid

    def remove_overlay(self, handle: int) -> None:
        """Remove a line or box overlay by its handle."""
        if (wh := self._live()) is not None:
            wh.enqueue_remove_scatter_overlay(handle)
        else:
            self._pending_scene_ops.append(("remove_overlay", (handle,)))

    def set_overlay_visibility(self, handle: int, visible: bool) -> None:
        """Show or hide a line or box overlay."""
        if (wh := self._live()) is not None:
            wh.enqueue_set_scatter_overlay_visible(handle, bool(visible))
        else:
            self._pending_scene_ops.append(("set_overlay_visibility", (handle, bool(visible))))

    def clear_overlays(self) -> None:
        """Remove all user-added line and box overlays."""
        if (wh := self._live()) is not None:
            wh.enqueue_clear_scatter_overlays()
        else:
            self._pending_scene_ops.append(("clear_overlays", ()))

    # ── Multi-actor API ──────────────────────────────────────────────────────

    def create_live_frame(
        self,
        frame: Any = None,
        *,
        capacity: int | None = None,
        x: str | None = None,
        y: str | None = None,
        z: str | None = None,
        color: Any | None = None,
        colors: Any | None = None,
        scalars: str | Any | None = None,
        point_size: float | None = None,
        point_sizes: str | Any | None = None,
        opacity: float | None = None,
        colormap: str | None = None,
        clim: tuple[float, float] | None = None,
        log_scale: bool | None = None,
        nan_color: tuple[float, float, float] | None = None,
        size_range: tuple[float, float] | None = None,
        mode: str = "primary",
        fit: bool = False,
    ) -> ScatterLiveFrame:
        """Create a retained full-frame replacement layer.

        This is the preferred path for sensors such as LiDAR that publish one
        complete current frame per tick. The default ``mode="primary"`` uses
        the primary scatter upload path without rebuilding the declarative
        widget tree. Use ``mode="actor"`` when the live frame should be an
        independent point layer alongside existing primary data.
        """
        live = ScatterLiveFrame(
            self,
            capacity=capacity,
            x=x,
            y=y,
            z=z,
            color=color,
            colors=colors,
            scalars=scalars,
            point_size=point_size,
            point_sizes=point_sizes,
            opacity=opacity,
            colormap=colormap,
            clim=clim,
            log_scale=log_scale,
            nan_color=nan_color,
            size_range=size_range,
            mode=mode,
        )
        if frame is not None:
            live.replace(frame, fit=fit)
        return live

    @staticmethod
    def _coerce_point_input(
        frame_or_positions: Any,
        x: "str | None",
        y: "str | None",
        z: "str | None",
    ) -> "tuple[Any, str, str, str]":
        """Normalize point input to (frame, x_col, y_col, z_col).

        Accepts:
          - (N, 3) or (N, 2) array-like (numpy array or list-of-lists) → dict frame
          - named frame with explicit x/y/z column names → passed through unchanged
        """
        import numpy as np
        if isinstance(frame_or_positions, np.ndarray):
            arr = frame_or_positions
        elif isinstance(frame_or_positions, (list, tuple)) and frame_or_positions:
            first = frame_or_positions[0]
            if isinstance(first, (list, tuple, np.ndarray)):
                arr = np.asarray(frame_or_positions, dtype=np.float32)
            else:
                arr = None
        else:
            arr = None

        if arr is not None:
            if arr.ndim != 2 or arr.shape[1] not in (2, 3):
                raise ValueError(
                    f"Array-like point input must be shape (N, 2) or (N, 3), got {arr.shape}"
                )
            frame = {"x": arr[:, 0], "y": arr[:, 1], "z": arr[:, 2] if arr.shape[1] == 3 else np.zeros(len(arr), dtype=np.float32)}
            return frame, "x", "y", "z"

        if x is None or y is None or z is None:
            raise ValueError(
                "x, y, and z column names are required for frame inputs; "
                "pass an (N, 2) or (N, 3) array to omit them"
            )
        return frame_or_positions, x, y, z

    def _pack_actor_payload(
        self,
        frame: Any,
        x: str,
        y: str,
        z: str,
        color: Any | None = None,
        colors: Any | None = None,
        scalars: str | Any | None = None,
        point_size: float = 4.0,
        point_sizes: str | Any | None = None,
        opacity: float = 1.0,
        colormap: str | None = None,
        clim: tuple[float, float] | None = None,
        log_scale: bool = False,
        nan_color: tuple[float, float, float] | None = None,
        size_range: tuple[float, float] | None = None,
    ) -> tuple[bytes | None, str, str]:
        cmap = _scatter_colormap(colormap or self.colormap)
        if _scatter_needs_v1(color, colors, scalars, point_sizes, opacity, nan_color, clim, log_scale):
            fmt = "point_instance_v1"
            buf = _pack_point_instances(
                frame, x, y, z,
                color=color,
                colors=colors,
                scalars=scalars,
                point_size=point_size,
                point_sizes=point_sizes,
                size_range=size_range,
                opacity=opacity,
                colormap=cmap,
                clim=clim,
                log_scale=log_scale,
                nan_color=nan_color,
            )
        else:
            fmt = "xyz_f32_v0"
            buf = _pack_xyz_bytes(frame, x, y, z)
        return buf, cmap, fmt

    def add_points(
        self,
        frame: Any,
        *,
        x: "str | None" = None,
        y: "str | None" = None,
        z: "str | None" = None,
        color: Any | None = None,
        colors: Any | None = None,
        scalars: str | Any | None = None,
        point_size: float = 4.0,
        point_sizes: str | Any | None = None,
        opacity: float = 1.0,
        colormap: str | None = None,
        clim: tuple[float, float] | None = None,
        log_scale: bool = False,
        nan_color: tuple[float, float, float] | None = None,
        size_range: tuple[float, float] | None = None,
        hover: "str | list[str] | None" = None,
    ) -> int:
        """Add an independent point actor layer. Returns an actor handle."""
        frame, x, y, z = self._coerce_point_input(frame, x, y, z)
        if not hasattr(self, "_next_actor_id"):
            self._next_actor_id: int = 1
        aid = self._next_actor_id
        self._next_actor_id += 1
        buf, cmap, fmt = self._pack_actor_payload(
            frame, x, y, z, color, colors, scalars, point_size, point_sizes,
            opacity, colormap, clim, log_scale, nan_color, size_range,
        )
        hover_meta = self._extract_hover_meta(frame, hover)
        self._actor_row_labels[aid] = self._extract_row_labels(frame)
        if buf is not None:
            if (wh := self._live()) is not None:
                wh.enqueue_add_scatter_actor_packed(aid, buf, cmap, fmt, hover_meta, x, y, z)
            else:
                import base64 as _base64
                b64 = _base64.b64encode(buf).decode("ascii")
                self._pending_scene_ops.append(("add_points", (aid, b64, cmap, fmt, hover_meta, x, y, z)))
        return aid

    def update_actor(
        self,
        handle: int,
        frame: Any,
        *,
        x: "str | None" = None,
        y: "str | None" = None,
        z: "str | None" = None,
        color: Any | None = None,
        colors: Any | None = None,
        scalars: str | Any | None = None,
        point_size: float = 4.0,
        point_sizes: str | Any | None = None,
        opacity: float = 1.0,
        colormap: str | None = None,
        clim: tuple[float, float] | None = None,
        log_scale: bool = False,
        nan_color: tuple[float, float, float] | None = None,
        size_range: tuple[float, float] | None = None,
    ) -> None:
        """Replace all points in an existing actor."""
        frame, x, y, z = self._coerce_point_input(frame, x, y, z)
        buf, cmap, fmt = self._pack_actor_payload(
            frame, x, y, z, color, colors, scalars, point_size, point_sizes,
            opacity, colormap, clim, log_scale, nan_color, size_range,
        )
        self._actor_row_labels[handle] = self._extract_row_labels(frame)
        if (wh := self._live()) is not None and buf is not None:
            wh.enqueue_update_scatter_actor_packed(handle, buf, cmap, fmt, x, y, z)
        elif buf is not None:
            import base64 as _base64
            b64 = _base64.b64encode(buf).decode("ascii")
            for i, (op, args) in enumerate(self._pending_scene_ops):
                if op == "add_points" and args[0] == handle:
                    # Clear stale hover metadata (native does the same on update).
                    self._pending_scene_ops[i] = ("add_points", (handle, b64, cmap, fmt, None, x, y, z))
                    return

    def remove_actor(self, handle: int) -> None:
        """Remove a point actor by its handle."""
        self._actor_row_labels.pop(handle, None)
        if (wh := self._live()) is not None:
            wh.enqueue_remove_scatter_actor(handle)
        else:
            self._pending_scene_ops = [
                (op, args) for op, args in self._pending_scene_ops
                if not (op in ("add_points", "add_stream") and args[0] == handle)
            ]

    def set_actor_visibility(self, handle: int, visible: bool) -> None:
        """Show or hide a point actor."""
        if (wh := self._live()) is not None:
            wh.enqueue_set_scatter_actor_visible(handle, bool(visible))
        else:
            for i, (op, args) in enumerate(self._pending_scene_ops):
                if op == "set_actor_visibility" and args[0] == handle:
                    self._pending_scene_ops[i] = ("set_actor_visibility", (handle, bool(visible)))
                    return
            self._pending_scene_ops.append(("set_actor_visibility", (handle, bool(visible))))

    def clear(self) -> None:
        """Remove all actors, labels, overlays, meshes, streams, and hover/selection state."""
        self._actor_row_labels.clear()
        self._pending_scene_ops.clear()
        self._ellipsoid_params.clear()
        self._primary_row_labels = None
        self._primary_cleared = True
        self._cached_payload = b""
        self._cached_payload_b64 = ""
        self._payload_token = 0
        if hasattr(self, "_next_actor_id"):
            self._next_actor_id = 1
        if hasattr(self, "_next_label_id"):
            self._next_label_id = 0
        if hasattr(self, "_next_overlay_id"):
            self._next_overlay_id = 0
        if hasattr(self, "_next_mesh_id"):
            self._next_mesh_id = 0
        self.hover_point = None
        self.hover_index = None
        self.hover_actor = None
        self.hover_text = None
        self.selected = []
        self.selected_indices = []
        self.selected_index_values = None
        if (wh := self._live()) is not None:
            wh.enqueue_clear_scatter_scene()

    def add_stream(
        self,
        frame_or_max_points: Any = None,
        mode: str = "ring",
        *,
        max_points: int | None = None,
        x: str | None = None,
        y: str | None = None,
        z: str | None = None,
        color: Any | None = None,
        colors: Any | None = None,
        scalars: str | Any | None = None,
        point_size: float = 4.0,
        point_sizes: str | Any | None = None,
        opacity: float = 1.0,
        colormap: str | None = None,
        clim: tuple[float, float] | None = None,
        log_scale: bool = False,
        nan_color: tuple[float, float, float] | None = None,
        size_range: tuple[float, float] | None = None,
    ) -> int:
        """Pre-allocate a fixed-capacity streaming actor. Returns a stream handle.

        Call forms:
          add_stream(500)                          # legacy: max_points as int
          add_stream(max_points=500)               # keyword
          add_stream(positions_array, max_points=500)   # (N,3) or (N,2) numpy array
          add_stream(frame, max_points=500, x=, y=, z=) # DataFrame/frame with column names

        mode: 'ring' (overwrite oldest) or 'append' (stop at capacity).
        """
        # Resolve actual_max and initial_frame from the overloaded first argument.
        if isinstance(frame_or_max_points, int):
            if max_points is not None:
                raise ValueError("add_stream: cannot pass both a positional int and max_points=")
            actual_max = frame_or_max_points
            initial_frame: Any = None
        elif frame_or_max_points is None:
            if max_points is None:
                raise ValueError("add_stream: max_points is required")
            actual_max = int(max_points)
            initial_frame = None
        else:
            # First arg is initial data (frame, numpy array, or list-of-lists).
            if max_points is None:
                raise ValueError("add_stream: max_points= is required when providing initial data")
            actual_max = int(max_points)
            initial_frame, x, y, z = self._coerce_point_input(frame_or_max_points, x, y, z)
        if mode not in ("ring", "append"):
            raise ValueError(f"Scatter3D.add_stream: mode must be 'ring' or 'append', got {mode!r}")
        if not hasattr(self, "_next_actor_id"):
            self._next_actor_id = 1
        aid = self._next_actor_id
        self._next_actor_id += 1
        # Pack initial data if provided.
        init_b64: str | None = None
        init_buf: bytes | None = None
        init_cmap: str | None = None
        init_fmt: str | None = None
        if initial_frame is not None:
            buf, init_cmap, init_fmt = self._pack_actor_payload(
                initial_frame, x, y, z, color, colors, scalars, point_size, point_sizes,
                opacity, colormap, clim, log_scale, nan_color, size_range,
            )
            if buf is not None:
                init_buf = buf
        if (wh := self._live()) is not None:
            wh.enqueue_add_scatter_stream(aid, actual_max, str(mode))
            if init_buf is not None:
                wh.enqueue_stream_scatter_actor_packed(aid, init_buf, init_cmap, init_fmt)
        else:
            if init_buf is not None:
                import base64 as _base64
                init_b64 = _base64.b64encode(init_buf).decode("ascii")
                self._pending_scene_ops.append(("add_stream", (aid, actual_max, str(mode), init_b64, init_cmap, init_fmt)))
            else:
                self._pending_scene_ops.append(("add_stream", (aid, actual_max, str(mode))))
        return aid

    def stream(
        self,
        handle: int,
        frame: Any,
        *,
        x: "str | None" = None,
        y: "str | None" = None,
        z: "str | None" = None,
        color: Any | None = None,
        colors: Any | None = None,
        scalars: str | Any | None = None,
        point_size: float = 4.0,
        point_sizes: str | Any | None = None,
        opacity: float = 1.0,
        colormap: str | None = None,
        clim: tuple[float, float] | None = None,
        log_scale: bool = False,
        nan_color: tuple[float, float, float] | None = None,
        size_range: tuple[float, float] | None = None,
    ) -> None:
        """Push new points into a stream actor.

        ``frame`` may be a named frame (with explicit ``x``/``y``/``z`` column
        names), an ``(N, 3)`` or ``(N, 2)`` numpy array, or a list-of-lists of
        the same shape.  For array inputs ``x``/``y``/``z`` are ignored.
        """
        frame, x, y, z = self._coerce_point_input(frame, x, y, z)
        buf, cmap, fmt = self._pack_actor_payload(
            frame, x, y, z, color, colors, scalars, point_size, point_sizes,
            opacity, colormap, clim, log_scale, nan_color, size_range,
        )
        if (wh := self._live()) is not None and buf is not None:
            wh.enqueue_stream_scatter_actor_packed(handle, buf, cmap, fmt)

    def clear_stream(self, handle: int) -> None:
        """Reset a stream actor to empty without deallocating its buffer."""
        if (wh := self._live()) is not None:
            wh.enqueue_clear_scatter_stream(handle)

    # ── Phase 5: LOD and Picking ──────────────────────────────────────────────

    def set_lod(
        self,
        enabled: bool = True,
        threshold: int = 200_000,
        factor: int = 8,
    ) -> None:
        """Configure Level-of-Detail during camera interaction.

        When enabled and point_count > threshold, only point_count/factor
        points are drawn while orbiting/panning. Full density is restored on
        release. LOD never changes colors or point sizes.
        """
        self._lod_enabled = bool(enabled)
        self._lod_threshold = max(0, int(threshold))
        self._lod_factor = max(1, int(factor))
        if (wh := self._live()) is not None:
            wh.enqueue_set_scatter_lod(self._lod_enabled, self._lod_threshold, self._lod_factor)

    def set_auto_point_size(self, enabled: bool = True) -> None:
        """Enable or disable automatic point-size shrinking for dense views.

        When enabled, native may shrink rendered point sprites in dense scatter
        views to reduce overdraw. Disable this when exact point size encodes a
        data variable and should remain visually fixed.
        """
        self.auto_point_size = bool(enabled)
        if (wh := self._live()) is not None:
            wh.enqueue_set_scatter_auto_point_size(self.auto_point_size)

    def set_interactive_render_scale(self, scale: float) -> None:
        """Set lower-resolution scatter rendering used during orbit/pan interaction.

        ``1.0`` keeps full native resolution. Values below ``1.0`` reduce scatter
        scene fill cost while interacting, then return to full resolution when
        interaction stops. UI text and normal widgets still render full-res.
        """
        self._interactive_render_scale = max(0.25, min(1.0, float(scale)))
        if (wh := self._live()) is not None:
            wh.enqueue_set_scatter_interactive_render_scale(self._interactive_render_scale)

    def set_auto_quality(self, enabled: bool = True, target_fps: float | None = None) -> None:
        """Enable native interaction quality budgeting for dense scatter scenes.

        When enabled, native may temporarily lower interaction render scale
        while orbiting or panning if recent frame time exceeds the target.
        Quality resets when interaction stops.
        """
        self._auto_quality = bool(enabled)
        if target_fps is not None:
            self._quality_target_fps = max(1.0, float(target_fps))
        if (wh := self._live()) is not None:
            wh.enqueue_set_scatter_auto_quality(self._auto_quality, self._quality_target_fps)

    def enable_point_picking(self, on_pick=None) -> None:
        """Switch to point-picking mode (default). Left click picks the nearest point."""
        self._picking_mode = "point"
        if on_pick is not None:
            self.on_pick = on_pick
        if (wh := self._live()) is not None:
            wh.enqueue_set_scatter_picking_mode("point")

    def enable_rectangle_picking(self, on_select=None) -> None:
        """Switch to rectangle-selection mode. Left drag draws a selection rect;
        release emits a JSON payload to the on_select callback."""
        self._picking_mode = "rectangle"
        if on_select is not None:
            self._on_select = on_select
        if (wh := self._live()) is not None:
            wh.enqueue_set_scatter_picking_mode("rectangle")

    def enable_lasso_picking(self, on_select=None) -> None:
        """Switch to freehand lasso-selection mode. Left drag draws a polygon path;
        release performs point-in-polygon selection and emits a JSON payload to on_select."""
        self._picking_mode = "lasso"
        if on_select is not None:
            self._on_select = on_select
        if (wh := self._live()) is not None:
            wh.enqueue_set_scatter_picking_mode("lasso")

    def disable_picking(self) -> None:
        """Disable all picking and selection interaction."""
        self._picking_mode = "none"
        if (wh := self._live()) is not None:
            wh.enqueue_set_scatter_picking_mode("none")

    # ── Phase 6: Mesh and Statistical Overlays ────────────────────────────────

    @staticmethod
    def _pack_mesh_payload(positions, triangle_indices):
        """Encode positions (N×3 float32) and triangle_indices (M×3 uint32) to base64."""
        import struct
        import base64
        pos_flat = [float(v) for p in positions for v in p]
        pos_bytes = struct.pack(f"<{len(pos_flat)}f", *pos_flat)
        idx_flat = [int(v) for t in triangle_indices for v in t]
        idx_bytes = struct.pack(f"<{len(idx_flat)}I", *idx_flat)
        return (
            base64.b64encode(pos_bytes).decode(),
            base64.b64encode(idx_bytes).decode(),
        )

    def add_convex_hull(
        self,
        points,
        color=(1.0, 1.0, 1.0),
        opacity: float = 0.3,
        wireframe: bool = False,
    ) -> int:
        """Compute a convex hull of `points` (array-like N×3) and add it as a mesh overlay.

        Requires scipy (`pip install scipy`). Returns a handle for removal/update.
        """
        try:
            import numpy as np
            from scipy.spatial import ConvexHull
        except ImportError as e:
            raise ImportError("add_convex_hull requires scipy: pip install scipy") from e

        pts = np.asarray(points, dtype=np.float32)
        if pts.ndim != 2 or pts.shape[1] != 3:
            raise ValueError("points must be N×3")
        hull = ConvexHull(pts)
        verts = pts[hull.vertices]
        old_to_new = {old: new for new, old in enumerate(hull.vertices)}
        tris = [[old_to_new[i] for i in simplex] for simplex in hull.simplices]

        if not hasattr(self, "_next_mesh_id"):
            self._next_mesh_id: int = 0
        mid = self._next_mesh_id
        self._next_mesh_id += 1

        r, g, b = self._normalize_color(color)
        a = float(opacity)
        pos_b64, idx_b64 = self._pack_mesh_payload(verts.tolist(), tris)
        if (wh := self._live()) is not None:
            wh.enqueue_add_scatter_mesh(mid, pos_b64, idx_b64, r, g, b, a, wireframe)
        else:
            self._pending_scene_ops.append(("add_mesh", (mid, pos_b64, idx_b64, r, g, b, a, wireframe)))
        return mid

    def update_convex_hull(
        self,
        handle: int,
        points=None,
        color=None,
        opacity: float | None = None,
        wireframe: bool | None = None,
    ) -> None:
        """Update an existing convex hull mesh overlay by handle.

        Supply new `points` to recompute the hull geometry; omit to leave geometry unchanged.
        Only provided keyword arguments are updated.
        """
        pos_b64: str | None = None
        idx_b64: str | None = None
        if points is not None:
            try:
                import numpy as np
                from scipy.spatial import ConvexHull
            except ImportError as e:
                raise ImportError("update_convex_hull requires scipy: pip install scipy") from e
            pts = np.asarray(points, dtype=np.float32)
            if pts.ndim != 2 or pts.shape[1] != 3:
                raise ValueError("points must be N×3")
            hull = ConvexHull(pts)
            verts = pts[hull.vertices]
            old_to_new = {old: new for new, old in enumerate(hull.vertices)}
            tris = [[old_to_new[i] for i in simplex] for simplex in hull.simplices]
            pos_b64, idx_b64 = self._pack_mesh_payload(verts.tolist(), tris)
        if color is not None:
            r, g, b = self._normalize_color(color)
        else:
            r = g = b = None
        a = float(opacity) if opacity is not None else None
        if (wh := self._live()) is not None:
            wh.enqueue_update_scatter_mesh(handle, pos_b64, idx_b64, r, g, b, a, wireframe)

    def add_ellipsoid(
        self,
        center,
        covariance,
        color=(1.0, 1.0, 1.0),
        opacity: float = 0.3,
        n_std: float = 2.0,
        wireframe: bool = False,
        u_res: int = 20,
        v_res: int = 20,
    ) -> int:
        """Add an ellipsoid mesh overlay defined by `center` (3,) and `covariance` (3×3).

        Requires numpy. Returns a handle for removal/update.
        """
        import numpy as np

        ctr = np.asarray(center, dtype=np.float64)
        cov = np.asarray(covariance, dtype=np.float64)
        eigvals, eigvecs = np.linalg.eigh(cov)
        eigvals = np.maximum(eigvals, 0.0)
        radii = n_std * np.sqrt(eigvals)

        u = np.linspace(0, 2 * np.pi, u_res, endpoint=False)
        v = np.linspace(0, np.pi, v_res)
        uu, vv = np.meshgrid(u, v)
        xs = np.cos(uu) * np.sin(vv)
        ys = np.sin(uu) * np.sin(vv)
        zs = np.cos(vv)
        sphere = np.stack([xs.ravel(), ys.ravel(), zs.ravel()], axis=1)
        verts = (sphere * radii) @ eigvecs.T + ctr
        verts = verts.astype(np.float32)

        rows, cols = v_res, u_res
        tris = []
        for i in range(rows - 1):
            for j in range(cols):
                a = i * cols + j
                b = i * cols + (j + 1) % cols
                c = (i + 1) * cols + j
                d = (i + 1) * cols + (j + 1) % cols
                tris.append([a, b, c])
                tris.append([b, d, c])

        if not hasattr(self, "_next_mesh_id"):
            self._next_mesh_id = 0
        mid = self._next_mesh_id
        self._next_mesh_id += 1

        r, g, b_c = self._normalize_color(color)
        a = float(opacity)
        pos_b64, idx_b64 = self._pack_mesh_payload(verts.tolist(), tris)
        self._ellipsoid_params[mid] = {
            "center": ctr.tolist(), "covariance": cov.tolist(),
            "n_std": n_std, "u_res": u_res, "v_res": v_res,
        }
        if (wh := self._live()) is not None:
            wh.enqueue_add_scatter_mesh(mid, pos_b64, idx_b64, r, g, b_c, a, wireframe)
        else:
            self._pending_scene_ops.append(("add_mesh", (mid, pos_b64, idx_b64, r, g, b_c, a, wireframe)))
        return mid

    def update_ellipsoid(
        self,
        handle: int,
        center=None,
        covariance=None,
        color=None,
        opacity: float | None = None,
        n_std: float | None = None,
        wireframe: bool | None = None,
        u_res: int | None = None,
        v_res: int | None = None,
    ) -> None:
        """Update an existing ellipsoid mesh overlay by handle.

        Any subset of parameters may be changed. Geometry is recomputed when at least one
        of center, covariance, or n_std changes; stored params from add_ellipsoid are used
        for any omitted geometric parameters.
        """
        import numpy as np
        pos_b64: str | None = None
        idx_b64: str | None = None
        needs_geo = center is not None or covariance is not None or n_std is not None
        if needs_geo:
            stored = self._ellipsoid_params.get(handle, {})
            ctr = np.asarray(center if center is not None else stored.get("center", [0, 0, 0]), dtype=np.float64)
            cov = np.asarray(covariance if covariance is not None else stored.get("covariance", [[1,0,0],[0,1,0],[0,0,1]]), dtype=np.float64)
            _n_std = n_std if n_std is not None else stored.get("n_std", 2.0)
            _u_res = u_res if u_res is not None else stored.get("u_res", 20)
            _v_res = v_res if v_res is not None else stored.get("v_res", 20)
            eigvals, eigvecs = np.linalg.eigh(cov)
            eigvals = np.maximum(eigvals, 0.0)
            radii = _n_std * np.sqrt(eigvals)
            u = np.linspace(0, 2 * np.pi, _u_res, endpoint=False)
            v = np.linspace(0, np.pi, _v_res)
            uu, vv = np.meshgrid(u, v)
            xs = np.cos(uu) * np.sin(vv)
            ys = np.sin(uu) * np.sin(vv)
            zs = np.cos(vv)
            sphere = np.stack([xs.ravel(), ys.ravel(), zs.ravel()], axis=1)
            verts = ((sphere * radii) @ eigvecs.T + ctr).astype(np.float32)
            rows, cols = _v_res, _u_res
            tris = []
            for i in range(rows - 1):
                for j in range(cols):
                    a_i = i * cols + j
                    b_i = i * cols + (j + 1) % cols
                    c_i = (i + 1) * cols + j
                    d_i = (i + 1) * cols + (j + 1) % cols
                    tris.append([a_i, b_i, c_i])
                    tris.append([b_i, d_i, c_i])
            pos_b64, idx_b64 = self._pack_mesh_payload(verts.tolist(), tris)
            self._ellipsoid_params[handle] = {
                "center": ctr.tolist(), "covariance": cov.tolist(),
                "n_std": _n_std, "u_res": _u_res, "v_res": _v_res,
            }
        if color is not None:
            r, g, b = self._normalize_color(color)
        else:
            r = g = b = None
        a = float(opacity) if opacity is not None else None
        if (wh := self._live()) is not None:
            wh.enqueue_update_scatter_mesh(handle, pos_b64, idx_b64, r, g, b, a, wireframe)

    @staticmethod
    def _extract_row_labels(frame: Any) -> "list | None":
        """Return non-trivial dataframe index labels, or None for positional/unknown frames."""
        try:
            index = list(frame.index)
            if index == list(range(len(index))):
                return None  # trivial 0-based integer index
            return [str(i) for i in index]
        except AttributeError:
            return None

    @staticmethod
    def _extract_hover_columns(
        frame: Any,
        hover: "str | list[str] | None",
    ) -> "tuple[str, list[object]] | None":
        if hover is None or frame is None:
            return None
        try:
            import numpy as _np
        except ImportError:
            return None

        cols = [hover] if isinstance(hover, str) else list(hover)
        metadata: list[dict[str, object]] = []
        buffers: list[object] = []
        expected_len: int | None = None
        for col in cols:
            try:
                vals = frame[col] if hasattr(frame, "__getitem__") else getattr(frame, col)
            except (KeyError, AttributeError) as exc:
                raise ValueError(f"Scatter3D: hover column {col!r} not found in frame") from exc
            arr = _np.asarray(vals)
            if arr.ndim != 1:
                return None
            row_count = int(arr.shape[0])
            if expected_len is None:
                expected_len = row_count
            elif row_count != expected_len:
                raise ValueError("Scatter3D: hover columns must have the same length")

            kind = arr.dtype.kind
            if kind == "f":
                dtype = "f32" if arr.dtype.itemsize <= 4 else "f64"
                target = _np.float32 if dtype == "f32" else _np.float64
                packed = _np.ascontiguousarray(arr.astype(target, copy=False))
                buffer: object = memoryview(packed).cast("B")
            elif kind == "i":
                dtype = "i64"
                packed = _np.ascontiguousarray(arr.astype(_np.int64, copy=False))
                buffer = memoryview(packed).cast("B")
            elif kind == "u":
                dtype = "u64"
                packed = _np.ascontiguousarray(arr.astype(_np.uint64, copy=False))
                buffer = memoryview(packed).cast("B")
            elif kind == "b":
                dtype = "bool"
                packed = _np.ascontiguousarray(arr.astype(_np.uint8, copy=False))
                buffer = memoryview(packed).cast("B")
            elif kind in "OSU":
                dtype = "utf8"
                strings = arr.astype(str, copy=False).tolist()
                joined = "\0".join(strings)
                if joined.count("\0") != max(row_count - 1, 0):
                    return None
                buffer = joined.encode("utf-8")
            else:
                return None
            metadata.append({"name": str(col), "dtype": dtype, "len": row_count})
            buffers.append(buffer)

        if not metadata:
            return None
        return json.dumps(metadata, separators=(",", ":")), buffers

    @staticmethod
    def _extract_hover_meta(frame: Any, hover: "str | list[str] | None") -> "str | None":
        """Extract per-point hover lines from frame column(s). Returns encoded lines or None.

        Each encoded element is one multi-line tooltip suffix string
        for the corresponding point, formatted as "col: value" (one line per column).
        Native prepends the coordinates line, so the final tooltip reads:
            (x.xxx, y.yyy, z.zzz)
            col: value
        Raises ValueError if a requested column cannot be found in frame.
        """
        import json as _json
        if hover is None or frame is None:
            return None
        def _fmt(v: object) -> str:
            try:
                return f"{float(v):.4g}"  # type: ignore[arg-type]
            except (TypeError, ValueError):
                return str(v)

        cols = [hover] if isinstance(hover, str) else list(hover)
        raw_cols: list[tuple[str, Any]] = []
        for col in cols:
            try:
                vals = frame[col] if hasattr(frame, "__getitem__") else getattr(frame, col)
                raw_cols.append((col, vals))
            except (KeyError, AttributeError) as exc:
                raise ValueError(
                    f"Scatter3D: hover column {col!r} not found in frame"
                ) from exc
        if not raw_cols:
            return None

        try:
            import numpy as _np

            formatted_arrays = []
            expected_len: int | None = None
            for col, vals in raw_cols:
                arr = _np.asarray(vals)
                if arr.ndim != 1:
                    raise TypeError
                if expected_len is None:
                    expected_len = int(arr.shape[0])
                elif int(arr.shape[0]) != expected_len:
                    raise ValueError("hover columns must have the same length")
                if arr.dtype.kind in "biuf":
                    formatted = _np.char.mod("%.4g", arr.astype(_np.float64, copy=False))
                elif arr.dtype.kind == "c":
                    raise TypeError
                elif arr.dtype.kind == "?":
                    formatted = _np.char.mod("%.4g", arr.astype(_np.float64, copy=False))
                elif arr.dtype.kind in "SU":
                    formatted = arr.astype(str, copy=False)
                else:
                    raise TypeError
                formatted_arrays.append((col, formatted))

            lines = None
            for col, values in formatted_arrays:
                part = _np.char.add(f"{col}: ", values)
                lines = part if lines is None else _np.char.add(_np.char.add(lines, "\n"), part)
            if lines is not None:
                row_lines = lines.tolist()
                if not any("\0" in line for line in row_lines):
                    return "\0" + "\0".join(row_lines)
                return _json.dumps(row_lines)
        except ValueError:
            raise
        except Exception:
            pass

        extracted: list[tuple[str, list[str]]] = []
        for col, vals in raw_cols:
            extracted.append((col, [_fmt(v) for v in vals]))
        if not extracted:
            return None
        n = len(extracted[0][1])
        rows = [
            "\n".join(f"{col}: {col_vals[i]}" for col, col_vals in extracted)
            for i in range(n)
        ]
        if not any("\0" in row for row in rows):
            return "\0" + "\0".join(rows)
        return _json.dumps(rows)

    @staticmethod
    def _normalize_color(color) -> tuple[float, float, float]:
        """Convert color to (r, g, b) floats in [0, 1].

        Accepts: CSS hex string (#rrggbb / #rgb), named CSS colors, or a sequence of 3 floats.
        """
        if isinstance(color, str):
            c = color.strip()
            if c.startswith("#"):
                c = c.lstrip("#")
                if len(c) == 3:
                    c = "".join(ch * 2 for ch in c)
                if len(c) == 6:
                    r = int(c[0:2], 16) / 255.0
                    g = int(c[2:4], 16) / 255.0
                    b = int(c[4:6], 16) / 255.0
                    return (r, g, b)
            _CSS: dict[str, tuple[float, float, float]] = {
                "red": (1, 0, 0), "green": (0, 0.502, 0), "blue": (0, 0, 1),
                "white": (1, 1, 1), "black": (0, 0, 0), "yellow": (1, 1, 0),
                "cyan": (0, 1, 1), "magenta": (1, 0, 1), "orange": (1, 0.647, 0),
                "purple": (0.502, 0, 0.502), "gray": (0.502, 0.502, 0.502),
                "grey": (0.502, 0.502, 0.502),
            }
            if c.lower() in _CSS:
                return _CSS[c.lower()]
            raise ValueError(f"Unrecognized color string: {color!r}")
        return (float(color[0]), float(color[1]), float(color[2]))

    @staticmethod
    def _sample_colormap(colormap: str, t: float) -> tuple[float, float, float]:
        """Sample a named colormap at t in [0, 1], returning (r, g, b)."""
        from .colormap import _TABLES  # type: ignore
        table = _TABLES.get(colormap, _TABLES.get("viridis"))
        if table is None:
            return (1.0, 1.0, 1.0)
        n = len(table)
        if n == 1:
            return (table[0][0], table[0][1], table[0][2])
        idx = t * (n - 1)
        lo = int(idx)
        hi = min(lo + 1, n - 1)
        frac = idx - lo
        r = table[lo][0] + frac * (table[hi][0] - table[lo][0])
        g = table[lo][1] + frac * (table[hi][1] - table[lo][1])
        b = table[lo][2] + frac * (table[hi][2] - table[lo][2])
        return (r, g, b)

    def add_cluster_hulls(
        self,
        positions,
        labels,
        colormap: str = "viridis",
        opacity: float = 0.25,
    ) -> list[int]:
        """Add a convex hull per unique label. Returns list of handles."""
        try:
            import numpy as np
            from scipy.spatial import ConvexHull  # noqa: F401
        except ImportError as e:
            raise ImportError("add_cluster_hulls requires scipy") from e

        pts = np.asarray(positions, dtype=np.float32)
        lbls = np.asarray(labels)
        unique = np.unique(lbls)
        handles = []
        for i, lbl in enumerate(unique):
            mask = lbls == lbl
            subset = pts[mask]
            if len(subset) < 4:
                continue
            t = float(i) / max(len(unique) - 1, 1)
            color = self._sample_colormap(colormap, t)
            h = self.add_convex_hull(subset, color=color, opacity=opacity)
            handles.append(h)
        return handles

    def add_cluster_ellipsoids(
        self,
        positions,
        labels,
        colormap: str = "viridis",
        opacity: float = 0.25,
        n_std: float = 2.0,
    ) -> list[int]:
        """Add an ellipsoid per unique label. Returns list of handles."""
        import numpy as np

        pts = np.asarray(positions, dtype=np.float64)
        lbls = np.asarray(labels)
        unique = np.unique(lbls)
        handles = []
        for i, lbl in enumerate(unique):
            mask = lbls == lbl
            subset = pts[mask]
            if len(subset) < 4:
                continue
            ctr = subset.mean(axis=0)
            cov = np.cov(subset.T)
            t = float(i) / max(len(unique) - 1, 1)
            color = self._sample_colormap(colormap, t)
            h = self.add_ellipsoid(ctr, cov, color=color, opacity=opacity, n_std=n_std)
            handles.append(h)
        return handles

    def remove_mesh(self, handle: int) -> None:
        """Remove a mesh overlay by handle."""
        if (wh := self._live()) is not None:
            wh.enqueue_remove_scatter_mesh(handle)
        else:
            self._pending_scene_ops.append(("remove_mesh", (handle,)))

    def set_mesh_visibility(self, handle: int, visible: bool) -> None:
        if (wh := self._live()) is not None:
            wh.enqueue_set_scatter_mesh_visible(handle, bool(visible))
        else:
            self._pending_scene_ops.append(("set_mesh_visibility", (handle, bool(visible))))

    def clear_meshes(self) -> None:
        """Remove all mesh overlays."""
        if not hasattr(self, "_next_mesh_id"):
            self._next_mesh_id = 0
            return
        if (wh := self._live()) is not None:
            wh.enqueue_clear_scatter_meshes()
        else:
            # Drop any pending mesh ops and reset state so startup starts clean.
            self._pending_scene_ops = [
                op for op in self._pending_scene_ops
                if op[0] not in ("add_mesh", "remove_mesh", "set_mesh_visibility")
            ]
            self._ellipsoid_params.clear()
            self._next_mesh_id = 0
            self._pending_scene_ops.append(("clear_meshes", ()))

    def reset_camera(self) -> None:
        if (handle := self._live()) is not None:
            handle.enqueue_reset_scatter_camera()

    def view_xy(self) -> None:
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_view_direction("xy")

    def view_xz(self) -> None:
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_view_direction("xz")

    def view_yz(self) -> None:
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_view_direction("yz")

    def view_isometric(self) -> None:
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_view_direction("isometric")

    def fit(self, bounds: tuple[float, float, float, float, float, float] | None = None) -> None:
        """Fit the camera to data bounds.

        bounds: optional (x_min, y_min, z_min, x_max, y_max, z_max). When None
        the camera refits to the current uploaded point cloud bounds.
        """
        if (handle := self._live()) is not None:
            b = list(bounds) if bounds is not None else None
            handle.enqueue_fit_scatter_camera(b)

    def set_point_style(self, style: str) -> None:
        """Set point rendering style: 'circle', 'square', or 'gaussian'."""
        valid = {"circle", "square", "gaussian"}
        s = style.strip().lower()
        if s not in valid:
            raise ValueError(f"unknown point style {style!r}; expected one of: {', '.join(sorted(valid))}")
        self._point_style = s
        current = dict(self.style or {})
        current["scatter_point_style"] = s
        self.style = _copy_style(current, widget_kind=self.kind)
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_point_style(s)

    def set_point_size(self, size: float) -> None:
        """Set the renderer point-size override without repacking point data."""
        value = max(0.0, float(size))
        self.point_size = value
        current = dict(self.style or {})
        current["scatter_point_size"] = value
        self.style = _copy_style(current, widget_kind=self.kind)
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_point_size(value)

    @property
    def point_size_override(self) -> float:
        """Renderer point-size override in pixels."""
        return float(self.point_size)

    @point_size_override.setter
    def point_size_override(self, size: float) -> None:
        self.set_point_size(size)

    @property
    def point_style(self) -> str:
        """Point rendering style: 'circle', 'square', or 'gaussian'."""
        return getattr(self, "_point_style", "circle")

    @point_style.setter
    def point_style(self, style: str) -> None:
        self.set_point_style(style)

    def set_camera(self, state: dict) -> None:
        """Apply a camera state dict (as returned by get_camera())."""
        if (handle := self._live()) is not None:
            target = list(state.get("target", [0.0, 0.0, 0.0]))
            handle.enqueue_set_scatter_camera_state(
                target=target,
                distance=float(state.get("distance", 5.0)),
                yaw=float(state.get("yaw", 0.4)),
                pitch=float(state.get("pitch", 0.4)),
                parallel=bool(state.get("parallel", False)),
            )
            self._propagate_camera()

    def get_camera(self) -> dict | None:
        """Return the current camera state dict, or None if not live.

        Reads from the synchronous debug snapshot. Keys: target, distance,
        yaw, pitch, parallel.
        """
        if (handle := self._live()) is not None:
            try:
                snapshot = handle.app.debug_snapshot()
                scatters = snapshot.get("gpu", {}).get("resources", {}).get("scatters", {})
                cam = scatters.get(self.id, {}).get("camera")
                if cam is not None:
                    return dict(cam)
            except Exception:
                pass
        return None

    @property
    def parallel_projection(self) -> bool:
        return self._parallel_projection

    @parallel_projection.setter
    def parallel_projection(self, value: bool) -> None:
        self._parallel_projection = bool(value)
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_parallel_projection(self._parallel_projection)

    # ── Phase 7: Camera helpers, export, and camera linking ───────────────────

    # Plane → (yaw, pitch) in radians
    _FLATTEN_PLANES: dict[str, tuple[float, float]] = {
        "xy":  (3.14159265, 1.5707963),
        "xy-": (0.0,       -1.5707963),
        "xz":  (0.0,        0.0),
        "xz-": (3.14159265, 0.0),
        "yz":  (-1.5707963, 0.0),
        "yz-": ( 1.5707963, 0.0),
    }

    def flatten_view(self, plane: str = "xy") -> None:
        """Snap to an axis-aligned orthographic view.

        plane: "xy" | "xy-" | "xz" | "xz-" | "yz" | "yz-"
        """
        if plane not in self._FLATTEN_PLANES:
            raise ValueError(f"unknown plane {plane!r}; expected one of: {', '.join(self._FLATTEN_PLANES)}")
        yaw, pitch = self._FLATTEN_PLANES[plane]
        state = self.get_camera() or {}
        state["yaw"] = yaw
        state["pitch"] = pitch
        state["parallel"] = True
        self._parallel_projection = True
        self.set_camera(state)

    def get_view_bounds_2d(self) -> list[float] | None:
        """Return [x_min, y_min, x_max, y_max] for the current orthographic view.

        Computed from the camera state snapshot. Only meaningful when
        parallel_projection is True. Returns None when not live.
        """
        import math
        cam = self.get_camera()
        if cam is None:
            return None
        dist = float(cam.get("distance", 5.0))
        fov_y = math.radians(45.0)
        # aspect derived from widget dimensions (fall back to 1.0)
        aspect = 1.0
        if (handle := self._live()) is not None:
            try:
                snap = handle.app.debug_snapshot()
                sc = snap.get("gpu", {}).get("resources", {}).get("scatters", {})
                dims = sc.get(self.id, {}).get("dimensions")
                if dims and dims.get("width") and dims.get("height"):
                    aspect = dims["width"] / max(dims["height"], 1)
            except Exception:
                pass
        half_h = dist * math.tan(fov_y / 2.0)
        half_w = half_h * aspect
        tx, ty = float(cam.get("target", [0, 0, 0])[0]), float(cam.get("target", [0, 0, 0])[1])
        return [tx - half_w, ty - half_h, tx + half_w, ty + half_h]

    def set_parallel_scale(self, half_w: float, half_h: float) -> None:
        """Set explicit orthographic half-extents (overrides distance/fov calculation)."""
        if (handle := self._live()) is not None:
            handle.enqueue_set_scatter_parallel_scale(float(half_w), float(half_h))

    def link_cameras(self, *others: "Scatter3D") -> None:
        """Propagate camera changes from this widget to `others` (and vice versa)."""
        for other in others:
            if not hasattr(other, "_camera_links"):
                other._camera_links = set()
                other._propagating = False
            self._camera_links.add(other)
            other._camera_links.add(self)

    def unlink_cameras(self, *others: "Scatter3D") -> None:
        """Remove camera links between this widget and `others`."""
        other_set = set(others)
        if hasattr(self, "_camera_links"):
            self._camera_links -= other_set
        for other in others:
            if hasattr(other, "_camera_links"):
                other._camera_links.discard(self)

    def _propagate_camera(self) -> None:
        """Broadcast current camera state to all linked widgets."""
        if getattr(self, "_propagating", False):
            return
        links = getattr(self, "_camera_links", set())
        if not links:
            return
        state = self.get_camera()
        if state is None:
            return
        dead = set()
        for other in links:
            try:
                other._receive_camera(state)
            except Exception:
                dead.add(other)
        links -= dead

    def _receive_camera(self, state: dict) -> None:
        """Apply a camera state dict from a linked widget without re-broadcasting."""
        self._propagating = True
        try:
            self.set_camera(state)
        finally:
            self._propagating = False

    def _queue_startup_resources(self) -> None:
        """Replay scene operations that were queued before the widget went live."""
        import base64 as _base64
        handle = self._live()
        if handle is None:
            return
        total_t0 = time.perf_counter()
        timings: dict[str, Any] = {}

        def record_phase(name: str, start: float) -> None:
            timings[name] = (time.perf_counter() - start) * 1000.0

        can_send_primary = handle.app._native_method_available("enqueue_set_scatter_points_packed")
        phase_t0 = time.perf_counter()
        if not self._primary_cleared and can_send_primary:
            payload = self._cached_payload
            pack_ms = 0.0
            if payload is None:
                t0 = time.perf_counter()
                payload = self._build_payload()
                pack_ms = (time.perf_counter() - t0) * 1000.0
                self._cached_payload = payload
                self._cached_payload_b64 = None
                self._payload_token = (
                    zlib.crc32(payload) if payload is not None and len(payload) > 0 else 0
                )
                self._compute_auto_color_meta()
            if payload is not None:
                enqueue_t0 = time.perf_counter()
                handle.enqueue_set_scatter_points_packed(
                    payload,
                    pack_ms=pack_ms,
                    enqueue_epoch_ms=time.time() * 1000.0,
                    colormap=self.colormap,
                    payload_format=self.data_format,
                    coalesce=False,
                )
                record_phase("enqueue_points_ms", enqueue_t0)
                timings["payload_bytes"] = len(payload)
            timings["build_payload_ms"] = pack_ms
        record_phase("primary_points_ms", phase_t0)
        # Always sync hover_tooltip to native on startup (default is True on both sides,
        # but a user may have set it to False before the widget went live).
        phase_t0 = time.perf_counter()
        handle.enqueue_set_scatter_hover_tooltip(self._hover_tooltip)
        record_phase("hover_tooltip_ms", phase_t0)
        if not self._primary_cleared:
            # Sync column names so native tooltip shows the right axis labels.
            phase_t0 = time.perf_counter()
            handle.enqueue_set_scatter_tooltip_axis_labels(self.x, self.y, self.z)
            record_phase("tooltip_axis_labels_ms", phase_t0)
            phase_t0 = time.perf_counter()
            # Sync primary hover metadata (column names → per-point strings).
            columns_payload = self._primary_hover_columns_payload(handle)
            record_phase("hover_columns_extract_ms", phase_t0)
            if columns_payload is not None:
                columns_json, buffers = columns_payload
                timings["hover_columns_bytes"] = sum(
                    int(getattr(buffer, "nbytes", len(buffer))) for buffer in buffers
                )
                timings["hover_columns_count"] = len(buffers)
                phase_t0 = time.perf_counter()
                handle.enqueue_set_scatter_primary_hover_columns(columns_json, buffers)
                record_phase("hover_columns_enqueue_ms", phase_t0)
            else:
                phase_t0 = time.perf_counter()
                meta = self._extract_hover_meta(self.frame, self._hover)
                record_phase("hover_meta_extract_ms", phase_t0)
                if meta is not None:
                    timings["hover_meta_bytes"] = len(meta)
                    phase_t0 = time.perf_counter()
                    handle.enqueue_set_scatter_primary_hover_meta(meta)
                    record_phase("hover_meta_enqueue_ms", phase_t0)
        # Sync LOD config (may have been changed before going live).
        phase_t0 = time.perf_counter()
        handle.enqueue_set_scatter_lod(self._lod_enabled, self._lod_threshold, self._lod_factor)
        handle.enqueue_set_scatter_auto_point_size(self.auto_point_size)
        handle.enqueue_set_scatter_interactive_render_scale(self._interactive_render_scale)
        handle.enqueue_set_scatter_auto_quality(self._auto_quality, self._quality_target_fps)
        # Sync picking mode.
        handle.enqueue_set_scatter_picking_mode(self._picking_mode)
        record_phase("scatter_options_ms", phase_t0)
        # Sync point style override if set before going live.
        if hasattr(self, "_point_style"):
            phase_t0 = time.perf_counter()
            handle.enqueue_set_scatter_point_style(self._point_style)
            record_phase("point_style_ms", phase_t0)
        timings["total_ms"] = (time.perf_counter() - total_t0) * 1000.0
        self._last_startup_resource_timings = timings
        if not self._pending_scene_ops:
            return
        scene_t0 = time.perf_counter()
        for op, args in self._pending_scene_ops:
            if op == "add_label":
                lid, px, py, pz, text, r, g, b, size, anchor = args
                handle.enqueue_add_scatter_label(lid, px, py, pz, text, r, g, b, size, anchor)
            elif op == "update_label":
                lid, px, py, pz, text, r, g, b, size, anchor = args
                handle.enqueue_update_scatter_label(lid, px, py, pz, text, r, g, b, size, anchor)
            elif op == "remove_label":
                (lid,) = args
                handle.enqueue_remove_scatter_label(lid)
            elif op == "set_label_visibility":
                lid, visible = args
                handle.enqueue_set_scatter_label_visible(lid, visible)
            elif op == "clear_labels":
                handle.enqueue_clear_scatter_labels()
            elif op == "add_lines":
                oid, pts, r, g, b = args
                handle.enqueue_add_scatter_lines(oid, pts, r, g, b)
            elif op == "update_lines":
                oid, pts, r, g, b = args
                handle.enqueue_update_scatter_lines(oid, pts, r, g, b)
            elif op == "add_box":
                oid, bounds, r, g, b = args
                xmin, xmax, ymin, ymax, zmin, zmax = bounds
                handle.enqueue_add_scatter_box(oid, xmin, xmax, ymin, ymax, zmin, zmax, r, g, b)
            elif op == "remove_overlay":
                (oid,) = args
                handle.enqueue_remove_scatter_overlay(oid)
            elif op == "set_overlay_visibility":
                oid, visible = args
                handle.enqueue_set_scatter_overlay_visible(oid, visible)
            elif op == "clear_overlays":
                handle.enqueue_clear_scatter_overlays()
            elif op == "add_points":
                aid, b64, cmap, fmt, *rest = args
                hover_meta = rest[0] if len(rest) > 0 else None
                tx = rest[1] if len(rest) > 1 else None
                ty = rest[2] if len(rest) > 2 else None
                tz = rest[3] if len(rest) > 3 else None
                handle.enqueue_add_scatter_actor(aid, b64, cmap, fmt, hover_meta, tx, ty, tz)
            elif op == "add_stream":
                aid, max_pts, mode_s = args[0], args[1], args[2]
                handle.enqueue_add_scatter_stream(aid, max_pts, mode_s)
                if len(args) == 6:
                    init_b64, init_cmap, init_fmt = args[3], args[4], args[5]
                    handle.enqueue_stream_scatter_actor(aid, init_b64, init_cmap, init_fmt)
            elif op == "add_mesh":
                mid, pos_b64, idx_b64, r, g, b, a, wireframe = args
                handle.enqueue_add_scatter_mesh(mid, pos_b64, idx_b64, r, g, b, a, wireframe)
            elif op == "remove_mesh":
                (mid,) = args
                handle.enqueue_remove_scatter_mesh(mid)
            elif op == "set_mesh_visibility":
                mid, visible = args
                handle.enqueue_set_scatter_mesh_visible(mid, visible)
            elif op == "clear_meshes":
                handle.enqueue_clear_scatter_meshes()
            elif op == "set_actor_visibility":
                aid, visible = args
                handle.enqueue_set_scatter_actor_visible(aid, visible)
        self._pending_scene_ops.clear()
        timings["scene_ops_ms"] = (time.perf_counter() - scene_t0) * 1000.0
        timings["total_ms"] = (time.perf_counter() - total_t0) * 1000.0
        self._last_startup_resource_timings = timings

    def screenshot(self) -> "Any":
        """Capture the scatter viewport as an (H, W, 4) uint8 NumPy array, or None."""
        if (handle := self._live()) is not None:
            raw = handle.app.scatter_screenshot(self.id)
            if raw is not None:
                import numpy as np
                w, h, data = raw
                return np.frombuffer(data, dtype=np.uint8).reshape(h, w, 4).copy()
        return None

    def save_png(self, path: str) -> None:
        """Capture the scatter viewport and write it to a PNG file."""
        img_arr = self.screenshot()
        if img_arr is None:
            raise RuntimeError("screenshot() returned None — widget may not be live")
        try:
            from PIL import Image
            Image.fromarray(img_arr, "RGBA").save(path)
        except ImportError:
            _write_png_stdlib(img_arr, path)

    # ── GIF export ────────────────────────────────────────────────────────────

    def open_gif(self, path: str, fps: int = 20, loop: int = 0) -> None:
        """Begin a GIF recording session. Call write_frame() to append frames, close_gif() to finish.

        Requires Pillow (`pip install Pillow`). `loop=0` means infinite loop.
        """
        from PIL import Image  # noqa: F401 — validate early
        self._gif_path = str(path)
        self._gif_fps = int(fps)
        self._gif_loop = int(loop)
        self._gif_frames: list = []

    def write_frame(self) -> None:
        """Capture the current scatter viewport and append it as a GIF frame."""
        if not hasattr(self, "_gif_frames"):
            raise RuntimeError("call open_gif() before write_frame()")
        arr = self.screenshot()
        if arr is None:
            raise RuntimeError("screenshot() returned None — widget may not be live")
        from PIL import Image
        self._gif_frames.append(Image.fromarray(arr, "RGBA").convert("RGBA"))

    def close_gif(self) -> None:
        """Finalise the GIF and write it to the path given to open_gif()."""
        if not hasattr(self, "_gif_frames") or not self._gif_frames:
            raise RuntimeError("no frames captured — call open_gif() and write_frame() first")
        from PIL import Image
        frames = self._gif_frames
        duration_ms = max(1, int(1000 / self._gif_fps))
        frames[0].save(
            self._gif_path,
            format="GIF",
            save_all=True,
            append_images=frames[1:],
            duration=duration_ms,
            loop=self._gif_loop,
            disposal=2,
        )
        del self._gif_frames, self._gif_path, self._gif_fps, self._gif_loop

    def orbit_gif(
        self,
        path: str,
        n_frames: int = 60,
        fps: int = 20,
        loop: int = 0,
        elevation: float | None = None,
        on_progress: "Any" = None,
    ) -> None:
        """Render a full-rotation orbit GIF.

        Rotates the camera yaw by 2π across `n_frames` screenshots and saves as
        an animated GIF. Requires Pillow. `elevation` overrides pitch if given.
        """
        import math
        cam = self.get_camera()
        if cam is None:
            raise RuntimeError("orbit_gif() requires the widget to be live")
        orig_yaw = cam.get("yaw", 0.4)
        orig_pitch = cam.get("pitch", 0.4)
        pitch = float(elevation) if elevation is not None else orig_pitch
        self.open_gif(path, fps=fps, loop=loop)
        try:
            for i in range(n_frames):
                yaw = orig_yaw + 2 * math.pi * i / n_frames
                self.set_camera({**cam, "yaw": yaw, "pitch": pitch})
                self.write_frame()
                if on_progress is not None:
                    on_progress(i + 1, n_frames)
        finally:
            # Restore original camera state.
            self.set_camera(cam)
            self.close_gif()

    # ── Hover tooltip ─────────────────────────────────────────────────────────

    @property
    def hover_tooltip(self) -> bool:
        """Whether to show a coordinate tooltip when hovering over a point.

        When enabled, native hover picking displays the nearest point's
        coordinate tooltip using the widget's current tooltip axis labels.
        """
        return self._hover_tooltip

    @hover_tooltip.setter
    def hover_tooltip(self, value: bool) -> None:
        self._hover_tooltip = bool(value)
        if (wh := self._live()) is not None:
            wh.enqueue_set_scatter_hover_tooltip(bool(value))


def _scatter2d_class(class_: str | None) -> str:
    if class_ is None:
        return "scatter-plot-2d"
    classes = class_.split()
    if "scatter-plot-2d" in classes:
        return class_
    return f"scatter-plot-2d {class_}"


class ScatterPlot2D(Scatter3D):
    """2D scatter plot backed by the packed Scatter3D point renderer."""

    kind = Scatter3D.kind

    def __init__(
        self,
        frame: Any,
        *,
        x: str,
        y: str,
        color: str | Any | None = None,
        colors: Any | None = None,
        scalars: str | Any | None = None,
        colormap: str = "viridis",
        point_size: float = 4.0,
        point_sizes: str | Any | None = None,
        auto_point_size: bool = True,
        opacity: float = 1.0,
        clim: tuple[float, float] | None = None,
        log_scale: bool = False,
        nan_color: tuple[float, float, float] | None = None,
        size_range: tuple[float, float] | None = None,
        on_pick: ScatterPickCallback | None = None,
        grid: bool = True,
        axis_x: str | None = None,
        axis_y: str | None = None,
        background: tuple[float, float, float] | None = None,
        legend: bool = False,
        legend_position: str = "top-right",
        legend_entries: list[tuple[str, float, float, float]] | None = None,
        scalar_bar: bool = False,
        scalar_bar_vmin: "float | None" = None,
        scalar_bar_vmax: "float | None" = None,
        scalar_bar_log_scale: bool = False,
        scalar_bar_colormap: str = "viridis",
        scalar_bar_title: str | None = None,
        hover: "str | list[str] | None" = None,
        on_hover: "ScatterPickCallback | None" = None,
        lod: bool = False,
        lod_threshold: int = 200_000,
        lod_factor: int = 8,
        interactive_render_scale: float = 1.0,
        auto_quality: bool = False,
        quality_target_fps: float = 10.0,
        id: str | None = None,
        key: str | None = None,
        class_: str | None = None,
        style: Mapping[str, object] | None = None,
        tooltip: str | None = None,
        parent: Container | None | object = _AUTO_PARENT,
    ) -> None:
        self._source_frame = frame
        self._source_x = str(x)
        self._source_y = str(y)
        wrapped = _Scatter2DFrame(frame, self._source_x, self._source_y)
        super().__init__(
            wrapped,
            x=self._source_x,
            y=self._source_y,
            z=wrapped.z_col,
            colormap=colormap,
            color=color,
            colors=colors,
            scalars=scalars,
            point_size=point_size,
            point_sizes=point_sizes,
            auto_point_size=auto_point_size,
            opacity=opacity,
            clim=clim,
            log_scale=log_scale,
            nan_color=nan_color,
            size_range=size_range,
            on_pick=on_pick,
            grid=grid,
            major_planes=False,
            minor_planes=False,
            grid_sticky=True,
            grid_all_edges=False,
            axis_x=axis_x or self._source_x,
            axis_y=axis_y or self._source_y,
            axis_z="",
            background=background,
            legend=legend,
            legend_position=legend_position,
            legend_entries=legend_entries,
            scalar_bar=scalar_bar,
            scalar_bar_vmin=scalar_bar_vmin,
            scalar_bar_vmax=scalar_bar_vmax,
            scalar_bar_log_scale=scalar_bar_log_scale,
            scalar_bar_colormap=scalar_bar_colormap,
            scalar_bar_title=scalar_bar_title,
            orientation_axes=False,
            hover=hover,
            on_hover=on_hover,
            lod=lod,
            lod_threshold=lod_threshold,
            lod_factor=lod_factor,
            interactive_render_scale=interactive_render_scale,
            auto_quality=auto_quality,
            quality_target_fps=quality_target_fps,
            id=id,
            key=key,
            class_=_scatter2d_class(class_),
            style=style,
            tooltip=tooltip,
            parent=parent,
        )
        self.frame_summary = summarize_frame(frame)
        self._axis_visible = (True, True, False)
        self._parallel_projection = True

    def _sync_2d_camera(self, *, fit: bool = False) -> None:
        self._parallel_projection = True
        if (handle := self._live()) is not None:
            if fit:
                handle.enqueue_fit_scatter_camera(None)
            handle.enqueue_set_scatter_parallel_projection(True)
            handle.enqueue_set_scatter_view_direction("xy")
            handle.enqueue_set_scatter_axis_visibility(True, True, False)

    def _queue_startup_resources(self) -> None:
        super()._queue_startup_resources()
        self._sync_2d_camera(fit=True)

    def props(self) -> dict[str, Any]:
        props = super().props()
        props["interaction"] = "pan_2d"
        return props

    def set_points(
        self,
        frame: Any,
        *,
        x: str | None = None,
        y: str | None = None,
        color: str | Any | None = _UNSET,
        colors: Any | None = _UNSET,
        scalars: str | Any | None = _UNSET,
        point_sizes: str | Any | None = _UNSET,
        point_size: float | None = None,
        opacity: float | None = None,
        clim: tuple[float, float] | None = _UNSET,
        log_scale: bool | None = None,
        nan_color: tuple[float, float, float] | None = _UNSET,
        size_range: tuple[float, float] | None = _UNSET,
        hover: "str | list[str] | None" = _UNSET,
        fit: bool = False,
    ) -> None:
        self._source_frame = frame
        self._source_x = str(x or self._source_x)
        self._source_y = str(y or self._source_y)
        wrapped = _Scatter2DFrame(frame, self._source_x, self._source_y)
        super().set_points(
            wrapped,
            x=self._source_x,
            y=self._source_y,
            z=wrapped.z_col,
            color=color,
            colors=colors,
            scalars=scalars,
            point_sizes=point_sizes,
            point_size=point_size,
            opacity=opacity,
            clim=clim,
            log_scale=log_scale,
            nan_color=nan_color,
            size_range=size_range,
            hover=hover,
            fit=False,
        )
        self.frame_summary = summarize_frame(frame)
        self._axis_visible = (True, True, False)
        self._sync_2d_camera(fit=fit)

    def fit(
        self,
        bounds: tuple[float, float, float, float] | tuple[float, float, float, float, float, float] | None = None,
    ) -> None:
        if bounds is None:
            self._sync_2d_camera(fit=True)
            return
        values = tuple(float(value) for value in bounds)
        if len(values) == 4:
            x_min, y_min, x_max, y_max = values
            super().fit((x_min, y_min, 0.0, x_max, y_max, 0.0))
            self._sync_2d_camera()
            return
        if len(values) == 6:
            super().fit(values)  # type: ignore[arg-type]
            self._sync_2d_camera()
            return
        raise ValueError("ScatterPlot2D.fit bounds must be None, a 4-tuple, or a 6-tuple")


def _write_png_stdlib(rgba: "Any", path: str) -> None:
    """Minimal pure-stdlib PNG encoder (no PIL required)."""
    import struct, zlib
    import numpy as np

    arr = np.asarray(rgba, dtype=np.uint8)
    h, w = arr.shape[:2]

    def png_chunk(name: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(name + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + name + data + struct.pack(">I", crc)

    raw_rows = b"".join(b"\x00" + bytes(arr[r].ravel()) for r in range(h))
    compressed = zlib.compress(raw_rows, 9)
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)  # 8-bit RGB (drop alpha for simplicity)
    # Re-pack as RGB
    arr_rgb = arr[:, :, :3]
    raw_rows_rgb = b"".join(b"\x00" + bytes(arr_rgb[r].ravel()) for r in range(h))
    compressed_rgb = zlib.compress(raw_rows_rgb, 9)
    ihdr_rgb = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)

    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(png_chunk(b"IHDR", ihdr_rgb))
        f.write(png_chunk(b"IDAT", compressed_rgb))
        f.write(png_chunk(b"IEND", b""))


class DataFrameTable(Widget):
    kind = "dataframe_table"

    def __init__(
        self,
        frame: Any,
        *,
        page_size: int = 100,
        sample_rows: int = DEFAULT_TABLE_SAMPLE_ROWS,
        sortable: bool = True,
        resizable_columns: bool = True,
        on_select: TableSelectCallback | None = None,
        on_sort: TableSortCallback | None = None,
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
        self.sortable = bool(sortable)
        self.resizable_columns = bool(resizable_columns)
        self.on_select = on_select
        self.on_sort = on_sort
        self.selection: TableSelection | None = None
        self.sort: TableSort | None = None
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
                self._table_payload(include_cells=False),
                self.column_buffers,
            )

    def _sync_after_id_change(self, old_id: str) -> None:
        if self.resource_id == f"{old_id}:table":
            self.resource_id = f"{self.id}:table"

    def _table_payload(self, *, include_cells: bool = True) -> dict[str, Any]:
        return {
            "frame": self.frame_summary.to_dict(),
            "resource_id": self.resource_id,
            "resource_ref": id(self.frame),
            "page_size": self.page_size,
            "virtualized": True,
            "sortable": self.sortable,
            "resizable_columns": self.resizable_columns,
            "sample_rows": self.sample_rows,
            "buffer_columns": len(self.column_buffers),
            "cells": self.cells if include_cells else [],
        }

    def props(self) -> dict[str, Any]:
        include_cells = _include_startup_resource_payloads() or not self.column_buffers
        props = self._table_payload(include_cells=include_cells)
        props["events"] = ["change"] if (self.on_select is not None or self.on_sort is not None) else []
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
