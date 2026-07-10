from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg

from probe_helpers import probe_grid


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))

BASE_FORM_CSS = """
    Window {
        background: #0b1020;
        color: rgba(247, 250, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
        overflow-y: auto;
        padding-right: 24px;
        padding-bottom: 76px;
    }

    VLayout.root::scrollbar-track,
    Panel::scrollbar-track {
        width: 8px;
        padding: 2px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb,
    Panel::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.72);
        border-radius: 999px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(247, 250, 255, 0.72);
        line-height: 1.12;
    }

    Label.status {
        width: 100%;
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 10px;
        color: rgba(229, 255, 244, 0.96);
        font-weight: 800;
        padding: 8px 10px;
    }

    Label.case-title {
        color: white;
        font-weight: 850;
    }

    GridLayout.grid {
        width: 100%;
        height: auto;
        gap: 12px;
    }

    HLayout.control-row {
        height: 38px;
        gap: 10px;
        align-items: center;
    }

    Panel {
        background:
            radial-gradient(circle at 10% 8%, rgba(90, 169, 255, 0.12), transparent 52%),
            rgba(17, 24, 39, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 14px;
        padding: 14px;
        gap: 10px;
        box-shadow: 0 12px 30px rgba(0, 0, 0, 0.24);
    }

    Panel.case {
        min-height: 312px;
    }

    Panel.theme-switcher {
        width: 100%;
        min-height: 118px;
    }

    HLayout.theme-row,
    FlowLayout.theme-row {
        height: 40px;
        gap: 10px;
        align-items: center;
    }

    Button.theme-toggle {
        min-width: 138px;
    }

    TextInput,
    TextArea,
    Dropdown,
    NumberInput {
        width: 100%;
        background: rgba(255, 255, 255, 0.07);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 10px;
        color: rgba(247, 250, 255, 0.94);
    }

    TextInput:focus,
    TextArea:focus,
    Dropdown:focus,
    NumberInput:focus {
        outline: 2px solid rgba(255, 211, 106, 0.72);
        outline-offset: 2px;
    }

    TextInput.compact {
        width: 180px;
    }

    NumberInput.half {
        width: calc(50% - 5px);
        min-width: 120px;
        flex-shrink: 1;
    }

    TextArea.editor {
        text-area-rows: 5;
        line-height: 1.14;
    }

    TextArea.nowrap {
        font-family: "Consolas";
        line-height: 1.18;
    }

    Button {
        min-width: 104px;
        border-radius: 10px;
        font-weight: 800;
    }

    Button.primary {
        background: #5aa9ff;
        border-color: rgba(90, 169, 255, 0.8);
        color: #07111f;
    }

    Button.ghost {
        background: rgba(255, 255, 255, 0.06);
        border-color: rgba(255, 255, 255, 0.16);
        color: rgba(247, 250, 255, 0.92);
    }

    Button::badge {
        background: #ffd36a;
        color: #111827;
        border-radius: 999px;
        font-weight: 900;
        padding: 2px 7px;
    }

    Checkbox::row {
        width: 100%;
        background: rgba(255, 255, 255, 0.045);
        border-radius: 10px;
        padding: 8px 10px;
    }

    Checkbox::box {
        border-color: rgba(90, 169, 255, 0.55);
        border-radius: 6px;
    }

    Checkbox::indicator {
        background: #74ddb0;
        border-radius: 4px;
    }

    Checkbox::label {
        color: rgba(247, 250, 255, 0.88);
        font-weight: 700;
    }

    Dropdown::field {
        background: rgba(255, 255, 255, 0.055);
    }

    Dropdown::chevron {
        color: #ffd36a;
    }

    Dropdown::menu {
        background: rgba(14, 22, 36, 0.98);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 12px;
    }

    Dropdown::item-hover {
        background: rgba(90, 169, 255, 0.16);
    }

    Dropdown::item-selected {
        background: rgba(90, 169, 255, 0.28);
        color: white;
        font-weight: 850;
    }

    Slider {
        width: 100%;
    }

    Slider::track,
    ProgressBar::track {
        background: rgba(255, 255, 255, 0.12);
        border-radius: 999px;
    }

    Slider::fill,
    ProgressBar::fill {
        background: linear-gradient(90deg, #5aa9ff, #74ddb0);
        border-radius: 999px;
    }

    Slider::thumb {
        width: 18px;
        height: 18px;
        background: white;
        border: 2px solid rgba(90, 169, 255, 0.82);
    }

    NumberInput::field {
        background: rgba(255, 255, 255, 0.055);
    }

    NumberInput::stepper {
        background: rgba(90, 169, 255, 0.14);
    }

    NumberInput::stepper-up,
    NumberInput::stepper-down {
        color: white;
    }

    ProgressBar {
        height: 24px;
    }

    ProgressBar::label {
        color: rgba(247, 250, 255, 0.94);
        font-weight: 850;
    }

    .disabled-note {
        color: rgba(247, 250, 255, 0.54);
    }

    @media (max-width: 520px) {
        Window {
            padding: 0;
        }

        VLayout.root {
            padding-right: 8px;
        }

        Panel.theme-switcher {
            min-height: 156px;
        }

        HLayout.theme-row,
        FlowLayout.theme-row {
            height: auto;
        }

        Button.theme-toggle {
            min-width: 74px;
        }

        Button,
        Button.ghost,
        Button.primary {
            min-width: 78px;
        }

        TextInput.compact {
            width: 0;
            flex: 1;
            min-width: 0;
        }
    }
"""

app.stylesheet(BASE_FORM_CSS)


FORM_THEME_CSS = {
    "Midnight": """
        Window {
            background: #0b1020;
            color: rgba(247, 250, 255, 0.94);
        }

        Panel {
            background:
                radial-gradient(circle at 10% 8%, rgba(90, 169, 255, 0.12), transparent 52%),
                rgba(17, 24, 39, 0.96);
            border-color: rgba(255, 255, 255, 0.14);
            color: rgba(247, 250, 255, 0.94);
        }

        TextInput,
        TextArea,
        Dropdown,
        NumberInput {
            background: rgba(255, 255, 255, 0.07);
            border-color: rgba(255, 255, 255, 0.16);
            color: rgba(247, 250, 255, 0.94);
        }

        Button.primary,
        Slider::fill,
        ProgressBar::fill {
            background: linear-gradient(90deg, #5aa9ff, #74ddb0);
            color: #07111f;
        }

        VLayout.root::scrollbar-track,
        Panel::scrollbar-track {
            background: rgba(255, 255, 255, 0.08);
            border: 1px solid rgba(255, 255, 255, 0.10);
        }

        VLayout.root::scrollbar-thumb,
        Panel::scrollbar-thumb {
            background: linear-gradient(180deg, #5aa9ff, #74ddb0);
        }
    """,
    "Clinical Light": """
        Window {
            background: #eef3f8;
            color: #172033;
        }

        Panel {
            background: #ffffff;
            border: 1px solid #b9c6d6;
            border-radius: 6px;
            box-shadow: 0 8px 20px rgba(33, 56, 84, 0.12);
            color: #172033;
        }

        Label.title,
        Label.case-title {
            color: #0f172a;
        }

        Label.caption,
        .disabled-note {
            color: rgba(15, 23, 42, 0.66);
        }

        Label.status {
            background: #e0f2fe;
            border-color: #7dd3fc;
            color: #075985;
        }

        TextInput,
        TextArea,
        Dropdown,
        NumberInput {
            background: #f8fafc;
            border-color: #9fb0c4;
            color: #0f172a;
        }

        NumberInput::field {
            background: #ffffff;
        }

        NumberInput::stepper {
            background: #dbeafe;
            border-left: 1px solid #7c8da3;
        }

        NumberInput::stepper-up,
        NumberInput::stepper-down {
            color: #0f3b7a;
        }

        NumberInput::stepper-divider {
            background: #94a3b8;
        }

        Button {
            background: #f8fafc;
            border-color: #94a3b8;
            color: #0f172a;
        }

        Button.primary,
        Slider::fill,
        ProgressBar::fill {
            background: linear-gradient(90deg, #1d4ed8, #0891b2);
            color: #ffffff;
        }

        Slider::track,
        ProgressBar::track {
            background: #cbd5e1;
            border: 1px solid #94a3b8;
        }

        Slider::fill,
        ProgressBar::fill {
            background: linear-gradient(90deg, #1e40af, #0e7490);
        }

        Slider::thumb {
            background: #ffffff;
            border: 3px solid #1e40af;
            box-shadow: 0 2px 8px rgba(30, 64, 175, 0.28);
        }

        Checkbox::row {
            background: #f8fafc;
        }

        Checkbox::label {
            color: #172033;
        }

        Dropdown::menu {
            background: #ffffff;
            border-color: #94a3b8;
        }

        VLayout.root::scrollbar-track,
        Panel::scrollbar-track {
            background: #d9e2ec;
            border: 1px solid #b6c3d1;
        }

        VLayout.root::scrollbar-thumb,
        Panel::scrollbar-thumb {
            background: linear-gradient(180deg, #2563eb, #0891b2);
            border: 1px solid rgba(15, 23, 42, 0.18);
        }
    """,
    "Terminal": """
        Window {
            background: #030712;
            color: #d1fae5;
        }

        Panel {
            background:
                linear-gradient(180deg, rgba(6, 78, 59, 0.22), rgba(3, 7, 18, 0.98)),
                #030712;
            border: 1px solid #10b981;
            border-radius: 2px;
            box-shadow: 0 0 0 1px rgba(16, 185, 129, 0.16), 0 18px 36px rgba(0, 0, 0, 0.38);
            color: #d1fae5;
        }

        Label.title,
        Label.case-title {
            color: #6ee7b7;
        }

        Label.caption,
        .disabled-note {
            color: rgba(209, 250, 229, 0.68);
        }

        Label.status {
            background: rgba(16, 185, 129, 0.12);
            border-color: rgba(52, 211, 153, 0.58);
            color: #a7f3d0;
        }

        TextInput,
        TextArea,
        Dropdown,
        NumberInput {
            background: #020617;
            border-color: #10b981;
            border-radius: 2px;
            color: #d1fae5;
            font-family: "Consolas";
        }

        Button {
            background: #052e2b;
            border-color: #10b981;
            border-radius: 2px;
            color: #a7f3d0;
        }

        Button.primary,
        Slider::fill,
        ProgressBar::fill {
            background: linear-gradient(90deg, #10b981, #bef264);
            color: #022c22;
        }

        Button::badge {
            background: #bef264;
            color: #022c22;
        }

        Checkbox::row,
        Dropdown::field,
        NumberInput::field,
        NumberInput::stepper {
            background: rgba(16, 185, 129, 0.10);
        }

        Dropdown::menu {
            background: #020617;
            border-color: #10b981;
        }

        VLayout.root::scrollbar-track,
        Panel::scrollbar-track {
            background: rgba(16, 185, 129, 0.10);
            border: 1px solid rgba(16, 185, 129, 0.46);
        }

        VLayout.root::scrollbar-thumb,
        Panel::scrollbar-thumb {
            background: linear-gradient(180deg, #34d399, #bef264);
            border: 1px solid rgba(190, 242, 100, 0.50);
        }
    """,
    "Warning": """
        Window {
            background: #211004;
            color: #fff7ed;
        }

        Panel {
            background:
                radial-gradient(circle at 8% 10%, rgba(251, 191, 36, 0.28), transparent 44%),
                #2a1204;
            border: 2px solid #fb923c;
            border-radius: 18px;
            box-shadow: 0 18px 44px rgba(127, 29, 29, 0.32);
            color: #fff7ed;
        }

        Label.title,
        Label.case-title {
            color: #ffedd5;
        }

        Label.caption,
        .disabled-note {
            color: rgba(255, 247, 237, 0.70);
        }

        Label.status {
            background: rgba(251, 146, 60, 0.18);
            border-color: rgba(251, 191, 36, 0.56);
            color: #ffedd5;
        }

        TextInput,
        TextArea,
        Dropdown,
        NumberInput {
            background: rgba(67, 20, 7, 0.84);
            border-color: #fdba74;
            color: #fff7ed;
        }

        Button {
            background: rgba(154, 52, 18, 0.64);
            border-color: #fdba74;
            color: #fff7ed;
        }

        Button.primary,
        Slider::fill,
        ProgressBar::fill {
            background: linear-gradient(90deg, #f97316, #facc15);
            color: #2a1204;
        }

        Button::badge {
            background: #facc15;
            color: #431407;
        }

        Checkbox::indicator {
            background: #facc15;
        }

        VLayout.root::scrollbar-track,
        Panel::scrollbar-track {
            background: rgba(67, 20, 7, 0.84);
            border: 1px solid rgba(251, 191, 36, 0.42);
        }

        VLayout.root::scrollbar-thumb,
        Panel::scrollbar-thumb {
            background: linear-gradient(180deg, #f97316, #facc15);
            border: 1px solid rgba(255, 247, 237, 0.28);
        }
    """,
}


win = dg.Window("CSS Form Controls Probe", width=980, height=760)

with dg.VLayout(class_="root"):
    dg.Label("Form controls", class_="title")
    dg.Label(
        "This probe isolates text editing, dropdown overlays, checkbox parts, numeric controls, "
        "badges, disabled controls, and value-update callbacks.",
        class_="caption",
    )

    status = dg.Label("Interact with a control to update this status line.", class_="status")

    def set_status(message: str) -> None:
        status.set_value(message)

    def apply_form_theme(name: str) -> None:
        app.stylesheet(f"{BASE_FORM_CSS}\n{FORM_THEME_CSS[name]}")
        set_status(f"Theme switched: {name}")

    def cycle_badge() -> None:
        current = int(filter_button.badge or "0")
        next_value = 0 if current >= 5 else current + 1
        filter_button.set_badge(next_value)
        set_status(f"Button badge changed to {next_value}")

    with dg.Panel("Theme stress test", class_="theme-switcher"):
        dg.Label(
            "Switch between extreme CSS themes to check control geometry, contrast, focus rings, and part styling.",
            class_="case-title",
        )
        with dg.FlowLayout(class_="theme-row", cross_align="center"):
            for theme_name in FORM_THEME_CSS:
                dg.Button(
                    theme_name,
                    class_="theme-toggle",
                    on_click=lambda name=theme_name: apply_form_theme(name),
                )

    with probe_grid(gap=12):
        with dg.Panel("Text entry", class_="case"):
            dg.Label("Inputs should keep padding, focus rings, wrapping, and caret placement.", class_="case-title")
            dg.TextInput(
                "Northwind analysis",
                placeholder="Report name",
                on_change=lambda value: set_status(f"TextInput changed: {value}"),
            )
            with dg.HLayout(class_="control-row"):
                dg.TextInput(
                    "compact",
                    class_="compact",
                    placeholder="Short code",
                    on_change=lambda value: set_status(f"Compact input: {value}"),
                )
                dg.Button("Save", class_="primary", on_click=lambda: set_status("Save clicked"))
                dg.Button("Reset", class_="ghost", on_click=lambda: set_status("Reset clicked"))
            dg.TextArea(
                "select account_id, score, updated_at\nfrom model_runs\nwhere score >= 0.82\norder by updated_at desc",
                rows=5,
                class_="editor",
                on_change=lambda value: set_status(f"Editor chars: {len(value)}"),
            )
            dg.TextArea(
                "nowrap: C:/very/long/path/that/should/scroll/horizontally/instead/of/wrapping.csv",
                rows=2,
                wrap=False,
                class_="nowrap",
                on_change=lambda value: set_status(f"No-wrap chars: {len(value)}"),
            )

        with dg.Panel("Choice controls", class_="case"):
            dg.Label("Dropdown overlays, checkbox parts, and button badges should stay aligned.", class_="case-title")
            dg.Dropdown(
                ("Draft", "Review", "Approved", "Archived"),
                value="Review",
                on_change=lambda value: set_status(f"Status selected: {value}"),
            )
            dg.Dropdown(
                ("CSV export", "Parquet export", "SQLite snapshot", "Notebook bundle"),
                value="Parquet export",
                on_change=lambda value: set_status(f"Export type: {value}"),
            )
            dg.Checkbox(
                "Include archived records",
                checked=True,
                on_change=lambda checked: set_status(f"Archived records: {checked}"),
            )
            dg.Checkbox(
                "Notify workspace members",
                checked=False,
                on_change=lambda checked: set_status(f"Notifications: {checked}"),
            )
            with dg.HLayout(class_="control-row"):
                filter_button = dg.Button("Filters", badge=2, on_click=cycle_badge)
                dg.Button("Disabled", disabled=True)
                dg.Button("Apply", class_="primary", on_click=lambda: set_status("Apply clicked"))

    with probe_grid(gap=12):
        with dg.Panel("Numeric controls", class_="case"):
            dg.Label("Slider, NumberInput, and ProgressBar should align their internal parts.", class_="case-title")
            dg.Slider(
                0.37,
                min=0,
                max=1,
                step=0.01,
                on_change=lambda value: set_status(f"Confidence: {value:.2f}"),
            )
            dg.Slider(
                72,
                min=0,
                max=100,
                step=1,
                on_change=lambda value: set_status(f"Threshold: {value:.0f}"),
            )
            with dg.HLayout(class_="control-row"):
                dg.NumberInput(
                    24,
                    min=0,
                    max=100,
                    step=5,
                    class_="half",
                    on_change=lambda value: set_status(f"Batch size: {value:g}"),
                )
                dg.NumberInput(
                    0.85,
                    min=0,
                    max=1,
                    step=0.05,
                    class_="half",
                    on_change=lambda value: set_status(f"Cutoff: {value:.2f}"),
                )
            progress = dg.ProgressBar(0.64, show_value=True)
            dg.Button(
                "Advance",
                class_="primary",
                on_click=lambda: (
                    progress.set_value(0.0 if progress.value >= 1.0 else progress.value + 0.12),
                    set_status("Progress advanced"),
                ),
            )

        with dg.Panel("Disabled and edge states", class_="case"):
            dg.Label("Disabled controls should remain legible and keep stable geometry.", class_="case-title")
            dg.TextInput("Read-only value", disabled=True)
            dg.TextArea(
                "Disabled multiline text should keep its row height and padding.",
                rows=3,
                disabled=True,
            )
            dg.Dropdown(("Disabled A", "Disabled B", "Disabled C"), value="Disabled B", disabled=True)
            dg.Checkbox("Disabled checked option", checked=True, disabled=True)
            dg.Slider(0.48, disabled=True)
            dg.NumberInput(12, min=0, max=20, disabled=True)
            dg.Label("PASS: disabled controls are dimmed but not collapsed or clipped.", class_="disabled-note")


if __name__ == "__main__":
    print(app.run(win))
