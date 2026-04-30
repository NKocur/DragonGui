from __future__ import annotations

import json
import threading
import subprocess
import sys
import os
from pathlib import Path

import pytest

import dragongui as dg
import dragongui.app as app_module
import dragongui.dataframe as dataframe_module
import dragongui.dialogs as dialogs_module
import dragongui.widgets as widgets_module
from dragongui.runtime import AppHandle, _collect_runtime_callbacks, _set_active_app_handle


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


def test_collapsible_serializes_and_validates() -> None:
    win = dg.Window("Collapsible")
    calls: list[bool] = []
    with dg.Collapsible(
        "Advanced",
        expanded=False,
        on_change=lambda expanded: calls.append(expanded),
        id="advanced",
        style={"parts": {"header": {"background": "surface_alt"}}},
    ):
        dg.Checkbox("Normalize")

    serialized = win.to_dict()["children"][0]

    assert serialized["type"] == "collapsible"
    assert serialized["props"]["title"] == "Advanced"
    assert serialized["props"]["expanded"] is False
    assert serialized["props"]["events"] == ["change"]
    assert serialized["children"][0]["type"] == "checkbox"

    with pytest.raises(ValueError, match="title"):
        dg.Collapsible("", parent=None)
    with pytest.raises(ValueError, match="no CSS part"):
        dg.Collapsible("Bad", style={"parts": {"accent": {}}}, parent=None)


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
    with pytest.raises(ValueError, match="Button has no CSS part 'stepper'"):
        dg.Button("Bad", style={"parts": {"stepper": {"width": 32}}}, parent=None)

    number = dg.NumberInput(
        4,
        style={"parts": {"stepper_up": {"background": "surface_alt"}}},
        parent=None,
    )
    assert number.to_dict()["style"]["parts"]["stepper_up"]["background"] == "surface_alt"


def test_inline_part_style_catalog_serializes_for_supported_widgets() -> None:
    class TypedFrame:
        columns = ("a", "b")
        dtypes = ("int64", "str")
        shape = (2, 2)
        a = [1, 2]
        b = ["one", "two"]

    cases = [
        (
            lambda style: dg.HLayout(style=style, parent=None),
            ("scrollbar_track", "scrollbar_thumb"),
        ),
        (
            lambda style: dg.VLayout(style=style, parent=None),
            ("scrollbar_track", "scrollbar_thumb"),
        ),
        (
            lambda style: dg.Pages(style=style, parent=None),
            ("scrollbar_track", "scrollbar_thumb"),
        ),
        (
            lambda style: dg.Page("overview", style=style, parent=None),
            ("scrollbar_track", "scrollbar_thumb"),
        ),
        (
            lambda style: dg.Sidebar(style=style, parent=None),
            ("scrollbar_track", "scrollbar_thumb"),
        ),
        (
            lambda style: dg.Panel("Panel", style=style, parent=None),
            ("accent", "scrollbar_track", "scrollbar_thumb"),
        ),
        (
            lambda style: dg.Collapsible("Advanced", style=style, parent=None),
            (
                "header",
                "indicator",
                "body",
                "scrollbar_track",
                "scrollbar_thumb",
            ),
        ),
        (
            lambda style: dg.Modal("Details", style=style, parent=None),
            ("scrim", "scrollbar_track", "scrollbar_thumb"),
        ),
        (
            lambda style: dg.Button("Filters", style=style, parent=None),
            ("badge",),
        ),
        (
            lambda style: dg.NumberInput(4, style=style, parent=None),
            ("field", "stepper", "stepper_up", "stepper_down", "stepper_divider", "divider", "caret"),
        ),
        (
            lambda style: dg.Dropdown(("A", "B"), style=style, parent=None),
            ("field", "chevron", "menu", "item", "item_selected", "item_hover"),
        ),
        (
            lambda style: dg.Checkbox("Enabled", style=style, parent=None),
            ("row", "box", "indicator", "label"),
        ),
        (
            lambda style: dg.Slider(0.5, style=style, parent=None),
            ("track", "fill", "thumb"),
        ),
        (
            lambda style: dg.ProgressBar(0.5, style=style, parent=None),
            ("track", "fill", "label"),
        ),
        (
            lambda style: dg.Tabs(style=style, parent=None),
            ("header",),
        ),
        (
            lambda style: dg.Tab("One", style=style, parent=None),
            ("tab", "accent", "badge"),
        ),
        (
            lambda style: dg.NavItem("One", page="one", style=style, parent=None),
            ("item", "accent", "badge"),
        ),
        (
            lambda style: dg.DataFrameTable(TypedFrame(), style=style, parent=None),
            ("header", "row", "row_selected", "grid_line"),
        ),
    ]

    for factory, parts in cases:
        style = {"parts": {part: {"background": "surface_alt"} for part in parts}}
        widget = factory(style)

        assert widget.to_dict()["style"]["parts"] == style["parts"]


def test_app_stylesheet_serializes_startup_stylesheets() -> None:
    app = dg.App()
    app.stylesheet("Button { border-radius: 4px; }")
    app.stylesheet(".primary { background: accent; }")
    app.stylesheet("NumberInput::stepper-up { background: surface_alt; }")
    win = dg.Window("CSS")

    document = app.document(win)

    assert document["stylesheets"] == [
        {"origin": "user", "source": "Button { border-radius: 4px; }"},
        {"origin": "user", "source": ".primary { background: accent; }"},
        {
            "origin": "user",
            "source": "NumberInput::stepper-up { background: surface_alt; }",
        },
    ]

    with pytest.raises(TypeError, match="css"):
        app.stylesheet(123)  # type: ignore[arg-type]
    with pytest.raises(ValueError, match="non-empty"):
        app.stylesheet("  ")

    app.clear_stylesheets()
    assert "stylesheets" not in app.document(win)


def test_app_load_stylesheet_and_live_stylesheet_updates() -> None:
    class Sender:
        def __init__(self) -> None:
            self.stylesheets: list[tuple[str, str]] = []
            self.cleared: list[str] = []

        def enqueue_set_stylesheet(self, origin: str, css: str) -> None:
            self.stylesheets.append((origin, css))

        def enqueue_clear_stylesheets(self, origin: str) -> None:
            self.cleared.append(origin)

        def close(self) -> None:
            pass

    css_path = Path(".test-cache") / "app_stylesheet_test.dg.css"
    css_path.parent.mkdir(exist_ok=True)
    css_path.write_text("Button { border-radius: 4px; }", encoding="utf-8")
    app = dg.App()
    try:
        app.load_stylesheet(css_path)
    finally:
        css_path.unlink(missing_ok=True)

    assert app.document(dg.Window("CSS"))["stylesheets"] == [
        {"origin": "user", "source": "Button { border-radius: 4px; }"}
    ]

    sender = Sender()
    app._handle = AppHandle()
    app._handle._bind_native_sender(sender)
    try:
        app.stylesheet("NumberInput::stepper { width: 34px; }")
        app.clear_stylesheets()
    finally:
        app._handle._close()
        app._handle = None

    assert sender.stylesheets == [("user", "NumberInput::stepper { width: 34px; }")]
    assert sender.cleared == ["user"]


def test_widget_tooltip_serializes_as_common_prop() -> None:
    app = dg.App()
    win = dg.Window("Tooltips")

    label = dg.Label("Hover me", tooltip="Helpful detail", parent=win)
    button = dg.Button("Run", tooltip="Starts the operation", parent=win)

    children = app.document(win)["window"]["children"]

    assert children[0]["id"] == label.id
    assert children[0]["props"]["tooltip"] == "Helpful detail"
    assert children[1]["id"] == button.id
    assert children[1]["props"]["tooltip"] == "Starts the operation"


def test_rich_tooltip_serializes_target_and_children() -> None:
    app = dg.App()
    win = dg.Window("Rich tooltip")
    button = dg.Button("Inspect", parent=win)
    with dg.Tooltip(target=button, width=260, height=120, parent=win) as tooltip:
        dg.Label("Rows: 1,240")
        dg.ProgressBar(0.72)

    serialized = app.document(win)["window"]["children"]

    assert serialized[1]["id"] == tooltip.id
    assert serialized[1]["type"] == "tooltip"
    assert serialized[1]["props"]["target"] == button.id
    assert serialized[1]["props"]["width"] == 260.0
    assert serialized[1]["props"]["height"] == 120.0
    assert [child["type"] for child in serialized[1]["children"]] == ["label", "progress_bar"]

    with pytest.raises(ValueError, match="target"):
        dg.Tooltip(target="", parent=None)
    with pytest.raises(ValueError, match="width"):
        dg.Tooltip(target=button, width=0, parent=None)


def test_modal_serializes_and_live_open_updates() -> None:
    class Sender:
        def __init__(self) -> None:
            self.props: list[tuple[str, str, object]] = []

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def close(self) -> None:
            pass

    app = dg.App()
    win = dg.Window("Modal")
    modal = dg.Modal("Confirm", open=False, width=480, height=240, parent=win)
    dg.Label("Continue?", parent=modal)
    dg.Button("OK", parent=modal)

    serialized = app.document(win)["window"]["children"][0]
    assert serialized["type"] == "modal"
    assert serialized["props"]["title"] == "Confirm"
    assert serialized["props"]["open"] is False
    assert serialized["props"]["width"] == 480.0
    assert serialized["children"][0]["type"] == "label"

    sender = Sender()
    handle = AppHandle()
    handle._bind_native_sender(sender)
    modal._bind_live(handle.widget_handle(modal.id))

    modal.set_open(True)
    modal.close()
    modal.show()
    modal.close()

    assert sender.props == [
        (modal.id, "open", True),
        (modal.id, "open", False),
        (modal.id, "open", True),
        (modal.id, "open", False),
    ]


def test_alert_and_confirm_build_modal_helpers() -> None:
    app = dg.App()
    win = dg.Window("Helpers")

    alert = dg.alert("Notice", "Saved", open=True, parent=win)
    confirm = dg.confirm("Delete", "Remove rows?", open=False, parent=win)

    children = app.document(win)["window"]["children"]
    assert children[0]["id"] == alert.id
    assert children[0]["type"] == "modal"
    assert children[0]["props"]["open"] is True
    assert children[0]["children"][-1]["props"]["text"] == "OK"
    assert children[1]["id"] == confirm.id
    assert children[1]["props"]["open"] is False
    assert children[1]["children"][-1]["children"][-1]["props"]["text"] == "Confirm"


def test_file_dialog_sync_and_async_callback(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: list[object] = []

    def fake_open_file_dialog(*, title: str | None = None, filters: object = None) -> str:
        calls.append((title, filters))
        return "J:/data/example.csv"

    monkeypatch.setattr(dialogs_module._backend, "open_file_dialog", fake_open_file_dialog)

    selected = dg.open_file_dialog(
        title="Open CSV",
        filters=[("CSV", ["csv"])],
    )

    assert selected == "J:/data/example.csv"
    assert calls == [("Open CSV", [("CSV", ["csv"])])]

    event = threading.Event()

    class FakeApp:
        def call_soon_threadsafe(self, fn: object) -> None:
            assert callable(fn)
            fn()
            event.set()

    dg.open_file_dialog(
        on_select=lambda path: calls.append(("async", path)),
        app=FakeApp(),
    )

    assert event.wait(1.0)
    assert calls[-1] == ("async", "J:/data/example.csv")


def test_file_dialog_filter_validation() -> None:
    with pytest.raises(ValueError, match="filters"):
        dg.open_file_dialog(filters=[("", ["csv"])])
    with pytest.raises(ValueError, match="filters"):
        dg.FileDialog.open_file(filters=[("CSV", [])])


def test_file_dialog_helpers_delegate_to_backend(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: list[tuple[str, object]] = []

    def fake_open_files_dialog(*, title: str | None = None, filters: object = None) -> list[str]:
        calls.append(("open_files", (title, filters)))
        return ["a.csv", "b.csv"]

    def fake_save_file_dialog(*, title: str | None = None, filters: object = None) -> str:
        calls.append(("save_file", (title, filters)))
        return "out.csv"

    def fake_pick_folder_dialog(*, title: str | None = None) -> str:
        calls.append(("pick_folder", title))
        return "J:/data"

    monkeypatch.setattr(dialogs_module._backend, "open_files_dialog", fake_open_files_dialog)
    monkeypatch.setattr(dialogs_module._backend, "save_file_dialog", fake_save_file_dialog)
    monkeypatch.setattr(dialogs_module._backend, "pick_folder_dialog", fake_pick_folder_dialog)

    assert dg.FileDialog.open_files(title="Open", filters=[("CSV", ["csv"])]) == ["a.csv", "b.csv"]
    assert dg.open_files_dialog(title="Open 2", filters=[("JSON", ["json"])]) == ["a.csv", "b.csv"]
    assert dg.save_file_dialog(title="Save", filters=[("CSV", ["csv"])]) == "out.csv"
    assert dg.pick_folder_dialog(title="Folder") == "J:/data"
    assert calls == [
        ("open_files", ("Open", [("CSV", ["csv"])])),
        ("open_files", ("Open 2", [("JSON", ["json"])])),
        ("save_file", ("Save", [("CSV", ["csv"])])),
        ("pick_folder", "Folder"),
    ]


def test_color_picker_serializes_and_updates_from_channel_callback() -> None:
    calls: list[tuple[int, ...]] = []
    picker = dg.ColorPicker(
        (1.0, 0.5, 0.0),
        alpha=False,
        on_change=lambda value: calls.append(value),
        parent=None,
    )

    serialized = picker.to_dict()
    assert serialized["type"] == "panel"
    assert serialized["props"]["title"] == "Color"
    assert serialized["props"]["width"] is None
    assert serialized["style"]["max_width"] == 320
    assert serialized["style"]["flex_shrink"] == 1
    assert serialized["style"]["gap"] == 6
    assert serialized["children"][0]["type"] == "button"
    assert serialized["children"][0]["style"]["background"] == "#ff8000"
    assert serialized["children"][1]["style"]["height"] == 32
    assert serialized["children"][1]["style"]["gap"] == 4
    assert serialized["children"][1]["children"][0]["props"]["text"] == "R"
    assert serialized["children"][1]["children"][0]["style"]["width"] == 26
    assert serialized["children"][1]["children"][0]["style"]["height"] == 32
    assert serialized["children"][1]["children"][0]["style"]["color"] == "text"
    assert serialized["children"][1]["children"][0]["style"]["text_align"] == "center"
    assert picker.value == (255, 128, 0)

    _, change_cbs = _collect_runtime_callbacks(picker)
    change_cbs[picker._sliders["g"].id](64)

    assert picker.value == (255, 64, 0)
    assert picker._value_labels["g"].text == "64"
    assert picker._swatch.style is not None
    assert picker._swatch.style["background"] == "#ff4000"
    assert calls == [(255, 64, 0)]

    picker.set_value((10, 20, 30, 40))
    assert picker.value == (10, 20, 30)
    assert picker._sliders["r"].value == 10
    assert picker._value_labels["b"].text == "30"
    assert calls == [(255, 64, 0)]

    picker.set_value((40, 50, 60), notify=True)
    assert picker.value == (40, 50, 60)
    assert calls == [(255, 64, 0), (40, 50, 60)]

    assert dg.ColorPicker((1, 1, 1), alpha=False, parent=None).value == (1, 1, 1)
    assert dg.ColorPicker((1.0, 1.0, 1.0), alpha=False, parent=None).value == (255, 255, 255)
    assert dg.ColorPicker((0.0, 0.5, 1.0), alpha=True, parent=None).value == (0, 128, 255, 255)
    assert "max_width" not in dg.ColorPicker((1, 2, 3), width=None, parent=None).to_dict()["style"]

    with pytest.raises(ValueError, match="3 RGB or 4 RGBA"):
        dg.ColorPicker((1, 2), parent=None)


def test_image_serializes_validates_and_updates_live_props() -> None:
    class Sender:
        def __init__(self) -> None:
            self.props: list[tuple[str, str, object]] = []

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def close(self) -> None:
            pass

    image = dg.Image(
        Path("examples/assets/demo.png"),
        fit="cover",
        width=320,
        height=180,
        id="hero-image",
        parent=None,
    )

    serialized = image.to_dict()
    assert serialized["type"] == "image"
    assert Path(serialized["props"]["path"]).parts[-3:] == ("examples", "assets", "demo.png")
    assert serialized["props"]["fit"] == "cover"
    assert serialized["props"]["width"] == 320.0
    assert serialized["props"]["height"] == 180.0

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    image._bind_live(handle.widget_handle(image.id))

    image.set_fit("stretch")
    image.set_path("missing.png")
    image.reload()

    assert sender.props == [
        ("hero-image", "fit", "stretch"),
        ("hero-image", "path", "missing.png"),
        ("hero-image", "path", "missing.png"),
    ]

    with pytest.raises(ValueError, match="fit"):
        dg.Image("x.png", fit="tile", parent=None)
    with pytest.raises(ValueError, match="width"):
        dg.Image("x.png", width=0, parent=None)


def test_progress_bar_serializes_and_clamps_value() -> None:
    app = dg.App()
    win = dg.Window("Progress")

    progress = dg.ProgressBar(
        1.4,
        min=0,
        max=1,
        show_value=True,
        style={"accent": "success"},
        parent=win,
    )

    document = app.document(win)
    serialized = document["window"]["children"][0]

    assert progress.value == 1.0
    assert serialized["type"] == "progress_bar"
    assert serialized["props"]["value"] == 1.0
    assert serialized["props"]["min"] == 0.0
    assert serialized["props"]["max"] == 1.0
    assert serialized["props"]["label"] == "100%"
    assert serialized["style"]["accent"] == "success"

    with pytest.raises(ValueError, match="max"):
        dg.ProgressBar(0.5, min=1, max=0, parent=None)


def test_progress_bar_set_value_updates_live_native_value_and_label() -> None:
    class Sender:
        def __init__(self) -> None:
            self.props: list[tuple[str, str, object]] = []

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def close(self) -> None:
            pass

    progress = dg.ProgressBar(0.1, id="progress", show_value=True, parent=None)
    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    progress._bind_live(handle.widget_handle(progress.id))

    progress.set_value(0.42)

    assert progress.value == 0.42
    assert sender.props == [
        ("progress", "value", 0.42),
        ("progress", "label", "42%"),
    ]


def test_number_input_serializes_clamps_and_registers_callback() -> None:
    calls: list[float] = []
    app = dg.App()
    win = dg.Window("Number")

    number = dg.NumberInput(
        12.5,
        min=0,
        max=10,
        step=0.25,
        on_change=lambda value: calls.append(value),
        parent=win,
    )

    serialized = app.document(win)["window"]["children"][0]

    assert number.value == 10.0
    assert serialized["type"] == "number_input"
    assert serialized["props"]["value"] == 10.0
    assert serialized["props"]["min"] == 0.0
    assert serialized["props"]["max"] == 10.0
    assert serialized["props"]["step"] == 0.25
    assert serialized["props"]["text"] == "10"
    assert serialized["props"]["events"] == ["change"]

    handle = AppHandle()
    handle.register_widget_callbacks(number)
    assert handle._invoke_change_callback(number.id, 7.25) is True
    assert number.value == 7.25
    assert calls == [7.25]

    with pytest.raises(ValueError, match="step"):
        dg.NumberInput(1, step=0, parent=None)
    with pytest.raises(ValueError, match="max"):
        dg.NumberInput(1, min=2, max=1, parent=None)


def test_number_input_set_value_updates_live_native_value() -> None:
    class Sender:
        def __init__(self) -> None:
            self.props: list[tuple[str, str, object]] = []

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def close(self) -> None:
            pass

    number = dg.NumberInput(1, min=0, max=10, id="gain", parent=None)
    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    number._bind_live(handle.widget_handle(number.id))

    number.set_value(14)

    assert number.value == 10.0
    assert sender.props == [("gain", "value", 10.0)]


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

    number = dg.NumberInput(1, id="gain", parent=None)
    number._bind_live(handle.widget_handle(number.id))

    number.set_style({"parts": {"stepper_up": {"background": "danger", "width": 32}}})
    assert number.style == {
        "parts": {"stepper_up": {"background": "danger", "width": 32}}
    }
    assert sender.styles[-1] == (
        "gain",
        '{"parts":{"stepper_up":{"background":"danger","width":32}}}',
    )

    with pytest.raises(ValueError, match="NumberInput has no CSS part 'thumb'"):
        number.set_style({"parts": {"thumb": {"width": 18}}})
    assert number.style == {
        "parts": {"stepper_up": {"background": "danger", "width": 32}}
    }
    assert sender.styles[-1] == (
        "gain",
        '{"parts":{"stepper_up":{"background":"danger","width":32}}}',
    )

    button.set_style(None)
    assert "style" not in button.to_dict()
    assert sender.styles[-1] == (
        "run",
        '{"background":null,"border_radius":null}',
    )

    number.set_style(None)
    assert "style" not in number.to_dict()
    assert sender.styles[-1] == (
        "gain",
        '{"parts":null}',
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
    collapsible = dg.Collapsible(
        "Advanced",
        id="advanced",
        expanded=False,
        on_change=lambda value: calls.append(("advanced", value)),
        parent=None,
    )
    text = dg.TextInput(
        "",
        id="text",
        on_change=lambda value: calls.append(("text", value)),
        parent=None,
    )
    text_area = dg.TextArea(
        "one",
        id="notes",
        on_change=lambda value: calls.append(("notes", value)),
        parent=None,
    )

    handle.register_widget_callbacks(checkbox)
    handle.register_widget_callbacks(collapsible)
    handle.register_widget_callbacks(text)
    handle.register_widget_callbacks(text_area)

    assert handle._invoke_change_callback("check", True) is True
    assert handle._invoke_change_callback("advanced", True) is True
    assert handle._invoke_change_callback("text", "hello") is True
    assert handle._invoke_change_callback("notes", "hello\nworld") is True
    assert checkbox.checked is True
    assert collapsible.expanded is True
    assert text.value == "hello"
    assert text_area.value == "hello\nworld"
    assert calls == [
        ("check", True),
        ("advanced", True),
        ("text", "hello"),
        ("notes", "hello\nworld"),
    ]

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


def test_app_toast_enqueues_native_commands() -> None:
    class Sender:
        def __init__(self) -> None:
            self.shown: list[
                tuple[
                    str,
                    str,
                    str,
                    int | None,
                    float | None,
                    float | None,
                    float | None,
                    str | None,
                ]
            ] = []
            self.dismissed: list[str] = []

        def enqueue_show_toast(
            self,
            toast_id: str,
            message: str,
            level: str,
            duration_ms: int | None = None,
            opacity: float | None = None,
            radius: float | None = None,
            padding: float | None = None,
            position: str | None = None,
        ) -> None:
            self.shown.append(
                (toast_id, message, level, duration_ms, opacity, radius, padding, position)
            )

        def enqueue_dismiss_toast(self, toast_id: str) -> None:
            self.dismissed.append(toast_id)

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    app = dg.App()
    app._handle = handle

    toast = app.toast(
        "Export complete",
        level="success",
        duration=2500,
        opacity=0.85,
        radius=10,
        padding=16,
        position="bottom-left",
    )
    toast.update("Saved", level="info", duration=None, position="top_left")
    toast.dismiss()
    _set_active_app_handle(handle)
    try:
        dg.toast("Queued from callback", level="warning", duration=500)
    finally:
        _set_active_app_handle(None)

    assert isinstance(toast, dg.ToastHandle)
    assert sender.shown == [
        ("toast-1", "Export complete", "success", 2500, 0.85, 10.0, 16.0, "bottom-left"),
        ("toast-1", "Saved", "info", None, None, None, None, "top-left"),
        ("toast-2", "Queued from callback", "warning", 500, None, None, None, None),
    ]
    assert sender.dismissed == ["toast-1"]


def test_app_toast_validation_and_running_requirement() -> None:
    with pytest.raises(RuntimeError, match="not running"):
        dg.App().toast("Saved")

    handle = AppHandle()
    with pytest.raises(ValueError, match="level"):
        handle.toast("Saved", level="debug")
    with pytest.raises(ValueError, match="duration"):
        handle.toast("Saved", duration=0)
    with pytest.raises(ValueError, match="message"):
        handle.toast("")
    with pytest.raises(ValueError, match="opacity"):
        handle.toast("Saved", opacity=1.5)
    with pytest.raises(ValueError, match="radius"):
        handle.toast("Saved", radius=-1)
    with pytest.raises(ValueError, match="padding"):
        handle.toast("Saved", padding=-1)
    with pytest.raises(ValueError, match="position"):
        handle.toast("Saved", position="center")


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

    button = dg.Button("Filters", id="button", badge=1, parent=None)
    badge = dg.Badge("Ready", id="badge", parent=None)
    text = dg.TextInput("old", id="text", parent=None)
    text_area = dg.TextArea("old\nvalue", id="notes", parent=None)
    slider = dg.Slider(0.0, min=0, max=1, id="slider", parent=None)
    dropdown = dg.Dropdown(["x", "y"], id="dropdown", parent=None)
    checkbox = dg.Checkbox("Enabled", id="checkbox", parent=None)
    collapsible = dg.Collapsible("Advanced", id="advanced", expanded=False, parent=None)
    tabs = dg.Tabs(value="one", id="tabs", parent=None)
    dg.Tab("One", value="one", parent=tabs)
    dg.Tab("Two", value="two", parent=tabs)
    pages = dg.Pages(value="one", id="pages", parent=None)
    dg.Page("one", parent=pages)
    dg.Page("two", parent=pages)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="scatter", parent=None)
    for widget in (
        button,
        badge,
        text,
        text_area,
        slider,
        dropdown,
        checkbox,
        collapsible,
        tabs,
        pages,
        scatter,
    ):
        widget._bind_live(handle.widget_handle(widget.id))

    button.set_badge(None)
    badge.set_value("Busy")
    badge.set_level("warning")
    text.set_value("new")
    text_area.set_value("new\nvalue")
    slider.set_value(2.0)
    dropdown.set_value("y")
    checkbox.set_checked(True)
    collapsible.set_expanded(True)
    tabs.set_value("two")
    pages.set_value("two")
    monkey_payload = bytes(range(12))
    original_pack = widgets_module._pack_xyz_bytes
    widgets_module._pack_xyz_bytes = lambda frame, x, y, z: monkey_payload
    try:
        scatter.set_points(DemoFrame(), x="x", y="y", z="z")
    finally:
        widgets_module._pack_xyz_bytes = original_pack

    assert button.badge is None
    assert badge.text == "Busy"
    assert badge.level == "warning"
    assert text.value == "new"
    assert text_area.value == "new\nvalue"
    assert slider.value == 1.0
    assert dropdown.value == "y"
    assert checkbox.checked is True
    assert collapsible.expanded is True
    assert tabs.value == "two"
    assert pages.value == "two"
    assert sender.props == [
        ("button", "badge", None),
        ("badge", "text", "Busy"),
        ("badge", "level", "warning"),
        ("text", "value", "new"),
        ("notes", "value", "new\nvalue"),
        ("slider", "value", 1.0),
        ("dropdown", "value", "y"),
        ("checkbox", "checked", True),
        ("advanced", "expanded", True),
        ("tabs", "value", "two"),
        ("pages", "value", "two"),
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
    button = dg.Button("Filters", badge=3, parent=win)
    text = dg.TextInput("abc", placeholder="Name", parent=win)
    text_area = dg.TextArea("line 1\nline 2", placeholder="Notes", rows=5, wrap=False, parent=win)
    dropdown = dg.Dropdown(["x", "y"], value="y", disabled=True, parent=win)
    slider = dg.Slider(0.5, min=0, max=1, step=0.25, parent=win)
    badge = dg.Badge("live", level="success", parent=win)
    tag = dg.Tag("queued", level="warning", parent=win)

    document = app.document(win)

    assert document["theme"]["background"] == "#f6f7fb"
    assert document["theme"]["accent"] == "#0055ff"
    assert document["theme"]["spacing"] == 10.0
    assert document["theme"]["font_size"] == 14.0
    assert text.to_dict()["props"]["placeholder"] == "Name"
    assert dropdown.to_dict()["props"]["disabled"] is True
    assert slider.to_dict()["props"]["step"] == 0.25
    assert badge.to_dict()["type"] == "badge"
    assert badge.to_dict()["props"] == {"text": "live", "level": "success"}
    assert tag.to_dict()["type"] == "tag"
    assert tag.to_dict()["props"] == {"text": "queued", "level": "warning"}
    assert text_area.to_dict()["type"] == "text_area"
    assert text_area.to_dict()["props"]["value"] == "line 1\nline 2"
    assert text_area.to_dict()["props"]["placeholder"] == "Notes"
    assert text_area.to_dict()["props"]["rows"] == 5
    assert text_area.to_dict()["props"]["wrap"] is False
    assert button.to_dict()["props"]["badge"] == "3"

    with pytest.raises(ValueError, match="rows"):
        dg.TextArea(rows=0, parent=None)
    with pytest.raises(TypeError, match="badge"):
        dg.Button("Bad", badge=True, parent=None)
    with pytest.raises(ValueError, match="unknown badge level"):
        dg.Badge("Bad", level="urgent", parent=None)


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
    text_area = dg.TextArea(
        "",
        on_change=lambda v: calls.append(("notes", v)),
        parent=win,
    )

    _, change_cbs = _collect_runtime_callbacks(win)

    change_cbs[checkbox.id](True)
    change_cbs[slider.id](0.75)
    change_cbs[dropdown.id]("y")
    change_cbs[text.id]("hello")
    change_cbs[text_area.id]("hello\nworld")

    assert checkbox.checked is True
    assert slider.value == 0.75
    assert dropdown.value == "y"
    assert text.value == "hello"
    assert text_area.value == "hello\nworld"
    assert calls == [
        ("check", True),
        ("slider", 0.75),
        ("drop", "y"),
        ("text", "hello"),
        ("notes", "hello\nworld"),
    ]


def test_dataframe_table_select_callback_payload() -> None:
    class Frame:
        columns = ("city", "total")
        dtypes = ("str", "int64")
        shape = (2, 2)
        city = ["Oslo", "Lima"]
        total = [7, 9]

    calls = []
    win = dg.Window("Table")
    table = dg.DataFrameTable(
        Frame(),
        id="table",
        on_select=lambda row, column, value: calls.append((row, column, value)),
        parent=win,
    )

    _, change_cbs = _collect_runtime_callbacks(win)
    payload = {
        "row_index": 1,
        "column_index": 0,
        "column": "city",
        "value": "Lima",
    }
    change_cbs["table"](json.dumps(payload))

    assert table.to_dict()["props"]["events"] == ["change"]
    assert isinstance(table.selection, dg.TableSelection)
    assert table.selection.row_index == 1
    assert table.selection.column_index == 0
    assert table.selection.column == "city"
    assert table.selection.value == "Lima"
    assert calls == [(1, "city", "Lima")]


def test_dataframe_table_select_callback_accepts_selection_object() -> None:
    class Frame:
        columns = ("city",)
        shape = (1, 1)
        city = ["Oslo"]

    calls = []
    win = dg.Window("Table")
    table = dg.DataFrameTable(
        Frame(),
        id="table",
        on_select=lambda selection: calls.append(selection),
        parent=win,
    )

    _, change_cbs = _collect_runtime_callbacks(win)
    change_cbs["table"](
        {
            "row_index": 0,
            "column_index": 0,
            "column": "city",
            "value": "Oslo",
        }
    )

    assert calls == [dg.TableSelection(0, 0, "city", "Oslo")]
    assert table.selection == calls[0]


def test_scatter_pick_callback_payload() -> None:
    calls = []
    win = dg.Window("Scatter")
    scatter = dg.Scatter3D(
        DemoFrame(),
        x="x",
        y="y",
        z="z",
        id="scatter",
        on_pick=lambda pick: calls.append(pick),
        parent=win,
    )

    _, change_cbs = _collect_runtime_callbacks(win)
    change_cbs["scatter"](
        json.dumps({"index": 2, "x": 1.25, "y": -2.5, "z": 3.75})
    )

    assert scatter.to_dict()["props"]["events"] == ["change"]
    assert calls == [dg.ScatterPick(2, 1.25, -2.5, 3.75)]
    assert scatter.pick == calls[0]


def test_scatter_pick_callback_accepts_index_and_coordinates() -> None:
    calls = []
    win = dg.Window("Scatter")
    dg.Scatter3D(
        DemoFrame(),
        x="x",
        y="y",
        z="z",
        id="scatter",
        on_pick=lambda index, x, y, z: calls.append((index, x, y, z)),
        parent=win,
    )

    _, change_cbs = _collect_runtime_callbacks(win)
    change_cbs["scatter"]({"index": 4, "x": 1, "y": 2, "z": 3})

    assert calls == [(4, 1.0, 2.0, 3.0)]


def test_change_callbacks_only_registered_when_requested() -> None:
    win = dg.Window("No callbacks")
    dg.Checkbox("Enabled", checked=False, parent=win)
    dg.Slider(0.0, parent=win)
    dg.Dropdown(["x", "y"], parent=win)
    dg.TextInput("", parent=win)
    dg.TextArea("", parent=win)
    dg.DataFrameTable(DemoFrame(), parent=win)
    dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=win)

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
            dg.NavItem("Table", page="table", badge=12)

        with dg.Pages(value="scatter", on_change=lambda value: calls.append(("page", value))):
            with dg.Page("scatter", title="Scatter"):
                dg.Label("Scatter page")
            with dg.Page("table"):
                dg.Label("Table page")

    with dg.Tabs(value="table", on_change=lambda value: calls.append(("tab", value))):
        with dg.Tab("Scatter", value="scatter"):
            dg.Label("Scatter tab")
        with dg.Tab("Table", value="table", badge="new"):
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
    assert sidebar["children"][1]["props"]["badge"] == "12"
    assert pages["type"] == "pages"
    assert pages["props"]["value"] == "scatter"
    assert pages["children"][0]["type"] == "page"
    assert pages["children"][0]["props"]["title"] == "Scatter"
    assert tabs["type"] == "tabs"
    assert tabs["props"]["value"] == "table"
    assert tabs["children"][0]["type"] == "tab"
    assert tabs["children"][0]["props"]["label"] == "Scatter"
    assert tabs["children"][1]["props"]["badge"] == "new"
    assert calls == []

    _, change_cbs = _collect_runtime_callbacks(win)
    assert pages["id"] in change_cbs
    assert tabs["id"] in change_cbs
    change_cbs[pages["id"]]("table")
    change_cbs[tabs["id"]]("scatter")

    assert calls == [("page", "table"), ("tab", "scatter")]


def test_navigation_set_value_updates_route_and_optional_callback() -> None:
    calls: list[tuple[str, str]] = []
    pages = dg.Pages(value="one", on_change=lambda value: calls.append(("page", value)), parent=None)
    dg.Page("one", parent=pages)
    dg.Page("two", parent=pages)
    tabs = dg.Tabs(value="one", on_change=lambda value: calls.append(("tab", value)), parent=None)
    dg.Tab("One", value="one", parent=tabs)
    dg.Tab("Two", value="two", parent=tabs)

    pages.set_value("two")
    tabs.set_value("two", notify=True)

    assert pages.value == "two"
    assert tabs.value == "two"
    assert calls == [("tab", "two")]

    pages.set_value("one", notify=True)
    assert calls == [("tab", "two"), ("page", "one")]

    with pytest.raises(ValueError, match="Pages value"):
        pages.set_value("missing")
    with pytest.raises(ValueError, match="Tabs value"):
        tabs.set_value("missing")


def test_menu_widgets_serialize_and_register_callbacks() -> None:
    calls = []
    app = dg.App()
    win = dg.Window("Menus")

    with dg.MenuBar(height=32, tooltip="Application menu"):
        with dg.Menu("File"):
            open_item = dg.MenuItem("Open", on_click=lambda: calls.append("open"))
            dg.MenuItem("Disabled", disabled=True, on_click=lambda: calls.append("disabled"))
        with dg.Menu("Help", disabled=True):
            dg.MenuItem("About", on_click=lambda: calls.append("about"))

    table = dg.DataFrameTable({"x": [1, 2]}, id="table", parent=win)
    with dg.ContextMenu(target=table, width=240, parent=win):
        inspect_item = dg.MenuItem("Inspect row", on_click=lambda: calls.append("inspect"))

    document = app.document(win)
    menu_bar = document["window"]["children"][0]
    file_menu = menu_bar["children"][0]
    context_menu = document["window"]["children"][2]

    assert menu_bar["type"] == "menu_bar"
    assert menu_bar["props"]["height"] == 32.0
    assert menu_bar["props"]["tooltip"] == "Application menu"
    assert file_menu["type"] == "menu"
    assert file_menu["props"]["label"] == "File"
    assert file_menu["children"][0]["type"] == "menu_item"
    assert file_menu["children"][0]["props"]["events"] == ["click"]
    assert file_menu["children"][1]["props"]["events"] == []
    assert context_menu["type"] == "context_menu"
    assert context_menu["props"]["target"] == "table"
    assert context_menu["props"]["width"] == 240.0

    click_cbs, _ = _collect_runtime_callbacks(win)
    click_cbs[open_item.id]()
    click_cbs[inspect_item.id]()

    assert calls == ["open", "inspect"]


def test_menu_validation() -> None:
    dg.Window("Menu validation")

    with pytest.raises(RuntimeError, match="MenuBar context"):
        dg.Menu("Orphan")

    with pytest.raises(RuntimeError, match="Menu or ContextMenu"):
        dg.MenuItem("Orphan")

    with dg.MenuBar():
        with pytest.raises(ValueError, match="Menu label"):
            dg.Menu("")

    with dg.MenuBar():
        with dg.Menu("File"):
            with pytest.raises(ValueError, match="MenuItem label"):
                dg.MenuItem("")

    with pytest.raises(ValueError, match="ContextMenu width"):
        dg.ContextMenu(width=0, parent=None)

    with pytest.raises(ValueError, match="ContextMenu target"):
        dg.ContextMenu(target="", parent=None)


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
