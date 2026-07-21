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
        },
        {"id": "b", "status": "pass", "priority": "low", "notes": "ok"},
    ]

    merged = visual_audit.merge_results(existing, new, manifest)

    assert [item["id"] for item in merged] == ["a", "b"]
    assert merged[0]["status"] == "fail"
    assert merged[0]["priority"] == "high"
    assert merged[0]["screenshots"] == ["a-desktop.png", "a-mobile.png"]
    assert merged[0]["logs"] == ["a.stdout.txt", "a.stderr.txt"]
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
