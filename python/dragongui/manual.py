from __future__ import annotations

import inspect
from importlib import metadata as importlib_metadata
import re
from dataclasses import dataclass, field
from typing import Any, Iterable


_HELP_SCHEMA_VERSION = 1
_LIBRARY_VERSION_FALLBACK = "0.1.0"


def _library_version() -> str:
    try:
        return importlib_metadata.version("dragongui")
    except importlib_metadata.PackageNotFoundError:
        return _LIBRARY_VERSION_FALLBACK


def _norm(name: str) -> str:
    return name.strip().replace("-", "_").lower()


@dataclass
class HelpSection:
    """A small structured manual section exposed through attribute access."""

    name: str
    title: str
    summary: str
    body: str = ""
    children: dict[str, "HelpSection"] = field(default_factory=dict)
    aliases: dict[str, "HelpSection"] = field(default_factory=dict)
    metadata: dict[str, object] = field(default_factory=dict)

    def __post_init__(self) -> None:
        self.children = {_norm(name): section for name, section in self.children.items()}
        self.aliases = {_norm(name): section for name, section in self.aliases.items()}

    def __call__(self, topic: str | None = None) -> str:
        """Return this section, or a nested section when a topic path is supplied."""
        if topic is not None:
            return self.section(topic)()
        return self.render()

    def __getattr__(self, name: str) -> "HelpSection":
        key = _norm(name)
        if key in self.children:
            return self.children[key]
        if key in self.aliases:
            return self.aliases[key]
        raise AttributeError(f"DragonGUI help has no section {name!r}")

    def __dir__(self) -> list[str]:
        return sorted(set(super().__dir__()) | set(self.children) | set(self.aliases))

    def __repr__(self) -> str:
        return self.render()

    def __str__(self) -> str:
        return self.render()

    def render(self) -> str:
        lines = [f"# {self.title}", "", self.summary.strip()]
        body = self.body.strip()
        if body:
            lines.extend(["", body])
        if self.children:
            lines.extend(["", "## Sections"])
            for name, child in self.children.items():
                lines.append(f"- `{name}`: {child.summary}")
        return "\n".join(lines).rstrip() + "\n"

    def section(self, path: str) -> "HelpSection":
        node: HelpSection = self
        for part in path.replace("/", ".").split("."):
            if not part:
                continue
            key = _norm(part)
            if key in node.children:
                node = node.children[key]
            elif key in node.aliases:
                node = node.aliases[key]
            else:
                raise KeyError(f"DragonGUI help section not found: {path!r}")
        return node

    def topics(self) -> list[str]:
        return list(self._walk_paths())

    def search(self, query: str) -> list[dict[str, str]]:
        """Return matching help sections as dictionaries with path/title/summary."""
        words = [word for word in _norm(query).split() if word]
        if not words:
            return []
        scored: list[tuple[int, str, dict[str, str]]] = []
        for path, section in self._walk():
            if not path:
                continue
            path_text = _norm(path)
            title_text = _norm(section.title)
            summary_text = _norm(section.summary)
            body_text = _norm(section.body)
            metadata_text = _norm(" ".join(str(value) for value in section.metadata.values()))
            haystack = " ".join([path_text, title_text, summary_text, body_text, metadata_text])
            if all(word in haystack for word in words):
                score = 0
                for word in words:
                    score += 8 if word in title_text else 0
                    score += 6 if word in path_text else 0
                    score += 4 if word in summary_text else 0
                    score += 3 if word in metadata_text else 0
                    score += 1 if word in body_text else 0
                scored.append(
                    (
                        score,
                        path,
                        {
                            "path": path,
                            "title": section.title,
                            "summary": section.summary,
                        },
                    )
                )
        scored.sort(key=lambda item: (-item[0], item[1]))
        return [match for _score, _path, match in scored]

    def find_symbol(self, name: str) -> dict[str, str] | None:
        """Return the best exact public-symbol match, if the manual has one."""
        wanted = _norm(name)
        for path, section in self._walk():
            symbol = section.metadata.get("symbol")
            if symbol is not None and _norm(str(symbol)) == wanted:
                return {"path": path, "title": section.title, "summary": section.summary}
        matches = self.search(name)
        return matches[0] if matches else None

    def to_dict(self, *, include_body: bool = True, _path: str = "") -> dict[str, object]:
        path = f"{_path}.{self.name}" if _path and self.name else self.name or _path
        out: dict[str, object] = {
            "name": self.name,
            "path": path,
            "title": self.title,
            "summary": self.summary,
            "children": {
                name: child.to_dict(include_body=include_body, _path=path)
                for name, child in self.children.items()
            },
        }
        if not path:
            out["schema_version"] = _HELP_SCHEMA_VERSION
            out["library_version"] = _library_version()
        if include_body:
            out["body"] = self.body
        if self.metadata:
            out["metadata"] = dict(self.metadata)
        if self.aliases:
            out["aliases"] = {name: section.name for name, section in self.aliases.items()}
        return out

    def _walk(self, prefix: str = "") -> Iterable[tuple[str, "HelpSection"]]:
        path = f"{prefix}.{self.name}" if prefix and self.name else self.name
        yield path, self
        for child in self.children.values():
            yield from child._walk(path)

    def _walk_paths(self, prefix: str = "") -> Iterable[str]:
        for path, _section in self._walk(prefix):
            if path:
                yield path


def _section(
    name: str,
    title: str,
    summary: str,
    body: str = "",
    children: list[HelpSection] | None = None,
    metadata: dict[str, object] | None = None,
) -> HelpSection:
    return HelpSection(
        name=name,
        title=title,
        summary=summary,
        body=body,
        children={child.name: child for child in children or []},
        metadata=metadata or {},
    )


_LAYOUT_WIDGETS = (
    "Window",
    "HLayout",
    "VLayout",
    "ScrollArea",
    "GridLayout",
    "FlowLayout",
    "Splitter",
    "Pane",
    "Panel",
    "Collapsible",
    "Modal",
    "Separator",
    "Spacer",
    "Sidebar",
    "StatusBar",
)

_NAVIGATION_WIDGETS = (
    "MenuBar",
    "Menu",
    "MenuItem",
    "ContextMenu",
    "Tooltip",
    "Tabs",
    "Tab",
    "Pages",
    "Page",
    "NavItem",
    "Toolbar",
    "ToolbarSeparator",
    "Breadcrumbs",
    "SearchBox",
    "Command",
    "CommandPalette",
)

_CONTROL_WIDGETS = (
    "Label",
    "Badge",
    "Tag",
    "LED",
    "Button",
    "SmallButton",
    "IconButton",
    "ImageButton",
    "ArrowButton",
    "Selectable",
    "SelectableList",
    "RadioButton",
    "RadioGroup",
    "TextInput",
    "TextArea",
    "CodeEditor",
    "LogView",
    "DateInput",
    "TimeInput",
    "DateTimeInput",
    "Slider",
    "RangeSlider",
    "ProgressBar",
    "LoadingSpinner",
    "NumberInput",
    "DragNumber",
    "DragVector",
    "Property",
    "PropertyGrid",
    "Dropdown",
    "Checkbox",
    "ToggleSwitch",
    "ColorPicker",
)

_DATA_WIDGETS = (
    "Image",
    "HtmlReport",
    "DataFrameTable",
    "Histogram",
    "BarChart",
    "Heatmap",
    "LinePlot",
    "PieChart",
    "Scatter3D",
    "ScatterPlot2D",
)

_EXTENSION_WIDGETS = ("DragSource", "DropTarget", "DropZone", "ExtensionWidget", "PaintWidget")

_WIDGET_SYMBOLS = (
    *_LAYOUT_WIDGETS,
    *_NAVIGATION_WIDGETS,
    *_CONTROL_WIDGETS,
    *_DATA_WIDGETS,
    *_EXTENSION_WIDGETS,
)

_DATACLASS_SYMBOLS = (
    "BarChartBar",
    "BarChartData",
    "BreadcrumbItem",
    "BreadcrumbSelection",
    "DragDropPayload",
    "HeatmapCell",
    "HistogramBins",
    "LinePlotPayload",
    "MeasureConstraints",
    "PaintContext",
    "PaintKeyEvent",
    "PaintPointerEvent",
    "PropertyChange",
    "ScatterFrameStream",
    "ScatterHit",
    "ScatterLiveFrame",
    "ScatterPayload",
    "ScatterPick",
    "ScatterStreamMetrics",
    "TableSelection",
    "TableSort",
    "Size",
)

_DIALOG_SYMBOLS = (
    "FileDialog",
    "open_file_dialog",
    "open_files_dialog",
    "pick_folder_dialog",
    "save_file_dialog",
    "alert",
    "confirm",
)

_COMPONENT_SYMBOLS = (
    "component",
    "ComponentCtx",
    "ComponentInstance",
    "StateSlot",
    "VNode",
    "Patch",
    "ResourceRef",
)

_RUNTIME_SYMBOLS = (
    "App",
    "LoadingScreen",
    "Theme",
    "ToastHandle",
    "BackendUnavailableError",
    "backend_info",
    "native_backend_available",
    "toast",
    "run_with_loading",
    "ThreadMonitor",
    "register_thread_role",
    "thread_role",
    "link_cameras",
    "unlink_cameras",
    "help",
    "HelpSection",
)

_PUBLIC_EXPORT_GROUPS: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("runtime", _RUNTIME_SYMBOLS),
    ("components", _COMPONENT_SYMBOLS),
    ("dialogs", _DIALOG_SYMBOLS),
    ("widgets.layout", _LAYOUT_WIDGETS),
    ("widgets.navigation", _NAVIGATION_WIDGETS),
    ("widgets.controls", _CONTROL_WIDGETS),
    ("widgets.data", _DATA_WIDGETS),
    ("widgets.extensions", _EXTENSION_WIDGETS),
    ("dataclasses", _DATACLASS_SYMBOLS),
)

_SECTION_NAME_OVERRIDES = {
    "DataFrameTable": "dataframe_table",
    "DateTimeInput": "date_time_input",
    "DropZone": "drop_zone",
    "HLayout": "h_layout",
    "HtmlReport": "html_report",
    "ImageButton": "image_button",
    "LED": "led",
    "LinePlot": "line_plot",
    "LoadingSpinner": "loading_spinner",
    "MenuBar": "menu_bar",
    "MenuItem": "menu_item",
    "NavItem": "nav_item",
    "NumberInput": "number_input",
    "RadioButton": "radio_button",
    "RadioGroup": "radio_group",
    "RangeSlider": "range_slider",
    "Scatter3D": "scatter3d",
    "ScatterPlot2D": "scatter_plot_2d",
    "ScrollArea": "scroll_area",
    "SmallButton": "small_button",
    "TextArea": "text_area",
    "TextInput": "text_input",
    "TimeInput": "time_input",
    "ToggleSwitch": "toggle_switch",
    "ToolbarSeparator": "toolbar_separator",
    "VLayout": "v_layout",
}

_WIDGET_KIND_OVERRIDES = {
    **_SECTION_NAME_OVERRIDES,
    "Breadcrumbs": "breadcrumbs",
    "ColorPicker": "color_picker",
    "CommandPalette": "command_palette",
    "ContextMenu": "context_menu",
    "DragNumber": "drag_number",
    "DragSource": "drag_source",
    "DragVector": "drag_vector",
    "DropTarget": "drop_target",
    "ExtensionWidget": "extension",
    "PaintWidget": "extension",
    "FlowLayout": "flow_layout",
    "GridLayout": "grid_layout",
    "IconButton": "icon_button",
    "PropertyGrid": "property_grid",
    "SelectableList": "selectable_list",
    "TreeNode": "tree_node",
    "TreeView": "tree_view",
}

_SIGNATURE_OVERRIDES = {
    "help": "help(topic: str | None = None) -> str",
    "link_cameras": "link_cameras(*scatters: Scatter3D) -> None",
    "unlink_cameras": "unlink_cameras(*scatters: Scatter3D) -> None",
}

_SYMBOL_NOTES = {
    "App": "Owns the native runtime bridge, stylesheets, loading screen, toasts, and thread-safe scheduling.",
    "Window": "Top-level application shell passed to `app.run(window)`.",
    "Panel": "Framed container for related controls or content; use it as a major dashboard building block.",
    "HLayout": "Horizontal flex container for rows, split regions, toolbars, and compact control groups.",
    "VLayout": "Vertical flex container for stacked forms, panels, and page content.",
    "GridLayout": "Responsive grid container; use masonry mode when cards have different natural heights.",
    "FlowLayout": "Wrapping row container for chips, badges, action buttons, and compact controls.",
    "ScrollArea": "Explicit scroll owner for content that can exceed the available viewport.",
    "Splitter": "Resizable split layout with one or more `Pane` children.",
    "Modal": "Overlay dialog container; use `show()` and `close()` at runtime.",
    "Button": "Standard command button with `on_click`.",
    "Badge": "Compact status label; use `level` and CSS parts for semantic styling.",
    "NumberInput": "Numeric field with stepper controls; use for bounded scalar form values.",
    "DragNumber": "Numeric field optimized for drag-to-adjust workflows.",
    "Dropdown": "Single-selection menu control.",
    "DataFrameTable": "Tabular data viewer with sort, selection, and live frame replacement support.",
    "LinePlot": "2D time-series and streaming line plot.",
    "Scatter3D": "3D point cloud widget with packed data paths, camera controls, and live updates.",
    "ScatterPlot2D": "2D point cloud widget for dense scalar/color plots.",
    "Heatmap": "Matrix visualization with scalar color and optional cell labels.",
    "BarChart": "Categorical bar chart with vertical and horizontal modes.",
    "CommandPalette": "Searchable command overlay backed by `Command` entries.",
    "ExtensionWidget": "Renderer extension leaf for third-party or custom native widget hooks.",
    "PaintWidget": "Pure-Python custom widget base that records a native display list through `PaintContext`.",
    "PaintContext": "Records display-list commands such as rects, lines, polylines, circles, text, and images for a `PaintWidget`.",
    "MeasureConstraints": "Constraint object passed to `PaintWidget.measure(...)`.",
    "Size": "Logical width/height object returned from `PaintWidget.measure(...)`.",
    "PaintKeyEvent": "Event object delivered to focused extension widget key callbacks.",
    "PaintPointerEvent": "Event object delivered to extension widget pointer and wheel callbacks.",
    "component": "Decorator for reusable stateful Python UI functions.",
    "ToastHandle": "Runtime handle returned from `toast(...)` for updating or dismissing notifications.",
}

_REFERENCE_DETAILS = {
    "ExtensionWidget": """
Use `ExtensionWidget` when a third-party/native leaf needs a stable serialized
extension type and JSON-compatible props. It is lower level than
`@dg.component`: choose components first when the widget can be built from
existing DragonGUI controls.

Event hooks:
- `on_click()` receives normal click activation.
- `on_pointer_down(PaintPointerEvent)`, `on_pointer_move(PaintPointerEvent)`,
  `on_pointer_up(PaintPointerEvent)`, and `on_wheel(PaintPointerEvent)` receive
  local pointer coordinates and deltas.
- `on_key_down(PaintKeyEvent)` fires when the extension widget is focused.

Layout rules:
- Use `intrinsic_width` and `intrinsic_height` for the natural size.
- CSS `width`/`height` can still override or stretch the leaf.
- `disabled=True` suppresses hit testing and callbacks.

Related probes:
- `examples/css_feature_probes/custom_composite_widget_probe.py`
- `examples/css_feature_probes/paint_widget_events_probe.py`
""",
    "PaintWidget": """
Use `PaintWidget` when a custom widget needs native drawing but does not need a
new Rust widget type. Subclass it, override `measure(constraints)` and
`paint(ctx)`, and let DragonGUI serialize the display list.

Core pattern:
```python
class Sparkline(dg.PaintWidget):
    def __init__(self, values, **kwargs):
        self.values = list(values)
        super().__init__(extension_type="sparkline", **kwargs)

    def measure(self, constraints):
        return constraints.clamp(dg.Size(180, 64))

    def paint(self, ctx):
        ctx.rounded_rect(0, 0, ctx.width, ctx.height, radius=8, fill="surface")
        ctx.polyline([[0, 40], [60, 12], [180, 28]], stroke="accent", width=2)
```

Supported display-list commands:
- `ctx.rect(...)` and `ctx.rounded_rect(...)`
- `ctx.line(...)` and `ctx.polyline(...)`
- `ctx.circle(...)`
- `ctx.text(...)`
- `ctx.image(...)` using filesystem-backed paths like `Image`

Runtime rules:
- Call `repaint()` after changing fields used by `paint(ctx)`.
- Keep `paint(ctx)` deterministic and fast; it runs when props are generated.
- Use theme tokens such as `surface`, `text`, `muted`, and `accent` for colors
  when the widget should follow the app style.
- Use pointer/key callbacks only for lightweight input handling; schedule slow
  work through the app runtime.

Related probes:
- `examples/css_feature_probes/paint_widget_sparkline_probe.py`
- `examples/css_feature_probes/paint_widget_events_probe.py`
""",
    "PaintContext": """
`PaintContext` is the drawing recorder passed to `PaintWidget.paint(ctx)`.
It records logical coordinates inside the widget's measured size; the native
renderer scales the display list to the widget's actual layout rect.

Color values may be:
- theme tokens such as `accent`, `surface`, `text`, `muted`, `danger`
- CSS-like strings such as `#7dd3fc`
- RGB/RGBA tuples in either 0-1 float or 0-255 integer form

Use `stroke_width` for rectangle outlines. `line(...)` and `polyline(...)`
use `width` for stroke thickness.
""",
    "PaintPointerEvent": """
Fields:
- `widget_id`: target extension widget id.
- `event`: `pointer_down`, `pointer_move`, `pointer_up`, or `wheel`.
- `x`, `y`: window-space physical coordinates.
- `local_x`, `local_y`: coordinates relative to the extension widget rect.
- `dx`, `dy`: pointer movement delta or wheel delta depending on event type.
- `button`: mouse button name for pointer down/up, when available.

Use local coordinates for custom hit regions and window coordinates only when
coordinating with overlays or other widgets.
""",
    "PaintKeyEvent": """
Fields:
- `widget_id`: focused extension widget id.
- `event`: currently `key_down`.
- `key`: normalized logical key name such as `Enter`, `Escape`, or a character.
- `text`: text payload when the key generates text.
- `shift`, `ctrl`, `alt`, `super`: modifier state.
- `repeat`: whether the key event is an auto-repeat.

Click or tab-focus an extension widget before expecting key events.
""",
    "MeasureConstraints": """
Passed to `PaintWidget.measure(...)`. Use `constraints.clamp(dg.Size(...))` so
custom widgets respect explicit CSS/window constraints and do not overflow
their parent panel.
""",
    "Size": """
Return `Size(width, height)` from `PaintWidget.measure(...)`. Width and height
must be positive finite logical pixel values.
""",
}

_PARAMETER_NOTES = {
    "App": (
        "`theme` sets the starting colors/radii/fonts.",
        "`loading_screen` accepts `LoadingScreen` for native startup progress.",
        "Use `stylesheet(...)` after construction for CSS.",
    ),
    "Window": (
        "`title` is required.",
        "`width` and `height` are startup size hints; layout should still be responsive.",
    ),
    "Panel": (
        "`title` creates the panel header.",
        "`width`/`height` constrain the panel; prefer `flex_grow` for responsive fill.",
        "Use `scroll=True` or a nested `ScrollArea` when content can overflow.",
    ),
    "HLayout": (
        "Use for rows and horizontal control groups.",
        "`gap`, `padding`, `align_items`, and `flex_grow` are the important layout controls.",
    ),
    "VLayout": (
        "Use for stacked panels/forms.",
        "`gap`, `padding`, `align_items`, and `flex_grow` are the important layout controls.",
    ),
    "GridLayout": (
        "`columns` fixes the column count.",
        "`min_column_width` enables responsive auto-fill style layouts.",
        "`masonry=True` packs variable-height cards without row-aligned gaps.",
    ),
    "FlowLayout": (
        "Use for chips, badges, tool rows, and wrapped action groups.",
        "`gap`, `row_gap`, and `align_items` control wrapping density.",
    ),
    "ScrollArea": (
        "Use as the explicit owner for scrollable content.",
        "`height` or flex constraints usually need to bound the scroll area.",
    ),
    "Splitter": (
        "`orientation` controls horizontal vs vertical panes.",
        "Use `Pane(size=..., min_size=..., flex=...)` for each resizable region.",
    ),
    "Pane": (
        "`size` is the current pane size along the splitter axis.",
        "`min_size`, `max_size`, and `flex` constrain resizing.",
    ),
    "Button": (
        "`text` is required.",
        "`on_click` should be short and non-blocking.",
        "`badge` adds compact status text to the button.",
    ),
    "IconButton": (
        "`icon` should be one of the built-in symbol names.",
        "Use CSS color/foreground to keep the glyph visible across themes.",
    ),
    "Dropdown": (
        "`items` must be non-empty.",
        "`value` should match one of the items.",
        "`on_change(value)` receives the selected item string.",
    ),
    "NumberInput": (
        "`value` is normalized to a float.",
        "`min`, `max`, and `step` constrain keyboard and stepper changes.",
        "`on_change(value)` receives the new float.",
    ),
    "DragNumber": (
        "`speed` controls drag sensitivity.",
        "`min`, `max`, and `step` constrain the adjusted value.",
    ),
    "RangeSlider": (
        "`value` is a `(low, high)` pair.",
        "`min`, `max`, and `step` define the track and keyboard increments.",
    ),
    "TextInput": (
        "`value` is the editable text.",
        "`placeholder` is shown when empty.",
        "`on_change(value)` receives committed text updates.",
    ),
    "TextArea": (
        "`rows` controls the preferred visible height.",
        "`wrap=False` enables horizontal scrolling for long lines.",
    ),
    "PropertyGrid": (
        "`values` is the backing mapping.",
        "`schema` describes labels/editors/sections.",
        "`on_change(PropertyChange)` receives edited keys and values.",
    ),
    "DataFrameTable": (
        "`frame` can be a pandas/polars-like object or DragonGUI table payload.",
        "`page_size` bounds visible row work.",
        "`on_select(TableSelection)` and `on_sort(TableSort)` receive structured payloads.",
    ),
    "LinePlot": (
        "`series`/`data` define startup values.",
        "Use `append_points(...)` for streaming updates.",
        "Use `auto_fit`, `window_size`, and interaction settings for live plots.",
    ),
    "Scatter3D": (
        "`frame`, `x`, `y`, and `z` define point columns.",
        "Use prepared payloads for high-frequency or high-count updates.",
        "`on_pick(ScatterPick)` receives selected point information.",
    ),
    "ScatterPlot2D": (
        "`frame`, `x`, and `y` define point columns.",
        "Use for flat dense plots; use `Scatter3D` only when orbit/depth is needed.",
    ),
    "Heatmap": (
        "`matrix` must be rectangular 2D numeric data.",
        "`x_labels` and `y_labels` should match column/row counts.",
        "`on_hover(HeatmapCell | None)` receives cell hover updates.",
    ),
    "BarChart": (
        "`data` or `BarChartData` defines categories and series.",
        "`orientation='horizontal'` needs enough left label margin.",
        "`on_hover(BarChartBar | None)` receives bar hover updates.",
    ),
    "CommandPalette": (
        "`commands` is a list of `Command` entries.",
        "`open` controls overlay visibility.",
        "`on_run(command)` can centralize command execution.",
    ),
    "ExtensionWidget": (
        "`extension_type` is required and should be stable.",
        "`props` must be JSON-compatible.",
        "`intrinsic_width` and `intrinsic_height` tell layout the natural leaf size.",
    ),
    "PaintWidget": (
        "`extension_type` identifies the custom widget family.",
        "`on_pointer_*`, `on_wheel`, and `on_key_down` are optional lightweight input hooks.",
        "Override `measure(...)` and `paint(ctx)` in subclasses.",
    ),
}

_EXAMPLE_METADATA: dict[str, dict[str, tuple[str, ...]]] = {
    "Panel": {
        "probes": (
            "examples/css_feature_probes/layout_panel_bounds_probe.py",
            "examples/css_feature_probes/layout_flex_stress_probe.py",
        )
    },
    "GridLayout": {
        "probes": (
            "examples/css_feature_probes/responsive_layout_probe.py",
            "examples/css_feature_probes/layout_grid_masonry_probe.py",
        )
    },
    "FlowLayout": {"probes": ("examples/css_feature_probes/layout_flex_stress_probe.py",)},
    "ScrollArea": {
        "probes": (
            "examples/css_feature_probes/overflow_scrollbar_probe.py",
            "examples/css_feature_probes/layout_scrollable_composites_probe.py",
        )
    },
    "Splitter": {
        "probes": (
            "examples/css_feature_probes/splitter_probe.py",
            "examples/css_feature_probes/layout_overlay_collision_probe.py",
        )
    },
    "Modal": {
        "probes": (
            "examples/css_feature_probes/overlay_stack_probe.py",
            "examples/css_feature_probes/layout_overlay_collision_probe.py",
        )
    },
    "MenuBar": {
        "probes": (
            "examples/css_feature_probes/navigation_widgets_probe.py",
            "examples/css_feature_probes/menu_overlays_probe.py",
        )
    },
    "Toolbar": {"probes": ("examples/css_feature_probes/toolbar_probe.py",)},
    "CommandPalette": {
        "probes": ("examples/css_feature_probes/command_palette_probe.py",)
    },
    "PropertyGrid": {"probes": ("examples/css_feature_probes/property_grid_probe.py",)},
    "CodeEditor": {
        "probes": (
            "examples/css_feature_probes/code_editor_probe.py",
            "examples/css_feature_probes/layout_scrollable_composites_probe.py",
        )
    },
    "LogView": {"probes": ("examples/css_feature_probes/log_view_probe.py",)},
    "LoadingSpinner": {"probes": ("examples/css_feature_probes/loading_spinner_probe.py",)},
    "ProgressBar": {"examples": ("examples/pytorch_training_dashboard.py",)},
    "DataFrameTable": {
        "probes": (
            "examples/css_feature_probes/data_table_upgrades_probe.py",
            "examples/css_feature_probes/layout_scrollable_composites_probe.py",
        )
    },
    "LinePlot": {
        "probes": (
            "examples/css_feature_probes/line_plot_stream_benchmark_probe.py",
            "examples/css_feature_probes/layout_plot_embedding_probe.py",
        )
    },
    "Scatter3D": {
        "probes": (
            "examples/css_feature_probes/scatter3d_probe.py",
            "examples/css_feature_probes/scatter3d_dense_probe.py",
        )
    },
    "ScatterPlot2D": {"probes": ("examples/css_feature_probes/scatter_plot_2d_probe.py",)},
    "Heatmap": {
        "probes": (
            "examples/css_feature_probes/heatmap_probe.py",
            "examples/css_feature_probes/layout_plot_embedding_probe.py",
        )
    },
    "BarChart": {"probes": ("examples/css_feature_probes/bar_chart_probe.py",)},
    "DragSource": {"probes": ("examples/css_feature_probes/drag_drop_probe.py",)},
    "DropTarget": {"probes": ("examples/css_feature_probes/drag_drop_probe.py",)},
    "DropZone": {"probes": ("examples/css_feature_probes/drag_drop_probe.py",)},
    "ExtensionWidget": {
        "probes": (
            "examples/css_feature_probes/custom_composite_widget_probe.py",
            "examples/css_feature_probes/paint_widget_events_probe.py",
        )
    },
    "PaintWidget": {
        "probes": (
            "examples/css_feature_probes/paint_widget_sparkline_probe.py",
            "examples/css_feature_probes/paint_widget_events_probe.py",
        )
    },
}


def _symbol_to_section_name(name: str) -> str:
    if name in _SECTION_NAME_OVERRIDES:
        return _SECTION_NAME_OVERRIDES[name]
    if "_" in name and name.lower() == name:
        return name
    out = re.sub(r"(?<=[A-Z])(?=[A-Z][a-z])", "_", name)
    out = re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", out)
    return out.lower()


def _widget_kind_for_symbol(name: str) -> str:
    return _WIDGET_KIND_OVERRIDES.get(name, _symbol_to_section_name(name))


def _object_for_symbol(name: str) -> Any | None:
    if name == "HelpSection":
        return HelpSection
    if name in _SIGNATURE_OVERRIDES:
        return None

    from . import _backend as backend_module
    from . import app as app_module
    from . import components as components_module
    from . import diagnostics as diagnostics_module
    from . import dialogs as dialogs_module
    from . import notifications as notifications_module
    from . import runtime as runtime_module
    from . import theme as theme_module
    from . import thread_monitor as thread_monitor_module
    from . import vdom as vdom_module
    from . import widgets as widgets_module

    modules = (
        widgets_module,
        app_module,
        components_module,
        dialogs_module,
        diagnostics_module,
        notifications_module,
        runtime_module,
        theme_module,
        thread_monitor_module,
        vdom_module,
        backend_module,
    )
    for module in modules:
        if hasattr(module, name):
            return getattr(module, name)
    return None


def _annotation_text(annotation: object) -> str:
    if annotation is inspect.Signature.empty:
        return ""
    if isinstance(annotation, str):
        text = annotation
    else:
        text = getattr(annotation, "__name__", None) or str(annotation)
    return text.replace("typing.", "")


def _default_text(name: str, default: object) -> str | None:
    if default is inspect.Signature.empty:
        return None
    if name == "parent" and type(default) is object:
        return "_AUTO_PARENT"
    if default is None or isinstance(default, (bool, int, float, str)):
        return repr(default)
    if default is Ellipsis:
        return "..."
    if isinstance(default, (tuple, list, dict, set)):
        return repr(default)
    text = repr(default)
    if re.match(r"<.+ at 0x[0-9a-fA-F]+>", text):
        return type(default).__name__
    return text


def _format_parameter(parameter: inspect.Parameter) -> str:
    name = parameter.name
    if parameter.kind is inspect.Parameter.VAR_POSITIONAL:
        name = f"*{name}"
    elif parameter.kind is inspect.Parameter.VAR_KEYWORD:
        name = f"**{name}"

    annotation = _annotation_text(parameter.annotation)
    if annotation:
        name = f"{name}: {annotation}"

    default = _default_text(parameter.name, parameter.default)
    if default is not None:
        name = f"{name} = {default}"
    return name


def _format_signature(name: str, signature: inspect.Signature) -> str:
    parts: list[str] = []
    inserted_keyword_marker = False
    positional_only_count = 0
    parameters = list(signature.parameters.values())
    for index, parameter in enumerate(parameters):
        if parameter.kind is inspect.Parameter.POSITIONAL_ONLY:
            positional_only_count += 1
        if (
            parameter.kind is inspect.Parameter.KEYWORD_ONLY
            and not inserted_keyword_marker
            and not any(
                previous.kind is inspect.Parameter.VAR_POSITIONAL
                for previous in parameters[:index]
            )
        ):
            parts.append("*")
            inserted_keyword_marker = True
        parts.append(_format_parameter(parameter))
        if positional_only_count and index + 1 == positional_only_count:
            parts.append("/")

    return_annotation = _annotation_text(signature.return_annotation)
    suffix = f" -> {return_annotation}" if return_annotation else ""
    return f"{name}({', '.join(parts)}){suffix}"


def _signature_for_symbol(name: str) -> str:
    if name in _SIGNATURE_OVERRIDES:
        return _SIGNATURE_OVERRIDES[name]
    obj = _object_for_symbol(name)
    if obj is None:
        return name
    try:
        return _format_signature(name, inspect.signature(obj))
    except (TypeError, ValueError):
        return name


def _css_parts_for_symbol(name: str) -> tuple[str, tuple[str, ...]]:
    from .widgets import _SUPPORTED_PARTS_BY_KIND

    kind = _widget_kind_for_symbol(name)
    return kind, tuple(sorted(_SUPPORTED_PARTS_BY_KIND.get(kind, ())))


def _summary_for_symbol(name: str, category: str) -> str:
    if name in _SYMBOL_NOTES:
        return _SYMBOL_NOTES[name]
    if category == "widget":
        return f"Public widget reference for `{name}`."
    if category == "dataclass":
        return f"Public payload/data structure reference for `{name}`."
    if category == "dialog":
        return f"Dialog API reference for `{name}`."
    if category == "component":
        return f"Component model reference for `{name}`."
    return f"Runtime API reference for `{name}`."


def _reference_leaf(
    symbol: str,
    category: str,
    *,
    related: tuple[str, ...] = (),
) -> HelpSection:
    signature = _signature_for_symbol(symbol)
    summary = _summary_for_symbol(symbol, category)
    css_kind = ""
    css_parts: tuple[str, ...] = ()
    if category == "widget":
        css_kind, css_parts = _css_parts_for_symbol(symbol)

    lines = [
        f"Public symbol: `{symbol}`.",
        f"Category: `{category}`.",
        "",
        "Signature:",
        f"`{signature}`",
        "",
        summary,
    ]
    parameter_notes = _PARAMETER_NOTES.get(symbol, ())
    if parameter_notes:
        lines.extend(
            [
                "",
                "Important parameters:",
                *(f"- {note}" for note in parameter_notes),
            ]
        )
    if category == "widget":
        lines.extend(
            [
                "",
                f"CSS type selector: `{css_kind}`.",
                "CSS class selector: pass `class_=\"name\"` and style `.name { ... }`.",
                "Common construction: create it in a container context, or pass `parent=None` for detached children.",
            ]
        )
        if css_parts:
            lines.append(f"Supported CSS parts: {', '.join(f'`::{part}`' for part in css_parts)}.")
        else:
            lines.append("Supported CSS parts: none registered for this widget kind.")
    if category in {"widget", "runtime"}:
        lines.extend(
            [
                "",
                "Live updates:",
                "Use the widget's live methods after `app.run(...)` has bound native handles. Prefer live methods over rebuilding large subtrees.",
            ]
        )
    extra = _REFERENCE_DETAILS.get(symbol)
    if extra:
        lines.extend(["", "Details:", extra.strip()])
    example_metadata = _EXAMPLE_METADATA.get(symbol, {})
    if example_metadata:
        lines.append("")
        lines.append("Examples and probes:")
        for label in ("examples", "probes"):
            paths = example_metadata.get(label, ())
            if paths:
                lines.append(f"- {label}: " + ", ".join(f"`{path}`" for path in paths))
    if related:
        lines.extend(["", "Related symbols: " + ", ".join(f"`{name}`" for name in related) + "."])

    metadata: dict[str, object] = {
        "symbol": symbol,
        "category": category,
        "signature": signature,
    }
    if css_kind:
        metadata["css_type"] = css_kind
        metadata["css_parts"] = list(css_parts)
    if parameter_notes:
        metadata["parameter_notes"] = list(parameter_notes)
    for label, paths in example_metadata.items():
        metadata[label] = list(paths)

    return _section(
        _symbol_to_section_name(symbol),
        symbol,
        summary,
        "\n".join(lines),
        metadata=metadata,
    )


def _group_section(
    name: str,
    title: str,
    summary: str,
    symbols: tuple[str, ...],
    category: str,
) -> HelpSection:
    children = [_reference_leaf(symbol, category) for symbol in symbols]
    body = "Public symbols in this group:\n" + "\n".join(f"- `{symbol}`" for symbol in symbols)
    return _section(name, title, summary, body, children)


def _css_parts_reference() -> HelpSection:
    from .widgets import _SUPPORTED_PARTS_BY_KIND

    lines = [
        "Generated from `dragongui.widgets._SUPPORTED_PARTS_BY_KIND`.",
        "",
        "Use CSS parts with `Widget::part { ... }` or `.class::part { ... }`.",
        "",
    ]
    for kind in sorted(_SUPPORTED_PARTS_BY_KIND):
        parts = ", ".join(f"`::{part}`" for part in sorted(_SUPPORTED_PARTS_BY_KIND[kind]))
        lines.append(f"- `{kind}`: {parts or 'no registered parts'}")
    return _section(
        "css_parts",
        "CSS Parts Reference",
        "Supported widget CSS parts, synchronized with the widget registry.",
        "\n".join(lines),
        metadata={"source": "widgets._SUPPORTED_PARTS_BY_KIND"},
    )


def _css_selectors_reference() -> HelpSection:
    lines = [
        "DragonGUI CSS supports type selectors, class selectors, pseudo states, and registered parts.",
        "",
        "Widget type selectors:",
    ]
    for symbol in _WIDGET_SYMBOLS:
        lines.append(f"- `{symbol}` -> `{_widget_kind_for_symbol(symbol)}`")
    lines.extend(
        [
            "",
            "Selector forms:",
            "- `Button { ... }` or `button { ... }` for widget type rules.",
            "- `.primary { ... }` for class rules from `class_=\"primary\"`.",
            "- `Button:hover { ... }`, `Dropdown:open { ... }`, and `Tab:selected { ... }` for state rules.",
            "- `NumberInput::field { ... }` and `Button::badge { ... }` for part rules.",
        ]
    )
    return _section(
        "css_selectors",
        "CSS Selectors Reference",
        "Type, class, state, and part selector forms supported by DragonGUI.",
        "\n".join(lines),
    )


def _css_states_reference() -> HelpSection:
    states = (
        "hover",
        "focus",
        "active",
        "disabled",
        "checked",
        "selected",
        "open",
        "expanded",
        "collapsed",
    )
    body = "Supported pseudo states:\n" + "\n".join(f"- `:{state}`" for state in states)
    return _section(
        "css_states",
        "CSS States Reference",
        "Pseudo states available to DragonGUI CSS rules.",
        body,
        metadata={"states": list(states)},
    )


def _css_properties_reference() -> HelpSection:
    body = """
Supported property groups:
- Layout: `width`, `height`, `min_width`, `min_height`, `max_width`, `max_height`, `flex_grow`, `flex_shrink`, `gap`, `padding`, `margin`, `align_items`, `justify_content`.
- Grid: `columns`, `rows`, `min_column_width`, grid gaps, and masonry options through widget props.
- Overflow: `overflow`, `overflow_x`, `overflow_y`, and explicit `ScrollArea` ownership.
- Visual: `background`, `color`, `border`, `border_radius`, `box_shadow`, `outline`, opacity, and named theme tokens.
- Text: font size, weight, family, line height, and alignment properties supported by the renderer.
- Motion: transform and transition properties supported by the native style engine.
- Plot-specific: labels, value-label colors, colorbar spacing, and registered plot parts.

Unsupported browser features should be treated as no-ops unless a feature probe shows support.
"""
    return _section(
        "css_properties",
        "CSS Properties Reference",
        "Supported CSS property families and practical limits.",
        body,
    )


def _css_limits_reference() -> HelpSection:
    body = """
DragonGUI CSS is a renderer-specific subset, not a browser engine.

Practical limits:
- Unsupported browser selectors and properties should be treated as no-ops.
- Prefer explicit layout widgets over browser-only layout concepts.
- Use registered widget parts from `reference.css_parts`; arbitrary pseudo
  elements are not available.
- Use feature probes for transitions, transforms, shadows, outlines, gradients,
  and plot-specific styles before relying on them in a production UI.
- Use theme tokens and supported color syntax instead of browser-specific
  functions unless a probe confirms support.
"""
    return _section(
        "css_limits",
        "CSS Limits Reference",
        "Unsupported or renderer-specific CSS behavior to avoid assuming.",
        body,
    )


def _live_methods_reference() -> HelpSection:
    body = """
Common live methods by family:
- Values: `set_value`, `set_checked`, dropdown/select setters, and text setters.
- Style: `set_style`, `set_class`, and stylesheet updates through `App`.
- Tables: `DataFrameTable.set_frame(...)`.
- Line plots: `LinePlot.append_points(...)`, series replacement, and prepared payload updates.
- Scatter plots: `set_points`, `set_prepared_points`, `enqueue_prepared_points`, `create_live_frame`, `fit`, and camera methods.
- Overlays: `Modal.show()`, `Modal.close()`, command palette show/close flows.
- Notifications: `toast_handle.update(...)` and `toast_handle.dismiss()`.

Keep live update callbacks short. For slow work, schedule from worker threads
with `app.call_soon_threadsafe(...)` and coalesce high-rate updates before they
reach the UI.
"""
    return _section(
        "live_methods",
        "Live Methods Reference",
        "Runtime mutation methods grouped by widget family.",
        body,
    )


def _callbacks_reference() -> HelpSection:
    body = """
Common callback shapes:
- `on_click()` for buttons, toolbar actions, menu items, and commands.
- `on_change(value)` for inputs, sliders, toggles, dropdowns, and editors.
- `on_select(selection)` for tables, selectable lists, tabs, pages, and navigation.
- `on_sort(TableSort)` for sortable tables.
- `on_hover(payload)` and `on_pick(payload)` for plots.
- `on_drop(DragDropPayload)` for drag/drop targets and zones.
- `Command.on_run()` and `CommandPalette.on_run(command)` for palette actions.
- `ExtensionWidget`/`PaintWidget` support `on_pointer_down(PaintPointerEvent)`,
  `on_pointer_move(PaintPointerEvent)`, `on_pointer_up(PaintPointerEvent)`,
  `on_wheel(PaintPointerEvent)`, and focused `on_key_down(PaintKeyEvent)`.

Callbacks execute on the UI event path. Do not block them with model training,
large dataframe work, network calls, or filesystem scans. Use background work
and live update methods for results.
"""
    return _section(
        "callbacks",
        "Callbacks Reference",
        "Callback argument forms and runtime cost guidance.",
        body,
    )


def _drag_drop_reference() -> HelpSection:
    symbols = ("DragSource", "DropTarget", "DropZone", "DragDropPayload")
    body = """
Drag/drop is composed from source widgets, target widgets, zones, and payloads.

Use:
- `DragSource(...)` for content that can start a drag.
- `DropTarget(...)` for one target.
- `DropZone(...)` for grouped targets or lane-style UIs.
- `DragDropPayload` in `on_drop` callbacks.
"""
    return _section(
        "drag_drop",
        "Drag And Drop Reference",
        "Drag/drop widgets, payloads, and callbacks.",
        body,
        [_reference_leaf(symbol, "widget" if symbol != "DragDropPayload" else "dataclass") for symbol in symbols],
    )


def _build_reference_sections() -> HelpSection:
    exports_lines: list[str] = []
    for group, symbols in _PUBLIC_EXPORT_GROUPS:
        exports_lines.append(f"### {group}")
        exports_lines.extend(f"- `{symbol}`" for symbol in symbols)
        exports_lines.append("")

    runtime = _group_section(
        "runtime",
        "Runtime Reference",
        "Application, backend, threading, themes, toasts, loading, and manual APIs.",
        _RUNTIME_SYMBOLS,
        "runtime",
    )
    components = _group_section(
        "components",
        "Components Reference",
        "Reusable component model, state slots, patches, resources, and VDOM nodes.",
        _COMPONENT_SYMBOLS,
        "component",
    )
    dialogs = _group_section(
        "dialogs",
        "Dialogs Reference",
        "Native file dialogs, alerts, and confirmation helpers.",
        _DIALOG_SYMBOLS,
        "dialog",
    )
    dataclasses = _group_section(
        "dataclasses",
        "Dataclasses And Payloads Reference",
        "Selection, plot, drag/drop, table, chart, and callback payload structures.",
        _DATACLASS_SYMBOLS,
        "dataclass",
    )
    css_parts = _css_parts_reference()
    css_selectors = _css_selectors_reference()
    css_states = _css_states_reference()
    css_properties = _css_properties_reference()
    css_limits = _css_limits_reference()
    widgets = _group_section(
        "widgets",
        "Widgets Reference",
        "All public widget classes and widget-adjacent command models.",
        _WIDGET_SYMBOLS,
        "widget",
    )
    exports = _section(
        "exports",
        "Public Exports Reference",
        "Grouped inventory of every public `dragongui.__all__` symbol.",
        "\n".join(exports_lines).rstrip(),
        metadata={
            "groups": [group for group, _symbols in _PUBLIC_EXPORT_GROUPS],
            "symbol_count": sum(len(symbols) for _group, symbols in _PUBLIC_EXPORT_GROUPS),
        },
    )

    reference = _section(
        "reference",
        "Structured API Reference",
        "Machine-readable public API, widget, CSS, callback, and live-update reference.",
        """
This branch complements the guide sections with a stable reference tree for
LLMs and tooling. Prefer `find_symbol()` or `reference.widgets.<name>` when
you need a specific symbol.
""",
        [
            exports,
            runtime,
            components,
            dialogs,
            dataclasses,
            widgets,
            _drag_drop_reference(),
            css_parts,
            css_selectors,
            css_states,
            css_properties,
            css_limits,
            _live_methods_reference(),
            _callbacks_reference(),
        ],
    )
    reference.aliases.update({"css_type_selectors": css_selectors})
    return reference


def _build_manual() -> HelpSection:
    llm_rules = _section(
        "llm_rules",
        "LLM Build Rules",
        "Rules an agent should follow when generating DragonGUI code.",
        """
When generating DragonGUI code:
- Use `import dragongui as dg`.
- Prefer the public flat API from `dragongui.__all__`; do not import private
  modules for normal app code.
- Build one `dg.Window(...)`, populate it with context managers, then call
  `app.run(win)`.
- Use `parent=None` only when constructing detached widgets for `children=[...]`
  or reusable component returns.
- Prefer `Panel`, `HLayout`, `VLayout`, `GridLayout`, `FlowLayout`, and
  `ScrollArea` over absolute positioning.
- Prefer `gap`, `padding`, `flex_grow`, `width`, `height`, and CSS classes over
  manual spacer-heavy layouts.
- Keep callbacks short. For slow work, use a background thread and
  `app.call_soon_threadsafe(...)` for UI changes.
- Use live widget methods for runtime updates instead of rebuilding the whole
  widget tree.
- Use CSS tokens (`accent`, `surface`, `text`, `danger`, etc.) for reusable
  styling; use literal colors only for data encodings.
- For large plots/tables, choose packed/live update APIs.
""",
    )

    quickstart = _section(
        "quickstart",
        "Quickstart",
        "Minimal DragonGUI application patterns.",
        """
Use a flat import and construct widgets inside a Window:

```python
import dragongui as dg

app = dg.App(theme=dg.Theme.dark())
win = dg.Window("Tool", width=1200, height=800)

with dg.HLayout():
    with dg.Panel("Controls", width=320):
        dg.Button("Run", on_click=lambda: print("run"))
        dg.Checkbox("Enabled", checked=True)
    dg.Label("Main content")

app.run(win)
```

Rules:
- Create exactly one top-level `Window` for `app.run(window)`.
- Use container context managers for normal construction.
- Pass `parent=None` when creating children explicitly for later insertion.
- Keep long-running work out of callbacks; use a thread and `app.call_soon_threadsafe`.
""",
    )

    app_api = _section(
        "app",
        "App",
        "Application object, runtime bridge, stylesheets, and diagnostics.",
        """
Signature:
`App(title="DragonGUI", theme=None, metadata={}, loading_screen=None)`

Use:
```python
app = dg.App(theme=dg.Theme.dark())
app.stylesheet("Button.primary { background: accent; color: background; }")
result = app.run(win)
```

Important methods:
- `document(window)` serializes startup state without running the GUI.
- `stylesheet(css)`, `load_stylesheet(path)`, and `clear_stylesheets()` manage
  user CSS before and during runtime.
- `run(window)` starts the native event loop.
- `debug_snapshot(timeout_ms=1000)` returns runtime/layout/style diagnostics.
- `request_redraw()` asks the native runtime to draw another frame.
- `request_exit()` closes the app.
- `call_soon_threadsafe(fn)` schedules a callable from producer/background threads.
- `toast(...)` posts a native notification and returns a `ToastHandle`.
""",
    )
    window_api = _section(
        "window",
        "Window",
        "Top-level native window and root widget tree.",
        """
Signature:
`Window(title, width=1024, height=768, id=None, key=None, class_=None, style=None)`

Rules:
- Create the `Window` outside any active container context.
- The window is a container, so widgets created after it attach into it unless
  another context is active.
- For app shells, place `MenuBar` first, the main body next, and `StatusBar`
  last.
- Use `style={"overflow_y": "auto"}` when the whole window body should scroll.
""",
    )
    loading_api = _section(
        "loading_screen",
        "Loading Screen",
        "Startup loading screen configuration.",
        """
Signature:
`LoadingScreen(enabled=True, title="Loading", message=None, background=None,
text=None, accent=None, show_spinner=True, show_progress=False,
min_duration_ms=120)`

Use:
```python
app = dg.App(loading_screen=dg.LoadingScreen(
    title="Preparing dashboard",
    message="Loading data...",
    show_progress=True,
))
```

Pass `loading_screen=False` to disable the startup loading screen.
Use `run_with_loading(...)` when a costly startup builder should run while the
loading UI is already visible.
""",
    )
    threading_api = _section(
        "threads",
        "Threading",
        "How to update the UI safely from background work.",
        """
Callbacks should not block the UI. For work that takes time:

```python
def worker():
    result = train_one_epoch()
    app.call_soon_threadsafe(lambda: progress.set_value(result.progress))
```

Use `register_thread_role("trainer")` or `with dg.thread_role("loader"):` to
label worker threads in diagnostics. Background threads should enqueue live
widget calls rather than mutating native state directly.
""",
    )
    diagnostics_api = _section(
        "diagnostics",
        "Diagnostics",
        "Runtime inspection for layout, style, commands, and frame timing.",
        """
Use `app.debug_snapshot()` to inspect:
- Computed styles and matched CSS rules.
- Layout rectangles and widget tree data.
- Command queue activity and timings.
- Frame timing, upload timing, renderer status, active drags/scrolling.

Use `DRAGONGUI_SMOKE_FRAMES=3` for short example smoke runs. Use feature probes
in `examples/css_feature_probes` to isolate layout, CSS, and widget behavior.
""",
    )
    app_model = _section(
        "app_model",
        "App Model",
        "How Python, the native runtime, widgets, and live handles fit together.",
        """
DragonGUI is retained and native-rendered. Python creates a typed widget tree;
Rust owns the window, input, layout, text, drawing, GPU buffers, scrolling, and
retained widget state.

Core objects:
- `App(theme=None, loading_screen=None)` owns runtime connection and stylesheets.
- `Window(title, width=1024, height=768)` is the root widget.
- `Theme.dark()` and `Theme.light()` provide token values used by CSS.
- Every widget has `id`, `key`, `class_`, `style`, `tooltip`, and `parent`.

Live methods such as `set_value`, `set_style`, `set_frame`, `set_points`,
`fit`, `show`, and `close` enqueue native updates after the app is running.
""",
        [app_api, window_api, loading_api, threading_api, diagnostics_api],
    )

    panels = _section(
        "panels",
        "Panels",
        "Framed titled containers for grouped controls and dashboard cards.",
        """
Use `Panel(title=None, width=None)` for grouped controls. Panels lay out their
children vertically and can scroll overflowing content.

```python
with dg.Panel("Hyperparameters", width=300, style={"gap": 10, "padding": 14}):
    dg.NumberInput(0.001, min=0, step=0.0001)
    dg.Dropdown(["adamw", "sgd"], value="adamw")
```

Guidelines:
- Use `width` or CSS width for side panels; let main content flex.
- Use `gap` and `padding` on the panel, not spacer widgets between every row.
- Use `Panel::accent` in CSS for the left accent strip.
- For variable card grids, use `GridLayout(..., masonry=True)` when available.
""",
    )
    splitters = _section(
        "splitters",
        "Splitters",
        "Resizable pane containers.",
        """
Use `Splitter(orientation="horizontal", sizes=None, min_sizes=None,
max_sizes=None, gutter_size=6)` when the user should resize panes.

Each child of a splitter is normally a `Pane`:
```python
with dg.Splitter(orientation="horizontal", sizes=(280, "1fr")):
    with dg.Pane(min_size=220):
        dg.Panel("Inspector")
    with dg.Pane(flex=1):
        dg.Panel("Preview")
```

Use fixed numeric sizes for exact starts and `"fr"` strings for flexible panes.
`Pane(size=...)` fixes the initial preferred size; `flex` controls remaining
space. Use `Splitter::gutter` to style the drag handle.
""",
    )
    flex = _section(
        "flex",
        "Flex Layout",
        "Rows, columns, spacing, wrapping, and shrink behavior.",
        """
Use `HLayout` for rows and `VLayout` for columns. Both support `gap`, `width`,
`height`, `flex_grow`, `flex_shrink`, padding, and overflow styles.

Common patterns:
```python
with dg.HLayout(style={"gap": 12}):
    with dg.Panel("Controls", width=320): ...
    with dg.VLayout(style={"flex_grow": 1, "gap": 12}): ...
```

Use `FlowLayout` for badges, tags, and compact buttons that should wrap.
Use `Spacer(width=...)` or `Spacer(style={"flex_grow": 1})` only when the
blank space is semantically useful.

Common row recipe:
```python
with dg.HLayout(style={"gap": 8, "align_items": "center"}):
    dg.Label("Status", style={"width": 90})
    dg.Badge("ready", level="success")
    dg.Spacer(style={"flex_grow": 1})
    dg.Button("Restart")
```
""",
    )
    flow = _section(
        "flow",
        "Flow Layout",
        "Wrapping intrinsic-width rows for badges, tags, and compact tools.",
        """
Signature:
`FlowLayout(gap=None, row_gap=None, align="start", cross_align="start")`

Use `FlowLayout` when children should stay in a row while space allows and wrap
instead of clipping when the panel narrows.

Good fits:
- Badges and tags.
- Tool buttons.
- Filter chips.
- Action rows with a variable number of compact controls.

Avoid forcing badge rows into `HLayout` when width is uncertain; `FlowLayout`
is more robust because it wraps.
""",
    )
    grids = _section(
        "grids",
        "Grid And Masonry",
        "Responsive dashboard card layouts.",
        """
`GridLayout(columns=2, min_column_width=320, gap=12)` creates responsive card
grids. Integer `columns` acts as a maximum when `min_column_width` is present.

Use grid for dashboards with repeated cards. Use masonry card packing when
cards have different natural heights and should not reserve a full row height.

Examples:
```python
with dg.GridLayout(columns=3, min_column_width=260, gap=12):
    dg.Panel("Loss")
    dg.Panel("Accuracy")
    dg.Panel("Throughput")

with dg.GridLayout(columns=2, min_column_width=300, masonry=True, gap=12):
    dg.Panel("Short Card")
    dg.Panel("Tall Card")
```

Track values can be supplied through `template_columns` or CSS grid properties
when more control is needed.
""",
    )
    scroll = _section(
        "scroll",
        "Scrolling",
        "Bounded scroll areas and overflow ownership.",
        """
Use `ScrollArea(axis="y", height=...)` when a subregion owns scrolling.
Use `Window(..., style={"overflow_y": "auto"})` when the whole page should
scroll.

Avoid nested vertical scroll owners unless each area has a fixed height. A
`Panel` can scroll its own content when children exceed its bounds.

Common pattern:
```python
with dg.HLayout(style={"height": "100%"}):
    with dg.Panel("Controls", width=320): ...
    with dg.ScrollArea(axis="y", style={"flex_grow": 1}):
        with dg.GridLayout(columns=2, min_column_width=320): ...
```
""",
    )
    overlays = _section(
        "overlays",
        "Overlays",
        "Menus, context menus, tooltips, modals, and toasts.",
        """
Overlay widgets should not consume layout space:
- `MenuBar`, `Menu`, and `MenuItem` build top menus.
- `ContextMenu(target=widget)` attaches a right-click popup.
- `Tooltip(target=widget)` creates a rich hover overlay.
- `Modal(..., open=False)` floats over content; call `show()` and `close()`.
- `toast(...)` shows non-blocking status feedback and returns a `ToastHandle`.

Use `close_button=True` on modals when a top-right close affordance should be
rendered by the framework.
""",
    )
    layout = _section(
        "layout",
        "Layout",
        "Containers and rules for building stable native layouts.",
        """
Start with one main `HLayout` or `VLayout`, then add panels and content areas.
Prefer container gap/padding over manual pixel placement.

Important:
- Use `FlowLayout` for rows of variable-width pills/buttons.
- Use `GridLayout` for dashboards.
- Give scroll owners a bound (`height`, `flex_grow`, or window body space).
- Do not use overlays as normal children when you expect them to float.
""",
        [panels, flex, flow, grids, splitters, scroll, overlays],
    )

    text_inputs = _section(
        "text",
        "Text Inputs",
        "Single-line, multiline, code, and log text widgets.",
        """
Use:
- `TextInput(value="", placeholder="", on_change=None)` for one-line text.
- `TextArea(value="", rows=4, wrap=True, on_change=None)` for editable notes.
- `CodeEditor(value="", language="", rows=10, wrap=False, on_change=None)` for
  source-like text with gutter/line numbers.
- `LogView(lines=(), follow=True, max_lines=10000, rows=12, wrap=False)` for
  append-style operational output.

Long lines in `CodeEditor` and `LogView` can scroll horizontally when wrapping
is disabled. Use CSS parts such as `CodeEditor::gutter`,
`CodeEditor::line-number`, and `LogView::warning` for styling.
""",
    )
    numeric_inputs = _section(
        "numeric",
        "Numeric Inputs",
        "Number inputs, drag numbers, sliders, ranges, and vectors.",
        """
Use:
- `NumberInput(value=0, min=None, max=None, step=1, on_change=None)` when the
  value should be editable by keyboard and steppers.
- `DragNumber(value=0, min=None, max=None, step=1, speed=None)` for compact
  drag-edit fields.
- `DragVector(value, labels=None, component_width=88)` for vector-like values.
- `Slider(value=0, min=0, max=1, step=0.01)` for single bounded values.
- `RangeSlider(value=(0, 1), min=0, max=1, step=0.01)` for min/max ranges.

Validation:
- Numeric values must be finite.
- `step` must be finite and greater than zero.
- `min` cannot exceed `max`.

Styling parts:
- `NumberInput::field`, `stepper`, `stepper-up`, `stepper-down`,
  `stepper-divider`, `divider`, `caret`
- `DragNumber::field`, `value`, `grip`
- `Slider::track`, `fill`, `thumb`
- `RangeSlider::track`, `range`, `thumb-min`, `thumb-max`, `label`
""",
    )
    selection_inputs = _section(
        "selection",
        "Selection Controls",
        "Dropdowns, checkboxes, switches, radio groups, selectable lists, and trees.",
        """
Use:
- `Dropdown(items, value=None, on_change=None)` for one-of-many compact choices.
- `Checkbox(label, checked=False, on_change=None)` for boolean settings.
- `ToggleSwitch(label, checked=False, label_position="right")` for prominent
  boolean settings.
- `RadioGroup(items, value=None, orientation="vertical")` for visible one-of-many.
- `SelectableList(items, selection_mode="single" | "multi")` for list selection.
- `TreeView(items, selected=None)` and `TreeNode(...)` for hierarchical data.

Prefer `RadioGroup` over `Dropdown` when the option set is small and all choices
should be visible. Prefer `SelectableList` for larger lists with filtering or
multi-select workflows.
""",
    )
    temporal_inputs = _section(
        "date_time",
        "Date And Time Inputs",
        "Date, time, and datetime text controls with normalized values.",
        """
Use:
- `DateInput(value="", placeholder=None, on_change=None)`
- `TimeInput(value="", placeholder=None, on_change=None)`
- `DateTimeInput(value="", placeholder=None, on_change=None)`

Values may be strings or Python `date`, `time`, and `datetime` objects. Values
serialize as ISO-like strings. Use these when you want consistent app-level
date/time input without building custom parsers.
""",
    )
    forms = _section(
        "forms",
        "Forms And Property Grids",
        "Structured edit panels and schema-driven controls.",
        """
Manual form row:
```python
with dg.Panel("Training"):
    with dg.HLayout(style={"gap": 8, "align_items": "center"}):
        dg.Label("Learning rate", style={"width": 120})
        dg.NumberInput(0.001, min=0, step=0.0001)
```

Property grid:
```python
dg.PropertyGrid(
    values={"lr": 0.001, "optimizer": "adamw", "enabled": True},
    schema={
        "lr": {"label": "Learning Rate", "type": "number", "min": 0, "step": 0.0001},
        "optimizer": {"type": "choice", "choices": ["adamw", "sgd"]},
    },
    on_change=lambda change: print(change.key, change.value),
)
```

Use `Property(label, editor)` for hand-authored rows and `PropertyGrid` when a
dictionary/schema should generate a panel.
""",
    )
    inputs = _section(
        "inputs",
        "Inputs",
        "Text, numeric, selection, toggle, and date/time controls.",
        """
Inputs:
- `TextInput(value="", placeholder="", on_change=None)`
- `TextArea(value="", rows=4, wrap=True, on_change=None)`
- `NumberInput(value=0, min=None, max=None, step=1, on_change=None)`
- `DragNumber` and `DragVector` for compact numeric drag editing.
- `Slider`, `RangeSlider`, `Checkbox`, `ToggleSwitch`, `RadioGroup`
- `Dropdown(items, value=None, on_change=None)`
- `DateInput`, `TimeInput`, and `DateTimeInput`
- `ColorPicker`

Use live setters (`set_value`, `set_checked`) for programmatic changes.
Callbacks receive the new value.
""",
        [text_inputs, numeric_inputs, selection_inputs, temporal_inputs, forms],
    )
    tabs_pages = _section(
        "tabs_pages",
        "Tabs And Pages",
        "Route-style content switching.",
        """
Use `Tabs`/`Tab` when the tab itself owns the page contents:
```python
with dg.Tabs(value="metrics"):
    with dg.Tab("Metrics", value="metrics"):
        dg.LinePlot(frame, y="loss")
```

Use `Pages`/`Page` with `NavItem` or custom navigation when the navigation and
content should be separate:
```python
with dg.HLayout():
    with dg.Sidebar():
        dg.NavItem("Overview", page="overview")
    with dg.Pages(value="overview"):
        with dg.Page("overview"):
            dg.Label("Overview")
```

Call `set_value(value, notify=False)` for programmatic route changes.
""",
    )
    menus_toolbars = _section(
        "menus_toolbars",
        "Menus And Toolbars",
        "Menu bars, context menus, toolbars, and icon buttons.",
        """
Use:
- `MenuBar` with `Menu(label)` and `MenuItem(label, on_click=...)` for top menus.
- `ContextMenu(target=widget)` for right-click actions.
- `Toolbar(orientation="horizontal", compact=True)` for dense commands.
- `IconButton(icon, tooltip=...)`, `SmallButton`, `ImageButton`, and
  `ArrowButton(direction)` for command surfaces.

Toolbars should use icons for obvious repeated actions and tooltips for
meaning. Use `ToolbarSeparator` to group commands.
""",
    )
    breadcrumbs_help = _section(
        "breadcrumbs",
        "Breadcrumbs",
        "Path navigation and compact current-location display.",
        """
Use `Breadcrumbs(items, current=None, separator=">", max_items=None,
click_current=False, on_select=None)` for project/file/object paths. The
callback receives a `BreadcrumbSelection` with the selected item metadata.
""",
    )
    command_palette_help = _section(
        "command_palette",
        "Command Palette",
        "Searchable command modal.",
        """
Use `Command(id, title, on_run=None, subtitle=None, keywords=(), disabled=False)`
and `CommandPalette(commands, open=False, close_on_run=True, on_run=None)`.

Pattern:
```python
commands = [
    dg.Command("run", "Run Training", on_run=start_training, keywords=["train"]),
    dg.Command("stop", "Stop Run", on_run=stop_training),
]
palette = dg.CommandPalette(commands, close_button=True)
```

Call `palette.show()` and `palette.close()` at runtime.
""",
    )
    navigation = _section(
        "navigation",
        "Navigation",
        "Tabs, pages, nav items, breadcrumbs, menu bars, and toolbars.",
        """
Use `Tabs`/`Tab` for tabbed content and `Pages`/`Page` for route-like page
containers. `NavItem(label, page=...)` targets a page route. `Breadcrumbs`
represents a path and emits selected path changes.

`Toolbar` and `IconButton`/tool buttons are for dense command rows. Keep labels
short and use badges only when the count or state matters.
""",
        [tabs_pages, menus_toolbars, breadcrumbs_help, command_palette_help],
    )
    tables = _section(
        "tables",
        "Tables",
        "DataFrame-style tabular display with selection, sorting, and scrolling.",
        """
Use `DataFrameTable(frame, page_size=100, on_select=None)` for dataframe-like
objects. It extracts metadata and may upload packed column buffers.

Callbacks:
```python
def selected(sel: dg.TableSelection):
    print(sel.row_index, sel.column, sel.value)
```

CSS parts include `header`, `row`, `row-selected`, and `grid-line`.

Use `sortable=True` and `resizable_columns=True` unless there is a reason to
lock the table down. Runtime `set_frame(frame)` updates the data without
replacing surrounding layout.
""",
    )
    line_plot = _section(
        "line_plot",
        "LinePlot",
        "Streaming-friendly 2D line plots.",
        """
Signature highlights:
`LinePlot(frame=None, x=None, y=..., labels=None, colors=None,
show_toolbar=False, show_legend=False, interaction="inspect",
line_width=2.0, line_style="solid", window_size=None, max_points=None)`

Use:
```python
plot = dg.LinePlot(frame, x="step", y=["loss", "accuracy"],
                   show_grid=True, show_toolbar=True, show_legend=True)
```

Live methods:
- `set_data(frame, x=None, y=None, ...)`
- `append_points(series, x_values, y_values)` for streaming.
- `clear(series=None)`
- `fit()`
- `set_line_width`, `set_grid_visible`, `set_axes_visible`,
  `set_ticks_visible`, `set_legend_visible`, `set_window_size`.

Performance rule: append or set packed/prepared data instead of rebuilding the
plot widget for every frame.
""",
    )
    scatter_plot = _section(
        "scatter",
        "Scatter3D And ScatterPlot2D",
        "GPU point-cloud plots with picking, streaming, labels, overlays, and camera controls.",
        """
Use `Scatter3D(frame, x, y, z, color=None, scalars=None, point_size=4.0)` for
true 3D data. Use `ScatterPlot2D(frame, x, y, ...)` for fixed 2D scatter.

Color inputs:
- `color="column"` or `scalars="column"` for scalar colormap values.
- `colors=array_or_column` for per-point RGB/RGBA.
- Categorical string/low-cardinality integer columns build legend entries.

Live/performance methods:
- `prepare_points(...)`
- `set_prepared_points(...)`
- `enqueue_prepared_points(...)`
- `create_live_frame(mode="primary")`
- `set_points(..., fit=True)`
- `set_lod(enabled=True, threshold=200000, factor=8)`
- `set_auto_point_size(True)`
- `set_interactive_render_scale(0.75)`
- `set_auto_quality(True, target_fps=10)`

Interaction helpers:
- `fit(bounds=None)`, `reset_camera()`, `view_xy()`, `view_xz()`, `view_yz()`,
  `view_isometric()`, `set_camera(state)`, `get_camera()`.
- `enable_point_picking`, `enable_rectangle_picking`, `enable_lasso_picking`,
  and `disable_picking`.
""",
    )
    charts = _section(
        "charts",
        "Histogram, BarChart, Heatmap, And PieChart",
        "Native chart widgets for common summaries.",
        """
Use:
- `Histogram(data, value=None, bins=30, mode="count")` for distributions.
- `BarChart(data=None, category=None, value=None, labels=None, values=None,
  aggregate="sum", orientation="vertical")` for categorical comparisons.
- `Heatmap(matrix, x_labels=None, y_labels=None, colormap="viridis")` for
  matrices and dense grids.
- `PieChart(data=None, labels=None, values=None, category=None, value=None,
  aggregate="count", donut=False)` for composition views.

Chart callbacks:
- `BarChart(..., on_hover=fn)` receives `BarChartBar`.
- `Heatmap(..., on_hover=fn)` receives `HeatmapCell`.

Use CSS parts such as `Heatmap::cell`, `Heatmap::hover`,
`Heatmap::scalar-bar`, `BarChart::label`, and `BarChart::value-label` for
readability and theme integration.
""",
    )
    plots = _section(
        "plots",
        "Plots And Charts",
        "GPU plots, charts, and data visualizations.",
        """
Available plot widgets:
- `LinePlot(frame, y="loss", x=None, colors=None)` with `set_data`,
  `append_points`, `clear`, `fit`, and toolbar/grid/axes controls.
- `Scatter3D(frame, x, y, z, color=None, colormap="viridis")` with GPU point
  clouds, labels, overlays, meshes, streams, picking, and camera controls.
- `ScatterPlot2D(frame, x, y, ...)` uses the scatter backend in fixed 2D mode.
- `Histogram`, `BarChart`, `Heatmap`, and `PieChart`.

Performance rule: for large or streaming data, prefer packed/live methods such
as `prepare_points`, `set_prepared_points`, `create_live_frame`, or
`append_points` instead of rebuilding the widget tree.
""",
        [line_plot, scatter_plot, charts],
    )
    feedback = _section(
        "feedback",
        "Feedback Widgets",
        "Status indicators and non-blocking user feedback.",
        """
Use `Badge`, `Tag`, `LED`, `ProgressBar`, `LoadingSpinner`, `LogView`,
`Tooltip`, `Modal`, and `toast` for feedback.

Use semantic levels (`neutral`, `info`, `success`, `warning`, `danger`,
`error`) where available so themes can control color.

Use `LoadingSpinner(spinning=True)` for indeterminate work, `ProgressBar` for
known progress, `toast` for transient feedback, and `LogView` for persistent
operational records.
""",
    )
    media = _section(
        "media",
        "Media And Rich Text",
        "Images, HTML reports, code editors, and logs.",
        """
Use `Image(path, fit="contain")` for local images and `HtmlReport.from_html`
for report content. Use `CodeEditor` for source-like text and `LogView` for
append-only operational logs.

`Image.fit` accepts `contain`, `cover`, or `stretch`. `HtmlReport` can be
created from a local path or raw HTML; prefer local resources for reliable
native app behavior.
""",
    )
    widgets = _section(
        "widgets",
        "Widgets",
        "Catalog of common controls and data widgets.",
        "Most widgets accept `id`, `key`, `class_`, `style`, `tooltip`, and `parent`.",
        [inputs, navigation, tables, plots, feedback, media],
    )

    selectors = _section(
        "selectors",
        "CSS Selectors",
        "Supported selector patterns for styling widgets and parts.",
        """
Stylesheets are added with `app.stylesheet(css)` or `app.load_stylesheet(path)`.

Supported selector patterns include:
- Type selectors: `Button`, `Panel`, `NumberInput`
- Class selectors: `.primary`, `Button.primary`
- States: `:hover`, `:focus`, `:active`, `:disabled`, `:checked`, `:selected`
- Parts: `NumberInput::stepper-up`, `Dropdown::item-selected`
- Child/descendant patterns where supported by the CSS engine.

Additional supported forms include `#id`, attribute selectors for stable widget
metadata, `:open`, `:expanded`, `:collapsed`, structural child selectors, and
selector functions such as `:not(...)`, `:is(...)`, `:where(...)`, and the
supported subset of `:has(...)`.
""",
    )
    parts = _section(
        "parts",
        "CSS Parts",
        "Named sub-elements exposed by composite widgets.",
        """
Common parts:
- Scroll containers often support `::scrollbar-track` and `::scrollbar-thumb`.
- `Panel::accent`
- `Collapsible::header`, `indicator`, `body`
- `Modal::scrim`
- `Menu::menu`, `item`, `item-hover`, `item-disabled`
- `ContextMenu::menu`, `item`, `item-hover`, `item-disabled`
- `Button::badge`, `SmallButton::badge`
- `IconButton::icon`, `ImageButton::image`, `ArrowButton::icon`
- `NumberInput::field`, `stepper`, `stepper-up`, `stepper-down`,
  `stepper-divider`, `divider`, `caret`
- `CodeEditor::field`, `gutter`, `line-number`, `caret`
- `LogView::line`, `debug`, `info`, `warning`, `error`
- `DragNumber::field`, `value`, `grip`
- `Dropdown::field`, `chevron`, `menu`, `item`, `item-selected`, `item-hover`
- `Checkbox::row`, `box`, `indicator`, `label`
- `ToggleSwitch::row`, `track`, `thumb`, `label`
- `TreeNode::row`, `indicator`, `label`, `guide`
- `LED::dot`, `glow`, `highlight`
- `Slider::track`, `fill`, `thumb`
- `RangeSlider::track`, `range`, `thumb-min`, `thumb-max`, `label`
- `ProgressBar::track`, `fill`, `label`
- `LoadingSpinner::track`, `arc`, `label`
- `Heatmap::cell`, `grid`, `hover`, `scalar-bar`, `label`
- `BarChart::label`, `value-label`
- `Tab::tab`, `accent`, `badge`
- `NavItem::item`, `accent`, `badge`
- `DataFrameTable::header`, `row`, `row-selected`, `grid-line`,
  `scrollbar-track`, `scrollbar-thumb`

Inline style part names accept dashed or snake-case names.
""",
    )
    properties = _section(
        "properties",
        "CSS Properties",
        "Common supported layout, visual, and text style properties.",
        """
Common layout properties:
`display`, `width`, `height`, `min-width`, `min-height`, `max-width`,
`max-height`, `padding`, `margin`, `gap`, `row-gap`, `column-gap`,
`flex-grow`, `flex-shrink`, `overflow`, `overflow-x`, `overflow-y`,
`align-items`, `justify-content`, grid track properties.

Common visual properties:
`background`, `background-color`, gradients/background paint,
`border`, `border-color`, `border-width`, `border-radius`,
per-corner radius, `outline`, `outline-color`, `outline-width`,
`outline-offset`, `opacity`, `box-shadow`.

Common text properties:
`color`, `font-size`, `font-family`, `font-weight`, `text-align`,
`line-height`, and wrapping-related fields where widgets support them.

Use snake-case keys in Python inline styles or CSS property names in
stylesheets. Examples: `{"border_radius": 8}` and `border-radius: 8px;`.
""",
    )
    media_queries = _section(
        "queries",
        "Media, Container, Supports, And Fonts",
        "Advanced stylesheet features available in the native CSS subset.",
        """
DragonGUI supports a native subset of:
- `@media` rules for viewport/scale conditions.
- Container queries for width/inline-size conditions.
- `@supports` declarations and selector capability checks.
- `@font-face` for supported local font formats.

Use these sparingly for reusable app themes. Keep widget-local sizing in inline
style when the value is semantically tied to the widget instance.
""",
    )
    themes = _section(
        "themes",
        "Themes",
        "Theme tokens and style conventions.",
        """
Themes provide colors and spacing tokens. CSS can reference tokens such as
`background`, `surface`, `surface_alt`, `text`, `muted_text`, `border`,
`accent`, `success`, `warning`, `danger`, `focus`, and `disabled`.

Prefer token-based styles for reusable widgets. Use direct colors only for
domain-specific visual encodings.

Theme fields accept CSS color strings, RGB/RGBA sequences, and token-like values
where the style parser supports them.
""",
    )
    styling = _section(
        "styling",
        "Styling",
        "CSS subset, inline styles, widget parts, and theme tokens.",
        """
Use inline `style={...}` for one-off layout or widget-local tweaks. Use
stylesheets for app-wide visual systems.

```python
app.stylesheet(\"\"\"
Panel { padding: 14px; gap: 10px; border-radius: 8px; }
Button.primary { background: accent; color: background; }
NumberInput::stepper { background: surface_alt; color: text; }
\"\"\")
```
""",
        [selectors, parts, properties, media_queries, themes],
    )

    callbacks = _section(
        "callbacks",
        "Callbacks",
        "Event callbacks and value-change behavior.",
        """
Common callback names:
- `Button(..., on_click=fn)`
- `TextInput`, `TextArea`, `NumberInput`, `Slider`, `Dropdown` use `on_change`
- `Checkbox` and `ToggleSwitch` use `on_change(bool)`
- `DataFrameTable(..., on_select=fn)`
- `Scatter3D(..., on_pick=fn)`

Callbacks run on the GUI/runtime thread bridge. Keep them short. For expensive
work, start a worker thread and schedule UI updates through `app.call_soon_threadsafe`.

Callback compatibility:
- Table selection can receive a `TableSelection`, or compatible positional
  row/column/value forms.
- Scatter picking can receive a `ScatterPick`, or compatible index/x/y/z forms.
- Command palette callbacks receive the selected `Command` when using `on_run`.
""",
    )
    live_value_updates = _section(
        "values",
        "Value Updates",
        "Common live setters for ordinary widgets.",
        """
Use:
- `Label.set_value(text)`
- `TextInput.set_value(text)`, `TextArea.set_value(text)`
- `NumberInput.set_value(value)`, `Slider.set_value(value)`,
  `RangeSlider.set_value((min_value, max_value))`
- `Checkbox.set_checked(bool)`, `ToggleSwitch.set_checked(bool)`
- `Dropdown.set_value(value)`
- `Badge.set_value(value)`, `Badge.set_level(level)`
- `LED.set_state(state)`, `LED.set_on(bool)`, `LED.set_color(color)`
- `ProgressBar.set_value(value)`
- `Modal.show()`, `Modal.close()`
""",
    )
    live_data_updates = _section(
        "data",
        "Data Updates",
        "Live update paths for tables and plots.",
        """
Use:
- `DataFrameTable.set_frame(frame)`
- `LinePlot.set_data(...)`, `LinePlot.append_points(...)`, `LinePlot.clear(...)`
- `Scatter3D.set_points(...)`, `Scatter3D.set_prepared_points(...)`,
  `Scatter3D.enqueue_prepared_points(...)`, `Scatter3D.create_live_frame(...)`
- Chart-specific setters such as `Heatmap.set_data(...)`, `BarChart.fit()`,
  `Histogram.fit()`

Keep large data on the dedicated widget methods so the native backend can use
packed resource paths and avoid full document rebuilds.
""",
    )
    live_threads = _section(
        "threads",
        "Thread-Safe Updates",
        "Updating live widgets from worker threads.",
        """
Pattern:
```python
def worker():
    while running:
        xs, ys = read_batch()
        app.call_soon_threadsafe(lambda: plot.append_points("loss", xs, ys))
```

Guidelines:
- Do not call slow training/data code directly in GUI callbacks.
- Coalesce high-rate producer updates before scheduling UI work.
- Use live frame streams for high-volume scatter replacement.
- Prefer latest-frame semantics when old frames are not useful.
""",
    )
    live_updates = _section(
        "live_updates",
        "Live Updates",
        "Updating widgets after the app is running.",
        """
Live widgets bind native handles during `app.run`. Methods enqueue native
commands without rebuilding the whole document.

Examples:
```python
progress.set_value(0.42)
table.set_frame(df)
plot.append_points("loss", xs, ys)
scatter.set_points(df, x="x", y="y", z="z", fit=True)
modal.show()
toast_handle.update("Done", level="success")
```
""",
        [live_value_updates, live_data_updates, live_threads],
    )
    components = _section(
        "components",
        "Components",
        "Reactive Python component model for reusable UI.",
        """
Use `@dg.component` for stateful reusable views. State is keyed, not positional.

```python
@dg.component
def Tool(ctx, frame):
    selected = ctx.state("selected", "loss")
    return dg.Panel(children=[
        dg.Dropdown(["loss", "accuracy"], value=selected.value, on_change=selected.set),
        dg.LinePlot(frame, y=selected.value),
    ])
```

Use stable `key` values for children whose live state should survive rerenders.
""",
    )

    validation_data = _section(
        "data_shapes",
        "Data Shape Validation",
        "Common table, plot, and frame validation failures.",
        """
Rules:
- `DataFrameTable.page_size` must be greater than zero and sample rows cannot
  be negative.
- `LinePlot` x/y arrays must have matching lengths; multi-series labels,
  colors, and line styles must match the number of y series.
- `Scatter3D` needs x/y/z columns or N x 3 point arrays. Streaming updates must
  use the same column shape expected by the stream.
- `ScatterPlot2D` needs x/y columns and valid 2D or compatible fit bounds.
- `Heatmap` needs a rectangular non-empty 2D numeric matrix; label lengths must
  match matrix rows/columns.
- `BarChart` values must match labels and grouped values must be shaped
  `(series, labels)`.
- `Histogram` bin edges must be finite, strictly increasing, and contain at
  least two values.

When generated code fails here, inspect the input shape before changing layout
or styling.
""",
    )
    validation_ranges = _section(
        "ranges",
        "Numeric Range Validation",
        "Common scalar/range validation failures.",
        """
Rules:
- Numeric values used by `NumberInput`, `DragNumber`, sliders, progress bars,
  and plot limits must be finite.
- `max` must be greater than or equal to `min`.
- `step`, drag `speed`, sizes, and dimensions must be positive when required.
- `RangeSlider` values must contain exactly two values.
- `DragVector` supports one to four finite components and labels must match
  component count.
- `ProgressBar` values are clamped to its min/max range; low values should not
  rely on negative padding or overflowing rounded fills.
""",
    )
    validation_choices = _section(
        "choices",
        "Choice And Id Validation",
        "Common option-list, route, id, and empty-choice validation errors.",
        """
Rules:
- `Dropdown`, `RadioGroup`, and `SelectableList` items cannot be empty.
- Selected values must match one of the item values.
- Item values and command ids must be unique where the widget models identity.
- `Tabs` and `Pages` values must be valid and duplicate routes are rejected.
- `Sidebar` nav items need non-empty page targets.
- `Menu`, `MenuItem`, `Command`, `TreeNode`, and breadcrumbs require non-empty
  labels/ids where applicable.
""",
    )
    validation_css = _section(
        "css",
        "CSS And Style Validation",
        "Common style, part, and color validation failures.",
        """
Rules:
- Inline `style` must be a mapping.
- `class_` and `key` must be non-empty strings.
- Stylesheets must be non-empty strings.
- Unknown CSS parts raise clear errors, for example styling `NumberInput::thumb`
  when that widget has no `thumb` part.
- Theme/loading/toast colors must be valid strings or RGB/RGBA tuples.
- Use `dg.help.reference.css_parts()` and `dg.help.reference.css_properties()`
  before inventing selectors or browser CSS.
""",
    )
    validation_custom = _section(
        "custom_widgets",
        "Custom Widget Validation",
        "Common ExtensionWidget, PaintWidget, and PaintContext validation failures.",
        """
Rules:
- `ExtensionWidget.extension_type` must be non-empty.
- `ExtensionWidget.props` must be JSON-compatible.
- `PaintWidget.measure(...)` must return a positive finite `Size`.
- `MeasureConstraints` min values cannot exceed max values.
- `PaintContext.polyline(...)` requires coordinate pairs.
- `PaintContext.text(..., align=...)` accepts only supported alignment values.
- `PaintContext.image(..., fit=...)` accepts `contain`, `cover`, or `stretch`.
- Rectangle outlines use `stroke_width`, not `width`; line/polyline strokes use
  `width`.
""",
    )
    validation_runtime = _section(
        "runtime",
        "Runtime Validation",
        "Common app, window, callback, and thread-safety validation failures.",
        """
Rules:
- Create widgets while a window/container context is active, or pass an explicit
  `parent`.
- Do not call live widget methods after the widget has been removed.
- Call UI mutation APIs from the GUI thread, or use `app.call_soon_threadsafe`
  from worker threads.
- Toast updates require a running app and a valid toast handle.
- Dialog helpers require the native backend and a running app context.
- Drag/drop payload ids and accepted type names must be non-empty strings.
""",
    )
    validation = _section(
        "validation",
        "Validation And Errors",
        "Common constructor, data, style, and custom-widget errors.",
        """
Use this branch when generated code raises `ValueError` or `TypeError` before
the window opens. Most DragonGUI validation errors are intentional guardrails:
they prevent state drift and catch layout/data mistakes early.
""",
        [
            validation_data,
            validation_ranges,
            validation_choices,
            validation_css,
            validation_custom,
            validation_runtime,
        ],
    )
    validation.aliases.update(
        {
            "numeric_ranges": validation_ranges,
            "choices_ids": validation_choices,
            "ids": validation_choices,
            "styles": validation_css,
            "extensions": validation_custom,
            "paint": validation_custom,
            "threads": validation_runtime,
        }
    )

    perf_callbacks = _section(
        "callbacks",
        "Callback Performance",
        "Keep event callbacks short and schedule slow work safely.",
        """
Rules:
- GUI callbacks should update state, enqueue work, or call live setters.
- Do not train models, scan files, download data, or pack huge arrays directly
  in callbacks.
- Use worker threads for slow work and `app.call_soon_threadsafe(...)` to
  update widgets.
- Coalesce high-rate producers before scheduling UI updates.
- Prefer latest-frame semantics when stale frames are not useful.
""",
    )
    perf_tables = _section(
        "tables",
        "Table Performance",
        "Use bounded table resources for large frames.",
        """
Rules:
- Use `DataFrameTable.set_frame(frame)` for live replacement.
- Keep `page_size` bounded.
- Avoid wrapping a table in extra scrolling containers unless intentional.
- Prefer column-buffer paths for numeric data instead of formatting millions of
  Python cells.

Relevant probes: `data_table_upgrades_probe.py`,
`layout_scrollable_composites_probe.py`.
""",
        metadata={
            "probes": [
                "examples/css_feature_probes/data_table_upgrades_probe.py",
                "examples/css_feature_probes/layout_scrollable_composites_probe.py",
            ]
        },
    )
    perf_plots = _section(
        "plots",
        "Plot Performance",
        "Fast update paths for line, scatter, heatmap, and chart widgets.",
        """
Rules:
- `LinePlot`: use `append_points(...)` or prepared payload replacement; batch
  samples before calling into the UI.
- `Scatter3D`/`ScatterPlot2D`: use packed/prepared payloads for dense data.
- `Scatter3D`: use `create_live_frame(...)`, `enqueue_prepared_points(...)`,
  LOD, and auto quality for repeated high-count replacement.
- `Heatmap`/charts: update data through widget methods rather than rebuilding
  surrounding layout.
- Re-run benchmark probes after visual changes that touch labels, toolbars,
  smoothing, or point density.

Relevant probes: `line_plot_stream_benchmark_probe.py`,
`scatter3d_frame_benchmark_probe.py`, `layout_plot_embedding_probe.py`.
""",
        metadata={
            "probes": [
                "examples/css_feature_probes/line_plot_stream_benchmark_probe.py",
                "examples/css_feature_probes/scatter3d_frame_benchmark_probe.py",
                "examples/css_feature_probes/layout_plot_embedding_probe.py",
            ]
        },
    )
    perf_line_plots = _section(
        "line_plots",
        "Line Plot Performance",
        "Fast paths for streaming and repeatedly updated line plots.",
        """
Rules:
- Create the `LinePlot` once and update it with `append_points(...)`,
  `set_series(...)`, or prepared series payloads.
- Batch samples before crossing the Python/native boundary.
- Keep x/y series lengths aligned and avoid per-point widget updates.
- Use fixed visible windows or decimation for long-running streams.
- Disable expensive visual options first when benchmarking, then re-enable
  smoothing/labels only after measuring.
""",
        metadata={
            "probes": [
                "examples/css_feature_probes/line_plot_stream_benchmark_probe.py",
                "examples/all_features_v3_demo.py",
            ]
        },
    )
    perf_scatter = _section(
        "scatter",
        "Scatter Performance",
        "Fast paths for dense 2D and 3D scatter plots.",
        """
Rules:
- Prefer prepared payloads for dense point clouds.
- For `Scatter3D`, use `prepare_points`, `set_prepared_points`,
  `enqueue_prepared_points`, `create_live_frame`, LOD, and auto quality.
- Avoid rebuilding surrounding panels when only point data changes.
- Keep scalar color arrays and point arrays the same length.
- Re-fit only when bounds or data domain changes; do not fit every frame unless
  that interaction is intentional.
""",
        metadata={
            "probes": [
                "examples/css_feature_probes/scatter_plot_2d_probe.py",
                "examples/css_feature_probes/scatter3d_dense_probe.py",
                "examples/css_feature_probes/scatter3d_frame_benchmark_probe.py",
            ]
        },
    )
    perf_paint_widgets = _section(
        "paint_widgets",
        "Paint Widget Performance",
        "Fast paths for custom PaintWidget rendering.",
        """
Rules:
- Keep `paint(ctx)` deterministic and cheap.
- Call `repaint()` only when display-list state changes.
- Prefer a small number of batched primitives over many tiny display-list items.
- Cache text, geometry, and normalized values in Python object state when they
  do not change every frame.
- Use pointer/key callbacks to update state and schedule repaint, not to do slow
  work.
""",
        metadata={
            "probes": [
                "examples/css_feature_probes/paint_widget_sparkline_probe.py",
                "examples/css_feature_probes/paint_widget_events_probe.py",
            ]
        },
    )
    perf_layout_scroll = _section(
        "layout_scroll",
        "Layout And Scroll Performance",
        "Avoid layout churn and scroll jank in large composite UIs.",
        """
Rules:
- Keep one scroll owner per region.
- Bound tables, logs, code editors, plots, and nested panels with height or
  flex constraints.
- Use `GridLayout(masonry=True)` for variable-height cards instead of
  row-aligned cards with blank space.
- Avoid changing fixed sizes every frame; update content values instead.
- Use layout stress probes when a new composite widget clips, overlaps, or
  creates duplicate scrollbars.
""",
        metadata={
            "probes": [
                "examples/css_feature_probes/layout_flex_stress_probe.py",
                "examples/css_feature_probes/layout_scrollable_composites_probe.py",
                "examples/css_feature_probes/v4_scroll_benchmark_probe.py",
            ]
        },
    )
    perf_layout = _section(
        "layout",
        "Layout Performance",
        "Avoid expensive layout churn and scroll conflicts.",
        """
Rules:
- Avoid rebuilding large widget subtrees every frame.
- Use `GridLayout(masonry=True)` for variable-height cards instead of forcing
  row-aligned card heights.
- Keep one scroll owner per region.
- Avoid unbounded nested panels around large plots/tables/logs.
- Use live methods and CSS class changes for targeted updates.
""",
    )
    perf_custom = _section(
        "custom_widgets",
        "Custom Widget Performance",
        "Keep PaintWidget and ExtensionWidget updates cheap.",
        """
Rules:
- Keep `PaintWidget.paint(ctx)` deterministic and fast.
- Call `repaint()` only when display-list state changes.
- Prefer theme tokens and simple primitives for reusable custom widgets.
- For high-frequency custom widgets, benchmark display-list serialization and
  consider resource-backed data paths before sending very large display lists.
- Keep extension pointer/key callbacks lightweight.

Relevant probes: `paint_widget_sparkline_probe.py`,
`paint_widget_events_probe.py`.
""",
        metadata={
            "probes": [
                "examples/css_feature_probes/paint_widget_sparkline_probe.py",
                "examples/css_feature_probes/paint_widget_events_probe.py",
            ]
        },
    )
    performance = _section(
        "performance",
        "Performance",
        "Patterns for large data, streaming, and smooth interaction.",
        """
Performance guidance:
- Python should not rebuild large widget trees every frame.
- Use live setters and packed data paths for plots/tables.
- For `Scatter3D`, use `prepare_points`, `set_prepared_points`,
  `enqueue_prepared_points`, `create_live_frame`, LOD, and auto quality.
- For `LinePlot`, use `append_points` or prepared series payloads.
- Use `DataFrameTable.set_frame` instead of replacing surrounding layout.
- Avoid nested unconstrained scroll containers.
- Use `app.debug_snapshot()` and benchmark probes to inspect frame timings.
""",
        [
            perf_callbacks,
            perf_tables,
            perf_line_plots,
            perf_scatter,
            perf_paint_widgets,
            perf_layout_scroll,
            perf_plots,
            perf_layout,
            perf_custom,
        ],
    )
    performance.aliases.update(
        {
            "line_plot": perf_line_plots,
            "lineplot": perf_line_plots,
            "scroll": perf_layout_scroll,
            "scrolling": perf_layout_scroll,
            "paint": perf_paint_widgets,
        }
    )
    dashboard_recipe = _section(
        "dashboard",
        "Dashboard Recipe",
        "A stable multi-panel dashboard layout.",
        """
Pattern:
```python
win = dg.Window("Dashboard", width=1280, height=820)
with dg.VLayout(style={"gap": 12, "padding": 12}):
    with dg.HLayout(style={"gap": 12}):
        with dg.Panel("Controls", width=320): ...
        with dg.VLayout(style={"flex_grow": 1, "gap": 12}):
            with dg.GridLayout(columns=2, min_column_width=320, gap=12):
                dg.Panel("Metric A")
                dg.Panel("Metric B")
            dg.LinePlot(frame, y=["loss", "accuracy"], show_toolbar=True)
app.run(win)
```

Use one primary scroll owner if total content can exceed the viewport. Use
`GridLayout(..., masonry=True)` when card heights differ.

Related example: `examples/all_features_v3_demo.py`.
""",
    )
    form_recipe = _section(
        "form",
        "Form Recipe",
        "Building clean controls and inspector panels.",
        """
Prefer `PropertyGrid` for schema-driven settings and `Property` for hand-built
rows.

Manual row:
```python
with dg.Property("Learning rate", dg.NumberInput(0.001, step=0.0001, parent=None)):
    pass
```

For custom rows, keep labels fixed-width and editors flexible:
```python
with dg.HLayout(style={"gap": 8, "align_items": "center"}):
    dg.Label("Batch size", style={"width": 120})
    dg.NumberInput(64, min=1, step=1, style={"flex_grow": 1})
```

Related probe: `examples/css_feature_probes/property_grid_probe.py`.
""",
    )
    plotting_recipe = _section(
        "plotting",
        "Plotting Recipe",
        "Choosing the right plot widget and update path.",
        """
Choose:
- `LinePlot` for time series, training curves, sensors, and streaming lines.
- `ScatterPlot2D` for dense 2D point clouds.
- `Scatter3D` for real 3D point clouds and scenes.
- `Histogram` for distributions.
- `BarChart` for categorical totals/rankings.
- `Heatmap` for matrices.
- `PieChart` for composition summaries.

For streaming, create the plot once and call live update methods. Do not
replace the entire panel each frame.

Related probes: `examples/css_feature_probes/layout_plot_embedding_probe.py`,
`examples/css_feature_probes/scatter_plot_2d_probe.py`.
""",
    )
    app_shell_recipe = _section(
        "app_shell",
        "App Shell Recipe",
        "Menu/body/status layout for full applications.",
        """
Pattern:
```python
win = dg.Window("Workbench", width=1280, height=820)
with dg.VLayout(style={"height": "100%", "gap": 8, "padding": 10}):
    with dg.MenuBar():
        with dg.Menu("File"):
            dg.MenuItem("Open", on_click=open_file)
    with dg.HLayout(style={"flex_grow": 1, "gap": 10, "min_height": 0}):
        with dg.Sidebar("Navigation", width=220): ...
        with dg.ScrollArea(style={"flex_grow": 1, "min_height": 0}): ...
    with dg.StatusBar():
        dg.Label("Ready")
```

Keep one primary scroll owner in the body. Give the body `flex_grow` and
`min_height: 0` so it fits the window instead of pushing below the status bar.

Related example: `examples/all_features_v3_demo.py`.
""",
    )
    settings_panel_recipe = _section(
        "settings_panel",
        "Settings Panel Recipe",
        "Inspector and settings forms with stable label/editor sizing.",
        """
Use `PropertyGrid` for schema-driven panels and `Property` rows for manual
layouts. Editors should usually get `flex_grow: 1` and a sane `min_width`.

```python
with dg.Panel("Inspector", width=360):
    dg.PropertyGrid({
        "enabled": dg.Checkbox("Enabled", checked=True, parent=None),
        "lr": dg.NumberInput(0.001, step=0.0001, parent=None),
    })
```

For custom rows, align to center and avoid fixed widths that exceed the panel.

Related probe: `examples/css_feature_probes/property_grid_probe.py`.
""",
    )
    streaming_line_recipe = _section(
        "streaming_line_plot",
        "Streaming Line Plot Recipe",
        "High-rate line plotting without rebuilding the layout.",
        """
Create the plot once, then append or replace series data through live methods:

```python
plot = dg.LinePlot(frame, x="step", y=["loss", "accuracy"], show_toolbar=True)

def on_batch(step, loss, accuracy):
    plot.append_points("loss", [step], [loss])
    plot.append_points("accuracy", [step], [accuracy])
```

`LinePlot.append_points(...)` is the common path for small streaming batches.
Batch high-rate samples before updating the UI. Keep smoothing/antialiasing
settings consistent with the target frame budget.

Related probe: `examples/css_feature_probes/layout_plot_embedding_probe.py`.
""",
    )
    streaming_scatter_recipe = _section(
        "streaming_scatter",
        "Streaming Scatter Recipe",
        "Packed scatter update flow for dense 2D and 3D point clouds.",
        """
Use prepared payloads for large or repeated scatter updates:

```python
scatter = dg.Scatter3D(frame, x="x", y="y", z="z", color="loss")
prepared = scatter.prepare_points(frame, x="x", y="y", z="z", color="loss")
scatter.set_prepared_points(prepared, fit=True)
```

For worker threads, enqueue prepared payloads with `app.call_soon_threadsafe`
or the scatter live-frame helpers. Avoid sending millions of Python objects per
frame.

Related probes: `examples/css_feature_probes/scatter_plot_2d_probe.py`,
`examples/css_feature_probes/scatter3d_dense_probe.py`.
""",
    )
    table_browser_recipe = _section(
        "data_table_browser",
        "Data Table Browser Recipe",
        "Scrollable, sortable table view with filtering controls.",
        """
Pattern:
```python
with dg.Panel("Data"):
    dg.SearchBox(placeholder="Filter rows")
    table = dg.DataFrameTable(frame, sortable=True, selectable=True)
```

Use `DataFrameTable.set_frame(...)` when filters change. Keep the table in a
bounded panel or scroll owner so the rest of the page remains stable.

Related probe: `examples/css_feature_probes/data_table_upgrades_probe.py`.
""",
    )
    drag_drop_recipe = _section(
        "drag_drop",
        "Drag Drop Recipe",
        "Lane and target layouts using DragonGUI drag/drop widgets.",
        """
Use `DragSource` for the draggable item and `DropTarget` or `DropZone` for
receivers. Use badges or ghost previews so users can see what is being dragged.

```python
dg.DragSource("metric:latency", children=[dg.Badge("Latency", parent=None)])
dg.DropTarget("lane-a", on_drop=handle_drop)
```

Keep drag rows in `FlowLayout` or flexible `HLayout` containers so badges do
not run into panel edges.

Related probe: `examples/css_feature_probes/drag_drop_probe.py`.
""",
    )
    command_palette_recipe = _section(
        "command_palette",
        "Command Palette Recipe",
        "Searchable command launcher with overlay behavior.",
        """
Define commands once and show the palette as an overlay:

```python
commands = [
    dg.Command("refresh", "Refresh data", on_run=refresh),
    dg.Command("export", "Export report", on_run=export),
]
palette = dg.CommandPalette(commands, open=False)
```

Use a close affordance on the modal/palette chrome and keep the search field
compact enough that result rows remain visible.

Related probe: `examples/css_feature_probes/command_palette_probe.py`.
""",
    )
    custom_composite_recipe = _section(
        "custom_composite",
        "Custom Composite Recipe",
        "Reusable third-party style widgets built from public primitives.",
        """
Prefer `@dg.component` for third-party widgets that can be composed from
existing DragonGUI primitives:

```python
@dg.component
def MetricTile(ctx, title, value, level="info"):
    return dg.Panel(children=[
        dg.Label(title, parent=None),
        dg.Badge(value, level=level, parent=None),
    ])
```

Use `ExtensionWidget` only when a renderer/native extension leaf is required.

Related probes: `examples/css_feature_probes/custom_composite_widget_probe.py`,
`examples/css_feature_probes/custom_properties_probe.py`.
""",
    )
    pytorch_dashboard_recipe = _section(
        "pytorch_dashboard",
        "PyTorch Dashboard Recipe",
        "Training dashboard structure based on `examples/pytorch_training_dashboard.py`.",
        """
Use an app-shell layout with:
- left-side run configuration and hyperparameters,
- central loss/accuracy plots,
- right-side metrics, hardware status, and logs,
- progress bars and status badges for training state.

Keep training work off the UI thread. Push metric updates through
`app.call_soon_threadsafe(...)`, `LinePlot.append_points(...)`,
`ProgressBar.set_value(...)`, `LogView.append(...)`, and table/label setters.

Related example: `examples/pytorch_training_dashboard.py`.
""",
    )
    recipes = _section(
        "recipes",
        "Recipes",
        "Copyable construction patterns for common application shapes.",
        "Use these sections as starting points when generating complete apps.",
        [
            dashboard_recipe,
            form_recipe,
            plotting_recipe,
            app_shell_recipe,
            settings_panel_recipe,
            streaming_line_recipe,
            streaming_scatter_recipe,
            table_browser_recipe,
            drag_drop_recipe,
            command_palette_recipe,
            custom_composite_recipe,
            pytorch_dashboard_recipe,
        ],
    )

    extensions = _section(
        "extensions",
        "Extension Widgets",
        "Current hooks for third-party/composite widgets and planned lower-level hooks.",
        """
Current extension path:
- Build reusable composites with `@dg.component`.
- Use `ExtensionWidget(extension_type=..., props=...)` for renderer/runtime
  extension leaves.
- Use `on_click` for simple extension click callbacks.
- Use `on_pointer_down`, `on_pointer_move`, `on_pointer_up`, and `on_wheel`
  when a custom leaf needs local pointer coordinates or wheel deltas.
- Use `on_key_down` for focused custom leaf keyboard input.
- Use `style`, `class_`, and CSS parts on the composite children or extension
  leaf.

Example:
```python
@dg.component
def StatusTile(ctx, title, value, level="info"):
    return dg.Panel(children=[
        dg.Label(title),
        dg.Badge(value, level=level),
    ])
```

V5 currently includes `PaintWidget`/`PaintContext` for native display-list
drawing plus basic pointer, wheel, click, and key-down callbacks. Drag
semantics and key-up events are still planned.

`ExtensionWidget` signature:
`ExtensionWidget(extension_type, props=None, intrinsic_width=None,
intrinsic_height=None, width=None, height=None, on_click=None,
on_pointer_down=None, on_pointer_move=None, on_pointer_up=None,
on_wheel=None, on_key_down=None, disabled=False, ...)`

`props` must be JSON-compatible. Use `intrinsic_width` and `intrinsic_height`
to tell layout how much natural space the extension leaf wants before CSS
width/height overrides.

For native display-list drawing, subclass `PaintWidget` and override
`measure(...)` plus `paint(ctx)`. Use `ctx.rect`, `ctx.rounded_rect`,
`ctx.line`, `ctx.polyline`, `ctx.circle`, `ctx.text`, and `ctx.image`, then call
`repaint()` after changing any state used by `paint(ctx)`.
""",
    )

    layout_decision = _section(
        "layout",
        "Layout Decision Guide",
        "How to choose the right container for a layout problem.",
        """
Use:
- `Panel` when content needs a titled/visual group.
- `VLayout` for ordinary stacked content.
- `HLayout` for rows, sidebars, and compact tool groups.
- `GridLayout` for card dashboards or fixed column structures.
- `GridLayout(masonry=True)` for cards with different natural heights.
- `FlowLayout` for chips, badges, and wrapping controls.
- `ScrollArea` when one region should own overflow.
- `Splitter`/`Pane` when the user should resize neighboring regions.

Avoid:
- fixed pixel widths everywhere,
- nested scroll owners unless the interaction requires it,
- using a `Panel` only to get padding when a layout container would be enough.
""",
    )
    data_viz_decision = _section(
        "data_visualization",
        "Data Visualization Decision Guide",
        "How to choose plot and table widgets.",
        """
Use:
- `LinePlot` for ordered x/y series, trends, and streaming time-series data.
- `ScatterPlot2D` for dense flat point clouds.
- `Scatter3D` only when depth, orbiting, or 3D axes matter.
- `Heatmap` for matrix/2D scalar fields.
- `Histogram` for distributions.
- `BarChart` for categorical comparisons.
- `DataFrameTable` for browsing rows/columns with selection and sorting.

Performance rules:
- Use packed/prepared payloads for large scatter data.
- Batch high-rate line-plot updates with `append_points`.
- Bound tables with `page_size` and a scroll owner.
""",
    )
    forms_decision = _section(
        "forms",
        "Forms Decision Guide",
        "How to choose form controls and schema-driven editors.",
        """
Use:
- `PropertyGrid` when editing a mapping or schema-driven settings object.
- manual `Label` + control rows when the form layout is highly custom.
- `NumberInput` for typed numeric entry.
- `DragNumber` for interactive tuning.
- `Slider`/`RangeSlider` for bounded continuous values.
- `Dropdown`, `RadioGroup`, or `SelectableList` for choices depending on
  available space and whether all options should be visible.

Keep forms in responsive rows with `flex_grow` rather than hard-coded widths.
""",
    )
    extension_decision = _section(
        "extensions",
        "Extension Decision Guide",
        "When to use components, extension leaves, and custom paint widgets.",
        """
Use:
- `@dg.component` for reusable widgets made from existing DragonGUI primitives.
- `ExtensionWidget` for a leaf owned by a native/third-party renderer or custom
  event surface.
- `PaintWidget` for pure-Python custom drawing through native display lists.

Prefer components first. Move to `PaintWidget` only when composition cannot
produce the needed visual or interaction with acceptable complexity.
""",
    )
    decisions = _section(
        "decisions",
        "Decision Guides",
        "Use-this-vs-that guidance for generating DragonGUI UIs.",
        "These guides help choose the smallest widget/layout abstraction that fits the task.",
        [layout_decision, data_viz_decision, forms_decision, extension_decision],
    )

    troubleshooting_clipping = _section(
        "clipping",
        "Clipping And Overflow",
        "Diagnose widgets cut off at panel edges or running outside parents.",
        """
Symptoms:
- controls run off the right edge,
- badges touch panel edges,
- plot labels or colorbar labels disappear,
- content draws into a neighboring panel.

Checks:
- Is the child in a bounded row with no room to shrink?
- Does the row need `flex_wrap`/`FlowLayout` instead of a single `HLayout`?
- Does the child need `flex_grow: 1`, `min_width: 0`, or a smaller fixed width?
- Is the content meant to scroll? Put it in one explicit `ScrollArea`.

Common fixes:
- Prefer `GridLayout` for card dashboards.
- Prefer `FlowLayout` for badges/chips/action rows.
- Give text-heavy fields room with `flex_grow`.
- Bound large widgets with a height and a scroll owner.

Relevant probes: `layout_flex_stress_probe.py`,
`layout_panel_bounds_probe.py`, `layout_plot_embedding_probe.py`.
""",
        metadata={
            "probes": [
                "examples/css_feature_probes/layout_flex_stress_probe.py",
                "examples/css_feature_probes/layout_panel_bounds_probe.py",
                "examples/css_feature_probes/layout_plot_embedding_probe.py",
            ]
        },
    )
    troubleshooting_scroll = _section(
        "scroll_owners",
        "Scroll Owners",
        "Diagnose nested scrollbars, dead scrollbars, and scroll content bleeding.",
        """
Symptoms:
- two scrollbars appear for one area,
- a scrollbar overlaps a status bar or footer,
- wheel input scrolls the wrong panel,
- text from a scroll body appears over the next panel.

Rules:
- Choose exactly one scroll owner for a region.
- Bound scroll owners with `height`, `flex_grow`, or a containing pane.
- Avoid wrapping a table/log/code editor in an extra scrolling container unless
  that wrapper is intentionally the scroll owner.
- For text/code widgets, use their own horizontal/vertical scroll behavior
  instead of clipping long text without a scroll path.

Relevant probes: `overflow_scrollbar_probe.py`,
`layout_scrollable_composites_probe.py`, `layout_panel_bounds_probe.py`.
""",
        metadata={
            "probes": [
                "examples/css_feature_probes/overflow_scrollbar_probe.py",
                "examples/css_feature_probes/layout_scrollable_composites_probe.py",
                "examples/css_feature_probes/layout_panel_bounds_probe.py",
            ]
        },
    )
    troubleshooting_flex = _section(
        "flex_sizing",
        "Flex Sizing",
        "Diagnose rows that waste space, refuse to shrink, or align unrelated panels.",
        """
Symptoms:
- a compact control leaves a large empty area,
- rows force card heights to match when masonry packing is desired,
- a child stretches wider than its visual content,
- a control's right-side border has unexpected empty space.

Rules:
- Use `flex_grow` only for widgets that should consume remaining space.
- Use `GridLayout(masonry=True)` for independent card heights.
- Use `FlowLayout` for flexible badges/chips.
- Prefer explicit compact widths for small controls and flexible width for text
  inputs or plot/table regions.

Relevant probes: `layout_flex_stress_probe.py`,
`layout_grid_masonry_probe.py`, `property_grid_probe.py`.
""",
        metadata={
            "probes": [
                "examples/css_feature_probes/layout_flex_stress_probe.py",
                "examples/css_feature_probes/layout_grid_masonry_probe.py",
                "examples/css_feature_probes/property_grid_probe.py",
            ]
        },
    )
    troubleshooting_overlays = _section(
        "overlays",
        "Overlays And Popups",
        "Diagnose popups that push layout, clip incorrectly, or cannot be closed.",
        """
Symptoms:
- modal/menu/palette pushes panels down,
- close affordance is duplicated or misplaced,
- overlay content is cut off and cannot scroll,
- tooltip hides underlying text rather than drawing above it.

Rules:
- Overlay widgets should be positioned above content, not inserted as normal
  layout flow.
- Modal bodies need a bounded scroll region when content can exceed the modal.
- Close buttons should be part of overlay chrome, not duplicate clear actions.
- Tooltip/hover labels should draw as overlays and avoid erasing underlying
  content unless the design explicitly calls for it.

Relevant probes: `layout_overlay_collision_probe.py`,
`overlay_stack_probe.py`, `menu_overlays_probe.py`.
""",
        metadata={
            "probes": [
                "examples/css_feature_probes/layout_overlay_collision_probe.py",
                "examples/css_feature_probes/overlay_stack_probe.py",
                "examples/css_feature_probes/menu_overlays_probe.py",
            ]
        },
    )
    troubleshooting_css = _section(
        "css",
        "CSS Styling Not Applying",
        "Diagnose styles that do not affect a widget or one of its parts.",
        """
Checks:
- Confirm the widget type selector in `dg.help.reference.css_selectors`.
- Confirm registered parts in `dg.help.reference.css_parts`.
- Use `class_="name"` plus `.name { ... }` for targeted styling.
- Use state selectors only from `dg.help.reference.css_states`.
- Remember that not every widget exposes every visual part.

Common fixes:
- Move style from a nonexistent part to the widget root.
- Add a CSS part in the framework when a real widget sub-region needs styling.
- Use theme tokens for reusable colors and literal colors only for data.

Relevant probes: `selectors_probe.py`, `color_syntax_probe.py`,
`border_outline_shadow_probe.py`.
""",
        metadata={
            "probes": [
                "examples/css_feature_probes/selectors_probe.py",
                "examples/css_feature_probes/color_syntax_probe.py",
                "examples/css_feature_probes/border_outline_shadow_probe.py",
            ]
        },
    )
    troubleshooting_alignment = _section(
        "alignment",
        "Text And Control Alignment",
        "Diagnose text or symbols that look off-center in buttons, badges, dropdowns, and inputs.",
        """
Symptoms:
- badge text is vertically off-center in one theme,
- dropdown item text sits too high or low,
- icon glyphs are too large or not centered,
- progress bar rounding spills outside the track at low values.

Checks:
- Compare line-height, padding, border width, and control height across styles.
- For icon controls, verify the icon coordinate system and glyph scale.
- For badges/buttons, prefer framework-level centering fixes over per-example
  padding hacks when the issue appears in multiple themes.

Relevant probes: `tool_buttons_probe.py`, `data_table_upgrades_probe.py`,
`pytorch_training_dashboard.py`.
""",
        metadata={
            "probes": [
                "examples/css_feature_probes/tool_buttons_probe.py",
                "examples/css_feature_probes/data_table_upgrades_probe.py",
            ],
            "examples": ["examples/pytorch_training_dashboard.py"],
        },
    )
    troubleshooting_plots = _section(
        "plots",
        "Plot Layout And Performance",
        "Diagnose plot labels, toolbar spacing, dense points, and streaming stutter.",
        """
Symptoms:
- colorbar/tick labels run off the card,
- toolbar buttons are clipped or too far from the plot,
- dense scatter points look segmented or disappear,
- streaming line plots stutter during updates.

Rules:
- Keep plot cards bounded and give label/colorbar regions explicit padding.
- Prefer packed/prepared payloads for dense scatter.
- Batch streaming line updates and keep callbacks short.
- Recheck performance with benchmark probes after visual fixes.

Relevant probes: `layout_plot_embedding_probe.py`,
`line_plot_stream_benchmark_probe.py`, `scatter3d_frame_benchmark_probe.py`.
""",
        metadata={
            "probes": [
                "examples/css_feature_probes/layout_plot_embedding_probe.py",
                "examples/css_feature_probes/line_plot_stream_benchmark_probe.py",
                "examples/css_feature_probes/scatter3d_frame_benchmark_probe.py",
            ]
        },
    )
    troubleshooting = _section(
        "troubleshooting",
        "Troubleshooting",
        "Practical diagnosis guides for layout, styling, overlays, and performance.",
        """
Start here when generated UI code looks visually wrong. These sections point to
the most likely framework/layout cause and the feature probes that isolate it.
""",
        [
            troubleshooting_clipping,
            troubleshooting_scroll,
            troubleshooting_flex,
            troubleshooting_overlays,
            troubleshooting_css,
            troubleshooting_alignment,
            troubleshooting_plots,
        ],
    )

    debugging = _section(
        "debugging",
        "Debugging And Tests",
        "Inspection, smoke testing, and common failure modes.",
        """
Useful tools:
- `app.debug_snapshot()` returns runtime, layout, style, command, and widget data.
- `python -m py_compile examples/foo.py` catches Python syntax errors.
- Set `DRAGONGUI_SMOKE_FRAMES=3` to run short GUI smoke checks.
- Feature probes in `examples/css_feature_probes` isolate widgets and layout cases.

When layout looks wrong, inspect the scroll owner, fixed sizes, `flex_grow`,
wrapping containers, and whether an overlay is being inserted as normal content.
""",
    )
    reference = _build_reference_sections()

    root = _section(
        "",
        "DragonGUI Built-In Manual",
        "LLM-oriented index for DragonGUI usage, APIs, layout rules, styling, and performance.",
        """
Usage:
- `dg.help()` returns this index.
- `dg.help.layout.panels()` returns a nested section.
- `dg.help("widgets.plots")` returns a section by path.
- `dg.help.search("scatter streaming")` returns matching section metadata.
- `dg.help.to_dict()` returns structured data for tool or LLM ingestion.
""",
        [
            llm_rules,
            quickstart,
            reference,
            app_model,
            layout,
            widgets,
            styling,
            callbacks,
            live_updates,
            components,
            validation,
            decisions,
            recipes,
            performance,
            extensions,
            troubleshooting,
            debugging,
        ],
    )

    root.aliases.update(
        {
            "panels": panels,
            "flex": flex,
            "flow": flow,
            "grids": grids,
            "grid": grids,
            "splitters": splitters,
            "splitter": splitters,
            "scroll": scroll,
            "overlays": overlays,
            "inputs": inputs,
            "forms": forms,
            "tables": tables,
            "plots": plots,
            "charts": plots,
            "scatter": scatter_plot,
            "line_plot": line_plot,
            "css": styling,
            "parts": parts,
            "themes": themes,
            "events": callbacks,
            "live": live_updates,
            "validation": validation,
            "errors": validation,
            "bad_inputs": validation,
            "reference": reference,
            "exports": reference.exports,
            "dialogs": reference.dialogs,
            "drag_drop": reference.drag_drop,
            "recipes": recipes,
            "decisions": decisions,
            "choose": decisions,
            "extensions": extensions,
            "troubleshooting": troubleshooting,
            "trouble": troubleshooting,
            "clipping": troubleshooting_clipping,
            "scrollbars": troubleshooting_scroll,
            "llm": llm_rules,
        }
    )
    return root


help = _build_manual()


__all__ = ["HelpSection", "help"]
