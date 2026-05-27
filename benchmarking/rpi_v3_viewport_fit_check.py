from __future__ import annotations

import importlib.util
import json
import os
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
PYTHON_DIR = ROOT / "python"
DEMO_PATH = ROOT / "examples" / "all_features_v3_demo.py"


if str(PYTHON_DIR) not in sys.path:
    sys.path.insert(0, str(PYTHON_DIR))


def load_demo() -> Any:
    spec = importlib.util.spec_from_file_location("dragongui_v3_viewport_probe", DEMO_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"could not load {DEMO_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def walk(node: dict[str, Any]) -> list[dict[str, Any]]:
    items = [node]
    for child in node.get("children") or []:
        if isinstance(child, dict):
            items.extend(walk(child))
    return items


def rect_width(rect: dict[str, Any]) -> float:
    width = rect.get("width", rect.get("w"))
    if width is None:
        raise KeyError("rect has neither width nor w")
    return float(width)


def rect_height(rect: dict[str, Any]) -> float:
    height = rect.get("height", rect.get("h"))
    if height is None:
        raise KeyError("rect has neither height nor h")
    return float(height)


def rect_right(rect: dict[str, Any]) -> float:
    return float(rect["x"]) + rect_width(rect)


def rect_bottom(rect: dict[str, Any]) -> float:
    return float(rect["y"]) + rect_height(rect)


def main() -> int:
    os.environ.setdefault("DRAGONGUI_PROFILE", "pi")
    os.environ.setdefault("DRAGONGUI_WGPU_BACKEND", "gl")
    os.environ.setdefault("DRAGONGUI_WINDOW_BACKEND", "x11")
    os.environ.setdefault("DRAGONGUI_SMOKE_FRAMES", "3")
    expected_page = os.environ.get("DRAGONGUI_DEMO_PAGE", "overview")

    demo = load_demo()
    expected_window_w = float(demo.WINDOW_WIDTH)
    expected_window_h = float(demo.WINDOW_HEIGHT)

    try:
        result = demo.app.run_with_loading(
            demo.AllFeaturesV3,
            title="DragonGUI All Features V3 Demo",
            width=demo.WINDOW_WIDTH,
            height=demo.WINDOW_HEIGHT,
        )
    finally:
        demo.stats_stop.set()
        demo.stream_cancel.set()
        if getattr(demo, "stream_controller", None) is not None:
            demo.stream_controller.stop(timeout=0.25)

    snapshot = result.get("debug_snapshot")
    if not isinstance(snapshot, dict):
        raise RuntimeError("run result did not include a debug snapshot")

    gpu = snapshot.get("gpu") or {}
    tree = gpu.get("tree") or {}
    layout = gpu.get("layout") or {}
    rects = layout.get("rects") or {}
    clips = layout.get("clips") or {}
    scroll_max_y = layout.get("scroll_max_y") or {}
    scroll_max_x = layout.get("scroll_max_x") or {}
    nodes = walk(tree)
    by_id = {node.get("id"): node for node in nodes if isinstance(node.get("id"), str)}
    parent: dict[str, str] = {}
    for node in nodes:
        node_id = node.get("id")
        if not isinstance(node_id, str):
            continue
        for child in node.get("children") or []:
            child_id = child.get("id")
            if isinstance(child_id, str):
                parent[child_id] = node_id

    window = gpu.get("window") or {}
    window_w = float(window.get("width", 0.0) or 0.0)
    window_h = float(window.get("height", 0.0) or 0.0)
    failures: list[str] = []

    def rect_for(node_id: str) -> dict[str, Any]:
        rect = rects.get(node_id)
        if not isinstance(rect, dict):
            raise RuntimeError(f"missing layout rect for {node_id}")
        return rect

    def clip_for(node_id: str) -> dict[str, Any]:
        clip = clips.get(node_id)
        if isinstance(clip, dict):
            return clip
        return rect_for(node_id)

    main_pages = next(
        (
            node
            for node in nodes
            if node.get("type") == "pages"
            and (node.get("props") or {}).get("route_value") == expected_page
        ),
        None,
    )
    if main_pages is None:
        raise RuntimeError("V3 snapshot did not include the main Pages widget")
    pages_id = main_pages["id"]
    pages_rect = rect_for(pages_id)
    pages_clip = clip_for(pages_id)

    active_page = next(
        (
            node
            for node in nodes
            if node.get("type") == "page"
            and (node.get("props") or {}).get("route_value") == expected_page
            and node.get("id") in rects
        ),
        None,
    )
    if active_page is None:
        raise RuntimeError(f"V3 snapshot did not include the active {expected_page} Page")
    page_id = active_page["id"]
    page_rect = rect_for(page_id)
    page_clip = clip_for(page_id)
    page_children = [child for child in active_page.get("children") or [] if child.get("id") in rects]

    child_bottom = max((rect_bottom(rect_for(child["id"])) for child in page_children), default=0.0)
    child_right = max((rect_right(rect_for(child["id"])) for child in page_children), default=0.0)
    page_scroll_y = float(scroll_max_y.get(page_id, 0.0) or 0.0)

    if window_w > expected_window_w + 0.5 or window_h > expected_window_h + 0.5:
        failures.append(
            "window inner size exceeds requested logical size "
            f"{expected_window_w:.0f}x{expected_window_h:.0f}: "
            f"got {window_w:.0f}x{window_h:.0f}"
        )
    if rect_right(pages_clip) > window_w + 0.5 or rect_bottom(pages_clip) > window_h + 0.5:
        failures.append(f"main pages clip exceeds window: pages_clip={pages_clip} window={window}")
    if rect_bottom(page_clip) > window_h + 0.5:
        failures.append(f"active page clip exceeds window: page_clip={page_clip} window={window}")
    if child_bottom > rect_bottom(page_clip) + 0.5 and page_scroll_y <= 0.0:
        failures.append(
            f"active {expected_page} Page content overflows vertically but has no scroll range: "
            f"child_bottom={child_bottom:.1f} page_bottom={rect_bottom(page_clip):.1f}"
        )
    if child_right > rect_right(page_clip) + 0.5:
        failures.append(
            f"active {expected_page} Page content overflows horizontally: "
            f"child_right={child_right:.1f} page_right={rect_right(page_clip):.1f}"
        )

    def scroll_ancestor_between(node_id: str, stop_id: str, axis: str) -> str | None:
        current = parent.get(node_id)
        while current is not None:
            scroll_map = scroll_max_y if axis == "y" else scroll_max_x
            if float(scroll_map.get(current, 0.0) or 0.0) > 0.0:
                return current
            if current == stop_id:
                return None
            current = parent.get(current)
        return None

    filtered_descendants = []
    for node in nodes:
        node_id = node.get("id")
        if not isinstance(node_id, str) or node_id == page_id or node_id not in rects:
            continue
        current = parent.get(node["id"])
        while current is not None and current != page_id:
            current = parent.get(current)
        if current == page_id:
            filtered_descendants.append(node)

    descendant_overflows: list[dict[str, Any]] = []
    for node in filtered_descendants:
        node_id = node["id"]
        rect = rect_for(node_id)
        over_y = rect_bottom(rect) > rect_bottom(page_clip) + 0.5
        over_x = rect_right(rect) > rect_right(page_clip) + 0.5
        if over_y and scroll_ancestor_between(node_id, page_id, "y") is None:
            descendant_overflows.append(
                {
                    "id": node_id,
                    "type": node.get("type"),
                    "axis": "y",
                    "rect": rect,
                }
            )
        if over_x and scroll_ancestor_between(node_id, page_id, "x") is None:
            descendant_overflows.append(
                {
                    "id": node_id,
                    "type": node.get("type"),
                    "axis": "x",
                    "rect": rect,
                }
            )
    if descendant_overflows:
        failures.append(
            "active page has descendants outside the page clip with no scroll ancestor"
        )

    top_level = []
    root_id = tree.get("id")
    root_children = tree.get("children") if isinstance(tree.get("children"), list) else []
    for child in root_children:
        child_id = child.get("id")
        if not isinstance(child_id, str) or child_id not in rects:
            continue
        rect = rect_for(child_id)
        top_level.append(
            {
                "id": child_id,
                "type": child.get("type"),
                "rect": rect,
                "clip": clip_for(child_id),
            }
        )
        if rect_bottom(rect) > window_h + 0.5:
            failures.append(f"top-level {child.get('type')} {child_id} exceeds window height")

    summary = {
        "status": "ok" if not failures else "failed",
        "window": window,
        "expected_window": {"width": expected_window_w, "height": expected_window_h},
        "root_id": root_id,
        "main_pages": {
            "id": pages_id,
            "rect": pages_rect,
            "clip": pages_clip,
            "scroll_max_y": float(scroll_max_y.get(pages_id, 0.0) or 0.0),
        },
        "active_page": {
            "id": page_id,
            "expected_route": expected_page,
            "route": (by_id.get(page_id, {}).get("props") or {}).get("route_value"),
            "rect": page_rect,
            "clip": page_clip,
            "scroll_max_y": page_scroll_y,
            "child_right": child_right,
            "child_bottom": child_bottom,
            "unscrolled_descendant_overflows": descendant_overflows[:12],
        },
        "top_level": top_level,
        "failures": failures,
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
