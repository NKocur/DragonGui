from __future__ import annotations

import base64
from collections.abc import Callable, Iterable, Mapping, Sequence
import zlib
from contextlib import AbstractContextManager
from contextvars import ContextVar
from dataclasses import dataclass
from itertools import count
import math
import numbers
import re
import threading
import time
from typing import Any, ClassVar, Self

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
    "led": {"dot", "glow", "highlight"},
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
LedColorValue = str | Sequence[object]


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
    ) -> None:
        self.scatter = scatter
        self.frames = tuple(frames)
        if not self.frames:
            raise ValueError("ScatterFrameStream requires at least one prepared frame")
        self.interval_ms = interval_ms
        self.loop = bool(loop)
        self.on_frame = on_frame
        self.ui_interval_ms = max(0.0, float(ui_interval_ms))
        self.metrics = ScatterStreamMetrics()
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self._metrics_lock = threading.Lock()

    @property
    def running(self) -> bool:
        thread = self._thread
        return thread is not None and thread.is_alive()

    def start(self) -> None:
        if self.running:
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
        thread = self._thread
        if thread is not None and thread is not threading.current_thread():
            thread.join(timeout=timeout)

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
            if index >= len(self.frames) and not self.loop:
                break
            frame_index = index % len(self.frames)
            payload = self.frames[frame_index]
            with self._metrics_lock:
                self.metrics.produced += 1
            try:
                self.scatter.enqueue_prepared_points(
                    payload,
                    coalesce=True,
                    include_metadata=index == 0,
                )
                with self._metrics_lock:
                    self.metrics.submitted += 1
                now_ms = time.perf_counter() * 1000.0
                if (
                    self.on_frame is not None
                    and now_ms - last_ui_ms >= self.ui_interval_ms
                ):
                    last_ui_ms = now_ms
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


ScatterPickCallback = Callable[[ScatterPick], None]

_ids = count(1)
_AUTO_PARENT = object()
_UNSET = object()


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
        tick_count: int = 5,
        auto_fit: bool = True,
        line_width: float = 2.0,
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
        self.x_label = str(x_label if x_label is not None else (x if x is not None else "sample"))
        self.y_label = str(y_label if y_label is not None else self.y_columns[0])
        self.show_grid = bool(show_grid)
        self.show_axes = bool(show_axes)
        self.show_ticks = bool(show_ticks)
        self.show_toolbar = bool(show_toolbar)
        self.tick_count = max(2, min(9, int(tick_count)))
        self.auto_fit = bool(auto_fit)
        self.line_width = max(0.5, float(line_width))
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
        handle.enqueue_set_prop("x_label", self.x_label)
        handle.enqueue_set_prop("y_label", self.y_label)
        handle.enqueue_set_prop("show_grid", self.show_grid)
        handle.enqueue_set_prop("show_axes", self.show_axes)
        handle.enqueue_set_prop("show_ticks", self.show_ticks)
        handle.enqueue_set_prop("show_toolbar", self.show_toolbar)
        handle.enqueue_set_prop("tick_count", self.tick_count)
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
            "tick_count": self.tick_count,
            "auto_fit": self.auto_fit,
            "line_width": self.line_width,
            "max_points": self.max_points,
            "series": series_items,
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
        self._lod_enabled: bool = False
        self._lod_threshold: int = 200_000
        self._lod_factor: int = 8
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
        self._refresh_cached_payload_b64()
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

    def _refresh_cached_payload_b64(self) -> None:
        buf = self._build_payload()
        self._cached_payload = buf
        self._cached_payload_b64 = base64.b64encode(buf).decode("ascii") if buf is not None else None
        # Full-payload CRC used by _scatter_props_equal to detect any data change.
        self._payload_token: int = (
            zlib.crc32(buf) if buf is not None and len(buf) > 0 else 0
        )
        self._compute_auto_color_meta()

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
    ) -> None:
        """Thread-safe native enqueue for an already-packed primary scatter frame."""
        handle = self._live()
        if handle is None:
            return
        handle.enqueue_set_scatter_points_packed(
            payload.data,
            pack_ms=payload.pack_ms,
            enqueue_epoch_ms=time.time() * 1000.0,
            colormap=payload.colormap,
            payload_format=payload.payload_format,
            coalesce=coalesce,
            fit=fit,
        )
        if include_metadata:
            handle.enqueue_set_scatter_tooltip_axis_labels(*payload.axis_labels)
            if payload.hover_meta is not None:
                handle.enqueue_set_scatter_primary_hover_meta(payload.hover_meta)

    def stream_prepared_frames(
        self,
        frames: Iterable[ScatterPayload],
        *,
        interval_ms: float | Callable[[], float] = 16.0,
        loop: bool = True,
        on_frame: Callable[[ScatterPayload, int, ScatterStreamMetrics], None] | None = None,
        ui_interval_ms: float = 250.0,
    ) -> ScatterFrameStream:
        """Create a latest-frame stream for already-prepared scatter payloads."""
        return ScatterFrameStream(
            self,
            frames,
            interval_ms=interval_ms,
            loop=loop,
            on_frame=on_frame,
            ui_interval_ms=ui_interval_ms,
        )

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
            meta = self._extract_hover_meta(self.frame, self._hover)
            if meta is not None:
                handle.enqueue_set_scatter_primary_hover_meta(meta)
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
        self.colormap = _scatter_colormap(colormap)
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
            meta = self._extract_hover_meta(self.frame, self._hover)
            if meta is not None:
                handle.enqueue_set_scatter_primary_hover_meta(meta)
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
            self._scalar_bar_colormap = str(colormap)
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
        self._lod_threshold = int(threshold)
        self._lod_factor = int(factor)
        if (wh := self._live()) is not None:
            wh.enqueue_set_scatter_lod(self._lod_enabled, self._lod_threshold, self._lod_factor)

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
    def _extract_hover_meta(frame: Any, hover: "str | list[str] | None") -> "str | None":
        """Extract per-point hover lines from frame column(s). Returns JSON string or None.

        Each element of the returned JSON array is one multi-line tooltip suffix string
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
        extracted: list[tuple[str, list[str]]] = []
        for col in cols:
            try:
                if hasattr(frame, "__getitem__"):
                    vals = frame[col]
                else:
                    vals = getattr(frame, col)
                extracted.append((col, [_fmt(v) for v in vals]))
            except (KeyError, AttributeError) as exc:
                raise ValueError(
                    f"Scatter3D: hover column {col!r} not found in frame"
                ) from exc
        if not extracted:
            return None
        n = len(extracted[0][1])
        rows = [
            "\n".join(f"{col}: {col_vals[i]}" for col, col_vals in extracted)
            for i in range(n)
        ]
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
        can_send_primary = handle.app._native_method_available("enqueue_set_scatter_points_packed")
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
                handle.enqueue_set_scatter_points_packed(
                    payload,
                    pack_ms=pack_ms,
                    enqueue_epoch_ms=time.time() * 1000.0,
                    colormap=self.colormap,
                    payload_format=self.data_format,
                    coalesce=False,
                )
        # Always sync hover_tooltip to native on startup (default is True on both sides,
        # but a user may have set it to False before the widget went live).
        handle.enqueue_set_scatter_hover_tooltip(self._hover_tooltip)
        if not self._primary_cleared:
            # Sync column names so native tooltip shows the right axis labels.
            handle.enqueue_set_scatter_tooltip_axis_labels(self.x, self.y, self.z)
            # Sync primary hover metadata (column names → per-point strings).
            meta = self._extract_hover_meta(self.frame, self._hover)
            if meta is not None:
                handle.enqueue_set_scatter_primary_hover_meta(meta)
        # Sync LOD config (may have been changed before going live).
        handle.enqueue_set_scatter_lod(self._lod_enabled, self._lod_threshold, self._lod_factor)
        # Sync picking mode.
        handle.enqueue_set_scatter_picking_mode(self._picking_mode)
        # Sync point style override if set before going live.
        if hasattr(self, "_point_style"):
            handle.enqueue_set_scatter_point_style(self._point_style)
        if not self._pending_scene_ops:
            return
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
            "sample_rows": self.sample_rows,
            "buffer_columns": len(self.column_buffers),
            "cells": self.cells if include_cells else [],
        }

    def props(self) -> dict[str, Any]:
        include_cells = _include_startup_resource_payloads() or not self.column_buffers
        props = self._table_payload(include_cells=include_cells)
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
