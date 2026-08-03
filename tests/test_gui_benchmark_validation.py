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
from gui_update_pipeline_case import _locality_indices  # noqa: E402
import gui_live_dashboard_case as live_dashboard_case  # noqa: E402
import gui_telemetry_viewer_case as telemetry_case  # noqa: E402
import gui_telemetry_indicator_case as indicator_case  # noqa: E402
from run_gui_telemetry_indicator_memory import _aggregate as aggregate_indicator_memory  # noqa: E402
from run_gui_telemetry_viewer_matrix import _aggregate as aggregate_telemetry  # noqa: E402
from examples.cathode_ops_stress_demo import (  # noqa: E402
    _expected_first_lane_text,
    _find_node as find_cathode_node,
    parse_args as parse_cathode_args,
)
from run_gui_update_pipeline_runtime_ab import (  # noqa: E402
    _higher_is_better_regression,
    _lower_is_better_overhead,
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


def test_cathode_bounded_validation_cli_contract() -> None:
    args = parse_cathode_args(
        ["--validate-seconds", "10", "--validation-timeout", "20", "--report", "out.json"]
    )

    assert args.validate_seconds == 10.0
    assert args.validation_timeout == 20.0
    assert args.report == Path("out.json")
    assert args.no_live is False


def test_cathode_validation_helpers_find_native_final_state() -> None:
    tree = {"children": [{"id": "lane", "props": {"text": "78%"}}]}

    assert find_cathode_node(tree, "lane") == tree["children"][0]
    assert find_cathode_node(tree, "missing") is None
    assert _expected_first_lane_text(275) == "78%"


def test_update_pipeline_locality_indices_are_deterministic_distinct_and_bounded() -> None:
    first = _locality_indices(1_000, 20, 17)

    assert first == _locality_indices(1_000, 20, 17)
    assert len(first) == len(set(first)) == 20
    assert all(0 <= index < 1_000 for index in first)
    assert first != _locality_indices(1_000, 20, 18)
    assert _locality_indices(3, 20, 17) == _locality_indices(3, 3, 17)
    assert _locality_indices(0, 1, 17) == []


def test_runtime_ab_overhead_helpers_preserve_improvements_and_regressions() -> None:
    assert _lower_is_better_overhead(10.0, 10.5) == pytest.approx(5.0)
    assert _lower_is_better_overhead(10.0, 8.0) == pytest.approx(-20.0)
    assert _higher_is_better_regression(60.0, 57.0) == pytest.approx(5.0)
    assert _higher_is_better_regression(60.0, 63.0) == pytest.approx(-5.0)


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


def test_live_dashboard_memory_metrics_exclude_warmup_and_post_validation(
    monkeypatch,
) -> None:
    samples = iter(
        [
            {"rss_bytes": 10, "private_bytes": 7},
            {"rss_bytes": 20, "private_bytes": 12},
            {"rss_bytes": 30, "private_bytes": 17},
            {"rss_bytes": 40, "private_bytes": 22},
        ]
    )
    monkeypatch.setattr(live_dashboard_case, "_memory_bytes", lambda: next(samples))
    metrics = live_dashboard_case.RunMetrics(warmup_ticks=60, measure_ticks=120)
    metrics.measure_wall_start = 1.0
    metrics.measure_wall_end = 61.0

    for tick in (0, 60, 120, 180):
        metrics.sample(tick)

    report = metrics.report()

    assert report["rss_start_bytes"] == 10
    assert report["rss_end_bytes"] == 40
    assert report["rss_peak_bytes"] == 40
    assert report["measurement_memory"] == {
        "sample_count": 2,
        "rss_start_bytes": 20,
        "rss_end_bytes": 30,
        "rss_peak_bytes": 30,
        "rss_growth_bytes": 10,
        "rss_growth_bytes_per_minute": 10.0,
        "private_sample_count": 2,
        "private_start_bytes": 12,
        "private_end_bytes": 17,
        "private_peak_bytes": 17,
        "private_growth_bytes": 5,
        "private_growth_bytes_per_minute": 5.0,
    }
    assert [checkpoint["phase"] for checkpoint in report["checkpoints"]] == [
        "warmup",
        "measurement",
        "measurement",
        "post_measurement",
    ]


def test_telemetry_stages_scale_widgets_without_changing_trace_size() -> None:
    configs = list(telemetry_case.STAGES.values())

    assert [config.line_plots for config in configs] == [4, 8, 12, 16]
    assert [config.indicators for config in configs] == [24, 72, 160, 320]
    assert {config.line_points for config in configs} == {1_024}
    assert telemetry_case.TARGET_HZ == 30.0


def test_telemetry_data_and_final_indicator_values_are_deterministic() -> None:
    config = telemetry_case.STAGES["stage4"]
    data = telemetry_case.TelemetryData(config)
    x, y = data.line(15, 73)

    assert x.shape == y.shape == (1_024,)
    assert str(x.dtype) == str(y.dtype) == "float32"
    assert telemetry_case.TelemetryData.indicator(319, 73) == (
        72.8,
        "TM-319  072.8",
        True,
    )


def test_telemetry_aggregate_uses_native_dragon_frame_work() -> None:
    report = {
        "framework": "DragonGUI",
        "framework_version": "0.1.0",
        "stage": "stage1",
        "config": {"line_plots": 4, "indicators": 24, "line_points": 1_024},
        "build_ms": 1.0,
        "drain_recovery_ms": 0.2,
        "validation": {"passed": True, "check_count": 15, "failure_count": 0},
        "metrics": {
            "tick_throughput_hz": 30.0,
            "measurement_completed_ticks": 450,
            "measurement_dropped_ticks": 0,
            "submit_ms": {"median_ms": 1.0, "p95_ms": 2.0, "p99_ms": 3.0},
            "frame_ms": {"median_ms": 99.0, "p95_ms": 99.0, "p99_ms": 99.0},
            "frame_deadline_misses": 0,
            "process_cpu_percent_one_core": 20.0,
            "rss_peak_bytes": 100,
            "measurement_memory": {
                "rss_peak_bytes": 90,
                "rss_growth_bytes": 2,
                "private_start_bytes": 180,
                "private_peak_bytes": 200,
                "private_growth_bytes": 20,
            },
            "line_updates": 1_800,
            "control_updates": 32_400,
        },
        "native": {
            "layout_diagnostic_count": 0,
            "runtime": {
                "wall_fps": 30.0,
                "frame_timings": {
                    "work": {"p50_ms": 0.4, "p95_ms": 0.7, "p99_ms": 0.9}
                },
                "command_drain": {"timing": {"p95_ms": 2.5}},
                "command_queue": {"push_timing": {"p95_ms": 0.02}},
                "python": {
                    "task_drain_timing": {"max_ms": 3.0},
                    "task_queue_high_water": 1,
                    "tasks_coalesced": 0,
                },
            },
            "renderer": {
                "line_plot_renderer": {"source_point_count": 4_096, "point_count": 1_000}
            },
        },
    }

    aggregated = aggregate_telemetry([report])

    assert aggregated["frame_p50_ms"] == 0.4
    assert aggregated["frame_p95_ms"] == 0.7
    assert aggregated["frame_p99_ms"] == 0.9
    assert aggregated["measurement_private_start_bytes"] == 180
    assert aggregated["measurement_private_peak_bytes"] == 200
    assert aggregated["measurement_private_growth_bytes"] == 20
    assert aggregated["validation"]["passed"] is True


def test_indicator_decomposition_counts_actual_property_fanout() -> None:
    expected = {
        "labels": 321,
        "progress": 321,
        "leds": 641,
        "combined": 1_281,
    }

    assert indicator_case.COUNTS == (24, 72, 160, 320)
    for mode, properties in expected.items():
        config = indicator_case.IndicatorConfig(320, mode)
        assert config.properties_per_tick == properties


def test_indicator_memory_aggregate_excludes_invalid_capacity_samples() -> None:
    valid = {
        "validation_passed": True,
        "rss_start_bytes": 100,
        "rss_growth_bytes": 5,
        "private_start_bytes": 200,
        "private_growth_bytes": 3,
        "throughput_hz": 30.0,
    }
    invalid = {
        "validation_passed": False,
        "rss_start_bytes": 10_000,
        "rss_growth_bytes": 9_000,
        "private_start_bytes": 20_000,
        "private_growth_bytes": 18_000,
        "throughput_hz": 12.0,
    }

    aggregate = aggregate_indicator_memory([valid, invalid])

    assert aggregate["sample_count"] == 2
    assert aggregate["valid_sample_count"] == 1
    assert aggregate["all_valid"] is False
    assert aggregate["medians"]["rss_start_bytes"] == 100
    assert aggregate["medians"]["private_growth_bytes"] == 3
