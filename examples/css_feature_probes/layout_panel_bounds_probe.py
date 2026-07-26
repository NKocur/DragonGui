from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

from probe_helpers import probe_app, probe_grid, probe_header


class BoundsFrame:
    columns = ("metric", "owner", "state", "p50_ms", "p95_ms", "enabled")
    dtypes = ("str", "str", "str", "float64", "float64", "bool")

    def __init__(self, rows: int = 18) -> None:
        self.shape = (rows, len(self.columns))
        self.metric = [f"pipeline.segment.{idx:02d}.duration" for idx in range(1, rows + 1)]
        self.owner = [("ingest", "model", "export", "review")[idx % 4] for idx in range(rows)]
        self.state = [("ready", "backfill", "blocked", "active")[idx % 4] for idx in range(rows)]
        self.p50_ms = [round(3.4 + (idx * 1.8) % 14.0, 2) for idx in range(rows)]
        self.p95_ms = [round(11.0 + (idx * 2.7) % 42.0, 2) for idx in range(rows)]
        self.enabled = [idx % 5 != 3 for idx in range(rows)]

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


app, win = probe_app("Layout Panel Bounds Probe", width=940, height=720)
app.stylesheet(
    """
    Window {
        background: #0f141c;
        color: rgba(245, 248, 255, 0.94);
        padding: 16px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        overflow-y: auto;
        overflow-x: hidden;
        padding-right: 22px;
        padding-bottom: 30px;
        gap: 12px;
    }

    VLayout.root::scrollbar-track,
    ScrollArea::scrollbar-track,
    DataFrameTable::scrollbar-track,
    LogView::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb,
    ScrollArea::scrollbar-thumb,
    DataFrameTable::scrollbar-thumb,
    LogView::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.72);
        border-radius: 999px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(245, 248, 255, 0.70);
        line-height: 1.12;
    }

    Label.status {
        width: 100%;
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 8px;
        color: rgba(229, 255, 244, 0.96);
        font-weight: 800;
        padding: 8px 10px;
    }

    GridLayout.grid {
        width: 100%;
        height: auto;
        gap: 12px;
    }

    Panel.case {
        min-width: 0;
        background: rgba(22, 30, 41, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 10px;
        padding: 12px;
        gap: 10px;
    }

    Panel.fixed {
        height: 310px;
        min-height: 0;
    }

    Panel.compact-fixed {
        height: 252px;
        min-height: 0;
    }

    Panel.inner {
        width: 100%;
        min-width: 0;
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 8px;
        padding: 10px;
        gap: 8px;
    }

    HLayout.field-row {
        width: 100%;
        min-width: 0;
        min-height: 36px;
        gap: 8px;
        align-items: center;
    }

    HLayout.actions {
        width: 100%;
        min-width: 0;
        min-height: 34px;
        gap: 8px;
        align-items: center;
    }

    Label.field-label {
        width: 116px;
        flex-shrink: 0;
        color: rgba(245, 248, 255, 0.72);
        font-weight: 750;
    }

    TextInput.fill,
    Dropdown.fill,
    NumberInput.fill,
    Slider.fill,
    ProgressBar.fill {
        flex: 1;
        min-width: 0;
    }

    TextInput.fill,
    Dropdown.fill,
    NumberInput.fill {
        height: 34px;
    }

    ProgressBar.fill {
        height: 18px;
    }

    Button,
    SmallButton {
        flex-shrink: 0;
    }

    Button.shrink,
    SmallButton.shrink {
        flex-shrink: 1;
        min-width: 0;
    }

    Badge,
    Tag {
        flex-shrink: 0;
    }

    FlowLayout.chips {
        width: 100%;
        min-width: 0;
        gap: 8px;
        row-gap: 8px;
    }

    ScrollArea.panel-scroll {
        width: 100%;
        flex: 1;
        min-height: 0;
        padding-right: 22px;
        padding-bottom: 18px;
        gap: 8px;
    }

    LogView.bounds-log {
        width: 100%;
        flex: 1;
        min-height: 0;
        background: rgba(5, 9, 14, 0.72);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 8px;
        color: rgba(230, 238, 248, 0.90);
        font-family: "Consolas";
        font-size: 12px;
        line-height: 18px;
        padding: 8px 10px;
    }

    DataFrameTable.bounds-table {
        width: 100%;
        flex: 1;
        min-height: 0;
        background: rgba(5, 9, 14, 0.72);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 8px;
        color: rgba(230, 238, 248, 0.90);
        font-size: 12px;
        table-row-height: 25px;
        table-header-height: 32px;
        table-column-width: 132px;
        table-index-width: 50px;
    }

    DataFrameTable.bounds-table::header {
        background: rgba(90, 169, 255, 0.18);
        color: white;
        font-weight: 850;
    }

    DataFrameTable.bounds-table::grid-line {
        background: rgba(255, 255, 255, 0.12);
    }

    Label.bleed-check {
        width: 100%;
        min-width: 0;
        background: rgba(255, 211, 106, 0.12);
        border: 1px solid rgba(255, 211, 106, 0.28);
        border-radius: 7px;
        color: rgba(255, 240, 197, 0.96);
        font-weight: 760;
        padding: 7px 9px;
    }

    @media (max-width: 520px) {
        Label.field-label {
            width: 72px;
        }

        SmallButton.shrink {
            min-width: 48px;
        }
    }
    """
)


def set_status(message: str) -> None:
    status.set_value(message)


frame = BoundsFrame()
log_lines = [
    "INFO  panel bounds probe initialized",
    "DEBUG title band measured before body layout",
    "INFO  dense control rows inside auto-height card",
    "WARN  scroll body contains more rows than visible",
    "INFO  table viewport clipped to fixed panel",
    "DEBUG nested panel body padding remains visible",
    "ERROR sample diagnostic text with long trailing payload=alpha.beta.gamma.delta",
]

with dg.VLayout(class_="root"):
    probe_header(
        "Panel content bounds",
        "Resize the window and inspect each titled panel. Content should respect padding, stay inside rounded borders, and scroll when height is constrained.",
    )
    status = dg.Label("Ready: panel body bounds are intentionally stressed.", class_="status")

    with probe_grid(gap=12, min_column_width=390):
        with dg.Panel("Auto-height mixed controls", class_="case"):
            dg.Label("Rows should shrink to the panel body width without clipping right-side badges.", class_="caption")
            with dg.HLayout(class_="field-row"):
                dg.Label("Route", class_="field-label")
                dg.TextInput("north.zone.ingest.primary.stream.with.long.name", class_="fill")
                dg.Badge("live", level="success")
            with dg.HLayout(class_="field-row"):
                dg.Label("Mode", class_="field-label")
                dg.Dropdown(
                    ("Bounded realtime", "Batch replay with diagnostics", "Maintenance hold"),
                    value="Batch replay with diagnostics",
                    class_="fill",
                )
                dg.SmallButton("Apply", class_="shrink", on_click=lambda: set_status("Mode applied"))
            with dg.HLayout(class_="field-row"):
                dg.Label("Pressure", class_="field-label")
                dg.ProgressBar(0.73, show_value=True, class_="fill")
                dg.Tag("watch", level="warning")
            with dg.FlowLayout(class_="chips"):
                for label, level in [
                    ("compact", "neutral"),
                    ("scroll-aware", "info"),
                    ("rounded clipping", "success"),
                    ("long-badge-content", "warning"),
                ]:
                    dg.Tag(label, level=level)

        with dg.Panel("Fixed-height scroll body", class_="case fixed"):
            dg.Label("The title and caption should stay above a bounded scrolling body.", class_="caption")
            with dg.ScrollArea(axis="y", class_="panel-scroll"):
                for index in range(1, 12):
                    with dg.HLayout(class_="field-row"):
                        dg.Label(f"Row {index:02d}", class_="field-label")
                        dg.TextInput(f"bounded.panel.content.item.{index:02d}", class_="fill")
                        dg.Badge("ok" if index % 3 else "long-state", level="success" if index % 3 else "warning")
                dg.Label("PASS TARGET: final row scrolls fully into view inside the panel body.", class_="bleed-check")

        with dg.Panel("Nested panel padding and clipping", class_="case fixed"):
            dg.Label("Nested panels should not touch the parent border or collide with the title band.", class_="caption")
            with dg.ScrollArea(axis="y", class_="panel-scroll"):
                with dg.Panel("Inner group A", class_="inner"):
                    with dg.HLayout(class_="field-row"):
                        dg.Label("Owner", class_="field-label")
                        dg.TextInput("analytics-review-east-queue", class_="fill")
                    dg.Label(
                        "This full-width label should wrap inside the inner panel and not bleed through rounded corners.",
                        class_="bleed-check",
                    )
                with dg.Panel("Inner group B with longer title", class_="inner"):
                    with dg.HLayout(class_="field-row"):
                        dg.Label("Limit", class_="field-label")
                        dg.NumberInput(48, min=0, max=100, step=1, class_="fill")
                        dg.SmallButton("Reset", class_="shrink", on_click=lambda: set_status("Limit reset"))
                    with dg.FlowLayout(class_="chips"):
                        for label in ("alpha", "beta-long-label", "gamma", "delta-review-needed"):
                            dg.Tag(label, level="info")

        with dg.Panel("Log viewport plus footer actions", class_="case fixed"):
            dg.Label("The log should consume remaining height while the footer stays visible and inside padding.", class_="caption")
            log = dg.LogView(log_lines, follow=True, max_lines=100, rows=8, class_="bounds-log")

            def append_log() -> None:
                log.append_line(f"INFO  appended line count={len(log.lines) + 1:02d}")
                set_status("Log line appended")

            with dg.HLayout(class_="actions"):
                dg.Button("Append line", class_="shrink", on_click=append_log)
                dg.SmallButton("Clear", on_click=lambda: (log.clear(), set_status("Log cleared")))
                dg.Badge("footer", level="info")

        with dg.Panel("Short table constrained by panel", class_="case fixed"):
            dg.Label("The table should stay inside the card, with internal scrollbars for rows and columns.", class_="caption")
            dg.DataFrameTable(
                frame,
                page_size=18,
                sample_rows=18,
                sortable=True,
                resizable_columns=True,
                class_="bounds-table",
                on_select=lambda selection: set_status(
                    f"Selected row {selection.row_index}: {selection.column} = {selection.value}"
                ),
            )

        with dg.Panel("Compact title/body pressure", class_="case compact-fixed"):
            dg.Label(
                "This shorter panel forces mixed rows, chips, and footer actions into a tighter body.",
                class_="caption",
            )
            with dg.HLayout(class_="field-row"):
                dg.Label("Profile", class_="field-label")
                dg.Dropdown(("Default", "Operator review", "Very long constrained profile"), value="Operator review", class_="fill")
            with dg.HLayout(class_="field-row"):
                dg.Label("Value", class_="field-label")
                dg.Slider(0.58, class_="fill")
                dg.Badge("58%", level="info")
            with dg.FlowLayout(class_="chips"):
                dg.SmallButton("Save", on_click=lambda: set_status("Saved"))
                dg.SmallButton("Publish", on_click=lambda: set_status("Published"))
                dg.Tag("no clipping", level="success")

    dg.Label(
        "PASS TARGET: fixed panels expose scrollbars, title bands remain clear, and nested content stays inside each rounded panel.",
        class_="caption",
    )


if __name__ == "__main__":
    print(app.run(win))
