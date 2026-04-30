from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8))
app.stylesheet(
    """
    :root {
        --surface: rgba(18, 25, 39, 0.94);
        --border: rgba(255, 255, 255, 0.13);
        --muted: rgba(245, 248, 255, 0.74);
        --text: rgba(245, 248, 255, 0.96);
        --blue: #5aa9ff;
        --green: #74ddb0;
        --yellow: #ffd36a;
        --pink: #ff6584;
    }

    Window {
        background: #0d1320;
        color: var(--text);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    Panel {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: 12px;
        padding: 14px;
        gap: 8px;
    }

    Panel.swatch {
        width: 330px;
        height: 136px;
        box-shadow: 0 14px 34px rgba(0, 0, 0, 0.26);
    }

    Panel.compare {
        width: 220px;
        height: 126px;
        box-shadow: 0 12px 28px rgba(0, 0, 0, 0.24);
    }

    Panel.hero-swatch {
        width: 690px;
        height: 180px;
        padding: 18px;
        border-radius: 16px;
        box-shadow:
            0 2px 10px rgba(0, 0, 0, 0.20),
            0 20px 54px rgba(0, 0, 0, 0.34);
    }

    Panel.blob-swatch {
        width: 690px;
        height: 190px;
        padding: 18px;
        border-radius: 16px;
        box-shadow:
            0 2px 10px rgba(0, 0, 0, 0.20),
            0 20px 54px rgba(0, 0, 0, 0.34);
    }

    Panel.image-like-swatch {
        width: 690px;
        height: 190px;
        padding: 18px;
        border-radius: 16px;
        box-shadow:
            0 2px 10px rgba(0, 0, 0, 0.20),
            0 20px 54px rgba(0, 0, 0, 0.34);
    }

    Label.title {
        font-size: 20px;
        font-weight: 800;
        color: var(--blue);
    }

    Label.caption {
        color: var(--muted);
        line-height: 1.12;
    }

    Label.card-title {
        font-weight: 900;
        color: white;
        text-shadow: 0 1px 8px rgba(0, 0, 0, 0.32);
    }

    Label.card-copy {
        color: rgba(255, 255, 255, 0.78);
        line-height: 1.08;
    }

    Label.hero-title {
        font-size: 22px;
        font-weight: 900;
        color: rgba(255, 255, 255, 0.96);
        text-shadow: 0 1px 12px rgba(0, 0, 0, 0.38);
    }

    Label.hero-copy {
        color: rgba(245, 248, 255, 0.76);
        line-height: 1.10;
        width: 500px;
    }

    Panel.linear {
        background:
            linear-gradient(135deg,
                rgba(90, 169, 255, 0.52) 0%,
                rgba(90, 169, 255, 0.42) 18%,
                rgba(116, 221, 176, 0.30) 42%,
                rgba(255, 211, 106, 0.18) 66%,
                rgba(255, 101, 132, 0.16) 82%,
                rgba(12, 17, 28, 0.18) 100%);
        background-noise: 0.006;
    }

    Panel.radial {
        background:
            radial-gradient(circle at 24% 22%,
                rgba(255, 255, 255, 0.24) 0%,
                rgba(255, 255, 255, 0.07) 36%,
                transparent 66%),
            #172235;
        background-noise: 0.012;
    }

    Panel.layered {
        background:
            radial-gradient(circle at 18% 18%, rgba(255, 211, 106, 0.28) 0%, transparent 54%),
            radial-gradient(circle at 82% 72%, rgba(90, 169, 255, 0.22) 0%, transparent 62%),
            linear-gradient(145deg, rgba(35, 49, 73, 0.96), rgba(8, 13, 24, 0.96));
        background-noise: 0.008;
    }

    Panel.repeating {
        background:
            repeating-linear-gradient(135deg,
                rgba(255, 255, 255, 0.13) 0%,
                rgba(255, 255, 255, 0.13) 6%,
                transparent 6%,
                transparent 13%),
            linear-gradient(135deg, rgba(90, 169, 255, 0.38), rgba(116, 221, 176, 0.18));
    }

    Panel.image-gradient {
        background-color: rgba(18, 25, 39, 0.98);
        background-image:
            radial-gradient(circle at 80% 22%, rgba(116, 221, 176, 0.22), transparent 55%),
            linear-gradient(180deg, rgba(90, 169, 255, 0.24), transparent);
        background-noise: 0.007;
    }

    Panel.image-none {
        background-color: rgba(255, 211, 106, 0.16);
        background-image: none;
        border-color: rgba(255, 211, 106, 0.42);
    }

    Panel.modern {
        gradient-interpolation: oklab;
        background:
            radial-gradient(circle at 18% 18%,
                rgba(255, 255, 255, 0.16) 0%,
                rgba(255, 255, 255, 0.09) 14%,
                rgba(255, 255, 255, 0.045) 30%,
                rgba(255, 255, 255, 0.018) 46%,
                rgba(255, 255, 255, 0.006) 58%,
                transparent 72%),
            radial-gradient(circle at 78% 26%,
                rgba(90, 169, 255, 0.30) 0%,
                rgba(90, 169, 255, 0.22) 18%,
                rgba(90, 169, 255, 0.12) 36%,
                rgba(90, 169, 255, 0.045) 54%,
                rgba(90, 169, 255, 0.012) 68%,
                transparent 82%),
            radial-gradient(circle at 62% 88%,
                rgba(116, 221, 176, 0.22) 0%,
                rgba(116, 221, 176, 0.16) 18%,
                rgba(116, 221, 176, 0.075) 38%,
                rgba(116, 221, 176, 0.028) 56%,
                rgba(116, 221, 176, 0.010) 70%,
                transparent 86%),
            linear-gradient(145deg,
                rgba(30, 43, 69, 0.98) 0%,
                rgba(24, 35, 58, 0.98) 24%,
                rgba(17, 25, 42, 0.98) 46%,
                rgba(13, 19, 32, 0.98) 64%,
                rgba(10, 15, 27, 0.98) 82%,
                rgba(7, 11, 21, 0.98) 100%);
        background-noise: 0.006;
        border-color: rgba(255, 255, 255, 0.16);
    }

    Panel.compare-srgb,
    Panel.compare-linear,
    Panel.compare-oklab {
        background:
            radial-gradient(circle at 18% 18%,
                rgba(255, 255, 255, 0.13) 0%,
                rgba(255, 255, 255, 0.035) 42%,
                transparent 72%),
            linear-gradient(135deg,
                rgba(90, 169, 255, 0.78) 0%,
                rgba(255, 101, 132, 0.52) 48%,
                rgba(116, 221, 176, 0.48) 100%);
        background-noise: 0.004;
    }

    Panel.compare-srgb {
        gradient-interpolation: srgb;
    }

    Panel.compare-linear {
        gradient-interpolation: linear-srgb;
    }

    Panel.compare-oklab {
        gradient-interpolation: oklab;
    }

    Panel.organic-blob {
        gradient-interpolation: oklab;
        background:
            blob-gradient(
                at 22% 30% rgba(90, 169, 255, 0.68) 46%,
                at 76% 24% rgba(255, 101, 132, 0.54) 40%,
                at 58% 76% rgba(116, 221, 176, 0.50) 48%,
                at 34% 64% rgba(255, 211, 106, 0.34) 34%
            ),
            linear-gradient(145deg,
                rgba(28, 39, 64, 0.98) 0%,
                rgba(13, 19, 32, 0.98) 58%,
                rgba(7, 11, 21, 0.98) 100%);
        background-noise: 0.004;
        border-color: rgba(255, 255, 255, 0.16);
    }

    Panel.image-like-mesh {
        gradient-interpolation: oklab;
        background:
            mesh-gradient(
                rgb(35, 55, 170),
                rgb(195, 74, 138),
                rgb(54, 180, 148),
                rgb(12, 18, 32)
            );
        background-noise: 0.002;
        border-color: rgba(255, 255, 255, 0.16);
    }
    """
)


win = dg.Window("CSS Gradient Probe", width=760, height=980)

with dg.VLayout(style={"gap": 12}):
    dg.Label("Gradients and background layers", class_="title")
    dg.Label(
        "Each card isolates a paint path: linear, radial, layered, repeating, "
        "background-image, and background-image: none over a solid fallback.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="swatch linear"):
            dg.Label("linear-gradient", class_="card-title")
            dg.Label("Three stops with subtle background-noise.", class_="card-copy")

        with dg.Panel(class_="swatch radial"):
            dg.Label("radial-gradient", class_="card-title")
            dg.Label("Circle at an offset center over a solid base.", class_="card-copy")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="swatch layered"):
            dg.Label("layered background", class_="card-title")
            dg.Label("Two radial glows over a linear base.", class_="card-copy")

        with dg.Panel(class_="swatch repeating"):
            dg.Label("repeating gradient", class_="card-title")
            dg.Label("Repeating stripe layer over a soft gradient.", class_="card-copy")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="swatch image-gradient"):
            dg.Label("background-image", class_="card-title")
            dg.Label("Gradient image layers over background-color.", class_="card-copy")

        with dg.Panel(class_="swatch image-none"):
            dg.Label("background-image: none", class_="card-title")
            dg.Label("Should show only the warm solid color fallback.", class_="card-copy")

    with dg.Panel(class_="hero-swatch modern"):
        dg.Label("modern layered recipe", class_="hero-title")
        dg.Label(
            "This is the best-case CSS-only version: several low-alpha radial "
            "lights over a restrained dark base, Oklab interpolation, plus subtle noise.",
            class_="hero-copy",
        )

    with dg.Panel(class_="blob-swatch organic-blob"):
        dg.Label("organic blob-gradient()", class_="hero-title")
        dg.Label(
            "DragonGUI-specific blob paint merges colored soft fields in the shader, "
            "so the result is less rigid than stacked circular radial gradients.",
            class_="hero-copy",
        )

    with dg.Panel(class_="image-like-swatch image-like-mesh"):
        dg.Label("image-like mesh-gradient()", class_="hero-title")
        dg.Label(
            "This is closer to the Image widget reference: a smooth two-axis color field "
            "rather than geometric circles or thresholded blob shapes.",
            class_="hero-copy",
        )

    dg.Label("Interpolation comparison", class_="title")
    dg.Label(
        "These three cards use the same gradient stops. Only gradient-interpolation changes.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel(class_="compare compare-srgb"):
            dg.Label("srgb", class_="card-title")
            dg.Label("Current/default blending.", class_="card-copy")

        with dg.Panel(class_="compare compare-linear"):
            dg.Label("linear-srgb", class_="card-title")
            dg.Label("Linear-light blending.", class_="card-copy")

        with dg.Panel(class_="compare compare-oklab"):
            dg.Label("oklab", class_="card-title")
            dg.Label("Perceptual blend target.", class_="card-copy")


if __name__ == "__main__":
    print(app.run(win))
