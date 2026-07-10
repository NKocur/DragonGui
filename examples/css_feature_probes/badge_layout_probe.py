from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#4da3ff", radius=8, focus="#ffd166"))
app.stylesheet(
    """
    Window {
        background: #0d1118;
        color: rgba(247, 250, 255, 0.94);
        padding: 14px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
        overflow-y: auto;
        padding-right: 20px;
        padding-bottom: 80px;
    }

    VLayout.root::scrollbar-track {
        width: 8px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.08);
        border-radius: 999px;
    }

    VLayout.root::scrollbar-thumb {
        width: 6px;
        background: rgba(77, 163, 255, 0.72);
        border-radius: 999px;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(247, 250, 255, 0.70);
        line-height: 1.12;
    }

    Label.pass {
        background: rgba(108, 211, 151, 0.12);
        border: 1px solid rgba(108, 211, 151, 0.34);
        border-radius: 8px;
        color: rgba(228, 255, 238, 0.96);
        font-weight: 800;
        padding: 7px 9px;
        width: 100%;
    }

    Panel.summary {
        width: 100%;
        background: rgba(20, 28, 40, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        padding: 10px;
        gap: 8px;
    }

    FlowLayout.summary-row {
        width: 100%;
        height: auto;
        gap: 8px;
        row-gap: 8px;
    }

    VLayout.summary-stack {
        width: 132px;
        min-width: 118px;
        gap: 6px;
        height: auto;
    }

    Button.summary-narrow {
        width: 66px;
    }

    Tabs.summary-tabs {
        width: 126px;
        height: 92px;
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 8px;
    }

    Sidebar.summary-nav {
        width: 118px;
        height: 92px;
        background: rgba(11, 17, 27, 0.98);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 8px;
        padding: 6px;
        gap: 4px;
        overflow-y: auto;
    }

    Panel.case {
        width: 100%;
        background: rgba(20, 28, 40, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        padding: 12px;
        gap: 10px;
    }

    FlowLayout.badge-flow {
        width: 100%;
        gap: 7px;
        row-gap: 8px;
        height: auto;
    }

    FlowLayout.narrow-flow {
        width: 176px;
        gap: 5px;
        row-gap: 6px;
        height: auto;
        padding: 6px;
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 8px;
    }

    HLayout.row {
        width: 100%;
        height: auto;
        gap: 8px;
        align-items: center;
    }

    HLayout.button-row {
        width: 100%;
        height: auto;
        gap: 8px;
        align-items: center;
    }

    Button, SmallButton {
        font-weight: 760;
    }

    Button.narrow {
        width: 72px;
    }

    Button.long-label {
        width: 150px;
    }

    SmallButton.narrow {
        width: 54px;
    }

    Button.styled::badge,
    SmallButton.styled::badge {
        background: #ffd166;
        color: #10141b;
        border: 2px solid rgba(255, 255, 255, 0.75);
        border-radius: 999px;
        padding: 5px 9px;
        font-size: 13px;
    }

    Button.tall::badge {
        background: #6cd397;
        color: #07110c;
        padding: 7px 10px;
        border: 2px solid rgba(255, 255, 255, 0.80);
        font-size: 16px;
    }

    Badge.big,
    Tag.big {
        font-size: 16px;
        padding: 6px 12px;
        border: 2px solid rgba(255, 255, 255, 0.65);
    }

    Badge.mono {
        font-variant-numeric: tabular-nums;
        font-family: Consolas;
    }

    Badge.constrained,
    Tag.constrained {
        width: 92px;
    }

    Tabs.badge-tabs {
        width: 100%;
        height: 166px;
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 10px;
    }

    Tabs.narrow-tabs {
        width: 184px;
        height: 150px;
    }

    Tab::tab {
        padding: 8px 10px;
        border-radius: 8px 8px 0 0;
    }

    Tab:selected::tab {
        background: rgba(77, 163, 255, 0.20);
        color: white;
    }

    Tab::accent {
        background: #6cd397;
        height: 3px;
        border-radius: 999px 999px 0 0;
    }

    Tab.styled::badge {
        background: #ff7a90;
        color: white;
        border: 2px solid rgba(255, 255, 255, 0.64);
        padding: 4px 9px;
    }

    Sidebar.audit-nav {
        width: 210px;
        height: 292px;
        background: rgba(11, 17, 27, 0.98);
        border: 1px solid rgba(255, 255, 255, 0.12);
        border-radius: 10px;
        padding: 10px;
        gap: 6px;
        overflow-y: auto;
    }

    Sidebar.narrow-nav {
        width: 122px;
    }

    NavItem::item {
        padding: 8px 9px;
        border-radius: 8px;
    }

    NavItem:selected::item {
        background: rgba(77, 163, 255, 0.18);
        border: 1px solid rgba(77, 163, 255, 0.38);
    }

    NavItem::accent {
        background: #4da3ff;
        width: 4px;
        border-radius: 999px;
    }

    NavItem.styled::badge {
        background: #6cd397;
        color: #07110c;
        border: 2px solid rgba(255, 255, 255, 0.72);
        padding: 4px 8px;
    }

    NavItem:disabled {
        opacity: 0.58;
    }
    """
)

win = dg.Window("Badge layout visual probe", width=900, height=640)

with dg.VLayout(class_="root"):
    dg.Label("Badge layout visual probe", class_="title")
    dg.Label(
        "PASS labels mark expected containment: pills and text should stay inside controls, "
        "wrap rows should not overlap, and narrow controls should clip badge text instead of losing the badge.",
        class_="caption",
    )

    with dg.Panel("First-viewport badge stress summary", class_="summary"):
        with dg.FlowLayout(class_="summary-row"):
            with dg.VLayout(class_="summary-stack"):
                dg.Badge("owner: platform-design", class_="constrained")
                dg.Tag("tag-overflow-value", class_="constrained")
                dg.Button("N", badge="1234567890", class_="summary-narrow styled")
            with dg.Tabs(value="tiny", class_="summary-tabs"):
                with dg.Tab("Tiny", value="tiny", badge="1234567890"):
                    dg.Label("Tab badge")
                with dg.Tab("Long", value="long", badge="owner: platform-design", class_="styled"):
                    dg.Label("Long tab")
            with dg.Sidebar(title="Nav", class_="summary-nav"):
                dg.NavItem("Active", page="active-summary", badge="1234567890", class_="styled")
                dg.NavItem("Long label", page="long-summary", badge="overflow-count")
        dg.Label("PASS: summary badges remain visible and clipped inside their parents.", class_="pass")

    with dg.Panel("Standalone badges and tags", class_="case"):
        with dg.FlowLayout(class_="badge-flow"):
            for level in ("info", "success", "warning", "danger", "neutral"):
                dg.Badge(level, level=level)
            dg.Badge("")
            dg.Badge("7", class_="mono")
            dg.Badge("1234567890", class_="mono")
            dg.Badge("owner: platform-design", level="warning")
            dg.Badge("big padded", level="success", class_="big")
            dg.Tag("release-candidate")
            dg.Tag("custom border", level="info", class_="big")
        with dg.FlowLayout(class_="narrow-flow"):
            dg.Badge("constrained-long-badge", class_="constrained")
            dg.Tag("tag-overflow-value", class_="constrained")
            dg.Badge("99+", level="danger")
        dg.Label("PASS: standalone badge text stays inside each pill; narrow flow wraps without overlap.", class_="pass")

    with dg.Panel("Button and SmallButton badges", class_="case"):
        with dg.FlowLayout(class_="badge-flow"):
            dg.Button("Deploy", badge=3)
            dg.Button("Disabled", badge="new", disabled=True)
            dg.Button("Narrow", badge="1234567890", class_="narrow")
            dg.Button("Long deploy label", badge="99+", class_="long-label")
            dg.Button("Styled", badge="owner: platform-design", class_="styled")
            dg.Button("Tall", badge="42", class_="tall")
            dg.SmallButton("Small", badge=8)
            dg.SmallButton("N", badge="overflow-count", class_="narrow styled")
            dg.SmallButton("Disabled", badge="5", disabled=True)
        dg.Label("PASS: button labels yield space to badges; badges stay clipped inside narrow buttons.", class_="pass")

    with dg.Panel("Tab badges", class_="case"):
        with dg.Tabs(value="active", class_="badge-tabs"):
            with dg.Tab("Active metrics", value="active", badge="12"):
                dg.Label("Selected tab with badge.")
            with dg.Tab("Very long tab label", value="long", badge="overflow-count", class_="styled"):
                dg.Label("Long label gives way before colliding with the badge.")
            with dg.Tab("Disabled", value="disabled", badge="5", disabled=True):
                dg.Label("Disabled tab.")
        with dg.Tabs(value="tiny", class_="badge-tabs narrow-tabs"):
            with dg.Tab("Tiny selected", value="tiny", badge="1234567890"):
                dg.Label("Narrow selected tab keeps a clipped badge.")
            with dg.Tab("Other long label", value="other", badge="owner: platform-design", class_="styled"):
                dg.Label("Narrow unselected tab.")
        dg.Label("PASS: tab accents, labels, and badges align without overlap or missing badges.", class_="pass")

    with dg.Panel("NavItem badges", class_="case"):
        with dg.HLayout(class_="row"):
            with dg.Sidebar(title="Wide", class_="audit-nav"):
                dg.NavItem("Overview", page="overview", badge="3")
                dg.NavItem("Active analytics", page="analytics", badge="123", class_="styled")
                dg.NavItem("Long route label", page="long-route", badge="overflow-count")
                dg.NavItem("Disabled queue", page="disabled", badge="9", disabled=True)
                for index in range(1, 8):
                    dg.NavItem(f"Scrollable item {index}", page=f"scroll-{index}", badge=index if index % 2 else None)
            with dg.Sidebar(title="Narrow", class_="audit-nav narrow-nav"):
                dg.NavItem("Active", page="active", badge="1234567890", class_="styled")
                dg.NavItem("Very long label", page="long", badge="owner: platform-design")
                dg.NavItem("Disabled", page="disabled-narrow", badge="5", disabled=True)
        dg.Label("PASS: nav labels do not push badges past the sidebar edge; active accent remains separate.", class_="pass")


if __name__ == "__main__":
    print(app.run(win))
