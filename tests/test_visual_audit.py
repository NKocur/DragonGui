from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys

import pytest


ROOT = Path(__file__).resolve().parents[1]
VISUAL_AUDIT_PATH = ROOT / "tools" / "visual_audit.py"


def load_visual_audit():
    spec = importlib.util.spec_from_file_location("visual_audit_tool", VISUAL_AUDIT_PATH)
    assert spec is not None
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_merge_results_preserves_existing_artifacts_and_worst_status() -> None:
    visual_audit = load_visual_audit()
    manifest = [{"id": "a"}, {"id": "b"}]
    existing = [
        {
            "id": "a",
            "status": "pass",
            "priority": "low",
            "notes": "desktop ok",
            "screenshots": ["a-desktop.png"],
            "snapshots": ["a-desktop.json"],
            "logs": ["a.stdout.txt"],
            "reproduction": ["desktop"],
            "layout_issue_counts": {"clip-escape": 1},
            "unmatched_selectors": ["OldWidget"],
        }
    ]
    new = [
        {
            "id": "a",
            "status": "fail",
            "priority": "high",
            "notes": "mobile clipped",
            "screenshots": ["a-mobile.png"],
            "snapshots": ["a-mobile.json"],
            "logs": ["a.stdout.txt", "a.stderr.txt"],
            "reproduction": ["mobile"],
            "layout_issue_counts": {"clip-escape": 2, "negative-scroll": 1},
            "unmatched_selectors": ["MissingWidget"],
        },
        {"id": "b", "status": "pass", "priority": "low", "notes": "ok"},
    ]

    merged = visual_audit.merge_results(existing, new, manifest)

    assert [item["id"] for item in merged] == ["a", "b"]
    assert merged[0]["status"] == "fail"
    assert merged[0]["priority"] == "high"
    assert merged[0]["screenshots"] == ["a-desktop.png", "a-mobile.png"]
    assert merged[0]["logs"] == ["a.stdout.txt", "a.stderr.txt"]
    assert merged[0]["layout_issue_counts"] == {
        "clip-escape": 3,
        "negative-scroll": 1,
    }
    assert merged[0]["unmatched_selectors"] == ["OldWidget", "MissingWidget"]
    assert "desktop ok" in merged[0]["notes"]
    assert "mobile clipped" in merged[0]["notes"]


def test_load_existing_results_strict_rejects_malformed_json(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    (tmp_path / "report.json").write_text("{not json", encoding="utf-8")

    with pytest.raises(SystemExit, match="malformed JSON"):
        visual_audit.load_existing_results(tmp_path, strict=True)


def test_load_existing_results_non_strict_ignores_malformed_json(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    (tmp_path / "report.json").write_text("{not json", encoding="utf-8")

    assert visual_audit.load_existing_results(tmp_path) == []


def test_visual_audit_reads_unmatched_user_selectors(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    snapshot = tmp_path / "snapshot.json"
    snapshot.write_text(
        json.dumps(
            {
                "gpu": {
                    "stylesheets": {
                        "unmatched_user_selectors": [
                            "MissingWidget",
                            "MissingWidget",
                            "SearchBox.typo",
                        ]
                    }
                }
            }
        ),
        encoding="utf-8",
    )

    assert visual_audit.unmatched_user_selectors(snapshot) == [
        "MissingWidget",
        "SearchBox.typo",
    ]


def test_visual_audit_states_validate_actions_and_preserve_default_mode() -> None:
    visual_audit = load_visual_audit()
    assert visual_audit.target_states({"id": "simple"}) == [
        {"name": "default", "route": None, "actions": []}
    ]
    target = {
        "id": "app",
        "states": [
            {"name": "overview", "route": "overview"},
            {
                "name": "active",
                "actions": [
                    "click:#run",
                    "native-click:#run",
                    "hover:#help",
                    "type:#query=dragon",
                    "scroll:#body=0,240",
                    "resize:800x600",
                    "assert-window-state:maximized",
                    "assert-window-state:minimized",
                    "assert-window-state:normal",
                    "set-window-state:normal",
                    "key:tab",
                    "key:enter",
                    "key:space",
                    "key:escape",
                    "key:alt-space",
                    "assert-focus:#run",
                    "right-click:#help",
                    "assert-system-menu:open",
                    "assert-system-menu:closed",
                    "wait:120",
                ],
            },
        ],
    }
    assert [state["name"] for state in visual_audit.target_states(target)] == [
        "overview",
        "active",
    ]
    with pytest.raises(ValueError, match="unsupported"):
        visual_audit.target_states(
            {"id": "bad", "states": [{"name": "bad", "actions": ["drag:#run"]}]}
        )


def test_screenshot_error_note_reads_structured_error(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    screenshot = tmp_path / "target.png"
    visual_audit.screenshot_error_path(screenshot).write_text(
        json.dumps({"kind": "exception", "error": "native timeout"}),
        encoding="utf-8",
    )

    note = visual_audit.with_screenshot_error_note("fallback used", screenshot)

    assert note == "fallback used Native screenshot error: exception: native timeout"


def test_professional_demo_scroll_validator_flags_unreachable_descendants(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    snapshot = tmp_path / "all-features-professional-styling-320x640.json"
    snapshot.write_text(
        json.dumps(
            {
                "gpu": {
                    "tree": {
                        "id": "window",
                        "type": "window",
                        "children": [
                            {
                                "id": "body",
                                "type": "scroll_area",
                                "class": "body",
                                "children": [
                                    {
                                        "id": "pages",
                                        "type": "pages",
                                        "children": [
                                            {
                                                "id": "active-page",
                                                "type": "page",
                                                "children": [
                                                    {"id": "top", "type": "panel", "children": []},
                                                    {"id": "deep", "type": "panel", "children": []},
                                                ],
                                            }
                                        ],
                                    }
                                ],
                            }
                        ],
                    },
                    "layout": {
                        "rects": {
                            "window": {"x": 0, "y": 0, "w": 320, "h": 640},
                            "body": {"x": 8, "y": 333, "w": 304, "h": 265},
                            "pages": {"x": 8, "y": 333, "w": 284, "h": 265},
                            "active-page": {"x": 8, "y": 333, "w": 284, "h": 265},
                            "top": {"x": 16, "y": 360, "w": 260, "h": 80},
                            "deep": {"x": 16, "y": 1068, "w": 260, "h": 268},
                        },
                        "scroll_max_y": {"body": 10},
                    },
                }
            }
        ),
        encoding="utf-8",
    )

    violations = visual_audit.validate_professional_demo_scroll_snapshot(snapshot)

    assert violations
    assert "body scroll range too small" in violations[0]
    assert "got 10" in violations[0]


def test_professional_demo_scroll_validator_accepts_reachable_descendants(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    snapshot = tmp_path / "all-features-professional-styling-320x640.json"
    snapshot.write_text(
        json.dumps(
            {
                "gpu": {
                    "tree": {
                        "id": "window",
                        "type": "window",
                        "children": [
                            {
                                "id": "body",
                                "type": "scroll_area",
                                "class": "body",
                                "children": [
                                    {"id": "deep", "type": "panel", "children": []},
                                    {
                                        "id": "overlay",
                                        "type": "tooltip",
                                        "children": [
                                            {"id": "overlay-child", "type": "panel", "children": []}
                                        ],
                                    },
                                ],
                            }
                        ],
                    },
                    "layout": {
                        "rects": {
                            "window": {"x": 0, "y": 0, "w": 320, "h": 640},
                            "body": {"x": 8, "y": 333, "w": 304, "h": 265},
                            "deep": {"x": 16, "y": 560, "w": 260, "h": 80},
                            "overlay": {"x": 16, "y": 2000, "w": 260, "h": 80},
                            "overlay-child": {"x": 16, "y": 2100, "w": 260, "h": 80},
                        },
                        "scroll_max_y": {"body": 74},
                    },
                }
            }
        ),
        encoding="utf-8",
    )

    assert visual_audit.validate_professional_demo_scroll_snapshot(snapshot) == []


def test_professional_demo_scroll_validator_prefers_nested_page_owner(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    snapshot = tmp_path / "all-features-professional-overview-390x720.json"
    snapshot.write_text(
        json.dumps(
            {
                "gpu": {
                    "tree": {
                        "id": "window",
                        "type": "window",
                        "children": [{
                            "id": "body",
                            "type": "scroll_area",
                            "class": "body",
                            "children": [{
                                "id": "page-scroll",
                                "type": "scroll_area",
                                "class": "page-scroll",
                                "children": [{
                                    "id": "last-card",
                                    "type": "panel",
                                    "children": [],
                                }],
                            }],
                        }],
                    },
                    "layout": {
                        "rects": {
                            "window": {"x": 0, "y": 0, "w": 390, "h": 720},
                            "body": {"x": 0, "y": 80, "w": 390, "h": 600},
                            "page-scroll": {"x": 0, "y": 80, "w": 390, "h": 600},
                            "last-card": {"x": 10, "y": 760, "w": 360, "h": 120},
                        },
                        "scroll_max_y": {"body": 0, "page-scroll": 220},
                    },
                }
            }
        ),
        encoding="utf-8",
    )

    assert visual_audit.validate_professional_demo_scroll_snapshot(snapshot) == []


def test_professional_demo_splitter_validator_flags_underused_panes(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    snapshot = tmp_path / "all-features-professional-data-1440x900.json"
    snapshot.write_text(
        json.dumps(
            {
                "gpu": {
                    "tree": {
                        "id": "window",
                        "type": "window",
                        "children": [
                            {
                                "id": "splitter",
                                "type": "splitter",
                                "props": {"orientation": "horizontal"},
                                "children": [
                                    {"id": "pane-a", "type": "pane", "props": {}},
                                    {"id": "pane-b", "type": "pane", "props": {}},
                                ],
                            }
                        ],
                    },
                    "layout": {
                        "rects": {
                            "splitter": {"x": 222, "y": 255, "w": 1190, "h": 620},
                            "pane-a": {"x": 222, "y": 255, "w": 360, "h": 620},
                            "pane-b": {"x": 585, "y": 255, "w": 280, "h": 620},
                        }
                    },
                }
            }
        ),
        encoding="utf-8",
    )

    violations = visual_audit.validate_professional_demo_splitter_snapshot(snapshot)

    assert violations
    assert "splitter underutilized" in violations[0]
    assert "1190" in violations[0]


def test_professional_demo_splitter_validator_accepts_filled_panes(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    snapshot = tmp_path / "all-features-professional-data-1440x900.json"
    snapshot.write_text(
        json.dumps(
            {
                "gpu": {
                    "tree": {
                        "id": "window",
                        "type": "window",
                        "children": [
                            {
                                "id": "splitter",
                                "type": "splitter",
                                "props": {"orientation": "horizontal"},
                                "children": [
                                    {"id": "pane-a", "type": "pane", "props": {}},
                                    {"id": "pane-b", "type": "pane", "props": {}},
                                ],
                            }
                        ],
                    },
                    "layout": {
                        "rects": {
                            "splitter": {"x": 222, "y": 255, "w": 1190, "h": 620},
                            "pane-a": {"x": 222, "y": 255, "w": 829, "h": 620},
                            "pane-b": {"x": 1057, "y": 255, "w": 355, "h": 620},
                        }
                    },
                }
            }
        ),
        encoding="utf-8",
    )

    assert visual_audit.validate_professional_demo_splitter_snapshot(snapshot) == []


def test_select_targets_rejects_unknown_id() -> None:
    visual_audit = load_visual_audit()

    with pytest.raises(SystemExit, match="Unknown target"):
        visual_audit.select_targets([{"id": "known"}], ["missing"], "all")


def test_scale_selectors_validate_and_expand_manifest_values() -> None:
    visual_audit = load_visual_audit()

    assert visual_audit.parse_scales("manifest") == [None]
    assert visual_audit.parse_scales("1,1.5") == [1.0, 1.5]
    assert visual_audit.target_scales({"scales": [1.0, 1.5]}, [None]) == [1.0, 1.5]
    assert visual_audit.target_scales({"scales": [1.0, 1.5]}, [2.0]) == [2.0]
    with pytest.raises(SystemExit, match="0.5 to 4.0"):
        visual_audit.parse_scales("0.25")


def test_layout_torture_targets_cover_standard_viewports() -> None:
    manifest_path = ROOT / "examples" / "css_feature_probes" / "visual_audit_manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    targets = {target["id"]: target for target in manifest}
    required_sizes = {
        (390, 720),
        (640, 480),
        (1024, 768),
        (1440, 900),
    }

    for target_id in {
        "layout-flex-stress",
        "layout-panel-bounds",
        "layout-grid-masonry",
        "layout-overlay-collision",
        "layout-scrollable-composites",
        "layout-plot-embedding",
        "overflow-scrollbar",
        "responsive-layout",
    }:
        configured_sizes = {tuple(size) for size in targets[target_id]["sizes"]}
        assert required_sizes <= configured_sizes, target_id
        assert {1.0, 1.5} <= set(targets[target_id]["scales"]), target_id
        assert targets[target_id]["resize_checkpoints"] == [[640, 480], [1024, 768]]


def test_aurora_visual_audit_covers_routes_and_interaction_states() -> None:
    visual_audit = load_visual_audit()
    manifest_path = ROOT / "examples" / "css_feature_probes" / "visual_audit_manifest.json"
    targets = {target["id"]: target for target in visual_audit.load_manifest(manifest_path)}
    states = {
        state["name"]: state
        for state in visual_audit.target_states(targets["aurora-command-center"])
    }
    assert {"overview", "analytics", "automation", "settings"} <= states.keys()
    assert states["modal-open"]["actions"][0] == "click:#new-review"
    assert states["sidebar-collapsed"]["actions"][0] == "click:#sidebar-toggle"
    assert states["workspace-menu"]["actions"][0] == "click:#workspace-menu"
    assert states["sidebar-tooltip"]["actions"][0] == "hover:#sidebar-toggle"
    assert states["analytics-scrolled"]["actions"][0].startswith(
        "scroll:#analytics-scroll="
    )


def test_layout_styling_evolution_baselines_are_registered() -> None:
    manifest_path = ROOT / "examples" / "css_feature_probes" / "visual_audit_manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    targets = {target["id"]: target for target in manifest}

    expected = {
        "semantic-css-identity": "css",
        "cascade-origin": "css",
        "sidebar-flex-allocation": "layout",
        "responsive-grid-orphan": "layout",
    }
    for target_id, category in expected.items():
        target = targets[target_id]
        assert target["category"] == category
        assert target["manual"] is False
        assert {1.0, 1.5} <= set(target["scales"])
        assert (390, 720) in {tuple(size) for size in target["sizes"]}


def test_client_window_chrome_visual_matrix_is_registered() -> None:
    visual_audit = load_visual_audit()
    manifest_path = ROOT / "examples" / "css_feature_probes" / "visual_audit_manifest.json"
    targets = {target["id"]: target for target in visual_audit.load_manifest(manifest_path)}
    target = targets["client-window-chrome"]

    assert target["script"] == "examples/client_window_chrome_demo.py"
    assert target["category"] == "layout"
    assert target["manual"] is True
    assert {tuple(size) for size in target["sizes"]} == {
        (720, 520),
        (940, 620),
        (1280, 800),
    }
    assert target["scales"] == [1.0, 1.5, 2.0]
    assert target["resize_checkpoints"] == [[720, 520], [940, 620]]


def test_client_window_chrome_maximized_interaction_is_registered() -> None:
    visual_audit = load_visual_audit()
    manifest_path = ROOT / "examples" / "css_feature_probes" / "visual_audit_manifest.json"
    targets = {target["id"]: target for target in visual_audit.load_manifest(manifest_path)}
    target = targets["client-window-chrome-maximized"]
    states = visual_audit.target_states(target)

    assert target["strict_css"] is True
    assert "resize_checkpoints" not in target
    assert states == [
        {
            "name": "maximized",
            "route": None,
            "actions": ["click:#client-chrome-window--dg-window-maximize"],
        }
    ]


def test_client_window_chrome_transition_interactions_are_registered() -> None:
    visual_audit = load_visual_audit()
    manifest_path = ROOT / "examples" / "css_feature_probes" / "visual_audit_manifest.json"
    targets = {target["id"]: target for target in visual_audit.load_manifest(manifest_path)}
    states = {
        state["name"]: state
        for state in visual_audit.target_states(targets["client-window-chrome-transitions"])
    }

    assert states["button-restore"]["actions"] == [
        "click:#client-chrome-window--dg-window-maximize",
        "assert-window-state:maximized",
        "click:#client-chrome-window--dg-window-maximize",
        "assert-window-state:normal",
    ]
    assert "titlebar-double-click" not in states


def test_client_window_chrome_windows_input_interactions_are_registered() -> None:
    visual_audit = load_visual_audit()
    manifest_path = ROOT / "examples" / "css_feature_probes" / "visual_audit_manifest.json"
    targets = {target["id"]: target for target in visual_audit.load_manifest(manifest_path)}
    target = targets["client-window-chrome-windows-input"]
    states = {
        state["name"]: state for state in visual_audit.target_states(target)
    }

    assert target["manual"] is False
    assert states["keyboard-traversal"]["actions"] == [
        "native-click:#client-chrome-app-button",
        "assert-focus:#client-chrome-app-button",
        "key:tab",
        "assert-focus:#client-chrome-window--dg-window-minimize",
        "key:tab",
        "assert-focus:#client-chrome-window--dg-window-maximize",
        "key:tab",
        "assert-focus:#client-chrome-window--dg-window-close",
        "key:tab",
        "assert-focus:#client-chrome-checkbox",
    ]
    assert states["keyboard-maximize-restore"]["actions"][-4:] == [
        "key:enter",
        "assert-window-state:maximized",
        "key:space",
        "assert-window-state:normal",
    ]
    assert "minimize-roundtrip" not in states
    assert states["alt-space-system-menu"]["actions"] == [
        "native-click:#client-chrome-app-button",
        "key:alt-space",
        "assert-system-menu:open",
        "key:escape",
        "assert-system-menu:closed",
    ]
    assert "right-click-system-menu" not in states


def test_layout_snapshot_relations_accept_contained_owned_geometry(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    snapshot_path = tmp_path / "layout.json"
    snapshot_path.write_text(
        json.dumps(
            {
                "gpu": {
                    "window": {"width": 320, "height": 240, "scale_factor": 1},
                    "tree": {"id": "window", "type": "window", "children": []},
                    "layout": {
                        "schema_version": 1,
                        "rects": {
                            "window": {"x": 0, "y": 0, "w": 320, "h": 240},
                            "body": {"x": 10, "y": 10, "w": 300, "h": 220},
                        },
                        "clips": {
                            "window": {"x": 0, "y": 0, "w": 320, "h": 240},
                            "body": {"x": 10, "y": 10, "w": 300, "h": 220},
                        },
                        "paint_clips": {
                            "window": {"x": 0, "y": 0, "w": 320, "h": 240},
                            "body": {"x": 0, "y": 0, "w": 320, "h": 240},
                        },
                        "diagnostics": {
                            "window": {"issues": []},
                            "body": {"issues": []},
                        },
                        "scroll_x": {"body": 0},
                        "scroll_y": {"body": 0},
                        "scroll_max_x": {"body": 0},
                        "scroll_max_y": {"body": 0},
                    },
                }
            }
        ),
        encoding="utf-8",
    )

    assert visual_audit.validate_layout_snapshot_relations(snapshot_path) == ([], {})
    assert visual_audit.validate_layout_resize_round_trip(snapshot_path, snapshot_path) == []


def test_layout_snapshot_relations_report_clip_scroll_diagnostic_and_round_trip_drift(
    tmp_path: Path,
) -> None:
    visual_audit = load_visual_audit()
    start_path = tmp_path / "start.json"
    final_path = tmp_path / "final.json"
    payload = {
        "gpu": {
            "window": {"width": 100, "height": 80, "scale_factor": 1},
            "tree": {"id": "window", "type": "window", "children": []},
            "layout": {
                "schema_version": 1,
                "rects": {"window": {"x": 0, "y": 0, "w": 100, "h": 80}},
                "clips": {"window": {"x": -5, "y": 0, "w": 105, "h": 80}},
                "paint_clips": {"window": {"x": 0, "y": 0, "w": 100, "h": 80}},
                "diagnostics": {
                    "window": {
                        "issues": [
                            {
                                "code": "unreachable-root-overflow",
                                "message": "content is unreachable",
                            }
                        ]
                    }
                },
                "scroll_x": {},
                "scroll_y": {},
                "scroll_max_x": {"window": -1},
                "scroll_max_y": {},
            },
        }
    }
    start_path.write_text(json.dumps(payload), encoding="utf-8")
    final_payload = json.loads(json.dumps(payload))
    final_payload["gpu"]["layout"]["rects"]["window"]["w"] = 90
    final_path.write_text(json.dumps(final_payload), encoding="utf-8")

    violations, counts = visual_audit.validate_layout_snapshot_relations(start_path)
    assert counts == {"unreachable-root-overflow": 1}
    assert any("escapes its paint clip" in violation for violation in violations)
    assert any("invalid scroll_max_x" in violation for violation in violations)
    assert any("content is unreachable" in violation for violation in violations)
    assert any(
        "resize round trip changed rects.window" in violation
        for violation in visual_audit.validate_layout_resize_round_trip(start_path, final_path)
    )


def test_visual_audit_layout_entries_include_page_capture_and_artifact_context(
    tmp_path: Path,
) -> None:
    visual_audit = load_visual_audit()
    snapshot = tmp_path / "capture.json"
    snapshot.write_text(
        json.dumps(
            {
                "gpu": {
                    "tree": {
                        "id": "window",
                        "type": "window",
                        "children": [
                            {
                                "id": "settings",
                                "type": "page",
                                "children": [
                                    {"id": "save", "type": "button", "children": []}
                                ],
                            }
                        ],
                    },
                    "layout": {
                        "diagnostics": {
                            "save": {
                                "issues": [
                                    {
                                        "code": "fully-clipped-interactive",
                                        "severity": "error",
                                        "widget_id": "save",
                                        "widget_type": "button",
                                        "message": "save is entirely clipped",
                                    }
                                ]
                            }
                        }
                    },
                }
            }
        ),
        encoding="utf-8",
    )

    entries = visual_audit.snapshot_layout_diagnostic_entries(
        snapshot,
        size=(390, 720),
        scale_factor=1.5,
        snapshot_artifact="snapshots/capture.json",
        screenshot_artifact="screenshots/capture.png",
    )

    assert entries == [
        {
            "code": "fully-clipped-interactive",
            "severity": "error",
            "widget_id": "save",
            "widget_type": "button",
            "page_id": "settings",
            "size": "390x720",
            "scale": 1.5,
            "message": "save is entirely clipped",
            "snapshot": "snapshots/capture.json",
            "screenshot": "screenshots/capture.png",
        }
    ]


def test_visual_audit_diagnostic_entries_write_direct_node_data(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    snapshot = tmp_path / "capture.json"
    snapshot.write_text(
        json.dumps(
            {
                "gpu": {
                    "tree": {"id": "window", "type": "window", "children": [
                        {"id": "run", "type": "button", "children": []}
                    ]},
                    "computed_styles": {"run": {"matched_selectors": []}},
                    "layout": {"diagnostics": {"run": {"issues": [{
                        "code": "fully-clipped-interactive",
                        "severity": "error",
                        "widget_id": "run",
                        "message": "clipped",
                    }]}}},
                }
            }
        ),
        encoding="utf-8",
    )
    out = tmp_path / "out"
    entries = visual_audit.snapshot_layout_diagnostic_entries(
        snapshot,
        size=(640, 480),
        scale_factor=1.0,
        snapshot_artifact="snapshots/capture.json",
        screenshot_artifact="screenshots/capture.png",
        route="overview",
        state="modal-open",
        artifact_root=out,
    )
    assert entries[0]["route"] == "overview"
    assert entries[0]["state"] == "modal-open"
    detail = out / entries[0]["node_data"]
    payload = json.loads(detail.read_text(encoding="utf-8"))
    assert payload["node"]["id"] == "run"
    assert payload["computed_style"] == {"matched_selectors": []}


def test_overflow_probe_relationships_require_owned_ranges(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    snapshot_path = tmp_path / "overflow.json"
    nodes = [
        {"id": "vertical", "type": "panel", "class": "vertical-scroll", "children": []},
        {"id": "horizontal", "type": "h_layout", "class": "horizontal-scroll", "children": []},
        {"id": "both", "type": "panel", "class": "both-scroll", "children": []},
    ]
    rects = {
        "window": {"x": 0, "y": 0, "w": 320, "h": 240},
        **{
            node["id"]: {"x": 0, "y": index * 60, "w": 300, "h": 50}
            for index, node in enumerate(nodes)
        },
    }
    snapshot_path.write_text(
        json.dumps(
            {
                "gpu": {
                    "tree": {
                        "id": "window",
                        "type": "window",
                        "children": nodes,
                    },
                    "layout": {
                        "rects": rects,
                        "clips": rects,
                        "scroll_max_x": {"horizontal": 20, "both": 30},
                        "scroll_max_y": {"vertical": 40, "both": 50},
                    },
                }
            }
        ),
        encoding="utf-8",
    )

    assert (
        visual_audit.validate_layout_target_relationships(
            snapshot_path, "overflow-scrollbar"
        )
        == []
    )
    payload = json.loads(snapshot_path.read_text(encoding="utf-8"))
    payload["gpu"]["layout"]["scroll_max_y"]["both"] = 0
    snapshot_path.write_text(json.dumps(payload), encoding="utf-8")
    assert any(
        ".both-scroll both lacks owned y-scroll" in violation
        for violation in visual_audit.validate_layout_target_relationships(
            snapshot_path, "overflow-scrollbar"
        )
    )


def test_append_skip_existing_all_selected_noop(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    visual_audit = load_visual_audit()
    script = tmp_path / "probe.py"
    script.write_text("print('not run')\n", encoding="utf-8")
    manifest = [
        {
            "id": "probe",
            "name": "Probe",
            "script": str(script),
            "category": "widgets",
            "features": [],
            "sizes": [[640, 480]],
            "manual": False,
        }
    ]
    manifest_path = tmp_path / "manifest.json"
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
    out = tmp_path / "out"
    out.mkdir()
    existing_result = {
        "id": "probe",
        "name": "Probe",
        "script": str(script),
        "category": "widgets",
        "features": [],
        "status": "pass",
        "priority": "low",
        "screenshots": [],
        "snapshots": [],
        "logs": [],
        "notes": "already audited",
        "suspected_modules": [],
        "reproduction": [],
    }
    (out / "report.json").write_text(
        json.dumps([existing_result]),
        encoding="utf-8",
    )
    monkeypatch.setattr(
        "sys.argv",
        [
            "visual_audit.py",
            "--manifest",
            str(manifest_path),
            "--out",
            str(out),
            "--append",
            "--skip-existing",
        ],
    )

    assert visual_audit.main() == 0
    assert json.loads((out / "report.json").read_text(encoding="utf-8")) == [existing_result]


def test_run_target_preserves_manual_manifest_notes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    visual_audit = load_visual_audit()
    script = tmp_path / "probe.py"
    script.write_text("print('probe')\n", encoding="utf-8")

    def fake_run_probe_process(*args, **kwargs):
        kwargs["screenshot_path"].parent.mkdir(parents=True, exist_ok=True)
        kwargs["snapshot_path"].parent.mkdir(parents=True, exist_ok=True)
        kwargs["screenshot_path"].write_bytes(b"png")
        kwargs["snapshot_path"].write_text("{}", encoding="utf-8")
        return {
            "status": "pass",
            "notes": "Captured with native DragonGUI window screenshot API.",
            "screenshot": True,
            "snapshot": True,
        }

    monkeypatch.setattr(visual_audit, "run_probe_process", fake_run_probe_process)
    monkeypatch.setattr(visual_audit, "ROOT", tmp_path)
    target = {
        "id": "manual-probe",
        "name": "Manual Probe",
        "script": str(script),
        "category": "widgets",
        "features": ["Menu"],
        "sizes": [[640, 480]],
        "manual": True,
        "notes": "Open menu and hover states still need interaction.",
    }

    result = visual_audit.run_target(
        target,
        out_dir=tmp_path / "out",
        wait_ms=1,
        timeout_ms=100,
        size_selectors=[None],
        no_capture=False,
    )

    assert result["status"] == "needs_manual_interaction"
    assert "Open menu and hover states still need interaction." in result["notes"]
    assert "Captured with native DragonGUI window screenshot API." in result["notes"]


def test_run_target_promotes_snapshot_action_error_to_failure(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    visual_audit = load_visual_audit()
    script = tmp_path / "probe.py"
    script.write_text("print('probe')\n", encoding="utf-8")

    def fake_run_probe_process(*args, **kwargs):
        kwargs["screenshot_path"].parent.mkdir(parents=True, exist_ok=True)
        kwargs["snapshot_path"].parent.mkdir(parents=True, exist_ok=True)
        kwargs["screenshot_path"].write_bytes(b"png")
        kwargs["snapshot_path"].write_text(
            json.dumps({"error": "expected maximized, observed normal"}),
            encoding="utf-8",
        )
        return {
            "status": "pass",
            "notes": "screenshot captured",
            "screenshot": True,
            "snapshot": True,
        }

    monkeypatch.setattr(visual_audit, "run_probe_process", fake_run_probe_process)
    monkeypatch.setattr(visual_audit, "ROOT", tmp_path)
    result = visual_audit.run_target(
        {
            "id": "action-error",
            "name": "Action Error",
            "script": str(script),
            "category": "layout",
            "features": [],
            "sizes": [[640, 480]],
            "manual": False,
        },
        out_dir=tmp_path / "out",
        wait_ms=1,
        timeout_ms=100,
        size_selectors=[None],
        no_capture=False,
    )

    assert result["status"] == "fail"
    assert "expected maximized, observed normal" in result["notes"]
    assert result["captures"][0]["error"] == "expected maximized, observed normal"
    assert result["captures"][0]["diagnostic_counts"]["capture-error"] == 1


def test_run_target_expands_manifest_states_and_records_capture_context(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    visual_audit = load_visual_audit()
    script = tmp_path / "probe.py"
    script.write_text("print('probe')\n", encoding="utf-8")
    calls: list[dict[str, object]] = []

    def fake_run_probe_process(*args, **kwargs):
        calls.append(kwargs)
        kwargs["screenshot_path"].parent.mkdir(parents=True, exist_ok=True)
        kwargs["snapshot_path"].parent.mkdir(parents=True, exist_ok=True)
        kwargs["screenshot_path"].write_bytes(b"png")
        kwargs["snapshot_path"].write_text(
            json.dumps({"gpu": {"tree": {}, "layout": {"diagnostics": {}}}}),
            encoding="utf-8",
        )
        return {
            "status": "pass",
            "notes": "captured",
            "screenshot": True,
            "snapshot": True,
        }

    monkeypatch.setattr(visual_audit, "run_probe_process", fake_run_probe_process)
    monkeypatch.setattr(visual_audit, "ROOT", tmp_path)
    target = {
        "id": "states",
        "name": "States",
        "script": str(script),
        "category": "layout",
        "features": ["Pages", "Modal"],
        "sizes": [[640, 480]],
        "manual": False,
        "states": [
            {"name": "overview", "route": "overview"},
            {"name": "modal-open", "route": "overview", "actions": ["click:#new-review"]},
        ],
    }
    result = visual_audit.run_target(
        target,
        out_dir=tmp_path / "out",
        wait_ms=1,
        timeout_ms=100,
        size_selectors=[(640, 480)],
        no_capture=False,
    )

    assert len(calls) == 2
    assert calls[0]["route"] == "overview"
    assert calls[1]["actions"] == ["click:#new-review"]
    assert [capture["state"] for capture in result["captures"]] == [
        "overview",
        "modal-open",
    ]
    assert result["captures"][1]["screenshot"].endswith("-modal-open.png")
    assert result["diagnostic_comparisons"][0]["diagnostic_delta"] == {}


def test_visual_audit_report_embeds_state_thumbnails_and_node_links(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    result = {
        "id": "app",
        "name": "App",
        "script": "examples/app.py",
        "category": "layout",
        "features": ["Pages"],
        "manual": False,
        "status": "fail",
        "priority": "high",
        "notes": "diagnostic",
        "suspected_modules": ["native/src/layout.rs"],
        "screenshots": ["screenshots/app-modal.png"],
        "snapshots": ["snapshots/app-modal.json"],
        "logs": [],
        "reproduction": ["python app.py"],
        "unmatched_selectors": [],
        "layout_issue_counts": {"empty-paint-clip": 1},
        "captures": [{
            "size": "640x480",
            "scale": 1.5,
            "route": "overview",
            "state": "modal-open",
            "screenshot": "screenshots/app-modal.png",
        }],
        "diagnostic_comparisons": [{
            "size": "640x480",
            "scale": 1.5,
            "from_state": "overview",
            "to_state": "modal-open",
            "diagnostic_delta": {"empty-paint-clip": 1},
        }],
        "layout_issues": [{
            "code": "empty-paint-clip",
            "widget_id": "modal",
            "widget_type": "modal",
            "page_id": "overview",
            "route": "overview",
            "state": "modal-open",
            "size": "640x480",
            "scale": 1.5,
            "snapshot": "snapshots/app-modal.json",
            "screenshot": "screenshots/app-modal.png",
            "node_data": "diagnostics/app-modal.json",
            "message": "empty clip",
        }],
    }
    visual_audit.write_report(tmp_path, [result])
    report = (tmp_path / "REPORT.md").read_text(encoding="utf-8")
    assert "Capture Gallery" in report
    assert '<img src="screenshots/app-modal.png"' in report
    assert "Diagnostic State Comparisons" in report
    assert "[node data](diagnostics/app-modal.json)" in report
    assert "`overview / modal-open`" in report


def test_professional_explore_scatter_snapshot_requires_startup_fit(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    snapshot = tmp_path / "explore.json"
    snapshot.write_text(
        json.dumps(
            {
                "runtime": {
                    "commands": {
                        "recent": [
                            {
                                "command": "SetScatterPointsPacked",
                                "target": "left",
                                "detail": "payload_bytes=480, colormap=turbo, format=point_instance_v1, fit=false",
                            },
                            {
                                "command": "SetScatterPointsPacked",
                                "target": "right",
                                "detail": "payload_bytes=480, colormap=viridis, format=point_instance_v1, fit=false",
                            },
                            {"command": "FitScatterCamera", "target": "right", "detail": None},
                        ]
                    }
                }
            }
        ),
        encoding="utf-8",
    )

    violations = visual_audit.validate_professional_explore_scatter_snapshot(snapshot)

    assert violations == ["explore.json: scatter left uploaded points without startup fit"]


def test_professional_explore_scatter_snapshot_accepts_fit_flag(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    snapshot = tmp_path / "explore.json"
    snapshot.write_text(
        json.dumps(
            {
                "runtime": {
                    "commands": {
                        "recent": [
                            {
                                "command": "SetScatterPointsPacked",
                                "target": "left",
                                "detail": "payload_bytes=480, colormap=turbo, format=point_instance_v1, fit=true",
                            },
                            {
                                "command": "SetScatterPointsPacked",
                                "target": "right",
                                "detail": "payload_bytes=480, colormap=viridis, format=point_instance_v1, fit=false",
                            },
                            {"command": "FitScatterCamera", "target": "right", "detail": None},
                        ]
                    }
                }
            }
        ),
        encoding="utf-8",
    )

    assert visual_audit.validate_professional_explore_scatter_snapshot(snapshot) == []


def test_adjacent_scatter_interaction_log_requires_pass_marker_and_paths(tmp_path: Path) -> None:
    visual_audit = load_visual_audit()
    stdout = tmp_path / "adjacent.stdout.txt"

    stdout.write_text("ADJACENT_SCATTER_INTERACTION_PASS\n", encoding="utf-8")
    assert visual_audit.validate_adjacent_scatter_interaction_log(stdout) == [
        "adjacent.stdout.txt: missing dragongui import path diagnostics"
    ]

    stdout.write_text(
        "\n".join(
            [
                "dragongui_import=J:/Projects/DragonFrame/python/dragongui/__init__.py",
                "dragongui_native_import=J:/Projects/DragonFrame/python/dragongui/_dragongui.pyd",
                "ADJACENT_SCATTER_INTERACTION_PASS",
            ]
        ),
        encoding="utf-8",
    )
    assert visual_audit.validate_adjacent_scatter_interaction_log(stdout) == []

    stdout.write_text(
        "dragongui_import=x\n"
        "dragongui_native_import=y\n"
        "ADJACENT_SCATTER_INTERACTION_FAIL left stable scatter signature changed\n",
        encoding="utf-8",
    )
    assert visual_audit.validate_adjacent_scatter_interaction_log(stdout) == [
        "ADJACENT_SCATTER_INTERACTION_FAIL left stable scatter signature changed"
    ]


def test_adjacent_scatter_interaction_screenshot_requires_plot_pixels(tmp_path: Path) -> None:
    pytest.importorskip("PIL")
    from PIL import Image, ImageDraw

    visual_audit = load_visual_audit()
    snapshot = tmp_path / "adjacent.json"
    snapshot.write_text(
        json.dumps(
            {
                "gpu": {
                    "resources": {
                        "scatters": {
                            "adjacent-left-scatter": {
                                "viewport": {"offset": [10, 10], "size": [80, 80]}
                            },
                            "adjacent-right-scatter": {
                                "viewport": {"offset": [110, 10], "size": [80, 80]}
                            },
                        }
                    }
                }
            }
        ),
        encoding="utf-8",
    )

    image = Image.new("RGB", (200, 100), "#08111c")
    draw = ImageDraw.Draw(image)
    draw.line((20, 70, 80, 20), fill="#ffd84a", width=2)
    draw.line((120, 70, 180, 20), fill="#8df2ff", width=2)
    screenshot = tmp_path / "adjacent.png"
    image.save(screenshot)

    assert visual_audit.validate_adjacent_scatter_interaction_screenshot(
        screenshot, snapshot
    ) == []

    blank_left = Image.new("RGB", (200, 100), "#08111c")
    draw = ImageDraw.Draw(blank_left)
    draw.line((120, 70, 180, 20), fill="#8df2ff", width=2)
    blank_path = tmp_path / "adjacent-blank-left.png"
    blank_left.save(blank_path)

    violations = visual_audit.validate_adjacent_scatter_interaction_screenshot(
        blank_path, snapshot
    )
    assert any("adjacent-left-scatter has too few plot pixels" in item for item in violations)
