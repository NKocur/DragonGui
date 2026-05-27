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
    spec = importlib.util.spec_from_file_location("dragongui_v3_demo_probe", DEMO_PATH)
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


def rect_right(rect: dict[str, Any]) -> float:
    width = rect.get("width", rect.get("w"))
    if width is None:
        raise KeyError("rect has neither width nor w")
    return float(rect["x"]) + float(width)


def main() -> int:
    os.environ.setdefault("DRAGONGUI_PROFILE", "pi")
    os.environ.setdefault("DRAGONGUI_WGPU_BACKEND", "gl")
    os.environ.setdefault("DRAGONGUI_WINDOW_BACKEND", "x11")
    os.environ.setdefault("DRAGONGUI_SMOKE_FRAMES", "3")

    demo = load_demo()
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
    scroll_max_y = layout.get("scroll_max_y") or {}
    nodes = walk(tree)

    sidebar = next((node for node in nodes if node.get("type") == "sidebar"), None)
    if sidebar is None:
        raise RuntimeError("V3 snapshot did not include a Sidebar")
    sidebar_id = sidebar["id"]
    sidebar_rect = rects.get(sidebar_id)
    if not isinstance(sidebar_rect, dict):
        raise RuntimeError("V3 snapshot did not include Sidebar layout rect")

    badge_labels = {"Grid", "Scatter3D", "LinePlot", "Histogram", "HtmlReport"}
    badge_nodes = [
        node
        for node in nodes
        if node.get("type") in {"badge", "tag"}
        and (node.get("props") or {}).get("text") in badge_labels
    ]
    if len(badge_nodes) != len(badge_labels):
        found = sorted((node.get("props") or {}).get("text") for node in badge_nodes)
        raise RuntimeError(f"expected sidebar badges {sorted(badge_labels)}, found {found}")

    # This mirrors the default library gutter reserve: 4px track, 8px edge pad,
    # and 8px content gap. Styled scrollbars may reserve more.
    min_gutter = 20.0
    allowed_right = rect_right(sidebar_rect) - min_gutter + 0.5
    failures: list[str] = []
    badge_summary: dict[str, Any] = {}
    for node in badge_nodes:
        label = (node.get("props") or {}).get("text")
        rect = rects.get(node["id"])
        if not isinstance(rect, dict):
            failures.append(f"{label}: missing layout rect")
            continue
        right = rect_right(rect)
        badge_summary[str(label)] = rect
        if right > allowed_right:
            failures.append(
                f"{label}: right edge {right:.1f} exceeds gutter-safe right {allowed_right:.1f}"
            )

    sidebar_scroll = float(scroll_max_y.get(sidebar_id, 0.0) or 0.0)
    if sidebar_scroll <= 0.0:
        failures.append("sidebar has no vertical scroll range")

    summary = {
        "status": "ok" if not failures else "failed",
        "window": gpu.get("window"),
        "sidebar_id": sidebar_id,
        "sidebar_rect": sidebar_rect,
        "sidebar_scroll_max_y": sidebar_scroll,
        "gutter_safe_right": allowed_right,
        "badges": badge_summary,
        "failures": failures,
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
