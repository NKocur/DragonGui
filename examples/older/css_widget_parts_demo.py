from __future__ import annotations

import sys
from pathlib import Path
from types import SimpleNamespace

import numpy as np

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


def sample_table() -> SimpleNamespace:
    rows = 48
    idx = np.arange(rows, dtype=np.float32)
    return SimpleNamespace(
        columns=("stage", "latency", "queue", "load"),
        shape=(rows, 4),
        stage=(idx % 6).astype(np.float32),
        latency=(12.0 + np.sin(idx * 0.25) * 5.0).astype(np.float32),
        queue=(80.0 + idx * 3.0).astype(np.float32),
        load=(0.35 + np.cos(idx * 0.17) * 0.22).astype(np.float32),
    )


PARTS_CSS = """
:root {
    --bg: #08111f;
    --panel: #111d2f;
    --panel-hi: #17263d;
    --line: #395472;
    --text-hi: #f2f8ff;
    --text: #c8d7e8;
    --muted: #8fa4bb;
    --blue: #58a6ff;
    --green: #4ade80;
    --pink: #f778ba;
    --amber: #f2cc60;
    --red: #ff6b6b;
}

Window {
    background: var(--bg);
    color: var(--text);
    font-size: 14px;
}

Sidebar {
    background: #0c1727;
    border-color: var(--line);
    padding: 14px;
    gap: 10px;
}

Panel {
    background: var(--panel);
    border-color: var(--line);
    border-width: 1px;
    border-radius: 12px;
    padding: 14px;
    gap: 10px;
    color: var(--text);
}

Panel::accent {
    width: 4px;
    background: #24405e;
}

Panel:hover::accent {
    background: var(--blue);
}

Label {
    color: var(--text);
}

Button,
NumberInput,
Dropdown {
    height: 38px;
    background: #0d1829;
    border-color: var(--line);
    border-radius: 12px;
    color: var(--text-hi);
}

Button:hover,
NumberInput:hover,
Dropdown:hover {
    border-color: var(--blue);
}

NavItem::item {
    background: #0f1b2c;
    color: var(--text);
    padding: 12px;
    border-radius: 9px;
}

NavItem:hover::item {
    background: #16263d;
    color: var(--text-hi);
}

NavItem::accent {
    background: var(--blue);
    width: 5px;
    border-radius: 999px;
}

NumberInput::stepper {
    width: 38px;
}

NumberInput::stepper-up {
    background: #17375c;
    color: var(--text-hi);
    border-top-right-radius: 12px;
}

NumberInput::stepper-down {
    background: #341c47;
    color: var(--pink);
    border-bottom-right-radius: 12px;
}

NumberInput:hover::stepper-up {
    background: var(--blue);
    color: #06101f;
}

NumberInput:hover::stepper-down {
    background: var(--pink);
    color: #06101f;
}

NumberInput::stepper-divider {
    background: #6d7f93;
}

Dropdown::chevron {
    color: var(--amber);
    width: 18px;
}

Dropdown::menu {
    background: #0b1626;
    border-color: var(--blue);
    border-radius: 12px;
}

Dropdown::item {
    color: var(--text);
    padding: 12px;
}

Dropdown::item-hover {
    background: #1b3760;
    color: var(--text-hi);
}

Dropdown::item-selected {
    background: #273c23;
    color: var(--green);
}

Checkbox::row {
    background: #0f1b2c;
    border-radius: 12px;
}

Checkbox:hover::row {
    background: #172941;
}

Checkbox::box {
    background: #07101d;
    border-color: var(--line);
    border-radius: 5px;
}

Checkbox:checked::indicator {
    background: var(--green);
    border-radius: 999px;
}

Checkbox::label {
    color: var(--text-hi);
    font-weight: 700;
}

Slider::track {
    background: #0a1423;
    border-color: #314966;
    height: 9px;
    border-radius: 999px;
}

Slider::fill {
    background: var(--blue);
    border-radius: 999px;
}

Slider::thumb {
    background: var(--amber);
    border-color: #07101d;
    width: 24px;
    height: 24px;
    border-radius: 999px;
}

ProgressBar::track {
    background: #07101d;
    border-color: var(--line);
    border-radius: 999px;
}

ProgressBar::fill {
    background: var(--green);
    height: 12px;
    border-radius: 999px;
}

ProgressBar::label {
    color: var(--text-hi);
    font-weight: 800;
}

Tabs::header {
    height: 40px;
    background: #0b1626;
    border-color: var(--line);
}

Tab::tab {
    background: #101d30;
    border-color: var(--line);
    border-radius: 10px;
    color: var(--text);
    padding: 12px;
}

Tab:hover::tab {
    background: #172b46;
    color: var(--text-hi);
}

Tab::accent {
    background: var(--pink);
    height: 5px;
    border-radius: 999px;
}

DataFrameTable {
    table-row-height: 28px;
    table-header-height: 34px;
    background: #0b1626;
    border-color: var(--line);
    border-radius: 12px;
}

DataFrameTable::header {
    background: #17263d;
    color: var(--amber);
    font-weight: 900;
}

DataFrameTable::row {
    background: #0b1626;
    color: var(--text);
}

DataFrameTable::row-selected {
    background: #25355c;
    color: var(--text-hi);
}

DataFrameTable::grid-line {
    background: #314966;
    width: 1px;
}
"""


app = dg.App(theme=dg.Theme.dark(accent="#58a6ff", radius=8))
app.stylesheet(PARTS_CSS)
win = dg.Window("DragonGUI CSS Widget Parts", width=1180, height=760)
table_frame = sample_table()

inline_stepper_style = {
    "parts": {
        "stepper": {"width": 42},
        "stepper_up": {
            "background": "#f2cc60",
            "color": "#08111f",
            "border_top_right_radius": 12,
        },
        "stepper_down": {
            "background": "#ff6b6b",
            "color": "#08111f",
            "border_bottom_right_radius": 12,
        },
    }
}

with dg.HLayout(style={"gap": 0}):
    with dg.Sidebar(title="Parts", width=210):
        dg.Label("CSS Parts", style={"font_size": 19, "font_weight": 800, "color": "#f2f8ff"})
        dg.Label("Internal widget hooks", style={"color": "#8fa4bb"})
        dg.Separator()
        dg.NavItem("Parts", page="parts")
        dg.NavItem("Table", page="table")
        dg.NavItem("States", page="states", disabled=True)
        dg.Spacer()
        dg.Label("NavItem::accent")

    with dg.Pages(value="parts", style={"flex": 1}):
        with dg.Page("parts"):
            with dg.VLayout(style={"padding": 14, "gap": 14}):
                with dg.HLayout(style={"gap": 14, "height": 216}):
                    with dg.Panel("NumberInput Parts", width=430):
                        dg.Label("Stepper width, halves, divider, and text.")
                        dg.NumberInput(72, min=0, max=100, step=4)
                        dg.NumberInput(128, min=0, max=255, step=8, style=inline_stepper_style)
                        dg.Label("Second field uses inline style={parts: ...}.")

                    with dg.Panel("Dropdown And Checkbox", width=430):
                        dg.Label("Open the menu to see item parts.")
                        dg.Dropdown(("Baseline", "Elevated", "Critical"), value="Elevated")
                        dg.Checkbox("Checked indicator uses ::indicator", checked=True)
                        dg.Checkbox("Unchecked box still uses ::box", checked=False)

                with dg.HLayout(style={"gap": 14, "height": 212}):
                    with dg.Panel("Slider And Progress", width=430):
                        dg.Label("Track, fill, thumb, and label parts.")
                        dg.Slider(0.68)
                        dg.ProgressBar(0.73, label="73% staged")
                        dg.ProgressBar(0.38, label="38% queued")

                    with dg.Panel("Tabs And Navigation", style={"flex": 1}):
                        dg.Label("Tabs expose header, tab body, and accent.")
                        with dg.Tabs(value="one", style={"height": 112}):
                            with dg.Tab("One", value="one"):
                                dg.Label("Tab::accent is active here.")
                            with dg.Tab("Two", value="two"):
                                dg.Label("Tab body is styled separately.")
                            with dg.Tab("Three", value="three"):
                                dg.Label("Header height comes from Tabs::header.")

                with dg.Panel("DataFrameTable Parts", style={"height": 270}):
                    dg.DataFrameTable(table_frame, page_size=42, sample_rows=48)


print(app.run(win))
