"""CSS Theme Gallery

Four themes that make the same widget tree look and feel fundamentally
different — not just different colors, but different density, radius, heights,
type scale, border weight, spacing, and table geometry.

- **Nord**       — Medium density, 6px radius, cool arctic frost
- **Catppuccin** — Roomy, 14px pill radius, warm pastel chocolate
- **Vercel**     — Ultra-tight, 0px radius, monochrome with pure-black bg
- **Rose Pine**  — Spacious, 20px soft radius, muted lavender and rose

Each theme exercises: :root variables, type/class/child selectors,
:hover/:focus/:disabled pseudo-states, text inheritance, font-size/weight
overrides, control height changes, gap/padding density, table row/header
height, and border shorthand.

Run: python examples/css_theme_gallery.py
"""

from __future__ import annotations

import math
import sys
from pathlib import Path
from types import SimpleNamespace

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:
    raise SystemExit("css_theme_gallery.py requires NumPy") from exc


def sample_frame(rows: int = 8_000) -> SimpleNamespace:
    t = np.linspace(0.0, math.tau * 6.0, rows, dtype=np.float32)
    return SimpleNamespace(
        columns=("x", "y", "z", "signal"),
        dtypes=("float32", "float32", "float32", "float32"),
        shape=(rows, 4),
        x=np.cos(t) * (1.0 + 0.4 * np.sin(t * 0.3)),
        y=np.sin(t) * (1.0 + 0.4 * np.cos(t * 0.3)),
        z=(t / t.max() - 0.5) * 6.0,
        signal=np.sin(t * 0.5).astype(np.float32),
    )


# ---------------------------------------------------------------------------
# NORD -- medium density, 6px radius, cool blue
# Palette: nordtheme.com
# ---------------------------------------------------------------------------

CSS_NORD = """\
:root {
    --bg:      #2e3440;
    --bg1:     #3b4252;
    --bg2:     #434c5e;
    --bg3:     #4c566a;
    --fg:      #d8dee9;
    --fg2:     #eceff4;
    --accent:  #88c0d0;
    --accent2: #81a1c1;
    --accent3: #5e81ac;
    --green:   #a3be8c;
    --red:     #bf616a;
    --yellow:  #ebcb8b;
    --purple:  #b48ead;
    --radius:  6px;
    --ctrl-h:  34px;
    --gap:     10px;
    --pad:     14px;
    --font:    13px;
}

Window   { background: var(--bg); }
MenuBar  { background: var(--bg1); border-color: var(--bg2); }
StatusBar { background: var(--bg1); border-color: var(--bg2); font-size: var(--font); }
Sidebar  { background: var(--bg); border-color: var(--bg2); }

Panel {
    padding: var(--pad); gap: var(--gap);
    background: var(--bg1); border: 1px solid var(--bg2);
    border-radius: var(--radius); color: var(--fg); font-size: var(--font);
}
Panel.sidebar {
    width: 280px; background: var(--bg); border-color: var(--accent3);
    accent: var(--accent2);
}
Panel.card {
    flex-grow: 1; padding: 10px; gap: 6px;
    background: var(--bg2); border-color: var(--bg3);
}

Label         { color: var(--fg); font-size: var(--font); }
Label.title   { color: var(--accent); font-size: 17px; font-weight: 800; }
Label.section { color: var(--accent2); font-weight: 700; font-size: var(--font); }
Label.muted   { color: var(--bg3); font-size: 12px; }
Label.metric  { color: var(--fg2); font-size: 22px; font-weight: 800; }
Label.hint    { color: var(--bg3); font-size: 11px; }

Button, Dropdown, TextInput, NumberInput {
    height: var(--ctrl-h); background: var(--bg2);
    border: 1px solid var(--bg3); border-radius: var(--radius);
    color: var(--fg); font-size: var(--font);
}
Button:hover, Dropdown:hover { background: var(--bg3); border-color: var(--accent); }
Button:focus, TextInput:focus, NumberInput:focus { border-color: var(--accent); }
Button:disabled { opacity: 0.45; }
Button.primary { background: var(--accent3); border-color: var(--accent3); color: var(--fg2); font-weight: 700; }
Button.danger  { background: var(--red); border-color: var(--red); color: var(--fg2); }
Panel.sidebar > Button { border-color: var(--accent2); }

Slider       { accent: var(--accent); track-color: var(--bg3); thumb-color: var(--accent2); }
Checkbox     { accent: var(--green); color: var(--fg); }
ProgressBar  { accent: var(--accent); background: var(--bg2); border: 1px solid var(--bg3); border-radius: var(--radius); }
TextArea     { background: var(--bg2); border: 1px solid var(--bg3); border-radius: var(--radius); color: var(--fg); }
TextArea:focus { border-color: var(--accent); }

Tab          { height: 32px; border-radius: var(--radius); accent: var(--accent); color: var(--fg); }
NavItem      { accent: var(--accent2); color: var(--fg); }
Collapsible  { border-color: var(--bg3); color: var(--fg); }

Scatter3D      { border-color: var(--bg3); border-width: 1px; }
DataFrameTable { border-color: var(--bg3); border-width: 1px; table-row-height: 26px; table-header-height: 30px; }

Toast         { background: var(--bg1); border: 1px solid var(--accent2); border-radius: 8px; color: var(--fg); }
Toast.error   { background: var(--red); color: var(--fg2); }
Toast.success { background: var(--green); color: var(--bg); }
"""


# ===========================================================================
# CATPPUCCIN MOCHA — roomy, 14px pill radius, warm pastel
# Palette: catppuccin.com/palette (Mocha flavor)
# ===========================================================================

CSS_CATPPUCCIN = """\
:root {
    --crust:   #11111b;
    --mantle:  #181825;
    --base:    #1e1e2e;
    --surf0:   #313244;
    --surf1:   #45475a;
    --surf2:   #585b70;
    --overlay: #6c7086;
    --fg:      #cdd6f4;
    --fg2:     #bac2de;
    --mauve:   #cba6f7;
    --lav:     #b4befe;
    --blue:    #89b4fa;
    --teal:    #94e2d5;
    --green:   #a6e3a1;
    --red:     #f38ba8;
    --peach:   #fab387;
    --yellow:  #f9e2af;
    --radius:  14px;
    --ctrl-h:  40px;
    --gap:     14px;
    --pad:     18px;
    --font:    14px;
}

Window   { background: var(--crust); }
MenuBar  { background: var(--mantle); border-color: var(--surf0); }
StatusBar { background: var(--mantle); border-color: var(--surf0); font-size: var(--font); }
Sidebar  { background: var(--mantle); border-color: var(--surf0); }

Panel {
    padding: var(--pad); gap: var(--gap);
    background: var(--base); border: 1px solid var(--surf0);
    border-radius: var(--radius); color: var(--fg); font-size: var(--font);
}
Panel.sidebar {
    width: 300px; background: var(--mantle); border-color: var(--mauve);
    accent: var(--mauve);
}
Panel.card {
    flex-grow: 1; padding: 14px; gap: 8px;
    background: var(--mantle); border-color: var(--surf1);
}

Label         { color: var(--fg); font-size: var(--font); }
Label.title   { color: var(--mauve); font-size: 20px; font-weight: 800; }
Label.section { color: var(--lav); font-weight: 700; font-size: var(--font); }
Label.muted   { color: var(--overlay); font-size: 12px; }
Label.metric  { color: var(--fg); font-size: 26px; font-weight: 800; }
Label.hint    { color: var(--overlay); font-size: 12px; }

Button, Dropdown, TextInput, NumberInput {
    height: var(--ctrl-h); background: var(--surf0);
    border: 1px solid var(--surf1); border-radius: var(--radius);
    color: var(--fg); font-size: var(--font);
}
Button:hover, Dropdown:hover { background: var(--surf1); border-color: var(--mauve); }
Button:focus, TextInput:focus, NumberInput:focus { border-color: var(--teal); }
Button:disabled { opacity: 0.45; }
Button.primary { background: var(--mauve); border-color: var(--mauve); color: var(--crust); font-weight: 700; }
Button.danger  { background: var(--red); border-color: var(--red); color: var(--crust); }
Panel.sidebar > Button { border-color: var(--lav); }

Slider       { accent: var(--mauve); track-color: var(--surf1); thumb-color: var(--teal); }
Checkbox     { accent: var(--green); color: var(--fg); }
ProgressBar  { accent: var(--blue); background: var(--surf0); border: 1px solid var(--surf1); border-radius: var(--radius); }
TextArea     { background: var(--surf0); border: 1px solid var(--surf1); border-radius: var(--radius); color: var(--fg); }
TextArea:focus { border-color: var(--teal); }

Tab          { height: 38px; border-radius: var(--radius); accent: var(--mauve); color: var(--fg); }
NavItem      { accent: var(--lav); color: var(--fg); }
Collapsible  { border-color: var(--surf1); color: var(--fg); }

Scatter3D      { border-color: var(--surf1); border-width: 1px; }
DataFrameTable { border-color: var(--surf1); border-width: 1px; table-row-height: 30px; table-header-height: 36px; }

Toast         { background: var(--base); border: 1px solid var(--lav); border-radius: 16px; color: var(--fg); }
Toast.error   { background: var(--red); color: var(--crust); }
Toast.success { background: var(--green); color: var(--crust); }
"""


# ===========================================================================
# VERCEL / GEIST — ultra-tight, 0px radius, monochrome
# Inspired by vercel.com/geist design system
# ===========================================================================

CSS_VERCEL = """\
:root {
    --black:   #000000;
    --bg:      #0a0a0a;
    --bg1:     #111111;
    --bg2:     #1a1a1a;
    --border:  #333333;
    --border2: #444444;
    --fg:      #ededed;
    --fg2:     #a1a1a1;
    --fg3:     #888888;
    --blue:    #0070f3;
    --cyan:    #79ffe1;
    --red:     #ee0000;
    --green:   #50e3c2;
    --yellow:  #f5a623;
    --radius:  0px;
    --ctrl-h:  28px;
    --gap:     6px;
    --pad:     8px;
    --font:    12px;
}

Window   { background: var(--black); }
MenuBar  { background: var(--bg); border-color: var(--border); }
StatusBar { background: var(--bg); border-color: var(--border); font-size: 11px; }
Sidebar  { background: var(--black); border-color: var(--border); }

Panel {
    padding: var(--pad); gap: var(--gap);
    background: var(--bg1); border: 1px solid var(--border);
    border-radius: var(--radius); color: var(--fg); font-size: var(--font);
}
Panel.sidebar {
    width: 240px; background: var(--bg); border-color: var(--border);
    accent: var(--fg);
}
Panel.card {
    flex-grow: 1; padding: 6px; gap: 3px;
    background: var(--bg); border-color: var(--border);
}

Label         { color: var(--fg); font-size: var(--font); }
Label.title   { color: var(--fg); font-size: 14px; font-weight: 700; }
Label.section { color: var(--fg2); font-weight: 600; font-size: 11px; }
Label.muted   { color: var(--fg3); font-size: 11px; }
Label.metric  { color: var(--fg); font-size: 18px; font-weight: 700; }
Label.hint    { color: var(--fg3); font-size: 10px; }

Button, Dropdown, TextInput, NumberInput {
    height: var(--ctrl-h); background: var(--bg);
    border: 1px solid var(--border); border-radius: var(--radius);
    color: var(--fg); font-size: var(--font);
}
Button:hover, Dropdown:hover { background: var(--bg2); border-color: var(--border2); }
Button:focus, TextInput:focus, NumberInput:focus { border-color: var(--fg2); }
Button:disabled { opacity: 0.35; }
Button.primary { background: var(--fg); border-color: var(--fg); color: var(--black); font-weight: 600; }
Button.danger  { background: var(--red); border-color: var(--red); color: var(--fg); }
Panel.sidebar > Button { border-color: var(--border2); }

Slider       { accent: var(--fg); track-color: var(--border); thumb-color: var(--fg); }
Checkbox     { accent: var(--fg); color: var(--fg); }
ProgressBar  { accent: var(--fg); background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); }
TextArea     { background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); color: var(--fg); font-size: 11px; }
TextArea:focus { border-color: var(--fg2); }

Tab          { height: 26px; border-radius: var(--radius); accent: var(--fg); color: var(--fg); font-size: 11px; }
NavItem      { accent: var(--fg); color: var(--fg); font-size: 12px; }
Collapsible  { border-color: var(--border); color: var(--fg); font-size: var(--font); }

Scatter3D      { border-color: var(--border); border-width: 1px; }
DataFrameTable { border-color: var(--border); border-width: 1px; table-row-height: 20px; table-header-height: 24px; font-size: 11px; }

Toast         { background: var(--bg1); border: 1px solid var(--border); border-radius: 0px; color: var(--fg); font-size: 12px; }
Toast.error   { background: var(--red); color: var(--fg); }
Toast.success { background: var(--green); color: var(--black); }
"""


# ===========================================================================
# ROSE PINE — spacious, 20px soft radius, muted lavender
# Palette: rosepinetheme.com/palette
# ===========================================================================

CSS_ROSE_PINE = """\
:root {
    --base:     #191724;
    --surface:  #1f1d2e;
    --overlay:  #26233a;
    --muted:    #6e6a86;
    --subtle:   #908caa;
    --fg:       #e0def4;
    --love:     #eb6f92;
    --gold:     #f6c177;
    --rose:     #ebbcba;
    --pine:     #31748f;
    --foam:     #9ccfd8;
    --iris:     #c4a7e7;
    --hl-low:   #21202e;
    --hl-med:   #403d52;
    --hl-high:  #524f67;
    --radius:   20px;
    --ctrl-h:   42px;
    --gap:      16px;
    --pad:      20px;
    --font:     15px;
}

Window   { background: var(--base); }
MenuBar  { background: var(--surface); border-color: var(--hl-med); }
StatusBar { background: var(--surface); border-color: var(--hl-med); font-size: 13px; }
Sidebar  { background: var(--base); border-color: var(--hl-med); }

Panel {
    padding: var(--pad); gap: var(--gap);
    background: var(--surface); border: 1px solid var(--hl-med);
    border-radius: var(--radius); color: var(--fg); font-size: var(--font);
}
Panel.sidebar {
    width: 320px; background: var(--base); border-color: var(--iris);
    accent: var(--iris);
}
Panel.card {
    flex-grow: 1; padding: 16px; gap: 10px;
    background: var(--hl-low); border-color: var(--hl-med);
}

Label         { color: var(--fg); font-size: var(--font); }
Label.title   { color: var(--iris); font-size: 22px; font-weight: 800; }
Label.section { color: var(--foam); font-weight: 700; font-size: var(--font); }
Label.muted   { color: var(--muted); font-size: 13px; }
Label.metric  { color: var(--fg); font-size: 28px; font-weight: 800; }
Label.hint    { color: var(--muted); font-size: 12px; }

Button, Dropdown, TextInput, NumberInput {
    height: var(--ctrl-h); background: var(--overlay);
    border: 1px solid var(--hl-med); border-radius: var(--radius);
    color: var(--fg); font-size: var(--font);
}
Button:hover, Dropdown:hover { background: var(--hl-med); border-color: var(--iris); }
Button:focus, TextInput:focus, NumberInput:focus { border-color: var(--foam); }
Button:disabled { opacity: 0.45; }
Button.primary { background: var(--iris); border-color: var(--iris); color: var(--base); font-weight: 700; }
Button.danger  { background: var(--love); border-color: var(--love); color: var(--base); }
Panel.sidebar > Button { border-color: var(--iris); }

Slider       { accent: var(--iris); track-color: var(--hl-med); thumb-color: var(--foam); }
Checkbox     { accent: var(--foam); color: var(--fg); }
ProgressBar  { accent: var(--iris); background: var(--overlay); border: 1px solid var(--hl-med); border-radius: var(--radius); }
TextArea     { background: var(--overlay); border: 1px solid var(--hl-med); border-radius: var(--radius); color: var(--fg); }
TextArea:focus { border-color: var(--foam); }

Tab          { height: 40px; border-radius: var(--radius); accent: var(--iris); color: var(--fg); }
NavItem      { accent: var(--iris); color: var(--fg); }
Collapsible  { border-color: var(--hl-med); color: var(--fg); }

Scatter3D      { border-color: var(--hl-med); border-width: 1px; }
DataFrameTable { border-color: var(--hl-med); border-width: 1px; table-row-height: 34px; table-header-height: 40px; }

Toast         { background: var(--surface); border: 1px solid var(--iris); border-radius: 24px; color: var(--fg); }
Toast.error   { background: var(--love); color: var(--base); }
Toast.success { background: var(--foam); color: var(--base); }
"""


# ===========================================================================
# Registry
# ===========================================================================

CSS_THEMES = {
    "nord": CSS_NORD,
    "catppuccin": CSS_CATPPUCCIN,
    "vercel": CSS_VERCEL,
    "rose-pine": CSS_ROSE_PINE,
}

THEME_INFO = {
    "nord":       ("Nord",              "6px radius, 34px controls, 13px type, medium density."),
    "catppuccin": ("Catppuccin Mocha",  "14px pill radius, 40px controls, 14px type, roomy."),
    "vercel":     ("Vercel / Geist",    "0px square, 28px controls, 12px type, ultra-compact."),
    "rose-pine":  ("Rose Pine",         "20px soft radius, 42px controls, 15px type, spacious."),
}


# ===========================================================================
# App
# ===========================================================================

frame = sample_frame()
app = dg.App(theme=dg.Theme.dark(accent="#88c0d0", focus="#8fbcbb"))
app.stylesheet(CSS_NORD)
win = dg.Window("DragonGUI CSS Theme Gallery", width=1200, height=760)


def apply_theme(name: str) -> None:
    app.stylesheet(CSS_THEMES[name])
    title, desc = THEME_INFO[name]
    theme_name.set_value(title)
    theme_desc.set_value(desc)
    status.set_value(f"Theme: {name}")
    dg.toast(f"{title}", level="success", duration=1600, app=app)


def show_snapshot() -> None:
    snapshot = app.debug_snapshot()
    gpu = snapshot.get("gpu", {})
    styles = gpu.get("stylesheets", {})
    status.set_value(
        f"rules={styles.get('user_rules', '?')}  "
        f"warnings={styles.get('warning_count', '?')}  "
        f"widgets={gpu.get('renderer', {}).get('widget_count', '?')}"
    )


# ===========================================================================
# Widget tree
# ===========================================================================

with dg.MenuBar():
    with dg.Menu("Theme"):
        for key, (label, _) in THEME_INFO.items():
            dg.MenuItem(label, on_click=(lambda k: lambda: apply_theme(k))(key))
    with dg.Menu("Debug"):
        dg.MenuItem("Print Snapshot", on_click=show_snapshot)

with dg.HLayout(style={"gap": 0}):
    with dg.Sidebar(width=230, style={"padding": 14, "gap": 10}):
        dg.Label("Theme Gallery", class_="title")
        dg.Label("4 design systems", class_="muted")
        dg.Separator()
        dg.NavItem("Dashboard", page="dashboard")
        dg.NavItem("Controls", page="controls")
        dg.NavItem("Data", page="data")
        dg.NavItem("Disabled", page="off", disabled=True)
        dg.Spacer()
        dg.Separator()
        with dg.HLayout(style={"gap": 6, "height": 24}):
            dg.Badge("CSS", level="success")
            dg.Tag("gallery")

    with dg.Pages(value="dashboard", on_change=lambda v: status.set_value(f"Page: {v}")):

        # ── Dashboard ─────────────────────────────────────────────
        with dg.Page("dashboard"):
            with dg.HLayout(style={"gap": 14, "padding": 14}):
                with dg.Panel("Theme Picker", class_="sidebar"):
                    dg.Label("ACTIVE THEME", class_="section")
                    theme_name = dg.Label("Nord", class_="title")
                    theme_desc = dg.Label(THEME_INFO["nord"][1], class_="muted")
                    dg.Separator()
                    dg.Button("Nord", class_="primary", on_click=lambda: apply_theme("nord"))
                    dg.Button("Catppuccin Mocha", on_click=lambda: apply_theme("catppuccin"))
                    dg.Button("Vercel / Geist", on_click=lambda: apply_theme("vercel"))
                    dg.Button("Rose Pine", on_click=lambda: apply_theme("rose-pine"))
                    dg.Separator()
                    dg.Label("WHAT CHANGES", class_="section")
                    dg.Label("Radius, control height, font size", class_="hint")
                    dg.Label("Gap, padding, table row density", class_="hint")
                    dg.Label("Colors, borders, hover/focus", class_="hint")

                with dg.VLayout(style={"gap": 14, "flex_grow": 1}):
                    with dg.HLayout(style={"gap": 14}):
                        with dg.Panel("Latency", class_="card"):
                            dg.Label("FRAME", class_="section")
                            dg.Label("16 ms", class_="metric")
                            dg.Label("render budget", class_="hint")
                        with dg.Panel("Queue", class_="card"):
                            dg.Label("COMMANDS", class_="section")
                            dg.Label("0", class_="metric")
                            dg.Label("pending", class_="hint")
                        with dg.Panel("Points", class_="card"):
                            dg.Label("SCATTER", class_="section")
                            dg.Label("8K", class_="metric")
                            dg.Label("GPU rendered", class_="hint")

                    with dg.Panel(style={"flex_grow": 1, "padding": 0, "gap": 0}):
                        dg.Scatter3D(frame, x="x", y="y", z="z", colormap="viridis")

        # ── Controls ──────────────────────────────────────────────
        with dg.Page("controls"):
            with dg.HLayout(style={"gap": 14, "padding": 14}):
                with dg.Panel("Inputs", class_="sidebar"):
                    dg.TextInput("Editable", on_change=lambda v: status.set_value(f"Text: {v}"))
                    dg.NumberInput(42, min=0, max=100, on_change=lambda v: status.set_value(f"Num: {v:g}"))
                    dg.Dropdown(("Alpha", "Beta", "Gamma"), value="Alpha")
                    dg.Slider(0.5, on_change=lambda v: status.set_value(f"Slider: {v:.2f}"))
                    dg.Checkbox("Enable feature", checked=True)
                    dg.ProgressBar(0.72, show_value=True)
                    dg.Separator()
                    dg.Button("Primary", class_="primary",
                              on_click=lambda: dg.toast("Clicked", app=app))
                    dg.Button("Danger", class_="danger",
                              on_click=lambda: dg.toast("Danger", level="error", app=app))
                    dg.Button("Disabled", disabled=True)

                with dg.VLayout(style={"gap": 14, "flex_grow": 1}):
                    with dg.Panel("Collapsible"):
                        with dg.Collapsible("Advanced", expanded=False):
                            dg.Label("Hidden by default.")
                            dg.Slider(0.3)
                            dg.Checkbox("Verbose output")
                        with dg.Collapsible("Notes"):
                            dg.TextArea("Line 1\nLine 2\nLine 3", rows=3)

                    with dg.Panel("Tabs"):
                        with dg.Tabs(value="a"):
                            with dg.Tab("Alpha", value="a"):
                                dg.Label("Alpha content")
                                dg.Button("Action", on_click=lambda: status.set_value("Alpha"))
                            with dg.Tab("Beta", value="b", badge="3"):
                                dg.Label("Beta content")
                                dg.Checkbox("Option")
                            with dg.Tab("Gamma", value="c"):
                                dg.Label("Gamma content")

        # ── Data ──────────────────────────────────────────────────
        with dg.Page("data"):
            with dg.HLayout(style={"gap": 14, "padding": 14}):
                with dg.Panel("Controls", class_="sidebar"):
                    dg.Label("DATA SURFACE", class_="section")
                    dg.Label("Table row height and header height change per theme.", class_="muted")
                    dg.Separator()
                    dg.Button("Snapshot", on_click=show_snapshot)

                with dg.VLayout(style={"gap": 14, "flex_grow": 1}):
                    dg.DataFrameTable(frame, page_size=50)

with dg.StatusBar(height=36):
    status = dg.TextInput("Ready", placeholder="status", style={"width": 300})
    dg.Separator(orientation="vertical")
    dg.Label("Themes change density, radius, heights, type, and spacing.")


try:
    result = app.run(win)
except dg.BackendUnavailableError:
    print("Native backend not built. Run: maturin develop --manifest-path native/Cargo.toml")
else:
    print(result)
