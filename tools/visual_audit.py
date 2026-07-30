from __future__ import annotations

import argparse
import ctypes
from ctypes import wintypes
from dataclasses import dataclass
import json
import math
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import tempfile
import textwrap
import time
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "examples" / "css_feature_probes" / "visual_audit_manifest.json"
DEFAULT_OUT = ROOT / "artifacts" / "visual_audit"
SIZE_ALIASES = {
    "desktop": None,
    "mobile": (390, 720),
}
LAYOUT_TORTURE_TARGETS = {
    "layout-flex-stress",
    "layout-panel-bounds",
    "layout-grid-masonry",
    "layout-overlay-collision",
    "layout-scrollable-composites",
    "layout-plot-embedding",
    "overflow-scrollbar",
    "responsive-layout",
}


@dataclass(frozen=True)
class WindowInfo:
    hwnd: int
    title: str
    rect: tuple[int, int, int, int]


def main() -> int:
    parser = argparse.ArgumentParser(description="Run DragonGUI visual audit probes.")
    parser.add_argument("--manifest", default=str(DEFAULT_MANIFEST))
    parser.add_argument("--list", action="store_true", help="List manifest targets and exit.")
    parser.add_argument("--target", action="append", default=[], help="Target id to run; repeatable.")
    parser.add_argument(
        "--category",
        default="all",
        choices=["widgets", "layout", "css", "plots", "all"],
        help="Target category to run when --target is omitted.",
    )
    parser.add_argument("--out", default=str(DEFAULT_OUT), help="Artifact output directory.")
    parser.add_argument("--wait-ms", type=int, default=1800, help="Delay before screenshot capture.")
    parser.add_argument(
        "--sizes",
        default="desktop",
        help="Comma-separated aliases desktop,mobile or explicit WIDTHxHEIGHT values.",
    )
    parser.add_argument(
        "--scales",
        default="manifest",
        help="Comma-separated scale factors, or manifest to use each target's configured scales.",
    )
    parser.add_argument("--no-capture", action="store_true", help="Generate report shell only.")
    parser.add_argument(
        "--append",
        action="store_true",
        help="Merge new results into an existing report.json instead of replacing it.",
    )
    parser.add_argument(
        "--skip-existing",
        action="store_true",
        help="When appending, skip target ids already present in report.json.",
    )
    parser.add_argument(
        "--timeout-ms",
        type=int,
        default=12000,
        help="Maximum time to wait for each probe process.",
    )
    args = parser.parse_args()

    manifest_path = resolve_path(args.manifest)
    targets = load_manifest(manifest_path)

    if args.list:
        for target in targets:
            manual = " manual" if target.get("manual") else ""
            print(f"{target['id']}\t{target.get('category', '')}\t{target['script']}{manual}")
        return 0

    out_dir = resolve_path(args.out)
    existing_results = load_existing_results(out_dir, strict=args.append) if args.append else []
    selected = select_targets(targets, args.target, args.category)
    if args.append and args.skip_existing:
        existing_ids = {str(result.get("id")) for result in existing_results}
        selected = [target for target in selected if str(target.get("id")) not in existing_ids]
    if not selected:
        if args.append and existing_results:
            print("All selected targets are already present; no audit probes were run.")
            write_report(out_dir, order_results(existing_results, targets))
            return 0
        print("No targets selected.", file=sys.stderr)
        return 2

    sizes = parse_sizes(args.sizes)
    scales = parse_scales(args.scales)
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "screenshots").mkdir(exist_ok=True)
    (out_dir / "snapshots").mkdir(exist_ok=True)
    (out_dir / "logs").mkdir(exist_ok=True)

    results = []
    for target in selected:
        result = run_target(
            target,
            out_dir=out_dir,
            wait_ms=max(0, args.wait_ms),
            timeout_ms=max(args.timeout_ms, args.wait_ms + 1000),
            size_selectors=sizes,
            scale_selectors=scales,
            no_capture=args.no_capture,
        )
        results.append(result)
        print(f"{result['id']}: {result['status']} ({result['notes']})")

    if args.append:
        results = merge_results(existing_results, results, targets)
    write_report(out_dir, results)
    return 1 if any(result["status"] == "fail" for result in results) else 0


def resolve_path(value: str | Path) -> Path:
    path = Path(value)
    if not path.is_absolute():
        path = ROOT / path
    return path


def load_manifest(path: Path) -> list[dict[str, Any]]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, list):
        raise ValueError("visual audit manifest must be a JSON list")
    seen: set[str] = set()
    for item in data:
        if not isinstance(item, dict):
            raise ValueError("manifest entries must be JSON objects")
        for key in ("id", "name", "script", "category", "features", "sizes", "manual"):
            if key not in item:
                raise ValueError(f"manifest entry is missing {key!r}: {item!r}")
        target_id = str(item["id"])
        if target_id in seen:
            raise ValueError(f"duplicate visual audit target id: {target_id}")
        seen.add(target_id)
        script = resolve_path(str(item["script"]))
        if not script.exists():
            raise ValueError(f"probe script does not exist for {target_id}: {script}")
        script_args = item.get("args", [])
        if not isinstance(script_args, list) or not all(
            isinstance(arg, str) and arg and "\x00" not in arg for arg in script_args
        ):
            raise ValueError(
                f"visual audit target {target_id} args must be a list of non-empty strings"
            )
        target_states(item)
    return data


def target_states(target: dict[str, Any]) -> list[dict[str, Any]]:
    raw_states = target.get("states")
    if raw_states is None:
        return [{"name": "default", "route": None, "actions": []}]
    if not isinstance(raw_states, list) or not raw_states:
        raise ValueError(f"visual audit target {target.get('id')} states must be a non-empty list")
    states: list[dict[str, Any]] = []
    seen: set[str] = set()
    for raw in raw_states:
        if not isinstance(raw, dict) or not isinstance(raw.get("name"), str):
            raise ValueError(f"visual audit target {target.get('id')} has an invalid state")
        name = raw["name"].strip()
        if not name or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", name):
            raise ValueError(f"visual audit state name must be a stable identifier: {name!r}")
        if name in seen:
            raise ValueError(f"duplicate visual audit state name for {target.get('id')}: {name}")
        route = raw.get("route")
        if route is not None and (not isinstance(route, str) or not route.strip()):
            raise ValueError(f"visual audit state {name} route must be a non-empty string")
        actions = raw.get("actions", [])
        if not isinstance(actions, list) or not all(
            isinstance(action, str) and action.strip() for action in actions
        ):
            raise ValueError(f"visual audit state {name} actions must be non-empty strings")
        for action in actions:
            validate_state_action(action)
        seen.add(name)
        states.append({"name": name, "route": route, "actions": actions})
    return states


def validate_state_action(action: str) -> None:
    if re.fullmatch(
        r"(?:click|native-click|right-click|hover):#[A-Za-z_][A-Za-z0-9_.:-]*",
        action,
    ):
        return
    if re.fullmatch(r"assert-window-state:(?:normal|maximized|minimized)", action):
        return
    if re.fullmatch(r"set-window-state:normal", action):
        return
    if re.fullmatch(r"assert-focus:#[A-Za-z_][A-Za-z0-9_.:-]*", action):
        return
    if re.fullmatch(r"assert-system-menu:(?:open|closed)", action):
        return
    if re.fullmatch(r"key:(?:tab|enter|space|escape|alt-space)", action):
        return
    if re.fullmatch(r"type:#[A-Za-z_][A-Za-z0-9_.:-]*=.+", action):
        return
    if re.fullmatch(r"scroll:#[A-Za-z_][A-Za-z0-9_.:-]*=-?\d+(?:\.\d+)?,-?\d+(?:\.\d+)?", action):
        return
    if re.fullmatch(r"resize:\d+x\d+", action):
        return
    if re.fullmatch(r"wait:\d+", action):
        return
    raise ValueError(f"unsupported visual audit state action: {action!r}")


def select_targets(
    targets: list[dict[str, Any]],
    target_ids: list[str],
    category: str,
) -> list[dict[str, Any]]:
    if target_ids:
        by_id = {str(target["id"]): target for target in targets}
        missing = [target_id for target_id in target_ids if target_id not in by_id]
        if missing:
            raise SystemExit(f"Unknown target id(s): {', '.join(missing)}")
        return [by_id[target_id] for target_id in target_ids]
    if category == "all":
        return targets
    return [target for target in targets if target.get("category") == category]


def load_existing_results(out_dir: Path, *, strict: bool = False) -> list[dict[str, Any]]:
    path = out_dir / "report.json"
    if not path.exists():
        return []
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except OSError as exc:
        if strict:
            raise SystemExit(f"Could not read existing visual audit report {path}: {exc}") from exc
        return []
    except json.JSONDecodeError as exc:
        if strict:
            raise SystemExit(f"Existing visual audit report is malformed JSON: {path}: {exc}") from exc
        return []
    if not isinstance(data, list):
        if strict:
            raise SystemExit(f"Existing visual audit report must be a JSON list: {path}")
        return []
    return [item for item in data if isinstance(item, dict) and "id" in item]


def merge_results(
    existing: list[dict[str, Any]],
    new: list[dict[str, Any]],
    manifest_targets: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    merged = {str(result["id"]): result for result in existing if "id" in result}
    for result in new:
        result_id = str(result["id"])
        if result_id not in merged:
            merged[result_id] = result
            continue
        previous = merged[result_id]
        combined = {**previous, **result}
        for key in (
            "screenshots",
            "snapshots",
            "logs",
            "reproduction",
            "unmatched_selectors",
            "captures",
            "layout_issues",
            "diagnostic_comparisons",
        ):
            combined[key] = unique_list(previous.get(key, []), result.get(key, []))
        combined["layout_issue_counts"] = merge_count_maps(
            previous.get("layout_issue_counts"),
            result.get("layout_issue_counts"),
        )
        combined["notes"] = combine_notes(previous.get("notes", ""), result.get("notes", ""))
        combined["status"] = combined_status(str(previous.get("status")), str(result.get("status")))
        combined["priority"] = combined_priority(str(previous.get("priority")), str(result.get("priority")))
        merged[result_id] = combined
    return order_results(list(merged.values()), manifest_targets)


def order_results(
    results: list[dict[str, Any]],
    manifest_targets: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    order = {str(target["id"]): index for index, target in enumerate(manifest_targets)}
    return sorted(results, key=lambda result: order.get(str(result.get("id")), len(order)))


def unique_list(*values: Any) -> list[Any]:
    out: list[Any] = []
    seen: set[str] = set()
    for value in values:
        if not isinstance(value, list):
            continue
        for item in value:
            key = str(item)
            if key in seen:
                continue
            seen.add(key)
            out.append(item)
    return out


def merge_count_maps(*values: Any) -> dict[str, int]:
    merged: dict[str, int] = {}
    for value in values:
        if not isinstance(value, dict):
            continue
        for key, count in value.items():
            if isinstance(count, int) and count >= 0:
                merged[str(key)] = merged.get(str(key), 0) + count
    return merged


def combine_notes(previous: str, current: str) -> str:
    previous = str(previous).strip()
    current = str(current).strip()
    if not previous:
        return current
    if not current or current == previous:
        return previous
    if current in previous:
        return previous
    return f"{previous} Additional run: {current}"


def combined_status(previous: str, current: str) -> str:
    rank = {"pass": 0, "needs_manual_interaction": 1, "blocked": 2, "fail": 3}
    return previous if rank.get(previous, 0) >= rank.get(current, 0) else current


def combined_priority(previous: str, current: str) -> str:
    rank = {"low": 0, "medium": 1, "high": 2}
    return previous if rank.get(previous, 0) >= rank.get(current, 0) else current


def parse_sizes(value: str) -> list[tuple[int, int] | None]:
    parsed: list[tuple[int, int] | None] = []
    for raw in value.split(","):
        part = raw.strip().lower()
        if not part:
            continue
        if part in SIZE_ALIASES:
            parsed.append(SIZE_ALIASES[part])
            continue
        if "x" not in part:
            raise SystemExit(f"Invalid --sizes entry {raw!r}; use desktop, mobile, or WIDTHxHEIGHT.")
        width_text, height_text = part.split("x", 1)
        parsed.append((int(width_text), int(height_text)))
    return parsed or [None]


def parse_scales(value: str) -> list[float | None]:
    parsed: list[float | None] = []
    for raw in value.split(","):
        part = raw.strip().lower()
        if not part:
            continue
        if part == "manifest":
            parsed.append(None)
            continue
        try:
            scale = float(part)
        except ValueError as exc:
            raise SystemExit(
                f"Invalid --scales entry {raw!r}; use manifest or positive numbers."
            ) from exc
        if not 0.5 <= scale <= 4.0:
            raise SystemExit(f"Invalid --scales entry {raw!r}; expected a value from 0.5 to 4.0.")
        parsed.append(scale)
    return parsed or [None]


def run_target(
    target: dict[str, Any],
    *,
    out_dir: Path,
    wait_ms: int,
    timeout_ms: int,
    size_selectors: list[tuple[int, int] | None],
    no_capture: bool,
    scale_selectors: list[float | None] | None = None,
) -> dict[str, Any]:
    target_id = str(target["id"])
    script = resolve_path(str(target["script"]))
    script_args = list(target.get("args", []))
    selected_sizes = target_sizes(target, size_selectors)
    selected_scales = target_scales(target, scale_selectors or [None])
    result = {
        "id": target_id,
        "name": target["name"],
        "script": str(script.relative_to(ROOT)),
        "args": script_args,
        "category": target["category"],
        "features": target["features"],
        "manual": bool(target.get("manual")),
        "status": "needs_manual_interaction" if target.get("manual") else "pass",
        "priority": "medium" if target.get("manual") else "low",
        "notes": str(target.get("notes") or "No visual issue recorded by automated first pass."),
        "suspected_modules": suspected_modules(target),
        "screenshots": [],
        "snapshots": [],
        "captures": [],
        "diagnostic_comparisons": [],
        "layout_issue_counts": {},
        "layout_issues": [],
        "unmatched_selectors": [],
        "logs": [],
        "reproduction": [],
    }

    if no_capture:
        result["status"] = "blocked"
        result["priority"] = "low"
        result["notes"] = "--no-capture was used; report shell generated without launching probe."
        result["reproduction"] = [
            subprocess.list2cmdline(["python", str(target["script"]), *script_args])
        ]
        return result

    captures = [
        (size, scale, state)
        for size in selected_sizes
        for scale in selected_scales
        for state in target_states(target)
    ]
    for index, (size, scale_factor, capture_state) in enumerate(captures, start=1):
        state_name = str(capture_state["name"])
        route = capture_state.get("route")
        actions = list(capture_state.get("actions", []))
        label = capture_label(
            size,
            scale_factor,
            index,
            state_name=None if state_name == "default" else state_name,
        )
        stdout_path = out_dir / "logs" / f"{target_id}-{label}.stdout.txt"
        stderr_path = out_dir / "logs" / f"{target_id}-{label}.stderr.txt"
        snapshot_path = out_dir / "snapshots" / f"{target_id}-{label}.json"
        screenshot_path = out_dir / "screenshots" / f"{target_id}-{label}.png"
        run_result = run_probe_process(
            script,
            target_id=target_id,
            snapshot_path=snapshot_path,
            wait_ms=wait_ms,
            timeout_ms=timeout_ms,
            size=size,
            scale_factor=scale_factor,
            resize_checkpoints=[
                tuple(checkpoint) for checkpoint in target.get("resize_checkpoints", [])
            ],
            screenshot_path=screenshot_path,
            stdout_path=stdout_path,
            stderr_path=stderr_path,
            route=route,
            actions=actions,
            script_args=script_args,
        )
        result["logs"].extend(
            [relative_artifact(stdout_path, out_dir), relative_artifact(stderr_path, out_dir)]
        )
        result["reproduction"].append(
            f"python tools/visual_audit.py --target {target_id} "
            f"--sizes {size_label(size, index)} --scales {scale_factor:g}"
            + (f" # state={state_name}" if state_name != "default" else "")
        )
        capture_record = {
            "name": label,
            "state": state_name,
            "route": route,
            "actions": actions,
            "size": f"{size[0]}x{size[1]}" if size is not None else "default",
            "scale": float(scale_factor),
            "screenshot": None,
            "snapshot": None,
            "error": None,
            "diagnostic_counts": {},
        }
        if run_result["screenshot"]:
            screenshot_artifact = relative_artifact(screenshot_path, out_dir)
            result["screenshots"].append(screenshot_artifact)
            capture_record["screenshot"] = screenshot_artifact
        if run_result["snapshot"]:
            snapshot_artifact = relative_artifact(snapshot_path, out_dir)
            result["snapshots"].append(snapshot_artifact)
            capture_record["snapshot"] = snapshot_artifact
            capture_error = snapshot_capture_error(snapshot_path)
            if capture_error:
                capture_record["error"] = capture_error
                capture_record["diagnostic_counts"]["capture-error"] = 1
                run_result["status"] = "fail"
                run_result["notes"] = combine_notes(
                    run_result["notes"],
                    f"Capture action or snapshot failed: {capture_error}",
                )
            screenshot_artifact = (
                relative_artifact(screenshot_path, out_dir)
                if run_result["screenshot"]
                else None
            )
            capture_issues = snapshot_layout_diagnostic_entries(
                snapshot_path,
                size=size,
                scale_factor=scale_factor,
                snapshot_artifact=snapshot_artifact,
                screenshot_artifact=screenshot_artifact,
                route=route,
                state=state_name,
                artifact_root=out_dir,
            )
            result["layout_issues"].extend(capture_issues)
            for issue in capture_issues:
                code = issue["code"]
                result["layout_issue_counts"][code] = (
                    result["layout_issue_counts"].get(code, 0) + 1
                )
                capture_record["diagnostic_counts"][code] = (
                    capture_record["diagnostic_counts"].get(code, 0) + 1
                )
        checkpoint_paths = sorted(
            snapshot_path.parent.glob(f"{snapshot_path.stem}-resize-*.json")
        )
        for checkpoint_path in checkpoint_paths:
            result["snapshots"].append(relative_artifact(checkpoint_path, out_dir))
        capture_unmatched = sorted(
            {
                selector
                for artifact_path in [snapshot_path, *checkpoint_paths]
                for selector in unmatched_user_selectors(artifact_path)
            }
        )
        new_unmatched = [
            selector
            for selector in capture_unmatched
            if selector not in result["unmatched_selectors"]
        ]
        result["unmatched_selectors"].extend(new_unmatched)
        if new_unmatched:
            result["priority"] = "medium"
            result["notes"] = combine_notes(
                result["notes"],
                "Active user selectors matched zero nodes: "
                + ", ".join(new_unmatched[:6]),
            )
            if target.get("strict_css"):
                run_result["status"] = "fail"
                run_result["notes"] = combine_notes(
                    run_result["notes"],
                    "Strict CSS check failed because active user selectors matched zero nodes.",
                )
        if target_id in LAYOUT_TORTURE_TARGETS:
            layout_violations: list[str] = []
            for artifact_path in [snapshot_path, *checkpoint_paths]:
                violations, _counts = validate_layout_snapshot_relations(artifact_path)
                layout_violations.extend(violations)
                layout_violations.extend(
                    validate_layout_target_relationships(artifact_path, target_id)
                )
            start_paths = [
                path for path in checkpoint_paths if "-resize-0-start-" in path.name
            ]
            if start_paths:
                return_paths = [
                    path for path in checkpoint_paths if "-resize-0-start-" not in path.name
                ]
                round_trip_path = max(
                    return_paths,
                    key=lambda path: int(
                        re.search(r"-resize-(\d+)-", path.name).group(1)
                    ),
                    default=snapshot_path,
                )
                layout_violations.extend(
                    validate_layout_resize_round_trip(start_paths[0], round_trip_path)
                )
            if layout_violations:
                run_result["status"] = "fail"
                run_result["notes"] = combine_notes(
                    run_result["notes"],
                    "Layout relational check failed: "
                    + "; ".join(layout_violations[:4]),
                )
        badge_violations = (
            validate_badge_layout_snapshot(snapshot_path) if target_id == "badge-layout" else []
        )
        if badge_violations:
            run_result["status"] = "fail"
            run_result["notes"] = combine_notes(
                run_result["notes"],
                "Badge bounds check failed: " + "; ".join(badge_violations[:4]),
            )
        scroll_violations = (
            validate_professional_demo_scroll_snapshot(snapshot_path)
            if target_id.startswith("all-features-professional-")
            else []
        )
        if scroll_violations:
            run_result["status"] = "fail"
            run_result["notes"] = combine_notes(
                run_result["notes"],
                "Scroll reachability check failed: " + "; ".join(scroll_violations[:4]),
            )
        splitter_violations = (
            validate_professional_demo_splitter_snapshot(snapshot_path)
            if target_id.startswith("all-features-professional-")
            else []
        )
        if splitter_violations:
            run_result["status"] = "fail"
            run_result["notes"] = combine_notes(
                run_result["notes"],
                "Splitter utilization check failed: " + "; ".join(splitter_violations[:4]),
            )
        scatter_violations = (
            validate_professional_explore_scatter_snapshot(snapshot_path)
            if target_id == "all-features-professional-explore"
            else []
        )
        if scatter_violations:
            run_result["status"] = "fail"
            run_result["notes"] = combine_notes(
                run_result["notes"],
                "Scatter startup framing check failed: " + "; ".join(scatter_violations[:4]),
            )
        adjacent_violations = (
            validate_adjacent_scatter_interaction_log(stdout_path)
            if target_id == "adjacent-scatter-interaction"
            else []
        )
        if target_id == "adjacent-scatter-interaction" and run_result["screenshot"]:
            adjacent_violations.extend(
                validate_adjacent_scatter_interaction_screenshot(screenshot_path, snapshot_path)
            )
        if adjacent_violations:
            run_result["status"] = "fail"
            run_result["notes"] = combine_notes(
                run_result["notes"],
                "Adjacent scatter interaction check failed: "
                + "; ".join(adjacent_violations[:4]),
            )
        if run_result["status"] == "blocked":
            result["status"] = "blocked"
            result["priority"] = "high"
            result["notes"] = combine_notes(result["notes"], run_result["notes"])
        elif run_result["status"] == "fail":
            result["status"] = "fail"
            result["priority"] = "high"
            result["notes"] = combine_notes(result["notes"], run_result["notes"])
        else:
            result["notes"] = combine_notes(result["notes"], run_result["notes"])
        result["captures"].append(capture_record)

    result["diagnostic_comparisons"] = compare_capture_diagnostics(result["captures"])
    return result


def _read_layout_snapshot(
    snapshot_path: Path,
) -> tuple[dict[str, Any] | None, dict[str, Any] | None, list[str]]:
    try:
        snapshot = json.loads(snapshot_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return None, None, [f"{snapshot_path.name}: could not read layout snapshot: {exc}"]
    gpu = snapshot.get("gpu") if isinstance(snapshot, dict) else None
    layout = gpu.get("layout") if isinstance(gpu, dict) else None
    if not isinstance(gpu, dict) or not isinstance(layout, dict):
        return None, None, [f"{snapshot_path.name}: missing gpu.layout"]
    return gpu, layout, []


def unmatched_user_selectors(snapshot_path: Path) -> list[str]:
    try:
        snapshot = json.loads(snapshot_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return []
    gpu = snapshot.get("gpu") if isinstance(snapshot, dict) else None
    stylesheets = gpu.get("stylesheets") if isinstance(gpu, dict) else None
    unmatched = (
        stylesheets.get("unmatched_user_selectors")
        if isinstance(stylesheets, dict)
        else None
    )
    if not isinstance(unmatched, list):
        return []
    return sorted(
        {
            selector
            for selector in unmatched
            if isinstance(selector, str) and selector
        }
    )


def snapshot_capture_error(snapshot_path: Path) -> str | None:
    try:
        snapshot = json.loads(snapshot_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    error = snapshot.get("error") if isinstance(snapshot, dict) else None
    return error.strip() if isinstance(error, str) and error.strip() else None


def _snapshot_rect(value: object) -> tuple[float, float, float, float] | None:
    if not isinstance(value, dict):
        return None
    try:
        rect = tuple(float(value[key]) for key in ("x", "y", "w", "h"))
    except (KeyError, TypeError, ValueError):
        return None
    return rect if len(rect) == 4 else None


def _rect_contains(
    outer: tuple[float, float, float, float],
    inner: tuple[float, float, float, float],
    tolerance: float = 0.75,
) -> bool:
    ox, oy, ow, oh = outer
    ix, iy, iw, ih = inner
    return (
        ix >= ox - tolerance
        and iy >= oy - tolerance
        and ix + iw <= ox + ow + tolerance
        and iy + ih <= oy + oh + tolerance
    )


def validate_layout_snapshot_relations(
    snapshot_path: Path,
) -> tuple[list[str], dict[str, int]]:
    gpu, layout, violations = _read_layout_snapshot(snapshot_path)
    if gpu is None or layout is None:
        return violations, {}
    if layout.get("schema_version") != 1:
        violations.append(
            f"{snapshot_path.name}: unsupported layout schema {layout.get('schema_version')!r}"
        )
    rects = layout.get("rects")
    clips = layout.get("clips")
    paint_clips = layout.get("paint_clips")
    diagnostics = layout.get("diagnostics")
    if not isinstance(rects, dict) or not isinstance(clips, dict):
        return [*violations, f"{snapshot_path.name}: missing rect or clip maps"], {}
    if not isinstance(paint_clips, dict):
        return [*violations, f"{snapshot_path.name}: missing paint_clips"], {}

    parsed_rects: dict[str, tuple[float, float, float, float]] = {}
    for map_name, geometry_map in (
        ("rects", rects),
        ("clips", clips),
        ("paint_clips", paint_clips),
    ):
        for widget_id, value in geometry_map.items():
            rect = _snapshot_rect(value)
            if rect is None or not all(math.isfinite(component) for component in rect):
                violations.append(
                    f"{snapshot_path.name}: {map_name}.{widget_id} is malformed or non-finite"
                )
                continue
            if rect[2] < 0.0 or rect[3] < 0.0:
                violations.append(
                    f"{snapshot_path.name}: {map_name}.{widget_id} has negative size"
                )
            if map_name == "rects":
                parsed_rects[str(widget_id)] = rect

    for widget_id, clip_value in clips.items():
        clip = _snapshot_rect(clip_value)
        paint_clip = _snapshot_rect(paint_clips.get(widget_id))
        if clip is not None and paint_clip is not None and not _rect_contains(paint_clip, clip):
            violations.append(
                f"{snapshot_path.name}: clip for {widget_id} escapes its paint clip"
            )

    tree = gpu.get("tree")
    root_id = tree.get("id") if isinstance(tree, dict) else None
    window = gpu.get("window")
    if isinstance(root_id, str) and isinstance(window, dict) and root_id in parsed_rects:
        root = parsed_rects[root_id]
        width = safe_float(window.get("width"), -1.0)
        height = safe_float(window.get("height"), -1.0)
        if abs(root[2] - width) > 1.0 or abs(root[3] - height) > 1.0:
            violations.append(
                f"{snapshot_path.name}: root {root_id} does not match physical window size"
            )

    for axis in ("x", "y"):
        ranges = layout.get(f"scroll_max_{axis}")
        if not isinstance(ranges, dict):
            violations.append(f"{snapshot_path.name}: missing scroll_max_{axis}")
            continue
        for owner_id, value in ranges.items():
            maximum = safe_float(value, float("nan"))
            if not math.isfinite(maximum) or maximum < 0.0:
                violations.append(
                    f"{snapshot_path.name}: invalid scroll_max_{axis} for {owner_id}"
                )
            if owner_id not in rects:
                violations.append(
                    f"{snapshot_path.name}: scroll_max_{axis} owner {owner_id} has no rect"
                )

    issue_counts: dict[str, int] = {}
    if not isinstance(diagnostics, dict):
        violations.append(f"{snapshot_path.name}: missing diagnostics")
    else:
        for diagnostic in diagnostics.values():
            issues = diagnostic.get("issues") if isinstance(diagnostic, dict) else None
            if not isinstance(issues, list):
                continue
            for issue in issues:
                code = str(issue.get("code", "unknown")) if isinstance(issue, dict) else "unknown"
                issue_counts[code] = issue_counts.get(code, 0) + 1
                message = issue.get("message") if isinstance(issue, dict) else None
                violations.append(
                    f"{snapshot_path.name}: native {code}: {message or 'layout issue'}"
                )
    return violations, issue_counts


def validate_layout_resize_round_trip(start_path: Path, final_path: Path) -> list[str]:
    _start_gpu, start, violations = _read_layout_snapshot(start_path)
    _final_gpu, final, final_violations = _read_layout_snapshot(final_path)
    violations.extend(final_violations)
    if start is None or final is None:
        return violations
    for map_name in (
        "rects",
        "clips",
        "paint_clips",
        "scroll_x",
        "scroll_y",
        "scroll_max_x",
        "scroll_max_y",
    ):
        first = start.get(map_name)
        second = final.get(map_name)
        if not isinstance(first, dict) or not isinstance(second, dict):
            violations.append(f"{final_path.name}: round-trip map {map_name} is missing")
            continue
        if set(first) != set(second):
            violations.append(f"{final_path.name}: round-trip keys changed in {map_name}")
            continue
        for widget_id, first_value in first.items():
            second_value = second[widget_id]
            first_rect = _snapshot_rect(first_value)
            second_rect = _snapshot_rect(second_value)
            if first_rect is not None and second_rect is not None:
                equal = all(
                    abs(left - right) <= 0.75
                    for left, right in zip(first_rect, second_rect, strict=True)
                )
            else:
                left = safe_float(first_value, float("nan"))
                right = safe_float(second_value, float("nan"))
                equal = math.isfinite(left) and math.isfinite(right) and abs(left - right) <= 0.75
            if not equal:
                violations.append(
                    f"{final_path.name}: resize round trip changed {map_name}.{widget_id}"
                )
                if len(violations) >= 8:
                    return violations
    return violations


def _walk_snapshot_tree(node: object) -> list[dict[str, Any]]:
    if not isinstance(node, dict):
        return []
    nodes = [node]
    children = node.get("children")
    if isinstance(children, list):
        for child in children:
            nodes.extend(_walk_snapshot_tree(child))
    return nodes


def snapshot_layout_diagnostic_entries(
    snapshot_path: Path,
    *,
    size: tuple[int, int] | None,
    scale_factor: float | None,
    snapshot_artifact: str,
    screenshot_artifact: str | None,
    route: str | None = None,
    state: str | None = None,
    artifact_root: Path | None = None,
) -> list[dict[str, Any]]:
    """Return report-ready native diagnostics with capture and page context."""
    gpu, layout, _violations = _read_layout_snapshot(snapshot_path)
    if gpu is None or layout is None:
        return []
    diagnostics = layout.get("diagnostics")
    if not isinstance(diagnostics, dict):
        return []

    widget_types: dict[str, str] = {}
    page_by_widget: dict[str, str | None] = {}
    nodes_by_widget: dict[str, dict[str, Any]] = {}

    def collect(node: object, active_page: str | None = None) -> None:
        if not isinstance(node, dict):
            return
        widget_id = node.get("id")
        widget_type = node.get("type")
        if isinstance(widget_id, str):
            widget_types[widget_id] = str(widget_type or "unknown")
            nodes_by_widget[widget_id] = node
            if widget_type == "page":
                active_page = widget_id
            page_by_widget[widget_id] = active_page
        children = node.get("children")
        if isinstance(children, list):
            for child in children:
                collect(child, active_page)

    collect(gpu.get("tree"))
    size_value = f"{size[0]}x{size[1]}" if size is not None else "default"
    scale_value = float(scale_factor) if scale_factor is not None else 1.0
    entries: list[dict[str, Any]] = []
    for diagnostic_id, diagnostic in diagnostics.items():
        if not isinstance(diagnostic_id, str) or not isinstance(diagnostic, dict):
            continue
        issues = diagnostic.get("issues")
        if not isinstance(issues, list):
            continue
        for issue in issues:
            if not isinstance(issue, dict):
                continue
            widget_id = str(issue.get("widget_id") or diagnostic_id)
            entry = {
                    "code": str(issue.get("code") or "unknown"),
                    "severity": str(issue.get("severity") or "error"),
                    "widget_id": widget_id,
                    "widget_type": str(
                        issue.get("widget_type")
                        or widget_types.get(widget_id)
                        or "unknown"
                    ),
                    "page_id": page_by_widget.get(widget_id),
                    "size": size_value,
                    "scale": scale_value,
                    "message": str(issue.get("message") or "layout issue"),
                    "snapshot": snapshot_artifact,
                    "screenshot": screenshot_artifact,
                }
            if route is not None:
                entry["route"] = route
            if state is not None:
                entry["state"] = state
            if artifact_root is not None:
                safe_widget = re.sub(r"[^A-Za-z0-9_.-]+", "-", widget_id).strip("-") or "node"
                detail_path = (
                    artifact_root
                    / "diagnostics"
                    / f"{snapshot_path.stem}-{safe_widget}.json"
                )
                detail_path.parent.mkdir(parents=True, exist_ok=True)
                computed_styles = gpu.get("computed_styles")
                detail_path.write_text(
                    json.dumps(
                        {
                            "capture": {
                                "size": size_value,
                                "scale": scale_value,
                                "route": route,
                                "state": state,
                            },
                            "node": nodes_by_widget.get(widget_id),
                            "computed_style": (
                                computed_styles.get(widget_id)
                                if isinstance(computed_styles, dict)
                                else None
                            ),
                            "layout_diagnostic": diagnostic,
                            "issue": issue,
                        },
                        indent=2,
                        sort_keys=True,
                    ),
                    encoding="utf-8",
                )
                entry["node_data"] = relative_artifact(detail_path, artifact_root)
            entries.append(entry)
    return sorted(
        entries,
        key=lambda item: (
            item["code"],
            item["widget_id"],
            str(item["page_id"] or ""),
            item["size"],
            item["scale"],
        ),
    )


def _node_classes(node: dict[str, Any]) -> set[str]:
    value = node.get("class")
    return set(value.split()) if isinstance(value, str) else set()


def validate_layout_target_relationships(snapshot_path: Path, target_id: str) -> list[str]:
    gpu, layout, violations = _read_layout_snapshot(snapshot_path)
    if gpu is None or layout is None:
        return violations
    rects = layout.get("rects")
    clips = layout.get("clips")
    if not isinstance(rects, dict) or not isinstance(clips, dict):
        return [f"{snapshot_path.name}: target relationships need rect and clip maps"]
    tree = gpu.get("tree")
    nodes = _walk_snapshot_tree(tree)
    by_class: dict[str, list[dict[str, Any]]] = {}
    for node in nodes:
        for class_name in _node_classes(node):
            by_class.setdefault(class_name, []).append(node)

    def node_rect(node: dict[str, Any]) -> tuple[float, float, float, float] | None:
        node_id = node.get("id")
        return _snapshot_rect(rects.get(node_id)) if isinstance(node_id, str) else None

    def require_class(class_name: str) -> list[dict[str, Any]]:
        matched = by_class.get(class_name, [])
        if not matched:
            violations.append(f"{snapshot_path.name}: missing .{class_name} relationship target")
        return matched

    def require_owned_scroll(class_name: str, *axes: str) -> None:
        for node in require_class(class_name):
            node_id = node.get("id")
            if not isinstance(node_id, str):
                continue
            for axis in axes:
                ranges = layout.get(f"scroll_max_{axis}")
                maximum = safe_float(ranges.get(node_id), -1.0) if isinstance(ranges, dict) else -1.0
                if maximum <= 0.5:
                    violations.append(
                        f"{snapshot_path.name}: .{class_name} {node_id} lacks owned {axis}-scroll"
                    )

    def content_exceeds_node(node: dict[str, object], axis: str) -> bool:
        owner = node_rect(node)
        if owner is None:
            return False
        edge_index = 0 if axis == "x" else 1
        size_index = 2 if axis == "x" else 3
        owner_end = owner[edge_index] + owner[size_index]
        pending = list(node.get("children") or [])
        while pending:
            child = pending.pop()
            if not isinstance(child, dict):
                continue
            child_rect = node_rect(child)
            if (
                child_rect is not None
                and child_rect[edge_index] + child_rect[size_index] > owner_end + 1.0
            ):
                return True
            pending.extend(child.get("children") or [])
        return False

    def require_children_nonoverlap(class_name: str) -> None:
        for parent in require_class(class_name):
            children = parent.get("children")
            if not isinstance(children, list):
                continue
            child_rects = [
                (str(child.get("id")), node_rect(child))
                for child in children
                if isinstance(child, dict)
            ]
            for index, (first_id, first) in enumerate(child_rects):
                if first is None or first[2] <= 0.0 or first[3] <= 0.0:
                    continue
                for second_id, second in child_rects[index + 1 :]:
                    if second is None or second[2] <= 0.0 or second[3] <= 0.0:
                        continue
                    overlap_w = min(first[0] + first[2], second[0] + second[2]) - max(
                        first[0], second[0]
                    )
                    overlap_h = min(first[1] + first[3], second[1] + second[3]) - max(
                        first[1], second[1]
                    )
                    if overlap_w > 1.0 and overlap_h > 1.0:
                        violations.append(
                            f"{snapshot_path.name}: .{class_name} children "
                            f"{first_id} and {second_id} overlap"
                        )

    if target_id == "layout-flex-stress":
        require_children_nonoverlap("stress-row")
        require_children_nonoverlap("two-up")
    elif target_id == "layout-panel-bounds":
        for panel in require_class("fixed"):
            panel_rect = node_rect(panel)
            panel_id = panel.get("id")
            clip = _snapshot_rect(clips.get(panel_id)) if isinstance(panel_id, str) else None
            if (
                panel_rect is not None
                and clip is not None
                and clip[2] > 0.0
                and clip[3] > 0.0
                and not _rect_contains(panel_rect, clip)
            ):
                violations.append(
                    f"{snapshot_path.name}: fixed panel {panel_id} clip escapes its bounds"
                )
    elif target_id == "layout-grid-masonry":
        require_children_nonoverlap("card-grid")
    elif target_id == "layout-overlay-collision":
        root_id = tree.get("id") if isinstance(tree, dict) else None
        root = _snapshot_rect(rects.get(root_id)) if isinstance(root_id, str) else None
        overlay_types = {"modal", "tooltip", "context_menu", "command_palette"}
        for node in nodes:
            if node.get("type") not in overlay_types:
                continue
            overlay = node_rect(node)
            if (
                root is not None
                and overlay is not None
                and overlay[2] > 0.0
                and overlay[3] > 0.0
                and not _rect_contains(root, overlay, 1.0)
            ):
                violations.append(
                    f"{snapshot_path.name}: overlay {node.get('id')} escapes the root"
                )
    elif target_id == "layout-scrollable-composites":
        for node in require_class("fill-scroll"):
            node_id = node.get("id")
            if not isinstance(node_id, str) or node_id not in clips:
                violations.append(
                    f"{snapshot_path.name}: fill-scroll node {node_id} has no owned clip"
                )
        for node in require_class("both-axis"):
            if content_exceeds_node(node, "x"):
                node_id = node.get("id")
                ranges = layout.get("scroll_max_x")
                if (
                    not isinstance(node_id, str)
                    or not isinstance(ranges, dict)
                    or safe_float(ranges.get(node_id), 0.0) <= 0.0
                ):
                    violations.append(
                        f"{snapshot_path.name}: .both-axis {node_id} lacks owned x-scroll"
                    )
    elif target_id == "layout-plot-embedding":
        require_children_nonoverlap("split")
        require_class("plot-column")
        require_class("table-column")
    elif target_id == "overflow-scrollbar":
        require_owned_scroll("vertical-scroll", "y")
        require_owned_scroll("horizontal-scroll", "x")
        require_owned_scroll("both-scroll", "x", "y")
    elif target_id == "responsive-layout":
        for child in require_class("percent-child"):
            child_rect = node_rect(child)
            parent = next(
                (
                    node
                    for node in nodes
                    if child in (node.get("children") or [])
                ),
                None,
            )
            parent_rect = node_rect(parent) if parent is not None else None
            if (
                child_rect is not None
                and parent_rect is not None
                and not _rect_contains(parent_rect, child_rect, 1.0)
            ):
                violations.append(
                    f"{snapshot_path.name}: percent child escapes its owning panel"
                )
        require_children_nonoverlap("named-grid")
    return violations


def validate_professional_explore_scatter_snapshot(snapshot_path: Path) -> list[str]:
    try:
        snapshot = json.loads(snapshot_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return [f"{snapshot_path.name}: could not read snapshot for scatter framing: {exc}"]
    runtime = snapshot.get("runtime") if isinstance(snapshot, dict) else None
    commands = runtime.get("commands") if isinstance(runtime, dict) else None
    recent = commands.get("recent") if isinstance(commands, dict) else None
    if not isinstance(recent, list):
        return [f"{snapshot_path.name}: missing runtime command history"]

    fit_targets = {
        str(command.get("target"))
        for command in recent
        if isinstance(command, dict)
        and command.get("command") == "FitScatterCamera"
        and command.get("target")
    }
    violations: list[str] = []
    for command in recent:
        if not isinstance(command, dict) or command.get("command") != "SetScatterPointsPacked":
            continue
        target = str(command.get("target") or "")
        detail = str(command.get("detail") or "")
        if not target or "payload_bytes=0" in detail:
            continue
        if "fit=true" in detail or target in fit_targets:
            continue
        violations.append(
            f"{snapshot_path.name}: scatter {target} uploaded points without startup fit"
        )
    return violations


def validate_adjacent_scatter_interaction_log(stdout_path: Path) -> list[str]:
    try:
        text = stdout_path.read_text(encoding="utf-8")
    except OSError as exc:
        return [f"{stdout_path.name}: could not read interaction log: {exc}"]
    if "ADJACENT_SCATTER_INTERACTION_FAIL" in text:
        lines = [
            line.strip()
            for line in text.splitlines()
            if "ADJACENT_SCATTER_INTERACTION_FAIL" in line
        ]
        return lines or [f"{stdout_path.name}: adjacent scatter interaction failed"]
    if "ADJACENT_SCATTER_INTERACTION_PASS" not in text:
        return [f"{stdout_path.name}: missing adjacent scatter interaction pass marker"]
    if "dragongui_import=" not in text or "dragongui_native_import=" not in text:
        return [f"{stdout_path.name}: missing dragongui import path diagnostics"]
    return []


def validate_adjacent_scatter_interaction_screenshot(
    screenshot_path: Path, snapshot_path: Path
) -> list[str]:
    try:
        from PIL import Image
    except ImportError as exc:
        return [f"{screenshot_path.name}: Pillow is required for scatter pixel validation: {exc}"]
    try:
        image = Image.open(screenshot_path).convert("RGB")
    except OSError as exc:
        return [f"{screenshot_path.name}: could not read adjacent scatter screenshot: {exc}"]
    try:
        snapshot = json.loads(snapshot_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return [f"{snapshot_path.name}: could not read snapshot for scatter pixel validation: {exc}"]

    def viewport_rect(scatter_id: str) -> tuple[int, int, int, int] | None:
        scatters = (
            snapshot.get("gpu", {})
            .get("resources", {})
            .get("scatters", {})
        )
        if not isinstance(scatters, dict):
            return None
        scatter = scatters.get(scatter_id)
        if not isinstance(scatter, dict):
            return None
        viewport = scatter.get("viewport")
        if not isinstance(viewport, dict):
            return None
        offset = viewport.get("offset")
        size = viewport.get("size")
        if not isinstance(offset, list) or not isinstance(size, list):
            return None
        if len(offset) < 2 or len(size) < 2:
            return None
        x = int(round(float(offset[0])))
        y = int(round(float(offset[1])))
        w = int(round(float(size[0])))
        h = int(round(float(size[1])))
        if w <= 0 or h <= 0:
            return None
        return x, y, w, h

    def plot_content_pixels(rect: tuple[int, int, int, int]) -> tuple[int, int]:
        x, y, w, h = rect
        inset = 8
        left = max(0, x + inset)
        top = max(0, y + inset)
        right = min(image.width, x + w - inset)
        bottom = min(image.height, y + h - inset)
        if right <= left or bottom <= top:
            return 0, 0
        pixels = image.load()
        content = 0
        area = (right - left) * (bottom - top)
        for py in range(top, bottom):
            for px in range(left, right):
                r, g, b = pixels[px, py]
                mx = max(r, g, b)
                mn = min(r, g, b)
                if (mx >= 120 and mx - mn >= 45) or (mn >= 105 and mx >= 120):
                    content += 1
        return content, area

    violations: list[str] = []
    for scatter_id in ("adjacent-left-scatter", "adjacent-right-scatter"):
        rect = viewport_rect(scatter_id)
        if rect is None:
            violations.append(f"{snapshot_path.name}: missing viewport for {scatter_id}")
            continue
        content, area = plot_content_pixels(rect)
        minimum = max(24, int(area * 0.002))
        if content < minimum:
            violations.append(
                f"{screenshot_path.name}: {scatter_id} has too few plot pixels "
                f"({content} < {minimum})"
            )
    return violations


def validate_professional_demo_splitter_snapshot(snapshot_path: Path) -> list[str]:
    try:
        snapshot = json.loads(snapshot_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return [f"{snapshot_path.name}: could not read snapshot for splitter utilization: {exc}"]
    gpu = snapshot.get("gpu") if isinstance(snapshot, dict) else None
    if not isinstance(gpu, dict):
        return [f"{snapshot_path.name}: missing gpu snapshot"]
    tree = gpu.get("tree")
    layout = gpu.get("layout")
    rects = layout.get("rects") if isinstance(layout, dict) else None
    if not isinstance(tree, dict) or not isinstance(rects, dict):
        return [f"{snapshot_path.name}: missing tree/layout rects"]

    violations: list[str] = []

    def walk(node: dict[str, Any]) -> None:
        node_type = str(node.get("type") or "")
        if node_type == "splitter":
            violation = splitter_underutilization_violation(snapshot_path.name, node, rects)
            if violation:
                violations.append(violation)
        for child in node.get("children") or []:
            if isinstance(child, dict):
                walk(child)

    walk(tree)
    return violations


def splitter_underutilization_violation(
    snapshot_name: str,
    splitter: dict[str, Any],
    rects: dict[str, Any],
) -> str | None:
    props = splitter.get("props")
    orientation = str(props.get("orientation") if isinstance(props, dict) else "horizontal")
    if orientation == "vertical":
        return None
    splitter_id = str(splitter.get("id") or "")
    splitter_rect = rect_as_floats(rects.get(splitter_id))
    if splitter_rect is None or splitter_rect["w"] <= 0 or splitter_rect["h"] <= 0:
        return None
    panes = [
        child
        for child in splitter.get("children") or []
        if isinstance(child, dict) and str(child.get("type") or "") == "pane"
    ]
    if len(panes) < 2:
        return None
    if any(pane_has_explicit_max_size(pane) for pane in panes):
        return None
    pane_rects = [
        rect_as_floats(rects.get(str(pane.get("id") or "")))
        for pane in panes
    ]
    pane_rects = [
        rect for rect in pane_rects if rect is not None and rect["w"] > 0 and rect["h"] > 0
    ]
    if len(pane_rects) < 2:
        return None
    left = min(rect["x"] for rect in pane_rects)
    right = max(rect["x"] + rect["w"] for rect in pane_rects)
    consumed = right - left
    unused = splitter_rect["w"] - consumed
    threshold = max(120.0, splitter_rect["w"] * 0.18)
    if unused > threshold:
        return (
            f"{snapshot_name}: splitter underutilized: splitter width "
            f"{splitter_rect['w']:.0f}px, pane span {consumed:.0f}px, unused {unused:.0f}px"
        )
    return None


def pane_has_explicit_max_size(pane: dict[str, Any]) -> bool:
    props = pane.get("props")
    if isinstance(props, dict) and props.get("max_size") is not None:
        return True
    style = pane.get("style")
    if isinstance(style, dict):
        if style.get("max_width") is not None or style.get("max-height") is not None:
            return True
        layout = style.get("layout")
        if isinstance(layout, dict) and (
            layout.get("max_width") is not None or layout.get("max_height") is not None
        ):
            return True
    return False


def validate_professional_demo_scroll_snapshot(snapshot_path: Path) -> list[str]:
    try:
        snapshot = json.loads(snapshot_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return [f"{snapshot_path.name}: could not read snapshot for scroll reachability: {exc}"]
    gpu = snapshot.get("gpu") if isinstance(snapshot, dict) else None
    if not isinstance(gpu, dict):
        return [f"{snapshot_path.name}: missing gpu snapshot"]
    tree = gpu.get("tree")
    layout = gpu.get("layout")
    rects = layout.get("rects") if isinstance(layout, dict) else None
    scroll_max_y = layout.get("scroll_max_y") if isinstance(layout, dict) else None
    if not isinstance(tree, dict) or not isinstance(rects, dict):
        return [f"{snapshot_path.name}: missing tree/layout rects"]
    if not isinstance(scroll_max_y, dict):
        scroll_max_y = {}

    body = find_primary_body_scroll_area(tree, rects)
    if body is None:
        return []
    body_id, body_node = body
    body_rect = rect_as_floats(rects.get(body_id))
    if body_rect is None:
        return []
    deepest = deepest_visible_descendant_bottom(body_node, rects, root_id=body_id)
    if deepest is None:
        return []

    viewport_bottom = body_rect["y"] + body_rect["h"]
    required_scroll = max(0.0, deepest - viewport_bottom)
    actual_scroll = safe_float(scroll_max_y.get(body_id), 0.0)
    tolerance = 32.0
    if required_scroll > actual_scroll + tolerance:
        return [
            f"{snapshot_path.name}: body scroll range too small: required ~{required_scroll:.0f}px, got {actual_scroll:.0f}px"
        ]
    return []


def find_primary_body_scroll_area(
    tree: dict[str, Any], rects: dict[str, Any]
) -> tuple[str, dict[str, Any]] | None:
    fallback: tuple[str, dict[str, Any]] | None = None

    def walk(node: dict[str, Any]) -> tuple[str, dict[str, Any]] | None:
        nonlocal fallback
        node_id = str(node.get("id") or "")
        node_type = str(node.get("type") or "")
        class_name = str(node.get("class") or "")
        rect = rect_as_floats(rects.get(node_id))
        if node_type == "scroll_area" and rect is not None and rect["w"] > 0 and rect["h"] > 0:
            classes = class_name.split()
            if "page-scroll" in classes:
                return node_id, node
            if fallback is None or "body" in classes:
                fallback = (node_id, node)
        for child in node.get("children") or []:
            if isinstance(child, dict):
                found = walk(child)
                if found is not None:
                    return found
        return None

    return walk(tree) or fallback


def deepest_visible_descendant_bottom(
    node: dict[str, Any],
    rects: dict[str, Any],
    *,
    root_id: str,
) -> float | None:
    deepest: float | None = None

    def walk(current: dict[str, Any]) -> None:
        nonlocal deepest
        node_id = str(current.get("id") or "")
        if node_id != root_id and snapshot_node_excluded_from_scroll_bounds(current):
            return
        rect = rect_as_floats(rects.get(node_id))
        if node_id != root_id and rect is not None and rect["w"] > 0 and rect["h"] > 0:
            deepest = max(deepest or float("-inf"), rect["y"] + rect["h"])
        for child in current.get("children") or []:
            if isinstance(child, dict):
                walk(child)

    walk(node)
    return deepest


def snapshot_node_excluded_from_scroll_bounds(node: dict[str, Any]) -> bool:
    node_type = str(node.get("type") or "")
    if node_type in {
        "modal",
        "tooltip",
        "context_menu",
        "command_palette",
        "toast",
        "menu",
        "menu_item",
    }:
        return True
    style = node.get("style")
    if not isinstance(style, dict):
        return False
    position = style.get("position")
    if isinstance(position, str) and position.lower() == "fixed":
        return True
    layout = style.get("layout")
    if isinstance(layout, dict):
        position = layout.get("position")
        if isinstance(position, str) and position.lower() == "fixed":
            return True
    return False


def rect_as_floats(rect: Any) -> dict[str, float] | None:
    if not isinstance(rect, dict):
        return None
    try:
        return {
            "x": float(rect["x"]),
            "y": float(rect["y"]),
            "w": float(rect["w"]),
            "h": float(rect["h"]),
        }
    except (KeyError, TypeError, ValueError):
        return None


def safe_float(value: Any, default: float) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def validate_badge_layout_snapshot(snapshot_path: Path) -> list[str]:
    try:
        snapshot = json.loads(snapshot_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return [f"{snapshot_path.name}: could not read snapshot for badge bounds check: {exc}"]
    gpu = snapshot.get("gpu") if isinstance(snapshot, dict) else None
    if not isinstance(gpu, dict):
        return [f"{snapshot_path.name}: missing gpu snapshot"]
    tree = gpu.get("tree")
    layout = gpu.get("layout")
    rects = layout.get("rects") if isinstance(layout, dict) else None
    styles = gpu.get("computed_styles")
    theme = gpu.get("theme")
    if not isinstance(tree, dict) or not isinstance(rects, dict):
        return [f"{snapshot_path.name}: missing tree/layout rects"]
    if not isinstance(styles, dict):
        styles = {}
    if not isinstance(theme, dict):
        theme = {}

    spacing = float(theme.get("spacing") or 8.0)
    font_size = float(theme.get("font_size") or 14.0)
    violations: list[str] = []

    def walk(node: dict[str, Any]) -> None:
        node_type = str(node.get("type") or "")
        props = node.get("props")
        badge = props.get("badge") if isinstance(props, dict) else None
        if badge and node_type in {"button", "small_button", "tab", "nav_item"}:
            rect = rects.get(str(node.get("id")))
            if isinstance(rect, dict):
                violation = inline_badge_bounds_violation(
                    str(node.get("id")),
                    str(badge),
                    node_type,
                    rect,
                    styles.get(str(node.get("id"))),
                    spacing,
                    font_size,
                )
                if violation:
                    violations.append(violation)
        for child in node.get("children") or []:
            if isinstance(child, dict):
                walk(child)

    walk(tree)
    return violations


def inline_badge_bounds_violation(
    node_id: str,
    badge: str,
    node_type: str,
    rect: dict[str, Any],
    computed_style: Any,
    spacing: float,
    theme_font_size: float,
) -> str | None:
    try:
        x = float(rect["x"])
        y = float(rect["y"])
        w = float(rect["w"])
        h = float(rect["h"])
    except (KeyError, TypeError, ValueError):
        return f"{node_id}: malformed rect"
    style = computed_style.get("style") if isinstance(computed_style, dict) else {}
    if not isinstance(style, dict):
        style = {}
    text_style = style.get("text") if isinstance(style.get("text"), dict) else {}
    parts = style.get("parts") if isinstance(style.get("parts"), dict) else {}
    badge_part = parts.get("badge") if isinstance(parts.get("badge"), dict) else {}
    badge_text = badge_part.get("text") if isinstance(badge_part.get("text"), dict) else {}
    badge_visual = badge_part.get("visual") if isinstance(badge_part.get("visual"), dict) else {}

    badge_font = float(
        badge_text.get("font_size")
        or text_style.get("font_size")
        or max(theme_font_size - 2.0, 10.0)
    )
    border = max(0.0, float(badge_visual.get("border_width") or 0.0))
    preferred_w = max(len(badge) * badge_font * 0.68 + 8.0 * 2.0 + border * 2.0, 16.0)
    preferred_h = max(badge_font + 6.0 + border * 2.0, 16.0)
    right_inset = 8.0 if node_type == "tab" else spacing
    badge_w = min(preferred_w, max(w - right_inset, 0.0))
    badge_h = min(preferred_h, max(h - 4.0, 1.0))
    badge_x = x + w - right_inset - badge_w
    badge_y = y + (h - badge_h) * 0.5

    if badge_w <= 0.0 or badge_h <= 0.0:
        return f"{node_id}: badge has no visible area"
    if badge_x < x - 0.5 or badge_y < y - 0.5:
        return f"{node_id}: badge starts outside parent"
    if badge_x + badge_w > x + w + 0.5 or badge_y + badge_h > y + h + 0.5:
        return f"{node_id}: badge exceeds parent"
    return None


def target_sizes(
    target: dict[str, Any],
    selectors: list[tuple[int, int] | None],
) -> list[tuple[int, int] | None]:
    manifest_sizes = [tuple(size) for size in target.get("sizes", [])]
    sizes: list[tuple[int, int] | None] = []
    for selector in selectors:
        if selector is None:
            sizes.append(None)
        elif selector in manifest_sizes:
            sizes.append(selector)
        else:
            sizes.append(selector)
    return sizes


def target_scales(
    target: dict[str, Any],
    selectors: list[float | None],
) -> list[float]:
    manifest_scales = [float(scale) for scale in target.get("scales", [1.0])]
    scales: list[float] = []
    for selector in selectors:
        candidates = manifest_scales if selector is None else [selector]
        for scale in candidates:
            if scale not in scales:
                scales.append(scale)
    return scales or [1.0]


def screenshot_capture_available() -> tuple[bool, str]:
    if platform.system() != "Windows":
        return False, "Whole-window external capture currently supports Windows only."
    try:
        from PIL import ImageGrab  # noqa: F401
    except Exception as exc:
        return False, f"Pillow ImageGrab is unavailable: {exc!r}"
    return True, "ok"


def screenshot_error_path(screenshot_path: Path) -> Path:
    return screenshot_path.with_suffix(".error.json")


def read_screenshot_error(screenshot_path: Path) -> str | None:
    error_path = screenshot_error_path(screenshot_path)
    if not error_path.exists():
        return None
    try:
        payload = json.loads(error_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return f"could not read screenshot error {error_path.name}: {exc}"
    if not isinstance(payload, dict):
        return f"screenshot error file {error_path.name} is not a JSON object"
    message = str(payload.get("error") or payload.get("message") or "unknown error")
    kind = str(payload.get("kind") or "native_screenshot_error")
    return f"{kind}: {message}"


def with_screenshot_error_note(note: str, screenshot_path: Path) -> str:
    error = read_screenshot_error(screenshot_path)
    if not error:
        return note
    return f"{note} Native screenshot error: {error}"


def collect_process_output(
    proc: subprocess.Popen[str],
    *,
    timeout_s: float,
) -> tuple[str, str, bool]:
    try:
        stdout, stderr = proc.communicate(timeout=max(1.0, timeout_s))
        return stdout, stderr, False
    except subprocess.TimeoutExpired:
        proc.terminate()
        try:
            stdout, stderr = proc.communicate(timeout=2)
        except subprocess.TimeoutExpired:
            proc.kill()
            stdout, stderr = proc.communicate(timeout=2)
        return stdout, stderr, True


def write_process_logs(
    stdout_path: Path,
    stderr_path: Path,
    stdout: str,
    stderr: str,
) -> None:
    stdout_path.write_text(stdout, encoding="utf-8")
    stderr_path.write_text(stderr, encoding="utf-8")


def run_probe_process(
    script: Path,
    *,
    target_id: str,
    snapshot_path: Path,
    wait_ms: int,
    timeout_ms: int,
    size: tuple[int, int] | None,
    scale_factor: float,
    resize_checkpoints: list[tuple[int, int]],
    screenshot_path: Path,
    stdout_path: Path,
    stderr_path: Path,
    route: str | None = None,
    actions: list[str] | None = None,
    script_args: list[str] | None = None,
) -> dict[str, Any]:
    wrapper = make_wrapper(script)
    screenshot_error = screenshot_error_path(screenshot_path)
    screenshot_error.unlink(missing_ok=True)
    for checkpoint_path in snapshot_path.parent.glob(f"{snapshot_path.stem}-resize-*.json"):
        checkpoint_path.unlink(missing_ok=True)
    env = os.environ.copy()
    env["PYTHONPATH"] = str(ROOT / "python") + os.pathsep + env.get("PYTHONPATH", "")
    env["DRAGONGUI_VISUAL_AUDIT"] = "1"
    env["DRAGONGUI_AUDIT_TARGET"] = target_id
    env["DRAGONGUI_AUDIT_WAIT_MS"] = str(wait_ms)
    env["DRAGONGUI_AUDIT_EXIT_MS"] = str(wait_ms + 1200)
    env["DRAGONGUI_AUDIT_SNAPSHOT"] = str(snapshot_path)
    env["DRAGONGUI_AUDIT_SCREENSHOT"] = str(screenshot_path)
    env["DRAGONGUI_AUDIT_SCREENSHOT_ERROR"] = str(screenshot_error)
    if size is not None:
        env["DRAGONGUI_AUDIT_WIDTH"] = str(size[0])
        env["DRAGONGUI_AUDIT_HEIGHT"] = str(size[1])
    env["DRAGONGUI_AUDIT_SCALE_FACTOR"] = f"{scale_factor:g}"
    env["DRAGONGUI_AUDIT_RESIZE_CHECKPOINTS"] = json.dumps(resize_checkpoints)
    env["DRAGONGUI_AUDIT_ROUTE"] = route or ""
    env["DRAGONGUI_AUDIT_ACTIONS"] = json.dumps(actions or [])
    env["DRAGONGUI_AUDIT_SCRIPT_ARGS"] = json.dumps(script_args or [])
    env.pop("DRAGONGUI_DEV_FALLBACK", None)

    proc = subprocess.Popen(
        [sys.executable, str(wrapper)],
        cwd=str(ROOT),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    screenshot_ok = False
    notes = "No visual issue recorded by automated first pass."
    try:
        native_deadline = time.monotonic() + (wait_ms + 4000) / 1000
        while time.monotonic() < native_deadline and proc.poll() is None:
            if screenshot_path.exists() and screenshot_path.stat().st_size > 0:
                screenshot_ok = True
                stdout, stderr, _terminated = collect_process_output(
                    proc, timeout_s=timeout_ms / 1000
                )
                write_process_logs(stdout_path, stderr_path, stdout, stderr)
                if proc.returncode not in (0, None):
                    return {
                        "status": "blocked",
                        "notes": f"Probe exited with code {proc.returncode}; native screenshot was still written.",
                        "screenshot": screenshot_ok,
                        "snapshot": snapshot_path.exists(),
                    }
                return {
                    "status": "pass",
                    "notes": "Captured with native DragonGUI window screenshot API.",
                    "screenshot": screenshot_ok,
                    "snapshot": snapshot_path.exists(),
                }
            time.sleep(0.05)

        if proc.poll() is not None:
            stdout, stderr = proc.communicate(timeout=1)
            write_process_logs(stdout_path, stderr_path, stdout, stderr)
            return {
                "status": "blocked",
                "notes": with_screenshot_error_note(
                    window_blocker(proc.returncode, stdout, stderr), screenshot_path
                ),
                "screenshot": False,
                "snapshot": snapshot_path.exists(),
            }

        capture_available, capture_reason = screenshot_capture_available()
        if not capture_available:
            stdout, stderr, _terminated = collect_process_output(
                proc, timeout_s=timeout_ms / 1000
            )
            write_process_logs(stdout_path, stderr_path, stdout, stderr)
            return {
                "status": "blocked",
                "notes": with_screenshot_error_note(
                    f"Native screenshot was not written and external capture is unavailable: {capture_reason}",
                    screenshot_path,
                ),
                "screenshot": False,
                "snapshot": snapshot_path.exists(),
            }

        hwnd = wait_for_window(proc.pid, timeout_ms=max(wait_ms, 1000))
        if hwnd is None:
            if proc.poll() is None:
                screenshot_ok = capture_screen(screenshot_path)
                stdout, stderr, _terminated = collect_process_output(
                    proc, timeout_s=timeout_ms / 1000
                )
                write_process_logs(stdout_path, stderr_path, stdout, stderr)
                if proc.returncode not in (0, None):
                    return {
                        "status": "blocked",
                        "notes": with_screenshot_error_note(
                            f"Probe exited with code {proc.returncode}; full-screen fallback captured because window bounds were not detected.",
                            screenshot_path,
                        ),
                        "screenshot": screenshot_ok,
                        "snapshot": snapshot_path.exists(),
                    }
                return {
                    "status": "blocked",
                    "notes": with_screenshot_error_note(
                        "Whole-window bounds were not detected; captured the desktop as diagnostic evidence only.",
                        screenshot_path,
                    ),
                    "screenshot": screenshot_ok,
                    "snapshot": snapshot_path.exists(),
                }
            stdout, stderr = proc.communicate(timeout=timeout_ms / 1000)
            write_process_logs(stdout_path, stderr_path, stdout, stderr)
            return {
                "status": "blocked",
                "notes": with_screenshot_error_note(
                    window_blocker(proc.returncode, stdout, stderr), screenshot_path
                ),
                "screenshot": False,
                "snapshot": snapshot_path.exists(),
            }
        if size is not None:
            move_window(hwnd.hwnd, size[0], size[1])
        time.sleep(wait_ms / 1000)
        hwnd = window_for_pid(proc.pid) or hwnd
        if not likely_probe_window(hwnd):
            screenshot_ok = capture_screen(screenshot_path)
            stdout, stderr, _terminated = collect_process_output(
                proc, timeout_s=(timeout_ms - wait_ms) / 1000
            )
            write_process_logs(stdout_path, stderr_path, stdout, stderr)
            return {
                "status": "blocked",
                "notes": with_screenshot_error_note(
                    f"Detected window title/rect was not a reliable DragonGUI probe crop: {hwnd.title!r} {hwnd.rect}; captured desktop as diagnostic evidence only.",
                    screenshot_path,
                ),
                "screenshot": screenshot_ok,
                "snapshot": snapshot_path.exists(),
            }
        screenshot_ok = capture_window(hwnd.rect, screenshot_path)
        stdout, stderr, terminated = collect_process_output(
            proc, timeout_s=(timeout_ms - wait_ms) / 1000
        )
        if terminated:
            notes = "Probe did not exit after audit capture and was terminated."
        write_process_logs(stdout_path, stderr_path, stdout, stderr)
        if proc.returncode not in (0, None):
            return {
                "status": "blocked",
                "notes": f"Probe exited with code {proc.returncode}; see stderr log.",
                "screenshot": screenshot_ok,
                "snapshot": snapshot_path.exists(),
            }
        return {
            "status": "pass",
            "notes": notes,
            "screenshot": screenshot_ok,
            "snapshot": snapshot_path.exists(),
        }
    finally:
        try:
            wrapper.unlink(missing_ok=True)
        except OSError:
            pass
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                proc.kill()


def make_wrapper(script: Path) -> Path:
    content = f"""
from __future__ import annotations

import json
import os
from pathlib import Path
import runpy
import sys
import threading
import time
import traceback
import struct
import zlib
import ctypes
from ctypes import wintypes

sys.path.insert(0, {str(ROOT / "python")!r})
sys.path.insert(0, {str(script.parent)!r})

import dragongui as dg

_original_run = dg.App.run
_original_window_init = dg.Window.__init__


def _png_chunk(kind, data):
    return (
        struct.pack(">I", len(data))
        + kind
        + data
        + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
    )


def _write_rgba_png(path, width, height, rgba):
    rows = []
    stride = width * 4
    for y in range(height):
        start = y * stride
        rows.append(b"\\x00" + rgba[start:start + stride])
    payload = b"".join(rows)
    png = (
        b"\\x89PNG\\r\\n\\x1a\\n"
        + _png_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + _png_chunk(b"IDAT", zlib.compress(payload))
        + _png_chunk(b"IEND", b"")
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(png)


def _write_screenshot_error(path, kind, error, **extra):
    payload = {{"kind": kind, "error": str(error), **extra}}
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
    print(f"DragonGUI visual audit screenshot error: {{kind}}: {{error}}", file=sys.stderr)


def _audited_window_init(self, *args, **kwargs):
    width = os.environ.get("DRAGONGUI_AUDIT_WIDTH")
    height = os.environ.get("DRAGONGUI_AUDIT_HEIGHT")
    if width and height:
        kwargs["width"] = int(width)
        kwargs["height"] = int(height)
    return _original_window_init(self, *args, **kwargs)


def _audited_run(self, window):
    wait_ms = int(os.environ.get("DRAGONGUI_AUDIT_WAIT_MS", "1800"))
    exit_ms = int(os.environ.get("DRAGONGUI_AUDIT_EXIT_MS", str(wait_ms + 1200)))
    snapshot_path = Path(os.environ["DRAGONGUI_AUDIT_SNAPSHOT"])
    screenshot_path = Path(os.environ["DRAGONGUI_AUDIT_SCREENSHOT"])
    screenshot_error_path = Path(os.environ["DRAGONGUI_AUDIT_SCREENSHOT_ERROR"])
    resize_checkpoints = json.loads(
        os.environ.get("DRAGONGUI_AUDIT_RESIZE_CHECKPOINTS", "[]")
    )
    audit_route = os.environ.get("DRAGONGUI_AUDIT_ROUTE") or None
    audit_actions = json.loads(os.environ.get("DRAGONGUI_AUDIT_ACTIONS", "[]"))
    audit_scale = max(
        float(os.environ.get("DRAGONGUI_AUDIT_SCALE_FACTOR", "1")),
        1.0,
    )
    snapshot_timeout_ms = max(3000, round(5000 * audit_scale))

    def walk_widgets(widget):
        yield widget
        for child in getattr(widget, "children", ()) or ():
            yield from walk_widgets(child)

    def find_widget(selector):
        widget_id = selector[1:] if selector.startswith("#") else selector
        return next(
            (candidate for candidate in walk_widgets(window) if getattr(candidate, "id", None) == widget_id),
            None,
        )

    def current_window_handle():
        if os.name != "nt":
            return None
        found = []
        callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
        def callback(hwnd, _lparam):
            process_id = wintypes.DWORD()
            ctypes.windll.user32.GetWindowThreadProcessId(hwnd, ctypes.byref(process_id))
            if process_id.value == os.getpid() and ctypes.windll.user32.IsWindowVisible(hwnd):
                found.append(hwnd)
                return False
            return True
        ctypes.windll.user32.EnumWindows(callback_type(callback), 0)
        return found[0] if found else None

    def pointer_action(selector, click_count=0, button="left", post_message=False):
        if os.name != "nt":
            return False
        snapshot = debug_snapshot_with_retry()
        rect = snapshot.get("gpu", {{}}).get("layout", {{}}).get("rects", {{}}).get(selector.lstrip("#"))
        hwnd = current_window_handle()
        if not isinstance(rect, dict) or not hwnd:
            return False
        client_x = round(float(rect.get("x", 0)) + float(rect.get("w", 0)) * 0.5)
        client_y = round(float(rect.get("y", 0)) + float(rect.get("h", 0)) * 0.5)
        point = wintypes.POINT(client_x, client_y)
        ctypes.windll.user32.ClientToScreen(hwnd, ctypes.byref(point))
        ctypes.windll.user32.SetForegroundWindow(hwnd)
        ctypes.windll.user32.SetCursorPos(point.x, point.y)
        if click_count:
            if post_message:
                down_message, up_message, down_wparam = (
                    (0x0204, 0x0205, 0x0002)
                    if button == "right"
                    else (0x0201, 0x0202, 0x0001)
                )
                packed_point = (
                    (int(client_y) & 0xFFFF) << 16
                ) | (int(client_x) & 0xFFFF)
                ctypes.windll.user32.PostMessageW(hwnd, 0x0200, 0, packed_point)
                for index in range(click_count):
                    ctypes.windll.user32.PostMessageW(
                        hwnd, down_message, down_wparam, packed_point
                    )
                    ctypes.windll.user32.PostMessageW(
                        hwnd, up_message, 0, packed_point
                    )
                    if index + 1 < click_count:
                        time.sleep(0.12)
                return True
            down_flag, up_flag = (
                (0x0008, 0x0010) if button == "right" else (0x0002, 0x0004)
            )
            interval_ms = min(
                max(int(ctypes.windll.user32.GetDoubleClickTime()) // 3, 100),
                200,
            )
            for index in range(click_count):
                ctypes.windll.user32.mouse_event(down_flag, 0, 0, 0, 0)
                ctypes.windll.user32.mouse_event(up_flag, 0, 0, 0, 0)
                if index + 1 < click_count:
                    time.sleep(interval_ms / 1000.0)
        return True

    def key_action(name):
        if os.name != "nt":
            return False
        hwnd = current_window_handle()
        if not hwnd:
            return False
        ctypes.windll.user32.SetForegroundWindow(hwnd)
        key_codes = {{
            "tab": 0x09,
            "enter": 0x0D,
            "space": 0x20,
            "escape": 0x1B,
        }}
        if name == "alt-space":
            ctypes.windll.user32.PostMessageW(hwnd, 0x0104, 0x12, 0x20000001)
            ctypes.windll.user32.PostMessageW(hwnd, 0x0104, 0x20, 0x20390001)
            ctypes.windll.user32.PostMessageW(hwnd, 0x0105, 0x20, 0xE0390001)
            ctypes.windll.user32.PostMessageW(hwnd, 0x0105, 0x12, 0xC0000001)
            return True
        virtual_key = key_codes.get(name)
        if virtual_key is None:
            return False
        if name == "escape":
            # A native system-menu loop owns keyboard dispatch while it is open.
            # WM_CANCELMODE reaches that loop reliably from the audit worker,
            # whereas a posted WM_KEYDOWN remains queued on the client window.
            ctypes.windll.user32.PostMessageW(hwnd, 0x001F, 0, 0)
        ctypes.windll.user32.PostMessageW(hwnd, 0x0100, virtual_key, 0x00000001)
        ctypes.windll.user32.PostMessageW(hwnd, 0x0101, virtual_key, 0xC0000001)
        return True

    def window_system_menu_open(hwnd):
        class GuiThreadInfo(ctypes.Structure):
            _fields_ = [
                ("cbSize", wintypes.DWORD),
                ("flags", wintypes.DWORD),
                ("hwndActive", wintypes.HWND),
                ("hwndFocus", wintypes.HWND),
                ("hwndCapture", wintypes.HWND),
                ("hwndMenuOwner", wintypes.HWND),
                ("hwndMoveSize", wintypes.HWND),
                ("hwndCaret", wintypes.HWND),
                ("rcCaret", wintypes.RECT),
            ]
        process_id = wintypes.DWORD()
        thread_id = ctypes.windll.user32.GetWindowThreadProcessId(
            hwnd, ctypes.byref(process_id)
        )
        info = GuiThreadInfo()
        info.cbSize = ctypes.sizeof(GuiThreadInfo)
        if not ctypes.windll.user32.GetGUIThreadInfo(thread_id, ctypes.byref(info)):
            raise RuntimeError("GetGUIThreadInfo failed while checking the system menu")
        return bool(info.flags & (0x0004 | 0x0008)) and bool(info.hwndMenuOwner)

    def debug_snapshot_with_retry():
        last_error = None
        for _attempt in range(2):
            try:
                return self.debug_snapshot(timeout_ms=snapshot_timeout_ms)
            except RuntimeError as exc:
                last_error = exc
                time.sleep(0.15)
        raise last_error

    def initialize_route(route):
        if not route:
            return
        for widget in walk_widgets(window):
            if getattr(widget, "kind", None) != "pages":
                continue
            values = {{getattr(child, "value", None) for child in getattr(widget, "children", ())}}
            if route in values:
                widget.set_value(route)
                return
        raise RuntimeError(f"visual audit route {{route!r}} does not match any Pages child")

    def perform_action(action):
        if action.startswith("wait:"):
            time.sleep(int(action.split(":", 1)[1]) / 1000.0)
            return
        if action.startswith("resize:"):
            width, height = action.split(":", 1)[1].lower().split("x", 1)
            self._handle.request_window_resize(int(width), int(height))
            wait_for_logical_size(int(width), int(height))
            return
        command, payload = action.split(":", 1)
        if command == "hover":
            if not pointer_action(payload):
                raise RuntimeError(f"could not hover visual audit target {{payload}}")
            time.sleep(0.65)
            return
        if command == "click":
            widget = find_widget(payload)
            callback = (
                getattr(widget, "click", None)
                or getattr(widget, "on_click", None)
                if widget is not None
                else None
            )
            if callable(callback):
                callback()
            elif not pointer_action(payload, click_count=1):
                raise RuntimeError(f"visual audit {{command}} target {{payload}} is not clickable")
            time.sleep(0.15)
            return
        if command == "native-click":
            if not pointer_action(payload, click_count=1, post_message=True):
                raise RuntimeError(f"could not native-click visual audit target {{payload}}")
            time.sleep(0.2)
            return
        if command == "right-click":
            if not pointer_action(
                payload, click_count=1, button="right", post_message=True
            ):
                raise RuntimeError(f"could not right-click visual audit target {{payload}}")
            time.sleep(0.15)
            return
        if command == "key":
            if not key_action(payload):
                raise RuntimeError(f"could not send visual audit key {{payload!r}}")
            time.sleep(0.15)
            return
        if command == "assert-focus":
            snapshot = debug_snapshot_with_retry()
            focused = snapshot.get("gpu", {{}}).get("state", {{}}).get("focused")
            expected = payload.lstrip("#")
            if focused != expected:
                raise RuntimeError(
                    f"expected focus {{expected!r}}, observed {{focused!r}}"
                )
            return
        if command == "assert-system-menu":
            if os.name != "nt":
                raise RuntimeError("assert-system-menu currently requires Windows")
            hwnd = current_window_handle()
            if not hwnd:
                raise RuntimeError("assert-system-menu could not find the DragonGUI window")
            actual_open = window_system_menu_open(hwnd)
            expected_open = payload == "open"
            if actual_open != expected_open:
                actual = "open" if actual_open else "closed"
                raise RuntimeError(
                    f"expected system menu {{payload!r}}, observed {{actual!r}}"
                )
            return
        if command == "assert-window-state":
            if os.name != "nt":
                raise RuntimeError("assert-window-state currently requires Windows")
            hwnd = current_window_handle()
            if not hwnd:
                raise RuntimeError("assert-window-state could not find the DragonGUI window")
            deadline = time.monotonic() + 3.0
            actual = None
            while time.monotonic() < deadline:
                minimized = bool(ctypes.windll.user32.IsIconic(hwnd))
                maximized = bool(ctypes.windll.user32.IsZoomed(hwnd))
                actual = "minimized" if minimized else ("maximized" if maximized else "normal")
                if actual == payload:
                    return
                time.sleep(0.025)
            raise RuntimeError(
                f"expected window state {{payload!r}}, observed {{actual!r}}"
            )
        if command == "set-window-state":
            if os.name != "nt" or payload != "normal":
                raise RuntimeError("set-window-state:normal currently requires Windows")
            hwnd = current_window_handle()
            if not hwnd:
                raise RuntimeError("set-window-state could not find the DragonGUI window")
            ctypes.windll.user32.ShowWindow(hwnd, 9)
            deadline = time.monotonic() + 3.0
            while time.monotonic() < deadline:
                if not ctypes.windll.user32.IsIconic(hwnd) and not ctypes.windll.user32.IsZoomed(hwnd):
                    return
                time.sleep(0.025)
            raise RuntimeError("window did not return to normal state")
        selector, value = payload.split("=", 1)
        widget = find_widget(selector)
        if widget is None:
            raise RuntimeError(f"visual audit action target {{selector}} was not found")
        if command == "type":
            setter = getattr(widget, "set_value", None)
            if not callable(setter):
                raise RuntimeError(f"visual audit type target {{selector}} has no set_value")
            setter(value)
        elif command == "scroll":
            x, y = value.split(",", 1)
            scroller = getattr(widget, "scroll_to", None)
            if not callable(scroller):
                raise RuntimeError(f"visual audit scroll target {{selector}} has no scroll_to")
            scroller(x=float(x), y=float(y))
        time.sleep(0.12)

    def wait_for_logical_size(width, height):
        deadline = time.monotonic() + 3.0
        latest = None
        while time.monotonic() < deadline:
            latest = self.debug_snapshot(timeout_ms=3000)
            window_state = latest.get("gpu", {{}}).get("window", {{}})
            scale = max(float(window_state.get("scale_factor", 1.0)), 0.001)
            logical_width = float(window_state.get("width", 0)) / scale
            logical_height = float(window_state.get("height", 0)) / scale
            if abs(logical_width - width) <= 1.0 and abs(logical_height - height) <= 1.0:
                return latest
            time.sleep(0.025)
        raise RuntimeError(
            f"resize checkpoint {{width}}x{{height}} was not reached; "
            f"last window={{latest.get('gpu', {{}}).get('window') if latest else None}}"
        )

    def worker():
        deadline = time.monotonic() + 3.0
        while time.monotonic() < deadline:
            if getattr(self, "_handle", None) is not None:
                break
            time.sleep(0.025)
        try:
            initialize_route(audit_route)
            for action in audit_actions:
                perform_action(action)
            time.sleep(max(0.0, wait_ms / 1000.0))
            initial_snapshot = self.debug_snapshot(timeout_ms=snapshot_timeout_ms)
            initial_window = initial_snapshot.get("gpu", {{}}).get("window", {{}})
            initial_scale = max(float(initial_window.get("scale_factor", 1.0)), 0.001)
            initial_width = round(float(initial_window.get("width", 0)) / initial_scale)
            initial_height = round(float(initial_window.get("height", 0)) / initial_scale)
            if resize_checkpoints and initial_width > 0 and initial_height > 0:
                initial_path = snapshot_path.with_name(
                    f"{{snapshot_path.stem}}-resize-0-start-"
                    f"{{initial_width}}x{{initial_height}}.json"
                )
                initial_path.write_text(
                    json.dumps(initial_snapshot, indent=2, sort_keys=True),
                    encoding="utf-8",
                )
            sequence = [
                (int(checkpoint[0]), int(checkpoint[1]))
                for checkpoint in resize_checkpoints
            ]
            if initial_width > 0 and initial_height > 0 and sequence:
                sequence.append((initial_width, initial_height))
            snapshot = initial_snapshot
            for index, (width, height) in enumerate(sequence, start=1):
                self._handle.request_window_resize(width, height)
                snapshot = wait_for_logical_size(width, height)
                checkpoint_path = snapshot_path.with_name(
                    f"{{snapshot_path.stem}}-resize-{{index}}-{{width}}x{{height}}.json"
                )
                checkpoint_path.write_text(
                    json.dumps(snapshot, indent=2, sort_keys=True),
                    encoding="utf-8",
                )
            snapshot_path.parent.mkdir(parents=True, exist_ok=True)
            snapshot_path.write_text(json.dumps(snapshot, indent=2, sort_keys=True), encoding="utf-8")
        except Exception as exc:
            snapshot_path.parent.mkdir(parents=True, exist_ok=True)
            payload = {{"error": repr(exc), "traceback": traceback.format_exc()}}
            snapshot_path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")
        try:
            raw = self._window_screenshot(timeout_ms=5000)
            if raw is None:
                _write_screenshot_error(
                    screenshot_error_path,
                    "native_unavailable",
                    "app._window_screenshot() returned None",
                )
            else:
                width, height, rgba = raw
                expected_len = int(width) * int(height) * 4
                if int(width) <= 0 or int(height) <= 0:
                    _write_screenshot_error(
                        screenshot_error_path,
                        "invalid_dimensions",
                        f"window screenshot returned {{width}}x{{height}}",
                    )
                elif not rgba:
                    _write_screenshot_error(
                        screenshot_error_path,
                        "empty_rgba",
                        "window screenshot returned empty RGBA bytes",
                        width=int(width),
                        height=int(height),
                    )
                elif len(rgba) != expected_len:
                    _write_screenshot_error(
                        screenshot_error_path,
                        "wrong_rgba_length",
                        f"expected {{expected_len}} bytes, got {{len(rgba)}}",
                        width=int(width),
                        height=int(height),
                    )
                else:
                    _write_rgba_png(screenshot_path, int(width), int(height), bytes(rgba))
        except Exception as exc:
            _write_screenshot_error(
                screenshot_error_path,
                "exception",
                repr(exc),
                traceback=traceback.format_exc(),
            )
        extra = max(0.0, (exit_ms - wait_ms) / 1000.0)
        if extra:
            time.sleep(extra)
        try:
            self.request_exit()
        except Exception:
            pass

    threading.Thread(target=worker, daemon=True).start()
    return _original_run(self, window)


dg.App.run = _audited_run
dg.Window.__init__ = _audited_window_init
sys.argv = [
    {str(script)!r},
    *json.loads(os.environ.get("DRAGONGUI_AUDIT_SCRIPT_ARGS", "[]")),
]
runpy.run_path({str(script)!r}, run_name="__main__")
"""
    handle = tempfile.NamedTemporaryFile(
        "w",
        suffix="_dragongui_visual_audit.py",
        prefix="",
        delete=False,
        encoding="utf-8",
    )
    with handle:
        handle.write(textwrap.dedent(content))
    return Path(handle.name)


def wait_for_window(pid: int, *, timeout_ms: int) -> WindowInfo | None:
    deadline = time.monotonic() + timeout_ms / 1000
    while time.monotonic() < deadline:
        info = window_for_pid(pid)
        if info is not None:
            return info
        time.sleep(0.05)
    return None


def likely_probe_window(window: WindowInfo) -> bool:
    title = window.title.lower()
    if any(token in title for token in ("probe", "benchmark", "dragongui", "css")):
        return True
    left, top, right, bottom = window.rect
    width = right - left
    height = bottom - top
    # A borderless full-desktop capture is not a useful per-probe screenshot.
    return width < 2500 and height < 1600


def window_for_pid(pid: int) -> WindowInfo | None:
    user32 = ctypes.windll.user32
    windows: list[WindowInfo] = []

    enum_proc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    def callback(hwnd: int, _lparam: int) -> bool:
        if not user32.IsWindowVisible(hwnd):
            return True
        proc_id = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(proc_id))
        if proc_id.value != pid:
            return True
        title_len = user32.GetWindowTextLengthW(hwnd)
        if title_len <= 0:
            return True
        buffer = ctypes.create_unicode_buffer(title_len + 1)
        user32.GetWindowTextW(hwnd, buffer, title_len + 1)
        rect = wintypes.RECT()
        user32.GetWindowRect(hwnd, ctypes.byref(rect))
        if rect.right <= rect.left or rect.bottom <= rect.top:
            return True
        windows.append(
            WindowInfo(
                hwnd=int(hwnd),
                title=buffer.value,
                rect=(int(rect.left), int(rect.top), int(rect.right), int(rect.bottom)),
            )
        )
        return True

    user32.EnumWindows(enum_proc(callback), 0)
    if not windows:
        return None
    return max(windows, key=lambda item: (item.rect[2] - item.rect[0]) * (item.rect[3] - item.rect[1]))


def move_window(hwnd: int, width: int, height: int) -> None:
    user32 = ctypes.windll.user32
    rect = wintypes.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(rect))
    user32.MoveWindow(hwnd, rect.left, rect.top, int(width), int(height), True)


def capture_window(rect: tuple[int, int, int, int], path: Path) -> bool:
    from PIL import ImageGrab

    try:
        image = ImageGrab.grab(bbox=rect, all_screens=True)
    except OSError:
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path)
    return path.exists()


def capture_screen(path: Path) -> bool:
    from PIL import ImageGrab

    image = ImageGrab.grab(all_screens=True)
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path)
    return path.exists()


def window_blocker(returncode: int | None, stdout: str, stderr: str) -> str:
    combined = "\n".join(part for part in (stdout, stderr) if part).strip()
    if "native extension is not built" in combined:
        return "Native extension is not built; screenshot inspection requires a real window."
    if "dev-fallback" in combined or "event_loop': 'not_started'" in combined:
        return "Probe used DragonGUI dev fallback; screenshot inspection requires native event loop."
    if "requires " in combined:
        return combined.splitlines()[0]
    if returncode not in (0, None):
        return f"Probe exited before a window was detected with code {returncode}; see logs."
    return "No visible DragonGUI window was detected before capture timeout."


def size_label(size: tuple[int, int] | None, index: int) -> str:
    if size is None:
        return f"desktop-{index}"
    return f"{size[0]}x{size[1]}"


def capture_label(
    size: tuple[int, int] | None,
    scale_factor: float,
    index: int,
    *,
    state_name: str | None = None,
) -> str:
    label = f"{size_label(size, index)}@{scale_factor:g}x"
    return f"{label}-{state_name}" if state_name else label


def compare_capture_diagnostics(captures: list[dict[str, Any]]) -> list[dict[str, Any]]:
    groups: dict[tuple[str, float], list[dict[str, Any]]] = {}
    for capture in captures:
        key = (str(capture.get("size") or "default"), safe_float(capture.get("scale"), 1.0))
        groups.setdefault(key, []).append(capture)
    comparisons: list[dict[str, Any]] = []
    for (size, scale), group in groups.items():
        if len(group) < 2:
            continue
        baseline = group[0]
        baseline_counts = baseline.get("diagnostic_counts") or {}
        for capture in group[1:]:
            current = capture.get("diagnostic_counts") or {}
            codes = sorted(set(baseline_counts) | set(current))
            delta = {
                code: int(current.get(code, 0)) - int(baseline_counts.get(code, 0))
                for code in codes
                if int(current.get(code, 0)) != int(baseline_counts.get(code, 0))
            }
            comparisons.append(
                {
                    "size": size,
                    "scale": scale,
                    "from_state": baseline.get("state"),
                    "to_state": capture.get("state"),
                    "diagnostic_delta": delta,
                }
            )
    return comparisons


def relative_artifact(path: Path, out_dir: Path) -> str:
    return str(path.relative_to(out_dir)).replace("\\", "/")


def suspected_modules(target: dict[str, Any]) -> list[str]:
    category = target.get("category")
    features = {str(feature).lower() for feature in target.get("features", [])}
    modules = ["native/src/runtime.rs", "native/src/primitives/mod.rs"]
    if category == "layout" or any("layout" in feature or "grid" in feature for feature in features):
        modules.append("native/src/layout.rs")
    if category == "plots" or any(
        feature in features for feature in {"scatter3d", "scatterplot2d", "lineplot", "histogram"}
    ):
        modules.extend(["native/src/scatter/mod.rs", "native/src/table.rs"])
    if any("text" in feature or "codeeditor" in feature or "logview" in feature for feature in features):
        modules.append("native/src/text/mod.rs")
    if any("htmlreport" in feature for feature in features):
        modules.append("native/src/html_report_webview.rs")
    return sorted(set(modules))


def write_report(out_dir: Path, results: list[dict[str, Any]]) -> None:
    report_json = out_dir / "report.json"
    report_json.write_text(json.dumps(results, indent=2, sort_keys=True), encoding="utf-8")

    lines = [
        "# DragonGUI Visual Audit Report",
        "",
        f"Generated: {time.strftime('%Y-%m-%d %H:%M:%S %Z')}",
        "",
        "This is a manifest-driven visual audit. `pass` means the saved screenshot state was visually reviewed against `artifacts/SPEC.md` and no obvious defect was recorded. `needs_manual_interaction` means the static capture was reviewed, but important hover/open/drag/focus or animated states still need manual or automated interaction coverage.",
        "",
        "## Summary",
        "",
    ]
    counts: dict[str, int] = {}
    for result in results:
        counts[result["status"]] = counts.get(result["status"], 0) + 1
    for status in ("pass", "needs_manual_interaction", "fail", "blocked"):
        lines.append(f"- {status}: {counts.get(status, 0)}")
    lines.extend(["", "## Targets", ""])

    for result in results:
        lines.extend(
            [
                f"### {result['name']} (`{result['id']}`)",
                "",
                f"- Status: `{result['status']}`",
                f"- Priority: `{result['priority']}`",
                f"- Probe: `{result['script']}`",
                f"- Features: {', '.join(f'`{feature}`' for feature in result['features'])}",
                f"- Screenshots: {format_paths(result['screenshots'])}",
                f"- Debug snapshots: {format_paths(result['snapshots'])}",
                f"- Logs: {format_paths(result['logs'])}",
                f"- Unmatched selectors: {format_code_values(result.get('unmatched_selectors', []))}",
                f"- Layout diagnostics by code: {format_issue_counts(result.get('layout_issue_counts', {}))}",
                f"- Notes: {result['notes']}",
                f"- Suspected modules: {', '.join(f'`{module}`' for module in result['suspected_modules'])}",
                f"- Reproduction: {format_repro(result['reproduction'])}",
                "",
            ]
        )
        captures = result.get("captures", [])
        screenshot_captures = [
            capture
            for capture in captures
            if isinstance(capture, dict) and isinstance(capture.get("screenshot"), str)
        ]
        if screenshot_captures:
            lines.extend(
                [
                    "#### Capture Gallery",
                    "",
                    "| Size | Scale | Route | State | Thumbnail |",
                    "| --- | ---: | --- | --- | --- |",
                ]
            )
            for capture in screenshot_captures:
                screenshot = str(capture["screenshot"]).replace(" ", "%20")
                route = str(capture.get("route") or "_default_")
                state = str(capture.get("state") or "default")
                alt = f"{result['id']} {capture.get('size')} {state}".replace('"', "'")
                thumbnail = (
                    f'<a href="{screenshot}"><img src="{screenshot}" '
                    f'width="240" alt="{alt}"></a>'
                )
                lines.append(
                    f"| `{capture.get('size', 'default')}` | "
                    f"`{safe_float(capture.get('scale'), 1.0):g}x` | "
                    f"`{route}` | `{state}` | {thumbnail} |"
                )
            lines.append("")
        comparisons = result.get("diagnostic_comparisons", [])
        if comparisons:
            lines.extend(["#### Diagnostic State Comparisons", ""])
            for comparison in comparisons:
                delta = comparison.get("diagnostic_delta") or {}
                rendered_delta = (
                    ", ".join(f"`{code}` {change:+d}" for code, change in delta.items())
                    if delta
                    else "_no diagnostic changes_"
                )
                lines.append(
                    f"- `{comparison.get('size')} @ "
                    f"{safe_float(comparison.get('scale'), 1.0):g}x`: "
                    f"`{comparison.get('from_state')}` → "
                    f"`{comparison.get('to_state')}` — {rendered_delta}"
                )
            lines.append("")
        layout_issues = result.get("layout_issues", [])
        if layout_issues:
            lines.extend(
                [
                    "| Code | Widget | Page | Route / state | Size / scale | Artifacts | Reason |",
                    "| --- | --- | --- | --- | --- | --- | --- |",
                ]
            )
            for issue in layout_issues:
                artifacts = [markdown_artifact_link(issue.get("snapshot"), "snapshot")]
                if issue.get("node_data"):
                    artifacts.append(markdown_artifact_link(issue.get("node_data"), "node data"))
                if issue.get("screenshot"):
                    artifacts.append(
                        markdown_artifact_link(issue.get("screenshot"), "screenshot")
                    )
                message = str(issue.get("message") or "").replace("|", "\\|")
                page = str(issue.get("page_id") or "_root_")
                lines.append(
                    f"| `{issue.get('code', 'unknown')}` | "
                    f"`{issue.get('widget_id', 'unknown')}` "
                    f"(`{issue.get('widget_type', 'unknown')}`) | `{page}` | "
                    f"`{issue.get('route') or '_default_'} / "
                    f"{issue.get('state') or 'default'}` | "
                    f"`{issue.get('size', 'default')} @ "
                    f"{safe_float(issue.get('scale'), 1.0):g}x` | "
                    f"{', '.join(artifacts)} | {message} |"
                )
            lines.append("")

    (out_dir / "REPORT.md").write_text("\n".join(lines), encoding="utf-8")


def format_paths(paths: list[str]) -> str:
    if not paths:
        return "_none_"
    return ", ".join(markdown_artifact_link(path, Path(path).name) for path in paths)


def markdown_artifact_link(path: object, label: str) -> str:
    if not isinstance(path, str) or not path:
        return "_none_"
    return f"[{label}]({path.replace(' ', '%20')})"


def format_issue_counts(counts: object) -> str:
    if not isinstance(counts, dict) or not counts:
        return "_none_"
    return ", ".join(
        f"`{code}`: {count}"
        for code, count in sorted(counts.items())
    )


def format_code_values(values: list[str]) -> str:
    if not values:
        return "_none_"
    return ", ".join(f"`{value}`" for value in values)


def format_repro(steps: list[str]) -> str:
    if not steps:
        return "_none_"
    return "; ".join(f"`{step}`" for step in steps)


if __name__ == "__main__":
    raise SystemExit(main())
