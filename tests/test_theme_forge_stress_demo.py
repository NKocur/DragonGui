from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from types import ModuleType
from typing import Any, Iterator


ROOT = Path(__file__).resolve().parents[1]
DEMO_PATH = ROOT / "examples" / "theme_forge_stress_demo.py"


def _load_demo(name: str) -> ModuleType:
    spec = importlib.util.spec_from_file_location(name, DEMO_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def _walk(node: dict[str, Any]) -> Iterator[dict[str, Any]]:
    yield node
    for child in node.get("children", []):
        yield from _walk(child)


def test_theme_forge_builds_all_pages_and_named_stylesheets_headlessly() -> None:
    module = _load_demo("theme_forge_stress_demo_build_test")

    app, window = module.build_app(
        "modern-dark",
        rows=12,
        decorations="client",
        long_title=True,
    )
    document = app.document(window)
    nodes = list(_walk(document["window"]))

    assert document["window"]["props"]["title"] == module.LONG_TITLE
    assert document["window"]["props"]["decorations"] == "client"
    assert [sheet["id"] for sheet in document["stylesheets"]] == [
        "variables",
        "structure",
        "appearance",
    ]

    expected_routes = [route for route, _label, _badge in module.ROUTES]
    pages = [node for node in nodes if node["type"] == "page"]
    assert [page["props"]["value"] for page in pages] == expected_routes
    assert len(pages) == 12

    title = next(node for node in nodes if node["id"] == "forge-window--dg-window-title")
    assert title["props"]["wrap"] is False
    assert title["default_style"] == {
        "width": 0,
        "flex_grow": 1,
        "flex_shrink": 1,
        "min_width": 0,
        "height": 34,
        "padding_left": 12,
        "padding_right": 8,
        "overflow": "hidden",
        "text_overflow": "ellipsis",
    }
    for suffix in ("minimize", "maximize", "close"):
        control = next(
            node for node in nodes if node["id"] == f"forge-window--dg-window-{suffix}"
        )
        assert control["type"] == "icon_button"
        assert control["default_style"]["flex_shrink"] == 0

    assert [
        next(
            node
            for node in nodes
            if node["id"] == f"forge-window--dg-window-{suffix}"
        )["props"]["icon"]
        for suffix in ("minimize", "maximize", "close")
    ] == ["minus", "stop", "close"]

    for search_id in ("gallery-search-clearable", "gallery-search-preset"):
        search = next(node for node in nodes if node["id"] == search_id)
        assert search["default_style"]["min_height"] == 38

    mini_titlebar = next(
        node for node in nodes if "mini-titlebar" in node.get("class", "").split()
    )
    assert [child["type"] for child in mini_titlebar["children"][-3:]] == [
        "icon_button",
        "icon_button",
        "icon_button",
    ]

    thrash_targets = [
        next(node for node in nodes if node["id"] == f"extreme-thrash-target-{index}")
        for index in range(6)
    ]
    assert all(node["class"] == "xt-thrash-target" for node in thrash_targets)
    assert all(node["children"][0]["props"]["wrap"] is False for node in thrash_targets)
    assert any(node["id"] == "extreme-hostile-grid" for node in nodes)
    assert any(node["id"] == "extreme-malformed-panel" for node in nodes)
