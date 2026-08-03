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

    assert "Panel.fx-stage::body { padding: 36px; }" in module._LAB_EFFECTS

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

    swatch_ids = {
        "theme-swatch-accent",
        "theme-swatch-success",
        "theme-swatch-warning",
        "theme-swatch-danger",
        "theme-swatch-surface",
    }
    swatches = [node for node in nodes if node["id"] in swatch_ids]
    assert {node["id"] for node in swatches} == swatch_ids
    assert all(node["children"][0]["props"]["wrap"] is False for node in swatches)
    assert any(node["id"] == "theme-swatch-panel" for node in nodes)
    assert any(node["id"] == "theme-swatch-row" for node in nodes)
    assert any(node["id"] == "parts-panel-card" for node in nodes)
    assert any(node["id"] == "parts-panel-probe" for node in nodes)
    assert any(node["id"] == "parts-panel-scroll" for node in nodes)
    assert any(node["id"] == "parts-splitter-card" for node in nodes)
    assert any(node["id"] == "parts-splitter" for node in nodes)
    assert any(node["id"] == "parts-unsupported-card" for node in nodes)

    for stage in ("fx-dark", "fx-light"):
        expected_spacing = {
            f"{stage}-button-glows": 34,
            f"{stage}-indicator-glows": 28,
            f"{stage}-interaction-effects": 24,
        }
        for row_id, spacing in expected_spacing.items():
            row = next(node for node in nodes if node["id"] == row_id)
            assert row["default_style"]["gap"] == spacing
            assert row["default_style"]["row_gap"] == spacing

    flex_surfaces = [
        next(node for node in nodes if node["id"] == f"paint-flex-{index}")
        for index in range(4)
    ]
    initial_widths = [
        float(node["style"]["width"].removesuffix("%")) for node in flex_surfaces
    ]
    assert all(42.0 <= width <= 97.0 for width in initial_widths)

    # A displayed update advances by 0.09 radians regardless of how many
    # producer snapshots were coalesced. The maximum per-frame change stays
    # small enough that a busy stress frame cannot look like a full-width flash.
    phase = module.SAMPLE_COUNT * 0.09
    previous = module.resized_surface_widths(phase)
    for _ in range(240):
        phase += 0.09
        current = module.resized_surface_widths(phase)
        assert all(42.0 <= width <= 97.0 for width in current)
        assert max(abs(after - before) for before, after in zip(previous, current)) < 1.8
        previous = current
