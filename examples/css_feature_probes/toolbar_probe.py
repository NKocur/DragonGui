from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


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

    Panel.case {
        width: 100%;
        background: rgba(22, 31, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        padding: 14px;
        gap: 12px;
    }

    HLayout.editor-shell {
        width: 100%;
        height: 186px;
        gap: 10px;
    }

    VLayout.editor-body {
        flex: 1;
        min-width: 0;
        height: 100%;
        gap: 10px;
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
        width: 100%;
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 8px;
        color: rgba(229, 255, 244, 0.96);
        font-weight: 750;
        padding: 8px 10px;
    }

    HLayout.toolbar {
        background: rgba(255, 255, 255, 0.055);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 9px;
        padding: 4px;
    }

    HLayout.toolbar-horizontal {
        height: 42px;
    }

    HLayout.toolbar-vertical {
        width: 42px;
    }

    Separator.toolbar-separator {
        background: rgba(255, 255, 255, 0.18);
    }

    IconButton,
    ArrowButton {
        width: 32px;
        height: 32px;
        border-radius: 7px;
        padding: 5px;
    }

    IconButton::icon,
    ArrowButton::icon {
        color: rgba(247, 250, 255, 0.92);
    }

    SmallButton {
        height: 30px;
        border-radius: 7px;
        padding-left: 10px;
        padding-right: 10px;
        font-size: 13px;
        font-weight: 800;
    }

    SmallButton.primary {
        background: rgba(116, 221, 176, 0.16);
        border-color: rgba(116, 221, 176, 0.42);
        color: white;
    }

    IconButton:disabled,
    SmallButton:disabled {
        opacity: 0.46;
    }

    HLayout.search-box {
        flex: 1;
        min-width: 0;
    }

    HLayout.search-box TextInput {
        flex: 1;
        min-width: 0;
    }
    """
)

win = dg.Window("Toolbar probe", width=900, height=520)


with dg.VLayout(class_="root"):
    dg.Label("Toolbar", class_="title")
    status = dg.Label("Ready", class_="status")

    def mark(action: str) -> None:
        status.set_value(f"Action: {action}")

    with dg.Panel("Editor command surface", class_="case"):
        dg.Label("Toolbars keep common actions compact while preserving grouped spacing and embedded controls.", class_="caption")
        with dg.Toolbar():
            dg.IconButton("play", tooltip="Run", on_click=lambda: mark("run"))
            dg.IconButton("pause", tooltip="Pause", on_click=lambda: mark("pause"))
            dg.IconButton("stop", tooltip="Stop", on_click=lambda: mark("stop"))
            dg.ToolbarSeparator()
            dg.IconButton("save", tooltip="Save", on_click=lambda: mark("save"))
            dg.IconButton("search", tooltip="Find", on_click=lambda: mark("find"))
            dg.ToolbarSeparator()
            dg.SearchBox("", placeholder="Filter commands...", on_change=lambda value: mark(f"filter {value}"))
            dg.SmallButton("Deploy", class_="primary", on_click=lambda: mark("deploy"))
            dg.SmallButton("Disabled", disabled=True)

    with dg.Panel("Horizontal and vertical placement", class_="case"):
        with dg.HLayout(class_="editor-shell"):
            with dg.Toolbar(orientation="vertical"):
                dg.ArrowButton("up", tooltip="Move up", on_click=lambda: mark("up"))
                dg.ArrowButton("down", tooltip="Move down", on_click=lambda: mark("down"))
                dg.ToolbarSeparator()
                dg.IconButton("plus", tooltip="Add", on_click=lambda: mark("add"))
                dg.IconButton("close", tooltip="Remove", on_click=lambda: mark("remove"))
            with dg.VLayout(class_="editor-body"):
                dg.Label("Vertical toolbars use horizontal separators and hold square tool buttons without layout jumps.", class_="caption")
                dg.CodeEditor(
                    "def render_frame(frame):\n    frame.prepare()\n    frame.submit()\n",
                    language="python",
                    rows=5,
                    disabled=True,
                )


if __name__ == "__main__":
    print(app.run(win))
