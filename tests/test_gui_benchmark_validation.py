from __future__ import annotations

from pathlib import Path
import sys

import pytest


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "benchmarks"))

from gui_benchmark_validation import (  # noqa: E402
    ValidationRecorder,
    add_common_runtime_checks,
    find_tree_node,
)
from run_gui_update_pipeline_matrix import _filter_cases  # noqa: E402


def _snapshot(*, widgets: int = 7, issues: list[dict[str, object]] | None = None) -> dict[str, object]:
    return {
        "runtime": {
            "startup_readiness": "application_frame_presented",
            "gpu_ready": True,
            "frames_rendered": 60,
            "command_queue_depth": 0,
            "command_drain": {"last_pending": 0},
        },
        "gpu": {
            "renderer": {"widget_count": widgets},
            "layout": {"diagnostics": {"root": {"issues": issues or []}}},
            "tree": {
                "id": "root",
                "children": [{"id": "nested", "children": []}],
            },
        },
    }


def test_common_benchmark_validation_accepts_complete_snapshot() -> None:
    recorder = ValidationRecorder()
    add_common_runtime_checks(recorder, _snapshot(), expected_widgets=7, minimum_frames=60)

    report = recorder.report()

    assert report["passed"] is True
    assert report["check_count"] == 7
    assert report["failure_count"] == 0


def test_common_benchmark_validation_reports_mismatches() -> None:
    recorder = ValidationRecorder()
    add_common_runtime_checks(
        recorder,
        _snapshot(widgets=6, issues=[{"kind": "clip-escape"}]),
        expected_widgets=7,
        minimum_frames=60,
    )

    report = recorder.report()

    assert report["passed"] is False
    assert report["failure_count"] == 2
    assert {check["name"] for check in report["checks"] if not check["passed"]} == {
        "retained widget count",
        "layout diagnostics remained clean",
    }


def test_find_tree_node_walks_nested_children() -> None:
    tree = _snapshot()["gpu"]["tree"]

    assert find_tree_node(tree, "nested") == {"id": "nested", "children": []}
    assert find_tree_node(tree, "missing") is None


def test_update_pipeline_case_filter_supports_scenario_and_exact_scale() -> None:
    cases = (
        ("labels-fixed", 20, 100),
        ("labels-fixed", 1000, 100),
        ("mixed-state", 200, 100),
    )

    assert _filter_cases(cases, ["labels-fixed:1000"]) == (
        ("labels-fixed", 1000, 100),
    )
    assert _filter_cases(cases, ["labels-fixed"]) == cases[:2]


def test_update_pipeline_case_filter_supports_exact_burst_scale() -> None:
    cases = (
        ("same-property-burst", 1, 100),
        ("same-property-burst", 1, 10_000),
    )

    assert _filter_cases(cases, ["same-property-burst:1:10000"]) == (
        ("same-property-burst", 1, 10_000),
    )


def test_update_pipeline_case_filter_rejects_unknown_selector() -> None:
    cases = (("labels-fixed", 20, 100),)

    with pytest.raises(ValueError, match="unknown --case"):
        _filter_cases(cases, ["missing:20"])
