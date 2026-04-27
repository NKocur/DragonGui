from __future__ import annotations

import argparse
import ast
import math
import os
import re
import subprocess
import sys
from pathlib import Path


DEFAULT_DEMOS = (
    "examples/css_showcase.py",
    "examples/css_design_system_demo.py",
    "examples/all_features_css_demo.py",
    "examples/css_widget_parts_demo.py",
)

TEXT_WIDGET_TYPES = {
    "button",
    "checkbox",
    "dropdown",
    "label",
    "menu",
    "menu_item",
    "modal",
    "nav_item",
    "number_input",
    "panel",
    "progress_bar",
    "sidebar",
    "tab",
    "text_input",
}

CONTAINER_TYPES = {
    "h_layout",
    "v_layout",
    "panel",
    "sidebar",
    "status_bar",
    "tabs",
    "pages",
    "page",
    "window",
}

OVERLAY_TYPES = {
    "context_menu",
    "menu_item",
    "modal",
}


def _first_match(pattern: str, text: str, default: str = "-") -> str:
    match = re.search(pattern, text)
    return match.group(1) if match else default


def _last_output_line(text: str, limit: int = 8) -> str:
    lines = [line for line in text.splitlines() if line.strip()]
    if not lines:
        return ""
    return "\n".join(lines[-limit:])


def _parse_result(output: str) -> dict[str, object] | None:
    for line in output.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            result = ast.literal_eval(line)
        except (SyntaxError, ValueError):
            continue
        if isinstance(result, dict) and result.get("status"):
            return result
    return None


def _rect(layout: dict[str, object], widget_id: str) -> dict[str, float] | None:
    value = layout.get(widget_id)
    if not isinstance(value, dict):
        return None
    try:
        return {
            "x": float(value["x"]),
            "y": float(value["y"]),
            "w": float(value["w"]),
            "h": float(value["h"]),
        }
    except (KeyError, TypeError, ValueError):
        return None


def _walk_nodes(node: dict[str, object], parent: dict[str, object] | None = None):
    yield node, parent
    children = node.get("children")
    if isinstance(children, list):
        for child in children:
            if isinstance(child, dict):
                yield from _walk_nodes(child, node)


def _computed_style(snapshot: dict[str, object], widget_id: str) -> dict[str, object]:
    computed = snapshot.get("computed_styles")
    if not isinstance(computed, dict):
        return {}
    entry = computed.get(widget_id)
    if not isinstance(entry, dict):
        return {}
    style = entry.get("style")
    return style if isinstance(style, dict) else {}


def _font_size(snapshot: dict[str, object], widget_id: str) -> float:
    style = _computed_style(snapshot, widget_id)
    text = style.get("text")
    if isinstance(text, dict) and isinstance(text.get("font_size"), (int, float)):
        return float(text["font_size"])
    theme = snapshot.get("theme")
    if isinstance(theme, dict) and isinstance(theme.get("font_size"), (int, float)):
        return float(theme["font_size"])
    return 13.0


def _text_for_node(node: dict[str, object]) -> str | None:
    props = node.get("props")
    if not isinstance(props, dict):
        return None
    text = props.get("text")
    if isinstance(text, str) and text:
        return text
    if node.get("type") == "text_input":
        route_value = props.get("route_value")
        if isinstance(route_value, str) and route_value:
            return route_value
        placeholder = props.get("placeholder")
        if isinstance(placeholder, str) and placeholder:
            return placeholder
    return None


def _char_width(ch: str, font_size: float) -> float:
    if ch.isspace():
        factor = 0.32
    elif ch in "ilI1|.,:;!'`":
        factor = 0.30
    elif ch in "MW@#%&":
        factor = 0.82
    elif ch.isupper():
        factor = 0.64
    else:
        factor = 0.54
    return font_size * factor


def _estimate_text_width(text: str, font_size: float) -> float:
    return sum(_char_width(ch, font_size) for ch in text)


def _estimate_line_height(font_size: float) -> float:
    return max(font_size + 5.0, font_size * 1.25)


def _audit_layout(result: dict[str, object]) -> list[str]:
    gpu = result.get("debug_snapshot")
    if isinstance(gpu, dict):
        gpu = gpu.get("gpu")
    if not isinstance(gpu, dict):
        return ["missing debug_snapshot.gpu"]

    tree = gpu.get("tree")
    layout = gpu.get("layout")
    if not isinstance(tree, dict) or not isinstance(layout, dict):
        return ["missing widget tree or layout rects"]

    issues: list[str] = []
    for node, parent in _walk_nodes(tree):
        widget_id = node.get("id")
        widget_type = node.get("type")
        if not isinstance(widget_id, str) or not isinstance(widget_type, str):
            continue
        node_rect = _rect(layout, widget_id)
        if node_rect is None:
            continue
        if not all(math.isfinite(value) for value in node_rect.values()):
            issues.append(f"{widget_id} {widget_type}: non-finite layout rect")
            continue
        if node_rect["w"] < -0.5 or node_rect["h"] < -0.5:
            issues.append(f"{widget_id} {widget_type}: negative layout size {node_rect}")

        if parent is not None and widget_type not in OVERLAY_TYPES:
            parent_id = parent.get("id")
            parent_type = parent.get("type")
            if (
                isinstance(parent_id, str)
                and isinstance(parent_type, str)
                and parent_type in CONTAINER_TYPES
                and parent_type != "tabs"
            ):
                parent_rect = _rect(layout, parent_id)
                if parent_rect is not None and parent_rect["w"] > 0 and parent_rect["h"] > 0:
                    tolerance = 2.0
                    outside = (
                        node_rect["x"] < parent_rect["x"] - tolerance
                        or node_rect["y"] < parent_rect["y"] - tolerance
                        or node_rect["x"] + node_rect["w"] > parent_rect["x"] + parent_rect["w"] + tolerance
                        or node_rect["y"] + node_rect["h"] > parent_rect["y"] + parent_rect["h"] + tolerance
                    )
                    if outside:
                        issues.append(
                            f"{widget_id} {widget_type}: rect extends outside parent "
                            f"{parent_id} {parent_type}"
                        )

        text = _text_for_node(node)
        if not text or widget_type not in TEXT_WIDGET_TYPES or node_rect["w"] <= 0 or node_rect["h"] <= 0:
            continue

        font_size = _font_size(gpu, widget_id)
        line_height = _estimate_line_height(font_size)
        if widget_type not in {"panel", "sidebar", "modal"} and node_rect["h"] + 1.0 < line_height:
            issues.append(
                f"{widget_id} {widget_type}: rect height {node_rect['h']:.1f}px "
                f"is below estimated line height {line_height:.1f}px"
            )

        if widget_type in {"label", "button", "tab", "nav_item", "menu", "menu_item"}:
            estimated = _estimate_text_width(text, font_size)
            available = max(0.0, node_rect["w"] - 12.0)
            if estimated > available * 1.25 and len(text) <= 80:
                issues.append(
                    f"{widget_id} {widget_type}: text may clip "
                    f"({estimated:.1f}px estimated into {available:.1f}px)"
                )

    return issues


def _run_demo(repo_root: Path, demo: str, frames: int, audit_layout: bool, strict_audit: bool) -> int:
    env = os.environ.copy()
    python_path = str(repo_root / "python")
    if env.get("PYTHONPATH"):
        env["PYTHONPATH"] = python_path + os.pathsep + env["PYTHONPATH"]
    else:
        env["PYTHONPATH"] = python_path
    env["DRAGONGUI_SMOKE_FRAMES"] = str(frames)

    proc = subprocess.run(
        [sys.executable, demo],
        cwd=repo_root,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        errors="replace",
        check=False,
    )

    output = proc.stdout + "\n" + proc.stderr
    result = _parse_result(output)
    if result is not None:
        snapshot = result.get("debug_snapshot")
        gpu = snapshot.get("gpu") if isinstance(snapshot, dict) else {}
        stylesheets = gpu.get("stylesheets") if isinstance(gpu, dict) else {}
        status = str(result.get("status", "-"))
        renderer = str(result.get("renderer", "-"))
        frame_ms = str(result.get("frame_ms", "-"))
        upload_ms = str(result.get("upload_ms", "-"))
        framework_rules = str(stylesheets.get("framework_rules", "-")) if isinstance(stylesheets, dict) else "-"
        user_rules = str(stylesheets.get("user_rules", "-")) if isinstance(stylesheets, dict) else "-"
        warning_count = str(stylesheets.get("warning_count", "-")) if isinstance(stylesheets, dict) else "-"
        last_error = str(stylesheets.get("last_error", "-")) if isinstance(stylesheets, dict) else "-"
    else:
        status = _first_match(r"'status': '([^']+)'", output)
        renderer = _first_match(r"'renderer': '([^']+)'", output)
        frame_ms = _first_match(r"'frame_ms': ([0-9.]+)", output)
        upload_ms = _first_match(r"'upload_ms': ([0-9.]+)", output)
        framework_rules = _first_match(r"'framework_rules': ([0-9]+)", output)
        user_rules = _first_match(r"'user_rules': ([0-9]+)", output)
        warning_count = _first_match(r"'warning_count': ([0-9]+)", output)
        last_error = _first_match(r"'last_error': ([^,}]+)", output)

    layout_issues: list[str] = []
    if result is not None and audit_layout:
        layout_issues = _audit_layout(result)

    label = demo.replace("\\", "/")
    if proc.returncode == 0 and status == "ok" and (not strict_audit or not layout_issues):
        print(
            f"OK   {label} renderer={renderer} frame_ms={frame_ms} upload_ms={upload_ms} "
            f"framework_rules={framework_rules} user_rules={user_rules} "
            f"warnings={warning_count} layout_issues={len(layout_issues)} last_error={last_error}"
        )
        for issue in layout_issues[:5]:
            print(f"     layout: {issue}")
        if len(layout_issues) > 5:
            print(f"     layout: ... {len(layout_issues) - 5} more")
        return 0

    print(f"FAIL {label} exit={proc.returncode} status={status} layout_issues={len(layout_issues)}")
    for issue in layout_issues[:10]:
        print(f"     layout: {issue}")
    tail = _last_output_line(output)
    if tail:
        print(tail)
    return 1


def main() -> int:
    parser = argparse.ArgumentParser(description="Run DragonGUI CSS demos in smoke mode.")
    parser.add_argument(
        "demos",
        nargs="*",
        default=DEFAULT_DEMOS,
        help="Demo scripts to run. Defaults to all CSS demos.",
    )
    parser.add_argument(
        "--frames",
        type=int,
        default=3,
        help="Number of smoke frames to render per demo.",
    )
    parser.add_argument(
        "--no-layout-audit",
        action="store_true",
        help="Skip debug-snapshot layout sanity checks.",
    )
    parser.add_argument(
        "--strict-layout",
        action="store_true",
        help="Fail when layout sanity checks report issues.",
    )
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[1]
    failures = 0
    for demo in args.demos:
        failures += _run_demo(
            repo_root,
            demo,
            args.frames,
            audit_layout=not args.no_layout_audit,
            strict_audit=args.strict_layout,
        )
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
