from __future__ import annotations

from collections.abc import Callable, Sequence
import threading
from typing import Optional, TypeVar

from . import _backend


PathCallback = Callable[[Optional[str]], None]
PathsCallback = Callable[[Optional[list[str]]], None]
T = TypeVar("T")


class FileDialog:
    """Native file dialog helpers.

    If a callback is provided, the OS dialog runs on a background Python thread
    and the callback is scheduled through ``app.call_soon_threadsafe`` when an
    app handle is supplied. If no app handle is supplied, the callback runs on
    the dialog worker thread and should not mutate live widgets directly.
    Without a callback, the call is synchronous and returns the selected
    path(s), or ``None`` on cancel.
    """

    @staticmethod
    def open_file(
        *,
        title: str | None = None,
        filters: Sequence[tuple[str, Sequence[str]]] | None = None,
        on_select: PathCallback | None = None,
        app: object | None = None,
    ) -> str | None:
        return _run_dialog(
            lambda: _backend.open_file_dialog(title=title, filters=filters),
            on_select,
            app,
        )

    @staticmethod
    def open_files(
        *,
        title: str | None = None,
        filters: Sequence[tuple[str, Sequence[str]]] | None = None,
        on_select: PathsCallback | None = None,
        app: object | None = None,
    ) -> list[str] | None:
        return _run_dialog(
            lambda: _backend.open_files_dialog(title=title, filters=filters),
            on_select,
            app,
        )

    @staticmethod
    def save_file(
        *,
        title: str | None = None,
        filters: Sequence[tuple[str, Sequence[str]]] | None = None,
        on_select: PathCallback | None = None,
        app: object | None = None,
    ) -> str | None:
        return _run_dialog(
            lambda: _backend.save_file_dialog(title=title, filters=filters),
            on_select,
            app,
        )

    @staticmethod
    def pick_folder(
        *,
        title: str | None = None,
        on_select: PathCallback | None = None,
        app: object | None = None,
    ) -> str | None:
        return _run_dialog(
            lambda: _backend.pick_folder_dialog(title=title),
            on_select,
            app,
        )


def open_file_dialog(
    *,
    title: str | None = None,
    filters: Sequence[tuple[str, Sequence[str]]] | None = None,
    on_select: PathCallback | None = None,
    app: object | None = None,
) -> str | None:
    """Open a native single-file picker.

    This is a convenience wrapper around ``FileDialog.open_file``.
    """
    return FileDialog.open_file(
        title=title,
        filters=filters,
        on_select=on_select,
        app=app,
    )


def open_files_dialog(
    *,
    title: str | None = None,
    filters: Sequence[tuple[str, Sequence[str]]] | None = None,
    on_select: PathsCallback | None = None,
    app: object | None = None,
) -> list[str] | None:
    """Open a native multi-file picker."""
    return FileDialog.open_files(
        title=title,
        filters=filters,
        on_select=on_select,
        app=app,
    )


def save_file_dialog(
    *,
    title: str | None = None,
    filters: Sequence[tuple[str, Sequence[str]]] | None = None,
    on_select: PathCallback | None = None,
    app: object | None = None,
) -> str | None:
    """Open a native save-file picker."""
    return FileDialog.save_file(
        title=title,
        filters=filters,
        on_select=on_select,
        app=app,
    )


def pick_folder_dialog(
    *,
    title: str | None = None,
    on_select: PathCallback | None = None,
    app: object | None = None,
) -> str | None:
    """Open a native folder picker."""
    return FileDialog.pick_folder(title=title, on_select=on_select, app=app)


def _run_dialog(
    call: Callable[[], T],
    callback: Callable[[T], None] | None,
    app: object | None,
) -> T | None:
    if callback is None:
        return call()

    def worker() -> None:
        result = call()
        if app is not None and hasattr(app, "call_soon_threadsafe"):
            app.call_soon_threadsafe(lambda: callback(result))  # type: ignore[attr-defined]
        else:
            callback(result)

    threading.Thread(target=worker, daemon=True).start()
    return None
