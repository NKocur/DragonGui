from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))
    sys.path.insert(0, str(Path(__file__).resolve().parent))

import dragongui as dg
from probe_helpers import probe_grid


app = dg.App(theme=dg.Theme.dark(accent="#69d2ff", radius=8, focus="#ffd166"))
app.stylesheet(
    """
    Window {
        background: #0f141d;
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

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(246, 249, 255, 0.70);
        line-height: 1.14;
    }

    GridLayout.grid {
        width: 100%;
        flex-grow: 1;
        min-height: 0;
        gap: 12px;
        row-gap: 12px;
    }

    Panel.case {
        min-height: 0;
        background: rgba(22, 31, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        padding: 14px;
        gap: 10px;
    }

    Panel.tile {
        background: rgba(18, 26, 36, 0.94);
        border: 1px solid rgba(105, 210, 255, 0.22);
        border-radius: 9px;
        padding: 12px;
        gap: 8px;
    }

    Label.metric-name {
        color: rgba(246, 249, 255, 0.68);
        font-weight: 650;
    }

    Label.metric-value {
        color: white;
        font-size: 22px;
        font-weight: 850;
    }

    ExtensionWidget {
        width: 100%;
        border-radius: 8px;
        border: 1px solid rgba(105, 210, 255, 0.28);
        background: linear-gradient(135deg, rgba(105, 210, 255, 0.20), rgba(117, 224, 177, 0.10));
        box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.04);
    }

    ExtensionWidget.sparkline {
        background: linear-gradient(90deg, rgba(105, 210, 255, 0.16), rgba(105, 210, 255, 0.04));
        border-color: rgba(105, 210, 255, 0.34);
    }

    ExtensionWidget.gauge {
        background: linear-gradient(90deg, rgba(117, 224, 177, 0.18), rgba(255, 209, 102, 0.10));
        border-color: rgba(117, 224, 177, 0.34);
    }

    HLayout.actions {
        width: 100%;
        flex-grow: 0;
        flex-shrink: 0;
        gap: 8px;
    }

    Badge.action-pill {
        flex: 1;
        min-width: 0px;
        text-overflow: ellipsis;
    }

    Button {
        height: 34px;
        text-align: center;
    }
    """
)


@dg.component
def StatusTile(ctx: dg.ComponentCtx, name: str, value: str, level: str):
    clicks = ctx.state("clicks", 0)

    def bump() -> None:
        clicks.set(int(clicks.value) + 1)

    with dg.Panel(name, class_="tile", key=f"{name}-tile", parent=None) as panel:
        dg.Label(name.upper(), class_="metric-name", key="metric-name")
        dg.Label(f"{value}  /  updates {clicks.value}", class_="metric-value", key="metric-value")
        dg.Badge(level, level=level if level != "normal" else "info", key="level-badge")
        dg.Button("Patch component state", on_click=bump, key="bump")
    return panel


@dg.component
def ExtensionHooksProbe(ctx: dg.ComponentCtx):
    win = dg.Window("V5 custom widget hooks probe", width=960, height=620)

    with dg.VLayout(class_="root"):
        dg.Label("V5 extension hooks", class_="title")
        dg.Label(
            "Composite widgets should patch through normal state and keys. ExtensionWidget boxes should keep stable CSS, intrinsic sizing, and layout.",
            class_="caption",
        )

        with probe_grid(min_column_width=360, gap=12, row_gap=12):
            with dg.Panel("Composite widgets", class_="case"):
                dg.Label("These cards are pure Python components built from existing widgets.", class_="caption")
                StatusTile("Latency", "12.8 ms", "success", key="latency")
                StatusTile("Queue depth", "47 jobs", "warning", key="queue")

            with dg.Panel("Extension leaf widgets", class_="case"):
                dg.Label("These are native extension leaves. Paint and events are intentionally not active yet.", class_="caption")
                dg.ExtensionWidget(
                    "sparkline",
                    {"series": [2, 4, 3, 8, 6, 10], "label": "frame time"},
                    intrinsic_height=44,
                    class_="sparkline",
                    key="sparkline",
                )
                dg.ExtensionWidget(
                    "gauge",
                    {"value": 0.72, "label": "cache hit rate"},
                    intrinsic_height=64,
                    class_="gauge",
                    key="gauge",
                )
                with dg.HLayout(class_="actions"):
                    dg.Badge("ExtensionWidget", level="info", class_="action-pill")
                    dg.Badge("CSS type selector", level="success", class_="action-pill")

        dg.Label("PASS: component state/keys update, extension leaves render as styled layout participants.", class_="caption")

    return win


win = ExtensionHooksProbe()


if __name__ == "__main__":
    print(app.run(win))
