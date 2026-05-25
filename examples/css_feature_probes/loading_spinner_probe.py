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

    HLayout.row {
        width: 100%;
        gap: 14px;
        align-items: center;
    }

    HLayout.tile {
        flex: 1;
        min-width: 0;
        height: 92px;
        background: rgba(255, 255, 255, 0.055);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 9px;
        padding: 12px;
        align-items: center;
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

    LoadingSpinner {
        color: rgba(246, 249, 255, 0.90);
        font-weight: 760;
    }

    LoadingSpinner::track {
        background: rgba(255, 255, 255, 0.16);
    }

    LoadingSpinner::arc {
        background: #74ddb0;
    }

    LoadingSpinner.warning::arc {
        background: #ffd166;
    }

    LoadingSpinner.cold::arc {
        background: #78a8ff;
    }

    LoadingSpinner:disabled {
        opacity: 0.48;
    }

    Button {
        height: 32px;
        border-radius: 7px;
        font-weight: 800;
        padding-left: 12px;
        padding-right: 12px;
    }
    """
)

win = dg.Window("LoadingSpinner probe", width=860, height=480)


with dg.VLayout(class_="root"):
    dg.Label("LoadingSpinner", class_="title")
    status = dg.Label("Running", class_="status")

    with dg.Panel("Native spinner states", class_="case"):
        dg.Label("Animated and paused indicators share the same layout and CSS parts.", class_="caption")
        with dg.HLayout(class_="row"):
            with dg.HLayout(class_="tile"):
                dg.LoadingSpinner(size=22, label="Fetching rows", stroke_width=2.6)
            with dg.HLayout(class_="tile"):
                dg.LoadingSpinner(size=30, label="Indexing cache", stroke_width=3.2, speed=0.72, class_="warning")
            with dg.HLayout(class_="tile"):
                dg.LoadingSpinner(size=18, label="Paused", spinning=False, class_="cold")

    with dg.Panel("Live controls", class_="case"):
        spinner = dg.LoadingSpinner(size=26, label="Syncing packets", stroke_width=3, speed=1.2)

        def pause() -> None:
            spinner.set_spinning(False)
            spinner.set_label("Paused")
            status.set_value("Paused")

        def resume() -> None:
            spinner.set_spinning(True)
            spinner.set_label("Syncing packets")
            status.set_value("Running")

        with dg.HLayout(class_="row"):
            dg.Button("Pause", on_click=pause)
            dg.Button("Resume", on_click=resume)
            dg.LoadingSpinner(size=22, label="Disabled", disabled=True)


if __name__ == "__main__":
    print(app.run(win))
