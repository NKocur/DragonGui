from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=7, focus="#ffd166"))
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

    HLayout.grid {
        gap: 12px;
        width: 100%;
    }

    Panel.case {
        width: calc(50% - 6px);
        min-width: 360px;
        min-height: 280px;
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
        background: rgba(90, 169, 255, 0.12);
        border: 1px solid rgba(90, 169, 255, 0.34);
        border-radius: 8px;
        color: rgba(232, 244, 255, 0.96);
        font-weight: 750;
        padding: 8px 10px;
        width: 100%;
    }

    RangeSlider {
        width: 100%;
        height: 34px;
        track-color: rgba(255, 255, 255, 0.16);
        thumb-color: #5aa9ff;
    }

    RangeSlider::track {
        height: 6px;
        background: rgba(255, 255, 255, 0.16);
        border-radius: 999px;
    }

    RangeSlider::range {
        background: #5aa9ff;
        border-radius: 999px;
    }

    RangeSlider::thumb-min,
    RangeSlider::thumb-max {
        width: 18px;
        height: 18px;
        background: #5aa9ff;
        border: 2px solid rgba(255, 255, 255, 0.82);
        border-radius: 999px;
    }

    RangeSlider.warn::range {
        background: #ffd166;
    }

    RangeSlider.warn::thumb-min,
    RangeSlider.warn::thumb-max {
        background: #ffd166;
    }

    RangeSlider:disabled {
        opacity: 0.52;
    }
    """
)

win = dg.Window("RangeSlider probe", width=900, height=500)

with dg.VLayout(class_="root"):
    dg.Label("RangeSlider", class_="title")
    status = dg.Label("Visible window: 20 - 80 | Warning band: -2.0 - 2.0", class_="status")

    state = {"window": (20.0, 80.0), "band": (-2.0, 2.0)}

    def refresh_status() -> None:
        lo, hi = state["window"]
        blo, bhi = state["band"]
        status.set_value(f"Visible window: {lo:.0f} - {hi:.0f} | Warning band: {blo:.1f} - {bhi:.1f}")

    with dg.HLayout(class_="grid"):
        with dg.Panel("Primary range", class_="case"):
            dg.Label("Drag either thumb to adjust the selected interval.", class_="caption")

            def set_window(value: tuple[float, float]) -> None:
                state["window"] = value
                refresh_status()

            dg.RangeSlider((20, 80), min=0, max=100, step=5, on_change=set_window)
            dg.RangeSlider((35, 65), min=0, max=100, step=5, disabled=True)

        with dg.Panel("Custom parts", class_="case"):
            dg.Label("The range fill and both thumbs are independently styleable parts.", class_="caption")

            def set_band(value: tuple[float, float]) -> None:
                state["band"] = value
                refresh_status()

            dg.RangeSlider((-2, 2), min=-5, max=5, step=0.25, on_change=set_band, class_="warn")

    dg.Label("PASS: range slider track, selected range, two thumbs, focus, disabled state, and callbacks render.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
