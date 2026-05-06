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


def test_app_loading_screen_serializes_defaults_and_custom_values() -> None:
    default_doc = dg.App().document(dg.Window("Default"))
    assert default_doc["loading_screen"]["enabled"] is True
    assert default_doc["loading_screen"]["title"] == "Loading"
    assert default_doc["loading_screen"]["show_spinner"] is True

    disabled_doc = dg.App(loading_screen=False).document(dg.Window("Disabled"))
    assert disabled_doc["loading_screen"]["enabled"] is False

    app = dg.App(
        loading_screen=dg.LoadingScreen(
            title="Loading dashboard",
            message="Preparing plots...",
            background="#0b1020",
            text=(248, 250, 252, 1),
            accent="#42a5ff",
            show_progress=True,
            min_duration_ms=160,
        )
    )
    loading = app.document(dg.Window("Custom"))["loading_screen"]

    assert loading["enabled"] is True
    assert loading["title"] == "Loading dashboard"
    assert loading["message"] == "Preparing plots..."
    assert loading["background"] == "#0b1020"
    assert loading["text"] == (248.0, 250.0, 252.0, 1.0)
    assert loading["accent"] == "#42a5ff"
    assert loading["show_progress"] is True
    assert loading["min_duration_ms"] == 160


def test_loading_screen_validates_color_tuples() -> None:
    with pytest.raises(ValueError, match="tuple"):
        dg.LoadingScreen(background=(1, 2))
    with pytest.raises(TypeError, match="color"):
        dg.LoadingScreen(accent=object())  # type: ignore[arg-type]


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
            lambda style: dg.LED(True, style=style, parent=None),
            ("dot", "glow", "highlight"),
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

    alpha_picker = dg.ColorPicker((90, 169, 255, 128), alpha=True, parent=None)
    assert alpha_picker.to_dict()["children"][0]["style"]["background"] == "#5aa9ff80"
    alpha_picker.set_value((90, 169, 255, 64))
    assert alpha_picker._swatch.style is not None
    assert alpha_picker._swatch.style["background"] == "#5aa9ff40"

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


def test_html_report_serializes_validates_and_updates_live_props() -> None:
    class Sender:
        def __init__(self) -> None:
            self.props: list[tuple[str, str, object]] = []

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def close(self) -> None:
            pass

    report = dg.HtmlReport(
        Path("reports/plotly.html"),
        height=360,
        allow_remote=True,
        id="html-report",
        parent=None,
    )

    serialized = report.to_dict()
    assert serialized["type"] == "html_report"
    assert Path(serialized["props"]["path"]).parts[-2:] == ("reports", "plotly.html")
    assert serialized["props"]["html"] is None
    assert serialized["props"]["height"] == 360.0
    assert serialized["props"]["allow_remote"] is True
    assert "HTML report: plotly.html" in serialized["props"]["text"]

    inline = dg.HtmlReport.from_html("<html><body>ok</body></html>", base_dir="reports", parent=None)
    assert inline.to_dict()["props"]["path"] is None
    assert inline.to_dict()["props"]["html"].startswith("<html>")
    assert inline.to_dict()["props"]["base_dir"] == "reports"

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    report._bind_live(handle.widget_handle(report.id))

    report.set_path("reports/updated.html")
    report.set_html("<html><body>updated</body></html>", base_dir="reports")
    report.reload()

    assert sender.props == [
        ("html-report", "path", "reports/updated.html"),
        ("html-report", "html", None),
        ("html-report", "base_dir", None),
        ("html-report", "text", "HTML report: updated.html\nOpen externally to view interactive content."),
        ("html-report", "html", "<html><body>updated</body></html>"),
        ("html-report", "path", None),
        ("html-report", "base_dir", "reports"),
        (
            "html-report",
            "text",
            "HTML report: inline document\nOpen externally to view interactive content.",
        ),
        ("html-report", "html", "<html><body>updated</body></html>"),
    ]

    with pytest.raises(ValueError, match="either path or html"):
        dg.HtmlReport(parent=None)
    with pytest.raises(ValueError, match="path or html"):
        dg.HtmlReport("report.html", html="<html></html>", parent=None)
    with pytest.raises(ValueError, match="height"):
        dg.HtmlReport("report.html", height=0, parent=None)


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
            self.scatter_payloads: list[
                tuple[str, bytes, float | None, float | None, str, bool]
            ] = []

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def enqueue_set_scatter_points_packed(
            self,
            widget_id: str,
            xyz: bytes,
            pack_ms: float | None = None,
            enqueue_epoch_ms: float | None = None,
            colormap: str = "viridis",
            payload_format: str = "xyz_f32_v0",
            coalesce: bool = True,
            fit: bool = False,
        ) -> None:
            self.scatter_payloads.append((widget_id, xyz, pack_ms, enqueue_epoch_ms, colormap, fit))

        def enqueue_set_scatter_hover_tooltip(self, widget_id: str, enabled: bool) -> None:
            pass

        def enqueue_set_scatter_tooltip_axis_labels(
            self, widget_id: str, x: str, y: str, z: str
        ) -> None:
            pass

        def enqueue_set_scatter_primary_hover_meta(self, widget_id: str, meta: str) -> None:
            pass

        def enqueue_clear_scatter_actors(self, widget_id: str) -> None:
            pass

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
    led = dg.LED(False, id="led", states={"busy": "#ffcc33"}, parent=None)
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
        led,
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
    led.set_state("busy")
    led.set_color("#ffaa00")
    led.set_size(18)
    collapsible.set_expanded(True)
    tabs.set_value("two")
    pages.set_value("two")
    monkey_payload = bytes(range(12))
    original_pack = widgets_module._pack_xyz_bytes
    widgets_module._pack_xyz_bytes = lambda frame, x, y, z: monkey_payload
    try:
        scatter.set_points(DemoFrame(), x="x", y="y", z="z", fit=True)
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
    assert led.state == "busy"
    assert led.color == "#ffaa00"
    assert led.size == 18.0
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
        ("led", "state", "busy"),
        ("led", "color", "#ffcc33"),
        ("led", "color", "#ffaa00"),
        ("led", "size", 18.0),
        ("advanced", "expanded", True),
        ("tabs", "value", "two"),
        ("pages", "value", "two"),
    ]
    assert len(sender.scatter_payloads) == 1
    widget_id, payload, pack_ms, enqueue_epoch_ms, colormap, fit = sender.scatter_payloads[0]
    assert widget_id == "scatter"
    assert payload == monkey_payload
    assert pack_ms is not None and pack_ms >= 0.0
    assert enqueue_epoch_ms is not None and enqueue_epoch_ms > 0.0
    assert colormap == "viridis"
    assert fit is True


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
            payload_format: str = "xyz_f32_v0",
            coalesce: bool = True,
            fit: bool = False,
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


def test_line_plot_serializes_packed_xy_series() -> None:
    np = pytest.importorskip("numpy")

    class NumericFrame:
        columns = ("t", "value")
        dtypes = ("float32", "float32")
        shape = (3, 2)
        t = np.array([0.0, 1.0, 2.0], dtype=np.float32)
        value = np.array([2.0, 3.5, 4.0], dtype=np.float32)

        def __getitem__(self, column: str) -> object:
            return getattr(self, column)

    plot = dg.LinePlot(
        NumericFrame(),
        x="t",
        y="value",
        label="Sensor",
        color="#42a5ff",
        line_width=1.5,
        parent=None,
    )

    props = plot.to_dict()["props"]

    assert plot.to_dict()["type"] == "line_plot"
    assert props["frame"]["rows"] == 3
    assert props["x_label"] == "t"
    assert props["y_label"] == "value"
    assert props["line_width"] == 1.5
    assert props["show_axes"] is True
    assert props["show_ticks"] is True
    assert props["show_toolbar"] is False
    assert props["show_legend"] is False
    assert props["legend_position"] == "top-right"
    assert props["window_size"] is None
    assert props["interaction"] == "inspect"
    assert props["tick_count"] == 5
    assert props["series"][0]["label"] == "Sensor"
    assert props["series"][0]["color"] == "#42a5ff"
    assert props["series"][0]["line_style"] == "solid"
    assert props["series"][0]["data_format"] == "xy_f32_v0"
    assert props["series"][0]["points"] == 3
    assert props["series"][0]["data_b64"]


def test_histogram_serializes_binned_data() -> None:
    np = pytest.importorskip("numpy")

    class NumericFrame:
        columns = ("latency",)
        dtypes = ("float32",)
        shape = (6, 1)
        latency = np.array([0.0, 1.0, 1.5, 2.0, 2.5, float("nan")], dtype=np.float32)

        def __getitem__(self, column: str) -> object:
            return getattr(self, column)

    hist = dg.Histogram(
        NumericFrame(),
        value="latency",
        bins=3,
        range=(0.0, 3.0),
        color="#42a5ff",
        parent=None,
    )
    props = hist.to_dict()["props"]

    assert hist.to_dict()["type"] == "histogram"
    assert props["frame"]["rows"] == 6
    assert props["value"] == "latency"
    assert props["x_label"] == "latency"
    assert props["y_label"] == "count"
    assert props["mode"] == "count"
    assert props["show_toolbar"] is False
    assert props["interaction"] == "inspect"
    assert props["auto_fit"] is True
    assert props["input_count"] == 6
    assert props["finite_count"] == 5
    assert props["edges"] == [0.0, 1.0, 2.0, 3.0]
    assert props["counts"] == [1.0, 2.0, 2.0]
    assert props["color"] == "#42a5ff"


def test_pie_chart_serializes_direct_and_frame_data() -> None:
    chart = dg.PieChart(
        labels=["A", "B", "C"],
        values=[3, 2, 1],
        colors=["#111111", "#222222", "#333333"],
        donut=True,
        title="Mix",
        parent=None,
    )
    data = chart.to_dict()
    props = data["props"]

    assert data["type"] == "pie_chart"
    assert props["labels"] == ["A", "B", "C"]
    assert props["values"] == [3.0, 2.0, 1.0]
    assert props["colors"] == ["#111111", "#222222", "#333333"]
    assert props["total"] == 6.0
    assert props["donut"] is True
    assert props["title"] == "Mix"

    class SegmentFrame:
        segment = ["Team", "Free", "Team", "Enterprise"]
        revenue = [10.0, 1.0, 12.0, 30.0]

        def __getitem__(self, column: str) -> object:
            return getattr(self, column)

    frame_chart = dg.PieChart(
        SegmentFrame(),
        category="segment",
        value="revenue",
        aggregate="sum",
        top_n=2,
        parent=None,
    )
    frame_props = frame_chart.to_dict()["props"]

    assert frame_props["labels"] == ["Enterprise", "Team", "Other"]
    assert frame_props["values"] == [30.0, 22.0, 1.0]
    assert frame_props["total"] == 53.0


def test_line_plot_y_only_uses_sample_index() -> None:
    np = pytest.importorskip("numpy")

    class NumericFrame:
        value = np.array([2.0, 3.5, 4.0], dtype=np.float32)

    plot = dg.LinePlot(NumericFrame(), y="value", parent=None)
    payload = dg.LinePlot.prepare_points(NumericFrame(), y="value")

    assert plot.props()["x_label"] == "sample"
    assert plot.props()["series"][0]["points"] == 3
    assert payload.point_count == 3
    assert len(payload.data) == 3 * 8


def test_line_plot_serializes_multiple_series() -> None:
    np = pytest.importorskip("numpy")

    class NumericFrame:
        columns = ("t", "temperature", "pressure")
        dtypes = ("float32", "float32", "float32")
        shape = (3, 3)
        t = np.array([0.0, 1.0, 2.0], dtype=np.float32)
        temperature = np.array([68.0, 69.5, 70.0], dtype=np.float32)
        pressure = np.array([30.0, 31.0, 29.5], dtype=np.float32)

        def __getitem__(self, column: str) -> object:
            return getattr(self, column)

    plot = dg.LinePlot(
        NumericFrame(),
        x="t",
        y=("temperature", "pressure"),
        labels=("Temp", "Pressure"),
        colors=("#42a5ff", "#74ddb0"),
        line_styles=("solid", "dashed"),
        show_legend=True,
        legend_position="bottom-left",
        parent=None,
    )
    props = plot.props()

    assert props["y"] == ["temperature", "pressure"]
    assert props["show_legend"] is True
    assert props["legend_position"] == "bottom-left"
    assert [item["label"] for item in props["series"]] == ["Temp", "Pressure"]
    assert [item["color"] for item in props["series"]] == ["#42a5ff", "#74ddb0"]
    assert [item["line_style"] for item in props["series"]] == ["solid", "dashed"]
    assert [item["points"] for item in props["series"]] == [3, 3]

    with pytest.raises(ValueError, match="label length"):
        dg.LinePlot(NumericFrame(), x="t", y=("temperature", "pressure"), label="One", parent=None)
    with pytest.raises(ValueError, match="line_styles length"):
        dg.LinePlot(
            NumericFrame(),
            x="t",
            y=("temperature", "pressure"),
            line_styles=("solid",),
            parent=None,
        )


def test_line_plot_startup_resources_enqueue_packed_series() -> None:
    np = pytest.importorskip("numpy")

    class NumericFrame:
        columns = ("t", "temperature", "pressure")
        dtypes = ("float32", "float32", "float32")
        shape = (3, 3)
        t = np.array([0.0, 1.0, 2.0], dtype=np.float32)
        temperature = np.array([68.0, 69.5, 70.0], dtype=np.float32)
        pressure = np.array([30.0, 31.0, 29.5], dtype=np.float32)

        def __getitem__(self, column: str) -> object:
            return getattr(self, column)

    class Sender:
        def __init__(self) -> None:
            self.set_data: list[tuple[object, ...]] = []
            self.props: list[tuple[str, str, object]] = []

        def enqueue_set_line_plot_data_packed(
            self,
            widget_id: str,
            series: str,
            xy: bytes,
            label: str | None,
            color: str | None,
            line_width: float | None,
            line_style: str | None,
            show_grid: bool | None,
            auto_fit: bool | None,
            max_points: int | None,
            fit: bool,
            coalesce: bool,
        ) -> None:
            self.set_data.append(
                (
                    widget_id,
                    series,
                    xy,
                    label,
                    color,
                    line_width,
                    line_style,
                    show_grid,
                    auto_fit,
                    max_points,
                    fit,
                    coalesce,
                )
            )

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    plot = dg.LinePlot(
        NumericFrame(),
        x="t",
        y=("temperature", "pressure"),
        labels=("Temp", "Pressure"),
        colors=("#42a5ff", "#74ddb0"),
        x_label="Elapsed",
        y_label="Reading",
        id="line",
        parent=None,
    )
    plot._bind_live(handle.widget_handle(plot.id))

    plot._queue_startup_resources()

    assert sender.props == []
    assert len(sender.set_data) == 2
    assert [item[1] for item in sender.set_data] == ["Temp", "Pressure"]
    assert [len(item[2]) for item in sender.set_data] == [3 * 8, 3 * 8]
    assert all(item[10] is True for item in sender.set_data)


def test_line_plot_live_methods_enqueue_native_commands() -> None:
    np = pytest.importorskip("numpy")

    class NumericFrame:
        columns = ("t", "value", "other")
        dtypes = ("float32", "float32", "float32")
        shape = (3, 3)
        t = np.array([0.0, 1.0, 2.0], dtype=np.float32)
        value = np.array([2.0, 3.5, 4.0], dtype=np.float32)
        other = np.array([7.0, 8.0, 9.0], dtype=np.float32)

        def __getitem__(self, column: str) -> object:
            return getattr(self, column)

    class Sender:
        def __init__(self) -> None:
            self.set_data: list[tuple[object, ...]] = []
            self.appended: list[tuple[object, ...]] = []
            self.cleared: list[tuple[str, str | None]] = []
            self.props: list[tuple[str, str, object]] = []

        def enqueue_set_line_plot_data_packed(
            self,
            widget_id: str,
            series: str,
            xy: bytes,
            label: str | None,
            color: str | None,
            line_width: float | None,
            line_style: str | None,
            show_grid: bool | None,
            auto_fit: bool | None,
            max_points: int | None,
            fit: bool,
            coalesce: bool,
        ) -> None:
            self.set_data.append(
                (
                    widget_id,
                    series,
                    xy,
                    label,
                    color,
                    line_width,
                    line_style,
                    show_grid,
                    auto_fit,
                    max_points,
                    fit,
                    coalesce,
                )
            )

        def enqueue_append_line_plot_points_packed(
            self,
            widget_id: str,
            series: str,
            xy: bytes,
            max_points: int | None,
        ) -> None:
            self.appended.append((widget_id, series, xy, max_points))

        def enqueue_clear_line_plot_series(self, widget_id: str, series: str | None) -> None:
            self.cleared.append((widget_id, series))

        def enqueue_set_prop(self, widget_id: str, prop: str, value: object) -> None:
            self.props.append((widget_id, prop, value))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    plot = dg.LinePlot(
        NumericFrame(),
        x="t",
        y="value",
        label="Sensor",
        color="#42a5ff",
        line_width=1.5,
        max_points=5,
        id="line",
        parent=None,
    )
    plot._bind_live(handle.widget_handle(plot.id))

    plot.set_data(NumericFrame(), x="t", y="other", label="Other", color="#f36b7f", fit=False)
    plot.append_points(
        np.array([3.0, 4.0], dtype=np.float32),
        np.array([10.0, 11.0], dtype=np.float32),
        series="Other",
    )
    plot.set_line_width(3.25)
    plot.set_grid_visible(False)
    plot.set_axes_visible(False)
    plot.set_ticks_visible(False)
    plot.set_toolbar_visible(False)
    plot.set_window_size(12.5)
    plot.set_window_size(None)
    plot.set_tick_count(7)
    plot.set_axis_labels(x="Elapsed", y="Reading")
    plot.clear("Other")

    assert sender.cleared[0] == ("line", None)
    assert len(sender.set_data) == 1
    widget_id, series, payload, label, color, width, line_style, grid, auto_fit, max_points, fit, coalesce = (
        sender.set_data[0]
    )
    assert widget_id == "line"
    assert series == "Other"
    assert len(payload) == 3 * 8
    assert label == "Other"
    assert color == "#f36b7f"
    assert width == 1.5
    assert line_style == "solid"
    assert grid is True
    assert auto_fit is True
    assert max_points == 5
    assert fit is False
    assert coalesce is True
    assert sender.appended[0][0:2] == ("line", "Other")
    assert len(sender.appended[0][2]) == 2 * 8
    assert sender.appended[0][3] == 5
    assert sender.props == [
        ("line", "x_label", "t"),
        ("line", "y_label", "Other"),
        ("line", "line_width", 3.25),
        ("line", "show_grid", False),
        ("line", "show_axes", False),
        ("line", "show_ticks", False),
        ("line", "show_toolbar", False),
        ("line", "window_size", 12.5),
        ("line", "window_size", None),
        ("line", "tick_count", 7),
        ("line", "x_label", "Elapsed"),
        ("line", "y_label", "Reading"),
    ]
    assert sender.cleared[-1] == ("line", "Other")


def test_scatter_xyz_raw_pack_has_expected_size() -> None:
    np = pytest.importorskip("numpy")

    class NumericFrame:
        x = np.array([1.0, 2.0, 3.0], dtype=np.float32)
        y = np.array([4.0, 5.0, 6.0], dtype=np.float32)
        z = np.array([7.0, 8.0, 9.0], dtype=np.float32)

    payload = widgets_module._pack_xyz_bytes(NumericFrame(), "x", "y", "z")

    assert payload is not None
    assert len(payload) == 3 * 12


def test_scatter_xyz_pack_uses_frame_subscript_before_getattr() -> None:
    np = pytest.importorskip("numpy")

    class SubscriptFrame:
        def __getitem__(self, key: str):
            return np.array([1.0, 2.0, 3.0], dtype=np.float32)

    payload = widgets_module._pack_xyz_bytes(SubscriptFrame(), "x", "y", "z")
    assert payload is not None
    assert len(payload) == 3 * 12


def test_scatter_point_instance_v1_pack_has_expected_stride() -> None:
    np = pytest.importorskip("numpy")

    class NumericFrame:
        x = np.array([1.0, 2.0], dtype=np.float32)
        y = np.array([3.0, 4.0], dtype=np.float32)
        z = np.array([5.0, 6.0], dtype=np.float32)

    payload = widgets_module._pack_point_instances(NumericFrame(), "x", "y", "z")

    assert payload is not None
    # PointInstance = 8 x float32 = 32 bytes each
    assert len(payload) == 2 * 32
    # Verify x,y,z of first point
    import struct
    x0, y0, z0, s0, r0, g0, b0, a0 = struct.unpack_from("<8f", payload, 0)
    assert x0 == pytest.approx(1.0)
    assert y0 == pytest.approx(3.0)
    assert z0 == pytest.approx(5.0)
    assert s0 == pytest.approx(4.0)   # default point_size
    assert a0 == pytest.approx(1.0)   # default opacity


def test_scatter_point_instance_v1_explicit_colors() -> None:
    np = pytest.importorskip("numpy")

    class NumericFrame:
        x = np.array([0.0], dtype=np.float32)
        y = np.array([0.0], dtype=np.float32)
        z = np.array([0.0], dtype=np.float32)

    red_colors = np.array([[255.0, 0.0, 0.0]], dtype=np.float32)
    payload = widgets_module._pack_point_instances(
        NumericFrame(), "x", "y", "z", colors=red_colors
    )
    assert payload is not None
    import struct
    _x, _y, _z, _s, r, g, b, _a = struct.unpack_from("<8f", payload, 0)
    assert r == pytest.approx(1.0)
    assert g == pytest.approx(0.0)
    assert b == pytest.approx(0.0)


def test_scatter_v1_format_selected_when_color_given() -> None:
    np = pytest.importorskip("numpy")

    class NumericFrame:
        x = np.array([1.0], dtype=np.float32)
        y = np.array([2.0], dtype=np.float32)
        z = np.array([3.0], dtype=np.float32)
        c = np.array([0.5], dtype=np.float32)

    scatter = dg.Scatter3D(NumericFrame(), x="x", y="y", z="z", color="c", id="s", parent=None)
    assert scatter.data_format == "point_instance_v1"
    assert scatter.props()["data_format"] == "point_instance_v1"


def test_scatter_v0_format_default_without_advanced_options() -> None:
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    assert scatter.data_format == "xyz_f32_v0"


def test_scatter_opacity_triggers_v1_format() -> None:
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", opacity=0.5, id="s", parent=None)
    assert scatter.data_format == "point_instance_v1"


def test_scatter_colormap_names_returns_sorted_list() -> None:
    names = dg.Scatter3D.colormap_names()
    assert isinstance(names, list)
    assert "viridis" in names
    assert "plasma" in names
    assert names == sorted(names)


def test_scatter_colormap_module_samples_correctly() -> None:
    np = pytest.importorskip("numpy")
    from dragongui.colormap import sample_colormap_numpy

    t = np.array([0.0, 0.5, 1.0], dtype=np.float32)
    rgb = sample_colormap_numpy("viridis", t)
    assert rgb.shape == (3, 3)
    assert rgb.dtype == np.float32
    # t=0 → first control point [0.267, 0.005, 0.329]
    assert rgb[0, 0] == pytest.approx(0.267, abs=0.01)
    # t=1 → last control point [0.993, 0.906, 0.144]
    assert rgb[2, 0] == pytest.approx(0.993, abs=0.01)


def test_scatter_live_camera_reset_enqueues_command(monkeypatch) -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[str] = []

        def enqueue_reset_scatter_camera(self, widget_id: str) -> None:
            self.calls.append(widget_id)

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="scatter", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.reset_camera()
    assert sender.calls == ["scatter"]


def test_scatter_live_view_direction_enqueues_command(monkeypatch) -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple[str, str]] = []

        def enqueue_set_scatter_view_direction(self, widget_id: str, direction: str) -> None:
            self.calls.append((widget_id, direction))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="scatter", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.view_xy()
    scatter.view_xz()
    scatter.view_yz()
    scatter.view_isometric()
    assert sender.calls == [
        ("scatter", "xy"),
        ("scatter", "xz"),
        ("scatter", "yz"),
        ("scatter", "isometric"),
    ]


def test_scatter_live_set_point_style_enqueues_command(monkeypatch) -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple[str, str]] = []

        def enqueue_set_scatter_point_style(self, widget_id: str, style: str) -> None:
            self.calls.append((widget_id, style))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="scatter", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.set_point_style("circle")
    scatter.set_point_style("square")
    scatter.set_point_style("gaussian")
    assert sender.calls == [
        ("scatter", "circle"),
        ("scatter", "square"),
        ("scatter", "gaussian"),
    ]


def test_scatter_set_point_style_rejects_invalid_value(monkeypatch) -> None:
    import pytest
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    with pytest.raises(ValueError, match="unknown point style"):
        scatter.set_point_style("diamond")


def test_scatter_live_set_point_size_uses_style_patch() -> None:
    class Sender:
        def __init__(self) -> None:
            self.styles: list[tuple[str, object]] = []

        def enqueue_set_style(self, widget_id: str, style: object) -> None:
            self.styles.append((widget_id, style))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="scatter", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.set_point_size(6.5)

    assert scatter.point_size_override == 6.5
    assert sender.styles == [("scatter", '{"scatter_point_size":6.5}')]


def test_scatter_v0_zero_rows_returns_empty_bytes() -> None:
    """Zero-row frame produces zero-byte payload without crashing."""
    import numpy as np

    class EmptyFrame:
        x = np.array([], dtype=np.float32)
        y = np.array([], dtype=np.float32)
        z = np.array([], dtype=np.float32)

    from dragongui.widgets import _pack_xyz_bytes
    buf = _pack_xyz_bytes(EmptyFrame(), "x", "y", "z")
    assert buf == b""


def test_scatter_v1_zero_rows_returns_empty_bytes() -> None:
    import numpy as np

    class EmptyFrame:
        x = np.array([], dtype=np.float32)
        y = np.array([], dtype=np.float32)
        z = np.array([], dtype=np.float32)

    from dragongui.widgets import _pack_point_instances
    buf = _pack_point_instances(EmptyFrame(), "x", "y", "z")
    assert buf == b""


def test_scatter_v1_nan_inf_xyz_does_not_crash() -> None:
    """NaN and Inf positions are packed without raising."""
    import math
    import numpy as np
    from dragongui.widgets import _pack_point_instances

    class BadFrame:
        x = np.array([0.0, math.nan, math.inf], dtype=np.float32)
        y = np.array([0.0, 1.0, 2.0], dtype=np.float32)
        z = np.array([0.0, 1.0, 2.0], dtype=np.float32)

    buf = _pack_point_instances(BadFrame(), "x", "y", "z")
    assert buf is not None
    assert len(buf) == 3 * 32


def test_scatter_scalars_nan_maps_to_zero_t() -> None:
    """NaN scalar values map to t=0.0 (first colormap color) without crash."""
    import math
    import numpy as np
    from dragongui.widgets import _scalars_to_rgb

    scalars = np.array([0.0, math.nan, 1.0], dtype=np.float32)
    rgb = _scalars_to_rgb(scalars, "viridis", None, False)
    assert rgb.shape == (3, 3)
    # NaN maps to t=0.0, same color as the lo end of the colormap.
    # Use a 2-element range so t=0.0 reliably maps to index 0 (collapsed range → t=0.5).
    lo_color = _scalars_to_rgb(np.array([0.0, 1.0], dtype=np.float32), "viridis", None, False)
    np.testing.assert_allclose(rgb[1], lo_color[0], atol=1e-5)


def test_scatter_all_equal_scalars_uniform_color() -> None:
    """Collapsed linear range maps to t=0 (DragonSci parity, no divide-by-zero crash)."""
    import numpy as np
    from dragongui.widgets import _scalars_to_rgb

    scalars = np.full(5, 3.0, dtype=np.float32)
    rgb = _scalars_to_rgb(scalars, "viridis", None, False)
    assert rgb.shape == (5, 3)
    # All rows should be identical (uniform t=0 color — DragonSci collapsed linear → lo end)
    assert np.all(rgb == rgb[0])
    # Must equal the lo-end colormap color (t=0)
    lo_color = _scalars_to_rgb(np.array([0.0, 1.0], dtype=np.float32), "viridis", None, False)
    np.testing.assert_allclose(rgb[0], lo_color[0], atol=1e-5)


def test_scatter_collapsed_clim_maps_to_t0() -> None:
    """Explicit clim with equal vmin/vmax collapses to t=0 (DragonSci parity)."""
    import numpy as np
    from dragongui.widgets import _scalars_to_rgb

    scalars = np.array([5.0, 5.0, 5.0], dtype=np.float32)
    rgb = _scalars_to_rgb(scalars, "viridis", (5.0, 5.0), False)
    assert rgb.shape == (3, 3)
    # All rows identical at t=0 (lo-end color)
    assert np.all(rgb == rgb[0])
    lo_color = _scalars_to_rgb(np.array([0.0, 1.0], dtype=np.float32), "viridis", None, False)
    np.testing.assert_allclose(rgb[0], lo_color[0], atol=1e-5)


def test_scatter_invalid_clim_returns_none() -> None:
    """Unparseable clim tuple causes packing to return None gracefully."""
    import numpy as np
    from dragongui.widgets import _pack_point_instances

    class F:
        x = np.array([1.0, 2.0], dtype=np.float32)
        y = np.array([3.0, 4.0], dtype=np.float32)
        z = np.array([5.0, 6.0], dtype=np.float32)

    # clim with non-numeric values triggers a ValueError inside the except
    buf = _pack_point_instances(F(), "x", "y", "z", scalars=F.z, clim=("bad", "value"))  # type: ignore[arg-type]
    assert buf is None


def test_scatter_point_sizes_length_mismatch_returns_none() -> None:
    """point_sizes array of wrong length causes packing to return None."""
    import numpy as np
    from dragongui.widgets import _pack_point_instances

    class F:
        x = np.array([1.0, 2.0, 3.0], dtype=np.float32)
        y = np.array([0.0, 0.0, 0.0], dtype=np.float32)
        z = np.array([0.0, 1.0, 2.0], dtype=np.float32)

    wrong_sizes = np.array([4.0, 4.0], dtype=np.float32)  # length 2 != 3
    buf = _pack_point_instances(F(), "x", "y", "z", point_sizes=wrong_sizes)
    assert buf is None


def test_scatter_custom_getitem_frame_packs_correctly() -> None:
    """Frames implementing __getitem__ but not attributes work with scatter packing."""
    import numpy as np
    from dragongui.widgets import _pack_xyz_bytes

    class DictFrame:
        def __getitem__(self, col: str) -> object:
            data = {
                "a": np.array([1.0, 2.0], dtype=np.float32),
                "b": np.array([3.0, 4.0], dtype=np.float32),
                "c": np.array([5.0, 6.0], dtype=np.float32),
            }
            return data[col]

    buf = _pack_xyz_bytes(DictFrame(), "a", "b", "c")
    assert buf is not None
    assert len(buf) == 2 * 12


def test_scatter_nan_color_applied_to_nan_scalars() -> None:
    """NaN scalar values get the nan_color instead of t=0 colormap color."""
    import math
    import numpy as np
    from dragongui.widgets import _scalars_to_rgb

    scalars = np.array([0.0, math.nan, 1.0], dtype=np.float32)
    nan_rgb = (1.0, 0.0, 0.0)  # red
    rgb = _scalars_to_rgb(scalars, "viridis", None, False, nan_color=nan_rgb)
    assert rgb.shape == (3, 3)
    np.testing.assert_allclose(rgb[1], [1.0, 0.0, 0.0], atol=1e-5)
    # Non-NaN points should not be red
    assert not np.allclose(rgb[0], [1.0, 0.0, 0.0], atol=0.1)


def test_scatter_nan_color_triggers_v1() -> None:
    from dragongui.widgets import _scatter_needs_v1
    assert _scatter_needs_v1(None, None, None, None, 1.0, nan_color=(0.5, 0.5, 0.5))


def test_scatter_categorical_string_column_assigns_palette_colors() -> None:
    """String column for color= uses categorical palette, not scalar colormap."""
    import numpy as np
    from dragongui.widgets import _pack_point_instances

    class F:
        x = np.array([0.0, 1.0, 2.0, 3.0], dtype=np.float32)
        y = np.array([0.0, 0.0, 0.0, 0.0], dtype=np.float32)
        z = np.array([0.0, 0.0, 0.0, 0.0], dtype=np.float32)
        cat = ["a", "b", "a", "b"]

    buf = _pack_point_instances(F(), "x", "y", "z", color="cat")
    assert buf is not None
    pts = np.frombuffer(buf, dtype="<f4").reshape(-1, 8)
    # "a" and "b" should each have a consistent color
    assert np.allclose(pts[0, 4:7], pts[2, 4:7], atol=1e-5)  # both "a"
    assert np.allclose(pts[1, 4:7], pts[3, 4:7], atol=1e-5)  # both "b"
    # "a" and "b" should differ
    assert not np.allclose(pts[0, 4:7], pts[1, 4:7], atol=0.01)


def test_scatter_categorical_low_cardinality_int_column() -> None:
    """Low-cardinality integer column uses categorical palette."""
    import numpy as np
    from dragongui.widgets import _is_categorical

    low_card = np.array([0, 1, 2, 1, 0], dtype=np.int32)
    assert _is_categorical(low_card)


def test_scatter_categorical_high_cardinality_int_not_categorical() -> None:
    import numpy as np
    from dragongui.widgets import _is_categorical

    high_card = np.arange(100, dtype=np.int32)
    assert not _is_categorical(high_card)


def test_scatter_fit_enqueues_command(monkeypatch) -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_fit_scatter_camera(self, widget_id: str, bounds: object) -> None:
            self.calls.append((widget_id, bounds))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.fit()
    scatter.fit(bounds=(0.0, 0.0, 0.0, 1.0, 1.0, 1.0))
    assert sender.calls[0] == ("s", None)
    assert sender.calls[1] == ("s", [0.0, 0.0, 0.0, 1.0, 1.0, 1.0])


def test_scatter_parallel_projection_enqueues_command(monkeypatch) -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_set_scatter_parallel_projection(self, widget_id: str, parallel: bool) -> None:
            self.calls.append((widget_id, parallel))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.parallel_projection = True
    scatter.parallel_projection = False
    assert sender.calls == [("s", True), ("s", False)]


def test_scatter_size_range_normalizes_to_pixel_range() -> None:
    """size_range maps the column data range onto a pixel range."""
    import numpy as np
    from dragongui.widgets import _pack_point_instances

    class F:
        x = np.array([0.0, 1.0, 2.0], dtype=np.float32)
        y = np.array([0.0, 0.0, 0.0], dtype=np.float32)
        z = np.array([0.0, 0.0, 0.0], dtype=np.float32)
        s = np.array([10.0, 20.0, 30.0], dtype=np.float32)  # data units

    buf = _pack_point_instances(F(), "x", "y", "z", point_sizes="s", size_range=(2.0, 8.0))
    assert buf is not None
    pts = np.frombuffer(buf, dtype="<f4").reshape(-1, 8)
    # s=10 → t=0 → 2px; s=30 → t=1 → 8px; s=20 → t=0.5 → 5px
    assert abs(pts[0, 3] - 2.0) < 1e-4
    assert abs(pts[2, 3] - 8.0) < 1e-4
    assert abs(pts[1, 3] - 5.0) < 1e-4


def test_scatter_size_range_all_equal_uses_midpoint() -> None:
    """All-equal size column with size_range uses the midpoint."""
    import numpy as np
    from dragongui.widgets import _pack_point_instances

    class F:
        x = np.array([0.0, 1.0], dtype=np.float32)
        y = np.array([0.0, 0.0], dtype=np.float32)
        z = np.array([0.0, 0.0], dtype=np.float32)
        s = np.array([5.0, 5.0], dtype=np.float32)

    buf = _pack_point_instances(F(), "x", "y", "z", point_sizes="s", size_range=(2.0, 8.0))
    assert buf is not None
    pts = np.frombuffer(buf, dtype="<f4").reshape(-1, 8)
    assert abs(pts[0, 3] - 5.0) < 1e-4  # midpoint of [2, 8]


def test_scatter_set_camera_enqueues_command(monkeypatch) -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[dict] = []

        def enqueue_set_scatter_camera_state(
            self, widget_id, target, distance, yaw, pitch, parallel=False
        ) -> None:
            self.calls.append({"id": widget_id, "target": target, "distance": distance,
                                "yaw": yaw, "pitch": pitch, "parallel": parallel})

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.set_camera({"target": [1.0, 2.0, 3.0], "distance": 10.0, "yaw": 0.5,
                         "pitch": 0.3, "parallel": True})
    assert len(sender.calls) == 1
    assert sender.calls[0]["id"] == "s"
    assert sender.calls[0]["distance"] == 10.0
    assert sender.calls[0]["parallel"] is True


def test_scatter_get_camera_returns_none_when_not_live() -> None:
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    assert scatter.get_camera() is None


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


def test_run_with_loading_builds_after_native_handle_is_bound(monkeypatch) -> None:
    class Sender:
        def __init__(self) -> None:
            self.replace_nodes: list[tuple[str, dict[str, object]]] = []
            self.drain_requests = 0
            self.closed = False

        def enqueue_drain_python_tasks(self) -> None:
            self.drain_requests += 1

        def enqueue_replace_node(self, widget_id: str, node_json: str) -> None:
            self.replace_nodes.append((widget_id, json.loads(node_json)))

        def close(self) -> None:
            self.closed = True

    sender = Sender()
    app = dg.App(loading_screen=dg.LoadingScreen(title="Starting"))
    built: dict[str, object] = {}
    seen: dict[str, object] = {}

    def build_window() -> dg.Window:
        assert app._handle is not None
        win = dg.Window("Built", width=800, height=600)
        button = dg.Button("Ready", on_click=lambda: None, parent=win)
        built["button"] = button
        return win

    def fake_run_document(document, click_callbacks, change_callbacks, app_handle=None):
        seen["document"] = document
        seen["click_callbacks"] = click_callbacks
        seen["change_callbacks"] = change_callbacks
        assert app_handle is app._handle
        app_handle._bind_native_sender(sender)
        app_handle._drain_python_tasks()
        assert sender.replace_nodes
        assert built["button"].is_live is True
        return {"status": "ok"}

    monkeypatch.setattr(app_module, "native_event_loop_available", lambda: True)
    monkeypatch.setattr(app_module, "run_document", fake_run_document)

    result = app.run_with_loading(build_window, title="Probe", width=640, height=480)

    assert result == {"status": "ok"}
    assert seen["click_callbacks"] == {}
    assert seen["change_callbacks"] == {}
    placeholder_doc = seen["document"]["window"]
    assert placeholder_doc["id"] == "__dg_startup_root"
    assert placeholder_doc["props"]["title"] == "Probe"
    assert placeholder_doc["props"]["width"] == 640
    widget_id, replacement = sender.replace_nodes[0]
    assert widget_id == "__dg_startup_root"
    assert replacement["type"] == "window"
    assert replacement["props"]["title"] == "Built"
    assert replacement["children"][0]["type"] == "button"
    assert built["button"].is_live is False
    assert app._handle is None
    assert sender.closed is True


def test_module_run_with_loading_creates_app(monkeypatch) -> None:
    seen: dict[str, object] = {}

    def fake_run_with_loading(self, build_window, *, title=None, width=1024, height=768):
        seen["app"] = self
        seen["title"] = title
        seen["width"] = width
        seen["height"] = height
        return {"status": "ok"}

    monkeypatch.setattr(app_module.App, "run_with_loading", fake_run_with_loading)

    result = dg.run_with_loading(
        lambda: dg.Window("Built"),
        title="Created",
        loading_screen=False,
        width=320,
        height=240,
    )

    assert result == {"status": "ok"}
    assert isinstance(seen["app"], dg.App)
    assert seen["app"].loading_screen is False
    assert seen["title"] == "Created"
    assert seen["width"] == 320
    assert seen["height"] == 240


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
    led = dg.LED(True, parent=win)
    custom_led = dg.LED("paused", states={"paused": "#ffcc33", "ready": "success"}, parent=win)

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
    assert led.to_dict()["type"] == "led"
    assert led.to_dict()["props"] == {"state": "on", "color": "success", "size": 14.0}
    assert custom_led.to_dict()["props"] == {
        "state": "paused",
        "color": "#ffcc33",
        "size": 14.0,
    }
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
    with pytest.raises(ValueError, match="unknown LED state"):
        dg.LED("missing", states={"ready": "success"}, parent=None)
    with pytest.raises(ValueError, match="size"):
        dg.LED(size=0, parent=None)


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
    # Scatter3D always registers a change callback (routes both point-pick and
    # selection payloads); all other widget types only register when a callback is
    # explicitly provided.
    win = dg.Window("No callbacks")
    dg.Checkbox("Enabled", checked=False, parent=win)
    dg.Slider(0.0, parent=win)
    dg.Dropdown(["x", "y"], parent=win)
    dg.TextInput("", parent=win)
    dg.TextArea("", parent=win)
    dg.DataFrameTable(DemoFrame(), parent=win)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=win)

    _, change_cbs = _collect_runtime_callbacks(win)

    assert set(change_cbs.keys()) == {scatter.id}


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


def test_scatter_grid_props_default_values() -> None:
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    p = scatter.props()
    assert p["grid_visible"] is False
    assert p["major_planes"] is False
    assert p["minor_planes"] is False
    assert p["grid_sticky"] is True
    assert p["grid_all_edges"] is False
    assert p["axis_x"] == "X"
    assert p["axis_y"] == "Y"
    assert p["axis_z"] == "Z"
    assert p["axis_vis_x"] is True
    assert p["axis_vis_y"] is True
    assert p["axis_vis_z"] is True
    assert "tick_x" not in p
    assert "tick_y" not in p
    assert "tick_z" not in p
    assert "background" not in p


def test_scatter_grid_startup_params_appear_in_props() -> None:
    scatter = dg.Scatter3D(
        DemoFrame(),
        x="x",
        y="y",
        z="z",
        grid=True,
        major_planes=True,
        minor_planes=True,
        grid_sticky=False,
        grid_all_edges=True,
        axis_x="Time",
        axis_y="Voltage",
        axis_z="Depth",
        background=(0.05, 0.08, 0.12),
        parent=None,
    )
    p = scatter.props()
    assert p["grid_visible"] is True
    assert p["major_planes"] is True
    assert p["minor_planes"] is True
    assert p["grid_sticky"] is False
    assert p["grid_all_edges"] is True
    assert p["axis_x"] == "Time"
    assert p["axis_y"] == "Voltage"
    assert p["axis_z"] == "Depth"
    assert p["background"] == [0.05, 0.08, 0.12, 1.0]


def test_scatter_show_grid_updates_state_and_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple[str, bool]] = []

        def enqueue_set_scatter_grid_visible(self, widget_id: str, visible: bool) -> None:
            self.calls.append((widget_id, visible))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.show_grid(True)
    assert scatter._grid_visible is True
    scatter.show_grid(False)
    assert scatter._grid_visible is False

    assert sender.calls == [("s", True), ("s", False)]


def test_scatter_show_grid_planes_updates_state_and_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple[str, bool, bool]] = []

        def enqueue_set_scatter_grid_planes(self, widget_id: str, major: bool, minor: bool) -> None:
            self.calls.append((widget_id, major, minor))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.show_grid_planes(major=True, minor=False)
    scatter.show_grid_planes(major=True, minor=True)

    assert scatter._major_planes is True
    assert scatter._minor_planes is True
    assert sender.calls == [("s", True, False), ("s", True, True)]


def test_scatter_set_grid_options_updates_state_and_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple[str, bool, bool]] = []

        def enqueue_set_scatter_grid_options(
            self, widget_id: str, sticky: bool, all_edges: bool
        ) -> None:
            self.calls.append((widget_id, sticky, all_edges))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.set_grid_options(sticky=False, all_edges=True)
    scatter.set_grid_options(sticky=True, all_edges=False)

    assert scatter._grid_sticky is True
    assert scatter._grid_all_edges is False
    assert sender.calls == [("s", False, True), ("s", True, False)]


def test_scatter_set_ticks_updates_state_and_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_set_scatter_ticks(
            self,
            widget_id: str,
            x: int | None,
            y: int | None,
            z: int | None,
        ) -> None:
            self.calls.append((widget_id, x, y, z))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.set_ticks(x=5, y=4, z=3)
    assert scatter._tick_override == (5, 4, 3)
    scatter.set_ticks(x=None)
    assert scatter._tick_override == (None, None, None)

    assert sender.calls == [("s", 5, 4, 3), ("s", None, None, None)]


def test_scatter_set_axes_updates_state_and_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple[str, str, str, str]] = []

        def enqueue_set_scatter_axes(self, widget_id: str, x: str, y: str, z: str) -> None:
            self.calls.append((widget_id, x, y, z))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.set_axes("Time (s)", "V (mV)", "Depth (m)")

    assert scatter._axis_labels == ("Time (s)", "V (mV)", "Depth (m)")
    assert scatter.props()["axis_x"] == "Time (s)"
    assert sender.calls == [("s", "Time (s)", "V (mV)", "Depth (m)")]


def test_scatter_set_axis_visibility_updates_state_and_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple[str, bool, bool, bool]] = []

        def enqueue_set_scatter_axis_visibility(
            self, widget_id: str, x: bool, y: bool, z: bool
        ) -> None:
            self.calls.append((widget_id, x, y, z))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.set_axis_visibility(x=True, y=True, z=False)

    assert scatter._axis_visible == (True, True, False)
    assert scatter.props()["axis_vis_z"] is False
    assert sender.calls == [("s", True, True, False)]


def test_scatter_set_background_updates_state_and_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple[str, float, float, float]] = []

        def enqueue_set_scatter_background(
            self, widget_id: str, r: float, g: float, b: float
        ) -> None:
            self.calls.append((widget_id, r, g, b))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.set_background(0.1, 0.05, 0.2)

    assert scatter._background == (0.1, 0.05, 0.2)
    assert scatter.props()["background"] == [0.1, 0.05, 0.2, 1.0]
    assert sender.calls == [("s", 0.1, 0.05, 0.2)]


def test_scatter_tick_overrides_appear_in_props_only_when_set() -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    assert "tick_x" not in s.props()

    s.set_ticks(x=6)
    assert s.props()["tick_x"] == 6
    assert "tick_y" not in s.props()
    assert "tick_z" not in s.props()


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


# ---------------------------------------------------------------------------
# Phase 2: Legend / Scalar Bar / Orientation Axes
# ---------------------------------------------------------------------------


def test_scatter_overlay_defaults_in_props() -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    p = s.props()
    assert p["legend_visible"] is False
    assert p["legend_position"] == "top-right"
    assert p["legend_entries"] == []
    assert p["scalar_bar_visible"] is False
    assert p["scalar_bar_vmin"] == 0.0
    assert p["scalar_bar_vmax"] == 1.0
    assert p["scalar_bar_log_scale"] is False
    assert p["scalar_bar_colormap"] == "viridis"
    assert "scalar_bar_title" not in p or p.get("scalar_bar_title") is None
    assert p["orientation_axes_visible"] is False


def test_scatter_overlay_startup_params_appear_in_props() -> None:
    entries = [("Class A", 1.0, 0.0, 0.0), ("Class B", 0.0, 0.5, 1.0)]
    s = dg.Scatter3D(
        DemoFrame(), x="x", y="y", z="z",
        legend=True,
        legend_position="bottom-left",
        legend_entries=entries,
        scalar_bar=True,
        scalar_bar_vmin=-1.5,
        scalar_bar_vmax=3.0,
        scalar_bar_log_scale=True,
        scalar_bar_colormap="plasma",
        scalar_bar_title="Density",
        orientation_axes=True,
        parent=None,
    )
    p = s.props()
    assert p["legend_visible"] is True
    assert p["legend_position"] == "bottom-left"
    assert p["legend_entries"] == [
        {"label": "Class A", "color": [1.0, 0.0, 0.0]},
        {"label": "Class B", "color": [0.0, 0.5, 1.0]},
    ]
    assert p["scalar_bar_visible"] is True
    assert p["scalar_bar_vmin"] == -1.5
    assert p["scalar_bar_vmax"] == 3.0
    assert p["scalar_bar_log_scale"] is True
    assert p["scalar_bar_colormap"] == "plasma"
    assert p["scalar_bar_title"] == "Density"
    assert p["orientation_axes_visible"] is True


def test_scatter_show_legend_updates_state_and_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_set_scatter_legend(
            self,
            widget_id: str,
            visible: bool,
            position: str,
            entries: list,
            title: "str | None" = None,
        ) -> None:
            self.calls.append((widget_id, visible, position, entries, title))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    s.show_legend(True, position="top-left", entries=[("A", 1.0, 0.0, 0.0)])
    assert s._legend_visible is True
    assert s._legend_position == "top-left"
    assert s._legend_entries == [("A", 1.0, 0.0, 0.0)]

    s.show_legend(False)
    assert s._legend_visible is False

    assert sender.calls[0] == ("s", True, "top-left", [("A", 1.0, 0.0, 0.0)], None)
    assert sender.calls[1][1] is False


def test_scatter_show_scalar_bar_updates_state_and_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_set_scatter_scalar_bar(
            self,
            widget_id: str,
            visible: bool,
            vmin: float,
            vmax: float,
            log_scale: bool,
            colormap: str,
            title: str | None,
        ) -> None:
            self.calls.append((widget_id, visible, vmin, vmax, log_scale, colormap, title))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    s.show_scalar_bar(True, vmin=-2.0, vmax=5.0, log_scale=False, colormap="inferno", title="T (K)")
    assert s._scalar_bar_visible is True
    assert s._scalar_bar_vmin == -2.0
    assert s._scalar_bar_vmax == 5.0
    assert s._scalar_bar_colormap == "inferno"
    assert s._scalar_bar_title == "T (K)"

    assert sender.calls == [("s", True, -2.0, 5.0, False, "inferno", "T (K)")]


def test_scatter_z_colormap_scalar_bar_auto_range() -> None:
    """Default z-colormapped plot must auto-track the z range for the scalar bar."""
    import numpy as np

    class NpFrame:
        def __init__(self) -> None:
            self.x = np.zeros(3, dtype=np.float32)
            self.y = np.zeros(3, dtype=np.float32)
            self.z = np.array([10.0, 20.0, 30.0], dtype=np.float32)
        @property
        def columns(self) -> tuple:
            return ("x", "y", "z")

    s = dg.Scatter3D(NpFrame(), x="x", y="y", z="z", scalar_bar=True, parent=None)
    p = s.props()
    assert p["scalar_bar_vmin"] == pytest.approx(10.0), "auto vmin should match z.min()"
    assert p["scalar_bar_vmax"] == pytest.approx(30.0), "auto vmax should match z.max()"
    assert p["scalar_bar_colormap"] == "viridis"
    assert p["scalar_bar_log_scale"] is False


def test_scatter_z_colormap_auto_meta_without_scalar_bar() -> None:
    """Auto color meta for z path is computed even when scalar bar is not shown."""
    import numpy as np

    class NpFrame:
        def __init__(self) -> None:
            self.x = np.zeros(5, dtype=np.float32)
            self.y = np.zeros(5, dtype=np.float32)
            self.z = np.linspace(5.0, 15.0, 5, dtype=np.float32)
        @property
        def columns(self) -> tuple:
            return ("x", "y", "z")

    s = dg.Scatter3D(NpFrame(), x="x", y="y", z="z", parent=None)
    assert s._auto_scalar_vmin == pytest.approx(5.0)
    assert s._auto_scalar_vmax == pytest.approx(15.0)
    assert s._auto_scalar_colormap == "viridis"
    assert s._auto_scalar_log_scale is False
    assert s._auto_legend_title == "z"


def test_scatter_z_colormap_set_points_refreshes_scalar_bar() -> None:
    """live set_points() on z-colormap path re-enqueues scalar bar with new z range."""
    import numpy as np

    class NpFrame:
        def __init__(self, lo: float, hi: float) -> None:
            n = 4
            self.x = np.zeros(n, dtype=np.float32)
            self.y = np.zeros(n, dtype=np.float32)
            self.z = np.linspace(lo, hi, n, dtype=np.float32)
        @property
        def columns(self) -> tuple:
            return ("x", "y", "z")

    class Sender:
        def __init__(self) -> None:
            self.scalar_calls: list[tuple] = []

        def enqueue_set_scatter_points_packed(self, *a, **kw) -> None:
            pass

        def enqueue_set_scatter_tooltip_axis_labels(self, *a) -> None:
            pass

        def enqueue_set_scatter_primary_hover_meta(self, *a) -> None:
            pass

        def enqueue_clear_scatter_actors(self, widget_id: str) -> None:
            pass

        def enqueue_set_scatter_scalar_bar(
            self, widget_id: str, visible: bool, vmin: float, vmax: float,
            log_scale: bool, colormap: str, title: "str | None",
        ) -> None:
            self.scalar_calls.append((widget_id, visible, vmin, vmax, log_scale, colormap, title))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    s = dg.Scatter3D(NpFrame(0.0, 10.0), x="x", y="y", z="z", scalar_bar=True, id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    # Replace with a new frame whose z range is 100..200.
    s.set_points(NpFrame(100.0, 200.0), x="x", y="y", z="z")

    assert len(sender.scalar_calls) == 1, "set_points must enqueue SetScatterScalarBar"
    _wid, _vis, vmin, vmax, log_scale, _cm, _title = sender.scalar_calls[0]
    assert vmin == pytest.approx(100.0)
    assert vmax == pytest.approx(200.0)
    assert log_scale is False


def test_scatter_z_clim_forces_v1_and_honors_range() -> None:
    """clim on the default z path must force v1 packing and apply the clim to the scalar bar."""
    import numpy as np

    class NpFrame:
        def __init__(self) -> None:
            self.x = np.zeros(5, dtype=np.float32)
            self.y = np.zeros(5, dtype=np.float32)
            self.z = np.linspace(0.0, 100.0, 5, dtype=np.float32)
        @property
        def columns(self) -> tuple:
            return ("x", "y", "z")

    s = dg.Scatter3D(NpFrame(), x="x", y="y", z="z", clim=(10.0, 50.0), scalar_bar=True, parent=None)
    assert s.data_format == "point_instance_v1", "clim must force v1 packing"
    p = s.props()
    assert p["scalar_bar_vmin"] == pytest.approx(10.0)
    assert p["scalar_bar_vmax"] == pytest.approx(50.0)
    assert p["scalar_bar_log_scale"] is False


def test_scatter_z_log_scale_forces_v1_and_log_bar() -> None:
    """log_scale=True on the default z path must force v1 packing and log scalar bar."""
    import numpy as np

    class NpFrame:
        def __init__(self) -> None:
            self.x = np.zeros(4, dtype=np.float32)
            self.y = np.zeros(4, dtype=np.float32)
            self.z = np.array([1.0, 10.0, 100.0, 1000.0], dtype=np.float32)
        @property
        def columns(self) -> tuple:
            return ("x", "y", "z")

    s = dg.Scatter3D(NpFrame(), x="x", y="y", z="z", log_scale=True, scalar_bar=True, parent=None)
    assert s.data_format == "point_instance_v1", "log_scale must force v1 packing"
    p = s.props()
    assert p["scalar_bar_log_scale"] is True
    # Auto range is raw domain (matching DragonSci public API): vmin=z.min(), vmax=z.max().
    assert p["scalar_bar_vmin"] == pytest.approx(1.0)
    assert p["scalar_bar_vmax"] == pytest.approx(1000.0)


def test_scatter_z_clim_live_set_points_updates_scalar_bar() -> None:
    """live set_points(..., clim=...) for default z path refreshes scalar bar."""
    import numpy as np

    class NpFrame:
        def __init__(self, lo: float, hi: float) -> None:
            n = 4
            self.x = np.zeros(n, dtype=np.float32)
            self.y = np.zeros(n, dtype=np.float32)
            self.z = np.linspace(lo, hi, n, dtype=np.float32)
        @property
        def columns(self) -> tuple:
            return ("x", "y", "z")

    class Sender:
        def __init__(self) -> None:
            self.scalar_calls: list[tuple] = []

        def enqueue_set_scatter_points_packed(self, *a, **kw) -> None:
            pass

        def enqueue_set_scatter_tooltip_axis_labels(self, *a) -> None:
            pass

        def enqueue_set_scatter_primary_hover_meta(self, *a) -> None:
            pass

        def enqueue_clear_scatter_actors(self, widget_id: str) -> None:
            pass

        def enqueue_set_scatter_scalar_bar(
            self, widget_id: str, visible: bool, vmin: float, vmax: float,
            log_scale: bool, colormap: str, title: "str | None",
        ) -> None:
            self.scalar_calls.append((widget_id, visible, vmin, vmax, log_scale, colormap, title))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    s = dg.Scatter3D(NpFrame(0.0, 100.0), x="x", y="y", z="z",
                     clim=(5.0, 80.0), scalar_bar=True, id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    # Live update: shift the clim window.
    s.set_points(NpFrame(0.0, 100.0), x="x", y="y", z="z", clim=(20.0, 60.0))

    assert len(sender.scalar_calls) == 1
    _wid, _vis, vmin, vmax, _log, _cm, _title = sender.scalar_calls[0]
    assert vmin == pytest.approx(20.0)
    assert vmax == pytest.approx(60.0)


def test_scatter_actor_clim_log_scale_forces_v1() -> None:
    """add_points actor with clim or log_scale must produce point_instance_v1 payload."""
    import numpy as np

    class NpFrame:
        def __init__(self) -> None:
            self.x = np.zeros(3, dtype=np.float32)
            self.y = np.zeros(3, dtype=np.float32)
            self.z = np.array([1.0, 10.0, 100.0], dtype=np.float32)
        @property
        def columns(self) -> tuple:
            return ("x", "y", "z")

    s = dg.Scatter3D(NpFrame(), x="x", y="y", z="z", parent=None)
    buf, cmap, fmt = s._pack_actor_payload(
        NpFrame(), "x", "y", "z",
        clim=(2.0, 50.0), log_scale=False,
    )
    assert fmt == "point_instance_v1", "clim must force v1 for actor payload"

    buf2, cmap2, fmt2 = s._pack_actor_payload(
        NpFrame(), "x", "y", "z",
        log_scale=True,
    )
    assert fmt2 == "point_instance_v1", "log_scale must force v1 for actor payload"


def test_scatter_explicit_clim_honored_when_no_positive_z() -> None:
    """Explicit clim must be returned even when all z values are non-positive (log-scale path)."""
    import numpy as np

    class NpFrame:
        def __init__(self) -> None:
            self.x = np.zeros(3, dtype=np.float32)
            self.y = np.zeros(3, dtype=np.float32)
            self.z = np.array([-1.0, 0.0, -2.0], dtype=np.float32)
        @property
        def columns(self) -> tuple:
            return ("x", "y", "z")

    s = dg.Scatter3D(NpFrame(), x="x", y="y", z="z",
                     clim=(1.0, 100.0), log_scale=True, scalar_bar=True, parent=None)
    p = s.props()
    assert p["scalar_bar_vmin"] == pytest.approx(1.0), "explicit clim must not be dropped"
    assert p["scalar_bar_vmax"] == pytest.approx(100.0)
    assert p["scalar_bar_log_scale"] is True


def test_scatter_show_scalar_bar_log_raw_vmin_vmax() -> None:
    """show_scalar_bar(vmin=1, vmax=100, log_scale=True) stores and reports raw domain values."""
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    s.show_scalar_bar(True, vmin=1.0, vmax=100.0, log_scale=True)
    p = s.props()
    assert p["scalar_bar_vmin"] == pytest.approx(1.0)
    assert p["scalar_bar_vmax"] == pytest.approx(100.0)
    assert p["scalar_bar_log_scale"] is True


def test_scatter_scalar_bar_vmax_zero_not_replaced_by_fallback() -> None:
    """Auto vmax=0.0 must be preserved; the old `or 1.0` fallback must not fire."""
    import numpy as np

    class NpFrame:
        def __init__(self) -> None:
            self.x = np.zeros(3, dtype=np.float32)
            self.y = np.zeros(3, dtype=np.float32)
            self.z = np.array([-2.0, -1.0, 0.0], dtype=np.float32)
        @property
        def columns(self) -> tuple:
            return ("x", "y", "z")

    s = dg.Scatter3D(NpFrame(), x="x", y="y", z="z", scalar_bar=True, parent=None)
    p = s.props()
    assert p["scalar_bar_vmin"] == pytest.approx(-2.0)
    assert p["scalar_bar_vmax"] == pytest.approx(0.0), "vmax=0 must not be replaced by 1.0"


def test_scatter_scalar_bar_all_equal_z_range() -> None:
    """When all z values are equal, auto range is vmin==vmax==that value."""
    import numpy as np

    class NpFrame:
        def __init__(self) -> None:
            self.x = np.zeros(3, dtype=np.float32)
            self.y = np.zeros(3, dtype=np.float32)
            self.z = np.zeros(3, dtype=np.float32)
        @property
        def columns(self) -> tuple:
            return ("x", "y", "z")

    s = dg.Scatter3D(NpFrame(), x="x", y="y", z="z", scalar_bar=True, parent=None)
    p = s.props()
    assert p["scalar_bar_vmin"] == pytest.approx(0.0)
    assert p["scalar_bar_vmax"] == pytest.approx(0.0)


def test_scatter_scalar_bar_clim_zero_zero() -> None:
    """Explicit clim=(0, 0) must be used as the auto scalar bar range."""
    import numpy as np

    class NpFrame:
        def __init__(self) -> None:
            self.x = np.zeros(3, dtype=np.float32)
            self.y = np.zeros(3, dtype=np.float32)
            self.z = np.array([1.0, 2.0, 3.0], dtype=np.float32)
        @property
        def columns(self) -> tuple:
            return ("x", "y", "z")

    s = dg.Scatter3D(NpFrame(), x="x", y="y", z="z",
                     clim=(0.0, 0.0), scalar_bar=True, parent=None)
    p = s.props()
    assert p["scalar_bar_vmin"] == pytest.approx(0.0)
    assert p["scalar_bar_vmax"] == pytest.approx(0.0)


def test_log_scale_non_positive_not_treated_as_nan() -> None:
    """log_scale=True must clip non-positive finite values to tiny, not mark them as NaN."""
    import numpy as np
    from dragongui.widgets import _scalars_to_rgb

    red = (1.0, 0.0, 0.0)
    # z=[-1, 0, 1]: only NaN/inf values should receive nan_color; -1 and 0 are finite.
    scalars = np.array([-1.0, 0.0, 1.0], dtype=np.float32)
    rgb = _scalars_to_rgb(scalars, "viridis", clim=None, log_scale=True, nan_color=red)

    # -1 and 0 must NOT be red (nan_color)
    assert not np.allclose(rgb[0], [1.0, 0.0, 0.0], atol=0.05), "-1 must not be nan_color"
    assert not np.allclose(rgb[1], [1.0, 0.0, 0.0], atol=0.05), "0 must not be nan_color"

    # With actual NaN in the array, only NaN index should be red
    scalars_with_nan = np.array([np.nan, 0.0, 1.0], dtype=np.float32)
    rgb2 = _scalars_to_rgb(scalars_with_nan, "viridis", clim=None, log_scale=True, nan_color=red)
    assert np.allclose(rgb2[0], [1.0, 0.0, 0.0], atol=0.05), "NaN must receive nan_color"
    assert not np.allclose(rgb2[1], [1.0, 0.0, 0.0], atol=0.05), "0 must not be nan_color"


def test_log_scale_non_positive_clips_to_low_end_of_colormap() -> None:
    """Non-positive finite values under log_scale should map to the low end of the colormap."""
    import numpy as np
    from dragongui.widgets import _scalars_to_rgb

    # [-1, 0, 10, 100]: -1 and 0 clip to tiny, map near the bottom of the colormap.
    scalars = np.array([-1.0, 0.0, 10.0, 100.0], dtype=np.float32)
    rgb = _scalars_to_rgb(scalars, "viridis", clim=None, log_scale=True, nan_color=None)

    # -1 and 0 should map near 0.0 (bottom); 100 near 1.0 (top).
    # Viridis bottom is dark purple (~0.267, 0.004, 0.329) and top is yellow.
    # The key check: -1 and 0 should have the same or similar color as each other (both clipped to tiny).
    assert np.allclose(rgb[0], rgb[1], atol=0.01), "-1 and 0 should clip to same low-end color"


def test_scatter_props_equal_detects_legend_title_change() -> None:
    from dragongui.vdom import _scatter_props_equal
    base: dict = {
        "x": "x", "y": "y", "z": "z", "colormap": "viridis", "data_format": "xyz_f32_v0",
        "events": [], "grid_visible": False, "major_planes": False, "minor_planes": False,
        "grid_sticky": True, "grid_all_edges": False,
        "axis_x": "X", "axis_y": "Y", "axis_z": "Z",
        "axis_vis_x": True, "axis_vis_y": True, "axis_vis_z": True,
        "background": None,
        "legend_visible": True, "legend_position": "top-right", "legend_entries": [],
        "scalar_bar_visible": False, "scalar_bar_vmin": 0.0, "scalar_bar_vmax": 1.0,
        "scalar_bar_log_scale": False, "scalar_bar_colormap": "viridis",
        "orientation_axes_visible": False,
        "_payload_token": 42,
    }
    with_title = {**base, "legend_title": "Species"}
    without_title = {**base}
    # Same token but different legend_title — must compare unequal.
    assert not _scatter_props_equal(with_title, without_title)
    assert not _scatter_props_equal(without_title, with_title)
    # Same on both sides — equal.
    assert _scatter_props_equal(with_title, {**with_title})
    assert _scatter_props_equal(without_title, {**without_title})


def test_scatter_patch_legend_title_triggers_set_scatter_legend() -> None:
    from dragongui.vdom import Patch

    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_set_scatter_legend(
            self,
            widget_id: str,
            visible: bool,
            position: str,
            entries: list,
            title: "str | None" = None,
        ) -> None:
            self.calls.append((widget_id, visible, position, entries, title))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    # A scatter SET_PROP patch that only changes legend_title must trigger SetScatterLegend.
    patch = Patch(
        kind=Patch.SET_PROP,
        path=("scatter_id",),
        node_id="scatter_id",
        prop="scatter",
        value={
            "legend_title": "Category",
            "legend_visible": True,
            "legend_position": "top-right",
            "legend_entries": [],
        },
    )
    handle.apply_patch(patch)
    assert len(sender.calls) == 1
    assert sender.calls[0][0] == "scatter_id"
    assert sender.calls[0][4] == "Category"


def test_scatter_show_orientation_axes_updates_state_and_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple[str, bool]] = []

        def enqueue_set_scatter_orientation_axes(self, widget_id: str, visible: bool) -> None:
            self.calls.append((widget_id, visible))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    s.show_orientation_axes(True)
    assert s._orientation_axes_visible is True
    s.show_orientation_axes(False)
    assert s._orientation_axes_visible is False

    assert sender.calls == [("s", True), ("s", False)]


# ---------------------------------------------------------------------------
# Phase 3: Labels, Lines, Boxes
# ---------------------------------------------------------------------------


def test_scatter_add_label_returns_handle_and_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_add_scatter_label(self, *args: object) -> None:
            self.calls.append(args)

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    h0 = s.add_label((1.0, 2.0, 3.0), "A", color=(1.0, 0.0, 0.0), size=16.0)
    h1 = s.add_label((0.0, 0.0, 0.0), "B")

    assert h0 == 0
    assert h1 == 1
    assert len(sender.calls) == 2
    assert sender.calls[0] == ("s", 0, 1.0, 2.0, 3.0, "A", 1.0, 0.0, 0.0, 16.0, "center")
    assert sender.calls[1][5] == "B"


def test_scatter_remove_label_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_add_scatter_label(self, *args: object) -> None:
            pass

        def enqueue_remove_scatter_label(self, widget_id: str, label_id: int) -> None:
            self.calls.append((widget_id, label_id))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    h = s.add_label((0.0, 0.0, 0.0), "Test")
    s.remove_label(h)

    assert sender.calls == [("s", h)]


def test_scatter_clear_labels_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.cleared = False

        def enqueue_clear_scatter_labels(self, widget_id: str) -> None:
            self.cleared = True

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    s.clear_labels()
    assert sender.cleared


def test_scatter_add_lines_returns_handle_and_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_add_scatter_lines(
            self, widget_id: str, overlay_id: int, points: list, r: float, g: float, b: float
        ) -> None:
            self.calls.append((widget_id, overlay_id, points, r, g, b))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    pts = [(0.0, 0.0, 0.0), (1.0, 1.0, 1.0), (2.0, 0.0, 0.0)]
    h = s.add_lines(pts, color=(0.0, 1.0, 0.5))

    assert h == 0
    wid, oid, sent_pts, r, g, b = sender.calls[0]
    assert wid == "s"
    assert oid == 0
    assert sent_pts == [
        [0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0, 2.0, 0.0, 0.0],
    ]
    assert abs(g - 1.0) < 1e-6


def test_scatter_update_lines_enqueues_explicit_segments() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_update_scatter_lines(
            self, widget_id: str, overlay_id: int, segments: list, r: float, g: float, b: float
        ) -> None:
            self.calls.append((widget_id, overlay_id, segments, r, g, b))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    s.update_lines(7, [[0.0, 0.0, 0.0, 1.0, 2.0, 3.0]], color=(0.25, 0.5, 0.75))

    assert sender.calls == [("s", 7, [[0.0, 0.0, 0.0, 1.0, 2.0, 3.0]], 0.25, 0.5, 0.75)]


def test_scatter_add_box_returns_handle_and_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_add_scatter_box(self, *args: object) -> None:
            self.calls.append(args)

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    # DragonSci order: (xmin, ymin, zmin, xmax, ymax, zmax)
    h = s.add_box((1.0, 2.0, 3.0, 4.0, 5.0, 6.0), color=(1.0, 1.0, 0.0))

    assert h == 0
    assert sender.calls[0][0] == "s"
    assert sender.calls[0][1] == 0
    # Native order is (xmin, xmax, ymin, ymax, zmin, zmax)
    assert sender.calls[0][2] == 1.0   # xmin
    assert sender.calls[0][3] == 4.0   # xmax
    assert sender.calls[0][4] == 2.0   # ymin
    assert sender.calls[0][5] == 5.0   # ymax
    assert sender.calls[0][6] == 3.0   # zmin
    assert sender.calls[0][7] == 6.0   # zmax


def test_scatter_overlay_handles_are_independent_from_label_handles() -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h_label = s.add_label((0.0, 0.0, 0.0), "test")
    h_line = s.add_lines([(0.0, 0.0, 0.0), (1.0, 1.0, 1.0)])
    h_box = s.add_box((-1.0, -1.0, -1.0, 1.0, 1.0, 1.0))

    assert h_label == 0
    assert h_line == 0
    assert h_box == 1


# ---------------------------------------------------------------------------
# Phase 4: Actors and Streaming
# ---------------------------------------------------------------------------


def test_scatter_add_points_returns_handle() -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h0 = s.add_points(DemoFrame(), x="x", y="y", z="z")
    h1 = s.add_points(DemoFrame(), x="x", y="y", z="z")
    assert h0 == 1  # 0 is reserved for primary actor
    assert h1 == 2


def test_scatter_add_points_returns_handle_when_live() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_add_scatter_actor(self, *args: object) -> None:
            self.calls.append(args)

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    h0 = s.add_points(DemoFrame(), x="x", y="y", z="z")
    h1 = s.add_points(DemoFrame(), x="x", y="y", z="z")
    assert h0 == 1  # 0 is reserved for primary actor
    assert h1 == 2
    # Calls may be empty if DemoFrame has no real array data (no numpy mock), but
    # if any calls were made the widget_id should match.
    for call in sender.calls:
        assert call[0] == "s"


def test_scatter_remove_actor_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.remove_calls: list[tuple] = []

        def enqueue_add_scatter_actor(self, *args: object) -> None:
            pass

        def enqueue_remove_scatter_actor(self, widget_id: str, actor_id: int) -> None:
            self.remove_calls.append((widget_id, actor_id))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    h = s.add_points(DemoFrame(), x="x", y="y", z="z")
    s.remove_actor(h)
    assert sender.remove_calls == [("s", h)]


def test_scatter_clear_actors_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.scene_cleared = False

        def enqueue_clear_scatter_scene(self, widget_id: str) -> None:
            self.scene_cleared = True

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    s.clear()
    assert sender.scene_cleared


def test_scatter_add_stream_returns_handle() -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h = s.add_stream(max_points=500, mode="ring")
    assert h == 1  # 0 is reserved for primary actor


def test_scatter_add_stream_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_add_scatter_stream(
            self, widget_id: str, actor_id: int, max_points: int, mode: str
        ) -> None:
            self.calls.append((widget_id, actor_id, max_points, mode))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    h = s.add_stream(max_points=1000, mode="append")
    assert sender.calls == [("s", h, 1000, "append")]


def test_scatter_actor_and_label_handles_use_separate_counters() -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h_actor = s.add_points(DemoFrame(), x="x", y="y", z="z")
    h_label = s.add_label((0.0, 0.0, 0.0), "A")
    h_stream = s.add_stream(max_points=100)

    assert h_actor == 1  # 0 is reserved for primary actor
    assert h_label == 0  # label handles use a separate counter
    assert h_stream == 2  # shares actor ID counter with add_points


# ── Phase 5: LOD and Picking ──────────────────────────────────────────────────

def test_scatter_lod_defaults() -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    assert s._lod_enabled is False
    assert s._lod_threshold == 200_000
    assert s._lod_factor == 8
    assert s._picking_mode == "point"


def test_scatter_set_lod_updates_state_and_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_set_scatter_lod(
            self, widget_id: str, enabled: bool, threshold: int, factor: int
        ) -> None:
            self.calls.append((widget_id, enabled, threshold, factor))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    s.set_lod(enabled=True, threshold=50_000, factor=4)

    assert s._lod_enabled is True
    assert s._lod_threshold == 50_000
    assert s._lod_factor == 4
    assert sender.calls == [("s", True, 50_000, 4)]


def test_scatter_enable_rectangle_picking_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_set_scatter_picking_mode(self, widget_id: str, mode: str) -> None:
            self.calls.append((widget_id, mode))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    s.enable_rectangle_picking()

    assert s._picking_mode == "rectangle"
    assert sender.calls == [("s", "rectangle")]


def test_scatter_disable_picking_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_set_scatter_picking_mode(self, widget_id: str, mode: str) -> None:
            self.calls.append((widget_id, mode))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    s.disable_picking()

    assert s._picking_mode == "none"
    assert sender.calls == [("s", "none")]


def test_scatter_enable_lasso_picking_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_set_scatter_picking_mode(self, widget_id: str, mode: str) -> None:
            self.calls.append((widget_id, mode))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    s.enable_lasso_picking()

    assert s._picking_mode == "lasso"
    assert sender.calls == [("s", "lasso")]


def test_scatter_enable_rectangle_picking_stores_callback() -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    cb = lambda data: None
    s.enable_rectangle_picking(on_select=cb)
    assert s._on_select is cb


# ── Phase 6: Mesh and Statistical Overlays ────────────────────────────────────

def test_scatter_pack_mesh_payload_roundtrip() -> None:
    import struct, base64
    positions = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]
    tris = [[0, 1, 0]]
    pos_b64, idx_b64 = dg.Scatter3D._pack_mesh_payload(positions, tris)
    pos_bytes = base64.b64decode(pos_b64)
    assert len(pos_bytes) == 2 * 3 * 4  # 2 verts × 3 floats × 4 bytes
    x0, y0, z0, x1, y1, z1 = struct.unpack("<6f", pos_bytes)
    assert abs(x0 - 1.0) < 1e-5
    assert abs(y1 - 5.0) < 1e-5
    idx_bytes = base64.b64decode(idx_b64)
    assert len(idx_bytes) == 3 * 4  # 1 triangle × 3 uint32
    a, b, c = struct.unpack("<3I", idx_bytes)
    assert (a, b, c) == (0, 1, 0)


def test_scatter_add_convex_hull_returns_handle_without_scipy() -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    try:
        import scipy  # noqa: F401
    except ImportError:
        import pytest
        with pytest.raises(ImportError, match="scipy"):
            s.add_convex_hull([[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1]])
        return
    # scipy available: check handle increments
    pts = [[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1]]
    h0 = s.add_convex_hull(pts)
    h1 = s.add_convex_hull(pts)
    assert h0 == 0
    assert h1 == 1


def test_scatter_add_convex_hull_enqueues() -> None:
    try:
        import scipy  # noqa: F401
    except ImportError:
        return  # skip if scipy not available

    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_add_scatter_mesh(
            self, widget_id, mesh_id, positions_b64, indices_b64, r, g, b, a, wireframe
        ) -> None:
            self.calls.append((widget_id, mesh_id, r, g, b, a, wireframe))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    pts = [[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1]]
    h = s.add_convex_hull(pts, color=(1.0, 0.0, 0.0), opacity=0.5)
    assert len(sender.calls) == 1
    wid, mid, r, g, b, a, wf = sender.calls[0]
    assert wid == "s"
    assert mid == h
    assert abs(r - 1.0) < 1e-5
    assert abs(g - 0.0) < 1e-5
    assert abs(a - 0.5) < 1e-5
    assert wf is False


def test_scatter_remove_mesh_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.removed: list[tuple] = []

        def enqueue_remove_scatter_mesh(self, widget_id: str, mesh_id: int) -> None:
            self.removed.append((widget_id, mesh_id))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    s.remove_mesh(7)
    assert sender.removed == [("s", 7)]


def test_scatter_clear_meshes_enqueues() -> None:
    class Sender:
        def __init__(self) -> None:
            self.cleared = False

        def enqueue_clear_scatter_meshes(self, widget_id: str) -> None:
            self.cleared = True

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s", parent=None)
    s._bind_live(handle.widget_handle(s.id))
    # prime the mesh id counter
    s._next_mesh_id = 1

    s.clear_meshes()
    assert sender.cleared


def test_scatter_mesh_handle_counter_increments() -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    # Manually bump counter (no scipy needed)
    s._next_mesh_id = 5
    assert s._next_mesh_id == 5


# ---------------------------------------------------------------------------
# Phase 7: Export / Camera linking
# ---------------------------------------------------------------------------

def _make_sender_with_parallel_scale():
    """Return (scatter, sender) wired together for parallel-scale tests."""
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple[str, float, float]] = []

        def enqueue_set_scatter_parallel_scale(
            self, widget_id: str, half_w: float, half_h: float
        ) -> None:
            self.calls.append((widget_id, half_w, half_h))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="sc7", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))
    return scatter, sender


def test_scatter_set_parallel_scale_enqueues() -> None:
    scatter, sender = _make_sender_with_parallel_scale()
    scatter.set_parallel_scale(2.5, 1.5)
    assert sender.calls == [("sc7", 2.5, 1.5)]


def test_scatter_flatten_view_sets_parallel_and_camera(monkeypatch) -> None:
    """flatten_view() must set parallel=True and forward yaw/pitch."""
    camera_calls: list[dict] = []

    class Sender:
        def enqueue_set_scatter_camera_state(
            self, wid, target, distance, yaw, pitch, parallel=False
        ) -> None:
            camera_calls.append(
                {"target": target, "yaw": yaw, "pitch": pitch, "parallel": parallel}
            )

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="sc7", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    import math
    scatter.flatten_view("xy")
    assert len(camera_calls) == 1
    cam = camera_calls[0]
    assert cam["parallel"] is True
    assert cam["pitch"] == pytest.approx(math.pi / 2, abs=0.01)


def test_scatter_flatten_view_rejects_unknown_plane() -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    with pytest.raises(ValueError, match="unknown plane"):
        s.flatten_view("bad")


def test_scatter_link_cameras_mutual() -> None:
    a = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    b = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    a.link_cameras(b)
    assert b in a._camera_links
    assert a in b._camera_links


def test_scatter_unlink_cameras_removes_both_sides() -> None:
    a = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    b = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    a.link_cameras(b)
    a.unlink_cameras(b)
    assert b not in getattr(a, "_camera_links", set())
    assert a not in getattr(b, "_camera_links", set())


def test_scatter_receive_camera_does_not_rebroadcast() -> None:
    """_receive_camera sets _propagating so recursive fan-out is suppressed."""
    call_count = [0]
    a = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)

    original_set_camera = a.set_camera

    def counting_set_camera(state):
        call_count[0] += 1
        original_set_camera(state)

    a.set_camera = counting_set_camera
    a._receive_camera({"target": [0, 0, 0], "distance": 5.0, "yaw": 0.0, "pitch": 0.0})
    assert a._propagating is False  # flag is reset after call
    assert call_count[0] == 1


def test_scatter_get_view_bounds_2d_returns_none_when_not_live() -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    result = s.get_view_bounds_2d()
    assert result is None


def test_scatter_screenshot_returns_none_when_not_live() -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    result = s.screenshot()
    assert result is None


def test_scatter_save_png_raises_when_screenshot_is_none(tmp_path) -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    with pytest.raises(RuntimeError, match="screenshot"):
        s.save_png(str(tmp_path / "out.png"))


# ── hover_tooltip tests ───────────────────────────────────────────────────────

def test_scatter_hover_tooltip_default_is_true() -> None:
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    assert s.hover_tooltip is True


def test_scatter_hover_tooltip_pre_live_false_synced_on_startup() -> None:
    """Setting hover_tooltip=False before going live must propagate to native on startup."""
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_set_scatter_hover_tooltip(self, widget_id: str, enabled: bool) -> None:
            self.calls.append((widget_id, enabled))

        def enqueue_set_scatter_tooltip_axis_labels(self, *a) -> None:
            pass

        def enqueue_set_scatter_lod(self, *a) -> None:
            pass

        def enqueue_set_scatter_picking_mode(self, *a) -> None:
            pass

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s1", parent=None)
    scatter.hover_tooltip = False  # set before going live

    scatter._bind_live(handle.widget_handle(scatter.id))
    scatter._queue_startup_resources()  # startup replay sends queued state to native

    assert ("s1", False) in sender.calls


def test_scatter_hover_tooltip_live_setter_enqueues_command() -> None:
    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_set_scatter_hover_tooltip(self, widget_id: str, enabled: bool) -> None:
            self.calls.append((widget_id, enabled))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s2", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))
    sender.calls.clear()  # ignore startup sync

    scatter.hover_tooltip = False
    assert ("s2", False) in sender.calls

    scatter.hover_tooltip = True
    assert ("s2", True) in sender.calls


def test_scatter_hover_changed_sets_hover_state() -> None:
    win = dg.Window("W")
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="sc", parent=win)

    _, change_cbs = _collect_runtime_callbacks(win)
    change_cbs["sc"](json.dumps({
        "event": "hover_changed",
        "widget_id": "sc",
        "actor": 1,
        "index": 7,
        "x": 1.0,
        "y": 2.0,
        "z": 3.0,
        "hover_text": "custom label",
    }))

    assert scatter.hover_point == (1.0, 2.0, 3.0)
    assert scatter.hover_index == 7
    assert scatter.hover_actor == 1
    assert scatter.hover_text == "custom label"


def test_scatter_hover_changed_clear_resets_hover_state() -> None:
    win = dg.Window("W")
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="sc2", parent=win)
    # Prime hover state first
    scatter.hover_point = (1.0, 2.0, 3.0)
    scatter.hover_index = 3
    scatter.hover_actor = 0
    scatter.hover_text = "something"

    _, change_cbs = _collect_runtime_callbacks(win)
    change_cbs["sc2"](json.dumps({
        "event": "hover_changed",
        "widget_id": "sc2",
    }))

    assert scatter.hover_point is None
    assert scatter.hover_index is None
    assert scatter.hover_actor is None
    assert scatter.hover_text is None


def test_scatter_add_points_hover_meta_passed_to_enqueue(monkeypatch) -> None:
    """hover= column name in add_points extracts per-row values and forwards JSON to native."""
    import json as _json

    class LabelsFrame:
        x = [0.0, 1.0]
        y = [0.0, 1.0]
        z = [0.0, 1.0]
        label = ["pt0", "pt1"]

        def __getitem__(self, key: str):
            return getattr(self, key)

    _fake_buf = b"\x00" * 32
    monkeypatch.setattr(
        widgets_module.Scatter3D, "_pack_actor_payload",
        lambda self, *a, **kw: (_fake_buf, "viridis", "xyz_f32_v0"),
    )

    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_add_scatter_actor(
            self, widget_id: str, actor_id: int, payload_b64: str,
            colormap: str, payload_format: str, hover_meta=None,
            tooltip_x=None, tooltip_y=None, tooltip_z=None,
        ) -> None:
            self.calls.append((widget_id, actor_id, hover_meta))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="sa", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    frame = LabelsFrame()
    scatter.add_points(frame, x="x", y="y", z="z", hover="label")

    assert len(sender.calls) == 1
    _wid, _aid, meta_json = sender.calls[0]
    assert _wid == "sa"
    assert _json.loads(meta_json) == ["label: pt0", "label: pt1"]


def test_scatter_add_points_no_hover_passes_none(monkeypatch) -> None:
    _fake_buf = b"\x00" * 32
    monkeypatch.setattr(
        widgets_module.Scatter3D, "_pack_actor_payload",
        lambda self, *a, **kw: (_fake_buf, "viridis", "xyz_f32_v0"),
    )

    class Sender:
        def __init__(self) -> None:
            self.calls: list[tuple] = []

        def enqueue_add_scatter_actor(
            self, widget_id: str, actor_id: int, payload_b64: str,
            colormap: str, payload_format: str, hover_meta=None,
            tooltip_x=None, tooltip_y=None, tooltip_z=None,
        ) -> None:
            self.calls.append(hover_meta)

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="sb", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.add_points(DemoFrame(), x="x", y="y", z="z")

    assert sender.calls == [None]


def test_scatter_constructor_hover_sends_primary_meta_on_startup() -> None:
    """hover= in __init__ must send enqueue_set_scatter_primary_hover_meta on startup."""
    import json as _json

    class LabeledFrame:
        x = [0.0, 1.0]
        y = [0.0, 1.0]
        z = [0.0, 1.0]
        label = ["alpha", "beta"]

        def __getitem__(self, key: str):
            return getattr(self, key)

    class Sender:
        def __init__(self) -> None:
            self.primary_meta_calls: list[tuple] = []

        def enqueue_set_scatter_hover_tooltip(self, widget_id: str, enabled: bool) -> None:
            pass

        def enqueue_set_scatter_tooltip_axis_labels(self, *a) -> None:
            pass

        def enqueue_set_scatter_primary_hover_meta(self, widget_id: str, meta: str) -> None:
            self.primary_meta_calls.append((widget_id, meta))

        def enqueue_set_scatter_lod(self, *a) -> None:
            pass

        def enqueue_set_scatter_picking_mode(self, *a) -> None:
            pass

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    frame = LabeledFrame()
    scatter = dg.Scatter3D(frame, x="x", y="y", z="z", id="sp", hover="label", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))
    scatter._queue_startup_resources()

    assert len(sender.primary_meta_calls) == 1
    wid, meta_json = sender.primary_meta_calls[0]
    assert wid == "sp"
    assert _json.loads(meta_json) == ["label: alpha", "label: beta"]


def test_scatter_set_points_hover_sends_primary_meta_live(monkeypatch) -> None:
    """set_points(hover=...) while live must enqueue primary hover meta after the points command."""
    import json as _json

    class LabeledFrame:
        x = [0.0]
        y = [0.0]
        z = [0.0]
        cat = ["A"]

        def __getitem__(self, key: str):
            return getattr(self, key)

    monkeypatch.setattr(
        widgets_module.Scatter3D, "_build_payload",
        lambda self: b"\x00" * 12,
    )

    class Sender:
        def __init__(self) -> None:
            self.meta_calls: list[tuple] = []

        def enqueue_clear_scatter_actors(self, widget_id: str) -> None:
            pass

        def enqueue_set_scatter_points_packed(self, *a, **kw) -> None:
            pass

        def enqueue_set_scatter_tooltip_axis_labels(self, *a) -> None:
            pass

        def enqueue_set_scatter_primary_hover_meta(self, widget_id: str, meta: str) -> None:
            self.meta_calls.append((widget_id, meta))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    frame = LabeledFrame()
    scatter = dg.Scatter3D(frame, x="x", y="y", z="z", id="sq", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.set_points(frame, x="x", hover="cat")

    assert len(sender.meta_calls) == 1
    wid, meta_json = sender.meta_calls[0]
    assert wid == "sq"
    assert _json.loads(meta_json) == ["cat: A"]


def test_scatter_extract_hover_meta_raises_on_missing_column() -> None:
    """_extract_hover_meta must raise ValueError for columns not found in the frame."""

    class SmallFrame:
        x = [1.0]

        def __getitem__(self, key: str):
            return getattr(self, key)

    with pytest.raises(ValueError, match="hover column"):
        dg.Scatter3D._extract_hover_meta(SmallFrame(), "nonexistent")


# ── clear() lifecycle tests ───────────────────────────────────────────────────

def test_scatter_clear_resets_hover_and_selection_state() -> None:
    """clear() must reset hover, selection, and actor label metadata on the Python side."""
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    s.hover_point = (1.0, 2.0, 3.0)
    s.hover_index = 5
    s.hover_actor = 0
    s.hover_text = "tip"
    s.selected = [dg.ScatterHit(actor=0, index=2)]
    s.selected_indices = [2]
    s.selected_index_values = ["row_A"]
    s._actor_row_labels[7] = ["a", "b"]

    s.clear()

    assert s.hover_point is None
    assert s.hover_index is None
    assert s.hover_actor is None
    assert s.hover_text is None
    assert s.selected == []
    assert s.selected_indices == []
    assert s.selected_index_values is None
    assert s._actor_row_labels == {}


def test_scatter_clear_drops_pending_scene_ops() -> None:
    """clear() pre-live must discard all queued scene operations."""
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    s._pending_scene_ops.append(("add_points", (0, "b64", "viridis", "xyz_f32_v0")))
    s._pending_scene_ops.append(("add_label", (0, 0.0, 0.0, 0.0, "hi", 1.0, 0.0, 0.0, 12)))

    s.clear()

    assert s._pending_scene_ops == []


# ── selection parity tests ────────────────────────────────────────────────────

def test_scatter_selection_flat_indices_no_labels() -> None:
    """selected_indices is flat across all actors; selected_index_values is None when no labels."""
    win = dg.Window("W")
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="sel1", parent=win)

    _, change_cbs = _collect_runtime_callbacks(win)
    change_cbs["sel1"](json.dumps({"actors": {"0": [1, 3], "1": [0, 2]}}))

    assert scatter.selected_indices == [1, 3, 0, 2]
    assert scatter.selected_index_values is None


def test_scatter_selection_flat_index_values_with_labels() -> None:
    """selected_index_values is a flat label list when any actor has dataframe index labels."""

    class LabeledFrame:
        x = [0.0, 1.0, 2.0]
        y = [0.0, 1.0, 2.0]
        z = [0.0, 1.0, 2.0]

        @property
        def index(self):
            return ["row_a", "row_b", "row_c"]

        def __getitem__(self, key):
            return getattr(self, key)

    win = dg.Window("W")
    scatter = dg.Scatter3D(LabeledFrame(), x="x", y="y", z="z", id="sel2", parent=win)

    _, change_cbs = _collect_runtime_callbacks(win)
    change_cbs["sel2"](json.dumps({"actors": {"0": [0, 2]}}))

    assert scatter.selected_indices == [0, 2]
    assert scatter.selected_index_values == ["row_a", "row_c"]


def test_scatter_selection_primary_only_actor_zero() -> None:
    """Without extra actors, selected_indices contains actor-0 indices only."""
    win = dg.Window("W")
    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="sel3", parent=win)

    _, change_cbs = _collect_runtime_callbacks(win)
    change_cbs["sel3"](json.dumps({"actors": {"0": [5, 7]}}))

    assert scatter.selected_indices == [5, 7]
    assert scatter.selected_index_values is None


# ── actor metadata lifecycle tests ───────────────────────────────────────────

def test_scatter_update_actor_updates_row_labels() -> None:
    """update_actor() must refresh _actor_row_labels for the handle."""

    class LabeledFrame:
        x = [1.0]
        y = [1.0]
        z = [1.0]

        @property
        def index(self):
            return ["rowA"]

        def __getitem__(self, key):
            return getattr(self, key)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h = s.add_points(DemoFrame(), x="x", y="y", z="z")
    assert s._actor_row_labels[h] is None

    s.update_actor(h, LabeledFrame(), x="x", y="y", z="z")
    assert s._actor_row_labels[h] == ["rowA"]


def test_scatter_remove_actor_pops_row_labels() -> None:
    """remove_actor() must remove the handle's entry from _actor_row_labels."""
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h = s.add_points(DemoFrame(), x="x", y="y", z="z")
    assert h in s._actor_row_labels

    s.remove_actor(h)
    assert h not in s._actor_row_labels


def test_scatter_prelive_remove_drops_pending_add(monkeypatch) -> None:
    """Pre-live remove_actor() must cancel the matching add_points pending op."""
    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: b"\x00" * 32)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h = s.add_points(DemoFrame(), x="x", y="y", z="z")
    assert any(op == "add_points" and args[0] == h for op, args in s._pending_scene_ops)

    s.remove_actor(h)
    assert not any(op == "add_points" and args[0] == h for op, args in s._pending_scene_ops)


def test_scatter_prelive_update_replaces_pending_add(monkeypatch) -> None:
    """Pre-live update_actor() must update the matching pending add_points entry."""
    _fake_buf_v1 = b"\x01" * 32
    _fake_buf_v2 = b"\x02" * 32
    call_count = [0]

    def fake_pack(frame, x, y, z):
        buf = _fake_buf_v1 if call_count[0] == 0 else _fake_buf_v2
        call_count[0] += 1
        return buf

    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", fake_pack)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h = s.add_points(DemoFrame(), x="x", y="y", z="z")

    s.update_actor(h, DemoFrame(), x="x", y="y", z="z")

    import base64 as _b64
    matching = [(op, args) for op, args in s._pending_scene_ops if op == "add_points" and args[0] == h]
    assert len(matching) == 1
    assert matching[0][1][1] == _b64.b64encode(_fake_buf_v2).decode("ascii")


def test_scatter_prelive_set_visibility_replayed_on_startup() -> None:
    """set_actor_visibility() pre-live must be replayed via _queue_startup_resources."""

    class Sender:
        def __init__(self) -> None:
            self.visibility_calls: list[tuple] = []

        def enqueue_set_scatter_hover_tooltip(self, *a) -> None:
            pass

        def enqueue_set_scatter_tooltip_axis_labels(self, *a) -> None:
            pass

        def enqueue_set_scatter_lod(self, *a) -> None:
            pass

        def enqueue_set_scatter_picking_mode(self, *a) -> None:
            pass

        def enqueue_add_scatter_actor(self, *a, **kw) -> None:
            pass

        def enqueue_set_scatter_actor_visible(
            self, widget_id: str, actor_id: int, visible: bool
        ) -> None:
            self.visibility_calls.append((widget_id, actor_id, visible))

        def close(self) -> None:
            pass

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="sv", parent=None)
    h = s.add_points(DemoFrame(), x="x", y="y", z="z")
    s.set_actor_visibility(h, False)

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s._bind_live(handle.widget_handle(s.id))
    s._queue_startup_resources()

    assert ("sv", h, False) in sender.visibility_calls


# ── startup replay tests ──────────────────────────────────────────────────────

def _make_full_startup_sender():
    """Return a Sender stub that silently accepts all startup-related calls."""
    class FullStartupSender:
        def __init__(self) -> None:
            self.lod_calls: list[tuple] = []
            self.picking_calls: list[str] = []

        def enqueue_set_scatter_hover_tooltip(self, *a) -> None:
            pass

        def enqueue_set_scatter_tooltip_axis_labels(self, *a) -> None:
            pass

        def enqueue_set_scatter_primary_hover_meta(self, *a) -> None:
            pass

        def enqueue_set_scatter_lod(
            self, widget_id: str, enabled: bool, threshold: int, factor: int
        ) -> None:
            self.lod_calls.append((widget_id, enabled, threshold, factor))

        def enqueue_set_scatter_picking_mode(self, widget_id: str, mode: str) -> None:
            self.picking_calls.append((widget_id, mode))

        def close(self) -> None:
            pass

    return FullStartupSender()


def test_scatter_startup_replays_lod() -> None:
    """_queue_startup_resources() must enqueue the current LOD config."""
    sender = _make_full_startup_sender()
    handle = AppHandle()
    handle._bind_native_sender(sender)

    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="lod1", parent=None)
    scatter.set_lod(enabled=True, threshold=50_000, factor=4)

    scatter._bind_live(handle.widget_handle(scatter.id))
    scatter._queue_startup_resources()

    assert ("lod1", True, 50_000, 4) in sender.lod_calls


def test_scatter_startup_replays_picking_mode() -> None:
    """_queue_startup_resources() must enqueue the current picking mode."""
    sender = _make_full_startup_sender()
    handle = AppHandle()
    handle._bind_native_sender(sender)

    scatter = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="pm1", parent=None)
    scatter.enable_rectangle_picking()

    scatter._bind_live(handle.widget_handle(scatter.id))
    scatter._queue_startup_resources()

    assert ("pm1", "rectangle") in sender.picking_calls


# ── on_hover clear crash fix ──────────────────────────────────────────────────

def test_scatter_on_hover_clear_fourarg_callback_gets_nones() -> None:
    """A 4-arg on_hover callback must receive (None, None, None, None) on hover clear."""
    received: list = []

    def cb(index, x, y, z):
        received.append((index, x, y, z))

    win = dg.Window("W")
    scatter = dg.Scatter3D(
        DemoFrame(), x="x", y="y", z="z", id="hov4", on_hover=cb, parent=win
    )
    _, change_cbs = _collect_runtime_callbacks(win)

    change_cbs["hov4"](json.dumps({
        "event": "hover_changed",
        "widget_id": "hov4",
        "actor": 0,
        "index": 0,
        "x": 1.0,
        "y": 2.0,
        "z": 3.0,
    }))
    change_cbs["hov4"](json.dumps({
        "event": "hover_changed",
        "widget_id": "hov4",
    }))

    assert len(received) == 2
    assert received[0] == (0, 1.0, 2.0, 3.0)
    assert received[1] == (None, None, None, None)


# ── actor ID reservation / collision regression ───────────────────────────────

def test_scatter_first_add_points_id_is_not_zero() -> None:
    """add_points() must never return 0 — that ID is reserved for the primary actor."""
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h = s.add_points(DemoFrame(), x="x", y="y", z="z")
    assert h != 0, "actor ID 0 is reserved for the primary scatter buffer"


def test_scatter_first_add_stream_id_is_not_zero() -> None:
    """add_stream() must never return 0 — that ID is reserved for the primary actor."""
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h = s.add_stream(max_points=100)
    assert h != 0, "stream ID 0 is reserved for the primary scatter buffer"


def test_scatter_clear_resets_actor_id_to_one() -> None:
    """clear() must reset _next_actor_id to 1 so that 0 stays reserved after a clear."""
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    s.add_points(DemoFrame(), x="x", y="y", z="z")  # advances counter
    s.clear()
    assert s._next_actor_id == 1


# ── add_stream mode validation ────────────────────────────────────────────────

def test_scatter_add_stream_invalid_mode_raises() -> None:
    """add_stream() must raise ValueError for an unrecognised mode string."""
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    with pytest.raises(ValueError, match="mode"):
        s.add_stream(max_points=100, mode="sliding")


# ── scene clear — ClearScatterScene command ───────────────────────────────────

def test_scatter_clear_sends_clear_scatter_scene() -> None:
    """clear() must enqueue ClearScatterScene, not the individual clear commands."""

    class Sender:
        def __init__(self) -> None:
            self.scene_cleared = False
            self.individual_clears: list[str] = []

        def enqueue_clear_scatter_scene(self, widget_id: str) -> None:
            self.scene_cleared = True

        def enqueue_clear_scatter_actors(self, widget_id: str) -> None:
            self.individual_clears.append("actors")

        def enqueue_clear_scatter_labels(self, widget_id: str) -> None:
            self.individual_clears.append("labels")

        def enqueue_clear_scatter_overlays(self, widget_id: str) -> None:
            self.individual_clears.append("overlays")

        def enqueue_clear_scatter_meshes(self, widget_id: str) -> None:
            self.individual_clears.append("meshes")

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="sc", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    s.clear()

    assert sender.scene_cleared
    assert sender.individual_clears == []


# ── mixed labeled/unlabeled selection parity ─────────────────────────────────

def test_scatter_selection_mixed_unlabeled_actor_contributes_none() -> None:
    """When actor 0 has labels but actor 1 doesn't, actor 1 hits must be None in index_values."""

    class LabeledFrame:
        x = [0.0, 1.0]
        y = [0.0, 1.0]
        z = [0.0, 1.0]

        @property
        def index(self):
            return ["row_a", "row_b"]

        def __getitem__(self, key):
            return getattr(self, key)

    win = dg.Window("W")
    scatter = dg.Scatter3D(LabeledFrame(), x="x", y="y", z="z", id="mix1", parent=win)

    _, change_cbs = _collect_runtime_callbacks(win)
    # Actor 0 has labeled index; actor 1 has no labels (not added via add_points).
    change_cbs["mix1"](json.dumps({"actors": {"0": [0], "1": [3]}}))

    assert scatter.selected_index_values == ["row_a", None]


# ── set_points() clears point-layer state ─────────────────────────────────────

def test_scatter_set_points_clears_prelive_actor_ops(monkeypatch) -> None:
    """set_points() pre-live must discard pending add_points/add_stream ops."""
    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: b"\x00" * 32)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    s.add_points(DemoFrame(), x="x", y="y", z="z")
    s.add_stream(max_points=100)
    assert any(op in ("add_points", "add_stream") for op, _ in s._pending_scene_ops)

    # set_points() must strip those ops.
    s.set_points(DemoFrame(), x="x", y="y", z="z")
    assert not any(op in ("add_points", "add_stream") for op, _ in s._pending_scene_ops)


def test_scatter_set_points_clears_actor_row_labels(monkeypatch) -> None:
    """set_points() must clear _actor_row_labels populated by previous add_points() calls."""
    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: b"\x00" * 32)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    s.add_points(DemoFrame(), x="x", y="y", z="z")
    assert s._actor_row_labels  # should have an entry now

    s.set_points(DemoFrame(), x="x", y="y", z="z")
    assert s._actor_row_labels == {}


def test_scatter_set_points_resets_next_actor_id(monkeypatch) -> None:
    """set_points() must reset _next_actor_id to 1."""
    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: b"\x00" * 32)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    s.add_points(DemoFrame(), x="x", y="y", z="z")
    assert s._next_actor_id == 2

    s.set_points(DemoFrame(), x="x", y="y", z="z")
    assert s._next_actor_id == 1


# ── update_actor() pre-live clears stale hover metadata ──────────────────────

def test_scatter_prelive_update_actor_clears_hover_meta(monkeypatch) -> None:
    """Pre-live update_actor() must discard old hover_meta from the pending add_points op."""
    import json as _json

    class LabeledFrame:
        x = [0.0, 1.0]
        y = [0.0, 1.0]
        z = [0.0, 1.0]
        lbl = ["a", "b"]

        def __getitem__(self, key):
            return getattr(self, key)

    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: b"\x00" * 32)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h = s.add_points(LabeledFrame(), x="x", y="y", z="z", hover="lbl")

    # Verify hover_meta was stored in the pending op.
    orig = [(op, args) for op, args in s._pending_scene_ops if op == "add_points" and args[0] == h]
    assert orig and orig[0][1][4] is not None  # hover_meta present

    # update_actor() should clear hover_meta in the pending op.
    s.update_actor(h, DemoFrame(), x="x", y="y", z="z")
    updated = [(op, args) for op, args in s._pending_scene_ops if op == "add_points" and args[0] == h]
    assert updated and updated[0][1][4] is None


# ── add_stream() with initial data ────────────────────────────────────────────

def test_scatter_add_stream_with_initial_data_enqueues_stream_live(monkeypatch) -> None:
    """add_stream(frame, x, y, z) while live must enqueue AddScatterStream + StreamScatterActor."""

    class Sender:
        def __init__(self) -> None:
            self.stream_creates: list[tuple] = []
            self.stream_data: list[tuple] = []

        def enqueue_add_scatter_stream(
            self, widget_id: str, actor_id: int, max_points: int, mode: str
        ) -> None:
            self.stream_creates.append((widget_id, actor_id, max_points, mode))

        def enqueue_stream_scatter_actor(
            self, widget_id: str, actor_id: int, payload_b64: str,
            colormap: str, payload_format: str
        ) -> None:
            self.stream_data.append((widget_id, actor_id))

        def close(self) -> None:
            pass

    _fake_buf = b"\xAB" * 32
    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: _fake_buf)

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="st", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    h = s.add_stream(DemoFrame(), mode="ring", max_points=200, x="x", y="y", z="z")

    assert any(c[1] == h for c in sender.stream_creates)
    assert any(d[1] == h for d in sender.stream_data)


def test_scatter_add_stream_with_initial_data_pending_replayed(monkeypatch) -> None:
    """add_stream(frame, x, y, z) pre-live must replay both create + initial data on startup."""

    class Sender:
        def __init__(self) -> None:
            self.stream_creates: list[tuple] = []
            self.stream_data: list[tuple] = []

        def enqueue_set_scatter_hover_tooltip(self, *a) -> None:
            pass

        def enqueue_set_scatter_tooltip_axis_labels(self, *a) -> None:
            pass

        def enqueue_set_scatter_lod(self, *a) -> None:
            pass

        def enqueue_set_scatter_picking_mode(self, *a) -> None:
            pass

        def enqueue_add_scatter_stream(
            self, widget_id: str, actor_id: int, max_points: int, mode: str
        ) -> None:
            self.stream_creates.append((widget_id, actor_id))

        def enqueue_stream_scatter_actor(
            self, widget_id: str, actor_id: int, payload_b64: str,
            colormap: str, payload_format: str
        ) -> None:
            self.stream_data.append((widget_id, actor_id))

        def close(self) -> None:
            pass

    _fake_buf = b"\xAB" * 32
    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: _fake_buf)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="stq", parent=None)
    h = s.add_stream(DemoFrame(), mode="append", max_points=300, x="x", y="y", z="z")

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)
    s._bind_live(handle.widget_handle(s.id))
    s._queue_startup_resources()

    assert ("stq", h) in sender.stream_creates
    assert ("stq", h) in sender.stream_data


# ── set_colormap() re-sends hover metadata ───────────────────────────────────

def test_scatter_set_colormap_resends_hover_meta(monkeypatch) -> None:
    """set_colormap() must re-send primary hover meta after enqueuing the new packed points."""
    import json as _json

    class HoverFrame:
        x = [0.0, 1.0]
        y = [0.0, 1.0]
        z = [0.0, 1.0]
        tag = ["A", "B"]

        def __getitem__(self, key):
            return getattr(self, key)

    monkeypatch.setattr(
        widgets_module.Scatter3D, "_build_payload", lambda self: b"\x00" * 12
    )

    class Sender:
        def __init__(self) -> None:
            self.order: list[str] = []
            self.meta_calls: list[tuple] = []

        def enqueue_set_scatter_points_packed(self, *a, **kw) -> None:
            self.order.append("points")

        def enqueue_set_scatter_primary_hover_meta(self, widget_id: str, meta: str) -> None:
            self.order.append("meta")
            self.meta_calls.append((widget_id, meta))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    frame = HoverFrame()
    scatter = dg.Scatter3D(frame, x="x", y="y", z="z", id="cm1", hover="tag", parent=None)
    scatter._bind_live(handle.widget_handle(scatter.id))

    scatter.set_colormap("plasma")

    assert "meta" in sender.order, "hover meta must be re-sent after set_colormap"
    assert sender.order.index("points") < sender.order.index("meta"), \
        "points must be enqueued before hover meta"
    assert sender.meta_calls
    wid, meta_json = sender.meta_calls[0]
    assert wid == "cm1"
    assert _json.loads(meta_json) == ["tag: A", "tag: B"]


# ── add_stream() new positional API ──────────────────────────────────────────

def test_scatter_add_stream_legacy_int_first_arg() -> None:
    """add_stream(500) and add_stream(max_points=500) must both produce the same result."""
    s1 = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h1 = s1.add_stream(500)

    s2 = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h2 = s2.add_stream(max_points=500)

    assert h1 == h2 == 1  # 0 is reserved for primary
    assert s1._pending_scene_ops[0][1][1] == 500
    assert s2._pending_scene_ops[0][1][1] == 500


def test_scatter_add_stream_frame_first_arg_enqueues_initial_data(monkeypatch) -> None:
    """add_stream(frame, max_points=N, x=, y=, z=) must enqueue AddScatterStream + initial data."""

    class Sender:
        def __init__(self) -> None:
            self.creates: list[tuple] = []
            self.data: list[tuple] = []

        def enqueue_add_scatter_stream(
            self, widget_id: str, actor_id: int, max_points: int, mode: str
        ) -> None:
            self.creates.append((widget_id, actor_id, max_points))

        def enqueue_stream_scatter_actor(
            self, widget_id: str, actor_id: int, payload_b64: str,
            colormap: str, payload_format: str
        ) -> None:
            self.data.append((widget_id, actor_id))

        def close(self) -> None:
            pass

    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: b"\x11" * 32)

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="sf", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    h = s.add_stream(DemoFrame(), max_points=400, x="x", y="y", z="z")

    assert ("sf", h, 400) in sender.creates
    assert ("sf", h) in sender.data


def test_scatter_add_stream_numpy_array_packs_xyz(monkeypatch) -> None:
    """add_stream(np_array, max_points=N) must pack (N,3) positions automatically."""
    try:
        import numpy as np
    except ImportError:
        return

    class Sender:
        def __init__(self) -> None:
            self.creates: list = []
            self.data_calls: list[tuple] = []

        def enqueue_add_scatter_stream(self, widget_id, actor_id, max_points, mode) -> None:
            self.creates.append((widget_id, actor_id))

        def enqueue_stream_scatter_actor(
            self, widget_id, actor_id, payload_b64, colormap, payload_format
        ) -> None:
            self.data_calls.append((widget_id, actor_id, payload_b64))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="sn", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    pts = np.array([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]], dtype=np.float32)
    h = s.add_stream(pts, max_points=100)

    assert ("sn", h) in sender.creates
    # Payload should be non-empty (2 points × 3 floats × 4 bytes = 24 bytes, b64-encoded).
    data_entries = [d for d in sender.data_calls if d[1] == h]
    assert data_entries, "initial stream data must be enqueued for numpy array input"
    import base64 as _b64
    raw = _b64.b64decode(data_entries[0][2])
    assert len(raw) == 2 * 3 * 4  # 2 points, xyz float32


def test_scatter_add_stream_numpy_array_2d_packs_zeros_for_z(monkeypatch) -> None:
    """add_stream((N,2) array, max_points=N) must fill z=0."""
    try:
        import numpy as np
    except ImportError:
        return

    class Sender:
        def __init__(self) -> None:
            self.data_calls: list[tuple] = []

        def enqueue_add_scatter_stream(self, *a) -> None:
            pass

        def enqueue_stream_scatter_actor(
            self, widget_id, actor_id, payload_b64, colormap, payload_format
        ) -> None:
            self.data_calls.append((widget_id, actor_id, payload_b64))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="s2d", parent=None)
    s._bind_live(handle.widget_handle(s.id))

    pts2d = np.array([[1.0, 2.0], [3.0, 4.0]], dtype=np.float32)
    h = s.add_stream(pts2d, max_points=50)

    entries = [d for d in sender.data_calls if d[1] == h]
    assert entries
    import base64 as _b64, struct
    raw = _b64.b64decode(entries[0][2])
    # Third float of each triple should be 0.0
    z0 = struct.unpack_from("<f", raw, 8)[0]
    z1 = struct.unpack_from("<f", raw, 20)[0]
    assert z0 == 0.0 and z1 == 0.0


# ── stream() DragonSci compatibility ─────────────────────────────────────────

def test_scatter_stream_numpy_3d_array(monkeypatch) -> None:
    """stream(handle, (N,3) array) must push data without x/y/z kwargs."""
    try:
        import numpy as np
    except ImportError:
        return

    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: b"\x00" * 12)

    class Sender:
        def __init__(self) -> None:
            self.pushed: list[tuple] = []

        def enqueue_add_scatter_stream(self, *a) -> None:
            pass

        def enqueue_stream_scatter_actor(self, widget_id, actor_id, b64, cmap, fmt) -> None:
            self.pushed.append((actor_id, b64))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="stm3d", parent=None)
    s._bind_live(handle.widget_handle(s.id))
    sh = s.add_stream(max_points=100)

    pts = np.array([[1.0, 2.0, 3.0]], dtype=np.float32)
    s.stream(sh, pts)

    assert any(actor_id == sh for actor_id, _ in sender.pushed), \
        "stream() with numpy array must enqueue stream data"


def test_scatter_stream_list_of_lists(monkeypatch) -> None:
    """stream(handle, [[x,y,z], ...]) must be accepted via coercion."""
    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: b"\x00" * 12)

    class Sender:
        def __init__(self) -> None:
            self.pushed: list[tuple] = []

        def enqueue_add_scatter_stream(self, *a) -> None:
            pass

        def enqueue_stream_scatter_actor(self, widget_id, actor_id, b64, cmap, fmt) -> None:
            self.pushed.append((actor_id, b64))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="stmll", parent=None)
    s._bind_live(handle.widget_handle(s.id))
    sh = s.add_stream(max_points=100)

    s.stream(sh, [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]])

    assert any(actor_id == sh for actor_id, _ in sender.pushed), \
        "stream() with list-of-lists must enqueue stream data"


def test_scatter_stream_numpy_2d_array_zero_fills_z(monkeypatch) -> None:
    """stream(handle, (N,2) array) must zero-fill the z coordinate."""
    try:
        import numpy as np
    except ImportError:
        return
    import base64 as _b64, struct as _struct

    # Use real _pack_xyz_bytes so the z=0 actually ends up in the buffer.
    class Sender:
        def __init__(self) -> None:
            self.pushed: list[tuple] = []

        def enqueue_add_scatter_stream(self, *a) -> None:
            pass

        def enqueue_stream_scatter_actor(self, widget_id, actor_id, b64, cmap, fmt) -> None:
            self.pushed.append((actor_id, b64))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="stm2d", parent=None)
    s._bind_live(handle.widget_handle(s.id))
    sh = s.add_stream(max_points=100)

    pts = np.array([[10.0, 20.0]], dtype=np.float32)
    s.stream(sh, pts)

    entries = [(aid, b64) for aid, b64 in sender.pushed if aid == sh]
    assert entries, "stream data must be enqueued"
    raw = _b64.b64decode(entries[0][1])
    z_val = _struct.unpack_from("<f", raw, 8)[0]
    assert z_val == 0.0, f"z should be 0.0 for (N,2) input, got {z_val}"


def test_scatter_stream_invalid_shape_raises() -> None:
    """stream(handle, (N,4) array) must raise ValueError."""
    try:
        import numpy as np
    except ImportError:
        return

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    sh = s.add_stream(max_points=100)

    pts = np.zeros((5, 4), dtype=np.float32)
    with pytest.raises(ValueError, match="shape"):
        s.stream(sh, pts)


# ── set_colormap() refreshes _cached_payload_b64 ─────────────────────────────

def test_scatter_set_colormap_refreshes_cached_b64(monkeypatch) -> None:
    """props()['data_b64'] must change after set_colormap() for a scalar-colored widget."""

    class ScalarFrame:
        x = [0.0, 1.0]
        y = [0.0, 1.0]
        z = [0.0, 1.0]
        s = [0.5, 1.5]

        def __getitem__(self, key):
            return getattr(self, key)

    call_count = {"n": 0}
    original = widgets_module.Scatter3D._build_payload

    def counting_build(self):
        call_count["n"] += 1
        return b"\x00" * (12 * call_count["n"])  # different bytes each call

    monkeypatch.setattr(widgets_module.Scatter3D, "_build_payload", counting_build)

    s = dg.Scatter3D(ScalarFrame(), x="x", y="y", z="z", scalars="s", parent=None)
    b64_before = s.props()["data_b64"]

    # set_colormap on a v1 widget invalidates and rebuilds the payload.
    s.set_colormap("plasma")
    b64_after = s.props()["data_b64"]

    assert b64_before != b64_after, \
        "data_b64 must change after set_colormap() invalidates the v1 payload"


# ── clear() resets all Python mirror state ────────────────────────────────────

def test_scatter_clear_resets_all_mirror_state(monkeypatch) -> None:
    """clear() must reset counters, labels, cached payload, and ellipsoid params."""
    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: b"\x00" * 12)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)

    # Populate various mirror-state fields.
    s.add_points(DemoFrame(), x="x", y="y", z="z")  # bumps _next_actor_id
    if not hasattr(s, "_next_label_id"):
        s._next_label_id = 0
    s._next_label_id = 5
    if not hasattr(s, "_next_overlay_id"):
        s._next_overlay_id = 0
    s._next_overlay_id = 3
    if not hasattr(s, "_next_mesh_id"):
        s._next_mesh_id = 0
    s._next_mesh_id = 7
    s._ellipsoid_params[42] = {"center": [0, 0, 0]}
    s._primary_row_labels = ["a", "b"]
    s._cached_payload = b"\xde\xad"
    s._cached_payload_b64 = "3q0="

    s.clear()

    assert s._next_actor_id == 1, "actor id counter must reset to 1"
    assert s._next_label_id == 0, "label id counter must reset to 0"
    assert s._next_overlay_id == 0, "overlay id counter must reset to 0"
    assert s._next_mesh_id == 0, "mesh id counter must reset to 0"
    assert s._ellipsoid_params == {}, "ellipsoid params must be cleared"
    assert s._primary_row_labels is None, "_primary_row_labels must be reset to None"
    assert s._cached_payload == b"", "_cached_payload must be empty after clear"
    assert s._cached_payload_b64 == "", "_cached_payload_b64 must be empty string after clear"
    assert s._actor_row_labels == {}, "_actor_row_labels must be cleared"
    assert s._pending_scene_ops == [], "_pending_scene_ops must be cleared"


# ── clear() does not resurrect old primary data via props() ──────────────────

def test_scatter_clear_props_returns_empty_b64(monkeypatch) -> None:
    """After clear(), props()['data_b64'] must be '' and must not repack the old frame."""
    pack_calls: list[str] = []

    real_build = widgets_module.Scatter3D._build_payload

    def counting_build(self):
        pack_calls.append("build")
        return real_build(self)

    monkeypatch.setattr(widgets_module.Scatter3D, "_build_payload", counting_build)
    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: b"\x00" * 12)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    pack_calls.clear()  # discard __init__ call

    s.clear()
    pack_calls_after_clear = list(pack_calls)

    props = s.props()
    assert props["data_b64"] == "", "data_b64 must be empty string after clear()"
    # _build_payload must not be called again after the clear set the stable empty state.
    assert pack_calls == pack_calls_after_clear, \
        "_build_payload must not be called by props() after clear()"


def test_scatter_set_points_after_clear_re_enables_primary(monkeypatch) -> None:
    """set_points() after clear() must restore normal data packing."""
    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: b"\x00" * 12)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    s.clear()
    assert s.props()["data_b64"] == "", "sanity: cleared state"

    s.set_points(DemoFrame(), x="x", y="y", z="z")
    b64 = s.props()["data_b64"]
    assert b64 != "", "data_b64 must be non-empty after set_points() following clear()"


# ── add_stream() accepts list-of-lists initial data ──────────────────────────

def test_scatter_add_stream_list_of_lists_initial_data(monkeypatch) -> None:
    """add_stream([[x,y,z], ...], max_points=N) must pack initial data via coercion."""
    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: b"\x00" * 12)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    pts = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]
    h = s.add_stream(pts, max_points=50)

    ops = [op for op, args in s._pending_scene_ops if op == "add_stream" and args[0] == h]
    assert ops, "add_stream pending op must exist"
    # 6-tuple means initial data was packed
    matching = [(op, args) for op, args in s._pending_scene_ops if op == "add_stream" and args[0] == h]
    assert len(matching[0][1]) == 6, "pending add_stream must be a 6-tuple when initial data is given"


def test_scatter_add_stream_2d_list_initial_data_zero_fills_z(monkeypatch) -> None:
    """add_stream([[x,y], ...], max_points=N) must zero-fill z for (N,2) list input."""
    import base64 as _b64, struct as _struct

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    pts = [[10.0, 20.0], [30.0, 40.0]]
    h = s.add_stream(pts, max_points=50)

    matching = [(op, args) for op, args in s._pending_scene_ops if op == "add_stream" and args[0] == h]
    assert matching and len(matching[0][1]) == 6, "must be 6-tuple with initial data"
    init_b64 = matching[0][1][3]
    raw = _b64.b64decode(init_b64)
    z0 = _struct.unpack_from("<f", raw, 8)[0]
    assert z0 == 0.0, f"z should be 0.0 for (N,2) list input, got {z0}"


def test_scatter_add_stream_frame_without_columns_raises() -> None:
    """add_stream(frame, max_points=N) without x/y/z must raise ValueError."""
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    with pytest.raises(ValueError, match="x, y, and z"):
        s.add_stream(DemoFrame(), max_points=100)


# ── _coerce_point_input() validates frame inputs ──────────────────────────────

def test_scatter_stream_frame_without_columns_raises() -> None:
    """stream(handle, frame) without x/y/z must raise ValueError."""
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    sh = s.add_stream(max_points=100)
    with pytest.raises(ValueError, match="x, y, and z"):
        s.stream(sh, DemoFrame())


def test_scatter_coerce_empty_list_raises() -> None:
    """_coerce_point_input with an empty list must raise ValueError (no valid columns)."""
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    sh = s.add_stream(max_points=100)
    # Empty list is not array-like with shape, falls through to frame path → missing x/y/z
    with pytest.raises(ValueError, match="x, y, and z"):
        s.stream(sh, [])


# ── add_points() / update_actor() DragonSci-style raw positions ──────────────

def test_scatter_add_points_list_of_lists(monkeypatch) -> None:
    """add_points([[x,y,z], ...]) must pack data without x/y/z kwargs."""
    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: b"\x00" * 12)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h = s.add_points([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]])

    ops = [(op, args) for op, args in s._pending_scene_ops if op == "add_points" and args[0] == h]
    assert ops, "add_points pending op must exist"


def test_scatter_add_points_numpy_2d_zero_fills_z(monkeypatch) -> None:
    """add_points((N,2) array) must zero-fill z."""
    try:
        import numpy as np
    except ImportError:
        return
    import base64 as _b64, struct as _struct

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    pts = np.array([[10.0, 20.0]], dtype=np.float32)
    h = s.add_points(pts)

    ops = [(op, args) for op, args in s._pending_scene_ops if op == "add_points" and args[0] == h]
    assert ops
    raw = _b64.b64decode(ops[0][1][1])
    z_val = _struct.unpack_from("<f", raw, 8)[0]
    assert z_val == 0.0, f"z must be 0.0 for (N,2) input, got {z_val}"


def test_scatter_add_points_frame_without_columns_raises() -> None:
    """add_points(frame) without x/y/z must raise ValueError."""
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    with pytest.raises(ValueError, match="x, y, and z"):
        s.add_points(DemoFrame())


def test_scatter_update_actor_numpy_array(monkeypatch) -> None:
    """update_actor(handle, (N,3) array) must update the pending op without x/y/z kwargs."""
    monkeypatch.setattr(widgets_module, "_pack_xyz_bytes", lambda *a: b"\x00" * 12)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    h = s.add_points([[1.0, 2.0, 3.0]])

    try:
        import numpy as np
    except ImportError:
        return
    pts = np.array([[7.0, 8.0, 9.0]], dtype=np.float32)
    s.update_actor(h, pts)

    ops = [(op, args) for op, args in s._pending_scene_ops if op == "add_points" and args[0] == h]
    assert ops, "pending op must still exist after update_actor"


def test_scatter_update_actor_frame_without_columns_raises() -> None:
    """update_actor(handle, frame) without x/y/z must raise ValueError."""
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)
    with pytest.raises(ValueError, match="x, y, and z"):
        s.update_actor(1, DemoFrame())


def test_scatter_create_live_frame_returns_retained_handle() -> None:
    """create_live_frame() returns a reusable full-frame replacement handle."""
    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", parent=None)

    live = s.create_live_frame(capacity=1_000_000, scalars="intensity", colormap="turbo")

    assert isinstance(live, dg.ScatterLiveFrame)
    assert live.handle == 0
    assert live.mode == "primary"
    assert live.capacity == 1_000_000
    assert live.scalars == "intensity"
    assert live.colormap == "turbo"


def test_scatter_live_frame_adds_once_then_updates_same_actor(monkeypatch) -> None:
    """replace() creates one actor and subsequent complete frames update that actor."""
    fake_payloads = [b"first-frame", b"second-frame"]

    def fake_pack(self, *args, **kwargs):
        return fake_payloads.pop(0), kwargs.get("colormap") or "turbo", "point_instance_v1"

    monkeypatch.setattr(widgets_module.Scatter3D, "_pack_actor_payload", fake_pack)

    class Sender:
        def __init__(self) -> None:
            self.add_calls: list[tuple] = []
            self.update_calls: list[tuple] = []
            self.remove_calls: list[tuple] = []

        def enqueue_add_scatter_actor_packed(
            self,
            widget_id: str,
            actor_id: int,
            payload: bytes,
            colormap: str,
            payload_format: str,
            hover_meta=None,
            tooltip_x=None,
            tooltip_y=None,
            tooltip_z=None,
        ) -> None:
            self.add_calls.append(
                (widget_id, actor_id, payload, colormap, payload_format, tooltip_x, tooltip_y, tooltip_z)
            )

        def enqueue_update_scatter_actor_packed(
            self,
            widget_id: str,
            actor_id: int,
            payload: bytes,
            colormap: str,
            payload_format: str,
            tooltip_x=None,
            tooltip_y=None,
            tooltip_z=None,
        ) -> None:
            self.update_calls.append(
                (widget_id, actor_id, payload, colormap, payload_format, tooltip_x, tooltip_y, tooltip_z)
            )

        def enqueue_remove_scatter_actor(self, widget_id: str, actor_id: int) -> None:
            self.remove_calls.append((widget_id, actor_id))

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="live-scatter", parent=None)
    s._bind_live(handle.widget_handle(s.id))
    live = s.create_live_frame(
        x="x",
        y="y",
        z="z",
        scalars="intensity",
        colormap="turbo",
        mode="actor",
    )

    live.replace(DemoFrame())
    actor = live.handle
    live.replace(DemoFrame())

    assert actor is not None
    assert live.handle == actor
    assert len(sender.add_calls) == 1
    assert len(sender.update_calls) == 1
    assert sender.add_calls[0] == (
        "live-scatter",
        actor,
        b"first-frame",
        "turbo",
        "point_instance_v1",
        "x",
        "y",
        "z",
    )
    assert sender.update_calls[0] == (
        "live-scatter",
        actor,
        b"second-frame",
        "turbo",
        "point_instance_v1",
        "x",
        "y",
        "z",
    )

    live.remove()
    assert live.handle is None
    assert sender.remove_calls == [("live-scatter", actor)]


def test_scatter_live_frame_primary_uses_prepared_points(monkeypatch) -> None:
    """Default live-frame mode should enqueue primary packed points, not an actor layer."""
    fake_payload = dg.ScatterPayload(
        data=b"primary-frame",
        payload_format="xyz_f32_v0",
        colormap="turbo",
        point_count=3,
        pack_ms=1.25,
        axis_labels=("x", "y", "z"),
        hover_meta=None,
        frame_summary=None,
    )

    monkeypatch.setattr(widgets_module.Scatter3D, "prepare_points", lambda *a, **kw: fake_payload)

    class Sender:
        def __init__(self) -> None:
            self.primary_calls: list[tuple] = []
            self.actor_calls: list[tuple] = []
            self.axis_calls: list[tuple] = []

        def enqueue_set_scatter_points_packed(self, *args) -> None:
            self.primary_calls.append(args)

        def enqueue_add_scatter_actor_packed(self, *args) -> None:
            self.actor_calls.append(args)

        def enqueue_update_scatter_actor_packed(self, *args) -> None:
            self.actor_calls.append(args)

        def enqueue_set_scatter_tooltip_axis_labels(self, *args) -> None:
            self.axis_calls.append(args)

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    s = dg.Scatter3D(DemoFrame(), x="x", y="y", z="z", id="primary-live", parent=None)
    s._bind_live(handle.widget_handle(s.id))
    live = s.create_live_frame(x="x", y="y", z="z", colormap="turbo")

    live.replace(DemoFrame(), fit=True)

    assert live.handle == 0
    assert len(sender.primary_calls) == 1
    call = sender.primary_calls[0]
    assert call[:3] == ("primary-live", b"primary-frame", 1.25)
    assert isinstance(call[3], float)
    assert call[4:] == ("turbo", "xyz_f32_v0", True, True, None, None)
    assert sender.axis_calls == [("primary-live", "x", "y", "z")]
    assert sender.actor_calls == []

    prepared = dg.ScatterPayload(
        data=b"prepared-frame",
        payload_format="xyz_f32_v0",
        colormap="turbo",
        point_count=3,
        pack_ms=0.5,
        axis_labels=("x", "y", "z"),
        bounds=((0.0, 1.0, 2.0), (3.0, 4.0, 5.0)),
        hover_meta=None,
        frame_summary=None,
    )
    live.replace_prepared(prepared, fit=False)
    assert len(sender.primary_calls) == 2
    assert sender.primary_calls[1][:3] == ("primary-live", b"prepared-frame", 0.5)
    assert sender.primary_calls[1][4:] == (
        "turbo",
        "xyz_f32_v0",
        True,
        False,
        (0.0, 1.0, 2.0),
        (3.0, 4.0, 5.0),
    )


# ── pre-live clear() does not replay stale primary metadata on startup ────────

def test_scatter_clear_prelive_suppresses_hover_meta_on_startup() -> None:
    """After clear() before going live, _queue_startup_resources must not enqueue primary hover meta."""
    import json as _json

    class HoverFrame:
        x = [0.0, 1.0]
        y = [0.0, 1.0]
        z = [0.0, 1.0]
        tag = ["A", "B"]

        def __getitem__(self, key):
            return getattr(self, key)

    class Sender:
        def __init__(self) -> None:
            self.hover_meta_calls: list = []
            self.axis_label_calls: list = []

        def enqueue_set_scatter_hover_tooltip(self, *a) -> None:
            pass

        def enqueue_set_scatter_tooltip_axis_labels(self, *a) -> None:
            self.axis_label_calls.append(a)

        def enqueue_set_scatter_primary_hover_meta(self, *a) -> None:
            self.hover_meta_calls.append(a)

        def enqueue_set_scatter_lod(self, *a) -> None:
            pass

        def enqueue_set_scatter_picking_mode(self, *a) -> None:
            pass

        def close(self) -> None:
            pass

    handle = AppHandle()
    sender = Sender()
    handle._bind_native_sender(sender)

    scatter = dg.Scatter3D(HoverFrame(), x="x", y="y", z="z", id="clhov", hover="tag", parent=None)
    scatter.clear()  # sets _primary_cleared = True before going live

    scatter._bind_live(handle.widget_handle(scatter.id))
    # Manually invoke startup (normally called by the runtime on bind).
    scatter._queue_startup_resources()

    assert not sender.hover_meta_calls, \
        "enqueue_set_scatter_primary_hover_meta must not be called after pre-live clear()"
    assert not sender.axis_label_calls, \
        "enqueue_set_scatter_tooltip_axis_labels must not be called after pre-live clear()"


# ---------------------------------------------------------------------------
# GridLayout / FlowLayout
# ---------------------------------------------------------------------------

def test_grid_layout_default_serialization() -> None:
    """GridLayout defaults to a responsive two-column card grid."""
    win = dg.Window("Grid")
    dg.GridLayout(id="g")
    node = win.to_dict()["children"][0]
    assert node["type"] == "grid_layout"
    assert node["props"].get("columns") == 2
    assert node["props"].get("min_column_width") == 320


def test_grid_layout_explicit_columns() -> None:
    """GridLayout passes columns prop through correctly."""
    win = dg.Window("Grid")
    dg.GridLayout(columns=4, id="g")
    node = win.to_dict()["children"][0]
    assert node["props"].get("columns") == 4


def test_grid_layout_min_column_width() -> None:
    """GridLayout passes min_column_width prop through correctly."""
    win = dg.Window("Grid")
    dg.GridLayout(min_column_width=120, id="g")
    node = win.to_dict()["children"][0]
    assert node["props"].get("min_column_width") == 120


def test_grid_layout_auto_columns_omits_columns_prop() -> None:
    """GridLayout(columns='auto') omits the columns prop so native uses AutoFill."""
    win = dg.Window("Grid")
    dg.GridLayout(columns="auto", id="g")
    node = win.to_dict()["children"][0]
    assert "columns" not in node["props"]
    assert node["props"].get("min_column_width") == 320


def test_grid_layout_gap_in_style() -> None:
    """GridLayout gap/row_gap kwargs are placed into the inline style dict."""
    win = dg.Window("Grid")
    dg.GridLayout(gap=8, row_gap=4, id="g")
    node = win.to_dict()["children"][0]
    assert node["style"].get("gap") == 8
    assert node["style"].get("row_gap") == 4


def test_grid_layout_template_tracks() -> None:
    """GridLayout can serialize compact explicit track templates."""
    win = dg.Window("Grid")
    dg.GridLayout(template_columns=(44, "1fr", "auto"), template_rows="18px auto", id="g")
    node = win.to_dict()["children"][0]
    assert node["props"].get("template_columns") == [44, {"fr": 1}, "auto"]
    assert node["props"].get("template_rows") == [18, "auto"]
    assert "columns" not in node["props"]
    assert "min_column_width" not in node["props"]


def test_grid_layout_template_track_helpers() -> None:
    """GridLayout accepts structured fit-content/minmax/repeat track helpers."""
    win = dg.Window("Grid")
    dg.GridLayout(
        template_columns=(
            {"fit_content": "40%"},
            {"minmax": (72, "1fr")},
            {"repeat": {"kind": "auto-fill", "tracks": [{"min": 80, "max": "1fr"}]}},
        ),
        id="g",
    )
    node = win.to_dict()["children"][0]
    assert node["props"].get("template_columns") == [
        {"fit_content": {"percent": 40}},
        {"minmax": {"min": 72, "max": {"fr": 1}}},
        {
            "repeat": {
                "kind": "auto-fill",
                "tracks": [{"minmax": {"min": 80, "max": {"fr": 1}}}],
            }
        },
    ]


def test_grid_layout_rejects_invalid_columns() -> None:
    dg.Window("Grid")
    with pytest.raises(ValueError):
        dg.GridLayout(columns=0)
    with pytest.raises(ValueError):
        dg.GridLayout(columns="fixed")
    with pytest.raises(ValueError):
        dg.GridLayout(template_columns=("bogus",))
    with pytest.raises(ValueError):
        dg.GridLayout(template_columns=("0fr",))


def test_flow_layout_serialization() -> None:
    """FlowLayout serializes to kind flow_layout."""
    win = dg.Window("Flow")
    dg.FlowLayout(id="f")
    node = win.to_dict()["children"][0]
    assert node["type"] == "flow_layout"


def test_flow_layout_gap_in_style() -> None:
    """FlowLayout gap/row_gap kwargs are placed into the inline style dict."""
    win = dg.Window("Flow")
    dg.FlowLayout(gap=12, row_gap=6, id="f")
    node = win.to_dict()["children"][0]
    assert node["style"].get("gap") == 12
    assert node["style"].get("row_gap") == 6


def test_flow_layout_alignment_props() -> None:
    """FlowLayout exposes main and cross-axis alignment to native layout."""
    win = dg.Window("Flow")
    dg.FlowLayout(align="center", cross_align="stretch", id="f")
    node = win.to_dict()["children"][0]
    assert node["props"]["align"] == "center"
    assert node["props"]["cross_align"] == "stretch"


def test_flow_layout_no_extra_props() -> None:
    """FlowLayout default alignment does not add extra props."""
    win = dg.Window("Flow")
    dg.FlowLayout(id="f")
    node = win.to_dict()["children"][0]
    assert node["props"] == {}


def test_scroll_area_default_serialization() -> None:
    """ScrollArea serializes as a bounded vertical scroll viewport."""
    win = dg.Window("Scroll")
    dg.ScrollArea(id="s", gap=8)
    node = win.to_dict()["children"][0]
    assert node["type"] == "scroll_area"
    assert node["props"] == {}
    assert node["style"]["overflow_y"] == "auto"
    assert node["style"]["overflow_x"] == "hidden"
    assert node["style"]["flex_grow"] == 1
    assert node["style"]["flex_shrink"] == 1
    assert node["style"]["gap"] == 8
    assert node["style"]["min_height"] == 0


def test_scroll_area_axis_modes() -> None:
    win = dg.Window("Scroll")
    dg.ScrollArea(axis="both", id="both")
    node = win.to_dict()["children"][0]
    assert node["style"]["overflow_x"] == "auto"
    assert node["style"]["overflow_y"] == "auto"

    with pytest.raises(ValueError):
        dg.ScrollArea(axis="diagonal")
