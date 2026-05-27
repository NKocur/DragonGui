from __future__ import annotations

import os
import platform
from collections.abc import Sequence
from typing import Any


class BackendUnavailableError(RuntimeError):
    """Raised when the Rust native backend has not been built."""


try:
    from . import _dragongui as _native
except ImportError:  # pragma: no cover - depends on local build state
    _native = None


def native_backend_available() -> bool:
    return _native is not None


def native_event_loop_available() -> bool:
    return _native is not None and not _dev_fallback_enabled()


def backend_info() -> dict[str, Any]:
    if _native is None:
        profile = _runtime_profile()
        return {
            "name": "dragongui",
            "native": False,
            "renderer": "unavailable",
            "status": "native extension has not been built",
            "platform": {
                "os": platform.system().lower(),
                "arch": platform.machine().lower(),
                "profile": profile,
                "profile_requested": os.environ.get("DRAGONGUI_PROFILE", "auto"),
                "profile_source": "python-fallback",
                "pi_feature": False,
                "auto_pi_target": _auto_pi_target(),
                "scatter_max_points": _scatter_max_points(profile),
                "scatter_lod_threshold": _scatter_lod_threshold(profile),
                "line_plot_max_points": _line_plot_max_points(profile),
                "table_page_size": _table_page_size(profile),
                "table_sample_rows": _table_sample_rows(profile),
                "table_column_buffer_rows": _table_column_buffer_rows(profile),
                "wgpu_backend_override": os.environ.get("DRAGONGUI_WGPU_BACKEND"),
            },
            "features": {
                "pi": False,
                "gpu": False,
                "webview": False,
            },
            "webview_available": False,
        }

    info = dict(_native.backend_info())
    return _normalize_backend_info(info)


def _normalize_backend_info(info: dict[str, Any]) -> dict[str, Any]:
    profile = _runtime_profile()
    platform_info = dict(info.get("platform") or {})
    effective_profile = str(platform_info.get("profile") or profile)
    platform_info.setdefault("os", platform.system().lower())
    platform_info.setdefault("arch", platform.machine().lower())
    platform_info.setdefault("profile", effective_profile)
    platform_info.setdefault("profile_requested", os.environ.get("DRAGONGUI_PROFILE", "auto"))
    platform_info.setdefault("profile_source", "python-normalized")
    platform_info.setdefault("pi_feature", False)
    platform_info.setdefault("auto_pi_target", _auto_pi_target())
    platform_info.setdefault("scatter_max_points", _scatter_max_points(effective_profile))
    platform_info.setdefault("scatter_lod_threshold", _scatter_lod_threshold(effective_profile))
    platform_info.setdefault("line_plot_max_points", _line_plot_max_points(effective_profile))
    platform_info.setdefault("table_page_size", _table_page_size(effective_profile))
    platform_info.setdefault("table_sample_rows", _table_sample_rows(effective_profile))
    platform_info.setdefault(
        "table_column_buffer_rows", _table_column_buffer_rows(effective_profile)
    )
    platform_info.setdefault("wgpu_backend_override", os.environ.get("DRAGONGUI_WGPU_BACKEND"))
    info["platform"] = platform_info

    features = dict(info.get("features") or {})
    features.setdefault("pi", False)
    features.setdefault("gpu", bool(info.get("native")))
    features.setdefault("webview", False)
    info["features"] = features
    info.setdefault("webview_available", bool(features["webview"]))
    return info


DialogFilters = Sequence[tuple[str, Sequence[str]]]


def open_file_dialog(
    *,
    title: str | None = None,
    filters: DialogFilters | None = None,
) -> str | None:
    normalized = _dialog_filters(filters)
    if _native is not None and hasattr(_native, "open_file_dialog"):
        return _native.open_file_dialog(title, normalized)
    return _tk_file_dialog("open_file", title=title, filters=filters)


def open_files_dialog(
    *,
    title: str | None = None,
    filters: DialogFilters | None = None,
) -> list[str] | None:
    normalized = _dialog_filters(filters)
    if _native is not None and hasattr(_native, "open_files_dialog"):
        result = _native.open_files_dialog(title, normalized)
        return None if result is None else list(result)
    return _tk_file_dialog("open_files", title=title, filters=filters)


def save_file_dialog(
    *,
    title: str | None = None,
    filters: DialogFilters | None = None,
) -> str | None:
    normalized = _dialog_filters(filters)
    if _native is not None and hasattr(_native, "save_file_dialog"):
        return _native.save_file_dialog(title, normalized)
    return _tk_file_dialog("save_file", title=title, filters=filters)


def pick_folder_dialog(*, title: str | None = None) -> str | None:
    if _native is not None and hasattr(_native, "pick_folder_dialog"):
        return _native.pick_folder_dialog(title)
    return _tk_file_dialog("pick_folder", title=title, filters=None)


def run_document(
    document: dict[str, Any],
    click_callbacks: dict[str, Any] | None = None,
    change_callbacks: dict[str, Any] | None = None,
    app_handle: Any | None = None,
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
            "debug_snapshot": {
                "schema": 1,
                "runtime": {
                    "window_open": False,
                    "gpu_ready": False,
                    "platform": {
                        "os": platform.system().lower(),
                        "arch": platform.machine().lower(),
                        "profile": _runtime_profile(),
                        "profile_requested": os.environ.get("DRAGONGUI_PROFILE", "auto"),
                        "profile_source": "python-dev-fallback",
                        "pi_feature": False,
                        "auto_pi_target": _auto_pi_target(),
                        "webview_available": False,
                        "scatter_max_points": _scatter_max_points(_runtime_profile()),
                        "scatter_lod_threshold": _scatter_lod_threshold(_runtime_profile()),
                        "line_plot_max_points": _line_plot_max_points(_runtime_profile()),
                        "table_page_size": _table_page_size(_runtime_profile()),
                        "table_sample_rows": _table_sample_rows(_runtime_profile()),
                        "table_column_buffer_rows": _table_column_buffer_rows(_runtime_profile()),
                        "wgpu_backend_override": os.environ.get("DRAGONGUI_WGPU_BACKEND"),
                    },
                    "frames_rendered": 0,
                    "upload_ms": 0.0,
                    "frame_ms": 0.0,
                    "command_queue_depth": 0,
                    "loading_screen": {
                        "enabled": bool(
                            (document.get("loading_screen") or {}).get("enabled", True)
                        ),
                        "shown": False,
                        "frames": 0,
                        "present_ms": 0.0,
                        "startup_resource_ms": 0.0,
                        "min_duration_ms": int(
                            (document.get("loading_screen") or {}).get("min_duration_ms", 0)
                        ),
                    },
                },
                "document": document,
            },
        }

    if _native is None:
        raise BackendUnavailableError(
            "DragonGUI's native extension is not built. Run `maturin develop` "
            "or install a wheel from PyPI before calling App.run()."
        )

    if app_handle is not None:
        return dict(
            _native.run_app_with_handle(
                document,
                click_callbacks or {},
                change_callbacks or {},
                app_handle,
            )
        )

    return dict(_native.run_app(document, click_callbacks or {}, change_callbacks or {}))


def _dev_fallback_enabled() -> bool:
    value = os.environ.get("DRAGONGUI_DEV_FALLBACK", "")
    return value.lower() in {"1", "true", "yes", "on"}


def _auto_pi_target() -> bool:
    return platform.system().lower() == "linux" and platform.machine().lower() in {
        "aarch64",
        "arm64",
    }


def _runtime_profile() -> str:
    requested = os.environ.get("DRAGONGUI_PROFILE", "auto").strip().lower()
    if requested in {"pi", "rpi", "raspberry-pi", "raspberry_pi"}:
        return "pi"
    if requested == "desktop":
        return "desktop"
    return "pi" if _auto_pi_target() else "desktop"


def _scatter_max_points(profile: str) -> int | None:
    return 200_000 if profile == "pi" else None


def _scatter_lod_threshold(profile: str) -> int | None:
    return 50_000 if profile == "pi" else None


def _line_plot_max_points(profile: str) -> int | None:
    return 50_000 if profile == "pi" else None


def _table_page_size(profile: str) -> int | None:
    return 64 if profile == "pi" else None


def _table_sample_rows(profile: str) -> int | None:
    return 512 if profile == "pi" else None


def _table_column_buffer_rows(profile: str) -> int | None:
    return 10_000 if profile == "pi" else None


def _dialog_filters(filters: DialogFilters | None) -> list[tuple[str, list[str]]] | None:
    if filters is None:
        return None
    normalized: list[tuple[str, list[str]]] = []
    for name, extensions in filters:
        label = str(name).strip()
        exts = [str(ext).strip().lstrip("*.") for ext in extensions if str(ext).strip()]
        if not label or not exts:
            raise ValueError("file dialog filters must contain a name and at least one extension")
        normalized.append((label, exts))
    return normalized


def _tk_file_dialog(
    kind: str,
    *,
    title: str | None,
    filters: DialogFilters | None,
) -> Any:
    try:
        import tkinter as tk
        from tkinter import filedialog
    except Exception as exc:  # pragma: no cover - platform/display dependent
        raise BackendUnavailableError("no native or tkinter file dialog is available") from exc

    root = tk.Tk()
    root.withdraw()
    try:
        filetypes = None
        normalized = _dialog_filters(filters)
        if normalized:
            filetypes = [
                (name, " ".join(f"*.{ext}" for ext in extensions))
                for name, extensions in normalized
            ]
        options: dict[str, Any] = {}
        if title:
            options["title"] = title
        if filetypes:
            options["filetypes"] = filetypes
        if kind == "open_file":
            path = filedialog.askopenfilename(**options)
            return path or None
        if kind == "open_files":
            paths = filedialog.askopenfilenames(**options)
            return list(paths) or None
        if kind == "save_file":
            path = filedialog.asksaveasfilename(**options)
            return path or None
        if kind == "pick_folder":
            path = filedialog.askdirectory(**options)
            return path or None
        raise ValueError(f"unknown file dialog kind: {kind}")
    finally:
        root.destroy()
