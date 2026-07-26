from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App()
app.stylesheet(
    """
    Window {
        background: #101722;
        color: #eef7ff;
        padding: 18px;
        gap: 14px;
    }

    GridLayout.metric-grid {
        width: 100%;
        gap: 12px;
    }

    Panel.metric-card {
        min-width: 0;
        min-height: 140px;
        background: linear-gradient(145deg, #243746, #1b2a36);
        border: 1px solid rgba(255, 255, 255, 0.14);
        padding: 12px;
        gap: 8px;
    }
    """
)


with dg.Window("Responsive Grid Orphan Baseline", width=1024, height=640) as win:
    with dg.VLayout(style={"width": "100%", "height": "100%", "gap": 12}):
        dg.Label("Adaptive metric grid", style={"font_size": 22, "font_weight": 850})
        dg.Label(
            "Explicit logical-viewport breakpoints produce 4 columns on wide screens, "
            "2 columns on compact screens, and one column on phones."
        )
        with dg.GridLayout(
            columns={"default": 4, 1100: 2, 700: 1},
            min_column_width=210,
            balance_last_row=True,
            gap=12,
            class_="metric-grid",
        ):
            for label, value in (
                ("Availability", "99.98%"),
                ("Median latency", "34 ms"),
                ("Review queue", "12"),
                ("Events / minute", "8.4k"),
            ):
                with dg.Panel(class_="metric-card"):
                    dg.Label(label)
                    dg.Label(value, style={"font_size": 28, "font_weight": 900})
                    dg.ProgressBar(0.72, show_value=False)


if __name__ == "__main__":
    print(app.run(win))
