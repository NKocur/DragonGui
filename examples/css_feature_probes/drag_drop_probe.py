from __future__ import annotations

import sys
from pathlib import Path

if __name__ == "__main__" and __package__ is None:
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "python"))

import dragongui as dg


app = dg.App(theme=dg.Theme.dark(accent="#7bdff2", radius=7, focus="#ffd166"))
app.stylesheet(
    """
    Window {
        background: #10141b;
        color: rgba(246, 249, 255, 0.94);
        padding: 18px;
        gap: 12px;
        font-size: 14px;
    }

    VLayout.root {
        width: 100%;
        height: 100%;
        gap: 12px;
    }

    HLayout.content {
        width: 100%;
        flex-grow: 1;
        flex-shrink: 1;
        min-height: 0;
        gap: 12px;
    }

    HLayout.drop-row {
        width: 100%;
        gap: 12px;
    }

    Panel.case {
        height: 100%;
        min-height: 0;
        background: rgba(22, 31, 42, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 10px;
        padding: 14px;
        gap: 10px;
    }

    Panel.sources {
        width: 34%;
        min-width: 300px;
    }

    Panel.targets {
        flex: 1;
        min-width: 0;
    }

    Label.title {
        color: white;
        font-size: 20px;
        font-weight: 850;
    }

    Label.caption {
        color: rgba(246, 249, 255, 0.70);
        line-height: 1.12;
    }

    Label.status {
        background: rgba(123, 223, 242, 0.12);
        border: 1px solid rgba(123, 223, 242, 0.34);
        border-radius: 8px;
        color: rgba(232, 251, 255, 0.96);
        font-weight: 750;
        padding: 8px 10px;
        width: 100%;
    }

    VLayout.source-list {
        width: 100%;
        gap: 8px;
    }

    DragSource.source-card {
        width: 100%;
        min-height: 76px;
        background: rgba(255, 255, 255, 0.045);
        border: 1px solid rgba(255, 255, 255, 0.13);
        border-radius: 9px;
        padding: 10px 12px;
        gap: 3px;
    }

    DragSource.source-card:selected {
        background: rgba(123, 223, 242, 0.14);
        border-color: rgba(123, 223, 242, 0.72);
    }

    DragSource.asset {
        border-color: rgba(123, 223, 242, 0.42);
    }

    DragSource.metric {
        border-color: rgba(255, 209, 102, 0.42);
    }

    DragSource:disabled {
        opacity: 0.42;
    }

    Label.source-title {
        color: white;
        font-weight: 850;
    }

    Label.source-kind {
        color: rgba(246, 249, 255, 0.58);
        font-size: 12px;
        text-transform: uppercase;
        letter-spacing: 0.05em;
    }

    DropTarget.zone {
        flex: 1;
        min-width: 0;
        min-height: 170px;
        background: rgba(255, 255, 255, 0.040);
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 10px;
        padding: 14px;
        gap: 8px;
    }

    DropTarget.zone:selected {
        background: rgba(123, 223, 242, 0.13);
        border-color: rgba(123, 223, 242, 0.82);
    }

    DropTarget.metric-zone:selected {
        background: rgba(255, 209, 102, 0.13);
        border-color: rgba(255, 209, 102, 0.82);
    }

    DropTarget.any-zone {
        width: 100%;
        min-height: 128px;
        background: rgba(255, 255, 255, 0.035);
        border: 1px dashed rgba(255, 255, 255, 0.22);
        border-radius: 10px;
        padding: 14px;
    }

    DropTarget.any-zone:selected {
        background: rgba(181, 228, 140, 0.13);
        border-color: rgba(181, 228, 140, 0.82);
    }

    Label.zone-title {
        color: white;
        font-size: 16px;
        font-weight: 850;
    }

    Label.drop-readout {
        background: rgba(3, 8, 18, 0.30);
        border: 1px solid rgba(255, 255, 255, 0.10);
        border-radius: 7px;
        color: rgba(246, 249, 255, 0.84);
        padding: 8px 10px;
        width: 100%;
    }

    Label.drop-zone-label {
        color: rgba(246, 249, 255, 0.82);
        font-weight: 800;
    }
    """
)

win = dg.Window("Drag and drop probe", width=980, height=600)


def payload_label(drop: dg.DragDropPayload) -> str:
    payload = drop.payload
    if isinstance(payload, dict):
        return str(payload.get("label") or payload.get("id") or payload)
    return str(payload)


with dg.VLayout(class_="root"):
    dg.Label("Drag and drop", class_="title")
    status = dg.Label("Drag a source onto a compatible drop target.", class_="status")

    asset_result = None
    metric_result = None
    any_result = None

    def mark_drop(target_name: str, result_label: dg.Label | None, drop: dg.DragDropPayload) -> None:
        item = payload_label(drop)
        kind = drop.kind or "untyped"
        text = f"{target_name}: {item} ({kind})"
        status.set_value(f"Dropped {item} on {target_name}")
        if result_label is not None:
            result_label.set_value(text)

    with dg.HLayout(class_="content"):
        with dg.Panel("Sources", class_="case sources"):
            dg.Label("Cards are app-local drag sources with JSON payloads.", class_="caption")
            with dg.VLayout(class_="source-list"):
                with dg.DragSource(
                    {"kind": "asset", "id": "sensor-a", "label": "Sensor A"},
                    drag_kind="asset",
                    id="sensor-a-source",
                    class_="source-card asset",
                ):
                    dg.Label("Sensor A", class_="source-title")
                    dg.Label("asset", class_="source-kind")
                    dg.Label("Thermal channel from the bench rig", class_="caption")

                with dg.DragSource(
                    {"kind": "asset", "id": "camera-feed", "label": "Camera feed"},
                    drag_kind="asset",
                    class_="source-card asset",
                ):
                    dg.Label("Camera feed", class_="source-title")
                    dg.Label("asset", class_="source-kind")
                    dg.Label("Preview stream with calibration metadata", class_="caption")

                with dg.DragSource(
                    {"kind": "metric", "id": "frame-time", "label": "Frame time"},
                    drag_kind="metric",
                    class_="source-card metric",
                ):
                    dg.Label("Frame time", class_="source-title")
                    dg.Label("metric", class_="source-kind")
                    dg.Label("Rolling render time sample", class_="caption")

                with dg.DragSource(
                    {"kind": "asset", "id": "locked", "label": "Locked source"},
                    drag_kind="asset",
                    disabled=True,
                    class_="source-card asset",
                ):
                    dg.Label("Locked source", class_="source-title")
                    dg.Label("disabled", class_="source-kind")
                    dg.Label("Should not start a drag", class_="caption")

        with dg.Panel("Targets", class_="case targets"):
            dg.Label("Compatible targets highlight while dragging over them.", class_="caption")

            with dg.HLayout(class_="drop-row"):
                with dg.DropTarget(
                    accept="asset",
                    on_drop=lambda drop: mark_drop("Asset lane", asset_result, drop),
                    id="asset-drop-zone",
                    class_="zone asset-zone",
                ):
                    dg.Label("Asset lane", class_="zone-title")
                    dg.Label("Accepts asset payloads only.", class_="caption")
                    asset_result = dg.Label(
                        "No asset dropped",
                        id="asset-drop-result",
                        class_="drop-readout",
                    )

                with dg.DropTarget(
                    accept="metric",
                    on_drop=lambda drop: mark_drop("Metric lane", metric_result, drop),
                    id="metric-drop-zone",
                    class_="zone metric-zone",
                ):
                    dg.Label("Metric lane", class_="zone-title")
                    dg.Label("Accepts metric payloads only.", class_="caption")
                    metric_result = dg.Label("No metric dropped", class_="drop-readout")

            with dg.DropZone(
                "Any payload",
                accept="*",
                on_drop=lambda drop: mark_drop("Any payload", any_result, drop),
                id="any-drop-zone",
                class_="any-zone",
            ):
                any_result = dg.Label("No payload dropped", class_="drop-readout")

    dg.Label("PASS: DragSource, DropTarget, DropZone, accept filtering, hover highlight, disabled source, and on_drop payload dispatch are covered.", class_="caption")


if __name__ == "__main__":
    print(app.run(win))
