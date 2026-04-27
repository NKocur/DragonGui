from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg

try:
    import numpy as np
except ImportError as exc:  # pragma: no cover - manual demo guard
    raise SystemExit("css_julius_style_demo.py requires NumPy") from exc


class ActionFrame:
    columns = ("id", "risk", "latency", "state", "owner")
    dtypes = ("int32", "float32", "float32", "str", "str")

    def __init__(self, rows: int = 36) -> None:
        idx = np.arange(rows, dtype=np.int32)
        self.shape = (rows, len(self.columns))
        self.id = idx + 101
        self.risk = ((idx % 9) * 0.11 + 0.03).astype(np.float32)
        self.latency = (18.0 + (idx % 7) * 6.5).astype(np.float32)
        self.state = np.where(idx % 5 == 0, "blocked", np.where(idx % 3 == 0, "review", "ready"))
        self.owner = np.where(idx % 4 == 0, "agent", np.where(idx % 4 == 1, "user", "policy"))

    def __getitem__(self, column: str) -> object:
        return getattr(self, column)


JULIUS_CSS = """
:root {
    --j-bg: #f5f4f0;
    --j-sidebar: #eeede8;
    --j-surface: #ffffff;
    --j-raised: #f9f8f5;
    --j-hover: #f2f1ed;
    --j-active: #eae9e4;
    --j-border: #e2e1dc;
    --j-border-raised: #d4d3ce;
    --j-focus: #4f46e5;
    --j-text: #111111;
    --j-secondary: #888888;
    --j-tertiary: #c0bfba;
    --j-brand: #4f46e5;
    --j-brand-soft: #eef2ff;
    --j-brand-border: #c7d2fe;
    --j-safe: #18794e;
    --j-safe-soft: #e8f5ee;
    --j-amber: #946800;
    --j-amber-soft: #fef3c7;
    --j-amber-border: #fcd34d;
    --j-danger: #b91c1c;
    --j-danger-soft: #fef2f2;
    --j-danger-border: #fca5a5;
}

Window {
    background: var(--j-bg);
    color: var(--j-text);
    font-size: 13px;
}

MenuBar.titlebar,
StatusBar.footer {
    background: var(--j-sidebar);
    border-color: var(--j-border);
    padding: 4px;
    gap: 4px;
}

Menu {
    height: 28px;
    background: var(--j-sidebar);
    border-color: var(--j-sidebar);
    border-radius: 5px;
    color: var(--j-secondary);
    font-size: 12px;
}

Menu:hover {
    background: var(--j-hover);
    color: var(--j-text);
}

HLayout.shell {
    gap: 0px;
    background: var(--j-bg);
}

Sidebar.rail {
    width: 246px;
    background: var(--j-sidebar);
    border-color: var(--j-border);
    padding: 14px;
    gap: 8px;
}

VLayout.workspace {
    flex-grow: 1;
    padding: 14px;
    gap: 12px;
    background: var(--j-bg);
}

HLayout.main-grid {
    flex-grow: 1;
    gap: 12px;
}

HLayout.action-buttons {
    height: 34px;
    gap: 8px;
}

HLayout.compact-row {
    height: 34px;
    gap: 8px;
}

Panel {
    background: var(--j-surface);
    border: 1px solid var(--j-border);
    border-radius: 9px;
    padding: 12px;
    gap: 8px;
    color: var(--j-text);
}

Panel::accent {
    width: 1px;
    background: var(--j-border);
}

Panel.banner::accent,
Panel.table-wrap::accent {
    width: 0px;
}

Panel.banner {
    height: 54px;
    background: var(--j-amber-soft);
    border-color: var(--j-amber-border);
    border-radius: 0px;
    padding: 9px;
}

Panel.column {
    flex-grow: 1;
    gap: 10px;
}

Panel.left-column {
    width: 390px;
    flex-grow: 0;
    flex-shrink: 0;
}

Panel.right-column {
    width: 280px;
    flex-grow: 0;
    flex-shrink: 0;
}

Panel.action-card {
    height: 104px;
    background: var(--j-surface);
    border-color: var(--j-border);
    padding: 9px;
    gap: 4px;
}

Panel.action-card:hover {
    background: var(--j-hover);
}

Panel.chat {
    flex-grow: 1;
    background: var(--j-surface);
    padding: 12px;
}

Panel.message-agent {
    height: 70px;
    background: var(--j-raised);
    border-color: var(--j-border);
    padding: 8px;
    gap: 3px;
}

Panel.message-user {
    height: 64px;
    background: var(--j-brand);
    border-color: var(--j-brand);
    padding: 8px;
    gap: 3px;
    color: #ffffff;
}

Panel.status-chip {
    height: 34px;
    background: var(--j-brand-soft);
    border-color: var(--j-brand-border);
    padding: 6px;
    gap: 0px;
}

Panel.safe-chip {
    background: var(--j-safe-soft);
    border-color: var(--j-safe);
}

Panel.danger-chip {
    background: var(--j-danger-soft);
    border-color: var(--j-danger-border);
}

Panel.table-wrap {
    height: 190px;
    padding: 0px;
    gap: 0px;
    background: var(--j-surface);
}

Panel.gap-card {
    height: 43px;
    padding: 7px;
    gap: 2px;
    background: var(--j-raised);
}

Label {
    color: var(--j-text);
    font-size: 13px;
}

Label.logo {
    height: 28px;
    color: var(--j-text);
    font-size: 19px;
    font-weight: 700;
}

Label.subtle {
    color: var(--j-secondary);
    font-size: 12px;
}

Label.tiny {
    height: 18px;
    color: var(--j-tertiary);
    font-size: 11px;
}

Label.section {
    height: 22px;
    color: var(--j-secondary);
    font-size: 11px;
    font-weight: 700;
}

Label.row-title {
    height: 22px;
    color: var(--j-text);
    font-weight: 600;
}

Label.row-meta {
    height: 18px;
    color: var(--j-secondary);
    font-size: 11px;
}

Label.brand {
    color: var(--j-brand);
    font-weight: 700;
}

Label.safe {
    color: var(--j-safe);
    font-weight: 700;
}

Label.danger {
    color: var(--j-danger);
    font-weight: 700;
}

Label.white {
    color: #ffffff;
}

Button,
TextInput,
Dropdown,
NumberInput {
    height: 32px;
    background: var(--j-raised);
    border: 1px solid var(--j-border);
    border-radius: 5px;
    color: var(--j-text);
    font-size: 12px;
}

Button:hover,
TextInput:focus,
Dropdown:hover,
NumberInput:focus {
    border-color: var(--j-focus);
    background: var(--j-hover);
}

Button.primary {
    background: var(--j-brand);
    border-color: var(--j-brand);
    color: #ffffff;
    font-weight: 600;
}

Button.danger {
    background: var(--j-danger-soft);
    border-color: var(--j-danger-border);
    color: var(--j-danger);
}

Button.icon {
    width: 32px;
    height: 32px;
    border-radius: 999px;
    padding: 0px;
}

TextInput.prompt {
    height: 42px;
    background: var(--j-raised);
    border-radius: 9px;
}

NavItem::item {
    height: 34px;
    background: var(--j-sidebar);
    color: var(--j-secondary);
    padding: 8px;
    border-radius: 5px;
}

NavItem:hover::item {
    background: var(--j-hover);
    color: var(--j-text);
}

NavItem::accent {
    width: 4px;
    background: var(--j-brand);
    border-radius: 999px;
}

NumberInput::stepper {
    width: 34px;
}

NumberInput::stepper-up,
NumberInput::stepper-down {
    background: var(--j-raised);
    color: var(--j-secondary);
}

NumberInput:hover::stepper-up,
NumberInput:hover::stepper-down {
    background: var(--j-active);
    color: var(--j-text);
}

Dropdown::chevron {
    color: var(--j-secondary);
    width: 16px;
}

Dropdown::menu {
    background: var(--j-surface);
    border-color: var(--j-border-raised);
    border-radius: 9px;
}

Dropdown::item {
    color: var(--j-text);
    padding: 9px;
}

Dropdown::item-hover {
    background: var(--j-hover);
    color: var(--j-text);
}

Dropdown::item-selected {
    background: var(--j-brand-soft);
    color: var(--j-brand);
}

Checkbox::row {
    height: 32px;
    background: var(--j-surface);
    border-radius: 999px;
}

Checkbox:hover::row {
    background: var(--j-hover);
}

Checkbox::box {
    width: 36px;
    height: 20px;
    background: var(--j-active);
    border-color: var(--j-border-raised);
    border-radius: 999px;
}

Checkbox:checked::box {
    background: var(--j-brand);
    border-color: var(--j-brand);
}

Checkbox::indicator {
    width: 12px;
    height: 12px;
    background: #ffffff;
    border-radius: 999px;
}

Checkbox::label {
    color: var(--j-secondary);
    font-size: 12px;
}

Slider::track {
    background: var(--j-active);
    border-color: var(--j-active);
    border-radius: 999px;
}

Slider::fill {
    background: var(--j-brand);
    border-radius: 999px;
}

Slider::thumb {
    background: #ffffff;
    border-color: var(--j-brand);
    border-radius: 999px;
    width: 18px;
    height: 18px;
}

ProgressBar {
    height: 24px;
    background: var(--j-active);
    border-radius: 999px;
}

ProgressBar::track {
    background: var(--j-active);
    border-radius: 999px;
}

ProgressBar::fill {
    background: var(--j-brand);
    height: 10px;
    border-radius: 999px;
}

ProgressBar::label {
    color: var(--j-text);
    font-size: 11px;
    font-weight: 700;
}

DataFrameTable {
    table-row-height: 27px;
    table-header-height: 30px;
    background: var(--j-surface);
    border-color: var(--j-border);
    border-radius: 9px;
    font-size: 12px;
}

DataFrameTable::header {
    background: var(--j-raised);
    color: var(--j-secondary);
    font-weight: 700;
}

DataFrameTable::row {
    background: var(--j-surface);
    color: var(--j-text);
}

DataFrameTable::row-selected {
    background: var(--j-brand-soft);
    color: var(--j-brand);
}

DataFrameTable::grid-line {
    background: var(--j-border);
    width: 1px;
}
"""


frame = ActionFrame()
app = dg.App(theme=dg.Theme.light(accent="#4f46e5", radius=9, font_size=13))
app.stylesheet(JULIUS_CSS)
win = dg.Window("DragonGUI Julius CSS Comparison", width=1280, height=780)

with dg.MenuBar(class_="titlebar", height=38):
    with dg.Menu("Julius"):
        dg.MenuItem("Open workspace")
        dg.MenuItem("Command registry")
    with dg.Menu("Agent"):
        dg.MenuItem("Generate plan")
        dg.MenuItem("Review actions")
    with dg.Menu("View"):
        dg.MenuItem("Trust panel")

with dg.HLayout(class_="shell"):
    with dg.Sidebar(class_="rail"):
        dg.Label("Julius", class_="logo")
        dg.Label("Desktop control plane", class_="subtle")
        dg.Separator()
        dg.NavItem("Home", page="home")
        dg.NavItem("Staging", page="staging")
        dg.NavItem("Trust", page="trust")
        dg.NavItem("Registry", page="registry")
        dg.NavItem("Settings", page="settings")
        dg.Spacer()
        dg.Label("CSS gaps shown here:", class_="section")
        dg.Label("Icons are text placeholders", class_="tiny")
        dg.Label("Switch knob cannot slide", class_="tiny")
        dg.Label("No multiline text input", class_="tiny")

    with dg.VLayout(class_="workspace"):
        with dg.Panel(class_="banner"):
            with dg.HLayout(class_="compact-row"):
                dg.Label("Model notice:", class_="brand")
                dg.Label("Julius tokens ported into DragonGUI CSS.", class_="subtle")
                dg.Spacer()
                dg.Button("Dismiss", class_="danger")

        with dg.HLayout(class_="main-grid"):
            with dg.Panel(class_="column left-column"):
                dg.Label("STAGING QUEUE", class_="section")
                with dg.Panel(class_="action-card"):
                    dg.Label("Review generated workspace plan", class_="row-title")
                    dg.Label("Needs approval before filesystem writes", class_="row-meta")
                    with dg.HLayout(class_="action-buttons"):
                        dg.Button("Approve", class_="primary")
                        dg.Button("Edit")
                        dg.Button("Block", class_="danger")
                with dg.Panel(class_="action-card"):
                    dg.Label("Read linked project context", class_="row-title")
                    dg.Label("3 files, 8 symbols, safe read-only operation", class_="row-meta")
                    with dg.HLayout(class_="action-buttons"):
                        dg.Button("Allow", class_="primary")
                        dg.Button("Details")
                        dg.Button("Deny", class_="danger")
                with dg.Panel(class_="action-card"):
                    dg.Label("Update generated demo file", class_="row-title")
                    dg.Label("Shows CSS subset vs React/Tailwind affordances", class_="row-meta")
                    with dg.HLayout(class_="action-buttons"):
                        dg.Button("Queue", class_="primary")
                        dg.Button("Diff")
                        dg.Button("Cancel")

                dg.Label("ACTIONS TABLE", class_="section")
                with dg.Panel(class_="table-wrap"):
                    dg.DataFrameTable(frame, page_size=28, sample_rows=36)

            with dg.Panel(class_="column chat"):
                dg.Label("AGENT THREAD", class_="section")
                with dg.Panel(class_="message-agent"):
                    dg.Label("Palette and compact density transfer well.", class_="row-title")
                    dg.Label("Icons, badges, shadows, and scroll regions do not.", class_="row-meta")
                with dg.Panel(class_="message-user"):
                    dg.Label("Can it show the trust workflow?", class_="row-title white")
                    dg.Label("This bubble uses regular Panel and Label widgets.", class_="white")
                with dg.Panel(class_="message-agent"):
                    dg.Label("Mostly, with fewer primitives.", class_="row-title")
                    dg.Label("CSS styles pieces; it does not create new behavior.", class_="row-meta")
                dg.Spacer()
                dg.TextInput("Compare Julius CSS tokens against DragonGUI widgets", class_="prompt")

            with dg.Panel(class_="column right-column"):
                dg.Label("TRUST AND SETTINGS", class_="section")
                with dg.HLayout(class_="compact-row"):
                    dg.Checkbox("Local tools", checked=True)
                    dg.Label("enabled", class_="safe")
                with dg.HLayout(class_="compact-row"):
                    dg.Checkbox("Network", checked=False)
                    dg.Label("manual", class_="brand")
                with dg.HLayout(class_="compact-row"):
                    dg.Checkbox("Destructive ops", checked=False)
                    dg.Label("blocked", class_="danger")
                dg.Label("The switch-like checkbox is a useful stress test.", class_="tiny")
                dg.Separator()
                dg.Label("RUN PARAMETERS", class_="section")
                dg.Dropdown(("balanced", "fast", "deep audit"), value="balanced")
                dg.NumberInput(32, min=1, max=128, step=1)
                dg.Slider(0.62)
                dg.ProgressBar(0.74, label="74% ready")
                dg.Separator()
                dg.Label("WHAT DRAGONGUI STILL LACKS", class_="section")
                with dg.Panel(class_="gap-card"):
                    dg.Label("No icon slot or SVG/icon library in Button/NavItem", class_="tiny")
                    dg.Label("Julius uses lucide icons per row", class_="row-meta")
                with dg.Panel(class_="gap-card"):
                    dg.Label("No scroll container or overflow clipping widget yet", class_="tiny")
                    dg.Label("Dense lists must be manually sized", class_="row-meta")
                with dg.Panel(class_="gap-card"):
                    dg.Label("No shadow, transition, or absolute badge primitives", class_="tiny")
                    dg.Label("CSS styling cannot create those behaviors alone", class_="row-meta")

with dg.StatusBar(class_="footer", height=36):
    dg.Label("ready", class_="safe")
    dg.Label("local runtime", class_="subtle")
    dg.Spacer()
    dg.TextInput("Julius style transfer demo", style={"width": 310})


print(app.run(win))
