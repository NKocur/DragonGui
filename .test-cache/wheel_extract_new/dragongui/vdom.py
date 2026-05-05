from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass, field
from typing import Any, ClassVar


_MISSING = object()
_SCALAR_TYPES = (str, int, float, bool, type(None))


@dataclass(frozen=True, slots=True)
class ResourceRef:
    """Opaque identity/version handle for data that must not be deep-compared."""

    identity: int
    version: object | None = None
    kind: str = "resource"
    label: str | None = None

    @classmethod
    def from_value(
        cls,
        value: object,
        *,
        version: object | None = None,
        kind: str | None = None,
        label: str | None = None,
    ) -> ResourceRef:
        resolved_kind = kind or type(value).__name__
        return cls(identity=id(value), version=version, kind=resolved_kind, label=label)

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, ResourceRef):
            return False
        return (
            self.identity == other.identity
            and self.version == other.version
            and self.kind == other.kind
        )

    def __hash__(self) -> int:
        return hash((self.identity, self.version, self.kind))

    def to_dict(self) -> dict[str, object]:
        data: dict[str, object] = {
            "kind": self.kind,
            "identity": self.identity,
        }
        if self.version is not None:
            data["version"] = self.version
        if self.label is not None:
            data["label"] = self.label
        return data


@dataclass(frozen=True, slots=True)
class VNode:
    """Typed virtual node used by the future component diff layer."""

    type: str
    id: str | None = None
    key: str | None = None
    props: Mapping[str, object] = field(default_factory=dict)
    style: Mapping[str, object] | None = None
    class_name: str | None = None
    children: tuple[VNode, ...] = ()
    container: bool = False

    def identity(self) -> tuple[str, str | None, str | None]:
        if self.key is not None:
            return (self.type, "key", self.key)
        return (self.type, "id", self.id)

    def to_dict(self) -> dict[str, object]:
        data: dict[str, object] = {
            "id": self.id,
            "type": self.type,
            "props": dict(self.props),
        }
        if self.key is not None:
            data["key"] = self.key
        if self.class_name is not None:
            data["class"] = self.class_name
        if self.style is not None:
            data["style"] = _mapping_to_dict(self.style)
        if self.container or self.children:
            data["children"] = [child.to_dict() for child in self.children]
        return data


@dataclass(frozen=True, slots=True)
class Patch:
    """Internal patch emitted by VNode diffing before native command mapping."""

    SET_PROP: ClassVar[str] = "set_prop"
    SET_STYLE: ClassVar[str] = "set_style"
    REPLACE_NODE: ClassVar[str] = "replace_node"
    REPLACE_CHILDREN: ClassVar[str] = "replace_children"

    kind: str
    path: tuple[str, ...]
    node_id: str | None = None
    prop: str | None = None
    value: object = None
    style: Mapping[str, object | None] | None = None
    node: VNode | None = None
    children: tuple[VNode, ...] = ()


def widget_to_vnode(widget: object) -> VNode:
    """Convert the current widget-object tree into an internal VNode tree."""

    from .widgets import Container, Widget

    if not isinstance(widget, Widget):
        raise TypeError("widget_to_vnode expects a DragonGUI widget")
    is_container = isinstance(widget, Container)
    children: tuple[VNode, ...] = ()
    if is_container:
        children = tuple(widget_to_vnode(child) for child in widget.children)
    return VNode(
        type=widget.kind,
        id=widget.id,
        key=widget.key,
        props=widget.props(),
        style=widget.style,
        class_name=widget.class_,
        children=children,
        container=is_container,
    )


def diff(old: VNode, new: VNode) -> list[Patch]:
    """Return shallow patches needed to turn `old` into `new`."""

    return _diff_node(old, new, path=(_path_segment(old),))


def same_identity(left: VNode, right: VNode) -> bool:
    return left.identity() == right.identity()


def retain_old_ids(old: VNode, new: VNode) -> VNode:
    """Copy retained native ids from `old` onto identity-equivalent `new` nodes."""

    if not same_identity(old, new):
        return new
    children = _retain_child_ids(old.children, new.children)
    return VNode(
        type=new.type,
        id=old.id,
        key=new.key,
        props=new.props,
        style=new.style,
        class_name=new.class_name,
        children=children,
        container=new.container,
    )


def shallow_value_equal(left: object, right: object) -> bool:
    """Compare values without accidentally walking large user data objects."""

    if left is right:
        return True
    if _is_table_payload(left) and _is_table_payload(right):
        return _table_payload_equal(left, right)
    if _is_scatter_props(left) and _is_scatter_props(right):
        return _scatter_props_equal(left, right)
    if isinstance(left, ResourceRef) or isinstance(right, ResourceRef):
        return left == right
    if isinstance(left, _SCALAR_TYPES) and isinstance(right, _SCALAR_TYPES):
        return left == right
    if _is_scalar_sequence(left) and _is_scalar_sequence(right):
        return tuple(left) == tuple(right)  # type: ignore[arg-type]
    if _is_small_mapping(left) and _is_small_mapping(right):
        left_map = left  # type: ignore[assignment]
        right_map = right  # type: ignore[assignment]
        if left_map.keys() != right_map.keys():
            return False
        return all(shallow_value_equal(left_map[key], right_map[key]) for key in left_map)
    return False


def _diff_node(old: VNode, new: VNode, *, path: tuple[str, ...]) -> list[Patch]:
    if not same_identity(old, new):
        return [Patch(kind=Patch.REPLACE_NODE, path=path, node_id=old.id, node=new)]

    patches: list[Patch] = []
    patches.extend(_diff_props(old, new, path))
    style_patch = _diff_mapping(old.style or {}, new.style or {})
    if style_patch:
        patches.append(
            Patch(
                kind=Patch.SET_STYLE,
                path=path,
                node_id=old.id,
                style=style_patch,
            )
        )
    if old.class_name != new.class_name:
        patches.append(
            Patch(
                kind=Patch.SET_PROP,
                path=path,
                node_id=old.id,
                prop="class",
                value=new.class_name,
            )
        )

    if not _child_identities_equal(old.children, new.children):
        patches.append(
            Patch(
                kind=Patch.REPLACE_CHILDREN,
                path=path,
                node_id=old.id,
                children=new.children,
            )
        )
        return patches

    for old_child, new_child in zip(old.children, new.children, strict=True):
        patches.extend(
            _diff_node(
                old_child,
                new_child,
                path=path + (_path_segment(old_child),),
            )
        )
    return patches


def _child_identities_equal(left: Sequence[VNode], right: Sequence[VNode]) -> bool:
    if len(left) != len(right):
        return False
    return all(old.identity() == new.identity() for old, new in zip(left, right, strict=True))


def _diff_props(old: VNode, new: VNode, path: tuple[str, ...]) -> list[Patch]:
    patches: list[Patch] = []
    if old.type == "dataframe_table" and new.type == "dataframe_table":
        if not shallow_value_equal(old.props, new.props):
            patches.append(
                Patch(
                    kind=Patch.SET_PROP,
                    path=path,
                    node_id=old.id,
                    prop="table",
                    value=new.props,
                )
            )
        return patches
    if old.type == "scatter_3d" and new.type == "scatter_3d":
        if not _scatter_props_equal(old.props, new.props):
            patches.append(
                Patch(
                    kind=Patch.SET_PROP,
                    path=path,
                    node_id=old.id,
                    prop="scatter",
                    value=new.props,
                )
            )
        return patches
    old_props = old.props
    new_props = new.props
    for prop in old_props:
        old_value = old_props[prop]
        new_value = new_props.get(prop, _MISSING)
        if new_value is _MISSING:
            patches.append(
                Patch(
                    kind=Patch.SET_PROP,
                    path=path,
                    node_id=old.id,
                    prop=prop,
                    value=None,
                )
            )
        elif not shallow_value_equal(old_value, new_value):
            patches.append(
                Patch(
                    kind=Patch.SET_PROP,
                    path=path,
                    node_id=old.id,
                    prop=prop,
                    value=new_value,
                )
            )
    for prop, new_value in new_props.items():
        if prop in old_props:
            continue
        patches.append(
            Patch(
                kind=Patch.SET_PROP,
                path=path,
                node_id=old.id,
                prop=prop,
                value=new_value,
            )
        )
    return patches


def _diff_mapping(
    old: Mapping[str, object],
    new: Mapping[str, object],
) -> dict[str, object | None]:
    changes: dict[str, object | None] = {}
    for key in old:
        old_value = old[key]
        new_value = new.get(key, _MISSING)
        if new_value is _MISSING:
            changes[key] = None
        elif not shallow_value_equal(old_value, new_value):
            changes[key] = new_value
    for key, new_value in new.items():
        if key not in old:
            changes[key] = new_value
    return changes


def _retain_child_ids(old_children: Sequence[VNode], new_children: Sequence[VNode]) -> tuple[VNode, ...]:
    old_by_identity: dict[tuple[str, str | None, str | None], list[VNode]] = {}
    for old_child in old_children:
        old_by_identity.setdefault(old_child.identity(), []).append(old_child)

    retained: list[VNode] = []
    for new_child in new_children:
        matches = old_by_identity.get(new_child.identity())
        if matches:
            retained.append(retain_old_ids(matches.pop(0), new_child))
        else:
            retained.append(new_child)
    return tuple(retained)


def _path_segment(node: VNode) -> str:
    if node.key is not None:
        return f"{node.type}:key={node.key}"
    return f"{node.type}:id={node.id}"


def _is_scalar_sequence(value: object) -> bool:
    if isinstance(value, (str, bytes, bytearray, memoryview)):
        return False
    if not isinstance(value, (list, tuple)):
        return False
    return all(isinstance(item, _SCALAR_TYPES) for item in value)


def _is_small_mapping(value: object) -> bool:
    if not isinstance(value, Mapping):
        return False
    if len(value) > 64:
        return False
    return all(
        isinstance(key, str)
        and (
            isinstance(item, _SCALAR_TYPES)
            or _is_scalar_sequence(item)
            or isinstance(item, ResourceRef)
            or _is_small_mapping(item)
        )
        for key, item in value.items()
    )


def _is_table_payload(value: object) -> bool:
    return (
        isinstance(value, Mapping)
        and isinstance(value.get("frame"), Mapping)
        and isinstance(value.get("resource_id"), str)
        and "cells" in value
    )


def _table_payload_equal(left: object, right: object) -> bool:
    left_map = left  # type: ignore[assignment]
    right_map = right  # type: ignore[assignment]
    assert isinstance(left_map, Mapping)
    assert isinstance(right_map, Mapping)
    keys = (
        "resource_id",
        "resource_ref",
        "page_size",
        "virtualized",
        "sample_rows",
        "buffer_columns",
    )
    if any(left_map.get(key) != right_map.get(key) for key in keys):
        return False
    return left_map.get("frame") == right_map.get("frame")


def _is_scatter_props(value: object) -> bool:
    return (
        isinstance(value, Mapping)
        and isinstance(value.get("frame"), Mapping)
        and "data_b64" in value
        and "x" in value
    )


def _scatter_props_equal(left: object, right: object) -> bool:
    left_map = left  # type: ignore[assignment]
    right_map = right  # type: ignore[assignment]
    assert isinstance(left_map, Mapping)
    assert isinstance(right_map, Mapping)
    # Structural options: column names, colormap, format, callbacks, grid chrome, overlays.
    for key in (
        "x", "y", "z", "colormap", "data_format", "events",
        "grid_visible", "major_planes", "minor_planes", "grid_sticky", "grid_all_edges",
        "axis_x", "axis_y", "axis_z",
        "axis_vis_x", "axis_vis_y", "axis_vis_z",
        "tick_x", "tick_y", "tick_z",
        "background",
        "legend_visible", "legend_position", "legend_entries", "legend_title",
        "scalar_bar_visible", "scalar_bar_vmin", "scalar_bar_vmax",
        "scalar_bar_log_scale", "scalar_bar_colormap", "scalar_bar_title",
        "orientation_axes_visible",
    ):
        if left_map.get(key) != right_map.get(key):
            return False
    # Data identity: compact token from packed payload (avoids O(n) base64 comparison).
    # When present, comparing the token is sufficient to detect data changes.
    # When absent (e.g. manual VNode construction), fall back to comparing the
    # frame handle, which supports ResourceRef equality by identity+version.
    l_token = left_map.get("_payload_token")
    r_token = right_map.get("_payload_token")
    if l_token is not None or r_token is not None:
        return l_token == r_token
    return left_map.get("frame") == right_map.get("frame")


def _mapping_to_dict(value: Mapping[str, object]) -> dict[str, object]:
    data: dict[str, object] = {}
    for key, item in value.items():
        if isinstance(item, Mapping):
            data[key] = _mapping_to_dict(item)
        elif isinstance(item, ResourceRef):
            data[key] = item.to_dict()
        else:
            data[key] = item
    return data
