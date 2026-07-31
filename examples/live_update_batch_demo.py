"""Focused example of batching a dashboard's latest-state property updates."""

from __future__ import annotations

import threading
import time

import dragongui as dg


def build_demo() -> tuple[dg.App, dg.Window]:
    app = dg.App(theme=dg.Theme.dark(), loading_screen=False)
    window = dg.Window("Batched live updates", width=720, height=420)

    with dg.VLayout(style={"height": "100%", "padding": 20, "gap": 12}):
        dg.Label("One logical telemetry frame, one native property packet")
        with dg.Panel("Telemetry", style={"gap": 10, "padding": 14}):
            status = dg.Label("Waiting for samples")
            progress = dg.ProgressBar(0.0)
            with dg.HLayout(style={"gap": 10, "align_items": "center"}):
                mode = dg.Badge("idle", level="info")
                counter = dg.Label("sample 0")
        explanation = dg.Label(
            "The worker coalesces replaceable snapshots. Each applied snapshot "
            "batches the four ordinary live property setters below.",
            wrap=True,
        )

    def producer() -> None:
        runtime_ready = False
        readiness_deadline = time.monotonic() + 15.0
        for sample in range(1, 301):
            fraction = (sample % 100) / 100.0

            def apply(sample: int = sample, fraction: float = fraction) -> None:
                with app.update_batch():
                    status.set_value(f"Streaming sample {sample}")
                    progress.set_value(fraction)
                    mode.set_value("active" if fraction else "cycle")
                    counter.set_value(f"sample {sample}")

            while True:
                try:
                    app.call_soon_threadsafe(apply, coalesce_key="telemetry-frame")
                    runtime_ready = True
                    break
                except RuntimeError:
                    if runtime_ready or time.monotonic() >= readiness_deadline:
                        return
                    time.sleep(0.01)
            time.sleep(1.0 / 60.0)

        app.call_soon_threadsafe(
            lambda: explanation.set_value("Stream complete; the final retained state is visible."),
            coalesce_key="telemetry-frame",
        )

    threading.Thread(target=producer, name="batched-telemetry", daemon=True).start()
    return app, window


def main() -> None:
    app, window = build_demo()
    app.run(window)


if __name__ == "__main__":
    main()
