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
