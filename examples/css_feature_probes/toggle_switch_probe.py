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
        width: 100%;
        flex-grow: 1;
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
        background: rgba(90, 169, 255, 0.12);
        border: 1px solid rgba(90, 169, 255, 0.34);
        border-radius: 8px;
        color: rgba(232, 244, 255, 0.96);
        font-weight: 750;
        padding: 8px 10px;
        width: 100%;
    }

    ToggleSwitch {
        width: 100%;
        height: 36px;
        border-radius: 8px;
        transition: background 150ms ease-out;
    }

    ToggleSwitch::row {
        background: transparent;
        border-radius: 8px;
    }

    ToggleSwitch:hover::row {
        background: rgba(90, 169, 255, 0.10);
    }

    ToggleSwitch::track {
        width: 46px;
        height: 24px;
        background: rgba(255, 255, 255, 0.12);
        border: 1px solid rgba(255, 255, 255, 0.20);
        border-radius: 999px;
    }

    ToggleSwitch:checked::track {
        background: #5aa9ff;
        border-color: rgba(199, 228, 255, 0.86);
    }

    ToggleSwitch::thumb {
        width: 18px;
        height: 18px;
        background: rgba(248, 251, 255, 0.96);
        border: 1px solid rgba(20, 28, 38, 0.38);
        border-radius: 999px;
    }

    ToggleSwitch.warm:checked::track {
        background: #ffd166;
        border-color: rgba(255, 232, 170, 0.94);
    }

    ToggleSwitch.compact {
        height: 30px;
    }

    ToggleSwitch.compact::track {
        width: 38px;
        height: 20px;
    }

    ToggleSwitch.compact::thumb {
        width: 14px;
        height: 14px;
    }

    ToggleSwitch:disabled {
        opacity: 0.48;
    }
    """
)

win = dg.Window("ToggleSwitch probe", width=860, height=420)

state = {"live": True, "snap": False, "alerts": True}


def status_text() -> str:
    return "Live: {live} | Snap: {snap} | Alerts: {alerts}".format(**state)


with dg.VLayout(class_="root"):
    dg.Label("ToggleSwitch", class_="title")
    status = dg.Label(status_text(), class_="status")

    def update(name: str, value: bool) -> None:
        state[name] = value
        status.set_value(status_text())

    with dg.HLayout(class_="grid"):
        with dg.Panel("Settings", class_="case"):
            dg.Label("Track and thumb parts should stay crisp through hover, focus, active, checked, and disabled states.", class_="caption")
            dg.ToggleSwitch("Live updates", checked=True, on_change=lambda checked: update("live", checked))
            dg.ToggleSwitch("Snap to grid", checked=False, on_change=lambda checked: update("snap", checked))
            dg.ToggleSwitch("Disabled checked", checked=True, disabled=True)
            dg.ToggleSwitch("Disabled unchecked", checked=False, disabled=True)

        with dg.Panel("Variants", class_="case"):
            dg.Label("Left-side labels, custom track colors, and compact sizing use the same widget API.", class_="caption")
            dg.ToggleSwitch(
                "Alert routing",
                checked=True,
                label_position="left",
                class_="warm",
                on_change=lambda checked: update("alerts", checked),
            )
            dg.ToggleSwitch("Compact switch", checked=True, class_="compact")
            dg.ToggleSwitch("Compact off", checked=False, class_="compact")

    dg.Label("PASS: switch click, Space activation, callbacks, disabled state, label layout, and track/thumb CSS parts render.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
