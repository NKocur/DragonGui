"""Verify that a DragonGUI wheel works without importing the source tree.

Set ``PYTHONPATH`` to an extracted wheel before invoking this script.  The
caller supplies the extraction directory so an accidental source-tree import
fails the smoke check instead of producing a false positive.
"""

from __future__ import annotations

import argparse
import threading
from pathlib import Path

import dragongui as dg

BASE_CSS = """
AppShell.wheel-smoke { gap: 8px; padding: 10px; }
Panel.wheel-card { min-width: 0; }
Button.primary { background: accent; color: background; }
"""

RELOADED_CSS = """
AppShell.wheel-smoke { gap: 10px; padding: 12px; }
Panel.wheel-card { min-width: 0; border-color: accent; }
Button.primary { background: accent; color: background; }
"""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--expected-root", type=Path, required=True)
    parser.add_argument("--live-reload", action="store_true")
    args = parser.parse_args()

    package_file = Path(dg.__file__).resolve()
    expected_root = args.expected_root.resolve()
    if expected_root not in package_file.parents:
        raise RuntimeError(
            f"expected wheel import below {expected_root}, imported {package_file}"
        )

    app = dg.App(theme=dg.Theme.dark())
    app.stylesheet(BASE_CSS)
    with dg.Window("Packaged Wheel Smoke", width=640, height=480) as window:
        with dg.AppShell(class_="wheel-smoke"):
            with dg.Panel("Packaged runtime", class_="wheel-card"):
                dg.Label("Imported from the extracted wheel.")
                dg.Button("Ready", class_="primary")

    if args.live_reload:
        def schedule_reload() -> None:
            app.call_soon_threadsafe(
                lambda: (app.clear_stylesheets(), app.stylesheet(RELOADED_CSS))
            )

        threading.Timer(0.08, schedule_reload).start()

    result = app.run(window)
    if result.get("status") != "ok":
        raise RuntimeError(f"wheel runtime returned {result!r}")

    snapshot = result.get("debug_snapshot") or {}
    gpu = snapshot.get("gpu") or {}
    stylesheets = gpu.get("stylesheets") or {}
    layout = gpu.get("layout") or {}
    if stylesheets.get("warning_count", 0):
        raise RuntimeError(f"wheel stylesheet warnings: {stylesheets!r}")
    bad_layout_nodes: dict[str, object] = {}
    for node_id, diagnostic in (layout.get("diagnostics") or {}).items():
        if not isinstance(diagnostic, dict):
            continue
        inspection = diagnostic.get("inspection") or {}
        if (
            diagnostic.get("issues")
            or inspection.get("structural_diagnostics")
            or inspection.get("usability_advisories")
        ):
            bad_layout_nodes[node_id] = diagnostic
    if bad_layout_nodes:
        raise RuntimeError(f"wheel layout issues: {bad_layout_nodes!r}")
    if args.live_reload:
        commands = ((snapshot.get("runtime") or {}).get("commands") or {}).get(
            "recent", []
        )
        command_names = {entry.get("command") for entry in commands}
        if not {"ClearStylesheets", "SetStylesheet"}.issubset(command_names):
            raise RuntimeError(
                f"live stylesheet reload was not observed: {sorted(command_names)!r}"
            )

    print(f"wheel_package={package_file}")
    print(
        "status=ok "
        f"renderer={result.get('renderer')} "
        f"frame_ms={result.get('frame_ms')} "
        f"user_rules={stylesheets.get('user_rules')} "
        f"live_reload={args.live_reload}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
