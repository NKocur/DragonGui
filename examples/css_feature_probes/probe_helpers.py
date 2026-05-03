from __future__ import annotations

from contextlib import contextmanager
from typing import Iterator

import dragongui as dg


def probe_app(title: str, *, width: int = 940, height: int = 720) -> tuple[dg.App, dg.Window]:
    """Create a dark probe app/window pair with the common probe size."""
    app = dg.App(theme=dg.Theme.dark(accent="#5aa9ff", radius=8, focus="#ffd36a"))
    return app, dg.Window(title, width=width, height=height)


def probe_header(title: str, description: str) -> None:
    dg.Label(title, class_="title")
    dg.Label(description, class_="caption")


@contextmanager
def probe_grid(
    *,
    columns: int | str = 2,
    min_column_width: int = 390,
    gap: int | None = None,
    row_gap: int | None = None,
    class_: str = "grid",
) -> Iterator[dg.GridLayout]:
    grid = dg.GridLayout(
        columns=columns,
        min_column_width=min_column_width,
        gap=gap,
        row_gap=row_gap,
        class_=class_,
    )
    with grid:
        yield grid


@contextmanager
def probe_card(
    title: str,
    *,
    scroll: bool = False,
    class_: str = "case",
) -> Iterator[dg.Panel]:
    classes = f"{class_} scroll-card" if scroll else class_
    card = dg.Panel(title, class_=classes)
    with card:
        yield card
