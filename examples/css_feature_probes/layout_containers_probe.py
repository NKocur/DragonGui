from __future__ import annotations

import threading
import sys
import time
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #0d1320;
        color: rgba(245, 248, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        overflow-y: auto;
        padding-right: 22px;
        padding-bottom: 76px;
        gap: 12px;
    }

    VLayout.root::scrollbar-track,
    Panel::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb,
    Panel::scrollbar-thumb {
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
        color: rgba(245, 248, 255, 0.72);
        line-height: 1.12;
    }

    Label.case-title {
        color: white;
        font-weight: 850;
    }

    Label.pass {
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 10px;
        color: rgba(229, 255, 244, 0.96);
        font-weight: 800;
        padding: 8px 10px;
        width: 100%;
    }

    Panel {
        background:
            radial-gradient(circle at 14% 12%, rgba(90, 169, 255, 0.12), transparent 52%),
            rgba(18, 25, 39, 0.95);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 14px;
        padding: 14px;
        gap: 10px;
    }

    HLayout.row {
        width: 100%;
        height: auto;
        gap: 12px;
    }

    VLayout.stack {
        width: 100%;
        height: auto;
        gap: 10px;
    }

    Panel.fixed-card {
        width: 210px;
        height: 150px;
        flex-shrink: 0;
        background: rgba(90, 169, 255, 0.12);
        border-color: rgba(90, 169, 255, 0.30);
    }

    Panel.flex-card {
        width: auto;
        height: 150px;
        flex-grow: 1;
        flex-shrink: 1;
        background: rgba(116, 221, 176, 0.10);
        border-color: rgba(116, 221, 176, 0.28);
    }

    Panel.percent-card {
        width: calc(50% - 6px);
        min-width: 260px;
        height: 132px;
        background: rgba(255, 211, 106, 0.10);
        border-color: rgba(255, 211, 106, 0.30);
    }

    Separator {
        background: rgba(255, 255, 255, 0.18);
    }

    Panel.absolute-case {
        width: 100%;
        height: 210px;
        position: relative;
    }

    Tag.pin {
        position: absolute;
        top: 12px;
        right: 14px;
        background: rgba(255, 211, 106, 0.18);
        border: 1px solid rgba(255, 211, 106, 0.38);
        color: #ffe9a8;
    }

    Panel.nested-shell {
        width: 100%;
        height: auto;
        background: rgba(255, 255, 255, 0.045);
        border-color: rgba(255, 255, 255, 0.10);
    }

    Panel.inner {
        width: 100%;
        height: 96px;
        background: rgba(255, 255, 255, 0.055);
        border-color: rgba(255, 255, 255, 0.12);
    }

    Panel.scroll-shell {
        width: 100%;
        height: 318px;
        overflow: hidden;
    }

    VLayout.scroll-case {
        width: 100%;
        height: 210px;
        overflow-y: auto;
        overflow-x: hidden;
        padding-right: 26px;
        padding-bottom: 22px;
        gap: 10px;
    }

    VLayout.scroll-case::scrollbar-thumb {
        background: rgba(255, 211, 106, 0.78);
    }

    Button.scroll-row {
        height: 30px;
        flex-shrink: 0;
    }

    Panel.spacer-case {
        width: 100%;
        height: 180px;
    }

    HLayout.separator-row {
        height: 48px;
        gap: 10px;
        align-items: center;
    }

    Panel.spacer-tile {
        width: 140px;
        height: 50px;
        flex-shrink: 0;
        padding: 10px;
        background: rgba(199, 210, 254, 0.11);
        border-color: rgba(199, 210, 254, 0.28);
    }
    """
)


def print_scroll_snapshot(delay: float = 0.0) -> None:
    def worker() -> None:
        if delay > 0:
            time.sleep(delay)
        try:
            snapshot = app.debug_snapshot(timeout_ms=3000)
        except RuntimeError as exc:
            print("Scroll snapshot failed:", exc, flush=True)
            return

        layout = snapshot.get("gpu", {}).get("layout", {})
        scroll_id = "layout-scroll-body"
        summary = {
            "rect": layout.get("rects", {}).get(scroll_id),
            "clip": layout.get("clips", {}).get(scroll_id),
            "scroll_y": layout.get("scroll_y", {}).get(scroll_id),
            "scroll_max_y": layout.get("scroll_max_y", {}).get(scroll_id),
        }
        print("Layout scroll snapshot:", summary, flush=True)

    threading.Thread(target=worker, daemon=True).start()


win = dg.Window("CSS Layout Containers Probe", width=900, height=720)

with dg.VLayout(class_="root"):
    dg.Label("Layout containers", class_="title")
    dg.Label(
        "This probe isolates DragonGUI container layout: HLayout, VLayout, Panel titles, "
        "absolute children, nested panels, scrollable titled panels, Spacer, and Separator.",
        class_="caption",
    )

    with dg.Panel("HLayout fixed plus flexible sizing"):
        with dg.HLayout(class_="row"):
            with dg.Panel(class_="fixed-card"):
                dg.Label("Fixed 210px", class_="case-title")
                dg.Label("This card should keep its width.", class_="caption")
            with dg.Panel(class_="flex-card"):
                dg.Label("Flexible remainder", class_="case-title")
                dg.Label("This card should fill the remaining row width without overflow.", class_="caption")
        dg.Label("PASS: fixed and flexible panels share one row cleanly.", class_="pass")

    with dg.Panel("VLayout stacking and separators"):
        with dg.VLayout(class_="stack"):
            dg.Label("First stacked row", class_="case-title")
            dg.Separator()
            dg.Label("Horizontal separator should span the stack width.", class_="caption")
            with dg.HLayout(class_="separator-row"):
                dg.Label("Vertical separator")
                dg.Separator(orientation="vertical")
                dg.Label("should divide this row.")
        dg.Label("PASS: stack gap and separators remain stable.", class_="pass")

    with dg.Panel("Percent and calc panel sizing"):
        with dg.HLayout(class_="row"):
            with dg.Panel(class_="percent-card"):
                dg.Label("calc(50% - 6px)", class_="case-title")
                dg.Label("First half-width card.", class_="caption")
            with dg.Panel(class_="percent-card"):
                dg.Label("calc(50% - 6px)", class_="case-title")
                dg.Label("Second half-width card.", class_="caption")
        dg.Label("PASS: cards fit the row and wrap only if the window is genuinely too narrow.", class_="pass")

    with dg.Panel("Absolute child inside titled panel", class_="absolute-case"):
        dg.Tag("Pinned top-right", class_="pin")
        dg.Label("The tag uses position:absolute; top:12px; right:14px.", class_="caption")
        dg.Label("It should sit in the panel body, below the title band, not under the title.", class_="caption")
        dg.Spacer()
        dg.Label("PASS: pinned tag is visible and does not overlap the title.", class_="pass")

    with dg.Panel("Nested panel boundaries", class_="nested-shell"):
        dg.Label("Outer panel padding should remain visible around nested panels.", class_="caption")
        with dg.Panel("Nested child A", class_="inner"):
            dg.Label("Child A content remains inside its own titled body.", class_="caption")
        with dg.Panel("Nested child B", class_="inner"):
            dg.Label("Child B should not touch the parent border.", class_="caption")
        dg.Label("PASS: nested panels keep clear padding and title spacing.", class_="pass")

    with dg.Panel("Scrollable titled panel", class_="scroll-shell"):
        with dg.VLayout(
            id="layout-scroll-body",
            class_="scroll-case",
            style={
                "width": "100%",
                "height": 210,
                "overflow_y": "auto",
                "overflow_x": "hidden",
                "padding_right": 26,
                "padding_bottom": 22,
                "gap": 10,
            },
        ):
            dg.Label(
                "The title should stay above the scrollable body.",
                class_="caption",
                style={"flex_shrink": 0},
            )
            for index in range(1, 11):
                dg.Button(
                    f"Scrollable row {index}",
                    class_="scroll-row",
                    style={"height": 30, "flex_shrink": 0},
                )
            dg.Label(
                "PASS: final row can scroll fully into view.",
                class_="pass",
                style={"flex_shrink": 0},
            )
        dg.Button("Print scroll snapshot", on_click=lambda: print_scroll_snapshot(delay=0.2))

    with dg.Panel("Spacer behavior", class_="spacer-case"):
        with dg.HLayout(class_="row"):
            with dg.Panel(class_="spacer-tile"):
                dg.Label("Left tile")
            dg.Spacer()
            with dg.Panel(class_="spacer-tile"):
                dg.Label("Right tile")
        dg.Label("Spacer should push the right tile to the far side without resizing the tiles.", class_="caption")
        dg.Label("PASS: spacer consumes free row space.", class_="pass")


if __name__ == "__main__":
    print_scroll_snapshot(delay=1.0)
    print(app.run(win))
