from __future__ import annotations

import sys
from pathlib import Path
from pprint import pprint

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


class MillionRowFrame:
    columns = tuple(f"col_{i:02d}" for i in range(20))
    dtypes = tuple("float32" if i % 3 else "int64" for i in range(20))
    shape = (1_000_000, 20)


frame = MillionRowFrame()

app = dg.App(theme=dg.Theme.dark(accent="#3fc7ff"))
win = dg.Window("DragonGUI DataFrame Table Demo", width=1200, height=800)

with dg.VLayout():
    status = dg.Label("Select a cell")

    def show_selection(selection: dg.TableSelection) -> None:
        status.set_value(
            f"Selected row {selection.row_index}, {selection.column}: {selection.value}"
        )

    dg.Label("Metadata-backed virtualized table")
    dg.Label("Click a cell, then use Arrow keys, PageUp/PageDown, Home/End, or Enter.")
    dg.DataFrameTable(frame, page_size=80, on_select=show_selection)

try:
    result = app.run(win)
except dg.BackendUnavailableError:
    print("DragonGUI source import works.")
    print("Native backend is not built yet, so this run prints the UI document instead.")
    pprint(app.document(win))
else:
    if result.get("renderer") == "dev-fallback":
        print("DragonGUI dev fallback is active.")
        pprint(result["document"])
