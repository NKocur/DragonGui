from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from types import ModuleType


ROOT = Path(__file__).resolve().parents[1]
DEMO_PATH = ROOT / "examples" / "all_features_professional_demo.py"
GENERATED_DIR = ROOT / "artifacts" / "generated" / "all_features_professional_demo"


def _load_demo(name: str) -> ModuleType:
    spec = importlib.util.spec_from_file_location(name, DEMO_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def _tree_snapshot(path: Path) -> dict[str, tuple[int, int]]:
    if not path.exists():
        return {}
    return {
        str(item.relative_to(path)): (item.stat().st_size, item.stat().st_mtime_ns)
        for item in sorted(path.rglob("*"))
        if item.is_file()
    }


def test_professional_demo_import_is_inert() -> None:
    before = _tree_snapshot(GENERATED_DIR)

    module = _load_demo("all_features_professional_demo_import_test")

    assert _tree_snapshot(GENERATED_DIR) == before
    assert module.SCATTER_FRAME == {}
    assert module.TABLE_ROWS == []
    assert module.SERIES_FRAME == {}
    assert module.HEAT_MATRIX == []


def test_professional_demo_prepares_fixtures_and_builds_app(tmp_path: Path) -> None:
    module = _load_demo("all_features_professional_demo_build_test")

    fixtures = module.prepare_demo_fixtures(tmp_path, points=256, rows=16, series_points=96)
    app, win = module.build_app(tmp_path)
    document = app.document(win)

    assert fixtures["report_path"].exists()
    assert fixtures["image_path"].exists()
    assert len(fixtures["scatter_frame"]["x"]) == 256
    assert len(fixtures["table_rows"]) == 16
    assert document["window"]["props"]["title"] == "DragonGUI Professional All Features Demo"
    assert len(module.ROUTES) == 8
    assert module.state.pages is not None
