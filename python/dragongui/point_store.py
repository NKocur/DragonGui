"""Reusable exact column storage for scatter visualizations."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from threading import RLock
import time
from typing import Any, Literal
from uuid import uuid4

PointStoreOwnership = Literal["borrowed", "copied", "moved"]


class PointStore:
    """Own or borrow exact contiguous columns shared by compatible plots.

    ``borrowed`` retains views of caller-owned contiguous arrays. Call
    :meth:`touch` after mutating a borrowed source externally. ``copied`` makes
    private contiguous copies. ``moved`` reuses compatible arrays and may make
    them read-only; callers should stop mutating inputs passed in that mode.
    """

    def __init__(
        self,
        x: Any,
        y: Any,
        *,
        z: Any | None = None,
        scalars: Any | None = None,
        ownership: PointStoreOwnership = "borrowed",
        row_ids: Sequence[Any] | None = None,
    ) -> None:
        columns: dict[str, Any] = {"x": x, "y": y}
        if z is not None:
            columns["z"] = z
        if scalars is not None:
            columns["scalars"] = scalars
        self._initialize(columns, ownership=ownership, row_ids=row_ids)

    @classmethod
    def from_columns(
        cls,
        *,
        x: Any,
        y: Any,
        z: Any | None = None,
        scalars: Any | None = None,
        ownership: PointStoreOwnership = "borrowed",
        row_ids: Sequence[Any] | None = None,
    ) -> "PointStore":
        return cls(
            x,
            y,
            z=z,
            scalars=scalars,
            ownership=ownership,
            row_ids=row_ids,
        )

    @classmethod
    def from_mapping(
        cls,
        columns: Mapping[str, Any],
        *,
        ownership: PointStoreOwnership = "borrowed",
        row_ids: Sequence[Any] | None = None,
    ) -> "PointStore":
        if "x" not in columns or "y" not in columns:
            raise ValueError("PointStore.from_mapping requires 'x' and 'y' columns")
        store = cls.__new__(cls)
        store._initialize(dict(columns), ownership=ownership, row_ids=row_ids)
        return store

    def _initialize(
        self,
        columns: Mapping[str, Any],
        *,
        ownership: PointStoreOwnership,
        row_ids: Sequence[Any] | None,
    ) -> None:
        import numpy as np

        if ownership not in {"borrowed", "copied", "moved"}:
            raise ValueError("PointStore ownership must be 'borrowed', 'copied', or 'moved'")
        if not columns:
            raise ValueError("PointStore requires at least one column")

        normalized: dict[str, Any] = {}
        row_count: int | None = None
        for raw_name, values in columns.items():
            name = str(raw_name)
            if not name:
                raise ValueError("PointStore column names must be non-empty")
            arr = np.asarray(values)
            if arr.ndim != 1:
                raise ValueError(f"PointStore column {name!r} must be one-dimensional")
            if arr.dtype.kind not in {"b", "i", "u", "f"}:
                raise TypeError(f"PointStore column {name!r} must be numeric")
            if row_count is None:
                row_count = len(arr)
            elif len(arr) != row_count:
                raise ValueError("PointStore columns must have equal lengths")
            if ownership == "borrowed":
                if not arr.flags.c_contiguous:
                    raise ValueError(
                        f"borrowed PointStore column {name!r} must be C-contiguous"
                    )
                stored = arr.view()
                stored.flags.writeable = False
            elif ownership == "copied":
                stored = np.array(arr, copy=True, order="C")
                stored.flags.writeable = False
            else:
                stored = arr if arr.flags.c_contiguous else np.ascontiguousarray(arr)
                stored.flags.writeable = False
            normalized[name] = stored

        assert row_count is not None
        if row_ids is None:
            normalized_row_ids: Sequence[Any] = range(row_count)
        else:
            if len(row_ids) != row_count:
                raise ValueError("PointStore row_ids must match the column length")
            normalized_row_ids = tuple(row_ids) if ownership == "copied" else row_ids

        self._lock = RLock()
        self._columns = normalized
        self._ownership: PointStoreOwnership = ownership
        self._row_count = row_count
        self._row_ids = normalized_row_ids
        self._source_revision = 1
        self._data_revision = 1
        self._native_store_id = f"point-store-{uuid4().hex}"
        self._column_revisions = {name: 1 for name in normalized}
        self._finite_masks: dict[tuple[tuple[str, int], ...], Any] = {}
        self._bounds: dict[
            tuple[tuple[str, int], ...],
            tuple[tuple[float, ...], tuple[float, ...]] | None,
        ] = {}
        self._packed_xy: dict[tuple[str, int, str, int], Any] = {}
        self._packed_xyz: dict[tuple[str, int, str, int, str, int], Any] = {}
        self._spatial_indexes: dict[tuple[str, int], dict[str, Any]] = {}
        self._spatial_monotonic: dict[tuple[str, int], bool] = {}
        self._chunk_bounds: dict[
            tuple[str, int, str, int, str, int, int], dict[str, Any]
        ] = {}
        self._spatial_query_count = 0
        self._spatial_candidate_rows = 0
        self._chunk_query_count = 0
        self._chunk_candidate_rows = 0

    @property
    def ownership(self) -> PointStoreOwnership:
        return self._ownership

    @property
    def source_revision(self) -> int:
        return self._source_revision

    @property
    def data_revision(self) -> int:
        return self._data_revision

    @property
    def columns(self) -> tuple[str, ...]:
        return tuple(self._columns)

    @property
    def dtypes(self) -> tuple[str, ...]:
        return tuple(str(column.dtype) for column in self._columns.values())

    @property
    def shape(self) -> tuple[int, int]:
        return self._row_count, len(self._columns)

    @property
    def index(self) -> Sequence[Any]:
        return self._row_ids

    @property
    def row_ids(self) -> Sequence[Any]:
        return self._row_ids

    @property
    def source_bytes(self) -> int:
        return sum(int(column.nbytes) for column in self._columns.values())

    @property
    def render_cache_bytes(self) -> int:
        return sum(int(payload.nbytes) for payload in self._packed_xy.values()) + sum(
            int(payload.nbytes) for payload in self._packed_xyz.values()
        )

    def __len__(self) -> int:
        return self._row_count

    def __getitem__(self, name: str) -> Any:
        return self._columns[str(name)]

    def column_revision(self, name: str) -> int:
        return self._column_revisions[str(name)]

    def replace_column(
        self,
        name: str,
        values: Any,
        *,
        ownership: PointStoreOwnership | None = None,
    ) -> None:
        import numpy as np

        name = str(name)
        mode = ownership or self._ownership
        if mode not in {"borrowed", "copied", "moved"}:
            raise ValueError("PointStore ownership must be 'borrowed', 'copied', or 'moved'")
        arr = np.asarray(values)
        if arr.ndim != 1 or len(arr) != self._row_count:
            raise ValueError("replacement PointStore columns must match the existing row count")
        if mode == "borrowed":
            if not arr.flags.c_contiguous:
                raise ValueError("borrowed PointStore columns must be C-contiguous")
            stored = arr.view()
            stored.flags.writeable = False
        elif mode == "copied":
            stored = np.array(arr, copy=True, order="C")
            stored.flags.writeable = False
        else:
            stored = arr if arr.flags.c_contiguous else np.ascontiguousarray(arr)
            stored.flags.writeable = False
        with self._lock:
            self._columns[name] = stored
            self._source_revision += 1
            self._data_revision += 1
            self._column_revisions[name] = self._column_revisions.get(name, 0) + 1
            self._invalidate_columns({name})

    def touch(self, *names: str) -> None:
        """Declare externally-mutated borrowed columns and invalidate dependents."""
        changed = {str(name) for name in names} if names else set(self._columns)
        unknown = changed.difference(self._columns)
        if unknown:
            raise KeyError(next(iter(sorted(unknown))))
        with self._lock:
            self._source_revision += 1
            self._data_revision += 1
            for name in changed:
                self._column_revisions[name] += 1
            self._invalidate_columns(changed)

    def finite_mask(self, *names: str) -> Any:
        import numpy as np

        selected = tuple(str(name) for name in names)
        if not selected:
            selected = tuple(self._columns)
        key = tuple((name, self._column_revisions[name]) for name in selected)
        with self._lock:
            cached = self._finite_masks.get(key)
            if cached is not None:
                return cached
            mask = np.ones(self._row_count, dtype=np.bool_)
            for name in selected:
                mask &= np.isfinite(self._columns[name])
            mask.flags.writeable = False
            self._finite_masks[key] = mask
            return mask

    def bounds(
        self,
        *names: str,
    ) -> tuple[tuple[float, ...], tuple[float, ...]] | None:
        import numpy as np

        selected = tuple(str(name) for name in names) or tuple(self._columns)
        key = tuple((name, self._column_revisions[name]) for name in selected)
        with self._lock:
            if key in self._bounds:
                return self._bounds[key]
            mask = self.finite_mask(*selected)
            if not bool(np.any(mask)):
                result = None
            else:
                mins = tuple(float(np.min(self._columns[name][mask])) for name in selected)
                maxs = tuple(float(np.max(self._columns[name][mask])) for name in selected)
                result = mins, maxs
            self._bounds[key] = result
            return result

    def _dragongui_pack_xy(self, x_name: str | None, y_name: str) -> Any:
        import numpy as np

        y_name = str(y_name)
        if x_name is None:
            x_key = "<range>"
            x_revision = self._row_count
        else:
            x_name = str(x_name)
            x_key = x_name
            x_revision = self._column_revisions[x_name]
        key = (x_key, x_revision, y_name, self._column_revisions[y_name])
        with self._lock:
            cached = self._packed_xy.get(key)
            if cached is not None:
                return cached
            ys = np.asarray(self._columns[y_name], dtype=np.float32)
            xs = (
                np.arange(self._row_count, dtype=np.float32)
                if x_name is None
                else np.asarray(self._columns[x_name], dtype=np.float32)
            )
            packed = np.empty((self._row_count, 2), dtype="<f4")
            packed[:, 0] = xs
            packed[:, 1] = ys
            payload = packed.view(np.uint8).reshape(-1)
            payload.flags.writeable = False
            self._packed_xy[key] = payload
            return payload

    def _dragongui_xy_source(self, x_name: str | None, y_name: str) -> tuple[str, int]:
        """Return the stable native identity for the current XY render revision."""
        # data_revision is deliberately conservative: changing a non-coordinate
        # column may register a fresh revision, but can never alias changed bytes.
        projection = f"{self._native_store_id}:xy:{x_name!r}:{str(y_name)!r}"
        return projection, self._data_revision

    def _dragongui_pack_xyz(self, x_name: str, y_name: str, z_name: str) -> Any:
        import numpy as np

        x_name, y_name, z_name = str(x_name), str(y_name), str(z_name)
        key = (
            x_name,
            self._column_revisions[x_name],
            y_name,
            self._column_revisions[y_name],
            z_name,
            self._column_revisions[z_name],
        )
        with self._lock:
            cached = self._packed_xyz.get(key)
            if cached is not None:
                return cached
            packed = np.empty((self._row_count, 3), dtype="<f4")
            packed[:, 0] = np.asarray(self._columns[x_name], dtype=np.float32)
            packed[:, 1] = np.asarray(self._columns[y_name], dtype=np.float32)
            packed[:, 2] = np.asarray(self._columns[z_name], dtype=np.float32)
            payload = packed.view(np.uint8).reshape(-1)
            payload.flags.writeable = False
            self._packed_xyz[key] = payload
            return payload

    def _dragongui_xyz_source(self, x_name: str, y_name: str, z_name: str) -> tuple[str, int]:
        projection = (
            f"{self._native_store_id}:xyz:{str(x_name)!r}:{str(y_name)!r}:{str(z_name)!r}"
        )
        return projection, self._data_revision

    def _dragongui_xyz_bounds(
        self,
        x_name: str,
        y_name: str,
        z_name: str,
    ) -> tuple[tuple[float, float, float], tuple[float, float, float]] | None:
        result = self.bounds(x_name, y_name, z_name)
        if result is None:
            return None
        return result  # type: ignore[return-value]

    def query_rect(
        self,
        x_min: float,
        x_max: float,
        y_min: float,
        y_max: float,
        *,
        x: str = "x",
        y: str = "y",
        strategy: Literal["auto", "scan", "sorted_x"] = "auto",
    ) -> Any:
        """Return exact positional rows inside an axis-aligned 2D viewport."""
        import numpy as np

        x = str(x)
        y = str(y)
        if strategy not in {"auto", "scan", "sorted_x"}:
            raise ValueError("PointStore query strategy must be 'auto', 'scan', or 'sorted_x'")
        lo_x, hi_x = sorted((float(x_min), float(x_max)))
        lo_y, hi_y = sorted((float(y_min), float(y_max)))
        xs = np.asarray(self._columns[x])
        ys = np.asarray(self._columns[y])

        index_key = (x, self._column_revisions[x])
        use_index = strategy == "sorted_x" or (
            strategy == "auto"
            and (
                index_key in self._spatial_indexes
                or self._x_is_monotonic(x)
            )
        )
        if not use_index:
            mask = (
                np.isfinite(xs)
                & np.isfinite(ys)
                & (xs >= lo_x)
                & (xs <= hi_x)
                & (ys >= lo_y)
                & (ys <= hi_y)
            )
            result = np.flatnonzero(mask)
            candidate_count = self._row_count
        else:
            index = self._sorted_x_index(x)
            sorted_x = index["sorted_x"]
            search_lo = np.nextafter(
                np.asarray(lo_x, dtype=sorted_x.dtype),
                np.asarray(-np.inf, dtype=sorted_x.dtype),
            )
            search_hi = np.nextafter(
                np.asarray(hi_x, dtype=sorted_x.dtype),
                np.asarray(np.inf, dtype=sorted_x.dtype),
            )
            left = int(np.searchsorted(sorted_x, search_lo, side="left"))
            right = int(np.searchsorted(sorted_x, search_hi, side="right"))
            order = index["order"]
            if order is None:
                candidate_x = xs[left:right]
                candidate_y = ys[left:right]
                keep = (
                    np.isfinite(candidate_y)
                    & (candidate_x >= lo_x)
                    & (candidate_x <= hi_x)
                    & (candidate_y >= lo_y)
                    & (candidate_y <= hi_y)
                )
                result = np.flatnonzero(keep) + left
            else:
                candidates = order[left:right]
                candidate_x = xs[candidates]
                candidate_y = ys[candidates]
                keep = (
                    np.isfinite(candidate_y)
                    & (candidate_x >= lo_x)
                    & (candidate_x <= hi_x)
                    & (candidate_y >= lo_y)
                    & (candidate_y <= hi_y)
                )
                result = candidates[keep]
                result = np.sort(result)
            candidate_count = right - left

        result = np.asarray(result, dtype=np.intp)
        result.flags.writeable = False
        with self._lock:
            self._spatial_query_count += 1
            self._spatial_candidate_rows += int(candidate_count)
        return result

    def query_box(
        self,
        x_min: float,
        x_max: float,
        y_min: float,
        y_max: float,
        z_min: float,
        z_max: float,
        *,
        x: str = "x",
        y: str = "y",
        z: str = "z",
        strategy: Literal["auto", "scan", "sorted_x"] = "auto",
    ) -> Any:
        """Return exact positional rows inside an axis-aligned 3D box."""
        import numpy as np

        x, y, z = str(x), str(y), str(z)
        if strategy not in {"auto", "scan", "sorted_x"}:
            raise ValueError("PointStore query strategy must be 'auto', 'scan', or 'sorted_x'")
        lo_x, hi_x = sorted((float(x_min), float(x_max)))
        lo_y, hi_y = sorted((float(y_min), float(y_max)))
        lo_z, hi_z = sorted((float(z_min), float(z_max)))
        xs = np.asarray(self._columns[x])
        ys = np.asarray(self._columns[y])
        zs = np.asarray(self._columns[z])

        index_key = (x, self._column_revisions[x])
        use_index = strategy == "sorted_x" or (
            strategy == "auto"
            and (index_key in self._spatial_indexes or self._x_is_monotonic(x))
        )
        if not use_index:
            mask = (
                np.isfinite(xs)
                & np.isfinite(ys)
                & np.isfinite(zs)
                & (xs >= lo_x)
                & (xs <= hi_x)
                & (ys >= lo_y)
                & (ys <= hi_y)
                & (zs >= lo_z)
                & (zs <= hi_z)
            )
            result = np.flatnonzero(mask)
            candidate_count = self._row_count
        else:
            index = self._sorted_x_index(x)
            sorted_x = index["sorted_x"]
            search_lo = np.nextafter(
                np.asarray(lo_x, dtype=sorted_x.dtype),
                np.asarray(-np.inf, dtype=sorted_x.dtype),
            )
            search_hi = np.nextafter(
                np.asarray(hi_x, dtype=sorted_x.dtype),
                np.asarray(np.inf, dtype=sorted_x.dtype),
            )
            left = int(np.searchsorted(sorted_x, search_lo, side="left"))
            right = int(np.searchsorted(sorted_x, search_hi, side="right"))
            order = index["order"]
            if order is None:
                candidate_x = xs[left:right]
                candidate_y = ys[left:right]
                candidate_z = zs[left:right]
                keep = (
                    np.isfinite(candidate_y)
                    & np.isfinite(candidate_z)
                    & (candidate_x >= lo_x)
                    & (candidate_x <= hi_x)
                    & (candidate_y >= lo_y)
                    & (candidate_y <= hi_y)
                    & (candidate_z >= lo_z)
                    & (candidate_z <= hi_z)
                )
                result = np.flatnonzero(keep) + left
            else:
                candidates = order[left:right]
                candidate_x = xs[candidates]
                candidate_y = ys[candidates]
                candidate_z = zs[candidates]
                keep = (
                    np.isfinite(candidate_y)
                    & np.isfinite(candidate_z)
                    & (candidate_x >= lo_x)
                    & (candidate_x <= hi_x)
                    & (candidate_y >= lo_y)
                    & (candidate_y <= hi_y)
                    & (candidate_z >= lo_z)
                    & (candidate_z <= hi_z)
                )
                result = np.sort(candidates[keep])
            candidate_count = right - left

        result = np.asarray(result, dtype=np.intp)
        result.flags.writeable = False
        with self._lock:
            self._spatial_query_count += 1
            self._spatial_candidate_rows += int(candidate_count)
        return result

    def _sorted_x_index(self, x_name: str) -> dict[str, Any]:
        import numpy as np

        key = (x_name, self._column_revisions[x_name])
        with self._lock:
            cached = self._spatial_indexes.get(key)
            if cached is not None:
                return cached
            started = time.perf_counter()
            xs = np.asarray(self._columns[x_name])
            monotonic = self._x_is_monotonic(x_name)
            if monotonic:
                order = None
                sorted_x = xs
                owned_bytes = 0
            else:
                order = np.argsort(xs, kind="stable")
                order.flags.writeable = False
                sorted_x = np.asarray(xs[order])
                sorted_x.flags.writeable = False
                owned_bytes = int(order.nbytes + sorted_x.nbytes)
            cached = {
                "order": order,
                "sorted_x": sorted_x,
                "monotonic": monotonic,
                "owned_bytes": owned_bytes,
                "build_ms": (time.perf_counter() - started) * 1000.0,
            }
            self._spatial_indexes[key] = cached
            return cached

    def _x_is_monotonic(self, x_name: str) -> bool:
        import numpy as np

        key = (x_name, self._column_revisions[x_name])
        with self._lock:
            cached = self._spatial_monotonic.get(key)
            if cached is not None:
                return cached
            xs = np.asarray(self._columns[x_name])
            result = bool(np.all(np.isfinite(xs))) and bool(
                len(xs) < 2 or np.all(xs[1:] >= xs[:-1])
            )
            self._spatial_monotonic[key] = result
            return result

    def build_spatial_index(self, *, x: str = "x") -> dict[str, Any]:
        """Build and return diagnostics for the reusable sorted-X query index."""
        index = self._sorted_x_index(str(x))
        return {
            "column": str(x),
            "monotonic": bool(index["monotonic"]),
            "owned_bytes": int(index["owned_bytes"]),
            "build_ms": float(index["build_ms"]),
        }

    def build_chunk_bounds(
        self,
        *,
        x: str = "x",
        y: str = "y",
        z: str = "z",
        chunk_rows: int = 65_536,
    ) -> dict[str, Any]:
        """Build stable source-order chunks with finite 3D axis-aligned bounds."""
        index = self._chunk_bounds_index(str(x), str(y), str(z), int(chunk_rows))
        return {
            "chunk_rows": int(index["chunk_rows"]),
            "chunk_count": int(len(index["starts"])),
            "finite_chunk_count": int(index["finite"].sum()),
            "owned_bytes": int(index["owned_bytes"]),
            "build_ms": float(index["build_ms"]),
        }

    def query_box_chunks(
        self,
        x_min: float,
        x_max: float,
        y_min: float,
        y_max: float,
        z_min: float,
        z_max: float,
        *,
        x: str = "x",
        y: str = "y",
        z: str = "z",
        chunk_rows: int = 65_536,
    ) -> Any:
        """Return conservative source-row ranges whose 3D bounds intersect a box."""
        import numpy as np

        index = self._chunk_bounds_index(str(x), str(y), str(z), int(chunk_rows))
        lo = np.asarray(
            [min(x_min, x_max), min(y_min, y_max), min(z_min, z_max)],
            dtype=np.float64,
        )
        hi = np.asarray(
            [max(x_min, x_max), max(y_min, y_max), max(z_min, z_max)],
            dtype=np.float64,
        )
        keep = index["finite"] & np.all(index["maxs"] >= lo, axis=1) & np.all(
            index["mins"] <= hi, axis=1
        )
        result = np.column_stack((index["starts"][keep], index["stops"][keep]))
        result.flags.writeable = False
        with self._lock:
            self._chunk_query_count += 1
            self._chunk_candidate_rows += int(np.sum(result[:, 1] - result[:, 0]))
        return result

    def query_frustum_chunks(
        self,
        planes: Sequence[Sequence[float]],
        *,
        x: str = "x",
        y: str = "y",
        z: str = "z",
        chunk_rows: int = 65_536,
    ) -> Any:
        """Return chunk ranges intersecting planes whose inside half-space is >= 0."""
        import numpy as np

        plane_array = np.asarray(planes, dtype=np.float64)
        if plane_array.ndim != 2 or plane_array.shape[1] != 4 or len(plane_array) == 0:
            raise ValueError("frustum planes must have shape (n, 4)")
        if not bool(np.all(np.isfinite(plane_array))):
            raise ValueError("frustum planes must be finite")
        index = self._chunk_bounds_index(str(x), str(y), str(z), int(chunk_rows))
        keep = index["finite"].copy()
        for plane in plane_array:
            normal = plane[:3]
            positive = np.where(normal >= 0.0, index["maxs"], index["mins"])
            keep &= positive @ normal + plane[3] >= 0.0
        result = np.column_stack((index["starts"][keep], index["stops"][keep]))
        result.flags.writeable = False
        with self._lock:
            self._chunk_query_count += 1
            self._chunk_candidate_rows += int(np.sum(result[:, 1] - result[:, 0]))
        return result

    def _chunk_bounds_index(
        self,
        x_name: str,
        y_name: str,
        z_name: str,
        chunk_rows: int,
    ) -> dict[str, Any]:
        import numpy as np

        if chunk_rows <= 0:
            raise ValueError("chunk_rows must be positive")
        key = (
            x_name,
            self._column_revisions[x_name],
            y_name,
            self._column_revisions[y_name],
            z_name,
            self._column_revisions[z_name],
            chunk_rows,
        )
        with self._lock:
            cached = self._chunk_bounds.get(key)
            if cached is not None:
                return cached
            started = time.perf_counter()
            xs = np.asarray(self._columns[x_name])
            ys = np.asarray(self._columns[y_name])
            zs = np.asarray(self._columns[z_name])
            starts = np.arange(0, self._row_count, chunk_rows, dtype=np.int64)
            stops = np.minimum(starts + chunk_rows, self._row_count)
            mins = np.full((len(starts), 3), np.inf, dtype=np.float64)
            maxs = np.full((len(starts), 3), -np.inf, dtype=np.float64)
            finite = np.zeros(len(starts), dtype=np.bool_)
            for chunk, (start, stop) in enumerate(zip(starts, stops)):
                coordinates = (xs[start:stop], ys[start:stop], zs[start:stop])
                mask = np.isfinite(coordinates[0])
                mask &= np.isfinite(coordinates[1])
                mask &= np.isfinite(coordinates[2])
                if bool(np.any(mask)):
                    finite[chunk] = True
                    for axis, values in enumerate(coordinates):
                        mins[chunk, axis] = float(np.min(values[mask]))
                        maxs[chunk, axis] = float(np.max(values[mask]))
            for array in (starts, stops, mins, maxs, finite):
                array.flags.writeable = False
            owned_bytes = sum(int(array.nbytes) for array in (starts, stops, mins, maxs, finite))
            cached = {
                "starts": starts,
                "stops": stops,
                "mins": mins,
                "maxs": maxs,
                "finite": finite,
                "chunk_rows": chunk_rows,
                "owned_bytes": owned_bytes,
                "build_ms": (time.perf_counter() - started) * 1000.0,
            }
            self._chunk_bounds[key] = cached
            return cached

    def _invalidate_columns(self, names: set[str]) -> None:
        self._finite_masks = {
            key: value
            for key, value in self._finite_masks.items()
            if names.isdisjoint(name for name, _ in key)
        }
        self._bounds = {
            key: value
            for key, value in self._bounds.items()
            if names.isdisjoint(name for name, _ in key)
        }
        self._packed_xy = {
            key: value
            for key, value in self._packed_xy.items()
            if key[0] not in names and key[2] not in names
        }
        self._packed_xyz = {
            key: value
            for key, value in self._packed_xyz.items()
            if names.isdisjoint((key[0], key[2], key[4]))
        }
        self._spatial_indexes = {
            key: value for key, value in self._spatial_indexes.items() if key[0] not in names
        }
        self._spatial_monotonic = {
            key: value for key, value in self._spatial_monotonic.items() if key[0] not in names
        }
        self._chunk_bounds = {
            key: value
            for key, value in self._chunk_bounds.items()
            if names.isdisjoint((key[0], key[2], key[4]))
        }

    def stats(self) -> dict[str, Any]:
        return {
            "rows": self._row_count,
            "columns": list(self._columns),
            "dtypes": list(self.dtypes),
            "ownership": self._ownership,
            "source_revision": self._source_revision,
            "data_revision": self._data_revision,
            "native_store_id": self._native_store_id,
            "column_revisions": dict(self._column_revisions),
            "source_bytes": self.source_bytes,
            "render_cache_entries": len(self._packed_xy) + len(self._packed_xyz),
            "render_cache_xy_entries": len(self._packed_xy),
            "render_cache_xyz_entries": len(self._packed_xyz),
            "render_cache_bytes": self.render_cache_bytes,
            "bounds_cache_entries": len(self._bounds),
            "finite_mask_cache_entries": len(self._finite_masks),
            "spatial_index_entries": len(self._spatial_indexes),
            "spatial_monotonic_probes": len(self._spatial_monotonic),
            "spatial_index_bytes": sum(
                int(index["owned_bytes"]) for index in self._spatial_indexes.values()
            ),
            "spatial_queries": self._spatial_query_count,
            "spatial_candidate_rows": self._spatial_candidate_rows,
            "chunk_bounds_entries": len(self._chunk_bounds),
            "chunk_bounds_bytes": sum(
                int(index["owned_bytes"]) for index in self._chunk_bounds.values()
            ),
            "chunk_queries": self._chunk_query_count,
            "chunk_candidate_rows": self._chunk_candidate_rows,
        }
