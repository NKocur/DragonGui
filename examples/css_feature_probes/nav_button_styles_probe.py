"""Nav button style probe.

Compares the current sidebar NavItem look against three redesigned styles so
the styles can be judged (and clicked) side by side before adopting one in the
V3 demo. Each rail is bound to its own Pages instance, so hover and selected
states are fully live — click the items to see the highlighted (active) state.

Styles:
  * Current      - the existing V3 demo look (kept as the baseline).
  * A - Refined  - same "left accent bar" family, cleaned up: tinted fill,
                   defined border, rounded pill indicator, brighter active text.
  * B - Pill     - no left bar; the active item is a solid accent pill with
                   dark text. Modern app-shell / SaaS sidebar feel.
  * C - Card     - every item is a subtle card; the active item lifts with a
                   green-tinted fill, green border and a rounded left marker.
"""

from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


def log(message: str) -> None:
    print(message, flush=True)


app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))
app.stylesheet(
    """
    Window {
        background: #0c111d;
        color: rgba(245, 248, 255, 0.94);
        padding: 18px;
        gap: 14px;
        font-size: 14px;
    }

    /* Whole page scrolls when the rows exceed the window height. */
    ScrollArea.page {
        width: 100%;
        height: 100%;
        min-height: 0;
        flex-grow: 1;
        flex-shrink: 1;
        gap: 14px;
        padding-right: 14px;
        padding-bottom: 28px;
    }

    ScrollArea.page::scrollbar-track {
        width: 10px;
        padding: 1px;
        background: rgba(255, 255, 255, 0.06);
        border-radius: 999px;
    }

    ScrollArea.page::scrollbar-thumb {
        width: 8px;
        background: rgba(90, 169, 255, 0.60);
        border-radius: 999px;
    }

    Label.title {
        color: #ffffff;
        font-size: 21px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(245, 248, 255, 0.70);
        line-height: 1.15;
    }

    HLayout.rails {
        width: 100%;
        gap: 14px;
        align-items: start;
    }

    /* One column per style. */
    VLayout.col {
        width: 268px;
        gap: 8px;
        align-items: stretch;
    }

    Label.col-title {
        color: #ffffff;
        font-size: 15px;
        font-weight: 800;
    }

    Label.col-note {
        color: rgba(245, 248, 255, 0.60);
        font-size: 12px;
        line-height: 1.2;
        min-height: 44px;
    }

    /* The nav rail chrome (shared by all columns). */
    VLayout.rail {
        background: rgba(16, 23, 38, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 14px;
        padding: 10px;
        gap: 6px;
    }

    /* Small "which page is active" read-out under each rail. */
    Pages.peek {
        width: 100%;
    }

    Page {
        padding: 0;
    }

    Label.peek-value {
        color: rgba(245, 248, 255, 0.82);
        font-size: 12px;
        font-weight: 750;
        padding: 6px 4px;
    }

    /* ------------------------------------------------------------------ */
    /* Current (baseline) - reproduces the existing V3 demo NavItem look:  */
    /* minimal rule, native defaults draw the active fill + accent bar.    */
    /* ------------------------------------------------------------------ */
    NavItem.current {
        accent: #5aa9ff;
        border-radius: 5px;
        color: rgba(245, 248, 255, 0.94);
    }

    /* The accent bar is 3px wide; its left inset is now CSS-controllable via
       the accent part's padding. Inset 4px + item padding 11px gives the bar
       an even 4px gap on BOTH sides (edge->bar and bar->text). */
    NavItem.current::accent {
        padding: 4px;
    }

    NavItem.current::item {
        padding: 11px;
    }

    /* ------------------------------------------------------------------ */
    /* A - Refined: evolution of the current left-accent-bar style.        */
    /* ------------------------------------------------------------------ */
    /* Base radius lives on the NODE (not ::item) so the highlight glow/ring
       and the button body share ONE silhouette. ::item leaves its radius
       unset and inherits the node's corners. */
    NavItem.refined {
        color: rgba(233, 240, 252, 0.74);
        border-radius: 9px;
    }

    NavItem.refined::item {
        background: transparent;
        border: 1px solid transparent;
        padding: 9px 11px;
    }

    /* No accent bar - the highlight is a shape-following element instead. */
    NavItem.refined::accent {
        width: 0px;
    }

    NavItem.refined:hover {
        color: #ffffff;
    }

    NavItem.refined:hover::item {
        background: rgba(255, 255, 255, 0.06);
        border-color: rgba(255, 255, 255, 0.12);
    }

    NavItem.refined:selected {
        color: #ffffff;
    }

    NavItem.refined:selected::item {
        background: rgba(90, 169, 255, 0.20);
        border-color: rgba(120, 185, 255, 0.62);
    }

    /* ---- The second highlight element: a crisp line/shape, not a glow --
       Each is opt-in via a class and follows the button's silhouette, because
       the outline and the item border both use the node's corner radii. */

    /* Ring - a keyline drawn just OUTSIDE the shape, tracing its corners. */
    NavItem.hl-ring:selected {
        outline: 2px solid rgba(122, 185, 255, 0.90);
        outline-offset: 3px;
    }

    /* Keyline - a bright border ON the shape edge, hugging the silhouette. */
    NavItem.hl-keyline:selected::item {
        border: 2px solid #6fb4ff;
    }

    /* Double - two concentric lines: an edge keyline plus an outer ring. */
    NavItem.hl-double:selected {
        outline: 2px solid rgba(122, 185, 255, 0.55);
        outline-offset: 4px;
    }
    NavItem.hl-double:selected::item {
        border: 2px solid #6fb4ff;
    }

    /* Line - a single crisp vertical hairline on the leading edge. */
    NavItem.hl-line::accent {
        background: #6fb4ff;
        width: 3px;
        border-radius: 999px;
    }

    /* ---- Shape treatments (add one alongside `refined`) --------------- */
    /* Per-corner longhands on the NODE (:selected), so the body AND the
       highlight element share the silhouette. The 4-value shorthand is not
       expanded by the engine. Corners: TL / TR / BR / BL. */

    /* Blade - one diagonal swept round, the other kept crisp; a dynamic,
       leaning silhouette. */
    NavItem.shape-blade:selected {
        border-top-left-radius: 4px;
        border-top-right-radius: 18px;
        border-bottom-right-radius: 4px;
        border-bottom-left-radius: 18px;
    }

    /* Bookmark - the left edge swells into a full pill cap while the right
       stays squared, so the active row reads like a tab pulled from the rail. */
    NavItem.shape-bookmark:selected {
        border-top-left-radius: 999px;
        border-top-right-radius: 5px;
        border-bottom-right-radius: 5px;
        border-bottom-left-radius: 999px;
    }

    /* Wedge - one oversized corner throws the rectangle off-balance. */
    NavItem.shape-wedge:selected {
        border-top-left-radius: 5px;
        border-top-right-radius: 5px;
        border-bottom-right-radius: 22px;
        border-bottom-left-radius: 5px;
    }

    /* Chip - softer take: the other diagonal rounded, the rest tightened. */
    NavItem.shape-chip:selected {
        border-top-left-radius: 16px;
        border-top-right-radius: 5px;
        border-bottom-right-radius: 16px;
        border-bottom-left-radius: 5px;
    }

    NavItem.refined:disabled {
        color: rgba(233, 240, 252, 0.32);
    }

    /* ------------------------------------------------------------------ */
    /* B - Pill: no left bar; active item is a solid accent pill.          */
    /* ------------------------------------------------------------------ */
    NavItem.pill {
        color: rgba(233, 240, 252, 0.76);
    }

    NavItem.pill::item {
        background: transparent;
        border: 1px solid transparent;
        border-radius: 999px;
        padding: 9px 14px;
    }

    NavItem.pill::accent {
        width: 0px;
    }

    NavItem.pill:hover {
        color: #ffffff;
    }

    NavItem.pill:hover::item {
        background: rgba(255, 255, 255, 0.07);
    }

    NavItem.pill:selected {
        color: #06121f;
    }

    NavItem.pill:selected::item {
        background: #5aa9ff;
        border-color: #5aa9ff;
    }

    NavItem.pill:disabled {
        color: rgba(233, 240, 252, 0.32);
    }

    /* ------------------------------------------------------------------ */
    /* C - Card: each item is a card; active lifts with a green tint.      */
    /* ------------------------------------------------------------------ */
    NavItem.card {
        color: rgba(233, 240, 252, 0.72);
    }

    NavItem.card::item {
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.07);
        border-radius: 12px;
        padding: 10px 12px;
    }

    NavItem.card:hover {
        color: #ffffff;
    }

    NavItem.card:hover::item {
        background: rgba(255, 255, 255, 0.07);
        border-color: rgba(116, 221, 176, 0.30);
    }

    NavItem.card:selected {
        color: #ffffff;
    }

    NavItem.card:selected::item {
        background: rgba(116, 221, 176, 0.16);
        border-color: rgba(116, 221, 176, 0.55);
    }

    NavItem.card::accent {
        background: #74ddb0;
        width: 0px;
        border-radius: 999px;
    }

    NavItem.card:selected::accent {
        width: 4px;
    }

    NavItem.card:disabled {
        color: rgba(233, 240, 252, 0.32);
    }
    """
)


NAV_ITEMS = ("Overview", "Scatter", "Histograms", "Runtime")


def build_column(title: str, note: str, prefix: str, style_class: str) -> None:
    """Build one labelled rail wired to its own Pages so selection is live."""
    with dg.VLayout(class_="col"):
        dg.Label(title, class_="col-title")
        dg.Label(note, class_="col-note")
        with dg.VLayout(class_="rail"):
            for index, label in enumerate(NAV_ITEMS):
                dg.NavItem(
                    label,
                    page=f"{prefix}-{index}",
                    class_=style_class,
                    badge="3" if label == "Runtime" else None,
                )
            dg.NavItem("Disabled", page=f"{prefix}-disabled", class_=style_class, disabled=True)
        with dg.Pages(
            value=f"{prefix}-0",
            class_="peek",
            on_change=lambda value, name=title: log(f"{name}: {value}"),
        ):
            for index, label in enumerate(NAV_ITEMS):
                with dg.Page(f"{prefix}-{index}", title=label):
                    dg.Label(f"active: {label}", class_="peek-value")
            with dg.Page(f"{prefix}-disabled", title="Disabled"):
                dg.Label("active: Disabled", class_="peek-value")


win = dg.Window("Nav Button Style Probe", width=1200, height=860)

with dg.ScrollArea(class_="page"):
    dg.Label("Sidebar nav button styles", class_="title")
    dg.Label(
        "Four rails, each on its own Pages. Hover to see the resting/hover states and "
        "click an item to see the highlighted (active) state. The current look is kept "
        "as a baseline; A/B/C are the redesign candidates.",
        class_="caption",
    )

    with dg.HLayout(class_="rails"):
        build_column(
            "Current (baseline)",
            "Existing V3 look: tinted fill + default left accent bar.",
            "current",
            "current",
        )
        build_column(
            "A - Refined",
            "Active row re-shapes (Blade) AND a crisp ring traces that exact shape.",
            "refined",
            "refined shape-blade hl-ring",
        )
        build_column(
            "B - Pill",
            "No left bar. Active item is a solid accent pill with dark text.",
            "pill",
            "pill",
        )
        build_column(
            "C - Card",
            "Every item is a card; active lifts with a green tint and left marker.",
            "card",
            "card",
        )

    dg.Separator()
    dg.Label("A - Refined: shape treatments (each with the shape-tracing ring)", class_="title")
    dg.Label(
        "Same Refined body; the active row abstracts its outline AND a crisp ring "
        "traces that silhouette. Click an item to see the shape snap in with its ring.",
        class_="caption",
    )

    with dg.HLayout(class_="rails"):
        build_column(
            "Blade (current A)",
            "Opposite corners swept round - a dynamic, leaning silhouette.",
            "shape-blade",
            "refined shape-blade hl-ring",
        )
        build_column(
            "Bookmark",
            "Left edge swells to a pill cap; reads like a tab pulled from the rail.",
            "shape-bookmark",
            "refined shape-bookmark hl-ring",
        )
        build_column(
            "Wedge",
            "One oversized corner throws the rectangle off-balance.",
            "shape-wedge",
            "refined shape-wedge hl-ring",
        )
        build_column(
            "Chip",
            "Softer take - the other diagonal rounded, the rest tightened.",
            "shape-chip",
            "refined shape-chip hl-ring",
        )

    dg.Separator()
    dg.Label("A - Refined: highlight element options (crisp lines/shapes)", class_="title")
    dg.Label(
        "The second highlight element on its own - a line/keyline, not a glow. Same Blade "
        "shape, different treatment. Click to reveal it, then mix any shape above with any of these.",
        class_="caption",
    )

    with dg.HLayout(class_="rails"):
        build_column(
            "Ring (default)",
            "A keyline just outside the shape, tracing its corners.",
            "hl-ring",
            "refined shape-blade hl-ring",
        )
        build_column(
            "Keyline",
            "A bright border on the shape edge, hugging the silhouette.",
            "hl-keyline",
            "refined shape-blade hl-keyline",
        )
        build_column(
            "Double",
            "Two concentric lines: an edge keyline plus an outer ring.",
            "hl-double",
            "refined shape-blade hl-double",
        )
        build_column(
            "Line",
            "A single crisp vertical hairline on the leading edge.",
            "hl-line",
            "refined shape-blade hl-line",
        )


if __name__ == "__main__":
    print(app.run(win))
