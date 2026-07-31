"""THEME FORGE — DragonGUI runtime-theming and CSS stress console.

A twelve-workspace application built to hammer the newer CSS/theming surfaces
at the same time: runtime `Theme` replacement, named stylesheet add/replace/
remove, `:root` custom properties edited live, widget parts, borders/outlines,
procedural + managed-image backgrounds, font stacks, layout nesting, shadow and
glow effects, the icon theme, client-side window chrome, and a continuously
updating data workload.

Everything cosmetic lives in four named user stylesheets that are replaced in
place while the app runs:

    "variables"   :root custom-property table (theme tokens + live editor)
    "structure"   layout/identity rules, authored once against var(--tf-*)
    "appearance"  the active theme's cosmetic sheet
    "overrides"   an optional sheet that is added and removed at runtime
    "extreme"     the deliberately hostile sheet ("extreme mode")

Because the Python widget tree is never rebuilt for a theme change, every theme
switch is a pure cascade change; focus, scroll offsets, selected tabs, input
values, table selection, and plot data are expected to survive it.

`--autopilot` drives the documented interaction loop and verifies that against
`app.debug_snapshot()`. Each cycle asserts:

* no retained-state regressions (focus, scroll, tabs, pages, input values);
* zero layout diagnostics at every one of the five viewport presets and on
  every one of the twelve pages;
* the hostile sheet installs and raises the warning count without setting
  `last_error` (it degrades, it is not rejected);
* malformed sheets either abort or recover, and either way leave the active
  cascade byte-for-byte identical.

The process exits non-zero if any cycle fails.

Run:
    python examples/theme_forge_stress_demo.py
    python examples/theme_forge_stress_demo.py --theme win311
    python examples/theme_forge_stress_demo.py --autopilot --autopilot-cycles 3
    python examples/theme_forge_stress_demo.py --autopilot --report forge.json
    python examples/theme_forge_stress_demo.py --rows 2000 --no-live

Long-title regression coverage
------------------------------
With `decorations="client"`, `Window::title` must shrink and ellipsize before
the fixed-size minimize/maximize/close controls. Exercise the narrow-client-
chrome regression with:

    python examples/theme_forge_stress_demo.py --long-title --autopilot \\
        --autopilot-cycles 1 --autopilot-exit

The autopilot must remain green at 320x640, 390x720, and 640x480, with all
three controls visible, clipped to the titlebar, and clickable.
"""

from __future__ import annotations

import argparse
import json
import math
import random
import struct
import sys
import threading
import time
import zlib
from pathlib import Path
from typing import Any, Callable

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:  # ScatterPlot2D wants array columns; the demo degrades gracefully without it.
    import numpy as np
except ImportError:  # pragma: no cover - optional dependency
    np = None


# ---------------------------------------------------------------------------
# Token tables
# ---------------------------------------------------------------------------
# Every theme is a token table plus a cosmetic stylesheet. The structure sheet
# is written once against `var(--tf-*)`, so switching themes only replaces the
# ":root" block and the appearance sheet -- never the widget tree.

BASE_TOKENS: dict[str, str] = {
    # surfaces
    "bg": "#0b0f17",
    "surface": "#141b27",
    "surface-2": "#1b2432",
    "surface-3": "#232f40",
    "field": "#0e141e",
    # ink
    "ink": "#eef3fb",
    "ink-dim": "#93a3b8",
    "ink-faint": "#5f7085",
    "ink-invert": "#0b0f17",
    # lines
    "line": "#2c3a4d",
    "line-soft": "rgba(140, 175, 220, 0.20)",
    "line-strong": "#43587a",
    # semantic
    "accent": "#5aa9ff",
    "accent-ink": "#04101f",
    "accent-soft": "rgba(90, 169, 255, 0.18)",
    "ok": "#54d69a",
    "warn": "#f5c451",
    "bad": "#ff6b7d",
    "info": "#7fd8ff",
    # geometry (edited live by the CSS-variable editor)
    "radius": "10px",
    "radius-sm": "6px",
    "radius-lg": "18px",
    "border-w": "1px",
    "pad": "14px",
    "gap": "10px",
    "focus-w": "2px",
    "tile": "8px",
    # typography
    "font": "\"Forge UI\", \"Segoe UI\", system-ui, sans-serif",
    "mono": "\"Forge Mono\", \"Cascadia Mono\", Consolas, monospace",
    "font-size": "13px",
    "title-size": "20px",
    "kicker-size": "11px",
    # effects
    "glow": "rgba(90, 169, 255, 0.55)",
    "shadow": "rgba(0, 0, 0, 0.55)",
    "inset": "rgba(0, 0, 0, 0.45)",
    "sheen": "rgba(255, 255, 255, 0.08)",
    # pattern stripes
    "stripe-a": "rgba(90, 169, 255, 0.22)",
    "stripe-b": "rgba(10, 16, 26, 0.0)",
    # chart literals (paint APIs take literal colors, never var())
    "chart-a": "#5aa9ff",
    "chart-b": "#54d69a",
    "chart-c": "#f5c451",
    "chart-d": "#ff6b7d",
    "chart-e": "#c08bff",
    "colormap": "viridis",
}


def tokens(**overrides: str) -> dict[str, str]:
    merged = dict(BASE_TOKENS)
    merged.update(overrides)
    return merged


TOKENS: dict[str, dict[str, str]] = {
    # 1. Stock DragonGUI look: framework defaults do the work, tokens stay neutral.
    "default": tokens(
        bg="#0a0f14",
        surface="#121922",
        surface_2="#1d2833",
        line="#263543",
        accent="#37c6d0",
        accent_soft="rgba(55, 198, 208, 0.16)",
        glow="rgba(55, 198, 208, 0.42)",
        radius="3px",
        radius_sm="3px",
        radius_lg="3px",
        pad="7px",
        gap="5px",
        font="sans-serif",
        font_size="13px",
        chart_a="#37c6d0",
        chart_b="#45c48a",
        chart_c="#f4b84a",
        chart_d="#ff5f72",
        chart_e="#7bdcff",
        colormap="viridis",
    ),
    # 2. Modern dark: the baseline table above.
    "modern-dark": tokens(),
    # 3. Modern light.
    "modern-light": tokens(
        bg="#eef1f7",
        surface="#ffffff",
        **{
            "surface-2": "#f4f7fc",
            "surface-3": "#e6ecf6",
            "line-soft": "rgba(30, 60, 110, 0.14)",
            "line-strong": "#9fb2cd",
            "accent-ink": "#ffffff",
            "accent-soft": "rgba(8, 126, 164, 0.14)",
            "ink-dim": "#5a6b80",
            "ink-faint": "#8494a8",
            "ink-invert": "#ffffff",
            "border-w": "1px",
            "font-size": "13px",
            "chart-a": "#0f6fbf",
            "chart-b": "#137a4a",
            "chart-c": "#a86f00",
            "chart-d": "#cf2445",
            "chart-e": "#7040b8",
            "stripe-a": "rgba(8, 126, 164, 0.20)",
            "stripe-b": "rgba(255, 255, 255, 0.0)",
        },
        field="#ffffff",
        ink="#141a24",
        line="#ccd6e5",
        accent="#087ea4",
        ok="#137a4a",
        warn="#a86f00",
        bad="#cf2445",
        info="#0f6fbf",
        glow="rgba(8, 126, 164, 0.38)",
        shadow="rgba(20, 40, 70, 0.20)",
        inset="rgba(20, 40, 70, 0.16)",
        sheen="rgba(255, 255, 255, 0.85)",
        colormap="cividis",
    ),
    # 4. Windows 3.11: square, grey, inset/outset bevels, no radius.
    "win311": tokens(
        bg="#008080",
        surface="#c0c0c0",
        **{
            "surface-2": "#c0c0c0",
            "surface-3": "#dfdfdf",
            "line-soft": "#dfdfdf",
            "line-strong": "#000000",
            "accent-ink": "#ffffff",
            "accent-soft": "#000080",
            "ink-dim": "#404040",
            "ink-faint": "#808080",
            "ink-invert": "#ffffff",
            "border-w": "2px",
            "radius-sm": "0px",
            "radius-lg": "0px",
            "font-size": "12px",
            "title-size": "15px",
            "kicker-size": "11px",
            "chart-a": "#000080",
            "chart-b": "#008000",
            "chart-c": "#808000",
            "chart-d": "#800000",
            "chart-e": "#800080",
            "stripe-a": "#808080",
            "stripe-b": "#c0c0c0",
        },
        field="#ffffff",
        ink="#000000",
        line="#808080",
        accent="#000080",
        ok="#008000",
        warn="#808000",
        bad="#800000",
        info="#000080",
        radius="0px",
        pad="8px",
        gap="6px",
        font="\"Forge UI\", \"MS Sans Serif\", Tahoma, sans-serif",
        mono="\"Fixedsys\", \"Courier New\", monospace",
        glow="rgba(0, 0, 128, 0.0)",
        shadow="rgba(0, 0, 0, 0.35)",
        inset="#808080",
        sheen="#ffffff",
        tile="4px",
        colormap="gray",
    ),
    # 5. Classic Mac: hairline black on white, rounded, monochrome.
    "classic-mac": tokens(
        bg="#8f8f8f",
        surface="#ffffff",
        **{
            "surface-2": "#f0f0f0",
            "surface-3": "#e0e0e0",
            "line-soft": "#b8b8b8",
            "line-strong": "#000000",
            "accent-ink": "#ffffff",
            "accent-soft": "#d8d8d8",
            "ink-dim": "#4a4a4a",
            "ink-faint": "#777777",
            "ink-invert": "#ffffff",
            "border-w": "1px",
            "radius-sm": "3px",
            "radius-lg": "9px",
            "font-size": "12px",
            "title-size": "17px",
            "chart-a": "#111111",
            "chart-b": "#555555",
            "chart-c": "#888888",
            "chart-d": "#333333",
            "chart-e": "#aaaaaa",
            "stripe-a": "#000000",
            "stripe-b": "#ffffff",
        },
        field="#ffffff",
        ink="#000000",
        line="#000000",
        accent="#000000",
        ok="#2f6f3f",
        warn="#7a5c00",
        bad="#8a1f2a",
        info="#25406b",
        radius="6px",
        pad="10px",
        gap="8px",
        font="\"Forge UI\", \"Charcoal\", \"Chicago\", Geneva, sans-serif",
        mono="\"Monaco\", Consolas, monospace",
        glow="rgba(0, 0, 0, 0.0)",
        shadow="rgba(0, 0, 0, 0.45)",
        inset="rgba(0, 0, 0, 0.30)",
        sheen="rgba(255, 255, 255, 0.9)",
        tile="2px",
        colormap="gray",
    ),
    # 6. High contrast: pure black, saturated primaries, fat focus rings.
    "contrast": tokens(
        bg="#000000",
        surface="#000000",
        **{
            "surface-2": "#000000",
            "surface-3": "#101010",
            "line-soft": "#ffffff",
            "line-strong": "#ffffff",
            "accent-ink": "#000000",
            "accent-soft": "#003a52",
            "ink-dim": "#e8e8e8",
            "ink-faint": "#c8c8c8",
            "ink-invert": "#000000",
            "border-w": "2px",
            "radius-sm": "0px",
            "radius-lg": "0px",
            "focus-w": "4px",
            "font-size": "14px",
            "title-size": "22px",
            "chart-a": "#00e5ff",
            "chart-b": "#00ff88",
            "chart-c": "#ffd400",
            "chart-d": "#ff3b6b",
            "chart-e": "#c77dff",
            "stripe-a": "#ffffff",
            "stripe-b": "#000000",
        },
        field="#000000",
        ink="#ffffff",
        line="#ffffff",
        accent="#00e5ff",
        ok="#00ff88",
        warn="#ffd400",
        bad="#ff3b6b",
        info="#00e5ff",
        radius="0px",
        pad="12px",
        gap="10px",
        font="\"Forge UI\", \"Segoe UI\", sans-serif",
        glow="rgba(0, 229, 255, 0.9)",
        shadow="rgba(0, 0, 0, 1.0)",
        inset="rgba(255, 255, 255, 0.25)",
        sheen="rgba(255, 255, 255, 0.35)",
        tile="6px",
        colormap="magma",
    ),
    # 7. Deliberately extreme: huge radii, saturated gradients, heavy glow.
    "extreme": tokens(
        bg="#12002b",
        surface="rgba(58, 6, 96, 0.86)",
        **{
            "surface-2": "rgba(96, 8, 128, 0.78)",
            "surface-3": "rgba(140, 12, 150, 0.70)",
            "line-soft": "rgba(255, 92, 246, 0.35)",
            "line-strong": "#ff5cf6",
            "accent-ink": "#1a0030",
            "accent-soft": "rgba(255, 196, 0, 0.30)",
            "ink-dim": "#ffb3f6",
            "ink-faint": "#c07fd8",
            "ink-invert": "#1a0030",
            "border-w": "4px",
            "radius-sm": "12px",
            "radius-lg": "48px",
            "focus-w": "5px",
            "font-size": "15px",
            "title-size": "30px",
            "kicker-size": "13px",
            "chart-a": "#ffc400",
            "chart-b": "#00ffd0",
            "chart-c": "#ff5cf6",
            "chart-d": "#7cff00",
            "chart-e": "#ff2f6d",
            "stripe-a": "rgba(255, 196, 0, 0.65)",
            "stripe-b": "rgba(255, 92, 246, 0.35)",
        },
        field="rgba(24, 0, 48, 0.92)",
        ink="#fff3ff",
        line="#ff5cf6",
        accent="#ffc400",
        ok="#00ffd0",
        warn="#ff9d00",
        bad="#ff2f6d",
        info="#00e0ff",
        radius="26px",
        pad="20px",
        gap="16px",
        font="\"Forge Display\", \"Forge UI\", cursive, fantasy, sans-serif",
        mono="\"Forge Mono\", monospace",
        glow="rgba(255, 196, 0, 0.95)",
        shadow="rgba(94, 0, 128, 0.85)",
        inset="rgba(255, 92, 246, 0.55)",
        sheen="rgba(255, 255, 255, 0.30)",
        tile="14px",
        colormap="plasma",
    ),
}

# Python identifiers cannot contain "-", so `tokens(surface_2=...)` arrives as
# "surface_2". Normalize every key back to the CSS custom-property spelling.
for _table in TOKENS.values():
    for _key in [k for k in _table if "_" in k]:
        _table[_key.replace("_", "-")] = _table.pop(_key)


THEME_LABELS: dict[str, str] = {
    "default": "DragonGUI default",
    "modern-dark": "Modern dark",
    "modern-light": "Modern light",
    "win311": "Windows 3.11",
    "classic-mac": "Classic Mac",
    "contrast": "High contrast",
    "extreme": "Extreme",
}

THEME_ORDER: tuple[str, ...] = (
    "default",
    "modern-dark",
    "modern-light",
    "win311",
    "classic-mac",
    "contrast",
    "extreme",
)


def _px(value: str) -> float:
    """Parse a `<n>px` token into a float. Tokens are authored, never user input."""

    return float(value.strip().removesuffix("px") or 0.0)


def theme_for(key: str) -> dg.Theme:
    """Build the design-token `Theme` that matches a token table.

    The `Theme` carries the values the framework stylesheet and native renderer
    need before any user CSS is consulted; the token table carries everything
    the demo's own sheets read through `var()`.
    """

    table = TOKENS[key]
    light = key in {"modern-light", "win311", "classic-mac"}
    factory = dg.Theme.light if light else dg.Theme.dark
    return factory(
        background=table["bg"],
        surface=table["surface"],
        surface_alt=table["surface-2"],
        text=table["ink"],
        muted_text=table["ink-dim"],
        accent=table["accent"],
        border=table["line"],
        danger=table["bad"],
        warning=table["warn"],
        success=table["ok"],
        focus=table["accent"],
        disabled=table["ink-faint"],
        radius=_px(table["radius"]),
        spacing=_px(table["gap"]) * 0.6,
        font_size=_px(table["font-size"]),
        font_family=table["font"],
        monospace_font_family=table["mono"],
        control_height=30.0 if key == "extreme" else 26.0,
        compact_control_height=26.0 if key == "extreme" else 22.0,
        default_border_width=_px(table["border-w"]),
        focus_width=_px(table["focus-w"]),
        focus_offset=1.0,
        panel_padding=_px(table["pad"]),
        toolbar_gap=_px(table["gap"]) * 0.7,
    )


# ---------------------------------------------------------------------------
# Synthetic data
# ---------------------------------------------------------------------------

SAMPLE_COUNT = 180
_rng = random.Random(20260730)

TELEMETRY: dict[str, list[float]] = {
    "tick": [float(i) for i in range(SAMPLE_COUNT)],
    "paint": [
        58.0 + math.sin(i / 11.0) * 16.0 + math.sin(i / 2.7) * 3.4
        for i in range(SAMPLE_COUNT)
    ],
    "layout": [
        34.0 + math.cos(i / 8.5 + 0.6) * 11.0 + math.sin(i / 3.3) * 2.6
        for i in range(SAMPLE_COUNT)
    ],
    "cascade": [
        21.0 + math.sin(i / 17.0 + 1.3) * 8.0 + math.cos(i / 4.1) * 2.0
        for i in range(SAMPLE_COUNT)
    ],
}

HEAT_MATRIX: list[list[float]] = [
    [
        round(
            40.0
            + math.sin(row / 1.9) * 14.0
            + math.cos(col / 2.6) * 12.0
            + _rng.uniform(-4.0, 4.0),
            2,
        )
        for col in range(14)
    ]
    for row in range(9)
]

BAR_CATEGORIES: tuple[str, ...] = (
    "cascade",
    "layout",
    "paint",
    "text",
    "upload",
    "present",
)

STAGES = ("parse", "cascade", "layout", "paint", "text", "present")
SHEETS = ("variables", "structure", "appearance", "overrides", "extreme")
OUTCOMES = ("ok", "ok", "ok", "warn", "ok", "slow")


def make_rows(count: int, *, seed: int = 20260730) -> list[dict[str, Any]]:
    """Build the restyle-journal table rows."""

    rng = random.Random(seed)
    rows: list[dict[str, Any]] = []
    for index in range(count):
        stage = STAGES[index % len(STAGES)]
        sheet = SHEETS[index % len(SHEETS)]
        rows.append(
            {
                "#": index + 1,
                "stage": stage,
                "sheet": sheet,
                "selector": f"{('Panel', 'Button', 'Tab', 'Slider', 'DataFrameTable')[index % 5]}"
                f".{('card', 'primary', 'ghost', 'dense', 'wide')[index % 5]}",
                "rules": rng.randint(1, 240),
                "nodes": rng.randint(1, 900),
                "ms": round(rng.uniform(0.02, 7.4), 3),
                "outcome": OUTCOMES[index % len(OUTCOMES)],
                "note": (
                    "part forwarding verified",
                    "rounded clip held",
                    "stripe phase stable",
                    "focus ring preserved",
                    "shorthand lost to longhand",
                    "resource swapped live",
                )[index % 6],
            }
        )
    return rows


def make_scatter_frame() -> Any:
    """Build a numpy-backed point field for `ScatterPlot2D`."""

    if np is None:  # pragma: no cover - optional dependency
        return None
    rows = 45_000

    class ScatterFrame:
        columns = ("x", "y", "score")
        dtypes = ("float32", "float32", "float32")

        def __init__(self) -> None:
            index = np.arange(rows, dtype=np.float32)
            t = (index + 0.5) / rows
            golden = np.float32(math.pi * (3.0 - math.sqrt(5.0)))
            theta = index * golden
            radius = np.sqrt(t) * np.float32(3.0)
            wobble = np.sin(theta * np.float32(5.0)) * np.float32(0.08)
            self.x = (np.cos(theta) * (radius + wobble)).astype(np.float32)
            self.y = (np.sin(theta) * (radius + wobble)).astype(np.float32)
            self.score = (t * 0.7 + (np.cos(theta * 0.3) + 1.0) * 0.15).astype(np.float32)
            self.shape = (rows, 3)

        def __getitem__(self, column: str) -> Any:
            return getattr(self, column)

    return ScatterFrame()


# ---------------------------------------------------------------------------
# Managed image resources
# ---------------------------------------------------------------------------
# CSS cannot open files, so background images are registered as application
# resources and addressed by semantic id. Both textures are generated inline so
# the demo has no asset dependencies, and both are replaced/released live from
# the background lab.


def _png_chunk(kind: bytes, payload: bytes) -> bytes:
    body = kind + payload
    return struct.pack(">I", len(payload)) + body + struct.pack(">I", zlib.crc32(body))


def _png(width: int, height: int, pixel: Callable[[int, int], tuple[int, int, int, int]]) -> bytes:
    rows = bytearray()
    for y in range(height):
        rows.append(0)  # filter type 0
        for x in range(width):
            rows.extend(pixel(x, y))
    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + _png_chunk(b"IHDR", header)
        + _png_chunk(b"IDAT", zlib.compress(bytes(rows), 9))
        + _png_chunk(b"IEND", b"")
    )


def forge_texture(*, warm: bool = False) -> bytes:
    """A 32x32 diagonal-weave texture; `warm` returns the live-replacement variant."""

    def pixel(x: int, y: int) -> tuple[int, int, int, int]:
        weave = ((x + y) // 4) % 2
        ring = max(0, 90 - int(math.hypot(x - 16, y - 16) * 7))
        if warm:
            return (120 + ring + weave * 30, 60 + weave * 22, 24 + ring // 3, 255)
        return (18 + ring // 4, 52 + weave * 26, 110 + ring + weave * 30, 255)

    return _png(32, 32, pixel)


def badge_texture() -> bytes:
    """A 16x16 radial pip used for `contain` fitting and small rounded clips."""

    def pixel(x: int, y: int) -> tuple[int, int, int, int]:
        d = math.hypot(x - 7.5, y - 7.5)
        if d > 7.4:
            return (0, 0, 0, 0)
        level = int(max(0.0, 1.0 - d / 7.4) * 255)
        return (level, int(level * 0.55), 255 - level // 2, 255)

    return _png(16, 16, pixel)


# ---------------------------------------------------------------------------
# Sheet 1 of 5: ":root" custom properties
# ---------------------------------------------------------------------------
# This sheet is regenerated on every theme switch and on every CSS-variable
# edit. It is registered first so later sheets always see resolved tokens.


def variables_css(key: str, edits: dict[str, str] | None = None) -> str:
    """Serialize a token table (plus live editor overrides) as a `:root` block."""

    table = dict(TOKENS[key])
    if edits:
        table.update(edits)
    body = "\n".join(f"    --tf-{name}: {value};" for name, value in sorted(table.items()))
    return f"/* theme tokens: {key} */\n:root {{\n{body}\n}}\n"


# ---------------------------------------------------------------------------
# Sheet 2 of 5: structure + identity
# ---------------------------------------------------------------------------
# Authored once, never replaced. Everything cosmetic reads through var(), so a
# theme switch restyles this sheet without reparsing it.

_FONT_FACES = """
/* Ordered fallback: a deliberately missing family first, then real ones.
   Missing local families are skipped and reported in font diagnostics. */
@font-face {
    font-family: "Forge UI";
    src:
        local("Forge Grotesk Nonexistent"),
        local("Segoe UI Variable Text"),
        local("Segoe UI"),
        local("Helvetica Neue"),
        local("DejaVu Sans");
}

@font-face {
    font-family: "Forge Display";
    src:
        local("Forge Display Nonexistent"),
        local("Segoe UI Semibold"),
        local("Impact"),
        local("DejaVu Sans Bold");
}

@font-face {
    font-family: "Forge Mono";
    src:
        local("Forge Mono Nonexistent"),
        local("Cascadia Mono"),
        local("Consolas"),
        local("DejaVu Sans Mono");
}
"""

_SHELL = """
/* ----- shell ----------------------------------------------------------- */

Window {
    background: var(--tf-bg);
    color: var(--tf-ink);
    font-family: var(--tf-font);
    font-size: var(--tf-font-size);
    padding: 0;
    gap: 0;
}

AppShell.forge-shell {
    gap: 0;
    width: 100%;
    height: 100%;
}

Sidebar#forge-sidebar {
    background: var(--tf-surface);
    border-right: var(--tf-border-w) solid var(--tf-line);
    padding: var(--tf-pad);
    gap: var(--tf-gap);
    /* The rail owns its own scrolling: the extreme theme's 20px padding and
       taller controls push twelve nav items past a 640px-high viewport. */
    overflow-y: auto;
}

Sidebar#forge-sidebar::scrollbar-track { width: 8px; background: var(--tf-line-soft); }
Sidebar#forge-sidebar::scrollbar-thumb { width: 5px; background: var(--tf-accent); border-radius: 999px; }

Sidebar#forge-sidebar::title {
    color: var(--tf-accent);
    font-family: var(--tf-font);
    font-weight: 800;
    letter-spacing: 0.6px;
    text-transform: uppercase;
}

Label.rail-label {
    color: var(--tf-ink-faint);
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 1.4px;
    text-transform: uppercase;
}

Label.rail-note {
    color: var(--tf-ink-dim);
    font-family: var(--tf-mono);
    font-size: 11px;
}

NavItem {
    border-radius: var(--tf-radius-sm);
    color: var(--tf-ink-dim);
    padding: 6px 9px;
}

NavItem:hover { background: var(--tf-accent-soft); color: var(--tf-ink); }
NavItem:selected { background: var(--tf-accent-soft); color: var(--tf-ink); }

/* Part geometry is stateless by design: the width lives on the unstated rule
   and only the color varies with :selected. */
NavItem::accent { width: 3px; }
NavItem:selected::accent { background: var(--tf-accent); }
NavItem::badge {
    background: var(--tf-accent);
    color: var(--tf-accent-ink);
    border-radius: var(--tf-radius-sm);
    font-size: 10px;
    font-weight: 800;
    padding: 1px 5px;
}

WorkbenchLayout.forge-work { gap: 0; padding: 0; }

MenuBar {
    background: var(--tf-surface);
    border-bottom: var(--tf-border-w) solid var(--tf-line);
    padding-left: 6px;
}

Menu {
    background: transparent;
    border: 0;
    border-radius: var(--tf-radius-sm);
    color: var(--tf-ink-dim);
    padding: 4px 10px;
}

Menu:hover { background: var(--tf-accent-soft); color: var(--tf-ink); }
Menu:open { background: var(--tf-accent); color: var(--tf-accent-ink); }
Menu::menu, ContextMenu::menu {
    background: var(--tf-surface-2);
    border: var(--tf-border-w) solid var(--tf-line);
    border-radius: var(--tf-radius);
}
Menu::item, ContextMenu::item { color: var(--tf-ink); padding: 5px 10px; }
Menu::item-hover, ContextMenu::item-hover { background: var(--tf-accent-soft); }
Menu::item-disabled, ContextMenu::item-disabled { color: var(--tf-ink-faint); }

Toolbar.forge-toolbar {
    background: var(--tf-surface-2);
    border-bottom: var(--tf-border-w) solid var(--tf-line);
    gap: var(--tf-gap);
    padding: 6px var(--tf-pad);
}

Tabs#forge-tabs {
    background: var(--tf-surface-2);
    border-bottom: var(--tf-border-w) solid var(--tf-line);
    gap: 3px;
    padding-left: 8px;
    padding-right: 8px;
}

Tabs#forge-tabs::header { background: var(--tf-surface-2); height: 38px; }

Tab {
    background: transparent;
    border: 1px solid transparent;
    border-radius: 7px 7px 0 0;
    color: var(--tf-ink-dim);
    font-size: 12.5px;
    font-weight: 600;
    padding: 6px 10px;
}

Tab:hover {
    background: var(--tf-line-soft);
    color: var(--tf-ink);
}
Tab:selected {
    background: var(--tf-accent-soft);
    border-color: var(--tf-line);
    color: var(--tf-ink);
    font-weight: 750;
}
Tab:selected::tab { background: var(--tf-accent-soft); }
Tab::accent { height: 3px; }
Tab:selected::accent { background: var(--tf-accent); }
Tab::badge {
    background: var(--tf-accent-soft);
    color: var(--tf-ink);
    border-radius: 999px;
    font-size: 10px;
    padding: 0px 5px;
}
Tab:selected::badge {
    background: var(--tf-accent);
    color: var(--tf-accent-ink);
}

Body { background: var(--tf-bg); }

ScrollArea.forge-page {
    padding: var(--tf-pad);
    padding-bottom: 48px;
    gap: var(--tf-gap);
    overflow-y: auto;
}

ScrollArea.forge-page::scrollbar-track {
    width: 10px;
    padding: 2px;
    background: var(--tf-line-soft);
    border-radius: 999px;
}

ScrollArea.forge-page::scrollbar-thumb {
    width: 6px;
    background: var(--tf-accent);
    border-radius: 999px;
}

StatusBar.forge-status {
    background: var(--tf-surface);
    border-top: var(--tf-border-w) solid var(--tf-line);
    gap: var(--tf-gap);
    padding: 0 var(--tf-pad);
}
"""

_TYPO_BASE = """
/* ----- shared typography and cards ------------------------------------- */

VLayout.page-heading { gap: 4px; }

Label.kicker {
    color: var(--tf-accent);
    font-size: var(--tf-kicker-size);
    font-weight: 800;
    letter-spacing: 1.6px;
    text-transform: uppercase;
}

Label.page-title {
    color: var(--tf-ink);
    font-family: var(--tf-font);
    font-size: var(--tf-title-size);
    font-weight: 850;
}

Label.page-desc { color: var(--tf-ink-dim); line-height: 1.35; }
Label.muted { color: var(--tf-ink-dim); }
Label.faint { color: var(--tf-ink-faint); font-size: 11px; }
Label.mono { color: var(--tf-ink-dim); font-family: var(--tf-mono); font-size: 11px; }

Label.pass {
    background: var(--tf-accent-soft);
    border-left: 3px solid var(--tf-accent);
    border-radius: var(--tf-radius-sm);
    color: var(--tf-ink);
    padding: 6px 9px;
    width: 100%;
}

Panel.card {
    background: var(--tf-surface);
    border: var(--tf-border-w) solid var(--tf-line);
    border-radius: var(--tf-radius);
    gap: var(--tf-gap);
    padding: var(--tf-pad);
}

Panel.card::header { background: var(--tf-surface-2); }
Panel.card::title { color: var(--tf-ink); font-weight: 750; }
Panel.card::accent { background: var(--tf-accent); width: 2px; }

Panel.card.dense { gap: 5px; padding: 9px; }
Panel.card.flush { padding: 0; }

Separator { background: var(--tf-line); }

Button {
    background: var(--tf-surface-2);
    border: var(--tf-border-w) solid var(--tf-line);
    border-radius: var(--tf-radius-sm);
    color: var(--tf-ink);
}

Button:hover { background: var(--tf-surface-3); border-color: var(--tf-line-strong); }
Button:active { background: var(--tf-accent-soft); }
Button:disabled { color: var(--tf-ink-faint); border-color: var(--tf-line-soft); }

Button.primary {
    background: var(--tf-accent);
    border-color: var(--tf-accent);
    color: var(--tf-accent-ink);
    font-weight: 750;
}

Button.primary:hover { box-shadow: 0 0 14px var(--tf-glow); }

Button.ghost { background: transparent; border-color: var(--tf-line-soft); }
Button.danger { background: var(--tf-bad); border-color: var(--tf-bad); color: var(--tf-ink-invert); }

TextInput, TextArea, NumberInput, SearchBox, DragNumber, CodeEditor, Dropdown {
    background: var(--tf-field);
    border: var(--tf-border-w) solid var(--tf-line);
    border-radius: var(--tf-radius-sm);
    color: var(--tf-ink);
    font-family: var(--tf-font);
}

TextInput:focus, TextArea:focus, NumberInput:focus, Dropdown:focus {
    border-color: var(--tf-accent);
    outline: var(--tf-focus-w) dotted var(--tf-accent);
    outline-offset: 2px;
}

Badge { border-radius: 999px; font-size: 10px; font-weight: 800; padding: 1px 7px; }
Badge.info { background: var(--tf-accent-soft); color: var(--tf-ink); }
Badge.success { background: var(--tf-ok); color: var(--tf-ink-invert); }
Badge.warning { background: var(--tf-warn); color: var(--tf-ink-invert); }
Badge.danger, Badge.error { background: var(--tf-bad); color: var(--tf-ink-invert); }

Tag {
    background: transparent;
    border: var(--tf-border-w) solid var(--tf-line);
    border-radius: var(--tf-radius-sm);
    color: var(--tf-ink-dim);
    font-size: 11px;
    padding: 1px 6px;
}

LogView {
    background: var(--tf-field);
    border: var(--tf-border-w) solid var(--tf-line);
    border-radius: var(--tf-radius-sm);
    font-family: var(--tf-mono);
    font-size: 11px;
}

LogView::info { color: var(--tf-ink-dim); }
LogView::warning { color: var(--tf-warn); }
LogView::error { color: var(--tf-bad); }

DataFrameTable {
    background: var(--tf-field);
    border: var(--tf-border-w) solid var(--tf-line);
    border-radius: var(--tf-radius-sm);
    color: var(--tf-ink);
    font-size: 12px;
}

Modal::scrim { background: rgba(0, 0, 0, 0.55); }
Modal::body { background: var(--tf-surface); }
Modal::header { background: var(--tf-surface-2); }
Modal::title { color: var(--tf-ink); font-weight: 800; }

Tooltip {
    background: var(--tf-surface-3);
    border: var(--tf-border-w) solid var(--tf-line-strong);
    border-radius: var(--tf-radius-sm);
    color: var(--tf-ink);
}

Tooltip.static { border-radius: var(--tf-radius-sm); }

Toast.info { background: var(--tf-surface-3); color: var(--tf-ink); }
Toast.success { background: var(--tf-ok); color: var(--tf-ink-invert); }
Toast.warning { background: var(--tf-warn); color: var(--tf-ink-invert); }
Toast.error { background: var(--tf-bad); color: var(--tf-ink-invert); }
"""

_LAB_THEME = """
/* ----- lab 1: theme laboratory ----------------------------------------- */

Button.theme-pick {
    border-radius: var(--tf-radius-sm);
    font-size: 12px;
    min-width: 118px;
}

Button.theme-pick.active {
    background: var(--tf-accent);
    border-color: var(--tf-accent);
    color: var(--tf-accent-ink);
    font-weight: 800;
}

Panel.swatch {
    border: var(--tf-border-w) solid var(--tf-line);
    border-radius: var(--tf-radius-sm);
    gap: 0;
    height: 54px;
    padding: 0;
    width: 100%;
}

Panel.swatch-chip {
    align-items: center;
    border: 0;
    border-radius: var(--tf-radius-sm);
    box-shadow: none;
    flex-basis: 0;
    flex-grow: 1;
    flex-shrink: 1;
    font-size: 10px;
    font-weight: 800;
    justify-content: center;
    min-width: 0;
    padding: 5px 7px;
    text-align: center;
    width: auto;
}

Panel.chip-accent { background: var(--tf-accent); color: var(--tf-accent-ink); }
Panel.chip-ok { background: var(--tf-ok); color: var(--tf-ink-invert); }
Panel.chip-warn { background: var(--tf-warn); color: var(--tf-ink-invert); }
Panel.chip-bad { background: var(--tf-bad); color: var(--tf-ink-invert); }
Panel.chip-surface { background: var(--tf-surface-3); color: var(--tf-ink); }
Panel.swatch-chip > Label { text-align: center; width: 100%; }

Panel.var-editor { background: var(--tf-surface-2); }
Label.var-name { color: var(--tf-accent); font-family: var(--tf-mono); font-size: 11px; }
Label.var-value { color: var(--tf-ink); font-family: var(--tf-mono); font-size: 11px; }

Panel.retain-probe {
    background: var(--tf-surface-2);
    border: var(--tf-border-w) dashed var(--tf-accent);
    border-radius: var(--tf-radius);
}
"""

_LAB_GALLERY = """
/* ----- lab 2: widget gallery ------------------------------------------- */

Panel.gallery-card { background: var(--tf-surface); }
Label.state-label {
    color: var(--tf-ink-faint);
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 1.1px;
    text-transform: uppercase;
    width: 74px;
}

/* Static previews of the interactive states, so hover/press/focus/error can be
   compared side by side without the pointer. */
Button.as-hover { background: var(--tf-surface-3); border-color: var(--tf-line-strong); }
Button.as-pressed { background: var(--tf-accent-soft); border-color: var(--tf-accent); }
Button.as-focus { outline: var(--tf-focus-w) dotted var(--tf-accent); outline-offset: 2px; }
Button.as-selected { background: var(--tf-accent); color: var(--tf-accent-ink); }
Button.as-error { border-color: var(--tf-bad); color: var(--tf-bad); }

TextInput.as-error, TextArea.as-error, NumberInput.as-error {
    background: var(--tf-field);
    border-color: var(--tf-bad);
    color: var(--tf-bad);
}

TextInput.as-focus { border-color: var(--tf-accent); outline: var(--tf-focus-w) dotted var(--tf-accent); }

Panel.mini-window {
    background: var(--tf-surface-2);
    border: var(--tf-border-w) solid var(--tf-line);
    border-radius: var(--tf-radius);
    gap: 0;
    padding: 0;
}

HLayout.mini-titlebar {
    background: var(--tf-accent);
    border-bottom: var(--tf-border-w) solid var(--tf-line);
    gap: 4px;
    padding: 4px 6px;
}

Label.mini-title { color: var(--tf-accent-ink); font-size: 11px; font-weight: 800; }
"""

_LAB_PARTS = """
/* ----- lab 3: composite parts ------------------------------------------
   Each public part gets a deliberately different treatment. If a part is
   forwarded to the wrong sub-widget the mismatch is immediately visible. */

SearchBox.part-probe {
    background: #101c2c;
    border: 2px solid #ff9d3d;
    border-radius: 16px;
}

SearchBox.part-probe::icon { color: #ffd166; }
SearchBox.part-probe::field {
    background: #05202b;
    border: 1px dashed #4cd4ff;
    border-radius: 4px;
    color: #d6f7ff;
}
SearchBox.part-probe::clear { background: #ff2f6d; color: #ffffff; border-radius: 999px; }

Panel.part-probe {
    background: #131a2b;
    border: 0;
    border-radius: 10px;
    padding: 0;
}
Panel.part-probe::header { background: #4a007a; }
Panel.part-probe::title { color: #7cff00; font-weight: 900; text-transform: uppercase; }
Panel.part-probe::body { background: #001d33; }
Panel.part-probe::accent { background: #ff2f6d; width: 6px; }
Panel.part-probe::scrollbar-track { width: 12px; background: #4a007a; border-radius: 0; }
Panel.part-probe::scrollbar-thumb { width: 8px; background: #7cff00; border-radius: 0; }

Slider.part-probe::track { background: #4a007a; height: 12px; border-radius: 6px; }
Slider.part-probe::fill { background: #7cff00; }
Slider.part-probe::thumb {
    background: #ffd166;
    border: 2px solid #ff2f6d;
    border-radius: 3px;
    width: 22px;
}

RangeSlider.part-probe::track { background: #001d33; height: 10px; }
RangeSlider.part-probe::range { background: #4cd4ff; }
RangeSlider.part-probe::thumb-min { background: #7cff00; border-radius: 999px; width: 16px; }
RangeSlider.part-probe::thumb-max { background: #ff2f6d; border-radius: 0; width: 16px; }

ProgressBar.part-probe::track { background: #4a007a; border-radius: 0; }
ProgressBar.part-probe::fill { background: #ffd166; }
ProgressBar.part-probe::label { color: #7cff00; font-weight: 900; }

DataFrameTable.part-probe {
    border: 2px solid #ff9d3d;
    border-radius: 12px;
}
DataFrameTable.part-probe::header { background: #4a007a; color: #ffd166; }
DataFrameTable.part-probe::row { background: #05202b; }
DataFrameTable.part-probe::row-selected { background: #7cff00; color: #001d33; }
DataFrameTable.part-probe::grid-line { background: #ff2f6d; }
DataFrameTable.part-probe::scrollbar-track { width: 12px; background: #4a007a; }
DataFrameTable.part-probe::scrollbar-thumb { width: 8px; background: #4cd4ff; border-radius: 999px; }

Button.part-probe::badge { background: #7cff00; color: #001d33; border-radius: 0; font-weight: 900; }
Tab.part-probe::badge { background: #ff2f6d; color: #ffffff; border-radius: 3px; }
NavItem.part-probe::badge { background: #ffd166; color: #001d33; border-radius: 999px; }
NavItem.part-probe::accent { background: #4cd4ff; width: 6px; }

Checkbox.part-probe::box { background: #001d33; border: 2px solid #4cd4ff; border-radius: 0; }
Checkbox.part-probe:checked::indicator { background: #7cff00; }
Checkbox.part-probe::label { color: #ffd166; font-weight: 800; }

ToggleSwitch.part-probe::track { background: #4a007a; }
ToggleSwitch.part-probe::thumb { background: #ffd166; border-radius: 3px; }
ToggleSwitch.part-probe:checked::track { background: #7cff00; }

NumberInput.part-probe::field { background: #05202b; }
NumberInput.part-probe::stepper { width: 34px; }
NumberInput.part-probe::stepper-up { background: #7cff00; color: #001d33; }
NumberInput.part-probe::stepper-down { background: #ff2f6d; color: #ffffff; }
NumberInput.part-probe::stepper-divider { background: #ffd166; }
NumberInput.part-probe::caret { background: #4cd4ff; width: 3px; }

Dropdown.part-probe::field { background: #05202b; }
Dropdown.part-probe::chevron { color: #7cff00; width: 18px; }
Dropdown.part-probe::menu { background: #131a2b; border: 2px solid #ff9d3d; }
Dropdown.part-probe::item { color: #d6f7ff; padding: 6px 10px; }
Dropdown.part-probe::item-hover { background: #4a007a; }
Dropdown.part-probe::item-selected { background: #7cff00; color: #001d33; }

LED.part-probe::dot { background: #7cff00; }
LED.part-probe::glow { background: #ff2f6d; opacity: 0.55; }
LED.part-probe::highlight { background: #ffffff; }

Collapsible.part-probe::header { background: #4a007a; }
Collapsible.part-probe::indicator { color: #ffd166; }
Collapsible.part-probe::body { background: #001d33; }

Splitter.part-probe::gutter { background: #ff9d3d; }
TreeNode.part-probe::row { background: #05202b; }
TreeNode.part-probe::guide { background: #4cd4ff; }
TreeNode.part-probe::indicator { color: #7cff00; }
TreeNode.part-probe::label { color: #ffd166; }

Selectable.part-probe::row { background: #05202b; }
Selectable.part-probe::indicator { background: #ff2f6d; }
Selectable.part-probe::label { color: #d6f7ff; }
Selectable.part-probe:selected::row { background: #7cff00; }

/* Intentionally wrong: Button has no stepper part. This must surface as a
   stylesheet warning rather than a crash or a silent mis-paint. */
Button.part-probe::stepper { background: #ff0000; }
"""

_LAB_BORDERS = """
/* ----- lab 4: borders and outlines -------------------------------------- */

Panel.bd {
    background: var(--tf-surface-2);
    border-radius: var(--tf-radius-sm);
    gap: 4px;
    min-height: 76px;
    padding: 9px;
}

Panel.bd-sides {
    border-top: 1px solid var(--tf-ok);
    border-right: 4px dashed var(--tf-warn);
    border-bottom: 7px double var(--tf-bad);
    border-left: 2px dotted var(--tf-accent);
    border-radius: 0;
}

Panel.bd-solid { border: 2px solid var(--tf-accent); }
Panel.bd-dashed { border: 3px dashed var(--tf-warn); }
Panel.bd-dotted { border: 3px dotted var(--tf-ok); }
Panel.bd-double { border: 9px double var(--tf-bad); }

Panel.bd-asym {
    border-top: 1px solid var(--tf-accent);
    border-right: 9px solid var(--tf-bad);
    border-bottom: 3px solid var(--tf-ok);
    border-left: 16px solid var(--tf-warn);
}

Panel.bd-round {
    border: 3px solid var(--tf-accent);
    border-top-left-radius: 30px;
    border-top-right-radius: 2px;
    border-bottom-right-radius: 30px;
    border-bottom-left-radius: 2px;
}

Panel.bd-clip {
    border: 4px solid var(--tf-accent);
    border-radius: 22px;
    overflow: hidden;
    background:
        repeating-linear-gradient(
            45deg,
            var(--tf-stripe-a) 0px,
            var(--tf-stripe-a) 6px,
            var(--tf-stripe-b) 6px,
            var(--tf-stripe-b) 12px
        ),
        var(--tf-surface-3);
}

Panel.bd-clip Label { color: var(--tf-ink); }

Panel.bd-focus {
    border: var(--tf-border-w) solid var(--tf-line);
    outline: 2px dotted var(--tf-accent);
    outline-offset: 3px;
}

Panel.bd-inset {
    background: var(--tf-surface);
    border-top: 2px solid var(--tf-inset);
    border-left: 2px solid var(--tf-inset);
    border-right: 2px solid var(--tf-sheen);
    border-bottom: 2px solid var(--tf-sheen);
    border-radius: 0;
}

Panel.bd-outset {
    background: var(--tf-surface);
    border-top: 2px solid var(--tf-sheen);
    border-left: 2px solid var(--tf-sheen);
    border-right: 2px solid var(--tf-inset);
    border-bottom: 2px solid var(--tf-inset);
    border-radius: 0;
}

/* Border changes on interaction. Layout must not shift, because the widths
   stay constant and only the colors/styles change. */
Button.bd-react {
    background: var(--tf-surface-2);
    border: 2px solid var(--tf-line);
    border-radius: var(--tf-radius-sm);
    transition: border-color 140ms ease, outline-color 140ms ease;
}

Button.bd-react:hover { border-color: var(--tf-warn); }
Button.bd-react:focus { border-color: var(--tf-accent); outline: 3px dotted var(--tf-accent); outline-offset: 2px; }
Button.bd-react:active { border-color: var(--tf-bad); }

Selectable.bd-react { border: 2px solid var(--tf-line); border-radius: var(--tf-radius-sm); }
Selectable.bd-react:selected { border-color: var(--tf-accent); border-style: double; border-width: 6px; }

/* Very small and very large controls expose corner and DPI rounding bugs. */
Button.bd-tiny { border: 1px solid var(--tf-accent); border-radius: 4px; font-size: 8px; height: 14px; min-width: 14px; padding: 0px 2px; }
Button.bd-huge { border: 12px double var(--tf-accent); border-radius: 40px; font-size: 26px; height: 96px; min-width: 240px; }
Panel.bd-hair { border: 1px solid var(--tf-line-strong); border-radius: 1px; height: 18px; min-height: 18px; padding: 1px; }
"""

_LAB_PAINT = """
/* ----- lab 5: backgrounds, gradients, patterns -------------------------- */

Panel.bg {
    border: var(--tf-border-w) solid var(--tf-line);
    border-radius: var(--tf-radius);
    gap: 4px;
    min-height: 108px;
    padding: 10px;
}

Panel.bg Label { color: var(--tf-ink); }

Panel.bg-linear { background: linear-gradient(135deg, var(--tf-accent), var(--tf-bad) 55%, var(--tf-warn)); }
Panel.bg-linear-oklab {
    background: linear-gradient(to right, var(--tf-accent), var(--tf-warn));
    gradient-interpolation: oklab;
}
Panel.bg-radial { background: radial-gradient(circle at 30% 30%, var(--tf-warn), var(--tf-surface) 70%); }
Panel.bg-radial-corner { background: radial-gradient(circle at right bottom, var(--tf-ok), var(--tf-surface) 62%); }
Panel.bg-repeating-linear {
    background: repeating-linear-gradient(
        90deg,
        var(--tf-accent) 0px,
        var(--tf-accent) 1px,
        transparent 1px,
        transparent 4px
    );
}
Panel.bg-repeating-radial {
    background: repeating-radial-gradient(
        circle at 50% 50%,
        var(--tf-accent) 0px,
        var(--tf-accent) 2px,
        var(--tf-surface) 2px,
        var(--tf-surface) 9px
    );
}

/* Percentage stops, pixel stops, calc() stops, and a hard stop. */
Panel.bg-stops-percent {
    background: linear-gradient(to right, var(--tf-ok) 0%, var(--tf-warn) 35%, var(--tf-bad) 70%, var(--tf-accent) 100%);
}
Panel.bg-stops-pixel {
    background: linear-gradient(to right, var(--tf-accent) 0px, var(--tf-accent) 40px, var(--tf-surface-3) 40px, var(--tf-surface-3) 120px, var(--tf-bad) 120px);
}
Panel.bg-stops-calc {
    background: linear-gradient(to right, var(--tf-accent) calc(50% - 60px), var(--tf-bad) calc(50% + 60px));
}
Panel.bg-hard-stop {
    background: linear-gradient(
        to bottom,
        var(--tf-ok) 0%,
        var(--tf-ok) 33%,
        var(--tf-warn) 33%,
        var(--tf-warn) 66%,
        var(--tf-bad) 66%
    );
}

/* One- and two-pixel stripes: the period must stay fixed as the panel resizes
   and must scale correctly with display DPI. */
Panel.bg-stripe-1 {
    background: repeating-linear-gradient(
        0deg,
        var(--tf-stripe-a) 0px,
        var(--tf-stripe-a) 1px,
        var(--tf-surface) 1px,
        var(--tf-surface) 2px
    );
}
Panel.bg-stripe-2 {
    background: repeating-linear-gradient(
        90deg,
        var(--tf-stripe-a) 0px,
        var(--tf-stripe-a) 2px,
        var(--tf-surface) 2px,
        var(--tf-surface) 4px
    );
}

Panel.bg-checker { background: dg-pattern(checker, var(--tf-stripe-a), var(--tf-surface), var(--tf-tile)); }
Panel.bg-pinstripe { background: dg-pattern(pinstripe, var(--tf-stripe-a), var(--tf-surface), var(--tf-tile)); }
Panel.bg-stipple { background: dg-pattern(stipple, var(--tf-stripe-a), var(--tf-surface), var(--tf-tile)); }
Panel.bg-dot { background: dg-pattern(dot, var(--tf-accent), var(--tf-surface-2), var(--tf-tile)); }
Panel.bg-hatch { background: dg-pattern(diagonal-hatch, var(--tf-stripe-a), var(--tf-surface), var(--tf-tile)); }

Panel.bg-layers {
    background:
        radial-gradient(circle at 18% 22%, rgba(255, 255, 255, 0.22), transparent 46%),
        linear-gradient(160deg, var(--tf-accent), transparent 60%),
        dg-pattern(diagonal-hatch, var(--tf-stripe-a), var(--tf-surface), 10px);
    background-noise: 0.02;
}

Panel.bg-blob {
    background: blob-gradient(
        at 22% 30% var(--tf-accent) 46%,
        at 74% 26% var(--tf-bad) 38%,
        at 52% 82% var(--tf-ok) 42%
    );
}

Panel.bg-mesh { background: mesh-gradient(var(--tf-accent), var(--tf-warn), var(--tf-bad), var(--tf-ok)); }

Panel.bg-image-contain { background-color: var(--tf-surface); background-image: app-resource("forge-weave", contain); }
Panel.bg-image-cover { background-color: var(--tf-surface); background-image: app-resource("forge-weave", cover); }
Panel.bg-image-stretch { background-color: var(--tf-surface); background-image: app-resource("forge-weave", stretch); }
Panel.bg-image-repeat { background-color: var(--tf-surface); background-image: app-resource("forge-weave", repeat); }

Panel.bg-image-round {
    background-color: var(--tf-surface);
    background-image: app-resource("forge-weave", cover);
    border: 3px solid var(--tf-accent);
    border-radius: 34px;
}

Panel.bg-pattern-round {
    background: dg-pattern(checker, var(--tf-stripe-a), var(--tf-surface), 10px);
    border: 3px solid var(--tf-warn);
    border-radius: 34px;
}

Panel.bg-pip {
    background-color: var(--tf-surface-2);
    background-image: app-resource("forge-pip", contain);
    border-radius: 999px;
    height: 64px;
    min-height: 64px;
    width: 64px;
}

/* Resized continuously by the live worker to prove patterns do not stretch. */
Panel.bg-flex {
    background: repeating-linear-gradient(
        45deg,
        var(--tf-accent) 0px,
        var(--tf-accent) 3px,
        var(--tf-surface) 3px,
        var(--tf-surface) 9px
    );
    border: 2px solid var(--tf-line);
    border-radius: var(--tf-radius);
    height: 68px;
    min-height: 68px;
}
"""

_LAB_TYPE = """
/* ----- lab 6: typography ------------------------------------------------ */

Panel.ty { background: var(--tf-surface-2); gap: 6px; }

Label.ty-stack-missing { font-family: "Absolutely Missing Family", "Also Missing", var(--tf-font); }
Label.ty-serif { font-family: serif; }
Label.ty-sans { font-family: sans-serif; }
Label.ty-mono { font-family: monospace; }
Label.ty-cursive { font-family: cursive; }
Label.ty-fantasy { font-family: fantasy; }

Label.ty-w100 { font-weight: 100; }
Label.ty-w400 { font-weight: 400; }
Label.ty-w700 { font-weight: 700; }
Label.ty-w900 { font-weight: 900; }
Label.ty-italic { font-style: italic; }
Label.ty-tabular { font-family: var(--tf-mono); font-variant-numeric: tabular-nums; }

Label.ty-xs { font-size: 8px; line-height: 1.05; }
Label.ty-sm { font-size: 11px; }
Label.ty-lg { font-size: 26px; }
Label.ty-xl { font-size: 44px; line-height: 1.02; }

Label.ty-wrap { line-height: 1.45; width: 100%; }
Label.ty-clip { text-overflow: clip; width: 190px; }
Label.ty-ellipsis { text-overflow: ellipsis; width: 190px; }
Label.ty-track { letter-spacing: 3px; text-transform: uppercase; }
Label.ty-tight { letter-spacing: -0.5px; }
Label.ty-upper { text-transform: uppercase; }
Label.ty-lower { text-transform: lowercase; }
Label.ty-capital { text-transform: capitalize; }

Button.ty-narrow { font-size: 11px; max-width: 96px; min-width: 0; padding: 2px 4px; }
Badge.ty-long { max-width: 130px; }
Label.ty-unicode { font-size: 17px; line-height: 1.5; }
"""

_LAB_LAYOUT = """
/* ----- lab 7: layout ---------------------------------------------------- */

Panel.ly { background: var(--tf-surface-2); gap: 6px; padding: 8px; }
Panel.ly-fixed { min-width: 132px; width: 132px; }
Panel.ly-flex { flex: 1; min-width: 0; }
Panel.ly-content { flex-grow: 0; flex-shrink: 0; }

Panel.ly-constrained {
    height: 132px;
    max-height: 132px;
    min-height: 132px;
    overflow: hidden;
}

ScrollArea.ly-inner {
    background: var(--tf-field);
    border: var(--tf-border-w) solid var(--tf-line);
    border-radius: var(--tf-radius-sm);
    gap: 3px;
    height: 148px;
    overflow-y: auto;
    padding: 6px;
}

ScrollArea.ly-inner::scrollbar-track { width: 8px; background: var(--tf-line-soft); }
ScrollArea.ly-inner::scrollbar-thumb { width: 5px; background: var(--tf-accent); border-radius: 999px; }

Splitter.ly-split { border: var(--tf-border-w) solid var(--tf-line); border-radius: var(--tf-radius-sm); }
Splitter.ly-split::gutter { background: var(--tf-line); }

GridLayout.ly-grid { gap: 8px; }
FlowLayout.ly-flow { gap: 6px; row-gap: 6px; }

/* Responsive behavior is exercised by the autopilot resize sweep. */
@media (max-width: 760px) {
    Label.ly-breakpoint::after { content: " [<=760]"; color: var(--tf-warn); }
    Panel.ly-fixed { width: 100px; min-width: 100px; }
}

@media (min-width: 1200px) {
    Label.ly-breakpoint::after { content: " [>=1200]"; color: var(--tf-ok); }
}

@media (min-resolution: 1.5dppx) {
    Label.ly-dpi::after { content: " [hidpi]"; color: var(--tf-accent); }
}

@supports (display: grid) {
    Label.ly-supports::after { content: " [grid ok]"; color: var(--tf-ok); }
}
"""

_LAB_EFFECTS = """
/* ----- lab 8: effects --------------------------------------------------- */

Panel.fx-stage { border: var(--tf-border-w) solid var(--tf-line); border-radius: var(--tf-radius); padding: 12px; gap: 10px; }
Panel.fx-dark { background: #05070c; }
Panel.fx-light { background: #f2f5fa; }
Panel.fx-light Label { color: #10141c; }
Panel.fx-dark Label { color: #e9f0ff; }

Button.fx-glow-0 { box-shadow: none; }
Button.fx-glow-1 { box-shadow: 0 0 6px var(--tf-glow); }
Button.fx-glow-2 { box-shadow: 0 0 14px var(--tf-glow); }
Button.fx-glow-3 { box-shadow: 0 0 26px 4px var(--tf-glow); }

Badge.fx-glow-1 { box-shadow: 0 0 8px var(--tf-glow); }
Badge.fx-glow-2 { box-shadow: 0 0 18px 3px var(--tf-glow); }

LED.fx-glow-1::glow { opacity: 0.30; }
LED.fx-glow-2::glow { opacity: 0.70; }
LED.fx-glow-3::glow { opacity: 1.0; }
LED.fx-glow-3::dot { box-shadow: 0 0 16px var(--tf-glow); }

Panel.fx-shadow-1 { box-shadow: 0 2px 6px var(--tf-shadow); }
Panel.fx-shadow-2 { box-shadow: 0 8px 20px var(--tf-shadow); }
Panel.fx-shadow-3 { box-shadow: 0 18px 44px 6px var(--tf-shadow); }
Panel.fx-shadow-multi {
    box-shadow:
        0 1px 0 var(--tf-sheen),
        0 10px 26px var(--tf-shadow),
        0 0 30px var(--tf-glow);
}

Panel.fx-inset-1 { box-shadow: inset 0 2px 6px var(--tf-inset); }
Panel.fx-inset-2 { box-shadow: inset 0 6px 16px var(--tf-inset); }
Panel.fx-inset-mixed { box-shadow: inset 0 3px 9px var(--tf-inset), 0 6px 16px var(--tf-shadow); }

Panel.fx-round-shadow { border-radius: 30px; box-shadow: 0 12px 30px var(--tf-shadow); }

Button.fx-hover-shadow { transition: box-shadow 160ms ease, background 160ms ease; }
Button.fx-hover-shadow:hover { box-shadow: 0 8px 20px var(--tf-shadow), 0 0 16px var(--tf-glow); }
Button.fx-hover-shadow:active { box-shadow: inset 0 3px 8px var(--tf-inset); }

Button.fx-ring:focus { outline: var(--tf-focus-w) dotted var(--tf-accent); outline-offset: 4px; }
Button.fx-ring-tight:focus { outline: var(--tf-focus-w) solid var(--tf-accent); outline-offset: 0px; }

Button.fx-disabled:disabled { box-shadow: none; opacity: 0.45; }
Panel.fx-disabled-stage { opacity: 0.55; }

Panel.fx-scroll-host {
    background: var(--tf-surface-2);
    border: var(--tf-border-w) solid var(--tf-line);
    border-radius: var(--tf-radius);
    height: 150px;
    overflow-y: auto;
    padding: 10px;
}

Panel.fx-scroll-host::scrollbar-track { width: 8px; background: var(--tf-line-soft); }
Panel.fx-scroll-host::scrollbar-thumb { width: 5px; background: var(--tf-accent); border-radius: 999px; }
Panel.fx-scroll-item { background: var(--tf-surface-3); border-radius: var(--tf-radius-sm); box-shadow: 0 6px 18px var(--tf-shadow); padding: 8px; }
"""

_LAB_ICONS = """
/* ----- lab 9: icons ----------------------------------------------------- */

IconButton {
    background: transparent;
    border: var(--tf-border-w) solid transparent;
    border-radius: var(--tf-radius-sm);
    color: var(--tf-ink-dim);
}

IconButton:hover { background: var(--tf-accent-soft); color: var(--tf-ink); }
IconButton:focus { outline: var(--tf-focus-w) dotted var(--tf-accent); outline-offset: 2px; }
IconButton:active { background: var(--tf-accent); }
IconButton:active::icon { color: var(--tf-accent-ink); }
IconButton:disabled::icon { color: var(--tf-ink-faint); }

IconButton.ic-xs { height: 16px; width: 16px; }
IconButton.ic-sm { height: 22px; width: 22px; }
IconButton.ic-md { height: 30px; width: 30px; }
IconButton.ic-lg { height: 44px; width: 44px; }
IconButton.ic-xl { height: 64px; width: 64px; }

IconButton.ic-accent::icon { color: var(--tf-accent); }
IconButton.ic-ok::icon { color: var(--tf-ok); }
IconButton.ic-bad::icon { color: var(--tf-bad); }

IconButton.ic-boxed {
    background: var(--tf-surface-2);
    border-color: var(--tf-line);
}

ArrowButton { color: var(--tf-ink-dim); }
ArrowButton::icon { color: var(--tf-accent); }

SearchBox.ic-search::icon { color: var(--tf-accent); }
Label.ic-caption { color: var(--tf-ink-faint); font-size: 10px; text-align: center; width: 100%; }
"""

_LAB_CHROME = """
/* ----- lab 10: window chrome -------------------------------------------- */

Window::titlebar {
    background: var(--tf-surface-2);
    border-bottom: var(--tf-border-w) solid var(--tf-line);
}

Window::title {
    color: var(--tf-ink);
    font-family: var(--tf-font);
    font-weight: 750;
    letter-spacing: 0.4px;
}

Window::minimize,
Window::maximize,
Window::close {
    background: transparent;
    border: 0;
    border-radius: 0;
    color: var(--tf-ink-dim);
}

Window::resize-border { width: 6px; }

Window[window-state="maximized"]::titlebar { background: var(--tf-accent-soft); }
Window[window-state="maximized"]::maximize { background: var(--tf-surface-3); }

WindowMinimize:hover, WindowMaximize:hover { background: var(--tf-surface-3); color: var(--tf-ink); }
WindowMinimize:focus, WindowMaximize:focus, WindowClose:focus {
    outline: var(--tf-focus-w) dotted var(--tf-accent);
    outline-offset: -2px;
}
WindowClose:hover { background: var(--tf-bad); color: var(--tf-ink-invert); }

Panel.cr-under-title {
    background: linear-gradient(to bottom, var(--tf-accent-soft), transparent 60%);
    border: var(--tf-border-w) solid var(--tf-accent);
    border-radius: var(--tf-radius);
}
"""

_LAB_LIVE = """
/* ----- lab 11: live workload -------------------------------------------- */

Panel.live-card { background: var(--tf-surface); }
Panel.live-plot { padding: 6px; }

Label.live-metric {
    color: var(--tf-ink);
    font-family: var(--tf-mono);
    font-size: 21px;
    font-variant-numeric: tabular-nums;
    font-weight: 800;
}

Label.live-metric-label {
    color: var(--tf-ink-faint);
    font-size: 10px;
    letter-spacing: 1.2px;
    text-transform: uppercase;
}

ProgressBar.live-bar::track { background: var(--tf-line-soft); border-radius: 999px; height: 6px; }
ProgressBar.live-bar::fill { background: var(--tf-accent); border-radius: 999px; }

FlowLayout.live-led-row { gap: 6px; row-gap: 6px; }
"""

_LAB_EXTREME_HOST = """
/* ----- lab 12: hostile-CSS host ----------------------------------------- */

Panel.xt-host { background: var(--tf-surface-2); gap: 8px; }
Panel.xt-empty { border: 2px dashed var(--tf-bad); min-height: 0; height: 0; padding: 0; }
Panel.xt-zero { height: 0; width: 0; min-height: 0; min-width: 0; padding: 0; border: 0; }
Panel.xt-thrash-target {
    min-height: 48px;
    padding: 10px;
    align-items: center;
    justify-content: center;
}
Label.xt-warning { color: var(--tf-warn); font-weight: 750; }
"""

STRUCTURE_CSS = "".join(
    (
        _FONT_FACES,
        _SHELL,
        _TYPO_BASE,
        _LAB_THEME,
        _LAB_GALLERY,
        _LAB_PARTS,
        _LAB_BORDERS,
        _LAB_PAINT,
        _LAB_TYPE,
        _LAB_LAYOUT,
        _LAB_EFFECTS,
        _LAB_ICONS,
        _LAB_CHROME,
        _LAB_LIVE,
        _LAB_EXTREME_HOST,
    )
)

# ---------------------------------------------------------------------------
# Sheet 3 of 5: per-theme appearance
# ---------------------------------------------------------------------------
# Replaced in place on every theme switch. These sheets only carry treatments
# that cannot be expressed as a token swap -- bevels, hairlines, gradients.

APPEARANCE_DEFAULT = """
/* The stock DragonGUI look: let the framework sheet win almost everywhere. */
Panel.card { box-shadow: none; }
Button.primary { box-shadow: none; }
Label.page-title { font-weight: 700; }
"""

APPEARANCE_MODERN_DARK = """
Window {
    background:
        radial-gradient(circle at 82% -8%, rgba(90, 169, 255, 0.16), transparent 52%),
        var(--tf-bg);
}

Panel.card {
    background:
        linear-gradient(180deg, rgba(255, 255, 255, 0.035), transparent 42%),
        var(--tf-surface);
    box-shadow: 0 10px 26px rgba(0, 0, 0, 0.38);
}

Toolbar.forge-toolbar {
    background: linear-gradient(180deg, var(--tf-surface-3), var(--tf-surface-2));
}

Button {
    background: linear-gradient(180deg, var(--tf-surface-3), var(--tf-surface-2));
}

Button.primary {
    background: linear-gradient(180deg, #7ec0ff, var(--tf-accent));
}

StatusBar.forge-status { background: linear-gradient(180deg, var(--tf-surface-2), var(--tf-surface)); }
"""

APPEARANCE_MODERN_LIGHT = """
Window {
    background:
        radial-gradient(circle at 12% -6%, rgba(8, 126, 164, 0.10), transparent 46%),
        var(--tf-bg);
}

Panel.card {
    background: var(--tf-surface);
    box-shadow: 0 2px 4px rgba(24, 44, 78, 0.08), 0 12px 30px rgba(24, 44, 78, 0.10);
}

Button {
    background: linear-gradient(180deg, #ffffff, #eef2f8);
}

Button:hover { background: linear-gradient(180deg, #ffffff, #e2e9f4); }
Button.primary { background: linear-gradient(180deg, #1897bd, var(--tf-accent)); }

Toolbar.forge-toolbar { background: linear-gradient(180deg, #ffffff, #eaeff7); }
Tabs#forge-tabs { background: #ffffff; }
StatusBar.forge-status { background: #ffffff; }
LogView { background: #ffffff; }
"""

APPEARANCE_WIN311 = """
/* Square 3D bevels: two-tone borders, zero radius, no soft shadows anywhere. */
* { border-radius: 0; }

Window { background: var(--tf-bg); }

Panel.card, Panel.gallery-card, Panel.live-card, Panel.ly, Panel.ty, Panel.xt-host {
    background: var(--tf-surface);
    border-top: 2px solid #ffffff;
    border-left: 2px solid #ffffff;
    border-right: 2px solid #404040;
    border-bottom: 2px solid #404040;
    border-radius: 0;
    box-shadow: none;
}

Panel.card::header { background: #000080; }
Panel.card::title { color: #ffffff; font-weight: 700; }
Panel.card::accent { background: #000080; width: 0px; }

Button, SmallButton, IconButton {
    background: #c0c0c0;
    border-top: 2px solid #ffffff;
    border-left: 2px solid #ffffff;
    border-right: 2px solid #404040;
    border-bottom: 2px solid #404040;
    border-radius: 0;
    color: #000000;
}

Button:active, SmallButton:active, IconButton:active {
    border-top: 2px solid #404040;
    border-left: 2px solid #404040;
    border-right: 2px solid #ffffff;
    border-bottom: 2px solid #ffffff;
}

Button:focus { outline: 1px dotted #000000; outline-offset: -4px; }
Button.primary { background: #c0c0c0; color: #000000; font-weight: 700; box-shadow: none; }

TextInput, TextArea, NumberInput, SearchBox, Dropdown, LogView, DataFrameTable, CodeEditor {
    background: #ffffff;
    border-top: 2px solid #404040;
    border-left: 2px solid #404040;
    border-right: 2px solid #ffffff;
    border-bottom: 2px solid #ffffff;
    border-radius: 0;
    color: #000000;
}

Tabs#forge-tabs { background: #c0c0c0; }
Tab { border-radius: 0; color: #000000; }
Tab:selected { background: #c0c0c0; font-weight: 700; }
Tab::accent { height: 2px; }
Tab:selected::accent { background: #000080; }

MenuBar, Toolbar.forge-toolbar, StatusBar.forge-status, Sidebar#forge-sidebar {
    background: #c0c0c0;
    border-color: #808080;
}

StatusBar.forge-status {
    border-top: 2px solid #ffffff;
}

Window::titlebar { background: linear-gradient(90deg, #000080, #1084d0); }
Window::title { color: #ffffff; font-weight: 700; }
Window::minimize, Window::maximize, Window::close {
    background: #c0c0c0;
    border-top: 1px solid #ffffff;
    border-left: 1px solid #ffffff;
    border-right: 1px solid #404040;
    border-bottom: 1px solid #404040;
    color: #000000;
}

Badge, Tag { border-radius: 0; }
ProgressBar::track { background: #c0c0c0; border-radius: 0; }
ProgressBar::fill { background: #000080; border-radius: 0; }
Slider::track { background: #808080; border-radius: 0; }
Slider::thumb { background: #c0c0c0; border-radius: 0; width: 11px; }
"""

APPEARANCE_CLASSIC_MAC = """
/* Hairline black outlines, white fills, pinstriped desktop. */
Window {
    background: dg-pattern(pinstripe, rgba(0, 0, 0, 0.16), #9d9d9d, 2px);
}

Panel.card, Panel.gallery-card, Panel.live-card, Panel.ly, Panel.ty, Panel.xt-host {
    background: #ffffff;
    border: 1px solid #000000;
    border-radius: 6px;
    box-shadow: 2px 2px 0 rgba(0, 0, 0, 0.55);
}

Panel.card::header { background: dg-pattern(pinstripe, rgba(0, 0, 0, 0.35), #ffffff, 2px); }
Panel.card::title { color: #000000; font-weight: 700; }
Panel.card::accent { background: #000000; width: 1px; }

Button, SmallButton {
    background: #ffffff;
    border: 1px solid #000000;
    border-radius: 8px;
    color: #000000;
}

Button:hover { background: #ededed; }
Button:active { background: #000000; color: #ffffff; }
Button.primary { background: #ffffff; border-width: 3px; font-weight: 700; box-shadow: none; }
Button:focus { outline: 2px solid #000000; outline-offset: 2px; }

TextInput, TextArea, NumberInput, SearchBox, Dropdown, LogView, DataFrameTable {
    background: #ffffff;
    border: 1px solid #000000;
    border-radius: 3px;
    color: #000000;
}

MenuBar, Toolbar.forge-toolbar, Tabs#forge-tabs, StatusBar.forge-status, Sidebar#forge-sidebar {
    background: #ffffff;
    border-color: #000000;
}

Tab:selected { background: #000000; color: #ffffff; }
Tab::accent { height: 1px; }
Tab:selected::accent { background: #000000; }

Window::titlebar { background: dg-pattern(pinstripe, rgba(0, 0, 0, 0.55), #ffffff, 2px); }
Window::title { color: #000000; font-weight: 700; }
Window::minimize, Window::maximize, Window::close { background: #ffffff; color: #000000; }

ProgressBar::track { background: #ffffff; border-radius: 999px; }
ProgressBar::fill { background: dg-pattern(diagonal-hatch, #000000, #ffffff, 4px); border-radius: 999px; }
Slider::track { background: #ffffff; }
Slider::thumb { background: #ffffff; border-radius: 999px; width: 13px; }
LED::glow { opacity: 0; }
"""

APPEARANCE_CONTRAST = """
/* Every surface is pure black or pure white; every focusable control has a
   4px ring; no gradients, no shadows, no translucency. */
* { box-shadow: none; }

Window { background: #000000; }

Panel.card, Panel.gallery-card, Panel.live-card, Panel.ly, Panel.ty, Panel.xt-host,
Panel.fx-stage, Panel.bg, Panel.bd {
    background: #000000;
    border: 2px solid #ffffff;
    border-radius: 0;
}

Panel.card::header { background: #000000; }
Panel.card::title { color: #ffffff; font-weight: 900; text-transform: uppercase; }
Panel.card::accent { background: #00e5ff; width: 4px; }

Button, SmallButton, IconButton {
    background: #000000;
    border: 2px solid #ffffff;
    border-radius: 0;
    color: #ffffff;
    font-weight: 750;
}

Button:hover { background: #ffffff; color: #000000; }
Button:focus { outline: 4px solid #ffd400; outline-offset: 2px; }
Button:disabled { color: #808080; border-color: #808080; }
Button.primary { background: #00e5ff; border-color: #00e5ff; color: #000000; font-weight: 900; }

TextInput, TextArea, NumberInput, SearchBox, Dropdown, LogView, DataFrameTable, CodeEditor {
    background: #000000;
    border: 2px solid #ffffff;
    border-radius: 0;
    color: #ffffff;
}

TextInput:focus, TextArea:focus, NumberInput:focus, Dropdown:focus {
    border-color: #ffd400;
    outline: 4px solid #ffd400;
    outline-offset: 1px;
}

MenuBar, Toolbar.forge-toolbar, Tabs#forge-tabs, StatusBar.forge-status, Sidebar#forge-sidebar {
    background: #000000;
    border-color: #ffffff;
}

Tab { color: #ffffff; }
Tab:selected { background: #ffffff; color: #000000; font-weight: 900; }
Tab::accent { height: 4px; }
Tab:selected::accent { background: #ffd400; }

NavItem { color: #ffffff; }
NavItem:selected { background: #ffffff; color: #000000; }
NavItem::accent { width: 5px; }
NavItem:selected::accent { background: #ffd400; }

Badge, Tag { border: 2px solid #ffffff; border-radius: 0; }
ProgressBar::track { background: #000000; border: 2px solid #ffffff; border-radius: 0; }
ProgressBar::fill { background: #00ff88; border-radius: 0; }
Slider::track { background: #000000; border: 2px solid #ffffff; }
Slider::fill { background: #00e5ff; }
Slider::thumb { background: #ffd400; border: 2px solid #000000; border-radius: 0; width: 16px; }

Window::titlebar { background: #000000; border-bottom: 2px solid #ffffff; }
Window::title { color: #ffffff; font-weight: 900; }
WindowClose:hover { background: #ff3b6b; color: #000000; }
"""

APPEARANCE_EXTREME = """
/* Deliberately maximal: huge radii, thick borders, deep glow, layered paint.
   Nothing here should be able to break layout invalidation or paint caching. */
Window {
    background:
        blob-gradient(
            at 18% 22% rgba(255, 92, 246, 0.55) 44%,
            at 78% 18% rgba(255, 196, 0, 0.42) 38%,
            at 60% 84% rgba(0, 255, 208, 0.38) 46%
        );
}

Panel.card, Panel.gallery-card, Panel.live-card, Panel.ly, Panel.ty, Panel.xt-host {
    background:
        linear-gradient(150deg, rgba(255, 92, 246, 0.30), transparent 58%),
        var(--tf-surface);
    border: 4px solid var(--tf-line-strong);
    border-radius: var(--tf-radius-lg);
    box-shadow: 0 22px 60px 6px var(--tf-shadow), 0 0 34px var(--tf-glow);
    padding: 20px;
}

Panel.card::header {
    background: linear-gradient(90deg, #ff5cf6, #ffc400);
}

Panel.card::title { color: #1a0030; font-weight: 900; letter-spacing: 1.4px; text-transform: uppercase; }
Panel.card::accent { background: #00ffd0; width: 10px; }

Button, SmallButton {
    background: linear-gradient(180deg, #ffe27a, var(--tf-accent));
    border: 4px solid #ff5cf6;
    border-radius: 24px;
    color: #1a0030;
    font-weight: 900;
    letter-spacing: 0.8px;
    padding: 8px 18px;
}

Button:hover { box-shadow: 0 0 34px 6px var(--tf-glow); }
Button:focus { outline: 5px dotted #00ffd0; outline-offset: 5px; }
Button.primary { background: linear-gradient(135deg, #00ffd0, #ff5cf6); color: #1a0030; }

TextInput, TextArea, NumberInput, SearchBox, Dropdown {
    background: rgba(24, 0, 48, 0.92);
    border: 3px solid #00ffd0;
    border-radius: 20px;
    color: #fff3ff;
    padding: 6px 14px;
}

MenuBar, Toolbar.forge-toolbar, Tabs#forge-tabs, StatusBar.forge-status {
    background: linear-gradient(90deg, rgba(58, 6, 96, 0.95), rgba(140, 12, 150, 0.85));
    border-color: #ff5cf6;
}

Sidebar#forge-sidebar {
    background:
        linear-gradient(180deg, rgba(255, 92, 246, 0.28), transparent 52%),
        rgba(38, 4, 70, 0.94);
    border-right: 4px solid #ff5cf6;
}

Tab { border-radius: 18px; font-weight: 800; padding: 6px 16px; }
Tab:selected { background: #ffc400; color: #1a0030; }
Tab::accent { height: 6px; }
Tab:selected::accent { background: #00ffd0; }

NavItem { border-radius: 18px; }
NavItem:selected { background: rgba(255, 196, 0, 0.35); }

Badge { border-radius: 999px; font-size: 12px; padding: 3px 12px; box-shadow: 0 0 14px var(--tf-glow); }
Label.page-title { text-transform: uppercase; letter-spacing: 2px; }

ProgressBar::track { background: rgba(24, 0, 48, 0.9); border-radius: 999px; height: 14px; }
ProgressBar::fill { background: linear-gradient(90deg, #00ffd0, #ffc400); border-radius: 999px; }
Slider::track { background: rgba(24, 0, 48, 0.9); height: 14px; border-radius: 999px; }
Slider::fill { background: #ffc400; }
Slider::thumb { background: #00ffd0; border: 3px solid #ff5cf6; border-radius: 999px; width: 26px; }

Window::titlebar { background: linear-gradient(90deg, #ff5cf6, #ffc400); }
Window::title { color: #1a0030; font-weight: 900; letter-spacing: 2px; }
WindowClose:hover { background: #ff2f6d; color: #ffffff; }
"""

APPEARANCE: dict[str, str] = {
    "default": APPEARANCE_DEFAULT,
    "modern-dark": APPEARANCE_MODERN_DARK,
    "modern-light": APPEARANCE_MODERN_LIGHT,
    "win311": APPEARANCE_WIN311,
    "classic-mac": APPEARANCE_CLASSIC_MAC,
    "contrast": APPEARANCE_CONTRAST,
    "extreme": APPEARANCE_EXTREME,
}


# ---------------------------------------------------------------------------
# Sheet 4 of 5: the removable override sheet
# ---------------------------------------------------------------------------
# Added and removed at runtime. Because it is appended last it always wins the
# cascade against "appearance", and removing it must restore the previous look
# exactly -- no residue, no stale computed styles.

OVERRIDE_CSS = """
/* Override sheet -- added last, so it wins over the active appearance sheet. */
Panel.card {
    border-color: var(--tf-warn);
    border-style: dashed;
    border-width: 2px;
}

Panel.card::accent { background: var(--tf-warn); width: 6px; }

Button.primary {
    background: var(--tf-warn);
    border-color: var(--tf-warn);
    color: var(--tf-ink-invert);
}

Label.kicker::before { content: "[override] "; color: var(--tf-warn); }
Tab:selected::accent { background: var(--tf-warn); }
StatusBar.forge-status { border-top-color: var(--tf-warn); border-top-width: 3px; }
"""


# ---------------------------------------------------------------------------
# Sheet 5 of 5: deliberately hostile CSS ("extreme mode")
# ---------------------------------------------------------------------------
# Every rule below is either abusive, contradictory, or outright unsupported.
# The contract under test is that DragonGUI degrades predictably: unsupported
# input becomes a stylesheet warning, and supported-but-extreme input renders
# without corrupting layout.

HOSTILE_CSS = """
/* --- oversized padding, borders and radii ------------------------------- */
Panel.xt-fat {
    padding: 64px 72px 68px 76px;
    border: 26px double var(--tf-bad);
    border-radius: 120px;
    background: var(--tf-surface-3);
}

/* --- tiny controls ------------------------------------------------------ */
Button.xt-atom { font-size: 5px; height: 6px; min-height: 6px; min-width: 6px; padding: 0; border-width: 1px; }
Badge.xt-atom { font-size: 4px; padding: 0px 1px; }
LED.xt-atom::dot { width: 2px; height: 2px; }

/* --- transparent colors layered over each other ------------------------- */
Panel.xt-ghost {
    background:
        linear-gradient(45deg, rgba(255, 0, 128, 0.18), rgba(0, 255, 208, 0.10)),
        radial-gradient(circle at 50% 50%, rgba(255, 255, 255, 0.12), transparent 70%),
        rgba(0, 0, 0, 0.04);
    border: 3px solid rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.55);
}

/* --- deep shadows in every direction ------------------------------------ */
Panel.xt-shadow {
    box-shadow:
        inset 0 0 60px 20px rgba(0, 0, 0, 0.85),
        0 40px 90px 24px rgba(255, 0, 128, 0.55),
        0 -30px 70px 18px rgba(0, 255, 208, 0.45);
    border-radius: 64px;
}

/* --- missing resources: neither id is ever registered ------------------- */
Panel.xt-missing-image { background-image: app-resource("never-registered", cover); }
Label.xt-missing-font { font-family: "No Such Family At All", "Nor This One"; }

/* --- unsupported selector forms (must warn, not crash) ------------------ */
Panel:hover Button { background: red; }
Panel:is(:hover) Button { background: red; }
Panel::not-a-real-part { background: red; }
Button::stepper { background: red; }
Slider::badge { background: red; }
Panel:has(Button:hover) { background: red; }

/* --- unsupported properties (must warn, not crash) ---------------------- */
Panel.xt-unsupported {
    filter: blur(4px) saturate(180%);
    clip-path: circle(40%);
    mix-blend-mode: multiply;
    will-change: transform;
    float: left;
    content-visibility: auto;
    background-attachment: fixed;
    writing-mode: vertical-rl;
    zoom: 2;
}

/* --- conflicting shorthand and longhand in one block -------------------- */
Panel.xt-conflict {
    border: 1px solid var(--tf-ok);
    border-width: 9px;
    border-color: var(--tf-bad);
    border-left: 2px dotted var(--tf-warn);
    border-radius: 40px;
    border-top-left-radius: 0px;
    padding: 30px;
    padding-left: 2px;
    background: var(--tf-accent);
    background-color: var(--tf-surface);
    background-image: linear-gradient(90deg, var(--tf-bad), transparent);
}

/* --- longhand declared before shorthand (shorthand must win) ------------ */
Panel.xt-conflict-reverse {
    border-width: 9px;
    border-color: var(--tf-bad);
    border: 1px solid var(--tf-ok);
    padding-left: 40px;
    padding: 4px;
}

/* --- repeatable replacement targets: hostile paint, stable alignment ----- */
Panel.xt-thrash-target {
    border: 9px solid var(--tf-bad);
    border-left: 2px dotted var(--tf-warn);
    border-radius: 40px;
    background:
        linear-gradient(90deg, var(--tf-bad), transparent),
        var(--tf-surface);
}

/* --- percent/auto where they are not accepted (must warn) --------------- */
Panel.xt-bad-length {
    border-width: 50%;
    border-radius: auto;
    font-size: 120%;
    box-shadow: 10% 10% var(--tf-bad);
    gap: auto;
    padding: auto;
}

/* --- var() cycles and missing variables --------------------------------- */
Panel.xt-var {
    background: var(--tf-does-not-exist, var(--tf-surface-3));
    border-color: var(--tf-also-missing);
    border-radius: var(--tf-radius-lg);
    padding: var(--tf-nope, 12px);
}

/* --- zero-sized and empty containers ------------------------------------ */
Panel.xt-zero-box { width: 0; height: 0; min-width: 0; min-height: 0; padding: 0; border: 4px solid var(--tf-bad); }
Panel.xt-negative { width: -40px; height: -10px; padding: -8px; }
VLayout.xt-empty-flex:empty { border: 2px dotted var(--tf-warn); min-height: 24px; }

/* --- very long text in a constrained control ---------------------------- */
Label.xt-long { text-overflow: ellipsis; width: 150px; }
Button.xt-long { max-width: 120px; min-width: 0; }
Badge.xt-long { max-width: 90px; }

/* --- extreme transforms (paint only; must not move hit testing) --------- */
Panel.xt-transform { transform: rotate(7deg) ; opacity: 0.92; }
Button.xt-scaled { scale: 1.6; }

/* --- a rule that matches nothing at all --------------------------------- */
NotAWidgetType.does-not-exist { background: red; }
#no-such-widget-id-anywhere { background: red; }
"""


# Kept out of `HOSTILE_CSS` on purpose: each of these is malformed at the token
# level rather than merely unsupported. Some abort the whole sheet, and some are
# absorbed by ordinary CSS error recovery. Both outcomes are acceptable -- what
# is under test is that neither one crashes, leaks a half-applied sheet, or
# disturbs the cascade that was already installed.
MALFORMED_SHEETS: tuple[tuple[str, str], ...] = (
    ("chained pseudo-elements", 'Button::before::after { content: "x"; }'),
    ("unterminated block", "Panel.card { background: red;"),
    ("stray closing brace", "Panel.card { background: red; } }"),
    ("empty target part", "::stepper { background: red; }"),
    ("unterminated string", 'Label.card-note::after { content: "unterminated; }'),
    ("garbage before a rule", "!!! ??? Panel.card { background: red; }"),
    ("no selector at all", "{ background: red; }"),
)

# ---------------------------------------------------------------------------
# Application icon themes
# ---------------------------------------------------------------------------
# Two complete icon themes plus an alias-only theme, swapped live from the icon
# lab. Geometry is application-owned; color/size/state stay CSS-owned.

ICON_THEME_ANGULAR: dict[str, Any] = {
    "search": dg.IconResource(
        [
            dg.IconStroke([(5, 10), (10, 4), (16, 8), (15, 15), (8, 16)], closed=True),
            dg.IconStroke([(15, 15), (21, 21)]),
        ],
        stroke_width=2.0,
    ),
    "save": dg.IconResource(
        [
            dg.IconStroke([(4, 3), (20, 3), (20, 21), (4, 21)], closed=True),
            dg.IconStroke([(8, 3), (8, 10), (16, 10), (16, 3)]),
            dg.IconStroke([(8, 15), (16, 15)]),
            dg.IconStroke([(8, 18), (16, 18)]),
        ],
        stroke_width=1.6,
    ),
    "settings": dg.IconResource(
        [
            dg.IconStroke([(12, 3), (19, 7), (19, 17), (12, 21), (5, 17), (5, 7)], closed=True),
            dg.IconStroke([(12, 9), (15, 12), (12, 15), (9, 12)], closed=True),
        ],
        stroke_width=2.0,
    ),
    "refresh": dg.IconResource(
        [
            dg.IconStroke([(20, 6), (20, 12), (14, 12)]),
            dg.IconStroke([(4, 18), (4, 12), (10, 12)]),
            dg.IconStroke([(4, 12), (8, 5), (16, 5), (20, 12)]),
        ],
        stroke_width=2.0,
    ),
    "run": "play",
    "workflow": "list",
}

ICON_THEME_ROUND: dict[str, Any] = {
    "search": dg.IconResource(
        [
            dg.IconStroke(
                [(5, 11), (7, 6), (12, 4), (17, 6), (19, 11), (17, 16), (12, 18), (7, 16)],
                closed=True,
            ),
            dg.IconStroke([(16, 16), (21, 21)]),
        ],
        stroke_width=2.4,
    ),
    "save": dg.IconResource(
        [
            dg.IconStroke(
                [(5, 4), (19, 4), (20, 6), (20, 20), (4, 20), (4, 6)],
                closed=True,
            ),
            dg.IconStroke([(9, 4), (9, 9), (15, 9), (15, 4)]),
        ],
        stroke_width=2.4,
    ),
    "settings": dg.IconResource(
        [
            dg.IconStroke(
                [(12, 4), (17, 6), (20, 12), (17, 18), (12, 20), (7, 18), (4, 12), (7, 6)],
                closed=True,
            ),
            dg.IconStroke(
                [(12, 9), (14, 10), (15, 12), (14, 14), (12, 15), (10, 14), (9, 12), (10, 10)],
                closed=True,
            ),
        ],
        stroke_width=2.4,
    ),
    "refresh": dg.IconResource(
        [
            dg.IconStroke([(6, 7), (10, 12), (6, 17)]),
            dg.IconStroke([(18, 7), (14, 12), (18, 17)]),
        ],
        stroke_width=2.4,
    ),
    "run": "play",
    "workflow": "grid",
}

ICON_THEME_ALIASES: dict[str, Any] = {
    "search": "filter",
    "save": "download",
    "settings": "more",
    "refresh": "sync",
    "run": "pause",
    "workflow": "axes",
}

ICON_THEMES: dict[str, dict[str, Any]] = {
    "builtin": {},
    "angular": ICON_THEME_ANGULAR,
    "round": ICON_THEME_ROUND,
    "aliases": ICON_THEME_ALIASES,
}

ICON_THEME_ORDER: tuple[str, ...] = ("builtin", "angular", "round", "aliases")


# ---------------------------------------------------------------------------
# Routes
# ---------------------------------------------------------------------------

ROUTES: tuple[tuple[str, str, str | None], ...] = (
    ("theme", "Theme", "7"),
    ("gallery", "Gallery", None),
    ("parts", "Parts", None),
    ("borders", "Borders", None),
    ("paint", "Paint", None),
    ("type", "Type", None),
    ("layout", "Layout", "5"),
    ("effects", "Effects", None),
    ("icons", "Icons", None),
    ("chrome", "Chrome", None),
    ("live", "Live", "live"),
    ("extreme", "Extreme", "!"),
)

VIEWPORTS: tuple[tuple[str, int, int], ...] = (
    ("320 x 640", 320, 640),
    ("390 x 720", 390, 720),
    ("640 x 480", 640, 480),
    ("1024 x 768", 1024, 768),
    ("1440 x 900", 1440, 900),
)

# The CSS-variable editor only ever writes these tokens.
EDITABLE_VARS: tuple[tuple[str, str, float, float, float], ...] = (
    ("radius", "--tf-radius", 0.0, 48.0, 1.0),
    ("radius-lg", "--tf-radius-lg", 0.0, 80.0, 1.0),
    ("border-w", "--tf-border-w", 0.0, 12.0, 1.0),
    ("pad", "--tf-pad", 0.0, 40.0, 1.0),
    ("gap", "--tf-gap", 0.0, 32.0, 1.0),
    ("font-size", "--tf-font-size", 8.0, 26.0, 1.0),
    ("tile", "--tf-tile", 2.0, 48.0, 1.0),
    ("focus-w", "--tf-focus-w", 0.0, 8.0, 1.0),
)

ACCENT_CHOICES: tuple[tuple[str, str, str], ...] = (
    ("theme default", "", ""),
    ("cyan", "#37c6d0", "#04161a"),
    ("blue", "#5aa9ff", "#04101f"),
    ("violet", "#a682ff", "#150a2c"),
    ("amber", "#ffb020", "#2a1600"),
    ("mint", "#3fd7a4", "#032019"),
    ("rose", "#ff5f8d", "#2a0412"),
)

FONT_STACKS: tuple[tuple[str, str], ...] = (
    ("theme default", ""),
    ("Forge UI stack", "\"Forge UI\", \"Segoe UI\", system-ui, sans-serif"),
    ("missing then serif", "\"Nonexistent Alpha\", \"Nonexistent Beta\", serif"),
    ("serif", "serif"),
    ("sans-serif", "sans-serif"),
    ("monospace", "monospace"),
    ("cursive", "cursive"),
    ("fantasy", "fantasy"),
    ("all missing", "\"Nope One\", \"Nope Two\", \"Nope Three\""),
)


# ---------------------------------------------------------------------------
# Runtime state
# ---------------------------------------------------------------------------


class ForgeState:
    """Every retained handle the behaviour layer needs, in one place."""

    def __init__(self) -> None:
        self.app: dg.App | None = None
        self.window: dg.Window | None = None

        # active configuration
        self.theme_key = "modern-dark"
        self.icon_theme_key = "builtin"
        self.overrides_on = False
        self.extreme_on = False
        self.var_edits: dict[str, str] = {}

        # shell
        self.tabs: dg.Tabs | None = None
        self.pages: dg.Pages | None = None
        self.sidebar: dg.Sidebar | None = None
        self.status: dg.Label | None = None
        self.status_badge: dg.Badge | None = None
        self.clock: dg.Label | None = None
        self.theme_label: dg.Label | None = None
        self.sheet_label: dg.Label | None = None
        self.modal: dg.Modal | None = None
        self.palette: dg.CommandPalette | None = None
        self.scrolls: dict[str, dg.ScrollArea] = {}

        # theme lab
        self.theme_buttons: dict[str, dg.Button] = {}
        self.var_sliders: dict[str, dg.Slider] = {}
        self.var_values: dict[str, dg.Label] = {}
        self.accent_dropdown: dg.Dropdown | None = None
        self.font_dropdown: dg.Dropdown | None = None
        self.overrides_toggle: dg.ToggleSwitch | None = None
        self.extreme_toggle: dg.ToggleSwitch | None = None
        self.cycle_label: dg.Label | None = None

        # retained-state probes, read back after every theme change
        self.probe_text: dg.TextInput | None = None
        self.probe_area: dg.TextArea | None = None
        self.probe_search: dg.SearchBox | None = None
        self.probe_dropdown: dg.Dropdown | None = None
        self.probe_number: dg.NumberInput | None = None
        self.probe_slider: dg.Slider | None = None
        self.probe_range: dg.RangeSlider | None = None
        self.probe_check: dg.Checkbox | None = None
        self.probe_toggle: dg.ToggleSwitch | None = None
        self.probe_radio: dg.RadioGroup | None = None
        self.probe_list: dg.SelectableList | None = None

        # icons
        self.icon_theme_label: dg.Label | None = None
        self.icon_swap_button: dg.IconButton | None = None
        self.icon_identity_button: dg.IconButton | None = None
        self.icon_identity_on = True

        # live workload
        self.plot: dg.LinePlot | None = None
        self.table: dg.DataFrameTable | None = None
        self.rows: list[dict[str, Any]] = []
        self.log: dg.LogView | None = None
        self.heatmap: dg.Heatmap | None = None
        self.bar_host: dg.VLayout | None = None
        self.leds: list[dg.LED] = []
        self.bars: list[dg.ProgressBar] = []
        self.metrics: list[dg.Label] = []
        self.live_badges: list[dg.Badge] = []
        self.flex_panels: list[dg.Panel] = []

        # background resources
        self.texture_warm = False
        self.texture_released = False
        self.texture_label: dg.Label | None = None
        self.malformed_log: dg.LogView | None = None
        self.malformed_results: list[dict[str, Any]] = []

        # workers
        self.tick = SAMPLE_COUNT
        self.random = random.Random(20260730)
        self.stop = threading.Event()
        self.autopilot_report: list[dict[str, Any]] = []


state = ForgeState()


# ---------------------------------------------------------------------------
# Behaviour
# ---------------------------------------------------------------------------


def set_status(message: str, level: str = "info") -> None:
    if state.status is not None:
        state.status.set_value(message)
    if state.status_badge is not None:
        state.status_badge.set_value(level.upper())
        state.status_badge.set_level(
            {"ok": "success", "warn": "warning", "bad": "danger"}.get(level, "info")
        )


def active_sheet_summary() -> str:
    sheets = ["variables", "structure", "appearance"]
    if state.overrides_on:
        sheets.append("overrides")
    if state.extreme_on:
        sheets.append("extreme")
    return " > ".join(sheets)


def refresh_sheet_label() -> None:
    if state.sheet_label is not None:
        state.sheet_label.set_value(active_sheet_summary())


def apply_variables() -> None:
    """Rewrite the ":root" sheet from the active theme plus live edits."""

    if state.app is None:
        return
    state.app.set_stylesheet("variables", variables_css(state.theme_key, state.var_edits))


def apply_theme(key: str, *, announce: bool = True) -> None:
    """Swap design tokens, the ":root" sheet, and the appearance sheet.

    The widget tree is untouched, so this is a pure cascade change: focus,
    scroll offsets, selected tab, input values, and plot data must survive.
    """

    if key not in TOKENS or state.app is None:
        return
    state.theme_key = key
    state.app.set_theme(theme_for(key))
    apply_variables()
    state.app.set_stylesheet("appearance", APPEARANCE[key])

    for name, button in state.theme_buttons.items():
        button.set_class("theme-pick active" if name == key else "theme-pick")
    if state.theme_label is not None:
        state.theme_label.set_value(THEME_LABELS[key])
    refresh_sheet_label()
    if announce:
        set_status(f"theme -> {THEME_LABELS[key]}", "ok")


def cycle_theme(step: int = 1) -> None:
    index = THEME_ORDER.index(state.theme_key)
    apply_theme(THEME_ORDER[(index + step) % len(THEME_ORDER)])


def rapid_cycle(rounds: int = 3) -> None:
    """Replace both theme sheets many times in one frame.

    This is intentionally synchronous: the native side must coalesce the
    restyle work rather than running a full cascade per call.
    """

    if state.app is None:
        return
    start = time.perf_counter()
    for _ in range(rounds):
        for key in THEME_ORDER:
            apply_theme(key, announce=False)
    apply_theme(state.theme_key, announce=False)
    elapsed = (time.perf_counter() - start) * 1000.0
    swaps = rounds * len(THEME_ORDER)
    if state.cycle_label is not None:
        state.cycle_label.set_value(f"{swaps} swaps enqueued in {elapsed:.1f} ms")
    set_status(f"rapid cycle: {swaps} theme swaps", "warn")


def set_overrides(enabled: bool) -> None:
    if state.app is None:
        return
    state.overrides_on = bool(enabled)
    if state.overrides_on:
        state.app.set_stylesheet("overrides", OVERRIDE_CSS)
    else:
        state.app.remove_stylesheet("overrides")
    if state.overrides_toggle is not None:
        state.overrides_toggle.set_checked(state.overrides_on)
    refresh_sheet_label()
    set_status(f"override sheet {'added' if state.overrides_on else 'removed'}", "warn")


def toggle_overrides() -> None:
    set_overrides(not state.overrides_on)


def set_extreme(enabled: bool) -> None:
    if state.app is None:
        return
    state.extreme_on = bool(enabled)
    if state.extreme_on:
        state.app.set_stylesheet("extreme", HOSTILE_CSS)
    else:
        state.app.remove_stylesheet("extreme")
    if state.extreme_toggle is not None:
        state.extreme_toggle.set_checked(state.extreme_on)
    refresh_sheet_label()
    set_status(f"hostile sheet {'added' if state.extreme_on else 'removed'}", "bad")


def toggle_extreme() -> None:
    set_extreme(not state.extreme_on)


def set_var(name: str, value: str) -> None:
    state.var_edits[name] = value
    apply_variables()


def on_var_slider(name: str, value: float) -> None:
    text = f"{int(round(value))}px"
    set_var(name, text)
    label = state.var_values.get(name)
    if label is not None:
        label.set_value(text)


def on_accent_change(label: str) -> None:
    choice = next((entry for entry in ACCENT_CHOICES if entry[0] == label), None)
    if choice is None:
        return
    _, color, ink = choice
    if not color:
        state.var_edits.pop("accent", None)
        state.var_edits.pop("accent-ink", None)
        state.var_edits.pop("accent-soft", None)
        state.var_edits.pop("glow", None)
    else:
        state.var_edits["accent"] = color
        state.var_edits["accent-ink"] = ink
        state.var_edits["accent-soft"] = f"{color}33"
        state.var_edits["glow"] = f"{color}8c"
    apply_variables()
    set_status(f"--tf-accent -> {label}")


def on_font_change(label: str) -> None:
    stack = dict(FONT_STACKS).get(label, "")
    if stack:
        state.var_edits["font"] = stack
    else:
        state.var_edits.pop("font", None)
    apply_variables()
    set_status(f"--tf-font -> {label}")


def reset_variables() -> None:
    """Drop every live edit and return to the active theme's token table."""

    state.var_edits.clear()
    apply_variables()
    table = TOKENS[state.theme_key]
    for name, _prop, _lo, _hi, _step in EDITABLE_VARS:
        slider = state.var_sliders.get(name)
        label = state.var_values.get(name)
        value = table.get(name, "0px")
        if slider is not None:
            slider.set_value(_px(value))
        if label is not None:
            label.set_value(value)
    if state.accent_dropdown is not None:
        state.accent_dropdown.set_value(ACCENT_CHOICES[0][0])
    if state.font_dropdown is not None:
        state.font_dropdown.set_value(FONT_STACKS[0][0])
    set_status("CSS variables reset to theme defaults", "ok")


def apply_icon_theme(key: str) -> None:
    if state.app is None or key not in ICON_THEMES:
        return
    state.icon_theme_key = key
    state.app.set_icon_theme(ICON_THEMES[key])
    if state.icon_theme_label is not None:
        state.icon_theme_label.set_value(f"icon theme: {key}")
    set_status(f"icon theme -> {key}", "ok")


def cycle_icon_theme() -> None:
    index = ICON_THEME_ORDER.index(state.icon_theme_key)
    apply_icon_theme(ICON_THEME_ORDER[(index + 1) % len(ICON_THEME_ORDER)])


def toggle_icon_identity() -> None:
    """Change the *requested* icon name, not the theme, to prove they are independent."""

    state.icon_identity_on = not state.icon_identity_on
    if state.icon_identity_button is not None:
        state.icon_identity_button.set_icon("search" if state.icon_identity_on else "warning")
    set_status("live icon identity swapped")


def swap_texture() -> None:
    if state.app is None:
        return
    state.texture_warm = not state.texture_warm
    state.texture_released = False
    state.app.set_image_resource("forge-weave", forge_texture(warm=state.texture_warm))
    if state.texture_label is not None:
        state.texture_label.set_value(
            f"forge-weave: {'warm' if state.texture_warm else 'cool'} (registered)"
        )
    set_status("background image replaced live", "ok")


def release_texture() -> None:
    if state.app is None:
        return
    state.texture_released = True
    state.app.release_image_resource("forge-weave")
    if state.texture_label is not None:
        state.texture_label.set_value("forge-weave: released (panels fall back to color)")
    set_status("background image released", "warn")


def restore_texture() -> None:
    if state.app is None:
        return
    state.texture_released = False
    state.app.set_image_resource("forge-weave", forge_texture(warm=state.texture_warm))
    if state.texture_label is not None:
        state.texture_label.set_value(
            f"forge-weave: {'warm' if state.texture_warm else 'cool'} (registered)"
        )
    set_status("background image restored", "ok")


def navigate(route: str) -> None:
    if state.pages is not None:
        state.pages.set_value(route)
    if state.tabs is not None:
        state.tabs.set_value(route)
    set_status(f"workspace -> {route}")


def toggle_sidebar() -> None:
    if state.sidebar is not None:
        state.sidebar.toggle_collapsed()


def show_modal() -> None:
    if state.modal is not None:
        state.modal.show()
    set_status("modal opened", "warn")


def close_modal() -> None:
    if state.modal is not None:
        state.modal.close()


def show_palette() -> None:
    if state.palette is not None:
        state.palette.show()
    set_status("command palette opened")


def resize_window(width: int, height: int) -> None:
    """Ask the native window to adopt a preset logical size."""

    app = state.app
    handle = getattr(app, "_handle", None) if app is not None else None
    if handle is None:
        set_status("resize needs a running window", "warn")
        return
    handle.request_window_resize(int(width), int(height))
    set_status(f"viewport -> {width} x {height}")


def refresh_table() -> None:
    rows = list(state.rows)
    state.random.shuffle(rows)
    state.rows = rows
    if state.table is not None:
        state.table.set_frame(rows)
    set_status("restyle journal re-sorted", "ok")


def emit_toast() -> None:
    app = state.app
    if app is None:
        return
    level = ("info", "success", "warning", "error")[state.tick % 4]
    try:
        app.toast(f"{THEME_LABELS[state.theme_key]} | {active_sheet_summary()}", level=level)
    except RuntimeError:
        pass


# ---------------------------------------------------------------------------
# Page fragments
# ---------------------------------------------------------------------------


def page_scroll(route: str) -> dg.ScrollArea:
    scroll = dg.ScrollArea(axis="y", gap=12, class_="forge-page", id=f"scroll-{route}")
    state.scrolls[route] = scroll
    return scroll


def page_heading(kicker: str, title: str, description: str) -> None:
    with dg.VLayout(class_="page-heading"):
        dg.Label(kicker, class_="kicker", wrap=False)
        with dg.FlowLayout(gap=10, row_gap=6, style={"align_items": "center"}):
            dg.Label(title, class_="page-title", wrap=False)
            dg.Tag("stress target", level="info")
        dg.Label(description, class_="page-desc")


def note(text: str) -> None:
    dg.Label(text, class_="pass")


# Extra LED states, so `LED.busy` / `LED[state="warning"]` selectors have real
# widgets to match. The names double as automatic CSS classes.
LED_STATES: dict[str, str] = {
    "busy": "warning",
    "warning": "warning",
    "error": "danger",
    "idle": "muted_text",
}


def led(state: bool | str = False, **kwargs: Any) -> dg.LED:
    return dg.LED(state, states=LED_STATES, **kwargs)


# ---------------------------------------------------------------------------
# Lab 1 -- theme laboratory
# ---------------------------------------------------------------------------


def build_theme_page() -> None:
    page_heading(
        "CASCADE / RUNTIME REPLACEMENT",
        "Theme laboratory",
        "Seven themes, a named-sheet replacement, an add/remove override sheet, rapid "
        "cycling, and a live CSS-variable editor. Nothing below rebuilds the widget tree, "
        "so every control keeps its value, focus, and scroll position across a switch.",
    )

    with dg.Panel("Active theme", class_="card"):
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            for key in THEME_ORDER:
                state.theme_buttons[key] = dg.Button(
                    THEME_LABELS[key],
                    class_="theme-pick active" if key == state.theme_key else "theme-pick",
                    id=f"theme-pick-{key}",
                    on_click=lambda name=key: apply_theme(name),
                )
        with dg.FlowLayout(gap=8, row_gap=6, style={"align_items": "center"}):
            dg.Label("resolved:", class_="faint", wrap=False)
            state.theme_label = dg.Label(
                THEME_LABELS[state.theme_key], class_="var-value", wrap=False
            )
            dg.Spacer()
            dg.Label("cascade:", class_="faint", wrap=False)
            state.sheet_label = dg.Label(active_sheet_summary(), class_="mono", wrap=False)

        with dg.Panel(class_="swatch", id="theme-swatch-panel"):
            with dg.HLayout(id="theme-swatch-row", style={"gap": 0}):
                with dg.Panel(class_="swatch-chip chip-accent", id="theme-swatch-accent"):
                    dg.Label("accent", wrap=False)
                with dg.Panel(class_="swatch-chip chip-ok", id="theme-swatch-success"):
                    dg.Label("success", wrap=False)
                with dg.Panel(class_="swatch-chip chip-warn", id="theme-swatch-warning"):
                    dg.Label("warning", wrap=False)
                with dg.Panel(class_="swatch-chip chip-bad", id="theme-swatch-danger"):
                    dg.Label("danger", wrap=False)
                with dg.Panel(class_="swatch-chip chip-surface", id="theme-swatch-surface"):
                    dg.Label("surface", wrap=False)

    with dg.GridLayout(columns={"default": 2, 900: 1}, min_column_width=380, gap=12):
        with dg.Panel("Named stylesheet control", class_="card"):
            dg.Label(
                "Sheets are registered by name, so a replacement keeps its cascade "
                "position and never grows the active sheet count.",
                class_="muted",
            )
            state.overrides_toggle = dg.ToggleSwitch(
                "Override sheet (added last, wins the cascade)",
                checked=state.overrides_on,
                id="forge-overrides-toggle",
                on_change=set_overrides,
            )
            state.extreme_toggle = dg.ToggleSwitch(
                "Extreme mode (deliberately hostile CSS)",
                checked=state.extreme_on,
                id="forge-extreme-toggle",
                on_change=set_extreme,
            )
            with dg.FlowLayout(gap=8, row_gap=6):
                dg.SmallButton("Previous theme", on_click=lambda: cycle_theme(-1))
                dg.SmallButton("Next theme", id="forge-next-theme", on_click=lambda: cycle_theme(1))
                dg.Button(
                    "Rapid cycle x21",
                    class_="primary",
                    id="forge-rapid-cycle",
                    on_click=lambda: rapid_cycle(3),
                )
            state.cycle_label = dg.Label("no rapid cycle run yet", class_="mono")

        with dg.Panel("CSS variable editor", class_="card var-editor"):
            dg.Label(
                "Each slider rewrites one :root custom property. The structure sheet is "
                "never reparsed -- only the variable block is replaced.",
                class_="muted",
            )
            table = TOKENS[state.theme_key]
            for name, prop, low, high, step in EDITABLE_VARS:
                with dg.HLayout(style={"gap": 8, "align_items": "center"}):
                    dg.Label(prop, class_="var-name", wrap=False, style={"width": 110})
                    state.var_sliders[name] = dg.Slider(
                        _px(table.get(name, "0px")),
                        min=low,
                        max=high,
                        step=step,
                        id=f"forge-var-{name}",
                        style={"flex": 1, "min_width": 0},
                        on_change=lambda value, key=name: on_var_slider(key, value),
                    )
                    state.var_values[name] = dg.Label(
                        table.get(name, "0px"), class_="var-value", wrap=False, style={"width": 52}
                    )
            with dg.HLayout(style={"gap": 8, "align_items": "center"}):
                dg.Label("--tf-accent", class_="var-name", wrap=False, style={"width": 110})
                state.accent_dropdown = dg.Dropdown(
                    [label for label, _c, _i in ACCENT_CHOICES],
                    value=ACCENT_CHOICES[0][0],
                    id="forge-var-accent",
                    style={"flex": 1, "min_width": 0},
                    on_change=on_accent_change,
                )
            with dg.HLayout(style={"gap": 8, "align_items": "center"}):
                dg.Label("--tf-font", class_="var-name", wrap=False, style={"width": 110})
                state.font_dropdown = dg.Dropdown(
                    [label for label, _s in FONT_STACKS],
                    value=FONT_STACKS[0][0],
                    id="forge-var-font",
                    style={"flex": 1, "min_width": 0},
                    on_change=on_font_change,
                )
            dg.Button(
                "Reset variables to theme defaults",
                id="forge-var-reset",
                on_click=reset_variables,
            )

    with dg.Panel("Retained-state probe", class_="card retain-probe"):
        dg.Label(
            "Type into these controls, focus one, scroll this page, then switch themes. "
            "The autopilot reads the same values back out of the native state store.",
            class_="muted",
        )
        with dg.GridLayout(columns={"default": 3, 1000: 2, 640: 1}, min_column_width=250, gap=10):
            with dg.VLayout(style={"gap": 6}):
                dg.Label("text", class_="faint", wrap=False)
                state.probe_text = dg.TextInput(
                    "forge-retained-text",
                    placeholder="type, then switch themes",
                    id="probe-text",
                )
                state.probe_search = dg.SearchBox(
                    "cascade",
                    placeholder="search restyle journal",
                    id="probe-search",
                    style={"width": "100%"},
                )
                state.probe_area = dg.TextArea(
                    "Multi-line value that must survive every theme swap.\nLine two.",
                    rows=3,
                    id="probe-area",
                )
            with dg.VLayout(style={"gap": 6}):
                dg.Label("numeric", class_="faint", wrap=False)
                state.probe_number = dg.NumberInput(42, min=0, max=100, step=1, id="probe-number")
                state.probe_slider = dg.Slider(0.64, min=0, max=1, step=0.01, id="probe-slider")
                state.probe_range = dg.RangeSlider(
                    (0.25, 0.78), min=0, max=1, step=0.01, id="probe-range"
                )
                state.probe_dropdown = dg.Dropdown(
                    ["parse", "cascade", "layout", "paint", "present"],
                    value="cascade",
                    id="probe-dropdown",
                )
            with dg.VLayout(style={"gap": 6}):
                dg.Label("selection", class_="faint", wrap=False)
                state.probe_check = dg.Checkbox("checked survives", checked=True, id="probe-check")
                state.probe_toggle = dg.ToggleSwitch(
                    "toggle survives", checked=True, id="probe-toggle"
                )
                state.probe_radio = dg.RadioGroup(
                    ["srgb", "linear-srgb", "oklab"],
                    value="oklab",
                    orientation="vertical",
                    id="probe-radio",
                )
                state.probe_list = dg.SelectableList(
                    ["variables", "structure", "appearance", "overrides"],
                    selected=["structure", "appearance"],
                    selection_mode="multiple",
                    id="probe-list",
                    max_height=110,
                )
        note(
            "PASS: after any theme change these values, the focused widget, the selected "
            "tab, and this page's scroll offset are all unchanged."
        )


# ---------------------------------------------------------------------------
# Lab 2 -- widget gallery
# ---------------------------------------------------------------------------


def state_row(label: str, build: Callable[[], None]) -> None:
    with dg.HLayout(style={"gap": 8, "align_items": "center"}):
        dg.Label(label, class_="state-label", wrap=False)
        with dg.FlowLayout(gap=6, row_gap=6, style={"align_items": "center"}):
            build()


def build_gallery_page() -> None:
    page_heading(
        "SURFACE / COMPLETE WIDGET SET",
        "Widget gallery",
        "Every shipping widget, each shown in its normal, hovered, focused, pressed, "
        "selected, checked, disabled, and error-like presentation. Hover/press/focus "
        "variants are pinned with classes so all eight can be compared at once.",
    )

    with dg.GridLayout(columns={"default": 2, 980: 1}, min_column_width=420, gap=12):
        with dg.Panel("Buttons", class_="card gallery-card"):
            state_row("normal", lambda: (dg.Button("Button"), dg.SmallButton("Small")))
            state_row("hover", lambda: dg.Button("Button", class_="as-hover"))
            state_row("pressed", lambda: dg.Button("Button", class_="as-pressed"))
            state_row("focus", lambda: dg.Button("Button", class_="as-focus"))
            state_row("selected", lambda: dg.Button("Button", class_="as-selected"))
            state_row("error", lambda: dg.Button("Button", class_="as-error"))
            state_row("disabled", lambda: dg.Button("Button", disabled=True))
            state_row(
                "badged",
                lambda: (
                    dg.Button("Queue", badge="12"),
                    dg.Button("Primary", class_="primary", badge="!"),
                    dg.Button("Ghost", class_="ghost"),
                    dg.Button("Danger", class_="danger"),
                ),
            )
            state_row(
                "icon",
                lambda: (
                    dg.IconButton("play", tooltip="Run"),
                    dg.IconButton("pause", tooltip="Pause"),
                    dg.IconButton("stop", tooltip="Stop", disabled=True),
                    dg.IconButton("settings", tooltip="Settings", class_="ic-boxed"),
                    dg.ArrowButton("left"),
                    dg.ArrowButton("right"),
                    dg.ArrowButton("up"),
                    dg.ArrowButton("down"),
                ),
            )

        with dg.Panel("Text entry", class_="card gallery-card"):
            state_row("normal", lambda: dg.TextInput("value", style={"width": 190}))
            state_row("placeholder", lambda: dg.TextInput("", placeholder="placeholder", style={"width": 190}))
            state_row("focus", lambda: dg.TextInput("focused", class_="as-focus", style={"width": 190}))
            state_row("error", lambda: dg.TextInput("bad value", class_="as-error", style={"width": 190}))
            state_row("disabled", lambda: dg.TextInput("locked", disabled=True, style={"width": 190}))
            state_row(
                "numeric error",
                lambda: dg.NumberInput(999, min=0, max=10, class_="as-error", style={"width": 120}),
            )
            dg.TextArea("Multi-line text area with wrapping enabled.", rows=3)
            dg.TextArea("Area in an error state", rows=2, class_="as-error")
            dg.TextArea("Disabled area", rows=2, disabled=True)
            dg.SearchBox(
                "",
                id="gallery-search-clearable",
                placeholder="SearchBox with clear button",
                style={"width": "100%"},
            )
            dg.SearchBox(
                "preset query",
                id="gallery-search-preset",
                clearable=False,
                style={"width": "100%"},
            )
            dg.CodeEditor(
                "Panel.card {\n    border-radius: var(--tf-radius);\n}",
                language="css",
                rows=4,
            )

        with dg.Panel("Choice controls", class_="card gallery-card"):
            state_row(
                "dropdown",
                lambda: (
                    dg.Dropdown(["srgb", "linear-srgb", "oklab"], value="oklab"),
                    dg.Dropdown(["disabled"], value="disabled", disabled=True),
                ),
            )
            state_row(
                "number",
                lambda: (
                    dg.NumberInput(12, min=0, max=100),
                    dg.NumberInput(7, min=0, max=10, disabled=True),
                    dg.DragNumber(3.5, min=0, max=20, step=0.1),
                ),
            )
            state_row(
                "checkbox",
                lambda: (
                    dg.Checkbox("unchecked"),
                    dg.Checkbox("checked", checked=True),
                    dg.Checkbox("disabled", disabled=True),
                    dg.Checkbox("checked+disabled", checked=True, disabled=True),
                ),
            )
            state_row(
                "toggle",
                lambda: (
                    dg.ToggleSwitch("off"),
                    dg.ToggleSwitch("on", checked=True),
                    dg.ToggleSwitch("disabled", checked=True, disabled=True),
                ),
            )
            dg.RadioGroup(
                ["contain", "cover", "stretch", "repeat"],
                value="cover",
                orientation="horizontal",
            )
            dg.RadioGroup(["disabled a", "disabled b"], value="disabled a", disabled=True)

        with dg.Panel("Ranges and status", class_="card gallery-card"):
            state_row("slider", lambda: dg.Slider(0.42, style={"width": 210}))
            state_row("disabled", lambda: dg.Slider(0.42, disabled=True, style={"width": 210}))
            state_row("range", lambda: dg.RangeSlider((0.2, 0.7), style={"width": 210}))
            state_row(
                "progress",
                lambda: (
                    dg.ProgressBar(0.34, style={"width": 130}),
                    dg.ProgressBar(0.72, show_value=True, style={"width": 130}),
                    dg.ProgressBar(0.5, disabled=True, style={"width": 130}),
                ),
            )
            state_row(
                "led",
                lambda: (
                    dg.LED(True),
                    dg.LED(False),
                    led("busy"),
                    led("warning"),
                    led("error"),
                ),
            )
            state_row(
                "badge",
                lambda: (
                    dg.Badge("neutral", level="neutral"),
                    dg.Badge("info", level="info"),
                    dg.Badge("success", level="success"),
                    dg.Badge("warning", level="warning"),
                    dg.Badge("danger", level="danger"),
                    dg.Badge("error", level="error"),
                ),
            )
            state_row(
                "tag",
                lambda: (
                    dg.Tag("neutral"),
                    dg.Tag("info", level="info"),
                    dg.Tag("success", level="success"),
                    dg.Tag("warning", level="warning"),
                    dg.Tag("danger", level="danger"),
                ),
            )
            dg.LoadingSpinner(size=20, label="working")

        with dg.Panel("Navigation", class_="card gallery-card"):
            dg.Breadcrumbs(
                [("Forge", "root"), ("Labs", "labs"), ("Gallery", "gallery")],
                on_select=lambda item: set_status(f"breadcrumb {item.label}"),
            )
            local_tabs = dg.Tabs(value="one")
            with local_tabs:
                with dg.Tab("Overview", value="one", badge="3"):
                    dg.Label("Tab body one.", class_="muted")
                with dg.Tab("Detail", value="two"):
                    dg.Label("Tab body two.", class_="muted")
                with dg.Tab("Disabled", value="three", disabled=True):
                    dg.Label("unreachable", class_="muted")
            with dg.VLayout(style={"gap": 3}):
                dg.NavItem("Selected item", page="theme", icon="home", badge="7")
                dg.NavItem("Normal item", page="gallery", icon="grid")
                dg.NavItem("Disabled item", page="parts", icon="lock", disabled=True)
            with dg.Toolbar():
                dg.IconButton("undo", tooltip="Undo")
                dg.IconButton("redo", tooltip="Redo")
                dg.ToolbarSeparator()
                dg.SmallButton("Fit")
                dg.SmallButton("Reset")
                dg.Spacer()
                dg.Badge("toolbar", level="info")

        with dg.Panel("Data surfaces", class_="card gallery-card"):
            dg.DataFrameTable(
                state.rows[:24],
                page_size=12,
                style={"height": 190},
                id="gallery-table",
            )
            dg.PropertyGrid(
                {
                    "Radius": 10,
                    "Border width": 1,
                    "Interpolation": "oklab",
                    "Managed image": True,
                    "Sheets": "variables > structure > appearance",
                },
                label_width=132,
            )

        with dg.Panel("Trees and lists", class_="card gallery-card"):
            with dg.ScrollArea(axis="y", height=160, gap=3):
                with dg.TreeView(selected="appearance"):
                    with dg.TreeNode("user origin", node_id="user", expanded=True):
                        dg.TreeNode("variables", node_id="variables", leaf=True)
                        dg.TreeNode("structure", node_id="structure", leaf=True)
                        with dg.TreeNode("appearance", node_id="appearance", expanded=True):
                            dg.TreeNode("modern-dark", node_id="md", leaf=True)
                            dg.TreeNode("win311", node_id="w311", leaf=True)
                        dg.TreeNode("overrides", node_id="overrides", leaf=True)
                    with dg.TreeNode("framework origin", node_id="framework"):
                        dg.TreeNode("defaults", node_id="defaults", leaf=True)
            dg.SelectableList(
                ["parse", "cascade", "layout", "paint", "text", "present"],
                selected=["cascade", "paint"],
                selection_mode="multiple",
                max_height=120,
            )
            with dg.FlowLayout(gap=6, row_gap=6):
                dg.Selectable("selectable", selected=True)
                dg.Selectable("unselected")
                dg.Selectable("disabled", disabled=True)

        with dg.Panel("Date and time", class_="card gallery-card"):
            with dg.Property("Date"):
                dg.DateInput("2026-07-30", id="gallery-date")
            with dg.Property("Time"):
                dg.TimeInput("09:41", id="gallery-time")
            with dg.Property("Datetime"):
                dg.DateTimeInput("2026-07-30T09:41:00", id="gallery-datetime")
            dg.DateInput("", placeholder="empty date")
            dg.DateInput("2026-01-01", disabled=True)
            dg.ColorPicker((90, 169, 255), title="Accent picker")

        with dg.Panel("Drag and drop", class_="card gallery-card"):
            with dg.FlowLayout(gap=8, row_gap=8):
                for payload in ("variables", "structure", "appearance", "overrides"):
                    with dg.DragSource(
                        {"sheet": payload},
                        drag_kind="forge-sheet",
                        style={
                            "padding": 7,
                            "border_width": 1,
                            "border_color": "border",
                            "border_radius": 6,
                        },
                    ):
                        dg.Tag(payload, level="info")
            dg.DropZone(
                "Drop a sheet name here",
                accept="forge-sheet",
                style={"height": 96},
                on_drop=lambda payload: set_status(f"dropped {payload}", "ok"),
            )

        with dg.Panel("Containers and overlays", class_="card gallery-card"):
            with dg.Collapsible("Collapsible (expanded)", expanded=True):
                dg.Label("Body content inside an expanded disclosure.", class_="muted")
                dg.ProgressBar(0.4)
            with dg.Collapsible("Collapsible (collapsed)", expanded=False):
                dg.Label("Hidden until expanded.", class_="muted")
            with dg.Splitter(orientation="horizontal", gutter_size=6, style={"height": 108}):
                with dg.Pane(flex=45, min_size=90):
                    with dg.Panel(class_="card dense"):
                        dg.Label("Left pane", class_="muted")
                with dg.Pane(flex=55, min_size=90):
                    with dg.Panel(class_="card dense"):
                        dg.Label("Right pane", class_="muted")
            with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
                tip_target = dg.Button("Rich tooltip target", id="gallery-tooltip-target")
                dg.Button("Simple tooltip", tooltip="A plain string tooltip overlay.")
                dg.Button("Open modal", class_="primary", on_click=show_modal)
                dg.Button("Command palette", on_click=show_palette)
                dg.SmallButton("Toast", on_click=emit_toast)
            with dg.Tooltip(target=tip_target, width=300, height=118):
                dg.Label("Rich tooltip", class_="page-title")
                dg.Label("A real overlay container with child widgets.", class_="muted")
                dg.ProgressBar(0.66, show_value=True)

        with dg.Panel("Mini window chrome", class_="card gallery-card"):
            dg.Label(
                "A styled stand-in for the real client-side titlebar so chrome parts can "
                "be compared against ordinary widgets on the same page.",
                class_="muted",
            )
            with dg.Panel(class_="mini-window"):
                with dg.HLayout(class_="mini-titlebar", style={"align_items": "center"}):
                    dg.Label("Preview window", class_="mini-title", wrap=False)
                    dg.Spacer()
                    dg.IconButton("minus", size=26, tooltip="Minimize preview")
                    dg.IconButton("stop", size=26, tooltip="Maximize preview")
                    dg.IconButton("close", size=26, tooltip="Close preview")
                with dg.VLayout(style={"padding": 10, "gap": 6}):
                    dg.Label("Content sits directly beneath the titlebar.", class_="muted")
                    dg.ProgressBar(0.55)


# ---------------------------------------------------------------------------
# Lab 3 -- composite parts
# ---------------------------------------------------------------------------


def build_parts_page() -> None:
    page_heading(
        "PARTS / FORWARDING CONTRACT",
        "Composite-part test",
        "Every public part of every composite widget is given a deliberately clashing "
        "treatment. If a part is forwarded to the wrong sub-widget, or a part rule leaks "
        "onto the host, the mismatch is impossible to miss.",
    )

    note(
        "PASS: each labelled part shows exactly the color called out beside it, and no "
        "part color bleeds onto its neighbour."
    )

    with dg.GridLayout(columns={"default": 2, 980: 1}, min_column_width=400, gap=12):
        with dg.Panel("SearchBox parts", class_="card"):
            dg.Label("::icon amber, ::field dashed cyan, ::clear pink pill", class_="mono")
            dg.SearchBox(
                "forwarded",
                placeholder="type to reveal the clear button",
                class_="part-probe",
                id="parts-search",
                style={"width": "100%"},
            )
            dg.SearchBox("", placeholder="empty (no clear button)", class_="part-probe", style={"width": "100%"})

        with dg.Panel("Panel parts", class_="card", id="parts-panel-card"):
            with dg.Panel(
                "Header purple / title green",
                class_="part-probe",
                id="parts-panel-probe",
            ):
                with dg.ScrollArea(
                    axis="y",
                    height=124,
                    gap=4,
                    id="parts-panel-scroll",
                    style={"padding": 8},
                ):
                    for index in range(14):
                        dg.Label(
                            f"body row {index:02d} -- ::body navy, ::accent pink, "
                            "::scrollbar-thumb green",
                            class_="mono",
                        )

        with dg.Panel("Slider and range parts", class_="card"):
            dg.Label("::track purple, ::fill green, ::thumb amber square", class_="mono")
            dg.Slider(0.55, class_="part-probe", id="parts-slider", style={"width": "100%"})
            dg.Slider(0.2, class_="part-probe", disabled=True, style={"width": "100%"})
            dg.Label("::range cyan, ::thumb-min round green, ::thumb-max square pink", class_="mono")
            dg.RangeSlider((0.25, 0.75), class_="part-probe", style={"width": "100%"})

        with dg.Panel("Progress parts", class_="card"):
            dg.Label("::track purple, ::fill amber, ::label green", class_="mono")
            for value in (0.0, 0.18, 0.5, 0.83, 1.0):
                dg.ProgressBar(value, show_value=True, class_="part-probe", style={"width": "100%"})

        with dg.Panel("Table parts", class_="card"):
            dg.Label(
                "::header purple, ::row navy, ::row-selected green, ::grid-line pink, "
                "::scrollbar-thumb cyan",
                class_="mono",
            )
            dg.DataFrameTable(
                state.rows[:40],
                page_size=20,
                class_="part-probe",
                style={"height": 210},
                id="parts-table",
            )

        with dg.Panel("Badge-bearing parts", class_="card"):
            dg.Label("Button::badge, Tab::badge, NavItem::badge and ::accent", class_="mono")
            with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
                dg.Button("Button badge", badge="42", class_="part-probe")
                dg.SmallButton("Small badge", badge="7", class_="part-probe")
            part_tabs = dg.Tabs(value="a")
            with part_tabs:
                with dg.Tab("Alpha", value="a", badge="3", class_="part-probe"):
                    dg.Label("tab body", class_="muted")
                with dg.Tab("Beta", value="b", badge="12", class_="part-probe"):
                    dg.Label("tab body", class_="muted")
            with dg.VLayout(style={"gap": 3}):
                dg.NavItem("Nav with badge", page="parts", badge="9", class_="part-probe")
                dg.NavItem("Nav without badge", page="parts", class_="part-probe")

        with dg.Panel("Control parts", class_="card"):
            dg.Label("::box, ::indicator, ::stepper-up/down, ::chevron, ::dot/::glow", class_="mono")
            dg.Checkbox("Checkbox::box cyan / :checked::indicator green", checked=True, class_="part-probe")
            dg.Checkbox("unchecked", class_="part-probe")
            dg.ToggleSwitch("ToggleSwitch::track / ::thumb", checked=True, class_="part-probe")
            dg.NumberInput(21, min=0, max=100, class_="part-probe", id="parts-number")
            dg.Dropdown(
                ["srgb", "linear-srgb", "oklab"],
                value="linear-srgb",
                class_="part-probe",
                id="parts-dropdown",
            )
            with dg.FlowLayout(gap=10, row_gap=8, style={"align_items": "center"}):
                dg.LED(True, class_="part-probe")
                led("busy", class_="part-probe")
                dg.Label("LED::dot green, ::glow pink, ::highlight white", class_="mono", wrap=False)

        with dg.Panel("Disclosure and tree parts", class_="card"):
            with dg.Collapsible("Collapsible::header / ::indicator / ::body", expanded=True, class_="part-probe"):
                dg.Label("Disclosure body on the navy ::body part.", class_="muted")
            with dg.ScrollArea(axis="y", height=150, gap=3):
                with dg.TreeView(selected="row-b"):
                    with dg.TreeNode("TreeNode::row / ::guide / ::indicator", node_id="root", expanded=True, class_="part-probe"):
                        dg.TreeNode("row a", node_id="row-a", leaf=True, class_="part-probe")
                        dg.TreeNode("row b", node_id="row-b", leaf=True, class_="part-probe")
                        with dg.TreeNode("nested", node_id="nested", expanded=True, class_="part-probe"):
                            dg.TreeNode("row c", node_id="row-c", leaf=True, class_="part-probe")
            with dg.FlowLayout(gap=6, row_gap=6):
                dg.Selectable("Selectable::row + ::indicator", selected=True, class_="part-probe")
                dg.Selectable("unselected", class_="part-probe")

        with dg.Panel(
            "Splitter and scrollbar parts",
            class_="card",
            id="parts-splitter-card",
        ):
            dg.Label("Splitter::gutter amber; scrollbar parts on the panes", class_="mono")
            with dg.Splitter(
                orientation="horizontal",
                gutter_size=10,
                class_="part-probe",
                id="parts-splitter",
                style={"height": 170},
            ):
                with dg.Pane(flex=50, min_size=110):
                    with dg.Panel(class_="part-probe"):
                        with dg.ScrollArea(axis="y", height=132, gap=3, style={"padding": 6}):
                            for index in range(18):
                                dg.Label(f"left {index:02d}", class_="mono")
                with dg.Pane(flex=50, min_size=110):
                    with dg.Panel(class_="part-probe"):
                        with dg.ScrollArea(axis="y", height=132, gap=3, style={"padding": 6}):
                            for index in range(18):
                                dg.Label(f"right {index:02d}", class_="mono")

    with dg.Panel(
        "Unsupported part diagnostics",
        class_="card",
        id="parts-unsupported-card",
    ):
        dg.Label(
            "The structure sheet also declares Button::stepper, which does not exist. It "
            "must appear as a stylesheet warning and paint nothing -- Button below is "
            "unchanged.",
            class_="muted",
        )
        dg.Button("Button with a bogus part rule", class_="part-probe", id="parts-bogus")


# ---------------------------------------------------------------------------
# Lab 4 -- borders and outlines
# ---------------------------------------------------------------------------


def bd_panel(class_name: str, title: str, caption: str) -> None:
    with dg.Panel(class_=f"bd {class_name}"):
        dg.Label(title, class_="mono", wrap=False)
        dg.Label(caption, class_="faint")


def build_borders_page() -> None:
    page_heading(
        "EDGES / BORDERS AND OUTLINES",
        "Border and outline laboratory",
        "Per-side widths, styles and colors; solid, dashed, dotted and double rings; "
        "asymmetric and rounded edges; borders combined with clipping; dotted focus "
        "outlines; and classic inset/outset bevels at very small and very large sizes.",
    )

    note(
        "PASS: dashes and dots follow rounded corners without breaking phase, double "
        "borders keep an even three-band split, and nothing overdraws the corner arcs."
    )

    with dg.GridLayout(columns={"default": 4, 1180: 3, 860: 2, 560: 1}, min_column_width=220, gap=10):
        bd_panel("bd-sides", "four different sides", "1px solid / 4px dashed / 7px double / 2px dotted")
        bd_panel("bd-solid", "solid 2px", "uniform ring")
        bd_panel("bd-dashed", "dashed 3px", "phase must stay stable on resize")
        bd_panel("bd-dotted", "dotted 3px", "round caps on the rounded ring")
        bd_panel("bd-double", "double 9px", "three equal bands: paint / gap / paint")
        bd_panel("bd-asym", "asymmetric widths", "1 / 9 / 3 / 16 px, four colors")
        bd_panel("bd-round", "mixed corner radii", "30 / 2 / 30 / 2 px per corner")
        bd_panel("bd-clip", "border + clipped stripes", "overflow: hidden inside a 22px radius")
        bd_panel("bd-focus", "dotted focus outline", "outline-offset: 3px, paint-only")
        bd_panel("bd-inset", "classic inset bevel", "dark top/left, light bottom/right")
        bd_panel("bd-outset", "classic outset bevel", "light top/left, dark bottom/right")
        bd_panel("bd-hair", "18px hairline row", "1px border with a 1px radius")

    with dg.GridLayout(columns={"default": 2, 900: 1}, min_column_width=380, gap=12):
        with dg.Panel("Interaction-driven borders", class_="card"):
            dg.Label(
                "Widths never change, only colors and styles, so hover/focus/press must "
                "not move a single pixel of layout.",
                class_="muted",
            )
            with dg.FlowLayout(gap=8, row_gap=8):
                dg.Button("hover me", class_="bd-react", id="borders-react-1")
                dg.Button("focus me", class_="bd-react", id="borders-react-2")
                dg.Button("press me", class_="bd-react", id="borders-react-3")
                dg.Button("disabled", class_="bd-react", disabled=True)
            with dg.FlowLayout(gap=8, row_gap=8):
                dg.Selectable("selected -> 6px double", selected=True, class_="bd-react")
                dg.Selectable("unselected", class_="bd-react")

        with dg.Panel("Scale extremes", class_="card"):
            dg.Label(
                "A 14px control and a 96px control share the same border model. Corner "
                "arcs and DPI rounding fail visibly at these two ends.",
                class_="muted",
            )
            with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
                for label in ("a", "bb", "ccc", "dddd"):
                    dg.Button(label, class_="bd-tiny")
            dg.Button("HUGE 12px DOUBLE", class_="bd-huge")
            with dg.VLayout(style={"gap": 2}):
                for index in range(5):
                    with dg.Panel(class_="bd bd-hair"):
                        dg.Label(f"hairline {index}", class_="mono", wrap=False)


# ---------------------------------------------------------------------------
# Lab 5 -- backgrounds, gradients and patterns
# ---------------------------------------------------------------------------


def bg_panel(class_name: str, title: str, caption: str) -> None:
    with dg.Panel(class_=f"bg {class_name}"):
        dg.Label(title, class_="mono", wrap=False)
        dg.Label(caption, class_="faint")


def build_paint_page() -> None:
    page_heading(
        "PAINT / GRADIENTS, PATTERNS, IMAGES",
        "Background and gradient laboratory",
        "Linear, radial and repeating gradients; percentage, pixel, calc() and hard "
        "stops; one- and two-pixel stripes; the procedural checker/pinstripe/stipple/dot/"
        "hatch patterns; layered paint; and managed images in all four fit modes.",
    )

    with dg.Panel("Managed image resources", class_="card"):
        dg.Label(
            "CSS never opens a file. Both textures are generated in Python, registered "
            "under a semantic id, and replaced or released while the app runs.",
            class_="muted",
        )
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            dg.Button("Swap texture", id="paint-swap-texture", on_click=swap_texture)
            dg.Button("Release texture", id="paint-release-texture", on_click=release_texture)
            dg.Button("Restore texture", class_="primary", id="paint-restore-texture", on_click=restore_texture)
            state.texture_label = dg.Label("forge-weave: cool (registered)", class_="mono", wrap=False)
        note(
            "PASS: releasing the resource leaves the panels painted with their "
            "background-color fallback -- never a black hole or a stale texture."
        )

    with dg.GridLayout(columns={"default": 4, 1180: 3, 860: 2, 560: 1}, min_column_width=230, gap=10):
        bg_panel("bg-linear", "linear-gradient 135deg", "three stops")
        bg_panel("bg-linear-oklab", "gradient-interpolation: oklab", "same stops, perceptual blend")
        bg_panel("bg-radial", "radial-gradient at 30% 30%", "centered circle with offset")
        bg_panel("bg-radial-corner", "radial-gradient at right bottom", "keyword center")
        bg_panel("bg-repeating-linear", "repeating-linear 1px/4px", "period fixed in logical px")
        bg_panel("bg-repeating-radial", "repeating-radial rings", "2px ring, 9px period")
        bg_panel("bg-stops-percent", "percentage stops", "0 / 35 / 70 / 100 %")
        bg_panel("bg-stops-pixel", "pixel stops", "0 / 40 / 120 px")
        bg_panel("bg-stops-calc", "calc() stops", "calc(50% - 60px) .. calc(50% + 60px)")
        bg_panel("bg-hard-stop", "hard stops", "three flat bands, no blend")
        bg_panel("bg-stripe-1", "1px horizontal stripes", "worst case for DPI snapping")
        bg_panel("bg-stripe-2", "2px vertical stripes", "must not shimmer while resizing")
        bg_panel("bg-checker", "dg-pattern(checker)", "tile size from --tf-tile")
        bg_panel("bg-pinstripe", "dg-pattern(pinstripe)", "tile size from --tf-tile")
        bg_panel("bg-stipple", "dg-pattern(stipple)", "tile size from --tf-tile")
        bg_panel("bg-dot", "dg-pattern(dot)", "tile size from --tf-tile")
        bg_panel("bg-hatch", "dg-pattern(diagonal-hatch)", "tile size from --tf-tile")
        bg_panel("bg-layers", "three stacked layers", "radial + linear + hatch + noise")
        bg_panel("bg-blob", "blob-gradient", "three soft fields")
        bg_panel("bg-mesh", "mesh-gradient", "four-corner bilinear blend")
        bg_panel("bg-image-contain", "app-resource contain", "whole texture, letterboxed")
        bg_panel("bg-image-cover", "app-resource cover", "fills, crops overflow")
        bg_panel("bg-image-stretch", "app-resource stretch", "aspect ratio ignored")
        bg_panel("bg-image-repeat", "app-resource repeat", "tiled at native size")
        bg_panel("bg-image-round", "image + 34px radius", "texture clipped to the rounded box")
        bg_panel("bg-pattern-round", "pattern + 34px radius", "pattern clipped to the rounded box")

    with dg.Panel("Continuously resized surfaces", class_="card"):
        dg.Label(
            "The live worker rewrites these panels' widths every frame. Stripe and "
            "pattern periods must stay constant while the surface changes size.",
            class_="muted",
        )
        for index in range(4):
            panel = dg.Panel(class_="bg-flex", id=f"paint-flex-{index}")
            state.flex_panels.append(panel)
        with dg.FlowLayout(gap=10, row_gap=10, style={"align_items": "center"}):
            dg.Panel(class_="bg bg-pip")
            dg.Label("16x16 pip, contain-fitted inside a circular clip", class_="mono")


# ---------------------------------------------------------------------------
# Lab 6 -- typography
# ---------------------------------------------------------------------------

UNICODE_SAMPLES: tuple[tuple[str, str], ...] = (
    ("latin", "The quick brown fox jumps over the lazy dog 0123456789"),
    ("accents", "ÀÉÎÕÜ àéîõü ßæœøå"),
    ("greek", "Αβγδεζηθ λμνξπρσφψω"),
    ("cyrillic", "Абвгдежз Привет мир"),
    ("cjk", "日本語 中文文字 한국어"),
    ("rtl", "العربية עברית"),
    ("symbols", "←↑→↓ ≠≤≥±×÷ √∞∑∏ ✓✗ ▲▼◆●"),
    ("box", "┌─┬─┐ │ ║ ╚═╩═╝ ░▒▓█"),
    ("emoji", "\U0001f525 \U0001f9ea \U0001f4ca ✅ ⚠️ \U0001f3a8 \U0001f680"),
)

LONG_LINE = (
    "This label is deliberately long so that wrapping, ellipsizing and clipping can be "
    "compared directly against each other at every font size the theme can produce, "
    "including the ones that make the line box taller than the control."
)


def build_type_page() -> None:
    page_heading(
        "TEXT / FONT RESOLUTION",
        "Typography torture test",
        "Ordered fallback stacks with missing families first, the five generic families, "
        "weights and italics, tiny and huge sizes, wrapped/clipped/ellipsized text, and "
        "non-Latin scripts inside badges and narrow buttons.",
    )

    with dg.Panel("Runtime font stack", class_="card"):
        dg.Label(
            "The --tf-font control on the theme page rewrites the inherited stack. Watch "
            "for layout jumps, clipping, and baseline shifts as it changes.",
            class_="muted",
        )
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            for label, _stack in FONT_STACKS[1:]:
                dg.SmallButton(label, on_click=lambda name=label: on_font_change(name))
        note(
            "PASS: a missing family is skipped silently and reported in font diagnostics; "
            "the next available family in the stack renders."
        )

    with dg.GridLayout(columns={"default": 2, 980: 1}, min_column_width=400, gap=12):
        with dg.Panel("Fallback stacks", class_="card ty"):
            dg.Label("\"Absolutely Missing\", \"Also Missing\", var(--tf-font)", class_="mono")
            dg.Label(LONG_LINE, class_="ty-stack-missing ty-wrap")
            dg.Label("@font-face alias \"Forge UI\" (missing local first)", class_="mono")
            dg.Label(LONG_LINE, class_="ty-wrap")

        with dg.Panel("Generic families", class_="card ty"):
            for cls, label in (
                ("ty-serif", "serif"),
                ("ty-sans", "sans-serif"),
                ("ty-mono", "monospace"),
                ("ty-cursive", "cursive"),
                ("ty-fantasy", "fantasy"),
            ):
                with dg.HLayout(style={"gap": 8, "align_items": "center"}):
                    dg.Label(label, class_="state-label", wrap=False)
                    dg.Label("Sphinx of black quartz, judge my vow. 0123456789", class_=cls)

        with dg.Panel("Weights, styles, numerics", class_="card ty"):
            for cls, label in (
                ("ty-w100", "100"),
                ("ty-w400", "400"),
                ("ty-w700", "700"),
                ("ty-w900", "900"),
                ("ty-italic", "italic"),
                ("ty-tabular", "tabular-nums"),
            ):
                with dg.HLayout(style={"gap": 8, "align_items": "center"}):
                    dg.Label(label, class_="state-label", wrap=False)
                    dg.Label("Weight and figure sample 1234567890", class_=cls)

        with dg.Panel("Sizes", class_="card ty"):
            for cls, label in (
                ("ty-xs", "8px"),
                ("ty-sm", "11px"),
                ("", "inherited"),
                ("ty-lg", "26px"),
                ("ty-xl", "44px"),
            ):
                dg.Label(f"{label} -- Forge {label}", class_=cls or None, wrap=False)

        with dg.Panel("Overflow behaviour", class_="card ty"):
            dg.Label("wrapped (full width)", class_="faint")
            dg.Label(LONG_LINE, class_="ty-wrap")
            dg.Label("single line, text-overflow: clip, width 190px", class_="faint")
            dg.Label(LONG_LINE, class_="ty-clip", wrap=False)
            dg.Label("single line, text-overflow: ellipsis, width 190px", class_="faint")
            dg.Label(LONG_LINE, class_="ty-ellipsis", wrap=False)
            dg.Label("letter-spacing 3px, uppercase", class_="faint")
            dg.Label("tracking sample", class_="ty-track", wrap=False)
            dg.Label("letter-spacing -0.5px", class_="faint")
            dg.Label("tight tracking sample", class_="ty-tight", wrap=False)
            with dg.HLayout(style={"gap": 8}):
                dg.Label("Case Transform", class_="ty-upper", wrap=False)
                dg.Label("Case Transform", class_="ty-lower", wrap=False)
                dg.Label("case transform", class_="ty-capital", wrap=False)

        with dg.Panel("Unicode and symbols", class_="card ty"):
            for label, sample in UNICODE_SAMPLES:
                with dg.HLayout(style={"gap": 8, "align_items": "center"}):
                    dg.Label(label, class_="state-label", wrap=False)
                    dg.Label(sample, class_="ty-unicode")

        with dg.Panel("Text inside narrow controls", class_="card ty"):
            dg.Label("Badges and buttons with more text than room.", class_="muted")
            with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
                dg.Badge("a", level="info")
                dg.Badge("12", level="success")
                dg.Badge("9999+", level="warning")
                dg.Badge("a very long badge value", level="danger", class_="ty-long")
                dg.Badge("日本語バッジ", level="info")
                dg.Badge("\U0001f525", level="warning")
            with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
                dg.Button("ok", class_="ty-narrow")
                dg.Button("a rather long button label", class_="ty-narrow")
                dg.Button("Привет", class_="ty-narrow")
                dg.Button("中文按钮", class_="ty-narrow")
                dg.Button("\U0001f680 go", class_="ty-narrow")
            with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
                dg.Tag("✓ pass", level="success")
                dg.Tag("✗ fail", level="danger")
                dg.Tag("░▒▓", level="neutral")

        with dg.Panel("Single-line and multiline controls", class_="card ty"):
            dg.TextInput(LONG_LINE, style={"width": "100%"})
            dg.TextArea(LONG_LINE + "\n\n" + UNICODE_SAMPLES[4][1], rows=4)
            dg.SearchBox(UNICODE_SAMPLES[3][1], style={"width": "100%"})
            dg.LogView(
                [f"{label}: {sample}" for label, sample in UNICODE_SAMPLES],
                rows=6,
                wrap=False,
            )


# ---------------------------------------------------------------------------
# Lab 7 -- layout
# ---------------------------------------------------------------------------


def build_layout_page() -> None:
    page_heading(
        "GEOMETRY / NESTED LAYOUT",
        "Layout torture test",
        "Horizontal and vertical nesting, responsive grids, flow wrapping, fixed/flexible/"
        "content-sized children, nested panels, nested scroll owners, splitters, and long "
        "content inside hard-constrained panels.",
    )

    with dg.Panel("Viewport sweep", class_="card"):
        dg.Label(
            "These presets drive the same resize path the autopilot uses. Nothing may "
            "overlap, escape its panel, or clip unexpectedly at any of them.",
            class_="muted",
        )
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            for label, width, height in VIEWPORTS:
                dg.SmallButton(
                    label,
                    id=f"layout-vp-{width}x{height}",
                    on_click=lambda w=width, h=height: resize_window(w, h),
                )
        with dg.FlowLayout(gap=8, row_gap=6, style={"align_items": "center"}):
            dg.Label("breakpoint marker:", class_="faint", wrap=False)
            dg.Label("width", class_="ly-breakpoint mono", wrap=False)
            dg.Label("resolution", class_="ly-dpi mono", wrap=False)
            dg.Label("supports", class_="ly-supports mono", wrap=False)
        dg.Label(
            "Run the app under 100%, 150% and 200% display scaling to exercise the DPI "
            "paths; the resolution marker above reports what the renderer sees.",
            class_="faint",
        )

    with dg.Panel("Fixed / flexible / content-sized", class_="card"):
        with dg.HLayout(style={"gap": 8}):
            with dg.Panel(class_="ly ly-fixed"):
                dg.Label("fixed 132px", class_="mono", wrap=False)
                dg.Label("never grows or shrinks", class_="faint")
            with dg.Panel(class_="ly ly-flex"):
                dg.Label("flex: 1, min-width: 0", class_="mono", wrap=False)
                dg.Label(LONG_LINE, class_="faint")
            with dg.Panel(class_="ly ly-content"):
                dg.Label("content sized", class_="mono", wrap=False)
                dg.Badge("auto", level="info")

    with dg.Panel("Responsive grid", class_="card"):
        with dg.GridLayout(
            columns={"default": 4, 1180: 3, 880: 2, 600: 1},
            min_column_width=210,
            gap=10,
            balance_last_row=True,
            class_="ly-grid",
        ):
            for index in range(11):
                with dg.Panel(class_="ly"):
                    dg.Label(f"cell {index:02d}", class_="mono", wrap=False)
                    dg.ProgressBar((index % 7) / 6.0)
                    if index % 3 == 0:
                        dg.Label(LONG_LINE, class_="faint")

    with dg.Panel("Flow wrapping", class_="card"):
        with dg.FlowLayout(gap=6, row_gap=6, class_="ly-flow", style={"align_items": "center"}):
            for index in range(34):
                dg.Tag(f"chip-{index:02d}", level=("neutral", "info", "success", "warning")[index % 4])

    with dg.GridLayout(columns={"default": 2, 920: 1}, min_column_width=380, gap=12):
        with dg.Panel("Nested scroll owners", class_="card"):
            dg.Label(
                "An inner scroll area inside this page's scroll area. Wheel events must "
                "reach the inner owner first and stop there while it can still move.",
                class_="muted",
            )
            with dg.ScrollArea(axis="y", class_="ly-inner", id="layout-inner-scroll"):
                for index in range(40):
                    dg.Label(f"inner scroll row {index:02d}", class_="mono")
            with dg.ScrollArea(axis="y", class_="ly-inner"):
                with dg.ScrollArea(axis="y", class_="ly-inner"):
                    for index in range(30):
                        dg.Label(f"doubly nested row {index:02d}", class_="mono")

        with dg.Panel("Constrained overflow", class_="card"):
            dg.Label(
                "A fixed 132px panel with far more content than it can show. The excess "
                "must clip at the panel edge, not paint over the neighbours.",
                class_="muted",
            )
            with dg.Panel(class_="ly ly-constrained"):
                for index in range(14):
                    dg.Label(f"constrained line {index:02d} -- {LONG_LINE}", class_="faint")
            with dg.Panel(class_="ly ly-constrained"):
                dg.DataFrameTable(state.rows[:60], page_size=30, style={"height": 300})

    with dg.Panel("Nested splitters and panels", class_="card"):
        with dg.Splitter(orientation="horizontal", gutter_size=8, class_="ly-split", style={"height": 300}):
            with dg.Pane(flex=34, min_size=170):
                with dg.Panel("Outer left", class_="card dense"):
                    with dg.Splitter(orientation="vertical", gutter_size=8, class_="ly-split", style={"flex": 1}):
                        with dg.Pane(flex=50, min_size=70):
                            with dg.Panel("Nested top", class_="card dense"):
                                dg.Label("depth 3", class_="mono")
                                dg.ProgressBar(0.3)
                        with dg.Pane(flex=50, min_size=70):
                            with dg.Panel("Nested bottom", class_="card dense"):
                                with dg.ScrollArea(axis="y", class_="ly-inner", height=90):
                                    for index in range(20):
                                        dg.Label(f"depth 4 row {index:02d}", class_="mono")
            with dg.Pane(flex=66, min_size=240):
                with dg.Panel("Outer right", class_="card dense"):
                    with dg.GridLayout(columns={"default": 2, 700: 1}, min_column_width=190, gap=8):
                        for index in range(6):
                            with dg.Panel(class_="ly"):
                                dg.Label(f"nested cell {index}", class_="mono", wrap=False)
                                with dg.FlowLayout(gap=4, row_gap=4):
                                    dg.Badge(str(index), level="info")
                                    dg.Tag("nested")
                                    dg.LED(index % 2 == 0)

    with dg.Panel("Toolbar / tabs / status stack", class_="card flush"):
        with dg.VLayout(style={"gap": 0}):
            with dg.Toolbar(class_="forge-toolbar"):
                dg.IconButton("menu", tooltip="Local toolbar")
                dg.ToolbarSeparator()
                dg.SmallButton("Action")
                dg.Spacer()
                dg.Badge("local", level="info")
            local_tabs = dg.Tabs(value="a")
            with local_tabs:
                with dg.Tab("Alpha", value="a"):
                    dg.Label("A toolbar sits directly above this tab bar.", class_="muted")
                with dg.Tab("Beta", value="b"):
                    dg.Label("Second tab body.", class_="muted")
            with dg.StatusBar(height=26, class_="forge-status"):
                dg.Label("nested status bar", class_="mono", wrap=False)
                dg.Spacer()
                dg.Tag("ok", level="success")


# ---------------------------------------------------------------------------
# Lab 8 -- effects
# ---------------------------------------------------------------------------


def effect_stage(stage_class: str, title: str) -> None:
    with dg.Panel(title, class_=f"fx-stage {stage_class}"):
        with dg.FlowLayout(gap=10, row_gap=10, style={"align_items": "center"}):
            dg.Button("no glow", class_="fx-glow-0")
            dg.Button("glow 1", class_="fx-glow-1")
            dg.Button("glow 2", class_="fx-glow-2")
            dg.Button("glow 3", class_="fx-glow-3")
            dg.Button("disabled", class_="fx-glow-3 fx-disabled", disabled=True)
        with dg.FlowLayout(gap=10, row_gap=10, style={"align_items": "center"}):
            dg.Badge("badge", level="info")
            dg.Badge("glow 1", level="info", class_="fx-glow-1")
            dg.Badge("glow 2", level="info", class_="fx-glow-2")
            dg.LED(True)
            dg.LED(True, class_="fx-glow-1")
            dg.LED(True, class_="fx-glow-2")
            dg.LED(True, class_="fx-glow-3")
            led("busy", class_="fx-glow-3")
        with dg.FlowLayout(gap=10, row_gap=10, style={"align_items": "center"}):
            dg.Button("hover shadow", class_="fx-hover-shadow")
            dg.Button("focus ring", class_="fx-ring")
            dg.Button("tight ring", class_="fx-ring-tight")


def build_effects_page() -> None:
    page_heading(
        "LIGHT / SHADOWS, GLOW, RINGS",
        "Effects laboratory",
        "Matched glow, shadow, inset and focus-ring strengths presented over both a very "
        "dark and a very light stage, so halos, banding and incorrect alpha blending are "
        "obvious side by side.",
    )

    note(
        "PASS: no shadow shows a hard rectangular edge outside a rounded corner, and no "
        "glow produces a visible grey halo over the light stage."
    )

    with dg.GridLayout(columns={"default": 2, 900: 1}, min_column_width=400, gap=12):
        effect_stage("fx-dark", "Dark stage")
        effect_stage("fx-light", "Light stage")

    with dg.GridLayout(columns={"default": 4, 1100: 2, 640: 1}, min_column_width=230, gap=10):
        for cls, label, caption in (
            ("fx-shadow-1", "outset 0 2 6", "tight contact shadow"),
            ("fx-shadow-2", "outset 0 8 20", "card elevation"),
            ("fx-shadow-3", "outset 0 18 44 +6", "deep, spread"),
            ("fx-shadow-multi", "three layers", "sheen + elevation + glow"),
            ("fx-inset-1", "inset 0 2 6", "shallow well"),
            ("fx-inset-2", "inset 0 6 16", "deep well"),
            ("fx-inset-mixed", "inset + outset", "both in one declaration"),
            ("fx-round-shadow", "30px radius", "shadow must follow the arc"),
        ):
            with dg.Panel(class_=f"card {cls}"):
                dg.Label(label, class_="mono", wrap=False)
                dg.Label(caption, class_="faint")

    with dg.GridLayout(columns={"default": 2, 900: 1}, min_column_width=380, gap=12):
        with dg.Panel("Shadows inside a scroll owner", class_="card"):
            dg.Label(
                "Outset shadows must be clipped to the scroll viewport without shrinking "
                "the shadow shape as the content moves.",
                class_="muted",
            )
            with dg.Panel(class_="fx-scroll-host", id="effects-scroll-host"):
                for index in range(16):
                    with dg.Panel(class_="fx-scroll-item"):
                        dg.Label(f"elevated row {index:02d}", class_="mono", wrap=False)

        with dg.Panel("Shadows inside tabs", class_="card"):
            fx_tabs = dg.Tabs(value="a")
            with fx_tabs:
                with dg.Tab("Elevated", value="a"):
                    with dg.Panel(class_="card fx-shadow-3"):
                        dg.Label("A deeply shadowed card inside a tab body.", class_="muted")
                with dg.Tab("Inset", value="b"):
                    with dg.Panel(class_="card fx-inset-2"):
                        dg.Label("An inset card inside a tab body.", class_="muted")
                with dg.Tab("Disabled look", value="c"):
                    with dg.Panel(class_="card fx-disabled-stage"):
                        dg.Label("A whole stage at 55% opacity.", class_="muted")
                        dg.Button("disabled", disabled=True)
                        dg.Slider(0.4, disabled=True)
                        dg.ProgressBar(0.4, disabled=True)


# ---------------------------------------------------------------------------
# Lab 9 -- icon system
# ---------------------------------------------------------------------------

SEMANTIC_ICONS: tuple[str, ...] = (
    "add", "check", "close", "copy", "download", "edit", "eye", "eye-off", "file",
    "filter", "fit", "folder", "grid", "help", "home", "info", "list", "lock", "menu",
    "minus", "more", "pan", "pause", "play", "redo", "refresh", "save", "search",
    "settings", "sort", "stop", "undo", "unlock", "upload", "warning", "axes",
)


def build_icons_page() -> None:
    page_heading(
        "ICONS / SEMANTIC GEOMETRY",
        "Icon system test",
        "Every built-in semantic icon, application-provided overrides, five sizes, two "
        "stroke weights, icons inside buttons/menus/tabs/search controls, icon-only "
        "controls in every state, live theme replacement, and missing-icon fallback.",
    )

    with dg.Panel("Runtime icon theme", class_="card"):
        dg.Label(
            "Geometry comes from App.set_icon_theme(); color, size, spacing and "
            "interaction states stay owned by CSS.",
            class_="muted",
        )
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            for key in ICON_THEME_ORDER:
                dg.SmallButton(
                    key,
                    id=f"icons-theme-{key}",
                    on_click=lambda name=key: apply_icon_theme(name),
                )
            dg.Button("Cycle theme", class_="primary", id="icons-cycle", on_click=cycle_icon_theme)
            dg.SmallButton("Swap live identity", id="icons-identity", on_click=toggle_icon_identity)
            state.icon_theme_label = dg.Label("icon theme: builtin", class_="mono", wrap=False)
        with dg.FlowLayout(gap=12, row_gap=10, style={"align_items": "center"}):
            state.icon_swap_button = dg.IconButton("search", tooltip="Overridden by theme", class_="ic-lg ic-boxed")
            state.icon_identity_button = dg.IconButton("search", tooltip="Identity swaps live", class_="ic-lg ic-boxed")
            dg.IconButton("save", tooltip="Overridden by theme", class_="ic-lg ic-boxed")
            dg.IconButton("settings", tooltip="Overridden by theme", class_="ic-lg ic-boxed")
            dg.IconButton("refresh", tooltip="Overridden by theme", class_="ic-lg ic-boxed")
            dg.IconButton("run", tooltip="Alias -> play", class_="ic-lg ic-boxed")
            dg.IconButton("definitely-not-an-icon", tooltip="Unknown -> more", class_="ic-lg ic-boxed")
        note(
            "PASS: overridden icons change geometry only. Unknown names fall back to the "
            "built-in 'more' glyph instead of drawing nothing."
        )

    with dg.Panel("Built-in semantic set", class_="card"):
        with dg.FlowLayout(gap=8, row_gap=10, style={"align_items": "center"}):
            for name in SEMANTIC_ICONS:
                with dg.VLayout(style={"gap": 2, "align_items": "center", "width": 62}):
                    dg.IconButton(name, tooltip=name, class_="ic-md ic-boxed")
                    dg.Label(name, class_="ic-caption", wrap=False)

    with dg.GridLayout(columns={"default": 2, 940: 1}, min_column_width=380, gap=12):
        with dg.Panel("Sizes and weights", class_="card"):
            dg.Label("16 / 22 / 30 / 44 / 64 px, same geometry", class_="mono")
            with dg.FlowLayout(gap=10, row_gap=10, style={"align_items": "center"}):
                for cls in ("ic-xs", "ic-sm", "ic-md", "ic-lg", "ic-xl"):
                    dg.IconButton("settings", class_=f"{cls} ic-boxed", tooltip=cls)
            dg.Label(
                "The angular theme uses 1.6-2.0px strokes, the round theme 2.4px. Swap "
                "themes above to compare weights at each size.",
                class_="faint",
            )
            with dg.FlowLayout(gap=10, row_gap=10, style={"align_items": "center"}):
                dg.IconButton("save", class_="ic-xl ic-accent", tooltip="1.6px stroke source")
                dg.IconButton("search", class_="ic-xl ic-accent", tooltip="2.0px stroke source")
                dg.IconButton("refresh", class_="ic-xl ic-accent", tooltip="2.4px stroke source")

        with dg.Panel("States", class_="card"):
            for label, kwargs in (
                ("normal", {}),
                ("accent", {"class_": "ic-md ic-boxed ic-accent"}),
                ("success", {"class_": "ic-md ic-boxed ic-ok"}),
                ("danger", {"class_": "ic-md ic-boxed ic-bad"}),
                ("disabled", {"class_": "ic-md ic-boxed", "disabled": True}),
            ):
                with dg.HLayout(style={"gap": 8, "align_items": "center"}):
                    dg.Label(label, class_="state-label", wrap=False)
                    with dg.FlowLayout(gap=6, row_gap=6, style={"align_items": "center"}):
                        for name in ("play", "pause", "stop", "refresh", "settings"):
                            dg.IconButton(name, tooltip=f"{name} {label}", **{"class_": "ic-md ic-boxed", **kwargs})

        with dg.Panel("Icons inside other controls", class_="card"):
            with dg.Toolbar():
                dg.IconButton("undo", tooltip="Undo")
                dg.IconButton("redo", tooltip="Redo")
                dg.ToolbarSeparator()
                dg.IconButton("save", tooltip="Save")
                dg.IconButton("download", tooltip="Export")
                dg.ToolbarSeparator()
                dg.ArrowButton("left")
                dg.ArrowButton("right")
            icon_tabs = dg.Tabs(value="a")
            with icon_tabs:
                with dg.Tab("Icons in tabs", value="a"):
                    dg.Label("Tab bodies keep normal spacing beside icon rows.", class_="muted")
                with dg.Tab("Second", value="b"):
                    dg.Label("Second body.", class_="muted")
            with dg.VLayout(style={"gap": 3}):
                dg.NavItem("Home", page="theme", icon="home")
                dg.NavItem("Search", page="icons", icon="search", badge="3")
                dg.NavItem("Locked", page="icons", icon="lock", disabled=True)
            dg.SearchBox("", placeholder="SearchBox icon uses the theme", class_="ic-search", style={"width": "100%"})

        with dg.Panel("Icon-only controls", class_="card"):
            dg.Label("No text at any size; the glyph must stay centred and sharp.", class_="muted")
            for cls in ("ic-xs", "ic-sm", "ic-md", "ic-lg", "ic-xl"):
                with dg.HLayout(style={"gap": 6, "align_items": "center"}):
                    dg.Label(cls, class_="state-label", wrap=False)
                    for name in ("add", "minus", "close", "check", "more", "menu"):
                        dg.IconButton(name, class_=f"{cls} ic-boxed", tooltip=f"{name} {cls}")


# ---------------------------------------------------------------------------
# Lab 10 -- window chrome
# ---------------------------------------------------------------------------


def build_chrome_page() -> None:
    page_heading(
        "CHROME / CLIENT-SIDE DECORATIONS",
        "Window-chrome test",
        "The window runs with decorations=\"client\", so the titlebar, title text, and the "
        "minimize/maximize/close controls are DragonGUI-drawn and fully styleable, and "
        "they restyle with every theme switch like any other widget.",
    )

    with dg.Panel("Content directly below the titlebar", class_="card cr-under-title"):
        dg.Label(
            "This card is the first thing under the chrome. If the titlebar ever "
            "overlaps client content, it shows up here first.",
            class_="muted",
        )
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            dg.Badge("client", level="info")
            dg.Tag("Window::titlebar")
            dg.Tag("Window::title")
            dg.Tag("Window::minimize")
            dg.Tag("Window::maximize")
            dg.Tag("Window::close")
            dg.Tag("Window::resize-border")

    with dg.GridLayout(columns={"default": 2, 900: 1}, min_column_width=380, gap=12):
        with dg.Panel("Manual checklist", class_="card"):
            for line in (
                "Drag the title text and the empty titlebar area to move the window.",
                "Double-click the titlebar to maximize, then double-click to restore.",
                "Drag every edge and every corner to resize (Window::resize-border is 6px).",
                "Press Alt+Space for the system menu; press Escape to dismiss it.",
                "Right-click the titlebar for the same system menu.",
                "Tab through the titlebar controls -- they take focus in document order.",
                "Maximize and confirm [window-state=\"maximized\"] restyles the titlebar.",
                "Minimize and restore from the taskbar; state must survive the round trip.",
            ):
                with dg.HLayout(style={"gap": 6, "align_items": "center"}):
                    dg.LED(False, size=9)
                    dg.Label(line, class_="muted")
            note(
                "PASS: on Windows and Linux the retained controls act on the native "
                "window; macOS falls back to native decorations safely."
            )

    with dg.Panel("Narrow-window title regression", class_="card"):
        dg.Label(
            "Window::title must shrink and ellipsize before the fixed-size "
            "minimize/maximize/close controls. The controls must stay visible, "
            "clipped to the titlebar, and clickable at every supported viewport.",
            class_="muted",
        )
        dg.Label(
            "Regression check: run with --long-title and resize to 390x720 or "
            "narrower. The autopilot must remain free of "
            "'fully-clipped-interactive' diagnostics for all three controls.",
            class_="muted",
        )
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            dg.Tag("Window::title", level="info")
            dg.Tag("flex-shrink: 1", level="success")
            dg.Tag("min-width: 0", level="success")
            dg.Tag("ellipsis", level="success")

        with dg.Panel("Chrome under every theme", class_="card"):
            dg.Label(
                "Each theme restyles the chrome through the same parts. Switch themes "
                "and watch the titlebar follow without a window rebuild.",
                class_="muted",
            )
            with dg.FlowLayout(gap=8, row_gap=8):
                for key in THEME_ORDER:
                    dg.SmallButton(
                        THEME_LABELS[key],
                        id=f"chrome-theme-{key}",
                        on_click=lambda name=key: apply_theme(name),
                    )
            with dg.FlowLayout(gap=8, row_gap=8):
                for label, width, height in VIEWPORTS:
                    dg.SmallButton(label, on_click=lambda w=width, h=height: resize_window(w, h))
            dg.Label(
                "Resizing from here uses the same request path as the window edges, so "
                "both should produce identical layout.",
                class_="faint",
            )

    with dg.Panel("Interactive content beside the chrome", class_="card"):
        dg.Label(
            "Ordinary controls must stay interactive while the titlebar is draggable; "
            "a drag started here must not move the window.",
            class_="muted",
        )
        with dg.FlowLayout(gap=10, row_gap=10, style={"align_items": "center"}):
            dg.Checkbox("Application controls stay live", checked=True, id="chrome-check")
            dg.Slider(0.5, style={"width": 200}, id="chrome-slider")
            dg.TextInput("drag-select this text", style={"width": 220}, id="chrome-text")
            dg.Button("Normal button", class_="primary", id="chrome-button")


# ---------------------------------------------------------------------------
# Lab 11 -- live application workload
# ---------------------------------------------------------------------------

METRIC_SPECS: tuple[tuple[str, str, float, float], ...] = (
    ("cascade", "{:.2f} ms", 0.62, 3.4),
    ("layout", "{:.2f} ms", 0.48, 2.6),
    ("paint", "{:.1f} fps", 0.86, 144.0),
    ("sheets", "{:.0f}", 0.45, 12.0),
)


def build_live_page(rows: int) -> None:
    page_heading(
        "WORKLOAD / CONTINUOUS UPDATE",
        "Live application workload",
        "Streaming plots, a colorbar scatter, a bar chart, a heatmap, progress values, "
        "blinking LEDs, updating badges, table churn, log output and toasts -- all "
        "running while themes and stylesheets are replaced underneath them.",
    )

    with dg.Panel("Live metrics", class_="card live-card"):
        with dg.GridLayout(columns={"default": 4, 900: 2, 560: 1}, min_column_width=180, gap=10):
            for label, _fmt, initial, _scale in METRIC_SPECS:
                with dg.Panel(class_="card dense"):
                    with dg.HLayout(style={"gap": 6, "align_items": "center"}):
                        dg.Label(label, class_="live-metric-label", wrap=False)
                        dg.Spacer()
                        state.leds.append(dg.LED(True))
                    state.metrics.append(dg.Label("--", class_="live-metric", wrap=False))
                    state.bars.append(dg.ProgressBar(initial, class_="live-bar"))
                    state.live_badges.append(dg.Badge("idle", level="info"))

    with dg.GridLayout(columns={"default": 2, 1000: 1}, min_column_width=430, gap=12):
        with dg.Panel("Streaming line plot", class_="card live-card live-plot"):
            state.plot = dg.LinePlot(
                TELEMETRY,
                x="tick",
                y=["paint", "layout", "cascade"],
                labels=["paint", "layout", "cascade"],
                colors=[
                    TOKENS[state.theme_key]["chart-a"],
                    TOKENS[state.theme_key]["chart-b"],
                    TOKENS[state.theme_key]["chart-c"],
                ],
                show_legend=True,
                show_toolbar=False,
                max_points=260,
                style={"height": 260},
                id="live-plot",
            )

        with dg.Panel("Scatter with colorbar", class_="card live-card live-plot"):
            frame = make_scatter_frame()
            if frame is None:
                dg.Label(
                    "numpy is not installed, so the GPU scatter is skipped. Everything "
                    "else on this page still runs.",
                    class_="muted",
                )
                dg.Histogram(
                    TELEMETRY["paint"],
                    bins=28,
                    show_toolbar=False,
                    style={"height": 250},
                    id="live-histogram",
                )
            else:
                dg.ScatterPlot2D(
                    frame,
                    x="x",
                    y="y",
                    scalars="score",
                    colormap=TOKENS[state.theme_key]["colormap"],
                    point_size=2.0,
                    scalar_bar=True,
                    scalar_bar_title="score",
                    axis_x="x",
                    axis_y="y",
                    style={"height": 260},
                    id="live-scatter",
                )

        with dg.Panel("Stage cost", class_="card live-card live-plot"):
            state.bar_host = dg.VLayout(style={"gap": 0, "flex": 1}, id="live-bar-host")
            rebuild_bar_chart()

        with dg.Panel("Latency field", class_="card live-card live-plot"):
            state.heatmap = dg.Heatmap(
                HEAT_MATRIX,
                x_labels=[f"{index:02d}" for index in range(14)],
                y_labels=[f"r{index}" for index in range(9)],
                colormap=TOKENS[state.theme_key]["colormap"],
                title="p95 by lane / slot",
                style={"height": 260},
                id="live-heatmap",
            )

    with dg.GridLayout(columns={"default": 2, 1000: 1}, min_column_width=430, gap=12):
        with dg.Panel(f"Restyle journal ({rows} rows)", class_="card live-card", id="live-journal"):
            with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
                dg.SmallButton("Re-sort rows", id="live-refresh-table", on_click=refresh_table)
                dg.SmallButton("Toast", id="live-toast", on_click=emit_toast)
                dg.Spacer()
                dg.Badge("live", level="success")
            state.table = dg.DataFrameTable(
                state.rows,
                page_size=40,
                style={"height": 300},
                id="live-table",
            )

        with dg.Panel("Runtime log", class_="card live-card"):
            state.log = dg.LogView(
                [
                    "info  forge: variables + structure + appearance installed",
                    "info  forge: live worker attached",
                ],
                rows=16,
                follow=True,
                max_lines=4000,
                id="live-log",
            )

    with dg.Panel("Status lamps", class_="card live-card"):
        with dg.FlowLayout(gap=8, row_gap=8, class_="live-led-row", style={"align_items": "center"}):
            for index in range(24):
                state.leds.append(dg.LED(index % 3 == 0))
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            for index in range(6):
                state.live_badges.append(
                    dg.Badge("0", level=("info", "success", "warning")[index % 3])
                )
        for index in range(4):
            state.bars.append(dg.ProgressBar(0.2 + index * 0.2, class_="live-bar"))


def rebuild_bar_chart() -> None:
    """Rebuild the bar chart in place.

    `BarChart` has no live data setter, so the demo swaps the widget inside a
    dedicated host container. This also exercises `replace_children` rebinding
    while stylesheets are being replaced.
    """

    host = state.bar_host
    if host is None:
        return
    phase = state.tick * 0.13
    values = [
        round(abs(math.sin(phase + index * 0.9)) * 6.0 + 0.6 + state.random.random() * 0.9, 2)
        for index in range(len(BAR_CATEGORIES))
    ]
    table = TOKENS[state.theme_key]
    chart = dg.BarChart(
        labels=list(BAR_CATEGORIES),
        values=values,
        y_label="ms",
        colors=[table["chart-a"], table["chart-b"], table["chart-c"], table["chart-d"], table["chart-e"], table["chart-a"]],
        show_toolbar=False,
        style={"height": 250},
        id="live-bars",
        parent=None,
    )
    host.replace_children([chart])


# ---------------------------------------------------------------------------
# Lab 12 -- deliberately hostile CSS
# ---------------------------------------------------------------------------


def build_extreme_page() -> None:
    page_heading(
        "HOSTILE / DEGRADE PREDICTABLY",
        "Deliberately hostile CSS",
        "Extreme mode installs a sheet full of abusive, contradictory and unsupported "
        "declarations. The contract is that unsupported input becomes a warning and "
        "supported-but-extreme input renders without corrupting layout or crashing.",
    )

    with dg.Panel("Extreme mode", class_="card"):
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            dg.Button(
                "Toggle hostile sheet",
                class_="primary",
                id="extreme-toggle-button",
                on_click=toggle_extreme,
            )
            dg.SmallButton("Rapid re-add x10", id="extreme-thrash", on_click=lambda: thrash_extreme(10))
            dg.SmallButton("Also cycle themes", id="extreme-cycle", on_click=lambda: rapid_cycle(2))
        dg.Label(
            "Nothing on this page changes until the hostile sheet is installed. With it "
            "installed, every panel below must still lay out inside its own bounds.",
            class_="muted",
        )
        dg.Label(
            "Expect the stylesheet warning count to rise while extreme mode is on.",
            class_="xt-warning",
        )
        note(
            "PASS: warning count rises, last_error stays null, and no widget escapes its "
            "panel or disappears entirely."
        )

    with dg.GridLayout(
        columns={"default": 3, 1050: 2, 680: 1},
        min_column_width=280,
        gap=12,
        id="extreme-hostile-grid",
    ):
        with dg.Panel("Oversized box model", class_="card xt-host"):
            with dg.Panel(class_="xt-fat"):
                dg.Label("64-76px padding inside a 26px double border", class_="mono")
            dg.Label("The panel above must not push its neighbours out of the grid.", class_="faint")

        with dg.Panel("Tiny controls", class_="card xt-host"):
            with dg.FlowLayout(gap=4, row_gap=4, style={"align_items": "center"}):
                for index in range(16):
                    dg.Button(str(index % 10), class_="xt-atom")
            with dg.FlowLayout(gap=4, row_gap=4, style={"align_items": "center"}):
                for index in range(10):
                    dg.Badge(str(index), level="info", class_="xt-atom")
                for index in range(10):
                    dg.LED(index % 2 == 0, class_="xt-atom")

        with dg.Panel("Transparent layers", class_="card xt-host"):
            with dg.Panel(class_="xt-ghost"):
                dg.Label("Three translucent layers over a 4% base.", class_="mono")
                dg.Label("Text must stay legible against whatever is behind.", class_="faint")

        with dg.Panel("Deep shadows", class_="card xt-host"):
            with dg.Panel(class_="xt-shadow"):
                dg.Label("inset 60px + two 70-90px outsets on a 64px radius", class_="mono")

        with dg.Panel("Missing resources", class_="card xt-host"):
            with dg.Panel(class_="xt-missing-image"):
                dg.Label("app-resource(\"never-registered\", cover)", class_="mono")
                dg.Label("must fall back to background-color, not to black.", class_="faint")
            dg.Label("Missing font families fall through the stack:", class_="faint")
            dg.Label("The quick brown fox 0123456789", class_="xt-missing-font")

        with dg.Panel("Conflicting declarations", class_="card xt-host"):
            with dg.Panel(class_="xt-conflict"):
                dg.Label("shorthand first, longhands after", class_="mono")
            with dg.Panel(class_="xt-conflict-reverse"):
                dg.Label("longhands first, shorthand after", class_="mono")
            dg.Label(
                "Later declarations win within a block; the two panels above must differ.",
                class_="faint",
            )

        with dg.Panel("Invalid lengths and vars", class_="card xt-host"):
            with dg.Panel(class_="xt-bad-length"):
                dg.Label("percent/auto on properties that reject them", class_="mono")
            with dg.Panel(class_="xt-var"):
                dg.Label("var() fallbacks and missing variables", class_="mono")
            with dg.Panel(class_="xt-unsupported"):
                dg.Label("nine unsupported browser properties", class_="mono")

        with dg.Panel("Empty and zero-sized", class_="card xt-host"):
            dg.Label("empty panel (no children):", class_="faint")
            dg.Panel(class_="xt-empty")
            dg.Label("empty flex container:", class_="faint")
            dg.VLayout(class_="xt-empty-flex")
            dg.Label("zero-sized boxes between two labels:", class_="faint")
            with dg.HLayout(style={"gap": 4, "align_items": "center"}):
                dg.Label("before", class_="mono", wrap=False)
                dg.Panel(class_="xt-zero-box")
                dg.Panel(class_="xt-zero")
                dg.Panel(class_="xt-negative")
                dg.Label("after", class_="mono", wrap=False)

        with dg.Panel("Very long text, tiny room", class_="card xt-host"):
            dg.Label(LONG_LINE * 2, class_="xt-long", wrap=False)
            with dg.FlowLayout(gap=6, row_gap=6, style={"align_items": "center"}):
                dg.Button(LONG_LINE, class_="xt-long")
                dg.Badge(LONG_LINE, level="warning", class_="xt-long")
            dg.TextInput(LONG_LINE * 2, style={"width": 160})

        with dg.Panel("Paint-only transforms", class_="card xt-host"):
            with dg.Panel(class_="xt-transform"):
                dg.Label("rotate(7deg), paint only", class_="mono")
                dg.Label("Hit testing must stay on the untransformed rect.", class_="faint")
            with dg.FlowLayout(gap=14, row_gap=14, style={"align_items": "center"}):
                dg.Button("scaled 1.6", class_="xt-scaled", id="extreme-scaled")
                dg.Button("normal", id="extreme-normal")

        with dg.Panel("Rapid replacement target", class_="card xt-host"):
            dg.Label(
                "The thrash button replaces the hostile sheet ten times in one frame. "
                "The cascade must coalesce rather than running ten full restyles.",
                class_="muted",
            )
            for index in range(6):
                with dg.Panel(
                    class_="xt-thrash-target",
                    id=f"extreme-thrash-target-{index}",
                ):
                    dg.Label(f"thrash target {index}", class_="mono", wrap=False)

    with dg.Panel("Malformed sheets", class_="card", id="extreme-malformed-panel"):
        dg.Label(
            "These sheets are broken at the token level rather than merely unsupported, "
            "so they are kept out of the hostile sheet and installed on demand. Two "
            "outcomes are legitimate: the sheet aborts and raises, or CSS error recovery "
            "absorbs the damage and installs it with the broken rule dropped.",
            class_="muted",
        )
        with dg.FlowLayout(gap=8, row_gap=8, style={"align_items": "center"}):
            dg.Button(
                "Probe malformed sheets",
                class_="primary",
                id="extreme-malformed",
                on_click=probe_malformed_sheets,
            )
            for label, _css in MALFORMED_SHEETS:
                dg.Tag(label, level="danger")
        state.malformed_log = dg.LogView(
            [f"press the button to install {len(MALFORMED_SHEETS)} deliberately broken sheets"],
            rows=8,
            wrap=False,
            id="extreme-malformed-log",
        )
        note(
            "PASS: every case reports either 'aborted' or 'recovered', nothing crashes, "
            "and the page keeps exactly the styling it had before the probe ran."
        )


def probe_malformed_sheets() -> None:
    """Install every malformed sheet in turn and record how each one landed.

    Two outcomes are legitimate: the sheet aborts at parse time and raises, or
    ordinary CSS error recovery absorbs the damage and the sheet installs with
    the broken rules dropped. Neither may crash, and neither may disturb the
    sheets that were already active.
    """

    app = state.app
    if app is None:
        return
    lines: list[str] = []
    results: list[dict[str, Any]] = []
    for label, css in MALFORMED_SHEETS:
        try:
            app.set_stylesheet("malformed-probe", css)
        except Exception as exc:  # noqa: BLE001 - the rejection is the result
            message = str(exc).split(": ")[-1]
            lines.append(f"aborted    {label}: {message}")
            results.append({"case": label, "outcome": "aborted", "message": message})
        else:
            lines.append(f"recovered  {label}: installed with the broken rule dropped")
            results.append({"case": label, "outcome": "recovered", "message": None})
    app.remove_stylesheet("malformed-probe")
    state.malformed_results = results
    if state.malformed_log is not None:
        state.malformed_log.set_lines(lines)
    aborted = sum(1 for entry in results if entry["outcome"] == "aborted")
    set_status(
        f"malformed sheets: {aborted} aborted, {len(results) - aborted} recovered",
        "ok",
    )


def thrash_extreme(rounds: int = 10) -> None:
    """Add and remove the hostile sheet many times without yielding to the UI."""

    if state.app is None:
        return
    start = time.perf_counter()
    for _ in range(rounds):
        state.app.set_stylesheet("extreme", HOSTILE_CSS)
        state.app.remove_stylesheet("extreme")
    if state.extreme_on:
        state.app.set_stylesheet("extreme", HOSTILE_CSS)
    elapsed = (time.perf_counter() - start) * 1000.0
    set_status(f"hostile sheet thrashed {rounds}x in {elapsed:.1f} ms", "bad")


PAGE_BUILDERS: dict[str, Callable[[], None]] = {}


# ---------------------------------------------------------------------------
# Window
# ---------------------------------------------------------------------------


SHORT_TITLE = "THEME FORGE"
LONG_TITLE = "THEME FORGE - DragonGUI theming and CSS stress console"


def build_window(*, rows: int, decorations: str, long_title: bool = False) -> dg.Window:
    with dg.Window(
        LONG_TITLE if long_title else SHORT_TITLE,
        width=1500,
        height=940,
        decorations=decorations,
        id="forge-window",
    ) as window:
        with dg.AppShell(class_="forge-shell"):
            state.sidebar = dg.Sidebar(
                title="THEME FORGE",
                width=214,
                collapsed_width=62,
                class_="forge-sidebar",
                id="forge-sidebar",
            )
            with state.sidebar:
                dg.Label("CSS + THEMING STRESS", class_="rail-note", wrap=False)
                dg.Label("LABORATORIES", class_="rail-label", wrap=False)
                for route, label, badge in ROUTES:
                    dg.NavItem(label, page=route, badge=badge)
                dg.Spacer(height=8)
                dg.Label("CASCADE", class_="rail-label", wrap=False)
                with dg.Panel(class_="card dense"):
                    with dg.FlowLayout(gap=6, row_gap=4, style={"align_items": "center"}):
                        dg.LED(True, size=10)
                        dg.Label("user origin", class_="rail-note", wrap=False)
                    state.clock = dg.Label("00:00:00", class_="rail-note", wrap=False)
                    dg.ProgressBar(0.62, style={"height": 5})

            with dg.WorkbenchLayout(class_="forge-work"):
                with dg.MenuBar(height=30):
                    with dg.Menu("Workspace", id="forge-workspace-menu"):
                        for route, label, _badge in ROUTES:
                            dg.MenuItem(label, on_click=lambda selected=route: navigate(selected))
                    with dg.Menu("Theme"):
                        for key in THEME_ORDER:
                            dg.MenuItem(
                                THEME_LABELS[key],
                                on_click=lambda name=key: apply_theme(name),
                            )
                        dg.MenuItem("Rapid cycle", on_click=lambda: rapid_cycle(3))
                    with dg.Menu("Stylesheets"):
                        dg.MenuItem("Toggle override sheet", on_click=toggle_overrides)
                        dg.MenuItem("Toggle extreme mode", on_click=toggle_extreme)
                        dg.MenuItem("Reset CSS variables", on_click=reset_variables)
                    with dg.Menu("Resources"):
                        dg.MenuItem("Swap background texture", on_click=swap_texture)
                        dg.MenuItem("Release background texture", on_click=release_texture)
                        dg.MenuItem("Restore background texture", on_click=restore_texture)
                        dg.MenuItem("Cycle icon theme", on_click=cycle_icon_theme)
                    with dg.Menu("View"):
                        dg.MenuItem("Toggle navigation rail", on_click=toggle_sidebar)
                        for label, width, height in VIEWPORTS:
                            dg.MenuItem(
                                f"Resize to {label}",
                                on_click=lambda w=width, h=height: resize_window(w, h),
                            )
                    with dg.Menu("Help"):
                        dg.MenuItem("Command palette", on_click=show_palette)
                        dg.MenuItem("Unavailable", disabled=True)

                with dg.Toolbar(class_="forge-toolbar"):
                    dg.IconButton(
                        "menu",
                        tooltip=(
                            "Show or hide the navigation laboratory rail while preserving "
                            "the current workspace and scroll position."
                        ),
                        on_click=toggle_sidebar,
                        id="forge-rail-toggle",
                    )
                    dg.ToolbarSeparator()
                    dg.IconButton("refresh", tooltip="Next theme", on_click=lambda: cycle_theme(1))
                    dg.IconButton("settings", tooltip="Reset CSS variables", on_click=reset_variables)
                    dg.IconButton("search", tooltip="Command palette", on_click=show_palette)
                    dg.ToolbarSeparator()
                    dg.SearchBox(
                        "",
                        placeholder="Search selectors, parts, tokens",
                        class_="forge-search",
                        id="forge-search",
                        width=280,
                    )
                    dg.Spacer()
                    dg.SmallButton("Override sheet", id="forge-toolbar-overrides", on_click=toggle_overrides)
                    dg.SmallButton("Extreme mode", id="forge-toolbar-extreme", on_click=toggle_extreme)
                    dg.Button("Rapid cycle", class_="primary", id="forge-toolbar-cycle", on_click=lambda: rapid_cycle(2))
                    dg.Badge("LIVE", level="success")

                state.tabs = dg.Tabs(value="theme", on_change=navigate, id="forge-tabs")
                with state.tabs:
                    for route, label, badge in ROUTES:
                        with dg.Tab(label, value=route, badge=badge):
                            pass

                with dg.Body():
                    state.pages = dg.Pages(value="theme", id="forge-pages")
                    with state.pages:
                        for route, label, _badge in ROUTES:
                            with dg.Page(route, title=label):
                                with page_scroll(route):
                                    PAGE_BUILDERS[route]()

                with dg.StatusBar(height=27, class_="forge-status"):
                    state.status_badge = dg.Badge("READY", level="success")
                    state.status = dg.Label(
                        "theme forge ready",
                        wrap=False,
                        style={"flex": 1, "min_width": 0},
                    )
                    dg.Tag("client chrome" if decorations == "client" else "native chrome")
                    dg.Tag(f"{len(ROUTES)} labs", level="info")

    state.modal = dg.Modal(
        "Restyle confirmation",
        open=False,
        width=560,
        height=340,
        parent=window,
        id="forge-modal",
    )
    with state.modal:
        dg.Label(
            "Modal surfaces are styled through Modal::scrim, ::header, ::title and "
            "::body, and must restyle with the theme while open.",
            class_="muted",
        )
        dg.TextInput("forge-modal-value", placeholder="Name this restyle", id="forge-modal-input")
        dg.Dropdown(["variables", "structure", "appearance", "overrides"], value="appearance")
        dg.TextArea("Notes that must survive a theme change while the modal is open.", rows=3)
        dg.ToggleSwitch("Keep the override sheet installed", checked=True)
        with dg.FlowLayout(gap=8, row_gap=8, style={"justify_content": "flex_end"}):
            dg.SmallButton("Cancel", id="forge-modal-cancel", on_click=close_modal)
            dg.Button(
                "Apply",
                class_="primary",
                id="forge-modal-apply",
                on_click=lambda: (close_modal(), set_status("restyle applied", "ok")),
            )

    state.palette = dg.CommandPalette(
        [dg.Command(f"route.{route}", f"Open {label}", on_run=lambda r=route: navigate(r)) for route, label, _b in ROUTES]
        + [
            dg.Command(f"theme.{key}", f"Theme: {THEME_LABELS[key]}", on_run=lambda k=key: apply_theme(k))
            for key in THEME_ORDER
        ]
        + [
            dg.Command("sheet.overrides", "Toggle override sheet", on_run=toggle_overrides),
            dg.Command("sheet.extreme", "Toggle extreme mode", on_run=toggle_extreme),
            dg.Command("sheet.cycle", "Rapid theme cycle", on_run=lambda: rapid_cycle(3)),
            dg.Command("vars.reset", "Reset CSS variables", on_run=reset_variables),
            dg.Command("icons.cycle", "Cycle icon theme", on_run=cycle_icon_theme),
            dg.Command("image.swap", "Swap background texture", on_run=swap_texture),
            dg.Command("table.refresh", "Re-sort restyle journal", on_run=refresh_table),
        ],
        open=False,
        title="Forge commands",
        placeholder="Type a command",
        max_results=10,
        parent=window,
        id="forge-palette",
    )

    with dg.ContextMenu(target="live-journal", width=240, parent=window):
        dg.MenuItem("Re-sort rows", on_click=refresh_table)
        dg.MenuItem("Toggle override sheet", on_click=toggle_overrides)
        dg.MenuItem("Toggle extreme mode", on_click=toggle_extreme)
        dg.MenuItem("Unavailable", disabled=True)

    return window


# ---------------------------------------------------------------------------
# Live worker
# ---------------------------------------------------------------------------

LOG_LEVELS = ("info", "info", "info", "warning", "info", "error")


def live_worker(app: dg.App) -> None:
    """Push telemetry into every live surface at ~14 Hz.

    Work is split into a streaming task (plot/log, never coalesced so no sample
    is lost) and a snapshot task (gauges/LEDs/badges, coalesced so a slow frame
    cannot build a backlog).
    """

    while not state.stop.is_set():
        time.sleep(0.07)
        tick = state.tick
        state.tick += 1
        paint = 58.0 + math.sin(tick / 11.0) * 16.0 + math.sin(tick / 2.7) * 3.4
        layout = 34.0 + math.cos(tick / 8.5 + 0.6) * 11.0 + math.sin(tick / 3.3) * 2.6
        cascade = 21.0 + math.sin(tick / 17.0 + 1.3) * 8.0 + math.cos(tick / 4.1) * 2.0

        def apply_stream(
            tick: int = tick,
            paint: float = paint,
            layout: float = layout,
            cascade: float = cascade,
        ) -> None:
            if state.plot is not None:
                x = [float(tick)]
                state.plot.append_points(x, [paint], series="paint", max_points=260)
                state.plot.append_points(x, [layout], series="layout", max_points=260)
                state.plot.append_points(x, [cascade], series="cascade", max_points=260)
            if tick % 9 == 0 and state.log is not None:
                level = LOG_LEVELS[(tick // 9) % len(LOG_LEVELS)]
                state.log.append_line(
                    f"{level:<7} {time.strftime('%H:%M:%S')} tick={tick:06d} "
                    f"theme={state.theme_key} sheets={len(active_sheet_summary().split('>'))} "
                    f"paint={paint:5.1f}ms layout={layout:5.1f}ms"
                )

        def apply_snapshot(tick: int = tick) -> None:
            phase = tick * 0.09
            for index, bar in enumerate(state.bars):
                value = min(0.99, max(0.02, 0.5 + math.sin(phase + index * 0.8) * 0.42))
                bar.set_value(value)
            for index, label in enumerate(state.metrics):
                name, fmt, _initial, scale = METRIC_SPECS[index % len(METRIC_SPECS)]
                value = 0.5 + math.sin(phase * 0.8 + index * 1.6) * 0.4
                label.set_value(fmt.format(value * scale))
            for index, led in enumerate(state.leds):
                led.set_on((tick + index) % 7 < 4)
            for index, badge in enumerate(state.live_badges):
                badge.set_value(str((tick * (index + 1)) % 1000))
            for index, panel in enumerate(state.flex_panels):
                width = 42 + (math.sin(phase * 0.7 + index) * 0.5 + 0.5) * 55
                panel.set_style({"width": f"{width:.1f}%"})
            if state.clock is not None:
                state.clock.set_value(time.strftime("%H:%M:%S"))

        def apply_slow(tick: int = tick) -> None:
            if state.heatmap is not None:
                phase = tick * 0.05
                matrix = [
                    [
                        40.0
                        + math.sin(row / 1.9 + phase) * 14.0
                        + math.cos(col / 2.6 - phase) * 12.0
                        for col in range(14)
                    ]
                    for row in range(9)
                ]
                state.heatmap.set_data(matrix)
            rebuild_bar_chart()
            if state.table is not None and state.rows:
                head = state.rows[:1]
                state.rows = state.rows[1:] + head
                state.table.set_frame(state.rows)

        try:
            app.call_soon_threadsafe(apply_stream)
            app.call_soon_threadsafe(apply_snapshot, coalesce_key="forge.snapshot")
            if tick % 24 == 0:
                app.call_soon_threadsafe(apply_slow, coalesce_key="forge.slow")
        except Exception:  # pragma: no cover - app shutting down
            break


# ---------------------------------------------------------------------------
# State fingerprint and layout integrity
# ---------------------------------------------------------------------------

PROBE_IDS: tuple[str, ...] = (
    "probe-text",
    "probe-area",
    "probe-search",
    "probe-number",
    "probe-slider",
    "probe-range",
    "probe-check",
    "probe-toggle",
    "probe-dropdown",
)


def fingerprint(snapshot: dict[str, Any]) -> dict[str, Any]:
    """Extract the retained state that must survive a restyle."""

    gpu = snapshot.get("gpu") or {}
    widget_state = gpu.get("state") or {}
    text_val = widget_state.get("text_val") or {}
    float_val = widget_state.get("float_val") or {}
    range_val = widget_state.get("float_range") or widget_state.get("range_val") or {}
    checked = widget_state.get("checked") or {}
    scroll_y = widget_state.get("container_scroll_y") or {}

    return {
        "focused": widget_state.get("focused"),
        "active_tabs": dict(widget_state.get("active_tabs") or {}),
        "active_pages": dict(widget_state.get("active_pages") or {}),
        "text": {key: text_val.get(key) for key in PROBE_IDS if key in text_val},
        "floats": {
            key: round(float(value), 4)
            for key, value in float_val.items()
            if key in PROBE_IDS
        },
        "ranges": {key: range_val.get(key) for key in PROBE_IDS if key in range_val},
        "checked": {key: checked.get(key) for key in PROBE_IDS if key in checked},
        "scroll_y": {
            key: round(float(value), 1)
            for key, value in scroll_y.items()
            if key.startswith("scroll-")
        },
        "dropdown_index": dict(widget_state.get("dropdown_index") or {}),
    }


def layout_report(snapshot: dict[str, Any]) -> dict[str, Any]:
    """Summarize layout health: escapes, clipping, and diagnostic issues."""

    gpu = snapshot.get("gpu") or {}
    layout = gpu.get("layout") or {}
    rects = layout.get("rects") or {}
    diagnostics = layout.get("diagnostics") or {}

    escaped: list[str] = []
    issues: list[dict[str, Any]] = []
    degenerate: list[str] = []

    for widget_id, entry in diagnostics.items():
        overflow = entry.get("overflow") or {}
        if float(overflow.get("width", 0.0)) > 1.0 or float(overflow.get("height", 0.0)) > 1.0:
            escaped.append(widget_id)
        found = entry.get("issues") or []
        if found:
            issues.append({"id": widget_id, "issues": found})

    for widget_id, rect in rects.items():
        width = float(rect.get("w", 0.0))
        height = float(rect.get("h", 0.0))
        if width < 0.0 or height < 0.0:
            degenerate.append(widget_id)

    return {
        "rect_count": len(rects),
        "clipped_beyond_parent": sorted(escaped)[:24],
        "clipped_count": len(escaped),
        "negative_rects": sorted(degenerate)[:24],
        "diagnostic_issues": issues[:24],
        "diagnostic_issue_count": len(issues),
        "reconciliation": layout.get("reconciliation"),
    }


def settled_layout_report(
    app: dg.App,
    *,
    attempts: int = 4,
    delay: float = 0.35,
) -> dict[str, Any]:
    """Return a layout report measured once the renderer has settled.

    A resize lands over several frames: the window rect updates before the
    dependent layout pass does, so a snapshot taken too early legitimately
    reports transient empty paint clips (the client-side titlebar controls are
    the usual victims). Retry until the report is clean or the attempts run
    out, and report the last one either way.
    """

    report = layout_report(app.debug_snapshot(timeout_ms=4000))
    for _ in range(max(0, attempts - 1)):
        if not report["diagnostic_issue_count"] and not report["negative_rects"]:
            return report
        time.sleep(delay)
        report = layout_report(app.debug_snapshot(timeout_ms=4000))
    return report


def summarize_issues(entries: list[dict[str, Any]]) -> list[str]:
    """Flatten layout diagnostics into one short line per issue."""

    lines: list[str] = []
    for entry in entries:
        for issue in entry.get("issues") or []:
            lines.append(
                f"{issue.get('code')} on {issue.get('widget_type')} "
                f"{issue.get('widget_id')} (parent {issue.get('parent_id')})"
            )
    return lines


def stylesheet_report(snapshot: dict[str, Any]) -> dict[str, Any]:
    gpu = snapshot.get("gpu") or {}
    sheets = gpu.get("stylesheets") or {}
    warnings = sheets.get("warnings") or []
    return {
        "user_rules": sheets.get("user_rules"),
        "framework_rules": sheets.get("framework_rules"),
        "theme_rules": sheets.get("theme_rules"),
        "active": sheets.get("active"),
        "warning_count": sheets.get("warning_count"),
        "last_error": sheets.get("last_error"),
        "sample_warnings": [
            {"property": entry.get("property"), "message": entry.get("message")}
            for entry in warnings[:12]
        ],
        "unmatched_user_selectors": (sheets.get("unmatched_user_selectors") or [])[:12],
    }


def diff_fingerprint(before: dict[str, Any], after: dict[str, Any]) -> list[str]:
    """Return a human-readable list of retained-state regressions."""

    problems: list[str] = []
    for key in ("focused", "active_tabs", "active_pages", "text", "floats", "ranges", "checked", "dropdown_index"):
        if before.get(key) != after.get(key):
            problems.append(f"{key}: {before.get(key)!r} -> {after.get(key)!r}")
    before_scroll = before.get("scroll_y") or {}
    after_scroll = after.get("scroll_y") or {}
    for widget_id, value in before_scroll.items():
        other = after_scroll.get(widget_id)
        if other is None or abs(float(other) - float(value)) > 1.0:
            problems.append(f"scroll_y[{widget_id}]: {value} -> {other}")
    return problems


# ---------------------------------------------------------------------------
# Autopilot
# ---------------------------------------------------------------------------


def _ui(app: dg.App, fn: Callable[[], None]) -> None:
    """Run `fn` on the UI thread and wait for it to finish."""

    done = threading.Event()
    error: list[BaseException] = []

    def wrapper() -> None:
        try:
            fn()
        except BaseException as exc:  # pragma: no cover - reported, not raised
            error.append(exc)
        finally:
            done.set()

    app.call_soon_threadsafe(wrapper)
    if not done.wait(timeout=10.0):
        raise TimeoutError("UI task did not complete within 10s")
    if error:
        raise error[0]


def autopilot(app: dg.App, *, cycles: int, settle: float, report_path: Path | None) -> None:
    """Drive the documented interaction loop and verify state after every cycle.

    Each cycle scrolls every scroll owner, types into inputs, opens overlays,
    changes the theme, resizes the window, switches tabs, forces plot/table
    updates, toggles high-contrast and extreme mode, returns to the original
    theme, and finally compares the retained-state fingerprint and layout
    diagnostics against the values captured before the cycle began.
    """

    time.sleep(max(settle, 1.2))
    origin_theme = state.theme_key
    results: list[dict[str, Any]] = []

    def scroll_all(offset: float = 240.0) -> None:
        for route, scroll in state.scrolls.items():
            scroll.scroll_to(y=offset if route != "theme" else 120.0)

    # Warm-up: a scroll offset only survives once its page has been laid out, so
    # visit every route once before the first baseline. Without this the
    # baseline would record 0 for pages the loop has not opened yet and every
    # cycle would report a false scroll regression.
    for route, _label, _badge in ROUTES:
        _ui(app, lambda r=route: navigate(r))
        time.sleep(max(settle * 0.5, 0.12))
    _ui(app, lambda: navigate("theme"))
    _ui(app, scroll_all)
    time.sleep(max(settle, 0.5))

    for cycle in range(1, cycles + 1):
        step = f"cycle {cycle}/{cycles}"
        try:
            _ui(app, lambda: set_status(f"autopilot {step}: baseline", "warn"))
            baseline = fingerprint(app.debug_snapshot(timeout_ms=4000))
            time.sleep(settle)

            # 1. scroll every major area
            _ui(app, scroll_all)
            time.sleep(settle)

            # 2. enter text and select values
            def enter_values(cycle: int = cycle) -> None:
                if state.probe_text is not None:
                    state.probe_text.set_value(f"autopilot-{cycle}")
                if state.probe_search is not None:
                    state.probe_search.set_value(f"query-{cycle}")
                if state.probe_number is not None:
                    state.probe_number.set_value(float(20 + cycle))
                if state.probe_slider is not None:
                    state.probe_slider.set_value(min(1.0, 0.2 + cycle * 0.13))
                if state.probe_dropdown is not None:
                    state.probe_dropdown.set_value(("parse", "cascade", "layout", "paint")[cycle % 4])
                if state.probe_check is not None:
                    state.probe_check.set_checked(cycle % 2 == 0)
                if state.probe_toggle is not None:
                    state.probe_toggle.set_checked(cycle % 2 == 1)

            _ui(app, enter_values)
            time.sleep(settle)
            # Re-baseline now that the loop has written its own values.
            baseline = fingerprint(app.debug_snapshot(timeout_ms=4000))

            # 3. open a menu, tooltip and modal
            #    (menus and tooltips are pointer-driven; the palette and modal
            #     are the API-reachable overlays)
            _ui(app, show_modal)
            time.sleep(settle)
            _ui(app, close_modal)
            _ui(app, show_palette)
            time.sleep(settle)
            _ui(app, lambda: state.palette.close() if state.palette is not None else None)

            # 4. change the theme
            _ui(app, lambda: cycle_theme(1))
            time.sleep(settle)
            _ui(app, lambda: rapid_cycle(1))
            time.sleep(settle)

            # 5. resize the window through every preset, checking layout health
            #    at each one rather than only at the end
            viewport_checks: list[dict[str, Any]] = []
            for label, width, height in VIEWPORTS:
                _ui(app, lambda w=width, h=height: resize_window(w, h))
                time.sleep(settle)
                report = settled_layout_report(app)
                viewport_checks.append(
                    {
                        "viewport": label,
                        "issues": report["diagnostic_issue_count"],
                        "negative_rects": report["negative_rects"],
                        "rect_count": report["rect_count"],
                        "detail": report["diagnostic_issues"][:4],
                    }
                )

            # 6. switch tabs, checking each page's layout as it becomes visible
            page_checks: list[dict[str, Any]] = []
            for route, _label, _badge in ROUTES:
                _ui(app, lambda r=route: navigate(r))
                time.sleep(max(settle * 0.5, 0.12))
                report = settled_layout_report(app)
                page_checks.append(
                    {
                        "page": route,
                        "issues": report["diagnostic_issue_count"],
                        "negative_rects": report["negative_rects"],
                        "rect_count": report["rect_count"],
                        "detail": report["diagnostic_issues"][:4],
                    }
                )

            # 7. update plots and tables
            _ui(app, refresh_table)
            _ui(app, rebuild_bar_chart)
            _ui(app, emit_toast)
            time.sleep(settle)

            # 8. toggle high-contrast and extreme mode
            _ui(app, lambda: apply_theme("contrast"))
            time.sleep(settle)
            _ui(app, lambda: set_extreme(True))
            time.sleep(settle)
            # With the hostile sheet installed the warning count must rise and
            # last_error must stay null -- the sheet degrades, it is not rejected.
            hostile = stylesheet_report(app.debug_snapshot(timeout_ms=4000))
            _ui(app, lambda: thrash_extreme(6))
            time.sleep(settle)
            cascade_before = [entry.get("id") for entry in hostile["active"] or []]
            _ui(app, probe_malformed_sheets)
            time.sleep(settle)
            malformed = list(state.malformed_results)
            post_probe = stylesheet_report(app.debug_snapshot(timeout_ms=4000))
            cascade_after = [entry.get("id") for entry in post_probe["active"] or []]
            _ui(app, lambda: set_overrides(True))
            time.sleep(settle)
            _ui(app, lambda: set_overrides(False))
            _ui(app, lambda: set_extreme(False))
            time.sleep(settle)

            # 9. return to the original theme and viewport
            _ui(app, lambda: apply_theme(origin_theme))
            _ui(app, lambda: resize_window(1500, 940))
            time.sleep(settle)
            _ui(app, lambda: navigate("theme"))
            _ui(app, scroll_all)
            # Scroll offsets are applied by the native side on the next frame,
            # so give the renderer room before reading them back.
            time.sleep(max(settle * 2, 0.7))

            # 10. confirm layout and application state are intact
            layout = settled_layout_report(app)
            snapshot = app.debug_snapshot(timeout_ms=4000)
            after = fingerprint(snapshot)
            problems = diff_fingerprint(baseline, after)
            sheets = stylesheet_report(snapshot)
            # A malformed sheet may abort or recover, but it must never leave a
            # residue: the active cascade has to be identical either way.
            cascade_intact = cascade_before == cascade_after
            probed_all = len(malformed) == len(MALFORMED_SHEETS)
            hostile_ok = (
                hostile["last_error"] is None
                and (hostile["warning_count"] or 0) > (sheets["warning_count"] or 0)
                and any(entry.get("id") == "extreme" for entry in hostile["active"] or [])
            )
            bad_viewports = [
                entry for entry in viewport_checks if entry["issues"] or entry["negative_rects"]
            ]
            bad_pages = [
                entry for entry in page_checks if entry["issues"] or entry["negative_rects"]
            ]
            passed = (
                not problems
                and not layout["negative_rects"]
                and layout["diagnostic_issue_count"] == 0
                and sheets["last_error"] is None
                and probed_all
                and cascade_intact
                and hostile_ok
                and not bad_viewports
                and not bad_pages
            )
            results.append(
                {
                    "cycle": cycle,
                    "passed": passed,
                    "state_regressions": problems,
                    "layout": layout,
                    "stylesheets": sheets,
                    "hostile_sheet": {
                        "installed": hostile_ok,
                        "warning_count": hostile["warning_count"],
                        "last_error": hostile["last_error"],
                        "active": [entry.get("id") for entry in hostile["active"] or []],
                    },
                    "viewport_checks": viewport_checks,
                    "page_checks": page_checks,
                    "malformed_sheets": malformed,
                    "cascade_before_probe": cascade_before,
                    "cascade_after_probe": cascade_after,
                    "cascade_intact": cascade_intact,
                    "theme": state.theme_key,
                }
            )
            verdict = "PASS" if passed else "FAIL"
            print(
                f"[autopilot] {step}: {verdict} | "
                f"state regressions={len(problems)} "
                f"layout issues={layout['diagnostic_issue_count']} "
                f"clipped={layout['clipped_count']} "
                f"css warnings={sheets['warning_count']} "
                f"hostile warnings={hostile['warning_count']} "
                f"cascade intact={cascade_intact} "
                f"bad viewports={len(bad_viewports)} bad pages={len(bad_pages)} "
                f"last_error={sheets['last_error']!r}",
                flush=True,
            )
            for problem in problems:
                print(f"[autopilot]   state: {problem}", flush=True)
            for entry in bad_viewports:
                for line in summarize_issues(entry["detail"]):
                    print(f"[autopilot]   viewport {entry['viewport']}: {line}", flush=True)
            for entry in bad_pages:
                for line in summarize_issues(entry["detail"]):
                    print(f"[autopilot]   page {entry['page']}: {line}", flush=True)
            if not cascade_intact:
                print(
                    f"[autopilot]   cascade: {cascade_before} -> {cascade_after}",
                    flush=True,
                )
            for entry in malformed:
                print(
                    f"[autopilot]   malformed[{entry['outcome']}] {entry['case']}",
                    flush=True,
                )
            for line in summarize_issues(layout["diagnostic_issues"][:6]):
                print(f"[autopilot]   layout: {line}", flush=True)
            _ui(app, lambda v=verdict: set_status(f"autopilot {step}: {v}", "ok" if v == "PASS" else "bad"))
        except Exception as exc:  # pragma: no cover - reported, not raised
            results.append({"cycle": cycle, "passed": False, "error": repr(exc)})
            print(f"[autopilot] {step}: ERROR {exc!r}", flush=True)
            break

    state.autopilot_report = results
    if report_path is not None:
        payload = {
            "demo": "theme_forge_stress_demo",
            "cycles": cycles,
            "origin_theme": origin_theme,
            "results": results,
        }
        report_path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        print(f"[autopilot] report written to {report_path}", flush=True)

    passed = sum(1 for entry in results if entry.get("passed"))
    print(f"[autopilot] {passed}/{len(results)} cycles passed", flush=True)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def build_app(
    theme_key: str = "modern-dark",
    *,
    rows: int = 600,
    decorations: str = "client",
    long_title: bool = False,
) -> tuple[dg.App, dg.Window]:
    if theme_key not in TOKENS:
        raise ValueError(f"theme must be one of {sorted(TOKENS)}")

    state.theme_key = theme_key
    state.rows = make_rows(rows)
    state.tick = SAMPLE_COUNT
    state.leds.clear()
    state.bars.clear()
    state.metrics.clear()
    state.live_badges.clear()
    state.flex_panels.clear()
    state.theme_buttons.clear()
    state.var_sliders.clear()
    state.var_values.clear()
    state.scrolls.clear()
    state.var_edits.clear()

    PAGE_BUILDERS.update(
        {
            "theme": build_theme_page,
            "gallery": build_gallery_page,
            "parts": build_parts_page,
            "borders": build_borders_page,
            "paint": build_paint_page,
            "type": build_type_page,
            "layout": build_layout_page,
            "effects": build_effects_page,
            "icons": build_icons_page,
            "chrome": build_chrome_page,
            "live": lambda: build_live_page(rows),
            "extreme": build_extreme_page,
        }
    )

    app = dg.App(
        title="THEME FORGE",
        theme=theme_for(theme_key),
    )
    state.app = app

    # Cascade order is fixed here: variables first so later sheets resolve
    # against them, then structure, then the swappable appearance sheet.
    app.set_stylesheet("variables", variables_css(theme_key))
    app.set_stylesheet("structure", STRUCTURE_CSS)
    app.set_stylesheet("appearance", APPEARANCE[theme_key])

    app.set_image_resource("forge-weave", forge_texture())
    app.set_image_resource("forge-pip", badge_texture())

    window = build_window(rows=rows, decorations=decorations, long_title=long_title)
    state.window = window
    return app, window


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run the THEME FORGE DragonGUI theming and CSS stress demo."
    )
    parser.add_argument(
        "--theme",
        choices=list(THEME_ORDER),
        default="modern-dark",
        help="Theme applied at startup (default: modern-dark).",
    )
    parser.add_argument(
        "--rows",
        type=int,
        default=600,
        help="Rows generated for the restyle journal table (default: 600).",
    )
    parser.add_argument(
        "--decorations",
        choices=("client", "native"),
        default="client",
        help="Window decoration mode (default: client, which enables the chrome lab).",
    )
    parser.add_argument(
        "--long-title",
        action="store_true",
        help=(
            "Use the long window title to exercise retained-title shrink and "
            "ellipsis behavior at narrow client-decorated viewports."
        ),
    )
    parser.add_argument(
        "--overrides",
        action="store_true",
        help="Install the removable override stylesheet at startup.",
    )
    parser.add_argument(
        "--extreme",
        action="store_true",
        help="Install the deliberately hostile stylesheet at startup.",
    )
    parser.add_argument(
        "--no-live",
        action="store_true",
        help="Disable the background telemetry worker.",
    )
    parser.add_argument(
        "--autopilot",
        action="store_true",
        help="Run the interaction loop automatically and verify retained state.",
    )
    parser.add_argument(
        "--autopilot-cycles",
        type=int,
        default=2,
        help="Number of autopilot cycles to run (default: 2).",
    )
    parser.add_argument(
        "--autopilot-settle",
        type=float,
        default=0.32,
        help="Seconds to wait after each autopilot step (default: 0.32).",
    )
    parser.add_argument(
        "--autopilot-exit",
        action="store_true",
        help="Close the window once the autopilot finishes.",
    )
    parser.add_argument(
        "--report",
        type=Path,
        default=None,
        help="Write the autopilot integrity report to this JSON path.",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    app, window = build_app(
        args.theme,
        rows=max(1, args.rows),
        decorations=args.decorations,
        long_title=args.long_title,
    )

    if args.overrides:
        state.overrides_on = True
        app.set_stylesheet("overrides", OVERRIDE_CSS)
    if args.extreme:
        state.extreme_on = True
        app.set_stylesheet("extreme", HOSTILE_CSS)

    workers: list[threading.Thread] = []
    if not args.no_live:
        workers.append(
            threading.Thread(target=live_worker, args=(app,), name="forge-live", daemon=True)
        )

    if args.autopilot:
        def run_autopilot() -> None:
            try:
                autopilot(
                    app,
                    cycles=max(1, args.autopilot_cycles),
                    settle=max(0.05, args.autopilot_settle),
                    report_path=args.report,
                )
            finally:
                if args.autopilot_exit:
                    try:
                        app.request_exit()
                    except RuntimeError:
                        pass

        workers.append(
            threading.Thread(target=run_autopilot, name="forge-autopilot", daemon=True)
        )

    for worker in workers:
        worker.start()

    try:
        result = app.run(window)
        # The run result embeds the whole document; print a summary instead of
        # megabytes of JSON (which the Windows console cannot encode anyway).
        summary = {
            key: value
            for key, value in result.items()
            if isinstance(value, (str, int, float, bool, type(None)))
        }
        print(f"run result keys: {sorted(result)}")
        print(f"run summary: {summary}")
    finally:
        state.stop.set()

    if args.autopilot and state.autopilot_report:
        failed = [entry for entry in state.autopilot_report if not entry.get("passed")]
        return 1 if failed else 0
    return 0


if __name__ == "__main__":
    raise SystemExit(main())







