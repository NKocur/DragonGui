from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))
    sys.path.insert(0, str(Path(__file__).resolve().parent))

import dragongui as dg

from probe_helpers import probe_card, probe_grid, probe_header


class SegmentFrame:
    columns = ("segment", "revenue", "tickets")
    dtypes = ("object", "float32", "int32")
    shape = (10, 3)
    segment = [
        "Enterprise",
        "Team",
        "Enterprise",
        "Free",
        "Team",
        "Trial",
        "Enterprise",
        "Free",
        "Partner",
        "Team",
    ]
    revenue = [42.0, 18.0, 35.0, 3.0, 22.0, 6.0, 48.0, 4.0, 14.0, 21.0]
    tickets = [11, 7, 9, 24, 8, 3, 10, 18, 5, 6]

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


frame = SegmentFrame()

app = dg.App(theme=dg.Theme.dark(accent="#69b7ff", radius=8, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #0b1020;
        color: rgba(246, 249, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
        overflow-y: auto;
        padding-right: 20px;
    }

    VLayout.root::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb {
        width: 6px;
        background: rgba(105, 183, 255, 0.76);
        border-radius: 999px;
    }

    Label.title {
        font-size: 21px;
        font-weight: 700;
        color: #f7fbff;
    }

    Label.caption {
        color: rgba(214, 224, 242, 0.76);
        line-height: 1.25;
        text-wrap: wrap;
    }

    GridLayout.grid {
        width: 100%;
        gap: 12px;
        row-gap: 12px;
    }

    Panel.case {
        min-height: 305px;
        padding: 12px;
        gap: 10px;
        background: linear-gradient(145deg, rgba(26, 37, 62, 0.96), rgba(13, 19, 34, 0.98));
        border: 1px solid rgba(121, 155, 205, 0.28);
        border-radius: 10px;
        box-shadow: 0 14px 34px rgba(0, 0, 0, 0.26);
    }

    PieChart {
        min-height: 238px;
        background: rgba(6, 10, 20, 0.58);
        border: 1px solid rgba(130, 165, 215, 0.24);
        border-radius: 8px;
    }

    PieChart.clean {
        background: linear-gradient(145deg, rgba(255, 255, 255, 0.08), rgba(255, 255, 255, 0.025));
        border-color: rgba(255, 255, 255, 0.16);
    }

    PieChart.donut {
        background: radial-gradient(circle at 30% 20%, rgba(105, 183, 255, 0.12), transparent 42%),
                    rgba(6, 10, 20, 0.58);
    }
    """
)

win = dg.Window("Pie Chart Probe", width=980, height=760)

with win:
    with dg.VLayout(class_="root"):
        probe_header(
            "Pie Chart Probe",
            "Checks direct slices, frame aggregation, donut mode, top-N grouping, custom colors, legends, and optional slice labels.",
        )

        with probe_grid(columns=2, min_column_width=420, gap=12, row_gap=12):
            with probe_card("Direct Slice Values"):
                dg.PieChart(
                    labels=["Compute", "Storage", "Network", "Support"],
                    values=[42, 27, 18, 13],
                    title="Cloud Spend",
                    colors=["#69b7ff", "#76e0b1", "#ffd36a", "#f36b7f"],
                    class_="clean",
                    show_labels=True,
                )

            with probe_card("Donut With Legend"):
                dg.PieChart(
                    labels=["North", "South", "East", "West"],
                    values=[31, 26, 22, 21],
                    title="Regional Mix",
                    donut=True,
                    inner_radius=0.58,
                    class_="donut",
                )

            with probe_card("Frame Count Aggregation"):
                dg.PieChart(
                    frame,
                    category="segment",
                    aggregate="count",
                    title="Accounts By Segment",
                    top_n=4,
                    other_label="Long tail",
                )

            with probe_card("Frame Sum Aggregation"):
                dg.PieChart(
                    frame,
                    category="segment",
                    value="revenue",
                    aggregate="sum",
                    title="Revenue By Segment",
                    top_n=3,
                    donut=True,
                    show_labels=True,
                    colors=["#7ab8ff", "#8be7bd", "#ffe083", "#ff8aa1"],
                )


if __name__ == "__main__":
    print(app.run(win))
