from __future__ import annotations

import json
from functools import lru_cache
from importlib.resources import files
from typing import Any


_REGISTRY_RESOURCE = "widget_css_capabilities.json"


@lru_cache(maxsize=1)
def widget_css_capabilities() -> dict[str, Any]:
    """Load the packaged authoritative widget CSS capability registry."""

    resource = files(__package__).joinpath(_REGISTRY_RESOURCE)
    data = json.loads(resource.read_text(encoding="utf-8"))
    if data.get("schema_version") != 1:
        raise RuntimeError("unsupported DragonGui widget CSS capability schema")
    if not isinstance(data.get("widgets"), list):
        raise RuntimeError("widget CSS capability registry has no widget list")
    return data


def _part_names(widget: dict[str, Any]) -> set[str]:
    parts = widget.get("parts", {})
    if not isinstance(parts, dict):
        raise RuntimeError(f"invalid parts entry for {widget.get('public_type', 'unknown')}")
    names: set[str] = set()
    for renderer, renderer_parts in parts.items():
        if renderer not in {"paint", "text", "structural", "forwarded"}:
            raise RuntimeError(f"unknown widget CSS renderer status {renderer!r}")
        if not isinstance(renderer_parts, list) or not all(
            isinstance(part, str) for part in renderer_parts
        ):
            raise RuntimeError(f"invalid {renderer!r} parts list")
        names.update(renderer_parts)
    return names


def supports_generated_content_part(python_kind: str, part: str) -> bool:
    """Return whether a Python widget kind supports a global generated-content hook."""

    generated = widget_css_capabilities()["generated_content"]
    return (
        part in generated["parts"]
        and python_kind not in generated["excluded_python_kinds"]
    )


@lru_cache(maxsize=1)
def supported_parts_by_python_kind() -> dict[str, set[str]]:
    """Return Python inline-style part validation data derived from the registry."""

    result: dict[str, set[str]] = {}
    for widget in widget_css_capabilities()["widgets"]:
        if widget.get("semantic_only", False):
            continue
        kind = widget.get("python_kind")
        if not isinstance(kind, str) or not kind:
            raise RuntimeError("widget CSS capability has no Python kind")
        if kind in result:
            raise RuntimeError(f"duplicate widget CSS Python kind {kind!r}")
        result[kind] = _part_names(widget)
    return result


def supported_parts_for_widget(public_type: str, python_kind: str) -> set[str]:
    """Return inherited and semantic parts for one public Python widget."""

    supported = set(supported_parts_by_python_kind().get(python_kind, set()))
    capability = capability_by_public_type().get(public_type)
    if capability is not None:
        supported.update(_part_names(capability))
    return supported


@lru_cache(maxsize=1)
def capability_by_public_type() -> dict[str, dict[str, Any]]:
    """Index capability records by stable public CSS type."""

    return {
        widget["public_type"]: widget
        for widget in widget_css_capabilities()["widgets"]
    }
