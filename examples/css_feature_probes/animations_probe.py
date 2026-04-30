from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8))
app.stylesheet(
    """
    @keyframes status-pulse {
        from {
            opacity: 0.58;
            scale: 0.96;
            background: rgba(116, 221, 176, 0.14);
            border-color: rgba(116, 221, 176, 0.34);
            outline-color: rgba(116, 221, 176, 0.00);
            outline-offset: 1px;
        }

        50% {
            opacity: 1;
            scale: 1.06;
            background: rgba(116, 221, 176, 0.34);
            border-color: rgba(116, 221, 176, 0.76);
            outline-color: rgba(116, 221, 176, 0.40);
            outline-offset: 4px;
        }

        to {
            opacity: 0.76;
            scale: 1;
            background: rgba(90, 169, 255, 0.24);
            border-color: rgba(90, 169, 255, 0.62);
            outline-color: rgba(90, 169, 255, 0.20);
            outline-offset: 2px;
        }
    }

    @keyframes card-drift {
        from {
            translate: -4px 0;
            opacity: 0.72;
            background: rgba(90, 169, 255, 0.08);
            border-color: rgba(90, 169, 255, 0.24);
        }

        to {
            translate: 4px 0;
            opacity: 1;
            background: rgba(90, 169, 255, 0.18);
            border-color: rgba(90, 169, 255, 0.58);
        }
    }

    @keyframes finite-fill {
        from {
            opacity: 0.54;
            translate: 0 4px;
            scale: 0.96;
            background: rgba(255, 211, 106, 0.10);
            border-color: rgba(255, 211, 106, 0.30);
        }

        to {
            opacity: 1;
            translate: 0 -2px;
            scale: 1.03;
            background: rgba(255, 211, 106, 0.26);
            border-color: rgba(255, 211, 106, 0.74);
        }
    }

    @keyframes rotate-surface {
        from {
            rotate: -2deg;
            scale: 0.98;
            background: rgba(255, 255, 255, 0.06);
        }

        to {
            rotate: 2deg;
            scale: 1.02;
            background: rgba(255, 101, 132, 0.18);
        }
    }

    Window {
        background: #0d1320;
        color: rgba(245, 248, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.scroll-root {
        height: 620px;
        overflow-y: auto;
        padding-right: 18px;
        padding-bottom: 18px;
        gap: 12px;
    }

    VLayout.scroll-root::scrollbar-track {
        width: 8px;
        padding: 8px;
        background: rgba(255, 255, 255, 0.09);
        border-radius: 999px;
    }

    VLayout.scroll-root::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.72);
        border-radius: 999px;
    }

    Panel {
        background: rgba(18, 25, 39, 0.94);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 14px;
        box-shadow: 0 12px 30px rgba(0, 0, 0, 0.26);
        padding: 12px;
        gap: 8px;
    }

    Panel.case {
        width: 356px;
        min-height: 156px;
        overflow: visible;
    }

    Label.title {
        color: #5aa9ff;
        font-size: 20px;
        font-weight: 800;
    }

    Label.caption {
        color: rgba(245, 248, 255, 0.72);
        line-height: 1.16;
    }

    Label.case-title {
        color: rgba(245, 248, 255, 0.95);
        font-weight: 800;
    }

    Label.pass {
        color: #74ddb0;
        font-weight: 800;
    }

    Badge,
    Button,
    Panel.motion {
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 999px;
    }

    Badge {
        background: rgba(255, 255, 255, 0.07);
        color: white;
        padding: 5px 12px;
        outline: 1px solid transparent;
        outline-offset: 1px;
    }

    Button {
        height: 34px;
        background: rgba(255, 255, 255, 0.06);
        border-color: rgba(255, 255, 255, 0.16);
        border-radius: 10px;
        color: white;
        font-weight: 800;
    }

    Badge.live {
        animation: status-pulse 1400ms cubic-bezier(0.16, 1, 0.3, 1) infinite alternate both running;
    }

    Badge.steps {
        animation: status-pulse 1400ms steps(4, end) infinite alternate both running;
    }

    Badge.paused {
        animation: status-pulse 1400ms step-start infinite alternate both paused;
    }

    Badge.negative-delay {
        animation: status-pulse 1600ms -800ms ease-out infinite alternate both running;
    }

    Badge.fractional-count {
        animation: status-pulse 1200ms -300ms ease-out 2.5 alternate both running;
    }

    Button.forwards {
        animation-name: finite-fill;
        animation-duration: 900ms;
        animation-timing-function: ease-out;
        animation-iteration-count: 1;
        animation-direction: normal;
        animation-fill-mode: forwards;
        animation-play-state: running;
    }

    Button.backwards-delay {
        animation: finite-fill 1400ms ease-in-out 900ms 1 normal backwards running;
    }

    Panel.motion {
        border-radius: 14px;
        animation: card-drift 1600ms ease-in-out infinite alternate both running;
    }

    Panel.paused-motion {
        border-radius: 14px;
        animation: card-drift 1600ms ease-in-out infinite alternate both paused;
    }

    Button.rotate {
        animation: rotate-surface 1200ms ease-in-out infinite alternate both running;
    }
    """
)


win = dg.Window("CSS Animations Probe", width=820, height=660)

with dg.VLayout(class_="scroll-root"):
    dg.Label("Animations and keyframes", class_="title")
    dg.Label(
        "Animated widgets should update continuously. Paused and delayed cases "
        "should hold still according to fill mode and play state.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Running animations", class_="case"):
            dg.Label("Infinite and stepped pulses", class_="case-title")
            with dg.HLayout(style={"gap": 8}):
                dg.Badge("smooth pulse", class_="live")
                dg.Badge("steps pulse", class_="steps")
            dg.Label("PASS: both pulse; stepped one jumps between states.", class_="pass")

        with dg.Panel("Paused animation", class_="case"):
            dg.Label("animation-play-state: paused", class_="case-title")
            dg.Badge("paused", class_="paused")
            dg.Label("PASS: badge should hold a fixed keyframe and not pulse.", class_="pass")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Negative delay and fractional count", class_="case"):
            dg.Label("Starts halfway through", class_="case-title")
            with dg.HLayout(style={"gap": 8}):
                dg.Badge("negative delay loop", class_="negative-delay")
                dg.Badge("2.5 count", class_="fractional-count")
            dg.Label(
                "PASS: left keeps pulsing from mid-cycle; right starts mid-cycle then stops.",
                class_="caption",
            )

        with dg.Panel("Fill modes", class_="case"):
            dg.Label("forwards and backwards", class_="case-title")
            dg.Button("Forwards fill", class_="forwards")
            dg.Button("Backwards delay", class_="backwards-delay")
            dg.Label("PASS: first ends styled; second holds first keyframe before delay.", class_="caption")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Panel motion", class_="case motion"):
            dg.Label("Animated panel", class_="case-title")
            dg.Label("This whole panel should drift left and right.")
            dg.Label("PASS: border, background, and text move together.", class_="pass")

        with dg.Panel("Paused panel motion", class_="case paused-motion"):
            dg.Label("Paused panel", class_="case-title")
            dg.Label("This panel should stay fixed.")
            dg.Label("PASS: no movement here.", class_="pass")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Rotation", class_="case"):
            dg.Label("Surface rotation", class_="case-title")
            dg.Button("Rotate surface", class_="rotate")
            dg.Label("PASS: surface should rotate; text may not rotate in first slice.", class_="caption")

        with dg.Panel("No animation baseline", class_="case"):
            dg.Label("Static comparison", class_="case-title")
            dg.Badge("static badge")
            dg.Button("static button")
            dg.Label("PASS: this card should not animate.", class_="pass")


if __name__ == "__main__":
    print(app.run(win))
