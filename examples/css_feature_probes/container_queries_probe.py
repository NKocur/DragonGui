from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))
    sys.path.insert(0, str(Path(__file__).resolve().parent))

import dragongui as dg
from probe_helpers import probe_app, probe_header


app, win = probe_app("CSS Container Queries Probe", width=900, height=430)

app.stylesheet(
    """
    Window {
        background: #0d1320;
        color: rgba(245, 248, 255, 0.94);
        padding: 18px;
        gap: 14px;
        font-size: 14px;
    }

    Label.title {
        color: #5aa9ff;
        font-size: 20px;
        font-weight: 800;
    }

    Label.caption {
        color: rgba(245, 248, 255, 0.72);
        line-height: 1.12;
    }

    HLayout.cards {
        gap: 14px;
        height: 286px;
    }

    Panel.query-card {
        container-name: card;
        container-type: inline-size;
        min-height: 230px;
        padding: 14px;
        gap: 10px;
        background: rgba(18, 25, 39, 0.94);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 8px;
    }

    Panel.narrow {
        width: 280px;
    }

    Panel.wide {
        width: 520px;
    }

    Label.case-title {
        color: rgba(245, 248, 255, 0.96);
        font-weight: 800;
    }

    Label.status {
        width: 190px;
        padding: 8px;
        background: rgba(255, 101, 132, 0.18);
        border: 1px solid rgba(255, 101, 132, 0.54);
        border-radius: 6px;
        color: #ff9caf;
        font-weight: 800;
    }

    Label.detail {
        color: rgba(245, 248, 255, 0.68);
    }

    Panel.swatch {
        width: 120px;
        height: 54px;
        padding: 8px;
        background: rgba(255, 255, 255, 0.055);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 6px;
    }

    @container card (max-width: 300px) {
        Label.status {
            width: 150px;
            background: rgba(255, 211, 106, 0.18);
            border-color: rgba(255, 211, 106, 0.64);
            color: #ffd36a;
        }

        Panel.swatch {
            width: 86px;
            background: rgba(255, 211, 106, 0.14);
            border-color: rgba(255, 211, 106, 0.46);
        }
    }

    @container card (min-width: 420px) {
        Label.status {
            width: 260px;
            background: rgba(116, 221, 176, 0.16);
            border-color: rgba(116, 221, 176, 0.58);
            color: #74ddb0;
        }

        Panel.swatch {
            width: 220px;
            background: rgba(116, 221, 176, 0.12);
            border-color: rgba(116, 221, 176, 0.46);
        }
    }
    """
)

with win:
    probe_header(
        "Container queries",
        "Left card should use the max-width branch; right card should use the min-width branch.",
    )
    with dg.HLayout(class_="cards"):
        with dg.Panel("Narrow container", class_="query-card narrow"):
            dg.Label("280px card", class_="case-title")
            dg.Label("NARROW QUERY MATCHED", class_="status")
            with dg.Panel(class_="swatch"):
                dg.Label("compact", class_="detail")
            dg.Label("Status badge and swatch should be yellow and compact.", class_="detail")

        with dg.Panel("Wide container", class_="query-card wide"):
            dg.Label("520px card", class_="case-title")
            dg.Label("WIDE QUERY MATCHED", class_="status")
            with dg.Panel(class_="swatch"):
                dg.Label("expanded", class_="detail")
            dg.Label("Status badge and swatch should be green and wider.", class_="detail")


if __name__ == "__main__":
    print(app.run(win))
