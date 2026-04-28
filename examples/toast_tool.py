from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))

import dragongui as dg


app = dg.App()
app.stylesheet(
    """
    Toast {
        border-radius: 14px;
        border-width: 1px;
        padding: 16px;
        font-weight: 650;
    }

    Toast.success {
        background: success;
        border-color: success;
        color: white;
    }

    Toast.warning {
        background: warning;
        border-color: warning;
        color: background;
    }

    Toast.error {
        background: danger;
        border-color: danger;
        color: white;
    }
    """
)
win = dg.Window("DragonGUI Toast Demo", width=760, height=430)


def show_persistent() -> None:
    toast = app.toast(
        "Uploading report...",
        duration=None,
        opacity=0.94,
        radius=12,
        padding=16,
        position="bottom-right",
    )

    def finish() -> None:
        toast.update("Upload complete", level="success", duration=2400)

    app.call_soon_threadsafe(finish)


with dg.HLayout(style={"padding": 18, "gap": 16}):
    with dg.Panel("Notifications", width=300, style={"padding": 14, "gap": 10}):
        dg.Button("Info", on_click=lambda: dg.toast("3 items updated", level="info"))
        dg.Button(
            "Success",
            on_click=lambda: app.toast(
                "Export complete",
                level="success",
                radius=14,
                position="top-left",
            ),
        )
        dg.Button(
            "Warning",
            on_click=lambda: dg.toast(
                "Connection is slow",
                level="warning",
                duration=4500,
                opacity=0.86,
                position="bottom-left",
            ),
        )
        dg.Button(
            "Error",
            on_click=lambda: app.toast(
                "Error saving file",
                level="error",
                duration=5000,
                radius=3,
                padding=18,
            ),
        )
        dg.Button("Persistent Then Update", on_click=show_persistent)

    with dg.Panel("Status", style={"padding": 14, "gap": 10}):
        dg.Label("Click the buttons to show native toast overlays.")
        dg.Label("Toasts can stack in any window corner.")
        dg.Label("Toast surfaces can be styled globally with CSS.")
        dg.Label("Opacity, radius, and padding can still be overridden per toast.")
        dg.Label("Persistent toasts remain visible until updated or dismissed.")


if __name__ == "__main__":
    print(app.run(win))
