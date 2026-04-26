from __future__ import annotations

import json

import pytest

import dragongui as dg
import dragongui.app as app_module


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
