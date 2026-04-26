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
