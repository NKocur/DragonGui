from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #0c111d;
        color: rgba(246, 249, 255, 0.94);
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
        padding-bottom: 72px;
    }

    VLayout.root::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb {
        width: 6px;
        background: rgba(90, 169, 255, 0.72);
        border-radius: 999px;
    }

    HLayout.grid {
        gap: 12px;
        height: auto;
    }

    HLayout.wrap-row {
        gap: 8px;
        height: 34px;
        align-items: center;
    }

    Panel {
        background:
            radial-gradient(circle at 12% 18%, rgba(90, 169, 255, 0.12), transparent 54%),
            rgba(18, 25, 40, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 14px;
        padding: 14px;
        gap: 10px;
        box-shadow: 0 12px 30px rgba(0, 0, 0, 0.24);
    }

    Panel.case {
        width: calc(50% - 6px);
        min-width: 380px;
        min-height: 270px;
    }

    Panel.controls-case {
        min-height: 328px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(246, 249, 255, 0.72);
        line-height: 1.12;
    }

    Label.status {
        background: rgba(116, 221, 176, 0.12);
        border: 1px solid rgba(116, 221, 176, 0.34);
        border-radius: 10px;
        color: rgba(229, 255, 244, 0.95);
        font-weight: 750;
        padding: 8px 10px;
        width: 100%;
    }

    Label.case-title {
        color: white;
        font-weight: 850;
    }

    Button.primary {
        background: #5aa9ff;
        color: #07111f;
        font-weight: 850;
    }

    Button.danger {
        background: rgba(255, 107, 107, 0.18);
        border-color: rgba(255, 107, 107, 0.42);
        color: #ffd1d1;
    }

    TextInput,
    TextArea,
    Dropdown,
    NumberInput {
        background: rgba(255, 255, 255, 0.07);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 10px;
        color: rgba(246, 249, 255, 0.92);
    }

    TextArea.notes {
        text-area-rows: 4;
        line-height: 1.16;
    }

    Checkbox::row {
        background: rgba(255, 255, 255, 0.045);
        border-radius: 10px;
        padding: 8px;
    }

    Checkbox::box {
        border-color: rgba(90, 169, 255, 0.52);
    }

    Checkbox::indicator {
        background: #74ddb0;
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
        background: white;
        border-color: rgba(90, 169, 255, 0.75);
        width: 18px;
        height: 18px;
    }

    NumberInput::field {
        background: rgba(255, 255, 255, 0.055);
    }

    NumberInput::stepper {
        background: rgba(90, 169, 255, 0.14);
    }

    Dropdown::field {
        background: rgba(255, 255, 255, 0.055);
    }

    Dropdown::menu {
        background: rgba(16, 23, 37, 0.98);
        border: 1px solid rgba(255, 255, 255, 0.16);
        border-radius: 12px;
    }

    Dropdown::item-selected {
        background: rgba(90, 169, 255, 0.24);
        color: white;
    }

    ProgressBar {
        height: 24px;
    }

    Badge,
    Tag {
        font-weight: 850;
    }

    Collapsible {
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 12px;
    }

    Collapsible:expanded {
        border-color: rgba(116, 221, 176, 0.38);
    }

    Collapsible:collapsed {
        border-color: rgba(255, 211, 106, 0.34);
    }

    Modal {
        background:
            linear-gradient(180deg, rgba(255, 255, 255, 0.96), rgba(241, 246, 255, 0.98)),
            radial-gradient(circle at 8% 0%, rgba(90, 169, 255, 0.18), transparent 42%);
        accent: #2563eb;
        border: 2px solid #0b2f6b;
        border-radius: 18px;
        box-shadow: 0 26px 70px rgba(2, 8, 23, 0.36);
        color: #101828;
        padding: 18px;
        gap: 10px;
        font-size: 14px;
        font-weight: 650;
    }

    Modal Label.case-title {
        color: #0f172a;
        font-size: 15px;
        font-weight: 850;
    }

    Modal Label.caption {
        color: rgba(15, 23, 42, 0.66);
    }

    Modal Button {
        background: #2563eb;
        border: 1px solid rgba(37, 99, 235, 0.72);
        border-radius: 10px;
        color: white;
        font-weight: 850;
        width: 104px;
    }

    Modal::scrim {
        background:
            radial-gradient(circle at 50% 36%, rgba(37, 99, 235, 0.18), transparent 38%),
            rgba(2, 8, 23, 0.58);
    }

    Tooltip {
        background: rgba(17, 24, 38, 0.98);
        border: 1px solid rgba(90, 169, 255, 0.32);
        border-radius: 12px;
        padding: 10px;
        gap: 6px;
    }
    """
)


win = dg.Window("CSS Core Widgets Probe", width=980, height=760)

with dg.VLayout(class_="root"):
    dg.Label("Core widgets", class_="title")
    dg.Label(
        "This probe checks the common non-data widgets: controls, value inputs, "
        "badges, collapsibles, rich tooltip, modal, and toast callbacks.",
        class_="caption",
    )

    status = dg.Label("Interact with any control to update this status line.", class_="status")
    modal = dg.Modal("Modal probe", open=False, width=420, height=190)

    def set_status(message: str) -> None:
        status.set_value(message)

    def bump_progress() -> None:
        next_value = progress.value + 0.14
        if next_value > 1.0:
            next_value = 0.0
        progress.set_value(next_value)
        set_status(f"Progress set to {round(next_value * 100):.0f}%")

    def show_toast() -> None:
        set_status("Toast requested")
        dg.toast("Core widget probe toast", level="success", duration=1800)

    with dg.HLayout(class_="grid"):
        with dg.Panel("Buttons, badges, and overlays", class_="case controls-case"):
            dg.Label("Buttons should show badge placement, disabled state, modal, and toast behavior.", class_="case-title")
            with dg.HLayout(class_="wrap-row"):
                info_button = dg.Button(
                    "Hover details",
                    badge="?",
                    class_="primary",
                    on_click=lambda: set_status("Primary button clicked"),
                    tooltip="Simple text tooltip on a badged button",
                )
                dg.Button("Show modal", on_click=modal.show)
            with dg.HLayout(class_="wrap-row"):
                dg.Button("Toast", on_click=show_toast)
                dg.Button("Disabled", disabled=True)
            with dg.HLayout(class_="wrap-row"):
                dg.Badge("info", level="info")
                dg.Badge("success", level="success")
                dg.Badge("warning", level="warning")
            with dg.HLayout(class_="wrap-row"):
                dg.Badge("error", level="error")
                dg.Tag("owner: design", level="neutral")

            with dg.Tooltip(target=info_button, width=300, height=116):
                dg.Label("Rich tooltip", class_="case-title")
                dg.Label("This tooltip is a real container with arbitrary children.", class_="caption")
                dg.ProgressBar(0.66, show_value=True)

        with dg.Panel("Text and boolean inputs", class_="case"):
            dg.Label("TextInput, TextArea, Checkbox, and Dropdown should keep spacing and styled parts.", class_="case-title")
            dg.TextInput(
                "draft-query",
                placeholder="Name this query",
                on_change=lambda value: set_status(f"TextInput changed: {value}"),
            )
            dg.TextArea(
                "select *\nfrom events\nwhere score > 0.8",
                rows=3,
                class_="notes",
                on_change=lambda value: set_status(f"TextArea length: {len(value)}"),
            )
            dg.Checkbox(
                "Include archived rows",
                checked=True,
                on_change=lambda checked: set_status(f"Checkbox checked: {checked}"),
            )
            dg.Dropdown(
                ("Daily", "Weekly", "Monthly", "Quarterly"),
                value="Weekly",
                on_change=lambda value: set_status(f"Dropdown selected: {value}"),
            )

    with dg.HLayout(class_="grid"):
        with dg.Panel("Numeric controls", class_="case"):
            dg.Label("Slider and NumberInput should use part styling and update progress.", class_="case-title")
            dg.Slider(
                0.42,
                min=0.0,
                max=1.0,
                step=0.01,
                on_change=lambda value: set_status(f"Slider value: {value:.2f}"),
            )
            dg.NumberInput(
                24,
                min=0,
                max=100,
                step=5,
                on_change=lambda value: set_status(f"NumberInput value: {value:g}"),
            )
            progress = dg.ProgressBar(0.36, show_value=True)
            dg.Button("Bump progress", on_click=bump_progress)

        with dg.Panel("Composite and collapsible controls", class_="case"):
            dg.Label("ColorPicker is a composite widget; Collapsible checks header marker and expanded layout.", class_="case-title")
            dg.ColorPicker(
                (90, 169, 255, 220),
                alpha=True,
                width=330,
                on_change=lambda value: set_status(f"ColorPicker value: {value}"),
            )
            with dg.Collapsible(
                "Advanced options",
                expanded=False,
                on_change=lambda expanded: set_status(f"Collapsible expanded: {expanded}"),
            ):
                dg.Label("Collapsed content should appear below the header when expanded.", class_="caption")
                dg.Checkbox("Enable experimental styling", checked=False)

    with modal:
        dg.Label("Modal content should be padded and rounded.", class_="case-title")
        dg.Label("Closing this modal should return focus to the main probe.", class_="caption")
        dg.Button("Close", on_click=modal.close)


if __name__ == "__main__":
    print(app.run(win))
