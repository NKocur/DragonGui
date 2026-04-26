from __future__ import annotations

import json
import subprocess
import sys
import os
from pathlib import Path

import pytest

import dragongui as dg
import dragongui.app as app_module
import dragongui.dataframe as dataframe_module
import dragongui.widgets as widgets_module
from dragongui.runtime import AppHandle, _collect_runtime_callbacks


class DemoFrame:
    columns = ("x", "y", "z")
    shape = (1_000_000, 3)


def test_declarative_tree_serializes() -> None:
    app = dg.App()
    win = dg.Window("Tool", width=1200, height=800)
    frame = DemoFrame()

    with dg.HLayout():
        with dg.Panel("Controls"):
            col = dg.Dropdown(items=frame.columns)
            dg.Button("Plot")

        scatter = dg.Scatter3D(frame, x="x", y="y", z="z")

    document = app.document(win)

    assert document["window"]["props"]["title"] == "Tool"
    assert document["window"]["children"][0]["type"] == "h_layout"
    assert col.value == "x"
    assert scatter.to_dict()["props"]["frame"]["rows"] == 1_000_000
    assert scatter.to_dict()["props"]["frame"]["dtypes"] == ["", "", ""]


def test_button_callback_can_be_invoked_from_python() -> None:
    calls = []
    button = dg.Button("Plot", on_click=lambda: calls.append("clicked"), parent=None)

    button.click()

    assert calls == ["clicked"]


def test_widget_key_serializes_as_stable_metadata() -> None:
    app = dg.App()
    win = dg.Window("Keys", key="root-window")

    with dg.HLayout(key="main-row"):
        dg.Button("Run", key="run-button")

    document = app.document(win)

    assert document["window"]["key"] == "root-window"
    row = document["window"]["children"][0]
    assert row["key"] == "main-row"
    assert row["children"][0]["key"] == "run-button"

    label = dg.Label("No key", parent=None)
    assert "key" not in label.to_dict()

    with pytest.raises(ValueError, match="non-empty"):
        dg.Button("Bad", key="", parent=None)


def test_widget_style_and_class_serialize_as_v1_metadata() -> None:
    app = dg.App()
    win = dg.Window("Styles")
    button = dg.Button(
        "Run",
        class_="primary-action",
        style={
            "width": 180,
            "background": "surface_alt",
            "border_color": "#33ffaa",
            "border_radius": 9,
            "font_family": "monospace",
            "font_weight": "bold",
            "hover": {"background": "accent_mix_20"},
        },
        parent=win,
    )

    document = app.document(win)
    serialized = document["window"]["children"][0]

    assert serialized["id"] == button.id
    assert serialized["class"] == "primary-action"
    assert serialized["style"]["width"] == 180
    assert serialized["style"]["border_color"] == "#33ffaa"
    assert serialized["style"]["font_family"] == "monospace"
    assert serialized["style"]["font_weight"] == "bold"
    assert serialized["style"]["hover"]["background"] == "accent_mix_20"

    with pytest.raises(ValueError, match="class_"):
        dg.Button("Bad", class_="", parent=None)
    with pytest.raises(TypeError, match="style"):
        dg.Button("Bad", style=["not", "a", "mapping"], parent=None)  # type: ignore[arg-type]


def test_widget_set_style_updates_python_state_and_live_native_style() -> None:
    class Sender:
        def __init__(self) -> None:
            self.styles: list[tuple[str, str]] = []

        def enqueue_set_style(self, widget_id: str, style_json: str) -> None:
            self.styles.append((widget_id, style_json))

        def close(self) -> None:
            pass

    button = dg.Button(
        "Run",
        id="run",
        style={"background": "surface_alt", "border_width": 1},
        parent=None,
    )

    button.set_style({"background": "danger"})
    assert button.to_dict()["style"] == {"background": "danger"}

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    button._bind_live(handle.widget_handle(button.id))

    button.set_style({"background": "accent", "border_radius": 8})

    assert button.style == {"background": "accent", "border_radius": 8}
    assert sender.styles == [("run", '{"background":"accent","border_radius":8}')]

    button.set_style(None)
    assert "style" not in button.to_dict()
    assert sender.styles[-1] == (
        "run",
        '{"background":null,"border_radius":null}',
    )

    with pytest.raises(TypeError, match="style"):
        button.set_style(["bad"])  # type: ignore[arg-type]


def test_container_replace_children_updates_python_tree_and_live_native_children() -> None:
    class Sender:
        def __init__(self) -> None:
            self.children: list[tuple[str, str]] = []

        def enqueue_replace_children(self, widget_id: str, children_json: str) -> None:
            self.children.append((widget_id, children_json))

        def close(self) -> None:
            pass

    panel = dg.Panel("Content", id="panel", parent=None)
    old = dg.Label("Old", id="old", parent=panel)

    panel.replace_children([dg.Label("New", id="new", parent=None)])

    assert old.parent is None
    assert [child.id for child in panel.children] == ["new"]
    assert panel.to_dict()["children"][0]["props"]["text"] == "New"

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    panel._bind_live(handle.widget_handle(panel.id))
    for child in panel.children:
        child._bind_live(handle.widget_handle(child.id))

    panel.replace_children(
        [
            dg.Label("First", id="first", parent=None),
            dg.Spacer(height=8, id="gap", parent=None),
            dg.Label("Second", id="second", parent=None),
        ]
    )

    assert [child.id for child in panel.children] == ["first", "gap", "second"]
    assert panel.children[0].is_live is True
    assert sender.children
    assert '"id":"first"' in sender.children[0][1]
    assert '"type":"spacer"' in sender.children[0][1]

    with pytest.raises(TypeError, match="children"):
        panel.replace_children([object()])  # type: ignore[list-item]


def test_live_replace_children_registers_new_callbacks() -> None:
    class Sender:
        def __init__(self) -> None:
            self.children: list[tuple[str, str]] = []

        def enqueue_replace_children(self, widget_id: str, children_json: str) -> None:
            self.children.append((widget_id, children_json))

        def close(self) -> None:
            pass

    panel = dg.Panel("Content", id="panel", parent=None)
    calls: list[str] = []
    handle = AppHandle()
    handle._bind_native_sender(Sender())
    panel._bind_live(handle.widget_handle(panel.id))

    panel.replace_children(
        [dg.Button("New callback", id="new-button", on_click=lambda: calls.append("clicked"), parent=None)]
    )

    assert handle._invoke_click_callback("new-button") is True
    assert calls == ["clicked"]


def test_live_replace_children_queues_startup_resources_for_new_tables() -> None:
    np = pytest.importorskip("numpy")

    class Frame:
        columns = ("x", "label")
        dtypes = ("float32", "object")
        shape = (2, 2)
        x = np.array([1.0, 2.0], dtype=np.float32)
        label = np.array(["a", "b"], dtype=object)

        def __getitem__(self, column: str) -> object:
            return getattr(self, column)

    class Sender:
        def __init__(self) -> None:
            self.events: list[tuple[str, str, object]] = []

        def enqueue_replace_children(self, widget_id: str, children_json: str) -> None:
            self.events.append(("children", widget_id, children_json))

        def enqueue_set_table_data_columns(
            self,
            widget_id: str,
            table_json: str,
            columns_json: str,
            buffers: list[bytes],
        ) -> None:
            self.events.append(("table_columns", widget_id, (table_json, columns_json, buffers)))

        def close(self) -> None:
            pass

    panel = dg.Panel("Content", id="panel", parent=None)
    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    panel._bind_live(handle.widget_handle(panel.id))

    table = dg.DataFrameTable(Frame(), id="table", parent=None)
    panel.replace_children([table])

    assert [event[0] for event in sender.events] == ["children", "table_columns"]
    assert sender.events[0][1] == "panel"
    assert sender.events[1][1] == "table"
    table_json, columns_json, buffers = sender.events[1][2]
    assert json.loads(table_json)["resource_id"] == "table:table"
    assert json.loads(columns_json) == [
        {"dtype": "f32", "name": "x"},
        {"dtype": "utf8", "name": "label"},
    ]
    assert len(buffers) == 2


def test_app_handle_runtime_change_callbacks_update_widget_state() -> None:
    handle = AppHandle()
    calls: list[tuple[str, object]] = []
    checkbox = dg.Checkbox(
        "Enabled",
        id="check",
        checked=False,
        on_change=lambda value: calls.append(("check", value)),
        parent=None,
    )
    text = dg.TextInput(
        "",
        id="text",
        on_change=lambda value: calls.append(("text", value)),
        parent=None,
    )

    handle.register_widget_callbacks(checkbox)
    handle.register_widget_callbacks(text)

    assert handle._invoke_change_callback("check", True) is True
    assert handle._invoke_change_callback("text", "hello") is True
    assert checkbox.checked is True
    assert text.value == "hello"
    assert calls == [("check", True), ("text", "hello")]

    handle.unregister_widget_callbacks(checkbox)
    assert handle._invoke_change_callback("check", False) is False


def test_app_handle_queues_and_drains_python_tasks() -> None:
    class Sender:
        def __init__(self) -> None:
            self.wake_count = 0
            self.props: list[tuple[str, str, object]] = []
            self.closed = False

        def enqueue_drain_python_tasks(self) -> None:
            self.wake_count += 1

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def close(self) -> None:
            self.closed = True

    handle = AppHandle()
    sender = Sender()
    calls: list[str] = []

    handle.call_soon_threadsafe(lambda: calls.append("before-bind"))
    handle.enqueue_set_prop("field", "value", "queued")
    handle._bind_native_sender(sender)

    assert sender.wake_count == 1
    assert sender.props == [("field", "value", "queued")]
    handle._drain_python_tasks()
    assert calls == ["before-bind"]

    handle.call_soon_threadsafe(lambda: calls.append("after-bind"))
    assert sender.wake_count == 2
    handle._drain_python_tasks()
    assert calls == ["before-bind", "after-bind"]

    handle._close()
    assert sender.closed is True
    with pytest.raises(RuntimeError, match="closed"):
        handle.call_soon_threadsafe(lambda: None)


def test_app_handle_bounds_python_task_drain() -> None:
    handle = AppHandle()
    calls = 0

    def task() -> None:
        nonlocal calls
        calls += 1
        handle.call_soon_threadsafe(task)

    handle.call_soon_threadsafe(task)
    handle._drain_python_tasks()

    assert calls == 100
    snapshot = handle.debug_snapshot()
    assert snapshot["runtime"]["queued_python_tasks"] == 1


def test_app_handle_debug_snapshot_uses_native_sender() -> None:
    class Sender:
        def __init__(self) -> None:
            self.timeout_ms: int | None = None

        def debug_snapshot(self, timeout_ms: int) -> str:
            self.timeout_ms = timeout_ms
            return '{"schema":1,"runtime":{"frames_rendered":3}}'

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    snapshot = handle.debug_snapshot(timeout_ms=250)

    assert sender.timeout_ms == 250
    assert snapshot["schema"] == 1
    assert snapshot["runtime"]["frames_rendered"] == 3


def test_app_debug_snapshot_requires_running_app() -> None:
    with pytest.raises(RuntimeError, match="not running"):
        dg.App().debug_snapshot()


def test_app_handle_generic_buffer_resources_queue_and_release() -> None:
    class Sender:
        def __init__(self) -> None:
            self.buffers: list[tuple[str, str, object, str | None]] = []
            self.released: list[str] = []

        def enqueue_set_buffer_resource(
            self,
            resource_id: str,
            kind: str,
            data: object,
            owner_id: str | None = None,
        ) -> None:
            self.buffers.append((resource_id, kind, data, owner_id))

        def enqueue_release_resource(self, resource_id: str) -> None:
            self.released.append(resource_id)

        def close(self) -> None:
            pass

    handle = AppHandle()
    handle.enqueue_set_buffer_resource("buf-1", bytearray(b"abc"), kind="test")
    handle.release_resource("buf-1")

    sender = Sender()
    handle._bind_native_sender(sender)

    assert len(sender.buffers) == 1
    resource_id, kind, data, owner_id = sender.buffers[0]
    assert resource_id == "buf-1"
    assert kind == "test"
    assert bytes(data) == b"abc"
    assert owner_id is None
    assert sender.released == ["buf-1"]


def test_app_handle_generic_buffer_resources_accept_widget_owner() -> None:
    class Sender:
        def __init__(self) -> None:
            self.buffers: list[tuple[str, str, bytes, str | None]] = []

        def enqueue_set_buffer_resource(
            self,
            resource_id: str,
            kind: str,
            data: object,
            owner_id: str | None = None,
        ) -> None:
            self.buffers.append((resource_id, kind, bytes(data), owner_id))

        def close(self) -> None:
            pass

    owner = dg.Label("Owner", id="owner", parent=None)
    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    handle.enqueue_set_buffer_resource("buf-1", b"abc", kind="test", owner=owner)

    assert sender.buffers == [("buf-1", "test", b"abc", "owner")]


def test_app_handle_reports_sender_closed_after_lock_release_race() -> None:
    class Sender:
        def __init__(self) -> None:
            self.closed = False

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            raise RuntimeError("native sender is closed")

        def is_closed(self) -> bool:
            return self.closed

        def close(self) -> None:
            self.closed = True

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    sender.close()

    with pytest.raises(RuntimeError, match="app handle is closed"):
        handle.enqueue_set_prop("field", "value", "new")


def test_app_buffer_resource_methods_require_running_app() -> None:
    app = dg.App()

    with pytest.raises(RuntimeError, match="not running"):
        app.set_buffer_resource("buf", b"data")
    with pytest.raises(RuntimeError, match="not running"):
        app.release_resource("buf")


def test_live_widget_setter_survives_unbind_during_closed_check() -> None:
    field = dg.TextInput("old", parent=None)
    calls: list[tuple[str, object]] = []

    class RaceHandle:
        def __init__(self) -> None:
            self.id = field.id

        @property
        def closed(self) -> bool:
            field._unbind_live()
            return False

        def enqueue_set_prop(self, prop: str, value: object) -> None:
            calls.append((prop, value))

    field._bind_live(RaceHandle())

    field.set_value("new")

    assert field.value == "new"
    assert calls == [("value", "new")]
    assert field.is_live is False


def test_live_widget_setters_enqueue_native_props() -> None:
    class Sender:
        def __init__(self) -> None:
            self.props: list[tuple[str, str, object]] = []
            self.scatter_payloads: list[tuple[str, bytes, float | None, float | None, str]] = []

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def enqueue_set_scatter_points_packed(
            self,
            widget_id: str,
            xyz: bytes,
            pack_ms: float | None = None,
            enqueue_epoch_ms: float | None = None,
            colormap: str = "viridis",
        ) -> None:
            self.scatter_payloads.append((widget_id, xyz, pack_ms, enqueue_epoch_ms, colormap))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    text = dg.TextInput("old", id="text", parent=None)
    slider = dg.Slider(0.0, min=0, max=1, id="slider", parent=None)
    dropdown = dg.Dropdown(["x", "y"], id="dropdown", parent=None)
    checkbox = dg.Checkbox("Enabled", id="checkbox", parent=None)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="scatter", parent=None)
    for widget in (text, slider, dropdown, checkbox, scatter):
        widget._bind_live(handle.widget_handle(widget.id))

    text.set_value("new")
    slider.set_value(2.0)
    dropdown.set_value("y")
    checkbox.set_checked(True)
    monkey_payload = bytes(range(12))
    original_pack = widgets_module._pack_xyz_bytes
    widgets_module._pack_xyz_bytes = lambda frame, x, y, z: monkey_payload
    try:
        scatter.set_points(DemoFrame(), x="x", y="y", z="z")
    finally:
        widgets_module._pack_xyz_bytes = original_pack

    assert text.value == "new"
    assert slider.value == 1.0
    assert dropdown.value == "y"
    assert checkbox.checked is True
    assert sender.props == [
        ("text", "value", "new"),
        ("slider", "value", 1.0),
        ("dropdown", "value", "y"),
        ("checkbox", "checked", True),
    ]
    assert len(sender.scatter_payloads) == 1
    widget_id, payload, pack_ms, enqueue_epoch_ms, colormap = sender.scatter_payloads[0]
    assert widget_id == "scatter"
    assert payload == monkey_payload
    assert pack_ms is not None and pack_ms >= 0.0
    assert enqueue_epoch_ms is not None and enqueue_epoch_ms > 0.0
    assert colormap == "viridis"


def test_scatter_colormap_serializes_and_live_update_reuploads_points(monkeypatch) -> None:
    class Sender:
        def __init__(self) -> None:
            self.scatter_payloads: list[tuple[str, bytes, str]] = []

        def enqueue_set_scatter_points_packed(
            self,
            widget_id: str,
            xyz: bytes,
            pack_ms: float | None = None,
            enqueue_epoch_ms: float | None = None,
            colormap: str = "viridis",
        ) -> None:
            self.scatter_payloads.append((widget_id, xyz, colormap))

        def close(self) -> None:
            pass

    monkey_payload = bytes(range(12))
    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda frame, x, y, z: monkey_payload)

    scatter = dg.Scatter3D(
        DemoFrame(),
        x="x",
        y="y",
        z="z",
        colormap="Magma",
        id="scatter",
        parent=None,
    )
    assert scatter.props()["colormap"] == "magma"

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.set_colormap("Plasma")

    assert scatter.colormap == "plasma"
    assert sender.scatter_payloads == [("scatter", monkey_payload, "plasma")]


def test_scatter_live_set_points_requires_packable_data(monkeypatch) -> None:
    handle = AppHandle()
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="scatter", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda frame, x, y, z: None)

    with pytest.raises(RuntimeError, match="live Scatter3D"):
        scatter.set_points(DemoFrame(), x="x", y="y", z="z")


def test_dataframe_table_live_set_frame_enqueues_bounded_table_update() -> None:
    class Sender:
        def __init__(self) -> None:
            self.tables: list[tuple[str, str]] = []
            self.table_columns: list[tuple[str, str, str, list[bytes]]] = []

        def enqueue_set_table_data(self, widget_id: str, table_json: str) -> None:
            self.tables.append((widget_id, table_json))

        def enqueue_set_table_data_columns(
            self,
            widget_id: str,
            table_json: str,
            columns_json: str,
            buffers: list[bytes],
        ) -> None:
            self.table_columns.append((widget_id, table_json, columns_json, buffers))

        def close(self) -> None:
            pass

    class Frame:
        columns = ("x", "y")
        dtypes = ("int", "int")
        shape = (3, 2)
        x = [1, 2, 3]
        y = [4, 5, 6]

        def __getitem__(self, column: str) -> object:
            return getattr(self, column)

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    table = dg.DataFrameTable(Frame(), id="table", page_size=10, sample_rows=2, parent=None)
    table._bind_live(handle.widget_handle(table.id))

    table.set_frame(Frame(), sample_rows=1)

    assert len(sender.table_columns) == 1
    widget_id, table_json, columns_json, buffers = sender.table_columns[0]
    payload = json.loads(table_json)
    columns = json.loads(columns_json)
    assert widget_id == "table"
    assert payload["frame"]["columns"] == ["x", "y"]
    assert payload["frame"]["rows"] == 3
    assert payload["resource_id"] == "table:table"
    assert payload["sample_rows"] == 1
    assert payload["buffer_columns"] == 2
    assert payload["cells"] == [["1", "4"]]
    assert columns == [{"dtype": "i64", "name": "x"}, {"dtype": "i64", "name": "y"}]
    assert len(buffers) == 2
    assert all(len(buffer) == 3 * 8 for buffer in buffers)


def test_dataframe_table_column_buffers_pack_supported_numeric_columns() -> None:
    np = pytest.importorskip("numpy")

    class Frame:
        columns = ("x", "y", "label")
        dtypes = ("float32", "int64", "object")
        shape = (3, 3)
        x = np.array([1.0, 2.5, 3.75], dtype=np.float32)
        y = np.array([10, 20, 30], dtype=np.int64)
        label = np.array(["a", "b", "c"], dtype=object)

        def __getitem__(self, column: str) -> object:
            return getattr(self, column)

    summary = dataframe_module.summarize_frame(Frame())
    buffers = dataframe_module.extract_table_column_buffers(Frame(), summary)

    assert [(buffer["name"], buffer["dtype"]) for buffer in buffers] == [
        ("x", "f32"),
        ("y", "i64"),
        ("label", "utf8"),
    ]
    assert len(buffers[0]["data"]) == 3 * 4
    assert len(buffers[1]["data"]) == 3 * 8
    assert len(buffers[2]["data"]) > 8 + (3 + 1) * 8


def test_scatter_xyz_raw_pack_has_expected_size() -> None:
    np = pytest.importorskip("numpy")

    class NumericFrame:
        x = np.array([1.0, 2.0, 3.0], dtype=np.float32)
        y = np.array([4.0, 5.0, 6.0], dtype=np.float32)
        z = np.array([7.0, 8.0, 9.0], dtype=np.float32)

    payload = widgets_module._pack_xyz_bytes(NumericFrame(), "x", "y", "z")

    assert payload is not None
    assert len(payload) == 3 * 12


def test_app_run_binds_live_handles_only_for_native_event_loop(monkeypatch) -> None:
    app = dg.App()
    win = dg.Window("Live")
    button = dg.Button("Run", parent=win)
    seen: dict[str, object] = {}

    def fake_run_document(document, click_callbacks, change_callbacks, app_handle=None):
        seen["document"] = document
        seen["app_handle"] = app_handle
        assert app_handle is app._handle
        assert button.is_live
        assert button._live_handle.id == button.id
        return {"status": "ok"}

    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: True)
    monkeypatch.setattr(app_module, "run_document", fake_run_document)

    result = app.run(win)

    assert result == {"status": "ok"}
    assert seen["app_handle"] is not None
    assert app._handle is None
    assert button.is_live is False


def test_app_run_queues_startup_table_column_resources(monkeypatch) -> None:
    np = pytest.importorskip("numpy")

    class Frame:
        columns = ("x", "label")
        dtypes = ("float32", "object")
        shape = (3, 2)
        x = np.array([1.0, 2.0, 3.0], dtype=np.float32)
        label = np.array(["a", "b", "c"], dtype=object)

        def __getitem__(self, column: str) -> object:
            return getattr(self, column)

    class Sender:
        def __init__(self) -> None:
            self.table_columns: list[tuple[str, str, str, list[bytes]]] = []

        def enqueue_set_table_data_columns(
            self,
            widget_id: str,
            table_json: str,
            columns_json: str,
            buffers: list[bytes],
        ) -> None:
            self.table_columns.append((widget_id, table_json, columns_json, buffers))

        def close(self) -> None:
            pass

    sender = Sender()
    app = dg.App()
    win = dg.Window("Startup resources")
    table = dg.DataFrameTable(Frame(), id="table", parent=win)

    def fake_run_document(document, click_callbacks, change_callbacks, app_handle=None):
        assert app_handle is app._handle
        assert table.is_live
        assert sender.table_columns == []
        app_handle._bind_native_sender(sender)
        return {"status": "ok"}

    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: True)
    monkeypatch.setattr(app_module, "run_document", fake_run_document)

    result = app.run(win)

    assert result == {"status": "ok"}
    assert len(sender.table_columns) == 1
    widget_id, table_json, columns_json, buffers = sender.table_columns[0]
    assert widget_id == "table"
    assert json.loads(table_json)["resource_id"] == "table:table"
    assert json.loads(columns_json) == [
        {"dtype": "f32", "name": "x"},
        {"dtype": "utf8", "name": "label"},
    ]
    assert len(buffers) == 2


def test_app_run_skips_live_handles_for_dev_fallback(monkeypatch) -> None:
    app = dg.App()
    win = dg.Window("Fallback")
    button = dg.Button("Run", parent=win)

    def fake_run_document(document, click_callbacks, change_callbacks, app_handle=None):
        assert app_handle is None
        assert button.is_live is False
        return {"status": "ok", "renderer": "dev-fallback"}

    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: False)
    monkeypatch.setattr(app_module, "run_document", fake_run_document)

    result = app.run(win)

    assert result["renderer"] == "dev-fallback"
    assert app._handle is None
    assert button.is_live is False
    with pytest.raises(RuntimeError, match="not running"):
        app.call_soon_threadsafe(lambda: None)


def test_m5_theme_and_mutable_control_props_serialize() -> None:
    app = dg.App(theme=dg.Theme.light(accent="#0055ff", spacing=10.0, font_size=14.0))
    win = dg.Window("Controls")
    text = dg.TextInput("abc", placeholder="Name", parent=win)
    dropdown = dg.Dropdown(["x", "y"], value="y", disabled=True, parent=win)
    slider = dg.Slider(0.5, min=0, max=1, step=0.25, parent=win)

    document = app.document(win)

    assert document["theme"]["background"] == "#f6f7fb"
    assert document["theme"]["accent"] == "#0055ff"
    assert document["theme"]["spacing"] == 10.0
    assert document["theme"]["font_size"] == 14.0
    assert text.to_dict()["props"]["placeholder"] == "Name"
    assert dropdown.to_dict()["props"]["disabled"] is True
    assert slider.to_dict()["props"]["step"] == 0.25


def test_change_callback_wrappers_update_python_handles() -> None:
    calls = []
    win = dg.Window("State")
    checkbox = dg.Checkbox(
        "Enabled",
        checked=False,
        on_change=lambda v: calls.append(("check", v)),
        parent=win,
    )
    slider = dg.Slider(
        0.0,
        on_change=lambda v: calls.append(("slider", v)),
        parent=win,
    )
    dropdown = dg.Dropdown(
        ["x", "y"],
        on_change=lambda v: calls.append(("drop", v)),
        parent=win,
    )
    text = dg.TextInput(
        "",
        on_change=lambda v: calls.append(("text", v)),
        parent=win,
    )

    _, change_cbs = _collect_runtime_callbacks(win)

    change_cbs[checkbox.id](True)
    change_cbs[slider.id](0.75)
    change_cbs[dropdown.id]("y")
    change_cbs[text.id]("hello")

    assert checkbox.checked is True
    assert slider.value == 0.75
    assert dropdown.value == "y"
    assert text.value == "hello"
    assert calls == [
        ("check", True),
        ("slider", 0.75),
        ("drop", "y"),
        ("text", "hello"),
    ]


def test_change_callbacks_only_registered_when_requested() -> None:
    win = dg.Window("No callbacks")
    dg.Checkbox("Enabled", checked=False, parent=win)
    dg.Slider(0.0, parent=win)
    dg.Dropdown(["x", "y"], parent=win)
    dg.TextInput("", parent=win)

    _, change_cbs = _collect_runtime_callbacks(win)

    assert change_cbs == {}


def test_widget_validation_prevents_state_drift() -> None:
    slider = dg.Slider(10, min=0, max=5, parent=None)
    assert slider.value == 5
    assert slider.to_dict()["props"]["value"] == 5

    with pytest.raises(ValueError, match="max"):
        dg.Slider(0, min=5, max=0, parent=None)
    with pytest.raises(ValueError, match="step"):
        dg.Slider(0, step=0, parent=None)
    with pytest.raises(ValueError, match="cannot be empty"):
        dg.Dropdown([], parent=None)
    with pytest.raises(ValueError, match="one of its items"):
        dg.Dropdown(["x", "y"], value="z", parent=None)


def test_window_cannot_be_created_inside_layout_context() -> None:
    dg.Window("Outer")
    with dg.HLayout():
        with pytest.raises(RuntimeError, match="layout context"):
            dg.Window("Inner")


def test_navigation_widgets_serialize_and_register_callbacks() -> None:
    calls = []
    app = dg.App()
    win = dg.Window("Navigation")

    with dg.HLayout():
        with dg.Sidebar(title="Navigation", width=180):
            dg.NavItem("Scatter", page="scatter")
            dg.NavItem("Table", page="table")

        with dg.Pages(value="scatter", on_change=lambda value: calls.append(("page", value))):
            with dg.Page("scatter", title="Scatter"):
                dg.Label("Scatter page")
            with dg.Page("table"):
                dg.Label("Table page")

    with dg.Tabs(value="table", on_change=lambda value: calls.append(("tab", value))):
        with dg.Tab("Scatter", value="scatter"):
            dg.Label("Scatter tab")
        with dg.Tab("Table", value="table"):
            dg.Label("Table tab")

    document = app.document(win)
    row = document["window"]["children"][0]
    sidebar = row["children"][0]
    pages = row["children"][1]
    tabs = document["window"]["children"][1]

    assert sidebar["type"] == "sidebar"
    assert sidebar["props"]["title"] == "Navigation"
    assert sidebar["props"]["width"] == 180
    assert sidebar["children"][0]["type"] == "nav_item"
    assert sidebar["children"][0]["props"]["page"] == "scatter"
    assert pages["type"] == "pages"
    assert pages["props"]["value"] == "scatter"
    assert pages["children"][0]["type"] == "page"
    assert pages["children"][0]["props"]["title"] == "Scatter"
    assert tabs["type"] == "tabs"
    assert tabs["props"]["value"] == "table"
    assert tabs["children"][0]["type"] == "tab"
    assert tabs["children"][0]["props"]["label"] == "Scatter"

    _, change_cbs = _collect_runtime_callbacks(win)
    change_cbs[pages["id"]]("table")
    change_cbs[tabs["id"]]("scatter")

    assert calls == [("page", "table"), ("tab", "scatter")]


def test_navigation_validation() -> None:
    dg.Window("Navigation validation")

    with pytest.raises(RuntimeError, match="Tabs context"):
        dg.Tab("Orphan")

    with pytest.raises(RuntimeError, match="Pages context"):
        dg.Page("orphan")

    with dg.Tabs():
        dg.Tab("One", value="same")
        with pytest.raises(ValueError, match="duplicate Tab"):
            dg.Tab("Two", value="same")

    with dg.Pages():
        dg.Page("same")
        with pytest.raises(ValueError, match="duplicate Page"):
            dg.Page("same")

    with pytest.raises(ValueError, match="page cannot be empty"):
        dg.NavItem("Bad", page="", parent=None)

    with pytest.raises(ValueError, match="Sidebar width"):
        dg.Sidebar(width=0, parent=None)


def test_w0_layout_widgets_serialize_and_validate() -> None:
    app = dg.App()
    win = dg.Window("W0")

    with dg.VLayout():
        dg.Label("Top")
        dg.Separator()
        dg.Spacer(height=12)
        with dg.HLayout():
            dg.Label("Left")
            dg.Separator(orientation="vertical")
            dg.Spacer(width=16)
            dg.Label("Right")
        with dg.StatusBar(height=30):
            dg.Label("Ready")
            dg.Spacer()
            dg.Label("1,000 rows")

    document = app.document(win)
    root = document["window"]["children"][0]
    sep = root["children"][1]
    spacer = root["children"][2]
    row = root["children"][3]
    status = root["children"][4]

    assert sep["type"] == "separator"
    assert sep["props"]["orientation"] == "auto"
    assert spacer["type"] == "spacer"
    assert spacer["props"]["height"] == 12.0
    assert row["children"][1]["props"]["orientation"] == "vertical"
    assert row["children"][2]["props"]["width"] == 16.0
    assert status["type"] == "status_bar"
    assert status["props"]["height"] == 30.0
    assert status["children"][1]["type"] == "spacer"

    with pytest.raises(ValueError, match="orientation"):
        dg.Separator(orientation="diagonal", parent=None)
    with pytest.raises(ValueError, match="width"):
        dg.Spacer(width=-1, parent=None)
    with pytest.raises(ValueError, match="height"):
        dg.Spacer(height=-1, parent=None)
    with pytest.raises(ValueError, match="StatusBar height"):
        dg.StatusBar(height=0, parent=None)


def test_dataframe_table_serializes_metadata_and_bounded_cell_sample() -> None:
    class TypedFrame:
        columns = ("a", "b", "c")
        dtypes = ("int64", "float32", "str")
        shape = (5, 3)
        a = [1, 2, 3, 4, 5]
        b = [1.25, 2.5, 3.75, 4.0, 5.125]
        c = ["one", "two", "three", "four", "five"]

    table = dg.DataFrameTable(TypedFrame(), page_size=64, sample_rows=3, parent=None)
    props = table.to_dict()["props"]

    assert props["virtualized"] is True
    assert props["resource_id"] == table.resource_id
    assert props["resource_id"] == f"{table.id}:table"
    assert props["page_size"] == 64
    assert props["sample_rows"] == 3
    assert props["frame"]["columns"] == ["a", "b", "c"]
    assert props["frame"]["dtypes"] == ["int64", "float32", "str"]
    assert props["frame"]["rows"] == 5
    assert props["cells"] == [
        ["1", "1.25", "one"],
        ["2", "2.5", "two"],
        ["3", "3.75", "three"],
    ]


def test_dataframe_table_metadata_only_frame_keeps_empty_cell_sample() -> None:
    class MetadataOnlyFrame:
        columns = ("a", "b", "c")
        dtypes = ("int64", "float32", "str")
        shape = (1_000_000, 3)

    table = dg.DataFrameTable(MetadataOnlyFrame(), page_size=64, parent=None)
    props = table.to_dict()["props"]

    assert props["frame"]["rows"] == 1_000_000
    assert props["cells"] == []


def test_dataframe_table_validation() -> None:
    with pytest.raises(ValueError, match="page_size"):
        dg.DataFrameTable(DemoFrame(), page_size=0, parent=None)
    with pytest.raises(ValueError, match="sample_rows"):
        dg.DataFrameTable(DemoFrame(), sample_rows=-1, parent=None)


def test_backend_info_has_python_fallback() -> None:
    info = dg.backend_info()

    assert info["name"] == "dragongui"
    assert "native" in info


def test_dev_fallback_allows_run_without_native_backend(monkeypatch) -> None:
    monkeypatch.setenv("DRAGONGUI_DEV_FALLBACK", "1")
    app = dg.App()
    win = dg.Window("Dev")

    result = app.run(win)

    assert result["status"] == "ok"
    assert result["renderer"] in {"dev-fallback", "wgpu"}


def test_scatter_example_runs_from_source_tree() -> None:
    root = Path(__file__).resolve().parents[1]
    env = os.environ.copy()
    env["DRAGONGUI_SMOKE_FRAMES"] = "3"
    result = subprocess.run(
        [sys.executable, str(root / "examples" / "scatter_tool.py")],
        check=True,
        capture_output=True,
        text=True,
        env=env,
        timeout=30,
    )

    assert (
        "DragonGUI source import works." in result.stdout
        or "DragonGUI dev fallback is active." in result.stdout
        or result.stdout == ""
    )
