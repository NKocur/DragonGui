from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

from probe_helpers import probe_app, probe_header

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual visual probe requirement
    raise SystemExit("heatmap_probe.py requires NumPy") from exc


def correlation_matrix() -> np.ndarray:
    labels = np.arange(9, dtype=np.float32)
    distance = np.abs(labels[:, None] - labels[None, :])
    base = np.exp(-distance / 2.6)
    ripple = np.sin((labels[:, None] + 1.0) * (labels[None, :] + 2.0) * 0.24) * 0.16
    return (base + ripple).clip(-1.0, 1.0).astype(np.float32)


def dense_sensor_grid(phase: float = 0.0) -> np.ndarray:
    y = np.linspace(-2.4, 2.4, 96, dtype=np.float32)
    x = np.linspace(-3.2, 3.2, 144, dtype=np.float32)
    xx, yy = np.meshgrid(x, y)
    ridge = np.sin(xx * 2.2 + phase) * np.cos(yy * 1.5 - phase * 0.35)
    hot_spot = np.exp(-((xx - 1.05) ** 2 + (yy + 0.45) ** 2) * 1.9)
    cool_spot = np.exp(-((xx + 1.55) ** 2 + (yy - 0.85) ** 2) * 2.8)
    return (ridge * 0.45 + hot_spot * 1.2 - cool_spot * 0.75).astype(np.float32)


phase = {"value": 0.0}

feature_labels = ["temp", "load", "rpm", "flow", "vib", "volt", "amp", "press", "fan"]
confusion_labels = ["idle", "warm", "run", "peak", "fault"]
confusion = np.array(
    [
        [96, 8, 1, 0, 0],
        [7, 88, 12, 2, 0],
        [2, 10, 91, 15, 2],
        [0, 2, 11, 84, 9],
        [0, 0, 2, 12, 93],
    ],
    dtype=np.float32,
)

app, win = probe_app("Heatmap Probe", width=1040, height=760)
app.stylesheet(
    """
    Window {
        background: #101521;
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

    Label.status {
        width: 100%;
        padding: 8px 10px;
        background: rgba(99, 179, 237, 0.12);
        border: 1px solid rgba(99, 179, 237, 0.34);
        border-radius: 8px;
        color: rgba(231, 245, 255, 0.96);
        font-weight: 740;
    }

    HLayout.row {
        width: 100%;
        gap: 12px;
        height: 260px;
        min-height: 0;
    }

    Panel.case {
        flex-grow: 1;
        flex-shrink: 1;
        flex-basis: 0;
        min-width: 0;
        min-height: 0;
        padding: 12px;
        gap: 8px;
        background: rgba(18, 26, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        overflow: hidden;
    }

    Heatmap {
        width: 100%;
        height: 100%;
        min-height: 190px;
        background: #0b1020;
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 8px;
    }

    HLayout.controls {
        width: 100%;
        height: 36px;
        gap: 8px;
    }

    Button {
        height: 32px;
        padding-left: 12px;
        padding-right: 12px;
        font-weight: 760;
    }
    """
)

status: dg.Label | None = None


def set_status(text: str) -> None:
    if status is not None:
        status.set_value(text)


def hover_status(cell: dg.HeatmapCell | None) -> None:
    if cell is None:
        set_status("Hover a heatmap cell to inspect row, column, and value.")
        return
    row = cell.y_label or f"row {cell.row}"
    col = cell.x_label or f"col {cell.col}"
    set_status(f"{row} / {col}: {cell.value:.4g}")


def refresh_dense() -> None:
    phase["value"] += 0.7
    dense.set_data(dense_sensor_grid(phase["value"]))
    set_status(f"Dense sensor grid refreshed at phase {phase['value']:.2f}.")


with win:
    with dg.VLayout(class_="root"):
        probe_header(
            "Heatmap",
            "Packed matrix rendering with labels, scalar bars, and native hover readout.",
        )
        status = dg.Label("Hover a heatmap cell to inspect row, column, and value.", class_="status")
        with dg.HLayout(class_="row"):
            with dg.Panel("Correlation matrix", class_="case"):
                dg.Heatmap(
                    correlation_matrix(),
                    x_labels=feature_labels,
                    y_labels=feature_labels,
                    colormap="coolwarm",
                    clim=(-1.0, 1.0),
                    title="Feature correlation",
                    on_hover=hover_status,
                )
            with dg.Panel("Confusion matrix", class_="case"):
                dg.Heatmap(
                    confusion,
                    x_labels=confusion_labels,
                    y_labels=confusion_labels,
                    colormap="blues",
                    title="Classifier counts",
                    on_hover=hover_status,
                )
        with dg.HLayout(class_="row"):
            with dg.Panel("Dense sensor grid", class_="case"):
                dense = dg.Heatmap(
                    dense_sensor_grid(),
                    colormap="turbo",
                    show_labels=False,
                    scalar_bar=True,
                    title="96 x 144 live grid",
                    on_hover=hover_status,
                )
        with dg.HLayout(class_="controls"):
            dg.Button("Refresh dense grid", on_click=refresh_dense)


if __name__ == "__main__":
    print(app.run(win))
