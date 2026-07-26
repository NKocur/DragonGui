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
    "examples/older/css_showcase.py",
    "examples/older/css_design_system_demo.py",
    "examples/older/all_features_css_demo.py",
    "examples/older/css_widget_parts_demo.py",
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
    "collapsible",
    "drag_source",
    "drop_target",
    "flow_layout",
    "grid_layout",
    "h_layout",
    "pane",
    "v_layout",
    "panel",
    "sidebar",
    "scroll_area",
    "splitter",
    "status_bar",
    "tabs",
    "tree_view",
    "pages",
    "page",
    "window",
}

OVERLAY_TYPES = {
    "context_menu",
    "menu_item",
    "modal",
    "toast",
    "tooltip",
}

INTERACTIVE_TYPES = {
    "arrow_button",
    "button",
    "checkbox",
    "code_editor",
    "collapsible",
    "dataframe_table",
    "drag_number",
    "drop_target",
    "dropdown",
    "icon_button",
    "image_button",
    "menu",
    "nav_item",
    "number_input",
    "radio_button",
    "range_slider",
    "selectable",
    "slider",
    "small_button",
    "tab",
    "text_area",
    "text_input",
    "toggle_switch",
    "tree_node",
}

LAYOUT_SCHEMA_VERSION = 1

USABILITY_ADVISORY_CODES = {
    "excessive-unused-flex-space",
    "interactive-content-too-small",
    "placeholder-truncated",
    "responsive-orphan",
    "scroll-viewport-too-small",
    "text-truncated",
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


def _layout_style(snapshot: dict[str, object], widget_id: str) -> dict[str, object]:
    style = _computed_style(snapshot, widget_id)
    layout = style.get("layout")
    return layout if isinstance(layout, dict) else {}


def _position(snapshot: dict[str, object], widget_id: str) -> str:
    value = _layout_style(snapshot, widget_id).get("position")
    return str(value).lower() if isinstance(value, str) else "static"


def _overflow(snapshot: dict[str, object], widget_id: str, widget_type: str, axis: str) -> str:
    layout = _layout_style(snapshot, widget_id)
    value = layout.get(f"overflow_{axis}", layout.get("overflow"))
    if isinstance(value, str):
        return value.lower()
    if widget_type == "scroll_area":
        return "auto" if axis == "y" else "hidden"
    if widget_type in {"panel", "modal"} and axis == "y":
        return "auto"
    return "visible"


def _scroll_range(layout: dict[str, object], widget_id: str, axis: str) -> float:
    values = layout.get(f"scroll_max_{axis}")
    if not isinstance(values, dict):
        return 0.0
    value = values.get(widget_id)
    if not isinstance(value, (int, float)):
        return 0.0
    return max(0.0, float(value))


def _props(node: dict[str, object]) -> dict[str, object]:
    value = node.get("props")
    return value if isinstance(value, dict) else {}


def _may_omit_layout(
    node: dict[str, object],
    parent: dict[str, object] | None,
    missing_ids: set[str],
) -> bool:
    widget_type = node.get("type")
    if widget_type in {"context_menu", "menu_item", "tab", "page", "toast", "tooltip"}:
        return True
    if widget_type == "modal" and not bool(_props(node).get("open")):
        return True
    if (
        parent is not None
        and parent.get("type") == "modal"
        and not bool(_props(parent).get("open"))
    ):
        return True
    # Tab headers remain in the layout for every tab while only the selected
    # tab's body children are laid out. Missing children beneath a retained tab
    # header therefore represent an inactive body, not missing geometry.
    if parent is not None and parent.get("type") == "tab":
        return True
    if parent is not None and parent.get("id") in missing_ids:
        return True
    return False


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


def _text_is_placeholder(node: dict[str, object]) -> bool:
    if node.get("type") != "text_input":
        return False
    props = _props(node)
    route_value = props.get("route_value")
    placeholder = props.get("placeholder")
    return not route_value and isinstance(placeholder, str) and bool(placeholder)


def _uses_explicit_ellipsis(snapshot: dict[str, object], widget_id: str) -> bool:
    text_style = _computed_style(snapshot, widget_id).get("text")
    return (
        isinstance(text_style, dict)
        and str(text_style.get("text_overflow", "")).lower() == "ellipsis"
    )


def _partition_layout_issues(issues: list[str]) -> tuple[list[str], list[str]]:
    structural: list[str] = []
    advisories: list[str] = []
    for issue in issues:
        code = issue.partition(":")[0]
        if code in USABILITY_ADVISORY_CODES:
            advisories.append(issue)
        else:
            structural.append(issue)
    return structural, advisories


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


def _axis_outside(
    child: dict[str, float],
    parent: dict[str, float],
    axis: str,
    tolerance: float,
) -> bool:
    origin = axis
    extent = "w" if axis == "x" else "h"
    return (
        child[origin] < parent[origin] - tolerance
        or child[origin] + child[extent] > parent[origin] + parent[extent] + tolerance
    )


def _intersection_size(
    first: dict[str, float], second: dict[str, float]
) -> tuple[float, float]:
    width = min(first["x"] + first["w"], second["x"] + second["w"]) - max(
        first["x"], second["x"]
    )
    height = min(first["y"] + first["h"], second["y"] + second["h"]) - max(
        first["y"], second["y"]
    )
    return max(0.0, width), max(0.0, height)


def _audit_layout(result: dict[str, object]) -> list[str]:
    gpu = result.get("debug_snapshot")
    if isinstance(gpu, dict):
        gpu = gpu.get("gpu")
    if not isinstance(gpu, dict):
        return ["missing debug_snapshot.gpu"]

    layout = gpu.get("layout")
    if not isinstance(layout, dict):
        return ["missing debug_snapshot.gpu.layout"]
    version = layout.get("schema_version")
    if version != LAYOUT_SCHEMA_VERSION:
        return [
            f"unsupported layout schema_version {version!r}; expected {LAYOUT_SCHEMA_VERSION}"
        ]
    tree = gpu.get("tree")
    rects = layout.get("rects")
    clips = layout.get("clips")
    diagnostics = layout.get("diagnostics")
    if not isinstance(tree, dict):
        return ["missing debug_snapshot.gpu.tree"]
    if not isinstance(rects, dict):
        return ["layout schema is missing rects"]
    if not isinstance(clips, dict):
        return ["layout schema is missing clips"]
    if not isinstance(diagnostics, dict):
        return ["layout schema is missing diagnostics"]

    issues: list[str] = []
    native_root_overflow_axes: set[tuple[str, str]] = set()
    for widget_id, diagnostic in diagnostics.items():
        if not isinstance(widget_id, str) or not isinstance(diagnostic, dict):
            continue
        native_issues = diagnostic.get("issues", [])
        if not isinstance(native_issues, list):
            continue
        for issue in native_issues:
            if not isinstance(issue, dict):
                continue
            message = issue.get("message")
            if isinstance(message, str) and message:
                issues.append(message)
            if issue.get("code") == "unreachable-root-overflow":
                axis = issue.get("axis")
                if axis in {"x", "y"}:
                    native_root_overflow_axes.add((widget_id, axis))

    missing_ids: set[str] = set()
    for node, parent in _walk_nodes(tree):
        widget_id = node.get("id")
        widget_type = node.get("type")
        if not isinstance(widget_id, str) or not isinstance(widget_type, str):
            continue
        node_rect = _rect(rects, widget_id)
        if node_rect is None:
            missing_ids.add(widget_id)
            if not _may_omit_layout(node, parent, missing_ids):
                issues.append(f"missing-geometry: {widget_id} {widget_type}")
            continue
        if not all(math.isfinite(value) for value in node_rect.values()):
            issues.append(f"negative-or-nonfinite-geometry: {widget_id} {widget_type}")
            continue
        if node_rect["w"] < -0.5 or node_rect["h"] < -0.5:
            issues.append(
                f"negative-or-nonfinite-geometry: {widget_id} {widget_type} {node_rect}"
            )

        if (
            widget_type in INTERACTIVE_TYPES
            and not bool(_props(node).get("disabled"))
            and node_rect["w"] > 0.5
            and node_rect["h"] > 0.5
            and (node_rect["w"] < 24.0 or node_rect["h"] < 20.0)
        ):
            issues.append(
                f"interactive-content-too-small: {widget_id} {widget_type} "
                f"has a {node_rect['w']:.1f}x{node_rect['h']:.1f}px content box"
            )

        scroll_ranges = {
            axis: _scroll_range(layout, widget_id, axis) for axis in ("x", "y")
        }
        for axis, viewport_extent in (("x", node_rect["w"]), ("y", node_rect["h"])):
            if (
                _overflow(gpu, widget_id, widget_type, axis) in {"auto", "scroll"}
                and scroll_ranges[axis] > 0.5
                and 0.5 < viewport_extent < 32.0
            ):
                issues.append(
                    f"scroll-viewport-too-small: {widget_id} {widget_type} "
                    f"has only {viewport_extent:.1f}px on its scrolling {axis}-axis"
                )

        children = node.get("children")
        if (
            parent is not None
            and widget_type not in OVERLAY_TYPES
            and widget_type not in {"separator", "spacer"}
            and isinstance(children, list)
            and any(isinstance(child, dict) for child in children)
        ):
            parent_id = parent.get("id")
            parent_type = parent.get("type")
            parent_rect = _rect(rects, parent_id) if isinstance(parent_id, str) else None
            starved_axis = None
            if (
                parent_type == "h_layout"
                and parent_rect is not None
                and parent_rect["w"] > 1.0
                and node_rect["w"] <= 0.5
            ):
                starved_axis = "x"
            elif (
                parent_type == "v_layout"
                and parent_rect is not None
                and parent_rect["h"] > 1.0
                and node_rect["h"] <= 0.5
            ):
                starved_axis = "y"
            if starved_axis is not None:
                issues.append(
                    f"starved-subtree: {widget_id} {widget_type} received zero usable "
                    f"{'width' if starved_axis == 'x' else 'height'} inside "
                    f"{parent_id} {parent_type}"
                )

        positioned = _position(gpu, widget_id)
        if (
            parent is not None
            and widget_type not in OVERLAY_TYPES
            and positioned not in {"absolute", "fixed"}
        ):
            parent_id = parent.get("id")
            parent_type = parent.get("type")
            if (
                isinstance(parent_id, str)
                and isinstance(parent_type, str)
                and parent_type in CONTAINER_TYPES
                and parent_type != "tabs"
            ):
                parent_rect = _rect(rects, parent_id)
                if parent_rect is not None and parent_rect["w"] > 0 and parent_rect["h"] > 0:
                    tolerance = 2.0
                    for axis in ("x", "y"):
                        if not _axis_outside(node_rect, parent_rect, axis, tolerance):
                            continue
                        if (parent_id, axis) in native_root_overflow_axes:
                            continue
                        overflow = _overflow(gpu, parent_id, parent_type, axis)
                        scroll_range = _scroll_range(layout, parent_id, axis)
                        if overflow == "visible":
                            issues.append(
                                f"unowned-overflow-{axis}: {widget_id} {widget_type} "
                                f"extends outside {parent_id} {parent_type}"
                            )
                        elif overflow in {"auto", "scroll"} and scroll_range <= tolerance:
                            issues.append(
                                f"missing-scroll-range-{axis}: {parent_id} {parent_type} "
                                f"clips {widget_id} {widget_type}"
                            )
                        elif overflow in {"hidden", "clip"}:
                            issues.append(
                                f"unreachable-content-{axis}: {parent_id} {parent_type} "
                                f"hides {widget_id} {widget_type}"
                            )

        text = _text_for_node(node)
        if not text or widget_type not in TEXT_WIDGET_TYPES or node_rect["w"] <= 0 or node_rect["h"] <= 0:
            continue
        if _uses_explicit_ellipsis(gpu, widget_id):
            continue

        font_size = _font_size(gpu, widget_id)
        line_height = _estimate_line_height(font_size)
        if widget_type not in {"panel", "sidebar", "modal"} and node_rect["h"] + 1.0 < line_height:
            issues.append(
                f"text-truncated: {widget_id} {widget_type} rect height {node_rect['h']:.1f}px "
                f"is below estimated line height {line_height:.1f}px"
            )

        if widget_type in {
            "label",
            "button",
            "tab",
            "nav_item",
            "menu",
            "menu_item",
            "text_input",
        }:
            estimated = _estimate_text_width(text, font_size)
            available = max(0.0, node_rect["w"] - 12.0)
            if estimated > available * 1.25 and len(text) <= 80:
                code = (
                    "placeholder-truncated"
                    if _text_is_placeholder(node)
                    else "text-truncated"
                )
                issues.append(
                    f"{code}: {widget_id} {widget_type} text may clip "
                    f"({estimated:.1f}px estimated into {available:.1f}px)"
                )

    for parent, _ in _walk_nodes(tree):
        parent_id = parent.get("id")
        parent_type = parent.get("type")
        children = parent.get("children")
        if (
            not isinstance(parent_id, str)
            or parent_type not in CONTAINER_TYPES
            or not isinstance(children, list)
        ):
            continue
        normal_children: list[tuple[str, str, dict[str, float]]] = []
        for child in children:
            if not isinstance(child, dict):
                continue
            child_id = child.get("id")
            child_type = child.get("type")
            if (
                not isinstance(child_id, str)
                or not isinstance(child_type, str)
                or child_type in OVERLAY_TYPES
                or _position(gpu, child_id) in {"absolute", "fixed"}
            ):
                continue
            child_rect = _rect(rects, child_id)
            if child_rect is not None and child_rect["w"] > 0 and child_rect["h"] > 0:
                normal_children.append((child_id, child_type, child_rect))

        if parent_type == "grid_layout" and len(normal_children) >= 4:
            row_origins: list[float] = []
            row_counts: list[int] = []
            for _, _, child_rect in sorted(
                normal_children, key=lambda item: (item[2]["y"], item[2]["x"])
            ):
                matching_row = next(
                    (
                        index
                        for index, row_y in enumerate(row_origins)
                        if abs(child_rect["y"] - row_y) <= 2.0
                    ),
                    None,
                )
                if matching_row is None:
                    row_origins.append(child_rect["y"])
                    row_counts.append(1)
                else:
                    row_counts[matching_row] += 1
            if (
                len(row_counts) >= 2
                and max(row_counts[:-1], default=0) >= 3
                and row_counts[-1] == 1
            ):
                issues.append(
                    f"responsive-orphan: {parent_id} {parent_type} ends with "
                    f"one item after rows of up to {max(row_counts[:-1])}"
                )

        parent_layout = _layout_style(gpu, parent_id)
        if (
            parent_type in {"panel", "sidebar"}
            and float(parent_layout.get("flex_grow", 0) or 0) > 0
            and normal_children
        ):
            parent_rect = _rect(rects, parent_id)
            if parent_rect is not None and parent_rect["h"] >= 120.0:
                content_top = min(child[2]["y"] for child in normal_children)
                content_bottom = max(
                    child[2]["y"] + child[2]["h"] for child in normal_children
                )
                used = max(0.0, content_bottom - content_top)
                if used < parent_rect["h"] * 0.3:
                    issues.append(
                        f"excessive-unused-flex-space: {parent_id} {parent_type} "
                        f"uses {used:.1f}px of a {parent_rect['h']:.1f}px flexible region"
                    )
        for index, (first_id, first_type, first_rect) in enumerate(normal_children):
            for second_id, second_type, second_rect in normal_children[index + 1 :]:
                overlap_w, overlap_h = _intersection_size(first_rect, second_rect)
                if overlap_w > 2.0 and overlap_h > 2.0:
                    issues.append(
                        f"sibling-overlap: {first_id} {first_type} and "
                        f"{second_id} {second_type} inside {parent_id} {parent_type}"
                    )

    return issues


def _audit_stylesheets(result: dict[str, object]) -> list[str]:
    snapshot = result.get("debug_snapshot")
    gpu = snapshot.get("gpu") if isinstance(snapshot, dict) else None
    stylesheets = gpu.get("stylesheets") if isinstance(gpu, dict) else None
    if not isinstance(stylesheets, dict):
        return []
    unmatched = stylesheets.get("unmatched_user_selectors")
    if not isinstance(unmatched, list):
        return []
    return [
        f"unmatched-selector: {selector}"
        for selector in unmatched
        if isinstance(selector, str) and selector
    ]


def _run_demo(
    repo_root: Path,
    demo: str,
    frames: int,
    audit_layout: bool,
    strict_audit: bool,
    strict_usability: bool,
    strict_css: bool,
) -> int:
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
    stylesheet_issues: list[str] = []
    if result is not None and audit_layout:
        layout_issues = _audit_layout(result)
    if result is not None:
        stylesheet_issues = _audit_stylesheets(result)
    structural_issues, usability_advisories = _partition_layout_issues(layout_issues)

    label = demo.replace("\\", "/")
    if (
        proc.returncode == 0
        and status == "ok"
        and (not strict_audit or not structural_issues)
        and (not strict_usability or not usability_advisories)
        and (not strict_css or not stylesheet_issues)
    ):
        print(
            f"OK   {label} renderer={renderer} frame_ms={frame_ms} upload_ms={upload_ms} "
            f"framework_rules={framework_rules} user_rules={user_rules} "
            f"warnings={warning_count} layout_issues={len(layout_issues)} "
            f"structural_errors={len(structural_issues)} "
            f"usability_advisories={len(usability_advisories)} "
            f"stylesheet_issues={len(stylesheet_issues)} last_error={last_error}"
        )
        for issue in layout_issues[:5]:
            print(f"     layout: {issue}")
        if len(layout_issues) > 5:
            print(f"     layout: ... {len(layout_issues) - 5} more")
        for issue in stylesheet_issues[:5]:
            print(f"     stylesheet: {issue}")
        if isinstance(stylesheets, dict):
            for warning in stylesheets.get("warnings", [])[:5]:
                if isinstance(warning, dict):
                    print(
                        "     css warning: "
                        f"{warning.get('property')}: {warning.get('message')}"
                    )
        if len(stylesheet_issues) > 5:
            print(f"     stylesheet: ... {len(stylesheet_issues) - 5} more")
        return 0

    print(
        f"FAIL {label} exit={proc.returncode} status={status} "
        f"layout_issues={len(layout_issues)} structural_errors={len(structural_issues)} "
        f"usability_advisories={len(usability_advisories)} "
        f"stylesheet_issues={len(stylesheet_issues)}"
    )
    for issue in layout_issues[:10]:
        print(f"     layout: {issue}")
    for issue in stylesheet_issues[:10]:
        print(f"     stylesheet: {issue}")
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
        help="Fail when layout sanity checks report structural errors.",
    )
    parser.add_argument(
        "--strict-usability",
        action="store_true",
        help="Fail when layout checks report usability advisories.",
    )
    parser.add_argument(
        "--strict-css",
        action="store_true",
        help="Fail when an active user stylesheet selector matches no nodes.",
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
            strict_usability=args.strict_usability,
            strict_css=args.strict_css,
        )
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
