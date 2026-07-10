from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

from probe_helpers import probe_app, probe_header


app, win = probe_app("Layout Flex Stress Probe", width=820, height=680)
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
        padding-bottom: 28px;
        gap: 12px;
    }

    VLayout.root::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb {
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
        border: 1px solid rgba(116, 221, 176, 0.32);
        border-radius: 8px;
        color: rgba(230, 255, 244, 0.96);
        font-weight: 750;
        padding: 8px 10px;
    }

    Panel.case {
        width: 100%;
        min-width: 0;
        background: rgba(22, 30, 41, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 10px;
        padding: 12px;
        gap: 10px;
    }

    Panel.narrow-card {
        width: 100%;
        min-width: 0;
        background: rgba(255, 255, 255, 0.040);
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 8px;
        padding: 10px;
        gap: 8px;
    }

    HLayout.stress-row,
    FlowLayout.stress-row {
        width: 100%;
        min-width: 0;
        min-height: 36px;
        gap: 8px;
        align-items: center;
        background: rgba(255, 255, 255, 0.032);
        border: 1px solid rgba(255, 255, 255, 0.075);
        border-radius: 8px;
        padding: 7px;
    }

    HLayout.tight-row {
        width: 100%;
        min-width: 0;
        gap: 6px;
        align-items: center;
    }

    VLayout.flex-column {
        flex: 1;
        min-width: 0;
        gap: 7px;
    }

    Label.field-label {
        width: 172px;
        flex-shrink: 0;
        color: rgba(245, 248, 255, 0.72);
        font-weight: 700;
    }

    Label.long-value {
        flex: 1;
        min-width: 0;
        background: rgba(90, 169, 255, 0.10);
        border: 1px solid rgba(90, 169, 255, 0.24);
        border-radius: 7px;
        padding: 7px 9px;
        color: rgba(234, 244, 255, 0.96);
    }

    TextInput.flex-control,
    Dropdown.flex-control,
    NumberInput.flex-control,
    Slider.flex-control,
    RangeSlider.flex-control,
    ProgressBar.flex-control {
        flex: 1;
        min-width: 0;
    }

    TextInput.flex-control,
    Dropdown.flex-control,
    NumberInput.flex-control {
        height: 34px;
    }

    Slider.flex-control,
    RangeSlider.flex-control {
        height: 30px;
    }

    ProgressBar.flex-control {
        height: 18px;
    }

    Button.fixed-action,
    SmallButton.fixed-action,
    IconButton.fixed-action {
        flex-shrink: 0;
    }

    Button.shrink-action,
    SmallButton.shrink-action {
        flex-shrink: 1;
        min-width: 0;
    }

    Badge,
    Tag {
        flex-shrink: 0;
    }

    FlowLayout.wrap-row {
        width: 100%;
        min-width: 0;
        gap: 8px;
        row-gap: 8px;
    }

    FlowLayout.compact-tools {
        width: 100%;
        min-width: 0;
        min-height: 36px;
        gap: 8px;
        row-gap: 8px;
        background: rgba(255, 255, 255, 0.032);
        border: 1px solid rgba(255, 255, 255, 0.075);
        border-radius: 8px;
        padding: 7px;
        align-items: center;
    }

    HLayout.two-up {
        width: 100%;
        min-width: 0;
        gap: 10px;
    }

    Panel.half {
        flex: 1;
        min-width: 0;
        background: rgba(255, 255, 255, 0.040);
        border-color: rgba(255, 255, 255, 0.10);
    }

    @media (max-width: 520px) {
        Window {
            padding: 10px;
        }

        VLayout.root {
            padding-right: 8px;
            gap: 10px;
        }

        Panel.case,
        Panel.narrow-card {
            padding: 8px;
            gap: 8px;
        }

        HLayout.stress-row,
        FlowLayout.stress-row {
            gap: 6px;
            padding: 6px;
        }

        Label.field-label {
            width: 104px;
            font-size: 12px;
            line-height: 1.08;
        }

        Button.shrink-action,
        SmallButton.shrink-action {
            min-width: 72px;
        }

        HLayout.two-up {
            flex-direction: column;
            gap: 8px;
        }

        Panel.half {
            width: 100%;
        }

        Label.long-value {
            padding: 6px 7px;
            font-size: 12px;
            line-height: 1.1;
        }
    }
    """
)


def mark(message: str) -> None:
    status.set_value(message)


with dg.VLayout(class_="root"):
    probe_header(
        "Flex shrink/grow stress",
        "Resize this window narrower. Rows should keep controls inside panel bounds, preserve usable inputs, and avoid overlap.",
    )
    status = dg.Label("Ready: stress rows are intentionally dense.", class_="status")

    with dg.Panel("Long labels plus flexible controls", class_="case"):
        with dg.HLayout(class_="stress-row"):
            dg.Label("Extremely long telemetry channel label", class_="field-label")
            dg.TextInput(
                "station-alpha.pipeline.ingest.batch-window.duration-ms",
                class_="flex-control",
                tooltip="Long text input should shrink inside the row.",
            )
            dg.Badge("live", level="success")
            dg.IconButton("save", size=30, class_="fixed-action", tooltip="Save", on_click=lambda: mark("Saved channel"))

        with dg.HLayout(class_="stress-row"):
            dg.Label("Mode with verbose option names", class_="field-label")
            dg.Dropdown(
                [
                    "Realtime bounded latency",
                    "Buffered diagnostics capture",
                    "Maintenance sampling only",
                ],
                value="Buffered diagnostics capture",
                class_="flex-control",
            )
            dg.SmallButton("Apply", class_="fixed-action", on_click=lambda: mark("Mode applied"))

        with dg.HLayout(class_="stress-row"):
            dg.Label("Numeric row with mixed controls", class_="field-label")
            dg.NumberInput(42, min=0, max=100, step=1, class_="flex-control")
            dg.Slider(0.62, class_="flex-control")
            dg.Badge("62%", level="warning")

    with dg.Panel("Nested flexible rows", class_="case"):
        with dg.HLayout(class_="two-up"):
            with dg.Panel("Pipeline A", class_="half"):
                with dg.VLayout(class_="flex-column"):
                    with dg.HLayout(class_="tight-row"):
                        dg.Label("Queue pressure with a very long inline readout", class_="long-value")
                        dg.IconButton("play", size=30, class_="fixed-action", on_click=lambda: mark("Pipeline A started"))
                    with dg.HLayout(class_="tight-row"):
                        dg.ProgressBar(0.78, show_value=True, class_="flex-control")
                        dg.Badge("nominal", level="success")

            with dg.Panel("Pipeline B", class_="half"):
                with dg.VLayout(class_="flex-column"):
                    with dg.HLayout(class_="tight-row"):
                        dg.TextInput("dense window id: B-17-west-long-name", class_="flex-control")
                        dg.IconButton("close", size=30, class_="fixed-action", on_click=lambda: mark("Pipeline B closed"))
                    with dg.HLayout(class_="tight-row"):
                        dg.RangeSlider((18, 84), min=0, max=100, step=2, class_="flex-control")
                        dg.Tag("range locked", level="neutral")

    with dg.Panel("Action rows under pressure", class_="case"):
        with dg.FlowLayout(class_="stress-row", cross_align="center"):
            dg.Label("Toolbar-like action row", class_="field-label")
            dg.Button("Run full diagnostics", class_="shrink-action", on_click=lambda: mark("Diagnostics started"))
            dg.Button("Export selected report", class_="shrink-action", on_click=lambda: mark("Report exported"))
            dg.SmallButton("Reset", class_="fixed-action", on_click=lambda: mark("Reset"))
            dg.IconButton("search", size=30, class_="fixed-action", tooltip="Find")

        with dg.HLayout(class_="stress-row"):
            dg.Label("Status chips should wrap below", class_="field-label")
            with dg.FlowLayout(class_="wrap-row"):
                for label, level in [
                    ("ingest-ready", "success"),
                    ("calibration-waiting", "warning"),
                    ("north-zone", "info"),
                    ("high-resolution-mode", "neutral"),
                    ("manual-override-armed", "danger"),
                    ("recording", "success"),
                ]:
                    dg.Tag(label, level=level)

    with dg.Panel("Constrained card inside a row", class_="case"):
        with dg.HLayout(class_="two-up"):
            with dg.Panel("Compact controls", class_="half"):
                with dg.HLayout(class_="stress-row"):
                    dg.Label("Tiny card label", class_="field-label")
                    dg.TextInput("long value should not punch through the card", class_="flex-control")
                with dg.FlowLayout(class_="compact-tools", cross_align="center"):
                    dg.ToggleSwitch("Verbose safety interlock", checked=True)
                    dg.Badge("armed", level="warning")

            with dg.Panel("Readout", class_="half"):
                dg.Label(
                    "This long paragraph is here to verify wrapping and min-width behavior when a sibling panel becomes narrow.",
                    class_="long-value",
                )
                with dg.HLayout(class_="tight-row"):
                    dg.SmallButton("Acknowledge", class_="shrink-action", on_click=lambda: mark("Acknowledged"))
                    dg.SmallButton("Escalate", class_="shrink-action", on_click=lambda: mark("Escalated"))
                    dg.IconButton("plus", size=30, class_="fixed-action")

    dg.Label(
        "PASS TARGET: no right-edge clipping, no overlapping rows, and inputs remain usable after narrowing the window.",
        class_="caption",
    )


if __name__ == "__main__":
    print(app.run(win))
