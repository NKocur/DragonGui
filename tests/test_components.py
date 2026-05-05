from __future__ import annotations

import json
import time
from types import SimpleNamespace

import pytest

import dragongui as dg
import dragongui.app as app_module
from dragongui.diagnostics import DiagnosticsSnapshot, ThreadInfo, _reset_collector
from dragongui.runtime import AppHandle
from dragongui.vdom import Patch, diff, retain_old_ids, widget_to_vnode
import dragongui.thread_monitor as thread_monitor_module


def test_component_initial_render_has_live_app_context(monkeypatch) -> None:
    class Sender:
        def __init__(self, handle: object) -> None:
            self.handle = handle
            self.props: list[tuple[str, str, object]] = []

        def enqueue_drain_python_tasks(self) -> None:
            self.handle._drain_python_tasks()

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def close(self) -> None:
            pass

    app = dg.App()
    seen: dict[str, object] = {}
    sender: Sender | None = None

    @dg.component
    def StartupTask(ctx: dg.ComponentCtx) -> dg.Window:
        status = ctx.state("status", "waiting")
        once: list[bool] = ctx.state("once", [False]).value
        seen["ctx_app"] = ctx.app
        if not once[0] and ctx.app is not None:
            once[0] = True
            ctx.app.call_soon_threadsafe(lambda: status.set("ready"))
        win = dg.Window("Startup", key="window")
        dg.Label(str(status.value), id="status", key="status", parent=win)
        return win

    def fake_run_document(document, click_callbacks, change_callbacks, app_handle=None):
        nonlocal sender
        assert app_handle is not None
        assert seen["ctx_app"] is app_handle
        assert document["window"]["children"][0]["props"]["text"] == "waiting"
        sender = Sender(app_handle)
        app_handle._bind_native_sender(sender)
        return {"status": "ok"}

    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: True)
    monkeypatch.setattr(app_module, "run_document", fake_run_document)

    result = app.run(StartupTask())

    assert result == {"status": "ok"}
    assert sender is not None
    assert sender.props == [("status", "text", "ready")]


def test_thread_monitor_starts_during_initial_component_render(monkeypatch) -> None:
    started: dict[str, object] = {}

    def fake_start_monitor(app_handle, snap_slot, history_seconds, refresh_hz, **kwargs) -> None:
        started["app_handle"] = app_handle
        started["snap_slot"] = snap_slot
        started["history_seconds"] = history_seconds
        started["refresh_hz"] = refresh_hz
        started["kwargs"] = kwargs

    @dg.component
    def MonitorTool(ctx: dg.ComponentCtx) -> dg.Window:
        win = dg.Window("Monitor", key="window")
        with dg.VLayout(parent=win):
            dg.ThreadMonitor(
                key="monitor",
                history_seconds=12,
                refresh_hz=3.0,
            )
        return win

    def fake_run_document(document, click_callbacks, change_callbacks, app_handle=None):
        assert app_handle is not None
        assert started["app_handle"] is app_handle
        return {"status": "ok"}

    monkeypatch.setattr(thread_monitor_module, "_start_monitor", fake_start_monitor)
    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: True)
    monkeypatch.setattr(app_module, "run_document", fake_run_document)

    result = dg.App().run(MonitorTool())

    assert result == {"status": "ok"}
    assert started["history_seconds"] == 12
    assert started["refresh_hz"] == 3.0
    assert started["kwargs"]["max_threads"] == 80
    assert started["kwargs"]["max_dead_threads"] == 20
    assert callable(started["kwargs"]["enabled"])


def test_thread_monitor_uses_custom_header_shell() -> None:
    panel = thread_monitor_module._build_panel(
        DiagnosticsSnapshot(),
        show_threads=True,
        show_queue=True,
        show_failures=True,
        id=None,
        class_=None,
        style=None,
    )
    data = panel.to_dict()

    assert data["props"]["title"] == ""
    assert data["style"]["padding"] == 0
    assert [child["type"] for child in data["children"][:3]] == [
        "grid_layout",
        "separator",
        "scroll_area",
    ]
    header = data["children"][0]
    assert header["style"]["flex_grow"] == 0
    assert header["children"][0]["type"] == "led"
    assert header["children"][0]["style"]["align_self"] == "center"
    assert header["children"][0]["style"]["transform"] == {"translate_y": 15}
    assert header["children"][1]["children"][0]["style"]["height"] == 20
    assert header["children"][1]["children"][0]["style"]["line_height"] == "18px"
    assert header["children"][2]["type"] == "tag"
    assert header["children"][2]["style"]["height"] == 18
    assert header["children"][3]["type"] == "button"
    assert header["children"][3]["props"]["text"] == "Disable"
    assert header["children"][3]["style"]["height"] == 24
    body = data["children"][2]["children"][0]
    assert body["style"]["padding"] == 8
    assert body["style"]["flex_grow"] == 0
    assert body["children"][0]["style"]["flex_grow"] == 0


def test_thread_monitor_header_shows_enable_when_paused() -> None:
    panel = thread_monitor_module._build_panel(
        DiagnosticsSnapshot(),
        show_threads=True,
        show_queue=True,
        show_failures=True,
        id=None,
        class_=None,
        style=None,
        monitoring_enabled=False,
    )
    header = panel.to_dict()["children"][0]

    assert header["children"][1]["children"][1]["props"]["text"] == "updates paused"
    assert header["children"][2]["props"]["text"] == "PAUSED"
    assert header["children"][3]["props"]["text"] == "Enable"


def test_thread_monitor_compact_thread_rows_center_leds() -> None:
    snap = DiagnosticsSnapshot()
    snap.thread_total = 1
    snap.thread_alive = 1
    snap.threads = [
        ThreadInfo(
            name="worker",
            ident=123,
            native_id=None,
            alive=True,
            daemon=True,
            role="loader",
            cmd_count=4,
            cmd_per_sec=1.5,
        )
    ]
    panel = thread_monitor_module._build_panel(
        snap,
        show_threads=True,
        show_queue=False,
        show_failures=False,
        id=None,
        class_=None,
        style=None,
    )
    body = panel.to_dict()["children"][2]["children"][0]
    thread_grid = next(
        child
        for child in body["children"]
        if child["type"] == "grid_layout"
        and any(grand.get("props", {}).get("text") == "THREAD / ROLE" for grand in child["children"])
    )

    row_led = thread_grid["children"][5]
    status = thread_grid["children"][6]
    name = thread_grid["children"][8]

    assert row_led["type"] == "led"
    assert row_led["style"]["transform"] == {"translate_y": 5}
    assert status["style"]["height"] == 18
    assert status["style"]["line_height"] == "17px"
    assert name["style"]["height"] == 18


def test_thread_monitor_refresh_diff_uses_stable_widget_keys() -> None:
    old_snap = DiagnosticsSnapshot()
    old_snap.queue_depth = 1
    old_snap.queue_max = 8
    old_snap.queue_avg = 1.0
    old_snap.enqueued_total = 10
    old_snap.enqueue_rate = 1.0
    old_snap.thread_total = 1
    old_snap.thread_alive = 1
    old_snap.threads = [
        ThreadInfo("worker", 101, 201, True, True, "producer", 10, 1.0),
    ]

    new_snap = DiagnosticsSnapshot()
    new_snap.queue_depth = 2
    new_snap.queue_max = 8
    new_snap.queue_avg = 1.5
    new_snap.enqueued_total = 12
    new_snap.enqueue_rate = 2.0
    new_snap.thread_total = 1
    new_snap.thread_alive = 1
    new_snap.threads = [
        ThreadInfo("worker", 101, 201, True, True, "producer", 12, 2.0),
    ]

    old_widget = thread_monitor_module._build_panel(
        old_snap, True, True, True, None, None, None, root_key="monitor"
    )
    old_vnode = widget_to_vnode(old_widget)
    new_widget = thread_monitor_module._build_panel(
        new_snap, True, True, True, None, None, None, root_key="monitor"
    )
    new_vnode = retain_old_ids(old_vnode, widget_to_vnode(new_widget))

    patches = diff(old_vnode, new_vnode)

    assert patches
    assert all(patch.kind == Patch.SET_PROP for patch in patches)
    assert not any(patch.prop == "template_columns" for patch in patches)
    assert not any(
        patch.kind in {Patch.REPLACE_NODE, Patch.REPLACE_CHILDREN}
        for patch in patches
    )


def test_thread_monitor_failure_fingerprint_ages_at_display_granularity() -> None:
    failure = SimpleNamespace(
        ts_ms=0,
        callable_repr="task",
        thread_name="worker",
        thread_role="producer",
        exc_type="RuntimeError",
        exc_msg="failed",
    )
    first = DiagnosticsSnapshot()
    first.ts_ms = 3_700_000
    first.failure_count = 1
    first.recent_failures = [failure]
    second = DiagnosticsSnapshot()
    second.ts_ms = 3_701_000
    second.failure_count = 1
    second.recent_failures = [failure]

    first_fingerprint = thread_monitor_module._snapshot_fingerprint(
        first,
        show_threads=False,
        show_queue=False,
        show_failures=True,
    )
    second_fingerprint = thread_monitor_module._snapshot_fingerprint(
        second,
        show_threads=False,
        show_queue=False,
        show_failures=True,
    )

    assert first_fingerprint == second_fingerprint


def test_thread_monitor_worker_stops_when_component_detaches() -> None:
    _reset_collector()
    handle = AppHandle()

    class Runtime:
        app_handle = handle

    class Slot:
        _runtime = Runtime()

        def __init__(self) -> None:
            self.values: list[object] = []

        def set(self, value: object) -> None:
            self.values.append(value)

    slot = Slot()
    thread = thread_monitor_module._start_monitor(handle, slot, 30, 15.0)
    slot._runtime.app_handle = None
    thread.join(timeout=1.0)
    handle._close()

    assert not thread.is_alive()


def test_thread_monitor_refresh_does_not_record_user_enqueue_metrics() -> None:
    collector = _reset_collector()
    handle = AppHandle()

    class Sender:
        def __init__(self, app_handle: AppHandle) -> None:
            self.app_handle = app_handle

        def queue_depth(self) -> int:
            return 0

        def enqueue_drain_python_tasks(self) -> None:
            self.app_handle._drain_python_tasks()

        def close(self) -> None:
            pass

    class Runtime:
        app_handle = handle

    class Slot:
        _runtime = Runtime()

        def __init__(self) -> None:
            self.count = 0

        def set(self, value: object) -> None:
            self.count += 1

    handle._bind_native_sender(Sender(handle))
    slot = Slot()
    thread = thread_monitor_module._start_monitor(handle, slot, 30, 15.0)
    deadline = time.monotonic() + 1.0
    while slot.count == 0 and time.monotonic() < deadline:
        time.sleep(0.01)
    time.sleep(0.2)
    handle._close()
    thread.join(timeout=1.0)

    snap = collector.snapshot()
    assert slot.count == 1
    assert snap.enqueued_total == 0


def test_thread_monitor_enabled_predicate_pauses_background_refresh() -> None:
    _reset_collector()
    handle = AppHandle()
    enabled = {"value": False}

    class Sender:
        def __init__(self, app_handle: AppHandle) -> None:
            self.app_handle = app_handle

        def queue_depth(self) -> int:
            return 0

        def enqueue_drain_python_tasks(self) -> None:
            self.app_handle._drain_python_tasks()

        def close(self) -> None:
            pass

    class Runtime:
        app_handle = handle

    class Slot:
        _runtime = Runtime()

        def __init__(self) -> None:
            self.count = 0

        def set(self, value: object) -> None:
            self.count += 1

    handle._bind_native_sender(Sender(handle))
    slot = Slot()
    thread = thread_monitor_module._start_monitor(
        handle,
        slot,
        30,
        15.0,
        enabled=lambda: enabled["value"],
    )
    time.sleep(0.18)
    assert slot.count == 0

    enabled["value"] = True
    deadline = time.monotonic() + 1.0
    while slot.count == 0 and time.monotonic() < deadline:
        time.sleep(0.01)
    handle._close()
    thread.join(timeout=1.0)

    assert slot.count == 1
    assert not thread.is_alive()


def test_component_state_rerenders_and_applies_live_prop_patch(monkeypatch) -> None:
    class Sender:
        def __init__(self) -> None:
            self.props: list[tuple[str, str, object]] = []

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def close(self) -> None:
            pass

    sender = Sender()
    calls: dict[str, object] = {}

    @dg.component
    def Counter(ctx: dg.ComponentCtx) -> dg.Window:
        count = ctx.state("count", 0)
        win = dg.Window("Counter", id="counter-window", key="counter-window")
        with dg.Panel("Controls", id="controls", key="controls", parent=win):
            dg.Label(f"Count {count.value}", id="count-label", key="count-label")
            dg.Button(
                "Increment",
                id="increment",
                key="increment",
                on_click=lambda: count.set(int(count.value) + 1),
            )
        return win

    def fake_run_document(document, click_callbacks, change_callbacks, app_handle=None):
        assert app_handle is not None
        app_handle._bind_native_sender(sender)
        calls["initial_text"] = document["window"]["children"][0]["children"][0]["props"]["text"]
        assert click_callbacks == {}
        app_handle._invoke_click_callback("increment")
        app_handle._invoke_click_callback("increment")
        return {"status": "ok"}

    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: True)
    monkeypatch.setattr(app_module, "run_document", fake_run_document)

    result = dg.App().run(Counter())

    assert result == {"status": "ok"}
    assert calls["initial_text"] == "Count 0"
    assert sender.props == [
        ("count-label", "text", "Count 1"),
        ("count-label", "text", "Count 2"),
    ]


def test_component_rerender_retains_generated_ids_for_callbacks(monkeypatch) -> None:
    class Sender:
        def __init__(self) -> None:
            self.props: list[tuple[str, str, object]] = []

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def close(self) -> None:
            pass

    sender = Sender()
    ids: dict[str, str] = {}

    @dg.component
    def Counter(ctx: dg.ComponentCtx) -> dg.Window:
        count = ctx.state("count", 0)
        win = dg.Window("Counter", key="window")
        with dg.Panel("Controls", key="panel", parent=win):
            dg.Label(f"Count {count.value}", key="label")
            dg.Button(
                "Increment",
                key="increment",
                on_click=lambda: count.set(int(count.value) + 1),
            )
        return win

    def fake_run_document(document, click_callbacks, change_callbacks, app_handle=None):
        assert app_handle is not None
        app_handle._bind_native_sender(sender)
        panel = document["window"]["children"][0]
        ids["label"] = panel["children"][0]["id"]
        ids["button"] = panel["children"][1]["id"]

        assert app_handle._invoke_click_callback(ids["button"]) is True
        assert app_handle._invoke_click_callback(ids["button"]) is True
        return {"status": "ok"}

    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: True)
    monkeypatch.setattr(app_module, "run_document", fake_run_document)

    result = dg.App().run(Counter())

    assert result == {"status": "ok"}
    assert sender.props == [
        (ids["label"], "text", "Count 1"),
        (ids["label"], "text", "Count 2"),
    ]


def test_component_rerender_rebinds_live_handles_for_new_widget_objects(monkeypatch) -> None:
    class Sender:
        def __init__(self) -> None:
            self.props: list[tuple[str, str, object]] = []

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def close(self) -> None:
            pass

    sender = Sender()
    ids: dict[str, str] = {}

    @dg.component
    def Tool(ctx: dg.ComponentCtx) -> dg.Window:
        toggled = ctx.state("toggled", False)
        win = dg.Window("Live handles", key="window")
        with dg.Panel("Controls", key="panel", parent=win):
            field = dg.TextInput("initial", key="field")
            dg.Button(
                "Rerender",
                key="rerender",
                on_click=lambda: toggled.set(not bool(toggled.value)),
            )
            dg.Button(
                "Set field",
                key="set-field",
                on_click=lambda: field.set_value(f"value {toggled.value}"),
            )
        return win

    def fake_run_document(document, click_callbacks, change_callbacks, app_handle=None):
        assert app_handle is not None
        app_handle._bind_native_sender(sender)
        panel = document["window"]["children"][0]
        ids["field"] = panel["children"][0]["id"]
        ids["rerender"] = panel["children"][1]["id"]
        ids["set_field"] = panel["children"][2]["id"]

        assert app_handle._invoke_click_callback(ids["rerender"]) is True
        assert app_handle._invoke_click_callback(ids["set_field"]) is True
        return {"status": "ok"}

    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: True)
    monkeypatch.setattr(app_module, "run_document", fake_run_document)

    result = dg.App().run(Tool())

    assert result == {"status": "ok"}
    assert (ids["field"], "value", "value True") in sender.props


def test_component_rerender_keeps_table_resource_id_aligned_with_retained_id(monkeypatch) -> None:
    np = pytest.importorskip("numpy")

    class Frame:
        columns = ("x",)
        dtypes = ("float32",)
        shape = (2, 1)
        x = np.array([1.0, 2.0], dtype=np.float32)

        def __getitem__(self, column: str) -> object:
            return getattr(self, column)

    class Sender:
        def __init__(self) -> None:
            self.table_payloads: list[str] = []

        def enqueue_set_table_data(self, widget_id: str, table_json: str) -> None:
            self.table_payloads.append(table_json)

        def enqueue_set_table_data_columns(
            self,
            widget_id: str,
            table_json: str,
            columns_json: str,
            buffers: list[bytes],
        ) -> None:
            self.table_payloads.append(table_json)

        def close(self) -> None:
            pass

    sender = Sender()
    ids: dict[str, str] = {}

    @dg.component
    def Tool(ctx: dg.ComponentCtx) -> dg.Window:
        tick = ctx.state("tick", 0)
        win = dg.Window("Table", key="window")
        with dg.Panel("Data", key="panel", parent=win):
            dg.DataFrameTable(Frame(), key="table")
            dg.Button("Refresh", key="refresh", on_click=lambda: tick.set(int(tick.value) + 1))
        return win

    def fake_run_document(document, click_callbacks, change_callbacks, app_handle=None):
        assert app_handle is not None
        panel = document["window"]["children"][0]
        ids["table"] = panel["children"][0]["id"]
        ids["refresh"] = panel["children"][1]["id"]
        app_handle._bind_native_sender(sender)

        assert app_handle._invoke_click_callback(ids["refresh"]) is True
        return {"status": "ok"}

    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: True)
    monkeypatch.setattr(app_module, "run_document", fake_run_document)

    result = dg.App().run(Tool())

    assert result == {"status": "ok"}
    assert sender.table_payloads
    assert {
        json_payload["resource_id"]
        for json_payload in (json.loads(payload) for payload in sender.table_payloads)
    } == {f"{ids['table']}:table"}


def test_component_rerender_does_not_reupload_stable_table_resource(monkeypatch) -> None:
    np = pytest.importorskip("numpy")

    class Frame:
        columns = ("x",)
        dtypes = ("float32",)
        shape = (2, 1)
        x = np.array([1.0, 2.0], dtype=np.float32)

        def __getitem__(self, column: str) -> object:
            return getattr(self, column)

    class Sender:
        def __init__(self) -> None:
            self.props: list[tuple[str, str, object]] = []
            self.table_payloads: list[str] = []

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def enqueue_set_table_data(self, widget_id: str, table_json: str) -> None:
            self.table_payloads.append(table_json)

        def enqueue_set_table_data_columns(
            self,
            widget_id: str,
            table_json: str,
            columns_json: str,
            buffers: list[bytes],
        ) -> None:
            self.table_payloads.append(table_json)

        def close(self) -> None:
            pass

    frame = Frame()
    sender = Sender()
    ids: dict[str, str] = {}

    @dg.component
    def Tool(ctx: dg.ComponentCtx) -> dg.Window:
        tick = ctx.state("tick", 0)
        win = dg.Window("Stable Table", key="window")
        with dg.Panel("Data", key="panel", parent=win):
            dg.Label(f"Tick {tick.value}", key="label")
            dg.DataFrameTable(frame, key="table")
            dg.Button("Refresh", key="refresh", on_click=lambda: tick.set(int(tick.value) + 1))
        return win

    def fake_run_document(document, click_callbacks, change_callbacks, app_handle=None):
        assert app_handle is not None
        panel = document["window"]["children"][0]
        ids["label"] = panel["children"][0]["id"]
        ids["refresh"] = panel["children"][2]["id"]
        app_handle._bind_native_sender(sender)
        assert len(sender.table_payloads) == 1

        assert app_handle._invoke_click_callback(ids["refresh"]) is True
        return {"status": "ok"}

    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: True)
    monkeypatch.setattr(app_module, "run_document", fake_run_document)

    result = dg.App().run(Tool())

    assert result == {"status": "ok"}
    assert sender.props == [(ids["label"], "text", "Tick 1")]
    assert len(sender.table_payloads) == 1


def test_component_duplicate_state_keys_raise_clear_error() -> None:
    @dg.component
    def Bad(ctx: dg.ComponentCtx) -> dg.Window:
        ctx.state("value", 1)
        ctx.state("value", 2)
        return dg.Window("Bad")

    with pytest.raises(ValueError, match="duplicate component state key"):
        Bad()._runtime.render_initial()


def test_component_root_must_return_window(monkeypatch) -> None:
    @dg.component
    def NotAWindow(ctx: dg.ComponentCtx) -> dg.Panel:
        return dg.Panel("Panel", parent=None)

    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: False)

    with pytest.raises(TypeError, match="root component"):
        dg.App().run(NotAWindow())


def test_component_state_set_before_live_updates_state_without_native_patch() -> None:
    @dg.component
    def Counter(ctx: dg.ComponentCtx) -> dg.Window:
        count = ctx.state("count", 0)
        win = dg.Window("Counter")
        dg.Button("Increment", on_click=lambda: count.set(int(count.value) + 1), parent=win)
        return win

    root = Counter()
    win = root._runtime.render_initial()
    click = win.children[0]

    assert root._runtime.state["count"] == 0
    click.click()
    assert root._runtime.state["count"] == 1


def test_component_state_set_during_render_raises_clear_error() -> None:
    @dg.component
    def Bad(ctx: dg.ComponentCtx) -> dg.Window:
        value = ctx.state("value", 0)
        value.set(1)
        return dg.Window("Bad")

    with pytest.raises(RuntimeError, match="state cannot be updated during render"):
        Bad()._runtime.render_initial()


def test_nested_component_state_survives_parent_rerender(monkeypatch) -> None:
    class Sender:
        def __init__(self) -> None:
            self.props: list[tuple[str, str, object]] = []

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def close(self) -> None:
            pass

    sender = Sender()

    @dg.component
    def Child(ctx: dg.ComponentCtx, title: str) -> dg.Panel:
        child_count = ctx.state("child_count", 0)
        panel = dg.Panel(title, id="child-panel", key="child-panel", parent=None)
        dg.Label(
            f"Child {child_count.value}",
            id="child-label",
            key="child-label",
            parent=panel,
        )
        dg.Button(
            "Child +1",
            id="child-button",
            key="child-button",
            on_click=lambda: child_count.set(int(child_count.value) + 1),
            parent=panel,
        )
        return panel

    @dg.component
    def Parent(ctx: dg.ComponentCtx) -> dg.Window:
        parent_count = ctx.state("parent_count", 0)
        win = dg.Window("Nested", id="nested-window", key="nested-window")
        with dg.HLayout(id="nested-row", key="nested-row", parent=win):
            with dg.Panel("Parent", id="parent-panel", key="parent-panel"):
                dg.Label(
                    f"Parent {parent_count.value}",
                    id="parent-label",
                    key="parent-label",
                )
                dg.Button(
                    "Parent +1",
                    id="parent-button",
                    key="parent-button",
                    on_click=lambda: parent_count.set(int(parent_count.value) + 1),
                )
            Child("Child", key="child")
        return win

    def fake_run_document(document, click_callbacks, change_callbacks, app_handle=None):
        assert app_handle is not None
        app_handle._bind_native_sender(sender)
        assert click_callbacks == {}
        app_handle._invoke_click_callback("child-button")
        app_handle._invoke_click_callback("parent-button")
        app_handle._invoke_click_callback("child-button")
        return {"status": "ok"}

    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: True)
    monkeypatch.setattr(app_module, "run_document", fake_run_document)

    root = Parent()
    result = dg.App().run(root)

    assert result == {"status": "ok"}
    assert root._runtime.state["parent_count"] == 1
    child_runtime = next(iter(root._runtime.child_runtimes.values()))
    assert child_runtime.state["child_count"] == 2
    assert ("child-label", "text", "Child 2") in sender.props
    assert ("parent-label", "text", "Parent 1") in sender.props


def test_nested_component_calls_require_key() -> None:
    @dg.component
    def Child(ctx: dg.ComponentCtx) -> dg.Panel:
        return dg.Panel("Child", parent=None)

    @dg.component
    def Parent(ctx: dg.ComponentCtx) -> dg.Window:
        win = dg.Window("Nested")
        with dg.HLayout(parent=win):
            Child()
        return win

    with pytest.raises(ValueError, match="nested component calls require"):
        Parent()._runtime.render_initial()


def test_component_rerender_can_introduce_callback_widget(monkeypatch) -> None:
    class Sender:
        def __init__(self) -> None:
            self.children: list[tuple[str, str]] = []

        def enqueue_replace_children(self, widget_id: str, children_json: str) -> None:
            self.children.append((widget_id, children_json))

        def close(self) -> None:
            pass

    sender = Sender()
    calls: list[str] = []

    @dg.component
    def DynamicCallbacks(ctx: dg.ComponentCtx) -> dg.Window:
        expanded = ctx.state("expanded", False)
        win = dg.Window("Dynamic", id="dynamic-window", key="dynamic-window")
        with dg.Panel("Content", id="content", key="content", parent=win):
            dg.Button(
                "Toggle",
                id="toggle",
                key="toggle",
                on_click=lambda: expanded.set(not bool(expanded.value)),
            )
            if expanded.value:
                dg.Button(
                    "Inserted callback",
                    id="inserted",
                    key="inserted",
                    on_click=lambda: calls.append("inserted"),
                )
            else:
                dg.Label("Collapsed", id="placeholder", key="placeholder")
        return win

    def fake_run_document(document, click_callbacks, change_callbacks, app_handle=None):
        assert app_handle is not None
        app_handle._bind_native_sender(sender)
        app_handle._invoke_click_callback("toggle")
        app_handle._invoke_click_callback("inserted")
        return {"status": "ok"}

    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: True)
    monkeypatch.setattr(app_module, "run_document", fake_run_document)

    result = dg.App().run(DynamicCallbacks())

    assert result == {"status": "ok"}
    assert calls == ["inserted"]
    assert sender.children
    assert '"id":"inserted"' in sender.children[0][1]


def test_component_rerender_can_replace_root_node_identity(monkeypatch) -> None:
    class Sender:
        def __init__(self) -> None:
            self.nodes: list[tuple[str, str]] = []

        def enqueue_replace_node(self, widget_id: str, node_json: str) -> None:
            self.nodes.append((widget_id, node_json))

        def close(self) -> None:
            pass

    sender = Sender()

    @dg.component
    def RootSwap(ctx: dg.ComponentCtx) -> dg.Window:
        alternate = ctx.state("alternate", False)
        if alternate.value:
            win = dg.Window("Alternate", id="window-b", key="window-b")
            dg.Button("Swap Back", id="swap-back", key="swap-back", on_click=lambda: alternate.set(False), parent=win)
        else:
            win = dg.Window("Primary", id="window-a", key="window-a")
            dg.Button("Swap", id="swap", key="swap", on_click=lambda: alternate.set(True), parent=win)
        return win

    def fake_run_document(document, click_callbacks, change_callbacks, app_handle=None):
        assert app_handle is not None
        app_handle._bind_native_sender(sender)
        app_handle._invoke_click_callback("swap")
        return {"status": "ok"}

    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: True)
    monkeypatch.setattr(app_module, "run_document", fake_run_document)

    result = dg.App().run(RootSwap())

    assert result == {"status": "ok"}
    assert len(sender.nodes) == 1
    assert sender.nodes[0][0] == "window-a"
    assert '"id":"window-b"' in sender.nodes[0][1]
