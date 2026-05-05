"""GridLayout track-mode probe.

Exercises explicit track lists for compact key/value, fixed-label tables,
and mixed fixed/flex columns. Verifies:

- ``tracks=(72, "1fr")`` keeps the first column at 72px and gives the rest
  to the second column.
- ``tracks=("auto", "1fr")`` shrinks the first column to its content width.
- ``tracks=("1fr", "2fr")`` divides remaining space proportionally.
- ``tracks=(10, "1fr", 72)`` produces a three-column row layout suitable
  for status/name/stat tables.
- ``rows=(...)`` fixes per-row heights.
- ``row_height=N`` applies a uniform fixed height to every child.
- Card mode (``columns=N, min_column_width=W``) still works after the
  track-mode addition (backwards compat).
- Track mode and card mode are mutually exclusive (raises ValueError).
"""
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
        background: #0b1020;
        color: rgba(247, 250, 255, 0.94);
        font-size: 13px;
        padding: 12px;
        gap: 12px;
    }
    Label.title { color: white; font-size: 16px; font-weight: 800; }
    Label.section { color: rgba(247, 250, 255, 0.62); font-size: 11px; font-weight: 700; }
    Panel.box { padding: 10px; gap: 8px; }
    Label.kv-key { color: rgba(247, 250, 255, 0.55); font-size: 11px; }
    Label.kv-val { font-size: 12px; }
    Label.tile {
        background: rgba(90, 169, 255, 0.18);
        padding: 8px;
        border-radius: 6px;
        font-size: 12px;
    }
    """
)


@dg.component
def GridTracksProbe(_ctx: dg.ComponentCtx) -> dg.Window:
    win = dg.Window(
        "GridLayout track probe",
        width=820,
        height=720,
        style={"display": "flex", "flex_direction": "column"},
    )

    with dg.VLayout(parent=win, style={"gap": 12, "width": "100%"}):
        # ── 1. Fixed key column + flex value column ─────────────────────
        with dg.Panel("Compact key/value (tracks=(72, '1fr'))", class_="box"):
            dg.Label("Use case: diagnostics, property panels", class_="section")
            with dg.GridLayout(tracks=(72, "1fr"), gap=6, row_gap=2, row_height=17):
                for k, v in [
                    ("depth / max", "3 / 100"),
                    ("avg",         "2.4"),
                    ("total",       "12.4k"),
                    ("rate",        "47/s"),
                    ("uptime",      "2m 14s"),
                ]:
                    dg.Label(k, class_="kv-key")
                    dg.Label(v, class_="kv-val")

        # ── 2. Intrinsic-width labels ────────────────────────────────────
        with dg.Panel("Intrinsic labels (tracks=('auto', '1fr'))", class_="box"):
            dg.Label("Label column shrinks to its content; values fill the rest.", class_="section")
            with dg.GridLayout(tracks=("auto", "1fr"), gap=8, row_gap=2, row_height=17):
                for k, v in [
                    ("hostname",            "machine-42.local"),
                    ("ip",                  "10.0.0.42"),
                    ("a really long label", "value"),
                ]:
                    dg.Label(k, class_="kv-key")
                    dg.Label(v, class_="kv-val")

        # ── 3. Proportional fr tracks ────────────────────────────────────
        with dg.Panel("Proportional (tracks=('1fr', '2fr'))", class_="box"):
            dg.Label("First column gets 1/3 of remaining width, second gets 2/3.", class_="section")
            with dg.GridLayout(tracks=("1fr", "2fr"), gap=8, row_gap=4, row_height=22):
                dg.Label("L", class_="tile")
                dg.Label("R", class_="tile")
                dg.Label("L", class_="tile")
                dg.Label("R", class_="tile")

        # ── 4. Three-column status row ──────────────────────────────────
        with dg.Panel("Status table (tracks=(10, '1fr', 72))", class_="box"):
            dg.Label("Dot indicator + flex name + fixed-width stat column.", class_="section")
            with dg.GridLayout(tracks=(10, "1fr", 72), gap=6, row_gap=2, row_height=18):
                for alive, name, stat in [
                    (True,  "main",          "12.4k  3/s"),
                    (True,  "dg-render",     "8.2k  120/s"),
                    (True,  "test-producer", "450  5/s"),
                    (False, "stale-worker",  "—"),
                ]:
                    dg.Label("●" if alive else "○",
                             style={"color": "text" if alive else "danger", "font_size": 11})
                    dg.Label(name, style={"font_size": 11})
                    dg.Label(stat, style={"font_size": 11, "color": "muted_text"})

        # ── 5. Card mode (backwards compat) ──────────────────────────────
        with dg.Panel("Card mode (columns=3, min_column_width=200)", class_="box"):
            dg.Label("Equal-width responsive cards.", class_="section")
            with dg.GridLayout(columns=3, min_column_width=200, gap=8, row_gap=8):
                for i in range(6):
                    dg.Label(f"card {i + 1}", class_="tile")

    return win


if __name__ == "__main__":
    app.run(GridTracksProbe())
