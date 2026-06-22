from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))
    sys.path.insert(0, str(Path(__file__).resolve().parent))

import dragongui as dg

from probe_helpers import probe_card, probe_grid

app = dg.App(theme=dg.Theme.dark(accent="#9dccff", radius=10, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #07111f;
        color: rgba(246, 249, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 14px;
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
        color: rgba(218, 227, 244, 0.82);
        font-size: 14px;
        line-height: 1.25;
    }

    GridLayout.grid {
        width: 100%;
        gap: 12px;
        row-gap: 12px;
    }

    Panel.case {
        min-height: 448px;
        padding: 20px;
        gap: 14px;
        background: linear-gradient(145deg, rgba(31, 48, 74, 0.98), rgba(12, 24, 40, 0.98));
        border: 1px solid rgba(136, 171, 218, 0.38);
        border-radius: 12px;
        box-shadow: 0 18px 40px rgba(0, 0, 0, 0.30);
    }

    PieChart.dashboard {
        min-height: 320px;
        background: linear-gradient(145deg, rgba(8, 19, 33, 0.94), rgba(10, 24, 42, 0.88));
        border: 1px solid rgba(132, 166, 211, 0.42);
        border-radius: 10px;
    }

    PieChart::label {
        color: rgba(237, 243, 255, 0.92);
        font-size: 13px;
        font-weight: 650;
    }
    """
)

PALETTE = ["#2f6fb3", "#247a59", "#a97413", "#ad4058", "#6d4fb0"]

win = dg.Window("Pie Chart Probe", width=1520, height=1000)

with win:
    with dg.VLayout(class_="root"):
        with probe_grid(columns=2, min_column_width=620, gap=14, row_gap=14):
            with probe_card("Cloud spend by category"):
                dg.Label("Share of total cloud expenditure by category.", class_="caption")
                dg.PieChart(
                    labels=["Compute", "Storage", "Network", "Support"],
                    values=[42, 27, 18, 13],
                    colors=PALETTE,
                    donut=True,
                    inner_radius=0.58,
                    label_mode="legend",
                    center_value="$58.4k",
                    center_label="Total spend",
                    show_toolbar=True,
                    class_="dashboard",
                )

            with probe_card("Regional mix"):
                dg.Label("Distribution of accounts by region.", class_="caption")
                dg.PieChart(
                    labels=["Americas", "EMEA", "APAC", "Other"],
                    values=[38, 29, 21, 12],
                    colors=PALETTE,
                    donut=True,
                    inner_radius=0.58,
                    label_mode="legend",
                    center_value="1,842",
                    center_label="Total accounts",
                    show_toolbar=True,
                    class_="dashboard",
                )

            with probe_card("Accounts by segment"):
                dg.Label("Share of customer accounts by segment.", class_="caption")
                dg.PieChart(
                    labels=["Enterprise", "Team", "Free", "Trial", "Long tail"],
                    values=[30, 30, 20, 10, 10],
                    colors=PALETTE,
                    donut=True,
                    inner_radius=0.58,
                    label_mode="legend",
                    center_value="100%",
                    center_label="of accounts",
                    show_toolbar=True,
                    class_="dashboard",
                )

            with probe_card("Revenue by segment"):
                dg.Label("Share of total revenue by customer segment.", class_="caption")
                dg.PieChart(
                    labels=["Enterprise", "Team", "Partner", "Other"],
                    values=[59, 29, 7, 5.8],
                    colors=PALETTE,
                    donut=True,
                    inner_radius=0.58,
                    label_mode="legend",
                    center_value="$212k",
                    center_label="Total revenue",
                    show_toolbar=True,
                    class_="dashboard",
                )


if __name__ == "__main__":
    print(app.run(win))
