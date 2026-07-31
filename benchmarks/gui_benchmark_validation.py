"""Correctness assertions shared by DragonGUI performance benchmarks.

Performance results are not trustworthy when the benchmarked content was not
retained, populated, laid out, or drained.  These helpers keep validation data
JSON-friendly and make failed invariants visible in raw benchmark artifacts.
"""

from __future__ import annotations

from typing import Any, Callable


class ValidationRecorder:
    def __init__(self) -> None:
        self.checks: list[dict[str, Any]] = []

    def equal(self, name: str, observed: Any, expected: Any, *, source: str) -> None:
        self.checks.append({
            "name": name,
            "passed": observed == expected,
            "expected": expected,
            "observed": observed,
            "source": source,
        })

    def require(
        self,
        name: str,
        observed: Any,
        predicate: Callable[[Any], bool],
        expected: str,
        *,
        source: str,
    ) -> None:
        try:
            passed = bool(predicate(observed))
        except (TypeError, ValueError, AttributeError):
            passed = False
        self.checks.append({
            "name": name,
            "passed": passed,
            "expected": expected,
            "observed": observed,
            "source": source,
        })

    def report(self) -> dict[str, Any]:
        failures = [check for check in self.checks if not check["passed"]]
        return {
            "passed": not failures,
            "check_count": len(self.checks),
            "failure_count": len(failures),
            "checks": self.checks,
        }


def find_tree_node(node: Any, widget_id: str) -> dict[str, Any] | None:
    if not isinstance(node, dict):
        return None
    if node.get("id") == widget_id:
        return node
    for child in node.get("children") or ():
        found = find_tree_node(child, widget_id)
        if found is not None:
            return found
    return None


def layout_issue_count(snapshot: dict[str, Any]) -> int:
    diagnostics = (((snapshot.get("gpu") or {}).get("layout") or {}).get("diagnostics") or {})
    return sum(len(entry.get("issues") or ()) for entry in diagnostics.values())


def add_common_runtime_checks(
    recorder: ValidationRecorder,
    snapshot: dict[str, Any],
    *,
    expected_widgets: int,
    minimum_frames: int,
) -> None:
    runtime = snapshot.get("runtime") or {}
    gpu = snapshot.get("gpu") or {}
    renderer = gpu.get("renderer") or {}
    drain = runtime.get("command_drain") or {}
    recorder.equal(
        "application frame reached readiness",
        runtime.get("startup_readiness"),
        "application_frame_presented",
        source="native runtime snapshot",
    )
    recorder.equal(
        "GPU remained ready",
        runtime.get("gpu_ready"),
        True,
        source="native runtime snapshot",
    )
    recorder.require(
        "requested frame window rendered",
        runtime.get("frames_rendered"),
        lambda value: int(value) >= minimum_frames,
        f">= {minimum_frames}",
        source="native runtime snapshot",
    )
    recorder.equal(
        "retained widget count",
        renderer.get("widget_count"),
        expected_widgets,
        source="native renderer snapshot",
    )
    recorder.equal(
        "command bridge drained",
        runtime.get("command_queue_depth"),
        0,
        source="native runtime snapshot",
    )
    recorder.equal(
        "no deferred command batch remained",
        drain.get("last_pending"),
        0,
        source="native command-drain snapshot",
    )
    recorder.equal(
        "layout diagnostics remained clean",
        layout_issue_count(snapshot),
        0,
        source="native layout diagnostics",
    )
