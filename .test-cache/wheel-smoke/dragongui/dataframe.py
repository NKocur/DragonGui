from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True, slots=True)
class DataFrameSummary:
    kind: str
    columns: tuple[str, ...]
    rows: int | None

    def to_dict(self) -> dict[str, Any]:
        return {
            "kind": self.kind,
            "columns": list(self.columns),
            "rows": self.rows,
        }


def summarize_frame(frame: Any) -> DataFrameSummary:
    """Return lightweight metadata for pandas, Polars, or DataFrame-like values."""

    if frame is None:
        return DataFrameSummary(kind="none", columns=(), rows=None)

    module = type(frame).__module__.split(".", maxsplit=1)[0]
    kind = "dataframe"
    if module in {"pandas", "polars"}:
        kind = module

    columns = _columns(frame)
    rows = _row_count(frame)
    return DataFrameSummary(kind=kind, columns=columns, rows=rows)


def _columns(frame: Any) -> tuple[str, ...]:
    columns = getattr(frame, "columns", None)
    if columns is not None:
        return tuple(str(column) for column in columns)

    schema = getattr(frame, "schema", None)
    if schema is not None:
        keys = schema.keys() if hasattr(schema, "keys") else schema
        return tuple(str(column) for column in keys)

    return ()


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
