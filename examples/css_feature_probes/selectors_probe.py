from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8))
app.stylesheet(
    """
    Window {
        background: #0d1320;
        color: rgba(245, 248, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    Panel {
        background: rgba(18, 25, 39, 0.94);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 14px;
        box-shadow: 0 12px 30px rgba(0, 0, 0, 0.26);
        padding: 12px;
        gap: 8px;
    }

    VLayout.scroll-root {
        height: 684px;
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

    Label.title {
        color: #5aa9ff;
        font-size: 20px;
        font-weight: 800;
    }

    Label.caption {
        color: rgba(245, 248, 255, 0.72);
        line-height: 1.12;
    }

    Label.case-title {
        color: rgba(245, 248, 255, 0.95);
        font-weight: 800;
    }

    Label.pass {
        color: #74ddb0;
        font-weight: 800;
    }

    Panel.case {
        width: 348px;
        min-height: 150px;
        overflow: visible;
    }

    Button,
    TextInput,
    Dropdown {
        height: 30px;
    }

    Button {
        background: rgba(255, 255, 255, 0.055);
        border-color: rgba(255, 255, 255, 0.16);
    }

    Button.primary {
        background: rgba(90, 169, 255, 0.18);
        border-color: rgba(90, 169, 255, 0.42);
    }

    #id-target {
        border-width: 2px;
        border-color: #5aa9ff;
        background: rgba(90, 169, 255, 0.16);
    }

    [key="key-target"] {
        color: #74ddb0;
        font-weight: 800;
    }

    [level="warning"] {
        background: rgba(255, 211, 106, 0.18);
        border-color: rgba(255, 211, 106, 0.62);
        color: #ffd36a;
    }

    Button[text^="Run"] {
        background: rgba(116, 221, 176, 0.16);
        border-color: rgba(116, 221, 176, 0.44);
    }

    Button[text="run report" i] {
        outline: 2px solid rgba(116, 221, 176, 0.56);
        outline-offset: 2px;
    }

    Panel.descendant Button {
        border-color: rgba(116, 221, 176, 0.55);
    }

    Panel.direct > Button {
        background: rgba(255, 211, 106, 0.13);
    }

    Panel.direct Panel Button {
        background: rgba(255, 255, 255, 0.055);
        border-color: rgba(255, 255, 255, 0.16);
    }

    Panel.chain > HLayout > Button {
        background: rgba(90, 169, 255, 0.16);
        border-color: rgba(90, 169, 255, 0.40);
    }

    Panel.structural > Button:first-child {
        border-color: #74ddb0;
    }

    Panel.structural > Button:nth-child(3) {
        border-color: #ff6584;
    }

    Panel.structural > Button:nth-child(2) {
        background: rgba(255, 211, 106, 0.16);
    }

    Panel.structural > Panel:last-child:empty {
        height: 8px;
        padding: 0;
        background: rgba(255, 211, 106, 0.46);
        border-color: rgba(255, 211, 106, 0.68);
        border-radius: 999px;
    }

    Panel.only > Label:only-child {
        color: #74ddb0;
        font-weight: 800;
    }

    Panel.filtered > *:nth-child(2 of Button.primary) {
        background: rgba(255, 211, 106, 0.20);
        border-color: rgba(255, 211, 106, 0.72);
    }

    Button:not(.ghost) {
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.12);
    }

    Button:is(.danger, .primary) {
        color: white;
        font-weight: 800;
    }

    :where(.soft) {
        opacity: 0.72;
    }

    Panel.has-direct:has(> Badge[level="success"]) {
        border-color: rgba(116, 221, 176, 0.62);
        background: rgba(116, 221, 176, 0.09);
    }

    Panel.has-descendant:has(HLayout > Badge) {
        border-color: rgba(90, 169, 255, 0.62);
    }

    Panel.sibling-source:has(+ Button.primary) {
        border-color: rgba(255, 211, 106, 0.72);
        background: rgba(255, 211, 106, 0.08);
    }

    Panel.has-checked:has(> Checkbox:checked) {
        border-color: rgba(116, 221, 176, 0.72);
    }

    Button:hover {
        background: rgba(90, 169, 255, 0.28);
        border-color: #5aa9ff;
    }

    Button:active {
        scale: 0.98;
    }

    TextInput:focus {
        outline: 2px solid rgba(90, 169, 255, 0.70);
        outline-offset: 2px;
    }

    Button:disabled {
        opacity: 0.45;
        background: rgba(255, 255, 255, 0.035);
    }

    Checkbox:checked {
        accent: #74ddb0;
    }

    Dropdown:open {
        outline: 2px solid rgba(255, 211, 106, 0.66);
        outline-offset: 2px;
    }

    Collapsible:expanded {
        border-color: rgba(116, 221, 176, 0.56);
    }

    Collapsible:collapsed {
        border-color: rgba(255, 211, 106, 0.56);
        background: rgba(255, 211, 106, 0.07);
    }
    """
)


win = dg.Window("CSS Selectors Probe", width=790, height=720)

with dg.VLayout(class_="scroll-root"):
    dg.Label("Selectors and pseudo-states", class_="title")
    dg.Label(
        "Static cards should show green/yellow/blue highlights. Dynamic controls "
        "should react to hover, focus, checked, open, expanded, and disabled states.",
        class_="caption",
    )

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Type, class, id, key, attributes", class_="case"):
            dg.Label("Basic selector targets", class_="case-title")
            dg.Button("Primary class", class_="primary")
            dg.Button("ID target", id="id-target")
            dg.Label("key target should be green", key="key-target")
            with dg.HLayout(style={"gap": 8}):
                dg.Badge("warning badge", level="warning")
                dg.Button("Run report")

        with dg.Panel("Descendant and child chains", class_="case descendant direct chain"):
            dg.Label("Descendant, direct child, nested chain", class_="case-title")
            dg.Button("Direct child")
            with dg.Panel(style={"padding": 6, "gap": 6}):
                dg.Button("Nested descendant")
            with dg.HLayout(style={"gap": 8}):
                dg.Button("Chain target")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Structural selectors", class_="case structural"):
            dg.Button("First child button")
            dg.Button("Second child button")
            dg.Button("Third child button")
            dg.Panel()

        with dg.Panel("Only child and filtered nth", class_="case"):
            with dg.Panel(class_="only", style={"padding": 8}):
                dg.Label("Only child label should be green")
            with dg.Panel(class_="filtered", style={"padding": 8, "gap": 6}):
                dg.Button("Plain")
                dg.Button("Primary 1", class_="primary")
                dg.Button("Primary 2", class_="primary")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Selector functions", class_="case has-direct has-descendant has-checked"):
            dg.Label(":not, :is, :where, :has", class_="case-title")
            dg.Button("Primary via :is", class_="primary")
            dg.Button("Soft ghost via :where", class_="ghost soft")
            dg.Checkbox("Checked child for :has", checked=True)
            with dg.HLayout(style={"gap": 8}):
                dg.Badge("success badge", level="success")

        with dg.Panel("Sibling :has(+ ...)", class_="case"):
            with dg.Panel(class_="sibling-source"):
                dg.Label("This panel should turn yellow because next sibling is primary.")
            dg.Button("Primary next sibling", class_="primary")

    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Dynamic states", class_="case"):
            dg.Label("Hover button, focus input, open dropdown", class_="case-title")
            dg.Button("Hover / active target")
            dg.Button("Disabled target", disabled=True)
            dg.TextInput("Focus me", placeholder="Focus test")
            dg.Dropdown(["Closed", "Open state"], value="Closed")
            dg.Checkbox("Checked accent state", checked=True)

        with dg.Panel("Expanded and collapsed", class_="case"):
            dg.Label("Collapsible state pseudos", class_="case-title")
            with dg.Collapsible("Expanded starts green", expanded=True):
                dg.Label("Expanded body")
            with dg.Collapsible("Collapsed starts yellow", expanded=False):
                dg.Label("Collapsed body")


if __name__ == "__main__":
    print(app.run(win))
