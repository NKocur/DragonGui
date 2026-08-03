from __future__ import annotations

import math
from pathlib import Path
import sys
import threading
import time

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))
    sys.path.insert(0, str(Path(__file__).resolve().parent))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual visual probe requirement
    raise SystemExit("histogram_live_update_probe.py requires NumPy") from exc

from probe_helpers import probe_card, probe_header


class HistogramFrame:
    columns = ("value",)
    dtypes = ("float32",)

    def __init__(self, values: object) -> None:
        self.value = np.asarray(values, dtype=np.float32)
        self.shape = (len(self.value), 1)

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


def frame_for_step(step: int, rows: int = 3200) -> HistogramFrame:
    rng = np.random.default_rng(700 + step)
    phase = step * 0.28
    left_center = 32.0 + math.sin(phase) * 12.0
    right_center = 86.0 + math.cos(phase * 0.72) * 18.0
    left_weight = 0.48 + math.sin(phase * 0.55) * 0.22
    left_count = int(rows * left_weight)
    right_count = rows - left_count
    left = rng.normal(left_center, 8.0 + math.cos(phase) * 2.0, left_count)
    right = rng.normal(right_center, 15.0 + math.sin(phase * 0.8) * 4.0, right_count)
    values = np.clip(np.concatenate([left, right]), 0.0, 140.0)
    return HistogramFrame(values)


app = dg.App(theme=dg.Theme.dark(accent="#7bd88f", radius=8, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #0b1020;
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
        min-height: 440px;
        padding: 12px;
        gap: 10px;
        background: linear-gradient(145deg, rgba(24, 38, 53, 0.96), rgba(12, 18, 29, 0.98));
        border: 1px solid rgba(132, 180, 156, 0.30);
        border-radius: 10px;
    }

    Histogram.live {
        min-height: 330px;
        background: rgba(6, 10, 20, 0.58);
        border: 1px solid rgba(123, 216, 143, 0.28);
        border-radius: 8px;
        color: #7bd88f;
    }

    Label.caption {
        color: rgba(214, 224, 242, 0.76);
        text-wrap: wrap;
    }
    """
)

win = dg.Window("Live Histogram Update Probe", width=900, height=620)
status = dg.Label("Waiting for live updates...")
histogram: dg.Histogram | None = None
stop = threading.Event()

with dg.VLayout(class_="root"):
    probe_header(
        "Live histogram update probe",
        "The histogram below is one widget. A background worker calls Histogram.set_data() every 0.7s; the bars should morph without rebuilding the page.",
    )
    with probe_card("Live set_data"):
        status = dg.Label("Step 0: initial data", class_="caption")
        histogram = dg.Histogram(
            frame_for_step(0),
            value="value",
            bins=36,
            range=(0.0, 140.0),
            label="Latency mixture",
            x_label="value",
            y_label="samples",
            color="#7bd88f",
            show_toolbar=True,
            class_="live",
        )


def live_worker() -> None:
    step = 1
    while not stop.is_set():
        time.sleep(0.7)
        frame = frame_for_step(step)

        def update(step: int = step, frame: HistogramFrame = frame) -> None:
            if histogram is None:
                return
            histogram.set_data(frame)
            status.set_value(f"Step {step}: {frame.shape[0]} samples pushed through live Histogram.set_data()")

        app.call_soon_threadsafe(update, coalesce_key="histogram-live.latest")
        step += 1


threading.Thread(target=live_worker, name="histogram-live-probe", daemon=True).start()


if __name__ == "__main__":
    try:
        print(app.run(win))
    finally:
        stop.set()
