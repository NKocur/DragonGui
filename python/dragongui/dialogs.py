from __future__ import annotations

from collections.abc import Callable, Sequence
import threading
from typing import TypeVar

from . import _backend


PathCallback = Callable[[str | None], None]
PathsCallback = Callable[[list[str] | None], None]
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
