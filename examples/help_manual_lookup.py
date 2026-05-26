"""Print selected sections from the built-in DragonGUI manual."""

from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


def main() -> None:
    print(dg.help("widgets.plots.scatter"))
    print(dg.help.reference.widgets.number_input())
    print(dg.help.reference.css_parts())


if __name__ == "__main__":
    main()
