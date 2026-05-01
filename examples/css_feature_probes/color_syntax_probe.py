from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="oklch(72% 0.14 245)", radius=8))
app.stylesheet(
    """
    Window {
        background: #0d1320;
        color: rgba(245, 248, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.scroll-root {
        width: 100%;
        height: 100%;
        min-height: 560px;
        overflow-y: auto;
        padding-right: 18px;
        padding-bottom: 84px;
        gap: 12px;
    }

    VLayout.scroll-root::scrollbar-track {
        width: 8px;
        padding: 8px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.scroll-root::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.72);
        border-radius: 999px;
    }

    Panel {
        background: rgba(18, 25, 39, 0.92);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 16px;
        box-shadow: 0 12px 30px rgba(0, 0, 0, 0.24);
        padding: 12px;
        gap: 8px;
    }

    Panel.case {
        width: 360px;
        min-height: 232px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 800;
    }

    Label.caption {
        color: rgba(245, 248, 255, 0.74);
        line-height: 1.12;
    }

    Label.case-title {
        color: rgba(245, 248, 255, 0.96);
        font-weight: 800;
    }

    Label.swatch-label {
        font-size: 12px;
        color: rgba(245, 248, 255, 0.82);
        text-overflow: ellipsis;
    }

    Panel.swatch {
        width: 150px;
        height: 44px;
        padding: 8px;
        border-radius: 10px;
        border: 1px solid rgba(255, 255, 255, 0.18);
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.12);
    }

    Panel.named-white {
        background: white;
        color: black;
    }

    Panel.named-gray {
        background: gray;
    }

    Panel.named-red {
        background: red;
    }

    Panel.transparent-swatch {
        background:
            linear-gradient(45deg, rgba(255,255,255,0.18) 25%, transparent 25%, transparent 75%, rgba(255,255,255,0.18) 75%),
            linear-gradient(45deg, rgba(255,255,255,0.18) 25%, transparent 25%, transparent 75%, rgba(255,255,255,0.18) 75%),
            transparent;
        background-size: 18px 18px;
        background-position: 0 0, 9px 9px;
    }

    Panel.hex-alpha {
        background: #5aa9ff88;
    }

    Panel.rgb {
        background: rgb(90 169 255);
    }

    Panel.rgba {
        background: rgba(116, 221, 176, 0.68);
    }

    Panel.hsl {
        background: hsl(42 95% 63%);
    }

    Panel.hsla {
        background: hsla(345, 100%, 70%, 0.72);
    }

    Panel.hwb {
        background: hwb(205 18% 0% / 72%);
    }

    Panel.lab {
        background: lab(72% -12 -34 / 82%);
    }

    Panel.lch {
        background: lch(70% 58 290deg / 82%);
    }

    Panel.oklab {
        background: oklab(72% -0.06 -0.12 / 86%);
    }

    Panel.oklch {
        background: oklch(72% 0.16 245deg / 86%);
    }

    Panel.srgb {
        background: color(srgb 0.35 0.66 1 / 0.86);
    }

    Panel.srgb-linear {
        background: color(srgb-linear 0.18 0.40 1 / 0.86);
    }

    Panel.foreground-demo {
        width: calc(100% - 24px);
        min-height: 128px;
        background: rgba(255, 255, 255, 0.055);
        border-color: rgba(255, 255, 255, 0.16);
    }

    Label.foreground-rgb {
        color: rgb(116 221 176);
        font-weight: 800;
    }

    Label.foreground-oklch {
        color: oklch(78% 0.14 245);
        font-weight: 800;
    }

    Button.transparent-button {
        background: transparent;
        border: 1px solid color(srgb 0.35 0.66 1 / 0.52);
        color: hwb(205 82% 0%);
    }
    """
)


win = dg.Window("CSS Color Syntax Probe", width=850, height=700)


def swatch(class_name: str, label: str) -> None:
    with dg.Panel(class_=f"swatch {class_name}"):
        dg.Label(label, class_="swatch-label", wrap=False)


with dg.VLayout(class_="scroll-root"):
    dg.Label("Color syntax", class_="title")
    dg.Label(
        "This probe isolates supported color formats. Swatches should render as distinct colors; "
        "transparent and alpha samples should show through to the panel behind them.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Named and alpha colors", class_="case"):
            dg.Label("Named, transparent, hex alpha", class_="case-title")
            with dg.HLayout(style={"gap": 8}):
                swatch("named-white", "white")
                swatch("named-gray", "gray")
            with dg.HLayout(style={"gap": 8}):
                swatch("named-red", "red")
                swatch("transparent-swatch", "transparent")
            with dg.HLayout(style={"gap": 8}):
                swatch("hex-alpha", "#5aa9ff88")

        with dg.Panel("RGB family", class_="case"):
            dg.Label("rgb(), rgba(), hsl(), hwb()", class_="case-title")
            with dg.HLayout(style={"gap": 8}):
                swatch("rgb", "rgb()")
                swatch("rgba", "rgba()")
            with dg.HLayout(style={"gap": 8}):
                swatch("hsl", "hsl()")
                swatch("hsla", "hsla()")
            with dg.HLayout(style={"gap": 8}):
                swatch("hwb", "hwb()")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Perceptual color", class_="case"):
            dg.Label("lab(), lch(), oklab(), oklch()", class_="case-title")
            with dg.HLayout(style={"gap": 8}):
                swatch("lab", "lab()")
                swatch("lch", "lch()")
            with dg.HLayout(style={"gap": 8}):
                swatch("oklab", "oklab()")
                swatch("oklch", "oklch()")

        with dg.Panel("CSS color()", class_="case"):
            dg.Label("color(srgb) and color(srgb-linear)", class_="case-title")
            with dg.HLayout(style={"gap": 8}):
                swatch("srgb", "color(srgb)")
                swatch("srgb-linear", "srgb-linear")
            dg.Label(
                "These should both resolve to visible blue-family colors with alpha.",
                class_="caption",
            )

    with dg.Panel("Foreground and borders", class_="foreground-demo"):
        dg.Label("Color values outside backgrounds", class_="case-title")
        dg.Label("rgb() foreground text", class_="foreground-rgb")
        dg.Label("oklch() foreground text", class_="foreground-oklch")
        dg.Button("transparent button with color(srgb) border", class_="transparent-button")


if __name__ == "__main__":
    print(app.run(win))
