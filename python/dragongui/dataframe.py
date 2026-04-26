from __future__ import annotations

from dataclasses import dataclass
from typing import Any

DEFAULT_TABLE_SAMPLE_ROWS = 2048
MAX_CELL_CHARS = 80
_MISSING = object()


@dataclass(frozen=True, slots=True)
class DataFrameSummary:
    kind: str
    columns: tuple[str, ...]
    dtypes: tuple[str, ...]
    rows: int | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "kind": self.kind,
            "columns": list(self.columns),
            "dtypes": list(self.dtypes),
            "rows": self.rows,
        }


def summarize_frame(frame: Any) -> DataFrameSummary:
    """Return lightweight metadata for pandas, Polars, or DataFrame-like values."""

    if frame is None:
        return DataFrameSummary(kind="none", columns=(), dtypes=(), rows=None)

    module = type(frame).__module__.split(".", maxsplit=1)[0]
    kind = "dataframe"
    if module in {"pandas", "polars"}:
        kind = module

    columns = _columns(frame)
    dtypes = _dtypes(frame, columns)
    rows = _row_count(frame)
    return DataFrameSummary(kind=kind, columns=columns, dtypes=dtypes, rows=rows)


def extract_table_sample(
    frame: Any,
    summary: DataFrameSummary,
    max_rows: int = DEFAULT_TABLE_SAMPLE_ROWS,
) -> list[list[str]]:
    """Return a bounded row-major sample of formatted cell values.

    This is the first DataFrame interop slice: enough real cell data for the
    native virtualized table to render useful values at startup, without ever
    serializing a whole million-row frame into the UI document.
    """

    if frame is None or max_rows <= 0 or not summary.columns:
        return []

    requested = int(max_rows)
    if summary.rows is not None:
        requested = min(requested, max(summary.rows, 0))
    if requested <= 0:
        return []

    columns = tuple(summary.columns)
    column_data = [_column_data(frame, column) for column in columns]
    if all(column is None for column in column_data):
        return []

    rows: list[list[str]] = []
    for row_idx in range(requested):
        row_values: list[str] = []
        any_present = False
        for column in column_data:
            value = _value_at(column, row_idx)
            if value is _MISSING:
                row_values.append("")
            else:
                any_present = True
                row_values.append(_format_cell(value))
        if not any_present and summary.rows is None:
            break
        rows.append(row_values)
    return rows


def extract_table_column_buffers(
    frame: Any,
    summary: DataFrameSummary,
) -> list[dict[str, object]]:
    """Return packed native table column buffers for supported columns.

    This is deliberately not a JSON/base64 path: the caller passes the returned
    buffer-protocol objects through the native command bridge, while JSON carries
    only lightweight column metadata.
    """

    if frame is None or summary.rows is None or summary.rows <= 0 or not summary.columns:
        return []

    try:
        import numpy as np
    except ImportError:
        return []

    buffers: list[dict[str, object]] = []
    for column in summary.columns:
        column_data = _column_data(frame, column)
        if column_data is None:
            continue
        try:
            arr = np.asarray(column_data)
        except (TypeError, ValueError):
            continue
        if arr.ndim != 1 or arr.shape[0] < summary.rows:
            continue
        packed = _pack_numpy_column(np, arr[: summary.rows])
        if packed is None:
            continue
        dtype, payload = packed
        buffers.append({"name": str(column), "dtype": dtype, "data": payload})
    return buffers


def _pack_numpy_column(np: Any, arr: Any) -> tuple[str, object] | None:
    kind = arr.dtype.kind
    if kind == "b":
        packed = np.ascontiguousarray(arr, dtype=np.bool_)
        return "bool", _byte_view(packed)
    if kind == "f":
        if arr.dtype.itemsize <= 4:
            packed = np.ascontiguousarray(arr, dtype="<f4")
            return "f32", _byte_view(packed)
        packed = np.ascontiguousarray(arr, dtype="<f8")
        return "f64", _byte_view(packed)
    if kind == "i":
        if arr.dtype.itemsize <= 4:
            packed = np.ascontiguousarray(arr, dtype="<i4")
            return "i32", _byte_view(packed)
        packed = np.ascontiguousarray(arr, dtype="<i8")
        return "i64", _byte_view(packed)
    if kind == "u":
        if arr.dtype.itemsize <= 4:
            packed = np.ascontiguousarray(arr, dtype="<u4")
            return "u32", _byte_view(packed)
        packed = np.ascontiguousarray(arr, dtype="<u8")
        return "u64", _byte_view(packed)
    if kind in {"O", "U", "S"}:
        return "utf8", _pack_utf8_column(arr)
    return None


def _byte_view(arr: Any) -> memoryview:
    return memoryview(arr).cast("B")


def _pack_utf8_column(arr: Any) -> bytes:
    data = bytearray()
    offsets: list[int] = [0]
    for value in arr:
        encoded = _format_cell(value).encode("utf-8")
        data.extend(encoded)
        offsets.append(len(data))

    out = bytearray()
    out.extend(len(arr).to_bytes(8, "little", signed=False))
    for offset in offsets:
        out.extend(offset.to_bytes(8, "little", signed=False))
    out.extend(data)
    return bytes(out)


def _columns(frame: Any) -> tuple[str, ...]:
    columns = getattr(frame, "columns", None)
    if columns is not None:
        return tuple(str(column) for column in columns)

    schema = getattr(frame, "schema", None)
    if schema is not None:
        keys = schema.keys() if hasattr(schema, "keys") else schema
        return tuple(str(column) for column in keys)

    return ()


def _dtypes(frame: Any, columns: tuple[str, ...]) -> tuple[str, ...]:
    dtypes = getattr(frame, "dtypes", None)
    if dtypes is not None:
        try:
            return tuple(str(dtype) for dtype in dtypes)
        except TypeError:
            pass

    schema = getattr(frame, "schema", None)
    if schema is not None:
        if hasattr(schema, "items"):
            values = [str(dtype) for _, dtype in schema.items()]
        else:
            try:
                values = [str(dtype) for dtype in schema]
            except TypeError:
                values = []
        if values:
            return tuple(values)

    return tuple("" for _ in columns)


def _row_count(frame: Any) -> int | None:
    height = getattr(frame, "height", None)
    if isinstance(height, int):
        return height

    shape = getattr(frame, "shape", None)
    if isinstance(shape, tuple) and shape:
        rows = shape[0]
        return int(rows) if isinstance(rows, int) else None

    try:
        return len(frame)
    except TypeError:
        return None


def _column_data(frame: Any, column: str) -> Any:
    try:
        return frame[column]
    except (AttributeError, KeyError, TypeError, IndexError):
        pass

    try:
        return getattr(frame, column)
    except AttributeError:
        return None


def _value_at(column: Any, row: int) -> Any:
    if column is None:
        return _MISSING

    iloc = getattr(column, "iloc", None)
    if iloc is not None:
        try:
            return iloc[row]
        except (IndexError, KeyError, TypeError, AttributeError):
            pass

    try:
        return column[row]
    except (IndexError, KeyError, TypeError, AttributeError):
        return _MISSING


def _format_cell(value: Any) -> str:
    if value is None:
        return ""

    item = getattr(value, "item", None)
    if callable(item):
        try:
            value = item()
        except (TypeError, ValueError):
            pass

    if isinstance(value, float):
        text = f"{value:.6g}"
    else:
        text = str(value)

    if len(text) > MAX_CELL_CHARS:
        return f"{text[: MAX_CELL_CHARS - 1]}..."
    return text
