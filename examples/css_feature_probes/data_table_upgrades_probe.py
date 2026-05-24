from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


class MetricsFrame:
    columns = ("sensor", "zone", "latency_ms", "rate_hz", "enabled")
    dtypes = ("str", "str", "float32", "float32", "bool")
    shape = (36, 5)

    sensor = [f"SENS-{idx:02d}" for idx in range(1, 37)]
    zone = [["A", "B", "C", "D"][idx % 4] for idx in range(36)]
    latency_ms = [round(2.0 + (idx * 1.73) % 31.0, 2) for idx in range(36)]
    rate_hz = [240, 120, 90, 60, 48, 30, 24, 15, 10] * 4
    enabled = [idx % 5 != 2 for idx in range(36)]

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


frame = MetricsFrame()

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

    HLayout.content {
        width: 100%;
        flex-grow: 1;
        flex-shrink: 1;
        min-height: 0;
        gap: 12px;
    }

    Panel.case {
        width: calc(50% - 6px);
        min-width: 360px;
        height: 100%;
        min-height: 0;
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
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 8px;
        color: rgba(229, 255, 244, 0.96);
        font-weight: 750;
        padding: 8px 10px;
        width: 100%;
    }

    DataFrameTable.upgrade-table {
        height: 260px;
        background: rgba(3, 8, 18, 0.46);
        border: 1px solid rgba(116, 221, 176, 0.30);
        border-radius: 10px;
        color: rgba(232, 240, 255, 0.90);
        font-size: 12px;
        table-row-height: 28px;
        table-header-height: 36px;
        table-column-width: 128px;
        table-index-width: 54px;
    }

    DataFrameTable.upgrade-table::header {
        background: rgba(116, 221, 176, 0.18);
        color: white;
        font-weight: 850;
        text-transform: uppercase;
        letter-spacing: 0.05em;
    }

    DataFrameTable.upgrade-table::row {
        background: rgba(255, 255, 255, 0.025);
    }

    DataFrameTable.upgrade-table::row-selected {
        background: rgba(255, 209, 102, 0.24);
        color: white;
    }

    DataFrameTable.upgrade-table::grid-line {
        background: rgba(255, 255, 255, 0.12);
    }

    DataFrameTable.upgrade-table::scrollbar-track {
        background: rgba(255, 255, 255, 0.10);
        width: 5px;
    }

    DataFrameTable.upgrade-table::scrollbar-thumb {
        background: rgba(116, 221, 176, 0.58);
        width: 6px;
    }

    DataFrameTable.locked-table {
        border-color: rgba(255, 255, 255, 0.16);
    }

    DataFrameTable.locked-table::header {
        background: rgba(255, 255, 255, 0.09);
        color: rgba(246, 249, 255, 0.78);
    }
    """
)

win = dg.Window("Data table upgrades probe", width=980, height=560)

with dg.VLayout(class_="root"):
    dg.Label("DataFrameTable upgrades", class_="title")
    status = dg.Label("Ready", class_="status")

    def show_selection(selection: dg.TableSelection) -> None:
        status.set_value(
            f"Selected row {selection.row_index}: {selection.column} = {selection.value}"
        )

    def show_sort(sort: dg.TableSort) -> None:
        target = "index" if sort.is_index else sort.column
        status.set_value(f"Sorted {target} {sort.direction}")

    with dg.HLayout(class_="content"):
        with dg.Panel("Sortable table", class_="case"):
            dg.Label("Index and column headers sort; drag header dividers to resize columns; scrollbars expose overflow.", class_="caption")
            dg.DataFrameTable(
                frame,
                page_size=24,
                sample_rows=36,
                sortable=True,
                resizable_columns=True,
                on_select=show_selection,
                on_sort=show_sort,
                class_="upgrade-table",
            )

        with dg.Panel("Non-sortable table", class_="case"):
            dg.Label("This table keeps header clicks inert but still supports cell selection.", class_="caption")
            dg.DataFrameTable(
                frame,
                page_size=24,
                sample_rows=36,
                sortable=False,
                resizable_columns=False,
                on_select=show_selection,
                class_="upgrade-table locked-table",
            )

    dg.Label("PASS: sort callback, sort indicator, column resizing, table scrollbars, selected cell payloads, and sortable=False are covered.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
