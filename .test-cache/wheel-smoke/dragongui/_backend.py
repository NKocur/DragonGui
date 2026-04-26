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


def run_document(document: dict[str, Any]) -> dict[str, Any]:
    if _native is None:
        if _dev_fallback_enabled():
            return {
                "status": "ok",
                "native": False,
                "renderer": "dev-fallback",
                "event_loop": "not_started",
                "message": "native backend is unavailable; returned serialized UI document",
                "document": document,
            }

        raise BackendUnavailableError(
            "DragonGUI's native extension is not built. Run `maturin develop` "
            "or install a wheel from PyPI before calling App.run()."
        )

    return dict(_native.run_app(document))


def _dev_fallback_enabled() -> bool:
    value = os.environ.get("DRAGONGUI_DEV_FALLBACK", "")
    return value.lower() in {"1", "true", "yes", "on"}
