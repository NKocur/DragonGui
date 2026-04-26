from __future__ import annotations

import os
from typing import Any


class BackendUnavailableError(RuntimeError):
    """Raised when the Rust native backend has not been built."""


try:
    from . import _dragongui as _native
except ImportError:  # pragma: no cover - depends on local build state
    _native = None


def native_backend_available() -> bool:
    return _native is not None


def backend_info() -> dict[str, Any]:
    if _native is None:
        return {
            "name": "dragongui",
            "native": False,
            "renderer": "unavailable",
            "status": "native extension has not been built",
        }

    return dict(_native.backend_info())


def run_document(
    document: dict[str, Any],
    click_callbacks: dict[str, Any] | None = None,
    change_callbacks: dict[str, Any] | None = None,
) -> dict[str, Any]:
    # DRAGONGUI_DEV_FALLBACK=1 skips the native event loop entirely and returns
    # the serialized document.  Used in CI smoke tests and pytest to avoid
    # opening a real window.  Works even when the native extension is built.
    if _dev_fallback_enabled():
        return {
            "status": "ok",
            "native": _native is not None,
            "renderer": "dev-fallback",
            "event_loop": "not_started",
            "message": "native backend skipped; returned serialized UI document",
            "document": document,
        }

    if _native is None:
        raise BackendUnavailableError(
            "DragonGUI's native extension is not built. Run `maturin develop` "
            "or install a wheel from PyPI before calling App.run()."
        )

    return dict(
        _native.run_app(
            document,
            click_callbacks or {},
            change_callbacks or {},
        )
    )


def _dev_fallback_enabled() -> bool:
    value = os.environ.get("DRAGONGUI_DEV_FALLBACK", "")
    return value.lower() in {"1", "true", "yes", "on"}
