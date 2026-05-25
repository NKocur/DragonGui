from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#74ddb0", radius=7, focus="#ffd166"))
app.stylesheet(
    """
    Window {
        background: #10141b;
        color: rgba(246, 249, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
    }

    Panel.case {
        width: 100%;
        background: rgba(22, 31, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        padding: 14px;
        gap: 12px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(246, 249, 255, 0.70);
        line-height: 1.12;
    }

    Label.status {
        width: 100%;
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 8px;
        color: rgba(229, 255, 244, 0.96);
        font-weight: 750;
        padding: 8px 10px;
    }

    HLayout.breadcrumbs {
        width: 100%;
        min-height: 32px;
        gap: 6px;
        align-items: center;
    }

    SmallButton.breadcrumb-item,
    SmallButton.breadcrumb-current {
        height: 30px;
        padding-left: 10px;
        padding-right: 10px;
        border-radius: 7px;
        background: rgba(255, 255, 255, 0.055);
        border-color: rgba(255, 255, 255, 0.12);
        color: rgba(246, 249, 255, 0.86);
        font-size: 13px;
        font-weight: 750;
    }

    SmallButton.breadcrumb-item:hover {
        background: rgba(116, 221, 176, 0.14);
        border-color: rgba(116, 221, 176, 0.36);
        color: white;
    }

    Label.breadcrumb-current {
        height: 30px;
        padding-left: 10px;
        padding-right: 10px;
        border-radius: 7px;
        background: rgba(116, 221, 176, 0.16);
        border: 1px solid rgba(116, 221, 176, 0.40);
        color: white;
        font-weight: 850;
    }

    Label.breadcrumb-separator {
        width: 12px;
        color: rgba(246, 249, 255, 0.34);
        font-weight: 850;
    }

    Label.breadcrumb-overflow {
        height: 30px;
        padding-left: 8px;
        padding-right: 8px;
        color: rgba(246, 249, 255, 0.58);
        font-weight: 850;
    }

    SmallButton.breadcrumb-disabled,
    Label.breadcrumb-disabled {
        opacity: 0.45;
    }
    """
)

win = dg.Window("Breadcrumbs probe", width=860, height=540)


with dg.VLayout(class_="root"):
    dg.Label("Breadcrumbs", class_="title")
    status = dg.Label("Selected: log_view_probe.py", class_="status")

    def mark(selection: dg.BreadcrumbSelection) -> None:
        status.set_value(f"Selected: {selection.label} ({selection.value})")

    with dg.Panel("File path", class_="case"):
        dg.Label("Long paths collapse the middle while keeping the root and current segment visible.", class_="caption")
        dg.Breadcrumbs(
            [
                ("Workspace", "workspace"),
                ("Projects", "projects"),
                ("DragonFrame", "dragonframe"),
                ("examples", "examples"),
                ("css_feature_probes", "css-feature-probes"),
                ("log_view_probe.py", "log-view-probe"),
            ],
            current="log-view-probe",
            max_items=5,
            on_select=mark,
        )

    with dg.Panel("Object hierarchy", class_="case"):
        dg.Label("Middle segments can be selected and become the current segment immediately.", class_="caption")
        dg.Breadcrumbs(
            [
                {"label": "Dashboard", "value": "dashboard"},
                {"label": "Charts", "value": "charts"},
                {"label": "Latency", "value": "latency"},
                {"label": "P95", "value": "p95"},
            ],
            current="latency",
            click_current=True,
            on_select=mark,
        )

    with dg.Panel("Disabled segment", class_="case"):
        dg.Breadcrumbs(
            [
                ("System", "system"),
                {"label": "Readonly", "value": "readonly", "disabled": True},
                ("Snapshot", "snapshot"),
            ],
            on_select=mark,
        )


if __name__ == "__main__":
    print(app.run(win))
