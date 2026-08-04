from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#7dcfff", radius=8))
app.stylesheet(
    """
    Window {
        background: #10151e;
        color: #edf3fb;
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        min-height: 0;
        gap: 12px;
        overflow-y: auto;
        padding-right: 4px;
    }
    VLayout.stack { width: 100%; gap: 10px; }
    HLayout.row { width: 100%; gap: 12px; align-items: center; }

    Panel.case {
        width: 100%;
        flex-shrink: 0;
        background: #182231;
        border: 1px solid #314156;
        border-radius: 10px;
        padding: 14px;
        gap: 12px;
    }

    Label.title { font-size: 21px; font-weight: 850; color: white; }
    Label.caption { color: #aebdd0; }
    Label.readout {
        width: 190px;
        font-variant-numeric: tabular-nums;
        color: #dce8f6;
    }

    LimitsBar { width: 100%; height: 28px; }
    HLayout.row LimitsBar {
        width: auto;
        min-width: 0;
        flex: 1;
    }
    LimitsBar::track { border: 1px solid #607086; border-radius: 8px; }
    LimitsBar::red-low, LimitsBar::red-high { background: #e5484d; }
    LimitsBar::yellow-low, LimitsBar::yellow-high { background: #f5c542; }
    LimitsBar::green { background: #30a46c; }
    LimitsBar::indicator {
        width: 10px;
        height: 6px;
        background: white;
        border: 1px solid #10151e;
        border-radius: 2px;
    }

    LimitsBar.alternate::red-low, LimitsBar.alternate::red-high { background: #ff6b6b; }
    LimitsBar.alternate::yellow-low, LimitsBar.alternate::yellow-high { background: #ffca58; }
    LimitsBar.alternate::green { background: #56d6a0; }
    LimitsBar:disabled { opacity: 0.48; }

    Label.style-name {
        width: 190px;
        color: #dce8f6;
        font-weight: 700;
    }

    LimitsBar.neon {
        height: 32px;
    }
    LimitsBar.neon::track {
        background: #04131b;
        border: 1px solid #20d9ff;
        border-radius: 4px;
        box-shadow: 0 0 8px rgba(32, 217, 255, 0.38);
    }
    LimitsBar.neon::red-low, LimitsBar.neon::red-high { background: #ff315f; }
    LimitsBar.neon::yellow-low, LimitsBar.neon::yellow-high { background: #f7e55d; }
    LimitsBar.neon::green { background: #00d9a3; }
    LimitsBar.neon::indicator {
        width: 6px;
        height: 20px;
        background: #f5ffff;
        border: 1px solid #052833;
        border-radius: 3px;
        box-shadow: 0 0 7px #ffffff;
    }

    LimitsBar.pastel {
        height: 34px;
    }
    LimitsBar.pastel::track {
        background: #f7f1eb;
        border: 2px solid #d8c9bd;
        border-radius: 14px;
    }
    LimitsBar.pastel::red-low, LimitsBar.pastel::red-high { background: #f3a6a6; }
    LimitsBar.pastel::yellow-low, LimitsBar.pastel::yellow-high { background: #f5d99a; }
    LimitsBar.pastel::green { background: #9ed9ba; }
    LimitsBar.pastel::indicator {
        width: 5px;
        height: 20px;
        background: #493f48;
        border: 0;
        border-radius: 3px;
    }

    LimitsBar.industrial {
        height: 26px;
    }
    LimitsBar.industrial::track {
        background: #16191d;
        border: 2px solid #8d969f;
        border-radius: 0;
    }
    LimitsBar.industrial::red-low, LimitsBar.industrial::red-high { background: #c72f3e; }
    LimitsBar.industrial::yellow-low, LimitsBar.industrial::yellow-high { background: #d89d18; }
    LimitsBar.industrial::green { background: #398755; }
    LimitsBar.industrial::indicator {
        width: 8px;
        height: 18px;
        background: #ffffff;
        border: 2px solid #15181b;
        border-radius: 0;
    }

    LimitsBar.minimal {
        height: 14px;
    }
    LimitsBar.minimal::track {
        background: #273342;
        border: 0;
        border-radius: 7px;
    }
    LimitsBar.minimal::red-low, LimitsBar.minimal::red-high { background: #a94e59; }
    LimitsBar.minimal::yellow-low, LimitsBar.minimal::yellow-high { background: #b59a51; }
    LimitsBar.minimal::green { background: #4f9672; }
    LimitsBar.minimal::indicator {
        width: 3px;
        height: 14px;
        background: white;
        border: 0;
        border-radius: 2px;
    }

    LimitsBar.high-contrast {
        height: 32px;
    }
    LimitsBar.high-contrast::track {
        background: #000000;
        border: 3px solid #ffffff;
        border-radius: 2px;
    }
    LimitsBar.high-contrast::red-low, LimitsBar.high-contrast::red-high { background: #ff1744; }
    LimitsBar.high-contrast::yellow-low, LimitsBar.high-contrast::yellow-high { background: #ffe600; }
    LimitsBar.high-contrast::green { background: #00c853; }
    LimitsBar.high-contrast::indicator {
        width: 8px;
        height: 22px;
        background: #ffffff;
        border: 2px solid #000000;
        border-radius: 1px;
    }

    LimitsBar.monochrome {
        height: 24px;
    }
    LimitsBar.monochrome::track {
        background: #15191e;
        border: 1px solid #9aa3ad;
        border-radius: 3px;
    }
    LimitsBar.monochrome::red-low, LimitsBar.monochrome::red-high { background: #3c4249; }
    LimitsBar.monochrome::yellow-low, LimitsBar.monochrome::yellow-high { background: #707982; }
    LimitsBar.monochrome::green { background: #aeb6bf; }
    LimitsBar.monochrome::indicator {
        width: 5px;
        height: 16px;
        background: #111418;
        border: 1px solid #ffffff;
        border-radius: 0;
    }

    LimitsBar.ice-glass {
        height: 30px;
        opacity: 0.92;
    }
    LimitsBar.ice-glass::track {
        background: rgba(220, 244, 255, 0.10);
        border: 1px solid rgba(200, 238, 255, 0.82);
        border-radius: 10px;
        box-shadow: inset 0 1px 3px rgba(255, 255, 255, 0.38), 0 3px 9px rgba(0, 0, 0, 0.30);
    }
    LimitsBar.ice-glass::red-low, LimitsBar.ice-glass::red-high { background: rgba(255, 111, 145, 0.78); }
    LimitsBar.ice-glass::yellow-low, LimitsBar.ice-glass::yellow-high { background: rgba(255, 220, 128, 0.76); }
    LimitsBar.ice-glass::green { background: rgba(109, 224, 207, 0.74); }
    LimitsBar.ice-glass::indicator {
        width: 7px;
        height: 18px;
        background: #e8fbff;
        border: 1px solid #5ba9be;
        border-radius: 4px;
        box-shadow: 0 0 6px rgba(209, 248, 255, 0.90);
    }

    LimitsBar.terminal {
        height: 25px;
    }
    LimitsBar.terminal::track {
        background: #031108;
        border: 1px solid #39ff88;
        border-radius: 0;
        box-shadow: 0 0 5px rgba(57, 255, 136, 0.34);
    }
    LimitsBar.terminal::red-low, LimitsBar.terminal::red-high { background: #8f2638; }
    LimitsBar.terminal::yellow-low, LimitsBar.terminal::yellow-high { background: #9b8527; }
    LimitsBar.terminal::green { background: #126a39; }
    LimitsBar.terminal::indicator {
        width: 4px;
        height: 19px;
        background: #7dffac;
        border: 0;
        border-radius: 0;
        box-shadow: 0 0 7px #39ff88;
    }

    Button { height: 32px; padding-left: 12px; padding-right: 12px; }
    """
)

win = dg.Window("LimitsBar telemetry probe", width=920, height=820)

with dg.VLayout(class_="root"):
    dg.Label("LimitsBar telemetry probe", class_="title")
    dg.Label(
        "Red/yellow/green zones use four ordered thresholds. Values beyond the domain "
        "peg the marker at an end without changing the telemetry value.",
        class_="caption",
    )

    with dg.Panel("CSS style gallery", class_="case"):
        dg.Label(
            "Each row is the same LimitsBar widget and limits; only its CSS class changes.",
            class_="caption",
        )
        with dg.VLayout(class_="stack"):
            for name, class_name, value in (
                ("Neon telemetry", "neon", 18),
                ("Soft capsule", "pastel", 42),
                ("Industrial square", "industrial", 72),
                ("Minimal compact", "minimal", 91),
                ("High contrast", "high-contrast", 8),
                ("Monochrome", "monochrome", 34),
                ("Ice glass", "ice-glass", 64),
                ("Retro terminal", "terminal", 86),
            ):
                with dg.HLayout(class_="row"):
                    dg.Label(f"{name}  ·  {value}", class_="style-name")
                    dg.LimitsBar(value, min=0, max=100, class_=class_name)

    with dg.Panel("Five states and end pegging", class_="case"):
        with dg.VLayout(class_="stack"):
            for label, value in (
                ("Below range  -15", -15),
                ("Red low        5", 5),
                ("Yellow low    18", 18),
                ("Green         50", 50),
                ("Yellow high   82", 82),
                ("Red high      95", 95),
                ("Above range  120", 120),
            ):
                with dg.HLayout(class_="row"):
                    dg.Label(label, class_="readout")
                    dg.LimitsBar(value, min=0, max=100)

    with dg.Panel("Custom limits and live updates", class_="case"):
        dg.Label(
            "Domain -40..120; limits -20 / 0 / 80 / 100. Use the buttons while "
            "the probe is running.",
            class_="caption",
        )
        live = dg.LimitsBar(
            40,
            min=-40,
            red_low=-20,
            yellow_low=0,
            yellow_high=80,
            red_high=100,
            max=120,
            class_="alternate",
        )
        readout = dg.Label("value 40 | state green", class_="caption")

        def set_live(value: float) -> None:
            live.set_value(value)
            readout.set_value(f"value {live.value:g} | state {live.limits_state}")

        def move(delta: float) -> None:
            set_live(live.value + delta)

        with dg.HLayout(class_="row"):
            dg.Button("-20", on_click=lambda: move(-20))
            dg.Button("+20", on_click=lambda: move(20))
            dg.Button("Peg low", on_click=lambda: set_live(-200))
            dg.Button("Peg high", on_click=lambda: set_live(200))
            dg.LimitsBar(40, min=-40, max=120, disabled=True)

if __name__ == "__main__":
    result = app.run(win)
    print({key: result.get(key) for key in ("status", "renderer", "frame_ms")})
