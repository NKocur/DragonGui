from __future__ import annotations

import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

os.environ.setdefault("DG_PRO_DEMO_POINTS", "4000")
os.environ.setdefault("DG_PRO_DEMO_ROWS", "80")
os.environ.setdefault("DG_PRO_DEMO_SERIES_POINTS", "240")

import dragongui as dg
import all_features_professional_demo as demo


TARGET_ROUTE = {
    "all-features-professional-overview": "overview",
    "all-features-professional-explore": "explore",
    "all-features-professional-timeseries": "timeseries",
    "all-features-professional-data": "data",
    "all-features-professional-workflow": "workflow",
    "all-features-professional-reports": "reports",
    "all-features-professional-runtime": "runtime",
    "all-features-professional-styling": "styling",
}


def initial_route() -> str:
    target = os.environ.get("DRAGONGUI_AUDIT_TARGET", "all-features-professional-overview")
    explicit = os.environ.get("DG_PRO_DEMO_INITIAL_ROUTE")
    return explicit or TARGET_ROUTE.get(target, "overview")


app, win = demo.build_app()
route = initial_route()
if demo.state.pages is not None:
    demo.state.pages.set_value(route)
if demo.state.tabs is not None:
    demo.state.tabs.set_value(route)


if __name__ == "__main__":
    try:
        print(app.run(win))
    except dg.BackendUnavailableError as exc:
        print(f"Professional demo visual probe requires native backend: {exc}")
        raise
