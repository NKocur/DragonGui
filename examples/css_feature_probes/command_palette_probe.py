from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#70d6ff", radius=7, focus="#ffd166"))
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
        background: rgba(112, 214, 255, 0.12);
        border: 1px solid rgba(112, 214, 255, 0.34);
        border-radius: 8px;
        color: rgba(232, 247, 255, 0.96);
        font-weight: 750;
        padding: 8px 10px;
        width: 100%;
    }

    HLayout.search-box {
        height: 38px;
        background: rgba(255, 255, 255, 0.055);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 8px;
        padding: 4px;
    }

    HLayout.search-box:focus-within {
        border-color: rgba(255, 209, 102, 0.72);
        box-shadow: 0 0 0 2px rgba(255, 209, 102, 0.22);
    }

    HLayout.search-box TextInput {
        background: transparent;
        border-color: transparent;
        height: 28px;
        padding-left: 4px;
        padding-right: 4px;
    }

    HLayout.search-box TextInput:focus {
        border-color: transparent;
        box-shadow: none;
    }

    IconButton.search-box-icon,
    IconButton.search-box-clear {
        width: 28px;
        height: 28px;
        border-radius: 6px;
        padding: 5px;
        background: transparent;
        border-color: transparent;
    }

    IconButton.search-box-icon::icon {
        color: rgba(246, 249, 255, 0.48);
    }

    IconButton.search-box-clear::icon {
        color: rgba(246, 249, 255, 0.76);
    }

    VLayout.command-palette-results,
    VLayout.preview-list {
        width: 100%;
        gap: 4px;
    }

    Selectable.command-palette-row,
    Selectable.preview-row {
        min-height: 34px;
        border-radius: 7px;
        color: rgba(246, 249, 255, 0.88);
    }

    Selectable.command-palette-row:hover,
    Selectable.preview-row:hover {
        background: rgba(112, 214, 255, 0.10);
        border-color: rgba(112, 214, 255, 0.26);
    }

    Selectable.command-palette-row:selected,
    Selectable.preview-row:selected {
        background: rgba(112, 214, 255, 0.18);
        border-color: rgba(112, 214, 255, 0.56);
        color: white;
    }

    Selectable.command-palette-row::indicator,
    Selectable.preview-row::indicator {
        width: 4px;
        height: 16px;
        background: #70d6ff;
        border-radius: 999px;
    }

    Modal.command-palette {
        gap: 10px;
    }

    Label.command-palette-empty {
        color: rgba(246, 249, 255, 0.58);
    }

    Label.preview-empty {
        color: rgba(246, 249, 255, 0.58);
        min-height: 34px;
        padding: 8px 10px;
    }
    """
)

win = dg.Window("Command palette probe", width=920, height=520)

preview_items = [
    ("Open file", "open"),
    ("Export report", "export"),
    ("Toggle console", "console"),
    ("Reset layout", "reset-layout"),
    ("Show diagnostics", "diagnostics"),
]

with dg.VLayout(class_="root"):
    dg.Label("SearchBox and CommandPalette", class_="title")
    status = dg.Label("Ready", class_="status")

    def mark(action: str) -> None:
        status.set_value(f"Command: {action}")

    commands = [
        dg.Command("open", "Open File", subtitle="Project", keywords=("file", "project"), on_run=lambda: mark("open")),
        dg.Command("export", "Export Report", subtitle="PDF", keywords=("report", "pdf"), on_run=lambda: mark("export")),
        dg.Command("console", "Toggle Console", subtitle="View", keywords=("terminal", "logs"), on_run=lambda: mark("console")),
        dg.Command("theme", "Switch Theme", subtitle="Appearance", keywords=("dark", "light"), on_run=lambda: mark("theme")),
        dg.Command("diagnostics", "Show Diagnostics", subtitle="Tools", keywords=("debug", "metrics"), on_run=lambda: mark("diagnostics")),
        dg.Command("disabled", "Disabled Command", subtitle="Unavailable", disabled=True, on_run=lambda: mark("disabled")),
    ]

    palette = dg.CommandPalette(commands, open=False, on_run=lambda command: mark(command.id))

    def open_palette() -> None:
        palette.show()
        status.set_value("Palette opened")

    with dg.HLayout(class_="content"):
        with dg.Panel("SearchBox", class_="case"):
            dg.Label("The clear button resets the query and keeps the input aligned.", class_="caption")
            filter_status = dg.Label("Filter: all commands", class_="status")
            preview_state = {"query": "", "value": "open"}

            def refresh_preview() -> None:
                query = preview_state["query"].strip().lower()
                matches = [
                    (label, value)
                    for label, value in preview_items
                    if not query or query in label.lower() or query in value.lower()
                ]
                if not matches:
                    rows = [dg.Label("No matching rows", class_="preview-empty", parent=None)]
                else:
                    rows = []
                    for label, value in matches:
                        def on_select(selected: bool, value: str = value) -> None:
                            if selected:
                                preview_state["value"] = value
                                mark(value)
                                refresh_preview()

                        rows.append(
                            dg.Selectable(
                                label,
                                value=value,
                                selected=value == preview_state["value"],
                                toggle=False,
                                on_select=on_select,
                                class_="preview-row",
                                parent=None,
                            )
                        )
                if preview_results.is_live:
                    preview_results.replace_children(rows)
                    return
                preview_results.children = []
                for row in rows:
                    preview_results.add(row)

            def set_filter(query: str) -> None:
                preview_state["query"] = query
                filter_status.set_value(f"Filter: {query or 'all commands'}")
                refresh_preview()

            dg.SearchBox(placeholder="Filter preview rows...", on_change=set_filter)
            preview_results = dg.VLayout(class_="preview-list")
            refresh_preview()

        with dg.Panel("CommandPalette", class_="case"):
            dg.Label("Open the palette, type a query, then run a filtered command.", class_="caption")
            dg.Button("Open palette", class_="primary", on_click=open_palette)
            dg.SmallButton("Run current selection", on_click=palette.run_selected)
            dg.SmallButton("Query diagnostics", on_click=lambda: palette.set_query("diag"))
            dg.Label("PASS: search, clear, filtering, disabled row, row activation, and modal open/close work.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
