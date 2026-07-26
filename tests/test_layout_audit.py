from __future__ import annotations

import importlib.util
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
SMOKE_AUDIT_PATH = ROOT / "tools" / "smoke_css_demos.py"


def load_smoke_audit():
    spec = importlib.util.spec_from_file_location("smoke_css_demos_tool", SMOKE_AUDIT_PATH)
    assert spec is not None
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def rect(x: float, y: float, w: float, h: float) -> dict[str, float]:
    return {"x": x, "y": y, "w": w, "h": h}


def node(
    widget_id: str,
    widget_type: str,
    *,
    children: list[dict[str, object]] | None = None,
    props: dict[str, object] | None = None,
) -> dict[str, object]:
    return {
        "id": widget_id,
        "type": widget_type,
        "props": props or {},
        "children": children or [],
    }


def result(
    tree: dict[str, object],
    rects: dict[str, dict[str, float]],
    *,
    styles: dict[str, dict[str, object]] | None = None,
    text_styles: dict[str, dict[str, object]] | None = None,
    scroll_max_x: dict[str, float] | None = None,
    scroll_max_y: dict[str, float] | None = None,
    diagnostics: dict[str, dict[str, object]] | None = None,
    schema_version: int | None = 1,
) -> dict[str, object]:
    layout: dict[str, object] = {
        "rects": rects,
        "clips": dict(rects),
        "diagnostics": diagnostics or {widget_id: {} for widget_id in rects},
        "scroll_max_x": scroll_max_x or {},
        "scroll_max_y": scroll_max_y or {},
    }
    if schema_version is not None:
        layout["schema_version"] = schema_version
    computed_styles: dict[str, dict[str, object]] = {}
    for widget_id in set(styles or {}) | set(text_styles or {}):
        computed_styles[widget_id] = {
            "style": {
                "layout": (styles or {}).get(widget_id, {}),
                "text": (text_styles or {}).get(widget_id, {}),
            }
        }
    return {
        "debug_snapshot": {
            "gpu": {
                "tree": tree,
                "layout": layout,
                "computed_styles": computed_styles,
                "theme": {"font_size": 13},
            }
        }
    }


def test_layout_audit_rejects_missing_schema_version() -> None:
    audit = load_smoke_audit()
    tree = node("window", "window")

    issues = audit._audit_layout(
        result(tree, {"window": rect(0, 0, 320, 200)}, schema_version=None)
    )

    assert issues == ["unsupported layout schema_version None; expected 1"]


def test_layout_audit_reads_rects_from_current_nested_schema() -> None:
    audit = load_smoke_audit()
    flow = node("flow", "flow_layout")
    label = node("label", "label", children=[])
    row = node("row", "h_layout", children=[label, flow])
    tree = node("window", "window", children=[row])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 360, 200),
                "row": rect(0, 0, 360, 40),
                "label": rect(0, 0, 180, 40),
                "flow": rect(180, 0, 360, 40),
            },
        )
    )

    assert any(issue.startswith("unowned-overflow-x: flow") for issue in issues)


def test_layout_audit_reports_visible_starved_subtree() -> None:
    audit = load_smoke_audit()
    sidebar = node("sidebar", "sidebar")
    workbench = node(
        "workbench",
        "v_layout",
        children=[node("required-content", "panel")],
    )
    shell = node("shell", "h_layout", children=[sidebar, workbench])
    tree = node("window", "window", children=[shell])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 390, 720),
                "shell": rect(0, 0, 390, 720),
                "sidebar": rect(0, 0, 390, 720),
                "workbench": rect(390, 0, 0, 720),
            },
        )
    )

    assert any(issue.startswith("starved-subtree: workbench v_layout") for issue in issues)


def test_layout_audit_surfaces_native_semantic_diagnostics_without_duplicate_root_issue() -> None:
    audit = load_smoke_audit()
    wide = node("wide", "panel")
    tree = node("window", "window", children=[wide])
    message = (
        "unreachable-root-overflow-x: window window clips wide panel "
        "with no scroll owner"
    )

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 200, 120),
                "wide": rect(0, 0, 360, 40),
            },
            diagnostics={
                "window": {
                    "issues": [
                        {
                            "code": "unreachable-root-overflow",
                            "axis": "x",
                            "subject_id": "wide",
                            "message": message,
                        }
                    ]
                },
                "wide": {"issues": []},
            },
        )
    )

    assert issues == [message]


def test_layout_audit_partitions_structural_errors_from_usability_advisories() -> None:
    audit = load_smoke_audit()
    field = node(
        "query",
        "text_input",
        props={"placeholder": "Search an exceptionally long project name"},
    )
    tree = node("window", "window", children=[field])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 320, 200),
                "query": rect(0, 0, 80, 28),
            },
        )
    )
    structural, advisories = audit._partition_layout_issues(issues)

    assert structural == []
    assert len(advisories) == 1
    assert advisories[0].startswith("placeholder-truncated: query text_input")


def test_search_box_inner_input_emits_placeholder_truncation_advisory() -> None:
    audit = load_smoke_audit()
    search_input = node(
        "search-input",
        "text_input",
        props={"placeholder": "Search routes, owners, commands, and deployments"},
    )
    search = node("search", "h_layout", children=[search_input])
    tree = node("window", "window", children=[search])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 220, 120),
                "search": rect(0, 0, 180, 38),
                "search-input": rect(34, 5, 106, 28),
            },
        )
    )
    structural, advisories = audit._partition_layout_issues(issues)

    assert structural == []
    assert any(
        issue.startswith("placeholder-truncated: search-input text_input")
        for issue in advisories
    )


def test_layout_audit_exempts_explicit_ellipsis_from_truncation_advisory() -> None:
    audit = load_smoke_audit()
    label = node(
        "route",
        "label",
        props={"text": "A deliberately ellipsized navigation route"},
    )
    tree = node("window", "window", children=[label])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 320, 200),
                "route": rect(0, 0, 70, 24),
            },
            text_styles={"route": {"text_overflow": "ellipsis"}},
        )
    )

    assert issues == []


def test_layout_audit_reports_small_control_and_scroll_viewport_advisories() -> None:
    audit = load_smoke_audit()
    button = node("tiny", "button")
    content = node("content", "panel")
    scroller = node("scroller", "scroll_area", children=[content])
    tree = node("window", "window", children=[button, scroller])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 320, 200),
                "tiny": rect(0, 0, 18, 18),
                "scroller": rect(0, 30, 100, 20),
                "content": rect(0, 30, 100, 60),
            },
            styles={"scroller": {"overflow_y": "auto", "overflow_x": "hidden"}},
            scroll_max_y={"scroller": 40},
        )
    )

    assert any(
        issue.startswith("interactive-content-too-small: tiny button")
        for issue in issues
    )
    assert any(
        issue.startswith("scroll-viewport-too-small: scroller scroll_area")
        for issue in issues
    )
    assert audit._partition_layout_issues(issues)[0] == []


def test_layout_audit_reports_grid_orphan_and_unused_flex_advisories() -> None:
    audit = load_smoke_audit()
    cards = [node(f"card-{index}", "panel") for index in range(4)]
    grid = node("metrics", "grid_layout", children=cards)
    summary = node("summary", "label")
    flexible_panel = node("flex-panel", "panel", children=[summary])
    tree = node("window", "window", children=[grid, flexible_panel])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 400, 400),
                "metrics": rect(0, 0, 300, 100),
                "card-0": rect(0, 0, 90, 40),
                "card-1": rect(100, 0, 90, 40),
                "card-2": rect(200, 0, 90, 40),
                "card-3": rect(0, 50, 90, 40),
                "flex-panel": rect(0, 120, 300, 200),
                "summary": rect(0, 120, 300, 20),
            },
            styles={"flex-panel": {"flex_grow": 1}},
        )
    )

    assert any(
        issue.startswith("responsive-orphan: metrics grid_layout") for issue in issues
    )
    assert any(
        issue.startswith("excessive-unused-flex-space: flex-panel panel")
        for issue in issues
    )
    assert audit._partition_layout_issues(issues)[0] == []


def test_stylesheet_audit_surfaces_unmatched_user_selectors() -> None:
    audit = load_smoke_audit()
    snapshot = result(node("window", "window"), {"window": rect(0, 0, 320, 200)})
    snapshot["debug_snapshot"]["gpu"]["stylesheets"] = {
        "user_selector_matches": {
            "SearchBox.command-search": 1,
            "MisspelledWidget": 0,
        },
        "unmatched_user_selectors": ["MisspelledWidget"],
    }

    assert audit._audit_stylesheets(snapshot) == [
        "unmatched-selector: MisspelledWidget"
    ]


def test_layout_audit_accepts_owned_scroll_overflow() -> None:
    audit = load_smoke_audit()
    content = node("content", "v_layout")
    panel = node("panel", "panel", children=[content])
    tree = node("window", "window", children=[panel])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 320, 240),
                "panel": rect(0, 0, 200, 100),
                "content": rect(0, 0, 200, 160),
            },
            styles={"panel": {"overflow_y": "auto", "overflow_x": "hidden"}},
            scroll_max_y={"panel": 60},
        )
    )

    assert issues == []


def test_layout_audit_reports_missing_scroll_range() -> None:
    audit = load_smoke_audit()
    content = node("content", "v_layout")
    scroller = node("scroller", "scroll_area", children=[content])
    tree = node("window", "window", children=[scroller])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 320, 240),
                "scroller": rect(0, 0, 200, 100),
                "content": rect(0, 0, 200, 160),
            },
            styles={"scroller": {"overflow_y": "auto", "overflow_x": "hidden"}},
        )
    )

    assert issues == [
        "missing-scroll-range-y: scroller scroll_area clips content v_layout"
    ]


def test_layout_audit_reports_hidden_unreachable_content() -> None:
    audit = load_smoke_audit()
    content = node("content", "grid_layout")
    shell = node("shell", "h_layout", children=[content])
    tree = node("window", "window", children=[shell])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 320, 240),
                "shell": rect(0, 0, 200, 100),
                "content": rect(0, 0, 260, 100),
            },
            styles={"shell": {"overflow_x": "hidden", "overflow_y": "hidden"}},
        )
    )

    assert issues == [
        "unreachable-content-x: shell h_layout hides content grid_layout"
    ]


def test_layout_audit_allows_inactive_page_subtree_to_omit_geometry() -> None:
    audit = load_smoke_audit()
    hidden_child = node("hidden-child", "panel")
    inactive = node("inactive-page", "page", children=[hidden_child])
    pages = node("pages", "pages", children=[inactive])
    tree = node("window", "window", children=[pages])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 320, 240),
                "pages": rect(0, 0, 320, 240),
            },
        )
    )

    assert issues == []


def test_layout_audit_allows_closed_modal_children_to_omit_geometry() -> None:
    audit = load_smoke_audit()
    hidden_child = node("hidden-child", "label")
    modal = node("modal", "modal", children=[hidden_child], props={"open": False})
    tree = node("window", "window", children=[modal])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 320, 240),
                "modal": rect(0, 0, 0, 0),
            },
        )
    )

    assert issues == []


def test_layout_audit_allows_inactive_tab_body_to_omit_geometry() -> None:
    audit = load_smoke_audit()
    hidden_child = node("hidden-child", "label")
    tab = node("tab-two", "tab", children=[hidden_child], props={"route_value": "two"})
    tabs = node("tabs", "tabs", children=[tab], props={"route_value": "one"})
    tree = node("window", "window", children=[tabs])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 320, 240),
                "tabs": rect(0, 0, 320, 160),
                "tab-two": rect(80, 0, 80, 32),
            },
        )
    )

    assert issues == []


def test_layout_audit_reports_normal_flow_sibling_overlap() -> None:
    audit = load_smoke_audit()
    first = node("first", "panel")
    second = node("second", "panel")
    row = node("row", "h_layout", children=[first, second])
    tree = node("window", "window", children=[row])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 320, 240),
                "row": rect(0, 0, 300, 100),
                "first": rect(0, 0, 180, 100),
                "second": rect(160, 0, 140, 100),
            },
        )
    )

    assert issues == [
        "sibling-overlap: first panel and second panel inside row h_layout"
    ]


def test_layout_audit_ignores_fixed_overlay_parent_bounds_and_overlap() -> None:
    audit = load_smoke_audit()
    content = node("content", "panel")
    overlay = node("overlay", "panel")
    row = node("row", "h_layout", children=[content, overlay])
    tree = node("window", "window", children=[row])

    issues = audit._audit_layout(
        result(
            tree,
            {
                "window": rect(0, 0, 320, 240),
                "row": rect(0, 0, 300, 100),
                "content": rect(0, 0, 300, 100),
                "overlay": rect(250, 50, 100, 80),
            },
            styles={"overlay": {"position": "fixed"}},
        )
    )

    assert issues == []
