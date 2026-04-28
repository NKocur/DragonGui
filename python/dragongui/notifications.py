from __future__ import annotations

from .runtime import AppHandle, ToastHandle, current_app_handle


def toast(
    message: object,
    *,
    level: str = "info",
    duration: int | float | None = 3000,
    opacity: int | float | None = None,
    radius: int | float | None = None,
    padding: int | float | None = None,
    position: str | None = None,
    app: object = None,
) -> ToastHandle:
    """Show a native toast through the active app or an explicit app handle."""
    if app is None:
        return current_app_handle().toast(
            message,
            level=level,
            duration=duration,
            opacity=opacity,
            radius=radius,
            padding=padding,
            position=position,
        )
    if isinstance(app, AppHandle):
        return app.toast(
            message,
            level=level,
            duration=duration,
            opacity=opacity,
            radius=radius,
            padding=padding,
            position=position,
        )
    method = getattr(app, "toast", None)
    if callable(method):
        return method(
            message,
            level=level,
            duration=duration,
            opacity=opacity,
            radius=radius,
            padding=padding,
            position=position,
        )
    handle = getattr(app, "_handle", None)
    if isinstance(handle, AppHandle):
        return handle.toast(
            message,
            level=level,
            duration=duration,
            opacity=opacity,
            radius=radius,
            padding=padding,
            position=position,
        )
    raise TypeError("toast app must be a DragonGUI App, AppHandle, or None")
