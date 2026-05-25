from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

from probe_helpers import probe_app, probe_grid, probe_header


app, win = probe_app("Layout Overlay Collision Probe", width=980, height=740)
app.stylesheet(
    """
    Window {
        background: #0f141c;
        color: rgba(245, 248, 255, 0.94);
        padding: 0;
        gap: 0;
        font-size: 14px;
    }

    MenuBar {
        background: rgba(8, 13, 20, 0.98);
        border-bottom: 1px solid rgba(255, 255, 255, 0.12);
        padding-left: 8px;
    }

    Menu {
        background: transparent;
        border: 0;
        border-radius: 6px;
        color: rgba(245, 248, 255, 0.78);
        padding: 6px 10px;
    }

    Menu:hover,
    Menu:open {
        background: rgba(90, 169, 255, 0.16);
        color: white;
    }

    MenuItem {
        color: rgba(245, 248, 255, 0.90);
        padding: 7px 10px;
    }

    MenuItem:hover {
        background: rgba(90, 169, 255, 0.16);
    }

    VLayout.root {
        width: 100%;
        flex: 1;
        min-height: 0;
        overflow-y: auto;
        overflow-x: hidden;
        padding: 16px;
        padding-right: 24px;
        padding-bottom: 44px;
        gap: 12px;
    }

    VLayout.root::scrollbar-track,
    Panel::scrollbar-track,
    Modal::scrollbar-track,
    ScrollArea::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb,
    Panel::scrollbar-thumb,
    Modal::scrollbar-thumb,
    ScrollArea::scrollbar-thumb {
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
        row-gap: 12px;
    }

    Panel.case {
        width: 100%;
        min-width: 0;
        min-height: 292px;
        background: rgba(22, 30, 41, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 10px;
        padding: 12px;
        gap: 10px;
    }

    Panel.edge-field {
        position: relative;
        min-height: 300px;
        background:
            linear-gradient(90deg, rgba(255, 255, 255, 0.045) 1px, transparent 1px),
            linear-gradient(180deg, rgba(255, 255, 255, 0.045) 1px, transparent 1px),
            rgba(255, 255, 255, 0.035);
        background-size: 36px 36px;
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 8px;
        padding: 10px;
    }

    Button.anchor,
    DragSource.anchor {
        position: absolute;
        width: 148px;
        min-width: 0;
        height: 34px;
        background: rgba(90, 169, 255, 0.16);
        border: 1px solid rgba(90, 169, 255, 0.42);
        color: white;
        font-weight: 800;
        padding: 0 10px;
        text-align: center;
    }

    Button.top-left {
        left: 10px;
        top: 12px;
    }

    Button.top-right {
        right: 10px;
        top: 12px;
    }

    Button.bottom-left {
        left: 10px;
        bottom: 12px;
    }

    DragSource.bottom-right {
        right: 10px;
        bottom: 12px;
        align-items: center;
        justify-content: center;
        border-radius: 8px;
    }

    DragSource.bottom-right:selected {
        background: rgba(255, 211, 106, 0.18);
        border-color: rgba(255, 211, 106, 0.76);
    }

    DragSource.source-card {
        width: 100%;
        min-width: 0;
        min-height: 64px;
        background: rgba(90, 169, 255, 0.10);
        border: 1px solid rgba(90, 169, 255, 0.34);
        border-radius: 8px;
        padding: 9px 10px;
        gap: 4px;
    }

    DragSource.source-card:selected {
        background: rgba(255, 211, 106, 0.16);
        border-color: rgba(255, 211, 106, 0.76);
    }

    Label.source-title {
        color: white;
        font-weight: 850;
    }

    DragSource.bottom-right Label.source-title {
        width: 100%;
        text-align: center;
    }

    Panel.dense-copy {
        width: 100%;
        min-width: 0;
        background: rgba(255, 255, 255, 0.040);
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 8px;
        padding: 9px 10px;
        gap: 6px;
    }

    Label.copy-row {
        width: 100%;
        min-width: 0;
        background: rgba(255, 255, 255, 0.040);
        border-radius: 6px;
        color: rgba(245, 248, 255, 0.72);
        padding: 5px 7px;
    }

    HLayout.action-row {
        width: 100%;
        min-width: 0;
        min-height: 36px;
        gap: 8px;
        align-items: center;
    }

    Button,
    SmallButton,
    IconButton {
        flex-shrink: 0;
    }

    Button.shrink,
    SmallButton.shrink {
        flex-shrink: 1;
        min-width: 0;
    }

    Dropdown.fill,
    SearchBox.fill {
        width: 100%;
        min-width: 0;
    }

    Panel.scroll-host {
        width: 100%;
        height: 188px;
        min-height: 0;
        overflow-y: auto;
        padding-right: 22px;
        padding-bottom: 16px;
        background: rgba(255, 255, 255, 0.035);
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 8px;
        gap: 8px;
    }

    DropTarget.drop-zone {
        width: 100%;
        min-height: 78px;
        background: rgba(255, 255, 255, 0.040);
        border: 1px dashed rgba(116, 221, 176, 0.38);
        border-radius: 8px;
        padding: 10px;
        gap: 6px;
    }

    DropTarget.drop-zone:selected {
        background: rgba(116, 221, 176, 0.14);
        border-color: rgba(116, 221, 176, 0.82);
    }

    HLayout.search-box {
        height: 36px;
        background: rgba(255, 255, 255, 0.055);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 8px;
        padding: 4px;
    }

    HLayout.search-box TextInput {
        background: transparent;
        border-color: transparent;
        height: 28px;
        padding-left: 4px;
        padding-right: 4px;
    }

    IconButton.search-box-icon,
    IconButton.search-box-clear {
        width: 28px;
        height: 28px;
        border-radius: 999px;
        padding: 5px;
        background: transparent;
        border-color: transparent;
    }

    Selectable.command-palette-row {
        min-height: 34px;
        border-radius: 7px;
    }

    Selectable.command-palette-row:selected {
        background: rgba(90, 169, 255, 0.18);
        border-color: rgba(90, 169, 255, 0.58);
    }

    Modal {
        background: rgba(13, 20, 31, 0.98);
        border: 1px solid rgba(90, 169, 255, 0.42);
        border-radius: 12px;
        color: rgba(245, 248, 255, 0.94);
        padding: 14px;
        gap: 10px;
        box-shadow: 0 26px 70px rgba(0, 0, 0, 0.50);
    }

    Modal::scrim {
        background: rgba(3, 7, 13, 0.56);
        backdrop-filter: blur(4px) saturate(1.08);
    }

    Tooltip,
    Dropdown::menu,
    ContextMenu {
        background: rgba(9, 15, 25, 0.98);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 10px;
        color: rgba(245, 248, 255, 0.94);
        box-shadow: 0 20px 42px rgba(0, 0, 0, 0.44);
    }

    Tooltip {
        padding: 10px;
        gap: 7px;
    }

    Dropdown::item-hover {
        background: rgba(90, 169, 255, 0.16);
    }

    Dropdown::item-selected {
        background: rgba(90, 169, 255, 0.28);
        color: white;
        font-weight: 850;
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


with dg.MenuBar(height=34):
    with dg.Menu("Overlay"):
        dg.MenuItem("Open command palette", on_click=lambda: palette.show())
        dg.MenuItem("Show success toast", on_click=lambda: show_toast("success"))
        dg.MenuItem("Disabled item", disabled=True)
    with dg.Menu("View"):
        dg.MenuItem("Open modal", on_click=lambda: modal.show())
        dg.MenuItem("Reset status", on_click=lambda: mark("Status reset"))


def dense_rows(prefix: str, count: int = 6) -> None:
    with dg.Panel("", class_="dense-copy"):
        for index in range(1, count + 1):
            dg.Label(
                f"{prefix} row {index}: overlay text should remain readable over this background text.",
                class_="copy-row",
            )


def mark(message: str) -> None:
    status.set_value(message)


def show_toast(level: str) -> None:
    mark(f"{level.title()} toast requested")
    dg.toast(
        f"{level.title()} overlay over dense content",
        level=level,
        duration=1800,
        position="top-right",
    )


commands = [
    dg.Command("open", "Open Diagnostics", subtitle="Overlay", keywords=("inspect",), on_run=lambda: mark("Command: open diagnostics")),
    dg.Command("toggle", "Toggle Dense Layer", subtitle="View", keywords=("layout",), on_run=lambda: mark("Command: toggle dense layer")),
    dg.Command("export", "Export Snapshot", subtitle="File", keywords=("save",), on_run=lambda: mark("Command: export snapshot")),
    dg.Command("disabled", "Disabled Command", subtitle="Unavailable", disabled=True),
]

modal = dg.Modal("Overlay modal", open=False, width=430, height=220, close_button=True)
palette = dg.CommandPalette(
    commands,
    open=False,
    width=500,
    height=330,
    on_run=lambda command: mark(f"Command: {command.id}"),
)


with dg.VLayout(class_="root"):
    probe_header(
        "Overlay collision stress",
        "Open overlays over dense content, scroll hosts, and edge targets. Surfaces should stay above text, clamp to the window, and remain readable.",
    )
    status = dg.Label("Ready: hover, right-click, drag, open dropdowns, and show modal/palette.", class_="status")

    with probe_grid(columns=2, min_column_width=360, gap=12, row_gap=12):
        with dg.Panel("Edge placement", class_="case"):
            dg.Label("Hover and right-click targets near each edge. Tooltips and menus should clamp inside the window.", class_="caption")
            with dg.Panel("", class_="edge-field"):
                top_left = dg.Button("Top left", class_="anchor top-left", tooltip="Static tooltip near the top-left edge.", on_click=lambda: mark("Top left clicked"))
                top_right = dg.Button("Top right", class_="anchor top-right", tooltip="Static tooltip near the top-right edge.", on_click=lambda: mark("Top right clicked"))
                bottom_left = dg.Button("Bottom left", class_="anchor bottom-left", tooltip="Static tooltip near the bottom-left edge.", on_click=lambda: mark("Bottom left clicked"))
                with dg.DragSource(
                    {"kind": "overlay", "id": "edge-drag", "label": "Edge drag"},
                    drag_kind="overlay",
                    class_="anchor bottom-right",
                    tooltip="Drag ghost should remain visible over dense content.",
                ):
                    dg.Label("Drag edge", class_="source-title", wrap=False)

        with dg.Panel("Dense background overlays", class_="case"):
            dg.Label("Modal, toast, and command palette should not make underlying text disappear or visually merge.", class_="caption")
            with dg.HLayout(class_="action-row"):
                rich_target = dg.Button("Rich tooltip", class_="shrink", on_click=lambda: mark("Rich tooltip target clicked"))
                dg.Button("Open modal", on_click=modal.show)
                dg.Button("Palette", on_click=palette.show)
                dg.IconButton("bell", tooltip="Show toast", on_click=lambda: show_toast("info"))
            dense_rows("Dense overlay", 6)
            with dg.Tooltip(target=rich_target, width=300, height=136):
                dg.Label("Rich tooltip", class_="source-title")
                dg.Label("This is laid out as a real overlay container above dense text.", class_="caption")
                dg.ProgressBar(0.64, show_value=True)

        with dg.Panel("Scroll host collision", class_="case"):
            dg.Label("Open the dropdown while this panel has scrollable content. The menu should draw over rows without clipping.", class_="caption")
            with dg.Panel("", class_="scroll-host"):
                for index in range(1, 5):
                    dg.Label(f"Before dropdown row {index}", class_="copy-row")
                dg.Dropdown(
                    ("Primary layer", "Secondary layer", "Long option near scroll edge", "Disabled-looking background"),
                    value="Primary layer",
                    class_="fill",
                    on_change=lambda value: mark(f"Dropdown: {value}"),
                )
                for index in range(5, 13):
                    dg.Label(f"After dropdown row {index}: menu should overlay this row.", class_="copy-row")
            dg.Label("PASS: dropdown menu stays legible over scroll content.", class_="pass")

        with dg.Panel("Drag ghost and drop target", class_="case"):
            dg.Label("Drag the edge source or the source below over dense rows and this target.", class_="caption")
            with dg.DragSource(
                {"kind": "overlay", "id": "payload-card", "label": "Payload card"},
                drag_kind="overlay",
                class_="source-card",
            ):
                dg.Label("Payload card", class_="source-title")
                dg.Label("The drag ghost should be visible while crossing text and panels.", class_="caption")
            dense_rows("Drag background", 4)
            with dg.DropTarget(
                accept="overlay",
                class_="drop-zone",
                on_drop=lambda drop: mark(f"Dropped {drop.payload.get('label', drop.kind) if isinstance(drop.payload, dict) else drop.kind}"),
            ):
                dg.Label("Drop overlay payload here", class_="source-title")
                dg.Label("Target highlight should sit inside the panel bounds.", class_="caption")

    with modal:
        dg.Label("Modal with native close button", class_="source-title")
        dg.Label("The circular native close button should stay centered in the top-right chrome and above dense page content.", class_="caption")
        dense_rows("Modal background", 3)
        with dg.HLayout(class_="action-row"):
            dg.Button("Toast on modal", on_click=lambda: show_toast("warning"))
            dg.Button("Close", on_click=modal.close)


for target, name in [
    (top_left, "top-left"),
    (top_right, "top-right"),
    (bottom_left, "bottom-left"),
]:
    with dg.ContextMenu(target=target, width=230, parent=win):
        dg.MenuItem(f"Inspect {name}", on_click=lambda value=name: mark(f"Inspect {value}"))
        dg.MenuItem("Show toast", on_click=lambda: show_toast("success"))
        dg.MenuItem("Disabled action", disabled=True)


if __name__ == "__main__":
    print(app.run(win))
