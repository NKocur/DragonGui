from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

from probe_helpers import probe_app, probe_header


app, win = probe_app("Layout Grid Masonry Probe", width=1020, height=760)
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
    ScrollArea::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb,
    ScrollArea::scrollbar-thumb {
        width: 6px;
        background: rgba(116, 221, 176, 0.72);
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

    Panel.section {
        width: 100%;
        min-width: 0;
        background: rgba(22, 30, 41, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 10px;
        padding: 12px;
        gap: 10px;
    }

    GridLayout.card-grid {
        width: 100%;
        min-width: 0;
        height: auto;
        gap: 12px;
        row-gap: 12px;
    }

    Panel.card {
        width: 100%;
        min-width: 0;
        background: rgba(255, 255, 255, 0.040);
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 8px;
        padding: 10px;
        gap: 8px;
    }

    Panel.card::accent {
        width: 3px;
        background: rgba(116, 221, 176, 0.78);
    }

    Label.metric {
        width: 100%;
        min-width: 0;
        background: rgba(90, 169, 255, 0.10);
        border: 1px solid rgba(90, 169, 255, 0.24);
        border-radius: 7px;
        color: white;
        font-weight: 850;
        padding: 7px 9px;
    }

    Label.note {
        width: 100%;
        min-width: 0;
        color: rgba(245, 248, 255, 0.70);
        line-height: 1.12;
    }

    HLayout.row {
        width: 100%;
        min-width: 0;
        min-height: 34px;
        gap: 8px;
        align-items: center;
    }

    HLayout.toolbar {
        width: 100%;
        min-width: 0;
        height: 38px;
        background: rgba(255, 255, 255, 0.055);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 8px;
        padding: 4px;
    }

    HLayout.toolbar TextInput,
    HLayout.toolbar SearchBox {
        flex: 1;
        min-width: 0;
    }

    Label.key {
        width: 92px;
        flex-shrink: 0;
        color: rgba(245, 248, 255, 0.66);
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
        height: 32px;
    }

    ProgressBar.fill {
        height: 18px;
    }

    FlowLayout.chips {
        width: 100%;
        min-width: 0;
        gap: 7px;
        row-gap: 7px;
    }

    Badge,
    Tag,
    IconButton,
    SmallButton {
        flex-shrink: 0;
    }

    SmallButton.shrink {
        flex-shrink: 1;
        min-width: 0;
    }

    Splitter.embedded {
        width: 100%;
        height: 150px;
        min-height: 0;
    }

    Splitter.embedded::gutter {
        width: 4px;
        height: 4px;
        background: transparent;
    }

    Panel.pane {
        width: 100%;
        height: 100%;
        min-height: 0;
        min-width: 0;
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.095);
        border-radius: 7px;
        padding: 9px;
        gap: 7px;
    }

    ScrollArea.property-scroll {
        width: 100%;
        height: 158px;
        min-height: 0;
        padding-right: 18px;
        padding-bottom: 14px;
        gap: 7px;
    }

    PropertyGrid {
        width: 100%;
        min-width: 0;
    }

    HLayout.property-row {
        width: 100%;
        min-width: 0;
    }

    HLayout.property-editor {
        flex: 1;
        min-width: 0;
    }

    Label.pass {
        width: 100%;
        min-width: 0;
        background: rgba(255, 211, 106, 0.12);
        border: 1px solid rgba(255, 211, 106, 0.28);
        border-radius: 7px;
        color: rgba(255, 240, 197, 0.96);
        font-weight: 760;
        padding: 7px 9px;
    }
    """
)


def mark(message: str) -> None:
    status.set_value(message)


property_values = {
    "Mode": "Auto",
    "Enabled": True,
    "Gain": 0.42,
    "Window": (18, 76),
    "Notes": "Grid child should remain bounded.",
}

property_schema = {
    "Mode": {"type": "select", "options": ["Auto", "Manual", "Disabled"]},
    "Gain": {"type": "float", "min": 0.0, "max": 1.0, "step": 0.01},
    "Window": {"type": "range", "min": 0, "max": 100, "step": 2},
    "Notes": {"type": "multiline", "rows": 3},
}


with dg.VLayout(class_="root"):
    probe_header(
        "Grid and masonry stress",
        "Resize the window. Masonry cards should pack into the shortest column while standard grid cards stay row-aligned and both modes should collapse cleanly to one column.",
    )
    status = dg.Label("Ready: compare masonry packing against the standard grid below.", class_="status")

    with dg.Panel("Masonry card packing", class_="section"):
        dg.Label(
            "Uneven cards below should not leave large row gaps when masonry is enabled.",
            class_="caption",
        )
        with dg.GridLayout(columns=3, min_column_width=270, masonry=True, gap=12, row_gap=12, class_="card-grid"):
            with dg.Panel("Toolbar card", class_="card"):
                dg.Label("Short action surface", class_="metric")
                with dg.Toolbar():
                    dg.IconButton("play", tooltip="Run", on_click=lambda: mark("Run clicked"))
                    dg.IconButton("pause", tooltip="Pause", on_click=lambda: mark("Pause clicked"))
                    dg.IconButton("save", tooltip="Save", on_click=lambda: mark("Save clicked"))
                    dg.ToolbarSeparator()
                    dg.SmallButton("Deploy", class_="shrink", on_click=lambda: mark("Deploy clicked"))
                dg.Label("This compact card should allow the next card in this column to move up.", class_="note")

            with dg.Panel("Tall property card", class_="card"):
                dg.Label("Inspector", class_="metric")
                dg.Label("A taller card with scrollable properties should stay inside its grid column.", class_="note")
                with dg.ScrollArea(axis="y", class_="property-scroll"):
                    dg.PropertyGrid(
                        property_values,
                        schema=property_schema,
                        on_change=lambda change: mark(f"{change.key}: {change.value!r}"),
                        label_width=92,
                    )
                dg.SmallButton("Normalize", on_click=lambda: mark("Normalize clicked"))

            with dg.Panel("Status chips", class_="card"):
                dg.Label("Wrapped badges", class_="metric")
                with dg.FlowLayout(class_="chips"):
                    for label, level in [
                        ("north-zone", "info"),
                        ("calibrating", "warning"),
                        ("ready", "success"),
                        ("operator-review-required", "danger"),
                        ("batch-window", "neutral"),
                    ]:
                        dg.Tag(label, level=level)
                dg.Label("Chip wrapping should grow the card height without pushing other columns down.", class_="note")

            with dg.Panel("Nested split card", class_="card"):
                dg.Label("Embedded splitter", class_="metric")
                with dg.Splitter(
                    orientation="horizontal",
                    sizes=("1fr", "1fr"),
                    min_sizes=(90, 90),
                    gutter_size=8,
                    class_="embedded",
                ):
                    with dg.Pane():
                        with dg.Panel("Queue", class_="pane"):
                            dg.ProgressBar(0.72, show_value=True, class_="fill")
                            dg.Label("72% fill", class_="note")
                    with dg.Pane():
                        with dg.Panel("Latency", class_="pane"):
                            dg.Label("p95 18.4 ms", class_="metric")
                            dg.RangeSlider((8, 24), min=0, max=50, step=1)

            with dg.Panel("Dense row card", class_="card"):
                dg.Label("Mixed row sizing", class_="metric")
                for label, value, badge in [
                    ("Route", "warehouse.ingest.primary.long-route", "live"),
                    ("Owner", "analytics-operator-review", "ok"),
                    ("Limit", "48", "cap"),
                ]:
                    with dg.HLayout(class_="row"):
                        dg.Label(label, class_="key")
                        if label == "Limit":
                            dg.NumberInput(float(value), min=0, max=100, step=1, class_="fill")
                        else:
                            dg.TextInput(value, class_="fill")
                        dg.Badge(badge, level="success")

            with dg.Panel("Narrative card", class_="card"):
                dg.Label("Wrapping text", class_="metric")
                dg.Label(
                    "This paragraph is intentionally longer so the card becomes taller through natural wrapping instead of a fixed height. It should never overlap the next grid item.",
                    class_="note",
                )
                dg.Label("PASS: card height comes from content.", class_="pass")

    with dg.Panel("Standard row-aligned grid", class_="section"):
        dg.Label(
            "This grid keeps predictable row alignment. It may show vertical gaps by design, but cards should not overlap or resize columns unpredictably.",
            class_="caption",
        )
        with dg.GridLayout(columns=3, min_column_width=270, masonry=False, gap=12, row_gap=12, class_="card-grid"):
            for index, rows in enumerate((2, 5, 3, 4, 2, 6), start=1):
                with dg.Panel(f"Standard card {index}", class_="card"):
                    dg.Label(f"Row-aligned item {index}", class_="metric")
                    for row in range(1, rows + 1):
                        with dg.HLayout(class_="row"):
                            dg.Label(f"Row {row}", class_="key")
                            dg.ProgressBar(row / max(1, rows), show_value=False, class_="fill")
                    dg.SmallButton("Select", on_click=lambda i=index: mark(f"Selected standard card {i}"))

    dg.Label(
        "PASS TARGET: masonry packs uneven cards tightly, standard grid stays aligned, and both modes maintain stable responsive columns.",
        class_="caption",
    )


if __name__ == "__main__":
    print(app.run(win))
